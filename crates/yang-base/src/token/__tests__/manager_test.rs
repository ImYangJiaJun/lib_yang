//! TokenManager 单元测试

use crate::error::BaseError;
use crate::token::TokenManager;
use jsonwebtoken::Algorithm;
use serde_json::json;

/// 测试对称加密 Token 生成和验证
#[test]
fn test_symmetric_token_generation_and_verification() {
    // 创建对称加密的 Token 管理器
    let manager = TokenManager::new_symmetric(
        "test_secret_key_12345",
        Algorithm::HS256,
        "test-issuer".to_string(),
        "test-audience".to_string(),
        3600,  // 1 小时
        86400, // 1 天
    );

    // 生成 Access Token
    let custom_claims = json!({
        "role": "admin",
        "permissions": ["read", "write", "delete"]
    });

    let token = manager
        .generate_access_token("user_123", custom_claims.clone())
        .expect("生成 Token 失败");

    // 验证 Token 不为空
    assert!(!token.is_empty());

    // 验证 Token
    let claims = manager.verify_token(&token).expect("验证 Token 失败");

    // 验证标准声明
    assert_eq!(claims.iss, "test-issuer");
    assert_eq!(claims.sub, "user_123");
    assert_eq!(claims.aud, "test-audience");
    assert_eq!(claims.token_type, crate::token::TokenType::Access);

    // 验证自定义声明
    assert_eq!(claims.custom.get("role").unwrap(), "admin");
    assert_eq!(
        claims.custom.get("permissions").unwrap(),
        &json!(["read", "write", "delete"])
    );
}

/// 测试不同对称加密算法
#[test]
fn test_symmetric_algorithms() {
    let algorithms = vec![Algorithm::HS256, Algorithm::HS384, Algorithm::HS512];

    for algorithm in algorithms {
        let manager = TokenManager::new_symmetric(
            "test_secret_key",
            algorithm,
            "issuer".to_string(),
            "audience".to_string(),
            3600,
            86400,
        );

        let token = manager
            .generate_access_token("user_test", json!({}))
            .expect("生成 Token 失败");

        let claims = manager.verify_token(&token).expect("验证 Token 失败");

        assert_eq!(claims.sub, "user_test");
    }
}

/// 测试 Refresh Token 生成和验证
#[test]
fn test_refresh_token_generation() {
    let manager = TokenManager::new_symmetric(
        "test_secret",
        Algorithm::HS256,
        "issuer".to_string(),
        "audience".to_string(),
        3600,
        86400,
    );

    // 生成 Refresh Token
    let refresh_token = manager
        .generate_refresh_token("user_456")
        .expect("生成 Refresh Token 失败");

    // 验证 Refresh Token
    let claims = manager
        .verify_token(&refresh_token)
        .expect("验证 Refresh Token 失败");

    // 验证 Token 类型
    assert_eq!(claims.token_type, crate::token::TokenType::Refresh);
    assert_eq!(claims.sub, "user_456");

    // 验证自定义声明为空
    assert!(claims.custom.is_null() || claims.custom.as_object().unwrap().is_empty());
}

/// 测试 Token 对生成
#[test]
fn test_token_pair_generation() {
    let manager = TokenManager::new_symmetric(
        "test_secret",
        Algorithm::HS256,
        "issuer".to_string(),
        "audience".to_string(),
        3600,
        86400,
    );

    let custom_claims = json!({"role": "user"});

    // 生成 Token 对
    let (access_token, refresh_token) = manager
        .generate_token_pair("user_789", custom_claims)
        .expect("生成 Token 对失败");

    // 验证 Access Token
    let access_claims = manager
        .verify_token(&access_token)
        .expect("验证 Access Token 失败");
    assert_eq!(access_claims.token_type, crate::token::TokenType::Access);
    assert_eq!(access_claims.sub, "user_789");

    // 验证 Refresh Token
    let refresh_claims = manager
        .verify_token(&refresh_token)
        .expect("验证 Refresh Token 失败");
    assert_eq!(refresh_claims.token_type, crate::token::TokenType::Refresh);
    assert_eq!(refresh_claims.sub, "user_789");
    assert!(
        refresh_claims.custom.is_null()
            || refresh_claims
                .custom
                .as_object()
                .is_some_and(serde_json::Map::is_empty),
        "旧入口必须保持 Refresh Token 无自定义声明的兼容语义"
    );
}

