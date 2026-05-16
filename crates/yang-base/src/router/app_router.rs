//! AppRouter - 应用路由器
//!
//! 聚合多个 `ModuleRouter`，提供跨模块的统一请求分发入口。
//!
//! # 主要功能
//!
//! - 注册多个模块路由器
//! - 根据模块名称和 Action 名称分发请求
//! - 模块不存在时返回结构化错误
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
//! // 创建模块路由器
//! let user_router = ModuleRouter::new("user", "用户管理")
//!     .with_table_config(table_config.clone())
//!     .register_builtin_actions()?;
//!
//! // 创建应用路由器并注册模块
//! let app_router = AppRouter::new()
//!     .register_module(user_router);
//!
//! // 分发请求
//! let response = app_router.dispatch("user", "add", context).await?;
//! ```

use crate::action::{ActionContext, ApiResponse};
use crate::error::BaseError;
use crate::router::ModuleRouter;
use std::collections::HashMap;

/// AppRouter - 应用路由器
///
/// 聚合多个 `ModuleRouter`，提供跨模块的统一请求分发入口。
///
/// # 线程安全
///
/// `AppRouter` 自动满足 `Send + Sync`，因为：
/// - `HashMap<String, ModuleRouter>` 在 `ModuleRouter: Send + Sync` 时满足 `Send + Sync`
/// - `ModuleRouter` 中的 `Box<dyn Action>` 要求 `Action: Send + Sync`
///
/// # 字段
///
/// - `modules`: 模块路由器注册表，Key 为模块名称
pub struct AppRouter {
    /// 模块路由器注册表
    /// Key: 模块名称, Value: ModuleRouter 实例
    modules: HashMap<String, ModuleRouter>,
}

impl AppRouter {
    /// 创建新的应用路由器
    ///
    /// # 返回
    ///
    /// - 新的 `AppRouter` 实例（模块注册表为空）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::router::AppRouter;
    ///
    /// let app_router = AppRouter::new();
    /// ```
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
        }
    }

    /// 注册模块路由器
    ///
    /// 将 `ModuleRouter` 注册到应用路由器中，使用模块名称作为 Key。
    /// 若同名模块已存在，则覆盖旧的注册。
    ///
    /// # 参数
    ///
    /// - `router`: 要注册的模块路由器
    ///
    /// # 返回
    ///
    /// - 修改后的 `AppRouter` 实例（支持链式调用）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::router::{AppRouter, ModuleRouter};
    ///
    /// let app_router = AppRouter::new()
    ///     .register_module(ModuleRouter::new("user", "用户管理"))
    ///     .register_module(ModuleRouter::new("order", "订单管理"));
    /// ```
    pub fn register_module(mut self, router: ModuleRouter) -> Self {
        let module_name = router.module_name().to_string();
        self.modules.insert(module_name, router);
        self
    }

    /// 获取已注册的模块名称列表
    ///
    /// # 返回
    ///
    /// - 所有已注册模块的名称列表（顺序不保证）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::router::{AppRouter, ModuleRouter};
    ///
    /// let app_router = AppRouter::new()
    ///     .register_module(ModuleRouter::new("user", "用户管理"))
    ///     .register_module(ModuleRouter::new("order", "订单管理"));
    ///
    /// let names = app_router.module_names();
    /// assert_eq!(names.len(), 2);
    /// ```
    pub fn module_names(&self) -> Vec<String> {
        self.modules.keys().cloned().collect()
    }

    /// 分发请求到对应模块的 Action
    ///
    /// 根据模块名称找到对应的 `ModuleRouter`，再由其分发到具体的 Action。
    ///
    /// # 参数
    ///
    /// - `module_name`: 目标模块名称
    /// - `action_name`: 目标 Action 名称
    /// - `context`: Action 执行上下文
    ///
    /// # 返回
    ///
    /// - `Ok(ApiResponse)`: 执行成功
    /// - `Err(BaseError)`: 执行失败
    ///
    /// # 错误
    ///
    /// - `BaseError::ActionNotFound`: 模块不存在（错误信息包含模块名称）
    /// - `BaseError::ActionNotFound`: Action 不存在（由 ModuleRouter 返回）
    /// - `BaseError::Unauthorized`: 用户未认证
    /// - `BaseError::PermissionDenied`: 权限不足
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::router::{AppRouter, ModuleRouter};
    /// use yang_base::action::ActionContext;
    ///
    /// let app_router = AppRouter::new()
    ///     .register_module(ModuleRouter::new("user", "用户管理"));
    ///
    /// // 模块不存在时返回错误
    /// let result = app_router.dispatch("unknown", "add", context).await;
    /// assert!(matches!(result, Err(BaseError::ActionNotFound(_))));
    /// ```
    pub async fn dispatch(
        &self,
        module_name: &str,
        action_name: &str,
        context: ActionContext,
    ) -> Result<ApiResponse, BaseError> {
        // 查找目标模块，不存在时返回结构化错误
        let module_router = self.modules.get(module_name).ok_or_else(|| {
            BaseError::ActionNotFound(format!("模块不存在: {}", module_name))
        })?;

        // 委托给模块路由器分发
        module_router.dispatch(action_name, context).await
    }
}

/// 实现 `Default` trait，委托给 `new()`
impl Default for AppRouter {
    fn default() -> Self {
        Self::new()
    }
}
