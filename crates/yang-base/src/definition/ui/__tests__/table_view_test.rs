//! TableView 请求级投影测试：字段/Action 权限过滤、查询能力与服务端强制位对齐。

use super::super::{
    ActionConfirmation, ActionInteraction, ActionPlacement, ActionPresentationSpec,
    AvailabilityHint, AvailabilityState, SortDirection, TableSortSpec, WidgetHint,
};
use super::fixtures::{action, NoopAction, RelationOptionsAction};
use crate::action::{PermissionMode, Request, User};
use crate::definition::{
    AccessRule, ActionName, ActionRef, AddonName, AddonSpec, AppBuilder, FieldKind, FieldName,
    FieldRef, FieldSpec, ModuleName, ModuleSpec, ParamSource, ParamSpec, TableName, TableSpec,
    TreeViewSpec, ViewName, ViewSpec,
};
use crate::tools::ToolsBuilder;
use serde_json::json;

#[tokio::test]
async fn table_view_projection_filters_module_fields_and_actions_with_same_request_identity() {
    let module_name = ModuleName::new("org.member").expect("测试 Module 名称应有效");
    let table_name = TableName::new("org_member").expect("测试 Table 名称应有效");
    let field_ref = |name: &str| {
        FieldRef::new(
            table_name.clone(),
            FieldName::new(name).expect("测试字段名应有效"),
        )
    };
    let action_ref = |name: &str| {
        ActionRef::new(
            module_name.clone(),
            ActionName::new(name).expect("测试 Action 名称应有效"),
        )
    };

    let mut name = FieldSpec::new(
        FieldName::new("name").expect("测试字段名应有效"),
        FieldKind::Str,
    );
    name.presentation.title = "名称".to_string();
    name.access.searchable = true;
    name.access.filterable = true;
    name.access.sortable = true;
    let mut manager_id = FieldSpec::new(
        FieldName::new("manager_id").expect("测试字段名应有效"),
        FieldKind::Table,
    );
    manager_id.relation = Some(field_ref("id"));
    manager_id.select = Some(action_ref("options"));
    manager_id.presentation.display = vec![field_ref("name")];
    let mut parent_id = FieldSpec::new(
        FieldName::new("parent_id").expect("测试字段名应有效"),
        FieldKind::Int,
    );
    parent_id.access.readable = AccessRule::Roles(vec!["admin".to_string()]);
    let mut admin_note = FieldSpec::new(
        FieldName::new("admin_note").expect("测试字段名应有效"),
        FieldKind::Text,
    );
    admin_note.access.readable = AccessRule::Roles(vec!["admin".to_string()]);
    let mut secret = FieldSpec::new(
        FieldName::new("secret").expect("测试字段名应有效"),
        FieldKind::Str,
    );
    secret.access.secret = true;
    secret.access.readable = AccessRule::Everyone;
    secret.storage.required = true;
    let mut created_at = FieldSpec::new(
        FieldName::new("created_at").expect("测试字段名应有效"),
        FieldKind::Timestamp,
    );
    created_at.timestamp_mode = crate::definition::TimestampMode::CreatedAt;

    let view = ViewSpec::new(ViewName::new("main").expect("测试 View 名称应有效"))
        .data_action(action_ref("list"))
        .field(field_ref("id"))
        .field(field_ref("parent_id"))
        .field(field_ref("name"))
        .field(field_ref("manager_id"))
        .field(field_ref("admin_note"))
        .field(field_ref("secret"))
        .field(field_ref("created_at"))
        .action(action_ref("list"))
        .present_action(
            action_ref("edit"),
            ActionPresentationSpec::new(ActionPlacement::Row, ActionInteraction::Form)
                .record_parameter("id")
                .confirmation(ActionConfirmation::new("确认修改", "将保存当前行的修改"))
                .availability(AvailabilityHint::disabled("当前记录可能不允许修改")),
        )
        .tree(TreeViewSpec::new(
            field_ref("id"),
            field_ref("parent_id"),
            field_ref("name"),
        ))
        .default_sort(TableSortSpec::new(field_ref("name"), SortDirection::Asc));
    let module = ModuleSpec::new(module_name.clone())
        .table(
            TableSpec::new(table_name)
                .title("组织成员")
                .field(FieldSpec::new(
                    FieldName::new("id").expect("测试字段名应有效"),
                    FieldKind::Key,
                ))
                .field(parent_id)
                .field(name)
                .field(manager_id)
                .field(admin_note)
                .field(secret)
                .field(created_at),
        )
        .default_permissions(["module:view"], PermissionMode::All)
        .action(action("list", "org.member.list"), NoopAction)
        .action(
            action("options", "org.member.options")
                .permissions(["member:options"], PermissionMode::All),
            RelationOptionsAction,
        )
        .action(
            action("edit", "org.member.edit")
                .param(ParamSpec::new(
                    FieldName::new("id").expect("测试字段名应有效"),
                    ParamSource::Body,
                ))
                .permissions(["member:edit"], PermissionMode::All),
            NoopAction,
        )
        .view(view);
    let app = AppBuilder::new()
        .addon(AddonSpec::new(AddonName::new("org").expect("测试 Addon 名称应有效")).module(module))
        .build(ToolsBuilder::new().build().expect("测试 Tools 应构建成功"))
        .expect("TableView 测试应用应构建成功");

    let compiled_tree = app.compiled_views()[0]
        .tree()
        .expect("显式树拓扑应在启动期预编译");
    assert_eq!(compiled_tree.id_field_name(), "id");
    assert_eq!(compiled_tree.parent_field_name(), "parent_id");
    assert_eq!(compiled_tree.label_field_name(), "name");

    let anonymous = app
        .ui_catalog(&app.context(Request::new(json!({}))))
        .expect("匿名 UI Catalog revision 应可计算");
    assert!(
        anonymous.table_views.is_empty(),
        "匿名请求不得看到受保护 View"
    );

    let member = app
        .ui_catalog(
            &app.context(Request::new(json!({})))
                .with_user(User::new(7, "member").with_permissions(["module:view"])),
        )
        .expect("成员 UI Catalog revision 应可计算");
    assert_eq!(member.table_views.len(), 1);
    assert_eq!(member.table_views[0].view_id, "org.member.main");
    assert_eq!(member.table_views[0].table, "org_member");
    assert_eq!(member.table_views[0].data_action, "org.member.list");
    assert!(
        member.table_views[0].tree.is_none(),
        "任一拓扑字段不可读时必须安全降级为普通表格"
    );
    let member_wire =
        serde_json::to_value(&member.table_views[0]).expect("成员 TableView schema 应可序列化");
    assert!(
        member_wire.get("tree").is_none(),
        "不可用树拓扑不能以空壳契约泄漏给前端"
    );
    assert_eq!(
        member.table_views[0]
            .columns
            .iter()
            .map(|column| column.field.as_str())
            .collect::<Vec<_>>(),
        ["id", "name", "manager_id", "created_at"]
    );
    let name_column = &member.table_views[0].columns[1];
    assert_eq!(name_column.widget, WidgetHint::Text);
    assert!(name_column.filterable);
    assert!(name_column.sortable);
    assert_eq!(member.table_views[0].query.search_fields, ["name"]);
    assert_eq!(member.table_views[0].query.filter_fields, ["name"]);
    assert_eq!(member.table_views[0].query.default_sort.len(), 1);
    assert_eq!(member.table_views[0].query.default_sort[0].field, "name");
    assert_eq!(
        member.table_views[0].query.default_sort[0].direction,
        SortDirection::Asc
    );
    assert_eq!(
        member.table_views[0].query.default_page_size,
        crate::table::DEFAULT_QUERY_PAGE_SIZE
    );
    assert_eq!(
        member.table_views[0].query.max_page_size,
        crate::table::MAX_TABLE_QUERY_PAGE_SIZE
    );
    let member_relation = member.table_views[0]
        .columns
        .iter()
        .find(|column| column.field == "manager_id")
        .expect("成员目录应包含 manager_id");
    assert!(
        member_relation.relation.is_none(),
        "无 selector Action 权限时不得泄漏 operation id"
    );
    assert_eq!(member.table_views[0].actions, ["org.member.list"]);
    assert_eq!(member.table_views[0].action_presentations.len(), 1);
    let list_presentation = &member.table_views[0].action_presentations[0];
    assert_eq!(list_presentation.operation_id, "org.member.list");
    assert_eq!(list_presentation.placement, ActionPlacement::Toolbar);
    assert_eq!(list_presentation.interaction, ActionInteraction::Form);
    assert!(list_presentation.confirmation.is_none());
    assert!(list_presentation.view_id.is_none());

    let form = &member.table_views[0].form.fields;
    let form_field = |name: &str| {
        form.iter()
            .find(|field| field.field == name)
            .unwrap_or_else(|| panic!("表单应包含字段 {name}"))
    };
    assert!(form_field("id").read_only, "主键必须只读");
    assert!(form_field("created_at").read_only, "自动时间戳必须只读");
    assert!(
        form_field("admin_note").write_only,
        "不可读但可写字段不得预填"
    );
    let secret_form = form_field("secret");
    assert!(secret_form.write_only, "secret 字段必须只写");
    assert!(!secret_form.read_only);
    assert!(secret_form.required);
    assert_eq!(secret_form.widget, WidgetHint::Password);

    let admin = app
        .ui_catalog(
            &app.context(Request::new(json!({}))).with_user(
                User::new(8, "admin")
                    .with_roles(["admin"])
                    .with_permissions(["module:view", "member:edit", "member:options"]),
            ),
        )
        .expect("管理员 UI Catalog revision 应可计算");
    assert_eq!(
        admin.table_views[0]
            .columns
            .iter()
            .map(|column| column.field.as_str())
            .collect::<Vec<_>>(),
        [
            "id",
            "parent_id",
            "name",
            "manager_id",
            "admin_note",
            "created_at"
        ],
        "角色字段应出现，但 secret 字段即使 readable 也不得投影"
    );
    assert_eq!(
        admin.table_views[0].tree.as_ref().map(|tree| (
            tree.id_field.as_str(),
            tree.parent_field.as_str(),
            tree.label_field.as_str(),
            tree.max_nodes,
        )),
        Some((
            "id",
            "parent_id",
            "name",
            crate::table::DEFAULT_TREE_MAX_NODES
        )),
        "树拓扑契约必须携带服务端强制的节点上限"
    );
    assert_eq!(
        admin.table_views[0].actions,
        ["org.member.list", "org.member.edit"]
    );
    let manager_relation = admin.table_views[0]
        .columns
        .iter()
        .find(|column| column.field == "manager_id")
        .and_then(|column| column.relation.as_ref())
        .expect("有权限时应投影关系 options 契约");
    assert_eq!(manager_relation.operation_id, "org.member.options");
    assert_eq!(manager_relation.value_field, "org_member.id");
    assert_eq!(manager_relation.label_fields, ["org_member.name"]);
    let manager_form_relation = admin.table_views[0]
        .form
        .fields
        .iter()
        .find(|field| field.field == "manager_id")
        .and_then(|field| field.relation.as_ref())
        .expect("表单应复用同一关系 options 契约");
    assert_eq!(manager_form_relation, manager_relation);
    assert_eq!(admin.table_views[0].action_presentations.len(), 2);
    let edit_presentation = &admin.table_views[0].action_presentations[1];
    assert_eq!(edit_presentation.operation_id, "org.member.edit");
    assert_eq!(edit_presentation.placement, ActionPlacement::Row);
    assert_eq!(edit_presentation.interaction, ActionInteraction::Form);
    assert_eq!(
        edit_presentation
            .confirmation
            .as_ref()
            .map(|confirmation| confirmation.title.as_str()),
        Some("确认修改")
    );
    assert_eq!(
        edit_presentation
            .availability
            .as_ref()
            .map(|hint| (hint.state, hint.reason.as_str())),
        Some((AvailabilityState::Disabled, "当前记录可能不允许修改"))
    );
    let admin_note = admin.table_views[0]
        .form
        .fields
        .iter()
        .find(|field| field.field == "admin_note")
        .expect("管理员表单应包含 admin_note");
    assert!(!admin_note.read_only);
    assert!(!admin_note.write_only);

    let edit_handle = app
        .registry()
        .resolve(&action_ref("edit"))
        .expect("edit Action 应已注册");
    let response = app
        .dispatch_context(
            edit_handle,
            app.context(Request::new(json!({}))).with_user(
                User::new(8, "admin")
                    .with_roles(["admin"])
                    .with_permissions(["module:view", "member:edit", "member:options"]),
            ),
        )
        .await
        .expect("availability disabled 不能替代服务端授权或阻断真实派发");
    assert_eq!(response.code, 0);
}

