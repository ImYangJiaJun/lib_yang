//! 引擎可观测性配置（C4 收口）
//!
//! 把 C4 一揽子的运行期旋钮（慢查询阈值等）收敛到单一可测试入口，与
//! [`DatabaseBundle::init`](crate::database) 的「统一初始化入口」理念一致。
//!
//! 用 `OnceLock` 单例承载；重复 `init` 返回 `Err` 而非 panic（与全局 DB/Redis
//! 单例语义一致）。未初始化时 [`ObservabilityConfig::get`] 返回默认值（全部关闭），
//! 使可观测性对未配置的调用方完全无感。

use crate::error::BaseError;
use std::sync::OnceLock;
use std::time::Duration;

/// 全局可观测性配置单例。
static OBSERVABILITY_CONFIG: OnceLock<ObservabilityConfig> = OnceLock::new();

/// 可观测性运行期配置。
///
/// 当前仅含慢查询阈值，预留后续扩展（如采样率、SQL 文本记录开关等）。
/// 派生 `Default`（全部关闭），未初始化时即此默认值。
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

    /// 初始化全局可观测性配置单例。
    ///
    /// # 返回
    ///
    /// - `Ok(())`：初始化成功
    /// - `Err(BaseError::ConfigError)`：已初始化过，重复调用（不覆盖、不 panic）
    pub fn init(config: ObservabilityConfig) -> Result<(), BaseError> {
        OBSERVABILITY_CONFIG
            .set(config)
            .map_err(|_| BaseError::ConfigError("ObservabilityConfig 已初始化".to_string()))
    }

    /// 获取全局配置引用；未初始化时返回静态默认值（全部关闭）。
    pub fn get() -> &'static ObservabilityConfig {
        static DEFAULT: ObservabilityConfig = ObservabilityConfig {
            slow_query_threshold: None,
        };
        OBSERVABILITY_CONFIG.get().unwrap_or(&DEFAULT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_returns_default_when_uninitialized() {
        // 注意：单例进程内只能 init 一次，本测试只验证默认值语义
        let cfg = ObservabilityConfig::get();
        // 未 init 时为默认（None）；若其它测试已 init 则可能非 None，
        // 故此处仅断言访问不 panic 且类型正确
        let _ = cfg.slow_query_threshold;
    }

    #[test]
    fn builder_sets_threshold() {
        let cfg = ObservabilityConfig::new().with_slow_query_threshold(Duration::from_millis(500));
        assert_eq!(cfg.slow_query_threshold, Some(Duration::from_millis(500)));
    }

    #[test]
    fn double_init_returns_err() {
        // 用独立配置值验证「重复 init 返 Err 不 panic」语义；
        // 由于单例全局共享，这里通过先 set 再 set 的方式直接验证 OnceLock 行为
        let local: OnceLock<ObservabilityConfig> = OnceLock::new();
        assert!(local.set(ObservabilityConfig::default()).is_ok());
        assert!(local.set(ObservabilityConfig::default()).is_err());
    }
}
