//! 冻结 Tools 的并发读取回归测试。
//!
//! Tools 不提供请求期注册入口；构建后所有线程只读同一个类型化资源图，不需要
//! `RwLock`，也不存在同名字符串注册竞态。

use crate::tools::{Tools, ToolsBuilder};
use std::sync::Arc;
use std::thread;

#[derive(Debug, PartialEq, Eq)]
struct SharedExtension(usize);

fn fresh_tools() -> Arc<Tools> {
    Arc::new(
        ToolsBuilder::new()
            .extension(SharedExtension(42))
            .config(String::from("test"))
            .build()
            .expect("测试 Tools 应构建成功"),
    )
}

#[test]
fn concurrent_typed_reads_are_lock_free_and_consistent() {
    let tools = fresh_tools();
    let mut handles = Vec::new();

    for _ in 0..64 {
        let tools = Arc::clone(&tools);
        handles.push(thread::spawn(move || {
            for _ in 0..1_000 {
                assert_eq!(
                    tools
                        .extension::<SharedExtension>()
                        .expect("扩展应在构建期注册"),
                    &SharedExtension(42)
                );
                assert_eq!(
                    tools.config::<String>().expect("配置应在构建期注册"),
                    "test"
                );
            }
        }));
    }

    for handle in handles {
        handle.join().expect("并发读取不应 panic");
    }
}

#[test]
fn wrong_or_missing_type_returns_stable_error() {
    let tools = fresh_tools();
    let missing_extension = tools
        .extension::<String>()
        .expect_err("配置类型不能从扩展命名空间读取");
    let missing_config = tools
        .config::<SharedExtension>()
        .expect_err("扩展类型不能从配置命名空间读取");

    assert!(missing_extension.to_string().contains("未配置扩展"));
    assert!(missing_config.to_string().contains("未配置配置类型"));
}
