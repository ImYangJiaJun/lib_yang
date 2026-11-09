//! 声明式数据库结构同步入口。
//!
//! [`DatabaseInitializer`] 持有运行时容器 [`super::AnyDatabase`]，具体方言与
//! 连接类型由 schema 同步驱动按后端分派。

use super::AnyDatabase;

/// 持有数据库连接并执行 [`crate::table::TableDefinition`] 声明的结构同步。
pub struct DatabaseInitializer {
    /// 显式拥有的数据库实例。
    db: AnyDatabase,
}

impl DatabaseInitializer {
    /// 用 MySQL 连接构造同步器（保持既有调用方式）。
    #[cfg(feature = "mysql")]
    pub fn new(db: yang_db::mysql::Database) -> Self {
        Self::from_any(AnyDatabase::mysql(db))
    }

    /// 用 PostgreSQL 连接构造同步器。
    #[cfg(feature = "postgres")]
    pub fn new_postgres(db: yang_db::postgres::Database) -> Self {
        Self::from_any(AnyDatabase::postgres(db))
    }

    /// 用运行时数据库容器构造同步器。
    pub fn from_any(db: AnyDatabase) -> Self {
        Self { db }
    }

    /// 返回底层数据库容器。
    pub(crate) fn db(&self) -> &AnyDatabase {
        &self.db
    }
}
