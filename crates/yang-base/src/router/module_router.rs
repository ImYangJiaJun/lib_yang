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
//!     .table_typed::<User>()?;
//!
//! // 分发请求
//! let response = router.dispatch("add", context).await?;
//! ```

/// 内置 Action 名称常量
///
/// 包含所有内置 Action 的名称，用于注册和验证
pub const BUILTIN_ACTION_NAMES: &[&str] = &["add", "put", "del", "get", "select", "table"];

use crate::action::{ActionContext, ApiResponse, DynAction, User};
use crate::error::BaseError;
use crate::router::middleware::{Middleware, Next};
use crate::table::TableConfig;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::Instrument;

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
    /// Key: action 名称, Value: 类型化擦除后的 DynAction 实例
    actions: HashMap<String, Arc<dyn DynAction>>,

    /// 默认权限要求
    /// 所有 Action 都需要满足这些权限（除非 Action 是公开的）
    default_permissions: Vec<String>,

    /// 中间件链（按注册顺序构成洋葱模型，先注册的最先进入、最后离开）
    middlewares: Vec<Arc<dyn Middleware>>,
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
            middlewares: Vec::new(),
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

    /// 注册一个类型化 Action
    ///
    /// 接受任意实现 [`TypedAction`](crate::action::TypedAction) 的类型
    /// （通常由 `#[derive(Action)]` 派生），通过 blanket impl 自动转为
    /// `Arc<dyn DynAction>` 存入注册表。
    ///
    /// 若已存在同名 Action（含内置 CRUD：add/put/del/get/select/table），
    /// 将覆盖先前注册。
    ///
    /// # 参数
    ///
    /// - `action`: 类型化 Action 实例
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
    ///
    /// let router = ModuleRouter::new("user", "用户管理")
    ///     .register_action(AddAction::<User>::new());
    /// ```
    pub fn register_action<A>(mut self, action: A) -> Self
    where
        A: crate::action::TypedAction,
    {
        let action: Arc<dyn DynAction> = Arc::new(action);
        let name = action.meta().name.to_string();
        self.actions.insert(name, action);
        self
    }

    /// 注册一个中间件（builder setter）
    ///
    /// 中间件按注册顺序构成洋葱模型：先注册的最先进入、最后离开。
    /// 中间件位于鉴权之外的**最外层**，先于内置登录/权限检查运行
    /// （见 [`dispatch`](Self::dispatch)）。这意味着中间件的短路返回会
    /// **跳过**内置鉴权，请谨慎使用。
    ///
    /// # 参数
    ///
    /// - `middleware`: 实现 [`Middleware`](crate::router::Middleware) 的实例
    ///
    /// # 返回
    ///
    /// - 修改后的 ModuleRouter 实例（支持链式调用）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let router = ModuleRouter::new("user", "用户管理")
    ///     .middleware(LoggingMiddleware)
    ///     .middleware(RateLimitMiddleware);
    /// ```
    pub fn middleware<M>(mut self, middleware: M) -> Self
    where
        M: Middleware,
    {
        self.middlewares.push(Arc::new(middleware));
        self
    }

    /// 为指定实体类型 `T` 注册全部六个内置 CRUD Actions
    ///
    /// 注册 add、put、del、get、select、table 六个类型化内置 Action。
    /// 调用前需先通过 `with_table_config` / `table_config` 设置表配置——
    /// 内置 Action 在 dispatch 时通过 `ActionContext::table_query()` 取用该配置。
    ///
    /// 内置 Action 名称由 [`BUILTIN_ACTION_NAMES`] 常量定义。
    ///
    /// # 类型参数
    ///
    /// - `T`: 实现 [`TableEntity`](crate::table::TableEntity) 的实体类型
    ///   （通常由 `#[derive(TableEntity)]` 派生）
    ///
    /// # 返回
    ///
    /// - `Ok(Self)`: 注册成功，返回修改后的 ModuleRouter 实例（支持链式调用）
    /// - `Err(BaseError::TableConfigNotSet)`: 未设置 table_config
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::router::ModuleRouter;
    ///
    /// let router = ModuleRouter::new("user", "用户管理")
    ///     .with_table_config(user_table_config)
    ///     .table_typed::<User>()?;
    /// ```
    #[cfg(feature = "mysql")]
    pub fn table_typed<T>(self) -> Result<Self, BaseError>
    where
        T: crate::table::TableEntity,
    {
        use crate::action::builtin::{
            AddAction, DelAction, GetAction, PutAction, SelectAction, TableAction,
        };

        if self.table_config.is_none() {
            return Err(BaseError::TableConfigNotSet);
        }

        Ok(self
            .register_action(AddAction::<T>::new())
            .register_action(PutAction::<T>::new())
            .register_action(DelAction::<T>::new())
            .register_action(GetAction::<T>::new())
            .register_action(SelectAction::<T>::new())
            .register_action(TableAction::<T>::new()))
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
    ///     .table_typed::<User>()?;
    ///
    /// let response = router.dispatch("add", context).await?;
    /// ```
    pub async fn dispatch(
        &self,
        action_name: &str,
        mut context: ActionContext,
    ) -> Result<ApiResponse, BaseError> {
        // 1. 查找 Action（克隆 Arc，便于后续移动进中间件链 / 执行）
        let action = self
            .actions
            .get(action_name)
            .ok_or_else(|| BaseError::ActionNotFound(action_name.to_string()))?
            .clone();

        // 2. 设置表配置到上下文
        if let Some(table_config) = &self.table_config {
            context = context.with_table_config(table_config.clone());
        }

        // 3. 进入中间件链（最外层）。链尾执行内置鉴权 + Action 派发，
        //    使日志/限流/自定义认证等中间件能观察并干预所有请求。
        let next = Next {
            remaining: &self.middlewares,
            router: self,
            action,
        };

        // 根 span：串联整条派发链路。静态 span 名 + 借用字段，成功路径零分配；
        // request_id 先以 Empty 占位，由 RequestIdMiddleware 在链内 record 透传值。
        let span = tracing::info_span!(
            "dispatch",
            module = %self.module_name,
            action = %action_name,
            request_id = %context.request_id,
        );
        next.run(context).instrument(span).await
    }

    /// 中间件链的终点：执行内置鉴权后派发 Action。
    ///
    /// 由 [`Next::run`] 在中间件耗尽时调用。公开 Action 跳过鉴权；非公开
    /// Action 依次校验「已登录 -> 模块默认权限 -> Action 权限」，全部通过后
    /// 调用 `action.dispatch`。
    pub(crate) async fn authorize_and_dispatch(
        &self,
        action: Arc<dyn DynAction>,
        context: ActionContext,
    ) -> Result<ApiResponse, BaseError> {
        let meta = action.meta();

        // 鉴权子 span：记录是否公开 Action 与最终放行结果
        let span = tracing::info_span!(
            "authorize",
            is_public = meta.is_public,
            granted = tracing::field::Empty,
        );
        let _enter = span.enter();

        // 公开 Action 跳过登录/权限检查
        if !meta.is_public {
            // 检查用户是否已认证
            let user = context.user.as_ref().ok_or_else(|| {
                span.record("granted", false);
                BaseError::Unauthorized("需要登录".to_string())
            })?;

            // 检查默认权限
            if !self.default_permissions.is_empty()
                && !self.check_permissions(user, &self.default_permissions)
            {
                span.record("granted", false);
                return Err(BaseError::PermissionDenied(format!(
                    "缺少模块权限: {:?}",
                    self.default_permissions
                )));
            }

            // 检查 Action 权限
            //
            // 热路径零分配：成功路径仅借用权限名做 `all` 判断，不构造任何
            // 中间集合；仅当权限不足时才 collect 格式化中文错误信息。
            if !meta.permissions.is_empty()
                && !meta
                    .permissions
                    .iter()
                    .all(|p| user.has_permission(p.name()))
            {
                span.record("granted", false);
                let permission_names: Vec<&str> =
                    meta.permissions.iter().map(|p| p.name()).collect();
                return Err(BaseError::PermissionDenied(format!(
                    "缺少 Action 权限: {:?}",
                    permission_names
                )));
            }
        }

        span.record("granted", true);
        // 释放鉴权 span，进入 handler 自身的 span（在 DynAction::dispatch 内开启）
        drop(_enter);

        // 执行 Action 派发
        action.dispatch(context).await
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
    ///     .table_typed::<User>()?;
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
