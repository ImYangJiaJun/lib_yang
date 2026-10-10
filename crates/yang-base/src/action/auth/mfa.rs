//! 第二因子（TOTP）校验端口与默认实现（路线图 E-1）。
//!
//! 本模块只提供**校验端口**与基于 [`totp-lite`] 的默认实现，不自行编写
//! HOTP/TOTP 算法（RFC 6238 禁止自写，统一走 `totp-lite`）。密钥的
//! 生成/加密存储、激活状态与恢复码由业务方（yang-system）在 users 表
//! 与 Action 层处理，本模块不感知存储形态。
//!
//! # 设计约束
//!
//! - 校验器必须容忍 ±1 个 TOTP 时间窗口（30s×2）的时钟偏差，且**一次校验
//!   将全部命中窗口一起消费**——防止相邻窗口及碰撞重放。
//! - 校验入口是凭据猜测的在线入口，调用方必须做速率限制与失败计数
//!   （参考 [`AuthOperation::TotpVerify`](super::rate_limit::AuthOperation)）。

use crate::action::ActionContext;
use crate::error::BaseError;
use sha2::{Digest, Sha256};

/// TOTP 校验端口：业务方持有实现（默认 [`TotpLiteVerifier`]），
/// 测试可注入可控时钟或自定义实现。
#[async_trait::async_trait]
pub trait TotpVerifier: Send + Sync + 'static {
    /// 校验一次性 TOTP 码（容忍 ±1 时间窗口）。
    ///
    /// # 参数
    /// - `ctx`: 提供共享 Redis 消费状态的请求上下文
    /// - `subject`: 服务端确定的稳定用户标识，所有认证用途须保持一致
    /// - `secret`: TOTP 共享密钥（Base32 编码字符串）
    /// - `code`: 用户输入的一次性码
    ///
    /// # 返回
    /// - `Ok(())`: 校验通过（任一窗口命中即消费）
    /// - `Err(BaseError::Unauthorized("TOTP 校验失败".to_string()))`: 校验失败
    async fn verify(
        &self,
        ctx: &ActionContext,
        subject: &str,
        secret: &str,
        code: &str,
    ) -> Result<(), BaseError>;
}

/// 基于 `totp-lite` 的默认 TOTP 校验器（RFC 6238 / SHA-256 / 30s 窗口 / 6 位）。
///
/// 时间源取自 `std::time::SystemTime`，校验容忍当前窗口 ±1（即允许
/// 30 秒时钟偏差）。通过共享 Redis 原子消费命中的时间步，跨实例、跨用途
/// 及窗口翻转后均拒绝重放；Redis 不可用时拒绝认证。
#[derive(Debug, Clone)]
pub struct TotpLiteVerifier {
    /// 时间窗口（秒），默认 30；测试可注入。
    pub window_seconds: u64,
    /// 容忍的前向/后向窗口数（默认 1）。
    pub tolerance: u64,
}

impl Default for TotpLiteVerifier {
    fn default() -> Self {
        Self {
            window_seconds: 30,
            tolerance: 1,
        }
    }
}

