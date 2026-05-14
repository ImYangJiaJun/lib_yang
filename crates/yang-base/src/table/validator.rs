//! 字段验证器
//!
//! 提供灵活的字段值验证机制，支持长度验证、数值范围验证、格式验证和自定义验证。

use crate::error::BaseError;
#[cfg(feature = "validator")]
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 自定义验证函数类型
///
/// 接收字段名和字段值，返回验证结果
pub type ValidatorFn = Arc<dyn Fn(&str, &serde_json::Value) -> Result<(), BaseError> + Send + Sync>;

/// 字段验证器
///
/// 定义各种字段值验证规则，包括：
/// - 长度验证：MinLength, MaxLength
/// - 数值范围验证：Min, Max
/// - 格式验证：Email, Phone, Url
/// - 正则表达式验证：Regex
/// - 自定义验证：Custom
///
/// # 示例
///
/// ```rust
/// use yang_base::table::Validator;
/// use serde_json::json;
///
/// // 最小长度验证
/// let validator = Validator::MinLength(5);
/// assert!(validator.validate("username", &json!("alice")).is_ok());
/// assert!(validator.validate("username", &json!("bob")).is_err());
///
/// // 邮箱格式验证
/// let validator = Validator::Email;
/// assert!(validator.validate("email", &json!("user@example.com")).is_ok());
/// assert!(validator.validate("email", &json!("invalid")).is_err());
///
/// // 数值范围验证
/// let validator = Validator::Min(0.0);
/// assert!(validator.validate("age", &json!(18)).is_ok());
/// assert!(validator.validate("age", &json!(-5)).is_err());
/// ```
#[derive(Clone)]
pub enum Validator {
    /// 最小长度验证
    ///
    /// 验证字符串的字符数不小于指定值。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::Validator;
    /// use serde_json::json;
    ///
    /// let validator = Validator::MinLength(3);
    /// assert!(validator.validate("name", &json!("abc")).is_ok());
    /// assert!(validator.validate("name", &json!("ab")).is_err());
    /// ```
    MinLength(usize),

    /// 最大长度验证
    ///
    /// 验证字符串的字符数不大于指定值。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::Validator;
    /// use serde_json::json;
    ///
    /// let validator = Validator::MaxLength(10);
    /// assert!(validator.validate("name", &json!("short")).is_ok());
    /// assert!(validator.validate("name", &json!("this is too long")).is_err());
    /// ```
    MaxLength(usize),

    /// 最小值验证
    ///
    /// 验证数值不小于指定值。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::Validator;
    /// use serde_json::json;
    ///
    /// let validator = Validator::Min(0.0);
    /// assert!(validator.validate("age", &json!(18)).is_ok());
    /// assert!(validator.validate("age", &json!(-5)).is_err());
    /// ```
    Min(f64),

    /// 最大值验证
    ///
    /// 验证数值不大于指定值。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::Validator;
    /// use serde_json::json;
    ///
    /// let validator = Validator::Max(100.0);
    /// assert!(validator.validate("score", &json!(95)).is_ok());
    /// assert!(validator.validate("score", &json!(150)).is_err());
    /// ```
    Max(f64),

    /// 邮箱格式验证
    ///
    /// 验证字符串是否为有效的邮箱格式（包含 @ 符号）。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::Validator;
    /// use serde_json::json;
    ///
    /// let validator = Validator::Email;
    /// assert!(validator.validate("email", &json!("user@example.com")).is_ok());
    /// assert!(validator.validate("email", &json!("invalid")).is_err());
    /// ```
    Email,

    /// 手机号格式验证
    ///
    /// 验证字符串是否为有效的手机号格式（仅包含数字和连字符）。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::Validator;
    /// use serde_json::json;
    ///
    /// let validator = Validator::Phone;
    /// assert!(validator.validate("phone", &json!("13800138000")).is_ok());
    /// assert!(validator.validate("phone", &json!("138-0013-8000")).is_ok());
    /// assert!(validator.validate("phone", &json!("invalid")).is_err());
    /// ```
    Phone,

    /// URL 格式验证
    ///
    /// 验证字符串是否为有效的 URL 格式（以 http:// 或 https:// 开头）。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::Validator;
    /// use serde_json::json;
    ///
    /// let validator = Validator::Url;
    /// assert!(validator.validate("website", &json!("https://example.com")).is_ok());
    /// assert!(validator.validate("website", &json!("http://example.com")).is_ok());
    /// assert!(validator.validate("website", &json!("invalid")).is_err());
    /// ```
    Url,

    /// 正则表达式验证
    ///
    /// 使用正则表达式验证字符串格式。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::Validator;
    /// use serde_json::json;
    ///
    /// let validator = Validator::Regex(r"^\d{6}$".to_string());
    /// assert!(validator.validate("code", &json!("123456")).is_ok());
    /// assert!(validator.validate("code", &json!("12345")).is_err());
    /// ```
    Regex(String),

