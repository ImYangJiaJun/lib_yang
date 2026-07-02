//! 全局数据库统一初始化入口（H-3）
//!
//! [`GlobalDatabase`] 与 [`GlobalRedis`] 各自独立 `init`，调用方容易漏掉其中一个、
//! 或对初始化顺序无约束。[`DatabaseBundle`] 提供单一入口，一次性按固定顺序
//! （先 MySQL 再 Redis）初始化两个全局单例，任一步失败即返回，供调用方据此中止启动。
//!
//! 注意：两个单例底层都是进程级 `OnceLock`，一旦写入便无法重置。若 MySQL 初始化
//! 成功而 Redis 初始化失败，Bundle 会主动调用 [`GlobalDatabase::close`] 关闭已建立
//! 的 MySQL 连接池（释放底层连接资源），但 `OnceLock` 槽位仍被占用，同一进程内不支持
//! 二次调用 `init` 重试。如需重新初始化，只能重启进程。
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::database::DatabaseBundle;
//! use yang_db::{DatabaseConfig, redis::RedisConfig};
//!
//! DatabaseBundle::init(
//!     "mysql://user:pass@localhost/db",
//!     DatabaseConfig::default(),
//!     "redis://127.0.0.1:6379",
//!     RedisConfig::default(),
//! )
//! .await?;
//! ```

use crate::database::{GlobalDatabase, GlobalRedis};
use crate::error::BaseError;
use yang_db::redis::RedisConfig;
use yang_db::DatabaseConfig;

/// 全局数据库统一初始化入口。
///
/// 把 [`GlobalDatabase::init`] 与 [`GlobalRedis::init`] 组合成单次调用，
/// 固定"先 MySQL 再 Redis"的顺序。任一步失败直接返回错误，调用方据此中止启动。
pub struct DatabaseBundle;

impl DatabaseBundle {
    /// 一次性初始化全局 MySQL 与 Redis 单例。
    ///
    /// # 参数
    ///
    /// - `mysql_url`: MySQL 连接串
    /// - `mysql_config`: MySQL 连接池配置
    /// - `redis_url`: Redis 连接串
    /// - `redis_config`: Redis 连接池配置
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 两个全局单例均初始化成功
    /// - `Err(BaseError)`: MySQL 或 Redis 任一初始化失败（保持先 MySQL 后 Redis 的顺序）
    ///
    /// # 半初始化说明
    ///
    /// 按"先 MySQL 再 Redis"的固定顺序初始化，任一步失败立即返回供调用方中止启动。
    /// 若 MySQL 已成功写入而 Redis 失败，本方法会主动调用 [`GlobalDatabase::close`]
    /// 关闭已建立的 MySQL 连接池（释放底层连接资源），但 `OnceLock` 槽位仍被占用，
    /// 同一进程内**不支持**再次调用 `init` 重试。此时应视为启动失败并重启进程。
    ///
    /// # 错误
    ///
    /// - `DatabaseConnectionFailed` / `DatabaseAlreadyInitialized`: 来自 [`GlobalDatabase::init`]
    /// - Redis 连接 / 重复初始化错误: 来自 [`GlobalRedis::init`]
    pub async fn init(
        mysql_url: &str,
        mysql_config: DatabaseConfig,
        redis_url: &str,
        redis_config: RedisConfig,
    ) -> Result<(), BaseError> {
        GlobalDatabase::init(mysql_url, mysql_config).await?;
        if let Err(e) = GlobalRedis::init(redis_url, redis_config).await {
            // MySQL 池已写入 OnceLock，主动关闭连接池释放底层资源
            GlobalDatabase::close().await;
            return Err(e);
        }
        log::info!("全局数据库与 Redis 已通过 DatabaseBundle 统一初始化");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 非法 MySQL 连接串应在连接阶段失败，且不应 panic。
    ///
    /// 注：`Global*` 为进程级 `OnceLock` 单例，成功路径需真实 DB（见 testcontainers
    /// 集成测试），此处只覆盖"先 MySQL 失败即返回、不触达 Redis"的错误路径。
    /// 标记 `#[ignore]`：会发起真实网络连接，默认不在单元测试中执行。
    #[tokio::test]
    #[ignore = "需要网络/DB 环境，发起真实连接；默认跳过"]
    async fn test_bundle_init_invalid_mysql_returns_err() {
        let result = DatabaseBundle::init(
            "mysql://invalid:invalid@127.0.0.1:1/nonexistent_db",
            DatabaseConfig::default(),
            "redis://127.0.0.1:6379",
            RedisConfig::default(),
        )
        .await;

        assert!(result.is_err(), "非法 MySQL 连接串应返回 Err 而非 panic");
    }
}
