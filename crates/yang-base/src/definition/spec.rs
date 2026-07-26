//! 不可变定义的构建期输入模型。

use super::{
    ActionName, ActionPresentationSpec, ActionRef, AddonName, FieldName, FieldRef, FieldSpec,
    ModuleName, Params, PresentationSpec, TableSortSpec, TableSpec, ValidationSpec, ViewName,
};
use crate::action::DynAction;
use crate::action::PermissionMode;
use crate::router::Middleware;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

/// 业务字段的语义种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum FieldKind {
    /// 主键。
    Key,
    /// 有长度上限的字符串。
    Str,
    /// 长文本。
    Text,
    /// 整数。
    Int,
    /// 定点小数。
    Decimal,
    /// 布尔开关。
    Switch,
    /// 单选枚举。
    Radio,
    /// 表关系。
    Table,
    /// 树关系。
    Tree,
    /// 时间戳。
    Timestamp,
}

/// HTTP 参数来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ParamSource {
    /// JSON body。
    Body,
    /// Query string。
    Query,
    /// Path 参数。
    Path,
    /// Header。
    Header,
}

/// Action 的单个强类型参数定义。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamSpec {
    /// 参数名。
    pub name: FieldName,
    /// 参数来源。
    pub source: ParamSource,
    /// 是否必填。
    pub required: bool,
    /// 可选的复用字段。
    pub field: Option<FieldRef>,
    /// Action 专属参数的语义类型；复用字段时由引用目标提供。
    pub kind: Option<FieldKind>,
    /// Action 级验证覆盖。
    pub validation: ValidationSpec,
    /// Action 级展示覆盖。
    pub presentation: PresentationSpec,
}

impl ParamSpec {
    /// 创建参数定义。
    pub fn new(name: FieldName, source: ParamSource) -> Self {
        Self {
            name,
            source,
            required: false,
            field: None,
            kind: None,
            validation: ValidationSpec::default(),
            presentation: PresentationSpec::default(),
        }
    }

    /// 设置必填语义。
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// 复用一个已声明字段的共享语义。
    pub fn from_field(mut self, field: FieldRef) -> Self {
        self.field = Some(field);
        self
    }

    pub(crate) fn from_spec(name: FieldName, source: ParamSource, field: FieldSpec) -> Self {
        Self {
            name,
            source,
            required: field.is_required(),
            field: None,
            kind: Some(field.kind),
            validation: field.validation,
            presentation: field.presentation,
        }
    }
}

/// 支持的 HTTP method。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum HttpMethod {
    /// GET。
    Get,
    /// POST。
    Post,
    /// PUT。
    Put,
    /// PATCH。
    Patch,
    /// DELETE。
    Delete,
    /// OPTIONS。
    Options,
    /// HEAD。
    Head,
}

impl HttpMethod {
    /// 返回标准大写 method 名。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Options => "OPTIONS",
            Self::Head => "HEAD",
        }
    }
}

/// Action 的传输路由定义。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteSpec {
    /// HTTP method。
    pub method: HttpMethod,
    /// Axum 0.8 路由模板。
    pub path: String,
    /// OpenAPI operation id。
    pub operation_id: String,
}

impl RouteSpec {
    /// 创建路由定义。
    pub fn new(
        method: HttpMethod,
        path: impl Into<String>,
        operation_id: impl Into<String>,
    ) -> Self {
        Self {
            method,
            path: path.into(),
            operation_id: operation_id.into(),
        }
    }
}

/// Action 的定义期契约。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionSpec {
    /// Action 局部名称。
    pub name: ActionName,
    /// 唯一传输路由。
    pub route: RouteSpec,
    /// 强类型参数定义。
    pub params: Vec<ParamSpec>,
    /// 内部调用依赖。
    pub calls: Vec<ActionRef>,
    /// 用户可见名称。
    pub display_name: String,
    /// 业务说明。
    pub description: String,
    /// Action 所需权限。
    pub permissions: Vec<String>,
    /// 权限集合的匹配方式。
    pub permission_mode: PermissionMode,
    /// 是否允许匿名访问。
    pub is_public: bool,
    /// 请求媒体类型。
    pub request_media_type: super::ActionMediaType,
    /// multipart 资源与类型限制；仅 multipart Action 可用。
    pub multipart: Option<super::MultipartSpec>,
    /// 成功响应的静态类别，供调用方选择安全的展示方式。
    pub response_kind: super::ActionResponseKind,
    /// 成功响应状态码。
    pub success_status: u16,
    /// OpenAPI/后台元数据标签。
    pub tags: Vec<String>,
    /// Handler Input 的构建期 JSON Schema。
    pub input_schema: serde_json::Value,
    /// Handler Output 的构建期 JSON Schema。
    pub output_schema: serde_json::Value,
    pub(crate) contract_error: Option<String>,
}

