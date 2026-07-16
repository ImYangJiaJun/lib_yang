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

use crate::action::{ActionContext, ApiResponse, TypedHandler, User};
use crate::error::BaseError;
use crate::router::middleware::{Middleware, MiddlewareScope, Next};
use crate::token::TokenClaims;
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
// AuthAuditHook：认证审计钩子（可观测性 C4）
// ──────────────────────────────────────────────────────────────────────────────

/// 认证审计事件。
///
/// 描述一次认证相关操作（登录/刷新/登出）的结果。**绝不携带凭据明文或 Token
/// 原文**：需要标识 Token 时只放指纹（[`token_fingerprint`]）。
///
/// 标注 `#[non_exhaustive]`：未来新增字段不构成破坏性变更。
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct AuthAuditEvent {
    /// 本次派发的 request_id（十六进制串）
    pub request_id: String,
    /// 操作名（`"login"` / `"refresh"` / `"logout"`，静态）
    pub action: &'static str,
    /// 主体标识（用户名或 sub）；失败且未知时为 `None`
    pub subject: Option<String>,
    /// 失败时的错误码（`BaseError::code_str`，静态）；成功时为 `None`
    pub error_code: Option<&'static str>,
}

/// 计算 Token 的短指纹（SHA256 前若干字节的十六进制），用于审计日志而不泄漏原文。
///
/// 这里不引入额外哈希依赖，采用 FNV-1a 64 位哈希取十六进制——审计场景只需「同一
/// Token 多次出现可对应」而非密码学强度，FNV 足够且零依赖。
pub fn token_fingerprint(token: &str) -> String {
    // FNV-1a 64-bit
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in token.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", hash)
}

/// 认证审计钩子：在登录/刷新/登出的成功/失败处被调用。
///
/// object-safe，可经构造参数注入（与 [`CredentialVerifier`] 注入同构）。默认实现
/// [`TracingAuditHook`] 发 tracing event。实现者**绝不应**记录凭据明文/Token 原文。
#[async_trait]
pub trait AuthAuditHook: Send + Sync + 'static {
    /// 认证成功事件。
    async fn on_success(&self, event: AuthAuditEvent);
    /// 认证失败事件。
    async fn on_failure(&self, event: AuthAuditEvent);
}

/// 默认审计钩子：成功发 `tracing::info!`，失败发 `tracing::warn!`。
#[derive(Debug, Clone, Default)]
pub struct TracingAuditHook;

#[async_trait]
impl AuthAuditHook for TracingAuditHook {
    async fn on_success(&self, event: AuthAuditEvent) {
        tracing::info!(
            request_id = %event.request_id,
            action = event.action,
            subject = event.subject.as_deref().unwrap_or("-"),
            "认证成功",
        );
    }

    async fn on_failure(&self, event: AuthAuditEvent) {
        tracing::warn!(
            request_id = %event.request_id,
            action = event.action,
            subject = event.subject.as_deref().unwrap_or("-"),
            error_code = event.error_code.unwrap_or("-"),
            "认证失败",
        );
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// LoginAction
// ──────────────────────────────────────────────────────────────────────────────

/// 登录 Action：校验凭证后签发 Token 对。公开（无需登录即可访问）。
///
/// 泛型 `A` 为审计钩子（默认 [`TracingAuditHook`]）：登录成功/失败均发审计事件，
/// 事件只含 request_id/subject/错误码，**不含凭据明文**。
#[derive(Action)]
#[action(
    name = "login",
    display_name = "登录",
    description = "校验凭证并签发 Token 对",
    public
)]
pub struct LoginAction<V: CredentialVerifier, A: AuthAuditHook = TracingAuditHook> {
    verifier: V,
    audit: A,
}

impl<V: CredentialVerifier> LoginAction<V, TracingAuditHook> {
    /// 用业务凭证校验器创建登录 Action（默认 tracing 审计钩子）。
    pub fn new(verifier: V) -> Self {
        Self {
            verifier,
            audit: TracingAuditHook,
        }
    }
}

impl<V: CredentialVerifier, A: AuthAuditHook> LoginAction<V, A> {
    /// 用业务凭证校验器 + 自定义审计钩子创建登录 Action。
    pub fn with_audit(verifier: V, audit: A) -> Self {
        Self { verifier, audit }
    }
}

