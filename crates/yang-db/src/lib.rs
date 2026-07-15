// 错误类型模块
pub mod error;

// 事务隔离级别（NG-2）
pub mod isolation;

// MySQL 数据库模块
pub mod mysql;

// PostgreSQL 数据库模块
pub mod postgres;

// Redis 数据库模块
pub mod redis;

mod sql_types;

// 重新导出错误类型
pub use error::{DbError, DbErrorCategory};

// 重新导出事务隔离级别
pub use isolation::IsolationLevel;

// 重新导出 MySQL 核心类型
#[allow(deprecated)]
pub use mysql::{
    condition_to_sql_owned, condition_to_sql_owned_checked, quote_identifier, quote_qualified,
    Condition, Database, DatabaseConfig, FieldType, QueryBuilder, SqlValue, Transaction,
};

// 重新导出 PostgreSQL 核心类型（以 Pg 前缀避免与 MySQL 同名类型冲突）。
// 需要完整 PostgreSQL API 时也可直接使用 `yang_db::postgres::*`，
// 其内部类型名与 `yang_db::mysql::*` 一致，调用方式保持统一。
#[allow(deprecated)]
pub use postgres::{
    condition_to_sql_owned as pg_condition_to_sql_owned,
    condition_to_sql_owned_checked as pg_condition_to_sql_owned_checked, Condition as PgCondition,
    Database as PgDatabase, DatabaseConfig as PgDatabaseConfig, FieldType as PgFieldType,
    QueryBuilder as PgQueryBuilder, SqlValue as PgSqlValue, Transaction as PgTransaction,
};

// 重新导出 Redis 核心类型
pub use redis::{
    PoolStatus, RedisClient, RedisConfig, RedisPipeline, RedisTransaction, RedisValue,
};

// 类型别名
pub type Result<T> = std::result::Result<T, DbError>;
