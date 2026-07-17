//! transport-axum 适配器集成测试。
//!
//! 用 `tower::ServiceExt::oneshot` 直接驱动 Router，不开真实端口。
//! 文件下载/预览测试在临时目录写临时文件。
#![cfg(feature = "transport-axum")]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use axum::body::{to_bytes, Body};
use axum::extract::ConnectInfo;
use axum::http::{Request as HttpRequest, StatusCode};
use axum::response::Response;
use axum::Router;
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tower::ServiceExt;
use yang_base::action::{
    Action as BusinessAction, ActionContext, ResponseBody, UiCatalogAction, UploadedFile,
};
use yang_base::definition::{
    AddonName, AddonSpec, AppBuilder, BuiltApp, ModuleName, ModuleSpec, ParamInput, Params,
    UI_SCHEMA_VERSION,
};
use yang_base::tools::ToolsBuilder;
use yang_base::transport::axum::{router, AxumTransportConfig, CorsConfig};
use yang_base::{Action, BaseError};

// ---------------------------------------------------------------------------
// 测试 Action 定义
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct EchoInput {
    value: serde_json::Value,
}

impl ParamInput for EchoInput {
    fn params() -> Params {
        Params::new()
    }
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct EchoOutput {
    value: serde_json::Value,
    request_id: String,
}

/// 回显输入并返回 dispatch 使用的 request_id；success_status 201 用于断言。
#[derive(Action)]
#[action(
    name = "echo",
    display_name = "回显",
    method = "POST",
    path = "/api/test/echo",
    success_status = 201,
    public
)]
struct EchoAction;

#[async_trait::async_trait]
impl BusinessAction for EchoAction {
    type Input = EchoInput;
    type Output = EchoOutput;

    async fn index(
        &self,
        ctx: ActionContext,
        input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Ok(EchoOutput {
            value: input.value,
            request_id: ctx.request_id().to_string(),
        })
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct EmptyInput {}

impl ParamInput for EmptyInput {
    fn params() -> Params {
        Params::new()
    }
}

/// 返回客户端错误的 Action（400，message 对外可见）。
#[derive(Action)]
#[action(
    name = "fail",
    display_name = "失败",
    method = "POST",
    path = "/api/test/fail",
    public
)]
struct FailAction;

#[async_trait::async_trait]
impl BusinessAction for FailAction {
    type Input = EmptyInput;
    type Output = serde_json::Value;

    async fn index(
        &self,
        _ctx: ActionContext,
        _input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Err(BaseError::ParamInvalid(
            "title".to_string(),
            "标题不能为空".to_string(),
        ))
    }
}

/// 返回服务端错误的 Action（500，message 对外遮蔽）。
#[derive(Action)]
#[action(
    name = "crash",
    display_name = "崩溃",
    method = "POST",
    path = "/api/test/crash",
    public
)]
struct CrashAction;

#[async_trait::async_trait]
impl BusinessAction for CrashAction {
    type Input = EmptyInput;
    type Output = serde_json::Value;

    async fn index(
        &self,
        _ctx: ActionContext,
        _input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Err(BaseError::Unknown(
            "数据库连接池耗尽 secret-detail".to_string(),
        ))
    }
}

/// 文件下载 Action。
#[derive(Action)]
#[action(
    name = "download",
    display_name = "下载",
    method = "GET",
    path = "/api/test/download",
    public
)]
struct DownloadAction {
    path: PathBuf,
}

#[async_trait::async_trait]
impl BusinessAction for DownloadAction {
    type Input = EmptyInput;
    type Output = ResponseBody;

    async fn index(
        &self,
        _ctx: ActionContext,
        _input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Ok(ResponseBody::download(self.path.clone(), "报告 文件.bin"))
    }
}

/// 文件预览 Action。
#[derive(Action)]
#[action(
    name = "preview",
    display_name = "预览",
    method = "GET",
    path = "/api/test/preview",
    public
)]
struct PreviewAction {
    path: PathBuf,
}

#[async_trait::async_trait]
impl BusinessAction for PreviewAction {
    type Input = EmptyInput;
    type Output = ResponseBody;

