//! 有界正则缓存（`RegexCache`）并发回归测试（C6 / section 八）。
//!
//! `Validator::Regex` 经 [`Validator::validate_with`] 走由 `Tools` 拥有的共享
//! [`RegexCache`]，内部为 `Mutex<HashMap<String, Arc<Regex>>>`，采用「命中克隆 Arc 即
//! 释放锁 → 未命中编译并按容量上限插入」。本测试用真实多线程争用验证：
//! - 多线程同时校验**同一** pattern（首个未命中触发编译竞争）：结果一致，无 panic
//! - 多线程校验**不同** pattern（并发写入不同 entry）：互不干扰，无丢失
//! - 读写混合高并发不死锁
//!
//! 锁中毒策略为 `unwrap_or_else(|p| p.into_inner())`（中毒可恢复），这里不主动
//! 制造中毒，仅验证正常并发路径的正确性与活性。

#[cfg(feature = "validator")]
use crate::table::{RegexCache, Validator};
#[cfg(feature = "validator")]
use std::sync::Arc;
#[cfg(feature = "validator")]
use std::thread;

/// 多线程并发校验同一 pattern：首次未命中会有编译竞争，但最终缓存只保留一份，
/// 且所有线程结果一致。
#[cfg(feature = "validator")]
#[test]
fn concurrent_same_pattern_compiles_once_and_agrees() {
    // 用一个此前未被其它测试预热的独特 pattern，确保首次访问触发编译路径
    let pattern = r"^c6cache_[0-9]{3}$";
    let validator = Arc::new(Validator::Regex(pattern.to_string()));
    let cache = Arc::new(RegexCache::new(128));

    let mut handles = Vec::new();
    for t in 0..32 {
        let v = Arc::clone(&validator);
        let cache = Arc::clone(&cache);
        handles.push(thread::spawn(move || {
            // 偶数线程喂匹配值，奇数线程喂不匹配值
            let value = if t % 2 == 0 {
                serde_json::json!("c6cache_123")
            } else {
                serde_json::json!("nope")
            };
            let res = v.validate_with("field", &value, &cache);
            (t % 2 == 0, res.is_ok())
        }));
    }

    for h in handles {
        let (should_match, ok) = h.join().expect("校验线程不应 panic");
        assert_eq!(
            should_match, ok,
            "并发编译/命中同一正则后，匹配结果应与输入一致"
        );
    }
    assert_eq!(cache.len(), 1, "同一 pattern 并发编译后缓存应只保留一份");
}

/// 多线程并发校验不同 pattern：各自写入不同 entry，互不覆盖。
#[cfg(feature = "validator")]
#[test]
fn concurrent_distinct_patterns_do_not_interfere() {
    let cache = Arc::new(RegexCache::new(128));
    let mut handles = Vec::new();
    for t in 0..32u32 {
        let cache = Arc::clone(&cache);
        handles.push(thread::spawn(move || {
            // 每个线程一个独特 pattern：恰好匹配自己的编号字符串
            let pattern = format!(r"^c6distinct_{}$", t);
            let validator = Validator::Regex(pattern);
            let good = validator.validate_with(
                "f",
                &serde_json::json!(format!("c6distinct_{}", t)),
                &cache,
            );
            let bad = validator.validate_with(
                "f",
                &serde_json::json!(format!("c6distinct_{}", t + 1)),
                &cache,
            );
            (good.is_ok(), bad.is_err())
        }));
    }

    for h in handles {
        let (good_ok, bad_err) = h.join().expect("不应 panic");
        assert!(good_ok, "应匹配自身编号");
        assert!(bad_err, "不应匹配相邻编号（entry 未被串台）");
    }
    assert_eq!(cache.len(), 32, "32 个不同 pattern 应各自入表一条");
}

/// 读写混合高并发：重复命中已缓存 pattern + 偶尔引入新 pattern，验证不死锁。
#[cfg(feature = "validator")]
#[test]
fn concurrent_read_write_mix_no_deadlock() {
    let cache = Arc::new(RegexCache::new(128));
    let mut handles = Vec::new();
    for t in 0..16u32 {
        let cache = Arc::clone(&cache);
        handles.push(thread::spawn(move || {
            for i in 0..200u32 {
                // 大部分命中共享的热 pattern（读锁快路径），少量引入新 pattern（写锁）
                let pattern = if i % 25 == 0 {
                    format!(r"^c6mix_{}_{}$", t, i)
                } else {
                    r"^c6mix_hot$".to_string()
                };
                let v = Validator::Regex(pattern);
                let _ = v.validate_with("f", &serde_json::json!("c6mix_hot"), &cache);
            }
        }));
    }
    for h in handles {
        h.join().expect("读写混合不应 panic 或死锁");
    }
}
