use super::super::DatabaseInitializer;
use super::driver::SchemaConnection;
use super::driver::{OwnedSchemaConnection, SchemaDriver};
use super::plan::{plan_locked, report_tables};
use super::preflight::{format_violations, preflight_locked};
use super::{SchemaPreflightReport, SchemaSyncChangeKind, SchemaSyncReport};
use crate::error::BaseError;
#[cfg(feature = "mysql")]
use crate::table::SchemaColumn;
use crate::table::{SchemaValidationReport, TableDefinition};

impl DatabaseInitializer {
    /// 从数据库读取当前列并验证表定义的运行期字段契约。
    ///
    /// 本方法只读，不生成或执行 ALTER；数据库额外列不视为问题。
    pub async fn validate_table_definition(
        &self,
        table: &TableDefinition,
    ) -> Result<SchemaValidationReport, BaseError> {
        let driver = self.schema_driver()?;
        match driver {
            #[cfg(feature = "mysql")]
            SchemaDriver::MySql => {
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

                let database = self.db().as_mysql().ok_or_else(|| {
                    BaseError::DatabaseInitFailed(
                        "DatabaseInitializer 未持有 MySQL 后端".to_string(),
                    )
                })?;
                let rows: Vec<ColumnRow> = database
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
            #[cfg(feature = "postgres")]
            SchemaDriver::PostgreSql => {
                let mut owned = self.acquire_schema_connection().await?;
                let mut connection = owned.as_schema();
                let existing = driver.introspect(&mut connection, table).await?;
                let config = table.shared_config();
                Ok(driver
                    .dialect()
                    .validate_schema(config.as_ref(), &existing.columns))
            }
        }
    }

    /// 只读取当前数据库 schema 并计算待执行变更，不获取同步锁、不执行 DDL。
    ///
    /// 适用于生产启动门禁：返回非空 `changes` 表示数据库尚未与定义对齐；不兼容
    /// 变更仍返回错误。调用方若要实际应用变更，应使用 [`Self::sync_table_definitions`]。
    pub async fn plan_table_definitions(
        &self,
        definitions: &[&TableDefinition],
    ) -> Result<SchemaSyncReport, BaseError> {
        let driver = self.schema_driver()?;
        let definitions = driver.dialect().normalize_definitions(definitions)?;
        if definitions.is_empty() {
            return Ok(SchemaSyncReport::default());
        }
        let mut owned = self.acquire_schema_connection().await?;
        let mut connection = owned.as_schema();
        let plans = plan_locked(driver, &mut connection, &definitions).await?;
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
        let driver = self.schema_driver()?;
        let definitions = driver.dialect().normalize_definitions(definitions)?;
        if definitions.is_empty() {
            return Ok(SchemaPreflightReport::default());
        }
        let mut owned = self.acquire_schema_connection().await?;
        let mut connection = owned.as_schema();
        let plans = plan_locked(driver, &mut connection, &definitions).await?;
        let violations =
            preflight_locked(driver.dialect(), &mut connection, &definitions, &plans).await?;
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

    /// 同步一组不可变表定义。
    ///
    /// 多实例启动使用数据库级 advisory lock 串行化：MySQL 用 `GET_LOCK`，
    /// PostgreSQL 用 `pg_advisory_lock(bigint)`（session 级）。PostgreSQL 的全部
    /// DDL 在单个事务中执行，任一步失败整体回滚。
    pub async fn sync_table_definitions(
        &self,
        definitions: &[&TableDefinition],
    ) -> Result<SchemaSyncReport, BaseError> {
        let driver = self.schema_driver()?;
        let definitions = driver.dialect().normalize_definitions(definitions)?;
        if definitions.is_empty() {
            return Ok(SchemaSyncReport::default());
        }

        let mut owned = self.acquire_schema_connection().await?;
        let mut connection = owned.as_schema();
        let lock = driver.lock(&mut connection).await?;
        let result = sync_locked(driver, &mut connection, &definitions).await;
        let release_result = driver.unlock(&mut connection, &lock).await;

        match (result, release_result) {
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
            (Ok(report), Ok(())) => Ok(report),
        }
    }

    /// 选择当前后端的 schema 驱动。
    pub(crate) fn schema_driver(&self) -> Result<SchemaDriver, BaseError> {
        SchemaDriver::for_backend(self.db().backend())
    }

    /// 从 [`super::super::AnyDatabase`] 借出一个 schema 同步连接。
    pub(crate) async fn acquire_schema_connection(
        &self,
    ) -> Result<OwnedSchemaConnection, BaseError> {
        #[cfg(feature = "mysql")]
        if let Some(database) = self.db().as_mysql() {
            let connection = database
                .pool()
                .acquire()
                .await
                .map_err(|error| BaseError::DatabaseConnectionDbError(error.into()))?;
            return Ok(OwnedSchemaConnection::MySql(connection));
        }
        #[cfg(feature = "postgres")]
        if let Some(database) = self.db().as_postgres() {
            let connection = database
                .pool()
                .acquire()
                .await
                .map_err(|error| BaseError::DatabaseConnectionDbError(error.into()))?;
            return Ok(OwnedSchemaConnection::PostgreSql(connection));
        }
        Err(BaseError::DatabaseInitFailed(
            "DatabaseInitializer 未持有可用的数据库后端".to_string(),
        ))
    }
}

async fn sync_locked(
    driver: SchemaDriver,
    connection: &mut SchemaConnection<'_>,
    definitions: &[&TableDefinition],
) -> Result<SchemaSyncReport, BaseError> {
    let transactional = driver.transactional_ddl();
    if transactional {
        connection.begin().await?;
    }
    let result = execute_sync(driver, connection, definitions).await;
    if transactional {
        if result.is_ok() {
            connection.commit().await?;
        } else {
            // 回滚失败不覆盖原始 DDL 错误，调用方应优先处理根因。
            let _ = connection.rollback().await;
        }
    }
    result
}

async fn execute_sync(
    driver: SchemaDriver,
    connection: &mut SchemaConnection<'_>,
    definitions: &[&TableDefinition],
) -> Result<SchemaSyncReport, BaseError> {
    let mut report = SchemaSyncReport {
        tables: report_tables(definitions),
        changes: Vec::new(),
    };

    let plans = plan_locked(driver, connection, definitions).await?;
    let violations = driver.preflight(connection, definitions, &plans).await?;
    if !violations.is_empty() {
        return Err(BaseError::DatabaseInitFailed(format_violations(
            &violations,
        )));
    }
    for foreign_key_phase in [false, true] {
        for plan in &plans {
            for step in &plan.steps {
                if (step.change.kind == SchemaSyncChangeKind::AddedForeignKey) != foreign_key_phase
                {
                    continue;
                }
                for statement in &step.statements {
                    connection.execute(statement).await?;
                }
                tracing::info!(
                    table = %step.change.table,
                    object = %step.change.object,
                    kind = ?step.change.kind,
                    "数据库 schema 已同步"
                );
                report.changes.push(step.change.clone());
            }
        }
    }

    Ok(report)
}
