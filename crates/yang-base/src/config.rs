//! 引擎配置（I4）：从环境变量分层加载，遵循 12-factor。
//!
//! 本模块提供 [`EngineConfig::from_env`]：从 `YANG_` 前缀环境变量读取数据库/Redis
//! 连接串与连接池参数，解析失败一律走 checked 错误（[`BaseError::ConfigError`]），
//! **不** panic（遵守禁新增生产 panic 约定）。
//!
//! 设计取轻量替代（纯 `std::env`，不引入 figment/config/dotenv 重依赖）。未来如需
//! 「默认 < TOML 文件 < 环境变量」三层覆盖，可在可选 `config` feature 后增量补 TOML 层，
//! 本类型的 `from_env` 即最高优先级层。
//!
//! # 识别的环境变量
//!
//! | 变量 | 含义 | 默认 |
//! |------|------|------|
//! | `YANG_MYSQL_URL` | MySQL 连接串 | 无（缺失则 `mysql_url` 为 `None`） |
//! | `YANG_DATABASE_URL` | MySQL 连接串（deprecated，请改用 `YANG_MYSQL_URL`） | 同上 |
//! | `YANG_POSTGRES_URL` | PostgreSQL 连接串 | 无（缺失则 `postgres_url` 为 `None`） |
//! | `YANG_REDIS_URL` | Redis 连接串 | 无 |
//! | `YANG_DB_MAX_CONNECTIONS` | 最大连接数 | 由 `DatabaseConfig::default` 决定 |
//! | `YANG_DB_MIN_CONNECTIONS` | 最小（保活）连接数 | 同上 |
//! | `YANG_DB_CONNECT_TIMEOUT` | 连接超时（秒） | 同上 |
//! | `YANG_DB_IDLE_TIMEOUT` | 空闲超时（秒） | 同上 |
//! | `YANG_DB_MAX_LIFETIME` | 连接最大存活（秒，0/缺失=不限制） | 不限制 |
//! | `YANG_DB_TEST_BEFORE_ACQUIRE` | 借出前探活（true/false/1/0） | false |
//! | `YANG_DB_ENABLE_LOGGING` | SQL 日志（true/false/1/0） | false |
//!
//! `postgres` feature 下额外识别 PostgreSQL 会话/连接参数（见 [`EngineConfig::postgres_database`]）：
//!
//! | 变量 | 含义 | 默认 |
//! |------|------|------|
//! | `YANG_PG_SEARCH_PATH` | 连接 `search_path`（逗号分隔，如 `app,public`） | 服务端默认 |
//! | `YANG_PG_APPLICATION_NAME` | `application_name`（≤63 字节） | 不设置 |
//! | `YANG_PG_STATEMENT_TIMEOUT_MS` | 单条语句超时（毫秒，0=不限制） | 不设置 |
//! | `YANG_PG_LOCK_TIMEOUT_MS` | 锁等待超时（毫秒，0=不限制） | 不设置 |
//! | `YANG_PG_IDLE_IN_TRANSACTION_TIMEOUT_MS` | 事务空闲超时（毫秒，0=不限制） | 不设置 |
//! | `YANG_PG_SSL_MODE` | `disable`/`prefer`/`require`/`verify-ca`/`verify-full` | 沿用连接串 |
//! | `YANG_PG_SSL_ROOT_CERT` | 根证书（CA）PEM 路径 | 不设置 |
//! | `YANG_PG_SSL_CLIENT_CERT` | 客户端证书 PEM 路径（与私钥成对） | 不设置 |
//! | `YANG_PG_SSL_CLIENT_KEY` | 客户端私钥 PEM 路径（与证书成对） | 不设置 |
//!
//! Redis 连接池（`feature = "redis"`）：
//!
//! | 变量 | 含义 | 默认 |
//! |------|------|------|
//! | `YANG_REDIS_MAX_CONNECTIONS` | 最大连接数 | 10 |
//! | `YANG_REDIS_CONNECT_TIMEOUT` | 建连超时（秒） | 5 |
//! | `YANG_REDIS_WAIT_TIMEOUT` | 获取连接超时（秒） | 10 |
//! | `YANG_REDIS_IDLE_TIMEOUT` | 归还回收钩子超时（秒） | 300 |
//! | `YANG_REDIS_RESPONSE_TIMEOUT_MS` | 命令级响应超时（0=关闭） | 不限制 |
//! | `YANG_REDIS_RECONNECT_RETRIES` | 断线重试次数 | 2 |
//! | `YANG_REDIS_RECONNECT_MAX_DELAY_MS` | 重试最大退避 | 200 |
//! | `YANG_REDIS_PIPELINE_MAX_COMMANDS` | Pipeline 命令数上限 | 10000 |
//! | `YANG_REDIS_ENABLE_LOGGING` | 脱敏连接目标日志 | false |

