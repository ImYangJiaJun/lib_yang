//! 一次性验证码：摘要存储、独立限流与原子单次消费。
//!
//! 典型用途是注册邮箱验证，但机制本身与具体业务无关：
//!
//! - 验证码与邮箱均以 HMAC 摘要形式进入 Redis，不保存明文身份；
//! - 请求侧按来源 IP、目标身份与全局容量三维度限流，并有重发冷却；
//! - 校验侧用 Lua 脚本原子比对并单次消费，错误尝试有上限且用尽即销毁；
//! - 邮件投递经业务实现的 [`RegistrationEmailSender`] 注入（如 SMTP 适配器），
//!   投递失败会原子回收未消费的验证码，不留下可用凭证。
//!
//! TTL、验证码长度、密钥与限流阈值全部经 [`EmailVerificationConfig`] 注入，
//! 不读取任何应用配置。

use crate::action::ActionContext;
use crate::transport::client_ip::client_ip_identity;
use async_trait::async_trait;
use hmac::{Hmac, Mac};
use rand_core::{OsRng, RngCore};
use schemars::JsonSchema;
use serde::Serialize;
use sha2::Sha256;
use std::fmt;
use std::sync::Arc;
use yang_base::BaseError;

const RESERVE_SEND_SCRIPT: &str = r#"
local cooldown_ttl = redis.call('TTL', KEYS[4])
if cooldown_ttl > 0 then
    return {2, cooldown_ttl}
end

local retry_after = 0
local exceeded = 0
for index = 1, 3 do
    local current = redis.call('INCR', KEYS[index])
    if current == 1 then
        redis.call('EXPIRE', KEYS[index], ARGV[1])
    end
    local ttl = redis.call('TTL', KEYS[index])
    if ttl < 1 then
        redis.call('EXPIRE', KEYS[index], ARGV[1])
        ttl = tonumber(ARGV[1])
    end
    if current > tonumber(ARGV[index + 1]) then
        exceeded = 1
        if ttl > retry_after then
            retry_after = ttl
        end
    end
end
if exceeded == 1 then
    return {1, retry_after}
end

redis.call('SET', KEYS[4], '1', 'EX', ARGV[5], 'NX')
return {0, tonumber(ARGV[5])}
"#;

const VERIFY_AND_CONSUME_SCRIPT: &str = r#"
local value = redis.call('GET', KEYS[1])
if not value then
    return 0
end
local separator = string.find(value, ':', 1, true)
if not separator then
    redis.call('DEL', KEYS[1])
    return 0
end
local expected = string.sub(value, 1, separator - 1)
local attempts = tonumber(string.sub(value, separator + 1))
if not attempts then
    redis.call('DEL', KEYS[1])
    return 0
end
if expected == ARGV[1] then
    redis.call('DEL', KEYS[1])
    return 1
end
attempts = attempts + 1
if attempts >= tonumber(ARGV[2]) then
    redis.call('DEL', KEYS[1])
    return -2
end
redis.call('SET', KEYS[1], expected .. ':' .. attempts, 'KEEPTTL')
return -1
"#;

const DELETE_IF_CURRENT_SCRIPT: &str = r#"
local value = redis.call('GET', KEYS[1])
if value and string.sub(value, 1, string.len(ARGV[1]) + 1) == ARGV[1] .. ':' then
    return redis.call('DEL', KEYS[1])
end
return 0
"#;

/// 邮件投递失败的脱敏类别；不携带 SMTP 响应、收件人或凭据。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmailDeliveryError {
    /// 邮件内容或地址无法构造。
    InvalidMessage,
    /// 邮件服务未接受邮件或暂不可用。
    Unavailable,
}

impl fmt::Display for EmailDeliveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMessage => formatter.write_str("邮件内容无效"),
            Self::Unavailable => formatter.write_str("邮件服务暂不可用"),
        }
    }
}

impl std::error::Error for EmailDeliveryError {}

/// 验证码投递接口，由业务实现（SMTP、短信网关等）并注入。
///
/// 实现方不得记录 `recipient` 或 `code` 原文。
#[async_trait]
pub trait RegistrationEmailSender: Send + Sync + 'static {
    /// 投递一枚短期验证码。
    async fn send_registration_code(
        &self,
        recipient: &str,
        code: &str,
        expires_in_seconds: u64,
    ) -> Result<(), EmailDeliveryError>;
}

/// 可放入 `Tools` 扩展槽的类型擦除投递句柄。
#[derive(Clone)]
pub struct RegistrationEmailSenderHandle(Arc<dyn RegistrationEmailSender>);

impl RegistrationEmailSenderHandle {
    /// 用业务投递器创建句柄。
    pub fn new<T>(sender: T) -> Self
    where
        T: RegistrationEmailSender,
    {
        Self(Arc::new(sender))
    }

