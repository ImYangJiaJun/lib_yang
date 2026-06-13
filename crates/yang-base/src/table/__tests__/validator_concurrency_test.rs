//! REGEX_CACHE 并发回归测试（C6 / section 八）。
//!
//! `Validator::Regex` 走全局 `OnceLock<RwLock<HashMap<String, Regex>>>` 缓存，
//! 采用「读锁快路径命中 → 未命中升写锁编译并 `entry().or_insert_with`」。本测试
//! 用真实多线程争用验证：
//! - 多线程同时校验**同一** pattern（首个未命中触发编译竞争）：结果一致，无 panic
//! - 多线程校验**不同** pattern（并发写入不同 entry）：互不干扰，无丢失
//! - 读写混合高并发不死锁
//!
//! 锁中毒策略为 `unwrap_or_else(|p| p.into_inner())`（中毒可恢复），这里不主动
//! 制造中毒，仅验证正常并发路径的正确性与活性。

#[cfg(feature = "validator")]
use crate::table::Validator;
#[cfg(feature = "validator")]
use std::sync::Arc;
#[cfg(feature = "validator")]
use std::thread;

/// 多线程并发校验同一 pattern：首次未命中会有编译竞争，
/// 但 `entry().or_insert_with` 保证只采用一份，且所有线程结果一致。
#[cfg(feature = "validator")]
#[test]
fn concurrent_same_pattern_compiles_once_and_agrees() {
    // 用一个此前未被其它测试预热的独特 pattern，确保首次访问触发编译路径
    let pattern = r"^c6cache_[0-9]{3}$";
    let validator = Arc::new(Validator::Regex(pattern.to_string()));

    let mut handles = Vec::new();
    for t in 0..32 {
        let v = Arc::clone(&validator);
        handles.push(thread::spawn(move || {
            // 偶数线程喂匹配值，奇数线程喂不匹配值
            let value = if t % 2 == 0 {
                serde_json::json!("c6cache_123")
            } else {
                serde_json::json!("nope")
            };
            let res = v.validate("field", &value);
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
}

/// 多线程并发校验不同 pattern：各自写入不同 entry，互不覆盖。
#[cfg(feature = "validator")]
#[test]
fn concurrent_distinct_patterns_do_not_interfere() {
    let mut handles = Vec::new();
    for t in 0..32u32 {
        handles.push(thread::spawn(move || {
            // 每个线程一个独特 pattern：恰好匹配自己的编号字符串
            let pattern = format!(r"^c6distinct_{}$", t);
            let validator = Validator::Regex(pattern);
            let good = validator.validate("f", &serde_json::json!(format!("c6distinct_{}", t)));
            let bad = validator.validate("f", &serde_json::json!(format!("c6distinct_{}", t + 1)));
            (good.is_ok(), bad.is_err())
        }));
    }

    for h in handles {
        let (good_ok, bad_err) = h.join().expect("不应 panic");
        assert!(good_ok, "应匹配自身编号");
        assert!(bad_err, "不应匹配相邻编号（entry 未被串台）");
    }
}

/// 读写混合高并发：重复命中已缓存 pattern + 偶尔引入新 pattern，验证不死锁。
#[cfg(feature = "validator")]
#[test]
fn concurrent_read_write_mix_no_deadlock() {
    let mut handles = Vec::new();
    for t in 0..16u32 {
        handles.push(thread::spawn(move || {
            for i in 0..200u32 {
                // 大部分命中共享的热 pattern（读锁快路径），少量引入新 pattern（写锁）
                let pattern = if i % 25 == 0 {
                    format!(r"^c6mix_{}_{}$", t, i)
                } else {
                    r"^c6mix_hot$".to_string()
                };
                let v = Validator::Regex(pattern);
                let _ = v.validate("f", &serde_json::json!("c6mix_hot"));
            }
        }));
    }
    for h in handles {
        h.join().expect("读写混合不应 panic 或死锁");
    }
}
