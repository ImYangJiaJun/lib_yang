//! 认证相关内置 Action（L-2）
//!
//! 每个使用 yang-base 的项目都要重复实现 JWT 登录/刷新/登出。本模块提供
//! 可选的认证内置 Action（feature `token`）：
//!
//! - [`LoginAction<V>`]：验证凭证（由业务实现的 [`CredentialVerifier`]）后签发 Token 对。
//! - [`RefreshAction`]：用 Refresh Token 换取新的 Access Token。
//! - [`LogoutAction`]：撤销 Token（写入 Redis 黑名单，见 [`crate::token`] 撤销机制）。
//!
//! # 凭证验证是业务相关的
//!
//! 登录如何校验用户名/密码因项目而异，无法做成完全通用。因此 [`LoginAction<V>`]
//! 把校验委托给业务实现的 [`CredentialVerifier`] trait，自身只负责"校验通过 -> 签发 Token"。
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::action::auth::{CredentialVerifier, LoginAction, LoginInput, VerifiedSubject};
//! use yang_base::action::ActionContext;
//! use yang_base::error::BaseError;
//! use async_trait::async_trait;
//!
//! struct MyVerifier;
//!
//! #[async_trait]
//! impl CredentialVerifier for MyVerifier {
//!     async fn verify(&self, _ctx: &ActionContext, input: &LoginInput)
//!         -> Result<VerifiedSubject, BaseError>
//!     {
//!         // 查库校验 input.username / input.password ...
//!         Ok(VerifiedSubject::new(format!("user:{}", input.username)))
//!     }
//! }
//!
//! let login = LoginAction::new(MyVerifier);
//! ```

use crate::action::{ActionContext, TypedHandler};
use crate::error::BaseError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use yang_base_derive::Action;

// ──────────────────────────────────────────────────────────────────────────────
// 输入 / 输出 DTO
// ──────────────────────────────────────────────────────────────────────────────

/// 登录输入：凭证字段。
///
/// 注意：字段固定，与具体 [`CredentialVerifier`] 实现无关——`LoginAction<V>` 是
/// 泛型，而 `#[derive(Action)]` 生成的 schema 静态量在各实例化间共享，故 Input
/// 不能依赖 `V`。如需额外字段，可在 `extra` 中携带任意 JSON。
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct LoginInput {
    /// 用户名 / 账号 / 邮箱等登录标识
    pub username: String,
    /// 密码 / 凭据
    pub password: String,
    /// 额外的业务自定义字段（可选）
    #[serde(default)]
    pub extra: serde_json::Value,
}

/// Token 对响应。
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct TokenPairResponse {
    /// Access Token
    pub access_token: String,
    /// Refresh Token
    pub refresh_token: String,
}

/// 刷新输入。
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct RefreshInput {
    /// Refresh Token
    pub refresh_token: String,
}

/// 刷新响应（仅新的 Access Token）。
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct AccessTokenResponse {
    /// 新的 Access Token
    pub access_token: String,
}

/// 登出输入。
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct LogoutInput {
    /// 待撤销的 Token（通常是 Access Token）
    pub token: String,
}

/// 通用成功消息响应。
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct MessageResponse {
    /// 结果消息
    pub message: String,
}

/// 校验通过后的主体信息，由 [`CredentialVerifier`] 返回。
#[derive(Debug, Clone)]
pub struct VerifiedSubject {
    /// Token 主题（通常是用户 ID，写入 JWT `sub`）
    pub subject: String,
    /// 写入 Access Token 的自定义声明（如角色、权限）
    pub custom_claims: serde_json::Value,
}

impl VerifiedSubject {
    /// 用主题创建，无自定义声明。
    pub fn new(subject: impl Into<String>) -> Self {
        Self {
            subject: subject.into(),
            custom_claims: serde_json::Value::Null,
        }
    }

