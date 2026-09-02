//! Registry：ActionRef 到稳定运行时 slot 的不可变映射、dispatch 与请求级 UI 目录投影。

use crate::action::{ActionContext, ApiResponse, DynAction, ResponseAttachment};
use crate::definition::ActionRef;
use crate::error::BaseError;
use crate::router::middleware::{authorize, AuthorizationPolicy, Next};
use crate::table::TableDefinition;
use std::any::TypeId;
use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use super::handle::{ActionHandle, TypedActionHandle};
use super::project::{
    column_readable, form_field, project_table_query, project_tree, table_column_schema,
};

/// ActionRef 到稳定运行时 slot 的不可变映射。
#[derive(Clone, Default)]
pub struct Registry {
    pub(super) actions: BTreeMap<ActionRef, ActionHandle>,
    pub(super) handlers: Vec<RuntimeAction>,
    pub(super) table_views: Vec<RuntimeTableView>,
    pub(super) modules: Vec<RuntimeModule>,
}

#[derive(Clone)]
pub(super) struct RuntimeAction {
    pub(super) handler: Arc<dyn DynAction>,
    pub(super) middlewares: Arc<[Arc<dyn crate::router::Middleware>]>,
    pub(super) policy: AuthorizationPolicy,
    pub(super) module: String,
    pub(super) action: String,
    pub(super) table_definition: Option<TableDefinition>,
    pub(super) ui_schema: crate::definition::ActionDemoSchema,
}

#[derive(Clone)]
pub(super) struct RuntimeTableView {
    pub(super) module: String,
    pub(super) view_id: String,
    pub(super) title: String,
    pub(super) table: String,
    pub(super) columns: Arc<[RuntimeTableColumn]>,
    pub(super) data_action: Option<ActionHandle>,
    pub(super) actions: Arc<[RuntimeViewAction]>,
    pub(super) tree: Option<RuntimeTreeView>,
    pub(super) default_sort: Arc<[RuntimeTableSort]>,
    pub(super) policy: AuthorizationPolicy,
}

#[derive(Clone)]
pub(super) struct RuntimeTreeView {
    pub(super) schema: crate::definition::TreeViewSchema,
    pub(super) fields: [RuntimeTableColumn; 3],
}

#[derive(Clone)]
pub(super) struct RuntimeTableSort {
    pub(super) schema: crate::definition::TableSortSchema,
    pub(super) column: RuntimeTableColumn,
}

#[derive(Clone)]
pub(super) struct RuntimeViewAction {
    pub(super) handle: ActionHandle,
    pub(super) presentation: crate::definition::ActionPresentationSpec,
}

#[derive(Clone)]
pub(super) struct RuntimeTableColumn {
    pub(super) schema: crate::definition::TableColumnSchema,
    pub(super) relation: Option<RuntimeRelationOptions>,
    pub(super) validation: Option<crate::definition::FormFieldValidationSchema>,
    pub(super) readable: crate::definition::AccessRule,
    pub(super) writable: crate::definition::AccessRule,
    pub(super) secret: bool,
    pub(super) server_managed: bool,
}

#[derive(Clone)]
pub(super) struct RuntimeRelationOptions {
    pub(super) schema: crate::definition::RelationOptionsSchema,
    pub(super) policy: AuthorizationPolicy,
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
    ) -> Result<crate::definition::UiCatalog, BaseError> {
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
                Some(crate::definition::TableViewSchema {
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
                    form: crate::definition::FormSchema {
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
                        .map(|(runtime, presentation)| {
                            crate::definition::ActionPresentationSchema {
                                operation_id: runtime.ui_schema.operation_id.clone(),
                                title: runtime.ui_schema.title.clone(),
                                placement: presentation.placement,
                                interaction: presentation.interaction,
                                confirmation: presentation.confirmation.clone(),
                                availability: presentation.availability.clone(),
                                view_id: presentation.view_id.clone(),
                                record_parameter: presentation.record_parameter.clone(),
                            }
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
            Some(crate::definition::ModulePresentationSchema {
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
                    .map(
                        |(runtime, presentation)| crate::definition::ActionPresentationSchema {
                            operation_id: runtime.ui_schema.operation_id.clone(),
                            title: runtime.ui_schema.title.clone(),
                            placement: presentation.placement,
                            interaction: presentation.interaction,
                            confirmation: presentation.confirmation.clone(),
                            availability: presentation.availability.clone(),
                            view_id: presentation.view_id.clone(),
                            record_parameter: presentation.record_parameter.clone(),
                        },
                    )
                    .collect(),
                views,
            })
        });
        crate::definition::UiCatalog::new(actions)?
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
pub(super) struct RuntimeModule {
    pub(super) module_id: String,
    pub(super) identity: crate::definition::AccountIdentitySchema,
    pub(super) title: String,
    pub(super) description: String,
    pub(super) icon: String,
    pub(super) order: i32,
    pub(super) primary_action: Option<ActionHandle>,
    pub(super) actions: Arc<[RuntimeViewAction]>,
}

/// 比对运行时响应与 Action 声明的 `response_kind`，不一致时仅告警。
///
/// `response_kind` 是前端契约（UI 目录/OpenAPI 的投影来源），运行时附件才是真实响应；
/// 两者脱节属于 Action 实现与声明不符的契约缺陷，但不应升级为可用性故障，
/// 因此只 `warn`、不阻断、不改变响应。错误响应没有成功响应契约可比，跳过检查。
fn warn_response_kind_mismatch(runtime: &RuntimeAction, response: &ApiResponse) {
    let actual = match &response.attachment {
        Some(ResponseAttachment::Download { .. }) => {
            crate::definition::ActionResponseKind::Download
        }
        Some(ResponseAttachment::Preview { .. }) => crate::definition::ActionResponseKind::Preview,
        Some(ResponseAttachment::Redirect { .. }) => {
            crate::definition::ActionResponseKind::Redirect
        }
        None => crate::definition::ActionResponseKind::Json,
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
