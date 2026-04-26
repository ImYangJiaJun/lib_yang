//! ModuleRouter 集成测试

use crate::action::builtin::{AddAction, GetAction};
use crate::action::{ActionContext, GlobalTools, Request, User};
use crate::error::BaseError;
use crate::router::ModuleRouter;
use crate::table::{FieldConfig, FieldType, TableConfig};
use crate::token::TokenManager;
use jsonwebtoken::Algorithm;
use serde_json::json;
use std::sync::Arc;

/// 创建测试用的表配置
fn create_test_table_config() -> Arc<TableConfig> {
    Arc::new(
        TableConfig::new("test_users")
            .field(FieldConfig::new("id", FieldType::Integer).required(true))
            .field(
                FieldConfig::new("username", FieldType::String { max_length: 50 }).required(true),
            )
            .field(FieldConfig::new("email", FieldType::String { max_length: 100 }).required(false))
            .primary_key("id"),
    )
}

/// 创建测试用的用户
fn create_test_user() -> User {
    User {
        id: 1,
        username: "test_user".to_string(),
        nickname: "测试用户".to_string(),
        email: "test@example.com".to_string(),
        roles: vec!["admin".to_string()],
        permissions: vec![
            "user:read".to_string(),
            "user:write".to_string(),
            "user:delete".to_string(),
        ],
    }
}

/// 创建测试用的 GlobalTools
fn create_test_tools() -> Arc<GlobalTools> {
    let token_manager = TokenManager::new_symmetric(
        "test_secret_key",
        Algorithm::HS256,
        "test_issuer".to_string(),
        "test_audience".to_string(),
        3600,
        86400,
    );
    Arc::new(GlobalTools::new(token_manager))
}

#[test]
fn test_module_router_new() {
    let router = ModuleRouter::new("user", "用户管理");

    assert_eq!(router.module_name(), "user");
    assert_eq!(router.display_name(), "用户管理");
    assert!(router.table_config().is_none());
    assert_eq!(router.action_names().len(), 0);
}

#[test]
fn test_module_router_with_table_config() {
    let table_config = create_test_table_config();
    let router = ModuleRouter::new("user", "用户管理").with_table_config(table_config.clone());

    assert!(router.table_config().is_some());
    assert_eq!(router.table_config().unwrap().table_name, "test_users");
}

#[test]
fn test_module_router_register_action() {
    let table_config = create_test_table_config();
    let add_action = AddAction::new(table_config.clone());

    let router = ModuleRouter::new("user", "用户管理").register_action(add_action);

    let action_names = router.action_names();
    assert_eq!(action_names.len(), 1);
    assert!(action_names.contains(&"add".to_string()));
}

#[test]
fn test_module_router_register_builtin_actions() {
    let table_config = create_test_table_config();
    let router = ModuleRouter::new("user", "用户管理")
        .with_table_config(table_config)
        .register_builtin_actions();

    let action_names = router.action_names();
    assert_eq!(action_names.len(), 6);
    assert!(action_names.contains(&"add".to_string()));
    assert!(action_names.contains(&"put".to_string()));
    assert!(action_names.contains(&"del".to_string()));
    assert!(action_names.contains(&"get".to_string()));
    assert!(action_names.contains(&"select".to_string()));
    assert!(action_names.contains(&"table".to_string()));
}

#[test]
fn test_module_router_default_permissions() {
    let _router =
        ModuleRouter::new("user", "用户管理").default_permissions(vec!["user:access".to_string()]);

    // 默认权限是私有字段，无法直接测试，但可以通过 dispatch 测试
    // 这里只测试构建器方法不会 panic
}

#[tokio::test]
async fn test_module_router_dispatch_action_not_found() {
    let table_config = create_test_table_config();
    let router = ModuleRouter::new("user", "用户管理")
        .with_table_config(table_config)
        .register_builtin_actions();

    let request = Request::new(json!({}));
    let tools = create_test_tools();
    let context = ActionContext::new(request, tools);

    let result = router.dispatch("nonexistent", context).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        BaseError::ActionNotFound(name) => {
            assert_eq!(name, "nonexistent");
        }
        _ => panic!("期望 ActionNotFound 错误"),
    }
}

#[tokio::test]
async fn test_module_router_dispatch_public_action() {
    let table_config = create_test_table_config();
    let router = ModuleRouter::new("user", "用户管理")
        .with_table_config(table_config)
        .register_builtin_actions();

    let request = Request::new(json!({}));
    let tools = create_test_tools();
    let context = ActionContext::new(request, tools);

    // table action 是公开的，不需要认证
    let result = router.dispatch("table", context).await;

    // 由于没有设置数据库连接，这里会返回错误，但不是 Unauthorized 错误
    // 我们只测试能够找到 action 并尝试执行
    assert!(result.is_ok() || !matches!(result.unwrap_err(), BaseError::Unauthorized(_)));
}

