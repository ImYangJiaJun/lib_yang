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
//! use yang_base::table::{Field, Table};
//!
//! let users = Table::new("users")
//!     .fields(vec![Field::id("id"), Field::string("username", 64).required()])
//!     .build()?;
//!
//! // 创建模块路由器
//! let user_router = ModuleRouter::new("user", "用户管理")
//!     .table(users)
//!     .crud()?;
//!
//! // 创建应用路由器并注册模块
//! let app_router = AppRouter::new()
//!     .module(user_router)?;
//!
//! // 分发请求
//! let response = app_router.dispatch("user", "add", context).await?;
//! ```

use crate::action::{ActionContext, ApiResponse};
use crate::error::BaseError;
use crate::router::catalog::RoutePatternRegistry;
use crate::router::ModuleRouter;
use crate::router::{ApiCatalog, ModuleDescriptor};
use crate::table::TableDefinition;
use std::collections::{HashMap, HashSet};

/// AppRouter - 应用路由器
///
/// 聚合多个 `ModuleRouter`，提供跨模块的统一请求分发入口。
///
/// # 线程安全
///
/// `AppRouter` 自动满足 `Send + Sync`，因为：
/// - `HashMap<String, ModuleRouter>` 在 `ModuleRouter: Send + Sync` 时满足 `Send + Sync`
/// - `ModuleRouter` 中的 `Arc<dyn DynAction>` 要求 `DynAction: Send + Sync`
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
    /// 若同名模块已存在，则返回 `BaseError::ConfigError`，不覆盖旧的注册。
    ///
    /// # 参数
    ///
    /// - `router`: 要注册的模块路由器
    ///
    /// # 返回
    ///
    /// - `Ok(AppRouter)`: 修改后的实例（支持链式调用）
    /// - `Err(BaseError::ConfigError)`: 模块名重复
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::router::{AppRouter, ModuleRouter};
    ///
    /// let app_router = AppRouter::new()
    ///     .modules([
    ///         ModuleRouter::new("user", "用户管理"),
    ///         ModuleRouter::new("order", "订单管理"),
    ///     ])?;
    /// ```
    pub fn module(mut self, router: ModuleRouter) -> Result<Self, BaseError> {
        let module_name = router.module_name().to_string();
        if module_name.trim().is_empty() {
            return Err(BaseError::ConfigError("模块名称不能为空".to_string()));
        }
        if self.modules.contains_key(&module_name) {
            return Err(BaseError::ConfigError(format!(
                "模块已注册: {}",
                module_name
            )));
        }
        self.modules.insert(module_name, router);
        Ok(self)
    }

    /// 批量注册模块，最终只需处理一次构建错误。
    pub fn modules<I>(mut self, modules: I) -> Result<Self, BaseError>
    where
        I: IntoIterator<Item = ModuleRouter>,
    {
        for module in modules {
            self = self.module(module)?;
        }
        Ok(self)
    }

    /// 获取已注册的模块名称列表
    ///
    /// # 返回
    ///
    /// - 所有已注册模块的名称列表（按名称排序）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::router::{AppRouter, ModuleRouter};
    ///
    /// let app_router = AppRouter::new()
    ///     .modules([
    ///         ModuleRouter::new("user", "用户管理"),
    ///         ModuleRouter::new("order", "订单管理"),
    ///     ])?;
    ///
    /// let names = app_router.module_names();
    /// assert_eq!(names.len(), 2);
    /// ```
    pub fn module_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.modules.keys().cloned().collect();
        names.sort();
        names
    }

    /// 返回各模块声明的表配置，按模块名称确定性排序。
    ///
    /// 没有关联表的模块会被跳过。返回借用而非克隆，供数据库初始化器在启动期
    /// 汇总模块 schema；同一表名的重复/冲突声明由同步器统一校验。
    pub fn table_definitions(&self) -> Vec<&TableDefinition> {
        let mut names: Vec<&String> = self.modules.keys().collect();
        names.sort();
        names
            .into_iter()
            .flat_map(|name| self.modules[name].schema_definitions())
            .collect()
    }

    /// 构建确定性排序的只读 API Catalog，并校验跨模块 route/operation 冲突。
    pub fn catalog(&self) -> Result<ApiCatalog, BaseError> {
        let mut names: Vec<&String> = self.modules.keys().collect();
        names.sort();
        let mut modules: Vec<ModuleDescriptor> = Vec::with_capacity(names.len());
        let mut routes = RoutePatternRegistry::default();
        let mut operations = HashSet::new();
        for name in names {
            let descriptor = self.modules[name].descriptor()?;
            for action in &descriptor.actions {
                routes.insert(&action.route)?;
                if !operations.insert(action.route.operation_id.clone()) {
                    return Err(BaseError::ConfigError(format!(
                        "operation_id 冲突: {}",
                        action.route.operation_id
                    )));
                }
            }
            modules.push(descriptor);
        }
        Ok(ApiCatalog { modules })
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
    ///     .module(ModuleRouter::new("user", "用户管理"))?;
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
        let module_router = self
            .modules
            .get(module_name)
            .ok_or_else(|| BaseError::ActionNotFound(format!("模块不存在: {}", module_name)))?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_rejects_duplicate_module_name() {
        let router = AppRouter::new()
            .module(ModuleRouter::new("user", "用户管理"))
            .expect("首次注册 user 模块应成功");

        let result = router.module(ModuleRouter::new("user", "重复用户模块"));

        assert!(matches!(
            result,
            Err(BaseError::ConfigError(msg)) if msg.contains("模块已注册: user")
        ));
    }

    #[test]
    fn test_module_rejects_blank_module_name() {
        let result = AppRouter::new().module(ModuleRouter::new("   ", "空白模块"));

        assert!(matches!(
            result,
            Err(BaseError::ConfigError(msg)) if msg.contains("模块名称不能为空")
        ));
    }
}
