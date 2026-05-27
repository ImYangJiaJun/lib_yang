//! ModuleRouter - 模块路由器
//!
//! 管理单个模块的表配置和 Action 路由，负责 Action 的注册、查找和分发。
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::router::ModuleRouter;
//! use yang_base::table::{TableConfig, FieldConfig, FieldType};
//! use std::sync::Arc;
//!
//! // 创建表配置
//! let table_config = Arc::new(
//!     TableConfig::new("users")
//!         .field(FieldConfig::new("id", FieldType::Integer))
//!         .field(FieldConfig::new("username", FieldType::String { max_length: 50 }))
//! );
//!
//! // 创建模块路由器并注册内置 CRUD Actions
//! let router = ModuleRouter::new("user", "用户管理")
//!     .with_table_config(table_config.clone())
//!     .default_permissions(vec!["user:access".to_string()])
//!     .register_builtin_actions()?;
//!
//! // 分发请求
//! let response = router.dispatch("add", context).await?;
//! ```

/// 内置 Action 名称常量
///
/// 包含所有内置 Action 的名称，用于注册和验证
pub const BUILTIN_ACTION_NAMES: &[&str] = &["add", "put", "del", "get", "select", "table"];

use crate::action::{Action, ActionContext, ApiResponse, User};
use crate::error::BaseError;
use crate::table::TableConfig;
use std::collections::HashMap;
use std::sync::Arc;

/// ModuleRouter - 模块路由器
///
/// 管理单个模块的表配置和 Action 路由。
///
/// # 字段
///
/// - `module_name`: 模块名称
/// - `display_name`: 模块显示名称
/// - `table_config`: 表配置（可选）
/// - `actions`: Action 注册表
/// - `default_permissions`: 默认权限要求
pub struct ModuleRouter {
    /// 模块名称
    module_name: String,

    /// 模块显示名称
    display_name: String,

    /// 表配置（如果模块关联表）
    table_config: Option<Arc<TableConfig>>,

    /// Action 注册表
    /// Key: action 名称, Value: Action 实例
    actions: HashMap<String, Box<dyn Action>>,

    /// 默认权限要求
    /// 所有 Action 都需要满足这些权限（除非 Action 是公开的）
    default_permissions: Vec<String>,
}