impl ActionSpec {
    /// 创建最小 Action 定义。
    pub fn new(name: ActionName, route: RouteSpec) -> Self {
        let display_name = name.to_string();
        Self {
            name,
            route,
            params: Vec::new(),
            calls: Vec::new(),
            display_name,
            description: String::new(),
            permissions: Vec::new(),
            permission_mode: PermissionMode::All,
            is_public: false,
            request_media_type: super::ActionMediaType::Json,
            multipart: None,
            response_kind: super::ActionResponseKind::Json,
            success_status: 200,
            tags: Vec::new(),
            input_schema: serde_json::Value::Null,
            output_schema: serde_json::Value::Null,
            contract_error: None,
        }
    }

    /// 设置用户可见名称。
    #[must_use]
    pub fn display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = display_name.into();
        self
    }

    /// 设置业务说明。
    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// 设置所需权限及匹配方式。
    #[must_use]
    pub fn permissions<I, S>(mut self, permissions: I, mode: PermissionMode) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.permissions = permissions.into_iter().map(Into::into).collect();
        self.permission_mode = mode;
        self
    }

    /// 设置是否允许匿名访问。
    #[must_use]
    pub fn public(mut self, is_public: bool) -> Self {
        self.is_public = is_public;
        self
    }

    /// 声明 Action 接受受限的 `multipart/form-data` 请求。
    #[must_use]
    pub fn multipart(mut self, multipart: super::MultipartSpec) -> Self {
        self.request_media_type = super::ActionMediaType::Multipart;
        self.multipart = Some(multipart);
        self
    }

    /// 声明成功响应的静态类别。
    #[must_use]
    pub fn response_kind(mut self, response_kind: super::ActionResponseKind) -> Self {
        self.response_kind = response_kind;
        self
    }

    /// 设置成功响应 HTTP 状态码。
    #[must_use]
    pub fn success_status(mut self, success_status: u16) -> Self {
        self.success_status = success_status;
        self
    }

    /// 增加元数据标签。
    #[must_use]
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// 增加一个参数定义。
    #[must_use]
    pub fn param(mut self, param: ParamSpec) -> Self {
        self.params.push(param);
        self
    }

    /// 使用一个 Params 集合替换当前参数定义。
    #[must_use]
    pub fn params(mut self, params: Params) -> Self {
        self.params = params.into_vec();
        self
    }

    /// 声明一个内部 Action 调用引用。
    #[must_use]
    pub fn calls(mut self, action: ActionRef) -> Self {
        self.calls.push(action);
        self
    }

    fn bind_handler_contract(mut self, handler: &dyn DynAction) -> Self {
        if !self.input_schema.is_null() && !self.output_schema.is_null() {
            return self;
        }
        let meta = handler.meta();
        let input = serde_json::to_value(meta.input_schema);
        let output = serde_json::to_value(meta.output_schema);
        match (input, output) {
            (Ok(input_schema), Ok(output_schema)) => {
                self.input_schema = input_schema;
                self.output_schema = output_schema;
            }
            (Err(error), _) | (_, Err(error)) => {
                self.contract_error = Some(error.to_string());
            }
        }
        self
    }

    #[cfg(feature = "mysql")]
    fn bind_builtin_contract(
        mut self,
        contract: crate::action::builtin::BuiltinActionContract,
    ) -> Result<Self, crate::error::BaseError> {
        self.input_schema = serde_json::to_value(contract.input_schema)
            .map_err(|error| crate::error::BaseError::JsonSerializeFailed(error.to_string()))?;
        self.output_schema = serde_json::to_value(contract.output_schema)
            .map_err(|error| crate::error::BaseError::JsonSerializeFailed(error.to_string()))?;
        self.permissions = contract.permissions;
        self.permission_mode = contract.permission_mode;
        Ok(self)
    }
}

