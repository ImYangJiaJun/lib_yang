//! 字段类型定义
//!
//! 提供数据表字段的类型系统，支持基本类型、时间类型、复杂类型和关联类型。

use crate::error::BaseError;
use serde::{Deserialize, Serialize};

/// 获取 JSON 值的类型名称
///
/// # 参数
///
/// - `value`: JSON 值
///
/// # 返回值
///
/// 返回类型名称的字符串
fn value_type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

/// 字段类型
///
/// 定义数据表字段支持的所有类型，包括：
/// - 基本类型：String, Integer, BigInt, Float, Double, Boolean
/// - 时间类型：Date, DateTime, Timestamp
/// - 复杂类型：Json, Text
/// - 枚举类型：Enum
///
/// # 示例
///
/// ```rust
/// use yang_base::table::FieldType;
///
/// // 创建字符串类型，最大长度 50
/// let name_type = FieldType::String { max_length: 50 };
///
/// // 创建整数类型
/// let age_type = FieldType::Integer;
///
/// // 创建枚举类型
/// let status_type = FieldType::Enum {
///     values: vec!["active".to_string(), "inactive".to_string()],
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum FieldType {
    /// 字符串类型
    ///
    /// 用于存储文本数据，需要指定最大长度。
    ///
    /// # 字段
    ///
    /// - `max_length`: 字符串的最大长度（字符数）
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::FieldType;
    ///
    /// let field_type = FieldType::String { max_length: 100 };
    /// ```
    String {
        /// 最大长度（字符数）
        max_length: usize,
    },

    /// 整数类型
    ///
    /// 用于存储 32 位有符号整数（i32）。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::FieldType;
    ///
    /// let field_type = FieldType::Integer;
    /// ```
    Integer,

    /// 长整数类型
    ///
    /// 用于存储 64 位有符号整数（i64）。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::FieldType;
    ///
    /// let field_type = FieldType::BigInt;
    /// ```
    BigInt,

    /// 单精度浮点数类型
    ///
    /// 用于存储 32 位浮点数（f32）。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::FieldType;
    ///
    /// let field_type = FieldType::Float;
    /// ```
    Float,

    /// 双精度浮点数类型
    ///
    /// 用于存储 64 位浮点数（f64）。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::FieldType;
    ///
    /// let field_type = FieldType::Double;
    /// ```
    Double,

    /// 布尔类型
    ///
    /// 用于存储布尔值（true/false）。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::FieldType;
    ///
    /// let field_type = FieldType::Boolean;
    /// ```
    Boolean,

    /// 日期类型
    ///
    /// 用于存储日期（年-月-日），不包含时间信息。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::FieldType;
    ///
    /// let field_type = FieldType::Date;
    /// ```
    Date,

    /// 日期时间类型
    ///
    /// 用于存储日期和时间（年-月-日 时:分:秒）。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::FieldType;
    ///
    /// let field_type = FieldType::DateTime;
    /// ```
    DateTime,

    /// 时间戳类型
    ///
    /// 用于存储 Unix 时间戳（秒数）。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::FieldType;
    ///
    /// let field_type = FieldType::Timestamp;
    /// ```
    Timestamp,

    /// JSON 类型
    ///
    /// 用于存储 JSON 格式的复杂数据结构。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::FieldType;
    ///
    /// let field_type = FieldType::Json;
    /// ```
    Json,

    /// 文本类型
    ///
    /// 用于存储大文本数据，没有长度限制。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::FieldType;
    ///
    /// let field_type = FieldType::Text;
    /// ```
    Text,

    /// 枚举类型
    ///
    /// 用于存储预定义的可选值列表中的一个值。
    ///
    /// # 字段
    ///
    /// - `values`: 可选值列表
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::FieldType;
    ///
    /// let field_type = FieldType::Enum {
    ///     values: vec![
    ///         "pending".to_string(),
    ///         "approved".to_string(),
    ///         "rejected".to_string(),
    ///     ],
    /// };
    /// ```
    Enum {
        /// 可选值列表
        values: Vec<String>,
    },
}