impl ModuleRouter {
    /// 创建新的模块路由器
    ///
    /// # 参数
    ///
    /// - `module_name`: 模块名称
    /// - `display_name`: 模块显示名称
    ///
    /// # 返回
    ///
    /// - 新的 ModuleRouter 实例
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::router::ModuleRouter;
    ///
    /// let router = ModuleRouter::new("user", "用户管理");
    /// ```
    pub fn new(module_name: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            module_name: module_name.into(),
            display_name: display_name.into(),
            table_config: None,
            actions: HashMap::new(),
            default_permissions: Vec::new(),
        }
    }

    /// 设置表配置（builder setter）
    ///
    /// # 参数
    ///
    /// - `config`: 表配置
    ///
    /// # 返回
    ///
    /// - 修改后的 ModuleRouter 实例（支持链式调用）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::router::ModuleRouter;
    /// use yang_base::table::TableConfig;
    /// use std::sync::Arc;
    ///
    /// let table_config = Arc::new(TableConfig::new("users"));
    /// let router = ModuleRouter::new("user", "用户管理")
    ///     .with_table_config(table_config);
    /// ```
    pub fn with_table_config(mut self, config: Arc<TableConfig>) -> Self {
        self.table_config = Some(config);
        self
    }

    /// 设置表配置（链式 setter 别名，委托给 `with_table_config`）
    ///
    /// 与 `with_table_config` 功能相同，提供更简洁的链式调用语法。
    ///
    /// # 参数
    ///
    /// - `config`: 表配置
    ///
    /// # 返回
    ///
    /// - 修改后的 ModuleRouter 实例（支持链式调用）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::router::ModuleRouter;
    /// use yang_base::table::TableConfig;
    /// use std::sync::Arc;
    ///
    /// let table_config = Arc::new(TableConfig::new("users"));
    /// let router = ModuleRouter::new("user", "用户管理")
    ///     .table_config(table_config);
    /// ```
    pub fn table_config(self, config: Arc<TableConfig>) -> Self {
        self.with_table_config(config)
    }

    /// 设置默认权限要求
    ///
    /// # 参数
    ///
    /// - `permissions`: 权限列表
    ///
    /// # 返回
    ///
    /// - 修改后的 ModuleRouter 实例（支持链式调用）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::router::ModuleRouter;
    ///
    /// let router = ModuleRouter::new("user", "用户管理")
    ///     .default_permissions(vec!["user:access".to_string()]);
    /// ```
    pub fn default_permissions(mut self, permissions: Vec<String>) -> Self {
        self.default_permissions = permissions;
        self
    }

    /// 注册 Action
    ///
    /// # 参数
    ///
    /// - `action`: Action 实例
    ///
    /// # 返回
    ///
    /// - 修改后的 ModuleRouter 实例（支持链式调用）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::router::ModuleRouter;
    /// use yang_base::action::builtin::AddAction;
    /// use yang_base::table::TableConfig;
    /// use std::sync::Arc;
    ///
    /// let table_config = Arc::new(TableConfig::new("users"));
    /// let add_action = AddAction::new(table_config.clone());
    ///
    /// let router = ModuleRouter::new("user", "用户管理")
    ///     .register_action(add_action);
    /// ```
    pub fn register_action<A: Action + 'static>(mut self, action: A) -> Self {
        let name = action.name().to_string();
        self.actions.insert(name, Box::new(action));
        self
    }

    /// 注册所有内置 CRUD Actions
    ///
    /// 注册 add、put、del、get、select、table 六个内置 Action。
    /// 需要先通过 `with_table_config` 或 `table_config` 设置表配置。
    ///
    /// 内置 Action 名称由 [`BUILTIN_ACTION_NAMES`] 常量定义。
    ///
    /// # 返回
    ///
    /// - `Ok(Self)`: 注册成功，返回修改后的 ModuleRouter 实例（支持链式调用）
    /// - `Err(BaseError::TableConfigNotSet)`: 未设置 table_config
    ///
    /// # 错误
    ///
    /// - `BaseError::TableConfigNotSet`: 调用前未设置表配置
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::router::ModuleRouter;
    /// use yang_base::table::TableConfig;
    /// use std::sync::Arc;
    ///
    /// let table_config = Arc::new(TableConfig::new("users"));
    /// let router = ModuleRouter::new("user", "用户管理")
    ///     .with_table_config(table_config)
    ///     .register_builtin_actions()?;
    /// ```
    #[cfg(feature = "mysql")]
    pub fn register_builtin_actions(self) -> Result<Self, BaseError> {
        // Task 6 之后用 table_typed::<T>() 替换；此方法暂废
        Err(BaseError::Unknown(
            "旧 register_builtin_actions 在 H-1 重构期间禁用，请使用 table_typed::<T>()".into(),
        ))
    }

    /// 分发请求到对应的 Action
    ///
    /// 根据 action 名称查找对应的 Action，检查权限后执行。
    ///
    /// # 参数
    ///
    /// - `action_name`: Action 名称
    /// - `context`: Action 执行上下文
    ///
    /// # 返回
    ///
    /// - `Ok(ApiResponse)`: 执行成功
    /// - `Err(BaseError)`: 执行失败
    ///
    /// # 错误
    ///
    /// - `BaseError::ActionNotFound`: Action 不存在
    /// - `BaseError::Unauthorized`: 用户未认证（需要认证的 Action）
    /// - `BaseError::PermissionDenied`: 权限不足
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::router::ModuleRouter;
    /// use yang_base::action::ActionContext;
    ///
    /// let router = ModuleRouter::new("user", "用户管理")
    ///     .with_table_config(table_config)
    ///     .register_builtin_actions()?;
    ///
    /// let response = router.dispatch("add", context).await?;
    /// ```
    pub async fn dispatch(
        &self,
        action_name: &str,
        mut context: ActionContext,
    ) -> Result<ApiResponse, BaseError> {
        // 1. 查找 Action
        let action = self
            .actions
            .get(action_name)
            .ok_or_else(|| BaseError::ActionNotFound(action_name.to_string()))?;

        // 2. 设置表配置到上下文
        if let Some(table_config) = &self.table_config {
            context = context.with_table_config(table_config.clone());
        }

        // 3. 检查是否为公开 Action
        if action.is_public() {
            // 公开 Action，直接执行
            return action.execute(context).await;
        }

        // 4. 检查用户是否已认证
        let user = context
            .user
            .as_ref()
            .ok_or_else(|| BaseError::Unauthorized("需要登录".to_string()))?;

        // 5. 检查默认权限
        if !self.default_permissions.is_empty()
            && !self.check_permissions(user, &self.default_permissions)
        {
            return Err(BaseError::PermissionDenied(format!(
                "缺少模块权限: {:?}",
                self.default_permissions
            )));
        }

        // 6. 检查 Action 权限
        let action_permissions = action.permissions();
        if !action_permissions.is_empty() {
            let permission_names: Vec<String> = action_permissions
                .iter()
                .map(|p| p.name().to_string())
                .collect();

            if !self.check_permissions(user, &permission_names) {
                return Err(BaseError::PermissionDenied(format!(
                    "缺少 Action 权限: {:?}",
                    permission_names
                )));
            }
        }

        // 7. 执行 Action
        action.execute(context).await
    }

    /// 使用全局单例分发请求
    ///
    /// 自动从全局单例获取 `GlobalTools`，无需手动传入。
    /// 需要先调用 `GlobalTools::init` 初始化全局单例。
    ///
    /// # 参数
    ///
    /// - `action_name`: Action 名称
    /// - `request`: 请求数据
    ///
    /// # 返回
    ///
    /// - `Ok(ApiResponse)`: 执行成功
    /// - `Err(BaseError)`: 执行失败
    ///
    /// # 错误
    ///
    /// - `BaseError::ConfigError("GlobalTools 未初始化")`: 全局单例未初始化
    /// - `BaseError::ActionNotFound`: Action 不存在
    /// - `BaseError::Unauthorized`: 用户未认证
    /// - `BaseError::PermissionDenied`: 权限不足
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::router::ModuleRouter;
    /// use yang_base::action::{GlobalTools, Request};
    ///
    /// // 初始化全局单例
    /// GlobalTools::init(token_manager)?;
    ///
    /// let router = ModuleRouter::new("user", "用户管理")
    ///     .with_table_config(table_config)
    ///     .register_builtin_actions()?;
    ///
    /// // 无需传入 tools，自动从全局单例获取
    /// let response = router.dispatch_with_global("add", request).await?;
    /// ```
    pub async fn dispatch_with_global(
        &self,
        action_name: &str,
        request: crate::action::Request,
    ) -> Result<ApiResponse, BaseError> {
        // 从全局单例获取 GlobalTools，自动构建上下文
        let context = ActionContext::new_with_global_tools(request)?;
        self.dispatch(action_name, context).await
    }

    /// 检查用户权限
    ///
    /// # 参数
    ///
    /// - `user`: 用户信息
    /// - `required_permissions`: 需要的权限列表
    ///
    /// # 返回
    ///
    /// - `true`: 有权限
    /// - `false`: 无权限
    fn check_permissions(&self, user: &User, required_permissions: &[String]) -> bool {
        // 检查用户是否有所有需要的权限
        required_permissions
            .iter()
            .all(|perm| user.has_permission(perm))
    }

    /// 获取模块名称
    ///
    /// # 返回
    ///
    /// - 模块名称
    pub fn module_name(&self) -> &str {
        &self.module_name
    }

    /// 获取模块显示名称
    ///
    /// # 返回
    ///
    /// - 模块显示名称
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// 获取已注册的 Action 列表
    ///
    /// # 返回
    ///
    /// - Action 名称列表
    pub fn action_names(&self) -> Vec<String> {
        self.actions.keys().cloned().collect()
    }

    /// 获取表配置（getter，返回借用）
    ///
    /// # 返回
    ///
    /// - `Some(&Arc<TableConfig>)`: 表配置的借用
    /// - `None`: 未设置表配置
    pub fn get_table_config(&self) -> Option<&Arc<TableConfig>> {
        self.table_config.as_ref()
    }
}
