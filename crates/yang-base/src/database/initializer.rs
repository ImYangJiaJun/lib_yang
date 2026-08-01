//! 声明式数据库结构同步入口。

use yang_db::Database;

/// 持有数据库连接并执行 [`crate::table::TableDefinition`] 声明的结构同步。
pub struct DatabaseInitializer {
    /// 显式拥有的数据库实例。
    db: Database,
}

impl DatabaseInitializer {
    /// 创建数据库结构同步器。
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// 返回底层数据库实例的引用。
    pub(crate) fn db(&self) -> &Database {
        &self.db
    }
}
