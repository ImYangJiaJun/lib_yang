//! 刷新 Action 与刷新声明解析：用 Refresh Token 旋转换取新的 Token 对。

use super::audit::{AuthAuditEvent, AuthAuditHook, TracingAuditHook};
use super::dto::{RefreshInput, TokenPairClaims, TokenPairResponse};
use crate::action::{ActionContext, TypedHandler};
use crate::error::BaseError;
use crate::token::TokenClaims;
use async_trait::async_trait;
use yang_base_derive::Action;

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

    /// 从同一次业务快照解析新 Access/Refresh Token 的声明。
    ///
    /// 默认只复用 [`RefreshClaimsResolver::resolve`] 生成 Access 声明，Refresh
    /// 声明保持为空，因此既有实现无需修改。需要成对版本声明的实现可覆盖本方法，
    /// 保证只执行一次数据库快照查询。
    async fn resolve_pair(
        &self,
        ctx: &ActionContext,
        sub: &str,
    ) -> Result<TokenPairClaims, BaseError> {
        Ok(TokenPairClaims::new(self.resolve(ctx, sub).await?))
    }

    /// 基于已通过核心签名、时效、类型和撤销校验的旧 Refresh Token 完整声明，解析
    /// 新 Token 对的声明。
    ///
    /// 应用可覆盖本方法校验 `credential_version` 等 Refresh 专属事实，并从同一次数据库
    /// 快照生成新 Access/Refresh 声明。默认仅把可信 `sub` 转交给
    /// [`RefreshClaimsResolver::resolve_pair`]，因此既有实现保持兼容。
    ///
    /// 本 hook 在旧 JTI 被原子消费前执行；校验失败不会消耗仍然有效的 Refresh Token。
    async fn resolve_pair_from_claims(
        &self,
        ctx: &ActionContext,
        claims: &TokenClaims,
    ) -> Result<TokenPairClaims, BaseError> {
        self.resolve_pair(ctx, &claims.sub).await
    }
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
/// 原 `RefreshAction` 仅返回 [`AccessTokenResponse`](super::AccessTokenResponse)（只含新的 Access Token）。
/// 现改为返回 [`TokenPairResponse`](super::TokenPairResponse)（同时包含新的 Access Token 与新的 Refresh Token）。
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
        let manager = ctx.tools().token()?;

        let run = async {
            // 先验证旧 Token 以获取 subject（供业务解析器确定新声明）
            let claims = manager.verify_token_checked(&input.refresh_token).await?;
            if claims.token_type != crate::token::TokenType::Refresh {
                return Err(BaseError::TokenTypeInvalid(
                    "期望 refresh token".to_string(),
                ));
            }
            // 先由业务解析器校验旧 Refresh 完整声明并决定新 Token 声明；失败不消费旧 JTI
            let custom_claims = self
                .resolver
                .resolve_pair_from_claims(&ctx, &claims)
                .await?;
            // 使用已验证的 claims 直接轮换，跳过 rotate_refresh_token 内部二次验证
            let (access_token, refresh_token) = manager
                .rotate_refresh_token_from_claims_with_refresh_claims(
                    &claims,
                    custom_claims.access,
                    custom_claims.refresh,
                )
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
