//! 构建期契约校验测试：树拓扑、关系 options、默认排序、展示声明与确认文案。

use super::super::{
    ActionConfirmation, ActionInteraction, ActionPlacement, ActionPresentationSpec,
    AvailabilityHint, SortDirection, TableSortSpec,
};
use super::fixtures::{action, NoopAction};
use crate::action::{Request, User};
use crate::definition::{
    ActionName, ActionRef, AddonName, AddonSpec, AppBuilder, BuildError, FieldKind, FieldName,
    FieldRef, FieldSpec, ModuleName, ModuleSpec, TableName, TableSpec, TreeViewSpec, ViewName,
    ViewSpec,
};
use crate::tools::ToolsBuilder;
use serde_json::json;

#[test]
fn tree_view_contract_rejects_implicit_or_ambiguous_topology() {
    let build = |include_parent: bool, duplicate_id: bool| {
        let module_name = ModuleName::new("org.unit").expect("测试 Module 名称应有效");
        let table_name = TableName::new("org_unit").expect("测试 Table 名称应有效");
        let field_ref = |name: &str| {
            FieldRef::new(
                table_name.clone(),
                FieldName::new(name).expect("测试字段名应有效"),
            )
        };
        let mut view = ViewSpec::new(ViewName::new("tree").expect("测试 View 名称应有效"))
            .field(field_ref("id"))
            .field(field_ref("name"));
        if include_parent {
            view = view.field(field_ref("parent_id"));
        }
        view = view.tree(TreeViewSpec::new(
            field_ref("id"),
            if duplicate_id {
                field_ref("id")
            } else {
                field_ref("parent_id")
            },
            field_ref("name"),
        ));
        let module = ModuleSpec::new(module_name)
            .table(
                TableSpec::new(table_name)
                    .field(FieldSpec::new(
                        FieldName::new("id").expect("测试字段名应有效"),
                        FieldKind::Key,
                    ))
                    .field(FieldSpec::new(
                        FieldName::new("parent_id").expect("测试字段名应有效"),
                        FieldKind::Int,
                    ))
                    .field(FieldSpec::new(
                        FieldName::new("name").expect("测试字段名应有效"),
                        FieldKind::Str,
                    )),
            )
            .view(view);
        AppBuilder::new()
            .addon(
                AddonSpec::new(AddonName::new("org").expect("测试 Addon 名称应有效"))
                    .module(module),
            )
            .build(ToolsBuilder::new().build().expect("测试 Tools 应构建成功"))
    };

    let missing = build(false, false).expect_err("树拓扑字段必须显式包含在 View 中");
    assert!(matches!(
        missing,
        BuildError::InvalidReference {
            kind: "Tree View Field",
            ..
        }
    ));

    let ambiguous = build(true, true).expect_err("树 id/parent 字段必须不同");
    assert!(matches!(
        ambiguous,
        BuildError::InvalidReference {
            kind: "Tree View",
            ..
        }
    ));
}

#[test]
fn relation_options_action_requires_relation_target() {
    let module_name = ModuleName::new("org.unit").expect("测试 Module 名称应有效");
    let table_name = TableName::new("org_unit").expect("测试 Table 名称应有效");
    let options_ref = ActionRef::new(
        module_name.clone(),
        ActionName::new("options").expect("测试 Action 名称应有效"),
    );
    let broken = FieldSpec::new(
        FieldName::new("owner_id").expect("测试字段名应有效"),
        FieldKind::Str,
    )
    .select(options_ref);
    let module = ModuleSpec::new(module_name)
        .table(
            TableSpec::new(table_name)
                .field(FieldSpec::new(
                    FieldName::new("id").expect("测试字段名应有效"),
                    FieldKind::Key,
                ))
                .field(broken),
        )
        .action(action("options", "org.unit.options"), NoopAction);
    let error = AppBuilder::new()
        .addon(AddonSpec::new(AddonName::new("org").expect("测试 Addon 名称应有效")).module(module))
        .build(ToolsBuilder::new().build().expect("测试 Tools 应构建成功"))
        .expect_err("selector Action 缺少关系目标必须在启动期失败");
    assert!(
        matches!(
            error,
            BuildError::InvalidReference {
                kind: "Relation Options Field",
                ..
            }
        ),
        "实际错误: {error:?}"
    );
}

#[test]
fn table_view_default_sort_requires_sortable_view_field() {
    let module_name = ModuleName::new("org.unit").expect("测试 Module 名称应有效");
    let table_name = TableName::new("org_unit").expect("测试 Table 名称应有效");
    let id_ref = FieldRef::new(
        table_name.clone(),
        FieldName::new("id").expect("测试字段名应有效"),
    );
    let view = ViewSpec::new(ViewName::new("list").expect("测试 View 名称应有效"))
        .field(id_ref.clone())
        .default_sort(TableSortSpec::new(id_ref, SortDirection::Desc));
    let module = ModuleSpec::new(module_name)
        .table(TableSpec::new(table_name).field(FieldSpec::new(
            FieldName::new("id").expect("测试字段名应有效"),
            FieldKind::Key,
        )))
        .view(view);
    let error = AppBuilder::new()
        .addon(AddonSpec::new(AddonName::new("org").expect("测试 Addon 名称应有效")).module(module))
        .build(ToolsBuilder::new().build().expect("测试 Tools 应构建成功"))
        .expect_err("不可排序字段不能成为 View 默认排序");
    assert!(matches!(
        error,
        BuildError::InvalidReference {
            kind: "View Default Sort",
            ..
        }
    ));
}

