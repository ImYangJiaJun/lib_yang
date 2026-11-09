//! 通用 Redis 缓存与固定窗口限流门面（`feature = "redis"`）。
//!
//! 提供 cache-aside（`get_or_load`）、空值缓存、TTL 抖动、single-flight
//! 与通用限流；安全路径（token 撤销、MFA、鉴权限流）仍保持各自模块的
//! fail-closed 策略，不经过本模块的 `fail_open` 开关。

use crate::error::{BaseError, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex as StdMutex, Weak};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use yang_db::{JsonCodec, RedisClient, RedisCodec};

/// 空值缓存哨兵：与普通 JSON 值区分（JSON 编码不会产生该字节序列）。
const NULL_SENTINEL: &[u8] = b"\x00yang-cache-null";

/// 缓存行为配置。
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// 默认 TTL。
    pub default_ttl: Duration,
    /// TTL 随机抖动百分比（0-100），用于分散过期时间、避免缓存雪崩。
    pub ttl_jitter_percent: u8,
    /// 空值缓存 TTL。
    pub null_ttl: Duration,
    /// 是否启用 single-flight（同一 key 并发 miss 只回源一次）。
    pub single_flight: bool,
    /// Redis 故障时是否按 cache miss 处理（仅适用于纯缓存场景）。
    ///
    /// 安全敏感路径必须保持 `false`（fail-closed）。
    pub fail_open: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            default_ttl: Duration::from_secs(300),
            ttl_jitter_percent: 10,
            null_ttl: Duration::from_secs(60),
            single_flight: true,
            fail_open: false,
        }
    }
}

/// 通用缓存门面。
#[derive(Clone)]
pub struct Cache {
    client: RedisClient,
    config: CacheConfig,
    // 每个 key 一个 tokio Mutex，用 Weak 保存：所有 waiter 释放后条目可自动回收，
    // 不会因任务取消/panic 导致 map 无界增长，也不会在 remove 时误删他人正在用的锁。
    key_locks: Arc<StdMutex<HashMap<String, Weak<Mutex<()>>>>>,
}

enum Lookup {
    Hit(Vec<u8>),
    Null,
    Miss,
}

impl Cache {
    /// 创建缓存门面。
    pub fn new(client: RedisClient, config: CacheConfig) -> Self {
        Self {
            client,
            config,
            key_locks: Arc::new(StdMutex::new(HashMap::new())),
        }
    }

    /// 底层 Redis 客户端（高级用法）。
    pub fn client(&self) -> &RedisClient {
        &self.client
    }

    /// 当前配置。
    pub fn config(&self) -> &CacheConfig {
        &self.config
    }

