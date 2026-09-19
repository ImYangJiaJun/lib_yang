//! 字段验证器
//!
//! 提供灵活的字段值验证机制，支持长度验证、数值范围验证、格式验证和自定义验证。

use crate::error::BaseError;
#[cfg(feature = "validator")]
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
#[cfg(feature = "validator")]
use std::collections::HashMap;
use std::sync::Arc;
#[cfg(feature = "validator")]
use std::sync::{Mutex, OnceLock};

/// 自定义验证函数类型
///
/// 接收字段名和字段值，返回验证结果
pub type ValidatorFn = Arc<dyn Fn(&str, &serde_json::Value) -> Result<(), BaseError> + Send + Sync>;

/// 默认动态正则缓存容量：正则条目数有界，避免进程级无界增长。
pub const DEFAULT_REGEX_CACHE_CAP: usize = 128;

/// 单条正则编译的内存硬上限（1 MiB），防止病态正则占满 regex crate 默认 10 MiB。
#[cfg(feature = "validator")]
const REGEX_SIZE_LIMIT: usize = 1 << 20;

/// 超长 pattern 只编译不入表，避免用极长字符串撑大缓存键集合。
#[cfg(feature = "validator")]
const MAX_PATTERN_LENGTH: usize = 512;

/// 严格邮箱格式常量 pattern。
#[cfg(feature = "validator")]
const EMAIL_PATTERN: &str = r"^[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}$";

/// 严格手机号格式（E.164）常量 pattern。
#[cfg(feature = "validator")]
const PHONE_PATTERN: &str = r"^\+?[1-9]\d{1,14}$";

/// 按 `REGEX_SIZE_LIMIT` 硬上限编译一条正则。
#[cfg(feature = "validator")]
fn compile_bounded(pattern: &str) -> Result<Regex, regex::Error> {
    RegexBuilder::new(pattern)
        .size_limit(REGEX_SIZE_LIMIT)
        .build()
}

/// 有界正则缓存的共享可变状态。
#[cfg(feature = "validator")]
#[derive(Debug)]
struct RegexCacheInner {
    /// 邮箱常量正则（首次使用惰性编译，编译失败返回错误而非 panic）。
    email: OnceLock<Arc<Regex>>,
    /// 手机号常量正则（同上）。
    phone: OnceLock<Arc<Regex>>,
    /// 动态 pattern → 已编译正则，容量以 `RegexCache::cap` 为上限。
    dynamic: Mutex<HashMap<String, Arc<Regex>>>,
}

/// 有界正则缓存：由 [`crate::tools::Tools`] 拥有并随应用生命周期冻结。
///
/// 取代历史进程级 `EMAIL_REGEX`/`PHONE_REGEX`/`REGEX_CACHE` 三个 static：资源所有权
/// 收口到 `Tools`，动态条目数有上限（[`Self::cap`]），单条正则有编译内存硬上限。
///
/// 未启用 `validator` feature 时退化为仅含 `cap` 的空壳，调用链签名在两种 feature
/// 组合下保持一致。
#[derive(Debug, Clone)]
pub struct RegexCache {
    #[cfg(feature = "validator")]
    inner: Arc<RegexCacheInner>,
    cap: usize,
}

impl RegexCache {
    /// 创建容量为 `cap` 的缓存。
    ///
    /// 常量正则（邮箱/手机号）在首次使用时惰性编译；编译失败返回
    /// [`BaseError::ConfigError`] 而非 panic。
    pub fn new(cap: usize) -> Self {
        Self {
            #[cfg(feature = "validator")]
            inner: Arc::new(RegexCacheInner {
                email: OnceLock::new(),
                phone: OnceLock::new(),
                dynamic: Mutex::new(HashMap::new()),
            }),
            cap,
        }
    }

    /// 动态 pattern 缓存容量上限。
    pub fn cap(&self) -> usize {
        self.cap
    }

    /// 当前已缓存的动态 pattern 条数（测试与观测用）。
    pub fn len(&self) -> usize {
        #[cfg(feature = "validator")]
        {
            self.inner
                .dynamic
                .lock()
                .map(|guard| guard.len())
                .unwrap_or(0)
        }
        #[cfg(not(feature = "validator"))]
        {
            0
        }
    }

    /// 动态缓存是否为空。
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 邮箱常量正则（惰性编译，失败返回错误）。
    #[cfg(feature = "validator")]
    fn email(&self) -> Result<Arc<Regex>, BaseError> {
        if let Some(compiled) = self.inner.email.get() {
            return Ok(Arc::clone(compiled));
        }
        let compiled =
            Arc::new(compile_bounded(EMAIL_PATTERN).map_err(|error| {
                BaseError::ConfigError(format!("邮箱正则表达式编译失败: {error}"))
            })?);
        // 并发竞争下后到者被丢弃，返回的 Arc 与已存实例语义等价。
        let _ = self.inner.email.set(Arc::clone(&compiled));
        Ok(compiled)
    }

    /// 手机号常量正则（惰性编译，失败返回错误）。
    #[cfg(feature = "validator")]
    fn phone(&self) -> Result<Arc<Regex>, BaseError> {
        if let Some(compiled) = self.inner.phone.get() {
            return Ok(Arc::clone(compiled));
        }
        let compiled = Arc::new(compile_bounded(PHONE_PATTERN).map_err(|error| {
            BaseError::ConfigError(format!("手机号正则表达式编译失败: {error}"))
        })?);
        // 并发竞争下后到者被丢弃，返回的 Arc 与已存实例语义等价。
        let _ = self.inner.phone.set(Arc::clone(&compiled));
        Ok(compiled)
    }

