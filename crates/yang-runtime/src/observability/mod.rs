//! JSON 日志、Prometheus 管理面与 OpenTelemetry 运行时。

pub mod logging;
pub mod telemetry;

use serde::Deserialize;
use std::net::SocketAddr;
use thiserror::Error;

pub use logging::{ActionLogMiddleware, LogIdentity, RuntimeMetricNames};
pub use telemetry::{ReadinessGate, TelemetryRuntime};

/// 日志输出格式。
///
/// 同一份事件、同一批字段，两种排版：`Json` 给采集端（单行、可解析），`Pretty` 给人读
/// （多行缩进、字段逐行对齐）。**事件与字段完全一致**，只是排版不同——所以本地开
/// `Pretty` 看清楚、生产用 `Json` 送出去，不需要维护两套日志语句。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    /// 单行 JSON。采集与生产环境的默认值。
    #[default]
    Json,
    /// 多行缩进。命令行窗口里读日志用。
    Pretty,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservabilitySettings {
    #[serde(default)]
    pub metrics_enabled: bool,
    #[serde(default = "default_metrics_bind")]
    pub metrics_bind: String,
    #[serde(default)]
    pub traces_enabled: bool,
    #[serde(default = "default_traces_otlp_endpoint")]
    pub traces_otlp_endpoint: String,
    #[serde(default = "default_traces_sample_ratio")]
    pub traces_sample_ratio: f64,
    #[serde(default = "default_traces_export_timeout_seconds")]
    pub traces_export_timeout_seconds: u64,
    #[serde(default = "default_readiness_budget_ms")]
    pub readiness_budget_ms: u64,
    /// 日志输出格式。省略时保持单行 JSON——不改变既有部署的行为。
    #[serde(default)]
    pub log_format: LogFormat,
}

impl Default for ObservabilitySettings {
    fn default() -> Self {
        Self {
            metrics_enabled: false,
            metrics_bind: default_metrics_bind(),
            traces_enabled: false,
            traces_otlp_endpoint: default_traces_otlp_endpoint(),
            traces_sample_ratio: default_traces_sample_ratio(),
            traces_export_timeout_seconds: default_traces_export_timeout_seconds(),
            readiness_budget_ms: default_readiness_budget_ms(),
            log_format: LogFormat::default(),
        }
    }
}

#[derive(Debug, Error)]
pub enum ObservabilitySettingsError {
    #[error("observability.metrics_bind 地址无效: {value}")]
    InvalidMetricsBind { value: String },
    #[error("observability.metrics_bind 必须与 http.bind 使用不同地址")]
    MetricsBindConflictsWithHttp,
    #[error("observability.traces_sample_ratio 必须在 0.0..=1.0 范围内")]
    InvalidTraceSampleRatio,
    #[error("observability.traces_export_timeout_seconds 必须在 1..=60 范围内")]
    InvalidTraceExportTimeout,
    #[error("observability.readiness_budget_ms 必须在 50..=10000 范围内")]
    InvalidReadinessBudget,
    #[error("observability.traces_otlp_endpoint 必须是非空的 http:// 或 https:// 地址")]
    InvalidOtlpEndpoint,
}

impl ObservabilitySettings {
    pub fn metrics_bind_addr(&self) -> Result<SocketAddr, ObservabilitySettingsError> {
        self.metrics_bind
            .parse()
            .map_err(|_| ObservabilitySettingsError::InvalidMetricsBind {
                value: self.metrics_bind.clone(),
            })
    }

    pub fn validate(&self, http_bind: SocketAddr) -> Result<(), ObservabilitySettingsError> {
        if self.metrics_enabled && self.metrics_bind_addr()? == http_bind {
            return Err(ObservabilitySettingsError::MetricsBindConflictsWithHttp);
        }
        if !(0.0..=1.0).contains(&self.traces_sample_ratio) || !self.traces_sample_ratio.is_finite()
        {
            return Err(ObservabilitySettingsError::InvalidTraceSampleRatio);
        }
        if !(1..=60).contains(&self.traces_export_timeout_seconds) {
            return Err(ObservabilitySettingsError::InvalidTraceExportTimeout);
        }
        if !(50..=10_000).contains(&self.readiness_budget_ms) {
            return Err(ObservabilitySettingsError::InvalidReadinessBudget);
        }
        if self.traces_enabled {
            let endpoint = self.traces_otlp_endpoint.trim();
            if endpoint.is_empty()
                || endpoint.bytes().any(|byte| byte.is_ascii_whitespace())
                || !(endpoint.starts_with("http://") || endpoint.starts_with("https://"))
            {
                return Err(ObservabilitySettingsError::InvalidOtlpEndpoint);
            }
        }
        Ok(())
    }
}

fn default_metrics_bind() -> String {
    "127.0.0.1:9090".to_string()
}

fn default_traces_otlp_endpoint() -> String {
    "http://127.0.0.1:4317".to_string()
}

const fn default_traces_sample_ratio() -> f64 {
    0.1
}

const fn default_traces_export_timeout_seconds() -> u64 {
    5
}

const fn default_readiness_budget_ms() -> u64 {
    2_000
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 省略 `log_format` 必须落到 `json`：既有部署的配置里没有这个键，若默认值变成
    /// `pretty`，它们会在升级的那一刻**静默**把标准输出从一行一个 object 变成多行，
    /// 采集侧按行切分当场失效。
    #[test]
    fn log_format_defaults_to_json_for_existing_configs() {
        let settings: ObservabilitySettings =
            toml::from_str("metrics_enabled = true").unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(settings.log_format, LogFormat::Json);
        assert_eq!(LogFormat::default(), LogFormat::Json);
    }

    /// 两个取值都要能读进来，且大小写按 serde 的 `lowercase` 约定。
    #[test]
    fn log_format_accepts_both_spellings() {
        for (raw, expected) in [("json", LogFormat::Json), ("pretty", LogFormat::Pretty)] {
            let settings: ObservabilitySettings = toml::from_str(&format!(
                "metrics_enabled = false
log_format = \"{raw}\""
            ))
            .unwrap_or_else(|error| panic!("{raw} 应可解析: {error}"));
            assert_eq!(settings.log_format, expected);
        }
    }
}