#[test]
fn custom_action_presentation_rejects_paths_missing_ids_and_response_mismatch() {
    let build = |presentation: ActionPresentationSpec| {
        let module_name = ModuleName::new("dms.task").expect("测试 Module 名称应有效");
        let action_ref = ActionRef::new(
            module_name.clone(),
            ActionName::new("flow").expect("测试 Action 名称应有效"),
        );
        let module = ModuleSpec::new(module_name)
            .table(
                TableSpec::new(TableName::new("dms_task").expect("测试 Table 名称应有效")).field(
                    FieldSpec::new(
                        FieldName::new("id").expect("测试字段名应有效"),
                        FieldKind::Key,
                    ),
                ),
            )
            .action(action("flow", "dms.task.flow"), NoopAction)
            .view(
                ViewSpec::new(ViewName::new("main").expect("测试 View 名称应有效"))
                    .data_action(action_ref.clone())
                    .present_action(action_ref, presentation),
            );
        AppBuilder::new()
            .addon(
                AddonSpec::new(AddonName::new("dms").expect("测试 Addon 名称应有效"))
                    .module(module),
            )
            .build(ToolsBuilder::new().build().expect("测试 Tools 应构建成功"))
    };

    let missing_id = build(ActionPresentationSpec::new(
        ActionPlacement::Toolbar,
        ActionInteraction::Custom,
    ))
    .expect_err("custom 交互缺少 view_id 必须在启动期失败");
    assert!(matches!(
        missing_id,
        BuildError::InvalidReference {
            kind: "Action Presentation",
            ..
        }
    ));

    let physical_path = build(
        ActionPresentationSpec::new(ActionPlacement::Toolbar, ActionInteraction::Custom)
            .view_id("../views/TaskFlow.vue"),
    )
    .expect_err("物理路径不得作为 custom view_id");
    assert!(matches!(
        physical_path,
        BuildError::InvalidReference {
            kind: "Action Presentation",
            ..
        }
    ));

    let blank_availability = build(
        ActionPresentationSpec::new(ActionPlacement::Toolbar, ActionInteraction::Invoke)
            .availability(AvailabilityHint::hidden("   ")),
    )
    .expect_err("空白 availability reason 必须在启动期失败");
    assert!(matches!(
        blank_availability,
        BuildError::InvalidReference {
            kind: "Action Presentation",
            ..
        }
    ));

    let mismatch = build(ActionPresentationSpec::new(
        ActionPlacement::Toolbar,
        ActionInteraction::Preview,
    ))
    .expect_err("JSON Action 不得伪装成文件预览");
    assert!(matches!(
        mismatch,
        BuildError::InvalidReference {
            kind: "Action Presentation",
            ..
        }
    ));

    let app = build(
        ActionPresentationSpec::new(ActionPlacement::Toolbar, ActionInteraction::Custom)
            .view_id("dms.task.flow"),
    )
    .expect("稳定限定 view_id 应通过构建期校验");
    let catalog = app
        .ui_catalog(
            &app.context(Request::new(json!({})))
                .with_user(User::new(9, "designer")),
        )
        .expect("custom view UI Catalog revision 应可计算");
    assert_eq!(
        catalog.table_views[0].action_presentations[0]
            .view_id
            .as_deref(),
        Some("dms.task.flow")
    );
}

#[test]
fn action_confirmation_rejects_blank_or_overlong_content() {
    let build = |confirmation: ActionConfirmation| {
        let module_name = ModuleName::new("dms.task").expect("测试 Module 名称应有效");
        let action_ref = ActionRef::new(
            module_name.clone(),
            ActionName::new("flow").expect("测试 Action 名称应有效"),
        );
        let module = ModuleSpec::new(module_name)
            .table(
                TableSpec::new(TableName::new("dms_task").expect("测试 Table 名称应有效")).field(
                    FieldSpec::new(
                        FieldName::new("id").expect("测试字段名应有效"),
                        FieldKind::Key,
                    ),
                ),
            )
            .action(action("flow", "dms.task.flow"), NoopAction)
            .view(
                ViewSpec::new(ViewName::new("main").expect("测试 View 名称应有效")).present_action(
                    action_ref,
                    ActionPresentationSpec::new(
                        ActionPlacement::Toolbar,
                        ActionInteraction::Invoke,
                    )
                    .confirmation(confirmation),
                ),
            );
        AppBuilder::new()
            .addon(
                AddonSpec::new(AddonName::new("dms").expect("测试 Addon 名称应有效"))
                    .module(module),
            )
            .build(ToolsBuilder::new().build().expect("测试 Tools 应构建成功"))
    };
    let assert_invalid = |result: Result<crate::definition::BuiltApp, BuildError>, reason: &str| {
        assert!(
            matches!(
                result,
                Err(BuildError::InvalidReference {
                    kind: "Action Presentation",
                    ..
                })
            ),
            "{reason}"
        );
    };

    assert_invalid(
        build(ActionConfirmation::new("   ", "将执行危险操作")),
        "空白确认标题必须在启动期失败",
    );
    assert_invalid(
        build(ActionConfirmation::new("确认删除", "")),
        "空白确认正文必须在启动期失败",
    );
    assert_invalid(
        build(ActionConfirmation::new("题".repeat(501), "将执行危险操作")),
        "超长确认标题必须在启动期失败",
    );
    assert_invalid(
        build(ActionConfirmation::new("确认删除", "文".repeat(501))),
        "超长确认正文必须在启动期失败",
    );
    build(ActionConfirmation::new(
        "确认删除",
        "将删除当前记录，不可恢复",
    ))
    .expect("合法确认文案应通过构建期校验");
}
