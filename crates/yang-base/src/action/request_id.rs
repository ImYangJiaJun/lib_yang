//! 请求标识（request_id）
//!
//! 一次 Action 派发的运行期标识，用于串联日志、span、metrics、审计。放在
//! [`ActionContext`](crate::action::ActionContext) 上（运行期上下文），而非
//! [`Request`](crate::action::Request)（传输输入），避免污染请求数据模型。
//!
//! 类型刻意选 `u128` 轻量结构（时间高位 | 计数器低位），不引入 uuid 依赖：
//! - 高 64 位：生成时刻的毫秒级 Unix 时间戳（粗略有序，便于按时间排查）
//! - 低 64 位：进程内单调递增计数器（同一毫秒内去重）
//!
//! 注意：这是**进程内**唯一标识，不保证跨进程全局唯一。需要跨服务串联时，由
//! 上游通过 `X-Request-Id` header 传入字符串，[`RequestIdMiddleware`] 会优先透传。

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

/// 进程内单调计数器（request_id 低位来源）。
static COUNTER: AtomicU64 = AtomicU64::new(0);

/// 一次 Action 派发的运行期标识。
///
/// `Display` 输出为 32 位十六进制（无分隔），便于日志检索：
/// 形如 `0192f4a1c3d50000000000000000002a`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RequestId(u128);

impl RequestId {
    /// 生成一个新的 request_id（时间高位 | 计数器低位）。
    ///
    /// 单次整数运算 + 一次原子自增，无堆分配，满足热路径零分配要求。
    pub fn generate() -> Self {
        // chrono 毫秒时间戳作高位；为负（1970 前，理论不会）时按 0 处理
        let millis = chrono::Utc::now().timestamp_millis().max(0) as u128;
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed) as u128;
        RequestId((millis << 64) | counter)
    }

    /// 从已有 u128 构造（供从上游标识解析时使用）。
    pub fn from_u128(v: u128) -> Self {
        RequestId(v)
    }

    /// 取出底层 u128。
    pub fn as_u128(&self) -> u128 {
        self.0
    }

    /// 解析上游 `X-Request-Id` 为 RequestId（供透传跨服务标识）。
    ///
    /// 接受形式：
    /// - 1..=32 位十六进制（无分隔），如 `2a`、`0192f4a1c3d5...`；
    /// - 标准 UUID（带连字符，如 `550e8400-e29b-41d4-a716-446655440000`）——
    ///   去连字符后恰为 32 位十六进制，覆盖最常见的上游标识与 W3C traceparent 的
    ///   trace-id 段。
    ///
    /// 非十六进制或去连字符后超 32 位返回 `None`，调用方据此降级为新生成。
    ///
    /// **限制**：`RequestId` 底层是 `u128`，无法承载任意字符串标识（如纯字母 trace
    /// id）。这类上游标识会被拒绝并降级——若需端到端透传任意字符串标识，应在传输层
    /// 自行记录原始 header，本类型只负责进程内可排序的数值标识。
    pub fn parse_hex(s: &str) -> Option<Self> {
        let s = s.trim();
        // 标准 UUID：去连字符后按十六进制解析（36→32 位）
        let normalized = if s.contains('-') {
            s.replace('-', "")
        } else {
            s.to_string()
        };
        if normalized.is_empty() || normalized.len() > 32 {
            return None;
        }
        u128::from_str_radix(&normalized, 16).ok().map(RequestId)
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 32 位定宽十六进制，前导零补齐，便于对齐与检索
        write!(f, "{:032x}", self.0)
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::generate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_is_monotonic_within_process() {
        let a = RequestId::generate();
        let b = RequestId::generate();
        // 低位计数器严格递增，故同进程内 b > a（即便同毫秒）
        assert_ne!(a, b, "连续生成的 request_id 不应相同");
        assert!(b.as_u128() > a.as_u128(), "request_id 应单调递增");
    }

    #[test]
    fn display_is_32_hex_chars() {
        let id = RequestId::from_u128(0x0192_f4a1_c3d5_0000_0000_0000_0000_002a);
        let s = id.to_string();
        assert_eq!(s.len(), 32, "应为 32 位定宽十六进制: {}", s);
        assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn parse_hex_roundtrip() {
        let id = RequestId::from_u128(0xdead_beef);
        let parsed = RequestId::parse_hex(&id.to_string()).expect("应能解析");
        assert_eq!(id, parsed);
    }

    #[test]
    fn parse_hex_rejects_invalid() {
        assert!(RequestId::parse_hex("").is_none());
        assert!(RequestId::parse_hex("xyz").is_none());
        // 超过 32 位
        assert!(RequestId::parse_hex(&"f".repeat(33)).is_none());
    }

    #[test]
    fn parse_hex_accepts_upstream_short_form() {
        let parsed = RequestId::parse_hex("2a").expect("短十六进制应可解析");
        assert_eq!(parsed.as_u128(), 0x2a);
    }

    /// NEW-7：标准带连字符 UUID 去连字符后应可解析（覆盖最常见上游标识）。
    #[test]
    fn parse_hex_accepts_hyphenated_uuid() {
        let uuid = "550e8400-e29b-41d4-a716-446655440000";
        let parsed = RequestId::parse_hex(uuid).expect("UUID 应可解析");
        assert_eq!(parsed.as_u128(), 0x550e8400_e29b_41d4_a716_446655440000);
        // 纯字母 trace id 仍拒绝（u128 无法承载），降级为新生成
        assert!(RequestId::parse_hex("not-a-hex-id-zz").is_none());
    }
}
