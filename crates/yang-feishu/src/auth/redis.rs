//! 基于 Redis 的独立令牌缓存（feature `redis`）。
//!
//! 宿主已有 `yang_db::RedisClient` 时**不要**用本实现——宿主应实现
//! [`TokenCache`] 复用既有连接池，避免第二套 Redis 连接。

use async_trait::async_trait;
use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;

use crate::error::{Error, Result};

use super::token::TokenCache;

/// 「比较并删除」放锁：裸 DEL 会删掉别人 TTL 过期后抢到的锁。
const RELEASE_LOCK_SCRIPT: &str = r"
if redis.call('GET', KEYS[1]) == ARGV[1] then
    return redis.call('DEL', KEYS[1])
end
return 0
";

/// Redis 令牌缓存。
#[derive(Clone)]
pub struct RedisTokenCache {
    connection: MultiplexedConnection,
    key_prefix: String,
}

impl std::fmt::Debug for RedisTokenCache {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RedisTokenCache")
            .field("key_prefix", &self.key_prefix)
            .finish_non_exhaustive()
    }
}

impl RedisTokenCache {
    /// 用现有连接构造；`key_prefix` 为空时不加前缀。
    pub fn new(connection: MultiplexedConnection, key_prefix: impl Into<String>) -> Self {
        Self {
            connection,
            key_prefix: key_prefix.into(),
        }
    }

    /// 按 URL 建立连接（独立使用者）。
    pub async fn connect(url: &str, key_prefix: impl Into<String>) -> Result<Self> {
        let client = redis::Client::open(url)
            .map_err(|error| Error::fatal(format!("打开 Redis 连接失败: {error}")))?;
        let connection = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|error| Error::retry(format!("连接 Redis 失败: {error}")))?;
        Ok(Self::new(connection, key_prefix))
    }

    fn key(&self, key: &str) -> String {
        if self.key_prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}:{}", self.key_prefix, key)
        }
    }
}

fn cache_error(action: &str, error: redis::RedisError) -> Error {
    Error::retry(format!("{action}失败: {error}"))
}

#[async_trait]
impl TokenCache for RedisTokenCache {
    async fn get(&self, key: &str) -> Result<Option<String>> {
        let mut connection = self.connection.clone();
        connection
            .get(self.key(key))
            .await
            .map_err(|error| cache_error("读取 token 缓存", error))
    }

    async fn put(&self, key: &str, value: &str, ttl_seconds: i64) -> Result<()> {
        if ttl_seconds <= 0 {
            return Err(Error::fatal("tenant_access_token 缓存 TTL 必须为正数"));
        }
        let mut connection = self.connection.clone();
        let _: () = connection
            .set_ex(self.key(key), value, ttl_seconds as u64)
            .await
            .map_err(|error| cache_error("写入 token 缓存", error))?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let mut connection = self.connection.clone();
        let _: () = connection
            .del(self.key(key))
            .await
            .map_err(|error| cache_error("清除 token 缓存", error))?;
        Ok(())
    }

    async fn acquire_lock(&self, key: &str, owner: &str, ttl_seconds: i64) -> Result<bool> {
        if ttl_seconds <= 0 {
            return Err(Error::fatal("tenant_access_token 锁 TTL 必须为正数"));
        }
        let mut connection = self.connection.clone();
        let acquired: Option<String> = redis::cmd("SET")
            .arg(self.key(key))
            .arg(owner)
            .arg("NX")
            .arg("EX")
            .arg(ttl_seconds)
            .query_async(&mut connection)
            .await
            .map_err(|error| cache_error("抢 token 刷新锁", error))?;
        Ok(acquired.is_some())
    }

    async fn release_lock(&self, key: &str, owner: &str) -> Result<()> {
        let mut connection = self.connection.clone();
        let script = redis::Script::new(RELEASE_LOCK_SCRIPT);
        let _: i64 = script
            .key(self.key(key))
            .arg(owner)
            .invoke_async(&mut connection)
            .await
            .map_err(|error| cache_error("释放 token 刷新锁", error))?;
        Ok(())
    }
}
