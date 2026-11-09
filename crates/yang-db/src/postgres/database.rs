use crate::error::DbError;
use crate::postgres::transaction::{
    next_cursor_name, normalize_stream_sql, validate_prepared_gid, TransactionOptions,
    DEFAULT_STREAM_BATCH_SIZE,
};
use crate::postgres::{QueryBuilder, Transaction};
use crate::{BackendCapabilities, PoolStatus, POSTGRES_CAPABILITIES};
use sqlx::postgres::PgPool;
use std::collections::VecDeque;
use tokio::sync::OnceCell;

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
/// use yang_db::postgres::DatabaseConfig;
///
/// // 使用默认配置
/// let config = DatabaseConfig::default();
///
/// // 自定义配置：#[non_exhaustive] 结构体跨 crate 不能用字面量，
/// // 用 default() + 链式 setter 构造
/// let config = DatabaseConfig::default()
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
    /// 最小（保活）连接数。默认 0（行为同改造前）。
    pub min_connections: u32,
    /// 连接最大存活时间（秒）。`None`（默认）表示不限制，行为同改造前。
    pub max_lifetime: Option<u64>,
    /// 借出前是否 PING 探活。默认 false（行为同改造前）。
    pub test_before_acquire: bool,
    /// 连接建立时的 `search_path`（逗号分隔的 schema 列表，如 `"app,public"`）。
    ///
    /// 设置为 `Some` 后，连接参数会追加 `-c search_path=<值>`；未设置时使用
    /// 服务端默认值。值在 [`Self::validate`] 中逐段校验，拒绝任意连接参数注入。
    pub search_path: Option<String>,
    /// 单条语句超时（毫秒）。`None`（默认）不设置，沿用服务端默认。
    ///
    /// 连接建立后执行 `SET statement_timeout = <ms>`。超时被服务端取消时返回
    /// SQLSTATE 57014（[`crate::SqlErrorKind::QueryCanceled`]）。`0` 表示禁用超时。
    ///
    /// 注意：该设置通过连接池的 `after_connect` 钩子下发，只对
    /// [`Database::connect_with_config`] 建立的连接生效；[`Database::from_pool`]
    /// 的既有连接需调用方自行设置。
    pub statement_timeout_ms: Option<u64>,
    /// 锁等待超时（毫秒）。`None`（默认）不设置。超时返回 SQLSTATE 55P03
    /// （[`crate::SqlErrorKind::LockNotAvailable`]）。`0` 表示禁用超时。
    ///
    /// 下发方式同 [`Self::statement_timeout_ms`]。
    pub lock_timeout_ms: Option<u64>,
    /// 事务空闲超时（毫秒）。`None`（默认）不设置：事务开启后闲置超过该时长会被
    /// 服务端终止。`0` 表示禁用超时。
    ///
    /// 下发方式同 [`Self::statement_timeout_ms`]。
    pub idle_in_transaction_timeout_ms: Option<u64>,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            max_connections: 10,
            connect_timeout: 30,
            idle_timeout: 600,
            enable_logging: false,
            min_connections: 0,
            max_lifetime: None,
            test_before_acquire: false,
            search_path: None,
            statement_timeout_ms: None,
            lock_timeout_ms: None,
            idle_in_transaction_timeout_ms: None,
        }
    }
}

impl DatabaseConfig {
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

    /// 设置连接建立时的 `search_path`（链式）。
    ///
    /// 值为逗号分隔的 schema 列表，例如 `"app,public"`；每段都会在
    /// [`Self::validate`] 中按严格标识符校验。
    pub fn with_search_path(mut self, search_path: impl Into<String>) -> Self {
        self.search_path = Some(search_path.into());
        self
    }

    /// 设置单条语句超时（毫秒，链式）。`None` 或 `0` 表示不限制。
    pub fn with_statement_timeout_ms(mut self, timeout_ms: impl Into<Option<u64>>) -> Self {
        self.statement_timeout_ms = timeout_ms.into();
        self
    }

    /// 设置锁等待超时（毫秒，链式）。`None` 或 `0` 表示不限制。
    pub fn with_lock_timeout_ms(mut self, timeout_ms: impl Into<Option<u64>>) -> Self {
        self.lock_timeout_ms = timeout_ms.into();
        self
    }

    /// 设置事务空闲超时（毫秒，链式）。`None` 或 `0` 表示不限制。
    pub fn with_idle_in_transaction_timeout_ms(
        mut self,
        timeout_ms: impl Into<Option<u64>>,
    ) -> Self {
        self.idle_in_transaction_timeout_ms = timeout_ms.into();
        self
    }

    /// 校验数据库连接池配置是否适合生产运行。
    ///
    /// Builder 方法保持纯赋值以兼容历史调用；真正建池前由连接入口显式校验，避免将明显
    /// 非法的配置下推给 sqlx 后才在运行时失败。
    pub fn validate(&self) -> std::result::Result<(), DbError> {
        if self.max_connections == 0 {
            return Err(DbError::InvalidArgument(
                "PostgreSQL max_connections 必须大于 0".to_string(),
            ));
        }
        if self.min_connections > self.max_connections {
            return Err(DbError::InvalidArgument(format!(
                "PostgreSQL min_connections({}) 不能大于 max_connections({})",
                self.min_connections, self.max_connections
            )));
        }
        if self.connect_timeout == 0 {
            return Err(DbError::InvalidArgument(
                "PostgreSQL connect_timeout 必须大于 0 秒".to_string(),
            ));
        }
        if self.idle_timeout == 0 {
            return Err(DbError::InvalidArgument(
                "PostgreSQL idle_timeout 必须大于 0 秒".to_string(),
            ));
        }
        if self.idle_timeout <= self.connect_timeout {
            return Err(DbError::InvalidArgument(format!(
                "PostgreSQL idle_timeout({}) 必须大于 connect_timeout({})",
                self.idle_timeout, self.connect_timeout
            )));
        }
        if matches!(self.max_lifetime, Some(0)) {
            return Err(DbError::InvalidArgument(
                "PostgreSQL max_lifetime 为 Some 时必须大于 0 秒".to_string(),
            ));
        }
        if let Some(search_path) = &self.search_path {
            if search_path.trim().is_empty() {
                return Err(DbError::InvalidArgument(
                    "PostgreSQL search_path 不能为空".to_string(),
                ));
            }
            for segment in search_path.split(',') {
                let segment = segment.trim();
                if segment.is_empty() || crate::sql_types::Identifier::parse(segment).is_err() {
                    return Err(DbError::InvalidArgument(format!(
                        "PostgreSQL search_path 片段非法: {segment:?}"
                    )));
                }
            }
        }
        Ok(())
    }
}

