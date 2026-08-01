//! 生产 JSON 日志与 Action 完成事件。

use async_trait::async_trait;
use opentelemetry::propagation::Extractor;
use opentelemetry::trace::TraceContextExt;
use std::collections::HashMap;
use std::time::Instant;
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use yang_base::action::{ActionContext, ApiResponse};
use yang_base::router::{Middleware, Next};
use yang_base::tools::Tools;
use yang_base::BaseError;

const UNKNOWN_ENVIRONMENT: &str = "unknown";

/// 每条规范事件共享的低基数服务身份。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogIdentity {
    pub service: String,
    pub version: String,
    pub environment: String,
    pub metric_names: RuntimeMetricNames,
}

impl LogIdentity {
    pub fn new(service: &str, version: &str, environment: &str) -> Self {
        Self {
            service: service.to_string(),
            version: version.to_string(),
            environment: environment.to_string(),
            metric_names: RuntimeMetricNames::default(),
        }
    }

    #[must_use]
    pub const fn with_metric_names(mut self, metric_names: RuntimeMetricNames) -> Self {
        self.metric_names = metric_names;
        self
    }

    pub fn from_tools(tools: &Tools) -> Self {
        tools
            .config::<Self>()
            .cloned()
            .unwrap_or_else(|_| Self::fallback())
    }

    fn fallback() -> Self {
        Self {
            service: UNKNOWN_ENVIRONMENT.to_string(),
            version: UNKNOWN_ENVIRONMENT.to_string(),
            environment: UNKNOWN_ENVIRONMENT.to_string(),
            metric_names: RuntimeMetricNames::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeMetricNames {
    pub action_requests: &'static str,
    pub action_duration: &'static str,
    pub build_info: &'static str,
    pub readiness_checks: &'static str,
    pub readiness_duration: &'static str,
    pub readiness_ready: &'static str,
    pub readiness_resource_healthy: &'static str,
    pub resource_pool_connections: &'static str,
}

impl RuntimeMetricNames {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        action_requests: &'static str,
        action_duration: &'static str,
        build_info: &'static str,
        readiness_checks: &'static str,
        readiness_duration: &'static str,
        readiness_ready: &'static str,
        readiness_resource_healthy: &'static str,
        resource_pool_connections: &'static str,
    ) -> Self {
        Self {
            action_requests,
            action_duration,
            build_info,
            readiness_checks,
            readiness_duration,
            readiness_ready,
            readiness_resource_healthy,
            resource_pool_connections,
        }
    }
}

impl Default for RuntimeMetricNames {
    fn default() -> Self {
        Self::new(
            "yang_runtime_action_requests_total",
            "yang_runtime_action_duration_seconds",
            "yang_runtime_build_info",
            "yang_runtime_readiness_checks_total",
            "yang_runtime_readiness_duration_seconds",
            "yang_runtime_readiness_ready",
            "yang_runtime_readiness_resource_healthy",
            "yang_runtime_resource_pool_connections",
        )
    }
}

/// 位于 Addon 中间件链最外层，统一观察认证、租户解析和 Handler 的最终结果。
#[derive(Debug, Clone)]
pub struct ActionLogMiddleware {
    identity: LogIdentity,
}

impl ActionLogMiddleware {
    pub fn new(identity: LogIdentity) -> Self {
        Self { identity }
    }
}

#[async_trait]
impl Middleware for ActionLogMiddleware {
    async fn handle(
        &self,
        context: ActionContext,
        next: Next<'_>,
    ) -> Result<ApiResponse, BaseError> {
        let request_id = context.request_id();
        let operation = context
            .dispatch_target()
            .map(|(module, action)| format!("{module}.{action}"))
            .unwrap_or_else(|| "unknown.unknown".to_string());
        let request_span = tracing::info_span!(
            "action.request",
            operation = %operation,
            %request_id,
            result = tracing::field::Empty,
            error_code = tracing::field::Empty,
            duration_ms = tracing::field::Empty
        );
        let parent = opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.extract(&RequestHeaderExtractor(&context.request.headers))
        });
        if parent.span().span_context().is_valid() {
            request_span.set_parent(parent);
        }

        async move {
            let started = Instant::now();
            let result = next.run(context).await;
            let elapsed = started.elapsed();
            let duration_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
            let span = tracing::Span::current();
            span.record("duration_ms", duration_ms);

            let result_label = match &result {
                Ok(response) if response.code == 0 => {
                    span.record("result", "success");
                    span.record("error_code", 0);
                    tracing::info!(
                        service = %self.identity.service,
                        version = %self.identity.version,
                        environment = %self.identity.environment,
                        operation = %operation,
                        %request_id,
                        result = "success",
                        error_code = 0,
                        error = "",
                        duration_ms,
                        "Action 执行完成"
                    );
                    "success"
                }
                Ok(response) => {
                    span.record("result", "business_error");
                    span.record("error_code", response.code);
                    tracing::warn!(
                        service = %self.identity.service,
                        version = %self.identity.version,
                        environment = %self.identity.environment,
                        operation = %operation,
                        %request_id,
                        result = "business_error",
                        error_code = response.code,
                        error = %response.message,
                        duration_ms,
                        "Action 执行完成"
                    );
                    "business_error"
                }
                Err(error) => {
                    let error_code = error.code();
                    span.record("result", "error");
                    span.record("error_code", error_code);
                    tracing::warn!(
                        service = %self.identity.service,
                        version = %self.identity.version,
                        environment = %self.identity.environment,
                        operation = %operation,
                        %request_id,
                        result = "error",
                        error_code,
                        error = %error,
                        duration_ms,
                        "Action 执行完成"
                    );
                    "error"
                }
            };
            metrics::counter!(
                self.identity.metric_names.action_requests,
                "operation" => operation.clone(),
                "result" => result_label
            )
            .increment(1);
            metrics::histogram!(
                self.identity.metric_names.action_duration,
                "operation" => operation,
                "result" => result_label
            )
            .record(elapsed.as_secs_f64());
            result
        }
        .instrument(request_span)
        .await
    }
}

struct RequestHeaderExtractor<'a>(&'a HashMap<String, String>);

impl Extractor for RequestHeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(String::as_str).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::propagation::TextMapPropagator;
    use opentelemetry_sdk::propagation::TraceContextPropagator;

    #[test]
    fn service_identity_is_stable_and_has_a_safe_unconfigured_fallback() {
        let identity = LogIdentity::new("system-api", "1.2.3", "test");
        assert_eq!(identity.service, "system-api");
        assert_eq!(identity.version, "1.2.3");
        assert_eq!(identity.environment, "test");

        let tools = yang_base::tools::ToolsBuilder::new()
            .build()
            .unwrap_or_else(|error| panic!("测试 Tools 应构建成功: {error}"));
        let fallback = LogIdentity::from_tools(&tools);
        assert_eq!(fallback.service, UNKNOWN_ENVIRONMENT);
        assert_eq!(fallback.environment, UNKNOWN_ENVIRONMENT);
    }

    #[test]
    fn request_headers_preserve_w3c_trace_context_without_logging_values() {
        let headers = HashMap::from([(
            "traceparent".to_string(),
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".to_string(),
        )]);
        let context = TraceContextPropagator::new().extract(&RequestHeaderExtractor(&headers));
        let span_context = context.span().span_context().clone();

        assert!(span_context.is_remote());
        assert_eq!(
            span_context.trace_id().to_string(),
            "4bf92f3577b34da6a3ce929d0e0e4736"
        );
    }
}