    /// 设置自定义声明（链式）。
    pub fn with_claims(mut self, claims: serde_json::Value) -> Self {
        self.custom_claims = claims;
        self
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// CredentialVerifier：业务实现的凭证校验
// ──────────────────────────────────────────────────────────────────────────────

/// 凭证校验 trait，由业务方实现并注入 [`LoginAction`]。
#[async_trait]
pub trait CredentialVerifier: Send + Sync + 'static {
    /// 校验登录凭证。
    ///
    /// # 参数
    ///
    /// - `ctx`: 动作上下文（可借此访问数据库等）
    /// - `input`: 登录输入
    ///
    /// # 返回
    ///
    /// - `Ok(VerifiedSubject)`: 校验通过，携带签发 Token 所需的主体信息
    /// - `Err(BaseError)`: 校验失败（如 `Unauthorized`）
    async fn verify(
        &self,
        ctx: &ActionContext,
        input: &LoginInput,
    ) -> Result<VerifiedSubject, BaseError>;
}

// ──────────────────────────────────────────────────────────────────────────────
// LoginAction
// ──────────────────────────────────────────────────────────────────────────────

/// 登录 Action：校验凭证后签发 Token 对。公开（无需登录即可访问）。
#[derive(Action)]
#[action(
    name = "login",
    display_name = "登录",
    description = "校验凭证并签发 Token 对",
    public
)]
pub struct LoginAction<V: CredentialVerifier> {
    verifier: V,
}

impl<V: CredentialVerifier> LoginAction<V> {
    /// 用业务凭证校验器创建登录 Action。
    pub fn new(verifier: V) -> Self {
        Self { verifier }
    }
}

#[async_trait]
impl<V: CredentialVerifier> TypedHandler for LoginAction<V> {
    type Input = LoginInput;
    type Output = TokenPairResponse;

    async fn handle(
        &self,
        ctx: ActionContext,
        input: LoginInput,
    ) -> Result<TokenPairResponse, BaseError> {
        let subject = self.verifier.verify(&ctx, &input).await?;
        let (access_token, refresh_token) = ctx
            .tools
            .token_manager()
            .generate_token_pair(&subject.subject, subject.custom_claims)?;
        Ok(TokenPairResponse {
            access_token,
            refresh_token,
        })
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// RefreshAction
// ──────────────────────────────────────────────────────────────────────────────

/// 刷新 Action：用 Refresh Token 换取新的 Access Token。公开。
#[derive(Action, Default)]
#[action(
    name = "refresh",
    display_name = "刷新 Token",
    description = "用 Refresh Token 换取新的 Access Token",
    public
)]
pub struct RefreshAction;

impl RefreshAction {
    /// 创建 RefreshAction。
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedHandler for RefreshAction {
    type Input = RefreshInput;
    type Output = AccessTokenResponse;

    async fn handle(
        &self,
        ctx: ActionContext,
        input: RefreshInput,
    ) -> Result<AccessTokenResponse, BaseError> {
        let manager = ctx.tools.token_manager();
        // 撤销校验：被拉黑的 Refresh Token 不能再刷新
        let claims = manager.verify_token_checked(&input.refresh_token).await?;
        if claims.token_type != "refresh" {
            return Err(BaseError::TokenTypeInvalid("期望 refresh token".to_string()));
        }
        let access_token =
            manager.refresh_access_token(&input.refresh_token, serde_json::Value::Null)?;
        Ok(AccessTokenResponse { access_token })
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// LogoutAction
// ──────────────────────────────────────────────────────────────────────────────

/// 登出 Action：撤销 Token（写入 Redis 黑名单）。公开（凭 Token 自证）。
#[derive(Action, Default)]
#[action(
    name = "logout",
    display_name = "登出",
    description = "撤销 Token，使其在过期前失效",
    public
)]
pub struct LogoutAction;

impl LogoutAction {
    /// 创建 LogoutAction。
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedHandler for LogoutAction {
    type Input = LogoutInput;
    type Output = MessageResponse;

    async fn handle(
        &self,
        ctx: ActionContext,
        input: LogoutInput,
    ) -> Result<MessageResponse, BaseError> {
        ctx.tools.token_manager().revoke_token(&input.token).await?;
        Ok(MessageResponse {
            message: "已登出".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{DynAction, TypedAction};

    struct DummyVerifier;

    #[async_trait]
    impl CredentialVerifier for DummyVerifier {
        async fn verify(
            &self,
            _ctx: &ActionContext,
            input: &LoginInput,
        ) -> Result<VerifiedSubject, BaseError> {
            Ok(VerifiedSubject::new(format!("user:{}", input.username)))
        }
    }

    #[test]
    fn test_auth_actions_meta() {
        let login = LoginAction::new(DummyVerifier);
        assert_eq!(login.name(), "login");
        assert!(DynAction::meta(&login).is_public);

        let refresh = RefreshAction::new();
        assert_eq!(refresh.name(), "refresh");
        assert!(refresh.is_public());

        let logout = LogoutAction::new();
        assert_eq!(logout.name(), "logout");
        assert!(logout.is_public());
    }
}