/// 测试 Access/Refresh Token 分别携带由同一快照派生的声明
#[test]
fn test_token_pair_with_distinct_claims() {
    let manager = TokenManager::new_symmetric(
        "test_secret",
        Algorithm::HS256,
        "issuer".to_string(),
        "audience".to_string(),
        3600,
        86400,
    );

    let (access_token, refresh_token) = manager
        .generate_token_pair_with_refresh_claims(
            "user_790",
            json!({
                "authz_version": 7,
                "roles": ["admin"],
                "permissions": ["users:write"]
            }),
            json!({"authz_version": 7}),
        )
        .expect("生成带独立声明的 Token 对失败");

    let access_claims = manager
        .verify_token(&access_token)
        .expect("验证 Access Token 失败");
    let refresh_claims = manager
        .verify_token(&refresh_token)
        .expect("验证 Refresh Token 失败");

    assert_eq!(access_claims.custom["authz_version"], 7);
    assert_eq!(access_claims.custom["roles"], json!(["admin"]));
    assert_eq!(refresh_claims.custom["authz_version"], 7);
    assert!(
        refresh_claims.custom.get("roles").is_none(),
        "Refresh Token 不应复制角色等完整授权声明"
    );
}

/// 测试 Token 验证失败（错误的密钥）
#[test]
fn test_token_verification_with_wrong_secret() {
    // 使用密钥 A 生成 Token
    let manager_a = TokenManager::new_symmetric(
        "secret_key_a",
        Algorithm::HS256,
        "issuer".to_string(),
        "audience".to_string(),
        3600,
        86400,
    );

    let token = manager_a
        .generate_access_token("user_test", json!({}))
        .expect("生成 Token 失败");

    // 使用密钥 B 验证 Token（应该失败）
    let manager_b = TokenManager::new_symmetric(
        "secret_key_b",
        Algorithm::HS256,
        "issuer".to_string(),
        "audience".to_string(),
        3600,
        86400,
    );

    let result = manager_b.verify_token(&token);
    assert!(result.is_err());

    // 验证错误消息包含签名相关信息
    if let Err(BaseError::TokenVerifyFailed(ref err)) = result {
        let msg = err.to_string();
        assert!(msg.contains("signature") || msg.contains("Invalid"));
    }
    // 断言错误类型为 TokenVerifyFailed
    assert!(
        matches!(result, Err(BaseError::TokenVerifyFailed(_))),
        "期望 TokenVerifyFailed 错误，实际: {:?}",
        result
    );
}

/// 测试 Token 验证失败（错误的签发者）
#[test]
fn test_token_verification_with_wrong_issuer() {
    // 使用签发者 A 生成 Token
    let manager_a = TokenManager::new_symmetric(
        "secret_key",
        Algorithm::HS256,
        "issuer_a".to_string(),
        "audience".to_string(),
        3600,
        86400,
    );

    let token = manager_a
        .generate_access_token("user_test", json!({}))
        .expect("生成 Token 失败");

    // 使用签发者 B 验证 Token（应该失败）
    let manager_b = TokenManager::new_symmetric(
        "secret_key",
        Algorithm::HS256,
        "issuer_b".to_string(),
        "audience".to_string(),
        3600,
        86400,
    );

    let result = manager_b.verify_token(&token);
    assert!(result.is_err());
}

/// 测试 Token 验证失败（错误的受众）
#[test]
fn test_token_verification_with_wrong_audience() {
    // 使用受众 A 生成 Token
    let manager_a = TokenManager::new_symmetric(
        "secret_key",
        Algorithm::HS256,
        "issuer".to_string(),
        "audience_a".to_string(),
        3600,
        86400,
    );

    let token = manager_a
        .generate_access_token("user_test", json!({}))
        .expect("生成 Token 失败");

    // 使用受众 B 验证 Token（应该失败）
    let manager_b = TokenManager::new_symmetric(
        "secret_key",
        Algorithm::HS256,
        "issuer".to_string(),
        "audience_b".to_string(),
        3600,
        86400,
    );

    let result = manager_b.verify_token(&token);
    assert!(result.is_err());
}

