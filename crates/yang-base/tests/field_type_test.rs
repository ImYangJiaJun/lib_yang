//! FieldType 字段类型单元测试
//!
//! 测试所有字段类型的验证逻辑，包括：
//! - 基本类型验证（String, Integer, BigInt, Float, Double, Boolean）
//! - 时间类型验证（Date, DateTime, Timestamp）
//! - 复杂类型验证（Json, Text）
//! - 枚举类型验证（Enum）
//! - 外键类型验证（ForeignKey）
//! - 边界条件测试
//! - 错误情况测试

use yang_base::error::BaseError;
use yang_base::table::FieldType;

// ==================== String 类型验证测试 ====================

#[test]
fn test_string_valid_empty() {
    let field_type = FieldType::String { max_length: 10 };
    let result = field_type.validate("name", &serde_json::json!(""));
    assert!(result.is_ok(), "空字符串应该通过验证");
}

#[test]
fn test_string_valid_normal() {
    let field_type = FieldType::String { max_length: 10 };
    let result = field_type.validate("name", &serde_json::json!("hello"));
    assert!(result.is_ok(), "正常字符串应该通过验证");
}

#[test]
fn test_string_valid_boundary_exact_length() {
    let field_type = FieldType::String { max_length: 10 };
    let result = field_type.validate("name", &serde_json::json!("1234567890"));
    assert!(result.is_ok(), "正好等于最大长度的字符串应该通过验证");
}

#[test]
fn test_string_valid_chinese_characters() {
    let field_type = FieldType::String { max_length: 10 };
    let result = field_type.validate("name", &serde_json::json!("你好世界"));
    assert!(result.is_ok(), "中文字符串应该通过验证");
}

#[test]
fn test_string_valid_mixed_characters() {
    let field_type = FieldType::String { max_length: 20 };
    let result = field_type.validate("name", &serde_json::json!("Hello世界123"));
    assert!(result.is_ok(), "混合字符串应该通过验证");
}

#[test]
fn test_string_invalid_too_long() {
    let field_type = FieldType::String { max_length: 10 };
    let result = field_type.validate("name", &serde_json::json!("this is too long"));
    assert!(result.is_err(), "超过最大长度的字符串应该验证失败");

    match result.unwrap_err() {
        BaseError::StringTooLong(field, len, max) => {
            assert_eq!(field, "name");
            assert_eq!(len, 16);
            assert_eq!(max, 10);
        }
        _ => panic!("期望 StringTooLong 错误"),
    }
}

#[test]
fn test_string_invalid_chinese_too_long() {
    let field_type = FieldType::String { max_length: 5 };
    let result = field_type.validate("name", &serde_json::json!("这是一个很长的中文字符串"));
    assert!(result.is_err(), "超过最大长度的中文字符串应该验证失败");

    match result.unwrap_err() {
        BaseError::StringTooLong(field, len, _max) => {
            assert_eq!(field, "name");
            assert_eq!(len, 12); // 12 个中文字符
        }
        _ => panic!("期望 StringTooLong 错误"),
    }
}

#[test]
fn test_string_invalid_type_number() {
    let field_type = FieldType::String { max_length: 10 };
    let result = field_type.validate("name", &serde_json::json!(123));
    assert!(result.is_err(), "数字类型应该验证失败");

    match result.unwrap_err() {
        BaseError::InvalidFieldType(field, msg) => {
            assert_eq!(field, "name");
            assert!(msg.contains("期望字符串类型"));
            assert!(msg.contains("number"));
        }
        _ => panic!("期望 InvalidFieldType 错误"),
    }
}

#[test]
fn test_string_invalid_type_boolean() {
    let field_type = FieldType::String { max_length: 10 };
    let result = field_type.validate("name", &serde_json::json!(true));
    assert!(result.is_err(), "布尔类型应该验证失败");

    match result.unwrap_err() {
        BaseError::InvalidFieldType(field, msg) => {
            assert_eq!(field, "name");
            assert!(msg.contains("期望字符串类型"));
            assert!(msg.contains("boolean"));
        }
        _ => panic!("期望 InvalidFieldType 错误"),
    }
}

