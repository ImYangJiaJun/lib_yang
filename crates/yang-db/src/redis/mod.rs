// Redis 数据库模块

pub(crate) mod backend;
pub mod bytes;
pub mod client;
pub mod codec;
pub mod commands;
pub mod config;
pub mod lock;
pub mod namespace;
pub mod pipeline;
pub mod pubsub;
pub mod scan;
pub mod streams;
pub mod transaction;
pub mod value;

// 重新导出核心类型
pub use crate::PoolStatus;
pub use client::{RedisClient, RedisHealth, RedisTopology};
pub use codec::{JsonCodec, RedisCodec};
pub use commands::{GetExOption, SetOptions, ZAddOptions};
pub use config::RedisConfig;
pub use lock::{RedisLock, RedisLockGuard};
pub use namespace::KeyNamespace;
pub use pipeline::RedisPipeline;
pub use pubsub::{RedisMessage, RedisPubSub};
pub use streams::XReadGroupOptions;
pub use transaction::RedisTransaction;
pub use value::RedisValue;
