//! 熔断器并发回归测试（C6 / section 八）。
//!
//! 用真实多线程争用 `Arc<Mutex<HashMap>>` 状态，验证：
//! - 并发 `on_failure` 不丢计数：N 线程各打 1 次失败后，host 必然进入 Open
//! - 并发 `allow`/`on_success`/`on_failure` 混合调用不 panic、不死锁、状态自洽
//! - `clone()` 出的多个句柄共享同一份状态（跨线程可见）
//!
//! 这些测试在动 C1/C4 等并发热路径前先织好回归网：任何把锁策略改坏
//! （如换成非共享状态、引入 await 跨锁）都会在此暴露。

use crate::http::{CircuitBreaker, CircuitBreakerConfig};
use std::sync::Arc;
use std::thread;

fn cb(failure_threshold: u32, cooldown_secs: u64, success_threshold: u32) -> CircuitBreaker {
    CircuitBreaker::new(CircuitBreakerConfig {
        failure_threshold,
        cooldown_secs,
        success_threshold,
    })
    .expect("测试熔断器配置应合法")
}

/// N 个线程各记录一次失败，阈值设为 N：必然恰好打开熔断，无计数丢失。
#[test]
fn concurrent_failures_reach_threshold_without_loss() {
    const N: u32 = 64;
    let breaker = Arc::new(cb(N, 30, 1));
    let host = "api.example.com";

    let mut handles = Vec::new();
    for _ in 0..N {
        let b = Arc::clone(&breaker);
        handles.push(thread::spawn(move || {
            b.on_failure(host);
        }));
    }
    for h in handles {
        h.join().expect("线程不应 panic");
    }

    // 累计 N 次失败 == 阈值 N → Open，应拒绝放行
    assert!(
        !breaker.allow(host),
        "并发累计 {} 次失败达阈值后熔断应打开（计数未丢失）",
        N
    );
}

/// 阈值高于并发失败数：不应打开（验证不会“多计”导致误熔断）。
#[test]
fn concurrent_failures_below_threshold_stay_closed() {
    const N: u32 = 32;
    let breaker = Arc::new(cb(N + 1, 30, 1));
    let host = "h";

    let mut handles = Vec::new();
    for _ in 0..N {
        let b = Arc::clone(&breaker);
        handles.push(thread::spawn(move || b.on_failure(host)));
    }
    for h in handles {
        h.join().unwrap();
    }

    assert!(
        breaker.allow(host),
        "{} 次失败 < 阈值 {}，不应熔断（计数未虚增）",
        N,
        N + 1
    );
}

/// 高并发混合调用（allow / on_success / on_failure）跨多个 host：
/// 仅验证无 panic、无死锁、能在有限时间内全部完成。
#[test]
fn concurrent_mixed_calls_do_not_deadlock_or_panic() {
    let breaker = Arc::new(cb(5, 1, 2));
    let hosts = ["a", "b", "c", "d"];

    let mut handles = Vec::new();
    for t in 0..32 {
        let b = Arc::clone(&breaker);
        handles.push(thread::spawn(move || {
            for i in 0..500 {
                let host = hosts[(t + i) % hosts.len()];
                match i % 3 {
                    0 => {
                        let _ = b.allow(host);
                    }
                    1 => b.on_success(host),
                    _ => b.on_failure(host),
                }
            }
        }));
    }
    for h in handles {
        h.join().expect("混合并发调用不应 panic");
    }

    // 状态机仍可正常响应（不卡死）
    let _ = breaker.allow("a");
}

/// 共享性：clone 出的句柄看到彼此的写入。
#[test]
fn cloned_handles_share_state_across_threads() {
    let breaker = cb(2, 30, 1);
    let clone_a = breaker.clone();
    let clone_b = breaker.clone();
    let host = "shared";

    // 线程 A 打两次失败 → 应触发 Open
    let a = thread::spawn(move || {
        clone_a.on_failure(host);
        clone_a.on_failure(host);
    });
    a.join().unwrap();

    // 线程 B 通过另一个 clone 观察到 Open 状态
    let b = thread::spawn(move || clone_b.allow(host));
    let allowed = b.join().unwrap();
    assert!(!allowed, "另一个 clone 句柄应观察到已打开的熔断状态");
}
