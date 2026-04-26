//! 路由模块
//!
//! 提供模块路由器，负责 Action 的注册和分发。
//!
//! # 主要组件
//!
//! - `ModuleRouter`：模块路由器，管理单个模块的 Action 路由
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::router::ModuleRouter;
//! use yang_base::action::builtin::AddAction;
//! use yang_base::table::TableConfig;
//! use std::sync::Arc;
//!
//! // 创建表配置
//! let table_config = Arc::new(TableConfig::new("users"));
//!
//! // 创建模块路由器
//! let router = ModuleRouter::new("user", "用户管理")
//!     .table_config(table_config.clone())
//!     .register_builtin_actions()
//!     .default_permissions(vec!["user:access".to_string()]);
//! ```

mod module_router;

pub use module_router::ModuleRouter;

#[cfg(test)]
mod __tests__;
