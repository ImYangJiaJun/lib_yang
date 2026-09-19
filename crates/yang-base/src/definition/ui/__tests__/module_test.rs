//! Module 页面展示投影测试：显式主 Action 与身份页面完整性。

use super::super::{
    AccountIdentitySpec, ActionInteraction, ActionPlacement, ActionPresentationSpec,
    ModulePresentationSpec,
};
use super::fixtures::{action, NoopAction};
use crate::action::{PermissionMode, Request, User};
use crate::definition::{
    ActionName, ActionRef, AddonName, AddonSpec, AppBuilder, FieldKind, FieldName, FieldRef,
    FieldSpec, ModuleName, ModuleSpec, TableName, TableSpec, ViewName, ViewSpec,
};
use crate::tools::ToolsBuilder;
use serde_json::json;

#[test]
fn module_projection_uses_explicit_primary_and_hides_incomplete_identity_pages() {
    let module_name = ModuleName::new("account.profile").expect("测试 Module 名称应有效");
    let primary = ActionRef::new(
        module_name.clone(),
        ActionName::new("profile").expect("测试 Action 名称应有效"),
    );
    let secondary = ActionRef::new(
        module_name.clone(),
        ActionName::new("logout").expect("测试 Action 名称应有效"),
    );
    let module = ModuleSpec::new(module_name)
        .presentation(
            ModulePresentationSpec::new(
                AccountIdentitySpec::new("user", "个人账户", "person"),
                "用户中心",
                "account",
            )
            .primary_action(primary)
            .present_action(
                secondary,
                ActionPresentationSpec::new(ActionPlacement::Toolbar, ActionInteraction::Invoke),
            ),
        )
        .action(action("profile", "account.profile.profile"), NoopAction)
        .action(
            action("logout", "account.profile.logout").public(true),
            NoopAction,
        );
    let app = AppBuilder::new()
        .addon(
            AddonSpec::new(AddonName::new("account").expect("测试 Addon 名称应有效"))
                .module(module),
        )
        .build(ToolsBuilder::new().build().expect("测试 Tools 应构建成功"))
        .expect("Module 展示测试应用应构建成功");

    let anonymous = app
        .ui_catalog(&app.context(Request::new(json!({}))))
        .expect("匿名目录应可投影");
    assert!(
        anonymous.modules.is_empty(),
        "主 Action 不可访问时不得仅凭 public 次要 Action 暴露身份页面"
    );

    let authenticated = app
        .ui_catalog(
            &app.context(Request::new(json!({})))
                .with_user(User::new(7, "alice")),
        )
        .expect("认证目录应可投影");
    assert_eq!(authenticated.modules.len(), 1);
    let module = &authenticated.modules[0];
    assert_eq!(module.module_id, "account.profile");
    assert_eq!(
        module.primary_action.as_deref(),
        Some("account.profile.profile")
    );
    assert_eq!(module.actions, ["account.profile.logout"]);
    assert_eq!(module.identity.id, "user");
}

/// M20 回归：Module→View 投影改走构建期 `views_by_module` 预索引后，每个 Module
/// 仍只拿到自己的 Views，且相对顺序与 `table_views` 声明顺序逐位一致。
#[test]
fn module_views_come_from_own_module_in_declaration_order() {
    let build_module = |module_id: &str, table_id: &str, action_name: &str, views: &[&str]| {
        let module_name = ModuleName::new(module_id).expect("测试 Module 名称应有效");
        let table_name = TableName::new(table_id).expect("测试表名应有效");
        let field_ref = |name: &str| {
            FieldRef::new(
                table_name.clone(),
                FieldName::new(name).expect("测试字段名应有效"),
            )
        };
        let mut module = ModuleSpec::new(module_name.clone())
            .table(
                TableSpec::new(table_name.clone())
                    .field(FieldSpec::new(
                        FieldName::new("id").expect("测试字段名应有效"),
                        FieldKind::Key,
                    ))
                    .field(FieldSpec::new(
                        FieldName::new("name").expect("测试字段名应有效"),
                        FieldKind::Str,
                    )),
            )
            .presentation(ModulePresentationSpec::new(
                AccountIdentitySpec::new("user", "用户", "person"),
                module_id,
                "page",
            ))
            .default_permissions(["module:view"], PermissionMode::All)
            .action(
                action(action_name, &format!("{module_id}.{action_name}")),
                NoopAction,
            );
        for view in views {
            module = module.view(
                ViewSpec::new(ViewName::new(*view).expect("测试 View 名称应有效"))
                    .data_action(ActionRef::new(
                        module_name.clone(),
                        ActionName::new(action_name).expect("测试 Action 名称应有效"),
                    ))
                    .field(field_ref("id"))
                    .field(field_ref("name")),
            );
        }
        module
    };

    let app = AppBuilder::new()
        .addon(
            AddonSpec::new(AddonName::new("org").expect("测试 Addon 名称应有效"))
                .module(build_module(
                    "org.member",
                    "org_member",
                    "list_member",
                    &["second", "first"],
                ))
                .module(build_module(
                    "org.account",
                    "org_account",
                    "list_account",
                    &["only"],
                )),
        )
        .build(ToolsBuilder::new().build().expect("测试 Tools 应构建成功"))
        .expect("Module Views 索引测试应用应构建成功");

    let catalog = app
        .ui_catalog(
            &app.context(Request::new(json!({})))
                .with_user(User::new(7, "alice").with_permissions(["module:view"])),
        )
        .expect("认证目录应可投影");
    let module = |module_id: &str| {
        catalog
            .modules
            .iter()
            .find(|module| module.module_id == module_id)
            .unwrap_or_else(|| panic!("目录应包含 Module {module_id}"))
    };

    // 构建期已按 View 名排序，索引必须保持该顺序（与修复前全扫 `table_views` 一致）。
    assert_eq!(
        module("org.member").views,
        ["org.member.first", "org.member.second"]
    );
    assert_eq!(module("org.account").views, ["org.account.only"]);
    // 预索引不得跨模块泄漏：全部 Module Views 之和等于目录中的 View 总数。
    let indexed = catalog
        .modules
        .iter()
        .map(|module| module.views.len())
        .sum::<usize>();
    assert_eq!(indexed, catalog.table_views.len());
}
