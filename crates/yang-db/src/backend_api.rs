//! 跨后端统一管理面 trait。
//!
//! 让编排层用同一组调用处理 MySQL / PostgreSQL / Redis 的健康检查、连接池
//! 状态与关闭，而不必按后端逐个 match。

use crate::{DbError, PoolStatus};

/// 数据库后端统一管理面。
#[async_trait::async_trait]
pub trait Backend: Send + Sync {
    /// 后端名称（`mysql` / `postgres` / `redis`）。
    fn backend_name(&self) -> &'static str;

    /// 连接池状态。
    fn pool_status(&self) -> PoolStatus;

    /// 健康检查。
    async fn health_check(&self) -> Result<bool, DbError>;

    /// 关闭连接池（幂等）。
    async fn close(&self);
}

#[cfg(feature = "redis")]
#[async_trait::async_trait]
impl Backend for crate::RedisClient {
    fn backend_name(&self) -> &'static str {
        "redis"
    }

    fn pool_status(&self) -> PoolStatus {
        crate::RedisClient::pool_status(self)
    }

    async fn health_check(&self) -> Result<bool, DbError> {
        crate::RedisClient::health_check(self).await
    }

    async fn close(&self) {
        crate::RedisClient::close(self).await;
    }
}

#[cfg(feature = "mysql")]
#[async_trait::async_trait]
impl Backend for crate::mysql::Database {
    fn backend_name(&self) -> &'static str {
        "mysql"
    }

    fn pool_status(&self) -> PoolStatus {
        crate::mysql::Database::pool_status(self)
    }

    async fn health_check(&self) -> Result<bool, DbError> {
        crate::mysql::Database::health_check(self).await
    }

    async fn close(&self) {
        crate::mysql::Database::close(self).await;
    }
}

#[cfg(feature = "postgres")]
#[async_trait::async_trait]
impl Backend for crate::postgres::Database {
    fn backend_name(&self) -> &'static str {
        "postgres"
    }

    fn pool_status(&self) -> PoolStatus {
        crate::postgres::Database::pool_status(self)
    }

    async fn health_check(&self) -> Result<bool, DbError> {
        crate::postgres::Database::health_check(self).await
    }

    async fn close(&self) {
        crate::postgres::Database::close(self).await;
    }
}
