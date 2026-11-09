//! 二进制安全 API。
//!
//! 既有字符串 API 会把非 UTF-8 值视为类型错误；本模块提供不会丢字节的
//! 一等公民入口，适用于序列化数据、压缩数据、摘要、二进制 key 等场景。

use crate::{DbError, RedisClient, RedisValue, Result};

fn value_to_bytes(value: &RedisValue) -> Result<Option<Vec<u8>>> {
    match value {
        RedisValue::Nil => Ok(None),
        _ => value.as_bytes_owned().map(Some).ok_or_else(|| {
            DbError::RedisTypeConversionError(
                "Redis 值无法表示为字节（期望字符串/字节类型）".to_string(),
            )
        }),
    }
}

fn array_to_bytes(value: &RedisValue) -> Result<Vec<Vec<u8>>> {
    let items = value
        .as_array()
        .ok_or_else(|| DbError::RedisTypeConversionError("Redis 结果不是数组".to_string()))?;
    items
        .iter()
        .map(|item| {
            value_to_bytes(item)?.ok_or_else(|| {
                DbError::RedisTypeConversionError(
                    "Redis 数组包含 nil，不能转换为字节列表".to_string(),
                )
            })
        })
        .collect()
}

impl RedisClient {
    /// GET（二进制安全）。
    pub async fn get_bytes(&self, key: impl AsRef<[u8]>) -> Result<Option<Vec<u8>>> {
        let mut cmd = redis::cmd("GET");
        cmd.arg(key.as_ref());
        value_to_bytes(&self.execute(&cmd).await?)
    }

    /// SET（二进制安全）。
    pub async fn set_bytes(&self, key: impl AsRef<[u8]>, value: impl AsRef<[u8]>) -> Result<()> {
        let mut cmd = redis::cmd("SET");
        cmd.arg(key.as_ref()).arg(value.as_ref());
        self.execute(&cmd).await?;
        Ok(())
    }

    /// SETEX（二进制安全）。
    pub async fn setex_bytes(
        &self,
        key: impl AsRef<[u8]>,
        seconds: i64,
        value: impl AsRef<[u8]>,
    ) -> Result<()> {
        let mut cmd = redis::cmd("SETEX");
        cmd.arg(key.as_ref()).arg(seconds).arg(value.as_ref());
        self.execute(&cmd).await?;
        Ok(())
    }

    /// `SET key value NX PX ttl_ms`：原子获取锁/一次性占位。
    pub async fn set_nx_px(
        &self,
        key: impl AsRef<[u8]>,
        value: impl AsRef<[u8]>,
        ttl_ms: u64,
    ) -> Result<bool> {
        if ttl_ms == 0 {
            return Err(DbError::InvalidArgument(
                "set_nx_px 的 ttl_ms 必须大于 0".to_string(),
            ));
        }
        let mut cmd = redis::cmd("SET");
        cmd.arg(key.as_ref())
            .arg(value.as_ref())
            .arg("NX")
            .arg("PX")
            .arg(ttl_ms);
        Ok(!self.execute(&cmd).await?.is_nil())
    }

