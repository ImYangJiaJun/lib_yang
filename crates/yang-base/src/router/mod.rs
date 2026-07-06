//! 路由模块
//!
//! 提供模块路由器和应用路由器，负责 Action 的注册和分发。
//!
//! # 主要组件
//!
//! - `ModuleRouter`：模块路由器，管理单个模块的 Action 路由
//! - `AppRouter`：应用路由器，聚合多个模块路由器，提供跨模块的统一分发入口
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::router::{AppRouter, ModuleRouter};
//! use yang_base::table::TableConfig;
//! use std::sync::Arc;
//!
//! // 创建表配置
//! let table_config = Arc::new(TableConfig::new("users"));
//!
//! // 创建模块路由器并注册内置 CRUD Actions
//! let user_router = ModuleRouter::new("user", "用户管理")
//!     .with_table_config(table_config.clone())
//!     .table_typed::<User>()?
//!     .default_permissions(vec!["user:access".to_string()]);
//!
//! // 创建应用路由器并注册模块
//! let app_router = AppRouter::new()
//!     .register_module(user_router)?;
//! ```

mod app_router;
pub mod middleware;
mod module_router;

pub use app_router::AppRouter;
pub use middleware::{Middleware, Next, RequestIdMiddleware};
pub use module_router::ModuleRouter;
pub use module_router::BUILTIN_ACTION_NAMES;

#[cfg(test)]
mod __tests__;