impl FieldType {
    /// 获取字段类型的显示名称
    ///
    /// # 返回值
    ///
    /// 返回字段类型的中文显示名称
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::FieldType;
    ///
    /// let field_type = FieldType::String { max_length: 50 };
    /// assert_eq!(field_type.display_name(), "字符串");
    ///
    /// let field_type = FieldType::Integer;
    /// assert_eq!(field_type.display_name(), "整数");
    /// ```
    pub fn display_name(&self) -> &str {
        match self {
            FieldType::String { .. } => "字符串",
            FieldType::Integer => "整数",
            FieldType::BigInt => "长整数",
            FieldType::Float => "单精度浮点数",
            FieldType::Double => "双精度浮点数",
            FieldType::Boolean => "布尔值",
            FieldType::Date => "日期",
            FieldType::DateTime => "日期时间",
            FieldType::Timestamp => "时间戳",
            FieldType::Json => "JSON",
            FieldType::Text => "文本",
            FieldType::Enum { .. } => "枚举",
        }
    }

    /// 检查字段类型是否为数值类型
    ///
    /// # 返回值
    ///
    /// 如果是数值类型（Integer, BigInt, Float, Double）返回 true，否则返回 false
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::FieldType;
    ///
    /// assert!(FieldType::Integer.is_numeric());
    /// assert!(FieldType::Float.is_numeric());
    /// assert!(!FieldType::String { max_length: 50 }.is_numeric());
    /// ```
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            FieldType::Integer | FieldType::BigInt | FieldType::Float | FieldType::Double
        )
    }

    /// 检查字段类型是否为时间类型
    ///
    /// # 返回值
    ///
    /// 如果是时间类型（Date, DateTime, Timestamp）返回 true，否则返回 false
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::FieldType;
    ///
    /// assert!(FieldType::Date.is_temporal());
    /// assert!(FieldType::DateTime.is_temporal());
    /// assert!(!FieldType::String { max_length: 50 }.is_temporal());
    /// ```
    pub fn is_temporal(&self) -> bool {
        matches!(
            self,
            FieldType::Date | FieldType::DateTime | FieldType::Timestamp
        )
    }

    /// 检查字段类型是否为文本类型
    ///
    /// # 返回值
    ///
    /// 如果是文本类型（String, Text）返回 true，否则返回 false
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::FieldType;
    ///
    /// assert!(FieldType::String { max_length: 50 }.is_text());
    /// assert!(FieldType::Text.is_text());
    /// assert!(!FieldType::Integer.is_text());
    /// ```
    pub fn is_text(&self) -> bool {
        matches!(self, FieldType::String { .. } | FieldType::Text)
    }

    /// 验证字段值是否符合字段类型要求
    ///
    /// # 参数
    ///
    /// - `field_name`: 字段名称（用于错误消息）
    /// - `value`: 要验证的值
    ///
    /// # 返回值
    ///
    /// 如果验证通过返回 Ok(())，否则返回相应的错误
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::FieldType;
    /// use serde_json::json;
    ///
    /// let field_type = FieldType::String { max_length: 10 };
    /// assert!(field_type.validate("name", &json!("hello")).is_ok());
    /// assert!(field_type.validate("name", &json!("this is too long")).is_err());
    ///
    /// let field_type = FieldType::Integer;
    /// assert!(field_type.validate("age", &json!(25)).is_ok());
    /// assert!(field_type.validate("age", &json!("not a number")).is_err());
    /// ```
    pub fn validate(&self, field_name: &str, value: &serde_json::Value) -> Result<(), BaseError> {
        match self {
            // 字符串类型验证
            FieldType::String { max_length } => {
                if let Some(s) = value.as_str() {
                    let len = s.chars().count();
                    if len > *max_length {
                        return Err(BaseError::StringTooLong(
                            field_name.to_string(),
                            len,
                            *max_length,
                        ));
                    }
                    Ok(())
                } else {
                    Err(BaseError::InvalidFieldType(
                        field_name.to_string(),
                        format!("期望字符串类型，实际类型: {}", value_type_name(value)),
                    ))
                }
            }

            // 整数类型验证
            FieldType::Integer => {
                if value.is_i64() || value.is_u64() {
                    // 检查是否在 i32 范围内
                    if let Some(n) = value.as_i64() {
                        if n >= i32::MIN as i64 && n <= i32::MAX as i64 {
                            return Ok(());
                        }
                    }
                    Err(BaseError::InvalidFieldType(
                        field_name.to_string(),
                        "整数值超出 i32 范围".to_string(),
                    ))
                } else {
                    Err(BaseError::InvalidFieldType(
                        field_name.to_string(),
                        format!("期望整数类型，实际类型: {}", value_type_name(value)),
                    ))
                }
            }

            // 长整数类型验证
            FieldType::BigInt => {
                if value.as_i64().is_some() {
                    Ok(())
                } else {
                    Err(BaseError::InvalidFieldType(
                        field_name.to_string(),
                        format!("期望 i64 范围内的长整数，实际值: {}", value),
                    ))
                }
            }

            // 单精度浮点数类型验证
            FieldType::Float => {
                if value.is_f64() || value.is_i64() || value.is_u64() {
                    // 检查是否在 f32 范围内
                    if let Some(n) = value.as_f64() {
                        if n.is_finite() && n.abs() <= f32::MAX as f64 {
                            return Ok(());
                        }
                    }
                    Err(BaseError::InvalidFieldType(
                        field_name.to_string(),
                        "浮点数值超出 f32 范围".to_string(),
                    ))
                } else {
                    Err(BaseError::InvalidFieldType(
                        field_name.to_string(),
                        format!("期望浮点数类型，实际类型: {}", value_type_name(value)),
                    ))
                }
            }

            // 双精度浮点数类型验证
            FieldType::Double => {
                if value.is_f64() || value.is_i64() || value.is_u64() {
                    Ok(())
                } else {
                    Err(BaseError::InvalidFieldType(
                        field_name.to_string(),
                        format!("期望双精度浮点数类型，实际类型: {}", value_type_name(value)),
                    ))
                }
            }

            // 布尔类型验证
            FieldType::Boolean => {
                if value.is_boolean() {
                    Ok(())
                } else {
                    Err(BaseError::InvalidFieldType(
                        field_name.to_string(),
                        format!("期望布尔类型，实际类型: {}", value_type_name(value)),
                    ))
                }
            }

            // 日期类型验证
            FieldType::Date => validate_date(field_name, value),

            // 日期时间类型验证
            FieldType::DateTime => validate_datetime(field_name, value),

            // 时间戳类型验证
            FieldType::Timestamp => validate_timestamp(field_name, value),

            // 枚举类型验证
            FieldType::Enum { values } => {
                if let Some(s) = value.as_str() {
                    if values.contains(&s.to_string()) {
                        Ok(())
                    } else {
                        Err(BaseError::InvalidEnumValue(
                            field_name.to_string(),
                            s.to_string(),
                        ))
                    }
                } else {
                    Err(BaseError::InvalidFieldType(
                        field_name.to_string(),
                        format!(
                            "期望字符串类型的枚举值，实际类型: {}",
                            value_type_name(value)
                        ),
                    ))
                }
            }

            // JSON 类型验证
            FieldType::Json => {
                if value.is_object() || value.is_array() {
                    Ok(())
                } else {
                    Err(BaseError::InvalidFieldType(
                        field_name.to_string(),
                        format!("期望 JSON 对象或数组，实际类型: {}", value_type_name(value)),
                    ))
                }
            }

            // 文本类型验证：与 String 对等地要求字符串，但不施加长度上限
            FieldType::Text => {
                if value.as_str().is_some() {
                    Ok(())
                } else {
                    Err(BaseError::InvalidFieldType(
                        field_name.to_string(),
                        format!("期望文本字符串类型，实际类型: {}", value_type_name(value)),
                    ))
                }
            }
        }
    }
}