impl TotpLiteVerifier {
    fn now_seconds() -> Result<u64, BaseError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| BaseError::ConfigError("系统时间早于 Unix 纪元".to_string()))?;
        Ok(now.as_secs())
    }

    /// 对给定时间戳生成 TOTP 码（供测试与激活码生成共用）。
    /// 密钥或窗口配置非法时返回空字符串。
    pub fn generate(&self, secret: &str, timestamp_secs: u64) -> String {
        if self.validate_config().is_err() {
            return String::new();
        }
        let Ok(decoded) = base32_decode(secret) else {
            return String::new();
        };
        totp_lite::totp_custom::<Sha256>(self.window_seconds, 6, &decoded, timestamp_secs)
    }

    fn validate_config(&self) -> Result<(), BaseError> {
        if self.window_seconds == 0 || self.window_seconds > 3600 || self.tolerance > 10 {
            return Err(BaseError::ConfigError(
                "TOTP 窗口必须为 1..=3600 秒，容差不得超过 10 步".to_string(),
            ));
        }
        Ok(())
    }

    fn matching_steps(
        &self,
        decoded: &[u8],
        code: &str,
        timestamp_secs: u64,
    ) -> Result<Vec<u64>, BaseError> {
        self.validate_config()?;
        let current = timestamp_secs / self.window_seconds;
        let last = current
            .checked_add(self.tolerance)
            .ok_or_else(|| BaseError::ConfigError("TOTP 时间溢出".to_string()))?;
        let mut matches = Vec::new();
        for step in current.saturating_sub(self.tolerance)..=last {
            let timestamp = step
                .checked_mul(self.window_seconds)
                .ok_or_else(|| BaseError::ConfigError("TOTP 时间溢出".to_string()))?;
            let expected =
                totp_lite::totp_custom::<Sha256>(self.window_seconds, 6, decoded, timestamp);
            if constant_time_eq(expected.as_bytes(), code.as_bytes()) {
                matches.push(step);
            }
        }
        Ok(matches)
    }

    async fn verify_at(
        &self,
        ctx: &ActionContext,
        subject: &str,
        secret: &str,
        code: &str,
        now: u64,
    ) -> Result<(), BaseError> {
        self.validate_config()?;
        if subject.is_empty() {
            return Err(BaseError::ConfigError("TOTP 缺少可信用户标识".to_string()));
        }
        let decoded = base32_decode(secret)?;
        let steps = self.matching_steps(&decoded, code, now)?;
        let Some(last) = steps.last() else {
            return Err(invalid_totp());
        };
        let expires = last
            .checked_add(self.tolerance + 1)
            .and_then(|step| step.checked_mul(self.window_seconds))
            .ok_or_else(|| BaseError::ConfigError("TOTP 时间溢出".to_string()))?;
        let ttl = expires
            .checked_sub(now)
            .ok_or_else(|| BaseError::ConfigError("TOTP 消费过期时间非法".to_string()))?;
        let identity = format!(
            "{:x}:{:x}",
            Sha256::digest(subject.as_bytes()),
            Sha256::digest(&decoded)
        );
        let keys: Vec<_> = steps
            .iter()
            .map(|step| format!("auth:totp:{{{identity}}}:{step}"))
            .collect();
        let cache = ctx.tools().cache()?;
        // 所有碰撞命中步一起检查和消费，避免改走另一命中步或并发重放。
        let script = cache.script(
            r#"
for _, key in ipairs(KEYS) do
    if redis.call('EXISTS', key) == 1 then return 0 end
end
for _, key in ipairs(KEYS) do
    redis.call('SET', key, '1', 'EX', ARGV[1])
end
return 1
"#,
        );
        let consumed: i64 = cache
            .eval_script(&script, &keys, &[ttl.to_string()])
            .await?;
        if consumed == 1 {
            Ok(())
        } else {
            Err(invalid_totp())
        }
    }
}

#[async_trait::async_trait]
impl TotpVerifier for TotpLiteVerifier {
    async fn verify(
        &self,
        ctx: &ActionContext,
        subject: &str,
        secret: &str,
        code: &str,
    ) -> Result<(), BaseError> {
        self.verify_at(ctx, subject, secret, code, Self::now_seconds()?)
            .await
    }
}

/// Base32 解码（RFC 4648，忽略空白与 `=` 填充）。
///
/// `totp-lite` 接受原始字节 secret；应用侧存储的共享密钥通常以 Base32
/// 呈现（便于二维码/`otpauth://` URI），此处解码后交给算法。
fn base32_decode(input: &str) -> Result<Vec<u8>, BaseError> {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut bits: u64 = 0;
    let mut bit_count: u32 = 0;
    if input.len() > 256 {
        return Err(invalid_totp());
    }
    let mut out = Vec::with_capacity(input.len() * 5 / 8);
    let mut symbols = 0usize;
    let mut padding = 0usize;
    for byte in input.bytes() {
        if byte == b' ' || byte == b'\n' || byte == b'\r' || byte == b'\t' {
            continue;
        }
        if byte == b'=' {
            padding += 1;
            continue;
        }
        if padding != 0 {
            return Err(invalid_totp());
        }
        let Some(value) = ALPHABET
            .iter()
            .position(|&a| a == byte.to_ascii_uppercase())
        else {
            return Err(invalid_totp());
        };
        symbols += 1;
        bits = (bits << 5) | value as u64;
        bit_count += 5;
        if bit_count >= 8 {
            bit_count -= 8;
            out.push((bits >> bit_count) as u8);
            bits &= (1 << bit_count) - 1;
        }
    }
    if out.is_empty()
        || bits != 0
        || !matches!(symbols % 8, 0 | 2 | 4 | 5 | 7)
        || (padding != 0 && padding != (8 - symbols % 8) % 8)
    {
        return Err(invalid_totp());
    }
    Ok(out)
}

fn invalid_totp() -> BaseError {
    BaseError::Unauthorized("TOTP 校验失败".to_string())
}

