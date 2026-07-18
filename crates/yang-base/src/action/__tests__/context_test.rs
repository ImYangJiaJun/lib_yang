//! ActionContext 和 User 单元测试
#![cfg(feature = "token")]

use crate::action::{ActionContext, Request, RequestMeta, User};
#[cfg(feature = "mysql")]
use crate::action::{TenantContext, TenantId};
use crate::error::BaseError;
use crate::token::TokenManager;
use crate::tools::{Tools, ToolsBuilder};
use jsonwebtoken::Algorithm;
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;

#[cfg(feature = "mysql")]
fn tenant_table() -> crate::table::TableDefinition {
    crate::table::Table::new("tenant_rows")
        .fields([
            crate::table::Field::id("id"),
            crate::table::Field::bigint("org_id")
                .required()
                .tenant_key(),
            crate::table::Field::string("name", 64),
        ])
        .build()
        .expect("租户表定义应有效")
}

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

/// 创建测试用的 Tools
fn create_test_tools() -> Arc<Tools> {
    Arc::new(
        ToolsBuilder::new()
            .token(create_test_token_manager())
            .build()
            .expect("测试 Tools 应构建成功"),
    )
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
    user.permissions = HashSet::from(["user:read".to_string(), "user:write".to_string()]);

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
    user.roles = HashSet::from(["admin".to_string(), "user".to_string()]);

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
    user.roles = HashSet::from(["admin".to_string(), "user".to_string()]);

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
    user.roles = HashSet::from(["admin".to_string()]);

    // 空列表检查，应该返回 false
    assert!(!user.has_any_role(&[]));
}

#[test]
fn test_user_clone() {
    let mut user = User::new(1, "alice");
    user.nickname = "Alice".to_string();
    user.email = "alice@example.com".to_string();
    user.roles = HashSet::from(["admin".to_string()]);
    user.permissions = HashSet::from(["user:read".to_string()]);

    let cloned = user.clone();

    assert_eq!(cloned.id, user.id);
    assert_eq!(cloned.username, user.username);
    assert_eq!(cloned.nickname, user.nickname);
    assert_eq!(cloned.email, user.email);
    assert_eq!(cloned.roles, user.roles);
    assert_eq!(cloned.permissions, user.permissions);
}

#[test]
fn test_tools_token_manager() {
    let tools = create_test_tools();

    let token_manager = tools.token().expect("TokenManager 应存在");
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
    assert!(matches!(
        context.table_definition(),
        Err(BaseError::TableDefinitionNotSet)
    ));
}

#[cfg(feature = "mysql")]
#[test]
fn tenant_table_is_fail_closed_and_system_bypass_is_explicit() {
    let base = ActionContext::new(Request::new(json!({})), create_test_tools())
        .with_table_definition(tenant_table());
    assert!(matches!(
        base.table_query(),
        Err(BaseError::Unauthorized(_))
    ));

    let tenant = ActionContext::new(Request::new(json!({})), create_test_tools())
        .with_table_definition(tenant_table())
        .with_tenant(TenantContext::new(TenantId::new(7)));
    let (tenant_sql, _) = tenant
        .table_query()
        .expect("租户上下文应生成查询")
        .build_select_sql_for_test()
        .expect("租户查询 SQL 应可构建");
    assert!(tenant_sql.contains("`org_id` = ?"));

    let system = ActionContext::new(Request::new(json!({})), create_test_tools())
        .with_table_definition(tenant_table())
        .with_tenant(TenantContext::system());
    let (system_sql, _) = system
        .table_query()
        .expect("system 上下文应显式绕过租户范围")
        .build_select_sql_for_test()
        .expect("system 查询 SQL 应可构建");
    assert!(!system_sql.contains("`org_id` = ?"));
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
fn test_action_context_authenticated_user_getter() {
    let request = Request::new(json!({}));
    let tools = create_test_tools();

    let context = ActionContext::new(request.clone(), tools.clone());
    assert!(context.authenticated_user().is_none());

    let context = ActionContext::new(request, tools).with_user(User::new(1, "alice"));
    let user = context
        .authenticated_user()
        .expect("内部注入用户后应可只读获取");
    assert_eq!(user.id, 1);
    assert_eq!(user.username, "alice");
}

#[test]
fn test_action_context_user_roles() {
    let request = Request::new(json!({}));
    let tools = create_test_tools();

    // 没有用户时返回空列表
    let context = ActionContext::new(request.clone(), tools.clone());
    assert!(context.user_roles().is_empty());

    // 有用户时返回用户角色（顺序不保证，用集合比较）
    let mut user = User::new(1, "alice");
    user.roles = HashSet::from(["admin".to_string(), "user".to_string()]);
    let context = ActionContext::new(request, tools).with_user(user);
    let roles = context.user_roles();
    assert!(roles.contains(&"admin".to_string()));
    assert!(roles.contains(&"user".to_string()));
    assert_eq!(roles.len(), 2);
}

#[test]
fn test_action_context_user_roles_set() {
    let request = Request::new(json!({}));
    let tools = create_test_tools();

    // 没有用户时返回空集合
    let context = ActionContext::new(request.clone(), tools.clone());
    assert!(context.user_roles_set().is_none());

    // 有用户时返回用户角色集合
    let mut user = User::new(1, "alice");
    user.roles = HashSet::from(["admin".to_string(), "user".to_string()]);
    let context = ActionContext::new(request, tools).with_user(user);
    let roles_set = context.user_roles_set().unwrap();
    assert!(roles_set.contains("admin"));
    assert!(roles_set.contains("user"));
    assert!(!roles_set.contains("guest"));
}

#[test]
fn action_context_keeps_the_explicit_app_tools() {
    let tools = create_test_tools();
    let context = ActionContext::new(
        Request::new(serde_json::json!({ "name": "test" })),
        Arc::clone(&tools),
    );

    assert!(std::ptr::eq(context.tools(), tools.as_ref()));
    assert!(context.user.is_none());
    assert!(matches!(
        context.table_definition(),
        Err(BaseError::TableDefinitionNotSet)
    ));
}

#[test]
fn test_action_context_request_meta_default_and_builder_paths() {
    let context = ActionContext::new(Request::new(json!({})), create_test_tools());
    assert_eq!(context.request_meta, RequestMeta::default());

    let meta = RequestMeta::new()
        .with_method("PATCH")
        .with_original_uri("/users/42")
        .with_peer_addr("127.0.0.1:43120".parse().expect("peer 地址应合法"));
    let context = context.with_request_meta(meta.clone());

    assert_eq!(context.request_meta, meta);
}
