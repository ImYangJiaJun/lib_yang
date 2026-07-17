//! App 定义构建、交叉校验与运行时 slot 预解析。

use super::{
    ActionRef, AddonSpec, BuildError, FieldKind, FieldRef, ModuleName, ParamSource, TableName,
};
use crate::action::{ActionContext, ApiResponse, DynAction, Request};
use crate::error::BaseError;
use crate::router::middleware::{authorize, AuthorizationPolicy, Next, PermissionGroup};
use crate::table::TableDefinition;
use crate::tools::Tools;
use std::any::TypeId;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

/// 已预解析的 Action Registry slot。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActionHandle(usize);

impl ActionHandle {
    /// 返回稳定 slot 索引。
    pub const fn slot(self) -> usize {
        self.0
    }
}

/// 带 Input/Output 类型信息的预解析内部调用句柄。
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypedActionHandle<I, O> {
    raw: ActionHandle,
    marker: PhantomData<fn(I) -> O>,
}

impl<I, O> Copy for TypedActionHandle<I, O> {}

impl<I, O> Clone for TypedActionHandle<I, O> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<I, O> TypedActionHandle<I, O> {
    /// 返回底层稳定 slot。
    pub const fn raw(self) -> ActionHandle {
        self.raw
    }
}

/// 构建期产生的确定性只读定义快照。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DefinitionCatalog {
    addons: Vec<AddonSpec>,
}

impl DefinitionCatalog {
    /// 按名称稳定排序的 Addon 定义。
    pub fn addons(&self) -> &[AddonSpec] {
        &self.addons
    }

    /// 返回按稳定注册顺序排列的原生表定义。
    pub fn tables(&self) -> impl Iterator<Item = &super::TableSpec> {
        self.addons
            .iter()
            .flat_map(|addon| &addon.modules)
            .filter_map(|module| module.table.as_ref())
    }
}

/// ActionRef 到稳定运行时 slot 的不可变映射。
#[derive(Clone, Default)]
pub struct Registry {
    actions: BTreeMap<ActionRef, ActionHandle>,
    handlers: Vec<RuntimeAction>,
}

#[derive(Clone)]
struct RuntimeAction {
    handler: Arc<dyn DynAction>,
    middlewares: Arc<[Arc<dyn crate::router::Middleware>]>,
    policy: AuthorizationPolicy,
    module: String,
    action: String,
    table_definition: Option<TableDefinition>,
    ui_schema: super::ActionDemoSchema,
}

impl fmt::Debug for Registry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Registry")
            .field("actions", &self.actions)
            .field("handler_count", &self.handlers.len())
            .finish()
    }
}

impl Registry {
    /// 解析构建期已验证的 Action 引用。
    pub fn resolve(&self, action: &ActionRef) -> Option<ActionHandle> {
        self.actions.get(action).copied()
    }

    /// 解析并校验强类型内部调用句柄。
    pub fn resolve_typed<I, O>(
        &self,
        action: &ActionRef,
    ) -> Result<TypedActionHandle<I, O>, BaseError>
    where
        I: Send + 'static,
        O: Send + 'static,
    {
        let raw = self
            .resolve(action)
            .ok_or_else(|| BaseError::ActionNotFound(action.to_string()))?;
        let runtime = self
            .handlers
            .get(raw.slot())
            .ok_or_else(|| BaseError::ActionNotFound(action.to_string()))?;
        if runtime.handler.input_type_id() != TypeId::of::<I>()
            || runtime.handler.output_type_id() != TypeId::of::<O>()
        {
            return Err(BaseError::ConfigError(format!(
                "Action {action} 的内部调用类型不匹配"
            )));
        }
        Ok(TypedActionHandle {
            raw,
            marker: PhantomData,
        })
    }

    /// 返回 Action 总数。
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    /// Registry 是否为空。
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// 返回预绑定 Handler；无效 handle 稳定返回 ActionNotFound。
    pub fn handler(&self, handle: ActionHandle) -> Result<&dyn DynAction, BaseError> {
        self.handlers
            .get(handle.slot())
            .map(|runtime| runtime.handler.as_ref())
            .ok_or_else(|| BaseError::ActionNotFound(format!("slot {}", handle.slot())))
    }

    pub(crate) fn ui_catalog(&self, context: &ActionContext) -> super::UiCatalog {
        super::UiCatalog::new(
            self.handlers
                .iter()
                .filter(|runtime| runtime.policy.allows(context))
                .map(|runtime| runtime.ui_schema.clone()),
        )
    }

