//! UiCatalog 序列化、revision 稳定性与演示契约防泄漏测试。

use super::super::{
    ActionDemoSchema, ActionResponseKind, TreeViewSchema, UiCatalog, UI_SCHEMA_VERSION,
};
use super::fixtures::action;
use crate::definition::{ActionName, FieldName, ParamSource, ParamSpec};
use serde_json::json;

#[test]
fn ui_catalog_serializes_stable_minimal_action_contract() {
    let mut protected = action("export", "org.user.export")
        .display_name("导出用户")
        .description("导出当前租户用户")
        .response_kind(ActionResponseKind::Download)
        .param(
            ParamSpec::new(
                FieldName::new("tenant_id").expect("测试字段名应有效"),
                ParamSource::Path,
            )
            .required(false),
        )
        .param(ParamSpec::new(
            FieldName::new("filter").expect("测试字段名应有效"),
            ParamSource::Body,
        ))
        .param(ParamSpec::new(
            FieldName::new("search").expect("测试字段名应有效"),
            ParamSource::Query,
        ))
        .param(ParamSpec::new(
            FieldName::new("request_id").expect("测试字段名应有效"),
            ParamSource::Header,
        ));
    protected.input_schema =
        json!({"type": "object", "properties": {"filter": {"type": "string"}}});
    protected.output_schema = json!({"type": "string", "format": "binary"});
    let public = action("health", "health.check").public(true);
    let catalog = UiCatalog::new([
        ActionDemoSchema::from(&protected),
        ActionDemoSchema::from(&public),
    ])
    .expect("UI Catalog revision 应可计算");

    let value = serde_json::to_value(catalog).expect("UI Catalog 应可序列化");
    assert_eq!(value["schema_version"], UI_SCHEMA_VERSION);
    let revision = value["revision"]
        .as_str()
        .expect("UI Catalog 应携带 revision");
    assert_eq!(revision.len(), 64);
    assert!(revision.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_eq!(value["actions"][0]["operation_id"], "health.check");
    assert_eq!(value["actions"][0]["requires_auth"], false);
    assert_eq!(value["actions"][1]["response_kind"], "download");
    assert_eq!(value["actions"][1]["params"][0]["source"], "path");
    assert_eq!(value["actions"][1]["params"][0]["required"], true);
    assert_eq!(value["actions"][1]["params"][0]["title"], "tenant_id");
    assert_eq!(value["actions"][1]["params"][1]["source"], "body");
    assert_eq!(value["actions"][1]["params"][2]["source"], "query");
    assert_eq!(value["actions"][1]["params"][3]["source"], "header");
    assert_eq!(value["actions"][1]["input_schema"], protected.input_schema);
    assert_eq!(
        value["actions"][1]["output_schema"],
        protected.output_schema
    );
    assert_eq!(value["actions"][1]["method"], "POST");
}

#[test]
fn catalog_revision_is_order_independent_and_content_sensitive() {
    let first = ActionDemoSchema::from(&action("first", "org.user.first"));
    let second = ActionDemoSchema::from(&action("second", "org.user.second"));
    let ordered =
        UiCatalog::new([first.clone(), second.clone()]).expect("有序目录 revision 应可计算");
    let reversed = UiCatalog::new([second.clone(), first]).expect("逆序目录 revision 应可计算");
    assert_eq!(ordered.actions, reversed.actions);
    assert_eq!(ordered.revision, reversed.revision);

    let mut changed = second;
    changed.title = "新的展示标题".to_string();
    let changed = UiCatalog::new([changed]).expect("变更目录 revision 应可计算");
    assert_ne!(ordered.revision, changed.revision);
}

#[test]
fn catalog_json_schema_requires_version_revision_actions_and_views() {
    let schema = serde_json::to_value(schemars::schema_for!(UiCatalog))
        .expect("UiCatalog JSON Schema 应可序列化");
    let required = schema["required"]
        .as_array()
        .expect("UiCatalog schema.required 应存在");
    for field in ["schema_version", "revision", "actions", "table_views"] {
        assert!(
            required.iter().any(|value| value == field),
            "UiCatalog 运行时 schema 应要求字段 {field}: {schema}"
        );
    }

    let tree_schema = serde_json::to_value(schemars::schema_for!(TreeViewSchema))
        .expect("TreeViewSchema JSON Schema 应可序列化");
    let tree_required = tree_schema["required"]
        .as_array()
        .expect("TreeViewSchema schema.required 应存在");
    for field in ["id_field", "parent_field", "label_field"] {
        assert!(
            tree_required.iter().any(|value| value == field),
            "TreeViewSchema 运行时 schema 应要求字段 {field}: {tree_schema}"
        );
    }
}

#[test]
fn action_demo_does_not_leak_internal_permissions_calls_or_tags() {
    let spec = action("remove", "org.user.remove")
        .permissions(["org.user.delete"], crate::action::PermissionMode::All)
        .tag("internal")
        .calls(crate::definition::ActionRef::new(
            crate::definition::ModuleName::new("audit.log").expect("测试 Module 名称应有效"),
            ActionName::new("write").expect("测试 Action 名称应有效"),
        ));
    let value =
        serde_json::to_value(ActionDemoSchema::from(&spec)).expect("ActionDemoSchema 应可序列化");

    assert_eq!(value["requires_auth"], json!(true));
    assert!(value.get("permissions").is_none());
    assert!(value.get("permission_mode").is_none());
    assert!(value.get("calls").is_none());
    assert!(value.get("tags").is_none());
}
