// 错误类型模块
pub mod error;

// MySQL 数据库模块
pub mod mysql;

// Redis 数据库模块
pub mod redis;

// 重新导出错误类型
pub use error::DbError;

// 重新导出 MySQL 核心类型
pub use mysql::{
    condition_to_sql_owned, Condition, Database, DatabaseConfig, FieldType, QueryBuilder, SqlValue,
    Transaction,
};

// 重新导出 Redis 核心类型
pub use redis::{
    PoolStatus, RedisClient, RedisConfig, RedisPipeline, RedisTransaction, RedisValue,
};

// 类型别名
pub type Result<T> = std::result::Result<T, DbError>;