/// 通用树 View 的显式拓扑声明。
///
/// 树拓扑属于 View 语义，不能从字段存储类型或约定列名推断。三个字段都必须同时
/// 出现在所属 [`ViewSpec`] 中，并在构建期解析到同一张主表。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeViewSpec {
    /// 节点唯一标识字段。
    pub id_field: FieldRef,
    /// 父节点标识字段；根节点应返回 null。
    pub parent_field: FieldRef,
    /// 节点用户可见标签字段。
    pub label_field: FieldRef,
    /// 单次树查询的节点上限；`None` 回退到服务端默认值
    /// （[`crate::table::DEFAULT_TREE_MAX_NODES`]）。
    pub max_nodes: Option<usize>,
}

impl TreeViewSpec {
    /// 创建显式树拓扑声明。
    pub fn new(id_field: FieldRef, parent_field: FieldRef, label_field: FieldRef) -> Self {
        Self {
            id_field,
            parent_field,
            label_field,
            max_nodes: None,
        }
    }

    /// 设置单次树查询的节点上限；必须大于 0，否则构建期报错。
    #[must_use]
    pub fn max_nodes(mut self, max_nodes: usize) -> Self {
        self.max_nodes = Some(max_nodes);
        self
    }
}

/// 后台 Table/Select 等展示定义。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewSpec {
    /// View 局部名称。
    pub name: ViewName,
    /// 用户可见标题；默认使用 View 局部名称。
    pub title: String,
    /// 有序字段引用。
    pub fields: Vec<FieldRef>,
    /// 有序按钮/操作引用。
    pub actions: Vec<ActionRef>,
    /// 为通用表格返回标准分页行数据的 Action。
    pub data_action: Option<ActionRef>,
    /// 显式 Action 展示声明；未声明的 Action 在构建期按静态契约安全推导。
    pub action_presentations: BTreeMap<ActionRef, ActionPresentationSpec>,
    /// 可选树拓扑；未声明时按普通表格投影。
    pub tree: Option<TreeViewSpec>,
    /// 有序默认排序声明。
    pub default_sort: Vec<TableSortSpec>,
}

impl ViewSpec {
    /// 创建空 View。
    pub fn new(name: ViewName) -> Self {
        let title = name.to_string();
        Self {
            name,
            title,
            fields: Vec::new(),
            actions: Vec::new(),
            data_action: None,
            action_presentations: BTreeMap::new(),
            tree: None,
            default_sort: Vec::new(),
        }
    }

    /// 增加一个展示字段。
    #[must_use]
    pub fn field(mut self, field: FieldRef) -> Self {
        self.fields.push(field);
        self
    }

    /// 增加一个按钮或操作引用。
    #[must_use]
    pub fn action(mut self, action: ActionRef) -> Self {
        self.actions.push(action);
        self
    }

    /// 设置用户可见标题。
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// 设置通用表格的数据查询 Action。
    ///
    /// 该 Action 必须返回 JSON，且业务数据应符合 `items/page/page_size/total`
    /// 分页结构。它不会自动成为工具栏或行按钮。
    #[must_use]
    pub fn data_action(mut self, action: ActionRef) -> Self {
        self.data_action = Some(action);
        self
    }

    /// 增加一个带显式展示语义的按钮或操作引用。
    #[must_use]
    pub fn present_action(
        mut self,
        action: ActionRef,
        presentation: ActionPresentationSpec,
    ) -> Self {
        if !self.actions.contains(&action) {
            self.actions.push(action.clone());
        }
        self.action_presentations.insert(action, presentation);
        self
    }

    /// 将当前 View 显式声明为树 View。
    #[must_use]
    pub fn tree(mut self, tree: TreeViewSpec) -> Self {
        self.tree = Some(tree);
        self
    }