/// 测试 Token 过期验证
/// 注意：由于 jsonwebtoken 库可能有时间容差，我们使用足够长的等待时间
#[test]
fn test_token_expiration() {
    // 创建一个 Token 有效期为 3 秒的管理器
    let manager = TokenManager::new_symmetric(
        "test_secret",
        Algorithm::HS256,
        "issuer".to_string(),
        "audience".to_string(),
        3, // 3 秒过期
        86400,
    );

    let token = manager
        .generate_access_token("user_test", json!({}))
        .expect("生成 Token 失败");

    // 立即验证应该成功
    assert!(manager.verify_token(&token).is_ok());

    // 等待 5 秒后验证应该失败
    std::thread::sleep(std::time::Duration::from_secs(5));

    let result = manager.verify_token(&token);
    assert!(result.is_err(), "Token 应该已过期但验证仍然成功");

    // 验证错误类型为 TokenExpired
    if let Err(BaseError::TokenExpired) = &result {
        // TokenExpired 显示"Token 已过期"，符合预期
    }
    // 断言错误类型为 TokenExpired
    assert!(
        matches!(result, Err(BaseError::TokenExpired)),
        "期望 TokenExpired 错误，实际: {:?}",
        result
    );
}

/// 测试 parse_token_unsafe 方法
/// 注意：由于 jsonwebtoken 库可能有时间容差，我们使用足够长的等待时间
#[allow(deprecated)]
#[allow(clippy::expect_used)]
#[test]
fn test_parse_token_unsafe() {
    let manager = TokenManager::new_symmetric(
        "test_secret",
        Algorithm::HS256,
        "issuer".to_string(),
        "audience".to_string(),
        3, // 3 秒过期
        86400,
    );

    let custom_claims = json!({"role": "admin"});
    let token = manager
        .generate_access_token("user_test", custom_claims)
        .expect("生成 Token 失败");

    // 等待 Token 过期
    std::thread::sleep(std::time::Duration::from_secs(5));

    // 正常验证应该失败
    assert!(
        manager.verify_token(&token).is_err(),
        "Token 应该已过期但验证仍然成功"
    );

    // 不安全解析应该成功（不验证过期时间）
    let claims = manager.parse_token_unsafe(&token).expect("解析 Token 失败");

    assert_eq!(claims.sub, "user_test");
    assert_eq!(claims.custom.get("role").unwrap(), "admin");
}

/// 测试 is_token_expiring_soon 方法
#[test]
fn test_is_token_expiring_soon() {
    // 创建一个 Token 有效期为 10 秒的管理器
    let manager = TokenManager::new_symmetric(
        "test_secret",
        Algorithm::HS256,
        "issuer".to_string(),
        "audience".to_string(),
        10, // 10 秒过期
        86400,
    );

    let token = manager
        .generate_access_token("user_test", json!({}))
        .expect("生成 Token 失败");

    // 检查是否在 20 秒内过期（应该返回 true）
    let expiring_soon = manager
        .is_token_expiring_soon(&token, 20)
        .expect("检查失败");
    assert!(expiring_soon);

    // 检查是否在 5 秒内过期（应该返回 false）
    let expiring_soon = manager.is_token_expiring_soon(&token, 5).expect("检查失败");
    assert!(!expiring_soon);
}

/// 测试 refresh_access_token 方法
///
/// 此方法内部调用 `verify_token_checked`，需要 Redis 黑名单支持。
/// 运行时需通过 `ToolsBuilder` 注入 Redis 撤销存储，并通过 `--ignored` 执行：
/// ```bash
/// cargo test --test '' test_refresh_access_token -- --ignored --test-threads=1
/// ```
#[tokio::test]
#[ignore = "需要 Redis（verify_token_checked 依赖黑名单查询）"]
async fn test_refresh_access_token() {
    let manager = TokenManager::new_symmetric(
        "test_secret",
        Algorithm::HS256,
        "issuer".to_string(),
        "audience".to_string(),
        3600,
        86400,
    );

    // 生成 Refresh Token
    let refresh_token = manager
        .generate_refresh_token("user_test")
        .expect("生成 Refresh Token 失败");

    // 使用 Refresh Token 刷新 Access Token（async: 走 verify_token_checked 含黑名单检查）
    let new_access_token = manager
        .refresh_access_token(&refresh_token, json!({"role": "user"}))
        .await
        .expect("刷新 Access Token 失败");

    // 验证新的 Access Token
    let claims = manager
        .verify_token(&new_access_token)
        .expect("验证新 Access Token 失败");

    assert_eq!(claims.token_type, crate::token::TokenType::Access);
    assert_eq!(claims.sub, "user_test");
    assert_eq!(claims.custom.get("role").unwrap(), "user");
}

