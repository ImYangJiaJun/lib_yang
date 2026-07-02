//! 独立进程测试 ObservabilityConfig 的 init → get 链路。
//!
//! 集成测试中每个 `tests/*.rs` 文件编译为独立二进制，与其它测试文件不共享
//! 进程空间，因此 `OnceLock` 全局单例可在此安全地 init 并验证取值。
//! 本文件仅含一个测试函数，避免同一二进制内多次 init 冲突。

use std::time::Duration;
use yang_base::observability::ObservabilityConfig;

#[test]
fn init_get_link() {
    // Arrange: 构造一个带慢查询阈值的配置
    let threshold = Duration::from_millis(200);
    let config = ObservabilityConfig::new().with_slow_query_threshold(threshold);

    // Act: 初始化全局单例
    ObservabilityConfig::init(config).expect("首次 init 应成功");

    // Assert: get() 应返回刚设置的值
    let retrieved = ObservabilityConfig::get();
    assert_eq!(
        retrieved.slow_query_threshold,
        Some(threshold),
        "init 后 get() 应返回配置的 slow_query_threshold"
    );
}
