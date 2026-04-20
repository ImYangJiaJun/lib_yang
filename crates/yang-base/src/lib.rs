//! yang-base - YANG 基础库
//!
//! 提供插件管理、数据库访问、HTTP 客户端和 JWT Token 管理等核心功能。
//!
//! # 模块
//!
//! - `plugin`：插件管理模块
//! - `database`：数据库管理模块
//! - `http`：HTTP 客户端模块
//! - `token`：JWT Token 管理模块
//! - `error`：错误处理模块
//!
//! # 快速开始
//!
//! ```rust,ignore
//! use yang_base::plugin::{Plugin, PluginManager};
//! use yang_base::database::GlobalDatabase;
//! use yang_base::http::HttpClient;
//! use yang_base::token::TokenManager;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // 初始化插件管理器
//!     let plugin_manager = PluginManager::new();
//!     
//!     // 初始化全局数据库
//!     GlobalDatabase::init("mysql://user:pass@localhost/db", config).await?;
//!     
//!     // 初始化 HTTP 客户端
//!     HttpClient::init_global(30)?;
//!     
//!     Ok(())
//! }
//! ```

pub mod database;
pub mod error;
pub mod http;
pub mod plugin;
pub mod token;