#[tokio::test]
async fn test_module_router_dispatch_unauthorized() {
    let table_config = create_test_table_config();
    let router = ModuleRouter::new("user", "用户管理")
        .with_table_config(table_config)
        .register_builtin_actions();

    let request = Request::new(json!({
        "data": {
            "username": "alice",
            "email": "alice@example.com"
        }
    }));
    let tools = create_test_tools();
    let context = ActionContext::new(request, tools);

    // add action 需要认证，但没有提供用户信息
    let result = router.dispatch("add", context).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        BaseError::Unauthorized(_) => {}
        _ => panic!("期望 Unauthorized 错误"),
    }
}

#[tokio::test]
async fn test_module_router_dispatch_with_user() {
    let table_config = create_test_table_config();
    let router = ModuleRouter::new("user", "用户管理")
        .with_table_config(table_config)
        .register_builtin_actions();

    let user = create_test_user();
    let request = Request::new(json!({
        "data": {
            "id": 1,
            "username": "alice",
            "email": "alice@example.com"
        }
    }));
    let tools = create_test_tools();
    let context = ActionContext::new(request, tools).with_user(user);

    // 有用户信息，但没有数据库连接，会返回其他错误（不是 Unauthorized）
    let result = router.dispatch("add", context).await;

    // 我们只测试不是 Unauthorized 错误
    if let Err(e) = result {
        assert!(!matches!(e, BaseError::Unauthorized(_)));
    }
}

#[tokio::test]
async fn test_module_router_dispatch_permission_denied() {
    let table_config = create_test_table_config();
    let router = ModuleRouter::new("user", "用户管理")
        .with_table_config(table_config)
        .register_builtin_actions()
        .default_permissions(vec!["admin:access".to_string()]);

    // 创建没有 admin:access 权限的用户
    let user = User {
        id: 1,
        username: "test_user".to_string(),
        nickname: "测试用户".to_string(),
        email: "test@example.com".to_string(),
        roles: vec!["user".to_string()],
        permissions: vec!["user:read".to_string()],
    };

    let request = Request::new(json!({
        "data": {
            "id": 1,
            "username": "alice",
            "email": "alice@example.com"
        }
    }));
    let tools = create_test_tools();
    let context = ActionContext::new(request, tools).with_user(user);

    // 用户没有 admin:access 权限
    let result = router.dispatch("add", context).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        BaseError::PermissionDenied(_) => {}
        _ => panic!("期望 PermissionDenied 错误"),
    }
}

#[tokio::test]
async fn test_module_router_dispatch_with_sufficient_permissions() {
    let table_config = create_test_table_config();
    let router = ModuleRouter::new("user", "用户管理")
        .with_table_config(table_config)
        .register_builtin_actions()
        .default_permissions(vec!["user:access".to_string()]);

    // 创建有 user:access 权限的用户
    let user = User {
        id: 1,
        username: "test_user".to_string(),
        nickname: "测试用户".to_string(),
        email: "test@example.com".to_string(),
        roles: vec!["user".to_string()],
        permissions: vec!["user:access".to_string(), "user:write".to_string()],
    };

    let request = Request::new(json!({
        "data": {
            "id": 1,
            "username": "alice",
            "email": "alice@example.com"
        }
    }));
    let tools = create_test_tools();
    let context = ActionContext::new(request, tools).with_user(user);

    // 用户有足够的权限，但没有数据库连接，会返回其他错误（不是 PermissionDenied）
    let result = router.dispatch("add", context).await;

    // 我们只测试不是权限相关的错误
    if let Err(e) = result {
        assert!(!matches!(e, BaseError::PermissionDenied(_)));
        assert!(!matches!(e, BaseError::Unauthorized(_)));
    }
}

#[test]
fn test_module_router_action_names() {
    let table_config = create_test_table_config();
    let router = ModuleRouter::new("user", "用户管理")
        .with_table_config(table_config.clone())
        .register_action(AddAction::new(table_config.clone()))
        .register_action(GetAction::new(table_config));

    let action_names = router.action_names();
    assert_eq!(action_names.len(), 2);
    assert!(action_names.contains(&"add".to_string()));
    assert!(action_names.contains(&"get".to_string()));
}

#[test]
#[should_panic(expected = "必须先设置 table_config 才能注册内置 Actions")]
fn test_module_router_register_builtin_actions_without_table_config() {
    let _router = ModuleRouter::new("user", "用户管理").register_builtin_actions();
}
