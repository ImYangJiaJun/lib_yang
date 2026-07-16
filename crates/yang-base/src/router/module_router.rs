//! ModuleRouter - 模块路由器
//!
//! 管理单个模块的表定义与 API，负责注册、查找和分发。
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::router::ModuleRouter;
//! use yang_base::table::{Field, Table};
//!
//! let users = Table::new("users")
//!     .fields(vec![
//!         Field::id("id"),
//!         Field::string("username", 50).required().unique(),
//!     ])
//!     .build()?;
//!
//! // 创建模块路由器并注册内置 CRUD Actions
//! let router = ModuleRouter::new("user", "用户管理")
//!     .table(users)
//!     .crud()?;
//!
//! // 分发请求
//! let response = router.dispatch("add", context).await?;
//! ```

/// 内置 Action 名称常量
///
/// 包含所有内置 Action 的名称，用于注册和验证
pub const BUILTIN_ACTION_NAMES: &[&str] = &["add", "put", "del", "get", "select", "table"];

use crate::action::{ActionContext, ApiResponse, DynAction, PermissionMode, User};
use crate::error::BaseError;
use crate::router::catalog::{
    ActionDescriptor, ModuleDescriptor, RouteDescriptor, RoutePatternRegistry,
};
use crate::router::middleware::{Middleware, MiddlewareScope, Next};
use crate::router::Api;
use crate::table::TableDefinition;
use std::collections::{HashMap, HashSet};
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
/// - `table_definition`: 主表定义（可选）
/// - `actions`: Action 注册表
/// - `default_permissions`: 默认权限要求
#[must_use = "builder 返回新实例，忽略将丢失配置"]
pub struct ModuleRouter {
    /// 模块名称
    module_name: String,

    /// 模块显示名称
    display_name: String,

    /// 主表定义（如果模块关联表）
    table_definition: Option<TableDefinition>,

    /// 仅参与启动期 schema 汇总的附属表配置。
    schema_tables: Vec<TableDefinition>,

    /// Action 注册表
    /// Key: action 名称, Value: 类型化擦除后的 DynAction 实例
    actions: HashMap<String, Arc<dyn DynAction>>,

    /// Action 名称到传输路由的只读描述源。
    routes: HashMap<String, RouteDescriptor>,

    /// `.crud()` 是否已经注册，以及运行时使用的稳定权限名称。
    ///
    /// 表级 schema 不缓存在这里；[`Self::descriptor`] 始终从当前主表重新投影，
    /// 因而链式调用在 CRUD 注册后替换主表也不会让 Catalog 与运行时漂移。
    #[cfg(feature = "mysql")]
    crud_permissions: Option<CrudPermissions>,

    /// 默认权限要求
    /// 所有 Action 都需要满足这些权限（除非 Action 是公开的）
    default_permissions: Vec<String>,

    /// 默认权限匹配模式（AND / OR）
    default_permission_mode: PermissionMode,

    /// 中间件链（按注册顺序构成洋葱模型，先注册的最先进入、最后离开）
    middlewares: Vec<Arc<dyn Middleware>>,
}

#[cfg(feature = "mysql")]
struct CrudPermissions {
    read: String,
    write: String,
}

