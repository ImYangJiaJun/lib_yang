// MySQL 数据库模块

pub mod condition;
pub mod database;
pub mod field;
pub mod identifier;
pub mod init;
pub mod query_builder;
pub mod transaction;

// 测试模块（仅在测试时编译）
#[cfg(test)]
mod __tests__;

// 重新导出核心类型
#[allow(deprecated)]
pub use condition::{condition_to_sql_owned, condition_to_sql_owned_checked, Condition, SqlValue};
pub use database::{Database, DatabaseConfig};
pub use field::FieldType;
pub use identifier::{is_valid_identifier, quote_identifier, quote_qualified};
pub use query_builder::QueryBuilder;
pub use transaction::Transaction;
