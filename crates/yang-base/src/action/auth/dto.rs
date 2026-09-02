//! 认证内置 Action 的输入 / 输出 DTO 与 Token 声明载体。

use serde::{Deserialize, Serialize};

// ──────────────────────────────────────────────────────────────────────────────
// 输入 / 输出 DTO
// ──────────────────────────────────────────────────────────────────────────────

/// 登录输入：凭证字段。
///
/// 注意：字段固定，与具体 [`CredentialVerifier`](super::CredentialVerifier) 实现无关——`LoginAction<V>` 是
/// 泛型，而 `#[derive(Action)]` 生成的 schema 静态量在各实例化间共享，故 Input
/// 不能依赖 `V`。如需额外字段，可在 `extra` 中携带任意 JSON。
#[derive(Clone, Deserialize, schemars::JsonSchema)]
pub struct LoginInput {
    /// 用户名 / 账号 / 邮箱等登录标识
    pub username: String,
    /// 密码 / 凭据
    pub password: String,
    /// 额外的业务自定义字段（可选）
    #[serde(default)]
    pub extra: serde_json::Value,
}

// NEW-38: 手写 Debug 脱敏，防止密码明文泄漏到日志/tracing
impl core::fmt::Debug for LoginInput {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("LoginInput")
            .field("username", &self.username)
            .field("password", &"***")
            .field("extra", &self.extra)
            .finish()
    }
}

/// Token 对响应。
#[derive(Clone, Serialize, schemars::JsonSchema)]
pub struct TokenPairResponse {
    /// Access Token
    pub access_token: String,
    /// Refresh Token
    pub refresh_token: String,
}

// NEW-38: 手写 Debug 脱敏，防止 Token 明文泄漏到日志/tracing
impl core::fmt::Debug for TokenPairResponse {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TokenPairResponse")
            .field(
                "access_token",
                &format!("***({} chars)", self.access_token.len()),
            )
            .field(
                "refresh_token",
                &format!("***({} chars)", self.refresh_token.len()),
            )
            .finish()
    }
}

/// 刷新输入。
#[derive(Clone, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RefreshInput {
    /// Refresh Token
    pub refresh_token: String,
}

// NEW-38: 手写 Debug 脱敏
impl core::fmt::Debug for RefreshInput {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RefreshInput")
            .field(
                "refresh_token",
                &format!("***({} chars)", self.refresh_token.len()),
            )
            .finish()
    }
}

/// 刷新响应（仅新的 Access Token）。
#[derive(Clone, Serialize, schemars::JsonSchema)]
pub struct AccessTokenResponse {
    /// 新的 Access Token
    pub access_token: String,
}

// NEW-38: 手写 Debug 脱敏
impl core::fmt::Debug for AccessTokenResponse {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AccessTokenResponse")
            .field(
                "access_token",
                &format!("***({} chars)", self.access_token.len()),
            )
            .finish()
    }
}

/// 登出输入。
///
/// 终止会话时建议**同时**传入 Access Token 与 Refresh Token：仅撤销 Access Token
/// 会让攻击者仍能用未失效的 Refresh Token 刷出新的 Access Token，会话并未真正结束。
#[derive(Clone, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LogoutInput {
    /// 待撤销的 Token（通常是 Access Token）
    pub token: String,
    /// 待撤销的 Refresh Token（可选）
    ///
    /// 若提供，将与 `token` 一并写入黑名单，从而彻底终止整个会话；
    /// 不提供时仅撤销 `token`。
    #[serde(default)]
    pub refresh_token: Option<String>,
}

// NEW-38: 手写 Debug 脱敏
impl core::fmt::Debug for LogoutInput {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("LogoutInput")
            .field("token", &format!("***({} chars)", self.token.len()))
            .field(
                "refresh_token",
                &self
                    .refresh_token
                    .as_ref()
                    .map(|s| format!("***({} chars)", s.len())),
            )
            .finish()
    }
}

/// 通用成功消息响应。
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct MessageResponse {
    /// 结果消息
    pub message: String,
}

/// 同一授权快照派生出的 Access/Refresh Token 自定义声明。
///
/// Access Token 可携带角色、权限等完整授权信息；Refresh Token 应只保留刷新流程
/// 所需的最小稳定声明。默认不为 Refresh Token 附加声明，保持既有行为。
#[derive(Debug, Clone)]
pub struct TokenPairClaims {
    /// 写入 Access Token 的自定义声明。
    pub access: serde_json::Value,
    /// 写入 Refresh Token 的自定义声明。
    pub refresh: serde_json::Value,
}

impl TokenPairClaims {
    /// 创建仅含 Access Token 自定义声明的声明对。
    pub fn new(access: serde_json::Value) -> Self {
        Self {
            access,
            refresh: serde_json::Value::Null,
        }
    }

    /// 设置 Refresh Token 的最小自定义声明（链式）。
    pub fn with_refresh(mut self, refresh: serde_json::Value) -> Self {
        self.refresh = refresh;
        self
    }
}

/// 校验通过后的主体信息，由 [`CredentialVerifier`](super::CredentialVerifier) 返回。
#[derive(Debug, Clone)]
pub struct VerifiedSubject {
    /// Token 主题（通常是用户 ID，写入 JWT `sub`）
    pub subject: String,
    /// 写入 Access Token 的自定义声明（如角色、权限）
    pub custom_claims: serde_json::Value,
    /// 写入 Refresh Token 的最小自定义声明
    pub refresh_claims: serde_json::Value,
}

impl VerifiedSubject {
    /// 用主题创建，无自定义声明。
    pub fn new(subject: impl Into<String>) -> Self {
        Self {
            subject: subject.into(),
            custom_claims: serde_json::Value::Null,
            refresh_claims: serde_json::Value::Null,
        }
    }

    /// 设置自定义声明（链式）。
    pub fn with_claims(mut self, claims: serde_json::Value) -> Self {
        self.custom_claims = claims;
        self
    }

    /// 设置 Refresh Token 的最小自定义声明（链式）。
    pub fn with_refresh_claims(mut self, claims: serde_json::Value) -> Self {
        self.refresh_claims = claims;
        self
    }

    /// 设置同一授权快照派生出的 Access/Refresh Token 声明（链式）。
    pub fn with_token_pair_claims(mut self, claims: TokenPairClaims) -> Self {
        self.custom_claims = claims.access;
        self.refresh_claims = claims.refresh;
        self
    }
}