use crate::error::BaseError;
use std::env;
use std::fmt;

/// 引擎级聚合配置，从环境变量加载（I4）。
///
/// 手写 [`fmt::Debug`]（而非 derive）：`mysql_url` / `postgres_url` / `redis_url`
/// 可能包含 userinfo 密码与查询串 token，Debug 输出只保留脱敏后的
/// scheme + host[:port][/path]，绝不输出凭据。字段本身始终保存原始连接串。
#[derive(Clone, Default)]
#[non_exhaustive]
pub struct EngineConfig {
    /// MySQL 连接串（`YANG_MYSQL_URL`，兼容 deprecated `YANG_DATABASE_URL`，缺失为 `None`）。
    pub mysql_url: Option<String>,
    /// PostgreSQL 连接串（`YANG_POSTGRES_URL`，缺失为 `None`）。
    ///
    /// 与 `postgres` feature 配合使用：连接池、`Record` 解码、声明式 schema 同步与
    /// `TableQuery` / 内置 CRUD 的 PostgreSQL 执行层均已接入。
    pub postgres_url: Option<String>,
    /// Redis 连接串（`YANG_REDIS_URL`，缺失为 `None`）。
    pub redis_url: Option<String>,
    /// 数据库连接池配置（由 `YANG_DB_*` 覆盖 `DatabaseConfig::default`）。
    ///
    /// 同时启用 `mysql` 与 `postgres` 时沿用 MySQL 配置类型；仅启用 `postgres` 时
    /// 使用 PostgreSQL 配置类型。
    #[cfg(feature = "mysql")]
    pub database: yang_db::DatabaseConfig,
    /// 数据库连接池配置（由 `YANG_DB_*` 覆盖 `DatabaseConfig::default`）。
    #[cfg(all(feature = "postgres", not(feature = "mysql")))]
    pub database: yang_db::PgDatabaseConfig,
    /// PostgreSQL 连接/会话配置入口（`postgres` feature）。
    ///
    /// 除通用 `YANG_DB_*` 连接池参数外，还映射 `YANG_PG_*` 的 `search_path` /
    /// `application_name` / `statement_timeout` / `lock_timeout` / TLS 等
    /// [`yang_db::PgDatabaseConfig`] 字段，供 `Database::connect_with_config` 使用。
    /// 同时启用 `mysql` 时它是访问 PostgreSQL 会话参数的唯一入口（此时 [`Self::database`]
    /// 仍为 MySQL 配置类型）。
    #[cfg(feature = "postgres")]
    pub postgres_database: yang_db::PgDatabaseConfig,
    /// Redis 连接池配置（由 `YANG_REDIS_*` 覆盖 `RedisConfig::default`）。
    #[cfg(feature = "redis")]
    pub redis: yang_db::RedisConfig,
}

impl fmt::Debug for EngineConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("EngineConfig");
        debug.field("mysql_url", &self.mysql_url.as_deref().map(RedactedUrl));
        debug.field(
            "postgres_url",
            &self.postgres_url.as_deref().map(RedactedUrl),
        );
        debug.field("redis_url", &self.redis_url.as_deref().map(RedactedUrl));
        #[cfg(feature = "mysql")]
        debug.field("database", &self.database);
        #[cfg(all(feature = "postgres", not(feature = "mysql")))]
        debug.field("database", &self.database);
        #[cfg(feature = "postgres")]
        debug.field("postgres_database", &self.postgres_database);
        #[cfg(feature = "redis")]
        debug.field("redis", &self.redis);
        debug.finish()
    }
}

/// 仅用于 Debug/日志展示的脱敏连接串。
struct RedactedUrl<'a>(&'a str);

impl fmt::Debug for RedactedUrl<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}", redact_connection_url(self.0))
    }
}

