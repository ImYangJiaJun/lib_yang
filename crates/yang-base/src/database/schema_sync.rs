//! 基于模块 [`TableDefinition`] 的 MySQL 启动期保数据 schema 演进。
//!
//! 同步器创建缺失结构，并支持显式列改名、受控列修改、唯一索引、CHECK 与外键；
//! 永不自动删除未知列、索引或约束。任何可能被旧数据阻止的变更都会先只读扫描，
//! 返回表、对象和主键后拒绝全部 DDL。多实例启动使用同一 MySQL 会话持有 advisory
//! lock，DDL 中断后可按 information_schema 重新规划并幂等续作。

use super::DatabaseInitializer;
use crate::error::BaseError;
use crate::table::{
    CheckConfig, FieldConfig, FieldType, ForeignKeyConfig, SchemaColumn, SchemaIssueKind,
    TableConfig, TableDefinition,
};
use sqlx::mysql::MySqlConnection;
use sqlx::{Executor, FromRow};
use std::collections::BTreeMap;

const SCHEMA_LOCK_TIMEOUT_SECONDS: i64 = 30;

/// schema 同步变更类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SchemaSyncChangeKind {
    /// 创建整张表。
    CreatedTable,
    /// 增加字段。
    AddedColumn,
    /// 原位改名字段并保留全部数据。
    RenamedColumn,
    /// 在旧数据预检通过后修改字段类型、NULL 或默认值。
    ModifiedColumn,
    /// 增加主键。
    AddedPrimaryKey,
    /// 增加普通或唯一索引。
    AddedIndex,
    /// 增加 CHECK 约束。
    AddedCheck,
    /// 增加外键约束。
    AddedForeignKey,
}

/// 一项实际执行的 schema 变更。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SchemaSyncChange {
    /// 表名。
    pub table: String,
    /// 变更对象；建表时等于表名，其它变更为字段或索引名。
    pub object: String,
    /// 变更类型。
    pub kind: SchemaSyncChangeKind,
}

/// 启动期 schema 同步报告。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct SchemaSyncReport {
    /// 已检查的表名，按名称确定性排序。
    pub tables: Vec<String>,
    /// 本次实际执行的 additive 变更。
    pub changes: Vec<SchemaSyncChange>,
}

/// 一项阻止 Schema 更新的旧数据问题。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SchemaDataViolation {
    /// 问题表。
    pub table: String,
    /// 将要增加的约束或索引名。
    pub object: String,
    /// 命中的主键文本，最多返回 20 条且按主键排序。
    pub primary_keys: Vec<String>,
}

/// 只读 Schema 预检结果。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct SchemaPreflightReport {
    /// 待执行的结构变更。
    pub plan: SchemaSyncReport,
    /// 必须人工处理的旧数据问题。
    pub violations: Vec<SchemaDataViolation>,
}

impl SchemaPreflightReport {
    /// 是否可以安全进入 DDL 阶段。
    pub fn is_safe(&self) -> bool {
        self.violations.is_empty()
    }
}