    /// 读取缓存；miss 与缓存空值都返回 `None`。
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        match self.lookup(key).await? {
            Lookup::Hit(bytes) => Ok(Some(JsonCodec.decode(&bytes)?)),
            Lookup::Null | Lookup::Miss => Ok(None),
        }
    }

    /// 写入缓存（使用默认 TTL）。
    pub async fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.set_with_ttl(key, value, self.config.default_ttl).await
    }

    /// 写入缓存并指定 TTL。
    pub async fn set_with_ttl<T: Serialize>(
        &self,
        key: &str,
        value: &T,
        ttl: Duration,
    ) -> Result<()> {
        let bytes = JsonCodec.encode(value)?;
        self.store(key, &bytes, self.jitter(ttl)).await
    }

    /// 删除缓存。
    pub async fn delete(&self, key: &str) -> Result<()> {
        let namespaced = self.client.key_for(key);
        match self.client.del_bytes(&[namespaced]).await {
            Ok(_) => Ok(()),
            Err(error) if self.config.fail_open => {
                log::warn!("Redis 缓存删除失败（fail-open）: {error}");
                Ok(())
            }
            Err(error) => Err(BaseError::RedisOperationFailed(error)),
        }
    }

    /// Cache-aside：miss 时调用 loader 并回填。
    ///
    /// 启用 `single_flight` 时，同一 key 的并发 miss 只有一个请求回源。
    pub async fn get_or_load<T, F, Fut>(
        &self,
        key: &str,
        ttl: Option<Duration>,
        loader: F,
    ) -> Result<T>
    where
        T: Serialize + DeserializeOwned,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        if let Lookup::Hit(bytes) = self.lookup(key).await? {
            return Ok(JsonCodec.decode(&bytes)?);
        }

        let key_lock = self.key_lock(key);
        let guard = if self.config.single_flight {
            Some(key_lock.clone().lock_owned().await)
        } else {
            None
        };

        let result = async {
            // double-check：等待锁期间可能已有其他请求回填
            if let Lookup::Hit(bytes) = self.lookup(key).await? {
                return Ok(JsonCodec.decode(&bytes)?);
            }
            let value = loader().await?;
            let bytes = JsonCodec.encode(&value)?;
            self.store(
                key,
                &bytes,
                self.jitter(ttl.unwrap_or(self.config.default_ttl)),
            )
            .await?;
            Ok(value)
        }
        .await;

        drop(guard);
        result
    }

    /// Cache-aside（允许缓存空值）。
    ///
    /// loader 返回 `None` 时写入空值哨兵，避免穿透。
    pub async fn get_or_load_optional<T, F, Fut>(
        &self,
        key: &str,
        ttl: Option<Duration>,
        loader: F,
    ) -> Result<Option<T>>
    where
        T: Serialize + DeserializeOwned,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Option<T>>>,
    {
        match self.lookup(key).await? {
            Lookup::Hit(bytes) => return Ok(Some(JsonCodec.decode(&bytes)?)),
            Lookup::Null => return Ok(None),
            Lookup::Miss => {}
        }

        let key_lock = self.key_lock(key);
        let guard = if self.config.single_flight {
            Some(key_lock.clone().lock_owned().await)
        } else {
            None
        };

        let result = async {
            match self.lookup(key).await? {
                Lookup::Hit(bytes) => return Ok(Some(JsonCodec.decode(&bytes)?)),
                Lookup::Null => return Ok(None),
                Lookup::Miss => {}
            }
            let value = loader().await?;
            match &value {
                Some(value) => {
                    let bytes = JsonCodec.encode(value)?;
                    self.store(
                        key,
                        &bytes,
                        self.jitter(ttl.unwrap_or(self.config.default_ttl)),
                    )
                    .await?;
                }
                None => {
                    self.store(key, NULL_SENTINEL, self.config.null_ttl).await?;
                }
            }
            Ok(value)
        }
        .await;

        drop(guard);
        result
    }

    async fn lookup(&self, key: &str) -> Result<Lookup> {
        let namespaced = self.client.key_for(key);
        match self.client.get_bytes(namespaced).await {
            Ok(Some(bytes)) if bytes.as_slice() == NULL_SENTINEL => Ok(Lookup::Null),
            Ok(Some(bytes)) => Ok(Lookup::Hit(bytes)),
            Ok(None) => Ok(Lookup::Miss),
            Err(error) if self.config.fail_open => {
                log::warn!("Redis 缓存读取失败（fail-open，按 miss 处理）: {error}");
                Ok(Lookup::Miss)
            }
            Err(error) => Err(BaseError::RedisOperationFailed(error)),
        }
    }

    async fn store(&self, key: &str, bytes: &[u8], ttl: Duration) -> Result<()> {
        let namespaced = self.client.key_for(key);
        let ttl_seconds = i64::try_from(ttl.as_secs().max(1))
            .map_err(|_| BaseError::ConfigError("缓存 TTL 超出 i64 范围".to_string()))?;
        match self
            .client
            .setex_bytes(namespaced, ttl_seconds, bytes)
            .await
        {
            Ok(()) => Ok(()),
            Err(error) if self.config.fail_open => {
                log::warn!("Redis 缓存写入失败（fail-open）: {error}");
                Ok(())
            }
            Err(error) => Err(BaseError::RedisOperationFailed(error)),
        }
    }

    fn jitter(&self, ttl: Duration) -> Duration {
        jitter_ttl(ttl, self.config.ttl_jitter_percent)
    }

    fn key_lock(&self, key: &str) -> Arc<Mutex<()>> {
        let mut locks = self
            .key_locks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // 回收已经没有任何持有者的条目
        locks.retain(|_, weak| weak.strong_count() > 0);
        if let Some(lock) = locks.get(key).and_then(Weak::upgrade) {
            return lock;
        }
        let lock = Arc::new(Mutex::new(()));
        locks.insert(key.to_string(), Arc::downgrade(&lock));
        lock
    }
}

