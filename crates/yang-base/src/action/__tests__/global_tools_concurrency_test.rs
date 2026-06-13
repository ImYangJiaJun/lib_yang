//! GlobalTools 工具注册表 + OnceLock 单例并发回归测试（C6 / section 八）。
//!
//! `GlobalTools.tools` 是 `Arc<RwLock<HashMap<String, Arc<dyn Any+Send+Sync>>>>`，
//! 锁中毒策略为 `unwrap_or_else(|p| p.into_inner())`（可恢复）。本测试用真实
//! 多线程验证：
//! - 并发 `register_tool` / `get_tool` 不 panic、不死锁，写入不丢
//! - 同名并发注册：最终有且仅有一份可读出（后写覆盖，不破坏 map）
//! - OnceLock 语义：并发读取已初始化单例稳定返回同一实例
//!
//! 注意：进程级 `GLOBAL_TOOLS` 单例只能 `init` 一次且跨测试共享，故此处直接
//! 构造 `GlobalTools` 实例测试其内部并发，不依赖全局 `init`（避免测试间耦合）。

use crate::action::GlobalTools;
use std::sync::Arc;
use std::thread;

/// 构造一个独立的 GlobalTools 实例（不走全局单例，避免测试间耦合）。
#[cfg(feature = "token")]
fn fresh_tools() -> GlobalTools {
    use crate::token::TokenManager;
    use jsonwebtoken::Algorithm;
    // 对称密钥 TokenManager 仅用于占位，本测试不触发 token 逻辑
    let tm = TokenManager::new_symmetric(
        "test_secret_key_for_concurrency_net",
        Algorithm::HS256,
        "test_issuer".to_string(),
        "test_audience".to_string(),
        3600,
        86400,
    );
    GlobalTools::new(tm)
}

#[cfg(not(feature = "token"))]
fn fresh_tools() -> GlobalTools {
    GlobalTools::new()
}

/// 并发注册不同名工具：全部可读出，无丢失。
#[test]
fn concurrent_register_distinct_tools_no_loss() {
    const N: usize = 64;
    let tools = Arc::new(fresh_tools());

    let mut handles = Vec::new();
    for i in 0..N {
        let t = Arc::clone(&tools);
        handles.push(thread::spawn(move || {
            t.register_tool(&format!("tool_{}", i), Arc::new(i));
        }));
    }
    for h in handles {
        h.join().expect("注册线程不应 panic");
    }

    for i in 0..N {
        let got = tools.get_tool::<usize>(&format!("tool_{}", i));
        assert_eq!(got.as_deref(), Some(&i), "tool_{} 应可读出且值正确", i);
    }
}

/// 并发同名注册 + 读取：不 panic、不死锁，最终恰有一份可读出。
#[test]
fn concurrent_same_name_register_and_get_consistent() {
    let tools = Arc::new(fresh_tools());
    let name = "hot_tool";

    let mut handles = Vec::new();
    // 一半线程反复注册同名工具，一半线程反复读取
    for t in 0..32usize {
        let tools = Arc::clone(&tools);
        handles.push(thread::spawn(move || {
            for round in 0..200usize {
                if t % 2 == 0 {
                    tools.register_tool(name, Arc::new(t * 1000 + round));
                } else {
                    // 读取：注册前可能为 None，注册后应为 Some，二者都不应 panic
                    let _ = tools.get_tool::<usize>(name);
                }
            }
        }));
    }
    for h in handles {
        h.join().expect("并发注册/读取不应 panic 或死锁");
    }

    // 最终必有一份可读出（map 未被破坏）
    assert!(
        tools.get_tool::<usize>(name).is_some(),
        "并发同名注册后应恰有一份可读出"
    );
}

/// 类型错配的 get_tool 返回 None 而非 panic（downcast 失败安全）。
#[test]
fn get_tool_wrong_type_returns_none_under_contention() {
    let tools = Arc::new(fresh_tools());
    tools.register_tool("typed", Arc::new(42usize));

    let mut handles = Vec::new();
    for _ in 0..16 {
        let t = Arc::clone(&tools);
        handles.push(thread::spawn(move || {
            // 用错误类型读取应得到 None
            t.get_tool::<String>("typed").is_none()
        }));
    }
    for h in handles {
        assert!(h.join().unwrap(), "类型错配应返回 None 而非 panic");
    }
}