#[test]
fn test_string_invalid_type_null() {
    let field_type = FieldType::String { max_length: 10 };
    let result = field_type.validate("name", &serde_json::json!(null));
    assert!(result.is_err(), "null 类型应该验证失败");

    match result.unwrap_err() {
        BaseError::InvalidFieldType(field, msg) => {
            assert_eq!(field, "name");
            assert!(msg.contains("期望字符串类型"));
            assert!(msg.contains("null"));
        }
        _ => panic!("期望 InvalidFieldType 错误"),
    }
}

#[test]
fn test_string_invalid_type_array() {
    let field_type = FieldType::String { max_length: 10 };
    let result = field_type.validate("name", &serde_json::json!([1, 2, 3]));
    assert!(result.is_err(), "数组类型应该验证失败");

    match result.unwrap_err() {
        BaseError::InvalidFieldType(field, msg) => {
            assert_eq!(field, "name");
            assert!(msg.contains("期望字符串类型"));
            assert!(msg.contains("array"));
        }
        _ => panic!("期望 InvalidFieldType 错误"),
    }
}

#[test]
fn test_string_invalid_type_object() {
    let field_type = FieldType::String { max_length: 10 };
    let result = field_type.validate("name", &serde_json::json!({"key": "value"}));
    assert!(result.is_err(), "对象类型应该验证失败");

    match result.unwrap_err() {
        BaseError::InvalidFieldType(field, msg) => {
            assert_eq!(field, "name");
            assert!(msg.contains("期望字符串类型"));
            assert!(msg.contains("object"));
        }
        _ => panic!("期望 InvalidFieldType 错误"),
    }
}

// ==================== Integer 类型验证测试 ====================

#[test]
fn test_integer_valid_positive() {
    let field_type = FieldType::Integer;
    let result = field_type.validate("age", &serde_json::json!(25));
    assert!(result.is_ok(), "正整数应该通过验证");
}

#[test]
fn test_integer_valid_negative() {
    let field_type = FieldType::Integer;
    let result = field_type.validate("age", &serde_json::json!(-100));
    assert!(result.is_ok(), "负整数应该通过验证");
}

#[test]
fn test_integer_valid_zero() {
    let field_type = FieldType::Integer;
    let result = field_type.validate("age", &serde_json::json!(0));
    assert!(result.is_ok(), "零应该通过验证");
}

#[test]
fn test_integer_valid_boundary_max() {
    let field_type = FieldType::Integer;
    let result = field_type.validate("age", &serde_json::json!(i32::MAX));
    assert!(result.is_ok(), "i32 最大值应该通过验证");
}

#[test]
fn test_integer_valid_boundary_min() {
    let field_type = FieldType::Integer;
    let result = field_type.validate("age", &serde_json::json!(i32::MIN));
    assert!(result.is_ok(), "i32 最小值应该通过验证");
}

#[test]
fn test_integer_invalid_out_of_range_max() {
    let field_type = FieldType::Integer;
    let result = field_type.validate("age", &serde_json::json!(i64::MAX));
    assert!(result.is_err(), "超出 i32 范围的值应该验证失败");

    match result.unwrap_err() {
        BaseError::InvalidFieldType(field, msg) => {
            assert_eq!(field, "age");
            assert!(msg.contains("超出 i32 范围"));
        }
        _ => panic!("期望 InvalidFieldType 错误"),
    }
}

#[test]
fn test_integer_invalid_out_of_range_min() {
    let field_type = FieldType::Integer;
    let result = field_type.validate("age", &serde_json::json!(i64::MIN));
    assert!(result.is_err(), "超出 i32 范围的值应该验证失败");

    match result.unwrap_err() {
        BaseError::InvalidFieldType(field, msg) => {
            assert_eq!(field, "age");
            assert!(msg.contains("超出 i32 范围"));
        }
        _ => panic!("期望 InvalidFieldType 错误"),
    }
}

#[test]
fn test_integer_invalid_type_string() {
    let field_type = FieldType::Integer;
    let result = field_type.validate("age", &serde_json::json!("not a number"));
    assert!(result.is_err(), "字符串类型应该验证失败");

    match result.unwrap_err() {
        BaseError::InvalidFieldType(field, msg) => {
            assert_eq!(field, "age");
            assert!(msg.contains("期望整数类型"));
            assert!(msg.contains("string"));
        }
        _ => panic!("期望 InvalidFieldType 错误"),
    }
}

