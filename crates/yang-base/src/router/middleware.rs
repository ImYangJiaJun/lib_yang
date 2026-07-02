//! Router 中间件 / 拦截器机制（H-5）
//!
//! `ModuleRouter::dispatch` 原本把鉴权逻辑硬编码在派发流程里，跨切面逻辑
//! （日志、限流、请求追踪、自定义认证）无法优雅注入。本模块提供洋葱模型的
//! 可插拔中间件：每个中间件拿到 [`ActionContext`] 与代表"调用链剩余部分"的
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
//! use yang_base::router::{Middleware, Next, ModuleRouter};
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
//! let router = ModuleRouter::new("user", "用户管理")
//!     .middleware(LoggingMiddleware);
//! ```

use crate::action::{ActionContext, ApiResponse, DynAction};
use crate::error::BaseError;
use crate::router::ModuleRouter;
use async_trait::async_trait;
use std::sync::Arc;

/// 中间件 trait。
///
/// 实现者在 `handle` 中拦截一次 Action 派发：可在调用 `next.run(ctx)` 前后
/// 注入逻辑，或不调用 `next` 直接短路返回。
#[async_trait]
pub trait Middleware: Send + Sync + 'static {
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
/// 持有尚未执行的中间件切片、对所属 [`ModuleRouter`] 的引用、以及链尾的目标
/// [`DynAction`]。中间件链是**最外层**：当中间件耗尽时，链尾执行
/// 「内置鉴权 + Action 派发」（[`ModuleRouter::authorize_and_dispatch`]），
/// 因此日志、限流、自定义认证等中间件可以观察并干预**所有**请求，
/// 包括会被内置鉴权拒绝的请求。
pub struct Next<'a> {
    /// 尚未执行的中间件
    pub(crate) remaining: &'a [Arc<dyn Middleware>],
    /// 所属路由器（用于链尾的鉴权 + 派发）
    pub(crate) router: &'a ModuleRouter,
    /// 链尾要执行的目标 Action
    pub(crate) action: Arc<dyn DynAction>,
}

impl<'a> Next<'a> {
    /// 推进调用链。
    ///
    /// 若还有中间件，取出第一个执行，并把其余部分包装成新的 `Next` 传入；
    /// 否则执行链尾的「内置鉴权 + Action 派发」。
    ///
    /// # 参数
    ///
    /// - `ctx`: 动作上下文（所有权转移）
    pub async fn run(self, ctx: ActionContext) -> Result<ApiResponse, BaseError> {
        match self.remaining.split_first() {
            Some((current, rest)) => {
                let next = Next {
                    remaining: rest,
                    router: self.router,
                    action: self.action,
                };
                current.handle(ctx, next).await
            }
            None => self.router.authorize_and_dispatch(self.action, ctx).await,
        }
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
/// use yang_base::router::{ModuleRouter, RequestIdMiddleware};
///
/// let router = ModuleRouter::new("user", "用户管理")
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
