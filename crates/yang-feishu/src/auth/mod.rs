//! 凭证、令牌缓存与 OAuth 协议。

pub mod oauth;
#[cfg(feature = "redis")]
pub mod redis;
pub mod state;
pub mod token;

pub use oauth::{
    access_token_is_fresh, build_authorize_url, now_unix_secs, reauth_reminder_due, OAuthClient,
    TokenBundle, TokenEndpointError, REAUTH_REMINDER_DAYS,
};
pub use state::{InMemoryStateStore, StateStore, STATE_TTL_SECONDS};
pub use token::{
    cache_ttl_seconds, parse_token_grant, MemoryTokenCache, TenantTokenGrant, TokenCache,
    TokenProvider, LOCK_TTL_SECONDS,
};

/// 一个秘密字符串；`Debug` 恒打码。
#[derive(Clone, PartialEq, Eq)]
pub struct SecretString(String);

impl SecretString {
    /// 包装一个明文秘密。
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// 取出明文。调用方负责不把它写进日志。
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for SecretString {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("\"<已隐藏>\"")
    }
}

/// 自建应用凭证。一实例 = 一个自建应用；多应用由调用方建多个客户端。
#[derive(Clone, PartialEq, Eq)]
pub struct Credentials {
    /// 应用 app_id。
    pub app_id: String,
    /// 应用 app_secret。
    pub app_secret: SecretString,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Credentials")
            .field("app_id", &self.app_id)
            .field("app_secret", &"<已隐藏>")
            .finish()
    }
}

/// 凭证来源。宿主可注入动态读取（配置热更新、密钥托管）。
pub trait CredentialsProvider: Send + Sync {
    /// 返回当前凭证。
    fn credentials(&self) -> &Credentials;
}

/// 固定凭证。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticCredentials {
    credentials: Credentials,
}

impl StaticCredentials {
    /// 由一组固定凭证构造。
    pub fn new(credentials: Credentials) -> Self {
        Self { credentials }
    }
}

impl CredentialsProvider for StaticCredentials {
    fn credentials(&self) -> &Credentials {
        &self.credentials
    }
}
