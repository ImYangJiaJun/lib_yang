//! 可插拔的键值序列化。
//!
//! 基础库提供 [`JsonCodec`]；需要 MessagePack/Bincode 等格式时，调用方实现
//! [`RedisCodec`] 并通过 `set_typed` / `get_typed` 注入，不必改动客户端。

use crate::{DbError, RedisClient, Result};
use serde::{de::DeserializeOwned, Serialize};

/// Redis 值编解码器。
pub trait RedisCodec {
    /// 将值编码为字节。
    fn encode<T: Serialize>(&self, value: &T) -> Result<Vec<u8>>;
    /// 将字节解码为值。
    fn decode<T: DeserializeOwned>(&self, bytes: &[u8]) -> Result<T>;
}

/// JSON 编解码器。
#[derive(Debug, Default, Clone, Copy)]
pub struct JsonCodec;

impl RedisCodec for JsonCodec {
    fn encode<T: Serialize>(&self, value: &T) -> Result<Vec<u8>> {
        serde_json::to_vec(value)
            .map_err(|error| DbError::SerializationError(format!("Redis JSON 序列化失败: {error}")))
    }

    fn decode<T: DeserializeOwned>(&self, bytes: &[u8]) -> Result<T> {
        serde_json::from_slice(bytes).map_err(|error| {
            DbError::DeserializationError(format!("Redis JSON 反序列化失败: {error}"))
        })
    }
}

impl RedisClient {
    /// 使用任意 codec 写入字符串 key（可选 TTL 秒）。
    pub async fn set_typed<C, T>(
        &self,
        codec: &C,
        key: impl AsRef<[u8]>,
        value: &T,
        ttl_seconds: Option<i64>,
    ) -> Result<()>
    where
        C: RedisCodec,
        T: Serialize,
    {
        let bytes = codec.encode(value)?;
        let key = self.key_for(key);
        match ttl_seconds {
            Some(ttl) => self.setex_bytes(key, ttl, bytes).await,
            None => self.set_bytes(key, bytes).await,
        }
    }

    /// 使用任意 codec 读取并反序列化。
    pub async fn get_typed<C, T>(&self, codec: &C, key: impl AsRef<[u8]>) -> Result<Option<T>>
    where
        C: RedisCodec,
        T: DeserializeOwned,
    {
        match self.get_bytes(self.key_for(key)).await? {
            Some(bytes) => Ok(Some(codec.decode(&bytes)?)),
            None => Ok(None),
        }
    }

    /// 使用 JSON 写入字符串 key。
    pub async fn set_json<T: Serialize>(
        &self,
        key: impl AsRef<[u8]>,
        value: &T,
        ttl_seconds: Option<i64>,
    ) -> Result<()> {
        self.set_typed(&JsonCodec, key, value, ttl_seconds).await
    }

    /// 使用 JSON 读取字符串 key。
    pub async fn get_json<T: DeserializeOwned>(&self, key: impl AsRef<[u8]>) -> Result<Option<T>> {
        self.get_typed(&JsonCodec, key).await
    }

    /// 使用 JSON 写入哈希字段。
    pub async fn hset_json<T: Serialize>(
        &self,
        key: impl AsRef<[u8]>,
        field: impl AsRef<[u8]>,
        value: &T,
    ) -> Result<bool> {
        let bytes = JsonCodec.encode(value)?;
        self.hset_bytes(self.key_for(key), field, bytes).await
    }

    /// 使用 JSON 读取哈希字段。
    pub async fn hget_json<T: DeserializeOwned>(
        &self,
        key: impl AsRef<[u8]>,
        field: impl AsRef<[u8]>,
    ) -> Result<Option<T>> {
        match self.hget_bytes(self.key_for(key), field).await? {
            Some(bytes) => Ok(Some(JsonCodec.decode(&bytes)?)),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Payload {
        id: u64,
        name: String,
    }

    #[test]
    fn json_codec_round_trips() {
        let codec = JsonCodec;
        let payload = Payload {
            id: 7,
            name: "redis".to_string(),
        };
        let bytes = codec.encode(&payload).expect("编码应成功");
        let decoded: Payload = codec.decode(&bytes).expect("解码应成功");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn json_codec_reports_corrupt_payload() {
        let error = JsonCodec
            .decode::<Payload>(b"not-json")
            .expect_err("损坏 JSON 必须失败");
        assert!(matches!(error, DbError::DeserializationError(_)));
    }
}
