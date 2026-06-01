//! HTTP 熔断器（circuit breaker，L-4）。
//!
//! 经典三态熔断器，**按目标 host 分键**：一个故障上游被熔断时，不影响发往
//! 其它健康 host 的请求。状态机：
//!
//! - **Closed（闭合）**：正常放行，累计连续失败数；达到 `failure_threshold` → Open。
//! - **Open（断开）**：直接拒绝（返回 `HttpCircuitBreakerOpen`）；冷却 `cooldown_secs`
//!   秒后转 HalfOpen 放行探测。
//! - **HalfOpen（半开）**：放行探测请求；累计 `success_threshold` 次成功 → Closed，
//!   任一失败 → 重新 Open。
//!
//! 失败的定义由调用方（`RequestBuilder::send`）决定：传输错误与 5xx 视为失败，
//! 4xx 视为健康（服务端有能力正常拒绝请求）。
//!
//! 状态用 `Arc<Mutex<HashMap>>` 共享，随 [`HttpClient`](crate::http::HttpClient)
//! 的 `clone()` 复用同一份。锁仅在同步的检查/记录期间短暂持有，绝不跨 `.await`。
//! HalfOpen 不做并发探测限流——多个并发请求都会被放行探测，适合本场景的轻量需求。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 熔断器策略配置。默认关闭——只有在 [`HttpClientConfig`](crate::http::HttpClientConfig)
/// 中显式设置 `circuit_breaker: Some(..)` 才启用。
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// 触发熔断的连续失败次数（Closed 状态下）。
    pub failure_threshold: u32,
    /// Open 状态的冷却秒数，冷却后放行一次探测（转 HalfOpen）。
    pub cooldown_secs: u64,
    /// HalfOpen 状态下恢复 Closed 所需的连续成功次数。
    pub success_threshold: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            cooldown_secs: 30,
            success_threshold: 1,
        }
    }
}

/// 单个 host 的熔断状态。注意：HashMap 中**无 entry** 等价于「Closed 且零失败」，
/// 因此健康 host 不占用内存。
#[derive(Debug)]
enum Phase {
    /// 闭合，附带当前连续失败数（>=1；0 失败时直接从 map 移除）。
    Closed { failures: u32 },
    /// 断开，记录进入时刻用于判断冷却是否结束。
    Open { opened_at: Instant },
    /// 半开，附带当前连续成功数。
    HalfOpen { successes: u32 },
}

/// 按 host 分键的熔断器。`clone()` 共享同一份状态（内部 `Arc`）。
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    /// host → 当前状态。无 entry == Closed 且零失败。
    states: Arc<Mutex<HashMap<String, Phase>>>,
}

impl CircuitBreaker {
    /// 用给定策略创建熔断器。
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            states: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 请求发送前的准入检查。`true` 放行，`false` 表示熔断打开应直接拒绝。
    /// 冷却结束的 Open 会在此转为 HalfOpen 并放行探测。
    pub fn allow(&self, host: &str) -> bool {
        self.allow_at(host, Instant::now())
    }

    /// 记录一次成功（2xx/3xx/4xx）。HalfOpen 下累计成功，达阈值恢复 Closed。
    pub fn on_success(&self, host: &str) {
        let mut map = self.states.lock().expect("熔断器状态锁中毒");
        match map.get_mut(host) {
            // Closed 有失败累计 → 成功即清零（移除 entry 回到健康）。
            Some(Phase::Closed { .. }) => {
                map.remove(host);
            }
            Some(Phase::HalfOpen { successes }) => {
                *successes += 1;
                if *successes >= self.config.success_threshold {
                    map.remove(host); // 恢复 Closed
                }
            }
            // None（已健康）或 Open（探测尚未经 allow 放行）：无需变更。
            _ => {}
        }
    }

    /// 记录一次失败（传输错误或 5xx）。达阈值/半开失败 → 打开熔断。
    pub fn on_failure(&self, host: &str) {
        self.on_failure_at(host, Instant::now());
    }

    /// `allow` 的可注入时钟版本，供测试模拟冷却。
    pub(crate) fn allow_at(&self, host: &str, now: Instant) -> bool {
        let mut map = self.states.lock().expect("熔断器状态锁中毒");
        match map.get(host) {
            None | Some(Phase::Closed { .. }) | Some(Phase::HalfOpen { .. }) => true,
            Some(Phase::Open { opened_at }) => {
                let cooled = now.duration_since(*opened_at)
                    >= Duration::from_secs(self.config.cooldown_secs);
                if cooled {
                    map.insert(host.to_string(), Phase::HalfOpen { successes: 0 });
                    true // 放行一次探测
                } else {
                    false
                }
            }
        }
    }

    /// `on_failure` 的可注入时钟版本。
    pub(crate) fn on_failure_at(&self, host: &str, now: Instant) {
        let mut map = self.states.lock().expect("熔断器状态锁中毒");
        match map.get_mut(host) {
            None => {
                let phase = if self.config.failure_threshold <= 1 {
                    Phase::Open { opened_at: now }
                } else {
                    Phase::Closed { failures: 1 }
                };
                map.insert(host.to_string(), phase);
            }
            Some(Phase::Closed { failures }) => {
                *failures += 1;
                if *failures >= self.config.failure_threshold {
                    map.insert(host.to_string(), Phase::Open { opened_at: now });
                }
            }
            // 半开探测失败 → 立即重新打开；已打开 → 刷新冷却起点。
            Some(Phase::HalfOpen { .. }) | Some(Phase::Open { .. }) => {
                map.insert(host.to_string(), Phase::Open { opened_at: now });
            }
        }
    }
}
