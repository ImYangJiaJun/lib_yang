//! 基于模块 `TableConfig` 的 MySQL 启动期 additive schema 同步。
//!
//! 同步器只创建缺失表、字段、主键和索引；不会删除列/索引，也不会自动修改已有
//! 字段类型或 NULL 约束。多实例同时启动时使用同一 MySQL 会话持有 advisory lock，
//! DDL 中断后下一实例可按 information_schema 重新规划并幂等续作。

use super::DatabaseInitializer;
use crate::error::BaseError;
use crate::router::AppRouter;
use crate::table::{FieldConfig, FieldType, SchemaColumn, SchemaIssueKind, TableConfig};
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
    /// 增加主键。
    AddedPrimaryKey,
    /// 增加普通或唯一索引。
    AddedIndex,
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
    has_rows: bool,
}

impl ExistingTableSchema {
    pub(super) fn missing() -> Self {
        Self {
            exists: false,
            columns: Vec::new(),
            primary_key: Vec::new(),
            indexes: Vec::new(),
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
            has_rows: false,
        }
    }

    fn with_rows(mut self, has_rows: bool) -> Self {
        self.has_rows = has_rows;
        self
    }
}

#[derive(Debug)]
pub(super) struct TableSyncPlan {
    pub(super) statements: Vec<String>,
    pub(super) changes: Vec<SchemaSyncChange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DesiredIndex {
    name: String,
    unique: bool,
    columns: Vec<String>,
}

impl DatabaseInitializer {
    /// 根据 `AppRouter` 中各模块声明的表配置同步 MySQL schema。
    ///
    /// 该入口适合在监听 HTTP 端口前调用。同步策略固定为 additive：只创建缺失
    /// 对象；任何已有字段类型、NULL、自增或主键冲突都会失败并中止启动。
    pub async fn sync_app_schema(
        &self,
        app_router: &AppRouter,
    ) -> Result<SchemaSyncReport, BaseError> {
        let tables = app_router.table_configs();
        self.sync_table_configs(&tables).await
    }

