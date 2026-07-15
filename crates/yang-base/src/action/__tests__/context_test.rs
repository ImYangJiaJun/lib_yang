//! ActionContext 和 User 单元测试
#![cfg(feature = "token")]

use crate::action::{ActionContext, GlobalTools, Request, RequestMeta, User};
use crate::error::BaseError;
use crate::token::TokenManager;
use jsonwebtoken::Algorithm;
use serde_json::json;
use std::collections::HashSet;
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
    tools
        .register_tool("redis", redis_client.clone())
        .expect("首次注册 redis 工具应成功");

    // 获取工具
    let retrieved: Option<Arc<String>> = tools.get_tool("redis");
    assert!(retrieved.is_some());
    assert_eq!(*retrieved.unwrap(), "redis://localhost");
}

#[test]
fn test_global_tools_register_tool_rejects_duplicate_name() {
    let tools = create_test_tools();

    let first = tools.register_tool("redis", Arc::new("redis://first".to_string()));
    assert!(first.is_ok());

    let err = tools
        .register_tool("redis", Arc::new("redis://second".to_string()))
        .expect_err("重复注册同名工具应失败");

    assert!(matches!(err, BaseError::ConfigError(msg) if msg.contains("工具已注册: redis")));
    let retrieved: Option<Arc<String>> = tools.get_tool("redis");
    assert_eq!(
        retrieved.as_deref().map(String::as_str),
        Some("redis://first")
    );
}

#[test]
fn test_global_tools_trims_tool_names_for_register_and_get() {
    let tools = create_test_tools();

    tools
        .register_tool(" redis ", Arc::new("redis://first".to_string()))
        .expect("工具名应在注册前去除边界空格");

    let retrieved: Option<Arc<String>> = tools.get_tool("redis");
    assert_eq!(
        retrieved.as_deref().map(String::as_str),
        Some("redis://first")
    );

    let retrieved_with_spaces: Option<Arc<String>> = tools.get_tool(" redis ");
    assert_eq!(
        retrieved_with_spaces.as_deref().map(String::as_str),
        Some("redis://first")
    );

    let err = tools
        .register_tool("redis", Arc::new("redis://second".to_string()))
        .expect_err("trim 后的重复工具名应被拒绝");

    assert!(matches!(err, BaseError::ConfigError(msg) if msg.contains("工具已注册: redis")));
}

#[test]
fn test_global_tools_register_tool_rejects_blank_name() {
    let tools = create_test_tools();

    for name in ["", "   "] {
        let err = tools
            .register_tool(name, Arc::new("redis://localhost".to_string()))
            .expect_err("空白工具名应被拒绝");

        assert!(matches!(err, BaseError::ConfigError(msg) if msg.contains("工具名称不能为空")));
        assert!(tools.get_tool::<String>(name).is_none());
    }
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
    tools
        .register_tool("redis", redis_client)
        .expect("首次注册 redis 工具应成功");

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

#[test]
fn test_action_context_path_param_trims_lookup_key() {
    let request = Request::new(json!({})).path_param(" id ", "123");
    let tools = create_test_tools();
    let context = ActionContext::new(request, tools);

    let id: String = context
        .path_param(" id ")
        .expect("路径参数查询 key 应支持边界空格规范化");

    assert_eq!(id, "123");
}

#[test]
fn test_action_context_path_param_rejects_blank_key() {
    let mut request = Request::new(json!({}));
    request
        .path_params
        .insert("".to_string(), "123".to_string());
    let tools = create_test_tools();
    let context = ActionContext::new(request, tools);

    let result: Result<String, _> = context.path_param("");

    match result {
        Err(crate::error::BaseError::ParamInvalid(param, message)) => {
            assert_eq!(param, "");
            assert!(message.contains("路径参数名不能为空"));
        }
        other => panic!("空白路径参数名应返回 ParamInvalid，实际: {:?}", other),
    }
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

#[allow(deprecated)]
#[test]
fn test_action_context_param_optional_strict_rejects_blank_key() {
    let request = Request::new(json!({
        "": 25
    }));
    let tools = create_test_tools();
    let context = ActionContext::new(request, tools);

    let result: Result<Option<i64>, _> = context.param_optional_strict("");

    match result {
        Err(crate::error::BaseError::ParamInvalid(param, message)) => {
            assert_eq!(param, "");
            assert!(message.contains("参数名不能为空"));
        }
        other => panic!("空白参数名应返回 ParamInvalid，实际: {:?}", other),
    }
}

/// 测试 GlobalTools 全局单例功能
///
/// 注意：由于 OnceLock 只能初始化一次，所有单例相关测试放在同一个函数中按顺序执行
/// 需求: 3.1, 3.2, 3.3, 3.4
#[test]
fn test_global_tools_singleton() {
    // GlobalTools 是进程级 OnceLock 单例，多个测试并发运行时存在 TOCTOU 竞态：
    // 「检查未初始化」与「执行初始化」之间，另一个测试可能抢先初始化。
    // 因此这里只断言不随竞态变化的不变量，对「未初始化」消息做机会性校验。

    // 步骤 1（机会性）：若恰好观察到未初始化状态，校验其错误信息
    if let Err(err) = GlobalTools::get() {
        assert!(
            matches!(err, crate::error::BaseError::ConfigError(_)),
            "未初始化时应返回 ConfigError，实际返回: {:?}",
            err
        );
        if let crate::error::BaseError::ConfigError(msg) = err {
            assert!(
                msg.contains("GlobalTools 未初始化"),
                "错误信息应包含 '未初始化'，实际: {}",
                msg
            );
        }
    }

    // 步骤 2：确保单例已初始化（由本测试或并发测试完成均可，init 幂等失败无害）
    if GlobalTools::get().is_err() {
        let _ = GlobalTools::init(create_test_token_manager());
    }

    // 步骤 3：初始化后获取必定成功
    let tools = GlobalTools::get();
    assert!(
        tools.is_ok(),
        "初始化后获取应成功，实际错误: {:?}",
        tools.err()
    );

    // 步骤 4：重复初始化必定返回 ConfigError("已初始化")
    let err2 =
        GlobalTools::init(create_test_token_manager()).expect_err("已初始化后重复初始化应返回错误");
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
    assert!(
        result.is_ok(),
        "全局单例已初始化时应成功创建上下文，实际错误: {:?}",
        result.err()
    );

    let context = result.unwrap();
    // 验证上下文创建成功
    assert!(context.user.is_none());
    assert!(context.table_config.is_none());
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
