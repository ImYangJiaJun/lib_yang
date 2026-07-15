// PostgreSQL 数据库模块
//
// 与 `mysql` 模块保持一致的 API 形态：`Database` / `QueryBuilder` / `Transaction`
// 等类型在本模块内同名，使 `yang_db::postgres::Database::connect(...)` 与
// `yang_db::mysql::Database::connect(...)` 的调用方式完全一致。
//
// 方言差异（集中体现，便于审阅）：
// - 占位符：PostgreSQL 使用编号占位符 `$1`、`$2` ……（见 `condition.rs`），
//   MySQL 使用 `?`。
// - 自增主键：`insert()` 通过 `RETURNING` 子句取回生成的主键，而非 MySQL 的
//   `last_insert_id()`。
// - UPSERT：使用 `INSERT ... ON CONFLICT (...) DO UPDATE SET col = EXCLUDED.col`，
//   而非 MySQL 的 `ON DUPLICATE KEY UPDATE`。
// - 聚合：`sum`/`avg` 使用 `CAST(... AS DOUBLE PRECISION)`，而非 MySQL 的 `DOUBLE`。

pub mod condition;
pub mod database;
pub mod field;
pub mod identifier;
pub mod init;
pub mod query_builder;
pub mod transaction;

// 重新导出核心类型
#[allow(deprecated)]
pub use condition::{condition_to_sql_owned, condition_to_sql_owned_checked, Condition, SqlValue};
pub use database::{Database, DatabaseConfig};
pub use field::FieldType;
pub use identifier::{is_valid_identifier, quote_identifier, quote_qualified};
pub use query_builder::QueryBuilder;
pub use transaction::Transaction;
