use crate::error::DbError;
use crate::postgres::transaction::{
    next_cursor_name, normalize_stream_sql, validate_prepared_gid, TransactionOptions,
    DEFAULT_STREAM_BATCH_SIZE,
};
use crate::postgres::{QueryBuilder, Transaction};
use crate::{BackendCapabilities, PoolStatus, POSTGRES_CAPABILITIES};
use sqlx::postgres::{PgPool, PgSslMode};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::OnceCell;

/// PostgreSQL `application_name` 的字节上限（`NAMEDATALEN - 1`）。
const MAX_APPLICATION_NAME_BYTES: usize = 63;

/// `DISCARD ALL`（归还连接前的会话状态清理）。
const DISCARD_ALL_SQL: &str = "DISCARD ALL";

/// 取消目标后端正在执行的语句（不影响连接，语句以 SQLSTATE 57014 结束）。
const CANCEL_BACKEND_SQL: &str = "SELECT pg_cancel_backend($1)";

/// 强制终止目标后端连接（以 SQLSTATE 57P01 结束）。
const TERMINATE_BACKEND_SQL: &str = "SELECT pg_terminate_backend($1)";

/// PostgreSQL 连接 TLS 模式（与 libpq `sslmode` 对齐）。
///
/// 只暴露常用的 5 种：`Disable` / `Prefer` / `Require` / `VerifyCa` / `VerifyFull`。
/// 通过 [`DatabaseConfig::with_ssl_mode`] 设置；未设置时沿用连接串中的 `sslmode`
/// （连接串也没写时 sqlx 默认 `Prefer`，可能回退到明文）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SslMode {
    /// 只尝试非 TLS 连接。
    Disable,
    /// 优先 TLS，失败时回退到非 TLS。
    Prefer,
    /// 只建立 TLS 连接，但**不校验**服务端证书（与 libpq 一致）。需要校验证书请用
    /// [`SslMode::VerifyCa`] / [`SslMode::VerifyFull`]。
    Require,
    /// 只建立 TLS 连接，并校验服务端证书由受信任 CA 签发。
    VerifyCa,
    /// 只建立 TLS 连接，并校验 CA 签发与主机名。
    VerifyFull,
}

impl SslMode {
    fn as_pg_ssl_mode(self) -> PgSslMode {
        match self {
            Self::Disable => PgSslMode::Disable,
            Self::Prefer => PgSslMode::Prefer,
            Self::Require => PgSslMode::Require,
            Self::VerifyCa => PgSslMode::VerifyCa,
            Self::VerifyFull => PgSslMode::VerifyFull,
        }
    }

