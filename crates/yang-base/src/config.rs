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
//! | `YANG_DATABASE_URL` | MySQL 连接串 | 无（缺失则 `database_url` 为 `None`） |
//! | `YANG_REDIS_URL` | Redis 连接串 | 无 |
//! | `YANG_DB_MAX_CONNECTIONS` | 最大连接数 | 由 `DatabaseConfig::default` 决定 |
//! | `YANG_DB_MIN_CONNECTIONS` | 最小（保活）连接数 | 同上 |
//! | `YANG_DB_CONNECT_TIMEOUT` | 连接超时（秒） | 同上 |
//! | `YANG_DB_IDLE_TIMEOUT` | 空闲超时（秒） | 同上 |
//! | `YANG_DB_MAX_LIFETIME` | 连接最大存活（秒，0/缺失=不限制） | 不限制 |
//! | `YANG_DB_TEST_BEFORE_ACQUIRE` | 借出前探活（true/false/1/0） | false |
//! | `YANG_DB_ENABLE_LOGGING` | SQL 日志（true/false/1/0） | false |

use crate::error::BaseError;
use std::env;

/// 引擎级聚合配置，从环境变量加载（I4）。
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct EngineConfig {
    /// MySQL 连接串（`YANG_DATABASE_URL`，缺失为 `None`）。
    pub database_url: Option<String>,
    /// Redis 连接串（`YANG_REDIS_URL`，缺失为 `None`）。
    pub redis_url: Option<String>,
    /// 数据库连接池配置（由 `YANG_DB_*` 覆盖 `DatabaseConfig::default`）。
    #[cfg(feature = "mysql")]
    pub database: yang_db::DatabaseConfig,
}

/// 读取一个环境变量并按 `T` 解析；缺失返回 `Ok(None)`，存在但解析失败返回 ConfigError。
#[cfg(feature = "mysql")]
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
#[cfg(feature = "mysql")]
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

impl EngineConfig {
    /// 从 `YANG_` 前缀环境变量加载配置。
    ///
    /// 缺失的变量回退到默认值；存在但解析失败返回 [`BaseError::ConfigError`]。
    pub fn from_env() -> Result<Self, BaseError> {
        let database_url = match env::var("YANG_DATABASE_URL") {
            Ok(v) => Some(v),
            Err(env::VarError::NotPresent) => None,
            Err(env::VarError::NotUnicode(_)) => {
                return Err(BaseError::ConfigError(
                    "环境变量 YANG_DATABASE_URL 含非法 UTF-8".to_string(),
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

        #[cfg(feature = "mysql")]
        let database = {
            let mut cfg = yang_db::DatabaseConfig::default();
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
            cfg
        };

        Ok(Self {
            database_url,
            redis_url,
            #[cfg(feature = "mysql")]
            database,
        })
    }
}

#[cfg(all(test, feature = "mysql"))]
mod tests {
    use super::*;

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
}
