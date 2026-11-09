//! Redis 后端执行器：统一 standalone / cluster / sentinel 三种连接池。
//!
//! 对上层暴露 [`RedisConn`] 枚举，内部按拓扑分支执行同一条命令；这样
//! `client.rs` 的命令实现不需要为每种拓扑复制一份。

use crate::{DbError, PoolStatus, Result};
use deadpool_redis::Pool;
use redis::{FromRedisValue, RedisResult, ScriptInvocation};
use std::time::Duration;

/// Redis 拓扑后端。
#[derive(Clone)]
pub(crate) enum RedisBackend {
    /// 单机（含 TLS）
    Single(Pool),
    /// Redis Cluster
    #[cfg(feature = "redis-cluster")]
    Cluster(deadpool_redis::cluster::Pool),
    /// Redis Sentinel
    #[cfg(feature = "redis-sentinel")]
    Sentinel(deadpool_redis::sentinel::Pool),
}

/// 从池中借出的连接。
pub(crate) enum RedisConn {
    /// 单机连接
    Single(deadpool_redis::Connection),
    /// Cluster 连接
    #[cfg(feature = "redis-cluster")]
    Cluster(deadpool_redis::cluster::Connection),
    /// Sentinel 连接
    #[cfg(feature = "redis-sentinel")]
    Sentinel(deadpool_redis::sentinel::Connection),
}

impl RedisBackend {
    /// 从当前拓扑的池中借出一个连接。
    pub(crate) async fn get(&self) -> Result<RedisConn> {
        match self {
            RedisBackend::Single(pool) => pool
                .get()
                .await
                .map(RedisConn::Single)
                .map_err(DbError::from),
            #[cfg(feature = "redis-cluster")]
            RedisBackend::Cluster(pool) => pool
                .get()
                .await
                .map(RedisConn::Cluster)
                .map_err(map_cluster_pool_error),
            #[cfg(feature = "redis-sentinel")]
            RedisBackend::Sentinel(pool) => pool
                .get()
                .await
                .map(RedisConn::Sentinel)
                .map_err(map_sentinel_pool_error),
        }
    }

    /// 连接池状态。
    pub(crate) fn status(&self) -> PoolStatus {
        let status = match self {
            RedisBackend::Single(pool) => pool.status(),
            #[cfg(feature = "redis-cluster")]
            RedisBackend::Cluster(pool) => pool.status(),
            #[cfg(feature = "redis-sentinel")]
            RedisBackend::Sentinel(pool) => pool.status(),
        };
        PoolStatus {
            max_size: status.max_size,
            size: status.size,
            available: status.available,
            waiting: status.waiting,
        }
    }

    /// 关闭当前拓扑的连接池。
    pub(crate) fn close(&self) {
        match self {
            RedisBackend::Single(pool) => pool.close(),
            #[cfg(feature = "redis-cluster")]
            RedisBackend::Cluster(pool) => pool.close(),
            #[cfg(feature = "redis-sentinel")]
            RedisBackend::Sentinel(pool) => pool.close(),
        }
    }

    /// 连接池是否已关闭。
    pub(crate) fn is_closed(&self) -> bool {
        match self {
            RedisBackend::Single(pool) => pool.is_closed(),
            #[cfg(feature = "redis-cluster")]
            RedisBackend::Cluster(pool) => pool.is_closed(),
            #[cfg(feature = "redis-sentinel")]
            RedisBackend::Sentinel(pool) => pool.is_closed(),
        }
    }
}

impl RedisConn {
    /// 设置命令级响应超时。
    ///
    /// Cluster 的 `ClusterConnection` 在 redis 0.32 没有该 API，因此只在
    /// standalone / sentinel 生效。
    pub(crate) fn set_response_timeout(&mut self, timeout: Duration) {
        match self {
            RedisConn::Single(conn) => conn.set_response_timeout(timeout),
            #[cfg(feature = "redis-sentinel")]
            RedisConn::Sentinel(conn) => conn.set_response_timeout(timeout),
            #[cfg(feature = "redis-cluster")]
            RedisConn::Cluster(_) => {}
        }
    }

