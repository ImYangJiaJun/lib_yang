//! 独立进程测试 ObservabilityConfig 经 Tools 配置槽的注册 → 读取链路。
//!
//! 全局 `OnceLock` 单例已删除：配置在启动期经 `ToolsBuilder::config(...)` 注册，
//! 运行期由 `ActionContext::table_query()` 经 `tools.config::<ObservabilityConfig>()`
//! 读取；未注册时阈值解析为 `None`（关闭慢查询日志）。

use std::sync::Arc;
use std::time::Duration;
use yang_base::action::{ActionContext, Request};
use yang_base::observability::ObservabilityConfig;
use yang_base::table::{Field, Table};
use yang_base::tools::ToolsBuilder;

#[test]
fn config_slot_roundtrip() {
    // Arrange: 构造一个带慢查询阈值的配置并注册进 Tools 配置槽
    let threshold = Duration::from_millis(200);
    let tools = ToolsBuilder::new()
        .config(ObservabilityConfig::new().with_slow_query_threshold(threshold))
        .build()
        .expect("注册可观测性配置后应构建成功");

    // Act & Assert: 经 config 槽读回应得到刚注册的值
    let retrieved = tools
        .config::<ObservabilityConfig>()
        .expect("已注册的配置应可读回");
    assert_eq!(
        retrieved.slow_query_threshold,
        Some(threshold),
        "注册后 config::<ObservabilityConfig>() 应返回配置的 slow_query_threshold"
    );
}

#[test]
fn unregistered_config_disables_slow_query_log() {
    // 未注册 ObservabilityConfig 时 config 槽读取失败，
    // table_query() 路径将其映射为 None（关闭慢查询日志），与历史默认行为一致。
    let tools = ToolsBuilder::new().build().expect("空 Tools 应构建成功");

    assert!(tools.config::<ObservabilityConfig>().is_err());
}

#[test]
fn table_query_path_available_with_registered_config() {
    // 注册配置后，table_query() 路径（无需数据库）应正常构建查询对象；
    // 阈值注入由 crate 内 context 单测覆盖，这里验证集成路径可用。
    let definition = Table::new("obs_rows")
        .fields([Field::id("id"), Field::string("name", 64)])
        .build()
        .expect("表定义应有效");
    let tools = Arc::new(
        ToolsBuilder::new()
            .config(ObservabilityConfig::new().with_slow_query_threshold(Duration::from_millis(50)))
            .build()
            .expect("注册可观测性配置后应构建成功"),
    );
    let context = ActionContext::new(Request::new(serde_json::json!({})), tools)
        .with_table_definition(definition);

    let _query = context
        .table_query()
        .expect("注册配置后 table_query 路径应可用");
}
