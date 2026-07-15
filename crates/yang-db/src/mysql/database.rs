use crate::error::DbError;
use crate::mysql::query_builder::QueryBuilder;
use crate::mysql::transaction::Transaction;
use crate::{BackendCapabilities, PoolStatus, MYSQL_CAPABILITIES};
use sqlx::mysql::MySqlPool;

/// 数据库配置
///
/// 用于配置数据库连接池的参数
///
/// 标注 `#[non_exhaustive]`：未来新增连接池参数不构成破坏性变更，外部请用
/// `DatabaseConfig::default()` 加 `..Default::default()` 或链式 setter 构造。
///
/// # 示例
///
/// ```rust
/// use yang_db::DatabaseConfig;
///
/// // 使用默认配置
/// let config = DatabaseConfig::default();
///
/// // 自定义配置：#[non_exhaustive] 结构体跨 crate 不能用字面量，
/// // 用 default() + 链式 setter 或字段赋值构造
/// let config = DatabaseConfig::default()
///     .with_max_connections(20)
///     .with_connect_timeout(5)
///     .with_idle_timeout(300)
///     .with_enable_logging(true)
///     .with_min_connections(2)
///     .with_max_lifetime(Some(1800))
///     .with_test_before_acquire(true);
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct DatabaseConfig {
    /// 最大连接数
    pub max_connections: u32,
    /// 连接超时时间（秒）
    pub connect_timeout: u64,
    /// 空闲连接超时时间（秒）
    pub idle_timeout: u64,
    /// 是否启用日志
    pub enable_logging: bool,
    /// 最小（保活）连接数。维持热连接避免冷启动惊群。默认 0（不预热，行为同改造前）。
    pub min_connections: u32,
    /// 连接最大存活时间（秒）。超时后连接在归还时被主动轮换，规避 failover/wait_timeout
    /// 杀连接导致的「先失败一次再替换」。`None`（默认）表示不限制，行为同改造前。
    pub max_lifetime: Option<u64>,
    /// 借出前是否 PING 探活。把「先失败再替换」变为透明自愈，代价是每次 acquire 一次往返。
    /// 默认 false（行为同改造前）。
    pub test_before_acquire: bool,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            max_connections: 10,
            connect_timeout: 30,
            idle_timeout: 600,
            enable_logging: false,
            // 自愈参数默认值保持当前行为：不预热、不限寿命、不借前探活
            min_connections: 0,
            max_lifetime: None,
            test_before_acquire: false,
        }
    }
}

impl DatabaseConfig {
    /// 设置最大连接数（链式）。
    pub fn with_max_connections(mut self, n: u32) -> Self {
        self.max_connections = n;
        self
    }

    /// 设置连接超时时间（秒）（链式）。
    pub fn with_connect_timeout(mut self, secs: u64) -> Self {
        self.connect_timeout = secs;
        self
    }

    /// 设置空闲连接超时时间（秒）（链式）。
    pub fn with_idle_timeout(mut self, secs: u64) -> Self {
        self.idle_timeout = secs;
        self
    }

    /// 设置是否启用日志（链式）。
    pub fn with_enable_logging(mut self, enabled: bool) -> Self {
        self.enable_logging = enabled;
        self
    }

    /// 设置最小（保活）连接数（链式）。
    pub fn with_min_connections(mut self, n: u32) -> Self {
        self.min_connections = n;
        self
    }

    /// 设置连接最大存活时间（秒，`None` 为不限制）（链式）。
    pub fn with_max_lifetime(mut self, secs: Option<u64>) -> Self {
        self.max_lifetime = secs;
        self
    }

    /// 设置借出前是否 PING 探活（链式）。
    pub fn with_test_before_acquire(mut self, enabled: bool) -> Self {
        self.test_before_acquire = enabled;
        self
    }

