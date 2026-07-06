//! 熔断器单元测试（L-4）。
//!
//! 用 `allow_at` / `on_failure_at` 的可注入时钟版本模拟冷却，避免真实 sleep。

use crate::http::{CircuitBreaker, CircuitBreakerConfig};
use std::time::{Duration, Instant};

fn cb(failure_threshold: u32, cooldown_secs: u64, success_threshold: u32) -> CircuitBreaker {
    CircuitBreaker::new(CircuitBreakerConfig {
        failure_threshold,
        cooldown_secs,
        success_threshold,
    })
    .expect("测试熔断器配置应合法")
}

#[test]
fn default_config_values() {
    let c = CircuitBreakerConfig::default();
    assert_eq!(c.failure_threshold, 5);
    assert_eq!(c.cooldown_secs, 30);
    assert_eq!(c.success_threshold, 1);
}

#[test]
fn healthy_host_always_allowed() {
    let b = cb(3, 10, 1);
    assert!(b.allow("api.example.com"));
    // 未达阈值的失败不应熔断
    b.on_failure("api.example.com");
    b.on_failure("api.example.com");
    assert!(b.allow("api.example.com"));
}

#[test]
fn opens_after_threshold_consecutive_failures() {
    let b = cb(3, 10, 1);
    b.on_failure("h");
    b.on_failure("h");
    assert!(b.allow("h"), "2 次失败 < 阈值 3，仍放行");
    b.on_failure("h"); // 第 3 次 → 打开
    assert!(!b.allow("h"), "达阈值后应熔断");
}

#[test]
fn success_resets_failure_count() {
    let b = cb(3, 10, 1);
    b.on_failure("h");
    b.on_failure("h");
    b.on_success("h"); // 清零
    b.on_failure("h");
    b.on_failure("h");
    assert!(b.allow("h"), "成功清零后再两次失败不应熔断");
}

#[test]
fn per_host_isolation() {
    let b = cb(2, 10, 1);
    b.on_failure("bad");
    b.on_failure("bad"); // bad 熔断
    assert!(!b.allow("bad"));
    assert!(b.allow("good"), "好 host 不受坏 host 影响");
}

// COOLDOWN_TESTS_PLACEHOLDER

#[test]
fn open_rejects_until_cooldown_then_half_opens() {
    let b = cb(1, 10, 1); // 1 次失败即打开
    let t0 = Instant::now();
    b.on_failure_at("h", t0); // 打开于 t0
    assert!(!b.allow_at("h", t0 + Duration::from_secs(5)), "冷却内拒绝");
    // 冷却结束 → 放行一次探测（转 HalfOpen）
    assert!(
        b.allow_at("h", t0 + Duration::from_secs(10)),
        "冷却后放行探测"
    );
}

#[test]
fn half_open_success_closes() {
    let b = cb(1, 10, 2); // 需要 2 次成功才恢复
    let t0 = Instant::now();
    b.on_failure_at("h", t0);
    assert!(b.allow_at("h", t0 + Duration::from_secs(10))); // → HalfOpen
    b.on_success("h"); // 1/2
    assert!(b.allow("h"), "HalfOpen 期间继续放行");
    b.on_success("h"); // 2/2 → Closed
                       // Closed 后即便不推进时钟也应放行
    assert!(b.allow("h"));
}

#[test]
fn half_open_failure_reopens() {
    let b = cb(1, 10, 1);
    let t0 = Instant::now();
    b.on_failure_at("h", t0);
    assert!(b.allow_at("h", t0 + Duration::from_secs(10))); // → HalfOpen
                                                            // 探测失败 → 重新打开，冷却起点刷新为 t1
    let t1 = t0 + Duration::from_secs(11);
    b.on_failure_at("h", t1);
    assert!(
        !b.allow_at("h", t1 + Duration::from_secs(5)),
        "重开后冷却内拒绝"
    );
    assert!(
        b.allow_at("h", t1 + Duration::from_secs(10)),
        "新冷却结束后再放行"
    );
}

#[test]
fn threshold_one_opens_immediately() {
    let b = cb(1, 5, 1);
    let t0 = Instant::now();
    b.on_failure_at("h", t0);
    assert!(!b.allow_at("h", t0), "阈值 1 时单次失败立即熔断");
}

#[test]
fn shared_state_across_clones() {
    let b = cb(2, 10, 1);
    let b2 = b.clone();
    b.on_failure("h");
    b2.on_failure("h"); // clone 共享状态，累计到 2 → 打开
    assert!(!b.allow("h"));
    assert!(!b2.allow("h"));
}