    /// 该模式是否校验 CA 证书（需要根证书）。
    fn requires_root_cert(self) -> bool {
        matches!(self, Self::VerifyCa | Self::VerifyFull)
    }
}

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
    /// 连接建立时写入的 `application_name`（出现在 `pg_stat_activity` 中）。
    ///
    /// 通过 `after_connect` 钩子的 `set_config('application_name', $1, false)`
    /// 应用，值走绑定参数；最长 63 字节（服务端 `NAMEDATALEN - 1`）。
    pub application_name: Option<String>,
    /// 预编译语句缓存容量；`None`（默认）沿用 sqlx 默认值（100）。
    ///
    /// 经 PgBouncer 等事务级连接池时设为 `Some(0)` 关闭语句缓存，避免服务端
    /// prepared statement 与连接绑定不一致。
    pub statement_cache_size: Option<usize>,
    /// 连接归还池前是否执行 `DISCARD ALL`。默认 `false`。
    ///
    /// 开启后由 `after_release` 钩子下发，清理 `search_path`/`LISTEN`/临时表/
    /// prepared statement/advisory lock 等会话状态；同时在 `before_acquire` 钩子
    /// 重新应用本配置的会话 GUC（`DISCARD ALL` 中的 `RESET ALL` 会清掉它们）。
    pub reset_on_release: bool,
    /// 连接建立后是否校验收到的 `client_encoding` 为 UTF8。默认 `true`（fail-closed）。
    ///
    /// 非 UTF8（如 `SQL_ASCII`/`LATIN1`）会返回连接/配置错误，避免把非 UTF-8
    /// 字节当成 Rust `String`。
    pub require_utf8_client_encoding: bool,
    /// TLS 模式；`None`（默认）沿用连接串中的 `sslmode`，若连接串也没写则 sqlx
    /// 默认 `Prefer`，可能回退到明文连接。
    ///
    /// 配置了 [`Self::ssl_root_cert`] 时，`None` 会被推导为 `VerifyCa`（见
    /// [`Self::effective_ssl_mode`]），避免根证书被静默忽略；此时显式设置
    /// `Prefer`/`Require`/`Disable` 会与根证书冲突并在 [`Self::validate`] 中报错。
    pub ssl_mode: Option<SslMode>,
    /// 根证书（CA）PEM 文件路径；`VerifyCa`/`VerifyFull` 必填。
    ///
    /// 只在 `ssl_mode` 为 `None`（自动推导为 `VerifyCa`）、`VerifyCa` 或
    /// `VerifyFull` 时允许配置。
    pub ssl_root_cert: Option<PathBuf>,
    /// 客户端证书 PEM 文件路径；与 [`Self::ssl_client_key`] 成对出现。
    pub ssl_client_cert: Option<PathBuf>,
    /// 客户端私钥 PEM 文件路径；与 [`Self::ssl_client_cert`] 成对出现。
    pub ssl_client_key: Option<PathBuf>,
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
            application_name: None,
            statement_cache_size: None,
            reset_on_release: false,
            require_utf8_client_encoding: true,
            ssl_mode: None,
            ssl_root_cert: None,
            ssl_client_cert: None,
            ssl_client_key: None,
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

    /// 设置 `application_name`（链式）。`None` 表示不设置。
    pub fn with_application_name(mut self, application_name: impl Into<String>) -> Self {
        self.application_name = Some(application_name.into());
        self
    }

    /// 设置预编译语句缓存容量（链式）；`Some(0)` 关闭缓存（PgBouncer 事务池）。
    pub fn with_statement_cache_size(mut self, capacity: usize) -> Self {
        self.statement_cache_size = Some(capacity);
        self
    }

    /// 设置连接归还前是否执行 `DISCARD ALL`（链式，默认关闭）。
    pub fn with_reset_on_release(mut self, enabled: bool) -> Self {
        self.reset_on_release = enabled;
        self
    }

    /// 设置是否强制 `client_encoding` 为 UTF8（链式，默认开启）。
    pub fn with_require_utf8_client_encoding(mut self, enabled: bool) -> Self {
        self.require_utf8_client_encoding = enabled;
        self
    }

    /// 设置 TLS 模式（链式）；`None` 沿用连接串中的 `sslmode`。
    pub fn with_ssl_mode(mut self, mode: Option<SslMode>) -> Self {
        self.ssl_mode = mode;
        self
    }

    /// 设置根证书 PEM 路径（链式）。
    pub fn with_ssl_root_cert(mut self, path: impl Into<PathBuf>) -> Self {
        self.ssl_root_cert = Some(path.into());
        self
    }

    /// 实际生效的 TLS 模式。
    ///
    /// - `ssl_mode = Some(m)`：原样返回 `Some(m)`；
    /// - `ssl_mode = None` 且配置了 [`Self::ssl_root_cert`]：返回
    ///   `Some(SslMode::VerifyCa)`，避免根证书被静默忽略；
    /// - 其余情况返回 `None`，沿用连接串中的 `sslmode`（连接串也没写时 sqlx 默认
    ///   `Prefer`，可能回退到明文）。
    pub fn effective_ssl_mode(&self) -> Option<SslMode> {
        match self.ssl_mode {
            Some(mode) => Some(mode),
            None if self.ssl_root_cert.is_some() => Some(SslMode::VerifyCa),
            None => None,
        }
    }

    /// 设置客户端证书 PEM 路径（链式）；需同时设置 [`Self::with_ssl_client_key`]。
    pub fn with_ssl_client_cert(mut self, path: impl Into<PathBuf>) -> Self {
        self.ssl_client_cert = Some(path.into());
        self
    }

    /// 设置客户端私钥 PEM 路径（链式）；需同时设置 [`Self::with_ssl_client_cert`]。
    pub fn with_ssl_client_key(mut self, path: impl Into<PathBuf>) -> Self {
        self.ssl_client_key = Some(path.into());
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
        if let Some(application_name) = &self.application_name {
            if application_name.is_empty() {
                return Err(DbError::InvalidArgument(
                    "PostgreSQL application_name 不能为空字符串".to_string(),
                ));
            }
            if application_name.contains('\0') {
                return Err(DbError::InvalidArgument(
                    "PostgreSQL application_name 不能包含 NUL 字符".to_string(),
                ));
            }
            if application_name.len() > MAX_APPLICATION_NAME_BYTES {
                return Err(DbError::InvalidArgument(format!(
                    "PostgreSQL application_name 不能超过 {MAX_APPLICATION_NAME_BYTES} 字节，当前 {} 字节",
                    application_name.len()
                )));
            }
        }
        if let Some(mode) = self.ssl_mode {
            if mode.requires_root_cert() && self.ssl_root_cert.is_none() {
                return Err(DbError::InvalidArgument(format!(
                    "PostgreSQL ssl_mode = {mode:?} 必须提供 ssl_root_cert（CA 证书）"
                )));
            }
            if self.ssl_root_cert.is_some()
                && matches!(mode, SslMode::Prefer | SslMode::Require | SslMode::Disable)
            {
                return Err(DbError::InvalidArgument(format!(
                    "PostgreSQL ssl_mode = {mode:?} 不能与 ssl_root_cert 同时配置；根证书只适用于 \
                     VerifyCa/VerifyFull（ssl_mode 为 None 时会自动按 VerifyCa 使用根证书）"
                )));
            }
            if mode == SslMode::Disable
                && (self.ssl_root_cert.is_some()
                    || self.ssl_client_cert.is_some()
                    || self.ssl_client_key.is_some())
            {
                return Err(DbError::InvalidArgument(
                    "PostgreSQL ssl_mode = Disable 时不能配置 TLS 证书".to_string(),
                ));
            }
        }
        if self.ssl_client_cert.is_some() != self.ssl_client_key.is_some() {
            return Err(DbError::InvalidArgument(
                "PostgreSQL ssl_client_cert 与 ssl_client_key 必须成对提供".to_string(),
            ));
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
    /// 正在等待连接的 acquire 计数（池已无空闲连接且达到上限时计入）。
    waiting: Arc<AtomicUsize>,
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
        // TLS 配置：模式与证书都已在上面的 validate() 中校验（verify-ca/full 必须
        // 提供根证书，客户端证书与私钥必须成对）。effective_ssl_mode 会把
        // “有根证书但未指定模式”推导为 VerifyCa，避免根证书被静默忽略。
        if let Some(mode) = config.effective_ssl_mode() {
            connect_options = connect_options.ssl_mode(mode.as_pg_ssl_mode());
        }
        if let Some(path) = &config.ssl_root_cert {
            connect_options = connect_options.ssl_root_cert(path);
        }
        if let Some(path) = &config.ssl_client_cert {
            connect_options = connect_options.ssl_client_cert(path);
        }
        if let Some(path) = &config.ssl_client_key {
            connect_options = connect_options.ssl_client_key(path);
        }
        if let Some(capacity) = config.statement_cache_size {
            connect_options = connect_options.statement_cache_capacity(capacity);
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
        let session_setup = SessionSetup {
            application_name: config.application_name.clone(),
            statements: session_statements,
            require_utf8_client_encoding: config.require_utf8_client_encoding,
        };
        let after_connect_setup = session_setup.clone();
        options = options.after_connect(move |conn, _meta| {
            let setup = after_connect_setup.clone();
            Box::pin(async move { setup.apply(conn).await })
        });
        if config.reset_on_release {
            // 归还前 DISCARD ALL 清空会话状态；它内含 RESET ALL，会清掉 after_connect
            // 设置的 GUC，所以借出复用连接时（before_acquire）重新应用一遍。
            options = options.after_release(|conn, _meta| {
                Box::pin(async move {
                    sqlx::query(DISCARD_ALL_SQL).execute(&mut *conn).await?;
                    Ok(true)
                })
            });
            let before_acquire_setup = session_setup;
            options = options.before_acquire(move |conn, _meta| {
                let setup = before_acquire_setup.clone();
                Box::pin(async move { setup.apply(conn).await.map(|_| true) })
            });
        }

        let pool = options.connect_with(connect_options).await?;

        Ok(Self {
            pool,
            config,
            server_version: OnceCell::new(),
            waiting: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// 从调用方已创建的连接池构造数据库入口。
    ///
    /// 该构造器不建立连接，也不写入进程全局状态，供显式资源所有权与离线测试使用。
    ///
    /// 注意：`after_connect`/`after_release` 等会话钩子只在
    /// [`Self::connect_with_config`] 建立的连接池上生效；`from_pool` 的既有连接
    /// 需要调用方自行设置，可用 [`Self::reset_connection`] 手工清理单条连接。
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
            waiting: Arc::new(AtomicUsize::new(0)),
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
    /// - `max_size`：配置的最大连接数（`config.max_connections`）
    /// - `size`：当前池内连接总数（`pool.size()`）
    /// - `available`：当前空闲可借出连接数（`pool.num_idle()`）
    /// - `waiting`：调用 [`Database`] 自身入口（query/execute/transaction/acquire/
    ///   流式游标/advisory lock/COPY 等）时，处于「池已无空闲连接且已达上限」的
    ///   在途 acquire 数。sqlx 不直接暴露等待者数，因此这是本库维护的近似值：
    ///   直接经 [`Self::pool`] 借出的调用（含 `QueryBuilder` 执行路径）不计入。
    ///
    /// 连接耗尽排查：`available == 0 && size == max_size` 即池被打满。
    pub fn pool_status(&self) -> PoolStatus {
        PoolStatus {
            max_size: self.config.max_connections as usize,
            size: self.pool.size() as usize,
            available: self.pool.num_idle(),
            waiting: self.waiting.load(Ordering::Relaxed),
        }
    }

    /// 记录一次连接获取：调用时池已无空闲连接且达到上限，则计为潜在等待者。
    ///
    /// 返回的 guard 在 acquire 完成（成功或失败）后立即 drop，避免把执行时间
    /// 误算成等待时间。
    fn track_acquire(&self) -> AcquireWaiter<'_> {
        let would_wait = self.pool.num_idle() == 0
            && self.pool.size() >= self.pool.options().get_max_connections();
        if would_wait {
            self.waiting.fetch_add(1, Ordering::Relaxed);
        }
        AcquireWaiter {
            counter: &self.waiting,
            armed: would_wait,
        }
    }

    /// 从连接池借出一条连接，并计入 [`Self::pool_status`] 的 `waiting` 统计。
    pub(crate) async fn acquire(
        &self,
    ) -> Result<sqlx::pool::PoolConnection<sqlx::Postgres>, DbError> {
        let waiter = self.track_acquire();
        let conn = self.pool.acquire().await?;
        drop(waiter);
        Ok(conn)
    }

    /// 健康检查：执行 `SELECT 1` 验证连接可用（与 MySQL 后端对称；NEW-12）。
    ///
    /// # 返回
    /// - `Ok(true)`：连接正常
    /// - `Err(DbError)`：查询失败（连接不可用）
    pub async fn health_check(&self) -> Result<bool, DbError> {
        let mut conn = self.acquire().await?;
        sqlx::query("SELECT 1").execute(&mut *conn).await?;
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
        let waiter = self.track_acquire();
        let tx = self.pool.begin().await?;
        drop(waiter);
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
        let waiter = self.track_acquire();
        let mut tx = self.pool.begin().await?;
        drop(waiter);
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
    ///
    /// 已弃用：直接委托给 [`Self::execute_script`]（整脚本单事务、dollar-quote/
    /// 注释感知的分句器）。新代码请直接用 `execute_script`。
    #[deprecated(
        since = "0.1.4",
        note = "请使用 execute_script（整脚本单事务，正确处理函数/DO 块与 dollar-quote）"
    )]
    pub async fn init(&self, sql_script: &str) -> Result<(), DbError> {
        self.execute_script(sql_script).await.map(|_| ())
    }

    /// 创建表（执行一条可信 `CREATE TABLE` SQL）。
    ///
    /// 已弃用：本方法只透传裸 SQL，不做拆分、事务或标识符校验。请改用
    /// [`Self::execute_script`]（单事务、正确切分多语句）或外部迁移工具。
    #[deprecated(
        since = "0.2.2",
        note = "裸 SQL 建表逃生舱；请使用 execute_script（单事务、可含多条 DDL）或外部迁移工具"
    )]
    pub async fn create_table(&self, create_sql: &str) -> Result<(), DbError> {
        let mut conn = self.acquire().await?;
        sqlx::query(create_sql).execute(&mut *conn).await?;
        Ok(())
    }

    /// 删除表
    ///
    /// 表名经 `quote_identifier` 校验+转义（DB-6，双引号方言）；非法表名返回
    /// `InvalidArgument`。
    pub async fn drop_table(&self, table_name: &crate::TableRef) -> Result<(), DbError> {
        let quoted = crate::postgres::identifier::quote_table(table_name.as_str())?;
        let sql = format!("DROP TABLE IF EXISTS {}", quoted);
        let mut conn = self.acquire().await?;
        sqlx::query(&sql).execute(&mut *conn).await?;
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
        let mut conn = self.acquire().await?;
        let mut query = sqlx::query_as::<_, T>(sql);
        for param in &params {
            query = bind_json_param_as(query, param);
        }

        let rows = query.fetch_all(&mut *conn).await?;
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
        let mut conn = self.acquire().await?;
        let mut query = sqlx::query(sql);
        for param in &params {
            query = bind_json_param(query, param);
        }

        let result = query.execute(&mut *conn).await?;
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
        let mut conn = self.acquire().await?;
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

    /// 获取会话级排他 advisory lock（`pg_advisory_lock`），阻塞直到拿到锁。
    ///
    /// 会话级锁绑定在具体连接上，因此返回的 [`PgAdvisoryLock`] 持有该连接：
    /// 显式 [`PgAdvisoryLock::unlock`] 或丢弃 guard（导致连接关闭、服务端回收锁）
    /// 之前，这条连接不会再归还池中。guard 存活期间会占用一个池连接名额。
    pub async fn advisory_lock(&self, key: i64) -> Result<PgAdvisoryLock, DbError> {
        self.acquire_advisory_lock(key, AdvisoryLockMode::Exclusive)
            .await
    }

    /// 获取会话级共享 advisory lock（`pg_advisory_lock_shared`），阻塞直到拿到锁。
    ///
    /// 共享锁彼此不互斥，但与同 key 的排他锁互斥；其余语义与
    /// [`advisory_lock`](Self::advisory_lock) 相同（RAII 解锁、绑定单条连接）。
    pub async fn advisory_lock_shared(&self, key: i64) -> Result<PgAdvisoryLock, DbError> {
        self.acquire_advisory_lock(key, AdvisoryLockMode::Shared)
            .await
    }

    /// 尝试获取会话级排他 advisory lock（`pg_try_advisory_lock`），不阻塞。
    ///
    /// 返回 `None` 表示锁被其他会话占用；连接随即归还池。
    pub async fn try_advisory_lock(&self, key: i64) -> Result<Option<PgAdvisoryLock>, DbError> {
        self.try_acquire_advisory_lock(key, AdvisoryLockMode::Exclusive)
            .await
    }

    /// 尝试获取会话级共享 advisory lock（`pg_try_advisory_lock_shared`），不阻塞。
    pub async fn try_advisory_lock_shared(
        &self,
        key: i64,
    ) -> Result<Option<PgAdvisoryLock>, DbError> {
        self.try_acquire_advisory_lock(key, AdvisoryLockMode::Shared)
            .await
    }

    async fn acquire_advisory_lock(
        &self,
        key: i64,
        mode: AdvisoryLockMode,
    ) -> Result<PgAdvisoryLock, DbError> {
        let mut conn = self.acquire().await?;
        if self.config.enable_logging {
            log::debug!("获取 PG 会话级 advisory lock: key={key}, mode={mode:?}");
        }
        sqlx::query(mode.lock_sql())
            .bind(key)
            .execute(&mut *conn)
            .await?;
        Ok(PgAdvisoryLock {
            conn: Some(conn),
            key,
            mode,
        })
    }

    async fn try_acquire_advisory_lock(
        &self,
        key: i64,
        mode: AdvisoryLockMode,
    ) -> Result<Option<PgAdvisoryLock>, DbError> {
        let mut conn = self.acquire().await?;
        let acquired: bool = sqlx::query_scalar(mode.try_lock_sql())
            .bind(key)
            .fetch_one(&mut *conn)
            .await?;
        if acquired {
            Ok(Some(PgAdvisoryLock {
                conn: Some(conn),
                key,
                mode,
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
                let mut conn = self.acquire().await?;
                let value: i32 =
                    sqlx::query_scalar("SELECT current_setting('server_version_num')::int")
                        .fetch_one(&mut *conn)
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
        let mut conn = self.acquire().await?;
        sqlx::query(&sql).execute(&mut *conn).await?;
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
        let mut conn = self.acquire().await?;
        sqlx::query(&sql).execute(&mut *conn).await?;
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
        let mut conn = self.acquire().await?;
        let rows = query.fetch_all(&mut *conn).await?;
        explain_rows_to_strings(rows)
    }

    /// 查询当前会话活动（`pg_stat_activity`），返回结构化行。
    ///
    /// 包含当前连接自身；`client_addr` 以 `host()` 文本形式返回，避免 inet 解码
    /// 依赖。`query` 为最近一次执行的语句文本。
    pub async fn active_queries(&self) -> Result<Vec<ActiveQuery>, DbError> {
        let mut conn = self.acquire().await?;
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
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows)
    }

    /// 查询 `pg_locks` 阻塞链，返回结构化行。
    ///
    /// 每条记录表示一个未授予的锁请求及其一个阻塞者；同一等待者若有多个阻塞者
    /// 会出现多行（`pg_blocking_pids` 语义）。无阻塞时返回空列表。
    pub async fn lock_waits(&self) -> Result<Vec<LockWait>, DbError> {
        let mut conn = self.acquire().await?;
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
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows)
    }

    /// 借出一条连接，并返回绑定该连接后端 PID 的 [`CancelToken`]。
    ///
    /// 调用方在返回的连接上执行长查询（可直接用 sqlx 在该连接上执行），另一个
    /// 任务/线程用 token 调 [`CancelToken::cancel`] 取消它。连接由调用方持有，
    /// 归还池时照常触发 `after_release` 清理。
    ///
    /// 取消操作需要另借一条连接，因此 `max_connections < 2` 时直接返回
    /// [`DbError::InvalidArgument`]（池上限为 1 时取消会与目标查询互相等待）。
    ///
    /// token 中的 `backend_pid` 只在目标连接仍被持有时有效；连接归还池并被复用
    /// 后，同一 PID 可能已指向另一个会话，此时不应再用该 token。
    pub async fn acquire_with_cancel_token(
        &self,
    ) -> Result<(sqlx::pool::PoolConnection<sqlx::Postgres>, CancelToken), DbError> {
        if self.config.max_connections < 2 {
            return Err(DbError::InvalidArgument(format!(
                "PostgreSQL 取消令牌需要至少 2 条连接（取消要另借一条连接），当前 \
                 max_connections = {}",
                self.config.max_connections
            )));
        }
        let mut conn = self.acquire().await?;
        let backend_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *conn)
            .await?;
        let token = CancelToken {
            pool: self.pool.clone(),
            backend_pid,
        };
        Ok((conn, token))
    }

    /// 清洗连接池中一条连接的会话状态（`DISCARD ALL`）。
    ///
    /// 清空会话级 GUC、临时表、游标、prepared statement、LISTEN 与 advisory lock。
    /// 注意：`DISCARD ALL` 不能在事务块内执行。`Database::from_pool` 的调用方可用
    /// 它在借出/归还前后手工清理。
    pub async fn reset_connection(&self) -> Result<(), DbError> {
        let mut conn = self.acquire().await?;
        sqlx::query(DISCARD_ALL_SQL).execute(&mut *conn).await?;
        Ok(())
    }
}

/// 查询取消令牌：记录目标后端 PID，可在另一条连接上取消/终止其当前语句。
///
/// 关闭连接不会取消它正在执行的查询；只有 `pg_cancel_backend`（语句级，连接可
/// 继续使用）或 `pg_terminate_backend`（连接级）会。取消成功后目标语句以
/// SQLSTATE 57014 结束，映射为 [`crate::SqlErrorKind::QueryCanceled`] /
/// [`crate::DbErrorCategory::Timeout`]。
///
/// **生命周期约束**：token 只在目标连接仍被持有时有效。连接一旦归还连接池并被
/// 复用，`backend_pid` 可能已指向另一个会话，此时用该 token 取消会误伤其他查询。
#[derive(Debug, Clone)]
pub struct CancelToken {
    pool: PgPool,
    backend_pid: i32,
}

impl CancelToken {
    /// 目标连接的服务端后端 PID。
    pub fn backend_pid(&self) -> i32 {
        self.backend_pid
    }

    /// 取消目标后端正在执行的语句（`pg_cancel_backend`）。
    ///
    /// 返回 `true` 表示信号已送达；语句是否立即结束取决于服务端。后端已结束或
    /// 不存在时返回 `false`。
    pub async fn cancel(&self) -> Result<bool, DbError> {
        Ok(sqlx::query_scalar(CANCEL_BACKEND_SQL)
            .bind(self.backend_pid)
            .fetch_one(&self.pool)
            .await?)
    }

    /// 强制终止目标后端连接（`pg_terminate_backend`）。
    pub async fn terminate(&self) -> Result<bool, DbError> {
        Ok(sqlx::query_scalar(TERMINATE_BACKEND_SQL)
            .bind(self.backend_pid)
            .fetch_one(&self.pool)
            .await?)
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

/// `waiting` 计数的 RAII guard：离开作用域时把计入的等待者数减回去。
struct AcquireWaiter<'a> {
    counter: &'a AtomicUsize,
    armed: bool,
}

impl Drop for AcquireWaiter<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.counter.fetch_sub(1, Ordering::Relaxed);
        }
    }
}