    /// 校验数据库连接池配置是否适合生产运行。
    ///
    /// Builder 方法保持纯赋值以兼容历史调用；真正建池前由连接入口显式校验，避免将明显
    /// 非法的配置下推给 sqlx 后才在运行时失败。
    pub fn validate(&self) -> std::result::Result<(), DbError> {
        if self.max_connections == 0 {
            return Err(DbError::InvalidArgument(
                "MySQL max_connections 必须大于 0".to_string(),
            ));
        }
        if self.min_connections > self.max_connections {
            return Err(DbError::InvalidArgument(format!(
                "MySQL min_connections({}) 不能大于 max_connections({})",
                self.min_connections, self.max_connections
            )));
        }
        if self.connect_timeout == 0 {
            return Err(DbError::InvalidArgument(
                "MySQL connect_timeout 必须大于 0 秒".to_string(),
            ));
        }
        if self.idle_timeout == 0 {
            return Err(DbError::InvalidArgument(
                "MySQL idle_timeout 必须大于 0 秒".to_string(),
            ));
        }
        if self.idle_timeout <= self.connect_timeout {
            return Err(DbError::InvalidArgument(format!(
                "MySQL idle_timeout({}) 必须大于 connect_timeout({})",
                self.idle_timeout, self.connect_timeout
            )));
        }
        if matches!(self.max_lifetime, Some(0)) {
            return Err(DbError::InvalidArgument(
                "MySQL max_lifetime 为 Some 时必须大于 0 秒".to_string(),
            ));
        }
        Ok(())
    }
}

/// 数据库连接管理器
///
/// 管理 MySQL 数据库连接池，提供查询构建和执行的入口点
///
/// # 示例
///
/// ```rust,no_run
/// use yang_db::Database;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // 连接数据库
///     let db = Database::connect("mysql://user:password@localhost:3306/test").await?;
///     
///     // 使用查询构建器
///     let builder = db.table("users")
///         .field("id")
///         .field("name");
///     
///     // 执行查询（需要实现 select 方法）
///     // let users = builder.select::<User>().await?;
///     
///     Ok(())
/// }
/// ```
pub struct Database {
    pool: MySqlPool,
    config: DatabaseConfig,
}