/// 对连接串做展示层脱敏：仅保留 scheme 与 `host[:port][/path]`，剥离 userinfo
/// （含密码）与 `?` / `#` 查询串与片段；无法识别 scheme 或缺 scheme 的输入
/// 回退为固定占位文本，不 panic。
///
/// 该函数只用于 Debug 格式化，绝不修改 [`EngineConfig`] 中存储的原始连接串。
fn redact_connection_url(raw: &str) -> String {
    let Some((scheme, rest)) = raw.split_once("://") else {
        return "[REDACTED]".to_string();
    };
    if !matches!(
        scheme.to_ascii_lowercase().as_str(),
        "mysql" | "postgres" | "postgresql" | "redis" | "rediss"
    ) {
        return "[REDACTED]".to_string();
    }
    // 先剥离 fragment/query（其中可能含 token），再剥离 authority 中的 userinfo
    // （其中含密码）；path 中原生的 '@' 不参与 userinfo 判定。
    let rest = rest.split(['#', '?']).next().unwrap_or(rest);
    let (authority, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, Some(path)),
        None => (rest, None),
    };
    // 未编码的 `/` 出现在密码里会让 authority 在 `@` 之前被截断，
    // 此时剩下的 `@host` 落在 path 中；这类输入本身不是合法 URL，
    // 但 Debug 输出绝不能把密码片段原样带出，整体回退为占位文本。
    let ambiguous_userinfo = !authority.contains('@')
        && (path.is_some_and(|path| path.contains('@'))
            || authority.rsplit_once(':').is_some_and(|(_, tail)| {
                !tail.is_empty() && !tail.chars().all(|c| c.is_ascii_digit())
            }));
    if ambiguous_userinfo {
        return format!("{scheme}://[REDACTED]");
    }
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    match path {
        Some(path) => format!("{scheme}://{authority}/{path}"),
        None => format!("{scheme}://{authority}"),
    }
}

/// 当前编译所使用的数据库连接池配置类型。
#[cfg(feature = "mysql")]
type DatabaseConfigType = yang_db::DatabaseConfig;
#[cfg(all(feature = "postgres", not(feature = "mysql")))]
type DatabaseConfigType = yang_db::PgDatabaseConfig;

/// 读取一个环境变量并按 `T` 解析；缺失返回 `Ok(None)`，存在但解析失败返回 ConfigError。
#[cfg(any(feature = "mysql", feature = "postgres", feature = "redis"))]
fn parse_env<T: std::str::FromStr>(key: &str) -> Result<Option<T>, BaseError>
where
    T::Err: std::fmt::Display,
{
    match env::var(key) {
        Ok(raw) => raw
            .trim()
            .parse::<T>()
            .map(Some)
            .map_err(|e| BaseError::ConfigError(format!("环境变量 {key} 解析失败: {e}"))),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(BaseError::ConfigError(format!(
            "环境变量 {key} 含非法 UTF-8"
        ))),
    }
}

/// 解析布尔环境变量：接受 `true/false/1/0`（大小写不敏感），其余报 ConfigError。
#[cfg(any(feature = "mysql", feature = "postgres", feature = "redis"))]
fn parse_bool_env(key: &str) -> Result<Option<bool>, BaseError> {
    match env::var(key) {
        Ok(raw) => match raw.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => Ok(Some(true)),
            "false" | "0" => Ok(Some(false)),
            other => Err(BaseError::ConfigError(format!(
                "环境变量 {key} 期望 true/false/1/0，实得 {other:?}"
            ))),
        },
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(BaseError::ConfigError(format!(
            "环境变量 {key} 含非法 UTF-8"
        ))),
    }
}

