//! 独立进程验证入站 tracing/metrics，隔离其他测试的线程局部 subscriber。
#![cfg(all(feature = "transport-axum", feature = "metrics"))]
#![allow(clippy::expect_used)]

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request as HttpRequest, StatusCode},
    response::Response,
    Router,
};
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};
use tower::ServiceExt;
use yang_base::{
    action::ApiResponse,
    definition::{
        AddonName, AddonSpec, AppBuilder, BuiltApp, HttpMethod, ModuleName, ModuleSpec, ParamInput,
        Params,
    },
    tools::ToolsBuilder,
    transport::axum::{router, AxumTransportConfig},
    BaseError,
};

#[derive(serde::Deserialize, schemars::JsonSchema)]
struct EmptyInput {}
impl ParamInput for EmptyInput {
    fn params() -> Params {
        Params::new()
    }
}

fn build_app() -> Arc<BuiltApp> {
    let module = ModuleSpec::new(ModuleName::new("test.inbound").expect("模块"))
        .action_fn(yang_base::action_name!("echo"), |_, _: EmptyInput| async {
            Ok::<_, BaseError>(ApiResponse::success_value(serde_json::json!({}), "成功"))
        })
        .route(HttpMethod::Post, "/api/test/echo")
        .public()
        .register()
        .action_fn(yang_base::action_name!("slow"), |_, _: EmptyInput| async {
            tokio::time::sleep(Duration::from_secs(1)).await;
            Ok::<_, BaseError>(ApiResponse::success_value(serde_json::json!({}), "成功"))
        })
        .route(HttpMethod::Get, "/api/test/slow")
        .public()
        .register();
    Arc::new(
        AppBuilder::new()
            .addon(AddonSpec::new(AddonName::new("test").expect("Addon")).module(module))
            .build(Arc::new(ToolsBuilder::new().build().expect("Tools")))
            .expect("App"),
    )
}
fn json_request(method: &str, path: &str, body: &str) -> HttpRequest<Body> {
    HttpRequest::builder()
        .method(method)
        .uri(path)
        .body(Body::from(body.to_string()))
        .expect("请求")
}
async fn oneshot(router: Router, mut request: HttpRequest<Body>) -> Response {
    request.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:50000".parse::<SocketAddr>().expect("peer"),
    ));
    router.oneshot(request).await.expect("响应")
}
struct PendingInboundBody(Arc<tokio::sync::Notify>);
impl axum::body::HttpBody for PendingInboundBody {
    type Data = axum::body::Bytes;
    type Error = axum::BoxError;
    fn poll_frame(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        self.0.notify_one();
        std::task::Poll::Pending
    }
}

mod inbound_observability {
    use super::*;
    use metrics::{
        Counter, CounterFn, Gauge, Histogram, HistogramFn, Key, KeyName, Metadata, Recorder,
        SharedString, Unit,
    };
    use std::io::Write;

    #[derive(Clone, Default)]
    struct Capture(Arc<Mutex<Vec<u8>>>);
    impl Write for Capture {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().expect("日志锁").write(bytes)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Capture {
        type Writer = Self;
        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    #[derive(Default)]
    struct MetricCapture(Arc<Mutex<Vec<Key>>>);
    struct Observation {
        key: Key,
        values: Arc<Mutex<Vec<Key>>>,
    }
    impl CounterFn for Observation {
        fn increment(&self, value: u64) {
            assert_eq!(value, 1);
            self.values.lock().expect("指标锁").push(self.key.clone());
        }
        fn absolute(&self, _: u64) {
            panic!("入站计数只允许增量")
        }
    }
    impl HistogramFn for Observation {
        fn record(&self, value: f64) {
            assert!(value >= 0.0 && value.is_finite());
            self.values.lock().expect("指标锁").push(self.key.clone());
        }
    }
    impl Recorder for MetricCapture {
        fn describe_counter(&self, _: KeyName, _: Option<Unit>, _: SharedString) {}
        fn describe_gauge(&self, _: KeyName, _: Option<Unit>, _: SharedString) {}
        fn describe_histogram(&self, _: KeyName, _: Option<Unit>, _: SharedString) {}
        fn register_counter(&self, key: &Key, _: &Metadata<'_>) -> Counter {
            Counter::from_arc(Arc::new(Observation {
                key: key.clone(),
                values: self.0.clone(),
            }))
        }
        fn register_gauge(&self, _: &Key, _: &Metadata<'_>) -> Gauge {
            Gauge::noop()
        }
        fn register_histogram(&self, key: &Key, _: &Metadata<'_>) -> Histogram {
            Histogram::from_arc(Arc::new(Observation {
                key: key.clone(),
                values: self.0.clone(),
            }))
        }
    }

    #[tokio::test(start_paused = true)]
    async fn inbound_logs_and_metrics_cover_all_exit_paths_without_raw_uri() {
        let logs = Capture::default();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_writer(logs.clone())
            .with_ansi(false)
            .finish();
        let _trace_guard = tracing::subscriber::set_default(subscriber);
        let metrics = MetricCapture::default();
        let _metric_guard = metrics::set_default_local_recorder(&metrics);
        let router = router(
            build_app(),
            AxumTransportConfig {
                max_body_bytes: 32,
                max_concurrency: Some(1),
                request_timeout: Some(Duration::from_millis(10)),
                ..AxumTransportConfig::default()
            },
        )
        .expect("Router");
        for (method, path, body, status) in [
            ("GET", "/health/live?token=secret-in-uri", "", 200),
            ("GET", "/secret-in-uri?token=secret-in-uri", "", 404),
            ("PATCH", "/api/test/echo?token=secret-in-uri", "", 405),
            (
                "POST",
                "/api/test/echo?token=secret-in-uri",
                "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
                413,
            ),
            ("GET", "/api/test/slow?token=secret-in-uri", "", 408),
        ] {
            assert_eq!(
                oneshot(router.clone(), json_request(method, path, body))
                    .await
                    .status()
                    .as_u16(),
                status
            );
        }
        let entered = Arc::new(tokio::sync::Notify::new());
        let blocked = HttpRequest::builder()
            .method("POST")
            .uri("/api/test/echo?token=secret-in-uri")
            .body(Body::new(PendingInboundBody(entered.clone())))
            .expect("请求");
        let first = tokio::spawn(oneshot(router.clone(), blocked));
        entered.notified().await;
        assert_eq!(
            oneshot(router, json_request("GET", "/health/live", ""))
                .await
                .status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            first.await.expect("结束").status(),
            StatusCode::REQUEST_TIMEOUT
        );
        let logs = String::from_utf8(logs.0.lock().expect("日志锁").clone()).expect("UTF8");
        assert!(
            !logs.contains("secret-in-uri"),
            "不得记录原始路径/query: {logs}"
        );
        for status in [200, 404, 405, 408, 413, 503] {
            assert!(
                logs.contains(&format!("status={status}")),
                "缺少访问日志 {status}: {logs}"
            );
            for name in [
                "yang_http_inbound_requests_total",
                "yang_http_inbound_request_duration_seconds",
            ] {
                let captured = metrics.0.lock().expect("指标锁");
                let matches = captured
                    .iter()
                    .filter(|key| {
                        key.name() == name
                            && key.labels().any(|label| {
                                label.key() == "status" && label.value() == status.to_string()
                            })
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    matches.len(),
                    if status == 408 { 2 } else { 1 },
                    "{name} status={status}"
                );
                assert!(matches.iter().all(|key| key
                    .labels()
                    .all(|label| !label.value().contains("secret-in-uri"))));
            }
        }
    }
}