/// 按百分比抖动 TTL（0 表示不抖动）。
fn jitter_ttl(ttl: Duration, percent: u8) -> Duration {
    let percent = u64::from(percent.min(100));
    if percent == 0 {
        return ttl;
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::from(duration.subsec_nanos()))
        .unwrap_or(0);
    let extra_percent = nanos % (percent + 1);
    let extra_ms = (ttl.as_millis() as u64).saturating_mul(extra_percent) / 100;
    ttl + Duration::from_millis(extra_ms)
}

/// 固定窗口限流结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimitDecision {
    /// 是否放行。
    pub allowed: bool,
    /// 当前窗口内计数。
    pub current: i64,
    /// 配置的上限。
    pub limit: i64,
    /// 窗口剩余时间（毫秒）。
    pub reset_after_ms: i64,
}

const RATE_LIMIT_SCRIPT: &str = r#"
local current = redis.call('INCR', KEYS[1])
if current == 1 then
    redis.call('PEXPIRE', KEYS[1], ARGV[1])
end
return {current, redis.call('PTTL', KEYS[1])}
"#;

/// 通用固定窗口限流器。
#[derive(Debug, Clone)]
pub struct RateLimiter {
    client: RedisClient,
    prefix: String,
}

impl RateLimiter {
    /// 创建限流器。
    pub fn new(client: RedisClient, prefix: impl Into<String>) -> Self {
        Self {
            client,
            prefix: prefix.into(),
        }
    }

    /// 对 `key` 在 `window` 内做固定窗口计数，返回是否放行。
    pub async fn check(
        &self,
        key: &str,
        limit: i64,
        window: Duration,
    ) -> Result<RateLimitDecision> {
        if limit <= 0 {
            return Err(BaseError::ConfigError("限流 limit 必须大于 0".to_string()));
        }
        let window_ms = i64::try_from(window.as_millis())
            .map_err(|_| BaseError::ConfigError("限流窗口超出 i64 范围".to_string()))?;
        if window_ms <= 0 {
            return Err(BaseError::ConfigError(
                "限流窗口必须大于 0 毫秒".to_string(),
            ));
        }

        let cache_key = format!("{}:{}", self.prefix, key);
        let namespaced = match self.client.namespace() {
            Some(namespace) => namespace.key_str(&cache_key),
            None => cache_key,
        };
        let script = self.client.script(RATE_LIMIT_SCRIPT);
        let (current, ttl_ms): (i64, i64) = self
            .client
            .eval_script(&script, &[namespaced], &[window_ms.to_string()])
            .await
            .map_err(BaseError::RedisOperationFailed)?;

        Ok(RateLimitDecision {
            allowed: current <= limit,
            current,
            limit,
            reset_after_ms: ttl_ms.max(0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jitter_is_noop_when_percent_is_zero() {
        let ttl = Duration::from_secs(100);
        assert_eq!(jitter_ttl(ttl, 0), ttl);
    }

    #[test]
    fn jitter_stays_within_configured_percent() {
        let ttl = Duration::from_secs(100);
        for _ in 0..100 {
            let jittered = jitter_ttl(ttl, 10);
            assert!(jittered >= ttl);
            assert!(jittered <= ttl + Duration::from_secs(10));
        }
    }

    #[test]
    fn null_sentinel_is_not_valid_json() {
        assert!(serde_json::from_slice::<serde_json::Value>(NULL_SENTINEL).is_err());
    }
}
