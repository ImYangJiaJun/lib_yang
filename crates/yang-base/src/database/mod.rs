//! 数据库管理模块
//!
//! 提供全局数据库访问和数据库初始化功能。
//!
//! # 主要组件
//!
//! - `GlobalDatabase`：全局 MySQL 数据库访问器（需启用 `mysql` feature）
//! - `GlobalRedis`：全局 Redis 访问器
//! - `DatabaseInitializer`：数据库初始化器（需启用 `mysql` feature）
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::database::{GlobalDatabase, GlobalRedis, DatabaseInitializer};
//! use yang_base::plugin::PluginManager;
//! use yang_db::{Database, DatabaseConfig, RedisConfig};
//!
//! // 初始化全局 MySQL 数据库
//! GlobalDatabase::init("mysql://user:pass@localhost/db", DatabaseConfig::default()).await?;
//!
//! // 初始化全局 Redis
//! GlobalRedis::init("redis://127.0.0.1:6379", RedisConfig::default()).await?;
//!
//! // 使用全局数据库
//! let users = GlobalDatabase::table("users")?
//!     .field("id")
//!     .select::<User>()
//!     .await?;
//!
//! // 使用全局 Redis
//! GlobalRedis::set("key", "value", None).await?;
//! let value: Option<String> = GlobalRedis::get("key").await?;
//!
//! // 初始化插件数据库
//! let db = Database::connect("mysql://user:pass@localhost/db").await?;
//! let manager = PluginManager::new();
//! let initializer = DatabaseInitializer::new(db, true);
//! initializer.initialize_all(&manager).await?;
//! ```

#[cfg(all(feature = "mysql", feature = "redis"))]
mod bundle;
#[cfg(feature = "mysql")]
mod global;
#[cfg(feature = "redis")]
mod global_redis;
#[cfg(feature = "mysql")]
mod initializer;

#[cfg(all(feature = "mysql", feature = "redis"))]
pub use bundle::DatabaseBundle;
#[cfg(feature = "mysql")]
pub use global::GlobalDatabase;
#[cfg(feature = "redis")]
pub use global_redis::GlobalRedis;
#[cfg(feature = "mysql")]
pub use initializer::DatabaseInitializer;