    /// 增加一个默认排序字段。
    #[must_use]
    pub fn default_sort(mut self, sort: TableSortSpec) -> Self {
        self.default_sort.push(sort);
        self
    }
}

/// Module 的聚合定义。
#[derive(Clone)]
pub struct ModuleSpec {
    /// 全限定 Module 名称。
    pub name: ModuleName,
    /// 可选主表。
    pub table: Option<TableSpec>,
    /// Action 定义集合。
    actions: Vec<ActionSpec>,
    /// 与 Action 定义同索引的运行时 Handler。
    handlers: Vec<Arc<dyn DynAction>>,
    /// 模块级中间件链。
    middlewares: Vec<Arc<dyn Middleware>>,
    /// 模块默认权限。
    pub default_permissions: Vec<String>,
    /// 模块默认权限匹配方式。
    pub default_permission_mode: PermissionMode,
    /// View 集合。
    pub views: Vec<ViewSpec>,
}

impl ModuleSpec {
    /// 创建空 Module。
    pub fn new(name: ModuleName) -> Self {
        Self {
            name,
            table: None,
            actions: Vec::new(),
            handlers: Vec::new(),
            middlewares: Vec::new(),
            default_permissions: Vec::new(),
            default_permission_mode: PermissionMode::All,
            views: Vec::new(),
        }
    }

    /// 设置 Module 主表。
    #[must_use]
    pub fn table(mut self, table: TableSpec) -> Self {
        self.table = Some(table);
        self
    }

    /// 为模块主表注册标准 add/put/del/get/select/table Actions。
    #[cfg(feature = "mysql")]
    pub fn crud(self) -> Result<Self, crate::error::BaseError> {
        let prefix = format!("/api/{}", self.name.as_str().replace('.', "/"));
        self.crud_at(prefix)
    }

    /// 在显式资源路径下注册标准 add/put/del/get/select/table Actions。
    ///
    /// 用于版本化或资源复数化路由；`operation_id` 仍始终使用
    /// `<module>.<action>`，不与 HTTP 路径耦合。
    #[cfg(feature = "mysql")]
    pub fn crud_at(self, prefix: impl Into<String>) -> Result<Self, crate::error::BaseError> {
        use crate::action::builtin::{AddAction, DelAction, PutAction};

        self.crud_at_with_mutations(prefix, AddAction::new(), PutAction::new(), DelAction::new())
    }