/// 常量时间比较（防时序侧信道）；长度不等直接失败。
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod __tests__ {
    use super::*;
    use crate::tools::ToolsBuilder;

    #[test]
    fn rejects_invalid_secrets_and_window_configuration() {
        for secret in ["", "A", "!!!!", "MY=AA", "MZ", "MY==="] {
            assert!(base32_decode(secret).is_err(), "{secret}");
        }
        for verifier in [
            TotpLiteVerifier {
                window_seconds: 0,
                tolerance: 1,
            },
            TotpLiteVerifier {
                window_seconds: 30,
                tolerance: u64::MAX,
            },
        ] {
            assert!(verifier.matching_steps(b"secret", "123456", 100).is_err());
        }
    }

    #[tokio::test]
    #[ignore = "需要 Docker 启动 Redis 7"]
    async fn totp_consumption_rejects_replay_across_windows_and_instances() {
        use crate::{action::Request, tools::ToolsBuilder};
        use testcontainers::{core::WaitFor, runners::AsyncRunner, GenericImage};
        let container = GenericImage::new("redis", "7-alpine")
            .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
            .start()
            .await
            .expect("Redis 容器");
        let port = container.get_host_port_ipv4(6379).await.expect("端口");
        let cache = yang_db::RedisClient::connect(format!("redis://127.0.0.1:{port}"))
            .await
            .expect("Redis");
        let tools = std::sync::Arc::new(
            ToolsBuilder::new()
                .cache(cache.clone())
                .build()
                .expect("资源"),
        );
        let ctx = ActionContext::new(Request::new(serde_json::json!({})), tools);
        let verifier = TotpLiteVerifier::default();
        let other = TotpLiteVerifier::default();
        let secret = "MZXW6YTBOI======";
        let now = 1_700_000_010;
        let code = verifier.generate(secret, now + 30);
        verifier
            .verify_at(&ctx, "user", secret, &code, now)
            .await
            .expect("接受未来邻窗");
        assert!(other
            .verify_at(&ctx, "user", "mzxw6 ytboi", &code, now + 30)
            .await
            .is_err());
        assert!(other
            .verify_at(&ctx, "user", secret, &code, now + 60)
            .await
            .is_err());
        other
            .verify_at(&ctx, "other-user", secret, &code, now)
            .await
            .expect("用户隔离");
        let code = verifier.generate(secret, now);
        let (left, right) = tokio::join!(
            verifier.verify_at(&ctx, "concurrent", secret, &code, now),
            other.verify_at(&ctx, "concurrent", secret, &code, now)
        );
        assert_eq!(
            [left, right].iter().filter(|result| result.is_ok()).count(),
            1
        );
        let keys = cache.keys("auth:totp:*").await.expect("消费键");
        assert!(!keys.is_empty());
        for key in keys {
            assert!(
                cache.ttl(&key).await.expect("TTL") >= 59,
                "TTL 覆盖邻窗接受期"
            );
        }
        cache.close().await;
        assert!(
            verifier
                .verify_at(&ctx, "unavailable", secret, &code, now)
                .await
                .is_err(),
            "Redis 不可用时拒绝认证"
        );
    }

    #[test]
    fn base32_round_trip() {
        // RFC 4648 向量："foobar" -> "MZXW6YTBOI======"
        let decoded = base32_decode("MZXW6YTBOI======").expect("有效密钥");
        assert_eq!(decoded, b"foobar");
        // 小写与空白容忍
        let decoded = base32_decode("mzxw6ytboi").expect("有效密钥");
        assert_eq!(decoded, b"foobar");
    }

    #[test]
    fn totp_verify_accepts_current_and_neighbor_windows() {
        let verifier = TotpLiteVerifier::default();
        let secret = "JBSWY3DPEHPK3PXP";
        let now = 1_700_000_000u64;
        let code = verifier.generate(secret, now);
        assert!(!verifier
            .matching_steps(&base32_decode(secret).expect("密钥"), &code, now)
            .expect("窗口")
            .is_empty());
        // 前一窗口与后一窗口的码都应被容忍
        let past_code = verifier.generate(secret, now - 30);
        assert!(!verifier
            .matching_steps(&base32_decode(secret).expect("密钥"), &past_code, now)
            .expect("窗口")
            .is_empty());
        let future_code = verifier.generate(secret, now + 30);
        assert!(!verifier
            .matching_steps(&base32_decode(secret).expect("密钥"), &future_code, now)
            .expect("窗口")
            .is_empty());
    }

    #[test]
    fn totp_verify_rejects_wrong_code() {
        let verifier = TotpLiteVerifier::default();
        let secret = "JBSWY3DPEHPK3PXP";
        let now = 1_700_000_000u64;
        let code = verifier.generate(secret, now);
        let wrong = if code == "000000" { "000001" } else { "000000" };
        assert!(verifier
            .matching_steps(&base32_decode(secret).expect("密钥"), wrong, now)
            .expect("窗口")
            .is_empty());
    }

    #[tokio::test]
    async fn async_verify_rejects_unknown_secret() {
        let verifier = TotpLiteVerifier::default();
        let ctx = ActionContext::new(
            crate::action::Request::new(serde_json::json!({})),
            std::sync::Arc::new(ToolsBuilder::new().build().expect("资源")),
        );
        let result = verifier.verify(&ctx, "subject", "!!!!", "123456").await;
        assert!(matches!(result, Err(BaseError::Unauthorized(_))));
    }
}
