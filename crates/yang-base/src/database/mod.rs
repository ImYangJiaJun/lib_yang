//! 数据库管理模块
//!
//! 提供显式数据库初始化与 Schema 同步功能。
//!
//! # 主要组件
//!
//! - `DatabaseInitializer`：数据库初始化器（需启用 `mysql` feature）
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::database::DatabaseInitializer;
//! use yang_base::plugin::PluginManager;
//! use yang_db::Database;
//!
//! // 初始化插件数据库
//! let db = Database::connect("mysql://user:pass@localhost/db").await?;
//! let manager = PluginManager::new();
//! let initializer = DatabaseInitializer::new(db, true);
//! initializer.initialize_all(&manager).await?;
//! ```

#[cfg(feature = "mysql")]
mod initializer;
#[cfg(feature = "mysql")]
mod schema_sync;
#[cfg(all(test, feature = "mysql"))]
mod schema_sync_tests;

#[cfg(feature = "mysql")]
pub use initializer::{
    DatabaseInitializer, MigrationPlan, MigrationPlanEntry, MigrationPlanStatus,
};
#[cfg(feature = "mysql")]
pub use schema_sync::{SchemaSyncChange, SchemaSyncChangeKind, SchemaSyncReport};