    /// 获取（或编译并缓存）动态 pattern 的共享正则。
    ///
    /// 命中：锁内克隆 `Arc` 后立即释放锁，再在外侧匹配，缩短持锁时间；
    /// 未命中：编译，仅当 `len() < cap` 且 pattern 长度未超阈值时插入，否则直接用
    /// 刚编译的 `Arc` 不入表。
    #[cfg(feature = "validator")]
    pub(crate) fn dynamic(&self, pattern: &str) -> Result<Arc<Regex>, BaseError> {
        if let Some(compiled) = self
            .inner
            .dynamic
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(pattern)
            .cloned()
        {
            return Ok(compiled);
        }
        let compiled = Arc::new(
            compile_bounded(pattern)
                .map_err(|error| BaseError::ConfigError(format!("正则表达式无效: {error}")))?,
        );
        if pattern.len() <= MAX_PATTERN_LENGTH {
            let mut guard = self
                .inner
                .dynamic
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if guard.len() < self.cap {
                guard
                    .entry(pattern.to_string())
                    .or_insert_with(|| Arc::clone(&compiled));
            }
        }
        Ok(compiled)
    }
}

impl Default for RegexCache {
    fn default() -> Self {
        Self::new(DEFAULT_REGEX_CACHE_CAP)
    }
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
    /// URL 格式验证（仅校验协议前缀）
    ///
    /// **安全警告：** 此验证器仅检查字符串是否以 `http://` 或 `https://` 开头，
    /// 不解析主机名、不校验端口、不过滤内网/保留地址，**不提供 SSRF 防护**。
    ///
    /// 以下输入均能通过本验证器：
    /// - `http://169.254.169.254/latest/meta-data/` （云元数据端点）
    /// - `http://127.0.0.1:6379/` （本机 Redis）
    /// - `http://[::1]:8080/` （IPv6 loopback）
    /// - `http://localhost/` （本机服务）
    ///
    /// 若用于构造出站 HTTP 请求，必须在调用侧额外实施 SSRF 防护
    /// （如校验解析后的 IP 不属于保留/内网段）。
    Url,
    /// 正则表达式验证
    Regex(String),
    /// 自定义验证函数
    Custom(ValidatorFn),
}

/// 从 JSON 值提取数值：优先数字，其次十进制文本（Decimal 字段合法携带字符串）。
fn numeric_value(value: &serde_json::Value) -> Option<f64> {
    value.as_f64().or_else(|| {
        value
            .as_str()
            .and_then(|text| text.trim().parse::<f64>().ok())
    })
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

    /// 验证字段值是否符合验证规则（无缓存回退）。
    ///
    /// 保留此方法以兼容未持有 [`crate::tools::Tools`] 的公开调用方：Email/Phone/Regex
    /// 变体在此路径下一次性编译、不入缓存。运行期写路径请优先使用
    /// [`Validator::validate_with`] 传入共享 [`RegexCache`]。
    pub fn validate(&self, field_name: &str, value: &serde_json::Value) -> Result<(), BaseError> {
        self.validate_impl(field_name, value, None)
    }

    /// 使用共享正则缓存验证字段值。
    ///
    /// Email/Phone 走缓存内预编译常量正则；Regex 走有界动态缓存，避免重复编译与
    /// 进程级无界缓存。
    pub fn validate_with(
        &self,
        field_name: &str,
        value: &serde_json::Value,
        cache: &RegexCache,
    ) -> Result<(), BaseError> {
        self.validate_impl(field_name, value, Some(cache))
    }

    #[cfg_attr(not(feature = "validator"), allow(unused_variables))]
    fn validate_impl(
        &self,
        field_name: &str,
        value: &serde_json::Value,
        cache: Option<&RegexCache>,
    ) -> Result<(), BaseError> {
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
                let Some(num) = numeric_value(value) else {
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
                let Some(num) = numeric_value(value) else {
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
                    let matched = match cache {
                        Some(cache) => cache.email()?.is_match(s),
                        None => compile_bounded(EMAIL_PATTERN)
                            .map_err(|error| {
                                BaseError::ConfigError(format!("邮箱正则表达式编译失败: {error}"))
                            })?
                            .is_match(s),
                    };
                    if !matched {
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
                    let matched = match cache {
                        Some(cache) => cache.phone()?.is_match(s),
                        None => compile_bounded(PHONE_PATTERN)
                            .map_err(|error| {
                                BaseError::ConfigError(format!("手机号正则表达式编译失败: {error}"))
                            })?
                            .is_match(s),
                    };
                    if !matched {
                        return Err(BaseError::ValidationFailed(
                            field_name.to_string(),
                            "手机号格式无效，请使用 E.164 格式（如 +8613800138000 或 13800138000）"
                                .to_string(),
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
                    if !s
                        .chars()
                        .all(|c| c.is_ascii_digit() || c == '-' || c == '+')
                    {
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

            // URL 格式验证：仅校验 http:// / https:// 前缀，不解析 host，无 SSRF 防护
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
                    let compiled = match cache {
                        Some(cache) => cache.dynamic(pattern).map_err(|error| match error {
                            BaseError::ConfigError(message) => {
                                BaseError::ValidationFailed(field_name.to_string(), message)
                            }
                            other => other,
                        })?,
                        None => Arc::new(compile_bounded(pattern).map_err(|error| {
                            BaseError::ValidationFailed(
                                field_name.to_string(),
                                format!("正则表达式无效: {error}"),
                            )
                        })?),
                    };
                    if compiled.is_match(s) {
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
