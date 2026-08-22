//! BR 业务术语对应的唯一原生 Addon/Module 注册接口。

use super::{
    ActionName, ActionRef, ActionSpec, AddonName, AddonSpec, Fields, HttpMethod, ModuleName,
    ModuleSpec, ParamInput, RouteSpec, TableName, TableSpec, ViewSpec,
};
use crate::action::functional::FnAction;
use crate::action::DynAction;
use crate::action::{Action as BusinessAction, ActionContext, PermissionMode, TypedAction};
use crate::error::BaseError;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::future::Future;
use std::marker::PhantomData;
use std::sync::Arc;

/// 已原子绑定定义和 Handler 的 Action 集合。
#[derive(Default)]
pub struct Actions(Vec<(ActionSpec, Arc<dyn DynAction>)>);

impl Actions {
    /// 创建空集合。
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// 原子增加 Action 定义及 Handler。
    pub fn action<A>(mut self, spec: ActionSpec, handler: A) -> Self
    where
        A: DynAction,
    {
        self.0.push((spec, Arc::new(handler)));
        self
    }

    /// 从同一个原生 Action 类型原子生成 route/params/权限定义并绑定 Handler。
    pub fn native<A>(mut self, handler: A) -> Self
    where
        A: BusinessAction + TypedAction,
    {
        let name = handler.name();
        let path = if handler.path().is_empty() {
            format!("/{name}")
        } else {
            handler.path().to_string()
        };
        let permissions = handler
            .permissions()
            .iter()
            .map(|permission| permission.name().to_string())
            .collect::<Vec<_>>();
        let mut spec = ActionSpec::new(
            super::ActionName::__from_validated_literal(name),
            super::RouteSpec::new(handler.http_method(), path, name),
        )
        .display_name(handler.display_name())
        .description(handler.description())
        .permissions(permissions, handler.permission_mode())
        .public(handler.is_public())
        .response_kind(handler.response_kind())
        .success_status(handler.success_status())
        .params(<A as BusinessAction>::params());
        if let Some(multipart) = handler.multipart_spec() {
            spec = spec.multipart(multipart);
        }
        for call in handler.calls() {
            spec = spec.calls(call);
        }
        self.0.push((spec, Arc::new(handler)));
        self
    }

    pub(crate) fn into_vec(self) -> Vec<(ActionSpec, Arc<dyn DynAction>)> {
        self.0
    }
}

impl std::fmt::Debug for Actions {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Actions")
            .field("len", &self.0.len())
            .finish()
    }
}

impl ModuleSpec {
    /// 在手写 ModuleSpec 路径中原子注册一个原生 Action。
    pub fn native_action<A>(self, handler: A) -> Self
    where
        A: BusinessAction + TypedAction,
    {
        let mut values = Actions::new().native(handler).into_vec();
        let (mut spec, handler) = values.remove(0);
        if spec.route.operation_id == spec.name.as_str() {
            spec.route.operation_id = format!("{}.{}", self.name, spec.name);
        }
        self.dyn_action(spec, handler)
    }

    /// 以一个普通 async fn / 闭包为 Handler 原子注册函数式 Action。
    ///
    /// `I`/`O` 由 Handler 签名推断：Handler 形如
    /// `async fn(ActionContext, I) -> Result<O, BaseError>`；闭包写法必须标注
    /// 参数类型以便推断 `I`。`I` 通常由 `params!` 生成，从而同时提供强类型输入
    /// 与唯一原生参数定义。
    ///
    /// 默认契约与 [`ModuleSpec::native_action`] 一致：`POST /{name}`，
    /// `operation_id` 补全为 `<module>.<name>`；其余定义经返回的
    /// [`ActionFnBuilder`] 声明后由 [`ActionFnBuilder::register`] 终结注册，
    /// route/params/权限与 Handler 在同一次调用链中原子完成，不存在
    /// "先注册 spec 再绑 handler" 的两步形态。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// ModuleSpec::new(module!("account.user"))
    ///     .action_fn(ActionName::new("disable")?, disable_user)
    ///     .route(HttpMethod::Post, "/users/disable")
    ///     .display_name("停用用户")
    ///     .permissions(["account.user:write"])
    ///     .register()
    /// ```
    pub fn action_fn<F, I, O, Fut>(self, name: ActionName, handler: F) -> ActionFnBuilder<F, I, O>
    where
        F: Fn(ActionContext, I) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<O, BaseError>> + Send,
        I: ParamInput + DeserializeOwned + JsonSchema + Send + 'static,
        O: Serialize + JsonSchema + Send + 'static,
    {
        let spec = ActionSpec::new(
            name.clone(),
            RouteSpec::new(HttpMethod::Post, format!("/{name}"), name.as_str()),
        );
        ActionFnBuilder {
            module: self,
            spec,
            handler,
            marker: PhantomData,
        }
    }
}

