//! schema 同步的数据库驱动分派。
//!
//! [`SchemaDriver`] 按 [`yang_db::BackendKind`] 选择方言与连接类型；DDL 事务性与
//! advisory lock 实现也在此分派。MySQL 行为与历史实现一致，PostgreSQL 使用
//! `pg_advisory_lock(bigint)` 与单事务 DDL。

use super::dialect::SchemaDialect;
use super::model::{ExistingTableSchema, TableSyncPlan};
use super::preflight::preflight_locked;
use super::SchemaDataViolation;
use crate::error::BaseError;
use crate::table::TableDefinition;
use yang_db::BackendKind;

/// schema 同步锁等待上限（秒）。
const LOCK_TIMEOUT_SECONDS: u64 = 30;

/// schema 同步驱动。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SchemaDriver {
    /// MySQL。
    #[cfg(feature = "mysql")]
    MySql,
    /// PostgreSQL。
    #[cfg(feature = "postgres")]
    PostgreSql,
}

/// 借出的 schema 同步连接。
pub(crate) enum SchemaConnection<'a> {
    #[cfg(feature = "mysql")]
    MySql(&'a mut sqlx::MySqlConnection),
    #[cfg(feature = "postgres")]
    PostgreSql(&'a mut sqlx::PgConnection),
}

/// 从连接池借出的连接；保持生命周期，供同步入口持有。
pub(crate) enum OwnedSchemaConnection {
    #[cfg(feature = "mysql")]
    MySql(sqlx::pool::PoolConnection<sqlx::MySql>),
    #[cfg(feature = "postgres")]
    PostgreSql(sqlx::pool::PoolConnection<sqlx::Postgres>),
}

impl OwnedSchemaConnection {
    /// 借出底层 sqlx 连接。
    pub(crate) fn as_schema(&mut self) -> SchemaConnection<'_> {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql(connection) => SchemaConnection::MySql(connection),
            #[cfg(feature = "postgres")]
            Self::PostgreSql(connection) => SchemaConnection::PostgreSql(connection),
        }
    }
}

/// advisory lock 句柄。
#[derive(Debug)]
pub(crate) enum SchemaLock {
    #[cfg(feature = "mysql")]
    MySql(String),
    #[cfg(feature = "postgres")]
    PostgreSql(i64),
}

impl SchemaConnection<'_> {
    /// 执行一条 DDL/DML。
    pub(crate) async fn execute(&mut self, statement: &str) -> Result<(), BaseError> {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql(connection) => sqlx::query(statement)
                .execute(&mut **connection)
                .await
                .map(|_| ())
                .map_err(|error| BaseError::DatabaseExecuteFailed(error.into())),
            #[cfg(feature = "postgres")]
            Self::PostgreSql(connection) => sqlx::query(statement)
                .execute(&mut **connection)
                .await
                .map(|_| ())
                .map_err(|error| BaseError::DatabaseExecuteFailed(error.into())),
        }
    }

    /// 读取单列文本结果（预检 SQL 使用）。
    pub(crate) async fn fetch_text_column(
        &mut self,
        statement: &str,
    ) -> Result<Vec<String>, BaseError> {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql(connection) => sqlx::query_scalar::<_, String>(statement)
                .fetch_all(&mut **connection)
                .await
                .map_err(|error| BaseError::DatabaseQueryFailed(error.into())),
            #[cfg(feature = "postgres")]
            Self::PostgreSql(connection) => sqlx::query_scalar::<_, String>(statement)
                .fetch_all(&mut **connection)
                .await
                .map_err(|error| BaseError::DatabaseQueryFailed(error.into())),
        }
    }

    /// 开启事务（仅 PostgreSQL DDL 使用）。
    pub(crate) async fn begin(&mut self) -> Result<(), BaseError> {
        self.execute("BEGIN").await
    }

    /// 提交事务。
    pub(crate) async fn commit(&mut self) -> Result<(), BaseError> {
        self.execute("COMMIT").await
    }

    /// 回滚事务；失败不覆盖原始错误。
    pub(crate) async fn rollback(&mut self) -> Result<(), BaseError> {
        self.execute("ROLLBACK").await
    }
}

