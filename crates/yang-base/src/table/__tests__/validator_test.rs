//! Validator 验证器单元测试

use crate::error::BaseError;
use crate::table::Validator;
use serde_json::json;
use std::sync::Arc;

// ==================== 显示名称测试 ====================

#[test]
fn test_validator_display_name() {
    assert_eq!(Validator::MinLength(5).display_name(), "最小长度");
    assert_eq!(Validator::MaxLength(10).display_name(), "最大长度");
    assert_eq!(Validator::Min(0.0).display_name(), "最小值");
    assert_eq!(Validator::Max(100.0).display_name(), "最大值");
    assert_eq!(Validator::Email.display_name(), "邮箱格式（严格）");
    assert_eq!(Validator::Phone.display_name(), "手机号格式（严格）");
    assert_eq!(Validator::Url.display_name(), "URL格式");
    assert_eq!(
        Validator::Regex(r"^\d+$".to_string()).display_name(),
        "正则表达式"
    );

    let custom_validator = Validator::Custom(Arc::new(|_, _| Ok(())));
    assert_eq!(custom_validator.display_name(), "自定义验证");
}

// ==================== MinLength 验证器测试 ====================

#[test]
fn test_min_length_success() {
    let validator = Validator::MinLength(5);

    // 正常情况
    assert!(validator.validate("username", &json!("alice")).is_ok());
    assert!(validator.validate("username", &json!("hello")).is_ok());

    // 边界情况：正好 5 个字符
    assert!(validator.validate("username", &json!("12345")).is_ok());

    // 超过最小长度
    assert!(validator
        .validate("username", &json!("very long username"))
        .is_ok());
}

#[test]
fn test_min_length_too_short() {
    let validator = Validator::MinLength(5);

    // 少于最小长度
    let result = validator.validate("username", &json!("bob"));
    assert!(result.is_err());

    if let Err(BaseError::ValidationFailed(field, msg)) = result {
        assert_eq!(field, "username");
        assert!(msg.contains("字符串长度不能小于 5"));
        assert!(msg.contains("当前长度: 3"));
    } else {
        panic!("期望 ValidationFailed 错误");
    }
}

#[test]
fn test_min_length_empty_string() {
    let validator = Validator::MinLength(1);

    // 空字符串
    let result = validator.validate("username", &json!(""));
    assert!(result.is_err());
}

#[test]
fn test_min_length_invalid_type() {
    let validator = Validator::MinLength(5);

    // 类型不匹配
    assert!(validator.validate("username", &json!(123)).is_err());
    assert!(validator.validate("username", &json!(true)).is_err());
    assert!(validator.validate("username", &json!(null)).is_err());
}

// ==================== MaxLength 验证器测试 ====================

#[test]
fn test_max_length_success() {
    let validator = Validator::MaxLength(10);

    // 正常情况
    assert!(validator.validate("name", &json!("short")).is_ok());
    assert!(validator.validate("name", &json!("")).is_ok());

    // 边界情况：正好 10 个字符
    assert!(validator.validate("name", &json!("1234567890")).is_ok());
}

#[test]
fn test_max_length_too_long() {
    let validator = Validator::MaxLength(10);

    // 超过最大长度
    let result = validator.validate("name", &json!("this is too long"));
    assert!(result.is_err());

    if let Err(BaseError::ValidationFailed(field, msg)) = result {
        assert_eq!(field, "name");
        assert!(msg.contains("字符串长度不能大于 10"));
        assert!(msg.contains("当前长度: 16"));
    } else {
        panic!("期望 ValidationFailed 错误");
    }
}

#[test]
fn test_max_length_invalid_type() {
    let validator = Validator::MaxLength(10);

    // 类型不匹配
    assert!(validator.validate("name", &json!(123)).is_err());
    assert!(validator.validate("name", &json!(true)).is_err());
}

// ==================== Min 验证器测试 ====================

#[test]
fn test_min_value_success() {
    let validator = Validator::Min(0.0);

    // 正常情况
    assert!(validator.validate("age", &json!(18)).is_ok());
    assert!(validator.validate("age", &json!(100)).is_ok());

    // 边界情况：正好等于最小值
    assert!(validator.validate("age", &json!(0)).is_ok());
    assert!(validator.validate("age", &json!(0.0)).is_ok());

    // 浮点数
    assert!(validator.validate("price", &json!(9.99)).is_ok());
}

