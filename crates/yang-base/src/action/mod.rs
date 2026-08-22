//! Action 系统模块
//!
//! 提供 Action 系统的核心组件，包括请求、响应、上下文和内置 CRUD Actions。
//!
//! # 主要组件
//!
//! - `Request`：Action 请求结构，封装 HTTP 请求信息
//! - `RequestMeta`：独立、transport-neutral 的 method/URI/address 元数据
//! - `ApiResponse`：统一的 API 响应格式
//! - `ActionContext`：Action 执行上下文
//! - Action 行为契约：已迁移到 `typed.rs` 的 `TypedHandler` / `TypedAction` / `DynAction`
//!   三层 trait；`Action` 现为 crate 根导出的派生宏（`#[derive(Action)]`），用于生成元数据
//! - `builtin`：内置 CRUD Actions 模块
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::action::{Request, ApiResponse, ActionContext};
//! use yang_base::action::builtin::AddAction;
//! use yang_base::table::Record;
//! use serde_json::json;
//!
//! // 创建请求
//! let request = Request::new(json!({
//!     "username": "alice",
//!     "email": "alice@example.com"
//! }))
//! .header("Authorization", "Bearer token123")
//! .query("page", "1")
//! .path_param("id", "123");
//!
//! // 提取 Token
//! if let Some(token) = request.token() {
//!     println!("Token: {}", token);
//! }
//!
//! // 创建成功响应
//! let response = ApiResponse::success(
//!     json!({ "id": 123, "name": "Alice" }),
//!     "操作成功"
//! );
//!
//! // 创建失败响应
//! let response = ApiResponse::fail(400001, "参数错误");
//!
//! // 内置 Action 直接接收 Record；表定义在 dispatch 时由模块注入。
//! let input = Record::new().set("username", "alice");
//! let add_action = AddAction::new();
//! ```

mod action_trait;
#[cfg(feature = "token")]
pub mod auth;
pub mod builtin;
mod context;
pub(crate) mod functional;
pub mod meta;
mod request;
mod request_context;
mod request_id;
mod request_meta;
mod response;
pub mod sql_bridge;
#[cfg(feature = "token")]
mod step_up;
mod tenant;
pub mod typed;
mod ui_catalog;
mod upload;

pub use action_trait::{Permission, PermissionMode};
#[cfg(feature = "token")]
pub use auth::{
    AuthAuditEvent, AuthAuditHook, CredentialVerifier, DefaultRefreshClaims, LoginAction,
    LogoutAction, NoopTokenClaimsValidator, RefreshAction, RefreshClaimsResolver,
    TokenAuthMiddleware, TokenClaimsValidator, TracingAuditHook,
};
pub use context::{ActionContext, User};
pub use meta::ActionMeta;
pub use request::Request;
pub use request_context::{
    ActorContext, ContextKey, RequestContext, SystemTenantCapability, TenantContext, TenantId,
};
pub use request_id::RequestId;
pub use request_meta::RequestMeta;
pub use response::{ApiResponse, ResponseAttachment, ResponseBody};
#[cfg(feature = "token")]
pub use step_up::{
    InMemoryStepUpProofStore, RedisStepUpProofStore, StepUpChallenge, StepUpCompleteAction,
    StepUpCompleteInput, StepUpManager, StepUpMiddleware, StepUpProof, StepUpProofStore,
    StepUpResourceResolver, StepUpVerification, DEFAULT_STEP_UP_CHALLENGE_TTL,
    DEFAULT_STEP_UP_PROOF_TTL, STEP_UP_PROOF_HEADER,
};
pub use tenant::{TenantResolution, TenantResolver, TenantResolverMiddleware, TENANT_ID_HEADER};
pub use typed::{Action, DynAction, TypedAction, TypedHandler};
pub use ui_catalog::{UiCatalogAction, UiCatalogInput};
pub use upload::UploadedFile;

#[cfg(test)]
mod __tests__;