    /// 在显式资源路径下注册标准 CRUD 契约，并替换三个写 Action 的运行时 Handler。
    ///
    /// 路由、权限以及与表定义绑定的动态 JSON Schema 仍由框架统一生成；业务只负责
    /// add/put/del 的事务语义。自定义 Handler 必须分别接受与内置 Action 相同的
    /// [`Record`](crate::table::Record)、
    /// [`PutInput`](crate::action::builtin::PutInput) 与
    /// [`GetByPk`](crate::action::builtin::GetByPk) 输入，并返回对应的标准结果。
    #[cfg(feature = "mysql")]
    pub fn crud_at_with_mutations<A, P, D>(
        self,
        prefix: impl Into<String>,
        add: A,
        put: P,
        del: D,
    ) -> Result<Self, crate::error::BaseError>
    where
        A: DynAction,
        P: DynAction,
        D: DynAction,
    {
        use crate::action::builtin::{crud_contracts, GetAction, SelectAction, TableAction};

        let prefix = prefix.into();
        if prefix.len() < 2
            || !prefix.starts_with('/')
            || prefix.ends_with('/')
            || prefix.contains("//")
            || prefix.contains(['?', '#'])
        {
            return Err(crate::error::BaseError::ConfigError(format!(
                "CRUD 资源路径无效: {prefix}"
            )));
        }

        let table = self
            .table
            .as_ref()
            .ok_or(crate::error::BaseError::TableDefinitionNotSet)?
            .table_definition()?;
        let mut contracts: std::collections::BTreeMap<_, _> =
            crud_contracts(&table, self.name.as_str())?
                .into_iter()
                .collect();
        let module_name = self.name.to_string();
        let read_permission = format!("{}:read", self.name);
        let write_permission = format!("{}:write", self.name);
        let action_name = |value: &str| {
            ActionName::new(value)
                .map_err(|error| crate::error::BaseError::ConfigError(error.to_string()))
        };
        let action = |name: ActionName, method, path: String| {
            let operation_id = format!("{module_name}.{name}");
            ActionSpec::new(name, RouteSpec::new(method, path, operation_id))
        };
        let mut contract = |name: &str| {
            contracts.remove(name).ok_or_else(|| {
                crate::error::BaseError::ConfigError(format!(
                    "内置 CRUD 契约缺失: {module_name}.{name}"
                ))
            })
        };

        Ok(self
            .action(
                action(action_name("add")?, HttpMethod::Post, prefix.clone())
                    .display_name("新增")
                    .permissions([write_permission.clone()], PermissionMode::All)
                    .success_status(201)
                    .bind_builtin_contract(contract("add")?)?,
                add,
            )
            .action(
                action(action_name("put")?, HttpMethod::Put, prefix.clone())
                    .display_name("修改")
                    .permissions([write_permission.clone()], PermissionMode::All)
                    .bind_builtin_contract(contract("put")?)?,
                put,
            )
            .action(
                action(action_name("del")?, HttpMethod::Delete, prefix.clone())
                    .display_name("删除")
                    .permissions([write_permission], PermissionMode::All)
                    .bind_builtin_contract(contract("del")?)?,
                del,
            )
            .action(
                action(action_name("get")?, HttpMethod::Get, prefix.clone())
                    .display_name("详情")
                    .permissions([read_permission.clone()], PermissionMode::All)
                    .bind_builtin_contract(contract("get")?)?,
                GetAction::new(),
            )
            .action(
                action(
                    action_name("select")?,
                    HttpMethod::Post,
                    format!("{prefix}/query"),
                )
                .display_name("列表")
                .permissions([read_permission.clone()], PermissionMode::All)
                .bind_builtin_contract(contract("select")?)?,
                SelectAction::new(),
            )
            .action(
                action(
                    action_name("table")?,
                    HttpMethod::Get,
                    format!("{prefix}/schema"),
                )
                .display_name("表定义")
                .permissions([read_permission], PermissionMode::All)
                .bind_builtin_contract(contract("table")?)?,
                TableAction::new(),
            ))
    }

    /// 原子注册 Action 定义与其唯一运行时 Handler。
    #[must_use]
    pub fn action<A>(mut self, action: ActionSpec, handler: A) -> Self
    where
        A: DynAction,
    {
        self.actions.push(action.bind_handler_contract(&handler));
        self.handlers.push(Arc::new(handler));
        self
    }

    /// 原子注册已经类型擦除的 Action Handler。
    #[must_use]
    pub fn dyn_action(mut self, action: ActionSpec, handler: Arc<dyn DynAction>) -> Self {
        self.actions
            .push(action.bind_handler_contract(handler.as_ref()));
        self.handlers.push(handler);
        self
    }

    /// 增加一个模块级中间件。
    ///
    /// 多次调用按**注册顺序**构成洋葱链：先注册的中间件位于更外层，请求阶段
    /// 先于后注册的中间件执行（响应阶段则相反）。因此依赖上游注入状态的中间件
    /// 必须后注册——例如认证类中间件（如 `TokenAuthMiddleware`）必须先于
    /// [`TenantResolverMiddleware`](crate::action::TenantResolverMiddleware)
    /// 注册，租户 resolver 才能从 [`ActionContext`](crate::action::ActionContext)
    /// 读到已认证用户。框架会在构建期拒绝这两个内置中间件的反向顺序；自定义
    /// 中间件仍应通过 [`Middleware::role`](crate::router::Middleware::role) 声明已知角色。
    #[must_use]
    pub fn middleware<M>(mut self, middleware: M) -> Self
    where
        M: Middleware,
    {
        self.middlewares.push(Arc::new(middleware));
        self
    }

