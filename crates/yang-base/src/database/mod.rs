//! 数据库管理模块
//!
//! 提供全局数据库访问和数据库初始化功能。
//!
//! # 主要组件
//!
//! - `GlobalDatabase`：全局数据库访问器
//! - `DatabaseInitializer`：数据库初始化器
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::database::{GlobalDatabase, DatabaseInitializer};
//! use yang_base::plugin::PluginManager;
//! use yang_db::Database;
//!
//! // 初始化全局数据库
//! GlobalDatabase::init("mysql://user:pass@localhost/db", config).await?;
//!
//! // 使用全局数据库
//! let users = GlobalDatabase::table("users")
//!     .select()
//!     .get()
//!     .await?;
//!
//! // 初始化插件数据库
//! let db = Database::connect("mysql://user:pass@localhost/db").await?;
//! let manager = PluginManager::new();
//! let initializer = DatabaseInitializer::new(db, true);
//! initializer.initialize_all(&manager).await?;
//! ```

mod global;
mod initializer;

pub use global::GlobalDatabase;
pub use initializer::DatabaseInitializer;
