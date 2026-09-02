//! 表单校验提示契约测试：仅序列化已声明约束、随字段权限过滤。

use super::super::{FormFieldSchema, FormFieldValidationSchema, WidgetHint, UI_SCHEMA_VERSION};
#[cfg(feature = "validator")]
use super::fixtures::{action, NoopAction};
#[cfg(feature = "validator")]
use crate::action::{PermissionMode, Request, User};
#[cfg(feature = "validator")]
use crate::definition::{
    AccessRule, ActionName, ActionRef, AddonName, AddonSpec, AppBuilder, FieldKind, FieldName,
    FieldRef, FieldSpec, ModuleName, ModuleSpec, TableName, TableSpec, ViewName, ViewSpec,
};
#[cfg(feature = "validator")]
use crate::tools::ToolsBuilder;
use serde_json::json;

#[test]
fn form_field_validation_serializes_only_declared_constraints() {
    assert_eq!(
        UI_SCHEMA_VERSION, "2.3",
        "TreeViewSchema.max_nodes 进入线上契约，必须递增 schema 版本"
    );

    let validation = FormFieldValidationSchema {
        min_length: Some(2),
        max_length: Some(64),
        minimum: None,
        maximum: None,
        pattern: Some("^[a-z]+$".to_string()),
    };
    let wire = serde_json::to_value(&validation).expect("校验提示应可序列化");
    assert_eq!(
        wire,
        json!({"min_length": 2, "max_length": 64, "pattern": "^[a-z]+$"}),
        "未声明的约束不得出现在线上契约"
    );

    let mut field = FormFieldSchema {
        field: "name".to_string(),
        title: "名称".to_string(),
        description: String::new(),
        widget: WidgetHint::Text,
        required: true,
        read_only: false,
        write_only: false,
        relation: None,
        validation: Some(validation),
    };
    let wire = serde_json::to_value(&field).expect("表单字段应可序列化");
    assert_eq!(wire["validation"]["min_length"], 2);
    assert!(wire["validation"].get("minimum").is_none());

    field.validation = None;
    let wire = serde_json::to_value(&field).expect("无约束表单字段应可序列化");
    assert!(
        wire.get("validation").is_none(),
        "未声明约束的字段不得携带 validation 键"
    );
}

#[cfg(feature = "validator")]
#[tokio::test]
async fn form_projection_includes_validation_hints_and_filters_by_field_permission() {
    let module_name = ModuleName::new("org.profile").expect("测试 Module 名称应有效");
    let table_name = TableName::new("org_profile").expect("测试 Table 名称应有效");
    let field_ref = |name: &str| {
        FieldRef::new(
            table_name.clone(),
            FieldName::new(name).expect("测试字段名应有效"),
        )
    };

    let mut nickname = FieldSpec::new(
        FieldName::new("nickname").expect("测试字段名应有效"),
        FieldKind::Str,
    );
    nickname.validation.min_length = Some(2);
    nickname.validation.max_length = Some(64);
    nickname.validation.pattern = Some("^[a-z]+$".to_string());
    let mut score = FieldSpec::new(
        FieldName::new("score").expect("测试字段名应有效"),
        FieldKind::Decimal,
    );
    score.validation.minimum = Some("0".to_string());
    score.validation.maximum = Some("99.99".to_string());
    score.access.readable = AccessRule::Roles(vec!["admin".to_string()]);
    score.access.writable = AccessRule::Roles(vec!["admin".to_string()]);

    let list_action = ActionRef::new(
        module_name.clone(),
        ActionName::new("list").expect("测试 Action 名称应有效"),
    );
    let view = ViewSpec::new(ViewName::new("main").expect("测试 View 名称应有效"))
        .data_action(list_action.clone())
        .field(field_ref("id"))
        .field(field_ref("nickname"))
        .field(field_ref("score"))
        .field(field_ref("bio"))
        .action(list_action);
    let module = ModuleSpec::new(module_name)
        .table(
            TableSpec::new(table_name)
                .field(FieldSpec::new(
                    FieldName::new("id").expect("测试字段名应有效"),
                    FieldKind::Key,
                ))
                .field(nickname)
                .field(score)
                .field(FieldSpec::new(
                    FieldName::new("bio").expect("测试字段名应有效"),
                    FieldKind::Text,
                )),
        )
        .default_permissions(["module:view"], PermissionMode::All)
        .action(action("list", "org.profile.list"), NoopAction)
        .view(view);
    let app = AppBuilder::new()
        .addon(AddonSpec::new(AddonName::new("org").expect("测试 Addon 名称应有效")).module(module))
        .build(ToolsBuilder::new().build().expect("测试 Tools 应构建成功"))
        .expect("校验提示测试应用应构建成功");

    let member = app
        .ui_catalog(
            &app.context(Request::new(json!({})))
                .with_user(User::new(7, "member").with_permissions(["module:view"])),
        )
        .expect("成员 UI Catalog revision 应可计算");
    let member_form = &member.table_views[0].form.fields;
    let member_field = |name: &str| {
        member_form
            .iter()
            .find(|field| field.field == name)
            .unwrap_or_else(|| panic!("成员表单应包含字段 {name}"))
    };
    let nickname = member_field("nickname")
        .validation
        .as_ref()
        .expect("成员表单应投影 nickname 的校验提示");
    assert_eq!(nickname.min_length, Some(2));
    assert_eq!(nickname.max_length, Some(64));
    assert_eq!(nickname.pattern.as_deref(), Some("^[a-z]+$"));
    assert_eq!(nickname.minimum, None);
    assert_eq!(nickname.maximum, None);
    assert!(
        member_form.iter().all(|field| field.field != "score"),
        "无字段权限时整个字段（含校验提示）都不得投影"
    );
    let member_wire =
        serde_json::to_value(&member.table_views[0].form).expect("成员表单应可序列化");
    let bio_wire = member_wire["fields"]
        .as_array()
        .expect("表单字段应序列化为数组")
        .iter()
        .find(|field| field["field"] == "bio")
        .expect("成员表单应包含 bio");
    assert!(
        bio_wire.get("validation").is_none(),
        "未声明约束的字段不得携带 validation 键: {bio_wire}"
    );

    let admin = app
        .ui_catalog(
            &app.context(Request::new(json!({}))).with_user(
                User::new(8, "admin")
                    .with_roles(["admin"])
                    .with_permissions(["module:view"]),
            ),
        )
        .expect("管理员 UI Catalog revision 应可计算");
    let score = admin.table_views[0]
        .form
        .fields
        .iter()
        .find(|field| field.field == "score")
        .and_then(|field| field.validation.as_ref())
        .expect("管理员表单应投影 score 的校验提示");
    assert_eq!(score.minimum.as_deref(), Some("0"));
    assert_eq!(score.maximum.as_deref(), Some("99.99"));
    assert_eq!(score.min_length, None);
}