    /// 执行单条命令。
    pub(crate) async fn query<T>(&mut self, cmd: &redis::Cmd) -> RedisResult<T>
    where
        T: FromRedisValue,
    {
        match self {
            RedisConn::Single(conn) => cmd.query_async(conn).await,
            #[cfg(feature = "redis-cluster")]
            RedisConn::Cluster(conn) => cmd.query_async(conn).await,
            #[cfg(feature = "redis-sentinel")]
            RedisConn::Sentinel(conn) => cmd.query_async(conn).await,
        }
    }

    /// 执行 Pipeline。
    pub(crate) async fn query_pipeline<T>(&mut self, pipe: &redis::Pipeline) -> RedisResult<T>
    where
        T: FromRedisValue,
    {
        match self {
            RedisConn::Single(conn) => pipe.query_async(conn).await,
            #[cfg(feature = "redis-cluster")]
            RedisConn::Cluster(conn) => pipe.query_async(conn).await,
            #[cfg(feature = "redis-sentinel")]
            RedisConn::Sentinel(conn) => pipe.query_async(conn).await,
        }
    }

    /// 执行 Lua 脚本调用。
    pub(crate) async fn invoke_script<T>(
        &mut self,
        invocation: ScriptInvocation<'_>,
    ) -> RedisResult<T>
    where
        T: FromRedisValue,
    {
        match self {
            RedisConn::Single(conn) => invocation.invoke_async(conn).await,
            #[cfg(feature = "redis-cluster")]
            RedisConn::Cluster(conn) => invocation.invoke_async(conn).await,
            #[cfg(feature = "redis-sentinel")]
            RedisConn::Sentinel(conn) => invocation.invoke_async(conn).await,
        }
    }
}

/// 判断错误是否值得重新取连接重试。
///
/// 只对「连接已不可用」类错误重试；超时（包括 response timeout，redis-rs 会把它
/// 包装成 IoError/TimedOut）不重试，避免非幂等命令被重复执行。
pub(crate) fn should_reconnect(error: &redis::RedisError) -> bool {
    if error.is_timeout() {
        return false;
    }
    error.is_connection_dropped() || error.is_connection_refusal() || error.is_io_error()
}

/// 指数退避（上限由配置控制）。
pub(crate) fn reconnect_delay(attempt: u32, max_delay_ms: u64) -> Duration {
    let exponent = attempt.min(10);
    let delay = 10u64.saturating_mul(1u64 << exponent);
    Duration::from_millis(delay.min(max_delay_ms.max(1)))
}

#[cfg(feature = "redis-cluster")]
fn map_cluster_pool_error(err: deadpool_redis::cluster::PoolError) -> DbError {
    use deadpool_redis::cluster::PoolError as E;
    match err {
        E::Timeout(_) => DbError::RedisTimeoutError("连接池获取连接超时".to_string()),
        E::Closed => DbError::RedisPoolClosed,
        E::NoRuntimeSpecified => DbError::RedisPoolError("未指定运行时".to_string()),
        E::PostCreateHook(source) => {
            DbError::RedisPoolError(format!("连接创建后钩子失败: {:?}", source))
        }
        E::Backend(err) => DbError::from(err),
    }
}

#[cfg(feature = "redis-sentinel")]
fn map_sentinel_pool_error(err: deadpool_redis::sentinel::PoolError) -> DbError {
    use deadpool_redis::sentinel::PoolError as E;
    match err {
        E::Timeout(_) => DbError::RedisTimeoutError("连接池获取连接超时".to_string()),
        E::Closed => DbError::RedisPoolClosed,
        E::NoRuntimeSpecified => DbError::RedisPoolError("未指定运行时".to_string()),
        E::PostCreateHook(source) => {
            DbError::RedisPoolError(format!("连接创建后钩子失败: {:?}", source))
        }
        E::Backend(err) => DbError::from(err),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn timeout_errors_are_not_retried() {
        let error = redis::RedisError::from(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "response timeout",
        ));
        assert!(error.is_timeout(), "构造的超时错误应被识别为 timeout");
        assert!(
            !should_reconnect(&error),
            "response timeout 不能触发重连重试，否则非幂等命令会被重复执行"
        );
    }

    #[test]
    fn reconnect_delay_is_bounded() {
        assert!(reconnect_delay(1, 200) <= Duration::from_millis(200));
        assert!(reconnect_delay(10, 50) <= Duration::from_millis(50));
    }
}