    async fn index(
        &self,
        _ctx: ActionContext,
        _input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Ok(ResponseBody::preview(self.path.clone()))
    }
}

/// 重定向 Action。
#[derive(Action)]
#[action(
    name = "redirect",
    display_name = "重定向",
    method = "GET",
    path = "/api/test/redirect",
    public
)]
struct RedirectAction;

#[async_trait::async_trait]
impl BusinessAction for RedirectAction {
    type Input = EmptyInput;
    type Output = ResponseBody;

    async fn index(
        &self,
        _ctx: ActionContext,
        _input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Ok(ResponseBody::redirect("https://example.com/next"))
    }
}

/// 下载不存在文件的 Action。
#[derive(Action)]
#[action(
    name = "missing",
    display_name = "缺失文件",
    method = "GET",
    path = "/api/test/missing",
    public
)]
struct MissingFileAction;

#[async_trait::async_trait]
impl BusinessAction for MissingFileAction {
    type Input = EmptyInput;
    type Output = ResponseBody;

    async fn index(
        &self,
        _ctx: ActionContext,
        _input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Ok(ResponseBody::download(
            "/definitely/not/exist/yang-transport-axum-missing.bin",
            "missing.bin",
        ))
    }
}

/// 慢 Action（配合超时层测试）。
#[derive(Action)]
#[action(
    name = "slow",
    display_name = "慢查询",
    method = "GET",
    path = "/api/test/slow",
    public
)]
struct SlowAction;

#[async_trait::async_trait]
impl BusinessAction for SlowAction {
    type Input = EmptyInput;
    type Output = serde_json::Value;

    async fn index(
        &self,
        _ctx: ActionContext,
        _input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        tokio::time::sleep(Duration::from_secs(2)).await;
        Ok(serde_json::json!({"slow": true}))
    }
}

/// 未认证时不得出现在 UI 目录中的受保护 Action。
#[derive(Action)]
#[action(
    name = "protected",
    display_name = "受保护操作",
    method = "GET",
    path = "/api/test/protected"
)]
struct ProtectedAction;

#[async_trait::async_trait]
impl BusinessAction for ProtectedAction {
    type Input = EmptyInput;
    type Output = serde_json::Value;

    async fn index(
        &self,
        _ctx: ActionContext,
        _input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Ok(serde_json::json!({"protected": true}))
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct UploadInput {
    title: String,
    file: UploadedFile,
}

impl ParamInput for UploadInput {
    fn params() -> Params {
        Params::new()
    }
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct UploadOutput {
    title: String,
    field_name: String,
    filename: String,
    content_type: String,
    size: u64,
    content: String,
}

#[derive(Action)]
#[action(
    name = "upload",
    display_name = "上传",
    method = "POST",
    path = "/api/upload/file",
    public,
    request_media = "multipart",
    content_types("text/plain"),
    max_fields = 1,
    max_files = 1,
    max_file_bytes = 8,
    max_total_bytes = 1024
)]
struct UploadAction {
    observed_path: Arc<Mutex<Option<PathBuf>>>,
}

#[async_trait::async_trait]
impl BusinessAction for UploadAction {
    type Input = UploadInput;
    type Output = UploadOutput;