    /// 通过构建期 handle 执行唯一预绑定 Handler。
    pub async fn dispatch(
        &self,
        handle: ActionHandle,
        mut context: ActionContext,
    ) -> Result<ApiResponse, BaseError> {
        use tracing::Instrument;

        let runtime = self
            .handlers
            .get(handle.slot())
            .ok_or_else(|| BaseError::ActionNotFound(format!("slot {}", handle.slot())))?;
        if context.module.is_none() {
            context = context.with_module(runtime.module.clone());
        }
        if let Some(table) = &runtime.table_definition {
            context = context.with_table_definition(table.clone());
        }
        let next = Next {
            remaining: &runtime.middlewares,
            action: Arc::clone(&runtime.handler),
            policy: &runtime.policy,
        };
        let span = tracing::info_span!(
            "dispatch",
            module = %runtime.module,
            action = %runtime.action,
            request_id = tracing::field::Empty,
        );
        span.record("request_id", tracing::field::display(context.request_id));
        next.run(context).instrument(span).await
    }

    /// 使用强类型输入直接调用预解析 Action；不发生 JSON 序列化或名称查找。
    pub async fn call<I, O>(
        &self,
        handle: TypedActionHandle<I, O>,
        mut context: ActionContext,
        input: I,
    ) -> Result<O, BaseError>
    where
        I: Send + 'static,
        O: Send + 'static,
    {
        let runtime = self
            .handlers
            .get(handle.raw.slot())
            .ok_or_else(|| BaseError::ActionNotFound(format!("slot {}", handle.raw.slot())))?;
        authorize(&runtime.policy, &context)?;
        if context.module.is_none() {
            context = context.with_module(runtime.module.clone());
        }
        if let Some(table) = &runtime.table_definition {
            context = context.with_table_definition(table.clone());
        }
        let output = runtime.handler.call_boxed(context, Box::new(input)).await?;
        output.downcast::<O>().map(|value| *value).map_err(|_| {
            BaseError::ConfigError(format!(
                "Action {}.{} 的内部调用输出类型不匹配",
                runtime.module, runtime.action
            ))
        })
    }
}

/// 构建完成且运行期不可变的 App 定义。
#[derive(Clone)]
pub struct BuiltApp {
    catalog: DefinitionCatalog,
    registry: Arc<Registry>,
    tools: Arc<Tools>,
    table_definitions: Vec<TableDefinition>,
    compiled_views: Vec<super::CompiledTableView>,
}

impl fmt::Debug for BuiltApp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BuiltApp")
            .field("catalog", &self.catalog)
            .field("registry", &self.registry)
            .field("tools", &self.tools)
            .field("table_definition_count", &self.table_definitions.len())
            .field("compiled_view_count", &self.compiled_views.len())
            .finish()
    }
}

impl BuiltApp {
    /// 返回只读定义 Catalog。
    pub fn catalog(&self) -> &DefinitionCatalog {
        &self.catalog
    }

    /// 返回预解析 Registry。
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// 返回当前应用实例显式拥有的冻结资源。
    pub fn tools(&self) -> &Tools {
        &self.tools
    }

    /// 返回从 Catalog 唯一字段事实预编译的数据库 schema 定义。
    pub fn table_definitions(&self) -> &[TableDefinition] {
        &self.table_definitions
    }

    /// 返回启动期解析完成的默认/显式 Table Views。
    pub fn compiled_views(&self) -> &[super::CompiledTableView] {
        &self.compiled_views
    }

    /// 按当前请求的认证身份投影有权访问的版本化 UI 目录。
    ///
    /// 本方法复用 dispatch 的构建期冻结授权策略；前端可见性与直接调用的
    /// module/action `All`/`Any` 权限语义不会分叉。租户级字段和数据过滤由后续
    /// View projector 负责，本目录只决定 Action 是否可见。
    pub fn ui_catalog(&self, context: &ActionContext) -> super::UiCatalog {
        self.registry.ui_catalog(context)
    }

    /// 为一次请求创建绑定当前应用资源的 ActionContext。
    pub fn context(&self, request: Request) -> ActionContext {
        ActionContext::new(request, Arc::clone(&self.tools))
            .with_registry(Arc::clone(&self.registry))
    }

