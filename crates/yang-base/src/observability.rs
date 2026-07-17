//! 引擎可观测性配置（C4 收口）
//!
//! 把运行期可观测性旋钮（慢查询阈值等）收敛为单一纯数据配置。
//! 遵循「资源显式所有权」理念：启动期经
//! [`ToolsBuilder::config`](crate::tools::ToolsBuilder::config) 注册进
//! [`Tools`](crate::tools::Tools) 的只读配置槽，运行期由
//! [`ActionContext::table_query`](crate::action::ActionContext::table_query) 经
//! `tools.config::<ObservabilityConfig>()` 读取。
//!
//! 未注册时慢查询日志整体关闭（阈值解析为 `None`），可观测性对未配置的调用方完全无感。

use std::time::Duration;

/// 可观测性运行期配置。
///
/// 当前仅含慢查询阈值，预留后续扩展（如采样率、SQL 文本记录开关等）。
/// 派生 `Default`（全部关闭），未向 `Tools` 注册时即等价于此默认值。
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ObservabilityConfig {
    /// 慢查询阈值：受保护层 `TableQuery` 单次执行耗时超过此值时 `warn` 日志。
    ///
    /// `None`（默认）表示不启用慢查询日志，执行边界计时分支整体短路。
    pub slow_query_threshold: Option<Duration>,
}

impl ObservabilityConfig {
    /// 构造一个空配置（全部关闭），等价于 `Default`。
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置慢查询阈值（链式）。
    pub fn with_slow_query_threshold(mut self, threshold: Duration) -> Self {
        self.slow_query_threshold = Some(threshold);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::ToolsBuilder;

    #[test]
    fn builder_sets_threshold() {
        let cfg = ObservabilityConfig::new().with_slow_query_threshold(Duration::from_millis(500));
        assert_eq!(cfg.slow_query_threshold, Some(Duration::from_millis(500)));
    }

    #[test]
    fn default_disables_slow_query_log() {
        let cfg = ObservabilityConfig::new();
        assert_eq!(cfg.slow_query_threshold, None);
    }

    #[test]
    fn config_slot_roundtrip_via_tools() {
        let threshold = Duration::from_millis(200);
        let tools = ToolsBuilder::new()
            .config(ObservabilityConfig::new().with_slow_query_threshold(threshold))
            .build()
            .expect("注册可观测性配置后应构建成功");

        let retrieved = tools
            .config::<ObservabilityConfig>()
            .expect("已注册的配置应可读回");
        assert_eq!(retrieved.slow_query_threshold, Some(threshold));
    }
}
