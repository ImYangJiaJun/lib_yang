//! Router 中间件 / 拦截器机制（H-5）
//!
//! `Registry::dispatch` 使用本模块的洋葱模型组合日志、限流、请求追踪与认证。
//! 每个中间件拿到 [`ActionContext`] 与代表"调用链剩余部分"的
//! [`Next`]，可在调用 `next.run(ctx)` 前后插入逻辑，也可短路直接返回。
//!
//! # 注意：ActionContext 不是 Clone
//!
//! [`ActionContext`] 持有数据库句柄等不可克隆资源，因此中间件链以**移动**方式
//! 传递 ctx——`handle` 接收 `ctx` 的所有权，并在调用 `next.run(ctx)` 时把它交给
//! 链路的下一环。这保证全链路只有一份上下文。
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::router::{Middleware, Next};
//! use yang_base::action::{ActionContext, ApiResponse};
//! use yang_base::error::BaseError;
//! use async_trait::async_trait;
//!
//! struct LoggingMiddleware;
//!
//! #[async_trait]
//! impl Middleware for LoggingMiddleware {
//!     async fn handle(&self, ctx: ActionContext, next: Next<'_>)
//!         -> Result<ApiResponse, BaseError>
//!     {
//!         log::info!("进入 Action");
//!         let result = next.run(ctx).await;
//!         log::info!("离开 Action");
//!         result
//!     }
//! }
//!
//! // 通过 ModuleSpec::middleware 按顺序注册 LoggingMiddleware。
//! ```

use crate::action::{ActionContext, ApiResponse, DynAction, PermissionMode};
use crate::error::BaseError;
use async_trait::async_trait;
use std::sync::Arc;

/// 构建期冻结的 Action 授权策略。
#[derive(Debug, Clone)]
pub(crate) struct AuthorizationPolicy {
    pub(crate) is_public: bool,
    groups: Arc<[PermissionGroup]>,
}