/// 数据库连接管理器
///
/// 管理 PostgreSQL 数据库连接池，提供查询构建和执行的入口点
///
/// # 示例
///
/// ```rust,no_run
/// use yang_db::postgres::Database;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // 连接数据库
///     let db = Database::connect("postgres://user:password@localhost:5432/test").await?;
///
///     // 使用查询构建器
///     let builder = db.table(yang_db::table!("users"))
///         .field(yang_db::field!("id"))
///         .field(yang_db::field!("name"));
///
///     // 执行查询（需要实现 select 方法）
///     // let users = builder.select::<User>().await?;
///
///     Ok(())
/// }
/// ```
pub struct Database {
    pool: PgPool,
    config: DatabaseConfig,
    /// 服务端 `server_version_num` 的进程内缓存（首次探测后固定）。
    server_version: OnceCell<i32>,
}

impl Database {
    /// 返回 PostgreSQL 后端的静态能力契约。
    pub const fn capabilities() -> &'static BackendCapabilities {
        &POSTGRES_CAPABILITIES
    }

    /// 创建数据库连接
    ///
    /// # 参数
    /// - url: 数据库连接字符串，格式：postgres://user:password@host:port/database
    ///
    /// # 返回
    /// - Ok(Database): 成功创建的数据库实例
    /// - Err(DbError): 连接失败错误
    pub async fn connect(url: &str) -> Result<Self, DbError> {
        Self::connect_with_config(url, DatabaseConfig::default()).await
    }

    /// 使用自定义配置创建数据库连接
    pub async fn connect_with_config(url: &str, config: DatabaseConfig) -> Result<Self, DbError> {
        use sqlx::postgres::PgPoolOptions;
        use std::str::FromStr;
        use std::time::Duration;

        config.validate()?;

        // 使用配置参数创建连接池
        let mut options = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(Duration::from_secs(config.connect_timeout))
            .idle_timeout(Duration::from_secs(config.idle_timeout))
            .test_before_acquire(config.test_before_acquire);
        if let Some(secs) = config.max_lifetime {
            options = options.max_lifetime(Duration::from_secs(secs));
        }

        // 连接串解析为 PgConnectOptions 后再追加 search_path；值已在 validate()
        // 中逐段校验，不会把任意启动参数注入连接。
        let mut connect_options =
            sqlx::postgres::PgConnectOptions::from_str(url).map_err(|error| {
                DbError::ConnectionError(format!("无效的 PostgreSQL 连接串: {error}"))
            })?;
        if let Some(search_path) = &config.search_path {
            // sqlx 的 options 以空格切分 `-c k=v` 参数，值内空格会被当成下一段参数。
            // 这里统一去掉逗号两侧空白，再写入启动参数。
            let normalized = search_path
                .split(',')
                .map(str::trim)
                .collect::<Vec<_>>()
                .join(",");
            connect_options = connect_options.options([("search_path", normalized)]);
        }

        // 语句/锁/事务空闲超时通过每条新连接的 after_connect 钩子下发。三个值均为
        // 受控 u64，直接拼进 SET 语句无注入面。
        let statement_timeout_ms = config.statement_timeout_ms;
        let lock_timeout_ms = config.lock_timeout_ms;
        let idle_in_transaction_timeout_ms = config.idle_in_transaction_timeout_ms;
        let timeout_statements = timeout_statements(
            statement_timeout_ms,
            lock_timeout_ms,
            idle_in_transaction_timeout_ms,
        );
        // 会话 GUC 基线：统一时区/日期/浮点输出格式，避免同一查询随服务器
        // locale 或时区漂移。网络传输格式本身不受影响（BYTEA/JSONB 等）。
        let mut session_statements = vec![
            "SET TimeZone = 'UTC'".to_string(),
            "SET DateStyle = 'ISO, YMD'".to_string(),
            "SET IntervalStyle = 'postgres'".to_string(),
            "SET extra_float_digits = 3".to_string(),
            "SET standard_conforming_strings = on".to_string(),
        ];
        session_statements.extend(timeout_statements);
        options = options.after_connect(move |conn, _meta| {
            let session_statements = session_statements.clone();
            Box::pin(async move {
                for sql in &session_statements {
                    sqlx::query(sql).execute(&mut *conn).await?;
                }
                Ok(())
            })
        });

        let pool = options.connect_with(connect_options).await?;

        Ok(Self {
            pool,
            config,
            server_version: OnceCell::new(),
        })
    }

    /// 从调用方已创建的连接池构造数据库入口。
    ///
    /// 该构造器不建立连接，也不写入进程全局状态，供显式资源所有权与离线测试使用。
    ///
    /// # 错误
    ///
    /// 当 `config` 的连接数或超时参数非法时返回 [`DbError::InvalidArgument`]。
    pub fn from_pool(pool: PgPool, config: DatabaseConfig) -> Result<Self, DbError> {
        config.validate()?;
        Ok(Self {
            pool,
            config,
            server_version: OnceCell::new(),
        })
    }

    /// 选择表，返回查询构建器
    pub fn table(&self, table: &crate::TableRef) -> QueryBuilder<'_> {
        QueryBuilder::new(&self.pool, table.as_str(), self.config.enable_logging)
    }

    /// 获取底层 sqlx 连接池的引用
    ///
    /// sqlx 的 `PgPool` 内部基于 `Arc`，克隆代价低廉。上层（如 yang-base 的
    /// `ActionContext::table_query`）需要把连接池注入 `TableQuery` 时，可用
    /// `db.pool().clone()` 获得一个共享同一连接池的句柄。
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// 返回连接池状态快照（与 MySQL / Redis 后端对称；NEW-12）。
    ///
    /// 字段映射（sqlx `PgPool` 无「等待者数」直接 API，故 `waiting` 恒为 0）：
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

    /// 健康检查：执行 `SELECT 1` 验证连接可用（与 MySQL 后端对称；NEW-12）。
    ///
    /// # 返回
    /// - `Ok(true)`：连接正常
    /// - `Err(DbError)`：查询失败（连接不可用）
    pub async fn health_check(&self) -> Result<bool, DbError> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(true)
    }

    /// 优雅关闭连接池（I3）：停止发放新连接、等待在途连接归还后关闭。与 MySQL 对称。
    ///
    /// 幂等；close 后再用返回 `PoolClosed` 类错误而非 panic。
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
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
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

    /// 开始事务（使用数据库默认隔离级别 READ COMMITTED）
    pub async fn transaction(&self) -> Result<Transaction, DbError> {
        let tx = self.pool.begin().await?;
        Ok(Transaction::new(tx, self.config.enable_logging))
    }

    /// 开始事务并设置隔离级别（NG-2）。
    ///
    /// 等价于 [`transaction_with_options`](Self::transaction_with_options) 只指定
    /// `isolation`。
    pub async fn transaction_with_isolation(
        &self,
        isolation: crate::isolation::IsolationLevel,
    ) -> Result<Transaction, DbError> {
        self.transaction_with_options(TransactionOptions::default().with_isolation(isolation))
            .await
    }

    /// 按指定选项开始事务（Phase B）。
    ///
    /// `BEGIN` 之后立即执行一条 `SET TRANSACTION ...`；选项全默认时不下发该语句。
    /// 隔离级别名只来自 [`crate::isolation::IsolationLevel`] 的 `&'static str`，
    /// 其余属性为布尔开关，无注入面。
    pub async fn transaction_with_options(
        &self,
        options: TransactionOptions,
    ) -> Result<Transaction, DbError> {
        let mut tx = self.pool.begin().await?;
        if let Some(sql) = options.set_transaction_sql() {
            if self.config.enable_logging {
                log::debug!("设置 PostgreSQL 事务属性: {}", sql);
            }
            sqlx::query(&sql).execute(&mut *tx).await?;
        }
        Ok(Transaction::new(tx, self.config.enable_logging))
    }

    /// 开始只读事务。事务内写入由服务端拒绝（SQLSTATE 25006）。
    pub async fn read_only_transaction(&self) -> Result<Transaction, DbError> {
        self.transaction_with_options(TransactionOptions::read_only())
            .await
    }

    /// 带重试地执行一个事务（默认事务选项）。
    ///
    /// 仅对 PostgreSQL 的序列化失败（SQLSTATE 40001）与死锁（40P01）重试，
    /// 其他错误立即返回。重试之间按指数退避（10ms 起，封顶 1s）。
    ///
    /// `operation` 接收一个已开启的事务并对其负责：成功路径需自行 `commit()`；
    /// 返回 `Err` 时事务被丢弃并自动回滚。
    ///
    /// # 错误
    ///
    /// `max_attempts` 为 0 时返回 [`DbError::InvalidArgument`]。
    pub async fn retry_transaction<T, F, Fut>(
        &self,
        operation: F,
        max_attempts: u32,
    ) -> Result<T, DbError>
    where
        F: FnMut(Transaction) -> Fut,
        Fut: std::future::Future<Output = Result<T, DbError>>,
    {
        self.retry_transaction_with_options(TransactionOptions::default(), operation, max_attempts)
            .await
    }

    /// 同 [`retry_transaction`](Self::retry_transaction)，可指定事务选项。
    pub async fn retry_transaction_with_options<T, F, Fut>(
        &self,
        options: TransactionOptions,
        mut operation: F,
        max_attempts: u32,
    ) -> Result<T, DbError>
    where
        F: FnMut(Transaction) -> Fut,
        Fut: std::future::Future<Output = Result<T, DbError>>,
    {
        if max_attempts == 0 {
            return Err(DbError::InvalidArgument(
                "重试次数 max_attempts 必须大于 0".to_string(),
            ));
        }

        let mut attempt = 0u32;
        loop {
            attempt += 1;
            let transaction = self.transaction_with_options(options).await?;
            match operation(transaction).await {
                Ok(value) => return Ok(value),
                Err(error) => {
                    if !is_serialization_conflict(&error) || attempt >= max_attempts {
                        return Err(error);
                    }
                    let backoff = retry_backoff(attempt);
                    if self.config.enable_logging {
                        log::warn!(
                            "事务序列化冲突（第 {attempt} 次尝试），{:?} 后重试: {error}",
                            backoff
                        );
                    }
                    tokio::time::sleep(backoff).await;
                }
            }
        }
    }

    /// 初始化数据库（执行 SQL 脚本）
    #[deprecated(
        since = "0.1.4",
        note = "分号切割无法正确处理函数/DO 块；请使用逐 migration 语句或专用脚本执行器"
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
    /// 表名经 `quote_identifier` 校验+转义（DB-6，双引号方言）；非法表名返回
    /// `InvalidArgument`。
    #[allow(deprecated)]
    pub async fn drop_table(&self, table_name: &crate::TableRef) -> Result<(), DbError> {
        let quoted = crate::postgres::identifier::quote_table(table_name.as_str())?;
        let sql = format!("DROP TABLE IF EXISTS {}", quoted);
        self.execute(&sql).await?;
        Ok(())
    }

    /// 检查表是否存在。
    ///
    /// 支持单段表名与 `schema.table` 限定名：标识符先按表名口径逐段校验并加引号，
    /// 再交给 `to_regclass` 解析（限定名直接定位 schema，单段名遵循连接的
    /// `search_path`）。
    pub async fn table_exists(&self, table_name: &crate::TableRef) -> Result<bool, DbError> {
        // to_regclass 接收 SQL 标识符文本；传入已加引号的受控标识符，
        // 既避免大小写折叠，也不拼接进 SQL 文本（仍走 $1 绑定）。
        let quoted = crate::postgres::identifier::quote_table(table_name.as_str())?;
        let exists: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
            .bind(quoted)
            .fetch_one(&self.pool)
            .await?;

        Ok(exists)
    }

    /// 执行带参数的原生 SELECT 查询（参数化查询，防止 SQL 注入）
    ///
    /// # 参数
    /// - sql: SQL 查询语句，使用 `$N` 作为参数占位符
    /// - params: 参数列表，使用 `serde_json::Value` 类型
    ///
    /// # 返回
    /// - Ok(Vec<T>): 查询结果列表
    /// - Err(DbError): 查询失败错误
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use yang_db::postgres::Database;
    /// use serde_json::json;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("postgres://postgres:postgres@localhost/test").await?;
    /// let params = vec![json!("admin"), json!(1)];
    /// // let users: Vec<User> = db.query_with_params("SELECT * FROM users WHERE role = $1 AND status = $2", params).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn query_with_params<T>(
        &self,
        sql: &str,
        params: Vec<serde_json::Value>,
    ) -> Result<Vec<T>, DbError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
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
    /// - sql: SQL 语句，使用 `$N` 作为参数占位符
    /// - params: 参数列表，使用 `serde_json::Value` 类型
    ///
    /// # 返回
    /// - Ok(u64): 受影响的行数
    /// - Err(DbError): 执行失败错误
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use yang_db::postgres::Database;
    /// use serde_json::json;
    ///
    /// # async fn example() -> Result<(), yang_db::DbError> {
    /// let db = Database::connect("postgres://postgres:postgres@localhost/test").await?;
    /// let params = vec![json!("张三"), json!("zhangsan@example.com")];
    /// let rows = db.execute_with_params("INSERT INTO users (name, email) VALUES ($1, $2)", params).await?;
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

    /// 以服务端游标流式读取查询结果（默认每批 1000 行）。
    ///
    /// 游标在一条只读事务内声明（`BEGIN READ ONLY` + `DECLARE ... CURSOR`），
    /// 逐行消费时按批 `FETCH`，大结果集不会一次性进入内存。游标读尽或
    /// [`PgRowCursor::close`] 后，内部事务 `ROLLBACK` 并把连接归还连接池。
    ///
    /// 未读尽即丢弃游标时无法在同步 `Drop` 中执行 `ROLLBACK`，实现会
    /// `detach` 该连接（关闭而非归还），避免把带打开事务的连接放回池中。
    ///
    /// # 查询取消
    ///
    /// sqlx 0.8 没有从连接池直接 cancel 某个查询的 API。需要取消时，可行替代是：
    /// 用 `db.pool().acquire()` 取一条独立连接并 `SELECT pg_backend_pid()` 记下后端
    /// PID（即 cancel token），在另一条连接上执行
    /// `SELECT pg_cancel_backend($1)`（或 `pg_terminate_backend($1)`）。
    ///
    /// # 错误
    ///
    /// SQL 为空或 `batch_size` 为 0 时返回 [`DbError::InvalidArgument`]。
    pub async fn stream_query(
        &self,
        sql: &str,
        params: Vec<serde_json::Value>,
    ) -> Result<PgRowCursor, DbError> {
        self.stream_query_with_batch_size(sql, params, DEFAULT_STREAM_BATCH_SIZE)
            .await
    }

    /// 同 [`stream_query`](Self::stream_query)，可指定每批拉取行数。
    pub async fn stream_query_with_batch_size(
        &self,
        sql: &str,
        params: Vec<serde_json::Value>,
        batch_size: usize,
    ) -> Result<PgRowCursor, DbError> {
        if batch_size == 0 {
            return Err(DbError::InvalidArgument(
                "流式查询 batch_size 必须大于 0".to_string(),
            ));
        }
        let body = normalize_stream_sql(sql)?;
        let mut conn = self.pool.acquire().await?;
        // 服务端游标必须存在于事务块内；统一用只读事务，明确流式入口只面向查询。
        sqlx::query("BEGIN READ ONLY").execute(&mut *conn).await?;

        let cursor_name = next_cursor_name();
        let declare = format!("DECLARE {} CURSOR FOR {}", cursor_name, body);
        if self.config.enable_logging {
            log::debug!("声明流式游标: {}", declare);
        }
        let mut query = sqlx::query(&declare);
        for param in &params {
            query = bind_json_param(query, param);
        }
        if let Err(error) = query.execute(&mut *conn).await {
            // 声明失败时回滚，连接保持干净可归还池。
            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
            return Err(error.into());
        }

        Ok(PgRowCursor::new(
            conn,
            cursor_name,
            batch_size,
            self.config.enable_logging,
        ))
    }

    /// 获取会话级 advisory lock（`pg_advisory_lock`），阻塞直到拿到锁。
    ///
    /// 会话级锁绑定在具体连接上，因此返回的 [`PgAdvisoryLock`] 持有该连接：
    /// 显式 [`PgAdvisoryLock::unlock`] 或丢弃 guard（导致连接关闭、服务端回收锁）
    /// 之前，这条连接不会再归还池中。guard 存活期间会占用一个池连接名额。
    pub async fn advisory_lock(&self, key: i64) -> Result<PgAdvisoryLock, DbError> {
        let mut conn = self.pool.acquire().await?;
        if self.config.enable_logging {
            log::debug!("获取 PG 会话级 advisory lock: {}", key);
        }
        sqlx::query("SELECT pg_advisory_lock($1)")
            .bind(key)
            .execute(&mut *conn)
            .await?;
        Ok(PgAdvisoryLock {
            conn: Some(conn),
            key,
        })
    }

    /// 尝试获取会话级 advisory lock（`pg_try_advisory_lock`），不阻塞。
    ///
    /// 返回 `None` 表示锁被其他会话占用；连接随即归还池。
    pub async fn try_advisory_lock(&self, key: i64) -> Result<Option<PgAdvisoryLock>, DbError> {
        let mut conn = self.pool.acquire().await?;
        let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
            .bind(key)
            .fetch_one(&mut *conn)
            .await?;
        if acquired {
            Ok(Some(PgAdvisoryLock {
                conn: Some(conn),
                key,
            }))
        } else {
            Ok(None)
        }
    }

    /// 服务端的 `server_version_num`（如 PG 16.15 → `160015`），首次调用后缓存到实例。
    ///
    /// 版本门控请优先使用 [`Self::supports`]。
    pub async fn server_version_num(&self) -> Result<i32, DbError> {
        let version = self
            .server_version
            .get_or_try_init(|| async {
                let value: i32 =
                    sqlx::query_scalar("SELECT current_setting('server_version_num')::int")
                        .fetch_one(&self.pool)
                        .await?;
                Ok::<i32, DbError>(value)
            })
            .await?;
        Ok(*version)
    }

    /// 判断服务端版本是否不低于 `min_version`（`server_version_num` 口径）。
    ///
    /// 例如 PG 14 及以上传 `140000`。
    pub async fn supports(&self, min_version: i32) -> Result<bool, DbError> {
        Ok(self.server_version_num().await? >= min_version)
    }

    /// 将已开启的事务准备为两阶段提交（`PREPARE TRANSACTION`）。
    ///
    /// 成功返回后事务进入服务端 prepared 状态，`transaction` 句柄被消费；调用方需
    /// 通过 [`Self::commit_prepared`] 或 [`Self::rollback_prepared`] 收尾。
    /// `max_prepared_transactions = 0` 时返回明确的 [`DbError::TransactionError`]。
    pub async fn prepare_transaction(
        &self,
        transaction: Transaction,
        gid: &str,
    ) -> Result<(), DbError> {
        transaction.prepare(gid).await
    }

    /// 提交一个已准备的两阶段事务（`COMMIT PREPARED`）。
    ///
    /// 必须在事务块之外执行；GID 只能是 `[A-Za-z0-9_.:-]` 白名单字符。
    pub async fn commit_prepared(&self, gid: &str) -> Result<(), DbError> {
        validate_prepared_gid(gid)?;
        let sql = format!("COMMIT PREPARED '{gid}'");
        if self.config.enable_logging {
            log::debug!("提交已准备事务: gid={gid}");
        }
        sqlx::query(&sql).execute(&self.pool).await?;
        Ok(())
    }

    /// 回滚一个已准备的两阶段事务（`ROLLBACK PREPARED`）。
    ///
    /// 必须在事务块之外执行；GID 约束同 [`Self::commit_prepared`]。
    pub async fn rollback_prepared(&self, gid: &str) -> Result<(), DbError> {
        validate_prepared_gid(gid)?;
        let sql = format!("ROLLBACK PREPARED '{gid}'");
        if self.config.enable_logging {
            log::debug!("回滚已准备事务: gid={gid}");
        }
        sqlx::query(&sql).execute(&self.pool).await?;
        Ok(())
    }

    /// 执行 `EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)` 并返回解析后的计划文本。
    ///
    /// `sql` 与 [`Self::query_with_params`] 一样是调用方提供的受信 SQL，`$N`
    /// 占位符由 `params` 绑定，绝不拼接参数值。返回值为每个 EXPLAIN 行中每个
    /// 计划对象的格式化 JSON 文本（通常是 1 个元素）。
    ///
    /// 注意：`ANALYZE` 会真实执行语句（包括写操作）。
    pub async fn explain(
        &self,
        sql: &str,
        params: Vec<serde_json::Value>,
    ) -> Result<Vec<String>, DbError> {
        let statement = explain_statement(sql)?;
        if self.config.enable_logging {
            log::debug!("执行 EXPLAIN: {}, 参数数量: {}", statement, params.len());
        }
        let mut query = sqlx::query_as::<_, (serde_json::Value,)>(&statement);
        for param in &params {
            query = bind_json_param_as(query, param);
        }
        let rows = query.fetch_all(&self.pool).await?;
        explain_rows_to_strings(rows)
    }

    /// 查询当前会话活动（`pg_stat_activity`），返回结构化行。
    ///
    /// 包含当前连接自身；`client_addr` 以 `host()` 文本形式返回，避免 inet 解码
    /// 依赖。`query` 为最近一次执行的语句文本。
    pub async fn active_queries(&self) -> Result<Vec<ActiveQuery>, DbError> {
        let rows = sqlx::query_as::<_, ActiveQuery>(
            "SELECT pid,
                    usename::text AS usename,
                    application_name,
                    host(client_addr) AS client_addr,
                    state,
                    wait_event_type,
                    wait_event,
                    query_start,
                    query
             FROM pg_stat_activity
             ORDER BY query_start DESC NULLS LAST, pid",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// 查询 `pg_locks` 阻塞链，返回结构化行。
    ///
    /// 每条记录表示一个未授予的锁请求及其一个阻塞者；同一等待者若有多个阻塞者
    /// 会出现多行（`pg_blocking_pids` 语义）。无阻塞时返回空列表。
    pub async fn lock_waits(&self) -> Result<Vec<LockWait>, DbError> {
        let rows = sqlx::query_as::<_, LockWait>(
            "SELECT blocked.pid AS blocked_pid,
                    blocking.pid AS blocking_pid,
                    blocked.query AS blocked_query,
                    blocking.query AS blocking_query,
                    CASE
                        WHEN l.relation IS NULL OR l.relation = 0 THEN NULL
                        ELSE l.relation::regclass::text
                    END AS relation,
                    l.locktype AS lock_type,
                    l.mode AS mode,
                    blocked.wait_event AS wait_event
             FROM pg_locks l
             JOIN pg_stat_activity blocked ON blocked.pid = l.pid
             CROSS JOIN LATERAL unnest(pg_blocking_pids(l.pid)) AS bp(blocking_pid)
             JOIN pg_stat_activity blocking ON blocking.pid = bp.blocking_pid
             WHERE NOT l.granted
             ORDER BY blocked.pid, blocking.pid",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}

/// `pg_stat_activity` 中的一条会话快照。
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ActiveQuery {
    /// 后端进程 ID。
    pub pid: i32,
    /// 登录角色名。
    pub usename: Option<String>,
    /// 应用名（连接串中的 `application_name`）。
    pub application_name: Option<String>,
    /// 客户端地址（`host(client_addr)` 文本）。
    pub client_addr: Option<String>,
    /// 会话状态（`active` / `idle` / `idle in transaction` 等）。
    pub state: Option<String>,
    /// 等待事件类别。
    pub wait_event_type: Option<String>,
    /// 等待事件名。
    pub wait_event: Option<String>,
    /// 当前语句开始时间。
    pub query_start: Option<chrono::DateTime<chrono::Utc>>,
    /// 最近一次执行的语句文本。
    pub query: String,
}

/// `pg_locks` 中的一条阻塞关系。
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LockWait {
    /// 被阻塞的后端进程 ID。
    pub blocked_pid: i32,
    /// 阻塞该请求的后端进程 ID。
    pub blocking_pid: i32,
    /// 被阻塞会话最近执行的语句。
    pub blocked_query: Option<String>,
    /// 阻塞会话最近执行的语句。
    pub blocking_query: Option<String>,
    /// 锁对象（表/索引等）；事务级锁为 `None`。
    pub relation: Option<String>,
    /// 锁类型（`relation` / `tuple` / `transactionid` 等）。
    pub lock_type: Option<String>,
    /// 锁模式（`AccessShareLock` / `AccessExclusiveLock` 等）。
    pub mode: Option<String>,
    /// 被阻塞会话的等待事件。
    pub wait_event: Option<String>,
}

/// 构造 EXPLAIN 前置语句；`sql` 为空时拒绝。
fn explain_statement(sql: &str) -> Result<String, DbError> {
    let trimmed = sql.trim().trim_end_matches(';').trim_end();
    if trimmed.is_empty() {
        return Err(DbError::InvalidArgument(
            "EXPLAIN 的 SQL 不能为空".to_string(),
        ));
    }
    Ok(format!("EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) {trimmed}"))
}

/// 把 `EXPLAIN ... FORMAT JSON` 的行解析为格式化计划文本。
///
/// 每个行的第一列是 JSON 数组（通常只含一个计划对象）；对数组逐个元素输出
/// 格式化 JSON，非数组值原样输出。
fn explain_rows_to_strings(rows: Vec<(serde_json::Value,)>) -> Result<Vec<String>, DbError> {
    let mut plans = Vec::new();
    for (row,) in rows {
        let items = match row {
            serde_json::Value::Array(items) => items,
            other => vec![other],
        };
        for item in items {
            plans.push(serde_json::to_string_pretty(&item).map_err(|error| {
                DbError::SerializationError(format!("EXPLAIN 计划序列化失败: {error}"))
            })?);
        }
    }
    Ok(plans)
}

/// 生成连接建立后需要执行的超时 `SET` 语句。
///
/// 三个值都是受控 `u64`（毫秒），直接拼进语句无注入面；`None` 表示不下发。
fn timeout_statements(
    statement_timeout_ms: Option<u64>,
    lock_timeout_ms: Option<u64>,
    idle_in_transaction_timeout_ms: Option<u64>,
) -> Vec<String> {
    let mut statements = Vec::new();
    if let Some(ms) = statement_timeout_ms {
        statements.push(format!("SET statement_timeout = {ms}"));
    }
    if let Some(ms) = lock_timeout_ms {
        statements.push(format!("SET lock_timeout = {ms}"));
    }
    if let Some(ms) = idle_in_transaction_timeout_ms {
        statements.push(format!("SET idle_in_transaction_session_timeout = {ms}"));
    }
    statements
}

/// 是否为可整事务重试的序列化冲突（40001）或死锁（40P01）。
fn is_serialization_conflict(error: &DbError) -> bool {
    matches!(error.sqlstate(), Some("40001") | Some("40P01"))
}

/// 第 `attempt` 次失败后的指数退避：10ms、20ms、40ms……封顶 1s。
fn retry_backoff(attempt: u32) -> std::time::Duration {
    let exponent = attempt.saturating_sub(1).min(7);
    std::time::Duration::from_millis((10u64 << exponent).min(1_000))
}

/// 会话级 advisory lock 的 RAII guard。
///
/// 锁绑定在 guard 持有的那条连接上：显式 [`unlock`](Self::unlock) 会执行
/// `pg_advisory_unlock` 并把连接归还池；未解锁即丢弃时无法在同步 `Drop` 中执行
/// 异步解锁，因此 `detach` 连接使其关闭，由服务端回收该会话持有的全部会话级锁。
pub struct PgAdvisoryLock {
    conn: Option<sqlx::pool::PoolConnection<sqlx::Postgres>>,
    key: i64,
}

impl PgAdvisoryLock {
    /// 锁键。
    pub fn key(&self) -> i64 {
        self.key
    }

    /// 释放锁并归还连接；返回服务端 `pg_advisory_unlock` 的结果。
    pub async fn unlock(mut self) -> Result<bool, DbError> {
        let conn = self
            .conn
            .as_mut()
            .ok_or_else(|| DbError::TransactionError("advisory lock 已释放".to_string()))?;
        let unlocked: bool = sqlx::query_scalar("SELECT pg_advisory_unlock($1)")
            .bind(self.key)
            .fetch_one(&mut **conn)
            .await?;
        // 锁已释放，连接可以安全归还连接池。
        self.conn.take();
        Ok(unlocked)
    }

    /// 释放会话级 advisory lock，与 [`unlock`](Self::unlock) 等价。
    ///
    /// 会话级锁绑定在具体连接上，因此释放入口在 guard 而不是 `Database`：
    /// `Database` 上执行 `pg_advisory_unlock` 只会作用于池中随机一条连接，
    /// 无法保证解除的是持锁会话。
    pub async fn advisory_unlock(self) -> Result<bool, DbError> {
        self.unlock().await
    }
}

impl Drop for PgAdvisoryLock {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            log::warn!(
                "PG 会话级 advisory lock({}) 未显式 unlock 即被丢弃；关闭连接以让服务端回收锁",
                self.key
            );
            let _ = conn.detach();
        }
    }
}

/// 连接池上的服务端游标（`DECLARE ... CURSOR` + 分批 `FETCH`）。
///
/// 逐行消费：缓冲区为空时才向后端拉取下一批，内存占用以 `batch_size` 为上限。
pub struct PgRowCursor {
    conn: Option<sqlx::pool::PoolConnection<sqlx::Postgres>>,
    cursor_name: String,
    batch_size: usize,
    buffer: VecDeque<sqlx::postgres::PgRow>,
    exhausted: bool,
    closed: bool,
    enable_logging: bool,
}

impl PgRowCursor {
    fn new(
        conn: sqlx::pool::PoolConnection<sqlx::Postgres>,
        cursor_name: String,
        batch_size: usize,
        enable_logging: bool,
    ) -> Self {
        Self {
            conn: Some(conn),
            cursor_name,
            batch_size,
            buffer: VecDeque::with_capacity(batch_size.min(1024)),
            exhausted: false,
            closed: false,
            enable_logging,
        }
    }

    /// 每批拉取的最大行数。
    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    /// 取下一条结果；流结束返回 `Ok(None)`。
    pub async fn next(&mut self) -> Result<Option<sqlx::postgres::PgRow>, DbError> {
        loop {
            if let Some(row) = self.buffer.pop_front() {
                return Ok(Some(row));
            }
            if self.exhausted {
                self.release().await?;
                return Ok(None);
            }
            self.fetch_batch().await?;
        }
    }

    /// 提前关闭游标，丢弃尚未消费的缓冲行，并把连接归还池。
    pub async fn close(mut self) -> Result<(), DbError> {
        self.buffer.clear();
        self.release().await
    }

    async fn fetch_batch(&mut self) -> Result<(), DbError> {
        let conn = self
            .conn
            .as_mut()
            .ok_or_else(|| DbError::TransactionError("流式游标已关闭".to_string()))?;
        let sql = format!(
            "FETCH FORWARD {} FROM {}",
            self.batch_size, self.cursor_name
        );
        if self.enable_logging {
            log::debug!("游标拉取: {}", sql);
        }
        let rows = sqlx::query(&sql).fetch_all(&mut **conn).await?;
        if rows.len() < self.batch_size {
            self.exhausted = true;
        }
        self.buffer.extend(rows);
        Ok(())
    }

    /// 关闭游标并结束内部只读事务；连接清理失败时丢弃连接而非归还池。
    async fn release(&mut self) -> Result<(), DbError> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        self.exhausted = true;

        if let Some(mut conn) = self.conn.take() {
            let close_sql = format!("CLOSE {}", self.cursor_name);
            let close_ok = sqlx::query(&close_sql).execute(&mut *conn).await.is_ok();
            let rollback_ok = sqlx::query("ROLLBACK").execute(&mut *conn).await.is_ok();
            if !(close_ok && rollback_ok) {
                // 清理失败：不能把可能仍带打开事务的连接归还池。
                let _ = conn.detach();
                return Err(DbError::TransactionError(
                    "流式游标关闭失败，连接已丢弃".to_string(),
                ));
            }
        }
        Ok(())
    }
}

impl Drop for PgRowCursor {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            // 同步 Drop 无法执行 ROLLBACK，detach 让连接关闭而不是带着打开事务回池。
            let _ = conn.detach();
        }
    }
}