/// 每条新连接需要下发的会话初始化动作（`after_connect` / `before_acquire` 共用）。
#[derive(Debug, Clone)]
struct SessionSetup {
    /// `application_name`，经 `set_config(..., false)` 绑定写入。
    application_name: Option<String>,
    /// 会话 GUC 基线（时区/日期格式/超时等）。
    statements: Vec<String>,
    /// 是否要求 `client_encoding = UTF8`（fail-closed）。
    require_utf8_client_encoding: bool,
}

impl SessionSetup {
    /// 在指定连接上应用全部会话初始化动作。
    ///
    /// 返回 `sqlx::Error` 以便直接作为连接池钩子的错误；非 UTF8 编码返回
    /// `Error::Configuration`，经 `DbError::from` 映射为连接错误。
    async fn apply(&self, conn: &mut sqlx::PgConnection) -> Result<(), sqlx::Error> {
        if let Some(application_name) = &self.application_name {
            sqlx::query("SELECT set_config('application_name', $1, false)")
                .bind(application_name)
                .execute(&mut *conn)
                .await?;
        }
        for sql in &self.statements {
            sqlx::query(sql).execute(&mut *conn).await?;
        }
        if self.require_utf8_client_encoding {
            let encoding: String = sqlx::query_scalar("SELECT current_setting('client_encoding')")
                .fetch_one(&mut *conn)
                .await?;
            if !client_encoding_is_utf8(&encoding) {
                return Err(sqlx::Error::Configuration(
                    format!(
                        "PostgreSQL client_encoding 必须为 UTF8，当前为 {encoding:?}；\
                         如确需其他编码请设置 DatabaseConfig::with_require_utf8_client_encoding(false)"
                    )
                    .into(),
                ));
            }
        }
        Ok(())
    }
}