impl AuthorizationPolicy {
    pub(crate) fn new(is_public: bool, groups: impl Into<Arc<[PermissionGroup]>>) -> Self {
        Self {
            is_public,
            groups: groups.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PermissionGroup {
    label: &'static str,
    permissions: Arc<[String]>,
    mode: PermissionMode,
}

impl PermissionGroup {
    pub(crate) fn new(
        label: &'static str,
        permissions: impl Into<Arc<[String]>>,
        mode: PermissionMode,
    ) -> Self {
        Self {
            label,
            permissions: permissions.into(),
            mode,
        }
    }
}

/// 中间件适用的 Action 范围。
///
/// 通用中间件使用默认的 [`AllActions`](Self::AllActions)，因此日志、限流、
/// 请求追踪等横切逻辑会覆盖公开与受保护 Action。强制认证中间件应返回
/// [`ProtectedActions`](Self::ProtectedActions)，公开 Action 会跳过它，而受保护
/// Action 仍按原有洋葱链顺序执行。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MiddlewareScope {
    /// 对公开与受保护 Action 都生效。
    #[default]
    AllActions,
    /// 只对需要认证的受保护 Action 生效。
    ProtectedActions,
}

/// 中间件 trait。
///
/// 实现者在 `handle` 中拦截一次 Action 派发：可在调用 `next.run(ctx)` 前后
/// 注入逻辑，或不调用 `next` 直接短路返回。
#[async_trait]
pub trait Middleware: Send + Sync + 'static {
    /// 返回此中间件适用的 Action 范围。
    ///
    /// 默认覆盖全部 Action，以保持日志、限流、追踪和其他通用中间件的直观语义。
    /// 强制认证中间件应显式返回 [`MiddlewareScope::ProtectedActions`]。
    fn scope(&self) -> MiddlewareScope {
        MiddlewareScope::AllActions
    }

    /// 处理一次派发。
    ///
    /// # 参数
    ///
    /// - `ctx`: 动作上下文（所有权转移，需在调用 `next.run` 时传递下去）
    /// - `next`: 调用链的剩余部分
    async fn handle(&self, ctx: ActionContext, next: Next<'_>) -> Result<ApiResponse, BaseError>;
}

/// 调用链中"剩余部分"的句柄。
///
/// 持有尚未执行的中间件切片、构建期授权策略以及链尾的目标 [`DynAction`]。
/// 中间件链是**最外层**：当中间件耗尽时，链尾执行「授权 + Action 派发」。
/// [`MiddlewareScope::AllActions`] 中间件可以观察并干预所有请求；
/// [`MiddlewareScope::ProtectedActions`] 中间件则会在公开 Action 上被跳过。
pub struct Next<'a> {
    /// 尚未执行的中间件
    pub(crate) remaining: &'a [Arc<dyn Middleware>],
    /// 链尾要执行的目标 Action
    pub(crate) action: Arc<dyn DynAction>,
    /// 构建期冻结的授权策略。
    pub(crate) policy: &'a AuthorizationPolicy,
}

impl<'a> Next<'a> {
    /// 推进调用链。
    ///
    /// 依次跳过不适用于目标 Action 的中间件；遇到第一个适用中间件时执行它，
    /// 并把其余部分包装成新的 `Next` 传入。全部耗尽后执行链尾的
    /// 「内置鉴权 + Action 派发」。
    ///
    /// # 参数
    ///
    /// - `ctx`: 动作上下文（所有权转移）
    pub async fn run(self, ctx: ActionContext) -> Result<ApiResponse, BaseError> {
        let mut remaining = self.remaining;

        while let Some((current, rest)) = remaining.split_first() {
            remaining = rest;
            let applies = match current.scope() {
                MiddlewareScope::AllActions => true,
                MiddlewareScope::ProtectedActions => !self.policy.is_public,
            };
            if applies {
                let next = Next {
                    remaining: rest,
                    action: self.action,
                    policy: self.policy,
                };
                return current.handle(ctx, next).await;
            }
        }

        authorize_and_dispatch(self.action, self.policy, ctx).await
    }
}

async fn authorize_and_dispatch(
    action: Arc<dyn DynAction>,
    policy: &AuthorizationPolicy,
    context: ActionContext,
) -> Result<ApiResponse, BaseError> {
    let span = tracing::info_span!(
        "authorize",
        is_public = policy.is_public,
        granted = tracing::field::Empty,
    );
    let _enter = span.enter();

    authorize(policy, &context).inspect_err(|_| {
        span.record("granted", false);
    })?;

    span.record("granted", true);
    drop(_enter);
    action.dispatch(context).await
}

pub(crate) fn authorize(
    policy: &AuthorizationPolicy,
    context: &ActionContext,
) -> Result<(), BaseError> {
    if policy.is_public {
        return Ok(());
    }
    let user = context
        .authenticated_user()
        .ok_or_else(|| BaseError::Unauthorized("需要登录".to_string()))?;
    for group in policy.groups.iter() {
        if !permissions_match(user, &group.permissions, group.mode) {
            return Err(BaseError::PermissionDenied(format!(
                "缺少 {} 权限: {:?}",
                group.label, group.permissions
            )));
        }
    }
    Ok(())
}

fn permissions_match(
    user: &crate::action::User,
    permissions: &[String],
    mode: PermissionMode,
) -> bool {
    if permissions.is_empty() {
        return true;
    }
    match mode {
        PermissionMode::All => permissions
            .iter()
            .all(|permission| user.has_permission(permission)),
        PermissionMode::Any => permissions
            .iter()
            .any(|permission| user.has_permission(permission)),
    }
}

/// request_id 透传中间件（可观测性 C4）。
///
/// 作洋葱链**最外层**：若请求头含 `X-Request-Id` 则解析透传（解析失败回退为
/// `ActionContext` 已生成的标识），否则沿用默认生成值；随后写入根 span 的
/// `request_id` 字段以串联日志/metrics/审计。
///
/// 注：`ActionContext::new` 默认已生成一个 request_id，本中间件只负责「上游透传」
/// 这一增量语义，不破坏无中间件时的可观测性。
///
/// # 示例
///
/// ```rust,ignore
/// use yang_base::definition::{ModuleName, ModuleSpec};
/// use yang_base::router::RequestIdMiddleware;
///
/// let module = ModuleSpec::new(ModuleName::new("account.user")?)
///     .middleware(RequestIdMiddleware);
/// ```
pub struct RequestIdMiddleware;

#[async_trait]
impl Middleware for RequestIdMiddleware {
    async fn handle(
        &self,
        mut ctx: ActionContext,
        next: Next<'_>,
    ) -> Result<ApiResponse, BaseError> {
        // 上游 X-Request-Id 优先：存在且可解析则透传，否则保留默认生成值
        if let Some(raw) = ctx
            .request
            .headers
            .get("X-Request-Id")
            .or_else(|| ctx.request.headers.get("x-request-id"))
        {
            if let Some(rid) = crate::action::RequestId::parse_hex(raw) {
                ctx = ctx.with_request_id(rid);
            }
        }
        // 写入当前 span（dispatch 根 span 已声明 request_id 字段）
        tracing::Span::current().record("request_id", tracing::field::display(ctx.request_id));
        next.run(ctx).await
    }
}