/// 测试使用 Access Token 刷新应该失败
///
/// 此方法内部调用 `verify_token_checked`，需要 Redis 黑名单支持。
/// 运行时需通过 `ToolsBuilder` 注入 Redis 撤销存储，并通过 `--ignored` 执行：
/// ```bash
/// cargo test --test '' test_refresh_with_access_token_should_fail -- --ignored --test-threads=1
/// ```
#[tokio::test]
#[ignore = "需要 Redis（verify_token_checked 依赖黑名单查询）"]
async fn test_refresh_with_access_token_should_fail() {
    let manager = TokenManager::new_symmetric(
        "test_secret",
        Algorithm::HS256,
        "issuer".to_string(),
        "audience".to_string(),
        3600,
        86400,
    );

    // 生成 Access Token
    let access_token = manager
        .generate_access_token("user_test", json!({}))
        .expect("生成 Access Token 失败");

    // 尝试使用 Access Token 刷新（应该失败）
    let result = manager.refresh_access_token(&access_token, json!({})).await;

    assert!(result.is_err());

    // 断言错误类型为 TokenTypeInvalid 且消息包含 "refresh"
    assert!(
        matches!(result, Err(BaseError::TokenTypeInvalid(ref msg)) if msg.contains("refresh")),
        "期望 TokenTypeInvalid 错误，实际: {:?}",
        result
    );
}

/// 测试自定义声明的序列化和反序列化
#[test]
fn test_custom_claims_serialization() {
    let manager = TokenManager::new_symmetric(
        "test_secret",
        Algorithm::HS256,
        "issuer".to_string(),
        "audience".to_string(),
        3600,
        86400,
    );

    // 创建复杂的自定义声明
    let custom_claims = json!({
        "user_id": 12345,
        "username": "john_doe",
        "email": "john@example.com",
        "roles": ["admin", "user"],
        "permissions": {
            "read": true,
            "write": true,
            "delete": false
        },
        "metadata": {
            "last_login": "2024-01-01T00:00:00Z",
            "login_count": 42
        }
    });

    let token = manager
        .generate_access_token("user_12345", custom_claims.clone())
        .expect("生成 Token 失败");

    let claims = manager.verify_token(&token).expect("验证 Token 失败");

    // 验证所有自定义字段
    assert_eq!(claims.custom.get("user_id").unwrap(), 12345);
    assert_eq!(claims.custom.get("username").unwrap(), "john_doe");
    assert_eq!(claims.custom.get("email").unwrap(), "john@example.com");
    assert_eq!(
        claims.custom.get("roles").unwrap(),
        &json!(["admin", "user"])
    );

    let permissions = claims.custom.get("permissions").unwrap();
    assert_eq!(permissions.get("read").unwrap(), true);
    assert_eq!(permissions.get("write").unwrap(), true);
    assert_eq!(permissions.get("delete").unwrap(), false);

    let metadata = claims.custom.get("metadata").unwrap();
    assert_eq!(metadata.get("last_login").unwrap(), "2024-01-01T00:00:00Z");
    assert_eq!(metadata.get("login_count").unwrap(), 42);
}

/// 测试空自定义声明
#[test]
fn test_empty_custom_claims() {
    let manager = TokenManager::new_symmetric(
        "test_secret",
        Algorithm::HS256,
        "issuer".to_string(),
        "audience".to_string(),
        3600,
        86400,
    );

    let token = manager
        .generate_access_token("user_test", json!({}))
        .expect("生成 Token 失败");

    let claims = manager.verify_token(&token).expect("验证 Token 失败");

    assert!(claims.custom.is_object());
    assert_eq!(claims.custom.as_object().unwrap().len(), 0);
}

/// 测试 JTI 唯一性
#[test]
fn test_jti_uniqueness() {
    let manager = TokenManager::new_symmetric(
        "test_secret",
        Algorithm::HS256,
        "issuer".to_string(),
        "audience".to_string(),
        3600,
        86400,
    );

    // 生成多个 Token
    let token1 = manager
        .generate_access_token("user_test", json!({}))
        .expect("生成 Token 1 失败");
    let token2 = manager
        .generate_access_token("user_test", json!({}))
        .expect("生成 Token 2 失败");
    let token3 = manager
        .generate_access_token("user_test", json!({}))
        .expect("生成 Token 3 失败");

    // 验证 JTI 唯一性
    let claims1 = manager.verify_token(&token1).expect("验证 Token 1 失败");
    let claims2 = manager.verify_token(&token2).expect("验证 Token 2 失败");
    let claims3 = manager.verify_token(&token3).expect("验证 Token 3 失败");

    assert_ne!(claims1.jti, claims2.jti);
    assert_ne!(claims1.jti, claims3.jti);
    assert_ne!(claims2.jti, claims3.jti);
}