/// 函数式 Action 的终结式注册 Builder。
///
/// 由 [`ModuleSpec::action_fn`] 创建，持有半成品 [`ActionSpec`] 与业务函数；
/// 每个声明方法对应一个 ActionSpec 契约面，终结方法 [`Self::register`]
/// 一次性完成 params/JSON Schema 填充、operation_id 补全与 Handler 绑定。
pub struct ActionFnBuilder<F, I, O> {
    module: ModuleSpec,
    spec: ActionSpec,
    handler: F,
    marker: PhantomData<fn(I) -> O>,
}

impl<F, I, O> ActionFnBuilder<F, I, O> {
    /// 覆盖默认路由（`POST /{name}`）。
    #[must_use]
    pub fn route(mut self, method: HttpMethod, path: impl Into<String>) -> Self {
        self.spec.route.method = method;
        self.spec.route.path = path.into();
        self
    }

    /// 设置用户可见名称（默认同 Action 名）。
    #[must_use]
    pub fn display_name(mut self, display_name: impl Into<String>) -> Self {
        self.spec.display_name = display_name.into();
        self
    }

    /// 设置业务说明。
    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.spec.description = description.into();
        self
    }

    /// 设置所需权限列表（默认为空）。
    #[must_use]
    pub fn permissions<P, S>(mut self, permissions: P) -> Self
    where
        P: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.spec.permissions = permissions.into_iter().map(Into::into).collect();
        self
    }

    /// 设置权限匹配模式（默认 All，即 AND 语义）。
    #[must_use]
    pub fn permission_mode(mut self, mode: PermissionMode) -> Self {
        self.spec.permission_mode = mode;
        self
    }

    /// 标记为公开 Action（跳过登录/权限检查）；默认非公开。
    #[must_use]
    pub fn public(mut self) -> Self {
        self.spec.is_public = true;
        self
    }

    /// 设置成功响应 HTTP 状态码（默认 200）。
    #[must_use]
    pub fn success_status(mut self, status: u16) -> Self {
        self.spec.success_status = status;
        self
    }

    /// 声明成功响应的静态类别（默认普通 JSON）。
    #[must_use]
    pub fn response_kind(mut self, kind: super::ActionResponseKind) -> Self {
        self.spec.response_kind = kind;
        self
    }

    /// 声明 Action 接受受限的 `multipart/form-data` 请求。
    #[must_use]
    pub fn multipart(mut self, multipart: super::MultipartSpec) -> Self {
        self.spec = self.spec.multipart(multipart);
        self
    }

    /// 声明一个内部 Action 调用引用；AppBuilder 在构建期交叉校验。
    #[must_use]
    pub fn calls(mut self, action: ActionRef) -> Self {
        self.spec.calls.push(action);
        self
    }

    /// 终结注册：填充 params 与 Input/Output JSON Schema，补全 operation_id，
    /// 并经 `dyn_action` 把定义与 Handler 原子压入所属 Module。
    ///
    /// Schema 在注册期直接由 `schemars::schema_for!` 序列化填充，使
    /// `bind_handler_contract` 跳过 `DynAction::meta()` 的占位实现；
    /// 序列化失败会写入 `contract_error`，由 AppBuilder 构建期拒绝。
    pub fn register<Fut>(mut self) -> ModuleSpec
    where
        F: Fn(ActionContext, I) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<O, BaseError>> + Send,
        I: ParamInput + DeserializeOwned + JsonSchema + Send + 'static,
        O: Serialize + JsonSchema + Send + 'static,
    {
        self.spec.params = I::params().into_vec();
        match (
            serde_json::to_value(schemars::schema_for!(I)),
            serde_json::to_value(schemars::schema_for!(O)),
        ) {
            (Ok(input_schema), Ok(output_schema)) => {
                self.spec.input_schema = input_schema;
                self.spec.output_schema = output_schema;
            }
            (Err(error), _) | (_, Err(error)) => {
                self.spec.contract_error = Some(error.to_string());
            }
        }
        let mut spec = self.spec;
        if spec.route.operation_id == spec.name.as_str() {
            spec.route.operation_id = format!("{}.{}", self.module.name, spec.name);
        }
        self.module
            .dyn_action(spec, Arc::new(FnAction::<F, I, O>::new(self.handler)))
    }
}