/// 从 `YANG_DB_*` 读取连接池配置。
///
/// MySQL 与 PostgreSQL 的 `DatabaseConfig` 字段名一致，故共用同一读取逻辑；
/// 具体类型由编译 feature 决定（见 [`DatabaseConfigType`]）。
#[cfg(any(feature = "mysql", feature = "postgres"))]
fn load_database_config() -> Result<DatabaseConfigType, BaseError> {
    let mut cfg = DatabaseConfigType::default();
    if let Some(v) = parse_env::<u32>("YANG_DB_MAX_CONNECTIONS")? {
        cfg.max_connections = v;
    }
    if let Some(v) = parse_env::<u32>("YANG_DB_MIN_CONNECTIONS")? {
        cfg.min_connections = v;
    }
    if let Some(v) = parse_env::<u64>("YANG_DB_CONNECT_TIMEOUT")? {
        cfg.connect_timeout = v;
    }
    if let Some(v) = parse_env::<u64>("YANG_DB_IDLE_TIMEOUT")? {
        cfg.idle_timeout = v;
    }
    if let Some(v) = parse_env::<u64>("YANG_DB_MAX_LIFETIME")? {
        // 0 视为不限制（与 None 同义），其余设具体秒数
        cfg.max_lifetime = if v == 0 { None } else { Some(v) };
    }
    if let Some(v) = parse_bool_env("YANG_DB_TEST_BEFORE_ACQUIRE")? {
        cfg.test_before_acquire = v;
    }
    if let Some(v) = parse_bool_env("YANG_DB_ENABLE_LOGGING")? {
        cfg.enable_logging = v;
    }
    Ok(cfg)
}

