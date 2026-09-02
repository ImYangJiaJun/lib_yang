use super::super::DatabaseInitializer;
use super::plan::{plan_locked, report_tables};
use super::preflight::{format_violations, preflight_locked};
use super::render::{normalize_definitions, schema_lock_name};
use super::{SchemaPreflightReport, SchemaSyncChangeKind, SchemaSyncReport};
use crate::error::BaseError;
use crate::table::{SchemaColumn, SchemaValidationReport, TableDefinition};
use sqlx::mysql::MySqlConnection;
use sqlx::Executor;

const SCHEMA_LOCK_TIMEOUT_SECONDS: i64 = 30;

impl DatabaseInitializer {
    /// 从 MySQL information_schema 读取当前列并验证表定义的运行期字段契约。
    ///
    /// 本方法只读，不生成或执行 ALTER；数据库额外列不视为问题。
    pub async fn validate_table_definition(
        &self,
        table: &TableDefinition,
    ) -> Result<SchemaValidationReport, BaseError> {
        #[derive(sqlx::FromRow)]
        struct ColumnRow {
            column_name: String,
            data_type: String,
            column_type: String,
            is_nullable: String,
            character_maximum_length: Option<i64>,
            column_default: Option<String>,
            extra: String,
        }

        let rows: Vec<ColumnRow> = self
            .db()
            .query_with_params(
                "SELECT CAST(COLUMN_NAME AS CHAR) AS column_name, CAST(DATA_TYPE AS CHAR) AS data_type, CAST(COLUMN_TYPE AS CHAR) AS column_type, CAST(IS_NULLABLE AS CHAR) AS is_nullable, CAST(CHARACTER_MAXIMUM_LENGTH AS SIGNED) AS character_maximum_length, CAST(COLUMN_DEFAULT AS CHAR) AS column_default, CAST(EXTRA AS CHAR) AS extra FROM information_schema.columns WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ? ORDER BY ORDINAL_POSITION",
                vec![serde_json::Value::String(table.name().to_string())],
            )
            .await
            .map_err(BaseError::DatabaseQueryFailed)?;
        let columns = rows
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
            .collect::<Vec<_>>();
        Ok(table.validate_schema(&columns))
    }

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