impl SchemaDriver {
    /// 根据运行时后端选择驱动。
    pub(crate) fn for_backend(backend: BackendKind) -> Result<Self, BaseError> {
        match backend {
            #[cfg(feature = "mysql")]
            BackendKind::MySql => Ok(Self::MySql),
            #[cfg(feature = "postgres")]
            BackendKind::PostgreSql => Ok(Self::PostgreSql),
            #[allow(unreachable_patterns)]
            _ => Err(BaseError::DatabaseInitFailed(format!(
                "schema 同步不支持数据库后端: {backend:?}"
            ))),
        }
    }

    /// 对应方言。
    pub(crate) fn dialect(self) -> SchemaDialect {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql => SchemaDialect::MySql,
            #[cfg(feature = "postgres")]
            Self::PostgreSql => SchemaDialect::PostgreSql,
        }
    }

    /// DDL 是否在单事务中执行；MySQL 的 DDL 隐式提交，故为 `false`。
    pub(crate) fn transactional_ddl(self) -> bool {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql => false,
            #[cfg(feature = "postgres")]
            Self::PostgreSql => true,
        }
    }

    /// 读取现有表结构。
    pub(crate) async fn introspect(
        self,
        connection: &mut SchemaConnection<'_>,
        definition: &TableDefinition,
    ) -> Result<ExistingTableSchema, BaseError> {
        match (self, connection) {
            #[cfg(feature = "mysql")]
            (Self::MySql, SchemaConnection::MySql(connection)) => {
                super::inspect::load_existing_schema(connection, definition).await
            }
            #[cfg(feature = "postgres")]
            (Self::PostgreSql, SchemaConnection::PostgreSql(connection)) => {
                super::pg::load_existing_schema(connection, definition).await
            }
            #[allow(unreachable_patterns)]
            _ => Err(BaseError::DatabaseInitFailed(
                "schema driver 与数据库连接后端不一致".to_string(),
            )),
        }
    }

    /// 渲染单条 `CREATE TABLE`。
    #[allow(dead_code)]
    pub(crate) fn render_create_table(
        self,
        table: &crate::table::TableConfig,
        include_foreign_keys: bool,
    ) -> Result<String, BaseError> {
        self.dialect()
            .render_create_table(table, include_foreign_keys)
    }

    /// 计算变更计划。
    pub(crate) fn plan_changes(
        self,
        definition: &TableDefinition,
        existing: &ExistingTableSchema,
    ) -> Result<TableSyncPlan, BaseError> {
        super::plan::plan_table_sync_with(self.dialect(), definition, existing)
    }

    /// 只读预检。
    pub(crate) async fn preflight(
        self,
        connection: &mut SchemaConnection<'_>,
        definitions: &[&TableDefinition],
        plans: &[TableSyncPlan],
    ) -> Result<Vec<SchemaDataViolation>, BaseError> {
        preflight_locked(self.dialect(), connection, definitions, plans).await
    }

    /// 获取数据库级 advisory lock。
    pub(crate) async fn lock(
        self,
        connection: &mut SchemaConnection<'_>,
    ) -> Result<SchemaLock, BaseError> {
        match (self, connection) {
            #[cfg(feature = "mysql")]
            (Self::MySql, SchemaConnection::MySql(connection)) => {
                let database_name: Option<String> = sqlx::query_scalar("SELECT DATABASE()")
                    .fetch_one(&mut **connection)
                    .await
                    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
                let database_name = database_name.ok_or_else(|| {
                    BaseError::DatabaseInitFailed("当前 MySQL 连接没有选定数据库".to_string())
                })?;
                let lock_name = mysql_lock_name(&database_name);
                let acquired: Option<i64> = sqlx::query_scalar("SELECT GET_LOCK(?, ?)")
                    .bind(&lock_name)
                    .bind(LOCK_TIMEOUT_SECONDS as i64)
                    .fetch_one(&mut **connection)
                    .await
                    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
                if acquired != Some(1) {
                    return Err(BaseError::DatabaseInitFailed(format!(
                        "等待 schema 同步锁超时: {lock_name}"
                    )));
                }
                Ok(SchemaLock::MySql(lock_name))
            }
            #[cfg(feature = "postgres")]
            (Self::PostgreSql, SchemaConnection::PostgreSql(connection)) => {
                let database_name: String = sqlx::query_scalar("SELECT current_database()")
                    .fetch_one(&mut **connection)
                    .await
                    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
                let key = postgres_lock_key(&database_name);
                let deadline = std::time::Instant::now()
                    + std::time::Duration::from_secs(LOCK_TIMEOUT_SECONDS);
                loop {
                    let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
                        .bind(key)
                        .fetch_one(&mut **connection)
                        .await
                        .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
                    if acquired {
                        return Ok(SchemaLock::PostgreSql(key));
                    }
                    if std::time::Instant::now() >= deadline {
                        return Err(BaseError::DatabaseInitFailed(format!(
                            "等待 schema 同步锁超时: pg_advisory_lock({key})"
                        )));
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
            }
            #[allow(unreachable_patterns)]
            _ => Err(BaseError::DatabaseInitFailed(
                "schema driver 与数据库连接后端不一致".to_string(),
            )),
        }
    }

    /// 释放 advisory lock。
    pub(crate) async fn unlock(
        self,
        connection: &mut SchemaConnection<'_>,
        lock: &SchemaLock,
    ) -> Result<(), BaseError> {
        match (self, connection, lock) {
            #[cfg(feature = "mysql")]
            (Self::MySql, SchemaConnection::MySql(connection), SchemaLock::MySql(lock_name)) => {
                let released: Option<i64> = sqlx::query_scalar("SELECT RELEASE_LOCK(?)")
                    .bind(lock_name)
                    .fetch_one(&mut **connection)
                    .await
                    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
                if released != Some(1) {
                    return Err(BaseError::DatabaseInitFailed(format!(
                        "schema 同步锁释放失败: {lock_name}"
                    )));
                }
                Ok(())
            }
            #[cfg(feature = "postgres")]
            (
                Self::PostgreSql,
                SchemaConnection::PostgreSql(connection),
                SchemaLock::PostgreSql(key),
            ) => {
                let released: bool = sqlx::query_scalar("SELECT pg_advisory_unlock($1)")
                    .bind(key)
                    .fetch_one(&mut **connection)
                    .await
                    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
                if !released {
                    return Err(BaseError::DatabaseInitFailed(format!(
                        "schema 同步锁释放失败: pg_advisory_lock({key})"
                    )));
                }
                Ok(())
            }
            #[allow(unreachable_patterns)]
            _ => Err(BaseError::DatabaseInitFailed(
                "schema driver 与数据库连接后端不一致".to_string(),
            )),
        }
    }
}

/// 由数据库名生成 MySQL advisory lock 名称。
#[cfg(feature = "mysql")]
fn mysql_lock_name(database_name: &str) -> String {
    format!(
        "yang_base_schema_{:016x}",
        fnv1a_64(database_name.as_bytes())
    )
}

/// 由数据库名生成 PostgreSQL bigint advisory lock key。
#[cfg(feature = "postgres")]
fn postgres_lock_key(database_name: &str) -> i64 {
    fnv1a_64(database_name.as_bytes()) as i64
}

/// FNV-1a 64 位哈希；仅用于锁名，不追求密码学强度。
#[cfg(any(feature = "mysql", feature = "postgres"))]
fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn driver_dispatch_follows_backend_and_ddl_transactionality() {
        #[cfg(feature = "mysql")]
        {
            let driver = SchemaDriver::for_backend(BackendKind::MySql).expect("MySQL 驱动应可用");
            assert_eq!(driver.dialect(), SchemaDialect::MySql);
            assert!(!driver.transactional_ddl());
            assert_eq!(mysql_lock_name("test"), mysql_lock_name("test"));
        }
        #[cfg(feature = "postgres")]
        {
            let driver =
                SchemaDriver::for_backend(BackendKind::PostgreSql).expect("PostgreSQL 驱动应可用");
            assert_eq!(driver.dialect(), SchemaDialect::PostgreSql);
            assert!(driver.transactional_ddl());
            assert_eq!(postgres_lock_key("test"), postgres_lock_key("test"));
        }
        // Redis 没有 schema 同步驱动，必须 fail-closed。
        assert!(SchemaDriver::for_backend(BackendKind::Redis).is_err());
    }
}
