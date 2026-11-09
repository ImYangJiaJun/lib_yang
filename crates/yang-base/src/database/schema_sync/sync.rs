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
                Ok(driver.dialect().validate_schema(config.as_ref(), &existing))
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
        let (statements, deferred_statements) = plan_statements(&plans);
        Ok(SchemaSyncReport {
            tables: report_tables(&definitions),
            changes: plans.into_iter().flat_map(|plan| plan.changes).collect(),
            statements,
            deferred_statements,
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
        let (statements, deferred_statements) = plan_statements(&plans);
        let violations =
            preflight_locked(driver.dialect(), &mut connection, &definitions, &plans).await?;
        Ok(SchemaPreflightReport {
            plan: SchemaSyncReport {
                tables: report_tables(&definitions),
                changes: plans
                    .iter()
                    .flat_map(|plan| plan.changes.iter().cloned())
                    .collect(),
                statements,
                deferred_statements,
            },
            violations,
        })
    }

    /// 同步一组不可变表定义。
    ///
    /// 多实例启动使用数据库级 advisory lock 串行化：MySQL 用 `GET_LOCK`，
    /// PostgreSQL 用 `pg_advisory_lock(bigint)`（session 级）。PostgreSQL 按阶段
    /// 执行：地基 DDL（建表/加列）→ 同事务预检 → 其余事务步骤 → 提交 →
    /// 并发索引与非外键 `VALIDATE CONSTRAINT` → 外键事务（`ADD ... NOT VALID`）→
    /// 提交后执行外键 `VALIDATE CONSTRAINT`。
    ///
    /// Foundation 与 Main 阶段共享同一事务，整体提交或回滚；Concurrent 与外键阶段
    /// 在提交后以 autocommit 执行，失败不会回滚已提交的 DDL，只会留下可重试状态
    /// （INVALID 索引 / `NOT VALID` 约束），下次同步会自动自愈。
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
        // 复位会话级 lock_timeout：连接归还池后会被业务查询复用。
        let reset_result = driver.reset_session(&mut connection).await;
        let release_result = driver.unlock(&mut connection, &lock).await;

        let result = match result {
            Ok(report) => reset_result.map(|()| report),
            Err(error) => {
                // 同步失败时保留原始错误；复位失败只记录（连接随后仍会归还池）。
                if let Err(reset_error) = &reset_result {
                    tracing::warn!(
                        error = %reset_error,
                        "schema 同步失败后复位会话参数失败"
                    );
                }
                Err(error)
            }
        };

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

/// 步骤的执行阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepPhase {
    /// 地基步骤（建表/加列）：必须先落地，预检 SQL 才能看到新列。
    Foundation,
    /// 其余事务内、非外键步骤。
    Main,
    /// 事务提交后以 autocommit 执行：并发索引 + 非外键步骤的 deferred 语句。
    Concurrent,
    /// 外键事务（`ADD ... NOT VALID`）。
    ForeignKeys,
    /// 外键事务提交后执行 FK 步骤的 deferred 语句（`VALIDATE CONSTRAINT`）。
    PostForeignKeys,
    /// MySQL：全部非外键步骤（预检先于 DDL，无需地基阶段）。
    AllNonForeignKey,
}

impl StepPhase {
    /// 本阶段是否执行该步骤的 `statements`。
    fn runs_statements(self, step: &super::model::PlanStep) -> bool {
        let is_foreign_key = matches!(
            step.change.kind,
            SchemaSyncChangeKind::AddedForeignKey | SchemaSyncChangeKind::ValidatedForeignKey
        );
        match self {
            Self::Foundation => {
                !step.concurrent
                    && !is_foreign_key
                    && matches!(
                        step.change.kind,
                        SchemaSyncChangeKind::CreatedTable | SchemaSyncChangeKind::AddedColumn
                    )
            }
            Self::Main => {
                !step.concurrent
                    && !is_foreign_key
                    && !matches!(
                        step.change.kind,
                        SchemaSyncChangeKind::CreatedTable | SchemaSyncChangeKind::AddedColumn
                    )
            }
            Self::Concurrent => step.concurrent,
            Self::ForeignKeys => !step.concurrent && is_foreign_key,
            Self::PostForeignKeys => false,
            Self::AllNonForeignKey => !step.concurrent && !is_foreign_key,
        }
    }

    /// 本阶段是否执行该步骤的 deferred 语句（仅提交后阶段）。
    ///
    /// `Concurrent` 只处理非外键步骤；外键的 `VALIDATE` 属于 `PostForeignKeys`，
    /// 在外键事务提交后单独执行。
    fn runs_deferred(self, step: &super::model::PlanStep) -> bool {
        if step.deferred_statements.is_empty() {
            return false;
        }
        let is_foreign_key = matches!(
            step.change.kind,
            SchemaSyncChangeKind::AddedForeignKey | SchemaSyncChangeKind::ValidatedForeignKey
        );
        match self {
            Self::Concurrent => !is_foreign_key,
            Self::PostForeignKeys => is_foreign_key,
            _ => false,
        }
    }
}

