//! 构建期交叉校验、引用收集与确定性排序。

use crate::definition::{
    ActionRef, AddonSpec, BuildError, FieldKind, FieldRef, ParamSource, TableName,
};
use crate::router::middleware::MiddlewareRole;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use super::schema_contains_binary_field;

pub(super) fn resolve_param_fields(addons: &mut [AddonSpec]) -> Result<(), BuildError> {
    let definitions = addons
        .iter()
        .flat_map(|addon| &addon.modules)
        .filter_map(|module| module.table.as_ref())
        .flat_map(|table| {
            table.fields.iter().map(move |field| {
                (
                    crate::definition::FieldRef::new(table.name.clone(), field.name.clone()),
                    field.clone(),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();

    for action in addons
        .iter_mut()
        .flat_map(|addon| &mut addon.modules)
        .flat_map(crate::definition::ModuleSpec::actions_mut)
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

pub(super) fn validate_unique_addons(addons: &[AddonSpec]) -> Result<(), BuildError> {
    let mut names = BTreeSet::new();
    for addon in addons {
        insert_unique(&mut names, &addon.name, "Addon")?;
    }
    Ok(())
}

pub(super) fn validate_dependencies(addons: &[AddonSpec]) -> Result<(), BuildError> {
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

pub(super) fn validate_module_ownership(addons: &[AddonSpec]) -> Result<(), BuildError> {
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

pub(super) fn validate_unique_modules(addons: &[AddonSpec]) -> Result<(), BuildError> {
    let mut names = BTreeSet::new();
    for module in addons.iter().flat_map(|addon| &addon.modules) {
        insert_unique(&mut names, &module.name, "Module")?;
    }
    Ok(())
}

pub(super) fn validate_module_contents(addons: &[AddonSpec]) -> Result<(), BuildError> {
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

pub(super) fn validate_middleware_order(addons: &[AddonSpec]) -> Result<(), BuildError> {
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
    module: &crate::definition::ModuleSpec,
    action: &crate::definition::ActionSpec,
) -> Result<(), BuildError> {
    let invalid = |reason: &str| BuildError::InvalidReference {
        kind: "Action request media",
        reference: format!("{}.{} -> {reason}", module.name, action.name),
    };
    // C-1 双向强制：二进制文件字段（UploadedFile，schema 形态为 format: binary）
    // 只能出现在 multipart Action 中，multipart Action 也必须至少声明一个文件字段。
    let has_binary_input = schema_contains_binary_field(&action.input_schema);
    match (action.request_media_type, action.multipart.as_ref()) {
        (crate::definition::ActionMediaType::Json, None) => {
            if has_binary_input {
                return Err(invalid(
                    "JSON Action 的输入不得包含二进制文件字段（应声明 multipart 请求）",
                ));
            }
            return Ok(());
        }
        (crate::definition::ActionMediaType::Json, Some(_)) => {
            return Err(invalid("JSON Action 不得携带 multipart 配置"));
        }
        (crate::definition::ActionMediaType::Multipart, None) => {
            return Err(invalid("multipart Action 缺少资源限制"));
        }
        (crate::definition::ActionMediaType::Multipart, Some(_)) => {}
    }
    if !matches!(
        action.route.method,
        crate::definition::HttpMethod::Post
            | crate::definition::HttpMethod::Put
            | crate::definition::HttpMethod::Patch
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

pub(super) fn collect_fields(
    addons: &[AddonSpec],
) -> Result<BTreeMap<TableName, BTreeSet<crate::definition::FieldName>>, BuildError> {
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

pub(super) fn collect_actions(addons: &[AddonSpec]) -> Result<BTreeSet<ActionRef>, BuildError> {
    let mut result = BTreeSet::new();
    for module in addons.iter().flat_map(|addon| &addon.modules) {
        for action in module.actions() {
            let reference = ActionRef::new(module.name.clone(), action.name.clone());
            insert_unique(&mut result, &reference, "Action")?;
        }
    }
    Ok(result)
}

pub(super) fn collect_views(
    addons: &[AddonSpec],
) -> Result<BTreeSet<crate::definition::ViewRef>, BuildError> {
    let mut result = BTreeSet::new();
    for module in addons.iter().flat_map(|addon| &addon.modules) {
        for view in &module.views {
            let reference = crate::definition::ViewRef::new(module.name.clone(), view.name.clone());
            insert_unique(&mut result, &reference, "View")?;
        }
    }
    Ok(result)
}

fn validate_field_ref(
    reference: &FieldRef,
    fields: &BTreeMap<TableName, BTreeSet<crate::definition::FieldName>>,
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

pub(super) fn validate_references(
    addons: &[AddonSpec],
    fields: &BTreeMap<TableName, BTreeSet<crate::definition::FieldName>>,
    actions: &BTreeSet<ActionRef>,
    views: &BTreeSet<crate::definition::ViewRef>,
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
            let view_ref = crate::definition::ViewRef::new(module.name.clone(), view.name.clone());
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

pub(super) fn validate_routes(addons: &[AddonSpec]) -> Result<(), BuildError> {
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

pub(super) fn sort_definitions(addons: &mut [AddonSpec]) {
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
