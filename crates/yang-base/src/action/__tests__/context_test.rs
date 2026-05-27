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

#[ignore = "H-1 重构期间停用：param 已移除，Task 6 后用 extract_input 重写"]
#[test]
fn test_action_context_param() {
    // H-1: param 方法已移除，此测试保留占位；Task 6 后用 extract_input 重写
    let _ = create_test_tools();
}

#[ignore = "H-1 重构期间停用：param 已移除，Task 6 后用 extract_input 重写"]
#[test]
fn test_action_context_param_missing() {
    // H-1: param 方法已移除，此测试保留占位；Task 6 后用 extract_input 重写
    let _ = create_test_tools();
}

#[ignore = "H-1 重构期间停用：param_optional 已移除，Task 6 后用 extract_input 重写"]
#[test]
fn test_action_context_param_optional() {
    // H-1: param_optional 方法已移除，此测试保留占位；Task 6 后用 extract_input 重写
    let _ = create_test_tools();
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

#[test]
fn test_action_context_user_roles_slice() {
    let request = Request::new(json!({}));
    let tools = create_test_tools();

    // 没有用户时返回空切片
    let context = ActionContext::new(request.clone(), tools.clone());
    assert_eq!(context.user_roles_slice(), &[] as &[String]);

    // 有用户时返回用户角色切片
    let mut user = User::new(1, "alice");
    user.roles = vec!["admin".to_string(), "user".to_string()];
    let context = ActionContext::new(request, tools).with_user(user);
    assert_eq!(
        context.user_roles_slice(),
        &["admin".to_string(), "user".to_string()]
    );
}

#[test]
fn test_action_context_path_param() {
    let request = Request::new(json!({})).path_param("id", "123");
    let tools = create_test_tools();
    let context = ActionContext::new(request, tools);

    // 获取存在的路径参数
    let id: String = context.path_param("id").unwrap();
    assert_eq!(id, "123");

    // 获取不存在的路径参数
    let result: Result<String, _> = context.path_param("nonexistent");
    assert!(result.is_err());
}

#[ignore = "H-1 重构期间停用：query_param 已移除，Task 6 后用新 API 重写"]
#[test]
fn test_action_context_query_param() {
    // H-1: query_param 方法已移除，此测试保留占位；Task 6 后用新 API 重写
    let _ = create_test_tools();
}

#[allow(deprecated)]
#[test]
fn test_action_context_param_optional_strict() {
    let request = Request::new(json!({
        "age": 25,
        "name": "alice",
        "bad_age": "not_a_number"
    }));
    let tools = create_test_tools();
    let context = ActionContext::new(request, tools);

    // 参数存在且类型匹配
    let age: Result<Option<i64>, _> = context.param_optional_strict("age");
    assert_eq!(age.unwrap(), Some(25));

    // 参数不存在
    let missing: Result<Option<i64>, _> = context.param_optional_strict("missing");
    assert_eq!(missing.unwrap(), None);

    // 参数存在但类型不匹配，返回错误
    let bad: Result<Option<i64>, _> = context.param_optional_strict("bad_age");
    assert!(bad.is_err());
}

/// 测试 GlobalTools 全局单例功能
///
/// 注意：由于 OnceLock 只能初始化一次，所有单例相关测试放在同一个函数中按顺序执行
/// 需求: 3.1, 3.2, 3.3, 3.4
#[test]
fn test_global_tools_singleton() {
    // 步骤 1：在初始化之前，获取应返回错误
    // 注意：这个测试可能在其他测试已初始化单例后运行，所以我们先检查当前状态
    let initial_state = GlobalTools::get();

    if let Err(err) = initial_state {
        // 单例尚未初始化，测试未初始化场景
        assert!(
            matches!(err, crate::error::BaseError::ConfigError(_)),
            "未初始化时应返回 ConfigError，实际返回: {:?}",
            err
        );
        // 验证错误信息包含预期内容
        if let crate::error::BaseError::ConfigError(msg) = err {
            assert!(
                msg.contains("GlobalTools 未初始化"),
                "错误信息应包含 '未初始化'，实际: {}",
                msg
            );
        }

        // 步骤 2：初始化单例
        let token_manager = create_test_token_manager();
        let result = GlobalTools::init(token_manager);
        assert!(result.is_ok(), "第一次初始化应成功，实际错误: {:?}", result.err());

        // 步骤 3：初始化后获取应成功
        let tools = GlobalTools::get();
        assert!(tools.is_ok(), "初始化后获取应成功，实际错误: {:?}", tools.err());

        // 步骤 4：重复初始化应返回错误
        let token_manager2 = create_test_token_manager();
        let result2 = GlobalTools::init(token_manager2);
        assert!(result2.is_err(), "重复初始化应返回错误");
        let err2 = result2.unwrap_err();
        assert!(
            matches!(err2, crate::error::BaseError::ConfigError(_)),
            "重复初始化应返回 ConfigError，实际: {:?}",
            err2
        );
        if let crate::error::BaseError::ConfigError(msg) = err2 {
            assert!(
                msg.contains("GlobalTools 已初始化"),
                "错误信息应包含 '已初始化'，实际: {}",
                msg
            );
        }
    } else {
        // 单例已经初始化（其他测试先运行了）
        // 测试重复初始化应返回错误
        let token_manager = create_test_token_manager();
        let result = GlobalTools::init(token_manager);
        assert!(result.is_err(), "已初始化后重复初始化应返回错误");
        let err = result.unwrap_err();
        assert!(
            matches!(err, crate::error::BaseError::ConfigError(_)),
            "重复初始化应返回 ConfigError，实际: {:?}",
            err
        );
        if let crate::error::BaseError::ConfigError(msg) = err {
            assert!(
                msg.contains("GlobalTools 已初始化"),
                "错误信息应包含 '已初始化'，实际: {}",
                msg
            );
        }

        // 单例已初始化，获取应成功
        let tools = GlobalTools::get();
        assert!(tools.is_ok(), "已初始化后获取应成功，实际错误: {:?}", tools.err());
    }
}

/// 测试 ActionContext::new_with_global_tools
/// 需求: 3.5
#[test]
fn test_action_context_new_with_global_tools() {
    // 确保全局单例已初始化（如果还没有）
    if GlobalTools::get().is_err() {
        let token_manager = create_test_token_manager();
        let _ = GlobalTools::init(token_manager);
    }

    // 使用全局单例创建上下文
    let request = Request::new(serde_json::json!({ "name": "test" }));
    let result = ActionContext::new_with_global_tools(request);
    assert!(result.is_ok(), "全局单例已初始化时应成功创建上下文，实际错误: {:?}", result.err());

    let context = result.unwrap();
    // 验证上下文创建成功
    assert!(context.user.is_none());
    assert!(context.table_config.is_none());
}
