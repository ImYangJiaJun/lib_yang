//! 数据库管理模块
//!
//! 提供基于 [`crate::table::TableDefinition`] 的声明式 Schema 同步功能。
//!
//! # 主要组件
//!
//! - [`AnyDatabase`]：运行时数据库后端容器（MySQL / PostgreSQL，需启用 `mysql` 或 `postgres`）
//! - [`AnyPool`]：与后端共享连接池的可克隆句柄，供受保护查询层使用
//! - [`AnyTransaction`]：两种后端事务的统一句柄
//! - [`DatabaseInitializer`]：声明式 Schema 同步器（MySQL / PostgreSQL，按后端分派方言）
//!
//! # 后端支持范围
//!
//! `mysql` 与 `postgres` feature 均可单独启用并编译 Schema 同步。PostgreSQL 的
//! DDL 在单事务内执行、使用 `pg_advisory_lock(bigint)` 串行化多实例启动；枚举
//! 字段暂未支持（fail-closed，见驱动实现）。Record 解码与受保护 TableQuery /
//! 内置 CRUD 的 PostgreSQL 执行层均已接入。
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::database::DatabaseInitializer;
//! use yang_db::Database;
//!
//! let db = Database::connect("mysql://user:pass@localhost/db").await?;
//! let initializer = DatabaseInitializer::new(db);
//! initializer.sync_table_definitions(&table_definitions).await?;
//! ```

#[cfg(test)]
mod __tests__;
#[cfg(any(feature = "mysql", feature = "postgres"))]
mod backend;
#[cfg(any(feature = "mysql", feature = "postgres"))]
mod initializer;
#[cfg(any(feature = "mysql", feature = "postgres"))]
mod schema_sync;

#[cfg(any(feature = "mysql", feature = "postgres"))]
pub use backend::{AnyDatabase, AnyPool, AnyTransaction};
#[cfg(any(feature = "mysql", feature = "postgres"))]
pub use initializer::DatabaseInitializer;
#[cfg(any(feature = "mysql", feature = "postgres"))]
pub use schema_sync::{
    SchemaDataViolation, SchemaPreflightReport, SchemaSyncChange, SchemaSyncChangeKind,
    SchemaSyncReport,
};