async fn sync_locked(
    driver: SchemaDriver,
    connection: &mut SchemaConnection<'_>,
    definitions: &[&TableDefinition],
) -> Result<SchemaSyncReport, BaseError> {
    driver.configure_session(connection).await?;
    let plans = plan_locked(driver, connection, definitions).await?;

    if driver.transactional_ddl() {
        // PostgreSQL：地基步骤 -> 同事务内预检（能看到新列）-> 其余事务步骤
        // -> 提交 -> 并发索引与 deferred VALIDATE -> 外键事务。
        connection.begin().await?;
        let transaction = async {
            execute_steps(connection, &plans, StepPhase::Foundation).await?;
            let violations = driver.preflight(connection, definitions, &plans).await?;
            if !violations.is_empty() {
                return Err(BaseError::DatabaseInitFailed(format_violations(
                    &violations,
                )));
            }
            execute_steps(connection, &plans, StepPhase::Main).await
        }
        .await;
        match transaction {
            Ok(()) => connection.commit().await?,
            Err(error) => {
                // 回滚失败不覆盖原始 DDL/预检错误，调用方应优先处理根因。
                let _ = connection.rollback().await;
                return Err(error);
            }
        }
        // CREATE/DROP INDEX CONCURRENTLY 不能在事务块内执行；非外键步骤的
        // deferred `VALIDATE CONSTRAINT`（CHECK 等）也在此阶段以 autocommit 执行。
        execute_steps(connection, &plans, StepPhase::Concurrent).await?;
        // 外键最后执行，确保它依赖的唯一索引（可能刚并发创建）已存在。
        if plans
            .iter()
            .flat_map(|plan| &plan.steps)
            .any(|step| step.change.kind == SchemaSyncChangeKind::AddedForeignKey)
        {
            connection.begin().await?;
            match execute_steps(connection, &plans, StepPhase::ForeignKeys).await {
                Ok(()) => connection.commit().await?,
                Err(error) => {
                    let _ = connection.rollback().await;
                    return Err(error);
                }
            }
        }
        // 外键事务提交后再 VALIDATE，避免 ADD 的 ShareRowExclusiveLock 持有到验证结束；
        // 仅需自愈验证的既有外键也会在这里执行。
        execute_steps(connection, &plans, StepPhase::PostForeignKeys).await?;
    } else {
        // MySQL：保持既有顺序，预检先于全部 DDL。
        let violations = driver.preflight(connection, definitions, &plans).await?;
        if !violations.is_empty() {
            return Err(BaseError::DatabaseInitFailed(format_violations(
                &violations,
            )));
        }
        execute_steps(connection, &plans, StepPhase::AllNonForeignKey).await?;
        execute_steps(connection, &plans, StepPhase::ForeignKeys).await?;
    }

    // 报告按计划顺序生成，保证每个 step 恰好一条且顺序稳定（与执行阶段无关）。
    let (statements, deferred_statements) = plan_statements(&plans);
    Ok(SchemaSyncReport {
        tables: report_tables(definitions),
        changes: plans
            .iter()
            .flat_map(|plan| plan.steps.iter().map(|step| step.change.clone()))
            .collect(),
        statements,
        deferred_statements,
    })
}

/// 汇总计划中的事务内与 deferred DDL 文本，供同步/dry-run 报告审计。
fn plan_statements(plans: &[super::model::TableSyncPlan]) -> (Vec<String>, Vec<String>) {
    let mut statements = Vec::new();
    let mut deferred_statements = Vec::new();
    for plan in plans {
        for step in &plan.steps {
            statements.extend(step.statements.iter().cloned());
            deferred_statements.extend(step.deferred_statements.iter().cloned());
        }
    }
    (statements, deferred_statements)
}

async fn execute_steps(
    connection: &mut SchemaConnection<'_>,
    plans: &[super::model::TableSyncPlan],
    phase: StepPhase,
) -> Result<(), BaseError> {
    for plan in plans {
        for step in &plan.steps {
            let run_statements = phase.runs_statements(step);
            let run_deferred = phase.runs_deferred(step);
            if !run_statements && !run_deferred {
                continue;
            }
            if run_statements {
                for statement in &step.statements {
                    connection.execute(statement).await?;
                }
            }
            if run_deferred {
                for statement in &step.deferred_statements {
                    connection.execute(statement).await?;
                }
            }
            tracing::info!(
                table = %step.change.table,
                object = %step.change.object,
                kind = ?step.change.kind,
                phase = ?phase,
                "数据库 schema 已同步"
            );
        }
    }
    Ok(())
}
