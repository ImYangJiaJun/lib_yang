//! Redis Pub/Sub 订阅端。
//!
//! 订阅使用不占用连接池的专用长连接；断线后按配置的退避策略重连并自动重订阅。

use crate::redis::backend::reconnect_delay;
use crate::{DbError, RedisClient, RedisValue, Result};
use futures_util::StreamExt;

/// 一条 Pub/Sub 消息。
#[derive(Clone, PartialEq, Eq)]
pub struct RedisMessage {
    /// 频道名
    pub channel: String,
    /// 消息载荷（二进制安全）
    pub payload: Vec<u8>,
    /// 命中的模式（PSUBSCRIBE 时存在）
    pub pattern: Option<String>,
}

impl std::fmt::Debug for RedisMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedisMessage")
            .field("channel", &self.channel)
            .field("pattern", &self.pattern)
            .field(
                "payload",
                &format!("<redacted {} bytes>", self.payload.len()),
            )
            .finish()
    }
}

/// Pub/Sub 订阅端。
pub struct RedisPubSub {
    client: redis::Client,
    pubsub: redis::aio::PubSub,
    channels: Vec<String>,
    patterns: Vec<String>,
    reconnect_retries: u32,
    reconnect_max_delay_ms: u64,
}

impl RedisClient {
    /// 建立 Pub/Sub 专用连接。
    ///
    /// 仅 standalone 拓扑支持；cluster/sentinel 会返回明确错误。
    pub async fn pubsub(&self) -> Result<RedisPubSub> {
        let client = self.direct_client()?.clone();
        let pubsub = client.get_async_pubsub().await.map_err(DbError::from)?;
        Ok(RedisPubSub {
            client,
            pubsub,
            channels: Vec::new(),
            patterns: Vec::new(),
            reconnect_retries: self.config().reconnect_retries,
            reconnect_max_delay_ms: self.config().reconnect_max_delay_ms,
        })
    }

    /// `PUBSUB CHANNELS [pattern]`。
    pub async fn pubsub_channels(&self, pattern: Option<&str>) -> Result<Vec<String>> {
        let mut cmd = redis::cmd("PUBSUB");
        cmd.arg("CHANNELS");
        if let Some(pattern) = pattern {
            cmd.arg(pattern);
        }
        parse_string_array(&self.execute(&cmd).await?)
    }

    /// `PUBSUB NUMSUB channel...`。
    pub async fn pubsub_numsub(&self, channels: &[String]) -> Result<Vec<(String, i64)>> {
        let mut cmd = redis::cmd("PUBSUB");
        cmd.arg("NUMSUB");
        for channel in channels {
            cmd.arg(channel);
        }
        let value = self.execute(&cmd).await?;
        let items = value.as_array().ok_or_else(|| {
            DbError::RedisTypeConversionError("PUBSUB NUMSUB 结果不是数组".to_string())
        })?;
        if items.len() % 2 != 0 {
            return Err(DbError::RedisTypeConversionError(
                "PUBSUB NUMSUB 返回数量不是偶数".to_string(),
            ));
        }
        items
            .chunks_exact(2)
            .map(|pair| {
                let channel = pair[0].as_string().ok_or_else(|| {
                    DbError::RedisTypeConversionError("PUBSUB NUMSUB 频道不是字符串".to_string())
                })?;
                let count = pair[1].as_i64().ok_or_else(|| {
                    DbError::RedisTypeConversionError("PUBSUB NUMSUB 计数不是整数".to_string())
                })?;
                Ok((channel, count))
            })
            .collect()
    }

    /// `PUBSUB NUMPAT`。
    pub async fn pubsub_numpat(&self) -> Result<i64> {
        let mut cmd = redis::cmd("PUBSUB");
        cmd.arg("NUMPAT");
        self.execute(&cmd)
            .await?
            .as_i64()
            .ok_or_else(|| DbError::RedisTypeConversionError("PUBSUB NUMPAT 不是整数".to_string()))
    }
}

impl RedisPubSub {
    /// 订阅一个或多个频道。
    pub async fn subscribe(&mut self, channels: &[String]) -> Result<()> {
        for channel in channels {
            self.pubsub
                .subscribe(channel.as_str())
                .await
                .map_err(DbError::from)?;
            if !self.channels.contains(channel) {
                self.channels.push(channel.clone());
            }
        }
        Ok(())
    }

    /// 按模式订阅。
    pub async fn psubscribe(&mut self, patterns: &[String]) -> Result<()> {
        for pattern in patterns {
            self.pubsub
                .psubscribe(pattern.as_str())
                .await
                .map_err(DbError::from)?;
            if !self.patterns.contains(pattern) {
                self.patterns.push(pattern.clone());
            }
        }
        Ok(())
    }

    /// 取消订阅。
    pub async fn unsubscribe(&mut self, channels: &[String]) -> Result<()> {
        for channel in channels {
            self.pubsub
                .unsubscribe(channel.as_str())
                .await
                .map_err(DbError::from)?;
            self.channels.retain(|item| item != channel);
        }
        Ok(())
    }

    /// 取消模式订阅。
    pub async fn punsubscribe(&mut self, patterns: &[String]) -> Result<()> {
        for pattern in patterns {
            self.pubsub
                .punsubscribe(pattern.as_str())
                .await
                .map_err(DbError::from)?;
            self.patterns.retain(|item| item != pattern);
        }
        Ok(())
    }

    /// 读取下一条消息；连接断开时自动重连并重订阅。
    ///
    /// 返回 `Ok(None)` 仅在重连也被取消/流永久结束时出现；正常断线会自动重试。
    pub async fn next_message(&mut self) -> Result<Option<RedisMessage>> {
        loop {
            let message = {
                let mut stream = self.pubsub.on_message();
                stream.next().await
            };
            match message {
                Some(message) => {
                    return Ok(Some(RedisMessage {
                        channel: message.get_channel_name().to_string(),
                        payload: message.get_payload_bytes().to_vec(),
                        pattern: message.get_pattern::<String>().ok(),
                    }));
                }
                None => {
                    log::warn!("Redis Pub/Sub 连接已断开，准备重连并重订阅");
                    self.reconnect().await?;
                }
            }
        }
    }

    async fn reconnect(&mut self) -> Result<()> {
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            match self.client.get_async_pubsub().await {
                Ok(mut pubsub) => {
                    for channel in &self.channels {
                        pubsub
                            .subscribe(channel.as_str())
                            .await
                            .map_err(DbError::from)?;
                    }
                    for pattern in &self.patterns {
                        pubsub
                            .psubscribe(pattern.as_str())
                            .await
                            .map_err(DbError::from)?;
                    }
                    self.pubsub = pubsub;
                    return Ok(());
                }
                Err(error) if attempt <= self.reconnect_retries => {
                    let delay = reconnect_delay(attempt, self.reconnect_max_delay_ms);
                    log::warn!(
                        "Redis Pub/Sub 重连失败，第 {}/{} 次: {}",
                        attempt,
                        self.reconnect_retries,
                        error
                    );
                    tokio::time::sleep(delay).await;
                }
                Err(error) => return Err(DbError::from(error)),
            }
        }
    }
}

fn parse_string_array(value: &RedisValue) -> Result<Vec<String>> {
    let items = value
        .as_array()
        .ok_or_else(|| DbError::RedisTypeConversionError("Redis 结果不是数组".to_string()))?;
    items
        .iter()
        .map(|item| {
            item.as_string().ok_or_else(|| {
                DbError::RedisTypeConversionError("Redis 数组包含非字符串元素".to_string())
            })
        })
        .collect()
}