/// 读取字符串环境变量；缺失返回 `Ok(None)`，非法 UTF-8 返回 ConfigError。
#[cfg(feature = "postgres")]
fn parse_string_env(key: &str) -> Result<Option<String>, BaseError> {
    match env::var(key) {
        Ok(raw) => Ok(Some(raw)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(BaseError::ConfigError(format!(
            "环境变量 {key} 含非法 UTF-8"
        ))),
    }
}

/// 解析 `YANG_PG_SSL_MODE`（大小写不敏感，`_` 与 `-` 等价）。
#[cfg(feature = "postgres")]
fn parse_ssl_mode(key: &str, raw: &str) -> Result<yang_db::PgSslMode, BaseError> {
    use yang_db::PgSslMode;
    match raw.trim().to_ascii_lowercase().replace('_', "-").as_str() {
        "disable" => Ok(PgSslMode::Disable),
        "prefer" => Ok(PgSslMode::Prefer),
        "require" => Ok(PgSslMode::Require),
        "verify-ca" => Ok(PgSslMode::VerifyCa),
        "verify-full" => Ok(PgSslMode::VerifyFull),
        other => Err(BaseError::ConfigError(format!(
            "环境变量 {key} 取值非法: {other:?}（允许 disable/prefer/require/verify-ca/verify-full）"
        ))),
    }
}

/// 从 `YANG_DB_*`（连接池）与 `YANG_PG_*`（会话参数）读取 PostgreSQL 配置。
///
/// 加载后立即调用 [`yang_db::PgDatabaseConfig::validate`]，让非法组合（如
/// `idle_timeout <= connect_timeout`、`verify-ca` 缺少根证书）在启动期就报
/// [`BaseError::ConfigError`]，而不是等到建连时才失败。
#[cfg(feature = "postgres")]
fn load_postgres_database_config() -> Result<yang_db::PgDatabaseConfig, BaseError> {
    let mut cfg = yang_db::PgDatabaseConfig::default();
    // 通用连接池参数：与 MySQL 共用同一组 YANG_DB_* 变量。
    if let Some(v) = parse_env::<u32>("YANG_DB_MAX_CONNECTIONS")? {
        cfg.max_connections = v;
    }
    if let Some(v) = parse_env::<u32>("YANG_DB_MIN_CONNECTIONS")? {
        cfg.min_connections = v;
    }
    if let Some(v) = parse_env::<u64>("YANG_DB_CONNECT_TIMEOUT")? {
        cfg.connect_timeout = v;
    }
    if let Some(v) = parse_env::<u64>("YANG_DB_IDLE_TIMEOUT")? {
        cfg.idle_timeout = v;
    }
    if let Some(v) = parse_env::<u64>("YANG_DB_MAX_LIFETIME")? {
        cfg.max_lifetime = if v == 0 { None } else { Some(v) };
    }
    if let Some(v) = parse_bool_env("YANG_DB_TEST_BEFORE_ACQUIRE")? {
        cfg.test_before_acquire = v;
    }
    if let Some(v) = parse_bool_env("YANG_DB_ENABLE_LOGGING")? {
        cfg.enable_logging = v;
    }
    // PostgreSQL 会话/连接专属参数。
    if let Some(v) = parse_string_env("YANG_PG_SEARCH_PATH")? {
        cfg.search_path = Some(v);
    }
    if let Some(v) = parse_string_env("YANG_PG_APPLICATION_NAME")? {
        cfg.application_name = Some(v);
    }
    if let Some(v) = parse_env::<u64>("YANG_PG_STATEMENT_TIMEOUT_MS")? {
        cfg.statement_timeout_ms = Some(v);
    }
    if let Some(v) = parse_env::<u64>("YANG_PG_LOCK_TIMEOUT_MS")? {
        cfg.lock_timeout_ms = Some(v);
    }
    if let Some(v) = parse_env::<u64>("YANG_PG_IDLE_IN_TRANSACTION_TIMEOUT_MS")? {
        cfg.idle_in_transaction_timeout_ms = Some(v);
    }
    if let Some(v) = parse_string_env("YANG_PG_SSL_MODE")? {
        cfg.ssl_mode = Some(parse_ssl_mode("YANG_PG_SSL_MODE", &v)?);
    }
    if let Some(v) = parse_string_env("YANG_PG_SSL_ROOT_CERT")? {
        cfg.ssl_root_cert = Some(v.into());
    }
    if let Some(v) = parse_string_env("YANG_PG_SSL_CLIENT_CERT")? {
        cfg.ssl_client_cert = Some(v.into());
    }
    if let Some(v) = parse_string_env("YANG_PG_SSL_CLIENT_KEY")? {
        cfg.ssl_client_key = Some(v.into());
    }
    cfg.validate()
        .map_err(|error| BaseError::ConfigError(format!("PostgreSQL 配置无效: {error}")))?;
    Ok(cfg)
}

impl EngineConfig {
    /// 从 `YANG_` 前缀环境变量加载配置。
    ///
    /// 缺失的变量回退到默认值；存在但解析失败返回 [`BaseError::ConfigError`]。
    pub fn from_env() -> Result<Self, BaseError> {
        // 优先 YANG_MYSQL_URL；兼容 deprecated YANG_DATABASE_URL
        let mysql_url = match env::var("YANG_MYSQL_URL") {
            Ok(v) => Some(v),
            Err(env::VarError::NotPresent) => match env::var("YANG_DATABASE_URL") {
                Ok(v) => {
                    eprintln!(
                        "[yang-base] 环境变量 YANG_DATABASE_URL 已 deprecated，请改用 YANG_MYSQL_URL"
                    );
                    Some(v)
                }
                Err(env::VarError::NotPresent) => None,
                Err(env::VarError::NotUnicode(_)) => {
                    return Err(BaseError::ConfigError(
                        "环境变量 YANG_DATABASE_URL 含非法 UTF-8".to_string(),
                    ))
                }
            },
            Err(env::VarError::NotUnicode(_)) => {
                return Err(BaseError::ConfigError(
                    "环境变量 YANG_MYSQL_URL 含非法 UTF-8".to_string(),
                ))
            }
        };
        let postgres_url = match env::var("YANG_POSTGRES_URL") {
            Ok(v) => Some(v),
            Err(env::VarError::NotPresent) => None,
            Err(env::VarError::NotUnicode(_)) => {
                return Err(BaseError::ConfigError(
                    "环境变量 YANG_POSTGRES_URL 含非法 UTF-8".to_string(),
                ))
            }
        };
        let redis_url = match env::var("YANG_REDIS_URL") {
            Ok(v) => Some(v),
            Err(env::VarError::NotPresent) => None,
            Err(env::VarError::NotUnicode(_)) => {
                return Err(BaseError::ConfigError(
                    "环境变量 YANG_REDIS_URL 含非法 UTF-8".to_string(),
                ))
            }
        };

        #[cfg(any(feature = "mysql", feature = "postgres"))]
        let database = load_database_config()?;
        #[cfg(feature = "postgres")]
        let postgres_database = load_postgres_database_config()?;

        #[cfg(feature = "redis")]
        let redis = {
            let mut cfg = yang_db::RedisConfig::default();
            if let Some(v) = parse_env::<usize>("YANG_REDIS_MAX_CONNECTIONS")? {
                cfg.max_connections = v;
            }
            if let Some(v) = parse_env::<u64>("YANG_REDIS_CONNECT_TIMEOUT")? {
                cfg.connect_timeout = v;
            }
            if let Some(v) = parse_env::<u64>("YANG_REDIS_WAIT_TIMEOUT")? {
                cfg.wait_timeout = v;
            }
            if let Some(v) = parse_env::<u64>("YANG_REDIS_IDLE_TIMEOUT")? {
                cfg.idle_timeout = v;
            }
            if let Some(v) = parse_env::<u64>("YANG_REDIS_RESPONSE_TIMEOUT_MS")? {
                // 0 表示显式关闭命令级超时
                cfg.response_timeout_ms = if v == 0 { None } else { Some(v) };
            }
            if let Some(v) = parse_env::<u32>("YANG_REDIS_RECONNECT_RETRIES")? {
                cfg.reconnect_retries = v;
            }
            if let Some(v) = parse_env::<u64>("YANG_REDIS_RECONNECT_MAX_DELAY_MS")? {
                cfg.reconnect_max_delay_ms = v;
            }
            if let Some(v) = parse_env::<usize>("YANG_REDIS_PIPELINE_MAX_COMMANDS")? {
                cfg.pipeline_max_commands = v;
            }
            if let Some(v) = parse_bool_env("YANG_REDIS_ENABLE_LOGGING")? {
                cfg.enable_logging = v;
            }
            cfg.validate()
                .map_err(|error| BaseError::ConfigError(error.to_string()))?;
            cfg
        };

        Ok(Self {
            mysql_url,
            postgres_url,
            redis_url,
            #[cfg(any(feature = "mysql", feature = "postgres"))]
            database,
            #[cfg(feature = "postgres")]
            postgres_database,
            #[cfg(feature = "redis")]
            redis,
        })
    }

    /// 返回 Redis 连接串与池配置（仅在同时配置了 `YANG_REDIS_URL` 时）。
    #[cfg(feature = "redis")]
    pub fn redis_config(&self) -> Option<(&str, &yang_db::RedisConfig)> {
        self.redis_url.as_deref().map(|url| (url, &self.redis))
    }
}

#[cfg(all(test, any(feature = "mysql", feature = "postgres", feature = "redis")))]
mod tests {
    use super::*;

    #[test]
    fn from_env_reads_postgres_url() {
        std::env::set_var("YANG_POSTGRES_URL", "postgres://u:p@127.0.0.1:55432/test");
        let config = EngineConfig::from_env().expect("合法 PostgreSQL URL 应解析成功");
        std::env::remove_var("YANG_POSTGRES_URL");

        assert_eq!(
            config.postgres_url.as_deref(),
            Some("postgres://u:p@127.0.0.1:55432/test")
        );
    }

    #[test]
    fn parse_bool_env_accepts_forms() {
        // 使用唯一 key 避免与其它测试串扰
        std::env::set_var("YANG_TEST_BOOL_X", "TRUE");
        assert_eq!(parse_bool_env("YANG_TEST_BOOL_X").unwrap(), Some(true));
        std::env::set_var("YANG_TEST_BOOL_X", "0");
        assert_eq!(parse_bool_env("YANG_TEST_BOOL_X").unwrap(), Some(false));
        std::env::set_var("YANG_TEST_BOOL_X", "maybe");
        assert!(parse_bool_env("YANG_TEST_BOOL_X").is_err());
        std::env::remove_var("YANG_TEST_BOOL_X");
        assert_eq!(parse_bool_env("YANG_TEST_BOOL_X").unwrap(), None);
    }

    #[test]
    fn parse_env_rejects_bad_number() {
        std::env::set_var("YANG_TEST_NUM_X", "not_a_number");
        let r = parse_env::<u32>("YANG_TEST_NUM_X");
        assert!(matches!(r, Err(BaseError::ConfigError(_))));
        std::env::remove_var("YANG_TEST_NUM_X");
        assert_eq!(parse_env::<u32>("YANG_TEST_NUM_X").unwrap(), None);
    }

    /// `YANG_PG_*` 映射到 PostgreSQL 会话/连接参数。
    #[cfg(feature = "postgres")]
    #[test]
    fn from_env_maps_postgres_session_settings() {
        std::env::set_var("YANG_PG_SEARCH_PATH", "app,public");
        std::env::set_var("YANG_PG_APPLICATION_NAME", "yang-wave3");
        std::env::set_var("YANG_PG_STATEMENT_TIMEOUT_MS", "1500");
        std::env::set_var("YANG_PG_LOCK_TIMEOUT_MS", "2500");
        std::env::set_var("YANG_PG_SSL_MODE", "require");
        let config = EngineConfig::from_env().expect("合法 PostgreSQL 会话配置应解析成功");
        std::env::remove_var("YANG_PG_SEARCH_PATH");
        std::env::remove_var("YANG_PG_APPLICATION_NAME");
        std::env::remove_var("YANG_PG_STATEMENT_TIMEOUT_MS");
        std::env::remove_var("YANG_PG_LOCK_TIMEOUT_MS");
        std::env::remove_var("YANG_PG_SSL_MODE");

        let pg = &config.postgres_database;
        assert_eq!(pg.search_path.as_deref(), Some("app,public"));
        assert_eq!(pg.application_name.as_deref(), Some("yang-wave3"));
        assert_eq!(pg.statement_timeout_ms, Some(1500));
        assert_eq!(pg.lock_timeout_ms, Some(2500));
        assert_eq!(pg.ssl_mode, Some(yang_db::PgSslMode::Require));
    }

    /// 非法 SSL 模式必须报 ConfigError，且错误信息带变量名。
    #[cfg(feature = "postgres")]
    #[test]
    fn parse_ssl_mode_rejects_unknown_value() {
        let error =
            parse_ssl_mode("YANG_PG_SSL_MODE", "sometimes").expect_err("未知 SSL 模式应被拒绝");
        let message = error.to_string();
        assert!(message.contains("YANG_PG_SSL_MODE"), "实得: {message}");
        assert!(message.contains("sometimes"), "实得: {message}");
    }

    /// F058 RED 场景：derive(Debug) 会把连接串里的 userinfo 密码与查询串原样输出。
    #[test]
    fn engine_config_debug_redacts_connection_credentials() {
        let config = EngineConfig {
            mysql_url: Some(
                "mysql://mysql-user:mysql-pass@mysql.example.test:3306/app?ssl-mode=required"
                    .to_string(),
            ),
            postgres_url: Some(
                "postgres://pg-user:pg-pass@pg.example.test:5432/app#fragment".to_string(),
            ),
            redis_url: Some("rediss://redis-user:redis-pass@redis.example.test:6379/0".to_string()),
            ..EngineConfig::default()
        };

        let debug = format!("{config:?}");
        for secret in [
            "mysql-user",
            "mysql-pass",
            "pg-user",
            "pg-pass",
            "redis-user",
            "redis-pass",
            "ssl-mode=required",
            "fragment",
        ] {
            assert!(
                !debug.contains(secret),
                "EngineConfig Debug 泄露 {secret}: {debug}"
            );
        }
        // 展示层仍保留主机与库名，便于排障。
        assert!(debug.contains("mysql.example.test:3306/app"), "{debug}");
        assert!(debug.contains("pg.example.test:5432/app"), "{debug}");
        assert!(
            debug.contains("rediss://redis.example.test:6379/0"),
            "{debug}"
        );
        // 存储值必须保持原始连接串（脱敏只作用于 Debug 文本）。
        assert_eq!(
            config.mysql_url.as_deref(),
            Some("mysql://mysql-user:mysql-pass@mysql.example.test:3306/app?ssl-mode=required")
        );
    }

    #[test]
    fn redact_connection_url_handles_schemes_and_invalid_input() {
        assert_eq!(
            redact_connection_url("mysql://u:p@host:3306/db?x=1#frag"),
            "mysql://host:3306/db"
        );
        assert_eq!(
            redact_connection_url("postgresql://u:p@host/db"),
            "postgresql://host/db"
        );
        assert_eq!(
            redact_connection_url("redis://:p@host:6379/0"),
            "redis://host:6379/0"
        );
        assert_eq!(redact_connection_url("rediss://host/0"), "rediss://host/0");
        assert_eq!(redact_connection_url("not a url"), "[REDACTED]");
        assert_eq!(redact_connection_url("ftp://u:p@host/db"), "[REDACTED]");
        // 密码含未编码 `/`：authority 被截断，输出不得包含密码片段。
        assert_eq!(
            redact_connection_url("mysql://user:pa/ss@host/db"),
            "mysql://[REDACTED]"
        );
        assert_eq!(
            redact_connection_url("mysql://user:pa?ss@host/db"),
            "mysql://[REDACTED]"
        );
    }
}