#[test]
fn test_min_value_too_small() {
    let validator = Validator::Min(0.0);

    // 小于最小值
    let result = validator.validate("age", &json!(-5));
    assert!(result.is_err());

    if let Err(BaseError::ValidationFailed(field, msg)) = result {
        assert_eq!(field, "age");
        assert!(msg.contains("数值不能小于 0"));
        assert!(msg.contains("当前值: -5"));
    } else {
        panic!("期望 ValidationFailed 错误");
    }
}

#[test]
fn test_min_value_invalid_type() {
    let validator = Validator::Min(0.0);

    // 类型不匹配
    assert!(validator.validate("age", &json!("not a number")).is_err());
    assert!(validator.validate("age", &json!(true)).is_err());
}

// ==================== Max 验证器测试 ====================

#[test]
fn test_max_value_success() {
    let validator = Validator::Max(100.0);

    // 正常情况
    assert!(validator.validate("score", &json!(95)).is_ok());
    assert!(validator.validate("score", &json!(0)).is_ok());

    // 边界情况：正好等于最大值
    assert!(validator.validate("score", &json!(100)).is_ok());
    assert!(validator.validate("score", &json!(100.0)).is_ok());

    // 负数
    assert!(validator.validate("score", &json!(-50)).is_ok());
}

#[test]
fn test_max_value_too_large() {
    let validator = Validator::Max(100.0);

    // 大于最大值
    let result = validator.validate("score", &json!(150));
    assert!(result.is_err());

    if let Err(BaseError::ValidationFailed(field, msg)) = result {
        assert_eq!(field, "score");
        assert!(msg.contains("数值不能大于 100"));
        assert!(msg.contains("当前值: 150"));
    } else {
        panic!("期望 ValidationFailed 错误");
    }
}

#[test]
fn test_max_value_invalid_type() {
    let validator = Validator::Max(100.0);

    // 类型不匹配
    assert!(validator.validate("score", &json!("not a number")).is_err());
    assert!(validator.validate("score", &json!(true)).is_err());
}

// ==================== Email 验证器测试 ====================

#[test]
fn test_email_success() {
    let validator = Validator::Email;

    // 正常情况（严格模式）
    assert!(validator
        .validate("email", &json!("user@example.com"))
        .is_ok());
    assert!(validator.validate("email", &json!("test@test.org")).is_ok());
    // 顶级域名至少2个字符
    assert!(validator.validate("email", &json!("a@b.cn")).is_ok());
}

#[test]
fn test_email_invalid_format() {
    let validator = Validator::Email;

    // 缺少 @ 符号
    let result = validator.validate("email", &json!("invalid"));
    assert!(result.is_err());

    if let Err(BaseError::ValidationFailed(field, msg)) = result {
        assert_eq!(field, "email");
        assert!(msg.contains("邮箱格式无效"));
    } else {
        panic!("期望 ValidationFailed 错误");
    }

    // 空字符串
    assert!(validator.validate("email", &json!("")).is_err());
    // 缺少顶级域名
    assert!(validator.validate("email", &json!("user@example")).is_err());
}

#[test]
fn test_email_invalid_type() {
    let validator = Validator::Email;

    // 类型不匹配
    assert!(validator.validate("email", &json!(123)).is_err());
    assert!(validator.validate("email", &json!(true)).is_err());
}

// ==================== Phone 验证器测试 ====================

#[test]
fn test_phone_success() {
    let validator = Validator::Phone;

    // 正常情况（严格 E.164 模式）
    assert!(validator.validate("phone", &json!("13800138000")).is_ok());
    assert!(validator
        .validate("phone", &json!("+8613800138000"))
        .is_ok());
    assert!(validator.validate("phone", &json!("1234567890")).is_ok());
}

#[test]
fn test_phone_invalid_format() {
    let validator = Validator::Phone;

    // 包含字母
    let result = validator.validate("phone", &json!("138abc"));
    assert!(result.is_err());

    if let Err(BaseError::ValidationFailed(field, msg)) = result {
        assert_eq!(field, "phone");
        assert!(msg.contains("手机号格式无效"));
    } else {
        panic!("期望 ValidationFailed 错误");
    }

    // 包含空格
    assert!(validator.validate("phone", &json!("138 0013")).is_err());
    // 以 0 开头（E.164 要求第一位非零）
    assert!(validator.validate("phone", &json!("0138001380")).is_err());
}