/// 共享 `serde_json::Value` 参数绑定逻辑的内部宏
///
/// `Query` 与 `QueryAs` 的 `bind` 是各自的固有方法、没有公共 trait，
/// 无法用泛型函数复用，这里用宏共享同一套 `match` 体，
/// 保证两类查询的绑定行为完全一致。
///
/// PostgreSQL 适配要点：
/// - 数字优先按 `i64` 绑定，浮点按 `f64` 原生绑定（PostgreSQL 不会把文本
///   隐式转换为数值列，故不能像 MySQL 那样以字符串绑定浮点）。
/// - NULL 绑定为 `Option::<i32>::None`。
/// - 数组 / 对象作为原生 `serde_json::Value` 绑定（sqlx 的 json feature 已开启，
///   对应 PostgreSQL 的 JSON / JSONB），而非序列化成字符串。
macro_rules! bind_json_value {
    ($query:expr, $param:expr) => {
        match $param {
            // 字符串类型直接绑定
            serde_json::Value::String(s) => $query.bind(s.clone()),
            // 数字类型按 i64 → u64 顶半区 → f64 分类绑定
            serde_json::Value::Number(n) => match crate::json_param::classify_json_number(n) {
                Some(crate::json_param::JsonNumber::Int(i)) => $query.bind(i),
                Some(crate::json_param::JsonNumber::Uint(u)) => $query.bind(u.to_string()),
                Some(crate::json_param::JsonNumber::Float(f)) => $query.bind(f),
                None => $query.bind(Option::<i32>::None),
            },
            // 布尔类型绑定
            serde_json::Value::Bool(b) => $query.bind(*b),
            // NULL 类型绑定为类型化 None，规避 PostgreSQL 未类型化 NULL 限制
            serde_json::Value::Null => $query.bind(Option::<i32>::None),
            // 数组和对象作为原生 JSON / JSONB 绑定
            other => $query.bind(other.clone()),
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
    query: sqlx::query::QueryAs<'q, sqlx::Postgres, T, sqlx::postgres::PgArguments>,
    param: &serde_json::Value,
) -> sqlx::query::QueryAs<'q, sqlx::Postgres, T, sqlx::postgres::PgArguments>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
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
    query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    param: &serde_json::Value,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
    bind_json_value!(query, param)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
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
            DatabaseConfig {
                max_connections: 0,
                ..DatabaseConfig::default()
            }
            .validate(),
            Err(DbError::InvalidArgument(_))
        ));

        assert!(matches!(
            DatabaseConfig {
                max_connections: 2,
                min_connections: 3,
                ..DatabaseConfig::default()
            }
            .validate(),
            Err(DbError::InvalidArgument(_))
        ));
    }

    #[test]
    fn test_database_config_validate_rejects_invalid_timeouts() {
        for config in [
            DatabaseConfig {
                connect_timeout: 0,
                ..DatabaseConfig::default()
            },
            DatabaseConfig {
                idle_timeout: 0,
                ..DatabaseConfig::default()
            },
            DatabaseConfig {
                max_lifetime: Some(0),
                ..DatabaseConfig::default()
            },
        ] {
            assert!(matches!(
                config.validate(),
                Err(DbError::InvalidArgument(_))
            ));
        }

        assert!(matches!(
            DatabaseConfig {
                connect_timeout: 30,
                idle_timeout: 30,
                ..DatabaseConfig::default()
            }
            .validate(),
            Err(DbError::InvalidArgument(_))
        ));
    }

    #[tokio::test]
    async fn test_connect_with_config_rejects_invalid_config_before_connecting() {
        let config = DatabaseConfig {
            max_connections: 0,
            ..DatabaseConfig::default()
        };
        let result =
            Database::connect_with_config("postgres://postgres:bad@127.0.0.1:1/test", config).await;

        assert!(matches!(result, Err(DbError::InvalidArgument(_))));
    }

    #[test]
    fn test_search_path_validation_accepts_identifier_lists_only() {
        assert!(DatabaseConfig::default().search_path.is_none());

        let config = DatabaseConfig::default().with_search_path("app, public");
        assert_eq!(config.search_path.as_deref(), Some("app, public"));
        assert!(config.validate().is_ok());

        for invalid in [
            "",
            "   ",
            "app,,public",
            "app; DROP SCHEMA public",
            "app public",
        ] {
            let config = DatabaseConfig::default().with_search_path(invalid);
            assert!(
                matches!(config.validate(), Err(DbError::InvalidArgument(_))),
                "search_path 应拒绝 {invalid:?}"
            );
        }
    }

    #[tokio::test]
    async fn health_check_propagates_closed_pool_error() -> Result<(), sqlx::Error> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://postgres:test@127.0.0.1:5432/test")?;
        let db = Database {
            pool,
            config: DatabaseConfig::default(),
            server_version: OnceCell::new(),
        };

        db.close().await;

        assert!(db.is_closed());
        assert!(matches!(db.health_check().await, Err(DbError::PoolClosed)));
        Ok(())
    }

    #[test]
    fn timeout_config_is_opt_in_and_renders_set_statements() {
        let default_config = DatabaseConfig::default();
        assert!(default_config.statement_timeout_ms.is_none());
        assert!(default_config.lock_timeout_ms.is_none());
        assert!(default_config.idle_in_transaction_timeout_ms.is_none());
        assert!(timeout_statements(None, None, None).is_empty());

        let config = DatabaseConfig::default()
            .with_statement_timeout_ms(5_000u64)
            .with_lock_timeout_ms(Some(250))
            .with_idle_in_transaction_timeout_ms(30_000u64);
        assert_eq!(config.statement_timeout_ms, Some(5_000));
        assert_eq!(config.lock_timeout_ms, Some(250));
        assert_eq!(config.idle_in_transaction_timeout_ms, Some(30_000));
        assert!(config.validate().is_ok());

        assert_eq!(
            timeout_statements(
                config.statement_timeout_ms,
                config.lock_timeout_ms,
                config.idle_in_transaction_timeout_ms
            ),
            vec![
                "SET statement_timeout = 5000".to_string(),
                "SET lock_timeout = 250".to_string(),
                "SET idle_in_transaction_session_timeout = 30000".to_string(),
            ]
        );
    }

    #[test]
    fn only_serialization_failures_and_deadlocks_are_retried() {
        use crate::error::SqlErrorInfo;

        assert!(is_serialization_conflict(&DbError::DatabaseError(
            Box::new(SqlErrorInfo::new("40001", "could not serialize access"))
        )));
        assert!(is_serialization_conflict(&DbError::DatabaseError(
            Box::new(SqlErrorInfo::new("40P01", "deadlock detected"))
        )));
        for other in ["23505", "57014", "55P03", "42P01"] {
            assert!(
                !is_serialization_conflict(&DbError::DatabaseError(Box::new(SqlErrorInfo::new(
                    other, "other"
                )))),
                "SQLSTATE {other} 不应触发事务重试"
            );
        }
        assert!(!is_serialization_conflict(&DbError::TransactionError(
            "local".to_string()
        )));
    }

    #[test]
    fn retry_backoff_grows_exponentially_and_caps() {
        use std::time::Duration;

        assert_eq!(retry_backoff(1), Duration::from_millis(10));
        assert_eq!(retry_backoff(2), Duration::from_millis(20));
        assert_eq!(retry_backoff(3), Duration::from_millis(40));
        assert_eq!(retry_backoff(7), Duration::from_millis(640));
        assert_eq!(retry_backoff(8), Duration::from_millis(1_000));
        assert_eq!(retry_backoff(100), Duration::from_millis(1_000));
    }

    #[test]
    fn explain_statement_prefixes_trusted_sql_and_rejects_empty() {
        assert_eq!(
            explain_statement("SELECT $1::int + 1").unwrap(),
            "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) SELECT $1::int + 1"
        );
        assert_eq!(
            explain_statement("  SELECT 1;  ").unwrap(),
            "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) SELECT 1"
        );
        for invalid in ["", "   ", " ; "] {
            assert!(matches!(
                explain_statement(invalid),
                Err(DbError::InvalidArgument(_))
            ));
        }
    }

    #[test]
    fn explain_rows_are_flattened_to_pretty_json_per_plan() {
        let rows = vec![
            (serde_json::json!([
                {"Plan": {"Node Type": "Result"}, "Execution Time": 0.1}
            ]),),
            (serde_json::json!([{"Plan": {"Node Type": "Seq Scan"}}]),),
        ];
        let plans = explain_rows_to_strings(rows).unwrap();
        assert_eq!(plans.len(), 2);
        for plan in &plans {
            let parsed: serde_json::Value = serde_json::from_str(plan).unwrap();
            assert!(parsed.get("Plan").is_some(), "计划文本应可解析为 JSON");
        }
        assert!(plans[0].contains("\"Node Type\": \"Result\""));

        // 非数组行原样保留为单个计划文本。
        let plans = explain_rows_to_strings(vec![(serde_json::json!({"Plan": {}}),)]).unwrap();
        assert_eq!(plans.len(), 1);
        assert!(plans[0].contains("Plan"));
    }
}
