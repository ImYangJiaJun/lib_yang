//! BR 业务术语对应的唯一原生 Addon/Module 注册接口。

use super::{
    ActionSpec, AddonName, AddonSpec, Fields, ModuleName, ModuleSpec, TableName, TableSpec,
    ViewSpec,
};
use crate::action::DynAction;
use crate::action::{Action as BusinessAction, TypedAction};
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
        .success_status(handler.success_status())
        .params(<A as BusinessAction>::params());
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
            spec = spec.table(TableSpec::new(table).fields(self.fields()));
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