/// 原生 Module 集合。
#[derive(Debug, Clone, Default)]
pub struct Modules(Vec<ModuleSpec>);

impl Modules {
    /// 创建空集合。
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// 增加已经完成聚合的 Module。
    pub fn module<M>(mut self, module: M) -> Self
    where
        M: Module,
    {
        self.0.push(module.into_spec());
        self
    }

    pub(crate) fn into_vec(self) -> Vec<ModuleSpec> {
        self.0
    }
}

/// 业务 Module：fields/actions/views 仍集中在同一个实现中。
pub trait Module: Sized {
    /// 返回全限定 Module 名。
    fn name(&self) -> ModuleName;

    /// 返回可选数据库表名。
    fn table(&self) -> Option<TableName> {
        None
    }

    /// 返回主表字段。
    fn fields(&self) -> Fields {
        Fields::new()
    }

    /// 在统一字段投影后补充表标题、复合索引等表级约束。
    fn configure_table(&self, table: TableSpec) -> TableSpec {
        table
    }

    /// 返回 Module Actions。
    fn actions(&self) -> Actions {
        Actions::new()
    }

    /// 返回显式 Views；空集合由构建器生成默认 View。
    fn views(&self) -> Vec<ViewSpec> {
        Vec::new()
    }

    /// 直接聚合为唯一 ModuleSpec，不经过兼容投影。
    fn into_spec(self) -> ModuleSpec {
        let module_name = self.name();
        let mut spec = ModuleSpec::new(module_name.clone());
        if let Some(table) = self.table() {
            let table = TableSpec::new(table).fields(self.fields());
            spec = spec.table(self.configure_table(table));
        }
        for (mut action, handler) in self.actions().into_vec() {
            if action.route.operation_id == action.name.as_str() {
                action.route.operation_id = format!("{}.{}", module_name, action.name);
            }
            spec = spec.dyn_action(action, handler);
        }
        for view in self.views() {
            spec = spec.view(view);
        }
        spec
    }
}

/// 产品能力 Addon。
pub trait Addon: Sized {
    /// 返回 Addon 名。
    fn name(&self) -> AddonName;

    /// 返回所属 Modules。
    fn modules(&self) -> Modules;

    /// 返回依赖 Addon 名。
    fn dependencies(&self) -> Vec<AddonName> {
        Vec::new()
    }

    /// 直接聚合为唯一 AddonSpec。
    fn into_spec(self) -> AddonSpec {
        let mut spec = AddonSpec::new(self.name());
        for dependency in self.dependencies() {
            spec = spec.depends_on(dependency);
        }
        for module in self.modules().into_vec() {
            spec = spec.module(module);
        }
        spec
    }
}

impl<T> From<T> for AddonSpec
where
    T: Addon,
{
    fn from(value: T) -> Self {
        value.into_spec()
    }
}
