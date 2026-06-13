//! 全局数据库访问器
//!
//! 提供线程安全的全局数据库实例访问。
//!
//! # 设计说明
//!
//! GlobalDatabase 是对 yang-db::Database 的封装，使用 OnceLock 实现全局单例模式。
//! 所有数据库操作都通过 yang-db 库实现，确保类型安全和统一的数据库访问接口。
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::database::GlobalDatabase;
//! use yang_db::DatabaseConfig;
//!
//! // 初始化全局数据库
//! GlobalDatabase::init("mysql://user:pass@localhost/db", DatabaseConfig::default()).await?;
//!
//! // 使用查询构建器
//! let users = GlobalDatabase::table("users")?
//!     .field("id")
//!     .field("name")
//!     .select::<User>()
//!     .await?;
//!
//! // 执行原生查询
//! let result: Vec<User> = GlobalDatabase::query("SELECT * FROM users").await?;
//!
//! // 执行原生语句
//! let affected = GlobalDatabase::execute("DELETE FROM users WHERE id = 1").await?;
//!
//! // 使用事务
//! let mut tx = GlobalDatabase::transaction().await?;
//! tx.execute("INSERT INTO users (name) VALUES ('Alice')").await?;
//! tx.commit().await?;
//! ```

use crate::error::BaseError;
use std::sync::OnceLock;
use yang_db::{Database, DatabaseConfig, QueryBuilder, Transaction};

/// 全局数据库实例
///
/// 使用 OnceLock 确保线程安全的单例模式
static GLOBAL_DB: OnceLock<Database> = OnceLock::new();

/// 全局数据库访问器
///
/// 封装 yang-db::Database，提供全局静态访问接口。
/// 所有数据库操作都通过 yang-db 库实现。
pub struct GlobalDatabase;

impl GlobalDatabase {
    /// 初始化全局数据库
    ///
    /// # 参数
    ///
    /// - `url`: 数据库连接字符串，格式：`mysql://user:password@host:port/database`
    /// - `config`: 数据库配置，包含连接池参数
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 初始化成功
    /// - `Err(BaseError)`: 初始化失败
    ///
    /// # 错误
    ///
    /// - `DatabaseConnectionFailed`: 数据库连接失败
    /// - `DatabaseAlreadyInitialized`: 数据库已经初始化
    ///
    /// # 说明
    ///
    /// 使用 yang-db::Database::connect_with_config 创建数据库连接。
    /// 此方法只能调用一次，重复调用将返回错误。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::database::GlobalDatabase;
    /// use yang_db::DatabaseConfig;
    ///
    /// let config = DatabaseConfig {
    ///     max_connections: 20,
    ///     connect_timeout: 10,
    ///     idle_timeout: 300,
    ///     enable_logging: true,
    /// };
    ///
    /// GlobalDatabase::init("mysql://root:password@localhost/test", config).await?;
    /// ```
    pub async fn init(url: &str, config: DatabaseConfig) -> Result<(), BaseError> {
        // 使用 yang-db::Database::connect_with_config 创建数据库连接
        let db = Database::connect_with_config(url, config)
            .await
            .map_err(|e| BaseError::DatabaseConnectionFailed(e.to_string()))?;

        // 设置全局数据库实例
        GLOBAL_DB
            .set(db)
            .map_err(|_| BaseError::DatabaseAlreadyInitialized)?;

        log::info!("全局数据库已初始化");
        Ok(())
    }

