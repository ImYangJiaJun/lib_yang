//! App 定义构建、交叉校验与运行时 slot 预解析。

use super::{
    ActionRef, AddonSpec, BuildError, FieldKind, FieldRef, ModuleName, ParamSource, TableName,
};
use crate::action::{ActionContext, ApiResponse, DynAction, Request, ResponseAttachment};
use crate::error::BaseError;
use crate::router::middleware::{
    authorize, AuthorizationPolicy, MiddlewareRole, Next, PermissionGroup,
};
use crate::table::{RelationOptionsRequest, RelationOptionsResponse, TableDefinition};
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
    table_views: Vec<RuntimeTableView>,
    modules: Vec<RuntimeModule>,
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

#[derive(Clone)]
struct RuntimeTableView {
    module: String,
    view_id: String,
    title: String,
    table: String,
    columns: Arc<[RuntimeTableColumn]>,
    data_action: Option<ActionHandle>,
    actions: Arc<[RuntimeViewAction]>,
    tree: Option<RuntimeTreeView>,
    default_sort: Arc<[RuntimeTableSort]>,
    policy: AuthorizationPolicy,
}

#[derive(Clone)]
struct RuntimeTreeView {
    schema: super::TreeViewSchema,
    fields: [RuntimeTableColumn; 3],
}

#[derive(Clone)]
struct RuntimeTableSort {
    schema: super::TableSortSchema,
    column: RuntimeTableColumn,
}

#[derive(Clone)]
struct RuntimeViewAction {
    handle: ActionHandle,
    presentation: super::ActionPresentationSpec,
}

#[derive(Clone)]
struct RuntimeTableColumn {
    schema: super::TableColumnSchema,
    relation: Option<RuntimeRelationOptions>,
    validation: Option<super::FormFieldValidationSchema>,
    readable: super::AccessRule,
    writable: super::AccessRule,
    secret: bool,
    server_managed: bool,
}

#[derive(Clone)]
struct RuntimeRelationOptions {
    schema: super::RelationOptionsSchema,
    policy: AuthorizationPolicy,
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

    pub(crate) fn ui_catalog(
        &self,
        context: &ActionContext,
    ) -> Result<super::UiCatalog, BaseError> {
        let actions = self
            .handlers
            .iter()
            .filter(|runtime| runtime.policy.allows(context))
            .map(|runtime| runtime.ui_schema.clone());
        let table_views = self
            .table_views
            .iter()
            .filter(|view| view.policy.allows(context))
            .filter_map(|view| {
                let data_runtime = view
                    .data_action
                    .and_then(|handle| self.handlers.get(handle.slot()))
                    .filter(|runtime| runtime.policy.allows(context))?;
                let allowed_actions = view
                    .actions
                    .iter()
                    .filter_map(|action| {
                        self.handlers
                            .get(action.handle.slot())
                            .filter(|runtime| runtime.policy.allows(context))
                            .map(|runtime| (runtime, &action.presentation))
                    })
                    .collect::<Vec<_>>();
                Some(super::TableViewSchema {
                    view_id: view.view_id.clone(),
                    title: view.title.clone(),
                    table: view.table.clone(),
                    data_action: data_runtime.ui_schema.operation_id.clone(),
                    columns: view
                        .columns
                        .iter()
                        .filter(|column| column_readable(column, context))
                        .map(|column| table_column_schema(column, context))
                        .collect(),
                    form: super::FormSchema {
                        fields: view
                            .columns
                            .iter()
                            .filter_map(|column| form_field(column, context))
                            .collect(),
                    },
                    tree: project_tree(view, context),
                    query: project_table_query(view, context),
                    actions: allowed_actions
                        .iter()
                        .map(|(runtime, _)| runtime.ui_schema.operation_id.clone())
                        .collect(),
                    action_presentations: allowed_actions
                        .into_iter()
                        .map(|(runtime, presentation)| super::ActionPresentationSchema {
                            operation_id: runtime.ui_schema.operation_id.clone(),
                            title: runtime.ui_schema.title.clone(),
                            placement: presentation.placement,
                            interaction: presentation.interaction,
                            confirmation: presentation.confirmation.clone(),
                            availability: presentation.availability.clone(),
                            view_id: presentation.view_id.clone(),
                            record_parameter: presentation.record_parameter.clone(),
                        })
                        .collect(),
                })
            });
        let table_views = table_views.collect::<Vec<_>>();
        let visible_views = table_views
            .iter()
            .map(|view| view.view_id.clone())
            .collect::<HashSet<_>>();
        let modules = self.modules.iter().filter_map(|module| {
            let primary_action = module
                .primary_action
                .and_then(|handle| self.handlers.get(handle.slot()))
                .filter(|runtime| runtime.policy.allows(context))
                .map(|runtime| runtime.ui_schema.operation_id.clone());
            if module.primary_action.is_some() && primary_action.is_none() {
                return None;
            }
            let allowed_actions = module
                .actions
                .iter()
                .filter_map(|action| {
                    self.handlers
                        .get(action.handle.slot())
                        .filter(|runtime| runtime.policy.allows(context))
                        .map(|runtime| (runtime, &action.presentation))
                })
                .collect::<Vec<_>>();
            let views = self
                .table_views
                .iter()
                .filter(|view| view.module == module.module_id)
                .filter(|view| visible_views.contains(&view.view_id))
                .map(|view| view.view_id.clone())
                .collect::<Vec<_>>();
            if primary_action.is_none() && allowed_actions.is_empty() && views.is_empty() {
                return None;
            }
            Some(super::ModulePresentationSchema {
                module_id: module.module_id.clone(),
                identity: module.identity.clone(),
                title: module.title.clone(),
                description: module.description.clone(),
                icon: module.icon.clone(),
                order: module.order,
                primary_action,
                actions: allowed_actions
                    .iter()
                    .map(|(runtime, _)| runtime.ui_schema.operation_id.clone())
                    .collect(),
                action_presentations: allowed_actions
                    .into_iter()
                    .map(|(runtime, presentation)| super::ActionPresentationSchema {
                        operation_id: runtime.ui_schema.operation_id.clone(),
                        title: runtime.ui_schema.title.clone(),
                        placement: presentation.placement,
                        interaction: presentation.interaction,
                        confirmation: presentation.confirmation.clone(),
                        availability: presentation.availability.clone(),
                        view_id: presentation.view_id.clone(),
                        record_parameter: presentation.record_parameter.clone(),
                    })
                    .collect(),
                views,
            })
        });
        super::UiCatalog::new(actions)?
            .with_table_views(table_views)?
            .with_modules(modules)
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
        context = context.with_dispatch_target(runtime.module.clone(), runtime.action.clone());
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
            operation = %runtime.ui_schema.operation_id,
            module = %runtime.module,
            action = %runtime.action,
            request_id = tracing::field::Empty,
            actor_id = tracing::field::Empty,
            tenant_scope = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            result = tracing::field::Empty,
            error_code = tracing::field::Empty,
            duration_ms = tracing::field::Empty,
        );
        span.record("request_id", tracing::field::display(context.request_id));
        let result = next.run(context).instrument(span).await;
        if let Ok(response) = &result {
            warn_response_kind_mismatch(runtime, response);
        }
        result
    }

    /// 使用强类型输入直接调用预解析 Action；不发生 JSON 序列化或名称查找。
    ///
    /// # 约束范围（重要）
    ///
    /// 内部调用**不经过中间件链**：`StepUpMiddleware`、租户解析等中间件仅在
    /// [`Registry::dispatch`] 路径执行，本方法（以及 `Plugins::api_run`）只做
    /// 权限校验后直接调用 Handler。敏感 Action 的 step-up 因此不约束内部调用；
    /// 调用方是受信代码，如需重认证必须自行编排 `StepUpManager`。该语义由
    /// `internal_call_bypasses_step_up_middleware_by_design` 测试锁定。
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
        context = context.with_dispatch_target(runtime.module.clone(), runtime.action.clone());
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

