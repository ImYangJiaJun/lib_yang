//! Action 系统模块
//!
//! 提供 Action 系统的核心组件，包括请求、响应、上下文和内置 CRUD Actions。
//!
//! # 主要组件
//!
//! - `Request`：Action 请求结构，封装 HTTP 请求信息
//! - `ApiResponse`：统一的 API 响应格式
//! - `ActionContext`：Action 执行上下文
//! - `Action`：Action trait，定义 action 的基本行为
//! - `builtin`：内置 CRUD Actions 模块
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::action::{Request, ApiResponse, Action, ActionContext};
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
//! // 使用内置 Action
//! let table_config = Arc::new(TableConfig::new("users"));
//! let add_action = AddAction::new(table_config);
//! ```

mod action_trait;
pub mod builtin;
mod context;
pub mod meta;
mod request;
mod response;
pub mod typed;

pub use action_trait::{Action, Permission};
pub use context::{ActionContext, GlobalTools, User};
pub use meta::ActionMeta;
pub use request::Request;
pub use response::ApiResponse;
pub use typed::{DynAction, TypedAction, TypedHandler};

#[cfg(test)]
mod __tests__;
