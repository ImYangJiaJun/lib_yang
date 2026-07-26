//! TokenManager 单元测试

use crate::error::BaseError;
use crate::token::TokenManager;
use jsonwebtoken::Algorithm;
use serde_json::json;

const ACTIVE_SECRET: &str = "active-secret-0123456789abcdef0123456789abcdef";
const RETIRING_SECRET: &str = "retiring-secret-0123456789abcdef0123456789abcdef";

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

#[test]
fn keyring_signs_with_active_kid_and_verifies_active_token() {
    let manager = TokenManager::new_symmetric_keyring(
        "2026-07-active".to_string(),
        ACTIVE_SECRET,
        Vec::new(),
        Algorithm::HS256,
        "issuer".to_string(),
        "audience".to_string(),
        3600,
        86400,
    )
    .expect("合法 keyring 应构建成功");

    let token = manager
        .generate_access_token("user-keyring", json!({}))
        .expect("active key 应签发成功");
    let header = jsonwebtoken::decode_header(&token).expect("签发结果应有合法 Header");

    assert_eq!(header.kid.as_deref(), Some("2026-07-active"));
    assert_eq!(
        manager
            .verify_token(&token)
            .expect("active key Token 应验证成功")
            .sub,
        "user-keyring"
    );
}

#[test]
fn rotated_keyring_accepts_retiring_token_but_never_signs_with_retiring_key() {
    let previous = TokenManager::new_symmetric_keyring(
        "2026-06-retiring".to_string(),
        RETIRING_SECRET,
        Vec::new(),
        Algorithm::HS256,
        "issuer".to_string(),
        "audience".to_string(),
        3600,
        86400,
    )
    .expect("旧 keyring 应构建成功");
    let old_token = previous
        .generate_refresh_token("user-rotation")
        .expect("旧 active key 应签发成功");

    let rotated = TokenManager::new_symmetric_keyring(
        "2026-07-active".to_string(),
        ACTIVE_SECRET,
        vec![("2026-06-retiring".to_string(), RETIRING_SECRET.to_string())],
        Algorithm::HS256,
        "issuer".to_string(),
        "audience".to_string(),
        3600,
        86400,
    )
    .expect("轮换后 keyring 应构建成功");

    assert_eq!(
        rotated
            .verify_token(&old_token)
            .expect("retiring key 应验证存量 Token")
            .sub,
        "user-rotation"
    );
    let new_token = rotated
        .generate_refresh_token("user-rotation")
        .expect("新 active key 应签发成功");
    assert_eq!(
        jsonwebtoken::decode_header(&new_token)
            .expect("新 Token Header 应合法")
            .kid
            .as_deref(),
        Some("2026-07-active")
    );
    assert!(
        previous.verify_token(&new_token).is_err(),
        "旧 keyring 不得验证未知的新 kid"
    );
}

#[test]
fn keyring_fails_closed_for_missing_or_unknown_kid() {
    let keyring = TokenManager::new_symmetric_keyring(
        "known".to_string(),
        ACTIVE_SECRET,
        Vec::new(),
        Algorithm::HS256,
        "issuer".to_string(),
        "audience".to_string(),
        3600,
        86400,
    )
    .expect("keyring 应构建成功");
    let legacy = TokenManager::new_symmetric(
        ACTIVE_SECRET,
        Algorithm::HS256,
        "issuer".to_string(),
        "audience".to_string(),
        3600,
        86400,
    );
    let unknown = TokenManager::new_symmetric_keyring(
        "unknown".to_string(),
        ACTIVE_SECRET,
        Vec::new(),
        Algorithm::HS256,
        "issuer".to_string(),
        "audience".to_string(),
        3600,
        86400,
    )
    .expect("未知 kid 的独立 keyring 应构建成功");

    let missing_kid = legacy
        .generate_access_token("user", json!({}))
        .expect("legacy Token 应签发成功");
    let unknown_kid = unknown
        .generate_access_token("user", json!({}))
        .expect("未知 kid Token 应签发成功");

    assert!(matches!(
        keyring.verify_token(&missing_kid),
        Err(BaseError::TokenVerifyFailed(_))
    ));
    assert!(matches!(
        keyring.verify_token(&unknown_kid),
        Err(BaseError::TokenVerifyFailed(_))
    ));
}

#[test]
fn keyring_rejects_weak_ambiguous_or_unbounded_keys() {
    let build = |active_id: &str, active_secret: &str, retiring_keys| {
        TokenManager::new_symmetric_keyring(
            active_id.to_string(),
            active_secret,
            retiring_keys,
            Algorithm::HS256,
            "issuer".to_string(),
            "audience".to_string(),
            3600,
            86400,
        )
    };

    assert!(build("contains space", ACTIVE_SECRET, Vec::new()).is_err());
    assert!(build("active", "short-secret", Vec::new()).is_err());
    assert!(build(
        "active",
        ACTIVE_SECRET,
        vec![("active".to_string(), RETIRING_SECRET.to_string())]
    )
    .is_err());
    let excessive = (0..8)
        .map(|index| (format!("retiring-{index}"), RETIRING_SECRET.to_string()))
        .collect();
    assert!(build("active", ACTIVE_SECRET, excessive).is_err());
    assert!(TokenManager::new_symmetric_keyring(
        "active".to_string(),
        ACTIVE_SECRET,
        Vec::new(),
        Algorithm::RS256,
        "issuer".to_string(),
        "audience".to_string(),
        3600,
        86400,
    )
    .is_err());
}

#[test]
fn keyring_debug_reports_shape_without_secret_material() {
    let manager = TokenManager::new_symmetric_keyring(
        "active".to_string(),
        ACTIVE_SECRET,
        vec![("retiring".to_string(), RETIRING_SECRET.to_string())],
        Algorithm::HS256,
        "issuer".to_string(),
        "audience".to_string(),
        3600,
        86400,
    )
    .expect("keyring 应构建成功");
    let debug = format!("{manager:?}");

    assert!(debug.contains("keyring"));
    assert!(debug.contains("verification_key_count: 2"));
    assert!(!debug.contains(ACTIVE_SECRET));
    assert!(!debug.contains(RETIRING_SECRET));
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
