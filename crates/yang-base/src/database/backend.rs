//! 运行时数据库后端容器。
//!
//! [`AnyDatabase`] 把编译期可选的 MySQL / PostgreSQL 连接池收敛为一个运行时值，
//! 供 [`crate::tools::Tools`] 以同一槽位持有；[`AnyPool`] 是它的可克隆连接池视图，
//! [`AnyTransaction`] 是两种后端事务的统一句柄。本模块同时提供后端身份、能力契约
//! 与生命周期管理（健康检查 / 关闭 / 连接池状态）。
//!
//! 后端身份与能力契约来自 `yang-db` 的单一事实源：
//! [`yang_db::BackendKind`] 与 [`yang_db::BackendCapabilities`]。

use std::sync::Arc;
use yang_db::{BackendCapabilities, BackendKind, DbError, PoolStatus};

/// 持有单个关系数据库连接池的运行时容器。
///
/// 使用 `#[non_exhaustive]`：内部后端集合随 feature 变化，调用方不得按结构体字面量
/// 构造，必须通过 `AnyDatabase::mysql` / `AnyDatabase::postgres` 构造（按 feature 启用）。
#[non_exhaustive]
pub struct AnyDatabase {
    inner: AnyDatabaseKind,
}

/// 内部后端分派；各变体按 feature 独占编译。
enum AnyDatabaseKind {
    #[cfg(feature = "mysql")]
    MySql(yang_db::mysql::Database),
    #[cfg(feature = "postgres")]
    PostgreSql(yang_db::postgres::Database),
}

impl std::fmt::Debug for AnyDatabase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AnyDatabase")
            .field("backend", &self.backend())
            .field("closed", &self.is_closed())
            .finish()
    }
}

impl AnyDatabase {
    /// 用 MySQL 连接池构造容器。
    #[cfg(feature = "mysql")]
    pub fn mysql(database: yang_db::mysql::Database) -> Self {
        Self {
            inner: AnyDatabaseKind::MySql(database),
        }
    }

    /// 用 PostgreSQL 连接池构造容器。
    #[cfg(feature = "postgres")]
    pub fn postgres(database: yang_db::postgres::Database) -> Self {
        Self {
            inner: AnyDatabaseKind::PostgreSql(database),
        }
    }

    /// 返回后端类型。
    pub fn backend(&self) -> BackendKind {
        match &self.inner {
            #[cfg(feature = "mysql")]
            AnyDatabaseKind::MySql(_) => BackendKind::MySql,
            #[cfg(feature = "postgres")]
            AnyDatabaseKind::PostgreSql(_) => BackendKind::PostgreSql,
        }
    }

    /// 返回后端静态能力契约。
    pub fn capabilities(&self) -> &'static BackendCapabilities {
        match &self.inner {
            #[cfg(feature = "mysql")]
            AnyDatabaseKind::MySql(_) => yang_db::mysql::Database::capabilities(),
            #[cfg(feature = "postgres")]
            AnyDatabaseKind::PostgreSql(_) => yang_db::postgres::Database::capabilities(),
        }
    }

    /// 连接池是否已关闭。
    pub fn is_closed(&self) -> bool {
        match &self.inner {
            #[cfg(feature = "mysql")]
            AnyDatabaseKind::MySql(database) => database.is_closed(),
            #[cfg(feature = "postgres")]
            AnyDatabaseKind::PostgreSql(database) => database.is_closed(),
        }
    }

    /// 健康检查：执行 `SELECT 1` 验证连接可用。
    pub async fn health_check(&self) -> Result<bool, DbError> {
        match &self.inner {
            #[cfg(feature = "mysql")]
            AnyDatabaseKind::MySql(database) => database.health_check().await,
            #[cfg(feature = "postgres")]
            AnyDatabaseKind::PostgreSql(database) => database.health_check().await,
        }
    }

    /// 幂等关闭连接池。
    pub async fn close(&self) {
        match &self.inner {
            #[cfg(feature = "mysql")]
            AnyDatabaseKind::MySql(database) => database.close().await,
            #[cfg(feature = "postgres")]
            AnyDatabaseKind::PostgreSql(database) => database.close().await,
        }
    }

    /// 返回连接池状态快照。
    pub fn pool_status(&self) -> PoolStatus {
        match &self.inner {
            #[cfg(feature = "mysql")]
            AnyDatabaseKind::MySql(database) => database.pool_status(),
            #[cfg(feature = "postgres")]
            AnyDatabaseKind::PostgreSql(database) => database.pool_status(),
        }
    }

    /// 当容器持有 MySQL 后端时返回其连接池引用。
    #[cfg(feature = "mysql")]
    pub fn as_mysql(&self) -> Option<&yang_db::mysql::Database> {
        match &self.inner {
            AnyDatabaseKind::MySql(database) => Some(database),
            #[cfg(feature = "postgres")]
            AnyDatabaseKind::PostgreSql(_) => None,
        }
    }

    /// 当容器持有 PostgreSQL 后端时返回其连接池引用。
    #[cfg(feature = "postgres")]
    pub fn as_postgres(&self) -> Option<&yang_db::postgres::Database> {
        match &self.inner {
            #[cfg(feature = "mysql")]
            AnyDatabaseKind::MySql(_) => None,
            AnyDatabaseKind::PostgreSql(database) => Some(database),
        }
    }

    /// 返回与当前后端共享同一连接池的可克隆句柄。
    ///
    /// sqlx 连接池内部基于 `Arc`，克隆只是引用计数递增；本方法返回的 [`AnyPool`]
    /// 可在请求作用域内自由复制，不会复制物理连接。
    pub fn pool(&self) -> AnyPool {
        match &self.inner {
            #[cfg(feature = "mysql")]
            AnyDatabaseKind::MySql(database) => AnyPool::MySql(Arc::new(database.pool().clone())),
            #[cfg(feature = "postgres")]
            AnyDatabaseKind::PostgreSql(database) => {
                AnyPool::Postgres(Arc::new(database.pool().clone()))
            }
        }
    }

    /// 在当前后端上开启事务。
    ///
    /// 返回的 [`AnyTransaction`] 可传入 `TableQuery` 的 `*_in_tx` 系列方法，也可通过
    /// [`AnyTransaction::commit`] / [`AnyTransaction::rollback`] 显式结束。未显式结束时
    /// 由 sqlx 在 drop 时尽力回滚。
    pub async fn begin(&self) -> Result<AnyTransaction, DbError> {
        match &self.inner {
            #[cfg(feature = "mysql")]
            AnyDatabaseKind::MySql(database) => {
                database.transaction().await.map(AnyTransaction::MySql)
            }
            #[cfg(feature = "postgres")]
            AnyDatabaseKind::PostgreSql(database) => {
                database.transaction().await.map(AnyTransaction::Postgres)
            }
        }
    }
}