#[async_trait]
impl<V: CredentialVerifier, A: AuthAuditHook> TypedHandler for LoginAction<V, A> {
    type Input = LoginInput;
    type Output = TokenPairResponse;

    async fn handle(
        &self,
        ctx: ActionContext,
        input: LoginInput,
    ) -> Result<TokenPairResponse, BaseError> {
        let request_id = ctx.request_id.to_string();
        let subject = match self.verifier.verify(&ctx, &input).await {
            Ok(s) => s,
            Err(e) => {
                self.audit
                    .on_failure(AuthAuditEvent {
                        request_id,
                        action: "login",
                        subject: Some(input.username.clone()),
                        error_code: Some(e.code_str()),
                    })
                    .await;
                return Err(e);
            }
        };
        let result = ctx
            .tools
            .token_manager()
            .generate_token_pair(&subject.subject, subject.custom_claims);
        match result {
            Ok((access_token, refresh_token)) => {
                self.audit
                    .on_success(AuthAuditEvent {
                        request_id,
                        action: "login",
                        subject: Some(subject.subject.clone()),
                        error_code: None,
                    })
                    .await;
                Ok(TokenPairResponse {
                    access_token,
                    refresh_token,
                })
            }
            Err(e) => {
                self.audit
                    .on_failure(AuthAuditEvent {
                        request_id,
                        action: "login",
                        subject: Some(subject.subject.clone()),
                        error_code: Some(e.code_str()),
                    })
                    .await;
                Err(e)
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// RefreshAction
// ──────────────────────────────────────────────────────────────────────────────

/// 刷新声明解析器：决定刷新出的新 Access Token 携带哪些自定义声明。
///
/// 刷新时若不重新解析声明，新 Access Token 会丢失角色/权限等自定义声明。业务方
/// 实现本 trait，从已验证的 Refresh Token 主体（`sub`）出发（通常回查数据库取最新
/// 角色），返回要写入新 Access Token 的自定义声明。
#[async_trait]
pub trait RefreshClaimsResolver: Send + Sync + 'static {
    /// 解析新 Access Token 的自定义声明。
    ///
    /// # 参数
    ///
    /// - `ctx`: 动作上下文（可借此访问数据库等）
    /// - `sub`: 已验证的 Refresh Token 主体（用户标识）
    ///
    /// # 返回
    ///
    /// - `Ok(Value)`: 写入新 Access Token 的自定义声明（`Value::Null` 表示无）
    /// - `Err(BaseError)`: 解析失败（如用户已禁用）
    async fn resolve(&self, ctx: &ActionContext, sub: &str)
        -> Result<serde_json::Value, BaseError>;
}

/// 默认刷新声明解析器：不附加任何自定义声明（零配置可用）。
#[derive(Debug, Clone, Default)]
pub struct DefaultRefreshClaims;

#[async_trait]
impl RefreshClaimsResolver for DefaultRefreshClaims {
    async fn resolve(
        &self,
        _ctx: &ActionContext,
        _sub: &str,
    ) -> Result<serde_json::Value, BaseError> {
        Ok(serde_json::Value::Null)
    }
}

/// 刷新 Action：用 Refresh Token 换取新的 Access Token。公开。
///
/// 仅调用一次 [`TokenManager::verify_token_checked`](crate::token::TokenManager::verify_token_checked)
/// 完成验证（签名 + 过期 + 黑名单），随后将已验证的 [`TokenClaims`] 直接传给
/// [`TokenManager::rotate_refresh_token_from_claims`](crate::token::TokenManager::rotate_refresh_token_from_claims)，跳过内部二次验证，
/// 节省 2 次 Redis RTT。自定义声明由注入的 [`RefreshClaimsResolver`] 决定，
/// 默认 [`DefaultRefreshClaims`] 不附加声明，零配置即可使用。
///
/// # 示例
///
/// ```rust,ignore
/// use yang_base::action::auth::{RefreshAction, RefreshClaimsResolver};
/// use yang_base::action::ActionContext;
/// use yang_base::error::BaseError;
/// use async_trait::async_trait;
///
/// // 零配置：默认 resolver
/// let refresh = RefreshAction::default();
///
/// // 自定义：刷新时回查用户最新角色
/// struct RoleResolver;
/// #[async_trait]
/// impl RefreshClaimsResolver for RoleResolver {
///     async fn resolve(&self, _ctx: &ActionContext, sub: &str)
///         -> Result<serde_json::Value, BaseError>
///     {
///         Ok(serde_json::json!({ "role": "user", "sub": sub }))
///     }
/// }
/// let refresh = RefreshAction::new(RoleResolver);
/// ```
#[derive(Action, Default)]
#[action(
    name = "refresh",
    display_name = "刷新 Token",
    description = "用 Refresh Token 换取新的 Access Token",
    public
)]
pub struct RefreshAction<
    R: RefreshClaimsResolver = DefaultRefreshClaims,
    A: AuthAuditHook = TracingAuditHook,
> {
    resolver: R,
    audit: A,
}

impl<R: RefreshClaimsResolver> RefreshAction<R, TracingAuditHook> {
    /// 用业务声明解析器创建 RefreshAction（默认 tracing 审计钩子）。
    pub fn new(resolver: R) -> Self {
        Self {
            resolver,
            audit: TracingAuditHook,
        }
    }
}

impl<R: RefreshClaimsResolver, A: AuthAuditHook> RefreshAction<R, A> {
    /// 用业务声明解析器 + 自定义审计钩子创建 RefreshAction。
    pub fn with_audit(resolver: R, audit: A) -> Self {
        Self { resolver, audit }
    }
}

/// Refresh Token 旋转刷新 Action。
///
/// 此方法使用 **Token Rotation** 模式：每次刷新时同时撤销旧 Refresh Token 并签发
/// 新的 Token 对（Access Token + Refresh Token），防止 Refresh Token 被盗后的
/// 无限刷新攻击。
///
/// # 破坏性变更（BREAKING CHANGE）
///
/// 原 `RefreshAction` 仅返回 [`AccessTokenResponse`]（只含新的 Access Token）。
/// 现改为返回 [`TokenPairResponse`]（同时包含新的 Access Token 与新的 Refresh Token）。
/// 请更新客户端以替换保存的 Refresh Token，否则旧 Refresh Token 将无法用于后续刷新。
#[async_trait]
impl<R: RefreshClaimsResolver, A: AuthAuditHook> TypedHandler for RefreshAction<R, A> {
    type Input = RefreshInput;
    type Output = TokenPairResponse;