#[test]
fn test_integer_invalid_type_float() {
    let field_type = FieldType::Integer;
    let result = field_type.validate("age", &serde_json::json!(3.5));
    assert!(result.is_err(), "浮点数类型应该验证失败");

    match result.unwrap_err() {
        BaseError::InvalidFieldType(field, msg) => {
            assert_eq!(field, "age");
            assert!(msg.contains("期望整数类型"));
        }
        _ => panic!("期望 InvalidFieldType 错误"),
    }
}

#[test]
fn test_integer_invalid_type_boolean() {
    let field_type = FieldType::Integer;
    let result = field_type.validate("age", &serde_json::json!(true));
    assert!(result.is_err(), "布尔类型应该验证失败");
}

// ==================== BigInt 类型验证测试 ====================

#[test]
fn test_bigint_valid_positive() {
    let field_type = FieldType::BigInt;
    let result = field_type.validate("id", &serde_json::json!(123456789));
    assert!(result.is_ok(), "正长整数应该通过验证");
}

#[test]
fn test_bigint_valid_negative() {
    let field_type = FieldType::BigInt;
    let result = field_type.validate("id", &serde_json::json!(-123456789));
    assert!(result.is_ok(), "负长整数应该通过验证");
}

#[test]
fn test_bigint_valid_boundary_max() {
    let field_type = FieldType::BigInt;
    let result = field_type.validate("id", &serde_json::json!(i64::MAX));
    assert!(result.is_ok(), "i64 最大值应该通过验证");
}

#[test]
fn test_bigint_valid_boundary_min() {
    let field_type = FieldType::BigInt;
    let result = field_type.validate("id", &serde_json::json!(i64::MIN));
    assert!(result.is_ok(), "i64 最小值应该通过验证");
}

#[test]
fn test_bigint_invalid_type_string() {
    let field_type = FieldType::BigInt;
    let result = field_type.validate("id", &serde_json::json!("not a number"));
    assert!(result.is_err(), "字符串类型应该验证失败");

    match result.unwrap_err() {
        BaseError::InvalidFieldType(field, msg) => {
            assert_eq!(field, "id");
            assert!(msg.contains("期望长整数类型"));
        }
        _ => panic!("期望 InvalidFieldType 错误"),
    }
}

#[test]
fn test_bigint_invalid_type_float() {
    let field_type = FieldType::BigInt;
    let result = field_type.validate("id", &serde_json::json!(3.5));
    assert!(result.is_err(), "浮点数类型应该验证失败");
}

// ==================== Float 类型验证测试 ====================

#[test]
fn test_float_valid_positive() {
    let field_type = FieldType::Float;
    let result = field_type.validate("price", &serde_json::json!(3.5));
    assert!(result.is_ok(), "正浮点数应该通过验证");
}

#[test]
fn test_float_valid_negative() {
    let field_type = FieldType::Float;
    let result = field_type.validate("price", &serde_json::json!(-1.5));
    assert!(result.is_ok(), "负浮点数应该通过验证");
}

#[test]
fn test_float_valid_zero() {
    let field_type = FieldType::Float;
    let result = field_type.validate("price", &serde_json::json!(0.0));
    assert!(result.is_ok(), "零应该通过验证");
}

#[test]
fn test_float_valid_integer_as_float() {
    let field_type = FieldType::Float;
    let result = field_type.validate("price", &serde_json::json!(100));
    assert!(result.is_ok(), "整数应该可以转换为浮点数");
}

#[test]
fn test_float_valid_small_decimal() {
    let field_type = FieldType::Float;
    let result = field_type.validate("price", &serde_json::json!(0.0001));
    assert!(result.is_ok(), "小数应该通过验证");
}

#[test]
fn test_float_invalid_type_string() {
    let field_type = FieldType::Float;
    let result = field_type.validate("price", &serde_json::json!("not a number"));
    assert!(result.is_err(), "字符串类型应该验证失败");

    match result.unwrap_err() {
        BaseError::InvalidFieldType(field, msg) => {
            assert_eq!(field, "price");
            assert!(msg.contains("期望浮点数类型"));
        }
        _ => panic!("期望 InvalidFieldType 错误"),
    }
}

