//! 请求级 UI 投影与派发授权策略一致性测试。

use super::super::UiCatalog;
use super::fixtures::{action, NoopAction};
use crate::action::{PermissionMode, Request, User};
use crate::definition::{
    ActionName, ActionRef, AddonName, AddonSpec, AppBuilder, ModuleName, ModuleSpec,
};
use crate::error::BaseError;
use crate::tools::ToolsBuilder;
use serde_json::json;

#[tokio::test]
async fn request_projection_reuses_dispatch_authorization_policy() {
    let module = ModuleSpec::new(ModuleName::new("org.user").expect("测试 Module 名称应有效"))
        .default_permissions(["module:access"], PermissionMode::All)
        .action(
            action("public", "org.user.public")
                .public(true)
                .permissions(["never:granted"], PermissionMode::All),
            NoopAction,
        )
        .action(action("member", "org.user.member"), NoopAction)
        .action(
            action("all", "org.user.all")
                .permissions(["record:read", "record:write"], PermissionMode::All),
            NoopAction,
        )
        .action(
            action("any", "org.user.any")
                .permissions(["record:read", "record:write"], PermissionMode::Any),
            NoopAction,
        );
    let app = AppBuilder::new()
        .addon(AddonSpec::new(AddonName::new("org").expect("测试 Addon 名称应有效")).module(module))
        .build(ToolsBuilder::new().build().expect("测试 Tools 应构建成功"))
        .expect("测试 App 应构建成功");

    let anonymous = app.context(Request::new(serde_json::Value::Null));
    let anonymous_revision = app
        .ui_catalog(&anonymous)
        .expect("匿名 UI Catalog revision 应可计算")
        .revision;
    assert_eq!(
        anonymous_revision,
        app.ui_catalog(&anonymous)
            .expect("重复投影 revision 应可计算")
            .revision,
        "相同请求表示必须产生稳定 revision"
    );
    assert_eq!(
        operation_ids(app.ui_catalog(&anonymous)),
        ["org.user.public"]
    );

    let module_only = app
        .context(Request::new(serde_json::Value::Null))
        .with_user(User::new(1, "module").with_permissions(["module:access"]));
    assert_ne!(
        anonymous_revision,
        app.ui_catalog(&module_only)
            .expect("成员 UI Catalog revision 应可计算")
            .revision,
        "权限过滤后的不同表示必须使用不同 revision"
    );
    assert_eq!(
        operation_ids(app.ui_catalog(&module_only)),
        ["org.user.member", "org.user.public"]
    );

    let any = app
        .context(Request::new(serde_json::Value::Null))
        .with_user(User::new(2, "any").with_permissions(["module:access", "record:read"]));
    assert_eq!(
        operation_ids(app.ui_catalog(&any)),
        ["org.user.any", "org.user.member", "org.user.public"]
    );

    let all = app
        .context(Request::new(serde_json::Value::Null))
        .with_user(User::new(3, "all").with_permissions([
            "module:access",
            "record:read",
            "record:write",
        ]));
    assert_eq!(
        operation_ids(app.ui_catalog(&all)),
        [
            "org.user.all",
            "org.user.any",
            "org.user.member",
            "org.user.public"
        ]
    );

    let action_only = app
        .context(Request::new(serde_json::Value::Null))
        .with_user(User::new(4, "action").with_permissions(["record:read", "record:write"]));
    assert_eq!(
        operation_ids(app.ui_catalog(&action_only)),
        ["org.user.public"]
    );

    let all_handle = app
        .registry()
        .resolve(&ActionRef::new(
            ModuleName::new("org.user").expect("测试 Module 名称应有效"),
            ActionName::new("all").expect("测试 Action 名称应有效"),
        ))
        .expect("all Action 应已注册");
    let denied = app
        .dispatch_context(
            all_handle,
            app.context(Request::new(json!({}))).with_user(
                User::new(4, "action").with_permissions(["record:read", "record:write"]),
            ),
        )
        .await;
    assert!(matches!(denied, Err(BaseError::PermissionDenied(_))));

    let allowed = app
        .dispatch_context(
            all_handle,
            app.context(Request::new(json!({})))
                .with_user(User::new(3, "all").with_permissions([
                    "module:access",
                    "record:read",
                    "record:write",
                ])),
        )
        .await;
    assert!(allowed.is_ok(), "目录可见的 Action 应通过同一授权策略");

    let public_handle = app
        .registry()
        .resolve(&ActionRef::new(
            ModuleName::new("org.user").expect("测试 Module 名称应有效"),
            ActionName::new("public").expect("测试 Action 名称应有效"),
        ))
        .expect("public Action 应已注册");
    let public = app
        .dispatch_context(public_handle, app.context(Request::new(json!({}))))
        .await;
    assert!(
        public.is_ok(),
        "public Action 应同时绕过模块与 Action 权限组"
    );
}

fn operation_ids(catalog: Result<UiCatalog, BaseError>) -> Vec<String> {
    let catalog = catalog.expect("UI Catalog revision 应可计算");
    catalog
        .actions
        .into_iter()
        .map(|action| action.operation_id)
        .collect()
}