/// 可克隆的连接池句柄，按运行时后端分派。
///
/// 由 [`AnyDatabase::pool`] 构造，供受保护查询层（[`crate::table::TableQuery`]）
/// 在请求期共享连接池。变体在编译期按 feature 独占存在。
#[derive(Clone)]
pub enum AnyPool {
    /// MySQL 连接池。
    #[cfg(feature = "mysql")]
    MySql(Arc<sqlx::MySqlPool>),
    /// PostgreSQL 连接池。
    #[cfg(feature = "postgres")]
    Postgres(Arc<sqlx::PgPool>),
}

impl std::fmt::Debug for AnyPool {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("AnyPool")
            .field(&self.backend())
            .finish()
    }
}

impl AnyPool {
    /// 返回连接池所属后端。
    pub fn backend(&self) -> BackendKind {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql(_) => BackendKind::MySql,
            #[cfg(feature = "postgres")]
            Self::Postgres(_) => BackendKind::PostgreSql,
        }
    }

    /// 当持有 MySQL 连接池时返回其引用。
    #[cfg(feature = "mysql")]
    pub fn as_mysql(&self) -> Option<&sqlx::MySqlPool> {
        match self {
            Self::MySql(pool) => Some(pool.as_ref()),
            #[cfg(feature = "postgres")]
            Self::Postgres(_) => None,
        }
    }

    /// 当持有 PostgreSQL 连接池时返回其引用。
    #[cfg(feature = "postgres")]
    pub fn as_postgres(&self) -> Option<&sqlx::PgPool> {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql(_) => None,
            Self::Postgres(pool) => Some(pool.as_ref()),
        }
    }
}

/// 两种后端事务的统一句柄。
///
/// 由 [`AnyDatabase::begin`] 或 [`crate::action::ActionContext::begin_transaction`]
/// 创建；`TableQuery` 的 `*_in_tx` 系列方法按变体分派到对应后端。
pub enum AnyTransaction {
    /// MySQL 事务。
    #[cfg(feature = "mysql")]
    MySql(yang_db::mysql::Transaction),
    /// PostgreSQL 事务。
    #[cfg(feature = "postgres")]
    Postgres(yang_db::postgres::Transaction),
}