#[test]
fn test_float_invalid_type_boolean() {
    let field_type = FieldType::Float;
    let result = field_type.validate("price", &serde_json::json!(true));
    assert!(result.is_err(), "布尔类型应该验证失败");
}

// ==================== Double 类型验证测试 ====================

#[test]
fn test_double_valid_positive() {
    let field_type = FieldType::Double;
    let result = field_type.validate("amount", &serde_json::json!(123.456789));
    assert!(result.is_ok(), "正双精度浮点数应该通过验证");
}

#[test]
fn test_double_valid_negative() {
    let field_type = FieldType::Double;
    let result = field_type.validate("amount", &serde_json::json!(-987.654321));
    assert!(result.is_ok(), "负双精度浮点数应该通过验证");
}

#[test]
fn test_double_valid_zero() {
    let field_type = FieldType::Double;
    let result = field_type.validate("amount", &serde_json::json!(0.0));
    assert!(result.is_ok(), "零应该通过验证");
}

#[test]
fn test_double_valid_integer_as_double() {
    let field_type = FieldType::Double;
    let result = field_type.validate("amount", &serde_json::json!(100));
    assert!(result.is_ok(), "整数应该可以转换为双精度浮点数");
}

#[test]
fn test_double_valid_very_large() {
    let field_type = FieldType::Double;
    let result = field_type.validate("amount", &serde_json::json!(1.7976931348623157e308));
    assert!(result.is_ok(), "非常大的数应该通过验证");
}

#[test]
fn test_double_valid_very_small() {
    let field_type = FieldType::Double;
    let result = field_type.validate("amount", &serde_json::json!(2.2250738585072014e-308));
    assert!(result.is_ok(), "非常小的数应该通过验证");
}

#[test]
fn test_double_invalid_type_string() {
    let field_type = FieldType::Double;
    let result = field_type.validate("amount", &serde_json::json!("not a number"));
    assert!(result.is_err(), "字符串类型应该验证失败");

    match result.unwrap_err() {
        BaseError::InvalidFieldType(field, msg) => {
            assert_eq!(field, "amount");
            assert!(msg.contains("期望双精度浮点数类型"));
        }
        _ => panic!("期望 InvalidFieldType 错误"),
    }
}

#[test]
fn test_double_invalid_type_boolean() {
    let field_type = FieldType::Double;
    let result = field_type.validate("amount", &serde_json::json!(true));
    assert!(result.is_err(), "布尔类型应该验证失败");
}

// ==================== Boolean 类型验证测试 ====================

#[test]
fn test_boolean_valid_true() {
    let field_type = FieldType::Boolean;
    let result = field_type.validate("active", &serde_json::json!(true));
    assert!(result.is_ok(), "true 应该通过验证");
}

#[test]
fn test_boolean_valid_false() {
    let field_type = FieldType::Boolean;
    let result = field_type.validate("active", &serde_json::json!(false));
    assert!(result.is_ok(), "false 应该通过验证");
}

#[test]
fn test_boolean_invalid_type_string_true() {
    let field_type = FieldType::Boolean;
    let result = field_type.validate("active", &serde_json::json!("true"));
    assert!(result.is_err(), "字符串 'true' 应该验证失败");

    match result.unwrap_err() {
        BaseError::InvalidFieldType(field, msg) => {
            assert_eq!(field, "active");
            assert!(msg.contains("期望布尔类型"));
        }
        _ => panic!("期望 InvalidFieldType 错误"),
    }
}

#[test]
fn test_boolean_invalid_type_string_false() {
    let field_type = FieldType::Boolean;
    let result = field_type.validate("active", &serde_json::json!("false"));
    assert!(result.is_err(), "字符串 'false' 应该验证失败");
}

#[test]
fn test_boolean_invalid_type_number_one() {
    let field_type = FieldType::Boolean;
    let result = field_type.validate("active", &serde_json::json!(1));
    assert!(result.is_err(), "数字 1 应该验证失败");
}

#[test]
fn test_boolean_invalid_type_number_zero() {
    let field_type = FieldType::Boolean;
    let result = field_type.validate("active", &serde_json::json!(0));
    assert!(result.is_err(), "数字 0 应该验证失败");
}

