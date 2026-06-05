//! 字段验证器
//!
//! 提供灵活的字段值验证机制，支持长度验证、数值范围验证、格式验证和自定义验证。

use crate::error::BaseError;
#[cfg(feature = "validator")]
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
#[cfg(feature = "validator")]
use std::collections::HashMap;
#[cfg(feature = "validator")]
use std::sync::{OnceLock, RwLock};

/// 自定义验证函数类型
///
/// 接收字段名和字段值，返回验证结果
pub type ValidatorFn = Arc<dyn Fn(&str, &serde_json::Value) -> Result<(), BaseError> + Send + Sync>;

/// 缓存的邮箱正则表达式（严格模式）
///
/// 使用 OnceLock 确保线程安全的延迟初始化
#[cfg(feature = "validator")]
static EMAIL_REGEX: OnceLock<Regex> = OnceLock::new();

/// 缓存的手机号正则表达式（E.164 格式）
#[cfg(feature = "validator")]
static PHONE_REGEX: OnceLock<Regex> = OnceLock::new();

/// 动态正则表达式缓存（用于 Validator::Regex 变体）
///
/// 使用 RwLock 支持并发读写，避免重复编译相同的正则表达式
#[cfg(feature = "validator")]
static REGEX_CACHE: OnceLock<RwLock<HashMap<String, Regex>>> = OnceLock::new();

/// 获取缓存的邮箱正则表达式引用
#[cfg(feature = "validator")]
fn email_regex() -> &'static Regex {
    EMAIL_REGEX.get_or_init(|| {
        // 严格邮箱格式：用户名@域名.顶级域名（至少2个字符）
        Regex::new(r"^[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}$")
            .expect("邮箱正则表达式编译失败")
    })
}

/// 获取缓存的手机号正则表达式引用
#[cfg(feature = "validator")]
fn phone_regex() -> &'static Regex {
    PHONE_REGEX.get_or_init(|| {
        // E.164 格式：可选的 + 号，第一位非零数字，总长度 2-15 位
        Regex::new(r"^\+?[1-9]\d{1,14}$")
            .expect("手机号正则表达式编译失败")
    })
}

/// 字段验证器
#[derive(Clone)]
pub enum Validator {
    /// 最小长度验证
    MinLength(usize),
    /// 最大长度验证
    MaxLength(usize),
    /// 最小值验证
    Min(f64),
    /// 最大值验证
    Max(f64),
    /// 邮箱格式验证（严格模式）
    Email,
    /// 邮箱格式验证（宽松模式，向后兼容）
    EmailLoose,
    /// 手机号格式验证（严格模式，E.164 格式）
    Phone,
    /// 手机号格式验证（宽松模式，向后兼容）
    PhoneLoose,
    /// URL 格式验证
    Url,
    /// 正则表达式验证
    Regex(String),
    /// 自定义验证函数
    Custom(ValidatorFn),
}

impl Validator {
    /// 获取验证器的显示名称
    pub fn display_name(&self) -> &str {
        match self {
            Validator::MinLength(_) => "最小长度",
            Validator::MaxLength(_) => "最大长度",
            Validator::Min(_) => "最小值",
            Validator::Max(_) => "最大值",
            Validator::Email => "邮箱格式（严格）",
            Validator::EmailLoose => "邮箱格式（宽松）",
            Validator::Phone => "手机号格式（严格）",
            Validator::PhoneLoose => "手机号格式（宽松）",
            Validator::Url => "URL格式",
            Validator::Regex(_) => "正则表达式",
            Validator::Custom(_) => "自定义验证",
        }
    }