    /// MGET（二进制安全）；缺失键返回 `None`。
    pub async fn mget_bytes<K: AsRef<[u8]>>(&self, keys: &[K]) -> Result<Vec<Option<Vec<u8>>>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let mut cmd = redis::cmd("MGET");
        for key in keys {
            cmd.arg(key.as_ref());
        }
        let value = self.execute(&cmd).await?;
        let items = value
            .as_array()
            .ok_or_else(|| DbError::RedisTypeConversionError("MGET 结果不是数组".to_string()))?;
        items.iter().map(value_to_bytes).collect()
    }

    /// HSET（二进制安全）。
    pub async fn hset_bytes(
        &self,
        key: impl AsRef<[u8]>,
        field: impl AsRef<[u8]>,
        value: impl AsRef<[u8]>,
    ) -> Result<bool> {
        let mut cmd = redis::cmd("HSET");
        cmd.arg(key.as_ref())
            .arg(field.as_ref())
            .arg(value.as_ref());
        Ok(self.execute(&cmd).await?.as_i64() == Some(1))
    }

    /// HGET（二进制安全）。
    pub async fn hget_bytes(
        &self,
        key: impl AsRef<[u8]>,
        field: impl AsRef<[u8]>,
    ) -> Result<Option<Vec<u8>>> {
        let mut cmd = redis::cmd("HGET");
        cmd.arg(key.as_ref()).arg(field.as_ref());
        value_to_bytes(&self.execute(&cmd).await?)
    }

    /// HMGET（二进制安全）。
    pub async fn hmget_bytes<K: AsRef<[u8]>, F: AsRef<[u8]>>(
        &self,
        key: K,
        fields: &[F],
    ) -> Result<Vec<Option<Vec<u8>>>> {
        if fields.is_empty() {
            return Ok(Vec::new());
        }

        let mut cmd = redis::cmd("HMGET");
        cmd.arg(key.as_ref());
        for field in fields {
            cmd.arg(field.as_ref());
        }
        let value = self.execute(&cmd).await?;
        let items = value
            .as_array()
            .ok_or_else(|| DbError::RedisTypeConversionError("HMGET 结果不是数组".to_string()))?;
        items.iter().map(value_to_bytes).collect()
    }

    /// HGETALL（二进制安全）。
    pub async fn hgetall_bytes(&self, key: impl AsRef<[u8]>) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let mut cmd = redis::cmd("HGETALL");
        cmd.arg(key.as_ref());
        let value = self.execute(&cmd).await?;
        let items = value
            .as_array()
            .ok_or_else(|| DbError::RedisTypeConversionError("HGETALL 结果不是数组".to_string()))?;
        if items.len() % 2 != 0 {
            return Err(DbError::RedisTypeConversionError(
                "HGETALL 返回的字段/值数量不是偶数".to_string(),
            ));
        }
        items
            .chunks_exact(2)
            .map(|pair| {
                let field = value_to_bytes(&pair[0])?.ok_or_else(|| {
                    DbError::RedisTypeConversionError("HGETALL 字段为 nil".to_string())
                })?;
                let value = value_to_bytes(&pair[1])?.ok_or_else(|| {
                    DbError::RedisTypeConversionError("HGETALL 值为 nil".to_string())
                })?;
                Ok((field, value))
            })
            .collect()
    }

    /// LPUSH（二进制安全）。
    pub async fn lpush_bytes<V: AsRef<[u8]>>(
        &self,
        key: impl AsRef<[u8]>,
        values: &[V],
    ) -> Result<i64> {
        if values.is_empty() {
            return Ok(0);
        }

        let mut cmd = redis::cmd("LPUSH");
        cmd.arg(key.as_ref());
        for value in values {
            cmd.arg(value.as_ref());
        }
        Ok(self.execute(&cmd).await?.as_i64().unwrap_or(0))
    }

    /// LRANGE（二进制安全）。
    pub async fn lrange_bytes(
        &self,
        key: impl AsRef<[u8]>,
        start: i64,
        stop: i64,
    ) -> Result<Vec<Vec<u8>>> {
        let mut cmd = redis::cmd("LRANGE");
        cmd.arg(key.as_ref()).arg(start).arg(stop);
        array_to_bytes(&self.execute(&cmd).await?)
    }

    /// SADD（二进制安全）。
    pub async fn sadd_bytes<M: AsRef<[u8]>>(
        &self,
        key: impl AsRef<[u8]>,
        members: &[M],
    ) -> Result<i64> {
        if members.is_empty() {
            return Ok(0);
        }

        let mut cmd = redis::cmd("SADD");
        cmd.arg(key.as_ref());
        for member in members {
            cmd.arg(member.as_ref());
        }
        Ok(self.execute(&cmd).await?.as_i64().unwrap_or(0))
    }

    /// SMEMBERS（二进制安全）。
    pub async fn smembers_bytes(&self, key: impl AsRef<[u8]>) -> Result<Vec<Vec<u8>>> {
        let mut cmd = redis::cmd("SMEMBERS");
        cmd.arg(key.as_ref());
        array_to_bytes(&self.execute(&cmd).await?)
    }

    /// DEL（二进制安全）。
    pub async fn del_bytes<K: AsRef<[u8]>>(&self, keys: &[K]) -> Result<i64> {
        if keys.is_empty() {
            return Ok(0);
        }
        let mut cmd = redis::cmd("DEL");
        for key in keys {
            cmd.arg(key.as_ref());
        }
        Ok(self.execute(&cmd).await?.as_i64().unwrap_or(0))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn value_to_bytes_accepts_utf8_string_and_raw_bytes() {
        assert_eq!(
            value_to_bytes(&RedisValue::String("abc".to_string())).unwrap(),
            Some(b"abc".to_vec())
        );
        assert_eq!(
            value_to_bytes(&RedisValue::Bytes(vec![0xFF, 0x00])).unwrap(),
            Some(vec![0xFF, 0x00])
        );
        assert_eq!(value_to_bytes(&RedisValue::Nil).unwrap(), None);
        assert!(value_to_bytes(&RedisValue::Int(1)).is_err());
    }

    #[test]
    fn array_to_bytes_rejects_nil_items() {
        let value = RedisValue::Array(vec![RedisValue::Bytes(vec![1]), RedisValue::Nil]);
        assert!(array_to_bytes(&value).is_err());
    }
}