/// UI 投影的 search_fields / filter_fields 必须与服务端强制位（TableDefinition
/// 的 is_searchable / is_filterable）逐字段点对点对齐，四种开关组合全覆盖。
#[test]
fn table_view_query_projection_aligns_with_server_searchable_and_filterable_bits() {
    let module_name = ModuleName::new("org.doc").expect("测试 Module 名称应有效");
    let table_name = TableName::new("org_doc").expect("测试 Table 名称应有效");
    let field_ref = |name: &str| {
        FieldRef::new(
            table_name.clone(),
            FieldName::new(name).expect("测试字段名应有效"),
        )
    };
    let data_action = ActionRef::new(
        module_name.clone(),
        ActionName::new("list").expect("测试 Action 名称应有效"),
    );
    let with_access = |name: &str, searchable: bool, filterable: bool| {
        let mut field = FieldSpec::new(
            FieldName::new(name).expect("测试字段名应有效"),
            FieldKind::Str,
        );
        field.access.searchable = searchable;
        field.access.filterable = filterable;
        field
    };
    let view = ViewSpec::new(ViewName::new("main").expect("测试 View 名称应有效"))
        .data_action(data_action)
        .field(field_ref("id"))
        .field(field_ref("both"))
        .field(field_ref("search_only"))
        .field(field_ref("filter_only"))
        .field(field_ref("neither"));
    let module = ModuleSpec::new(module_name)
        .table(
            TableSpec::new(table_name)
                .field(FieldSpec::new(
                    FieldName::new("id").expect("测试字段名应有效"),
                    FieldKind::Key,
                ))
                .field(with_access("both", true, true))
                .field(with_access("search_only", true, false))
                .field(with_access("filter_only", false, true))
                .field(with_access("neither", false, false)),
        )
        .default_permissions(["doc:view"], PermissionMode::All)
        .action(action("list", "org.doc.list"), NoopAction)
        .view(view);
    let app = AppBuilder::new()
        .addon(AddonSpec::new(AddonName::new("org").expect("测试 Addon 名称应有效")).module(module))
        .build(ToolsBuilder::new().build().expect("测试 Tools 应构建成功"))
        .expect("查询投影测试应用应构建成功");

    let catalog = app
        .ui_catalog(
            &app.context(Request::new(json!({})))
                .with_user(User::new(1, "member").with_permissions(["doc:view"])),
        )
        .expect("成员 UI Catalog 应可计算");
    let query = &catalog.table_views[0].query;
    assert_eq!(query.search_fields, ["both", "search_only"]);
    assert_eq!(query.filter_fields, ["both", "filter_only"]);

    let definition = &app.table_definitions()[0];
    for name in ["both", "search_only", "filter_only", "neither"] {
        let metadata = definition.field(name).expect("字段应存在");
        assert_eq!(
            metadata.is_searchable(),
            query.search_fields.iter().any(|field| field == name),
            "{name} 的 searchable 投影与服务端位必须一致"
        );
        assert_eq!(
            metadata.is_filterable(),
            query.filter_fields.iter().any(|field| field == name),
            "{name} 的 filterable 投影与服务端位必须一致"
        );
    }
}