    /// 通过启动期解析的 handle 执行请求。
    pub async fn dispatch(
        &self,
        handle: ActionHandle,
        request: Request,
    ) -> Result<ApiResponse, BaseError> {
        self.registry.dispatch(handle, self.context(request)).await
    }

    /// 执行已由传输层补全元数据的上下文。
    pub async fn dispatch_context(
        &self,
        handle: ActionHandle,
        context: ActionContext,
    ) -> Result<ApiResponse, BaseError> {
        self.registry.dispatch(handle, context).await
    }
}

/// 仅在启动期可变的 App 定义构建器。
#[derive(Debug, Clone, Default)]
#[must_use = "AppBuilder 必须调用 build() 才会执行交叉校验并冻结定义"]
pub struct AppBuilder {
    addons: Vec<AddonSpec>,
}

impl AppBuilder {
    /// 创建空构建器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个原生 Addon 定义。
    pub fn addon(mut self, addon: impl Into<AddonSpec>) -> Self {
        self.addons.push(addon.into());
        self
    }

    /// 校验全部定义、解析引用并冻结 Catalog/Registry。
    pub fn build(mut self, tools: impl Into<Arc<Tools>>) -> Result<BuiltApp, BuildError> {
        validate_unique_addons(&self.addons)?;
        validate_dependencies(&self.addons)?;
        validate_module_ownership(&self.addons)?;
        validate_unique_modules(&self.addons)?;
        validate_module_contents(&self.addons)?;
        resolve_param_fields(&mut self.addons)?;

        let fields = collect_fields(&self.addons)?;
        let actions = collect_actions(&self.addons)?;
        let views = collect_views(&self.addons)?;
        validate_references(&self.addons, &fields, &actions, &views)?;
        validate_routes(&self.addons)?;

        sort_definitions(&mut self.addons);
        let registry = build_registry(&self.addons)?;
        for runtime in &registry.handlers {
            runtime.handler.bind_registry(&registry).map_err(|error| {
                BuildError::InvalidReference {
                    kind: "ActionLink",
                    reference: error.to_string(),
                }
            })?;
        }
        let registry = Arc::new(registry);
        let table_definitions = compile_table_definitions(&self.addons)?;
        let compiled_views = compile_views(&self.addons, &registry)?;
        for module in self.addons.iter_mut().flat_map(|addon| &mut addon.modules) {
            module.clear_handlers();
        }
        Ok(BuiltApp {
            catalog: DefinitionCatalog {
                addons: self.addons,
            },
            registry,
            tools: tools.into(),
            table_definitions,
            compiled_views,
        })
    }
}