impl Database {
    /// 返回 MySQL 后端的静态能力契约。
    pub const fn capabilities() -> &'static BackendCapabilities {
        &MYSQL_CAPABILITIES
    }

    /// 创建数据库连接
    ///
    /// # 参数
    /// - url: 数据库连接字符串，格式：mysql://user:password@host:port/database
    ///
    /// # 返回
    /// - Ok(Database): 成功创建的数据库实例
    /// - Err(DbError): 连接失败错误
    pub async fn connect(url: &str) -> Result<Self, DbError> {
        Self::connect_with_config(url, DatabaseConfig::default()).await
    }

    /// 使用自定义配置创建数据库连接
    pub async fn connect_with_config(url: &str, config: DatabaseConfig) -> Result<Self, DbError> {
        use sqlx::mysql::MySqlPoolOptions;
        use std::time::Duration;

        config.validate()?;

        // 使用配置参数创建连接池
        let mut options = MySqlPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(Duration::from_secs(config.connect_timeout))
            .idle_timeout(Duration::from_secs(config.idle_timeout))
            .test_before_acquire(config.test_before_acquire);
        // max_lifetime 为 None 时不设置（sqlx 默认不限制），保持改造前行为
        if let Some(secs) = config.max_lifetime {
            options = options.max_lifetime(Duration::from_secs(secs));
        }
        let pool = options.connect(url).await?;

        Ok(Self { pool, config })
    }

    /// 选择表，返回查询构建器
    pub fn table(&self, table_name: &str) -> QueryBuilder<'_> {
        QueryBuilder::new(&self.pool, table_name, self.config.enable_logging)
    }

    /// 获取底层 sqlx 连接池的引用
    ///
    /// sqlx 的 `MySqlPool` 内部基于 `Arc`，克隆代价低廉。上层（如 yang-base 的
    /// `ActionContext::table_query`）需要把连接池注入 `TableQuery` 时，可用
    /// `db.pool().clone()` 获得一个共享同一连接池的句柄。
    pub fn pool(&self) -> &MySqlPool {
        &self.pool
    }

    /// 返回连接池状态快照（与 Redis 侧 `pool_status` 对称）。
    ///
    /// 字段映射（sqlx `MySqlPool` 无「等待者数」直接 API，故 `waiting` 恒为 0）：
    /// - `max_size`：配置的最大连接数（`config.max_connections`）
    /// - `size`：当前池内连接总数（`pool.size()`）
    /// - `available`：当前空闲可借出连接数（`pool.num_idle()`）
    /// - `waiting`：sqlx 未暴露，恒为 0
    ///
    /// 连接耗尽排查：`available == 0 && size == max_size` 即池被打满。
    pub fn pool_status(&self) -> PoolStatus {
        PoolStatus {
            max_size: self.config.max_connections as usize,
            size: self.pool.size() as usize,
            available: self.pool.num_idle(),
            waiting: 0,
        }
    }

    /// 健康检查：执行 `SELECT 1` 验证连接可用。
    ///
    /// 与 yang-base 层 `GlobalDatabase::health_check` 语义一致，但下沉到持有连接池
    /// 的这一层，使 yang-db 直接消费者也能探活。
    ///
    /// # 返回
    /// - `Ok(true)`：连接正常
    /// - `Err(DbError)`：查询失败（连接不可用）
    pub async fn health_check(&self) -> Result<bool, DbError> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(true)
    }

    /// 优雅关闭连接池（I3）：停止发放新连接、等待在途连接归还后关闭。
    ///
    /// 这正是 K8s 滚动更新所需语义——收到 SIGTERM 后 drain 在途请求，避免 RST 在途
    /// 连接。幂等：重复调用安全。close 后再用本池的操作会返回 `PoolClosed` 类错误而非
    /// panic。
    pub async fn close(&self) {
        self.pool.close().await;
    }

    /// 连接池是否已关闭（I3）。
    pub fn is_closed(&self) -> bool {
        self.pool.is_closed()
    }

    /// 执行原生 SELECT 查询
    ///
    /// ⚠️ 安全警告：此方法接受裸 SQL 字符串，不进行参数化处理。调用方必须确保 SQL
    /// 字符串不包含用户输入，否则存在 SQL 注入风险。请优先使用 [`query_with_params`]。
    #[deprecated(
        since = "0.1.0",
        note = "使用 query_with_params 替代，避免 SQL 注入风险"
    )]
    pub async fn query<T>(&self, sql: &str) -> Result<Vec<T>, DbError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        if self.config.enable_logging {
            log::debug!("执行原生查询: {}", sql);
        }

        let rows = sqlx::query_as::<_, T>(sql).fetch_all(&self.pool).await?;

        Ok(rows)
    }

    /// 执行原生 INSERT/UPDATE/DELETE 查询
    ///
    /// ⚠️ 安全警告：此方法接受裸 SQL 字符串，不进行参数化处理。调用方必须确保 SQL
    /// 字符串不包含用户输入，否则存在 SQL 注入风险。请优先使用 [`execute_with_params`]。
    #[deprecated(
        since = "0.1.0",
        note = "使用 execute_with_params 替代，避免 SQL 注入风险"
    )]
    pub async fn execute(&self, sql: &str) -> Result<u64, DbError> {
        if self.config.enable_logging {
            log::debug!("执行原生语句: {}", sql);
        }

        let result = sqlx::query(sql).execute(&self.pool).await?;

        Ok(result.rows_affected())
    }

    /// 开始事务（使用数据库默认隔离级别 REPEATABLE READ）
    pub async fn transaction(&self) -> Result<Transaction, DbError> {
        let tx = self.pool.begin().await?;
        Ok(Transaction::new(tx, self.config.enable_logging))
    }

    /// 开始事务并设置隔离级别（NG-2）。
    ///
    /// 在 `BEGIN` 后立即执行 `SET TRANSACTION ISOLATION LEVEL <level>`。级别名取自
    /// [`IsolationLevel::as_sql`] 的 `&'static str` 字面量，无注入面。
    pub async fn transaction_with_isolation(
        &self,
        isolation: crate::isolation::IsolationLevel,
    ) -> Result<Transaction, DbError> {
        let mut tx = self.pool.begin().await?;
        let sql = format!("SET TRANSACTION ISOLATION LEVEL {}", isolation.as_sql());
        if self.config.enable_logging {
            log::debug!("设置 MySQL 事务隔离级别: {}", sql);
        }
        sqlx::query(&sql).execute(&mut *tx).await?;
        Ok(Transaction::new(tx, self.config.enable_logging))
    }

    /// 初始化数据库（执行 SQL 脚本）
    #[deprecated(
        since = "0.1.4",
        note = "分号切割无法正确处理存储过程/触发器；请使用逐 migration 语句或专用脚本执行器"
    )]
    #[allow(deprecated)]
    pub async fn init(&self, sql_script: &str) -> Result<(), DbError> {
        // 按分号分割多个 SQL 语句
        let statements: Vec<&str> = sql_script
            .split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        for statement in statements {
            self.execute(statement).await?;
        }

        Ok(())
    }

    /// 创建表
    #[allow(deprecated)]
    pub async fn create_table(&self, create_sql: &str) -> Result<(), DbError> {
        self.execute(create_sql).await?;
        Ok(())
    }

    /// 删除表
    ///
    /// 表名经 `quote_identifier` 校验+转义（DB-6）；DDL 不支持占位符绑定，非法表名
    /// （含空格/分号/反引号等）返回 `InvalidArgument` 而非拼进 SQL。
    #[allow(deprecated)]
    pub async fn drop_table(&self, table_name: &str) -> Result<(), DbError> {
        let quoted = crate::mysql::identifier::quote_identifier(table_name)?;
        let sql = format!("DROP TABLE IF EXISTS {}", quoted);
        self.execute(&sql).await?;
        Ok(())
    }

    /// 检查表是否存在
    ///
    /// 表名走 `?` 参数化绑定（DB-6，对齐 PG 的 `$1`），消除字面量注入。
    pub async fn table_exists(&self, table_name: &str) -> Result<bool, DbError> {
        let sql = "SELECT COUNT(*) as count FROM information_schema.tables \
             WHERE table_schema = DATABASE() AND table_name = ?";

        let row: (i64,) = sqlx::query_as(sql)
            .bind(table_name)
            .fetch_one(&self.pool)
            .await?;

        Ok(row.0 > 0)
    }

    /// 执行带参数的原生 SELECT 查询（参数化查询，防止 SQL 注入）
    ///
    /// # 参数
    /// - sql: SQL 查询语句，使用 `?` 作为参数占位符
    /// - params: 参数列表，使用 `serde_json::Value` 类型
    ///
    /// # 返回
    /// - Ok(Vec<T>): 查询结果列表
    /// - Err(DbError): 查询失败错误
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use yang_db::Database;
    /// use serde_json::json;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    /// let params = vec![json!("admin"), json!(1)];
    /// // let users: Vec<User> = db.query_with_params("SELECT * FROM users WHERE role = ? AND status = ?", params).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn query_with_params<T>(
        &self,
        sql: &str,
        params: Vec<serde_json::Value>,
    ) -> Result<Vec<T>, DbError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        if self.config.enable_logging {
            log::debug!("执行参数化查询: {}, 参数数量: {}", sql, params.len());
        }

        // 构建查询并逐一绑定参数
        let mut query = sqlx::query_as::<_, T>(sql);
        for param in &params {
            query = bind_json_param_as(query, param);
        }

        let rows = query.fetch_all(&self.pool).await?;
        Ok(rows)
    }

    /// 执行带参数的原生 INSERT/UPDATE/DELETE 语句（参数化查询，防止 SQL 注入）
    ///
    /// # 参数
    /// - sql: SQL 语句，使用 `?` 作为参数占位符
    /// - params: 参数列表，使用 `serde_json::Value` 类型
    ///
    /// # 返回
    /// - Ok(u64): 受影响的行数
    /// - Err(DbError): 执行失败错误
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use yang_db::Database;
    /// use serde_json::json;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("mysql://root:password@localhost/test").await?;
    /// let params = vec![json!("张三"), json!("zhangsan@example.com")];
    /// let rows = db.execute_with_params("INSERT INTO users (name, email) VALUES (?, ?)", params).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn execute_with_params(
        &self,
        sql: &str,
        params: Vec<serde_json::Value>,
    ) -> Result<u64, DbError> {
        if self.config.enable_logging {
            log::debug!("执行参数化语句: {}, 参数数量: {}", sql, params.len());
        }

        // 构建查询并逐一绑定参数
        let mut query = sqlx::query(sql);
        for param in &params {
            query = bind_json_param(query, param);
        }

        let result = query.execute(&self.pool).await?;
        Ok(result.rows_affected())
    }
}