    /// 从已共享的投递器创建句柄。
    pub fn from_arc(sender: Arc<dyn RegistrationEmailSender>) -> Self {
        Self(sender)
    }

    /// 投递一枚短期验证码。
    pub async fn send_registration_code(
        &self,
        recipient: &str,
        code: &str,
        expires_in_seconds: u64,
    ) -> Result<(), EmailDeliveryError> {
        self.0
            .send_registration_code(recipient, code, expires_in_seconds)
            .await
    }
}

impl fmt::Debug for RegistrationEmailSenderHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RegistrationEmailSenderHandle")
            .finish_non_exhaustive()
    }
}

/// [`RegistrationEmailVerification`] 的独立参数结构。
#[derive(Clone)]
pub struct EmailVerificationConfig {
    /// Redis key 完整前缀（按应用与部署环境隔离，
    /// 如 `"yang-system:prod:registration-email"`）。
    pub redis_key_prefix: String,
    /// 验证码摘要的独立服务端密钥，不得与 Token/Step-up 密钥复用。
    pub secret: String,
    /// 验证码有效期（秒）。
    pub ttl_seconds: u64,
    /// 同一身份的重发冷却（秒）。
    pub resend_cooldown_seconds: u64,
    /// 单枚验证码的最大错误尝试次数，用尽即销毁。
    pub max_attempts: u32,
    /// 验证码位数（1..=9，决定数字码空间 10^n）。
    pub code_digits: usize,
    /// 发送侧计数窗口长度（秒）。
    pub send_window_seconds: u64,
    /// 窗口内单来源 IP 允许的最大发送次数。
    pub send_ip_attempts: u64,
    /// 窗口内单身份允许的最大发送次数。
    pub send_email_attempts: u64,
    /// 窗口内全局允许的最大发送次数。
    pub send_global_attempts: u64,
    /// 发送侧计数器指标名（`metrics` feature 启用时发出）。
    pub send_metric_name: &'static str,
    /// 校验侧计数器指标名（`metrics` feature 启用时发出）。
    pub verify_metric_name: &'static str,
}

impl fmt::Debug for EmailVerificationConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EmailVerificationConfig")
            .field("redis_key_prefix", &self.redis_key_prefix)
            .field("secret", &"[REDACTED]")
            .field("ttl_seconds", &self.ttl_seconds)
            .field("resend_cooldown_seconds", &self.resend_cooldown_seconds)
            .field("max_attempts", &self.max_attempts)
            .field("code_digits", &self.code_digits)
            .field("send_window_seconds", &self.send_window_seconds)
            .field("send_ip_attempts", &self.send_ip_attempts)
            .field("send_email_attempts", &self.send_email_attempts)
            .field("send_global_attempts", &self.send_global_attempts)
            .field("send_metric_name", &self.send_metric_name)
            .field("verify_metric_name", &self.verify_metric_name)
            .finish()
    }
}

/// 验证码请求被接受后的通用响应（不暴露身份是否真实存在）。
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RegistrationEmailCodeAccepted {
    /// 请求已被接受（不保证邮件真实投递）。
    pub accepted: bool,
    /// 验证码有效期（秒）。
    pub expires_in: u64,
    /// 重发冷却（秒）。
    pub resend_after: u64,
}

/// 规范化邮箱地址：去空白、小写化，并做严格的 ASCII 结构校验。
///
/// 结构校验完整覆盖 dot-atom 本地部分与标签化域名规则（单 `@`、本地部分不含
/// 连续/首尾点号、域名标签只允许字母数字与内部连字符、顶级标签为纯字母且至少
/// 两字符），因此不再需要额外的地址解析库。
pub fn normalize_email(value: &str) -> Result<String, BaseError> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() || normalized.len() > 254 || !normalized.is_ascii() {
        return Err(invalid_email());
    }
    let Some((local, domain)) = normalized.split_once('@') else {
        return Err(invalid_email());
    };
    let local_valid = !local.is_empty()
        && local.len() <= 64
        && !local.starts_with('.')
        && !local.ends_with('.')
        && !local.contains("..")
        && local.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'%' | b'+' | b'-')
        });
    let labels = domain.split('.').collect::<Vec<_>>();
    let domain_valid = labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty()
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
        && labels.last().is_some_and(|label| {
            label.len() >= 2 && label.bytes().all(|byte| byte.is_ascii_alphabetic())
        });
    if !local_valid || !domain_valid {
        return Err(invalid_email());
    }
    Ok(normalized)
}

/// 一次性验证码引擎：配置经 `Tools` 的 config 槽注入，投递器经 extension 槽注入。
pub struct RegistrationEmailVerification<'a> {
    config: &'a EmailVerificationConfig,
}