#[test]
fn test_boolean_invalid_type_null() {
    let field_type = FieldType::Boolean;
    let result = field_type.validate("active", &serde_json::json!(null));
    assert!(result.is_err(), "null 应该验证失败");
}

// ==================== Enum 类型验证测试 ====================

#[test]
fn test_enum_valid_first_value() {
    let field_type = FieldType::Enum {
        values: vec![
            "pending".to_string(),
            "approved".to_string(),
            "rejected".to_string(),
        ],
    };
    let result = field_type.validate("status", &serde_json::json!("pending"));
    assert!(result.is_ok(), "第一个枚举值应该通过验证");
}

#[test]
fn test_enum_valid_middle_value() {
    let field_type = FieldType::Enum {
        values: vec![
            "pending".to_string(),
            "approved".to_string(),
            "rejected".to_string(),
        ],
    };
    let result = field_type.validate("status", &serde_json::json!("approved"));
    assert!(result.is_ok(), "中间枚举值应该通过验证");
}

#[test]
fn test_enum_valid_last_value() {
    let field_type = FieldType::Enum {
        values: vec![
            "pending".to_string(),
            "approved".to_string(),
            "rejected".to_string(),
        ],
    };
    let result = field_type.validate("status", &serde_json::json!("rejected"));
    assert!(result.is_ok(), "最后一个枚举值应该通过验证");
}

#[test]
fn test_enum_valid_single_value() {
    let field_type = FieldType::Enum {
        values: vec!["active".to_string()],
    };
    let result = field_type.validate("status", &serde_json::json!("active"));
    assert!(result.is_ok(), "单个枚举值应该通过验证");
}

#[test]
fn test_enum_valid_chinese_values() {
    let field_type = FieldType::Enum {
        values: vec![
            "待审核".to_string(),
            "已通过".to_string(),
            "已拒绝".to_string(),
        ],
    };
    let result = field_type.validate("status", &serde_json::json!("待审核"));
    assert!(result.is_ok(), "中文枚举值应该通过验证");
}

#[test]
fn test_enum_invalid_value_not_in_list() {
    let field_type = FieldType::Enum {
        values: vec![
            "pending".to_string(),
            "approved".to_string(),
            "rejected".to_string(),
        ],
    };
    let result = field_type.validate("status", &serde_json::json!("invalid"));
    assert!(result.is_err(), "不在列表中的枚举值应该验证失败");

    match result.unwrap_err() {
        BaseError::InvalidEnumValue(field, value) => {
            assert_eq!(field, "status");
            assert_eq!(value, "invalid");
        }
        _ => panic!("期望 InvalidEnumValue 错误"),
    }
}

#[test]
fn test_enum_invalid_value_empty_string() {
    let field_type = FieldType::Enum {
        values: vec!["pending".to_string(), "approved".to_string()],
    };
    let result = field_type.validate("status", &serde_json::json!(""));
    assert!(result.is_err(), "空字符串应该验证失败");

    match result.unwrap_err() {
        BaseError::InvalidEnumValue(field, value) => {
            assert_eq!(field, "status");
            assert_eq!(value, "");
        }
        _ => panic!("期望 InvalidEnumValue 错误"),
    }
}

#[test]
fn test_enum_invalid_value_case_sensitive() {
    let field_type = FieldType::Enum {
        values: vec!["pending".to_string()],
    };
    let result = field_type.validate("status", &serde_json::json!("Pending"));
    assert!(result.is_err(), "枚举值应该区分大小写");
}

#[test]
fn test_enum_invalid_type_number() {
    let field_type = FieldType::Enum {
        values: vec!["1".to_string(), "2".to_string()],
    };
    let result = field_type.validate("status", &serde_json::json!(1));
    assert!(result.is_err(), "数字类型应该验证失败");

    match result.unwrap_err() {
        BaseError::InvalidFieldType(field, msg) => {
            assert_eq!(field, "status");
            assert!(msg.contains("期望字符串类型的枚举值"));
        }
        _ => panic!("期望 InvalidFieldType 错误"),
    }
}