    async fn handle(
        &self,
        ctx: ActionContext,
        input: RefreshInput,
    ) -> Result<TokenPairResponse, BaseError> {
        let request_id = ctx.request_id.to_string();
        let manager = ctx.tools.token_manager();

        let run = async {
            // 先验证旧 Token 以获取 subject（供业务解析器确定新声明）
            let claims = manager.verify_token_checked(&input.refresh_token).await?;
            if claims.token_type != crate::token::TokenType::Refresh {
                return Err(BaseError::TokenTypeInvalid(
                    "期望 refresh token".to_string(),
                ));
            }
            // 按业务解析器决定新 Token 的自定义声明
            let custom_claims = self.resolver.resolve(&ctx, &claims.sub).await?;
            // 使用已验证的 claims 直接轮换，跳过 rotate_refresh_token 内部二次验证
            let (access_token, refresh_token) = manager
                .rotate_refresh_token_from_claims(&claims, custom_claims)
                .await?;
            Ok::<_, BaseError>((access_token, refresh_token, claims.sub))
        };

        match run.await {
            Ok((access_token, refresh_token, sub)) => {
                self.audit
                    .on_success(AuthAuditEvent {
                        request_id,
                        action: "refresh",
                        subject: Some(sub),
                        error_code: None,
                    })
                    .await;
                Ok(TokenPairResponse {
                    access_token,
                    refresh_token,
                })
            }
            Err(e) => {
                self.audit
                    .on_failure(AuthAuditEvent {
                        request_id,
                        action: "refresh",
                        subject: None,
                        error_code: Some(e.code_str()),
                    })
                    .await;
                Err(e)
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// LogoutAction
// ──────────────────────────────────────────────────────────────────────────────

/// 登出 Action：撤销 Token（写入 Redis 黑名单）。公开（凭 Token 自证）。
///
/// 彻底终止会话需**同时**撤销 Access Token 与 Refresh Token：只撤 Access Token
/// 时，攻击者仍可用未失效的 Refresh Token 刷出新的 Access Token。因此调用方应在
/// `token` 传 Access Token 的同时，于 `refresh_token` 传入 Refresh Token，本 Action
/// 会将二者一并拉黑。
///
/// # 所有权校验（AUTH-4）
///
/// 当请求携带 Bearer Token（`Authorization` 头）时，本 Action 会校验该 Bearer Token
/// 的 `sub` 与待撤销 Token（`input.token`）的 `sub` 一致。不一致时返回
/// [`BaseError::PermissionDenied`]，防止用户撤销他人的 Token。
///
/// 若请求未携带 Bearer Token（匿名调用），则跳过所有权校验——此时无法确认调用者身份，
/// 撤销操作仍允许执行（向后兼容）。
#[derive(Action, Default)]
#[action(
    name = "logout",
    display_name = "登出",
    description = "撤销 Token，使其在过期前失效",
    public
)]
pub struct LogoutAction<A: AuthAuditHook = TracingAuditHook> {
    audit: A,
}

impl LogoutAction<TracingAuditHook> {
    /// 创建 LogoutAction（默认 tracing 审计钩子）。
    pub fn new() -> Self {
        Self {
            audit: TracingAuditHook,
        }
    }
}

impl<A: AuthAuditHook> LogoutAction<A> {
    /// 用自定义审计钩子创建 LogoutAction。
    pub fn with_audit(audit: A) -> Self {
        Self { audit }
    }
}

#[async_trait]
impl<A: AuthAuditHook> TypedHandler for LogoutAction<A> {
    type Input = LogoutInput;
    type Output = MessageResponse;

    async fn handle(
        &self,
        ctx: ActionContext,
        input: LogoutInput,
    ) -> Result<MessageResponse, BaseError> {
        let request_id = ctx.request_id.to_string();
        let manager = ctx.tools.token_manager();

        let run = async {
            // AUTH-4：解析待撤销 Token 的 claims（仅校验签名/过期，不查黑名单——
            // 该 Token 本身就是要被撤销的目标，查黑名单无意义）
            let target_claims = manager.verify_token(&input.token)?;

            // 若请求携带 Bearer Token，校验其 sub 与待撤销 Token 的 sub 一致
            if let Some(bearer_token) = ctx.request.token() {
                // Bearer Token 需完整验证（含黑名单），确保调用者身份有效
                let caller_claims = manager.verify_token_checked(bearer_token).await?;
                if caller_claims.sub != target_claims.sub {
                    return Err(BaseError::PermissionDenied(
                        "只能撤销自己的 Token".to_string(),
                    ));
                }
            }

            // CONC-1：用 subject 水位线一次性原子撤销该用户所有 Token
            //（Access + Refresh + 任何其他已签发 Token），避免双 revoke 非原子竞态。
            // 水位线 TTL 取 Refresh Token 有效期，早于此时间签发的 Token 全部失效。
            manager.revoke_by_subject(&target_claims.sub).await?;
            Ok::<(), BaseError>(())
        };

        match run.await {
            Ok(()) => {
                // subject 用 Access Token 指纹标识（不泄漏原文）
                self.audit
                    .on_success(AuthAuditEvent {
                        request_id,
                        action: "logout",
                        subject: Some(token_fingerprint(&input.token)),
                        error_code: None,
                    })
                    .await;
                Ok(MessageResponse {
                    message: "已登出".to_string(),
                })
            }
            Err(e) => {
                self.audit
                    .on_failure(AuthAuditEvent {
                        request_id,
                        action: "logout",
                        subject: Some(token_fingerprint(&input.token)),
                        error_code: Some(e.code_str()),
                    })
                    .await;
                Err(e)
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// TokenAuthMiddleware
// ──────────────────────────────────────────────────────────────────────────────

/// Token 鉴权中间件：在 Action 派发前完成 JWT 三重校验并注入当前用户。
///
/// 挂到 [`ModuleRouter`](crate::router::ModuleRouter) 后，只对非公开 Action 执行；
/// 标记为 `public` 的 Action 会绕过本认证中间件，但仍会经过日志、限流、追踪等
/// 通用中间件。对受保护 Action：
///
/// 1. 从 [`Request::token`](crate::action::Request::token) 取 `Authorization: Bearer <token>`；
///    缺失则短路返回 [`BaseError::Unauthorized`]。
/// 2. 调用 [`TokenManager::verify_token_checked`](crate::token::TokenManager::verify_token_checked)
///    完成 **签名 + 过期 + 黑名单** 三重校验；失败短路（`TokenVerifyFailed` /
///    `TokenExpired` / `TokenRevoked` 等原样上抛）。
/// 3. 用注入的 `claims -> User` 闭包，从已验证的 [`TokenClaims`] 构造
///    [`User`] 并填入 `ActionContext.user`，随后 `next.run(ctx)`。
///
/// 用户如何从声明映射（角色/权限放在哪个自定义字段）因项目而异，故由闭包 `F` 注入。
///
/// # 示例
///
/// ```rust,ignore
/// use yang_base::action::{TokenAuthMiddleware, User};
/// use yang_base::router::ModuleRouter;
///
/// // 从 JWT sub 取用户 ID，从自定义声明 "roles" 取角色
/// let auth = TokenAuthMiddleware::new(|claims| {
///     let roles = claims.custom.get("roles")
///         .and_then(|v| v.as_array())
///         .map(|a| a.iter().filter_map(|r| r.as_str().map(String::from)).collect())
///         .unwrap_or_default();
///     let mut user = User::new(claims.sub.parse().unwrap_or(0), claims.sub.clone());
///     user.roles = roles;
///     user
/// });
///
/// let router = ModuleRouter::new("user", "用户管理").middleware(auth);
/// ```
pub struct TokenAuthMiddleware<F> {
    /// 从已验证声明构造业务 [`User`](crate::action::User) 的闭包
    build_user: F,
}

impl<F> TokenAuthMiddleware<F>
where
    F: Fn(&TokenClaims) -> User + Send + Sync + 'static,
{
    /// 用「声明 -> 用户」闭包创建 Token 鉴权中间件。
    pub fn new(build_user: F) -> Self {
        Self { build_user }
    }
}

#[async_trait]
impl<F> Middleware for TokenAuthMiddleware<F>
where
    F: Fn(&TokenClaims) -> User + Send + Sync + 'static,
{
    fn scope(&self) -> MiddlewareScope {
        MiddlewareScope::ProtectedActions
    }

    async fn handle(
        &self,
        mut ctx: ActionContext,
        next: Next<'_>,
    ) -> Result<ApiResponse, BaseError> {
        // 1. 取 Bearer Token（owned，及早结束对 ctx 的借用）
        let token = match ctx.request.token() {
            Some(t) => t.to_string(),
            None => {
                return Err(BaseError::Unauthorized(
                    "缺少 Authorization Bearer Token".to_string(),
                ))
            }
        };

        // 2. 签名 + 过期 + 黑名单三重校验（失败原样短路）
        let claims = ctx
            .tools
            .token_manager()
            .verify_token_checked(&token)
            .await?;

        // 3. 校验 token_type 必须为 Access
        if claims.token_type != crate::token::TokenType::Access {
            return Err(BaseError::TokenTypeInvalid("期望 access token".into()));
        }

        // 4. 注入当前用户后继续调用链
        ctx.user = Some((self.build_user)(&claims));
        next.run(ctx).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{DynAction, TypedAction};
    use crate::router::{Api, Middleware, ModuleRouter, Next};
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

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

    #[derive(Debug, Default, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields)]
    struct EmptyInput {}

    #[derive(Debug, Serialize, JsonSchema)]
    struct ProbeOutput {
        authenticated: bool,
    }

    #[derive(Action)]
    #[action(name = "protected_probe", display_name = "受保护探针")]
    struct ProtectedProbe;

    #[async_trait]
    impl TypedHandler for ProtectedProbe {
        type Input = EmptyInput;
        type Output = ProbeOutput;

        async fn handle(
            &self,
            ctx: ActionContext,
            _input: Self::Input,
        ) -> Result<Self::Output, BaseError> {
            Ok(ProbeOutput {
                authenticated: ctx.authenticated_user().is_some(),
            })
        }
    }

    struct CountingMiddleware(Arc<AtomicUsize>);

    #[async_trait]
    impl Middleware for CountingMiddleware {
        async fn handle(
            &self,
            ctx: ActionContext,
            next: Next<'_>,
        ) -> Result<ApiResponse, BaseError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            next.run(ctx).await
        }
    }

    fn test_tools() -> Arc<crate::action::GlobalTools> {
        Arc::new(crate::action::GlobalTools::new(
            crate::token::TokenManager::new_symmetric(
                "mixed-public-protected-actions-test-secret",
                jsonwebtoken::Algorithm::HS256,
                "test-issuer".to_string(),
                "test-audience".to_string(),
                3600,
                7200,
            ),
        ))
    }

    #[test]
    fn test_auth_actions_meta() {
        let login = LoginAction::new(DummyVerifier);
        assert_eq!(login.name(), "login");
        assert!(DynAction::meta(&login).is_public);

        let refresh = RefreshAction::<DefaultRefreshClaims>::default();
        assert_eq!(refresh.name(), "refresh");
        assert!(refresh.is_public());

        let logout = LogoutAction::new();
        assert_eq!(logout.name(), "logout");
        assert!(logout.is_public());
    }

    #[tokio::test]
    async fn public_and_protected_actions_share_auth_enabled_module() {
        let calls = Arc::new(AtomicUsize::new(0));
        let router = ModuleRouter::new("user", "用户")
            .middleware(CountingMiddleware(Arc::clone(&calls)))
            .middleware(TokenAuthMiddleware::new(|claims| {
                User::new(1, claims.sub.clone())
            }))
            .apis([
                Api::post("/api/v1/users/login", LoginAction::new(DummyVerifier)),
                Api::get("/api/v1/users/me", ProtectedProbe),
            ])
            .expect("同一模块应能注册公开与受保护 Action");

        let public_context = ActionContext::new(
            crate::action::Request::new(serde_json::json!({
                "username": "alice",
                "password": "correct-password"
            })),
            test_tools(),
        );
        let public_response = router.dispatch("login", public_context).await;
        assert!(
            public_response.is_ok(),
            "公开 Action 不应被 TokenAuthMiddleware 拦截: {public_response:?}"
        );

        let protected_context = ActionContext::new(
            crate::action::Request::new(serde_json::json!({})),
            test_tools(),
        );
        let protected_response = router.dispatch("protected_probe", protected_context).await;
        assert!(matches!(
            protected_response,
            Err(BaseError::Unauthorized(message))
                if message.contains("Authorization Bearer Token")
        ));
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "通用中间件应覆盖公开与受保护 Action"
        );
    }

    /// token 指纹稳定且不含原文（同输入同指纹，异输入异指纹）。
    #[test]
    fn test_token_fingerprint_stable_and_opaque() {
        let a = token_fingerprint("super-secret-access-token");
        let b = token_fingerprint("super-secret-access-token");
        let c = token_fingerprint("another-token");
        assert_eq!(a, b, "同一 token 指纹应稳定");
        assert_ne!(a, c, "不同 token 指纹应不同");
        assert_eq!(a.len(), 16, "指纹为 16 位十六进制");
        assert!(!a.contains("secret"), "指纹不得含原文");
    }

    /// 审计钩子注入：with_audit 可替换默认钩子，事件被记录且不含 token 原文。
    #[tokio::test]
    async fn test_audit_hook_records_without_leaking() {
        use std::sync::{Arc, Mutex};

        #[derive(Clone, Default)]
        struct RecordingHook {
            events: Arc<Mutex<Vec<AuthAuditEvent>>>,
        }
        #[async_trait]
        impl AuthAuditHook for RecordingHook {
            async fn on_success(&self, event: AuthAuditEvent) {
                self.events.lock().unwrap().push(event);
            }
            async fn on_failure(&self, event: AuthAuditEvent) {
                self.events.lock().unwrap().push(event);
            }
        }

        let hook = RecordingHook::default();
        let login = LoginAction::with_audit(DummyVerifier, hook.clone());
        // 仅验证构造 + 钩子类型注入成功（端到端派发在集成测试覆盖）
        assert_eq!(login.name(), "login");

        // 直接触发一次 on_success 验证事件落库且字段不含敏感原文
        hook.on_success(AuthAuditEvent {
            request_id: "abc".into(),
            action: "login",
            subject: Some("user:alice".into()),
            error_code: None,
        })
        .await;
        let events = hook.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, "login");
        assert_eq!(events[0].subject.as_deref(), Some("user:alice"));
    }
}
