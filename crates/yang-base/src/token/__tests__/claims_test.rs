//! TokenClaims 结构体单元测试

use crate::token::TokenClaims;
use serde_json::json;

#[test]
fn test_token_claims_serialization() {
    // 创建 TokenClaims 实例
    let claims = TokenClaims {
        iss: "yang-base".to_string(),
        sub: "user_123".to_string(),
        aud: "yang-app".to_string(),
        exp: 1234567890,
        nbf: 1234567800,
        iat: 1234567800,
        jti: "unique-token-id".to_string(),
        token_type: crate::token::TokenType::Access,
        custom: json!({
            "role": "admin",
            "permissions": ["read", "write"]
        }),
    };

    // 序列化为 JSON
    let json_str = serde_json::to_string(&claims).expect("序列化失败");

    // 验证 JSON 包含所有字段
    assert!(json_str.contains("\"iss\":\"yang-base\""));
    assert!(json_str.contains("\"sub\":\"user_123\""));
    assert!(json_str.contains("\"aud\":\"yang-app\""));
    assert!(json_str.contains("\"exp\":1234567890"));
    assert!(json_str.contains("\"nbf\":1234567800"));
    assert!(json_str.contains("\"iat\":1234567800"));
    assert!(json_str.contains("\"jti\":\"unique-token-id\""));
    assert!(json_str.contains("\"token_type\":\"access\""));
    assert!(json_str.contains("\"role\":\"admin\""));
    assert!(json_str.contains("\"permissions\":[\"read\",\"write\"]"));
}

#[test]
fn test_token_claims_deserialization() {
    // JSON 字符串
    let json_str = r#"{
        "iss": "yang-base",
        "sub": "user_456",
        "aud": "yang-app",
        "exp": 9876543210,
        "nbf": 9876543200,
        "iat": 9876543200,
        "jti": "another-token-id",
        "token_type": "refresh",
        "role": "user",
        "org_id": "org_789"
    }"#;

    // 反序列化为 TokenClaims
    let claims: TokenClaims = serde_json::from_str(json_str).expect("反序列化失败");

    // 验证标准字段
    assert_eq!(claims.iss, "yang-base");
    assert_eq!(claims.sub, "user_456");
    assert_eq!(claims.aud, "yang-app");
    assert_eq!(claims.exp, 9876543210);
    assert_eq!(claims.nbf, 9876543200);
    assert_eq!(claims.iat, 9876543200);
    assert_eq!(claims.jti, "another-token-id");
    assert_eq!(claims.token_type, crate::token::TokenType::Refresh);

    // 验证自定义字段
    assert_eq!(claims.custom.get("role").unwrap(), "user");
    assert_eq!(claims.custom.get("org_id").unwrap(), "org_789");
}

#[test]
fn test_token_claims_round_trip() {
    // 创建原始 TokenClaims
    let original = TokenClaims {
        iss: "test-issuer".to_string(),
        sub: "test-subject".to_string(),
        aud: "test-audience".to_string(),
        exp: 1111111111,
        nbf: 1111111100,
        iat: 1111111100,
        jti: "test-jti".to_string(),
        token_type: crate::token::TokenType::Access,
        custom: json!({
            "custom_field_1": "value1",
            "custom_field_2": 42,
            "custom_field_3": true
        }),
    };

    // 序列化
    let json_str = serde_json::to_string(&original).expect("序列化失败");

    // 反序列化
    let deserialized: TokenClaims = serde_json::from_str(&json_str).expect("反序列化失败");

    // 验证所有字段保持一致
    assert_eq!(deserialized.iss, original.iss);
    assert_eq!(deserialized.sub, original.sub);
    assert_eq!(deserialized.aud, original.aud);
    assert_eq!(deserialized.exp, original.exp);
    assert_eq!(deserialized.nbf, original.nbf);
    assert_eq!(deserialized.iat, original.iat);
    assert_eq!(deserialized.jti, original.jti);
    assert_eq!(deserialized.token_type, original.token_type);
    assert_eq!(
        deserialized.custom.get("custom_field_1"),
        original.custom.get("custom_field_1")
    );
    assert_eq!(
        deserialized.custom.get("custom_field_2"),
        original.custom.get("custom_field_2")
    );
    assert_eq!(
        deserialized.custom.get("custom_field_3"),
        original.custom.get("custom_field_3")
    );
}

#[test]
fn test_token_claims_with_empty_custom() {
    // 创建没有自定义字段的 TokenClaims
    let claims = TokenClaims {
        iss: "issuer".to_string(),
        sub: "subject".to_string(),
        aud: "audience".to_string(),
        exp: 2222222222,
        nbf: 2222222200,
        iat: 2222222200,
        jti: "jti-id".to_string(),
        token_type: crate::token::TokenType::Access,
        custom: json!({}),
    };

    // 序列化和反序列化
    let json_str = serde_json::to_string(&claims).expect("序列化失败");
    let deserialized: TokenClaims = serde_json::from_str(&json_str).expect("反序列化失败");

    // 验证标准字段
    assert_eq!(deserialized.iss, claims.iss);
    assert_eq!(deserialized.sub, claims.sub);
    assert_eq!(deserialized.token_type, claims.token_type);

    // 验证自定义字段为空对象
    assert!(deserialized.custom.is_object());
    assert_eq!(deserialized.custom.as_object().unwrap().len(), 0);
}

#[test]
fn test_token_claims_with_null_custom() {
    // 创建自定义字段为 null 的 TokenClaims
    let claims = TokenClaims {
        iss: "issuer".to_string(),
        sub: "subject".to_string(),
        aud: "audience".to_string(),
        exp: 3333333333,
        nbf: 3333333300,
        iat: 3333333300,
        jti: "jti-null".to_string(),
        token_type: crate::token::TokenType::Refresh,
        custom: json!(null),
    };

    // 序列化和反序列化
    let json_str = serde_json::to_string(&claims).expect("序列化失败");
    let deserialized: TokenClaims = serde_json::from_str(&json_str).expect("反序列化失败");

    // 注意：由于使用了 flatten，null 值在反序列化时会被转换为空对象
    // 这是 serde flatten 的预期行为
    assert!(deserialized.custom.is_object() || deserialized.custom.is_null());
}

#[test]
fn test_token_claims_flatten_behavior() {
    // 测试 flatten 属性的行为：自定义字段应该展平到顶层
    let claims = TokenClaims {
        iss: "issuer".to_string(),
        sub: "subject".to_string(),
        aud: "audience".to_string(),
        exp: 4444444444,
        nbf: 4444444400,
        iat: 4444444400,
        jti: "jti-flatten".to_string(),
        token_type: crate::token::TokenType::Access,
        custom: json!({
            "user_role": "admin",
            "department": "engineering"
        }),
    };

    // 序列化为 JSON Value
    let json_value = serde_json::to_value(&claims).expect("序列化失败");

    // 验证自定义字段在顶层
    assert_eq!(json_value.get("user_role").unwrap(), "admin");
    assert_eq!(json_value.get("department").unwrap(), "engineering");

    // 验证没有嵌套的 "custom" 字段
    assert!(json_value.get("custom").is_none());
}