#[test]
fn test_enum_invalid_type_boolean() {
    let field_type = FieldType::Enum {
        values: vec!["true".to_string(), "false".to_string()],
    };
    let result = field_type.validate("status", &serde_json::json!(true));
    assert!(result.is_err(), "布尔类型应该验证失败");
}

#[test]
fn test_enum_invalid_type_null() {
    let field_type = FieldType::Enum {
        values: vec!["pending".to_string()],
    };
    let result = field_type.validate("status", &serde_json::json!(null));
    assert!(result.is_err(), "null 应该验证失败");
}

// ==================== Json 类型验证测试 ====================

#[test]
fn test_json_valid_object() {
    let field_type = FieldType::Json;
    let result = field_type.validate("data", &serde_json::json!({"key": "value"}));
    assert!(result.is_ok(), "JSON 对象应该通过验证");
}

#[test]
fn test_json_valid_array() {
    let field_type = FieldType::Json;
    let result = field_type.validate("data", &serde_json::json!([1, 2, 3]));
    assert!(result.is_ok(), "JSON 数组应该通过验证");
}

#[test]
fn test_json_valid_nested_object() {
    let field_type = FieldType::Json;
    let result = field_type.validate(
        "data",
        &serde_json::json!({
            "user": {
                "name": "Alice",
                "age": 30,
                "tags": ["admin", "user"]
            }
        }),
    );
    assert!(result.is_ok(), "嵌套 JSON 对象应该通过验证");
}

#[test]
fn test_json_valid_empty_object() {
    let field_type = FieldType::Json;
    let result = field_type.validate("data", &serde_json::json!({}));
    assert!(result.is_ok(), "空 JSON 对象应该通过验证");
}

#[test]
fn test_json_valid_empty_array() {
    let field_type = FieldType::Json;
    let result = field_type.validate("data", &serde_json::json!([]));
    assert!(result.is_ok(), "空 JSON 数组应该通过验证");
}

#[test]
fn test_json_valid_string_object() {
    let field_type = FieldType::Json;
    let result = field_type.validate("data", &serde_json::json!("{\"key\": \"value\"}"));
    assert!(result.is_ok(), "JSON 字符串（对象）应该通过验证");
}

#[test]
fn test_json_valid_string_array() {
    let field_type = FieldType::Json;
    let result = field_type.validate("data", &serde_json::json!("[1, 2, 3]"));
    assert!(result.is_ok(), "JSON 字符串（数组）应该通过验证");
}

#[test]
fn test_json_invalid_string_not_json() {
    let field_type = FieldType::Json;
    let result = field_type.validate("data", &serde_json::json!("not a json"));
    assert!(result.is_err(), "非 JSON 格式的字符串应该验证失败");

    match result.unwrap_err() {
        BaseError::InvalidJsonFormat(field, _msg) => {
            assert_eq!(field, "data");
        }
        _ => panic!("期望 InvalidJsonFormat 错误"),
    }
}

#[test]
fn test_json_invalid_string_incomplete_json() {
    let field_type = FieldType::Json;
    let result = field_type.validate("data", &serde_json::json!("{\"key\": "));
    assert!(result.is_err(), "不完整的 JSON 字符串应该验证失败");

    match result.unwrap_err() {
        BaseError::InvalidJsonFormat(field, _msg) => {
            assert_eq!(field, "data");
        }
        _ => panic!("期望 InvalidJsonFormat 错误"),
    }
}

#[test]
fn test_json_invalid_type_number() {
    let field_type = FieldType::Json;
    let result = field_type.validate("data", &serde_json::json!(123));
    assert!(result.is_err(), "数字类型应该验证失败");

    match result.unwrap_err() {
        BaseError::InvalidFieldType(field, msg) => {
            assert_eq!(field, "data");
            assert!(msg.contains("期望 JSON 对象或数组"));
        }
        _ => panic!("期望 InvalidFieldType 错误"),
    }
}

#[test]
fn test_json_invalid_type_boolean() {
    let field_type = FieldType::Json;
    let result = field_type.validate("data", &serde_json::json!(true));
    assert!(result.is_err(), "布尔类型应该验证失败");
}

#[test]
fn test_json_invalid_type_null() {
    let field_type = FieldType::Json;
    let result = field_type.validate("data", &serde_json::json!(null));
    assert!(result.is_err(), "null 应该验证失败");
}