    /// 同步一组表配置，使用单个数据库级 advisory lock 串行化多实例启动。
    pub async fn sync_table_configs(
        &self,
        tables: &[&TableConfig],
    ) -> Result<SchemaSyncReport, BaseError> {
        let tables = normalize_tables(tables)?;
        if tables.is_empty() {
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

        let result = sync_locked(&mut connection, &tables).await;
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
    tables: &[&TableConfig],
) -> Result<SchemaSyncReport, BaseError> {
    let mut report = SchemaSyncReport {
        tables: tables
            .iter()
            .map(|table| table.table_name.clone())
            .collect(),
        changes: Vec::new(),
    };

    let mut plans = Vec::with_capacity(tables.len());
    for table in tables {
        let existing = load_existing_schema(connection, table).await?;
        let plan = plan_table_sync(table, &existing)?;
        if plan.statements.len() != plan.changes.len() {
            return Err(BaseError::DatabaseInitFailed(format!(
                "表 {} 的 schema 计划内部不一致",
                table.table_name
            )));
        }
        plans.push(plan);
    }

    for plan in plans {
        for (statement, change) in plan.statements.into_iter().zip(plan.changes) {
            connection
                .execute(sqlx::query(&statement))
                .await
                .map_err(|error| BaseError::DatabaseExecuteFailed(error.into()))?;
            tracing::info!(
                table = %change.table,
                object = %change.object,
                kind = ?change.kind,
                "数据库 schema 已同步"
            );
            report.changes.push(change);
        }
    }

    Ok(report)
}

async fn load_existing_schema(
    connection: &mut MySqlConnection,
    table: &TableConfig,
) -> Result<ExistingTableSchema, BaseError> {
    #[derive(FromRow)]
    struct ColumnRow {
        column_name: String,
        data_type: String,
        column_type: String,
        is_nullable: String,
        character_maximum_length: Option<i64>,
        extra: String,
    }

    #[derive(FromRow)]
    struct IndexRow {
        index_name: String,
        non_unique: i64,
        column_name: String,
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
        "SELECT CAST(COLUMN_NAME AS CHAR) AS column_name, CAST(DATA_TYPE AS CHAR) AS data_type, CAST(COLUMN_TYPE AS CHAR) AS column_type, CAST(IS_NULLABLE AS CHAR) AS is_nullable, CAST(CHARACTER_MAXIMUM_LENGTH AS SIGNED) AS character_maximum_length, CAST(EXTRA AS CHAR) AS extra FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name = ? ORDER BY ORDINAL_POSITION",
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

    let quoted_table = quote_identifier(&table.table_name)?;
    let has_rows: Option<i64> =
        sqlx::query_scalar(&format!("SELECT 1 FROM {quoted_table} LIMIT 1"))
            .fetch_optional(&mut *connection)
            .await
            .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;

    Ok(ExistingTableSchema::existing(columns, primary_key, indexes).with_rows(has_rows.is_some()))
}

pub(super) fn plan_table_sync(
    table: &TableConfig,
    existing: &ExistingTableSchema,
) -> Result<TableSyncPlan, BaseError> {
    validate_table_config(table)?;
    if !existing.exists {
        return Ok(TableSyncPlan {
            statements: vec![render_create_table(table)?],
            changes: vec![SchemaSyncChange {
                table: table.table_name.clone(),
                object: table.table_name.clone(),
                kind: SchemaSyncChangeKind::CreatedTable,
            }],
        });
    }

    let report = table.validate_schema(&existing.columns);
    let unsafe_issues: Vec<_> = report
        .issues
        .iter()
        .filter(|issue| issue.kind != SchemaIssueKind::MissingColumn)
        .collect();
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

    let quoted_table = quote_identifier(&table.table_name)?;
    let mut statements = Vec::new();
    let mut changes = Vec::new();
    let mut fields: Vec<&FieldConfig> = table.fields.values().collect();
    fields.sort_by(|left, right| left.name.cmp(&right.name));
    for field in fields {
        if existing
            .columns
            .iter()
            .any(|column| column.name == field.name)
        {
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

    let desired_primary = vec![table.primary_key.clone()];
    if existing.primary_key.is_empty() {
        statements.push(format!(
            "ALTER TABLE {quoted_table} ADD PRIMARY KEY ({})",
            quote_identifier(&table.primary_key)?
        ));
        changes.push(SchemaSyncChange {
            table: table.table_name.clone(),
            object: table.primary_key.clone(),
            kind: SchemaSyncChangeKind::AddedPrimaryKey,
        });
    } else if existing.primary_key != desired_primary {
        return Err(BaseError::DatabaseInitFailed(format!(
            "表 {} 的主键 {:?} 与声明 {:?} 不一致，拒绝自动修改",
            table.table_name, existing.primary_key, desired_primary
        )));
    }

    for desired in desired_indexes(table)? {
        if let Some(existing_named) = existing
            .indexes
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
        let already_exists = existing.indexes.iter().any(|index| {
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
            object: desired.name,
            kind: SchemaSyncChangeKind::AddedIndex,
        });
    }

    Ok(TableSyncPlan {
        statements,
        changes,
    })
}

fn normalize_tables<'a>(tables: &'a [&'a TableConfig]) -> Result<Vec<&'a TableConfig>, BaseError> {
    let mut normalized: BTreeMap<&str, (&TableConfig, String)> = BTreeMap::new();
    for table in tables {
        validate_table_config(table)?;
        let signature = render_create_table(table)?;
        if let Some((_, existing_signature)) = normalized.get(table.table_name.as_str()) {
            if existing_signature != &signature {
                return Err(BaseError::ConfigError(format!(
                    "表配置冲突: {} 在多个模块中声明不同 schema",
                    table.table_name
                )));
            }
            continue;
        }
        normalized.insert(table.table_name.as_str(), (*table, signature));
    }
    Ok(normalized.into_values().map(|(table, _)| table).collect())
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

fn render_create_table(table: &TableConfig) -> Result<String, BaseError> {
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
    Ok(format!(
        "CREATE TABLE {} ({}) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
        quote_identifier(&table.table_name)?,
        definitions.join(", ")
    ))
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
        FieldType::ForeignKey { .. } => {
            return Err(BaseError::ConfigError(format!(
                "字段 {} 的 ForeignKey 未携带本地列类型，不能自动生成 DDL",
                field.name
            )))
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
