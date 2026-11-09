//! 运行时数据库后端容器。
//!
//! [`AnyDatabase`] 把编译期可选的 MySQL / PostgreSQL 连接池收敛为一个运行时值，
//! 供 [`crate::tools::Tools`] 以同一槽位持有；[`AnyPool`] 是它的可克隆连接池视图，
//! [`AnyTransaction`] 是两种后端事务的统一句柄。本模块同时提供后端身份、能力契约
//! 与生命周期管理（健康检查 / 关闭 / 连接池状态）。
//!
//! 后端身份与能力契约来自 `yang-db` 的单一事实源：
//! [`yang_db::BackendKind`] 与 [`yang_db::BackendCapabilities`]。

use crate::error::BaseError;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use yang_db::{
    BackendCapabilities, BackendCapability, BackendKind, DbError, PoolStatus, SqlErrorKind,
};

/// 持有单个关系数据库连接池的运行时容器。
///
/// 使用 `#[non_exhaustive]`：内部后端集合随 feature 变化，调用方不得按结构体字面量
/// 构造，必须通过 `AnyDatabase::mysql` / `AnyDatabase::postgres` 构造（按 feature 启用）。
#[non_exhaustive]
pub struct AnyDatabase {
    inner: AnyDatabaseKind,
}

/// 内部后端分派；各变体按 feature 独占编译。
///
/// PG 变体因携带可选的 TLS 证书路径而明显大于 MySQL 变体，触发
/// `clippy::large_enum_variant`。`AnyDatabase` 在 `Tools` 中通常只构造一次，
/// 为省下这些栈空间把 PG 变体 Box 化反而多一次堆分配，收益不抵成本。
#[allow(clippy::large_enum_variant)]
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

    /// 启动期能力门禁：校验后端是否支持全部 `required` 能力。
    ///
    /// 缺失任一能力时返回 [`BaseError::UnsupportedCapability`]，错误消息包含后端
    /// 类型与缺失能力名。调用方应在启动/初始化时一次性校验自己实际用到的能力集合，
    /// 而不是把检查散落到每个查询上——这样配置错误在启动期就 fail-fast，而不是执行
    /// 到方言专属语法时才暴露。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// database.require_capabilities(&[
    ///     BackendCapability::Select,
    ///     BackendCapability::Savepoint,
    /// ])?;
    /// ```
    pub fn require_capabilities(&self, required: &[BackendCapability]) -> Result<(), BaseError> {
        let capabilities = self.capabilities();
        for capability in required {
            if !capabilities.supports(*capability) {
                return Err(BaseError::UnsupportedCapability(format!(
                    "后端 {:?} 缺少必需能力: {capability:?}",
                    self.backend()
                )));
            }
        }
        Ok(())
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

    /// 整事务重试：按次数与指数退避重跑 `operation`，只重试序列化失败与死锁。
    ///
    /// 仅 PostgreSQL 的 `40001`（序列化失败）与 `40P01`（死锁）会重试，判定复用
    /// [`SqlErrorKind`] 的 SQLSTATE 归一，与 `yang-db` 的 `PgDatabase::retry_transaction`
    /// 口径一致。其他错误立即原样返回——包括约束冲突（如 `23505`）、
    /// [`DbError::ConnectionError`] 与 SQLSTATE `08`/`53` 类瞬时连接错误。提交阶段
    /// 断连时事务是否已生效未知，重试可能重复执行副作用，因此不属于可重试范围。
    ///
    /// `operation` 每次被调用都应自行开启并结束一个事务（例如 `self.begin()` →
    /// 业务语句 → `commit()`）。重试前 helper 按 `backoff * 2^(n-1)` 退避（最多放大
    /// 32 倍）；失败事务的回滚由被丢弃的事务句柄兜底。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let value = database
    ///     .retry_transaction(3, Duration::from_millis(50), || async {
    ///         let mut tx = database.begin().await?;
    ///         // ... 业务语句 ...
    ///         tx.commit().await?;
    ///         Ok(())
    ///     })
    ///     .await?;
    /// ```
    pub async fn retry_transaction<T, F, Fut>(
        &self,
        max_attempts: u32,
        backoff: Duration,
        mut operation: F,
    ) -> Result<T, DbError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, DbError>>,
    {
        let max_attempts = max_attempts.max(1);
        let mut attempt = 1u32;
        loop {
            match operation().await {
                Ok(value) => return Ok(value),
                Err(error) if attempt < max_attempts && is_retryable_transaction_error(&error) => {
                    let delay = backoff.saturating_mul(1u32 << (attempt - 1).min(5));
                    if !delay.is_zero() {
                        tokio::time::sleep(delay).await;
                    }
                    attempt += 1;
                }
                Err(error) => return Err(error),
            }
        }
    }
}

