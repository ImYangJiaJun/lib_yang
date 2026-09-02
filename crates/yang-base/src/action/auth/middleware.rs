//! Token 鉴权中间件与应用级声明校验钩子。

use crate::action::{ActionContext, ApiResponse, User};
use crate::error::BaseError;
use crate::router::middleware::{Middleware, MiddlewareRole, MiddlewareScope, Next};
use crate::token::TokenClaims;
use async_trait::async_trait;

// ──────────────────────────────────────────────────────────────────────────────
// TokenAuthMiddleware
// ──────────────────────────────────────────────────────────────────────────────

/// Token 鉴权中间件：在 Action 派发前完成 JWT 三重校验并注入当前用户。
///
/// 挂到 [`ModuleSpec`](crate::definition::ModuleSpec) 后，只对非公开 Action 执行；
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
/// use yang_base::definition::{ModuleName, ModuleSpec};
///
/// // 从 JWT sub 取用户 ID；业务标识解析失败时必须拒绝认证
/// let auth = TokenAuthMiddleware::new(|claims| {
///     let id = claims.sub.parse::<i64>()
///         .map_err(|_| yang_base::BaseError::Unauthorized("Token subject 无效".into()))?;
///     Ok(User::new(id, claims.sub.clone()))
/// });
///
/// let module = ModuleSpec::new(ModuleName::new("account.user")?).middleware(auth);
/// ```
pub struct TokenAuthMiddleware<F, V = NoopTokenClaimsValidator> {
    /// 从已验证声明构造业务 [`User`](crate::action::User) 的闭包
    build_user: F,
    /// 签名与 Token 类型通过后的应用级异步声明校验器。
    claims_validator: V,
    /// 是否在公开 Action 上执行可选认证。
    authenticate_public_actions: bool,
}

/// 已验签 Access Token 的应用级校验钩子。
///
/// 基础库保持 JWT、黑名单和类型校验的唯一认证链；业务系统可在用户投影前校验
/// 授权版本、会话世代等应用事实，而无需重复解析或验签 Token。
#[async_trait]
pub trait TokenClaimsValidator: Send + Sync + 'static {
    /// 校验已通过核心 Token 验证的声明；返回错误会在用户投影前短路认证。
    async fn validate(
        &self,
        context: &ActionContext,
        claims: &TokenClaims,
    ) -> Result<(), BaseError>;
}

/// 不增加额外 I/O 的默认声明校验器。
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopTokenClaimsValidator;

#[async_trait]
impl TokenClaimsValidator for NoopTokenClaimsValidator {
    async fn validate(
        &self,
        _context: &ActionContext,
        _claims: &TokenClaims,
    ) -> Result<(), BaseError> {
        Ok(())
    }
}

/// 将不可失败的旧式用户投影和可失败的安全投影统一成认证结果。
///
/// 该适配 trait 公开仅用于满足 [`TokenAuthMiddleware`] 的泛型边界；业务代码通常
/// 只需让闭包返回 [`User`] 或 `Result<User, BaseError>`。
#[doc(hidden)]
pub trait IntoUserProjection {
    fn into_user_projection(self) -> Result<User, BaseError>;
}

impl IntoUserProjection for User {
    fn into_user_projection(self) -> Result<User, BaseError> {
        Ok(self)
    }
}

impl IntoUserProjection for Result<User, BaseError> {
    fn into_user_projection(self) -> Result<User, BaseError> {
        self
    }
}

impl<F, R> TokenAuthMiddleware<F, NoopTokenClaimsValidator>
where
    F: Fn(&TokenClaims) -> R + Send + Sync + 'static,
    R: IntoUserProjection,
{
    /// 用「声明 -> 用户」闭包创建 Token 鉴权中间件。
    ///
    /// 闭包可返回 `User` 保持简单场景兼容，也可返回 `Result<User, BaseError>`，在
    /// subject、角色或权限声明格式非法时 fail-closed。
    pub fn new(build_user: F) -> Self {
        Self {
            build_user,
            claims_validator: NoopTokenClaimsValidator,
            authenticate_public_actions: false,
        }
    }
}

impl<F, V> TokenAuthMiddleware<F, V> {
    /// 注入应用级异步声明校验器，并保留同一条 Token 认证链。
    pub fn with_claims_validator<N>(self, claims_validator: N) -> TokenAuthMiddleware<F, N>
    where
        N: TokenClaimsValidator,
    {
        TokenAuthMiddleware {
            build_user: self.build_user,
            claims_validator,
            authenticate_public_actions: self.authenticate_public_actions,
        }
    }

    /// 在公开 Action 上启用可选认证。
    ///
    /// 默认情况下，本中间件只处理受保护 Action，以确保登录、刷新等公开端点不会
    /// 因缺少 Token 被拦截。启用本选项后，公开 Action 在没有 Authorization header
    /// 时仍按匿名请求继续；携带 Bearer Token 时则完成完整校验并注入用户。该模式
    /// 适用于请求级 UI 目录等“匿名可用、登录后按身份投影”的公开端点。
    ///
    /// 非 Bearer Authorization header、无效 Token 和错误 Token 类型不会降级为匿名。
    pub fn authenticate_public_actions(mut self) -> Self {
        self.authenticate_public_actions = true;
        self
    }
}

#[async_trait]
impl<F, R, V> Middleware for TokenAuthMiddleware<F, V>
where
    F: Fn(&TokenClaims) -> R + Send + Sync + 'static,
    R: IntoUserProjection + Send + Sync + 'static,
    V: TokenClaimsValidator,
{
    fn role(&self) -> MiddlewareRole {
        MiddlewareRole::Authentication
    }

    fn scope(&self) -> MiddlewareScope {
        if self.authenticate_public_actions {
            MiddlewareScope::AllActions
        } else {
            MiddlewareScope::ProtectedActions
        }
    }

    async fn handle(
        &self,
        mut ctx: ActionContext,
        next: Next<'_>,
    ) -> Result<ApiResponse, BaseError> {
        // 1. 取 Bearer Token（owned，及早结束对 ctx 的借用）
        let token = match ctx.request.token() {
            Some(t) => t.to_string(),
            None if self.authenticate_public_actions
                && next.policy.is_public
                && ctx.request.get_header("authorization").is_none() =>
            {
                return next.run(ctx).await;
            }
            None => {
                return Err(BaseError::Unauthorized(
                    "缺少 Authorization Bearer Token".to_string(),
                ))
            }
        };

        // 2. 签名 + 过期 + 黑名单三重校验（失败原样短路）
        let claims = ctx.tools().token()?.verify_token_checked(&token).await?;

        // 3. 校验 token_type 必须为 Access
        if claims.token_type != crate::token::TokenType::Access {
            return Err(BaseError::TokenTypeInvalid("期望 access token".into()));
        }

        // 4. 应用级事实校验仍位于唯一认证链内，不重复解析或验签 Token
        self.claims_validator.validate(&ctx, &claims).await?;

        // 5. 注入当前用户后继续调用链
        ctx.user = Some((self.build_user)(&claims).into_user_projection()?);
        if let Some(user) = &ctx.user {
            tracing::Span::current().record("actor_id", user.id);
        }
        next.run(ctx).await
    }
}