impl AnyTransaction {
    /// 返回事务所属后端。
    pub fn backend(&self) -> BackendKind {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql(_) => BackendKind::MySql,
            #[cfg(feature = "postgres")]
            Self::Postgres(_) => BackendKind::PostgreSql,
        }
    }

    /// 当持有 MySQL 事务时返回其可变引用。
    #[cfg(feature = "mysql")]
    pub fn as_mysql(&mut self) -> Option<&mut yang_db::mysql::Transaction> {
        match self {
            Self::MySql(transaction) => Some(transaction),
            #[cfg(feature = "postgres")]
            Self::Postgres(_) => None,
        }
    }

    /// 当持有 PostgreSQL 事务时返回其可变引用。
    #[cfg(feature = "postgres")]
    pub fn as_postgres(&mut self) -> Option<&mut yang_db::postgres::Transaction> {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql(_) => None,
            Self::Postgres(transaction) => Some(transaction),
        }
    }

    /// 提交事务。
    pub async fn commit(self) -> Result<(), DbError> {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql(transaction) => transaction.commit().await,
            #[cfg(feature = "postgres")]
            Self::Postgres(transaction) => transaction.commit().await,
        }
    }

    /// 回滚事务。
    pub async fn rollback(self) -> Result<(), DbError> {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql(transaction) => transaction.rollback().await,
            #[cfg(feature = "postgres")]
            Self::Postgres(transaction) => transaction.rollback().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 用惰性连接池构造 MySQL 容器：不建立真实网络连接，仅验证运行时抽象。
    #[cfg(feature = "mysql")]
    fn mysql_database() -> AnyDatabase {
        let pool = sqlx::MySqlPool::connect_lazy("mysql://root:test@127.0.0.1:3306/test")
            .expect("惰性 MySQL 连接池应创建成功");
        let database =
            yang_db::mysql::Database::from_pool(pool, yang_db::mysql::DatabaseConfig::default())
                .expect("默认 MySQL 配置应通过校验");
        AnyDatabase::mysql(database)
    }

    /// 用惰性连接池构造 PostgreSQL 容器：不建立真实网络连接，仅验证运行时抽象。
    #[cfg(feature = "postgres")]
    fn postgres_database() -> AnyDatabase {
        let pool = sqlx::PgPool::connect_lazy("postgres://postgres:postgres@127.0.0.1:55432/test")
            .expect("惰性 PostgreSQL 连接池应创建成功");
        let database = yang_db::postgres::Database::from_pool(
            pool,
            yang_db::postgres::DatabaseConfig::default(),
        )
        .expect("默认 PostgreSQL 配置应通过校验");
        AnyDatabase::postgres(database)
    }

    #[cfg(feature = "mysql")]
    #[tokio::test]
    async fn mysql_backend_exposes_identity_capabilities_and_downcast() {
        let database = mysql_database();

        assert_eq!(database.backend(), BackendKind::MySql);
        assert!(database.capabilities().relational);
        assert!(database.as_mysql().is_some());
        assert!(!database.is_closed());
        assert_eq!(
            database.pool_status().max_size,
            yang_db::mysql::DatabaseConfig::default().max_connections as usize
        );

        database.close().await;
        assert!(database.is_closed());
    }

    #[cfg(feature = "postgres")]
    #[tokio::test]
    async fn postgres_backend_exposes_identity_capabilities_and_downcast() {
        let database = postgres_database();

        assert_eq!(database.backend(), BackendKind::PostgreSql);
        assert!(database.capabilities().relational);
        assert!(database.as_postgres().is_some());
        assert!(!database.is_closed());
        assert_eq!(
            database.pool_status().max_size,
            yang_db::postgres::DatabaseConfig::default().max_connections as usize
        );

        database.close().await;
        assert!(database.is_closed());
    }

    /// 同时启用两种后端时，`as_*` 必须对另一后端返回 `None`，避免静默误用。
    #[cfg(all(feature = "mysql", feature = "postgres"))]
    #[tokio::test]
    async fn downcasts_are_backend_exclusive() {
        let mysql = mysql_database();
        let postgres = postgres_database();

        assert!(mysql.as_postgres().is_none());
        assert!(postgres.as_mysql().is_none());

        mysql.close().await;
        postgres.close().await;
    }

    /// [`AnyDatabase::pool`] 必须返回与容器后端一致的共享句柄，且两后端互斥。
    #[cfg(feature = "mysql")]
    #[tokio::test]
    async fn mysql_pool_view_keeps_backend_identity() {
        let database = mysql_database();
        let pool = database.pool();

        assert_eq!(pool.backend(), BackendKind::MySql);
        assert!(pool.as_mysql().is_some());
        #[cfg(feature = "postgres")]
        assert!(pool.as_postgres().is_none());
    }

    /// [`AnyDatabase::pool`] 的 PostgreSQL 分支同样保持后端身份。
    #[cfg(feature = "postgres")]
    #[tokio::test]
    async fn postgres_pool_view_keeps_backend_identity() {
        let database = postgres_database();
        let pool = database.pool();

        assert_eq!(pool.backend(), BackendKind::PostgreSql);
        assert!(pool.as_postgres().is_some());
        #[cfg(feature = "mysql")]
        assert!(pool.as_mysql().is_none());
    }
}