/// 整事务重试只接受序列化失败（40001）与死锁（40P01）。
///
/// SQLSTATE 归一复用 `yang-db` 的 [`SqlErrorKind`]，与 `PgDatabase::retry_transaction`
/// 保持一致。连接类（`08`/`53`）错误不在其列：提交阶段断连时事务结果不明，重试
/// 可能重复执行已生效的副作用。
fn is_retryable_transaction_error(error: &DbError) -> bool {
    matches!(
        error.sql_error().map(|info| info.kind()),
        Some(SqlErrorKind::SerializationFailure | SqlErrorKind::DeadlockDetected)
    )
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

    /// 在当前事务内创建保存点（与后端无关的封装）。
    ///
    /// PostgreSQL 走其后端原生 `SAVEPOINT`；MySQL 同样支持保存点语法，这里用
    /// 校验过的标识符拼接控制语句。`name` 必须是 ASCII 标识符（字母/下划线开头，
    /// 仅字母/数字/下划线），非法名称返回 [`DbError::InvalidArgument`]。
    pub async fn savepoint(&mut self, name: &str) -> Result<(), DbError> {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql(transaction) => {
                let sql = mysql_savepoint_statement("SAVEPOINT", name)?;
                mysql_execute_control(transaction, &sql).await
            }
            #[cfg(feature = "postgres")]
            Self::Postgres(transaction) => transaction.savepoint(name).await,
        }
    }

    /// 回滚到指定保存点（`ROLLBACK TO SAVEPOINT`）。
    ///
    /// 只撤销保存点之后的工作；更早的保存点与外层事务不受影响。
    pub async fn rollback_to(&mut self, name: &str) -> Result<(), DbError> {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql(transaction) => {
                let sql = mysql_savepoint_statement("ROLLBACK TO SAVEPOINT", name)?;
                mysql_execute_control(transaction, &sql).await
            }
            #[cfg(feature = "postgres")]
            Self::Postgres(transaction) => transaction.rollback_to(name).await,
        }
    }

    /// 释放保存点（`RELEASE SAVEPOINT`）。
    pub async fn release(&mut self, name: &str) -> Result<(), DbError> {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql(transaction) => {
                let sql = mysql_savepoint_statement("RELEASE SAVEPOINT", name)?;
                mysql_execute_control(transaction, &sql).await
            }
            #[cfg(feature = "postgres")]
            Self::Postgres(transaction) => transaction.release_savepoint(name).await,
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

/// 校验 MySQL 保存点名并拼出控制语句。
///
/// MySQL 的 `SAVEPOINT` 语法不接受绑定参数形式的标识符，故先按严格白名单
/// （1..=128 字节，字母/下划线开头，仅字母/数字/下划线）校验，再拼接成 SQL，
/// 从根上消除注入面。
#[cfg(feature = "mysql")]
fn mysql_savepoint_statement(keyword: &str, name: &str) -> Result<String, DbError> {
    let valid = !name.is_empty()
        && name.len() <= 128
        && name.bytes().enumerate().all(|(index, byte)| {
            if index == 0 {
                byte == b'_' || byte.is_ascii_alphabetic()
            } else {
                byte == b'_' || byte.is_ascii_alphanumeric()
            }
        });
    if !valid {
        return Err(DbError::InvalidArgument(format!(
            "MySQL 保存点名非法: {name:?}（1..=128 字节，字母/下划线开头，仅字母/数字/下划线）"
        )));
    }
    Ok(format!("{keyword} {name}"))
}

/// 在 MySQL 事务上执行一条控制语句（保存点相关，无可绑定值）。
#[cfg(feature = "mysql")]
async fn mysql_execute_control(
    transaction: &mut yang_db::mysql::Transaction,
    sql: &str,
) -> Result<(), DbError> {
    let connection = transaction
        .executor()
        .ok_or_else(|| DbError::TransactionError("事务已提交或回滚".to_string()))?;
    sqlx::query(sql).execute(connection).await?;
    Ok(())
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

    /// 能力门禁：缺失能力返回 `UnsupportedCapability`，且错误消息包含能力名。
    #[cfg(feature = "mysql")]
    #[tokio::test]
    async fn require_capabilities_reports_missing_capability_by_name() {
        let database = mysql_database();

        database
            .require_capabilities(&[BackendCapability::Select, BackendCapability::Insert])
            .expect("MySQL 应支持基础 CRUD 能力");

        let error = database
            .require_capabilities(&[BackendCapability::Savepoint])
            .expect_err("MySQL 能力矩阵未声明 Savepoint，应 fail-fast");
        assert!(matches!(error, BaseError::UnsupportedCapability(_)));
        assert!(
            error.to_string().contains("Savepoint"),
            "错误消息应包含缺失能力名，实得: {error}"
        );
    }

    /// 能力门禁：PostgreSQL 声明的保存点能力应通过校验。
    #[cfg(feature = "postgres")]
    #[tokio::test]
    async fn postgres_declares_savepoint_capability() {
        let database = postgres_database();
        database
            .require_capabilities(&[BackendCapability::Savepoint])
            .expect("PostgreSQL 应声明保存点能力");
    }

    fn retryable_error() -> DbError {
        sqlstate_error(
            "40001",
            "could not serialize access due to concurrent update",
        )
    }

    fn sqlstate_error(code: &str, message: &str) -> DbError {
        DbError::DatabaseError(Box::new(yang_db::SqlErrorInfo::new(code, message)))
    }

    fn non_retryable_error() -> DbError {
        DbError::QueryError("syntax error".to_string())
    }

    /// 整事务重试：可重试错误按次数退避重试，成功后返回结果。
    #[tokio::test]
    async fn retry_transaction_retries_retryable_errors() {
        let database = instantiate_for_retry();

        let mut attempts = 0;
        let result = database
            .retry_transaction(3, Duration::ZERO, || {
                attempts += 1;
                let attempt = attempts;
                async move {
                    if attempt < 3 {
                        Err(retryable_error())
                    } else {
                        Ok(attempt)
                    }
                }
            })
            .await;

        assert_eq!(result.expect("第三次应成功"), 3);
        assert_eq!(attempts, 3);
    }

    /// 整事务重试：不可重试错误立即返回，只执行一次。
    #[tokio::test]
    async fn retry_transaction_returns_non_retryable_error_immediately() {
        let database = instantiate_for_retry();

        let mut attempts = 0;
        let error = database
            .retry_transaction(5, Duration::ZERO, || {
                attempts += 1;
                async move { Err::<(), _>(non_retryable_error()) }
            })
            .await
            .expect_err("不可重试错误应直接返回");

        assert!(matches!(error, DbError::QueryError(_)));
        assert_eq!(attempts, 1);
    }

    /// 整事务重试：用尽次数后返回最后一次可重试错误。
    #[tokio::test]
    async fn retry_transaction_stops_after_max_attempts() {
        let database = instantiate_for_retry();

        let mut attempts = 0;
        let error = database
            .retry_transaction(2, Duration::ZERO, || {
                attempts += 1;
                async move { Err::<(), _>(retryable_error()) }
            })
            .await
            .expect_err("用尽次数后应返回错误");

        assert!(error.is_retryable());
        assert_eq!(attempts, 2);
    }

    /// 死锁（40P01）同样属于整事务可重试错误。
    #[tokio::test]
    async fn retry_transaction_retries_deadlock() {
        let database = instantiate_for_retry();

        let mut attempts = 0;
        let result = database
            .retry_transaction(3, Duration::ZERO, || {
                attempts += 1;
                let attempt = attempts;
                async move {
                    if attempt < 3 {
                        Err(sqlstate_error("40P01", "deadlock detected"))
                    } else {
                        Ok(attempt)
                    }
                }
            })
            .await;

        assert_eq!(result.expect("第三次应成功"), 3);
        assert_eq!(attempts, 3);
    }

    /// 唯一约束冲突（23505）是确定性错误，只执行一次。
    #[tokio::test]
    async fn retry_transaction_does_not_retry_unique_violation() {
        let database = instantiate_for_retry();

        let mut attempts = 0;
        let error = database
            .retry_transaction(5, Duration::ZERO, || {
                attempts += 1;
                async move { Err::<(), _>(sqlstate_error("23505", "duplicate key value")) }
            })
            .await
            .expect_err("唯一约束冲突不应重试");

        assert_eq!(error.sqlstate(), Some("23505"));
        assert_eq!(attempts, 1);
    }

    /// 连接错误不在整事务重试范围（提交阶段断连结果不明），只执行一次。
    #[tokio::test]
    async fn retry_transaction_does_not_retry_connection_errors() {
        let database = instantiate_for_retry();

        let mut attempts = 0;
        let error = database
            .retry_transaction(5, Duration::ZERO, || {
                attempts += 1;
                async move {
                    Err::<(), _>(DbError::ConnectionError(
                        "connection closed during commit".to_string(),
                    ))
                }
            })
            .await
            .expect_err("连接错误不应重试");

        assert!(matches!(error, DbError::ConnectionError(_)));
        assert_eq!(attempts, 1);
    }

    /// SQLSTATE 08 类连接失败同样不参与整事务重试。
    #[tokio::test]
    async fn retry_transaction_does_not_retry_connection_class_sqlstate() {
        let database = instantiate_for_retry();

        let mut attempts = 0;
        let error = database
            .retry_transaction(5, Duration::ZERO, || {
                attempts += 1;
                async move { Err::<(), _>(sqlstate_error("08006", "connection failure")) }
            })
            .await
            .expect_err("08 类连接失败不应重试");

        assert_eq!(error.sqlstate(), Some("08006"));
        assert_eq!(attempts, 1);
    }

    /// `max_attempts = 0` 维持既有语义：至少执行一次，不因参数为 0 而跳过。
    #[tokio::test]
    async fn retry_transaction_zero_max_attempts_runs_once() {
        let database = instantiate_for_retry();

        let mut attempts = 0;
        let error = database
            .retry_transaction(0, Duration::ZERO, || {
                attempts += 1;
                async move { Err::<(), _>(retryable_error()) }
            })
            .await
            .expect_err("零次数仍应执行一次并返回错误");

        assert!(error.is_retryable());
        assert_eq!(attempts, 1);
    }

    /// 重试 helper 与后端无关；借一个惰性容器即可，不建立真实连接。
    fn instantiate_for_retry() -> AnyDatabase {
        #[cfg(feature = "mysql")]
        {
            mysql_database()
        }
        #[cfg(all(feature = "postgres", not(feature = "mysql")))]
        {
            postgres_database()
        }
    }

    /// MySQL 保存点名白名单拒绝注入形态，合法名称原样拼入控制语句。
    #[cfg(feature = "mysql")]
    #[test]
    fn mysql_savepoint_statement_validates_names() {
        assert_eq!(
            mysql_savepoint_statement("SAVEPOINT", "sp_1").expect("合法名称应通过"),
            "SAVEPOINT sp_1"
        );
        for invalid in ["", "1sp", "sp-1", "sp; DROP TABLE t", "名字"] {
            assert!(
                matches!(
                    mysql_savepoint_statement("SAVEPOINT", invalid),
                    Err(DbError::InvalidArgument(_))
                ),
                "非法保存点名 {invalid:?} 应被拒绝"
            );
        }
    }
}