/// 共享 `serde_json::Value` 参数绑定逻辑的内部宏
///
/// `Query` 与 `QueryAs` 的 `bind` 是各自的固有方法、没有公共 trait，
/// 无法用泛型函数复用，这里用宏共享同一套 `match` 体，
/// 保证两类查询的绑定行为完全一致。
macro_rules! bind_json_value {
    ($query:expr, $param:expr) => {
        match $param {
            // 字符串类型直接绑定
            serde_json::Value::String(s) => $query.bind(s.clone()),
            // 数字类型转为 i64 绑定
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    $query.bind(i)
                } else if let Some(f) = n.as_f64() {
                    // 浮点数转为字符串绑定，避免精度丢失
                    $query.bind(f.to_string())
                } else {
                    $query.bind(Option::<String>::None)
                }
            }
            // 布尔类型绑定
            serde_json::Value::Bool(b) => $query.bind(*b),
            // NULL 类型绑定为 None
            serde_json::Value::Null => $query.bind(Option::<String>::None),
            // 数组和对象类型序列化为 JSON 字符串绑定
            other => $query.bind(other.to_string()),
        }
    };
}

/// 将 `serde_json::Value` 参数绑定到 `query_as` 查询
///
/// # 参数
/// - query: sqlx query_as 查询对象
/// - param: JSON 参数值
///
/// # 返回
/// - 绑定参数后的查询对象
fn bind_json_param_as<'q, T>(
    query: sqlx::query::QueryAs<'q, sqlx::MySql, T, sqlx::mysql::MySqlArguments>,
    param: &serde_json::Value,
) -> sqlx::query::QueryAs<'q, sqlx::MySql, T, sqlx::mysql::MySqlArguments>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow>,
{
    bind_json_value!(query, param)
}