#[test]
fn test_phone_invalid_type() {
    let validator = Validator::Phone;

    // 类型不匹配
    assert!(validator.validate("phone", &json!(123)).is_err());
    assert!(validator.validate("phone", &json!(true)).is_err());
}

// ==================== Url 验证器测试 ====================

#[test]
fn test_url_success() {
    let validator = Validator::Url;

    // 正常情况
    assert!(validator
        .validate("website", &json!("https://example.com"))
        .is_ok());
    assert!(validator
        .validate("website", &json!("http://example.com"))
        .is_ok());
    assert!(validator
        .validate("website", &json!("https://www.example.com/path?query=1"))
        .is_ok());
}

#[test]
fn test_url_invalid_format() {
    let validator = Validator::Url;

    // 缺少协议
    let result = validator.validate("website", &json!("example.com"));
    assert!(result.is_err());

    if let Err(BaseError::ValidationFailed(field, msg)) = result {
        assert_eq!(field, "website");
        assert!(msg.contains("URL 格式无效"));
        assert!(msg.contains("必须以 http:// 或 https:// 开头"));
    } else {
        panic!("期望 ValidationFailed 错误");
    }

    // 错误的协议
    assert!(validator
        .validate("website", &json!("ftp://example.com"))
        .is_err());
    assert!(validator.validate("website", &json!("invalid")).is_err());
}

#[test]
fn test_url_invalid_type() {
    let validator = Validator::Url;

    // 类型不匹配
    assert!(validator.validate("website", &json!(123)).is_err());
    assert!(validator.validate("website", &json!(true)).is_err());
}

// ==================== Regex 验证器测试 ====================

#[test]
fn test_regex_success() {
    let validator = Validator::Regex(r"^\d{6}$".to_string());

    // 正常情况：6 位数字
    assert!(validator.validate("code", &json!("123456")).is_ok());
    assert!(validator.validate("code", &json!("000000")).is_ok());
}

#[test]
fn test_regex_invalid_format() {
    let validator = Validator::Regex(r"^\d{6}$".to_string());

    // 不匹配正则表达式
    let result = validator.validate("code", &json!("12345"));
    assert!(result.is_err());

    if let Err(BaseError::ValidationFailed(field, msg)) = result {
        assert_eq!(field, "code");
        assert!(msg.contains("值不匹配正则表达式"));
    } else {
        panic!("期望 ValidationFailed 错误");
    }

    // 包含字母
    assert!(validator.validate("code", &json!("12345a")).is_err());

    // 长度不对
    assert!(validator.validate("code", &json!("1234567")).is_err());
}

#[test]
fn test_regex_invalid_pattern() {
    let validator = Validator::Regex(r"[invalid(".to_string());

    // 无效的正则表达式
    let result = validator.validate("code", &json!("test"));
    assert!(result.is_err());

    if let Err(BaseError::ValidationFailed(field, msg)) = result {
        assert_eq!(field, "code");
        // 错误消息应包含正则表达式相关描述
        assert!(msg.contains("正则表达式"));
    } else {
        panic!("期望 ValidationFailed 错误");
    }
}

#[test]
fn test_regex_invalid_type() {
    let validator = Validator::Regex(r"^\d+$".to_string());

    // 类型不匹配
    assert!(validator.validate("code", &json!(123)).is_err());
    assert!(validator.validate("code", &json!(true)).is_err());
}

// ==================== Custom 验证器测试 ====================

#[test]
fn test_custom_validator_success() {
    let validator = Validator::Custom(Arc::new(|_field_name, value| {
        if let Some(s) = value.as_str() {
            if s.len() >= 3 {
                return Ok(());
            }
        }
        Err(BaseError::ValidationFailed(
            _field_name.to_string(),
            "自定义验证失败：长度必须大于等于 3".to_string(),
        ))
    }));

    // 正常情况
    assert!(validator.validate("field", &json!("abc")).is_ok());
    assert!(validator.validate("field", &json!("hello")).is_ok());
}

