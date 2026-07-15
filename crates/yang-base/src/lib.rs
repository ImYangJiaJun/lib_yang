//! yang-base - YANG 基础库
//!
//! 提供插件管理、数据库访问、HTTP 客户端和 JWT Token 管理等核心功能。
//!
//! # 模块
//!
//! - `plugin`：插件管理模块
//! - `database`：数据库管理模块（支持 MySQL 和 Redis）
//! - `http`：HTTP 客户端模块（需启用 `http` feature）
//! - `token`：JWT Token 管理模块（需启用 `token` feature）
//! - `error`：错误处理模块
//! - `table`：表配置系统模块
//! - `action`：Action 系统模块
//!
//! # Feature Gates
//!
//! - `token`：启用 JWT Token 管理功能（依赖 `jsonwebtoken`）
//! - `http`：启用 HTTP 客户端功能（依赖 `reqwest`、`serde_urlencoded`）
//! - `mysql`：启用 MySQL 数据库查询执行功能（依赖 `sqlx`）
//! - `redis`：启用 Redis 全局访问功能（`token` 会自动启用它）
//! - `validator`：启用正则表达式校验器（依赖 `regex`）
//! - `plugin-schema`：启用插件 JSON Schema 配置验证（依赖 `jsonschema`）
//!
//! # 快速开始
//!
//! ```rust,ignore
//! use yang_base::plugin::{Plugin, PluginManager};
//! use yang_base::database::{GlobalDatabase, GlobalRedis};
//! use yang_base::http::HttpClient;
//! use yang_base::token::TokenManager;
//! use yang_db::{DatabaseConfig, redis::RedisConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // 初始化插件管理器
//!     let plugin_manager = PluginManager::new();
//!     
//!     // 初始化全局 MySQL 数据库
//!     GlobalDatabase::init("mysql://user:pass@localhost/db", DatabaseConfig::default()).await?;
//!     
//!     // 初始化全局 Redis
//!     GlobalRedis::init("redis://127.0.0.1:6379", RedisConfig::default()).await?;
//!     
//!     // 初始化 HTTP 客户端
//!     HttpClient::init_global(30)?;
//!     
//!     // 使用 MySQL
//!     let users = GlobalDatabase::table("users")?
//!         .select::<User>()
//!         .await?;
//!     
//!     // 使用 Redis
//!     GlobalRedis::set("key", "value", None).await?;
//!     let value = GlobalRedis::get("key").await?;
//!     
//!     Ok(())
//! }
//! ```

// 强制公开 API 文档覆盖率检查：所有公开项必须有文档注释
#![warn(missing_docs)]

// 让派生宏内部的 `::yang_base::...` 路径在 yang-base 自身的测试中正常解析。
extern crate self as yang_base;

pub mod action;
pub mod config;
pub mod database;
pub mod error;
#[cfg(feature = "http")]
pub mod http;
pub mod lifecycle;
pub mod observability;
pub mod plugin;
pub mod router;
pub mod table;
#[cfg(feature = "token")]
pub mod token;

// 重新导出插件系统的核心类型，方便用户直接使用
pub use plugin::{Plugin, PluginManager, PluginManagerBuilder, PluginRegistry};

// 重新导出统一错误类型与 Result 别名，下游可直接写 yang_base::Result<T>
pub use error::{BaseError, ErrorCategory, Result};

// 重新导出派生宏
pub use yang_base_derive::{Action, TableEntity};
