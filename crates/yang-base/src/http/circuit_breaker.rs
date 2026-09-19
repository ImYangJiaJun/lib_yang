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
//! 失败 host 条目受 `max_tracked_hosts` 与 `host_idle_ttl_secs` 约束，防止状态 map
//! 随不同 host 数无限增长。

use crate::error::BaseError;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 熔断器策略配置。默认关闭——只有在 [`HttpClientConfig`](crate::http::HttpClientConfig)
/// 中显式设置 `circuit_breaker: Some(..)` 才启用。
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CircuitBreakerConfig {
    /// 触发熔断的连续失败次数（Closed 状态下）。
    pub failure_threshold: u32,
    /// Open 状态的冷却秒数，冷却后放行一次探测（转 HalfOpen）。
    pub cooldown_secs: u64,
    /// HalfOpen 状态下恢复 Closed 所需的连续成功次数。
    pub success_threshold: u32,
    /// 状态 map 最多保留的 host 数；超出后按最久未活跃淘汰。
    pub max_tracked_hosts: usize,
    /// host 条目空闲多久后可被淘汰（秒）。必须 >= cooldown_secs，
    /// 否则已打开的熔断可能在冷却期结束前被遗忘（fail-open）。
    pub host_idle_ttl_secs: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            cooldown_secs: 30,
            success_threshold: 1,
            max_tracked_hosts: 1024,
            host_idle_ttl_secs: 600,
        }
    }
}

