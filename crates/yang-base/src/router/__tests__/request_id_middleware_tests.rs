//! RequestIdMiddleware 专属测试（TEST-4）
#![cfg(feature = "token")]
//!
//! 验证三条核心路径：
//! - 合法 `X-Request-Id` 头透传
//! - 小写 `x-request-id` 头兼容
//! - 缺失 header 时回退为默认生成值
//!
//! 测试通过构造 Middleware 链（RequestIdMiddleware + CaptureMiddleware）绕过
//! ModuleRouter::authorize_and_dispatch 的鉴权路径，直接验证中间件行为。

use crate::action::{
    ActionContext, ActionMeta, ApiResponse, DynAction, GlobalTools, Request, RequestId,
};
use crate::error::BaseError;
use crate::router::middleware::{Middleware, Next, RequestIdMiddleware};
use crate::router::ModuleRouter;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

// ── 测试辅助 ──

/// 不会触达的 mock action（测试中用 CaptureMiddleware 短路，不会走到这里）。
struct MockAction;

#[async_trait]
impl DynAction for MockAction {
    async fn dispatch(&self, _ctx: ActionContext) -> Result<ApiResponse, BaseError> {
        unreachable!("request_id 测试不应触达 action 派发")
    }
    fn meta(&self) -> &'static ActionMeta {
        unreachable!("request_id 测试不应访问 meta")
    }
}

/// 捕获最终 request_id 的短路径中间件（不调用 next，直接返回）。
struct CaptureMiddleware {
    captured: Arc<std::sync::Mutex<Option<RequestId>>>,
}

#[async_trait]
impl Middleware for CaptureMiddleware {
    async fn handle(&self, ctx: ActionContext, _next: Next<'_>) -> Result<ApiResponse, BaseError> {
        *self.captured.lock().unwrap() = Some(ctx.request_id);
        Ok(ApiResponse::success_value(json!(null), "ok"))
    }
}

/// 构造带 TokenManager 的 GlobalTools（当前 feature 组合下的最小构造路径）。
fn test_tools() -> Arc<GlobalTools> {
    let tm = crate::token::TokenManager::new_symmetric(
        "test_secret_for_request_id_test",
        jsonwebtoken::Algorithm::HS256,
        "test_iss".into(),
        "test_aud".into(),
        3600,
        86400,
    );
    Arc::new(GlobalTools::new(tm))
}

/// 执行一次 RequestIdMiddleware 测试。
///
/// 构造洋葱链：RequestIdMiddleware(外层) -> CaptureMiddleware(内层)，
/// 内层不调用 next，因此不会触达 router 的授权+派发路径。
async fn run_test(request: Request, default_rid: RequestId) -> Option<RequestId> {
    let ctx = ActionContext::new(request, test_tools()).with_request_id(default_rid);

    let captured = Arc::new(std::sync::Mutex::new(None));
    let mw: Arc<dyn Middleware> = Arc::new(CaptureMiddleware {
        captured: captured.clone(),
    });
    let remaining = [mw];
    let router = ModuleRouter::new("test", "测试");

    let next = Next {
        remaining: &remaining,
        router: &router,
        action: Arc::new(MockAction),
    };

    RequestIdMiddleware
        .handle(ctx, next)
        .await
        .expect("RequestIdMiddleware 不应返回错误");

    // 将 MutexGuard 的生命周期与 captured 分开，避免临时值生命周期问题
    let result = captured.lock().unwrap().take();
    result
}

// ── 测试用例 ──

/// 合法 `X-Request-Id` 头应透传到 ActionContext.request_id。
#[tokio::test]
async fn test_request_id_middleware_propagates_header() {
    let rid = RequestId::from_u128(0xdead_beef);
    let request = Request::new(json!({})).header("X-Request-Id", rid.to_string());
    let captured = run_test(request, RequestId::from_u128(0)).await;
    assert_eq!(captured, Some(rid));
}

/// 小写 `x-request-id` 头也应被识别并透传。
#[tokio::test]
async fn test_request_id_middleware_lowercase_tolerant() {
    let rid = RequestId::from_u128(0xdead_beef);
    let request = Request::new(json!({})).header("x-request-id", rid.to_string());
    let captured = run_test(request, RequestId::from_u128(0)).await;
    assert_eq!(captured, Some(rid));
}

/// 全零 request_id 是无效哨兵值，不应覆盖 ActionContext 已生成的 request_id。
#[tokio::test]
async fn test_request_id_middleware_rejects_zero_header() {
    let default_rid = RequestId::from_u128(0xcafe_babe_0000_0000_0000_0000_0000_0001);
    let request =
        Request::new(json!({})).header("X-Request-Id", "00000000000000000000000000000000");
    let captured = run_test(request, default_rid).await;
    assert_eq!(captured, Some(default_rid));
}

/// 无 `X-Request-Id` 头时应保留默认生成的 request_id。
#[tokio::test]
async fn test_request_id_middleware_missing_header_fallback() {
    let default_rid = RequestId::from_u128(0xcafe_babe_0000_0000_0000_0000_0000_0001);
    let request = Request::new(json!({}));
    let captured = run_test(request, default_rid).await;
    assert_eq!(captured, Some(default_rid));
}