#[test]
fn test_custom_validator_failure() {
    let validator = Validator::Custom(Arc::new(|field_name, value| {
        if let Some(s) = value.as_str() {
            if s.contains("forbidden") {
                return Err(BaseError::ValidationFailed(
                    field_name.to_string(),
                    "包含禁止的词汇".to_string(),
                ));
            }
        }
        Ok(())
    }));

    // 验证失败
    let result = validator.validate("content", &json!("forbidden word"));
    assert!(result.is_err());

    if let Err(BaseError::ValidationFailed(field, msg)) = result {
        assert_eq!(field, "content");
        assert_eq!(msg, "包含禁止的词汇");
    } else {
        panic!("期望 ValidationFailed 错误");
    }

    // 验证成功
    assert!(validator.validate("content", &json!("normal text")).is_ok());
}

// ==================== 序列化测试 ====================

#[test]
fn test_validator_serialization() {
    // MinLength
    let validator = Validator::MinLength(5);
    let json = serde_json::to_string(&validator).unwrap();
    let deserialized: Validator = serde_json::from_str(&json).unwrap();
    assert!(matches!(deserialized, Validator::MinLength(5)));

    // MaxLength
    let validator = Validator::MaxLength(10);
    let json = serde_json::to_string(&validator).unwrap();
    let deserialized: Validator = serde_json::from_str(&json).unwrap();
    assert!(matches!(deserialized, Validator::MaxLength(10)));

    // Min
    let validator = Validator::Min(0.0);
    let json = serde_json::to_string(&validator).unwrap();
    let deserialized: Validator = serde_json::from_str(&json).unwrap();
    assert!(matches!(deserialized, Validator::Min(v) if (v - 0.0).abs() < f64::EPSILON));

    // Max
    let validator = Validator::Max(100.0);
    let json = serde_json::to_string(&validator).unwrap();
    let deserialized: Validator = serde_json::from_str(&json).unwrap();
    assert!(matches!(deserialized, Validator::Max(v) if (v - 100.0).abs() < f64::EPSILON));

    // Email
    let validator = Validator::Email;
    let json = serde_json::to_string(&validator).unwrap();
    let deserialized: Validator = serde_json::from_str(&json).unwrap();
    assert!(matches!(deserialized, Validator::Email));

    // Phone
    let validator = Validator::Phone;
    let json = serde_json::to_string(&validator).unwrap();
    let deserialized: Validator = serde_json::from_str(&json).unwrap();
    assert!(matches!(deserialized, Validator::Phone));

    // Url
    let validator = Validator::Url;
    let json = serde_json::to_string(&validator).unwrap();
    let deserialized: Validator = serde_json::from_str(&json).unwrap();
    assert!(matches!(deserialized, Validator::Url));

    // Regex
    let validator = Validator::Regex(r"^\d+$".to_string());
    let json = serde_json::to_string(&validator).unwrap();
    let deserialized: Validator = serde_json::from_str(&json).unwrap();
    if let Validator::Regex(pattern) = deserialized {
        assert_eq!(pattern, r"^\d+$");
    } else {
        panic!("期望 Regex 验证器");
    }
}

#[test]
fn test_custom_validator_serialization_fails() {
    let validator = Validator::Custom(Arc::new(|_, _| Ok(())));

    // Custom 验证器无法序列化
    let result = serde_json::to_string(&validator);
    assert!(result.is_err());
}

// ==================== Debug trait 测试 ====================

#[test]
fn test_validator_debug() {
    assert_eq!(format!("{:?}", Validator::MinLength(5)), "MinLength(5)");
    assert_eq!(format!("{:?}", Validator::MaxLength(10)), "MaxLength(10)");
    assert_eq!(format!("{:?}", Validator::Min(0.0)), "Min(0)");
    assert_eq!(format!("{:?}", Validator::Max(100.0)), "Max(100)");
    assert_eq!(format!("{:?}", Validator::Email), "Email");
    assert_eq!(format!("{:?}", Validator::Phone), "Phone");
    assert_eq!(format!("{:?}", Validator::Url), "Url");
    assert_eq!(
        format!("{:?}", Validator::Regex(r"^\d+$".to_string())),
        "Regex(\"^\\d+$\")"
    );

    let custom_validator = Validator::Custom(Arc::new(|_, _| Ok(())));
    assert_eq!(format!("{:?}", custom_validator), "Custom(<function>)");
}