fn validate_date(field_name: &str, value: &serde_json::Value) -> Result<(), BaseError> {
    if let Some(s) = value.as_str() {
        chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map(|_| ())
            .map_err(|_| {
                BaseError::InvalidFieldType(
                    field_name.to_string(),
                    format!("日期格式无效，期望 YYYY-MM-DD，实际: {}", s),
                )
            })
    } else {
        Err(BaseError::InvalidFieldType(
            field_name.to_string(),
            format!("期望日期字符串类型，实际类型: {}", value_type_name(value)),
        ))
    }
}

fn validate_datetime(field_name: &str, value: &serde_json::Value) -> Result<(), BaseError> {
    if let Some(s) = value.as_str() {
        chrono::DateTime::parse_from_rfc3339(s)
            .map(|_| ())
            .map_err(|_| {
                BaseError::InvalidFieldType(
                    field_name.to_string(),
                    format!("日期时间格式无效，期望 RFC 3339，实际: {s}"),
                )
            })
    } else {
        Err(BaseError::InvalidFieldType(
            field_name.to_string(),
            format!(
                "期望日期时间字符串类型，实际类型: {}",
                value_type_name(value)
            ),
        ))
    }
}

fn validate_timestamp(field_name: &str, value: &serde_json::Value) -> Result<(), BaseError> {
    if let Some(ts) = value.as_i64() {
        chrono::DateTime::<chrono::Utc>::from_timestamp_secs(ts)
            .map(|_| ())
            .ok_or_else(|| {
                BaseError::InvalidFieldType(
                    field_name.to_string(),
                    format!("时间戳超出有效范围，实际: {}", ts),
                )
            })
    } else {
        Err(BaseError::InvalidFieldType(
            field_name.to_string(),
            format!("期望 Unix 时间戳整数，实际类型: {}", value_type_name(value)),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_type_display_name() {
        assert_eq!(
            FieldType::String { max_length: 50 }.display_name(),
            "字符串"
        );
        assert_eq!(FieldType::Integer.display_name(), "整数");
        assert_eq!(FieldType::BigInt.display_name(), "长整数");
        assert_eq!(FieldType::Float.display_name(), "单精度浮点数");
        assert_eq!(FieldType::Double.display_name(), "双精度浮点数");
        assert_eq!(FieldType::Boolean.display_name(), "布尔值");
        assert_eq!(FieldType::Date.display_name(), "日期");
        assert_eq!(FieldType::DateTime.display_name(), "日期时间");
        assert_eq!(FieldType::Timestamp.display_name(), "时间戳");
        assert_eq!(FieldType::Json.display_name(), "JSON");
        assert_eq!(FieldType::Text.display_name(), "文本");
        assert_eq!(
            FieldType::Enum {
                values: vec!["a".to_string()]
            }
            .display_name(),
            "枚举"
        );
    }

    #[test]
    fn test_is_numeric() {
        assert!(FieldType::Integer.is_numeric());
        assert!(FieldType::BigInt.is_numeric());
        assert!(FieldType::Float.is_numeric());
        assert!(FieldType::Double.is_numeric());
        assert!(!FieldType::String { max_length: 50 }.is_numeric());
        assert!(!FieldType::Boolean.is_numeric());
    }

    #[test]
    fn test_is_temporal() {
        assert!(FieldType::Date.is_temporal());
        assert!(FieldType::DateTime.is_temporal());
        assert!(FieldType::Timestamp.is_temporal());
        assert!(!FieldType::String { max_length: 50 }.is_temporal());
        assert!(!FieldType::Integer.is_temporal());
    }

    #[test]
    fn test_is_text() {
        assert!(FieldType::String { max_length: 50 }.is_text());
        assert!(FieldType::Text.is_text());
        assert!(!FieldType::Integer.is_text());
        assert!(!FieldType::Boolean.is_text());
    }

    #[test]
    fn test_field_type_equality() {
        let type1 = FieldType::String { max_length: 50 };
        let type2 = FieldType::String { max_length: 50 };
        let type3 = FieldType::String { max_length: 100 };

        assert_eq!(type1, type2);
        assert_ne!(type1, type3);
    }

    #[test]
    fn test_enum_type() {
        let enum_type = FieldType::Enum {
            values: vec![
                "pending".to_string(),
                "approved".to_string(),
                "rejected".to_string(),
            ],
        };

        assert_eq!(enum_type.display_name(), "枚举");
        assert!(!enum_type.is_numeric());
        assert!(!enum_type.is_temporal());
        assert!(!enum_type.is_text());
    }

    // ==================== validate 方法测试 ====================

    #[test]
    fn test_validate_string_success() {
        let field_type = FieldType::String { max_length: 10 };

        // 正常情况
        assert!(field_type
            .validate("name", &serde_json::json!("hello"))
            .is_ok());
        assert!(field_type.validate("name", &serde_json::json!("")).is_ok());

        // 边界情况：正好 10 个字符
        assert!(field_type
            .validate("name", &serde_json::json!("1234567890"))
            .is_ok());
    }

    #[test]
    fn test_validate_string_too_long() {
        let field_type = FieldType::String { max_length: 10 };

        // 超过最大长度
        let result = field_type.validate("name", &serde_json::json!("this is too long"));
        assert!(result.is_err());

        assert!(
            matches!(result, Err(BaseError::StringTooLong(ref field, 16, 10)) if field == "name"),
            "期望 StringTooLong 错误，实际: {:?}",
            result
        );
    }

    #[test]
    fn test_validate_string_invalid_type() {
        let field_type = FieldType::String { max_length: 10 };

        // 类型不匹配
        assert!(field_type
            .validate("name", &serde_json::json!(123))
            .is_err());
        assert!(field_type
            .validate("name", &serde_json::json!(true))
            .is_err());
        assert!(field_type
            .validate("name", &serde_json::json!(null))
            .is_err());
    }

    #[test]
    fn test_validate_integer_success() {
        let field_type = FieldType::Integer;

        // 正常情况
        assert!(field_type.validate("age", &serde_json::json!(25)).is_ok());
        assert!(field_type.validate("age", &serde_json::json!(0)).is_ok());
        assert!(field_type.validate("age", &serde_json::json!(-100)).is_ok());

        // 边界情况：i32 范围
        assert!(field_type
            .validate("age", &serde_json::json!(i32::MAX))
            .is_ok());
        assert!(field_type
            .validate("age", &serde_json::json!(i32::MIN))
            .is_ok());
    }

    #[test]
    fn test_validate_integer_out_of_range() {
        let field_type = FieldType::Integer;

        // 超出 i32 范围
        let result = field_type.validate("age", &serde_json::json!(i64::MAX));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_integer_invalid_type() {
        let field_type = FieldType::Integer;

        // 类型不匹配
        assert!(field_type
            .validate("age", &serde_json::json!("not a number"))
            .is_err());
        assert!(field_type
            .validate("age", &serde_json::json!(true))
            .is_err());
        assert!(field_type.validate("age", &serde_json::json!(3.5)).is_err());
    }

    #[test]
    fn test_validate_bigint_success() {
        let field_type = FieldType::BigInt;

        // 正常情况
        assert!(field_type
            .validate("id", &serde_json::json!(123456789))
            .is_ok());
        assert!(field_type
            .validate("id", &serde_json::json!(i64::MAX))
            .is_ok());
        assert!(field_type
            .validate("id", &serde_json::json!(i64::MIN))
            .is_ok());
    }

    #[test]
    fn test_validate_bigint_invalid_type() {
        let field_type = FieldType::BigInt;

        // 类型不匹配或超出 MySQL BIGINT 有符号范围
        assert!(field_type
            .validate("id", &serde_json::json!("not a number"))
            .is_err());
        assert!(field_type.validate("id", &serde_json::json!(3.5)).is_err());
        assert!(field_type
            .validate("id", &serde_json::json!(u64::MAX))
            .is_err());
    }

    #[test]
    fn test_validate_float_success() {
        let field_type = FieldType::Float;

        // 正常情况
        assert!(field_type
            .validate("price", &serde_json::json!(3.5))
            .is_ok());
        assert!(field_type
            .validate("price", &serde_json::json!(0.0))
            .is_ok());
        assert!(field_type
            .validate("price", &serde_json::json!(-1.5))
            .is_ok());

        // 整数也可以转换为浮点数
        assert!(field_type
            .validate("price", &serde_json::json!(100))
            .is_ok());
    }

    #[test]
    fn test_validate_float_invalid_type() {
        let field_type = FieldType::Float;

        // 类型不匹配
        assert!(field_type
            .validate("price", &serde_json::json!("not a number"))
            .is_err());
        assert!(field_type
            .validate("price", &serde_json::json!(true))
            .is_err());
    }

    #[test]
    fn test_validate_double_success() {
        let field_type = FieldType::Double;

        // 正常情况
        assert!(field_type
            .validate("amount", &serde_json::json!(123.456))
            .is_ok());
        assert!(field_type
            .validate("amount", &serde_json::json!(0.0))
            .is_ok());
        assert!(field_type
            .validate("amount", &serde_json::json!(100))
            .is_ok());
    }

    #[test]
    fn test_validate_double_invalid_type() {
        let field_type = FieldType::Double;

        // 类型不匹配
        assert!(field_type
            .validate("amount", &serde_json::json!("not a number"))
            .is_err());
        assert!(field_type
            .validate("amount", &serde_json::json!(true))
            .is_err());
    }

    #[test]
    fn test_validate_boolean_success() {
        let field_type = FieldType::Boolean;

        // 正常情况
        assert!(field_type
            .validate("active", &serde_json::json!(true))
            .is_ok());
        assert!(field_type
            .validate("active", &serde_json::json!(false))
            .is_ok());
    }

    #[test]
    fn test_validate_boolean_invalid_type() {
        let field_type = FieldType::Boolean;

        // 类型不匹配
        assert!(field_type
            .validate("active", &serde_json::json!("true"))
            .is_err());
        assert!(field_type
            .validate("active", &serde_json::json!(1))
            .is_err());
        assert!(field_type
            .validate("active", &serde_json::json!(0))
            .is_err());
    }

    #[test]
    fn test_validate_date_success() {
        let field_type = FieldType::Date;

        assert!(field_type
            .validate("birthday", &serde_json::json!("2026-05-27"))
            .is_ok());
    }

    #[test]
    fn test_validate_date_invalid_format() {
        let field_type = FieldType::Date;

        let result = field_type.validate("birthday", &serde_json::json!("2026/05/27"));
        assert!(result.is_err());
        assert!(
            matches!(result, Err(BaseError::InvalidFieldType(ref field, _)) if field == "birthday")
        );
    }

    #[test]
    fn test_validate_date_invalid_type() {
        let field_type = FieldType::Date;

        let result = field_type.validate("birthday", &serde_json::json!(20260527));
        assert!(result.is_err());
        assert!(
            matches!(result, Err(BaseError::InvalidFieldType(ref field, _)) if field == "birthday")
        );
    }

    #[test]
    fn test_validate_datetime_success() {
        let field_type = FieldType::DateTime;

        assert!(field_type
            .validate(
                "created_at",
                &serde_json::json!("2026-05-27T13:45:30+08:00"),
            )
            .is_ok());
        assert!(field_type
            .validate("created_at", &serde_json::json!("2026-05-27T05:45:30Z"))
            .is_ok());
    }

    #[test]
    fn test_validate_datetime_invalid_format() {
        let field_type = FieldType::DateTime;

        // 缺少 RFC 3339 时区信息的 naive datetime 必须拒绝。
        assert!(field_type
            .validate("created_at", &serde_json::json!("2026-05-27T13:45:30"))
            .is_err());
        assert!(field_type
            .validate("created_at", &serde_json::json!("2026-05-27 13:45:30"))
            .is_err());

        // 真正非法的格式（斜杠分隔）仍然返回错误
        let result = field_type.validate("created_at", &serde_json::json!("2026/05/27 13:45:30"));
        assert!(result.is_err());
        assert!(
            matches!(result, Err(BaseError::InvalidFieldType(ref field, _)) if field == "created_at")
        );
    }

    #[test]
    fn test_validate_datetime_matches_json_schema_date_time_format() {
        // JSON Schema 的 `date-time` format 使用 RFC 3339。
        let field_type = FieldType::DateTime;
        let value = serde_json::json!("2026-05-27T13:45:30.123+08:00");
        assert!(field_type.validate("created_at", &value).is_ok());
    }

    #[test]
    fn test_validate_timestamp_success() {
        let field_type = FieldType::Timestamp;

        assert!(field_type
            .validate("created_at", &serde_json::json!(1_764_221_130_i64))
            .is_ok());
    }

    #[test]
    fn test_validate_timestamp_invalid_type() {
        let field_type = FieldType::Timestamp;

        let result = field_type.validate("created_at", &serde_json::json!("2026-05-27 13:45:30"));
        assert!(result.is_err());
        assert!(
            matches!(result, Err(BaseError::InvalidFieldType(ref field, _)) if field == "created_at")
        );
    }

    #[test]
    fn test_validate_text_success() {
        let field_type = FieldType::Text;

        // 任意长度字符串均可（无长度上限）
        assert!(field_type
            .validate("content", &serde_json::json!("hello"))
            .is_ok());
        assert!(field_type
            .validate("content", &serde_json::json!("a".repeat(100_000)))
            .is_ok());
        assert!(field_type
            .validate("content", &serde_json::json!(""))
            .is_ok());
    }

    #[test]
    fn test_validate_text_invalid_type() {
        let field_type = FieldType::Text;

        // 非字符串返回 InvalidFieldType
        let result = field_type.validate("content", &serde_json::json!(123));
        assert!(
            matches!(result, Err(BaseError::InvalidFieldType(ref field, _)) if field == "content")
        );
        assert!(field_type
            .validate("content", &serde_json::json!(true))
            .is_err());
        assert!(field_type
            .validate("content", &serde_json::json!(null))
            .is_err());
    }

    #[test]
    fn test_validate_enum_success() {
        let field_type = FieldType::Enum {
            values: vec![
                "pending".to_string(),
                "approved".to_string(),
                "rejected".to_string(),
            ],
        };

        // 正常情况
        assert!(field_type
            .validate("status", &serde_json::json!("pending"))
            .is_ok());
        assert!(field_type
            .validate("status", &serde_json::json!("approved"))
            .is_ok());
        assert!(field_type
            .validate("status", &serde_json::json!("rejected"))
            .is_ok());
    }

    #[test]
    fn test_validate_enum_invalid_value() {
        let field_type = FieldType::Enum {
            values: vec![
                "pending".to_string(),
                "approved".to_string(),
                "rejected".to_string(),
            ],
        };

        // 枚举值不在列表中
        let result = field_type.validate("status", &serde_json::json!("invalid"));
        assert!(result.is_err());

        assert!(
            matches!(result, Err(BaseError::InvalidEnumValue(ref field, ref value)) if field == "status" && value == "invalid"),
            "期望 InvalidEnumValue 错误，实际: {:?}",
            result
        );
    }

    #[test]
    fn test_validate_enum_invalid_type() {
        let field_type = FieldType::Enum {
            values: vec!["pending".to_string()],
        };

        // 类型不匹配
        assert!(field_type
            .validate("status", &serde_json::json!(123))
            .is_err());
        assert!(field_type
            .validate("status", &serde_json::json!(true))
            .is_err());
    }

    #[test]
    fn test_validate_json_success() {
        let field_type = FieldType::Json;

        // JSON 对象
        assert!(field_type
            .validate("data", &serde_json::json!({"key": "value"}))
            .is_ok());

        // JSON 数组
        assert!(field_type
            .validate("data", &serde_json::json!([1, 2, 3]))
            .is_ok());
    }

    #[test]
    fn test_validate_json_invalid_format() {
        let field_type = FieldType::Json;

        // JSON 字段只接受结构化对象/数组；即使字符串内容能解析为 JSON 也拒绝。
        let result = field_type.validate("data", &serde_json::json!("not a json"));
        assert!(result.is_err());

        assert!(field_type
            .validate("data", &serde_json::json!("{\"key\": \"value\"}"))
            .is_err());
        assert!(field_type
            .validate("data", &serde_json::json!("[1, 2, 3]"))
            .is_err());

        assert!(
            matches!(result, Err(BaseError::InvalidFieldType(ref field, _)) if field == "data"),
            "期望 InvalidFieldType 错误，实际: {:?}",
            result
        );
    }

    #[test]
    fn test_validate_json_invalid_type() {
        let field_type = FieldType::Json;

        // 类型不匹配（不是对象、数组或字符串）
        assert!(field_type
            .validate("data", &serde_json::json!(123))
            .is_err());
        assert!(field_type
            .validate("data", &serde_json::json!(true))
            .is_err());
        assert!(field_type
            .validate("data", &serde_json::json!(null))
            .is_err());
    }
}