impl SchemaSyncReport {
    /// 是否没有执行任何 DDL。
    pub fn is_noop(&self) -> bool {
        self.changes.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExistingIndex {
    name: String,
    unique: bool,
    columns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExistingCheck {
    name: String,
    expression: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExistingForeignKey {
    name: String,
    columns: Vec<String>,
    referenced_table: String,
    referenced_columns: Vec<String>,
    update_rule: String,
    delete_rule: String,
}

impl ExistingIndex {
    pub(super) fn new(name: impl Into<String>, unique: bool, columns: Vec<String>) -> Self {
        Self {
            name: name.into(),
            unique,
            columns,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExistingTableSchema {
    exists: bool,
    columns: Vec<SchemaColumn>,
    primary_key: Vec<String>,
    indexes: Vec<ExistingIndex>,
    checks: Vec<ExistingCheck>,
    foreign_keys: Vec<ExistingForeignKey>,
    has_rows: bool,
}

impl ExistingTableSchema {
    pub(super) fn missing() -> Self {
        Self {
            exists: false,
            columns: Vec::new(),
            primary_key: Vec::new(),
            indexes: Vec::new(),
            checks: Vec::new(),
            foreign_keys: Vec::new(),
            has_rows: false,
        }
    }

    pub(super) fn existing(
        columns: Vec<SchemaColumn>,
        primary_key: Vec<String>,
        indexes: Vec<ExistingIndex>,
    ) -> Self {
        Self {
            exists: true,
            columns,
            primary_key,
            indexes,
            checks: Vec::new(),
            foreign_keys: Vec::new(),
            has_rows: false,
        }
    }

    fn with_rows(mut self, has_rows: bool) -> Self {
        self.has_rows = has_rows;
        self
    }

    fn with_constraints(
        mut self,
        checks: Vec<ExistingCheck>,
        foreign_keys: Vec<ExistingForeignKey>,
    ) -> Self {
        self.checks = checks;
        self.foreign_keys = foreign_keys;
        self
    }
}

#[derive(Debug)]
pub(super) struct TableSyncPlan {
    pub(super) statements: Vec<String>,
    pub(super) changes: Vec<SchemaSyncChange>,
    preflight: Vec<SchemaPreflightCheck>,
}

#[derive(Debug)]
enum SchemaPreflightCheck {
    ColumnPredicate {
        object: String,
        predicate: String,
    },
    Check {
        object: String,
        expression: String,
    },
    ForeignKey {
        object: String,
        columns: Vec<String>,
        referenced_table: String,
        referenced_columns: Vec<String>,
    },
    UniqueIndex {
        object: String,
        columns: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DesiredIndex {
    name: String,
    unique: bool,
    columns: Vec<String>,
}

impl DatabaseInitializer {
    /// 只读取当前数据库 schema 并计算待执行变更，不获取同步锁、不执行 DDL。
    ///
    /// 适用于生产启动门禁：返回非空 `changes` 表示数据库尚未与定义对齐；不兼容
    /// 变更仍返回错误。调用方若要实际应用变更，应使用 [`Self::sync_table_definitions`]。
    pub async fn plan_table_definitions(
        &self,
        definitions: &[&TableDefinition],
    ) -> Result<SchemaSyncReport, BaseError> {
        let definitions = normalize_definitions(definitions)?;
        if definitions.is_empty() {
            return Ok(SchemaSyncReport::default());
        }
        let mut connection = self
            .db()
            .pool()
            .acquire()
            .await
            .map_err(|error| BaseError::DatabaseConnectionDbError(error.into()))?;
        let plans = plan_locked(&mut connection, &definitions).await?;
        Ok(SchemaSyncReport {
            tables: report_tables(&definitions),
            changes: plans.into_iter().flat_map(|plan| plan.changes).collect(),
        })
    }

    /// 只读预检结构计划及会阻止新约束生效的旧数据。
    pub async fn preflight_table_definitions(
        &self,
        definitions: &[&TableDefinition],
    ) -> Result<SchemaPreflightReport, BaseError> {
        let definitions = normalize_definitions(definitions)?;
        if definitions.is_empty() {
            return Ok(SchemaPreflightReport::default());
        }
        let mut connection = self
            .db()
            .pool()
            .acquire()
            .await
            .map_err(|error| BaseError::DatabaseConnectionDbError(error.into()))?;
        let plans = plan_locked(&mut connection, &definitions).await?;
        let violations = preflight_locked(&mut connection, &definitions, &plans).await?;
        Ok(SchemaPreflightReport {
            plan: SchemaSyncReport {
                tables: report_tables(&definitions),
                changes: plans
                    .iter()
                    .flat_map(|plan| plan.changes.iter().cloned())
                    .collect(),
            },
            violations,
        })
    }

    /// 同步一组不可变表定义，使用单个数据库级 advisory lock 串行化多实例启动。
    pub async fn sync_table_definitions(
        &self,
        definitions: &[&TableDefinition],
    ) -> Result<SchemaSyncReport, BaseError> {
        let definitions = normalize_definitions(definitions)?;
        if definitions.is_empty() {
            return Ok(SchemaSyncReport::default());
        }

        let mut connection = self
            .db()
            .pool()
            .acquire()
            .await
            .map_err(|error| BaseError::DatabaseConnectionDbError(error.into()))?;
        let database_name: Option<String> = sqlx::query_scalar("SELECT DATABASE()")
            .fetch_one(&mut *connection)
            .await
            .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
        let database_name = database_name.ok_or_else(|| {
            BaseError::DatabaseInitFailed("当前 MySQL 连接没有选定数据库".to_string())
        })?;
        let lock_name = schema_lock_name(&database_name);
        let acquired: Option<i64> = sqlx::query_scalar("SELECT GET_LOCK(?, ?)")
            .bind(&lock_name)
            .bind(SCHEMA_LOCK_TIMEOUT_SECONDS)
            .fetch_one(&mut *connection)
            .await
            .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
        if acquired != Some(1) {
            return Err(BaseError::DatabaseInitFailed(format!(
                "等待 schema 同步锁超时: {lock_name}"
            )));
        }

        let result = sync_locked(&mut connection, &definitions).await;
        let release_result: Result<Option<i64>, sqlx::Error> =
            sqlx::query_scalar("SELECT RELEASE_LOCK(?)")
                .bind(&lock_name)
                .fetch_one(&mut *connection)
                .await;

        match (result, release_result) {
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(BaseError::DatabaseQueryFailed(error.into())),
            (Ok(report), Ok(Some(1))) => Ok(report),
            (Ok(_), Ok(_)) => Err(BaseError::DatabaseInitFailed(format!(
                "schema 同步锁释放失败: {lock_name}"
            ))),
        }
    }
}

async fn sync_locked(
    connection: &mut MySqlConnection,
    definitions: &[&TableDefinition],
) -> Result<SchemaSyncReport, BaseError> {
    let mut report = SchemaSyncReport {
        tables: report_tables(definitions),
        changes: Vec::new(),
    };

    let plans = plan_locked(connection, definitions).await?;
    let violations = preflight_locked(connection, definitions, &plans).await?;
    if !violations.is_empty() {
        return Err(BaseError::DatabaseInitFailed(format_violations(
            &violations,
        )));
    }
    for foreign_key_phase in [false, true] {
        for plan in &plans {
            for (statement, change) in plan.statements.iter().zip(&plan.changes) {
                if (change.kind == SchemaSyncChangeKind::AddedForeignKey) != foreign_key_phase {
                    continue;
                }
                connection
                    .execute(sqlx::query(statement))
                    .await
                    .map_err(|error| BaseError::DatabaseExecuteFailed(error.into()))?;
                tracing::info!(
                    table = %change.table,
                    object = %change.object,
                    kind = ?change.kind,
                    "数据库 schema 已同步"
                );
                report.changes.push(change.clone());
            }
        }
    }

    Ok(report)
}

async fn preflight_locked(
    connection: &mut MySqlConnection,
    definitions: &[&TableDefinition],
    plans: &[TableSyncPlan],
) -> Result<Vec<SchemaDataViolation>, BaseError> {
    let mut violations = Vec::new();
    for (definition, plan) in definitions.iter().zip(plans) {
        let table = definition.config();
        for check in &plan.preflight {
            let primary_keys = preflight_primary_keys(connection, table, check).await?;
            if !primary_keys.is_empty() {
                violations.push(SchemaDataViolation {
                    table: table.table_name.clone(),
                    object: match check {
                        SchemaPreflightCheck::ColumnPredicate { object, .. }
                        | SchemaPreflightCheck::Check { object, .. }
                        | SchemaPreflightCheck::ForeignKey { object, .. }
                        | SchemaPreflightCheck::UniqueIndex { object, .. } => object.clone(),
                    },
                    primary_keys,
                });
            }
        }
    }
    Ok(violations)
}

async fn preflight_primary_keys(
    connection: &mut MySqlConnection,
    table: &TableConfig,
    check: &SchemaPreflightCheck,
) -> Result<Vec<String>, BaseError> {
    let quoted_table = quote_identifier(&table.table_name)?;
    let quoted_primary = quote_identifier(&table.primary_key)?;
    let statement = match check {
        SchemaPreflightCheck::ColumnPredicate { predicate, .. } => format!(
            "SELECT CAST({quoted_primary} AS CHAR) FROM {quoted_table} \
             WHERE {predicate} ORDER BY {quoted_primary} LIMIT 20"
        ),
        SchemaPreflightCheck::Check { expression, .. } => format!(
            "SELECT CAST({quoted_primary} AS CHAR) FROM {quoted_table} \
             WHERE NOT ({expression}) ORDER BY {quoted_primary} LIMIT 20"
        ),
        SchemaPreflightCheck::ForeignKey {
            columns,
            referenced_table,
            referenced_columns,
            ..
        } => {
            let joins = columns
                .iter()
                .zip(referenced_columns)
                .map(|(column, referenced)| {
                    Ok(format!(
                        "source.{} = target.{}",
                        quote_identifier(column)?,
                        quote_identifier(referenced)?
                    ))
                })
                .collect::<Result<Vec<_>, BaseError>>()?
                .join(" AND ");
            let non_null = columns
                .iter()
                .map(|column| Ok(format!("source.{} IS NOT NULL", quote_identifier(column)?)))
                .collect::<Result<Vec<_>, BaseError>>()?
                .join(" AND ");
            let first_target = quote_identifier(&referenced_columns[0])?;
            format!(
                "SELECT CAST(source.{quoted_primary} AS CHAR) FROM {quoted_table} AS source \
                 LEFT JOIN {} AS target ON {joins} \
                 WHERE {non_null} AND target.{first_target} IS NULL \
                 ORDER BY source.{quoted_primary} LIMIT 20",
                quote_identifier(referenced_table)?
            )
        }
        SchemaPreflightCheck::UniqueIndex { columns, .. } => {
            let quoted_columns = columns
                .iter()
                .map(|column| quote_identifier(column))
                .collect::<Result<Vec<_>, _>>()?;
            let group_columns = quoted_columns.join(", ");
            let join = quoted_columns
                .iter()
                .map(|column| format!("source.{column} = duplicates.{column}"))
                .collect::<Vec<_>>()
                .join(" AND ");
            let non_null = quoted_columns
                .iter()
                .map(|column| format!("{column} IS NOT NULL"))
                .collect::<Vec<_>>()
                .join(" AND ");
            format!(
                "SELECT CAST(source.{quoted_primary} AS CHAR) FROM {quoted_table} AS source \
                 INNER JOIN (SELECT {group_columns} FROM {quoted_table} \
                 WHERE {non_null} GROUP BY {group_columns} HAVING COUNT(*) > 1) AS duplicates \
                 ON {join} ORDER BY source.{quoted_primary} LIMIT 20"
            )
        }
    };
    sqlx::query_scalar::<_, String>(&statement)
        .fetch_all(&mut *connection)
        .await
        .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))
}

fn format_violations(violations: &[SchemaDataViolation]) -> String {
    let details = violations
        .iter()
        .map(|violation| {
            format!(
                "table={}, object={}, primary_keys=[{}]",
                violation.table,
                violation.object,
                violation.primary_keys.join(",")
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    format!("旧数据阻止数据库表结构更新，请人工处理后重试: {details}")
}

async fn plan_locked(
    connection: &mut MySqlConnection,
    definitions: &[&TableDefinition],
) -> Result<Vec<TableSyncPlan>, BaseError> {
    let mut plans = Vec::with_capacity(definitions.len());
    for definition in definitions {
        let existing = load_existing_schema(connection, definition).await?;
        let plan = plan_table_sync(definition, &existing)?;
        if plan.statements.len() != plan.changes.len() {
            return Err(BaseError::DatabaseInitFailed(format!(
                "表 {} 的 schema 计划内部不一致",
                definition.name()
            )));
        }
        plans.push(plan);
    }
    Ok(plans)
}

fn report_tables(definitions: &[&TableDefinition]) -> Vec<String> {
    definitions
        .iter()
        .map(|definition| definition.name().to_string())
        .collect()
}

async fn load_existing_schema(
    connection: &mut MySqlConnection,
    definition: &TableDefinition,
) -> Result<ExistingTableSchema, BaseError> {
    let table = definition.config();
    #[derive(FromRow)]
    struct ColumnRow {
        column_name: String,
        data_type: String,
        column_type: String,
        is_nullable: String,
        character_maximum_length: Option<i64>,
        column_default: Option<String>,
        extra: String,
    }

    #[derive(FromRow)]
    struct IndexRow {
        index_name: String,
        non_unique: i64,
        column_name: String,
    }

    #[derive(FromRow)]
    struct CheckRow {
        constraint_name: String,
        check_clause: String,
    }

    #[derive(FromRow)]
    struct ForeignKeyRow {
        constraint_name: String,
        column_name: String,
        referenced_table_name: String,
        referenced_column_name: String,
        update_rule: String,
        delete_rule: String,
    }

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name = ?",
    )
    .bind(&table.table_name)
    .fetch_one(&mut *connection)
    .await
    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
    if count == 0 {
        return Ok(ExistingTableSchema::missing());
    }

    let column_rows: Vec<ColumnRow> = sqlx::query_as(
        "SELECT CAST(COLUMN_NAME AS CHAR) AS column_name, CAST(DATA_TYPE AS CHAR) AS data_type, CAST(COLUMN_TYPE AS CHAR) AS column_type, CAST(IS_NULLABLE AS CHAR) AS is_nullable, CAST(CHARACTER_MAXIMUM_LENGTH AS SIGNED) AS character_maximum_length, CAST(COLUMN_DEFAULT AS CHAR) AS column_default, CAST(EXTRA AS CHAR) AS extra FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name = ? ORDER BY ORDINAL_POSITION",
    )
    .bind(&table.table_name)
    .fetch_all(&mut *connection)
    .await
    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
    let columns = column_rows
        .into_iter()
        .map(|row| {
            SchemaColumn::new(
                row.column_name,
                row.data_type,
                row.column_type,
                row.is_nullable.eq_ignore_ascii_case("YES"),
                row.character_maximum_length
                    .and_then(|length| u64::try_from(length).ok()),
                row.column_default,
            )
            .with_auto_increment(
                row.extra
                    .split_whitespace()
                    .any(|part| part.eq_ignore_ascii_case("auto_increment")),
            )
        })
        .collect();

    let primary_key: Vec<String> = sqlx::query_scalar(
        "SELECT CAST(COLUMN_NAME AS CHAR) FROM information_schema.statistics WHERE table_schema = DATABASE() AND table_name = ? AND index_name = 'PRIMARY' ORDER BY SEQ_IN_INDEX",
    )
    .bind(&table.table_name)
    .fetch_all(&mut *connection)
    .await
    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;

    let index_rows: Vec<IndexRow> = sqlx::query_as(
        "SELECT CAST(INDEX_NAME AS CHAR) AS index_name, CAST(NON_UNIQUE AS SIGNED) AS non_unique, CAST(COLUMN_NAME AS CHAR) AS column_name FROM information_schema.statistics WHERE table_schema = DATABASE() AND table_name = ? AND index_name <> 'PRIMARY' ORDER BY INDEX_NAME, SEQ_IN_INDEX",
    )
    .bind(&table.table_name)
    .fetch_all(&mut *connection)
    .await
    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
    let mut index_map: BTreeMap<String, (bool, Vec<String>)> = BTreeMap::new();
    for row in index_rows {
        let entry = index_map
            .entry(row.index_name)
            .or_insert_with(|| (row.non_unique == 0, Vec::new()));
        entry.1.push(row.column_name);
    }
    let indexes = index_map
        .into_iter()
        .map(|(name, (unique, columns))| ExistingIndex::new(name, unique, columns))
        .collect();

    let check_rows: Vec<CheckRow> = sqlx::query_as(
        "SELECT CAST(tc.CONSTRAINT_NAME AS CHAR) AS constraint_name, \
                CAST(cc.CHECK_CLAUSE AS CHAR) AS check_clause \
         FROM information_schema.TABLE_CONSTRAINTS AS tc \
         INNER JOIN information_schema.CHECK_CONSTRAINTS AS cc \
           ON cc.CONSTRAINT_SCHEMA = tc.CONSTRAINT_SCHEMA \
          AND cc.CONSTRAINT_NAME = tc.CONSTRAINT_NAME \
         WHERE tc.TABLE_SCHEMA = DATABASE() AND tc.TABLE_NAME = ? \
           AND tc.CONSTRAINT_TYPE = 'CHECK' \
         ORDER BY tc.CONSTRAINT_NAME",
    )
    .bind(&table.table_name)
    .fetch_all(&mut *connection)
    .await
    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
    let checks = check_rows
        .into_iter()
        .map(|row| ExistingCheck {
            name: row.constraint_name,
            expression: row.check_clause,
        })
        .collect();

    let foreign_key_rows: Vec<ForeignKeyRow> = sqlx::query_as(
        "SELECT CAST(kcu.CONSTRAINT_NAME AS CHAR) AS constraint_name, \
                CAST(kcu.COLUMN_NAME AS CHAR) AS column_name, \
                CAST(kcu.REFERENCED_TABLE_NAME AS CHAR) AS referenced_table_name, \
                CAST(kcu.REFERENCED_COLUMN_NAME AS CHAR) AS referenced_column_name, \
                CAST(rc.UPDATE_RULE AS CHAR) AS update_rule, \
                CAST(rc.DELETE_RULE AS CHAR) AS delete_rule \
         FROM information_schema.KEY_COLUMN_USAGE AS kcu \
         INNER JOIN information_schema.REFERENTIAL_CONSTRAINTS AS rc \
           ON rc.CONSTRAINT_SCHEMA = kcu.CONSTRAINT_SCHEMA \
          AND rc.CONSTRAINT_NAME = kcu.CONSTRAINT_NAME \
         WHERE kcu.TABLE_SCHEMA = DATABASE() AND kcu.TABLE_NAME = ? \
           AND kcu.REFERENCED_TABLE_NAME IS NOT NULL \
         ORDER BY kcu.CONSTRAINT_NAME, kcu.ORDINAL_POSITION",
    )
    .bind(&table.table_name)
    .fetch_all(&mut *connection)
    .await
    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
    let mut foreign_key_map = BTreeMap::<String, ExistingForeignKey>::new();
    for row in foreign_key_rows {
        let foreign_key = foreign_key_map
            .entry(row.constraint_name.clone())
            .or_insert_with(|| ExistingForeignKey {
                name: row.constraint_name,
                columns: Vec::new(),
                referenced_table: row.referenced_table_name,
                referenced_columns: Vec::new(),
                update_rule: row.update_rule,
                delete_rule: row.delete_rule,
            });
        foreign_key.columns.push(row.column_name);
        foreign_key
            .referenced_columns
            .push(row.referenced_column_name);
    }
    let foreign_keys = foreign_key_map.into_values().collect();

    let quoted_table = quote_identifier(&table.table_name)?;
    let has_rows: Option<i64> =
        sqlx::query_scalar(&format!("SELECT 1 FROM {quoted_table} LIMIT 1"))
            .fetch_optional(&mut *connection)
            .await
            .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;

    Ok(ExistingTableSchema::existing(columns, primary_key, indexes)
        .with_constraints(checks, foreign_keys)
        .with_rows(has_rows.is_some()))
}

pub(super) fn plan_table_sync(
    definition: &TableDefinition,
    existing: &ExistingTableSchema,
) -> Result<TableSyncPlan, BaseError> {
    let table = definition.config();
    validate_table_config(table)?;
    if !existing.exists {
        let mut statements = vec![render_create_table(table, false)?];
        let mut changes = vec![SchemaSyncChange {
            table: table.table_name.clone(),
            object: table.table_name.clone(),
            kind: SchemaSyncChangeKind::CreatedTable,
        }];
        for foreign_key in &table.foreign_keys {
            statements.push(format!(
                "ALTER TABLE {} ADD {}",
                quote_identifier(&table.table_name)?,
                render_foreign_key(foreign_key)?
            ));
            changes.push(SchemaSyncChange {
                table: table.table_name.clone(),
                object: foreign_key.name.clone(),
                kind: SchemaSyncChangeKind::AddedForeignKey,
            });
        }
        return Ok(TableSyncPlan {
            statements,
            changes,
            preflight: Vec::new(),
        });
    }

    let quoted_table = quote_identifier(&table.table_name)?;
    let mut statements = Vec::new();
    let mut changes = Vec::new();
    let mut preflight = Vec::new();
    let mut renamed_columns = BTreeMap::new();
    let mut fields: Vec<&FieldConfig> = table.fields.values().collect();
    fields.sort_by(|left, right| left.name.cmp(&right.name));
    for field in fields {
        let current_exists = existing
            .columns
            .iter()
            .any(|column| column.name == field.name);
        let legacy_exists = field.renamed_from.as_ref().is_some_and(|legacy_name| {
            existing
                .columns
                .iter()
                .any(|column| column.name == *legacy_name)
        });
        if current_exists && legacy_exists {
            return Err(BaseError::DatabaseInitFailed(format!(
                "表 {} 同时存在旧列 {} 与目标列 {}，拒绝猜测应保留哪一列；请人工合并数据",
                table.table_name,
                field.renamed_from.as_deref().unwrap_or_default(),
                field.name
            )));
        }
        if current_exists {
            continue;
        }
        if let Some(legacy_name) = field.renamed_from.as_deref().filter(|_| legacy_exists) {
            statements.push(format!(
                "ALTER TABLE {quoted_table} RENAME COLUMN {} TO {}",
                quote_identifier(legacy_name)?,
                quote_identifier(&field.name)?
            ));
            changes.push(SchemaSyncChange {
                table: table.table_name.clone(),
                object: format!("{legacy_name}->{}", field.name),
                kind: SchemaSyncChangeKind::RenamedColumn,
            });
            renamed_columns.insert(legacy_name.to_string(), field.name.clone());
            continue;
        }
        if field.auto_increment {
            return Err(BaseError::DatabaseInitFailed(format!(
                "表 {} 已存在，缺失的自增主键字段 {} 不能安全地分步增加；请人工处理后再启动",
                table.table_name, field.name
            )));
        }
        if existing.has_rows
            && field.required
            && field.default_value.is_none()
            && !field.auto_increment
        {
            return Err(BaseError::DatabaseInitFailed(format!(
                "表 {} 已有数据，不能自动增加无默认值的必填字段 {}",
                table.table_name, field.name
            )));
        }
        statements.push(format!(
            "ALTER TABLE {quoted_table} ADD COLUMN {}",
            render_column(field)?
        ));
        changes.push(SchemaSyncChange {
            table: table.table_name.clone(),
            object: field.name.clone(),
            kind: SchemaSyncChangeKind::AddedColumn,
        });
    }

    let effective_columns = existing
        .columns
        .iter()
        .cloned()
        .map(|mut column| {
            if let Some(renamed) = renamed_columns.get(&column.name) {
                column.name = renamed.clone();
            }
            column
        })
        .collect::<Vec<_>>();
    let report = table.validate_schema(&effective_columns);
    let unsafe_issues = report
        .issues
        .iter()
        .filter(|issue| {
            if issue.kind == SchemaIssueKind::MissingColumn {
                return false;
            }
            let Some(field) = table.fields.get(&issue.field) else {
                return true;
            };
            match issue.kind {
                SchemaIssueKind::NullabilityMismatch | SchemaIssueKind::DefaultMismatch => false,
                SchemaIssueKind::IncompatibleType => {
                    let data_type = effective_columns
                        .iter()
                        .find(|column| column.name == issue.field)
                        .map(|column| column.data_type.as_str());
                    match field.field_type {
                        FieldType::String { .. } => !matches!(
                            data_type,
                            Some("char" | "varchar" | "text" | "mediumtext" | "longtext")
                        ),
                        FieldType::Enum { .. } => {
                            !matches!(data_type, Some("char" | "varchar" | "enum"))
                        }
                        _ => true,
                    }
                }
                SchemaIssueKind::AutoIncrementMismatch | SchemaIssueKind::MissingColumn => true,
            }
        })
        .collect::<Vec<_>>();
    if !unsafe_issues.is_empty() {
        let details = unsafe_issues
            .iter()
            .map(|issue| format!("{}:{:?}", issue.field, issue.kind))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(BaseError::DatabaseInitFailed(format!(
            "表 {} 存在不可自动修改的 schema 差异: {details}",
            table.table_name
        )));
    }
    let mut changed_fields = report
        .issues
        .iter()
        .filter(|issue| issue.kind != SchemaIssueKind::MissingColumn)
        .map(|issue| issue.field.as_str())
        .collect::<Vec<_>>();
    changed_fields.sort_unstable();
    changed_fields.dedup();
    for field_name in changed_fields {
        let field = &table.fields[field_name];
        let existing_name = existing_column_name(field_name, &renamed_columns);
        let quoted_existing = quote_identifier(&existing_name)?;
        let issues = report
            .issues
            .iter()
            .filter(|issue| issue.field == field_name)
            .collect::<Vec<_>>();
        if issues
            .iter()
            .any(|issue| issue.kind == SchemaIssueKind::NullabilityMismatch && field.required)
        {
            preflight.push(SchemaPreflightCheck::ColumnPredicate {
                object: field.name.clone(),
                predicate: format!("{quoted_existing} IS NULL"),
            });
        }
        if issues
            .iter()
            .any(|issue| issue.kind == SchemaIssueKind::IncompatibleType)
        {
            match &field.field_type {
                FieldType::String { max_length } => {
                    preflight.push(SchemaPreflightCheck::ColumnPredicate {
                        object: field.name.clone(),
                        predicate: format!(
                            "{quoted_existing} IS NOT NULL AND CHAR_LENGTH({quoted_existing}) > {max_length}"
                        ),
                    });
                }
                FieldType::Enum { values } => {
                    let allowed = values
                        .iter()
                        .map(|value| quote_string(value))
                        .collect::<Vec<_>>()
                        .join(", ");
                    preflight.push(SchemaPreflightCheck::ColumnPredicate {
                        object: field.name.clone(),
                        predicate: format!(
                            "{quoted_existing} IS NOT NULL AND {quoted_existing} NOT IN ({allowed})"
                        ),
                    });
                }
                _ => {}
            }
        }
        statements.push(format!(
            "ALTER TABLE {quoted_table} MODIFY COLUMN {}",
            render_column(field)?
        ));
        changes.push(SchemaSyncChange {
            table: table.table_name.clone(),
            object: field.name.clone(),
            kind: SchemaSyncChangeKind::ModifiedColumn,
        });
    }

    let desired_primary = vec![table.primary_key.clone()];
    let effective_primary = existing
        .primary_key
        .iter()
        .map(|column| {
            renamed_columns
                .get(column)
                .cloned()
                .unwrap_or_else(|| column.clone())
        })
        .collect::<Vec<_>>();
    if effective_primary.is_empty() {
        statements.push(format!(
            "ALTER TABLE {quoted_table} ADD PRIMARY KEY ({})",
            quote_identifier(&table.primary_key)?
        ));
        changes.push(SchemaSyncChange {
            table: table.table_name.clone(),
            object: table.primary_key.clone(),
            kind: SchemaSyncChangeKind::AddedPrimaryKey,
        });
    } else if effective_primary != desired_primary {
        return Err(BaseError::DatabaseInitFailed(format!(
            "表 {} 的主键 {:?} 与声明 {:?} 不一致，拒绝自动修改",
            table.table_name, effective_primary, desired_primary
        )));
    }

    let effective_indexes = existing
        .indexes
        .iter()
        .cloned()
        .map(|mut index| {
            for column in &mut index.columns {
                if let Some(renamed) = renamed_columns.get(column) {
                    *column = renamed.clone();
                }
            }
            index
        })
        .collect::<Vec<_>>();
    for desired in desired_indexes(table)? {
        if let Some(existing_named) = effective_indexes
            .iter()
            .find(|index| index.name == desired.name)
        {
            if existing_named.unique == desired.unique && existing_named.columns == desired.columns
            {
                continue;
            }
            return Err(BaseError::DatabaseInitFailed(format!(
                "表 {} 的索引 {} 已存在但定义不同，拒绝自动修改",
                table.table_name, desired.name
            )));
        }
        let already_exists = effective_indexes.iter().any(|index| {
            index.columns == desired.columns && (index.unique == desired.unique || index.unique)
        });
        if already_exists {
            continue;
        }
        let columns = desired
            .columns
            .iter()
            .map(|column| quote_identifier(column))
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");
        let kind = if desired.unique {
            "UNIQUE INDEX"
        } else {
            "INDEX"
        };
        statements.push(format!(
            "ALTER TABLE {quoted_table} ADD {kind} {} ({columns})",
            quote_identifier(&desired.name)?
        ));
        changes.push(SchemaSyncChange {
            table: table.table_name.clone(),
            object: desired.name.clone(),
            kind: SchemaSyncChangeKind::AddedIndex,
        });
        if desired.unique {
            preflight.push(SchemaPreflightCheck::UniqueIndex {
                object: desired.name,
                columns: desired
                    .columns
                    .into_iter()
                    .map(|column| existing_column_name(&column, &renamed_columns))
                    .collect(),
            });
        }
    }

    for check in &table.checks {
        if let Some(existing_check) = existing.checks.iter().find(|item| item.name == check.name) {
            if normalize_check_expression(&existing_check.expression)
                == normalize_check_expression(&check.expression)
            {
                continue;
            }
            return Err(BaseError::DatabaseInitFailed(format!(
                "表 {} 的 CHECK {} 已存在但表达式不同，拒绝覆盖: actual={}, expected={}",
                table.table_name,
                check.name,
                normalize_check_expression(&existing_check.expression),
                normalize_check_expression(&check.expression)
            )));
        }
        if existing.checks.iter().any(|item| {
            normalize_check_expression(&item.expression)
                == normalize_check_expression(&check.expression)
        }) {
            continue;
        }
        statements.push(format!(
            "ALTER TABLE {quoted_table} ADD {}",
            render_check(check)?
        ));
        changes.push(SchemaSyncChange {
            table: table.table_name.clone(),
            object: check.name.clone(),
            kind: SchemaSyncChangeKind::AddedCheck,
        });
        preflight.push(SchemaPreflightCheck::Check {
            object: check.name.clone(),
            expression: expression_for_existing_schema(&check.expression, &renamed_columns),
        });
    }

    for foreign_key in &table.foreign_keys {
        let existing_columns = foreign_key
            .columns
            .iter()
            .map(|column| existing_column_name(column, &renamed_columns))
            .collect::<Vec<_>>();
        if let Some(existing_foreign_key) = existing
            .foreign_keys
            .iter()
            .find(|item| item.name == foreign_key.name)
        {
            if foreign_key_matches(existing_foreign_key, foreign_key, &renamed_columns) {
                continue;
            }
            return Err(BaseError::DatabaseInitFailed(format!(
                "表 {} 的外键 {} 已存在但定义不同，拒绝覆盖",
                table.table_name, foreign_key.name
            )));
        }
        if existing
            .foreign_keys
            .iter()
            .any(|item| foreign_key_matches(item, foreign_key, &renamed_columns))
        {
            continue;
        }
        statements.push(format!(
            "ALTER TABLE {quoted_table} ADD {}",
            render_foreign_key(foreign_key)?
        ));
        changes.push(SchemaSyncChange {
            table: table.table_name.clone(),
            object: foreign_key.name.clone(),
            kind: SchemaSyncChangeKind::AddedForeignKey,
        });
        preflight.push(SchemaPreflightCheck::ForeignKey {
            object: foreign_key.name.clone(),
            columns: existing_columns,
            referenced_table: foreign_key.referenced_table.clone(),
            referenced_columns: foreign_key.referenced_columns.clone(),
        });
    }

    Ok(TableSyncPlan {
        statements,
        changes,
        preflight,
    })
}

fn normalize_definitions<'a>(
    definitions: &'a [&'a TableDefinition],
) -> Result<Vec<&'a TableDefinition>, BaseError> {
    let mut normalized: BTreeMap<&str, (&TableDefinition, String)> = BTreeMap::new();
    for definition in definitions {
        let table = definition.config();
        validate_table_config(table)?;
        let signature = render_create_table(table, true)?;
        if let Some((_, existing_signature)) = normalized.get(table.table_name.as_str()) {
            if existing_signature != &signature {
                return Err(BaseError::ConfigError(format!(
                    "表配置冲突: {} 在多个模块中声明不同 schema",
                    table.table_name
                )));
            }
            continue;
        }
        normalized.insert(table.table_name.as_str(), (*definition, signature));
    }
    Ok(normalized
        .into_values()
        .map(|(definition, _)| definition)
        .collect())
}

fn validate_table_config(table: &TableConfig) -> Result<(), BaseError> {
    quote_identifier(&table.table_name)?;
    if table.fields.is_empty() {
        return Err(BaseError::ConfigError(format!(
            "表 {} 没有声明字段",
            table.table_name
        )));
    }
    let primary = table.fields.get(&table.primary_key).ok_or_else(|| {
        BaseError::ConfigError(format!(
            "表 {} 的主键字段 {} 未声明",
            table.table_name, table.primary_key
        ))
    })?;
    if !primary.required {
        return Err(BaseError::ConfigError(format!(
            "表 {} 的主键字段 {} 必须 required",
            table.table_name, table.primary_key
        )));
    }
    for field in table.fields.values() {
        quote_identifier(&field.name)?;
        if field.auto_increment
            && (field.name != table.primary_key
                || !matches!(field.field_type, FieldType::Integer | FieldType::BigInt))
        {
            return Err(BaseError::ConfigError(format!(
                "表 {} 的 auto_increment 仅允许用于整数主键字段: {}",
                table.table_name, field.name
            )));
        }
        if field.auto_increment && field.default_value.is_some() {
            return Err(BaseError::ConfigError(format!(
                "表 {} 的自增字段不能同时声明默认值: {}",
                table.table_name, field.name
            )));
        }
        let _ = render_column(field)?;
    }
    let _ = desired_indexes(table)?;
    Ok(())
}

fn render_create_table(
    table: &TableConfig,
    include_foreign_keys: bool,
) -> Result<String, BaseError> {
    let mut fields: Vec<&FieldConfig> = table.fields.values().collect();
    fields.sort_by(|left, right| left.name.cmp(&right.name));
    let mut definitions = fields
        .into_iter()
        .map(render_column)
        .collect::<Result<Vec<_>, _>>()?;
    definitions.push(format!(
        "PRIMARY KEY ({})",
        quote_identifier(&table.primary_key)?
    ));
    for index in desired_indexes(table)? {
        let columns = index
            .columns
            .iter()
            .map(|column| quote_identifier(column))
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");
        let kind = if index.unique { "UNIQUE KEY" } else { "KEY" };
        definitions.push(format!(
            "{kind} {} ({columns})",
            quote_identifier(&index.name)?
        ));
    }
    for check in &table.checks {
        definitions.push(render_check(check)?);
    }
    if include_foreign_keys {
        for foreign_key in &table.foreign_keys {
            definitions.push(render_foreign_key(foreign_key)?);
        }
    }
    Ok(format!(
        "CREATE TABLE {} ({}) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
        quote_identifier(&table.table_name)?,
        definitions.join(", ")
    ))
}

fn render_check(check: &CheckConfig) -> Result<String, BaseError> {
    Ok(format!(
        "CONSTRAINT {} CHECK ({})",
        quote_identifier(&check.name)?,
        check.expression
    ))
}

fn render_foreign_key(foreign_key: &ForeignKeyConfig) -> Result<String, BaseError> {
    let columns = foreign_key
        .columns
        .iter()
        .map(|column| quote_identifier(column))
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    let referenced_columns = foreign_key
        .referenced_columns
        .iter()
        .map(|column| quote_identifier(column))
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    Ok(format!(
        "CONSTRAINT {} FOREIGN KEY ({columns}) REFERENCES {} ({referenced_columns})",
        quote_identifier(&foreign_key.name)?,
        quote_identifier(&foreign_key.referenced_table)?
    ))
}

fn normalize_check_expression(expression: &str) -> String {
    let mut normalized = expression
        .chars()
        .filter(|character| !character.is_ascii_whitespace() && *character != '`')
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .replace("_utf8mb4", "")
        .replace("_utf8", "")
        .replace("\\'", "'");
    while has_redundant_outer_parentheses(&normalized) {
        normalized = normalized[1..normalized.len() - 1].to_string();
    }
    normalized
}

fn has_redundant_outer_parentheses(expression: &str) -> bool {
    if !expression.starts_with('(') || !expression.ends_with(')') {
        return false;
    }
    let mut depth = 0_i32;
    let mut quoted = false;
    let mut previous_quote = false;
    for (index, character) in expression.char_indices() {
        if character == '\'' {
            if quoted && previous_quote {
                previous_quote = false;
                continue;
            }
            previous_quote = quoted;
            quoted = !quoted;
            continue;
        }
        previous_quote = false;
        if quoted {
            continue;
        }
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 && index + character.len_utf8() != expression.len() {
                    return false;
                }
            }
            _ => {}
        }
        if depth < 0 {
            return false;
        }
    }
    depth == 0 && !quoted
}

fn foreign_key_matches(
    existing: &ExistingForeignKey,
    desired: &ForeignKeyConfig,
    renamed_columns: &BTreeMap<String, String>,
) -> bool {
    let columns = existing
        .columns
        .iter()
        .map(|column| {
            renamed_columns
                .get(column)
                .cloned()
                .unwrap_or_else(|| column.clone())
        })
        .collect::<Vec<_>>();
    columns == desired.columns
        && existing.referenced_table == desired.referenced_table
        && existing.referenced_columns == desired.referenced_columns
        && matches!(existing.update_rule.as_str(), "RESTRICT" | "NO ACTION")
        && matches!(existing.delete_rule.as_str(), "RESTRICT" | "NO ACTION")
}

fn existing_column_name(column: &str, renamed_columns: &BTreeMap<String, String>) -> String {
    renamed_columns
        .iter()
        .find_map(|(legacy, current)| (current == column).then(|| legacy.clone()))
        .unwrap_or_else(|| column.to_string())
}

fn expression_for_existing_schema(
    expression: &str,
    renamed_columns: &BTreeMap<String, String>,
) -> String {
    renamed_columns
        .iter()
        .fold(expression.to_string(), |expression, (legacy, current)| {
            expression.replace(&format!("`{current}`"), &format!("`{legacy}`"))
        })
}

fn render_column(field: &FieldConfig) -> Result<String, BaseError> {
    let sql_type = match &field.field_type {
        FieldType::String { max_length } if (1..=16_383).contains(max_length) => {
            format!("VARCHAR({max_length})")
        }
        FieldType::String { max_length } => {
            return Err(BaseError::ConfigError(format!(
                "字段 {} 的 VARCHAR 长度必须在 1..=16383: {}",
                field.name, max_length
            )))
        }
        FieldType::Integer => "INT".to_string(),
        FieldType::BigInt => "BIGINT".to_string(),
        FieldType::Float => "FLOAT".to_string(),
        FieldType::Double => "DOUBLE".to_string(),
        FieldType::Decimal { precision, scale } => format!("DECIMAL({precision},{scale})"),
        FieldType::Boolean => "TINYINT(1)".to_string(),
        FieldType::Date => "DATE".to_string(),
        FieldType::DateTime => "DATETIME".to_string(),
        FieldType::Timestamp => "BIGINT".to_string(),
        FieldType::Json => "JSON".to_string(),
        FieldType::Text => "TEXT".to_string(),
        FieldType::Enum { values } => {
            if values.is_empty() || values.iter().any(|value| value.is_empty()) {
                return Err(BaseError::ConfigError(format!(
                    "枚举字段 {} 必须声明非空值",
                    field.name
                )));
            }
            let mut sorted = values.clone();
            sorted.sort();
            if sorted.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(BaseError::ConfigError(format!(
                    "枚举字段 {} 包含重复值",
                    field.name
                )));
            }
            format!(
                "ENUM({})",
                values
                    .iter()
                    .map(|value| quote_string(value))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    };

    let mut definition = format!(
        "{} {sql_type} {}",
        quote_identifier(&field.name)?,
        if field.required { "NOT NULL" } else { "NULL" }
    );
    if field.auto_increment {
        definition.push_str(" AUTO_INCREMENT");
    }
    if let Some(default) = &field.default_value {
        if default.is_null() && field.required {
            return Err(BaseError::ConfigError(format!(
                "必填字段 {} 不能声明 NULL 默认值",
                field.name
            )));
        }
        if matches!(field.field_type, FieldType::Text | FieldType::Json) {
            return Err(BaseError::ConfigError(format!(
                "TEXT/JSON 字段 {} 不自动生成数据库默认值",
                field.name
            )));
        }
        if !default.is_null() {
            field.field_type.validate(&field.name, default)?;
        }
        definition.push_str(" DEFAULT ");
        definition.push_str(&render_default(default)?);
    }
    Ok(definition)
}

fn render_default(value: &serde_json::Value) -> Result<String, BaseError> {
    match value {
        serde_json::Value::Null => Ok("NULL".to_string()),
        serde_json::Value::Bool(value) => Ok(if *value { "1" } else { "0" }.to_string()),
        serde_json::Value::Number(value) => Ok(value.to_string()),
        serde_json::Value::String(value) => Ok(quote_string(value)),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => Err(BaseError::ConfigError(
            "数据库字段默认值只支持 null/bool/number/string".to_string(),
        )),
    }
}

fn desired_indexes(table: &TableConfig) -> Result<Vec<DesiredIndex>, BaseError> {
    let mut desired = Vec::new();
    for (unique, indexes) in [
        (true, table.unique_indexes.as_slice()),
        (false, table.indexes.as_slice()),
    ] {
        for index in indexes {
            if index.fields.is_empty() {
                return Err(BaseError::ConfigError(format!(
                    "表 {} 包含空索引",
                    table.table_name
                )));
            }
            for field in &index.fields {
                if !table.fields.contains_key(field) {
                    return Err(BaseError::ConfigError(format!(
                        "表 {} 的索引引用未声明字段: {}",
                        table.table_name, field
                    )));
                }
            }
            let prefix = if unique { "uk" } else { "idx" };
            let name = index.name.clone().unwrap_or_else(|| {
                format!("{prefix}_{}_{}", table.table_name, index.fields.join("_"))
            });
            if name.len() > 64 {
                return Err(BaseError::ConfigError(format!(
                    "索引名超过 MySQL 64 字符限制，请显式命名: {name}"
                )));
            }
            quote_identifier(&name)?;
            desired.push(DesiredIndex {
                name,
                unique,
                columns: index.fields.clone(),
            });
        }
    }
    Ok(desired)
}

fn quote_identifier(identifier: &str) -> Result<String, BaseError> {
    yang_db::mysql::quote_identifier(identifier).map_err(|error| {
        BaseError::ConfigError(format!("非法数据库标识符 {identifier:?}: {error}"))
    })
}

fn quote_string(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "''"))
}

fn schema_lock_name(database_name: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in database_name.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("yang_base_schema_{hash:016x}")
}

#[cfg(test)]
mod check_expression_tests {
    use super::normalize_check_expression;

    #[test]
    fn mysql_metadata_decorations_do_not_break_check_idempotency() {
        assert_eq!(
            normalize_check_expression(
                "(`status` in (_utf8mb4\\'active\\',_utf8mb4\\'disabled\\'))"
            ),
            normalize_check_expression("`status` IN ('active', 'disabled')")
        );
    }

    #[test]
    fn internal_parentheses_remain_semantically_significant() {
        assert_ne!(
            normalize_check_expression("`a` AND (`b` OR `c`)"),
            normalize_check_expression("(`a` AND `b`) OR `c`")
        );
    }
}
