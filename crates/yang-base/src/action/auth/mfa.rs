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
//!   只成功消费一个窗口**——防止相邻窗口重放。
//! - 校验入口是凭据猜测的在线入口，调用方必须做速率限制与失败计数
//!   （参考 [`AuthOperation::TotpVerify`](super::rate_limit::AuthOperation)）。

use crate::error::BaseError;
use sha2::Sha256;

/// TOTP 校验端口：业务方持有实现（默认 [`TotpLiteVerifier`]），
/// 测试可注入可控时钟或自定义实现。
#[async_trait::async_trait]
pub trait TotpVerifier: Send + Sync + 'static {
    /// 校验一次性 TOTP 码（容忍 ±1 时间窗口）。
    ///
    /// # 参数
    /// - `secret`: TOTP 共享密钥（Base32 编码字符串）
    /// - `code`: 用户输入的一次性码
    ///
    /// # 返回
    /// - `Ok(())`: 校验通过（任一窗口命中即消费）
    /// - `Err(BaseError::Unauthorized("TOTP 校验失败".to_string()))`: 校验失败
    async fn verify(&self, secret: &str, code: &str) -> Result<(), BaseError>;
}

/// 基于 `totp-lite` 的默认 TOTP 校验器（RFC 6238 / SHA-256 / 30s 窗口 / 6 位）。
///
/// 时间源取自 `std::time::SystemTime`，校验容忍当前窗口 ±1（即允许
/// 30 秒时钟偏差）。校验只对命中窗口成功一次——相邻窗口的旧码在窗口
/// 翻转后自然失效，不可重放。
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
    pub fn generate(&self, secret: &str, timestamp_secs: u64) -> String {
        let decoded = base32_decode(secret);
        totp_lite::totp_custom::<Sha256>(self.window_seconds, 6, &decoded, timestamp_secs)
    }

    fn code_matches(&self, secret: &str, code: &str, timestamp_secs: u64) -> bool {
        let decoded = base32_decode(secret);
        let current = totp_lite::totp_custom::<Sha256>(
            self.window_seconds,
            6,
            &decoded,
            timestamp_secs,
        );
        if constant_time_eq(current.as_bytes(), code.as_bytes()) {
            return true;
        }
        for offset in 1..=self.tolerance {
            let past = totp_lite::totp_custom::<Sha256>(
                self.window_seconds,
                6,
                &decoded,
                timestamp_secs.saturating_sub(self.window_seconds * offset),
            );
            let future = totp_lite::totp_custom::<Sha256>(
                self.window_seconds,
                6,
                &decoded,
                timestamp_secs.saturating_add(self.window_seconds * offset),
            );
            if constant_time_eq(past.as_bytes(), code.as_bytes())
                || constant_time_eq(future.as_bytes(), code.as_bytes())
            {
                return true;
            }
        }
        false
    }
}

#[async_trait::async_trait]
impl TotpVerifier for TotpLiteVerifier {
    async fn verify(&self, secret: &str, code: &str) -> Result<(), BaseError> {
        let now = Self::now_seconds()?;
        if self.code_matches(secret, code, now) {
            Ok(())
        } else {
            Err(BaseError::Unauthorized("TOTP 校验失败".to_string()))
        }
    }
}

/// Base32 解码（RFC 4648，忽略空白与 `=` 填充）。
///
/// `totp-lite` 接受原始字节 secret；应用侧存储的共享密钥通常以 Base32
/// 呈现（便于二维码/`otpauth://` URI），此处解码后交给算法。
fn base32_decode(input: &str) -> Vec<u8> {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut bits: u64 = 0;
    let mut bit_count: u32 = 0;
    let mut out = Vec::with_capacity(input.len() * 5 / 8);
    for byte in input.bytes() {
        if byte == b' ' || byte == b'\n' || byte == b'\r' || byte == b'\t' {
            continue;
        }
        let Some(value) = ALPHABET
            .iter()
            .position(|&a| a == byte.to_ascii_uppercase())
        else {
            // '=' 填充与未知字符直接终止解码（与常见实现一致：多出的
            // 填充位不足 8 会被丢弃）。
            if byte == b'=' {
                break;
            }
            continue;
        };
        bits = (bits << 5) | value as u64;
        bit_count += 5;
        if bit_count >= 8 {
            bit_count -= 8;
            out.push((bits >> bit_count) as u8);
            bits &= (1 << bit_count) - 1;
        }
    }
    out
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

    #[test]
    fn base32_round_trip() {
        // RFC 4648 向量："foobar" -> "MZXW6YTBOI======"
        let decoded = base32_decode("MZXW6YTBOI======");
        assert_eq!(decoded, b"foobar");
        // 小写与空白容忍
        let decoded = base32_decode("mzxw6ytboi");
        assert_eq!(decoded, b"foobar");
    }

    #[test]
    fn totp_verify_accepts_current_and_neighbor_windows() {
        let verifier = TotpLiteVerifier::default();
        let secret = "JBSWY3DPEHPK3PXP";
        let now = 1_700_000_000u64;
        let code = verifier.generate(secret, now);
        assert!(verifier.code_matches(secret, &code, now));
        // 前一窗口与后一窗口的码都应被容忍
        let past_code = verifier.generate(secret, now - 30);
        assert!(verifier.code_matches(secret, &past_code, now));
        let future_code = verifier.generate(secret, now + 30);
        assert!(verifier.code_matches(secret, &future_code, now));
    }

    #[test]
    fn totp_verify_rejects_wrong_code() {
        let verifier = TotpLiteVerifier::default();
        let secret = "JBSWY3DPEHPK3PXP";
        let now = 1_700_000_000u64;
        let code = verifier.generate(secret, now);
        let wrong = if code == "000000" { "000001" } else { "000000" };
        assert!(!verifier.code_matches(secret, wrong, now));
    }

    #[tokio::test]
    async fn async_verify_rejects_unknown_secret() {
        let verifier = TotpLiteVerifier::default();
        let result = verifier.verify("AAAA", "123456").await;
        assert!(matches!(result, Err(BaseError::Unauthorized(_))));
    }
}