fn resolve_param_fields(addons: &mut [AddonSpec]) -> Result<(), BuildError> {
    let definitions = addons
        .iter()
        .flat_map(|addon| &addon.modules)
        .filter_map(|module| module.table.as_ref())
        .flat_map(|table| {
            table.fields.iter().map(move |field| {
                (
                    super::FieldRef::new(table.name.clone(), field.name.clone()),
                    field.clone(),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();

    for action in addons
        .iter_mut()
        .flat_map(|addon| &mut addon.modules)
        .flat_map(super::ModuleSpec::actions_mut)
    {
        for param in &mut action.params {
            let Some(reference) = &param.field else {
                continue;
            };
            let field = definitions
                .get(reference)
                .ok_or_else(|| BuildError::InvalidReference {
                    kind: "Field",
                    reference: reference.to_string(),
                })?;
            param.kind.get_or_insert(field.kind);
            param.required |= field.is_required();
            if param.validation.min_length.is_none() {
                param.validation.min_length = field.validation.min_length;
            }
            if param.validation.max_length.is_none() {
                param.validation.max_length = field.validation.max_length;
            }
            if param.validation.minimum.is_none() {
                param.validation.minimum = field.validation.minimum.clone();
            }
            if param.validation.maximum.is_none() {
                param.validation.maximum = field.validation.maximum.clone();
            }
            if param.validation.pattern.is_none() {
                param.validation.pattern = field.validation.pattern.clone();
            }
            if param.presentation.title.is_empty() {
                param.presentation.title = field.presentation.title.clone();
            }
            if param.presentation.description.is_empty() {
                param.presentation.description = field.presentation.description.clone();
            }
        }
    }
    Ok(())
}

fn compile_table_definitions(addons: &[AddonSpec]) -> Result<Vec<TableDefinition>, BuildError> {
    addons
        .iter()
        .flat_map(|addon| &addon.modules)
        .filter_map(|module| module.table.as_ref())
        .map(|table| {
            table
                .table_definition()
                .map_err(|error| BuildError::InvalidFieldDefinition {
                    table: table.name.to_string(),
                    field: "<table>".to_string(),
                    reason: error.to_string(),
                })
        })
        .collect()
}

fn compile_views(
    addons: &[AddonSpec],
    registry: &Registry,
) -> Result<Vec<super::CompiledTableView>, BuildError> {
    let mut compiled = Vec::new();
    for module in addons.iter().flat_map(|addon| &addon.modules) {
        let Some(table) = &module.table else {
            continue;
        };
        let table_ref = yang_db::TableRef::new(table.name.to_string()).map_err(|error| {
            BuildError::InvalidReference {
                kind: "Table",
                reference: error.to_string(),
            }
        })?;
        if module.views.is_empty() {
            let name =
                super::ViewName::new("default").map_err(|error| BuildError::InvalidReference {
                    kind: "View",
                    reference: error.to_string(),
                })?;
            let fields = table
                .fields
                .iter()
                .filter(|field| {
                    !field.access.secret
                        && !matches!(field.access.readable, super::AccessRule::Nobody)
                })
                .map(|field| checked_runtime_field(&table.name, &field.name))
                .collect::<Result<Vec<_>, _>>()?;
            compiled.push(super::CompiledTableView::new(
                module.name.clone(),
                name,
                table_ref,
                fields,
                Vec::new(),
            ));
            continue;
        }

        for view in &module.views {
            let fields = view
                .fields
                .iter()
                .map(|field| checked_runtime_field(field.table(), field.field()))
                .collect::<Result<Vec<_>, _>>()?;
            let actions = view
                .actions
                .iter()
                .map(|action| {
                    registry
                        .resolve(action)
                        .ok_or_else(|| BuildError::InvalidReference {
                            kind: "View Action",
                            reference: action.to_string(),
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            compiled.push(super::CompiledTableView::new(
                module.name.clone(),
                view.name.clone(),
                table_ref.clone(),
                fields,
                actions,
            ));
        }
    }
    Ok(compiled)
}

fn checked_runtime_field(
    table: &super::TableName,
    field: &super::FieldName,
) -> Result<yang_db::FieldRef, BuildError> {
    yang_db::FieldRef::new(format!("{table}.{field}")).map_err(|error| {
        BuildError::InvalidReference {
            kind: "Field",
            reference: error.to_string(),
        }
    })
}

fn duplicate(kind: &'static str, name: impl ToString) -> BuildError {
    BuildError::DuplicateName {
        kind,
        name: name.to_string(),
    }
}

fn insert_unique<T: Ord + Clone + ToString>(
    set: &mut BTreeSet<T>,
    value: &T,
    kind: &'static str,
) -> Result<(), BuildError> {
    if set.insert(value.clone()) {
        Ok(())
    } else {
        Err(duplicate(kind, value.to_string()))
    }
}

fn validate_unique_addons(addons: &[AddonSpec]) -> Result<(), BuildError> {
    let mut names = BTreeSet::new();
    for addon in addons {
        insert_unique(&mut names, &addon.name, "Addon")?;
    }
    Ok(())
}

fn validate_dependencies(addons: &[AddonSpec]) -> Result<(), BuildError> {
    let names: BTreeSet<_> = addons.iter().map(|addon| addon.name.clone()).collect();
    for addon in addons {
        let mut dependencies = BTreeSet::new();
        for dependency in &addon.dependencies {
            insert_unique(&mut dependencies, dependency, "Addon dependency")?;
            if !names.contains(dependency) {
                return Err(BuildError::DependencyMissing {
                    addon: addon.name.to_string(),
                    dependency: dependency.to_string(),
                });
            }
        }
    }
    Ok(())
}

fn validate_module_ownership(addons: &[AddonSpec]) -> Result<(), BuildError> {
    for addon in addons {
        for module in &addon.modules {
            let owner = module.name.as_str().split('.').next().unwrap_or_default();
            if owner != addon.name.as_str() {
                return Err(BuildError::InvalidReference {
                    kind: "Module owner",
                    reference: format!("{} -> {}", addon.name, module.name),
                });
            }
        }
    }
    Ok(())
}

fn validate_unique_modules(addons: &[AddonSpec]) -> Result<(), BuildError> {
    let mut names = BTreeSet::new();
    for module in addons.iter().flat_map(|addon| &addon.modules) {
        insert_unique(&mut names, &module.name, "Module")?;
    }
    Ok(())
}

fn validate_module_contents(addons: &[AddonSpec]) -> Result<(), BuildError> {
    let mut tables = BTreeSet::new();
    let mut operations = BTreeSet::new();
    for module in addons.iter().flat_map(|addon| &addon.modules) {
        if let Some(table) = &module.table {
            insert_unique(&mut tables, &table.name, "Table")?;
            let mut fields = BTreeSet::new();
            let mut tenant_key = None;
            for field in &table.fields {
                insert_unique(&mut fields, &field.name, "Field")?;
                if field.tenant_key && tenant_key.replace(&field.name).is_some() {
                    return Err(BuildError::InvalidFieldDefinition {
                        table: table.name.to_string(),
                        field: field.name.to_string(),
                        reason: "同一表只能声明一个 tenant_key".to_string(),
                    });
                }
                if matches!(field.kind, FieldKind::Table | FieldKind::Tree)
                    && field.relation.is_none()
                {
                    return Err(BuildError::InvalidFieldDefinition {
                        table: table.name.to_string(),
                        field: field.name.to_string(),
                        reason: "Table/Tree 字段必须声明 relation".to_string(),
                    });
                }
                if !matches!(field.kind, FieldKind::Table | FieldKind::Tree)
                    && field.relation.is_some()
                {
                    return Err(BuildError::InvalidFieldDefinition {
                        table: table.name.to_string(),
                        field: field.name.to_string(),
                        reason: "只有 Table/Tree 字段可以声明 relation".to_string(),
                    });
                }
            }
        }

        let mut actions = BTreeSet::new();
        let mut module_permissions = BTreeSet::new();
        for permission in &module.default_permissions {
            if permission.trim().is_empty() {
                return Err(BuildError::InvalidReference {
                    kind: "Module permission",
                    reference: format!("{} -> 空权限", module.name),
                });
            }
            insert_unique(&mut module_permissions, permission, "Module permission")?;
        }
        for action in module.actions() {
            insert_unique(&mut actions, &action.name, "Action")?;
            if let Some(error) = &action.contract_error {
                return Err(BuildError::InvalidReference {
                    kind: "Action schema",
                    reference: format!("{}.{} -> {error}", module.name, action.name),
                });
            }
            if action.display_name.trim().is_empty() {
                return Err(BuildError::InvalidReference {
                    kind: "Action display_name",
                    reference: format!("{}.{}", module.name, action.name),
                });
            }
            if !(100..=599).contains(&action.success_status) {
                return Err(BuildError::InvalidReference {
                    kind: "Action success_status",
                    reference: format!(
                        "{}.{} -> {}",
                        module.name, action.name, action.success_status
                    ),
                });
            }
            let mut permissions = BTreeSet::new();
            for permission in &action.permissions {
                if permission.trim().is_empty() {
                    return Err(BuildError::InvalidReference {
                        kind: "Action permission",
                        reference: format!("{}.{} -> 空权限", module.name, action.name),
                    });
                }
                insert_unique(&mut permissions, permission, "Action permission")?;
            }
            if action.route.operation_id.trim().is_empty() {
                return Err(BuildError::InvalidRoute {
                    method: action.route.method.as_str().to_string(),
                    path: action.route.path.clone(),
                    reason: "operation_id 不能为空".to_string(),
                });
            }
            if !operations.insert(action.route.operation_id.clone()) {
                return Err(duplicate("operation_id", &action.route.operation_id));
            }
            let mut params = BTreeSet::new();
            for param in &action.params {
                let identity = (param.source, param.name.clone());
                if !params.insert(identity) {
                    return Err(duplicate("Param", &param.name));
                }
                if param.source == ParamSource::Path
                    && !action
                        .route
                        .path
                        .contains(&format!("{{{}}}", param.name.as_str()))
                {
                    return Err(BuildError::InvalidReference {
                        kind: "Path param",
                        reference: format!("{} -> {}", param.name, action.route.path),
                    });
                }
            }
        }

        let mut views = BTreeSet::new();
        for view in &module.views {
            insert_unique(&mut views, &view.name, "View")?;
        }
    }
    Ok(())
}

fn collect_fields(
    addons: &[AddonSpec],
) -> Result<BTreeMap<TableName, BTreeSet<super::FieldName>>, BuildError> {
    let mut result = BTreeMap::new();
    for table in addons
        .iter()
        .flat_map(|addon| &addon.modules)
        .filter_map(|module| module.table.as_ref())
    {
        let fields = table
            .fields
            .iter()
            .map(|field| field.name.clone())
            .collect();
        if result.insert(table.name.clone(), fields).is_some() {
            return Err(duplicate("Table", &table.name));
        }
    }
    Ok(result)
}

fn collect_actions(addons: &[AddonSpec]) -> Result<BTreeSet<ActionRef>, BuildError> {
    let mut result = BTreeSet::new();
    for module in addons.iter().flat_map(|addon| &addon.modules) {
        for action in module.actions() {
            let reference = ActionRef::new(module.name.clone(), action.name.clone());
            insert_unique(&mut result, &reference, "Action")?;
        }
    }
    Ok(result)
}

fn collect_views(addons: &[AddonSpec]) -> Result<BTreeSet<super::ViewRef>, BuildError> {
    let mut result = BTreeSet::new();
    for module in addons.iter().flat_map(|addon| &addon.modules) {
        for view in &module.views {
            let reference = super::ViewRef::new(module.name.clone(), view.name.clone());
            insert_unique(&mut result, &reference, "View")?;
        }
    }
    Ok(result)
}

fn validate_field_ref(
    reference: &FieldRef,
    fields: &BTreeMap<TableName, BTreeSet<super::FieldName>>,
) -> Result<(), BuildError> {
    if fields
        .get(reference.table())
        .is_some_and(|table| table.contains(reference.field()))
    {
        Ok(())
    } else {
        Err(BuildError::InvalidReference {
            kind: "Field",
            reference: reference.to_string(),
        })
    }
}

fn validate_action_ref(
    reference: &ActionRef,
    actions: &BTreeSet<ActionRef>,
) -> Result<(), BuildError> {
    if actions.contains(reference) {
        Ok(())
    } else {
        Err(BuildError::InvalidReference {
            kind: "Action",
            reference: reference.to_string(),
        })
    }
}

fn validate_references(
    addons: &[AddonSpec],
    fields: &BTreeMap<TableName, BTreeSet<super::FieldName>>,
    actions: &BTreeSet<ActionRef>,
    views: &BTreeSet<super::ViewRef>,
) -> Result<(), BuildError> {
    for module in addons.iter().flat_map(|addon| &addon.modules) {
        if let Some(table) = &module.table {
            for field in &table.fields {
                if let Some(relation) = &field.relation {
                    validate_field_ref(relation, fields)?;
                }
                if let Some(select) = &field.select {
                    validate_action_ref(select, actions)?;
                }
            }
        }
        for action in module.actions() {
            for param in &action.params {
                if let Some(field) = &param.field {
                    validate_field_ref(field, fields)?;
                }
            }
            for call in &action.calls {
                validate_action_ref(call, actions)?;
            }
        }
        for view in &module.views {
            let view_ref = super::ViewRef::new(module.name.clone(), view.name.clone());
            if !views.contains(&view_ref) {
                return Err(BuildError::InvalidReference {
                    kind: "View",
                    reference: view_ref.to_string(),
                });
            }
            for field in &view.fields {
                validate_field_ref(field, fields)?;
            }
            for action in &view.actions {
                validate_action_ref(action, actions)?;
            }
        }
    }
    Ok(())
}

#[derive(Default)]
struct RouteRegistry {
    matcher: matchit::Router<()>,
    methods_by_path: HashMap<String, HashSet<&'static str>>,
}

impl RouteRegistry {
    fn insert(&mut self, method: &'static str, path: &str) -> Result<(), BuildError> {
        validate_route(method, path)?;
        if let Some(methods) = self.methods_by_path.get_mut(path) {
            if methods.insert(method) {
                return Ok(());
            }
            return Err(route_conflict(method, path, String::new()));
        }
        self.matcher
            .insert(path.to_string(), ())
            .map_err(|error| route_conflict(method, path, format!(" ({error})")))?;
        self.methods_by_path
            .insert(path.to_string(), HashSet::from([method]));
        Ok(())
    }
}

fn validate_route(method: &str, path: &str) -> Result<(), BuildError> {
    if !path.starts_with('/') || path.contains(['?', '#']) || path.chars().any(char::is_whitespace)
    {
        return Err(BuildError::InvalidRoute {
            method: method.to_string(),
            path: path.to_string(),
            reason: "path 必须是无 query/fragment/空白的绝对路径".to_string(),
        });
    }
    if path
        .split('/')
        .any(|segment| segment.starts_with([':', '*']))
    {
        return Err(BuildError::InvalidRoute {
            method: method.to_string(),
            path: path.to_string(),
            reason: "path 必须使用 Axum 0.8 的 {name}/{*name} 参数语法".to_string(),
        });
    }
    let mut matcher = matchit::Router::new();
    matcher
        .insert(path.to_string(), ())
        .map_err(|error| BuildError::InvalidRoute {
            method: method.to_string(),
            path: path.to_string(),
            reason: error.to_string(),
        })?;
    Ok(())
}

fn route_conflict(method: &str, path: &str, detail: String) -> BuildError {
    BuildError::RouteConflict {
        method: method.to_string(),
        path: path.to_string(),
        detail,
    }
}

fn validate_routes(addons: &[AddonSpec]) -> Result<(), BuildError> {
    let mut routes = RouteRegistry::default();
    for action in addons
        .iter()
        .flat_map(|addon| &addon.modules)
        .flat_map(|module| module.actions())
    {
        routes.insert(action.route.method.as_str(), &action.route.path)?;
    }
    Ok(())
}

fn sort_definitions(addons: &mut [AddonSpec]) {
    for addon in addons.iter_mut() {
        addon.dependencies.sort();
        addon
            .modules
            .sort_by(|left, right| left.name.cmp(&right.name));
        for module in &mut addon.modules {
            module.default_permissions.sort();
            if let Some(table) = &mut module.table {
                table
                    .fields
                    .sort_by(|left, right| left.name.cmp(&right.name));
            }
            module.sort_actions();
            for action in module.actions_mut() {
                action.params.sort_by(|left, right| {
                    (left.source, &left.name).cmp(&(right.source, &right.name))
                });
                action.calls.sort();
                action.permissions.sort();
                action.tags.sort();
            }
            module
                .views
                .sort_by(|left, right| left.name.cmp(&right.name));
        }
    }
    addons.sort_by(|left, right| left.name.cmp(&right.name));
}

fn build_registry(addons: &[AddonSpec]) -> Result<Registry, BuildError> {
    let mut actions = BTreeMap::new();
    let mut handlers = Vec::new();
    for (slot, (module, table, action, handler, middlewares)) in addons
        .iter()
        .flat_map(|addon| &addon.modules)
        .flat_map(|module| {
            module.action_pairs().map(move |(action, handler)| {
                (
                    &module.name,
                    module.table.as_ref(),
                    action,
                    handler,
                    module.middlewares(),
                )
            })
        })
        .enumerate()
    {
        actions.insert(
            ActionRef::new(module.clone(), action.name.clone()),
            ActionHandle(slot),
        );
        let mut permission_groups = Vec::new();
        let module_spec = addons
            .iter()
            .flat_map(|addon| &addon.modules)
            .find(|candidate| &candidate.name == module);
        if let Some(module_spec) = module_spec {
            if !module_spec.default_permissions.is_empty() {
                permission_groups.push(PermissionGroup::new(
                    "模块",
                    Arc::<[String]>::from(module_spec.default_permissions.clone()),
                    module_spec.default_permission_mode,
                ));
            }
        }
        if !action.permissions.is_empty() {
            permission_groups.push(PermissionGroup::new(
                "Action",
                Arc::<[String]>::from(action.permissions.clone()),
                action.permission_mode,
            ));
        }
        handlers.push(RuntimeAction {
            handler: Arc::clone(handler),
            middlewares: Arc::from(middlewares.to_vec()),
            policy: AuthorizationPolicy::new(action.is_public, permission_groups),
            module: module.to_string(),
            action: action.name.to_string(),
            ui_schema: super::ActionDemoSchema::from(action),
            table_definition: table
                .map(super::TableSpec::table_definition)
                .transpose()
                .map_err(|error| BuildError::InvalidFieldDefinition {
                    table: table
                        .map_or_else(|| "<none>".to_string(), |value| value.name.to_string()),
                    field: "<table>".to_string(),
                    reason: error.to_string(),
                })?,
        });
    }
    Ok(Registry { actions, handlers })
}

#[allow(dead_code)]
fn _module_name_type_is_used(_: &ModuleName) {}