    async fn index(
        &self,
        _ctx: ActionContext,
        input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        let path = input.file.path().to_path_buf();
        let content = tokio::fs::read_to_string(&path).await?;
        *self.observed_path.lock().expect("上传路径锁不应中毒") = Some(path);
        if input.title == "fail" {
            return Err(BaseError::ParamInvalid(
                "title".to_string(),
                "测试 Handler 失败".to_string(),
            ));
        }
        Ok(UploadOutput {
            title: input.title,
            field_name: input.file.field_name().to_string(),
            filename: input.file.original_filename().to_string(),
            content_type: input.file.content_type().to_string(),
            size: input.file.size(),
            content,
        })
    }
}

// ---------------------------------------------------------------------------
// 测试辅助
// ---------------------------------------------------------------------------

fn build_app() -> Arc<BuiltApp> {
    let download = temp_file("download.bin", b"hello-download-bytes");
    let preview = temp_file("preview.txt", b"hello-preview-bytes");
    let module = ModuleSpec::new(ModuleName::new("test.probe").expect("模块名应有效"))
        .native_action(EchoAction)
        .native_action(FailAction)
        .native_action(CrashAction)
        .native_action(DownloadAction { path: download })
        .native_action(PreviewAction { path: preview })
        .native_action(RedirectAction)
        .native_action(MissingFileAction)
        .native_action(SlowAction)
        .native_action(ProtectedAction)
        .native_action(UiCatalogAction);
    let tools = Arc::new(ToolsBuilder::new().build().expect("空 Tools 应构建成功"));
    Arc::new(
        AppBuilder::new()
            .addon(AddonSpec::new(AddonName::new("test").expect("Addon 名应有效")).module(module))
            .build(tools)
            .expect("测试应用应构建成功"),
    )
}

fn temp_file(name: &str, content: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("yang_transport_axum_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("临时目录应创建成功");
    let path = dir.join(name);
    std::fs::write(&path, content).expect("临时文件应写入成功");
    path
}

fn peer() -> ConnectInfo<SocketAddr> {
    ConnectInfo("127.0.0.1:50000".parse().expect("测试对端地址应有效"))
}

async fn oneshot(router: Router, request: HttpRequest<Body>) -> Response {
    let mut request = request;
    request.extensions_mut().insert(peer());
    router.oneshot(request).await.expect("请求应返回响应")
}

fn json_request(method: &str, uri: &str, body: &str) -> HttpRequest<Body> {
    HttpRequest::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("测试请求应构建成功")
}

async fn body_json(response: Response) -> serde_json::Value {
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .expect("响应体应可读取");
    serde_json::from_slice(&bytes).expect("响应体应为 JSON")
}

async fn body_bytes(response: Response) -> Vec<u8> {
    to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .expect("响应体应可读取")
        .to_vec()
}

fn default_router() -> Router {
    router(build_app(), AxumTransportConfig::default()).expect("Router 应构建成功")
}

fn build_upload_app() -> (Arc<BuiltApp>, Arc<Mutex<Option<PathBuf>>>) {
    let observed_path = Arc::new(Mutex::new(None));
    let module = ModuleSpec::new(ModuleName::new("upload.file").expect("模块名应有效"))
        .native_action(UploadAction {
            observed_path: Arc::clone(&observed_path),
        });
    let tools = Arc::new(ToolsBuilder::new().build().expect("空 Tools 应构建成功"));
    let app = Arc::new(
        AppBuilder::new()
            .addon(AddonSpec::new(AddonName::new("upload").expect("Addon 名应有效")).module(module))
            .build(tools)
            .expect("上传测试应用应构建成功"),
    );
    (app, observed_path)
}

fn multipart_payload(
    boundary: &str,
    text_parts: &[(&str, &str)],
    file_parts: &[(&str, &str, &str, &[u8])],
) -> Vec<u8> {
    let mut body = Vec::new();
    for (name, value) in text_parts {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    for (name, filename, content_type, content) in file_parts {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\nContent-Type: {content_type}\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(content);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

fn multipart_request(boundary: &str, body: Vec<u8>) -> HttpRequest<Body> {
    HttpRequest::builder()
        .method("POST")
        .uri("/api/upload/file")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .expect("multipart 测试请求应构建成功")
}

// ---------------------------------------------------------------------------
// 健康端点
// ---------------------------------------------------------------------------

#[tokio::test]
async fn health_live_returns_200() {
    let response = oneshot(default_router(), json_request("GET", "/health/live", "")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["code"], 0);
    assert_eq!(json["data"]["status"], "live");
}

#[tokio::test]
async fn health_ready_returns_200_with_empty_tools() {
    let response = oneshot(default_router(), json_request("GET", "/health/ready", "")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["code"], 0);
    assert_eq!(json["data"]["status"], "ready");
}

// ---------------------------------------------------------------------------
// 请求级 UI 目录
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ui_catalog_endpoint_projects_only_anonymous_accessible_actions() {
    let response = oneshot(
        default_router(),
        json_request("GET", "/.well-known/yang/ui-catalog", ""),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["code"], 0);
    assert_eq!(json["data"]["schema_version"], UI_SCHEMA_VERSION);

    let operation_ids = json["data"]["actions"]
        .as_array()
        .expect("actions 应为数组")
        .iter()
        .filter_map(|action| action["operation_id"].as_str())
        .collect::<Vec<_>>();
    assert!(operation_ids.contains(&"test.probe.echo"));
    assert!(operation_ids.contains(&"test.probe.ui_catalog"));
    assert!(!operation_ids.contains(&"test.probe.protected"));
}

#[tokio::test]
async fn ui_catalog_endpoint_rejects_wrong_method_and_direct_protected_call() {
    let wrong_method = oneshot(
        default_router(),
        json_request("POST", "/.well-known/yang/ui-catalog", "{}"),
    )
    .await;
    assert_eq!(wrong_method.status(), StatusCode::METHOD_NOT_ALLOWED);

    let protected = oneshot(
        default_router(),
        json_request("GET", "/api/test/protected", ""),
    )
    .await;
    assert_eq!(protected.status(), StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// Catalog 路由与成功响应
// ---------------------------------------------------------------------------

#[tokio::test]
async fn catalog_route_uses_action_success_status() {
    let response = oneshot(
        default_router(),
        json_request("POST", "/api/test/echo", r#"{"value": 42}"#),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let json = body_json(response).await;
    assert_eq!(json["code"], 0);
    assert_eq!(json["data"]["value"], 42);
}

#[tokio::test]
async fn invalid_json_body_returns_400_before_dispatch() {
    let response = oneshot(
        default_router(),
        json_request("POST", "/api/test/echo", "not-json"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json = body_json(response).await;
    assert_eq!(json["code"], 700005);
}

#[tokio::test]
async fn oversized_body_returns_413() {
    let app = build_app();
    let config = AxumTransportConfig {
        max_body_bytes: 16,
        ..AxumTransportConfig::default()
    };
    let router = router(app, config).expect("Router 应构建成功");
    let body = format!(r#"{{"value": "{}"}}"#, "x".repeat(128));
    let response = oneshot(router, json_request("POST", "/api/test/echo", &body)).await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let json = body_json(response).await;
    assert_eq!(json["code"], 700005);
}

#[tokio::test]
async fn invalid_query_encoding_returns_400() {
    // %FF 百分号解码后不是合法 UTF-8，serde_urlencoded 应拒绝
    let response = oneshot(
        default_router(),
        json_request("POST", "/api/test/echo?a=%FF", "{}"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json = body_json(response).await;
    assert_eq!(json["code"], 700005);
}

// ---------------------------------------------------------------------------
// 错误映射与 5xx 遮蔽
// ---------------------------------------------------------------------------

#[tokio::test]
async fn client_error_keeps_message_and_maps_400() {
    let response = oneshot(
        default_router(),
        json_request("POST", "/api/test/fail", "{}"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json = body_json(response).await;
    assert_eq!(json["code"], 700005);
    assert!(
        json["message"]
            .as_str()
            .unwrap_or_default()
            .contains("标题不能为空"),
        "客户端错误 message 应对外可见: {json}"
    );
}

#[tokio::test]
async fn server_error_masks_message_and_maps_500() {
    let response = oneshot(
        default_router(),
        json_request("POST", "/api/test/crash", "{}"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let json = body_json(response).await;
    assert_eq!(json["code"], 999999);
    assert_eq!(json["message"], "服务器内部错误");
    assert!(
        !json.to_string().contains("secret-detail"),
        "5xx 响应不得泄漏内部细节: {json}"
    );
}

// ---------------------------------------------------------------------------
// request_id 透传
// ---------------------------------------------------------------------------

#[tokio::test]
async fn request_id_is_forwarded_from_header() {
    let mut request = json_request("POST", "/api/test/echo", r#"{"value": 1}"#);
    request
        .headers_mut()
        .insert("x-request-id", "c0ffee".parse().unwrap());
    let response = oneshot(default_router(), request).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let json = body_json(response).await;
    assert_eq!(
        json["data"]["request_id"], "00000000000000000000000000c0ffee",
        "dispatch 应沿用上游 x-request-id"
    );
}

#[tokio::test]
async fn request_id_is_generated_when_header_absent() {
    let response = oneshot(
        default_router(),
        json_request("POST", "/api/test/echo", r#"{"value": 1}"#),
    )
    .await;
    let json = body_json(response).await;
    let id = json["data"]["request_id"].as_str().unwrap_or_default();
    assert_eq!(id.len(), 32, "request_id 应为 32 位十六进制: {id}");
    assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    assert_ne!(id, "00000000000000000000000000000000");
}

// ---------------------------------------------------------------------------
// 附件响应：下载 / 预览 / 重定向
// ---------------------------------------------------------------------------

#[tokio::test]
async fn download_serves_file_with_attachment_disposition() {
    let response = oneshot(
        default_router(),
        json_request("GET", "/api/test/download", ""),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let disposition = response
        .headers()
        .get("content-disposition")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(
        disposition.starts_with("attachment;"),
        "下载应使用 attachment 处置: {disposition}"
    );
    assert!(
        disposition.contains("filename"),
        "下载应包含 filename 参数: {disposition}"
    );
    assert_eq!(body_bytes(response).await, b"hello-download-bytes");
}

#[tokio::test]
async fn preview_serves_file_inline_with_mime() {
    let response = oneshot(
        default_router(),
        json_request("GET", "/api/test/preview", ""),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let disposition = response
        .headers()
        .get("content-disposition")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert_eq!(disposition, "inline");
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert_eq!(content_type, "text/plain");
    assert_eq!(body_bytes(response).await, b"hello-preview-bytes");
}

#[tokio::test]
async fn redirect_returns_302_with_location() {
    let response = oneshot(
        default_router(),
        json_request("GET", "/api/test/redirect", ""),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FOUND);
    let location = response
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert_eq!(location, "https://example.com/next");
}

#[tokio::test]
async fn missing_file_returns_404_structured_error() {
    let response = oneshot(
        default_router(),
        json_request("GET", "/api/test/missing", ""),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = body_json(response).await;
    assert_eq!(json["code"], 700006);
    assert!(
        json["message"]
            .as_str()
            .unwrap_or_default()
            .contains("文件不存在"),
        "404 message 应说明文件不存在: {json}"
    );
}

// ---------------------------------------------------------------------------
// CORS
// ---------------------------------------------------------------------------

fn cors_router(cors: CorsConfig) -> Result<Router, BaseError> {
    router(
        build_app(),
        AxumTransportConfig {
            cors,
            ..AxumTransportConfig::default()
        },
    )
}

fn preflight(origin: &str) -> HttpRequest<Body> {
    HttpRequest::builder()
        .method("OPTIONS")
        .uri("/api/test/echo")
        .header("origin", origin)
        .header("access-control-request-method", "POST")
        .body(Body::empty())
        .expect("预检请求应构建成功")
}

#[tokio::test]
async fn cors_preflight_allows_configured_origin() {
    let router = cors_router(CorsConfig {
        origins: vec!["https://app.example.com".to_string()],
        ..CorsConfig::default()
    })
    .expect("Router 应构建成功");
    let response = oneshot(router, preflight("https://app.example.com")).await;
    let allow_origin = response
        .headers()
        .get("access-control-allow-origin")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert_eq!(allow_origin, "https://app.example.com");
}

#[tokio::test]
async fn cors_preflight_denies_unlisted_origin() {
    let router = cors_router(CorsConfig {
        origins: vec!["https://app.example.com".to_string()],
        ..CorsConfig::default()
    })
    .expect("Router 应构建成功");
    let response = oneshot(router, preflight("https://evil.example.com")).await;
    assert!(
        response
            .headers()
            .get("access-control-allow-origin")
            .is_none(),
        "未列入白名单的 Origin 不得获得 CORS 许可"
    );
}

#[tokio::test]
async fn cors_credentials_with_explicit_origin_is_allowed() {
    let router = cors_router(CorsConfig {
        origins: vec!["https://app.example.com".to_string()],
        credentials: true,
        ..CorsConfig::default()
    })
    .expect("显式 origins + credentials 应构建成功");
    let response = oneshot(router, preflight("https://app.example.com")).await;
    let allow_credentials = response
        .headers()
        .get("access-control-allow-credentials")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert_eq!(allow_credentials, "true");
}

#[tokio::test]
async fn cors_wildcard_origin_with_credentials_is_rejected_at_build() {
    let result = cors_router(CorsConfig {
        origins: vec!["*".to_string()],
        credentials: true,
        ..CorsConfig::default()
    });
    assert!(
        matches!(result, Err(BaseError::ConfigError(_))),
        "通配 origins 与 credentials 组合必须在构建期被拒绝"
    );
}

// ---------------------------------------------------------------------------
// 超时与压缩
// ---------------------------------------------------------------------------

#[tokio::test]
async fn timeout_layer_returns_408_for_slow_action() {
    let router = router(
        build_app(),
        AxumTransportConfig {
            request_timeout: Some(Duration::from_millis(50)),
            ..AxumTransportConfig::default()
        },
    )
    .expect("Router 应构建成功");
    let response = oneshot(router, json_request("GET", "/api/test/slow", "")).await;
    assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
}

#[tokio::test]
async fn compression_encodes_large_json_when_enabled() {
    let large = "x".repeat(2048);
    let body = format!(r#"{{"value": "{large}"}}"#);
    let mut request = json_request("POST", "/api/test/echo", &body);
    request
        .headers_mut()
        .insert("accept-encoding", "gzip".parse().unwrap());
    let response = oneshot(default_router(), request).await;
    let encoding = response
        .headers()
        .get("content-encoding")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert_eq!(encoding, "gzip", "开启压缩时大 JSON 响应应被 gzip 编码");
}

#[tokio::test]
async fn compression_disabled_leaves_response_unencoded() {
    let router = router(
        build_app(),
        AxumTransportConfig {
            compression: false,
            ..AxumTransportConfig::default()
        },
    )
    .expect("Router 应构建成功");
    let large = "x".repeat(2048);
    let body = format!(r#"{{"value": "{large}"}}"#);
    let mut request = json_request("POST", "/api/test/echo", &body);
    request
        .headers_mut()
        .insert("accept-encoding", "gzip".parse().unwrap());
    let response = oneshot(router, request).await;
    assert!(response.headers().get("content-encoding").is_none());
}

// ---------------------------------------------------------------------------
// 受限 multipart 与请求作用域临时文件
// ---------------------------------------------------------------------------

#[tokio::test]
async fn multipart_streams_to_generated_temp_file_and_cleans_after_success() {
    let (app, observed_path) = build_upload_app();
    let router = router(app, AxumTransportConfig::default()).expect("上传 Router 应构建成功");
    let boundary = "yang-boundary-success";
    let body = multipart_payload(
        boundary,
        &[("title", "document")],
        &[("file", "../../evil.txt", "text/plain", b"hello")],
    );
    let response = oneshot(router, multipart_request(boundary, body)).await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["data"]["title"], "document");
    assert_eq!(json["data"]["field_name"], "file");
    assert_eq!(json["data"]["filename"], "evil.txt");
    assert_eq!(json["data"]["content_type"], "text/plain");
    assert_eq!(json["data"]["size"], 5);
    assert_eq!(json["data"]["content"], "hello");

    let path = observed_path
        .lock()
        .expect("上传路径锁不应中毒")
        .clone()
        .expect("Handler 应观察到临时文件");
    assert_ne!(
        path.file_name().and_then(|name| name.to_str()),
        Some("evil.txt"),
        "客户端文件名不得参与临时路径生成"
    );
    assert!(!path.exists(), "Handler 返回后请求作用域临时文件必须清理");
}

#[tokio::test]
async fn multipart_cleans_temp_file_when_handler_returns_error() {
    let (app, observed_path) = build_upload_app();
    let router = router(app, AxumTransportConfig::default()).expect("上传 Router 应构建成功");
    let boundary = "yang-boundary-handler-error";
    let body = multipart_payload(
        boundary,
        &[("title", "fail")],
        &[("file", "report.txt", "text/plain", b"hello")],
    );
    let response = oneshot(router, multipart_request(boundary, body)).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let path = observed_path
        .lock()
        .expect("上传路径锁不应中毒")
        .clone()
        .expect("失败 Handler 也应观察到临时文件");
    assert!(!path.exists(), "Handler 失败后临时文件也必须清理");
}

#[tokio::test]
async fn multipart_rejects_wrong_media_oversized_and_excess_parts_before_dispatch() {
    let oversized_text = "x".repeat(1_200);
    let cases = [
        (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            multipart_payload(
                "wrong-type",
                &[("title", "document")],
                &[("file", "report.bin", "application/octet-stream", b"hello")],
            ),
            "wrong-type",
        ),
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            multipart_payload(
                "oversized",
                &[("title", "document")],
                &[("file", "report.txt", "text/plain", b"123456789")],
            ),
            "oversized",
        ),
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            multipart_payload(
                "too-many-files",
                &[("title", "document")],
                &[
                    ("file", "one.txt", "text/plain", b"one"),
                    ("file", "two.txt", "text/plain", b"two"),
                ],
            ),
            "too-many-files",
        ),
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            multipart_payload(
                "too-many-fields",
                &[("title", "document"), ("extra", "value")],
                &[("file", "report.txt", "text/plain", b"hello")],
            ),
            "too-many-fields",
        ),
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            multipart_payload("raw-total", &[("title", &oversized_text)], &[]),
            "raw-total",
        ),
    ];

    for (expected, body, boundary) in cases {
        let (app, observed_path) = build_upload_app();
        let router = router(app, AxumTransportConfig::default()).expect("上传 Router 应构建成功");
        let response = oneshot(router, multipart_request(boundary, body)).await;
        assert_eq!(response.status(), expected, "case={boundary}");
        assert!(
            observed_path.lock().expect("上传路径锁不应中毒").is_none(),
            "拒绝请求不得进入 Handler: {boundary}"
        );
    }
}

#[tokio::test]
async fn multipart_rejects_json_malformed_and_mixed_same_name_parts() {
    let (app, _) = build_upload_app();
    let upload_router =
        router(app, AxumTransportConfig::default()).expect("上传 Router 应构建成功");
    let json = oneshot(
        upload_router,
        json_request("POST", "/api/upload/file", r#"{"title":"x"}"#),
    )
    .await;
    assert_eq!(json.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

    let (app, _) = build_upload_app();
    let upload_router =
        router(app, AxumTransportConfig::default()).expect("上传 Router 应构建成功");
    let malformed = HttpRequest::builder()
        .method("POST")
        .uri("/api/upload/file")
        .header(
            "content-type",
            "multipart/form-data; boundary=missing-boundary",
        )
        .body(Body::from("not-a-multipart-body"))
        .expect("畸形 multipart 请求应构建成功");
    let malformed = oneshot(upload_router, malformed).await;
    assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);

    let boundary = "mixed-parts";
    let mixed = multipart_payload(
        boundary,
        &[("file", "forged")],
        &[("file", "report.txt", "text/plain", b"hello")],
    );
    let (app, _) = build_upload_app();
    let upload_router =
        router(app, AxumTransportConfig::default()).expect("上传 Router 应构建成功");
    let mixed = oneshot(upload_router, multipart_request(boundary, mixed)).await;
    assert_eq!(mixed.status(), StatusCode::BAD_REQUEST);
}

#[test]
fn multipart_router_rejects_global_body_limit_below_action_contract() {
    let (app, _) = build_upload_app();
    let result = router(
        app,
        AxumTransportConfig {
            max_body_bytes: 512,
            ..AxumTransportConfig::default()
        },
    );
    assert!(
        matches!(result, Err(BaseError::ConfigError(message)) if message.contains("max_total_bytes")),
        "传输层不得静默收紧已投影给客户端的 Action 上限"
    );
}
