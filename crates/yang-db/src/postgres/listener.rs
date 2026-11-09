//! PostgreSQL LISTEN / NOTIFY 封装。
//!
//! [`PgNotificationListener`] 持有独立连接（sqlx `PgListener` 内部为单连接池，
//! 不占用业务连接池的连接配额）：[`listen`](PgNotificationListener::listen) 注册
//! 通道，[`recv`](PgNotificationListener::recv) 等待异步通知，
//! [`reconnect`](PgNotificationListener::reconnect) 显式重建连接并重订阅全部通道。
//!
//! 发送侧使用 [`Database::notify`]，底层为参数化的 `pg_notify($1, $2)`，
//! 不拼接 SQL 文本。通知遵循 PostgreSQL 语义：事务内发送的通知在事务提交后
//! 才投递，回滚则不投递。

use crate::error::DbError;
use crate::postgres::Database;
use sqlx::postgres::PgListener;

/// 通道名最大字节数（`NAMEDATALEN - 1`）。
const MAX_CHANNEL_BYTES: usize = 63;
/// NOTIFY 载荷字节数上限；服务端要求载荷严格小于 8000 字节。
const MAX_PAYLOAD_BYTES: usize = 8000;

/// 校验通道名：非空、无 NUL、不超过 PostgreSQL 标识符长度上限。
fn validate_channel(channel: &str) -> Result<(), DbError> {
    if channel.is_empty() {
        return Err(DbError::InvalidArgument(
            "PostgreSQL 通知通道名不能为空".to_string(),
        ));
    }
    if channel.contains('\0') {
        return Err(DbError::InvalidArgument(
            "PostgreSQL 通知通道名不能包含 NUL 字符".to_string(),
        ));
    }
    if channel.len() > MAX_CHANNEL_BYTES {
        return Err(DbError::InvalidArgument(format!(
            "PostgreSQL 通知通道名不能超过 {MAX_CHANNEL_BYTES} 字节，当前 {} 字节",
            channel.len()
        )));
    }
    Ok(())
}

/// 校验 NOTIFY 载荷长度。
fn validate_payload(payload: &str) -> Result<(), DbError> {
    if payload.len() >= MAX_PAYLOAD_BYTES {
        return Err(DbError::InvalidArgument(format!(
            "PostgreSQL NOTIFY 载荷必须小于 {MAX_PAYLOAD_BYTES} 字节，当前 {} 字节",
            payload.len()
        )));
    }
    Ok(())
}

/// 一条 PostgreSQL 异步通知。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    process_id: u32,
    channel: String,
    payload: String,
}

impl Notification {
    /// 发送通知的服务端后端进程 ID。
    pub fn process_id(&self) -> u32 {
        self.process_id
    }

    /// 通知通道。
    pub fn channel(&self) -> &str {
        &self.channel
    }

    /// 通知载荷；空载荷为空字符串。
    pub fn payload(&self) -> &str {
        &self.payload
    }
}

/// 常驻 LISTEN 连接。
///
/// 该类型持有自己的连接，不从 [`Database`] 的连接池借出；监听期间不会占用
/// 业务查询的连接配额。
#[derive(Debug)]
pub struct PgNotificationListener {
    url: String,
    listener: PgListener,
    channels: Vec<String>,
}

impl PgNotificationListener {
    /// 建立监听连接。`url` 为 PostgreSQL 连接串，格式同 [`Database::connect`]。
    pub async fn connect(url: impl Into<String>) -> Result<Self, DbError> {
        let url = url.into();
        let listener = PgListener::connect(&url).await?;
        Ok(Self {
            url,
            listener,
            channels: Vec::new(),
        })
    }

    /// 注册一组通道；重复通道自动忽略，已注册的通道不会重复订阅。
    ///
    /// 通道名在发送 `LISTEN` 时按双引号标识符转义，因此允许大小写与常见
    /// 特殊字符；空名、含 NUL 或超过 63 字节的通道名返回
    /// [`DbError::InvalidArgument`]。
    pub async fn listen<I, S>(&mut self, channels: I) -> Result<(), DbError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut new_channels: Vec<String> = Vec::new();
        for channel in channels {
            let channel = channel.as_ref();
            validate_channel(channel)?;
            let known = self.channels.iter().any(|existing| existing == channel)
                || new_channels.iter().any(|existing| existing == channel);
            if !known {
                new_channels.push(channel.to_string());
            }
        }
        if new_channels.is_empty() {
            return Ok(());
        }
        self.listener
            .listen_all(new_channels.iter().map(String::as_str))
            .await?;
        self.channels.extend(new_channels);
        Ok(())
    }

    /// 等待下一条通知。连接意外断开时由底层监听器自动重连并重订阅。
    pub async fn recv(&mut self) -> Result<Notification, DbError> {
        let notification = self.listener.recv().await?;
        Ok(Notification {
            process_id: notification.process_id(),
            channel: notification.channel().to_string(),
            payload: notification.payload().to_string(),
        })
    }

    /// 显式重建连接并重订阅当前已注册的全部通道。
    ///
    /// 用于调用方主动轮换连接（如排障后强制刷新）。重建失败时保留原连接与
    /// 订阅状态。断线期间的通知是瞬态的，不会补发。
    pub async fn reconnect(&mut self) -> Result<(), DbError> {
        let mut listener = PgListener::connect(&self.url).await?;
        if !self.channels.is_empty() {
            listener
                .listen_all(self.channels.iter().map(String::as_str))
                .await?;
        }
        self.listener = listener;
        Ok(())
    }

    /// 当前已注册的通道列表。
    pub fn channels(&self) -> &[String] {
        &self.channels
    }

    /// 监听连接的服务端后端进程 ID。
    ///
    /// 排障用途（如从另一条连接 `pg_terminate_backend` 模拟断线）。
    pub async fn backend_pid(&mut self) -> Result<i32, DbError> {
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut self.listener)
            .await?;
        Ok(pid)
    }
}

impl Database {
    /// 向指定通道发送通知（`pg_notify`）。
    ///
    /// 参数通过 `$1` / `$2` 绑定，不拼接 SQL。事务内调用时通知在事务提交后
    /// 才投递；通道名与载荷长度在本地先做校验，非法输入返回
    /// [`DbError::InvalidArgument`]。
    pub async fn notify(&self, channel: &str, payload: &str) -> Result<(), DbError> {
        validate_channel(channel)?;
        validate_payload(payload)?;
        sqlx::query("SELECT pg_notify($1, $2)")
            .bind(channel)
            .bind(payload)
            .execute(self.pool())
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_validation_enforces_length_and_rejects_nul() {
        assert!(validate_channel("orders").is_ok());
        assert!(validate_channel("Order.Events").is_ok());
        assert!(validate_channel(&"a".repeat(MAX_CHANNEL_BYTES)).is_ok());

        for invalid in ["", "a\0b"] {
            assert!(
                matches!(validate_channel(invalid), Err(DbError::InvalidArgument(_))),
                "通道名应被拒绝: {invalid:?}"
            );
        }
        assert!(matches!(
            validate_channel(&"a".repeat(MAX_CHANNEL_BYTES + 1)),
            Err(DbError::InvalidArgument(_))
        ));
    }

    #[test]
    fn payload_validation_enforces_pg_limit() {
        assert!(validate_payload("").is_ok());
        assert!(validate_payload(&"x".repeat(MAX_PAYLOAD_BYTES - 1)).is_ok());
        assert!(matches!(
            validate_payload(&"x".repeat(MAX_PAYLOAD_BYTES)),
            Err(DbError::InvalidArgument(_))
        ));
    }
}