/// `client_encoding` 是否为 UTF8（服务端返回 `UTF8`；大小写不敏感）。
fn client_encoding_is_utf8(encoding: &str) -> bool {
    matches!(
        encoding.trim().to_ascii_uppercase().as_str(),
        "UTF8" | "UTF-8" | "UNICODE"
    )
}

/// 会话级 advisory lock 的共享/排他模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdvisoryLockMode {
    Exclusive,
    Shared,
}

impl AdvisoryLockMode {
    /// 阻塞获取锁的 SQL。
    fn lock_sql(self) -> &'static str {
        match self {
            Self::Exclusive => "SELECT pg_advisory_lock($1)",
            Self::Shared => "SELECT pg_advisory_lock_shared($1)",
        }
    }

    /// 非阻塞尝试获取锁的 SQL。
    fn try_lock_sql(self) -> &'static str {
        match self {
            Self::Exclusive => "SELECT pg_try_advisory_lock($1)",
            Self::Shared => "SELECT pg_try_advisory_lock_shared($1)",
        }
    }

    /// 释放会话级锁的 SQL（共享锁必须用 `_shared` 版本解锁）。
    fn unlock_sql(self) -> &'static str {
        match self {
            Self::Exclusive => "SELECT pg_advisory_unlock($1)",
            Self::Shared => "SELECT pg_advisory_unlock_shared($1)",
        }
    }
}

