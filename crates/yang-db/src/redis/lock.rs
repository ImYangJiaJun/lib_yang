//! 分布式锁。
//!
//! 基于 `SET NX PX` + owner token + Lua compare-and-delete，保证只有持有者能释放
//! 自己的锁。锁的 TTL 是进程崩溃时的最终兜底；不提供后台 watchdog 续期
//! （进程生命周期与取消语义复杂，属于上层策略）。

use crate::{DbError, RedisClient, Result};
use std::time::{Duration, Instant};

const RELEASE_SCRIPT: &str = r#"
if redis.call('GET', KEYS[1]) == ARGV[1] then
    return redis.call('DEL', KEYS[1])
else
    return 0
end
"#;

const EXTEND_SCRIPT: &str = r#"
if redis.call('GET', KEYS[1]) == ARGV[1] then
    return redis.call('PEXPIRE', KEYS[1], ARGV[2])
else
    return 0
end
"#;

/// 待获取的分布式锁。
///
/// owner token 在每次 `try_acquire` 成功时生成，确保过期后重新获取不会与
/// 旧 guard 共用 token。
#[derive(Debug, Clone)]
pub struct RedisLock {
    client: RedisClient,
    key: Vec<u8>,
    ttl: Duration,
}

/// 已持有的锁；释放时校验 owner token。
#[derive(Debug)]
#[must_use = "锁 guard 需要显式 release，或等待 TTL 自动过期"]
pub struct RedisLockGuard {
    client: RedisClient,
    key: Vec<u8>,
    token: String,
}

impl RedisClient {
    /// 创建锁实例（默认 TTL 30 秒）。
    pub fn lock(&self, key: impl AsRef<[u8]>) -> RedisLock {
        RedisLock {
            client: self.clone(),
            key: self.key_for(key),
            ttl: Duration::from_secs(30),
        }
    }
}

impl RedisLock {
    /// 设置锁 TTL。
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    /// 尝试获取锁；已被占用返回 `None`。
    pub async fn try_acquire(&self) -> Result<Option<RedisLockGuard>> {
        let ttl_ms = ttl_millis(self.ttl)?;
        // 每次获取都使用新的 owner token，避免旧 guard 释放/续期新的持有者。
        let token = uuid::Uuid::new_v4().to_string();
        if self
            .client
            .set_nx_px(&self.key, token.as_bytes(), ttl_ms)
            .await?
        {
            Ok(Some(RedisLockGuard {
                client: self.client.clone(),
                key: self.key.clone(),
                token,
            }))
        } else {
            Ok(None)
        }
    }

    /// 在 deadline 内轮询获取锁；超时返回 `RedisTimeoutError`。
    pub async fn acquire(&self, deadline: Duration) -> Result<RedisLockGuard> {
        let started = Instant::now();
        loop {
            if let Some(guard) = self.try_acquire().await? {
                return Ok(guard);
            }
            if started.elapsed() >= deadline {
                return Err(DbError::RedisTimeoutError("获取 Redis 锁超时".to_string()));
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

impl RedisLockGuard {
    /// 锁的 key。
    pub fn key(&self) -> &[u8] {
        &self.key
    }

    /// 释放锁；只有 owner token 仍匹配时才删除，返回是否真正释放。
    pub async fn release(self) -> Result<bool> {
        let mut cmd = redis::cmd("EVAL");
        cmd.arg(RELEASE_SCRIPT)
            .arg(1)
            .arg(self.key.as_slice())
            .arg(self.token.as_str());
        Ok(self.client.execute(&cmd).await?.as_i64() == Some(1))
    }

    /// 延长锁 TTL；只有 owner token 仍匹配时才续期。
    pub async fn extend(&self, ttl: Duration) -> Result<bool> {
        let ttl_ms = ttl_millis(ttl)?;
        let mut cmd = redis::cmd("EVAL");
        cmd.arg(EXTEND_SCRIPT)
            .arg(1)
            .arg(self.key.as_slice())
            .arg(self.token.as_str())
            .arg(ttl_ms);
        Ok(self.client.execute(&cmd).await?.as_i64() == Some(1))
    }
}

fn ttl_millis(ttl: Duration) -> Result<u64> {
    let millis = u64::try_from(ttl.as_millis())
        .map_err(|_| DbError::InvalidArgument("锁 TTL 超出 u64 范围".to_string()))?;
    if millis == 0 {
        return Err(DbError::InvalidArgument(
            "锁 TTL 必须大于 0 毫秒".to_string(),
        ));
    }
    Ok(millis)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn ttl_millis_rejects_zero_and_overflow() {
        assert!(ttl_millis(Duration::ZERO).is_err());
        assert_eq!(ttl_millis(Duration::from_millis(1)).unwrap(), 1);
    }
}
