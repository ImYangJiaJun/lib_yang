//! ActionContext 和 User 单元测试

use crate::action::{ActionContext, GlobalTools, Request, User};
use crate::token::TokenManager;
use jsonwebtoken::Algorithm;
use serde_json::json;
use std::sync::Arc;

/// 创建测试用的 TokenManager
fn create_test_token_manager() -> TokenManager {
    TokenManager::new_symmetric(
        "test_secret_key",
        Algorithm::HS256,
        "test_issuer".to_string(),
        "test_audience".to_string(),
        3600,
        86400,
    )
}

/// 创建测试用的 GlobalTools
fn create_test_tools() -> Arc<GlobalTools> {
    Arc::new(GlobalTools::new(create_test_token_manager()))
}

#[test]
fn test_user_new() {
    let user = User::new(1, "alice");

    assert_eq!(user.id, 1);
    assert_eq!(user.username, "alice");
    assert_eq!(user.nickname, "");
    assert_eq!(user.email, "");
    assert_eq!(user.roles.len(), 0);
    assert_eq!(user.permissions.len(), 0);
}

#[test]
fn test_user_has_permission() {
    let mut user = User::new(1, "alice");
    user.permissions = vec!["user:read".to_string(), "user:write".to_string()];

    assert!(user.has_permission("user:read"));
    assert!(user.has_permission("user:write"));
    assert!(!user.has_permission("user:delete"));
    assert!(!user.has_permission("admin:access"));
}

#[test]
fn test_user_has_permission_empty() {
    let user = User::new(1, "alice");

    assert!(!user.has_permission("user:read"));
    assert!(!user.has_permission("any:permission"));
}

#[test]
fn test_user_has_role() {
    let mut user = User::new(1, "alice");
    user.roles = vec!["admin".to_string(), "user".to_string()];

    assert!(user.has_role("admin"));
    assert!(user.has_role("user"));
    assert!(!user.has_role("guest"));
    assert!(!user.has_role("superadmin"));
}

#[test]
fn test_user_has_role_empty() {
    let user = User::new(1, "alice");

    assert!(!user.has_role("admin"));
    assert!(!user.has_role("any:role"));
}

#[test]
fn test_user_has_any_role() {
    let mut user = User::new(1, "alice");
    user.roles = vec!["admin".to_string(), "user".to_string()];

    // 有任一角色
    assert!(user.has_any_role(&["admin".to_string()]));
    assert!(user.has_any_role(&["user".to_string()]));
    assert!(user.has_any_role(&["admin".to_string(), "guest".to_string()]));
    assert!(user.has_any_role(&["guest".to_string(), "user".to_string()]));

    // 没有任何角色
    assert!(!user.has_any_role(&["guest".to_string()]));
    assert!(!user.has_any_role(&["guest".to_string(), "superadmin".to_string()]));
}

#[test]
fn test_user_has_any_role_empty_user_roles() {
    let user = User::new(1, "alice");

    assert!(!user.has_any_role(&["admin".to_string()]));
    assert!(!user.has_any_role(&["admin".to_string(), "user".to_string()]));
}

#[test]
fn test_user_has_any_role_empty_check_roles() {
    let mut user = User::new(1, "alice");
    user.roles = vec!["admin".to_string()];

    // 空列表检查，应该返回 false
    assert!(!user.has_any_role(&[]));
}

#[test]
fn test_user_clone() {
    let mut user = User::new(1, "alice");
    user.nickname = "Alice".to_string();
    user.email = "alice@example.com".to_string();
    user.roles = vec!["admin".to_string()];
    user.permissions = vec!["user:read".to_string()];

    let cloned = user.clone();

    assert_eq!(cloned.id, user.id);
    assert_eq!(cloned.username, user.username);
    assert_eq!(cloned.nickname, user.nickname);
    assert_eq!(cloned.email, user.email);
    assert_eq!(cloned.roles, user.roles);
    assert_eq!(cloned.permissions, user.permissions);
}