    /// 自定义验证函数
    ///
    /// 使用自定义函数进行验证，提供最大的灵活性。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::Validator;
    /// use yang_base::error::BaseError;
    /// use serde_json::json;
    /// use std::sync::Arc;
    ///
    /// let validator = Validator::Custom(Arc::new(|field_name, value| {
    ///     if let Some(s) = value.as_str() {
    ///         if s.contains("forbidden") {
    ///             return Err(BaseError::ValidationFailed(
    ///                 field_name.to_string(),
    ///                 "包含禁止的词汇".to_string(),
    ///             ));
    ///         }
    ///     }
    ///     Ok(())
    /// }));
    ///
    /// assert!(validator.validate("content", &json!("normal text")).is_ok());
    /// assert!(validator.validate("content", &json!("forbidden word")).is_err());
    /// ```
    Custom(ValidatorFn),
}

impl Validator {
    /// 获取验证器的显示名称
    ///
    /// # 返回值
    ///
    /// 返回验证器的中文显示名称
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::Validator;
    ///
    /// assert_eq!(Validator::MinLength(5).display_name(), "最小长度");
    /// assert_eq!(Validator::Email.display_name(), "邮箱格式");
    /// ```
    pub fn display_name(&self) -> &str {
        match self {
            Validator::MinLength(_) => "最小长度",
            Validator::MaxLength(_) => "最大长度",
            Validator::Min(_) => "最小值",
            Validator::Max(_) => "最大值",
            Validator::Email => "邮箱格式",
            Validator::Phone => "手机号格式",
            Validator::Url => "URL格式",
            Validator::Regex(_) => "正则表达式",
            Validator::Custom(_) => "自定义验证",
        }
    }

    /// 验证字段值是否符合验证规则
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
    /// use yang_base::table::Validator;
    /// use serde_json::json;
    ///
    /// let validator = Validator::MinLength(5);
    /// assert!(validator.validate("username", &json!("alice")).is_ok());
    /// assert!(validator.validate("username", &json!("bob")).is_err());
    /// ```
    pub fn validate(&self, field_name: &str, value: &serde_json::Value) -> Result<(), BaseError> {
        match self {
            // 最小长度验证
            Validator::MinLength(min_len) => {
                if let Some(s) = value.as_str() {
                    let len = s.chars().count();
                    if len < *min_len {
                        return Err(BaseError::ValidationFailed(
                            field_name.to_string(),
                            format!("字符串长度不能小于 {}，当前长度: {}", min_len, len),
                        ));
                    }
                    Ok(())
                } else {
                    Err(BaseError::ValidationFailed(
                        field_name.to_string(),
                        "MinLength 验证器只能用于字符串类型".to_string(),
                    ))
                }
            }

            // 最大长度验证
            Validator::MaxLength(max_len) => {
                if let Some(s) = value.as_str() {
                    let len = s.chars().count();
                    if len > *max_len {
                        return Err(BaseError::ValidationFailed(
                            field_name.to_string(),
                            format!("字符串长度不能大于 {}，当前长度: {}", max_len, len),
                        ));
                    }
                    Ok(())
                } else {
                    Err(BaseError::ValidationFailed(
                        field_name.to_string(),
                        "MaxLength 验证器只能用于字符串类型".to_string(),
                    ))
                }
            }

            // 最小值验证
            Validator::Min(min_val) => {
                let num = if let Some(n) = value.as_f64() {
                    n
                } else if let Some(n) = value.as_i64() {
                    n as f64
                } else if let Some(n) = value.as_u64() {
                    n as f64
                } else {
                    return Err(BaseError::ValidationFailed(
                        field_name.to_string(),
                        "Min 验证器只能用于数值类型".to_string(),
                    ));
                };

                if num < *min_val {
                    return Err(BaseError::ValidationFailed(
                        field_name.to_string(),
                        format!("数值不能小于 {}，当前值: {}", min_val, num),
                    ));
                }
                Ok(())
            }

            // 最大值验证
            Validator::Max(max_val) => {
                let num = if let Some(n) = value.as_f64() {
                    n
                } else if let Some(n) = value.as_i64() {
                    n as f64
                } else if let Some(n) = value.as_u64() {
                    n as f64
                } else {
                    return Err(BaseError::ValidationFailed(
                        field_name.to_string(),
                        "Max 验证器只能用于数值类型".to_string(),
                    ));
                };

                if num > *max_val {
                    return Err(BaseError::ValidationFailed(
                        field_name.to_string(),
                        format!("数值不能大于 {}，当前值: {}", max_val, num),
                    ));
                }
                Ok(())
            }

            // 邮箱格式验证
            Validator::Email => {
                if let Some(s) = value.as_str() {
                    if !s.contains('@') {
                        return Err(BaseError::ValidationFailed(
                            field_name.to_string(),
                            "邮箱格式无效，必须包含 @ 符号".to_string(),
                        ));
                    }
                    Ok(())
                } else {
                    Err(BaseError::ValidationFailed(
                        field_name.to_string(),
                        "Email 验证器只能用于字符串类型".to_string(),
                    ))
                }
            }

            // 手机号格式验证
            Validator::Phone => {
                if let Some(s) = value.as_str() {
                    // 检查是否只包含数字和连字符
                    if !s.chars().all(|c| c.is_ascii_digit() || c == '-') {
                        return Err(BaseError::ValidationFailed(
                            field_name.to_string(),
                            "手机号格式无效，只能包含数字和连字符".to_string(),
                        ));
                    }
                    Ok(())
                } else {
                    Err(BaseError::ValidationFailed(
                        field_name.to_string(),
                        "Phone 验证器只能用于字符串类型".to_string(),
                    ))
                }
            }

            // URL 格式验证
            Validator::Url => {
                if let Some(s) = value.as_str() {
                    if !s.starts_with("http://") && !s.starts_with("https://") {
                        return Err(BaseError::ValidationFailed(
                            field_name.to_string(),
                            "URL 格式无效，必须以 http:// 或 https:// 开头".to_string(),
                        ));
                    }
                    Ok(())
                } else {
                    Err(BaseError::ValidationFailed(
                        field_name.to_string(),
                        "Url 验证器只能用于字符串类型".to_string(),
                    ))
                }
            }

            // 正则表达式验证
            #[cfg(feature = "validator")]
            Validator::Regex(pattern) => {
                if let Some(s) = value.as_str() {
                    let re = Regex::new(pattern).map_err(|e| {
                        BaseError::ValidationFailed(
                            field_name.to_string(),
                            format!("正则表达式无效: {}", e),
                        )
                    })?;

                    if !re.is_match(s) {
                        return Err(BaseError::ValidationFailed(
                            field_name.to_string(),
                            format!("值不匹配正则表达式: {}", pattern),
                        ));
                    }
                    Ok(())
                } else {
                    Err(BaseError::ValidationFailed(
                        field_name.to_string(),
                        "Regex 验证器只能用于字符串类型".to_string(),
                    ))
                }
            }

            // 未启用 validator feature 时，正则验证不可用
            #[cfg(not(feature = "validator"))]
            Validator::Regex(_pattern) => {
                Err(BaseError::ValidationFailed(
                    field_name.to_string(),
                    "正则验证器需要启用 'validator' feature".to_string(),
                ))
            }

            // 自定义验证函数
            Validator::Custom(func) => func(field_name, value),
        }
    }
}