impl<'a> RegistrationEmailVerification<'a> {
    /// 从 Action 上下文的 `Tools` config 槽读取 [`EmailVerificationConfig`] 构建引擎。
    pub fn from_context(ctx: &'a ActionContext) -> Result<Self, BaseError> {
        let config = ctx.tools().config::<EmailVerificationConfig>()?;
        if !(1..=9).contains(&config.code_digits) {
            return Err(BaseError::ConfigError(
                "邮箱验证码位数必须在 1..=9 范围内".to_string(),
            ));
        }
        Ok(Self { config })
    }

    /// 请求签发一枚验证码。
    ///
    /// `deliver = false` 时（如身份已存在的防枚举场景）只消耗限额、不真实投递，
    /// 仍返回通用接受响应。
    pub async fn request(
        &self,
        ctx: &ActionContext,
        email: &str,
        deliver: bool,
    ) -> Result<RegistrationEmailCodeAccepted, BaseError> {
        let fingerprint = email_fingerprint(&self.config.secret, email);
        let prefix = self.key_prefix();
        let keys = [
            format!(
                "{prefix}:send:ip:{}",
                client_ip_fingerprint(&self.config.secret, ctx)
            ),
            format!("{prefix}:send:email:{fingerprint}"),
            format!("{prefix}:send:global"),
            format!("{prefix}:cooldown:{fingerprint}"),
        ];
        let args = [
            self.config.send_window_seconds.to_string(),
            self.config.send_ip_attempts.to_string(),
            self.config.send_email_attempts.to_string(),
            self.config.send_global_attempts.to_string(),
            self.config.resend_cooldown_seconds.to_string(),
        ];
        let cache = ctx.tools().cache()?;
        let decision: (i64, i64) = cache
            .eval_script(&cache.script(RESERVE_SEND_SCRIPT), &keys, &args)
            .await?;
        if decision.0 != 0 {
            #[cfg(feature = "metrics")]
            metrics::counter!(
                self.config.send_metric_name,
                "result" => if decision.0 == 2 { "cooldown" } else { "limited" }
            )
            .increment(1);
            return Err(BaseError::RateLimitExceeded {
                retry_after_seconds: u64::try_from(decision.1).unwrap_or(1).max(1),
            });
        }

        if !deliver {
            #[cfg(feature = "metrics")]
            metrics::counter!(self.config.send_metric_name, "result" => "suppressed").increment(1);
            return Ok(self.accepted());
        }

        let code = generate_code(self.config.code_digits);
        let digest = code_digest(&self.config.secret, email, &code);
        let code_key = format!("{prefix}:code:{fingerprint}");
        cache
            .setex(
                code_key.clone(),
                i64::try_from(self.config.ttl_seconds).map_err(|_| {
                    BaseError::ConfigError("邮箱验证码 TTL 超出 Redis 范围".to_string())
                })?,
                format!("{digest}:0"),
            )
            .await?;

        let sender = ctx.tools().extension::<RegistrationEmailSenderHandle>()?;
        if sender
            .send_registration_code(email, &code, self.config.ttl_seconds)
            .await
            .is_err()
        {
            let _: i64 = cache
                .eval_script(
                    &cache.script(DELETE_IF_CURRENT_SCRIPT),
                    &[code_key],
                    &[digest],
                )
                .await?;
            #[cfg(feature = "metrics")]
            metrics::counter!(self.config.send_metric_name, "result" => "failed").increment(1);
            return Err(delivery_unavailable());
        }
        #[cfg(feature = "metrics")]
        metrics::counter!(self.config.send_metric_name, "result" => "sent").increment(1);
        Ok(self.accepted())
    }

    /// 校验并原子消费一枚验证码；任何失败都返回统一的无效验证码错误。
    pub async fn consume(
        &self,
        ctx: &ActionContext,
        email: &str,
        code: &str,
    ) -> Result<(), BaseError> {
        if code.len() != self.config.code_digits || !code.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(invalid_code());
        }
        let fingerprint = email_fingerprint(&self.config.secret, email);
        let key = format!("{}:code:{fingerprint}", self.key_prefix());
        let digest = code_digest(&self.config.secret, email, code);
        let cache = ctx.tools().cache()?;
        let result: i64 = cache
            .eval_script(
                &cache.script(VERIFY_AND_CONSUME_SCRIPT),
                &[key],
                &[digest, self.config.max_attempts.to_string()],
            )
            .await?;
        if result != 1 {
            #[cfg(feature = "metrics")]
            metrics::counter!(self.config.verify_metric_name, "result" => "denied").increment(1);
            return Err(invalid_code());
        }
        #[cfg(feature = "metrics")]
        metrics::counter!(self.config.verify_metric_name, "result" => "consumed").increment(1);
        Ok(())
    }

    fn key_prefix(&self) -> &str {
        &self.config.redis_key_prefix
    }

    fn accepted(&self) -> RegistrationEmailCodeAccepted {
        RegistrationEmailCodeAccepted {
            accepted: true,
            expires_in: self.config.ttl_seconds,
            resend_after: self.config.resend_cooldown_seconds,
        }
    }
}

