//! Module 页面展示投影测试：显式主 Action 与身份页面完整性。

use super::super::{
    AccountIdentitySpec, ActionInteraction, ActionPlacement, ActionPresentationSpec,
    ModulePresentationSpec,
};
use super::fixtures::{action, NoopAction};
use crate::action::{Request, User};
use crate::definition::{
    ActionName, ActionRef, AddonName, AddonSpec, AppBuilder, ModuleName, ModuleSpec,
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