    /// 获取全局数据库实例
    ///
    /// # 返回
    ///
    /// - `Ok(&'static Database)`: yang-db::Database 实例的静态引用
    /// - `Err(BaseError)`: 数据库未初始化
    ///
    /// # 错误
    ///
    /// - `DatabaseNotInitialized`: 数据库未初始化，需要先调用 `init` 方法
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let db = GlobalDatabase::get()?;
    /// let builder = db.table("users");
    /// ```
    pub fn get() -> Result<&'static Database, BaseError> {
        GLOBAL_DB.get().ok_or(BaseError::DatabaseNotInitialized)
    }

    /// 选择表，返回查询构建器
    ///
    /// # 参数
    ///
    /// - `table_name`: 表名
    ///
    /// # 返回
    ///
    /// - `Ok(QueryBuilder)`: yang-db 查询构建器
    /// - `Err(BaseError)`: 数据库未初始化
    ///
    /// # 说明
    ///
    /// 调用 yang-db::Database::table 方法创建查询构建器。
    /// 返回的 QueryBuilder 可以链式调用各种查询方法。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // 查询用户
    /// let users = GlobalDatabase::table("users")?
    ///     .field("id")
    ///     .field("name")
    ///     .where_eq("status", 1)
    ///     .select::<User>()
    ///     .await?;
    ///
    /// // 插入数据
    /// GlobalDatabase::table("users")?
    ///     .insert(vec![
    ///         ("name", "Alice"),
    ///         ("email", "alice@example.com"),
    ///     ])
    ///     .await?;
    /// ```
    pub fn table(table_name: &str) -> Result<QueryBuilder<'static>, BaseError> {
        Ok(Self::get()?.table(table_name))
    }

    /// 执行原生 SELECT 查询
    ///
    /// # 参数
    ///
    /// - `sql`: SQL 查询语句
    ///
    /// # 返回
    ///
    /// - `Ok(Vec<T>)`: 查询结果列表
    /// - `Err(BaseError)`: 查询失败
    ///
    /// # 类型参数
    ///
    /// - `T`: 结果类型，必须实现 `sqlx::FromRow` trait
    ///
    /// # 错误
    ///
    /// - `DatabaseNotInitialized`: 数据库未初始化
    /// - `DatabaseQueryFailed`: 查询执行失败
    ///
    /// # 说明
    ///
    /// 调用 yang-db::Database::query 方法执行原生 SQL 查询。
    /// 适用于复杂查询或查询构建器无法满足的场景。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// #[derive(sqlx::FromRow)]
    /// struct User {
    ///     id: i32,
    ///     name: String,
    /// }
    ///
    /// let users: Vec<User> = GlobalDatabase::query(
    ///     "SELECT id, name FROM users WHERE status = 1"
    /// ).await?;
    /// ```
    pub async fn query<T>(sql: &str) -> Result<Vec<T>, BaseError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        Self::get()?
            .query(sql)
            .await
            .map_err(BaseError::DatabaseQueryFailed)
    }

    /// 执行参数化 SELECT 查询
    ///
    /// # 参数
    ///
    /// - `sql`: 包含 `?` 占位符的 SQL 查询语句
    /// - `params`: 按占位符顺序绑定的参数
    pub async fn query_with_params<T>(
        sql: &str,
        params: Vec<serde_json::Value>,
    ) -> Result<Vec<T>, BaseError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        Self::get()?
            .query_with_params(sql, params)
            .await
            .map_err(BaseError::DatabaseQueryFailed)
    }

    /// 执行原生 INSERT/UPDATE/DELETE 查询
    ///
    /// # 参数
    ///
    /// - `sql`: SQL 语句
    ///
    /// # 返回
    ///
    /// - `Ok(u64)`: 受影响的行数
    /// - `Err(BaseError)`: 执行失败
    ///
    /// # 错误
    ///
    /// - `DatabaseNotInitialized`: 数据库未初始化
    /// - `DatabaseExecuteFailed`: 语句执行失败
    ///
    /// # 说明
    ///
    /// 调用 yang-db::Database::execute 方法执行原生 SQL 语句。
    /// 适用于 INSERT、UPDATE、DELETE 等修改数据的操作。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // 插入数据
    /// let affected = GlobalDatabase::execute(
    ///     "INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com')"
    /// ).await?;
    ///
    /// // 更新数据
    /// let affected = GlobalDatabase::execute(
    ///     "UPDATE users SET status = 1 WHERE id = 1"
    /// ).await?;
    ///
    /// // 删除数据
    /// let affected = GlobalDatabase::execute(
    ///     "DELETE FROM users WHERE id = 1"
    /// ).await?;
    /// ```
    pub async fn execute(sql: &str) -> Result<u64, BaseError> {
        Self::get()?
            .execute(sql)
            .await
            .map_err(BaseError::DatabaseExecuteFailed)
    }

    /// 执行参数化 INSERT/UPDATE/DELETE 查询
    ///
    /// # 参数
    ///
    /// - `sql`: 包含 `?` 占位符的 SQL 语句
    /// - `params`: 按占位符顺序绑定的参数
    pub async fn execute_with_params(
        sql: &str,
        params: Vec<serde_json::Value>,
    ) -> Result<u64, BaseError> {
        Self::get()?
            .execute_with_params(sql, params)
            .await
            .map_err(BaseError::DatabaseExecuteFailed)
    }

    /// 开始事务
    ///
    /// # 返回
    ///
    /// - `Ok(Transaction)`: yang-db 事务对象
    /// - `Err(BaseError)`: 开始事务失败
    ///
    /// # 错误
    ///
    /// - `DatabaseNotInitialized`: 数据库未初始化
    /// - `DatabaseTransactionFailed`: 开始事务失败
    ///
    /// # 说明
    ///
    /// 调用 yang-db::Database::transaction 方法创建事务。
    /// 事务中的所有操作要么全部成功，要么全部回滚。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // 开始事务
    /// let mut tx = GlobalDatabase::transaction().await?;
    ///
    /// // 在事务中执行操作
    /// tx.execute("INSERT INTO users (name) VALUES ('Alice')").await?;
    /// tx.execute("INSERT INTO logs (action) VALUES ('user_created')").await?;
    ///
    /// // 提交事务
    /// tx.commit().await?;
    ///
    /// // 或者回滚事务
    /// // tx.rollback().await?;
    /// ```
    pub async fn transaction() -> Result<Transaction, BaseError> {
        Self::get()?
            .transaction()
            .await
            .map_err(BaseError::DatabaseTransactionFailed)
    }

    /// 数据库健康检查
    ///
    /// 通过执行 `SELECT 1` 验证数据库连接是否可用。查询返回非空结果即视为健康。
    ///
    /// # 返回
    ///
    /// - `Ok(true)`: 数据库连接正常
    /// - `Ok(false)`: 查询成功但未返回结果（异常状态）
    /// - `Err(BaseError)`: 数据库未初始化或查询失败
    ///
    /// # 错误
    ///
    /// - `DatabaseNotInitialized`: 数据库未初始化，需要先调用 `init` 方法
    /// - `DatabaseQueryFailed`: 健康检查查询执行失败
    pub async fn health_check() -> Result<bool, BaseError> {
        let rows = Self::query::<(i64,)>("SELECT 1").await?;
        Ok(!rows.is_empty())
    }

    /// 获取数据库连接池状态快照
    ///
    /// 转发 yang-db `Database::pool_status`，与 [`GlobalRedis::pool_status`](crate::database::GlobalRedis)
    /// 对称。用于监控连接池水位、排查连接耗尽。
    ///
    /// # 返回
    ///
    /// - `Ok(PoolStatus)`: 连接池状态（max_size/size/available/waiting）
    /// - `Err(BaseError::DatabaseNotInitialized)`: 数据库未初始化
    pub fn pool_status() -> Result<yang_db::PoolStatus, BaseError> {
        Ok(Self::get()?.pool_status())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_not_initialized() {
        // 测试未初始化时获取数据库实例
        let result = GlobalDatabase::get();
        assert!(result.is_err());
        assert!(matches!(result, Err(BaseError::DatabaseNotInitialized)));
    }

    #[test]
    fn test_table_not_initialized() {
        // 测试未初始化时调用 table 方法
        let result = GlobalDatabase::table("users");
        assert!(result.is_err());
        assert!(matches!(result, Err(BaseError::DatabaseNotInitialized)));
    }

    #[tokio::test]
    async fn test_query_with_params_not_initialized() {
        // 测试未初始化时调用参数化查询方法
        let result = GlobalDatabase::query_with_params::<(i32,)>(
            "SELECT id FROM users WHERE id = ?",
            vec![serde_json::json!(1)],
        )
        .await;
        assert!(result.is_err());
        assert!(matches!(result, Err(BaseError::DatabaseNotInitialized)));
    }

    #[tokio::test]
    async fn test_execute_with_params_not_initialized() {
        // 测试未初始化时调用参数化执行方法
        let result = GlobalDatabase::execute_with_params(
            "DELETE FROM users WHERE id = ?",
            vec![serde_json::json!(1)],
        )
        .await;
        assert!(result.is_err());
        assert!(matches!(result, Err(BaseError::DatabaseNotInitialized)));
    }
}