#[test]
fn test_global_tools_new() {
    let token_manager = create_test_token_manager();
    let tools = GlobalTools::new(token_manager);

    // 测试能够创建 GlobalTools
    assert!(tools
        .token_manager()
        .generate_access_token("test_user", json!({}))
        .is_ok());
}

#[test]
fn test_global_tools_register_and_get_tool() {
    let tools = create_test_tools();

    // 注册工具
    let redis_client = Arc::new("redis://localhost".to_string());
    tools.register_tool("redis", redis_client.clone());

    // 获取工具
    let retrieved: Option<Arc<String>> = tools.get_tool("redis");
    assert!(retrieved.is_some());
    assert_eq!(*retrieved.unwrap(), "redis://localhost");
}

#[test]
fn test_global_tools_get_nonexistent_tool() {
    let tools = create_test_tools();

    // 获取不存在的工具
    let result: Option<Arc<String>> = tools.get_tool("nonexistent");
    assert!(result.is_none());
}

#[test]
fn test_global_tools_get_tool_wrong_type() {
    let tools = create_test_tools();

    // 注册字符串类型的工具
    let redis_client = Arc::new("redis://localhost".to_string());
    tools.register_tool("redis", redis_client);

    // 尝试以错误的类型获取
    let result: Option<Arc<i32>> = tools.get_tool("redis");
    assert!(result.is_none());
}

#[test]
fn test_global_tools_token_manager() {
    let tools = create_test_tools();

    // 测试获取 TokenManager
    let token_manager = tools.token_manager();
    let token = token_manager.generate_access_token("user_123", json!({"role": "admin"}));
    assert!(token.is_ok());
}

#[test]
fn test_action_context_new() {
    let request = Request::new(json!({ "name": "alice" }));
    let tools = create_test_tools();

    let context = ActionContext::new(request.clone(), tools.clone());

    // 测试上下文创建成功
    assert!(context.user.is_none());
    assert!(context.table_config.is_none());
}

#[test]
fn test_action_context_with_user() {
    let request = Request::new(json!({}));
    let tools = create_test_tools();
    let user = User::new(1, "alice");

    let context = ActionContext::new(request, tools).with_user(user.clone());

    assert!(context.user.is_some());
    assert_eq!(context.user.unwrap().id, 1);
}

#[test]
fn test_action_context_param() {
    let request = Request::new(json!({
        "name": "alice",
        "age": 30
    }));
    let tools = create_test_tools();
    let context = ActionContext::new(request, tools);

    // 测试获取必填参数
    let name: String = context.param("name").unwrap();
    assert_eq!(name, "alice");

    let age: i64 = context.param("age").unwrap();
    assert_eq!(age, 30);
}

#[test]
fn test_action_context_param_missing() {
    let request = Request::new(json!({}));
    let tools = create_test_tools();
    let context = ActionContext::new(request, tools);

    // 测试获取不存在的参数
    let result: Result<String, _> = context.param("name");
    assert!(result.is_err());
}

#[test]
fn test_action_context_param_optional() {
    let request = Request::new(json!({
        "name": "alice"
    }));
    let tools = create_test_tools();
    let context = ActionContext::new(request, tools);

    // 测试获取存在的可选参数
    let name: Option<String> = context.param_optional("name");
    assert_eq!(name, Some("alice".to_string()));

    // 测试获取不存在的可选参数
    let email: Option<String> = context.param_optional("email");
    assert_eq!(email, None);
}

#[test]
fn test_action_context_user_roles() {
    let request = Request::new(json!({}));
    let tools = create_test_tools();

    // 没有用户时返回空列表
    let context = ActionContext::new(request.clone(), tools.clone());
    assert_eq!(context.user_roles(), Vec::<String>::new());

    // 有用户时返回用户角色
    let mut user = User::new(1, "alice");
    user.roles = vec!["admin".to_string(), "user".to_string()];
    let context = ActionContext::new(request, tools).with_user(user);
    assert_eq!(
        context.user_roles(),
        vec!["admin".to_string(), "user".to_string()]
    );
}
