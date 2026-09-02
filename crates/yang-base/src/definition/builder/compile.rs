//! 构建期编译：定义 → Registry/TableDefinition/运行时视图与模块投影。

use crate::definition::{ActionRef, AddonSpec, BuildError, FieldKind, FieldRef, ModuleName};
use crate::router::middleware::{AuthorizationPolicy, PermissionGroup};
use crate::table::{RelationOptionsRequest, RelationOptionsResponse, TableDefinition};
use std::any::TypeId;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::handle::ActionHandle;
use super::registry::{
    Registry, RuntimeAction, RuntimeModule, RuntimeRelationOptions, RuntimeTableColumn,
    RuntimeTableSort, RuntimeTableView, RuntimeTreeView, RuntimeViewAction,
};

pub(super) fn compile_table_definitions(
    addons: &[AddonSpec],
) -> Result<Vec<TableDefinition>, BuildError> {
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

pub(super) fn compile_views(
    addons: &[AddonSpec],
    registry: &Registry,
) -> Result<Vec<crate::definition::CompiledTableView>, BuildError> {
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
            let name = crate::definition::ViewName::new("default").map_err(|error| {
                BuildError::InvalidReference {
                    kind: "View",
                    reference: error.to_string(),
                }
            })?;
            let fields = table
                .fields
                .iter()
                .filter(|field| {
                    !field.access.secret
                        && !matches!(field.access.readable, crate::definition::AccessRule::Nobody)
                })
                .map(|field| checked_runtime_field(&table.name, &field.name))
                .collect::<Result<Vec<_>, _>>()?;
            compiled.push(crate::definition::CompiledTableView::new(
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
                    Ok(crate::definition::CompiledTreeView::new(
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
            compiled.push(crate::definition::CompiledTableView::new(
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

pub(super) fn compile_runtime_table_views(
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
                    if runtime.ui_schema.response_kind
                        != crate::definition::ActionResponseKind::Json
                    {
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
                        schema: crate::definition::TreeViewSchema {
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
                        schema: crate::definition::TableSortSchema {
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

pub(super) fn compile_runtime_modules(
    addons: &[AddonSpec],
    registry: &Registry,
) -> Result<Vec<RuntimeModule>, BuildError> {
    let mut compiled = Vec::new();
    let mut identities = BTreeMap::<String, crate::definition::AccountIdentitySchema>::new();

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

        let identity = crate::definition::AccountIdentitySchema {
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

fn infer_action_presentation(runtime: &RuntimeAction) -> crate::definition::ActionPresentationSpec {
    let interaction = match runtime.ui_schema.response_kind {
        crate::definition::ActionResponseKind::Download => {
            crate::definition::ActionInteraction::Download
        }
        crate::definition::ActionResponseKind::Preview => {
            crate::definition::ActionInteraction::Preview
        }
        crate::definition::ActionResponseKind::Redirect => {
            crate::definition::ActionInteraction::Navigate
        }
        crate::definition::ActionResponseKind::Json => match runtime.ui_schema.method.as_str() {
            "POST" | "PUT" | "PATCH" => crate::definition::ActionInteraction::Form,
            _ => crate::definition::ActionInteraction::Invoke,
        },
    };
    crate::definition::ActionPresentationSpec::new(
        crate::definition::ActionPlacement::Toolbar,
        interaction,
    )
}

fn validate_action_presentation(
    action: &ActionRef,
    presentation: &crate::definition::ActionPresentationSpec,
    runtime: &RuntimeAction,
) -> Result<(), BuildError> {
    let invalid = |reason: &str| BuildError::InvalidReference {
        kind: "Action Presentation",
        reference: format!("{action}: {reason}"),
    };

    match (presentation.interaction, presentation.view_id.as_deref()) {
        (crate::definition::ActionInteraction::Custom, Some(view_id)) => {
            ModuleName::new(view_id).map_err(|_| invalid("custom view_id 必须是稳定限定标识"))?;
        }
        (crate::definition::ActionInteraction::Custom, None) => {
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
        (crate::definition::ActionPlacement::Row, Some(parameter)) => {
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
        (crate::definition::ActionPlacement::Row, None) => {
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
        crate::definition::ActionInteraction::Download => {
            Some(crate::definition::ActionResponseKind::Download)
        }
        crate::definition::ActionInteraction::Preview => {
            Some(crate::definition::ActionResponseKind::Preview)
        }
        crate::definition::ActionInteraction::Navigate => {
            Some(crate::definition::ActionResponseKind::Redirect)
        }
        crate::definition::ActionInteraction::Form
        | crate::definition::ActionInteraction::Invoke => {
            Some(crate::definition::ActionResponseKind::Json)
        }
        crate::definition::ActionInteraction::Custom => None,
    };
    if let Some(expected) = expected_response {
        if runtime.ui_schema.response_kind != expected {
            return Err(invalid("交互方式与 Action 响应类型不一致"));
        }
    }
    Ok(())
}

fn module_view_policy(module: &crate::definition::ModuleSpec) -> AuthorizationPolicy {
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
    field: &crate::definition::FieldSpec,
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
                schema: crate::definition::RelationOptionsSchema {
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
        schema: crate::definition::TableColumnSchema {
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
        validation: crate::definition::FormFieldValidationSchema::from_spec(&field.validation),
        readable: field.access.readable.clone(),
        writable: field.access.writable.clone(),
        secret: field.access.secret,
        server_managed: field.kind == FieldKind::Key
            || (field.kind == FieldKind::Timestamp
                && field.timestamp_mode != crate::definition::TimestampMode::Value),
    })
}

fn checked_runtime_field(
    table: &crate::definition::TableName,
    field: &crate::definition::FieldName,
) -> Result<yang_db::FieldRef, BuildError> {
    yang_db::FieldRef::new(format!("{table}.{field}")).map_err(|error| {
        BuildError::InvalidReference {
            kind: "Field",
            reference: error.to_string(),
        }
    })
}

pub(super) fn build_registry(addons: &[AddonSpec]) -> Result<Registry, BuildError> {
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
            ui_schema: crate::definition::ActionDemoSchema::from(action),
            table_definition: table
                .map(crate::definition::TableSpec::table_definition)
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