impl ModuleRouter {
    /// 创建新的模块路由器
    ///
    /// # 安全注意
    ///
    /// 若路由器将注册**非公开 Action**（`is_public() == false`），调用方**必须**
    /// 通过 [`.middleware()`](Self::middleware) 注册认证中间件（如
    /// `TokenAuthMiddleware`）。否则未认证请求到达非公开 Action 时，
    /// `authorize_and_dispatch` 会返回
    /// `Unauthorized`，但中间件层的短路返回可能被意外绕过。
    ///
    /// 本方法在 debug build 中会通过 `tracing::warn!` 在首次 dispatch 时
    /// 检测此配置错误。
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
            table_definition: None,
            schema_tables: Vec::new(),
            actions: HashMap::new(),
            routes: HashMap::new(),
            #[cfg(feature = "mysql")]
            crud_permissions: None,
            default_permissions: Vec::new(),
            default_permission_mode: PermissionMode::default(),
            middlewares: Vec::new(),
        }
    }

    /// 绑定模块主表。
    pub fn table(mut self, definition: TableDefinition) -> Self {
        self.table_definition = Some(definition);
        self
    }

    /// 为模块注册一张附属 schema 表。
    ///
    /// 附属表会被 [`crate::router::AppRouter::table_definitions`] 汇总并由数据库初始化器
    /// 同步，但不会替换模块用于内置 CRUD 与 `ActionContext::table_query()` 的主表。
    /// 一个业务模块因此可以声明会话、审计等多张内部表，同时保持主表语义明确。
    pub fn schema(mut self, definition: TableDefinition) -> Self {
        self.schema_tables.push(definition);
        self
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
    ///     .default_permissions(vec!["user:access".to_string()])?;
    /// ```
    pub fn default_permissions(mut self, permissions: Vec<String>) -> Result<Self, BaseError> {
        if permissions
            .iter()
            .any(|permission| permission.trim().is_empty())
        {
            return Err(BaseError::ConfigError("默认权限名称不能为空".to_string()));
        }

        let mut seen = HashSet::with_capacity(permissions.len());
        for permission in &permissions {
            if !seen.insert(permission.as_str()) {
                return Err(BaseError::ConfigError(format!(
                    "默认权限重复: {}",
                    permission
                )));
            }
        }

        self.default_permissions = permissions;
        Ok(self)
    }

    /// 设置默认权限匹配模式（builder setter）
    ///
    /// 控制 `default_permissions` 中多个权限之间的逻辑关系：
    /// - [`PermissionMode::All`]（默认）：用户必须拥有全部权限（AND）
    /// - [`PermissionMode::Any`]：用户只需拥有其中任一权限（OR）
    ///
    /// # 参数
    ///
    /// - `mode`: 权限匹配模式
    ///
    /// # 返回
    ///
    /// - 修改后的 ModuleRouter 实例（支持链式调用）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::router::ModuleRouter;
    /// use yang_base::action::PermissionMode;
    ///
    /// let router = ModuleRouter::new("user", "用户管理")
    ///     .default_permissions(vec!["user:read".to_string(), "user:write".to_string()])?
    ///     .default_permission_mode(PermissionMode::Any);
    /// ```
    pub fn default_permission_mode(mut self, mode: PermissionMode) -> Self {
        self.default_permission_mode = mode;
        self
    }

    /// 原子注册一条 API。
    ///
    /// Action 与 HTTP 路由来自同一个 [`Api`]，因此不会出现只注册 handler、漏注册
    /// route，或用字符串绑定到错误 Action 的情况。Action 名、权限、路由、
    /// operation id 以及模块内冲突会在此统一校验。
    pub fn api(mut self, api: Api) -> Result<Self, BaseError> {
        let (action, route) = api.into_parts(&self.module_name)?;
        let meta = action.meta();
        let name = meta.name.to_string();
        if name.trim().is_empty() {
            return Err(BaseError::ConfigError("Action 名称不能为空".to_string()));
        }
        if meta
            .permissions
            .iter()
            .any(|permission| permission.name().trim().is_empty())
        {
            return Err(BaseError::ConfigError(
                "Action 权限名称不能为空".to_string(),
            ));
        }
        if self.actions.contains_key(&name) {
            return Err(BaseError::ConfigError(format!("Action 已注册: {name}")));
        }
        let mut route_patterns = RoutePatternRegistry::default();
        for existing in self.routes.values() {
            route_patterns.insert(existing)?;
        }
        route_patterns.insert(&route)?;
        if self
            .routes
            .values()
            .any(|existing| existing.operation_id == route.operation_id)
        {
            return Err(BaseError::ConfigError(format!(
                "operation_id 冲突: {}",
                route.operation_id
            )));
        }
        self.actions.insert(name.clone(), action);
        self.routes.insert(name, route);
        Ok(self)
    }

    /// 批量原子配置 API；异构 Action 可先通过 [`Api`] 擦除后放入数组或 `Vec`。
    pub fn apis<I>(mut self, apis: I) -> Result<Self, BaseError>
    where
        I: IntoIterator<Item = Api>,
    {
        for api in apis {
            self = self.api(api)?;
        }
        Ok(self)
    }

    /// 构建与运行时注册表隔离的只读模块描述快照。
    pub fn descriptor(&self) -> Result<ModuleDescriptor, BaseError> {
        #[cfg(feature = "mysql")]
        let builtin_contracts = match (&self.crud_permissions, &self.table_definition) {
            (Some(_), Some(definition)) => {
                crate::action::builtin::crud_contracts(definition, &self.module_name)?
                    .into_iter()
                    .collect::<HashMap<_, _>>()
            }
            (Some(_), None) => return Err(BaseError::TableDefinitionNotSet),
            (None, _) => HashMap::new(),
        };

        let mut action_names: Vec<&String> = self.actions.keys().collect();
        action_names.sort();
        let mut actions = Vec::with_capacity(action_names.len());
        for name in action_names {
            let action = &self.actions[name];
            let meta = action.meta();
            let route = self
                .routes
                .get(name)
                .ok_or_else(|| BaseError::ConfigError(format!("Action 缺少 route: {name}")))?;
            let permissions: Vec<String> = meta
                .permissions
                .iter()
                .map(|permission| permission.name().to_string())
                .collect();
            let permission_mode = meta.permission_mode;
            let input_schema = meta.input_schema.clone();
            let output_schema = meta.output_schema.clone();
            #[cfg(feature = "mysql")]
            let (permissions, permission_mode, input_schema, output_schema) =
                builtin_contracts.get(name.as_str()).map_or_else(
                    || (permissions, permission_mode, input_schema, output_schema),
                    |contract| {
                        (
                            contract.permissions.clone(),
                            contract.permission_mode,
                            contract.input_schema.clone(),
                            contract.output_schema.clone(),
                        )
                    },
                );
            actions.push(ActionDescriptor {
                name: meta.name.to_string(),
                display_name: meta.display_name.to_string(),
                description: meta.description.to_string(),
                permissions,
                permission_mode,
                is_public: meta.is_public,
                input_schema,
                output_schema,
                route: route.clone(),
            });
        }
        Ok(ModuleDescriptor {
            name: self.module_name.clone(),
            display_name: self.display_name.clone(),
            default_permissions: self.default_permissions.clone(),
            default_permission_mode: self.default_permission_mode,
            actions,
        })
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

    /// 为当前主表注册全部六个内置 CRUD Actions。
    ///
    /// 注册 add、put、del、get、select、table 六个基于 [`crate::table::Record`]
    /// 的内置 Action。调用前必须先通过 [`Self::table`] 绑定表定义；运行时字段、
    /// 类型和权限全部以这份定义为准，不再需要数据库实体类型。
    ///
    /// 内置 Action 名称由 [`BUILTIN_ACTION_NAMES`] 常量定义。
    ///
    /// # 返回
    ///
    /// - `Ok(Self)`: 注册成功，返回修改后的 ModuleRouter 实例（支持链式调用）
    /// - `Err(BaseError::TableDefinitionNotSet)`: 未绑定表定义
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::router::ModuleRouter;
    ///
    /// let router = ModuleRouter::new("user", "用户管理")
    ///     .table(user_table)
    ///     .crud()?;
    /// ```
    #[cfg(feature = "mysql")]
    pub fn crud(self) -> Result<Self, BaseError> {
        use crate::action::builtin::{
            crud_contracts, AddAction, DelAction, GetAction, PutAction, SelectAction, TableAction,
        };

        let definition = self
            .table_definition
            .as_ref()
            .ok_or(BaseError::TableDefinitionNotSet)?;
        crud_contracts(definition, &self.module_name)?;

        let base_path = format!("/api/{}", self.module_name);
        let mut router = self.apis([
            Api::post(&base_path, AddAction::new()).created(),
            Api::put(&base_path, PutAction::new()),
            Api::delete(&base_path, DelAction::new()),
            Api::get(&base_path, GetAction::new()),
            Api::post(format!("{base_path}/query"), SelectAction::new()),
            Api::get(format!("{base_path}/schema"), TableAction::new()),
        ])?;
        router.crud_permissions = Some(CrudPermissions {
            read: format!("{}:read", router.module_name),
            write: format!("{}:write", router.module_name),
        });
        Ok(router)
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
    ///     .table(user_table)
    ///     .crud()?;
    ///
    /// let response = router.dispatch("add", context).await?;
    /// ```
    pub async fn dispatch(
        &self,
        action_name: &str,
        mut context: ActionContext,
    ) -> Result<ApiResponse, BaseError> {
        // 0. 安全检查：存在非公开 Action 但未注册受保护范围中间件时发出警告。
        let has_protected_middleware = self
            .middlewares
            .iter()
            .any(|middleware| middleware.scope() == MiddlewareScope::ProtectedActions);
        if !has_protected_middleware && self.actions.values().any(|a| !a.meta().is_public) {
            tracing::warn!(
                module = %self.module_name,
                "模块 '{}' 包含非公开 Action 但未注册认证中间件（如 TokenAuthMiddleware）",
                self.module_name,
            );
        }

        // 1. 查找 Action（克隆 Arc，便于后续移动进中间件链 / 执行）
        let action = self
            .actions
            .get(action_name)
            .ok_or_else(|| BaseError::ActionNotFound(action_name.to_string()))?
            .clone();

        // 2. 设置表配置与模块名到上下文
        if let Some(table_definition) = &self.table_definition {
            context = context.with_table_definition(table_definition.clone());
        }
        // 注入模块名，供 metrics module 标签等可观测性标注（NEW-2）
        if context.module.is_none() {
            context = context.with_module(self.module_name.clone());
        }

        // 3. 进入中间件链（最外层）。链尾执行内置鉴权 + Action 派发；
        //    通用中间件覆盖全部请求，受保护范围中间件在公开 Action 上被跳过。
        let next = Next {
            remaining: &self.middlewares,
            router: self,
            is_public: action.meta().is_public,
            action,
        };

        // 根 span：串联整条派发链路。静态 span 名 + 借用字段，成功路径零分配；
        // request_id 以 Empty 声明——只有 Empty 字段才能被后续 record 更新，故
        // RequestIdMiddleware 透传上游 X-Request-Id 时能改写本字段（NEW-1）。
        // 同时在此先 record 一次默认值，保证未注册该中间件时根 span 也带 request_id。
        let span = tracing::info_span!(
            "dispatch",
            module = %self.module_name,
            action = %action_name,
            request_id = tracing::field::Empty,
        );
        span.record("request_id", tracing::field::display(context.request_id));
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
                && !self.check_permissions(
                    user,
                    &self.default_permissions,
                    self.default_permission_mode,
                )
            {
                span.record("granted", false);
                return Err(BaseError::PermissionDenied(format!(
                    "缺少模块权限: {:?}",
                    self.default_permissions
                )));
            }

            // `.crud()` 生成的权限名与 Catalog 使用同一模块命名规则。schema 则由
            // descriptor 从当前主表即时投影，使 `.crud()?.table(...)` 仍保持一致。
            #[cfg(feature = "mysql")]
            if let Some(crud_permissions) = &self.crud_permissions {
                let required_permission = match meta.name {
                    "add" | "put" | "del" => Some(&crud_permissions.write),
                    "get" | "select" | "table" => Some(&crud_permissions.read),
                    _ => None,
                };
                if let Some(permission) = required_permission {
                    if !user.has_permission(permission) {
                        span.record("granted", false);
                        return Err(BaseError::PermissionDenied(format!(
                            "缺少 Action 权限: {:?}",
                            [permission]
                        )));
                    }
                }
            }

            // 检查 Action 权限
            //
            // 热路径零分配：成功路径仅借用权限名做模式判断，不构造任何
            // 中间集合；仅当权限不足时才 collect 格式化中文错误信息。
            let action_perm_ok = match meta.permission_mode {
                PermissionMode::All => meta
                    .permissions
                    .iter()
                    .all(|p| user.has_permission(p.name())),
                PermissionMode::Any => meta
                    .permissions
                    .iter()
                    .any(|p| user.has_permission(p.name())),
            };
            if !meta.permissions.is_empty() && !action_perm_ok {
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
    ///     .table(users)
    ///     .crud()?;
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
    fn check_permissions(
        &self,
        user: &User,
        required_permissions: &[String],
        mode: PermissionMode,
    ) -> bool {
        match mode {
            PermissionMode::All => required_permissions
                .iter()
                .all(|perm| user.has_permission(perm)),
            PermissionMode::Any => required_permissions
                .iter()
                .any(|perm| user.has_permission(perm)),
        }
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
        let mut names: Vec<String> = self.actions.keys().cloned().collect();
        names.sort();
        names
    }

    /// 返回模块主表定义。
    pub fn table_definition(&self) -> Option<&TableDefinition> {
        self.table_definition.as_ref()
    }

    /// 返回模块全部 schema 表，按表名确定性排序。
    ///
    /// 结果包含可选主表和通过 [`Self::schema`] 注册的附属表。
    pub fn schema_definitions(&self) -> Vec<&TableDefinition> {
        let mut tables: Vec<&TableDefinition> = self
            .table_definition
            .iter()
            .chain(&self.schema_tables)
            .collect();
        tables.sort_by(|left, right| left.name().cmp(right.name()));
        tables
    }
}