// 实现 Debug trait（Custom 变体需要特殊处理）
impl std::fmt::Debug for Validator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Validator::MinLength(len) => write!(f, "MinLength({})", len),
            Validator::MaxLength(len) => write!(f, "MaxLength({})", len),
            Validator::Min(val) => write!(f, "Min({})", val),
            Validator::Max(val) => write!(f, "Max({})", val),
            Validator::Email => write!(f, "Email"),
            Validator::Phone => write!(f, "Phone"),
            Validator::Url => write!(f, "Url"),
            Validator::Regex(pattern) => write!(f, "Regex(\"{}\")", pattern),
            Validator::Custom(_) => write!(f, "Custom(<function>)"),
        }
    }
}

// 为了支持序列化，我们需要一个辅助结构
// 注意：Custom 验证器无法序列化，会被跳过
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
enum ValidatorSerde {
    MinLength(usize),
    MaxLength(usize),
    Min(f64),
    Max(f64),
    Email,
    Phone,
    Url,
    Regex(String),
}

impl Serialize for Validator {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let serde_variant = match self {
            Validator::MinLength(len) => ValidatorSerde::MinLength(*len),
            Validator::MaxLength(len) => ValidatorSerde::MaxLength(*len),
            Validator::Min(val) => ValidatorSerde::Min(*val),
            Validator::Max(val) => ValidatorSerde::Max(*val),
            Validator::Email => ValidatorSerde::Email,
            Validator::Phone => ValidatorSerde::Phone,
            Validator::Url => ValidatorSerde::Url,
            Validator::Regex(pattern) => ValidatorSerde::Regex(pattern.clone()),
            Validator::Custom(_) => {
                // Custom 验证器无法序列化，返回错误
                return Err(serde::ser::Error::custom("Custom 验证器无法序列化"));
            }
        };
        serde_variant.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Validator {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let serde_variant = ValidatorSerde::deserialize(deserializer)?;
        Ok(match serde_variant {
            ValidatorSerde::MinLength(len) => Validator::MinLength(len),
            ValidatorSerde::MaxLength(len) => Validator::MaxLength(len),
            ValidatorSerde::Min(val) => Validator::Min(val),
            ValidatorSerde::Max(val) => Validator::Max(val),
            ValidatorSerde::Email => Validator::Email,
            ValidatorSerde::Phone => Validator::Phone,
            ValidatorSerde::Url => Validator::Url,
            ValidatorSerde::Regex(pattern) => Validator::Regex(pattern),
        })
    }
}
