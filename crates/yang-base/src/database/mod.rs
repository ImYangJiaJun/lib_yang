//! 数据库管理模块
//!
//! 提供基于 [`crate::table::TableDefinition`] 的声明式 Schema 同步功能。
//!
//! # 主要组件
//!
//! - `DatabaseInitializer`：数据库结构同步器（需启用 `mysql` feature）
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::database::DatabaseInitializer;
//! use yang_db::Database;
//!
//! let db = Database::connect("mysql://user:pass@localhost/db").await?;
//! let initializer = DatabaseInitializer::new(db);
//! initializer.sync_tables(&table_definitions).await?;
//! ```

#[cfg(feature = "mysql")]
mod initializer;
#[cfg(feature = "mysql")]
mod schema_sync;
#[cfg(all(test, feature = "mysql"))]
mod schema_sync_tests;

#[cfg(feature = "mysql")]
pub use initializer::DatabaseInitializer;
#[cfg(feature = "mysql")]
pub use schema_sync::{
    SchemaDataViolation, SchemaPreflightReport, SchemaSyncChange, SchemaSyncChangeKind,
    SchemaSyncReport,
};