/// 会话级 advisory lock 的 RAII guard。
///
/// 锁绑定在 guard 持有的那条连接上：显式 [`unlock`](Self::unlock) 会执行
/// `pg_advisory_unlock`（共享锁为 `pg_advisory_unlock_shared`）并把连接归还池；
/// 未解锁即丢弃时无法在同步 `Drop` 中执行异步解锁，因此 `detach` 连接使其关闭，
/// 由服务端回收该会话持有的全部会话级锁。
pub struct PgAdvisoryLock {
    conn: Option<sqlx::pool::PoolConnection<sqlx::Postgres>>,
    key: i64,
    mode: AdvisoryLockMode,
}

impl PgAdvisoryLock {
    /// 锁键。
    pub fn key(&self) -> i64 {
        self.key
    }

    /// 是否为共享锁（`pg_advisory_lock_shared`）。
    pub fn is_shared(&self) -> bool {
        self.mode == AdvisoryLockMode::Shared
    }

    /// 释放锁；返回服务端解锁函数的结果。
    ///
    /// 返回 `true` 表示锁已真正释放，连接可安全归还池。返回 `false` 表示服务端
    /// 未释放该锁（例如锁不属于此会话或已被释放）——此时连接里可能仍带着会话级
    /// 锁，直接还池会让锁泄漏到池中，因此改为 `detach` 连接使其关闭，由服务端
    /// 回收该会话的全部会话级锁。
    pub async fn unlock(mut self) -> Result<bool, DbError> {
        let mode = self.mode;
        let conn = self
            .conn
            .as_mut()
            .ok_or_else(|| DbError::TransactionError("advisory lock 已释放".to_string()))?;
        let unlocked: bool = sqlx::query_scalar(mode.unlock_sql())
            .bind(self.key)
            .fetch_one(&mut **conn)
            .await?;
        if unlocked {
            // 锁已释放，连接可以安全归还连接池。
            self.conn.take();
        } else {
            // 服务端未确认释放：关闭会话，让服务端回收锁，避免泄漏到池中。
            if let Some(conn) = self.conn.take() {
                let _ = conn.detach();
            }
        }
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

/// 未类型化 NULL 参数。
///
/// PARSE 阶段发送参数类型 OID 0（未指定），让 PostgreSQL 依据 SQL 上下文推断
/// 参数类型（与字符串字面量一致）。这使 `$1::text`、`$1::uuid`、
/// `$1::timestamptz`、`$1::numeric`、`$1::int + 1` 等显式 cast 场景都能接受
/// NULL；若固定绑定成 `int4`，这些 cast 会报 42883。绑定值本身编码为 NULL
/// （长度 -1），不写入任何数据。
#[derive(Debug, Clone, Copy)]
struct PgUntypedNull;

impl sqlx::Type<sqlx::Postgres> for PgUntypedNull {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        // OID 0 = 未指定类型，由服务端推断。
        sqlx::postgres::PgTypeInfo::with_oid(sqlx::postgres::types::Oid(0))
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for PgUntypedNull {
    fn encode_by_ref(
        &self,
        _buffer: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        Ok(sqlx::encode::IsNull::Yes)
    }

    fn size_hint(&self) -> usize {
        0
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
/// - NULL 绑定为未类型化 NULL（[`PgUntypedNull`]），由服务端按 SQL 上下文推断
///   参数类型；固定绑定成 `int4` 会让 `$1::uuid` / `$1::timestamptz` 等显式
///   cast 报 42883。
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
                None => $query.bind(PgUntypedNull),
            },
            // 布尔类型绑定
            serde_json::Value::Bool(b) => $query.bind(*b),
            // 未类型化 NULL：PARSE 时发送 OID 0，由服务端按上下文推断类型
            serde_json::Value::Null => $query.bind(PgUntypedNull),
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
            waiting: Arc::new(AtomicUsize::new(0)),
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

    /// 取消令牌需要另一条连接：池上限 < 2 时必须立即报错，而不是等到超时。
    #[tokio::test]
    async fn acquire_with_cancel_token_requires_at_least_two_connections() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://postgres:test@127.0.0.1:5432/test")
            .unwrap();
        let db = Database {
            pool,
            config: DatabaseConfig {
                max_connections: 1,
                ..DatabaseConfig::default()
            },
            server_version: OnceCell::new(),
            waiting: Arc::new(AtomicUsize::new(0)),
        };
        assert!(matches!(
            db.acquire_with_cancel_token().await,
            Err(DbError::InvalidArgument(_))
        ));
    }

    /// P1-19：取消 SQL 必须绑定参数，57014 映射为 Timeout 且不可重试。
    #[test]
    fn cancel_token_sql_and_57014_mapping() {
        use crate::error::{DbErrorCategory, SqlErrorInfo};

        assert_eq!(CANCEL_BACKEND_SQL, "SELECT pg_cancel_backend($1)");
        assert_eq!(TERMINATE_BACKEND_SQL, "SELECT pg_terminate_backend($1)");
        assert!(CANCEL_BACKEND_SQL.contains("$1"));
        assert!(TERMINATE_BACKEND_SQL.contains("$1"));

        let canceled = DbError::DatabaseError(Box::new(SqlErrorInfo::new(
            "57014",
            "canceling statement due to user request",
        )));
        assert_eq!(canceled.sqlstate(), Some("57014"));
        assert_eq!(canceled.category(), DbErrorCategory::Timeout);
        assert!(!canceled.is_retryable());
    }

    /// P1-18：会话级共享锁使用 `_shared` 函数，排他锁保持原函数。
    #[test]
    fn advisory_lock_sql_covers_shared_and_exclusive() {
        assert_eq!(
            AdvisoryLockMode::Exclusive.lock_sql(),
            "SELECT pg_advisory_lock($1)"
        );
        assert_eq!(
            AdvisoryLockMode::Shared.lock_sql(),
            "SELECT pg_advisory_lock_shared($1)"
        );
        assert_eq!(
            AdvisoryLockMode::Exclusive.try_lock_sql(),
            "SELECT pg_try_advisory_lock($1)"
        );
        assert_eq!(
            AdvisoryLockMode::Shared.try_lock_sql(),
            "SELECT pg_try_advisory_lock_shared($1)"
        );
        assert_eq!(
            AdvisoryLockMode::Exclusive.unlock_sql(),
            "SELECT pg_advisory_unlock($1)"
        );
        assert_eq!(
            AdvisoryLockMode::Shared.unlock_sql(),
            "SELECT pg_advisory_unlock_shared($1)"
        );
    }

    /// unlock 返回 false 时不得把仍可能持锁的连接还池，必须 detach 关闭会话。
    #[tokio::test]
    #[ignore = "需要真实 PostgreSQL 实例"]
    async fn advisory_unlock_false_detaches_instead_of_returning_connection() {
        let url = std::env::var("PG_TEST_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432/test".to_string());
        let db = Database::connect(&url).await.expect("PG 必须可连接");
        let conn = db.acquire().await.expect("借出连接应成功");
        assert_eq!(db.pool.size(), 1);

        // 该会话从未持有这把锁，服务端 pg_advisory_unlock 返回 false。
        let lock = PgAdvisoryLock {
            conn: Some(conn),
            key: 987_654_321,
            mode: AdvisoryLockMode::Exclusive,
        };
        assert!(!lock.unlock().await.expect("解锁调用应成功"));
        assert_eq!(
            db.pool.size(),
            0,
            "unlock 返回 false 时必须 detach 关闭会话，而不是把连接还池"
        );
    }

    /// P1-20：application_name / statement cache / DISCARD ALL / UTF-8 校验配置。
    #[test]
    fn runtime_config_defaults_and_setters() {
        let default = DatabaseConfig::default();
        assert!(default.application_name.is_none());
        assert!(default.statement_cache_size.is_none());
        assert!(!default.reset_on_release);
        assert!(default.require_utf8_client_encoding);
        assert!(default.ssl_mode.is_none());

        let config = DatabaseConfig::default()
            .with_application_name("yang-test")
            .with_statement_cache_size(0)
            .with_reset_on_release(true)
            .with_require_utf8_client_encoding(false);
        assert_eq!(config.application_name.as_deref(), Some("yang-test"));
        assert_eq!(config.statement_cache_size, Some(0));
        assert!(config.reset_on_release);
        assert!(!config.require_utf8_client_encoding);
        assert!(config.validate().is_ok());

        assert_eq!(DISCARD_ALL_SQL, "DISCARD ALL");
    }

    #[test]
    fn application_name_is_validated() {
        for invalid in [String::new(), "a\0b".to_string(), "a".repeat(64)] {
            let config = DatabaseConfig::default().with_application_name(invalid.clone());
            assert!(
                matches!(config.validate(), Err(DbError::InvalidArgument(_))),
                "application_name 应拒绝 {invalid:?}"
            );
        }
        // 63 字节上限内合法。
        let config = DatabaseConfig::default().with_application_name("a".repeat(63));
        assert!(config.validate().is_ok());
    }

    /// P1-21：TLS 配置组合校验与 sqlx 模式映射。
    #[test]
    fn ssl_config_validation_and_mode_mapping() {
        assert!(matches!(
            SslMode::Disable.as_pg_ssl_mode(),
            PgSslMode::Disable
        ));
        assert!(matches!(
            SslMode::Prefer.as_pg_ssl_mode(),
            PgSslMode::Prefer
        ));
        assert!(matches!(
            SslMode::Require.as_pg_ssl_mode(),
            PgSslMode::Require
        ));
        assert!(matches!(
            SslMode::VerifyCa.as_pg_ssl_mode(),
            PgSslMode::VerifyCa
        ));
        assert!(matches!(
            SslMode::VerifyFull.as_pg_ssl_mode(),
            PgSslMode::VerifyFull
        ));

        // verify-ca / verify-full 缺根证书必须报错。
        for mode in [SslMode::VerifyCa, SslMode::VerifyFull] {
            let missing = DatabaseConfig::default().with_ssl_mode(Some(mode));
            assert!(
                matches!(missing.validate(), Err(DbError::InvalidArgument(_))),
                "{mode:?} 缺 ssl_root_cert 应报错"
            );
            let with_root = DatabaseConfig::default()
                .with_ssl_mode(Some(mode))
                .with_ssl_root_cert("/etc/ssl/root.pem");
            assert!(with_root.validate().is_ok());
        }

        // disable 模式不允许证书。
        assert!(matches!(
            DatabaseConfig::default()
                .with_ssl_mode(Some(SslMode::Disable))
                .with_ssl_root_cert("/etc/ssl/root.pem")
                .validate(),
            Err(DbError::InvalidArgument(_))
        ));

        // 客户端证书与私钥必须成对。
        assert!(matches!(
            DatabaseConfig::default()
                .with_ssl_mode(Some(SslMode::Require))
                .with_ssl_client_cert("/etc/ssl/client.pem")
                .validate(),
            Err(DbError::InvalidArgument(_))
        ));
        assert!(DatabaseConfig::default()
            .with_ssl_mode(Some(SslMode::Require))
            .with_ssl_client_cert("/etc/ssl/client.pem")
            .with_ssl_client_key("/etc/ssl/client.key")
            .validate()
            .is_ok());

        // 未指定模式且无根证书：沿用连接串 sslmode。
        assert!(DatabaseConfig::default().effective_ssl_mode().is_none());
        // 未指定模式但有根证书：自动按 verify-ca 使用根证书（不能静默忽略）。
        assert_eq!(
            DatabaseConfig::default()
                .with_ssl_root_cert("/etc/ssl/root.pem")
                .effective_ssl_mode(),
            Some(SslMode::VerifyCa)
        );
        // 显式模式原样返回。
        for mode in [
            SslMode::Disable,
            SslMode::Prefer,
            SslMode::Require,
            SslMode::VerifyCa,
            SslMode::VerifyFull,
        ] {
            assert_eq!(
                DatabaseConfig::default()
                    .with_ssl_mode(Some(mode))
                    .effective_ssl_mode(),
                Some(mode)
            );
        }
        // 有根证书但未指定模式：validate 通过。
        assert!(DatabaseConfig::default()
            .with_ssl_root_cert("/etc/ssl/root.pem")
            .validate()
            .is_ok());

        // prefer/require 不允许携带根证书（sqlx 在这两种模式下不校验证书，
        // 根证书会被静默忽略）；需要校验请显式用 verify-ca/verify-full。
        for mode in [SslMode::Prefer, SslMode::Require] {
            assert!(
                matches!(
                    DatabaseConfig::default()
                        .with_ssl_mode(Some(mode))
                        .with_ssl_root_cert("/etc/ssl/root.pem")
                        .validate(),
                    Err(DbError::InvalidArgument(_))
                ),
                "{mode:?} + ssl_root_cert 应报错（根证书不会被该校验模式使用）"
            );
        }
    }

    /// P1-22：client_encoding 判定口径（UTF8 大小写/别名容忍，其余 fail-closed）。
    #[test]
    fn client_encoding_must_be_utf8() {
        for ok in ["UTF8", "utf8", "Utf-8", "UNICODE", " UTF8 "] {
            assert!(client_encoding_is_utf8(ok), "{ok:?} 应视为 UTF8");
        }
        for bad in ["SQL_ASCII", "LATIN1", "WIN1252", "EUC_JP", ""] {
            assert!(!client_encoding_is_utf8(bad), "{bad:?} 不应视为 UTF8");
        }
    }

    /// P1-20：waiting 计数在 acquire 结束时归还，pool_status 读取同一原子值。
    #[tokio::test]
    async fn waiting_counter_is_raii_and_reflected_in_pool_status() -> Result<(), sqlx::Error> {
        let counter = AtomicUsize::new(0);
        {
            counter.fetch_add(1, Ordering::Relaxed);
            let _waiter = AcquireWaiter {
                counter: &counter,
                armed: true,
            };
            assert_eq!(counter.load(Ordering::Relaxed), 1);
        }
        assert_eq!(counter.load(Ordering::Relaxed), 0);

        // 未计入等待的 guard 不应改动计数。
        {
            let _waiter = AcquireWaiter {
                counter: &counter,
                armed: false,
            };
        }
        assert_eq!(counter.load(Ordering::Relaxed), 0);

        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://postgres:test@127.0.0.1:5432/test")?;
        let db = Database {
            pool,
            config: DatabaseConfig::default(),
            server_version: OnceCell::new(),
            waiting: Arc::new(AtomicUsize::new(3)),
        };
        assert_eq!(db.pool_status().waiting, 3);
        Ok(())
    }
}
