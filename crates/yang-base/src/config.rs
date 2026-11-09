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

use crate::error::BaseError;
use std::env;

/// 引擎级聚合配置，从环境变量加载（I4）。
#[derive(Debug, Clone, Default)]
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
}

/// 当前编译所使用的数据库连接池配置类型。
#[cfg(feature = "mysql")]
type DatabaseConfigType = yang_db::DatabaseConfig;
#[cfg(all(feature = "postgres", not(feature = "mysql")))]
type DatabaseConfigType = yang_db::PgDatabaseConfig;

/// 读取一个环境变量并按 `T` 解析；缺失返回 `Ok(None)`，存在但解析失败返回 ConfigError。
#[cfg(any(feature = "mysql", feature = "postgres"))]
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
#[cfg(any(feature = "mysql", feature = "postgres"))]
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

        Ok(Self {
            mysql_url,
            postgres_url,
            redis_url,
            #[cfg(any(feature = "mysql", feature = "postgres"))]
            database,
            #[cfg(feature = "postgres")]
            postgres_database,
        })
    }
}

#[cfg(all(test, any(feature = "mysql", feature = "postgres")))]
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
}