    /// 验证字段值是否符合验证规则
    pub fn validate(&self, field_name: &str, value: &serde_json::Value) -> Result<(), BaseError> {
        match self {
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

            // 邮箱格式验证（严格模式）：使用正则表达式
            #[cfg(feature = "validator")]
            Validator::Email => {
                if let Some(s) = value.as_str() {
                    if !email_regex().is_match(s) {
                        return Err(BaseError::ValidationFailed(
                            field_name.to_string(),
                            "邮箱格式无效，请使用标准邮箱格式（如 user@example.com）".to_string(),
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

            // 未启用 validator feature 时，Email 降级为宽松模式
            #[cfg(not(feature = "validator"))]
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

            // 邮箱格式验证（宽松模式）：仅检查 @ 符号
            Validator::EmailLoose => {
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
                        "EmailLoose 验证器只能用于字符串类型".to_string(),
                    ))
                }
            }

            // 手机号格式验证（严格模式）：使用 E.164 正则表达式
            #[cfg(feature = "validator")]
            Validator::Phone => {
                if let Some(s) = value.as_str() {
                    if !phone_regex().is_match(s) {
                        return Err(BaseError::ValidationFailed(
                            field_name.to_string(),
                            "手机号格式无效，请使用 E.164 格式（如 +8613800138000 或 13800138000）".to_string(),
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

            // 未启用 validator feature 时，Phone 降级为宽松模式
            #[cfg(not(feature = "validator"))]
            Validator::Phone => {
                if let Some(s) = value.as_str() {
                    if !s.chars().all(|c| c.is_ascii_digit() || c == '-' || c == '+') {
                        return Err(BaseError::ValidationFailed(
                            field_name.to_string(),
                            "手机号格式无效，只能包含数字、连字符和加号".to_string(),
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

            // 手机号格式验证（宽松模式）：仅检查数字和连字符
            Validator::PhoneLoose => {
                if let Some(s) = value.as_str() {
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
                        "PhoneLoose 验证器只能用于字符串类型".to_string(),
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

            // 正则表达式验证（使用缓存，编译错误使用字段名作为上下文）
            #[cfg(feature = "validator")]
            Validator::Regex(pattern) => {
                if let Some(s) = value.as_str() {
                    let cache = REGEX_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
                    // 快路径：单把读锁内命中即直接匹配返回，避免重复编译与额外加锁
                    {
                        let read_guard = cache.read().unwrap_or_else(|p| p.into_inner());
                        if let Some(re) = read_guard.get(pattern) {
                            return if re.is_match(s) {
                                Ok(())
                            } else {
                                Err(BaseError::ValidationFailed(
                                    field_name.to_string(),
                                    format!("值不匹配正则表达式: {}", pattern),
                                ))
                            };
                        }
                    }
                    // 未命中：取写锁，编译并缓存后立即用 &mut Regex 匹配
                    let re = Regex::new(pattern).map_err(|e| {
                        BaseError::ValidationFailed(
                            field_name.to_string(),
                            format!("正则表达式无效: {}", e),
                        )
                    })?;
                    let mut write_guard = cache.write().unwrap_or_else(|p| p.into_inner());
                    let cached = write_guard.entry(pattern.to_string()).or_insert_with(|| re);
                    if cached.is_match(s) {
                        Ok(())
                    } else {
                        Err(BaseError::ValidationFailed(
                            field_name.to_string(),
                            format!("值不匹配正则表达式: {}", pattern),
                        ))
                    }
                } else {
                    Err(BaseError::ValidationFailed(
                        field_name.to_string(),
                        "Regex 验证器只能用于字符串类型".to_string(),
                    ))
                }
            }

            // 未启用 validator feature 时，正则验证不可用
            #[cfg(not(feature = "validator"))]
            Validator::Regex(_pattern) => Err(BaseError::ValidationFailed(
                field_name.to_string(),
                "正则验证器需要启用 'validator' feature".to_string(),
            )),

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
            Validator::EmailLoose => write!(f, "EmailLoose"),
            Validator::Phone => write!(f, "Phone"),
            Validator::PhoneLoose => write!(f, "PhoneLoose"),
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
    EmailLoose,
    Phone,
    PhoneLoose,
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
            Validator::EmailLoose => ValidatorSerde::EmailLoose,
            Validator::Phone => ValidatorSerde::Phone,
            Validator::PhoneLoose => ValidatorSerde::PhoneLoose,
            Validator::Url => ValidatorSerde::Url,
            Validator::Regex(pattern) => ValidatorSerde::Regex(pattern.clone()),
            Validator::Custom(_) => {
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
            ValidatorSerde::EmailLoose => Validator::EmailLoose,
            ValidatorSerde::Phone => Validator::Phone,
            ValidatorSerde::PhoneLoose => Validator::PhoneLoose,
            ValidatorSerde::Url => Validator::Url,
            ValidatorSerde::Regex(pattern) => Validator::Regex(pattern),
        })
    }
}