/// 将 `serde_json::Value` 参数绑定到执行查询
///
/// # 参数
/// - query: sqlx 执行查询对象
/// - param: JSON 参数值
///
/// # 返回
/// - 绑定参数后的查询对象
fn bind_json_param<'q>(
    query: sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments>,
    param: &serde_json::Value,
) -> sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments> {
    bind_json_value!(query, param)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_config_validate_accepts_default_config() {
        let config = DatabaseConfig::default();

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_database_config_validate_rejects_invalid_pool_size() {
        assert!(matches!(
            DatabaseConfig::default().with_max_connections(0).validate(),
            Err(DbError::InvalidArgument(_))
        ));

        assert!(matches!(
            DatabaseConfig::default()
                .with_max_connections(2)
                .with_min_connections(3)
                .validate(),
            Err(DbError::InvalidArgument(_))
        ));
    }

    #[test]
    fn test_database_config_validate_rejects_invalid_timeouts() {
        for config in [
            DatabaseConfig::default().with_connect_timeout(0),
            DatabaseConfig::default().with_idle_timeout(0),
            DatabaseConfig::default().with_max_lifetime(Some(0)),
        ] {
            assert!(matches!(
                config.validate(),
                Err(DbError::InvalidArgument(_))
            ));
        }

        assert!(matches!(
            DatabaseConfig::default()
                .with_connect_timeout(30)
                .with_idle_timeout(30)
                .validate(),
            Err(DbError::InvalidArgument(_))
        ));
    }

    #[tokio::test]
    async fn test_connect_with_config_rejects_invalid_config_before_connecting() {
        let config = DatabaseConfig::default().with_max_connections(0);
        let result =
            Database::connect_with_config("mysql://root:bad@127.0.0.1:1/test", config).await;

        assert!(matches!(result, Err(DbError::InvalidArgument(_))));
    }

    #[tokio::test]
    async fn health_check_propagates_closed_pool_error() -> Result<(), sqlx::Error> {
        let pool = sqlx::mysql::MySqlPoolOptions::new()
            .connect_lazy("mysql://root:test@127.0.0.1:3306/test")?;
        let db = Database {
            pool,
            config: DatabaseConfig::default(),
        };

        db.close().await;

        assert!(db.is_closed());
        assert!(matches!(
            db.health_check().await,
            Err(DbError::ConnectionError(message)) if message.contains("连接池已关闭")
        ));
        Ok(())
    }
}