fn generate_code(code_digits: usize) -> String {
    let code_space = 10_u32.pow(u32::try_from(code_digits).unwrap_or(6));
    let unbiased_upper_bound = u32::MAX - (u32::MAX % code_space);
    let value = loop {
        let candidate = OsRng.next_u32();
        if candidate < unbiased_upper_bound {
            break candidate % code_space;
        }
    };
    format!("{value:0code_digits$}")
}

fn client_ip_fingerprint(secret: &str, ctx: &ActionContext) -> String {
    keyed_digest(
        secret,
        &[
            b"registration-email-ip-v1",
            client_ip_identity(ctx).as_bytes(),
        ],
    )
}

fn email_fingerprint(secret: &str, email: &str) -> String {
    keyed_digest(secret, &[b"registration-email-key-v1", email.as_bytes()])
}

fn code_digest(secret: &str, email: &str, code: &str) -> String {
    keyed_digest(
        secret,
        &[
            b"registration-email-code-v1",
            email.as_bytes(),
            code.as_bytes(),
        ],
    )
}

fn keyed_digest(secret: &str, parts: &[&[u8]]) -> String {
    let mut hasher = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .unwrap_or_else(|_| unreachable!("HMAC-SHA256 接受任意长度密钥"));
    for part in parts {
        hasher.update(&(part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    hasher
        .finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn invalid_email() -> BaseError {
    BaseError::ParamInvalid("email".to_string(), "邮箱格式无效".to_string())
}

fn invalid_code() -> BaseError {
    BaseError::ParamInvalid(
        "email_code".to_string(),
        "邮箱验证码无效或已过期".to_string(),
    )
}

/// 投递失败的脱敏映射。
///
/// `http` feature 关闭时 `BaseError::HttpRequestFailed` 持有字符串载荷，与既有
/// 应用行为完全一致（错误码 300002、Transient、可重试）；`http` feature 开启时该
/// 变体只能由真实 `reqwest::Error` 构造，退回等价的脱敏服务端错误。
fn delivery_unavailable() -> BaseError {
    #[cfg(not(feature = "http"))]
    {
        BaseError::HttpRequestFailed("邮件服务暂不可用".to_string())
    }
    #[cfg(feature = "http")]
    {
        BaseError::Unknown("邮件服务暂不可用".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CODE_DIGITS: usize = 6;

    #[test]
    fn email_is_canonical_ascii_and_rejects_ambiguous_shapes() {
        assert_eq!(
            normalize_email(" Alice.Tag+demo@Example.COM ")
                .unwrap_or_else(|error| panic!("合法邮箱应规范化: {error}")),
            "alice.tag+demo@example.com"
        );
        for invalid in [
            "alice@example",
            "alice@@example.com",
            ".alice@example.com",
            "alice..tag@example.com",
            "alice@-example.com",
            "用户@example.com",
        ] {
            assert!(normalize_email(invalid).is_err(), "应拒绝 {invalid:?}");
        }
    }

    #[test]
    fn generated_code_and_redis_material_do_not_expose_plaintext_identity() {
        let code = generate_code(CODE_DIGITS);
        assert_eq!(code.len(), CODE_DIGITS);
        assert!(code.bytes().all(|byte| byte.is_ascii_digit()));

        let email = "alice@example.com";
        let secret = "independent-email-verification-secret-32-bytes";
        let fingerprint = email_fingerprint(secret, email);
        let digest = code_digest(secret, email, &code);
        assert_eq!(fingerprint.len(), 64);
        assert_eq!(digest.len(), 64);
        assert!(!fingerprint.contains(email));
        assert!(!digest.contains(email));
        assert!(!digest.contains(&code));
    }

    #[test]
    fn config_debug_redacts_the_secret() {
        let config = EmailVerificationConfig {
            redis_key_prefix: "app:test:registration-email".to_string(),
            secret: "independent-email-verification-secret-32-bytes".to_string(),
            ttl_seconds: 600,
            resend_cooldown_seconds: 60,
            max_attempts: 5,
            code_digits: CODE_DIGITS,
            send_window_seconds: 3600,
            send_ip_attempts: 20,
            send_email_attempts: 5,
            send_global_attempts: 1000,
            send_metric_name: "test_registration_email_total",
            verify_metric_name: "test_registration_email_verify_total",
        };
        let debug = format!("{config:?}");
        assert!(!debug.contains("independent-email-verification-secret"));
        assert!(debug.contains("[REDACTED]"));
    }
}