impl CircuitBreakerConfig {
    /// 验证熔断器策略配置。
    ///
    /// 阈值和冷却时间为 0 时会破坏状态机语义，因此在构建客户端前 fail-fast。
    pub fn validate(&self) -> Result<(), BaseError> {
        if self.failure_threshold == 0 {
            return Err(BaseError::ParamInvalid(
                "http.circuit_breaker.failure_threshold".to_string(),
                "熔断器失败阈值必须大于 0".to_string(),
            ));
        }
        if self.cooldown_secs == 0 {
            return Err(BaseError::ParamInvalid(
                "http.circuit_breaker.cooldown_secs".to_string(),
                "熔断器冷却时间必须大于 0 秒".to_string(),
            ));
        }
        if self.success_threshold == 0 {
            return Err(BaseError::ParamInvalid(
                "http.circuit_breaker.success_threshold".to_string(),
                "熔断器恢复成功阈值必须大于 0".to_string(),
            ));
        }
        if self.max_tracked_hosts == 0 {
            return Err(BaseError::ParamInvalid(
                "http.circuit_breaker.max_tracked_hosts".to_string(),
                "熔断器 host 上限必须大于 0".to_string(),
            ));
        }
        if self.host_idle_ttl_secs < self.cooldown_secs {
            return Err(BaseError::ParamInvalid(
                "http.circuit_breaker.host_idle_ttl_secs".to_string(),
                "host 空闲淘汰时间不能小于冷却时间，否则会提前遗忘已打开的熔断".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;
    use crate::error::BaseError;

    #[test]
    fn test_circuit_breaker_config_validate_rejects_zero_values() {
        let invalid_configs = [
            (
                "http.circuit_breaker.failure_threshold",
                CircuitBreakerConfig {
                    failure_threshold: 0,
                    ..CircuitBreakerConfig::default()
                },
            ),
            (
                "http.circuit_breaker.cooldown_secs",
                CircuitBreakerConfig {
                    cooldown_secs: 0,
                    ..CircuitBreakerConfig::default()
                },
            ),
            (
                "http.circuit_breaker.success_threshold",
                CircuitBreakerConfig {
                    success_threshold: 0,
                    ..CircuitBreakerConfig::default()
                },
            ),
            (
                "http.circuit_breaker.max_tracked_hosts",
                CircuitBreakerConfig {
                    max_tracked_hosts: 0,
                    ..CircuitBreakerConfig::default()
                },
            ),
            (
                "http.circuit_breaker.host_idle_ttl_secs",
                CircuitBreakerConfig {
                    host_idle_ttl_secs: 10, // 小于默认 cooldown_secs 30，应被拒绝
                    ..CircuitBreakerConfig::default()
                },
            ),
        ];

        for (expected_field, config) in invalid_configs {
            let err = config.validate().expect_err("熔断器零值配置应被拒绝");

            assert!(matches!(
                err,
                BaseError::ParamInvalid(field, _) if field == expected_field
            ));
        }
    }

    #[test]
    fn test_circuit_breaker_new_rejects_invalid_config() {
        let err = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 0,
            ..CircuitBreakerConfig::default()
        })
        .expect_err("公开构造器应拒绝非法熔断器配置");

        assert!(matches!(
            err,
            BaseError::ParamInvalid(field, _) if field == "http.circuit_breaker.failure_threshold"
        ));
    }
}

/// 单个 host 的熔断状态。注意：HashMap 中**无 entry** 等价于「Closed 且零失败」，
/// 因此健康 host 不占用内存；失败 host 条目受 `max_tracked_hosts` 与
/// `host_idle_ttl_secs` 约束，达到容量上限或空闲超时后被淘汰。
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
    /// host → (最后活跃时刻, 当前状态)。无 entry == Closed 且零失败。
    /// 条目受 `max_tracked_hosts` 与 `host_idle_ttl_secs` 约束，防止无界增长。
    states: Arc<Mutex<HashMap<String, (Instant, Phase)>>>,
}

impl CircuitBreaker {
    /// 用给定策略创建熔断器。
    pub fn new(config: CircuitBreakerConfig) -> Result<Self, BaseError> {
        config.validate()?;

        Ok(Self {
            config,
            states: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// 请求发送前的准入检查。`true` 放行，`false` 表示熔断打开应直接拒绝。
    /// 冷却结束的 Open 会在此转为 HalfOpen 并放行探测。
    pub fn allow(&self, host: &str) -> bool {
        self.allow_at(host, Instant::now())
    }

    /// 记录一次成功（2xx/3xx/4xx）。HalfOpen 下累计成功，达阈值恢复 Closed。
    pub fn on_success(&self, host: &str) {
        let now = Instant::now();
        let mut map = self.states.lock().unwrap_or_else(|p| p.into_inner());
        match map.get(host) {
            // Closed 有失败累计 → 成功即清零（移除 entry 回到健康）。
            Some((_, Phase::Closed { .. })) => {
                map.remove(host);
            }
            Some((_, Phase::HalfOpen { successes })) => {
                let next = successes.saturating_add(1);
                if next >= self.config.success_threshold {
                    map.remove(host); // 恢复 Closed
                } else {
                    map.insert(host.to_string(), (now, Phase::HalfOpen { successes: next }));
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
        let mut map = self.states.lock().unwrap_or_else(|p| p.into_inner());
        match map.get(host) {
            None | Some((_, Phase::Closed { .. })) | Some((_, Phase::HalfOpen { .. })) => true,
            Some((_, Phase::Open { opened_at })) => {
                let cooled = now.duration_since(*opened_at)
                    >= Duration::from_secs(self.config.cooldown_secs);
                if cooled {
                    map.insert(host.to_string(), (now, Phase::HalfOpen { successes: 0 }));
                    true // 放行一次探测
                } else {
                    false
                }
            }
        }
    }

    /// `on_failure` 的可注入时钟版本。
    pub(crate) fn on_failure_at(&self, host: &str, now: Instant) {
        let mut map = self.states.lock().unwrap_or_else(|p| p.into_inner());

        // 新 host 首次失败会新增条目：先回收空间，避免 map 随不同 host 数无限增长。
        if !map.contains_key(host) {
            self.make_room(&mut map, now);
        }

        match map.get_mut(host) {
            None => {
                let phase = if self.config.failure_threshold <= 1 {
                    Phase::Open { opened_at: now }
                } else {
                    Phase::Closed { failures: 1 }
                };
                map.insert(host.to_string(), (now, phase));
            }
            Some((last_seen, Phase::Closed { failures })) => {
                *last_seen = now;
                *failures = failures.saturating_add(1);
                if *failures >= self.config.failure_threshold {
                    map.insert(host.to_string(), (now, Phase::Open { opened_at: now }));
                }
            }
            // 半开探测失败 → 立即重新打开；已打开 → 刷新冷却起点。
            Some((last_seen, Phase::HalfOpen { .. })) | Some((last_seen, Phase::Open { .. })) => {
                *last_seen = now;
                map.insert(host.to_string(), (now, Phase::Open { opened_at: now }));
            }
        }
    }

    /// 在插入新 host 条目前回收空间：先淘汰空闲超时的条目，
    /// 仍达上限时淘汰最久未活跃者（优先淘汰非 Open 条目，尽量不遗忘已打开的熔断）。
    ///
    /// 权衡：容量压力下淘汰 `Phase::Open` 条目等于遗忘一次已打开的熔断（fail-open），
    /// 因此 (a) 用 `host_idle_ttl_secs >= cooldown_secs` 的校验保证正常冷却周期内不会
    /// 被空闲淘汰，(b) 淘汰顺序优先挑非 Open，只有全部为 Open 时才动 Open。
    fn make_room(&self, map: &mut HashMap<String, (Instant, Phase)>, now: Instant) {
        let ttl = Duration::from_secs(self.config.host_idle_ttl_secs);
        map.retain(|_, (last_seen, _)| now.duration_since(*last_seen) < ttl);

        while map.len() >= self.config.max_tracked_hosts {
            let victim = map
                .iter()
                .min_by_key(|(_, (last_seen, phase))| {
                    (matches!(phase, Phase::Open { .. }), *last_seen)
                })
                .map(|(host, _)| host.clone());
            match victim {
                Some(host) => {
                    map.remove(&host);
                }
                None => break,
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn tracked_host_count(&self) -> usize {
        self.states.lock().unwrap_or_else(|p| p.into_inner()).len()
    }
}
