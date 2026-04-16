// MySQL 查询构建器模块
pub mod condition;
pub mod database;
pub mod error;
pub mod field;
pub mod init;
pub mod query_builder;
pub mod transaction;

// 测试模块
#[cfg(test)]
mod tests;

// 基于属性的测试模块
#[cfg(test)]
mod property_tests;

// 重新导出核心类型
pub use condition::{Condition, SqlValue};
pub use database::{Database, DatabaseConfig};
pub use error::DbError;
pub use field::FieldType;
pub use query_builder::QueryBuilder;
pub use transaction::Transaction;

// 类型别名
pub type Result<T> = std::result::Result<T, DbError>;
