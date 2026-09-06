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
//!
//! # 认证通用机制
//!
//! 除内置 Action 外，本模块还下沉了账号域可复用的机制能力：
//!
//! - [`password`]：Argon2 密码哈希与校验的受控执行边界（并发上限由构造函数注入）。
//! - [`rate_limit`]：认证入口的 Redis 原子限流与失败计数。
//! - [`email_verification`]：一次性邮箱验证码（摘要存储、防枚举、原子单次消费），
//!   投递经业务实现的 sender trait 注入。
//! - [`browser_session`]：浏览器刷新会话 Cookie 签发/清除与同源校验。

mod audit;
mod browser_session;
mod dto;
mod email_verification;
mod login;
mod logout;
mod mfa;
mod middleware;
mod password;
mod rate_limit;
mod refresh;

pub use audit::{token_fingerprint, AuthAuditEvent, AuthAuditHook, TracingAuditHook};
pub use browser_session::{BrowserAccessToken, BrowserSession, ReloginRequired};
pub use dto::{
    AccessTokenResponse, LoginInput, LogoutInput, MessageResponse, RefreshInput, TokenPairClaims,
    TokenPairResponse, VerifiedSubject,
};
pub use email_verification::{
    normalize_email, EmailDeliveryError, EmailVerificationConfig, RegistrationEmailCodeAccepted,
    RegistrationEmailSender, RegistrationEmailSenderHandle, RegistrationEmailVerification,
};
pub use login::{CredentialVerifier, LoginAction};
pub use logout::LogoutAction;
pub use mfa::{TotpLiteVerifier, TotpVerifier};
pub use middleware::{
    IntoUserProjection, NoopTokenClaimsValidator, TokenAuthMiddleware, TokenClaimsValidator,
};
pub use password::PasswordEngine;
pub use rate_limit::{AuthOperation, AuthRateLimitConfig, AuthRateLimiter};
pub use refresh::{DefaultRefreshClaims, RefreshAction, RefreshClaimsResolver};

#[cfg(test)]
mod __tests__;