#[derive(Clone)]
struct RuntimeModule {
    module_id: String,
    identity: super::AccountIdentitySchema,
    title: String,
    description: String,
    icon: String,
    order: i32,
    primary_action: Option<ActionHandle>,
    actions: Arc<[RuntimeViewAction]>,
}

/// 比对运行时响应与 Action 声明的 `response_kind`，不一致时仅告警。
///
/// `response_kind` 是前端契约（UI 目录/OpenAPI 的投影来源），运行时附件才是真实响应；
/// 两者脱节属于 Action 实现与声明不符的契约缺陷，但不应升级为可用性故障，
/// 因此只 `warn`、不阻断、不改变响应。错误响应没有成功响应契约可比，跳过检查。
fn warn_response_kind_mismatch(runtime: &RuntimeAction, response: &ApiResponse) {
    let actual = match &response.attachment {
        Some(ResponseAttachment::Download { .. }) => super::ActionResponseKind::Download,
        Some(ResponseAttachment::Preview { .. }) => super::ActionResponseKind::Preview,
        Some(ResponseAttachment::Redirect { .. }) => super::ActionResponseKind::Redirect,
        None => super::ActionResponseKind::Json,
    };
    let declared = runtime.ui_schema.response_kind;
    if declared != actual {
        tracing::warn!(
            module = %runtime.module,
            action = %runtime.action,
            declared = ?declared,
            actual = ?actual,
            "Action 声明的 response_kind 与运行时实际响应类别不一致"
        );
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
    /// Action 和 View 均复用 dispatch 的构建期冻结授权策略；TableView 列同时按
    /// 字段读取角色过滤，secret 字段始终 fail-closed。租户数据范围仍由实际
    /// TableQuery 独立强制执行，前端目录不能替代服务端数据隔离。
    pub fn ui_catalog(&self, context: &ActionContext) -> Result<super::UiCatalog, BaseError> {
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
        validate_middleware_order(&self.addons)?;
        resolve_param_fields(&mut self.addons)?;

        let fields = collect_fields(&self.addons)?;
        let actions = collect_actions(&self.addons)?;
        let views = collect_views(&self.addons)?;
        validate_references(&self.addons, &fields, &actions, &views)?;
        validate_routes(&self.addons)?;

        sort_definitions(&mut self.addons);
        let mut registry = build_registry(&self.addons)?;
        let compiled_views = compile_views(&self.addons, &registry)?;
        registry.table_views = compile_runtime_table_views(&self.addons, &registry)?;
        registry.modules = compile_runtime_modules(&self.addons, &registry)?;
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
            if param.presentation.widget.is_none() {
                param.presentation.widget = field.presentation.widget;
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
                None,
                Vec::new(),
                None,
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
            let data_action = view
                .data_action
                .as_ref()
                .map(|action| {
                    registry
                        .resolve(action)
                        .ok_or_else(|| BuildError::InvalidReference {
                            kind: "View Data Action",
                            reference: action.to_string(),
                        })
                })
                .transpose()?;
            let tree = view
                .tree
                .as_ref()
                .map(|tree| {
                    Ok(super::CompiledTreeView::new(
                        checked_runtime_field(tree.id_field.table(), tree.id_field.field())?,
                        checked_runtime_field(
                            tree.parent_field.table(),
                            tree.parent_field.field(),
                        )?,
                        checked_runtime_field(tree.label_field.table(), tree.label_field.field())?,
                        tree.max_nodes
                            .unwrap_or(crate::table::DEFAULT_TREE_MAX_NODES),
                    ))
                })
                .transpose()?;
            compiled.push(super::CompiledTableView::new(
                module.name.clone(),
                view.name.clone(),
                table_ref.clone(),
                fields,
                data_action,
                actions,
                tree,
            ));
        }
    }
    Ok(compiled)
}

fn compile_runtime_table_views(
    addons: &[AddonSpec],
    registry: &Registry,
) -> Result<Vec<RuntimeTableView>, BuildError> {
    let fields = addons
        .iter()
        .flat_map(|addon| &addon.modules)
        .filter_map(|module| module.table.as_ref())
        .flat_map(|table| {
            table
                .fields
                .iter()
                .map(move |field| (FieldRef::new(table.name.clone(), field.name.clone()), field))
        })
        .collect::<BTreeMap<_, _>>();
    let mut compiled = Vec::new();

    for module in addons.iter().flat_map(|addon| &addon.modules) {
        let Some(table) = &module.table else {
            continue;
        };
        let policy = module_view_policy(module);
        if module.views.is_empty() {
            let columns = table
                .fields
                .iter()
                .map(|field| runtime_table_column(field, registry))
                .collect::<Result<Vec<_>, _>>()?;
            compiled.push(RuntimeTableView {
                module: module.name.to_string(),
                view_id: format!("{}.default", module.name),
                title: if table.title.is_empty() {
                    module.name.to_string()
                } else {
                    table.title.clone()
                },
                table: table.name.to_string(),
                columns: columns.into(),
                data_action: None,
                actions: Arc::from(Vec::new()),
                tree: None,
                default_sort: Arc::from(Vec::new()),
                policy,
            });
            continue;
        }

        for view in &module.views {
            let columns = view
                .fields
                .iter()
                .map(|reference| {
                    let field = fields.get(reference).copied().ok_or_else(|| {
                        BuildError::InvalidReference {
                            kind: "View Field",
                            reference: reference.to_string(),
                        }
                    })?;
                    runtime_table_column(field, registry)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let actions = view
                .actions
                .iter()
                .map(|reference| {
                    let handle = registry.resolve(reference).ok_or_else(|| {
                        BuildError::InvalidReference {
                            kind: "View Action",
                            reference: reference.to_string(),
                        }
                    })?;
                    let runtime = registry.handlers.get(handle.slot()).ok_or_else(|| {
                        BuildError::InvalidReference {
                            kind: "View Action",
                            reference: reference.to_string(),
                        }
                    })?;
                    let presentation = view
                        .action_presentations
                        .get(reference)
                        .cloned()
                        .unwrap_or_else(|| infer_action_presentation(runtime));
                    validate_action_presentation(reference, &presentation, runtime)?;
                    Ok(RuntimeViewAction {
                        handle,
                        presentation,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let data_action = view
                .data_action
                .as_ref()
                .map(|reference| {
                    let handle = registry.resolve(reference).ok_or_else(|| {
                        BuildError::InvalidReference {
                            kind: "View Data Action",
                            reference: reference.to_string(),
                        }
                    })?;
                    let runtime = registry.handlers.get(handle.slot()).ok_or_else(|| {
                        BuildError::InvalidReference {
                            kind: "View Data Action",
                            reference: reference.to_string(),
                        }
                    })?;
                    if runtime.ui_schema.response_kind != super::ActionResponseKind::Json {
                        return Err(BuildError::InvalidReference {
                            kind: "View Data Action",
                            reference: format!("{reference}: 数据 Action 必须返回 JSON"),
                        });
                    }
                    Ok(handle)
                })
                .transpose()?;
            let tree = view
                .tree
                .as_ref()
                .map(|tree| {
                    let column = |reference: &FieldRef| {
                        let field = fields.get(reference).copied().ok_or_else(|| {
                            BuildError::InvalidReference {
                                kind: "Tree View Field",
                                reference: reference.to_string(),
                            }
                        })?;
                        runtime_table_column(field, registry)
                    };
                    Ok(RuntimeTreeView {
                        schema: super::TreeViewSchema {
                            id_field: tree.id_field.field().to_string(),
                            parent_field: tree.parent_field.field().to_string(),
                            label_field: tree.label_field.field().to_string(),
                            max_nodes: tree
                                .max_nodes
                                .unwrap_or(crate::table::DEFAULT_TREE_MAX_NODES),
                        },
                        fields: [
                            column(&tree.id_field)?,
                            column(&tree.parent_field)?,
                            column(&tree.label_field)?,
                        ],
                    })
                })
                .transpose()?;
            let mut seen_sort_fields = BTreeSet::new();
            let default_sort = view
                .default_sort
                .iter()
                .map(|sort| {
                    if !view.fields.contains(&sort.field) || !seen_sort_fields.insert(&sort.field) {
                        return Err(BuildError::InvalidReference {
                            kind: "View Default Sort",
                            reference: sort.field.to_string(),
                        });
                    }
                    let field = fields.get(&sort.field).copied().ok_or_else(|| {
                        BuildError::InvalidReference {
                            kind: "View Default Sort",
                            reference: sort.field.to_string(),
                        }
                    })?;
                    if !field.access.sortable {
                        return Err(BuildError::InvalidReference {
                            kind: "View Default Sort",
                            reference: format!("{}: 字段未允许排序", sort.field),
                        });
                    }
                    Ok(RuntimeTableSort {
                        schema: super::TableSortSchema {
                            field: sort.field.field().to_string(),
                            direction: sort.direction,
                        },
                        column: runtime_table_column(field, registry)?,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            compiled.push(RuntimeTableView {
                module: module.name.to_string(),
                view_id: format!("{}.{}", module.name, view.name),
                title: view.title.clone(),
                table: table.name.to_string(),
                columns: columns.into(),
                data_action,
                actions: actions.into(),
                tree,
                default_sort: default_sort.into(),
                policy: policy.clone(),
            });
        }
    }
    Ok(compiled)
}

fn compile_runtime_modules(
    addons: &[AddonSpec],
    registry: &Registry,
) -> Result<Vec<RuntimeModule>, BuildError> {
    let mut compiled = Vec::new();
    let mut identities = BTreeMap::<String, super::AccountIdentitySchema>::new();

    for module in addons.iter().flat_map(|addon| &addon.modules) {
        let Some(presentation) = &module.presentation else {
            continue;
        };
        validate_presentation_text("Identity title", &presentation.identity.title, 100)?;
        validate_presentation_text("Module title", &presentation.title, 100)?;
        if presentation.description.chars().count() > 500 {
            return Err(BuildError::InvalidReference {
                kind: "Module Presentation",
                reference: format!("{}: description 最多 500 字符", module.name),
            });
        }
        if !is_semantic_token(&presentation.identity.id) {
            return Err(BuildError::InvalidReference {
                kind: "Module Presentation",
                reference: format!("{}: identity id 必须是语义 token", module.name),
            });
        }
        for (kind, token) in [
            ("identity icon", presentation.identity.icon.as_str()),
            ("icon", presentation.icon.as_str()),
        ] {
            if !is_semantic_token(token) {
                return Err(BuildError::InvalidReference {
                    kind: "Module Presentation",
                    reference: format!("{}: {kind} 必须是语义 token", module.name),
                });
            }
        }

        let identity = super::AccountIdentitySchema {
            id: presentation.identity.id.clone(),
            title: presentation.identity.title.clone(),
            icon: presentation.identity.icon.clone(),
            order: presentation.identity.order,
        };
        if let Some(existing) = identities.get(&identity.id) {
            if existing != &identity {
                return Err(BuildError::InvalidReference {
                    kind: "Module Presentation",
                    reference: format!(
                        "{}: identity {} 的 title/icon/order 声明不一致",
                        module.name, identity.id
                    ),
                });
            }
        } else {
            identities.insert(identity.id.clone(), identity.clone());
        }

        let resolve_owned_action = |reference: &ActionRef| -> Result<ActionHandle, BuildError> {
            if reference.module() != &module.name {
                return Err(BuildError::InvalidReference {
                    kind: "Module Presentation Action",
                    reference: format!("{}: 只能引用当前 Module Action", reference),
                });
            }
            registry
                .resolve(reference)
                .ok_or_else(|| BuildError::InvalidReference {
                    kind: "Module Presentation Action",
                    reference: reference.to_string(),
                })
        };
        let primary_action = presentation
            .primary_action
            .as_ref()
            .map(resolve_owned_action)
            .transpose()?;
        let actions = presentation
            .action_presentations
            .iter()
            .map(|(reference, action_presentation)| {
                let handle = resolve_owned_action(reference)?;
                let runtime = registry.handlers.get(handle.slot()).ok_or_else(|| {
                    BuildError::InvalidReference {
                        kind: "Module Presentation Action",
                        reference: reference.to_string(),
                    }
                })?;
                validate_action_presentation(reference, action_presentation, runtime)?;
                Ok(RuntimeViewAction {
                    handle,
                    presentation: action_presentation.clone(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        compiled.push(RuntimeModule {
            module_id: module.name.to_string(),
            identity,
            title: presentation.title.clone(),
            description: presentation.description.clone(),
            icon: presentation.icon.clone(),
            order: presentation.order,
            primary_action,
            actions: actions.into(),
        });
    }
    Ok(compiled)
}

fn validate_presentation_text(
    kind: &'static str,
    value: &str,
    max_chars: usize,
) -> Result<(), BuildError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > max_chars {
        return Err(BuildError::InvalidReference {
            kind,
            reference: format!("文本必须在 1..={max_chars} 字符"),
        });
    }
    Ok(())
}

fn is_semantic_token(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn project_tree(view: &RuntimeTableView, context: &ActionContext) -> Option<super::TreeViewSchema> {
    let tree = view.tree.as_ref()?;
    tree.fields
        .iter()
        .all(|column| column_readable(column, context))
        .then(|| tree.schema.clone())
}

fn project_table_query(
    view: &RuntimeTableView,
    context: &ActionContext,
) -> super::TableQuerySchema {
    let readable_columns = view
        .columns
        .iter()
        .filter(|column| column_readable(column, context));
    let search_fields = readable_columns
        .clone()
        .filter(|column| column.schema.searchable)
        .map(|column| column.schema.field.clone())
        .collect();
    let filter_fields = readable_columns
        .filter(|column| column.schema.filterable)
        .map(|column| column.schema.field.clone())
        .collect();
    let default_sort = view
        .default_sort
        .iter()
        .filter(|sort| column_readable(&sort.column, context))
        .map(|sort| sort.schema.clone())
        .collect();
    super::TableQuerySchema {
        search_fields,
        filter_fields,
        default_sort,
        default_page_size: crate::table::DEFAULT_QUERY_PAGE_SIZE,
        max_page_size: crate::table::MAX_TABLE_QUERY_PAGE_SIZE,
    }
}

fn infer_action_presentation(runtime: &RuntimeAction) -> super::ActionPresentationSpec {
    let interaction = match runtime.ui_schema.response_kind {
        super::ActionResponseKind::Download => super::ActionInteraction::Download,
        super::ActionResponseKind::Preview => super::ActionInteraction::Preview,
        super::ActionResponseKind::Redirect => super::ActionInteraction::Navigate,
        super::ActionResponseKind::Json => match runtime.ui_schema.method.as_str() {
            "POST" | "PUT" | "PATCH" => super::ActionInteraction::Form,
            _ => super::ActionInteraction::Invoke,
        },
    };
    super::ActionPresentationSpec::new(super::ActionPlacement::Toolbar, interaction)
}

fn validate_action_presentation(
    action: &ActionRef,
    presentation: &super::ActionPresentationSpec,
    runtime: &RuntimeAction,
) -> Result<(), BuildError> {
    let invalid = |reason: &str| BuildError::InvalidReference {
        kind: "Action Presentation",
        reference: format!("{action}: {reason}"),
    };

    match (presentation.interaction, presentation.view_id.as_deref()) {
        (super::ActionInteraction::Custom, Some(view_id)) => {
            ModuleName::new(view_id).map_err(|_| invalid("custom view_id 必须是稳定限定标识"))?;
        }
        (super::ActionInteraction::Custom, None) => {
            return Err(invalid("custom 交互缺少 view_id"));
        }
        (_, Some(_)) => {
            return Err(invalid("只有 custom 交互可以声明 view_id"));
        }
        (_, None) => {}
    }

    match (
        presentation.placement,
        presentation.record_parameter.as_deref(),
    ) {
        (super::ActionPlacement::Row, Some(parameter)) => {
            if parameter.trim().is_empty()
                || !(runtime
                    .ui_schema
                    .params
                    .iter()
                    .any(|candidate| candidate.name == parameter)
                    || runtime
                        .ui_schema
                        .input_schema
                        .get("properties")
                        .and_then(serde_json::Value::as_object)
                        .is_some_and(|properties| properties.contains_key(parameter)))
            {
                return Err(invalid(
                    "row 展示必须声明 Action 中真实存在的 record_parameter",
                ));
            }
        }
        (super::ActionPlacement::Row, None) => {
            return Err(invalid("row 展示缺少 record_parameter"));
        }
        (_, Some(_)) => {
            return Err(invalid("只有 row 展示可以声明 record_parameter"));
        }
        (_, None) => {}
    }

    if let Some(confirmation) = &presentation.confirmation {
        for text in [&confirmation.title, &confirmation.message] {
            let trimmed = text.trim();
            if trimmed.is_empty() || trimmed.chars().count() > 500 {
                return Err(invalid("confirmation title/message 必须在 1..=500 字符"));
            }
        }
    }

    if let Some(availability) = &presentation.availability {
        let reason = availability.reason.trim();
        if reason.is_empty() || reason.chars().count() > 500 {
            return Err(invalid("availability reason 必须在 1..=500 字符"));
        }
    }

    let expected_response = match presentation.interaction {
        super::ActionInteraction::Download => Some(super::ActionResponseKind::Download),
        super::ActionInteraction::Preview => Some(super::ActionResponseKind::Preview),
        super::ActionInteraction::Navigate => Some(super::ActionResponseKind::Redirect),
        super::ActionInteraction::Form | super::ActionInteraction::Invoke => {
            Some(super::ActionResponseKind::Json)
        }
        super::ActionInteraction::Custom => None,
    };
    if let Some(expected) = expected_response {
        if runtime.ui_schema.response_kind != expected {
            return Err(invalid("交互方式与 Action 响应类型不一致"));
        }
    }
    Ok(())
}

fn module_view_policy(module: &super::ModuleSpec) -> AuthorizationPolicy {
    let has_module_permissions = !module.default_permissions.is_empty();
    let groups = if !has_module_permissions {
        Vec::new()
    } else {
        vec![PermissionGroup::new(
            "模块",
            Arc::<[String]>::from(module.default_permissions.clone()),
            module.default_permission_mode,
        )]
    };
    // 模块未声明额外权限时，View 是否可见由其 data_action 的策略决定；否则
    // 这里若仍强制登录，会错误隐藏显式 public 的数据页。
    AuthorizationPolicy::new(!has_module_permissions, groups)
}

fn runtime_table_column(
    field: &super::FieldSpec,
    registry: &Registry,
) -> Result<RuntimeTableColumn, BuildError> {
    let name = field.name.to_string();
    let relation = match (&field.select, &field.relation) {
        (Some(select), Some(target)) => {
            let handle = registry
                .resolve(select)
                .ok_or_else(|| BuildError::InvalidReference {
                    kind: "Relation Options Action",
                    reference: select.to_string(),
                })?;
            let runtime = registry.handlers.get(handle.slot()).ok_or_else(|| {
                BuildError::InvalidReference {
                    kind: "Relation Options Action",
                    reference: select.to_string(),
                }
            })?;
            // I-4：UI 目录向前端承诺 select Action 讲
            // RelationOptionsRequest/RelationOptionsResponse；构建期按 TypeId 强制
            // 输入/输出签名，签名不符的 Action 不得接入关系选择器。
            if runtime.handler.input_type_id() != TypeId::of::<RelationOptionsRequest>()
                || runtime.handler.output_type_id() != TypeId::of::<RelationOptionsResponse>()
            {
                return Err(BuildError::InvalidReference {
                    kind: "Relation Options Action",
                    reference: format!(
                        "{select}: 输入/输出必须是 RelationOptionsRequest/RelationOptionsResponse"
                    ),
                });
            }
            Some(RuntimeRelationOptions {
                schema: super::RelationOptionsSchema {
                    operation_id: runtime.ui_schema.operation_id.clone(),
                    value_field: target.to_string(),
                    label_fields: field
                        .presentation
                        .display
                        .iter()
                        .map(ToString::to_string)
                        .collect(),
                },
                policy: runtime.policy.clone(),
            })
        }
        (Some(select), None) => {
            return Err(BuildError::InvalidReference {
                kind: "Relation Options Field",
                reference: format!("{}: {select}", field.name),
            });
        }
        (None, _) => None,
    };
    Ok(RuntimeTableColumn {
        schema: super::TableColumnSchema {
            title: if field.presentation.title.is_empty() {
                name.clone()
            } else {
                field.presentation.title.clone()
            },
            field: name,
            description: field.presentation.description.clone(),
            widget: field.widget_hint(),
            required: field.is_required(),
            searchable: field.access.searchable,
            filterable: field.access.filterable,
            sortable: field.access.sortable,
            relation: None,
        },
        relation,
        validation: super::FormFieldValidationSchema::from_spec(&field.validation),
        readable: field.access.readable.clone(),
        writable: field.access.writable.clone(),
        secret: field.access.secret,
        server_managed: field.kind == FieldKind::Key
            || (field.kind == FieldKind::Timestamp
                && field.timestamp_mode != super::TimestampMode::Value),
    })
}

fn table_column_schema(
    column: &RuntimeTableColumn,
    context: &ActionContext,
) -> super::TableColumnSchema {
    let mut schema = column.schema.clone();
    schema.relation = column
        .relation
        .as_ref()
        .filter(|relation| relation.policy.allows(context))
        .map(|relation| relation.schema.clone());
    schema
}

fn column_readable(column: &RuntimeTableColumn, context: &ActionContext) -> bool {
    !column.secret && access_rule_allows(&column.readable, context)
}

fn form_field(
    column: &RuntimeTableColumn,
    context: &ActionContext,
) -> Option<super::FormFieldSchema> {
    let readable = column_readable(column, context);
    let writable = !column.server_managed && access_rule_allows(&column.writable, context);
    if !readable && !writable {
        return None;
    }
    Some(super::FormFieldSchema {
        field: column.schema.field.clone(),
        title: column.schema.title.clone(),
        description: column.schema.description.clone(),
        widget: column.schema.widget,
        required: column.schema.required && writable,
        read_only: !writable,
        write_only: column.secret || !readable,
        relation: column
            .relation
            .as_ref()
            .filter(|relation| relation.policy.allows(context))
            .map(|relation| relation.schema.clone()),
        validation: column.validation.clone(),
    })
}

fn access_rule_allows(rule: &super::AccessRule, context: &ActionContext) -> bool {
    match rule {
        super::AccessRule::Everyone => true,
        super::AccessRule::Nobody => false,
        super::AccessRule::Roles(roles) => context
            .user_roles_set()
            .is_some_and(|user_roles| roles.iter().any(|role| user_roles.contains(role))),
    }
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
                // I-5：服务端关键词搜索只会命中文本列；允许非文本字段声明
                // searchable 等于让 UI 投影的 search_fields 说谎，构建期拒绝。
                if field.access.searchable
                    && !matches!(field.kind, FieldKind::Str | FieldKind::Text)
                {
                    return Err(BuildError::InvalidFieldDefinition {
                        table: table.name.to_string(),
                        field: field.name.to_string(),
                        reason: "只有文本字段（Str/Text）可以声明 searchable".to_string(),
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
            validate_action_media(module, action)?;
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

fn validate_middleware_order(addons: &[AddonSpec]) -> Result<(), BuildError> {
    for module in addons.iter().flat_map(|addon| &addon.modules) {
        let mut authentication_dependent_seen = None;
        for middleware in module.middlewares() {
            match middleware.role() {
                MiddlewareRole::TenantResolution => {
                    authentication_dependent_seen = Some("TenantResolverMiddleware")
                }
                MiddlewareRole::StepUpProtection => {
                    authentication_dependent_seen = Some("StepUpMiddleware")
                }
                MiddlewareRole::Authentication if authentication_dependent_seen.is_some() => {
                    return Err(BuildError::InvalidReference {
                        kind: "Middleware order",
                        reference: format!(
                            "{}: TokenAuthMiddleware 必须先于 {} 注册",
                            module.name,
                            authentication_dependent_seen.unwrap_or("身份依赖中间件")
                        ),
                    });
                }
                MiddlewareRole::Unspecified | MiddlewareRole::Authentication => {}
            }
        }
    }
    Ok(())
}

fn validate_action_media(
    module: &super::ModuleSpec,
    action: &super::ActionSpec,
) -> Result<(), BuildError> {
    let invalid = |reason: &str| BuildError::InvalidReference {
        kind: "Action request media",
        reference: format!("{}.{} -> {reason}", module.name, action.name),
    };
    // C-1 双向强制：二进制文件字段（UploadedFile，schema 形态为 format: binary）
    // 只能出现在 multipart Action 中，multipart Action 也必须至少声明一个文件字段。
    let has_binary_input = schema_contains_binary_field(&action.input_schema);
    match (action.request_media_type, action.multipart.as_ref()) {
        (super::ActionMediaType::Json, None) => {
            if has_binary_input {
                return Err(invalid(
                    "JSON Action 的输入不得包含二进制文件字段（应声明 multipart 请求）",
                ));
            }
            return Ok(());
        }
        (super::ActionMediaType::Json, Some(_)) => {
            return Err(invalid("JSON Action 不得携带 multipart 配置"));
        }
        (super::ActionMediaType::Multipart, None) => {
            return Err(invalid("multipart Action 缺少资源限制"));
        }
        (super::ActionMediaType::Multipart, Some(_)) => {}
    }
    if !matches!(
        action.route.method,
        super::HttpMethod::Post | super::HttpMethod::Put | super::HttpMethod::Patch
    ) {
        return Err(invalid("multipart 只允许 POST/PUT/PATCH"));
    }

    let multipart = action
        .multipart
        .as_ref()
        .ok_or_else(|| invalid("multipart Action 缺少资源限制"))?;
    if multipart.max_files == 0 {
        return Err(invalid("max_files 必须大于 0"));
    }
    if multipart.max_file_bytes == 0 || multipart.max_total_bytes == 0 {
        return Err(invalid("文件与请求字节上限必须大于 0"));
    }
    if multipart.max_file_bytes > multipart.max_total_bytes {
        return Err(invalid("max_file_bytes 不能大于 max_total_bytes"));
    }
    if multipart.max_text_field_bytes == 0 {
        return Err(invalid("文本字段字节上限必须大于 0"));
    }
    if multipart.max_text_field_bytes > multipart.max_total_bytes {
        return Err(invalid("max_text_field_bytes 不能大于 max_total_bytes"));
    }
    if multipart.allowed_content_types.is_empty() {
        return Err(invalid("allowed_content_types 不能为空"));
    }
    let mut content_types = BTreeSet::new();
    for content_type in &multipart.allowed_content_types {
        if !is_exact_mime_type(content_type) {
            return Err(invalid("allowed_content_types 必须是小写精确 MIME 类型"));
        }
        if !content_types.insert(content_type) {
            return Err(invalid("allowed_content_types 不能重复"));
        }
    }
    if !has_binary_input {
        return Err(invalid("multipart Action 的输入必须至少声明一个文件字段"));
    }
    Ok(())
}

/// 递归检测 JSON Schema 是否声明二进制文件字段（`format: "binary"`，
/// 即 `UploadedFile` 经 schemars 生成的形态；`$ref`/`anyOf`/`items` 递归覆盖）。
pub(crate) fn schema_contains_binary_field(schema: &serde_json::Value) -> bool {
    schema_subtree_contains_binary(schema, schema)
}

/// 递归检测 schema 子树是否声明二进制文件字段；本地 `$ref`（`#/definitions/...`）
/// 按 document 解析后继续深入，带循环保护（自引用类型不会死循环）。
pub(crate) fn schema_subtree_contains_binary(
    document: &serde_json::Value,
    subtree: &serde_json::Value,
) -> bool {
    fn inner(
        document: &serde_json::Value,
        subtree: &serde_json::Value,
        visited: &mut BTreeSet<String>,
    ) -> bool {
        match subtree {
            serde_json::Value::Object(map) => {
                if map.get("format").and_then(serde_json::Value::as_str) == Some("binary") {
                    return true;
                }
                if let Some(name) = map
                    .get("$ref")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|reference| reference.strip_prefix("#/definitions/"))
                {
                    if visited.insert(name.to_string()) {
                        if let Some(target) = document
                            .get("definitions")
                            .and_then(|definitions| definitions.get(name))
                        {
                            if inner(document, target, visited) {
                                return true;
                            }
                        }
                    }
                }
                map.values().any(|value| inner(document, value, visited))
            }
            serde_json::Value::Array(items) => {
                items.iter().any(|item| inner(document, item, visited))
            }
            _ => false,
        }
    }
    inner(document, subtree, &mut BTreeSet::new())
}

fn is_exact_mime_type(value: &str) -> bool {
    let Some((top, subtype)) = value.split_once('/') else {
        return false;
    };
    !top.is_empty()
        && !subtype.is_empty()
        && !subtype.contains('/')
        && top.bytes().all(is_mime_token_byte)
        && subtype.bytes().all(is_mime_token_byte)
}

fn is_mime_token_byte(value: u8) -> bool {
    value.is_ascii_lowercase()
        || value.is_ascii_digit()
        || matches!(
            value,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
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
        for middleware in module.middlewares() {
            if let Some(target) = middleware.target_action() {
                if target.module() != &module.name {
                    return Err(BuildError::InvalidReference {
                        kind: "Middleware Action",
                        reference: target.to_string(),
                    });
                }
                validate_action_ref(target, actions).map_err(|_| BuildError::InvalidReference {
                    kind: "Middleware Action",
                    reference: target.to_string(),
                })?;
            }
        }
        if let Some(table) = &module.table {
            for field in &table.fields {
                if let Some(relation) = &field.relation {
                    validate_field_ref(relation, fields)?;
                }
                if let Some(select) = &field.select {
                    if field.relation.is_none() {
                        return Err(BuildError::InvalidReference {
                            kind: "Relation Options Field",
                            reference: format!("{}.{}: {select}", table.name, field.name),
                        });
                    }
                    validate_action_ref(select, actions)?;
                }
                for display in &field.presentation.display {
                    validate_field_ref(display, fields)?;
                    let same_target_table = field
                        .relation
                        .as_ref()
                        .is_some_and(|relation| display.table() == relation.table());
                    if !same_target_table {
                        return Err(BuildError::InvalidReference {
                            kind: "Relation Display Field",
                            reference: format!("{}.{}: {display}", table.name, field.name),
                        });
                    }
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
            if let Some(tree) = &view.tree {
                let table = module
                    .table
                    .as_ref()
                    .ok_or_else(|| BuildError::InvalidReference {
                        kind: "Tree View",
                        reference: format!("{view_ref}: 缺少主表"),
                    })?;
                let tree_fields = [&tree.id_field, &tree.parent_field, &tree.label_field];
                if tree.id_field == tree.parent_field {
                    return Err(BuildError::InvalidReference {
                        kind: "Tree View",
                        reference: format!("{view_ref}: id/parent 字段必须不同"),
                    });
                }
                if tree.max_nodes == Some(0) {
                    return Err(BuildError::InvalidReference {
                        kind: "Tree View",
                        reference: format!("{view_ref}: max_nodes 必须大于 0"),
                    });
                }
                for field in tree_fields {
                    validate_field_ref(field, fields)?;
                    if field.table() != &table.name || !view.fields.contains(field) {
                        return Err(BuildError::InvalidReference {
                            kind: "Tree View Field",
                            reference: format!("{view_ref}: {field}"),
                        });
                    }
                }
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
    Ok(Registry {
        actions,
        handlers,
        table_views: Vec::new(),
        modules: Vec::new(),
    })
}

#[allow(dead_code)]
fn _module_name_type_is_used(_: &ModuleName) {}
