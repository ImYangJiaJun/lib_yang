//! Action 系统模块
//!
//! 提供 Action 系统的核心组件，包括请求、响应、上下文和内置 CRUD Actions。
//!
//! # 主要组件
//!
//! - `Request`：Action 请求结构，封装 HTTP 请求信息
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
//! use yang_base::table::TableConfig;
//! use serde_json::json;
//! use std::sync::Arc;
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
//! // 使用内置 Action（表配置在 dispatch 时由 ActionContext 提供）
//! let _table_config = Arc::new(TableConfig::new("users"));
//! // let add_action = AddAction::<User>::new();
//! ```

mod action_trait;
#[cfg(feature = "token")]
pub mod auth;
pub mod builtin;
mod context;
pub mod meta;
mod request;
mod request_id;
mod response;
pub mod sql_bridge;
pub mod typed;

pub use action_trait::{Permission, PermissionMode};
#[cfg(feature = "token")]
pub use auth::{
    AuthAuditEvent, AuthAuditHook, CredentialVerifier, DefaultRefreshClaims, LoginAction,
    LogoutAction, RefreshAction, RefreshClaimsResolver, TokenAuthMiddleware, TracingAuditHook,
};
pub use context::{ActionContext, GlobalTools, User};
pub use meta::ActionMeta;
pub use request::Request;
pub use request_id::RequestId;
pub use response::ApiResponse;
pub use typed::{DynAction, TypedAction, TypedHandler};

#[cfg(test)]
mod __tests__;