    /// 设置模块默认权限。
    #[must_use]
    pub fn default_permissions<I, S>(mut self, permissions: I, mode: PermissionMode) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.default_permissions = permissions.into_iter().map(Into::into).collect();
        self.default_permission_mode = mode;
        self
    }

    /// 返回按局部名称稳定排序前的 Action 定义。
    pub fn actions(&self) -> &[ActionSpec] {
        &self.actions
    }

    pub(crate) fn action_pairs(&self) -> impl Iterator<Item = (&ActionSpec, &Arc<dyn DynAction>)> {
        self.actions.iter().zip(&self.handlers)
    }

    pub(crate) fn middlewares(&self) -> &[Arc<dyn Middleware>] {
        &self.middlewares
    }

    pub(crate) fn actions_mut(&mut self) -> &mut [ActionSpec] {
        &mut self.actions
    }

    pub(crate) fn sort_actions(&mut self) {
        let mut pairs: Vec<_> = self
            .actions
            .drain(..)
            .zip(self.handlers.drain(..))
            .collect();
        pairs.sort_by(|left, right| left.0.name.cmp(&right.0.name));
        for (action, handler) in pairs {
            self.actions.push(action);
            self.handlers.push(handler);
        }
    }

    pub(crate) fn clear_handlers(&mut self) {
        self.handlers.clear();
        self.middlewares.clear();
    }

    /// 增加一个 View。
    #[must_use]
    pub fn view(mut self, view: ViewSpec) -> Self {
        self.views.push(view);
        self
    }
}

impl fmt::Debug for ModuleSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModuleSpec")
            .field("name", &self.name)
            .field("table", &self.table)
            .field("actions", &self.actions)
            .field("default_permissions", &self.default_permissions)
            .field("default_permission_mode", &self.default_permission_mode)
            .field("views", &self.views)
            .finish()
    }
}

impl PartialEq for ModuleSpec {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.table == other.table
            && self.actions == other.actions
            && self.default_permissions == other.default_permissions
            && self.default_permission_mode == other.default_permission_mode
            && self.views == other.views
    }
}

impl Eq for ModuleSpec {}

/// Addon 的产品能力定义。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddonSpec {
    /// Addon 名称。
    pub name: AddonName,
    /// 依赖的其它 Addon。
    pub dependencies: Vec<AddonName>,
    /// Module 集合。
    pub modules: Vec<ModuleSpec>,
}

impl AddonSpec {
    /// 创建空 Addon。
    pub fn new(name: AddonName) -> Self {
        Self {
            name,
            dependencies: Vec::new(),
            modules: Vec::new(),
        }
    }

    /// 声明 Addon 依赖。
    #[must_use]
    pub fn depends_on(mut self, dependency: AddonName) -> Self {
        self.dependencies.push(dependency);
        self
    }

    /// 增加一个 Module。
    #[must_use]
    pub fn module(mut self, module: ModuleSpec) -> Self {
        self.modules.push(module);
        self
    }

    /// 向 Addon 的每个 Module 追加同一个跨切面中间件。
    ///
    /// 中间件位于各 Module 已声明中间件之前，适合让日志、trace 等跨切面观察包括认证
    /// 与租户解析失败在内的完整派发结果。所有 Module 共享同一个线程安全实例，不建立
    /// 第二条派发链。
    #[must_use]
    pub fn middleware<M>(mut self, middleware: M) -> Self
    where
        M: Middleware,
    {
        let middleware: Arc<dyn Middleware> = Arc::new(middleware);
        for module in &mut self.modules {
            module.middlewares.insert(0, Arc::clone(&middleware));
        }
        self
    }
}

#[cfg(test)]
mod addon_middleware_tests {
    use super::*;
    use crate::router::RequestIdMiddleware;

    #[test]
    fn addon_middleware_prepends_one_shared_instance_before_module_middleware() {
        let addon = AddonSpec::new(crate::addon!("test"))
            .module(ModuleSpec::new(crate::module!("test.first")).middleware(RequestIdMiddleware))
            .module(ModuleSpec::new(crate::module!("test.second")))
            .middleware(RequestIdMiddleware);

        assert_eq!(addon.modules[0].middlewares().len(), 2);
        assert_eq!(addon.modules[1].middlewares().len(), 1);
        assert!(Arc::ptr_eq(
            &addon.modules[0].middlewares()[0],
            &addon.modules[1].middlewares()[0]
        ));
    }
}
