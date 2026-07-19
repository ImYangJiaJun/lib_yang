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
#[cfg(feature = "token")]
use yang_base::action::StepUpChallenge;
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

/// 返回标准限流错误（429，并携带 Retry-After）。
#[derive(Action)]
#[action(
    name = "rate_limited",
    display_name = "限流",
    method = "POST",
    path = "/api/test/rate-limited",
    public
)]
struct RateLimitedAction;

#[async_trait::async_trait]
impl BusinessAction for RateLimitedAction {
    type Input = EmptyInput;
    type Output = serde_json::Value;

    async fn index(
        &self,
        _ctx: ActionContext,
        _input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Err(BaseError::RateLimitExceeded {
            retry_after_seconds: 30,
        })
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

/// 仅用于验证 Axum 的 step-up 结构化错误映射。
#[cfg(feature = "token")]
#[derive(Action)]
#[action(
    name = "step_up_required",
    display_name = "需要重认证",
    method = "POST",
    path = "/api/test/step-up-required",
    public
)]
struct StepUpRequiredAction;

#[cfg(feature = "token")]
#[async_trait::async_trait]
impl BusinessAction for StepUpRequiredAction {
    type Input = EmptyInput;
    type Output = serde_json::Value;

    async fn index(
        &self,
        _ctx: ActionContext,
        _input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Err(BaseError::StepUpRequired(StepUpChallenge {
            challenge: "signed-test-challenge".to_string(),
            expires_in: 120,
        }))
    }
}

/// 文件下载 Action。
#[derive(Action)]
#[action(
    name = "download",
    display_name = "下载",
    method = "GET",
    path = "/api/test/download",
    response_kind = "download",
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
    response_kind = "preview",
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
    response_kind = "redirect",
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
    response_kind = "download",
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

/// 声明 download 却返回重定向的 Action（response_kind 契约缺陷探针）。
#[derive(Action)]
#[action(
    name = "mismatch",
    display_name = "声明不一致",
    method = "GET",
    path = "/api/test/mismatch",
    response_kind = "download",
    public
)]
struct MismatchedAction;

#[async_trait::async_trait]
impl BusinessAction for MismatchedAction {
    type Input = EmptyInput;
    type Output = ResponseBody;

    async fn index(
        &self,
        _ctx: ActionContext,
        _input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Ok(ResponseBody::redirect("https://example.com/elsewhere"))
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
    copied: u64,
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
    max_total_bytes = 131072
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
        // C-1 端到端：transport 构造的受信实例必须能通过 copy_to 的临时根校验。
        let copied = input.file.copy_to(path.with_extension("copy")).await?;
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
            copied,
        })
    }
}

// ---------------------------------------------------------------------------
// C-1 复审 fixture：文件数组 / 包装结构体数组的 temp_root 走私防护
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct MultiUploadInput {
    files: Vec<UploadedFile>,
}

impl ParamInput for MultiUploadInput {
    fn params() -> Params {
        Params::new()
    }
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct MultiUploadOutput {
    count: usize,
    copied: Vec<u64>,
}

#[derive(Action)]
#[action(
    name = "multi_upload",
    display_name = "批量上传",
    method = "POST",
    path = "/api/upload/multi",
    public,
    request_media = "multipart",
    content_types("text/plain"),
    max_fields = 1,
    max_files = 2,
    max_file_bytes = 8,
    max_total_bytes = 131072
)]
struct MultiUploadAction;

#[async_trait::async_trait]
impl BusinessAction for MultiUploadAction {
    type Input = MultiUploadInput;
    type Output = MultiUploadOutput;

    async fn index(
        &self,
        _ctx: ActionContext,
        input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        let mut copied = Vec::new();
        for file in &input.files {
            copied.push(file.copy_to(file.path().with_extension("copy")).await?);
        }
        Ok(MultiUploadOutput {
            count: input.files.len(),
            copied,
        })
    }
}

/// 包装结构体：UploadedFile 经 `$ref` 藏在嵌套定义中，检验扫描器的 $ref 递归解析。
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct UploadBundle {
    note: String,
    file: UploadedFile,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct NestedUploadInput {
    bundles: Vec<UploadBundle>,
}

impl ParamInput for NestedUploadInput {
    fn params() -> Params {
        Params::new()
    }
}

#[derive(Action)]
#[action(
    name = "nested_upload",
    display_name = "嵌套上传",
    method = "POST",
    path = "/api/upload/nested",
    public,
    request_media = "multipart",
    content_types("text/plain"),
    max_fields = 1,
    max_files = 2,
    max_file_bytes = 8,
    max_total_bytes = 131072
)]
struct NestedUploadAction;

#[async_trait::async_trait]
impl BusinessAction for NestedUploadAction {
    type Input = NestedUploadInput;
    type Output = serde_json::Value;

    async fn index(
        &self,
        _ctx: ActionContext,
        input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        let mut copied = Vec::new();
        for bundle in &input.bundles {
            copied.push(
                bundle
                    .file
                    .copy_to(bundle.file.path().with_extension("copy"))
                    .await?,
            );
        }
        let notes: Vec<&str> = input
            .bundles
            .iter()
            .map(|bundle| bundle.note.as_str())
            .collect();
        Ok(serde_json::json!({
            "count": input.bundles.len(),
            "notes": notes,
            "copied": copied,
        }))
    }
}

// ---------------------------------------------------------------------------
// I-9 fixture：anyOf 标量解码 + 文本 part 单字段上限
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ScalarUploadInput {
    title: String,
    count: Option<i64>,
    note: Option<String>,
    file: UploadedFile,
}

impl ParamInput for ScalarUploadInput {
    fn params() -> Params {
        Params::new()
    }
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct ScalarUploadOutput {
    title: String,
    count: Option<i64>,
    note: Option<String>,
    size: u64,
}

#[derive(Action)]
#[action(
    name = "scalar_upload",
    display_name = "标量上传",
    method = "POST",
    path = "/api/upload/scalar",
    public,
    request_media = "multipart",
    content_types("text/plain"),
    max_fields = 3,
    max_files = 1,
    max_file_bytes = 64,
    max_total_bytes = 262144
)]
struct ScalarUploadAction;

#[async_trait::async_trait]
impl BusinessAction for ScalarUploadAction {
    type Input = ScalarUploadInput;
    type Output = ScalarUploadOutput;

    async fn index(
        &self,
        _ctx: ActionContext,
        input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Ok(ScalarUploadOutput {
            title: input.title,
            count: input.count,
            note: input.note,
            size: input.file.size(),
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
        .native_action(RateLimitedAction)
        .native_action(CrashAction)
        .native_action(DownloadAction { path: download })
        .native_action(PreviewAction { path: preview })
        .native_action(RedirectAction)
        .native_action(MissingFileAction)
        .native_action(SlowAction)
        .native_action(ProtectedAction)
        .native_action(UiCatalogAction);
    #[cfg(feature = "token")]
    let module = module.native_action(StepUpRequiredAction);
    let tools = Arc::new(ToolsBuilder::new().build().expect("空 Tools 应构建成功"));
    Arc::new(
        AppBuilder::new()
            .addon(AddonSpec::new(AddonName::new("test").expect("Addon 名应有效")).module(module))
            .build(tools)
            .expect("测试应用应构建成功"),
    )
}

fn temp_file(name: &str, content: &[u8]) -> PathBuf {
    // 并发测试会各自重写 fixture 文件；同名共享路径的 truncate-写入与
    // 文件响应的读取存在竞态（读到空内容），故每次调用使用独立子目录。
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "yang_transport_axum_test_{}_{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
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

fn build_multi_upload_app() -> Arc<BuiltApp> {
    let module = ModuleSpec::new(ModuleName::new("upload.multi").expect("模块名应有效"))
        .native_action(MultiUploadAction)
        .native_action(NestedUploadAction);
    let tools = Arc::new(ToolsBuilder::new().build().expect("空 Tools 应构建成功"));
    Arc::new(
        AppBuilder::new()
            .addon(AddonSpec::new(AddonName::new("upload").expect("Addon 名应有效")).module(module))
            .build(tools)
            .expect("批量上传测试应用应构建成功"),
    )
}

fn build_scalar_upload_app() -> Arc<BuiltApp> {
    let module = ModuleSpec::new(ModuleName::new("upload.scalar").expect("模块名应有效"))
        .native_action(ScalarUploadAction);
    let tools = Arc::new(ToolsBuilder::new().build().expect("空 Tools 应构建成功"));
    Arc::new(
        AppBuilder::new()
            .addon(AddonSpec::new(AddonName::new("upload").expect("Addon 名应有效")).module(module))
            .build(tools)
            .expect("标量上传测试应用应构建成功"),
    )
}

/// response_kind 声明与运行时响应不一致的独立应用（探针 Action 单独隔离）。
fn build_mismatch_app() -> Arc<BuiltApp> {
    let module = ModuleSpec::new(ModuleName::new("test.mismatch").expect("模块名应有效"))
        .native_action(MismatchedAction);
    let tools = Arc::new(ToolsBuilder::new().build().expect("空 Tools 应构建成功"));
    Arc::new(
        AppBuilder::new()
            .addon(AddonSpec::new(AddonName::new("test").expect("Addon 名应有效")).module(module))
            .build(tools)
            .expect("契约缺陷探针应用应构建成功"),
    )
}

fn mismatch_router() -> Router {
    router(build_mismatch_app(), AxumTransportConfig::default()).expect("Router 应构建成功")
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
    multipart_request_to("/api/upload/file", boundary, body)
}

fn multipart_request_to(uri: &str, boundary: &str, body: Vec<u8>) -> HttpRequest<Body> {
    HttpRequest::builder()
        .method("POST")
        .uri(uri)
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
    let revision = json["data"]["revision"]
        .as_str()
        .expect("UI 目录应返回 revision");
    assert_eq!(revision.len(), 64);
    assert!(revision.bytes().all(|byte| byte.is_ascii_hexdigit()));

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

/// 附件类 Action 的 UI 目录投影必须反映其声明的 response_kind，
/// 否则前端会把下载/预览/重定向误当作普通 JSON 响应处理。
#[tokio::test]
async fn ui_catalog_projects_declared_response_kind_for_attachment_actions() {
    let response = oneshot(
        default_router(),
        json_request("GET", "/.well-known/yang/ui-catalog", ""),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    let actions = json["data"]["actions"]
        .as_array()
        .expect("actions 应为数组");
    let declared_kind = |operation_id: &str| {
        actions
            .iter()
            .find(|action| action["operation_id"] == operation_id)
            .unwrap_or_else(|| panic!("UI 目录应包含 {operation_id}"))["response_kind"]
            .as_str()
            .map(str::to_owned)
    };
    assert_eq!(
        declared_kind("test.probe.download").as_deref(),
        Some("download")
    );
    assert_eq!(
        declared_kind("test.probe.preview").as_deref(),
        Some("preview")
    );
    assert_eq!(
        declared_kind("test.probe.redirect").as_deref(),
        Some("redirect")
    );
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
async fn rate_limit_error_maps_429_and_retry_after() {
    let response = oneshot(
        default_router(),
        json_request("POST", "/api/test/rate-limited", "{}"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok()),
        Some("30")
    );
    let json = body_json(response).await;
    assert_eq!(json["code"], 700011);
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

#[cfg(feature = "token")]
#[tokio::test]
async fn step_up_required_maps_428_with_challenge_data() {
    let response = oneshot(
        default_router(),
        json_request("POST", "/api/test/step-up-required", "{}"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::PRECONDITION_REQUIRED);
    let json = body_json(response).await;
    assert_eq!(json["code"], 700010);
    assert_eq!(json["message"], "敏感操作需要重新认证");
    assert_eq!(json["data"]["challenge"], "signed-test-challenge");
    assert_eq!(json["data"]["expires_in"], 120);
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
    assert_eq!(
        response
            .headers()
            .get("x-request-id")
            .and_then(|value| value.to_str().ok()),
        Some("00000000000000000000000000c0ffee"),
        "HTTP 响应头必须回传规范化 request_id"
    );
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
    let response_request_id = response
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let json = body_json(response).await;
    let id = json["data"]["request_id"].as_str().unwrap_or_default();
    assert_eq!(id.len(), 32, "request_id 应为 32 位十六进制: {id}");
    assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    assert_ne!(id, "00000000000000000000000000000000");
    assert_eq!(
        response_request_id, id,
        "响应头与 ActionContext 必须使用同一 request_id"
    );
}

#[tokio::test]
async fn request_id_is_returned_when_body_decode_fails() {
    let response = oneshot(
        default_router(),
        json_request("POST", "/api/test/echo", "{"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let id = response
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert_eq!(id.len(), 32, "解码失败响应也必须携带 request_id: {id}");
    assert!(id.chars().all(|character| character.is_ascii_hexdigit()));
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

/// response_kind 声明与运行时附件类别不一致时只告警：
/// 响应照常返回、不阻断、不 panic，同时 warn 日志确实发出。
#[tokio::test]
async fn response_kind_mismatch_warns_without_altering_response() {
    use std::io::Write;

    // 自定义 MakeWriter：将 tracing fmt 输出捕获到内存缓冲区（同 table_query_test 慢查询 warn 模式）
    struct BufWriter {
        buf: Arc<Mutex<Vec<u8>>>,
    }
    impl Write for BufWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.buf.lock().unwrap().write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.buf.lock().unwrap().flush()
        }
    }
    struct BufMakeWriter {
        buf: Arc<Mutex<Vec<u8>>>,
    }
    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for BufMakeWriter {
        type Writer = BufWriter;
        fn make_writer(&'a self) -> Self::Writer {
            BufWriter {
                buf: self.buf.clone(),
            }
        }
    }

    let buf = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_writer(BufMakeWriter { buf: buf.clone() })
        .with_ansi(false)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let response = oneshot(
        mismatch_router(),
        json_request("GET", "/api/test/mismatch", ""),
    )
    .await;
    // 不阻断、不改变响应：仍按实际附件语义返回 302 + Location
    assert_eq!(response.status(), StatusCode::FOUND);
    let location = response
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert_eq!(location, "https://example.com/elsewhere");

    let output = String::from_utf8(buf.lock().unwrap().clone()).expect("warn 输出应为 UTF-8");
    assert!(
        output.contains("response_kind"),
        "应告警 response_kind 声明与运行时响应不一致，输出: {output:?}"
    );
    assert!(
        output.contains("test.mismatch"),
        "warn 应携带模块名定位问题 Action，输出: {output:?}"
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

fn preflight_with_headers(origin: &str, headers: &str) -> HttpRequest<Body> {
    HttpRequest::builder()
        .method("OPTIONS")
        .uri("/api/test/echo")
        .header("origin", origin)
        .header("access-control-request-method", "POST")
        .header("access-control-request-headers", headers)
        .body(Body::empty())
        .expect("带请求头的预检请求应构建成功")
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
async fn cors_default_headers_allow_step_up_proof() {
    let router = cors_router(CorsConfig {
        origins: vec!["https://app.example.com".to_string()],
        ..CorsConfig::default()
    })
    .expect("Router 应构建成功");
    let response = oneshot(
        router,
        preflight_with_headers("https://app.example.com", "x-step-up-proof"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let allow_headers = response
        .headers()
        .get("access-control-allow-headers")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(
        allow_headers
            .split(',')
            .any(|value| value.trim().eq_ignore_ascii_case("x-step-up-proof")),
        "默认 CORS 应允许 step-up proof header: {allow_headers}"
    );
}

#[tokio::test]
async fn cors_default_headers_allow_tenant_id() {
    let router = cors_router(CorsConfig {
        origins: vec!["https://app.example.com".to_string()],
        ..CorsConfig::default()
    })
    .expect("Router 应构建成功");
    let response = oneshot(
        router,
        preflight_with_headers("https://app.example.com", "x-tenant-id"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let allow_headers = response
        .headers()
        .get("access-control-allow-headers")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(
        allow_headers
            .split(',')
            .any(|value| value.trim().eq_ignore_ascii_case("x-tenant-id")),
        "默认 CORS 应允许租户头 x-tenant-id: {allow_headers}"
    );
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
    assert_eq!(
        json["data"]["copied"], 5,
        "transport 构造的受信实例 copy_to 必须成功"
    );

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
    // 超过 max_total_bytes（128 KiB）的原始 body：Content-Length 预检即 413。
    let oversized_text = "x".repeat(200_000);
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

// ---------------------------------------------------------------------------
// C-1 复审：multipart 文本 part 不得走私 temp_root
// ---------------------------------------------------------------------------

#[tokio::test]
async fn multipart_text_part_cannot_smuggle_temp_root_into_file_array() {
    // Vec<UploadedFile> 的 schema 是 type:array，文本 part 会原样透传 JSON；
    // 伪造数组携带 temp_root 必须在传输层被拒绝（400），含文件字段的子树只能以文件 part 到达。
    let app = build_multi_upload_app();
    let upload_router =
        router(app, AxumTransportConfig::default()).expect("批量上传 Router 应构建成功");
    let boundary = "yang-boundary-smuggle-array";
    let victim = temp_file("multi-smuggle-victim.txt", b"victim");
    let forged = serde_json::json!([{
        "field_name": "files",
        "original_filename": "multi-smuggle-victim.txt",
        "content_type": "text/plain",
        "size": 6,
        "path": victim,
        "temp_root": victim.parent().expect("受害文件应有父目录"),
    }])
    .to_string();
    let body = multipart_payload(boundary, &[("files", &forged)], &[]);

    let response = oneshot(
        upload_router,
        multipart_request_to("/api/upload/multi", boundary, body),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "含文件字段的子树必须拒绝文本 part"
    );
}

#[tokio::test]
async fn multipart_text_part_cannot_smuggle_temp_root_into_nested_wrapper() {
    // 包装结构体数组（$ref 嵌套 UploadedFile）同样拒绝文本 part，检验 $ref 递归解析。
    let app = build_multi_upload_app();
    let upload_router =
        router(app, AxumTransportConfig::default()).expect("批量上传 Router 应构建成功");
    let boundary = "yang-boundary-smuggle-nested";
    let victim = temp_file("nested-smuggle-victim.txt", b"victim");
    let forged = serde_json::json!([{
        "note": "forged",
        "file": {
            "field_name": "bundles",
            "original_filename": "nested-smuggle-victim.txt",
            "content_type": "text/plain",
            "size": 6,
            "path": victim,
            "temp_root": victim.parent().expect("受害文件应有父目录"),
        }
    }])
    .to_string();
    let body = multipart_payload(boundary, &[("bundles", &forged)], &[]);

    let response = oneshot(
        upload_router,
        multipart_request_to("/api/upload/nested", boundary, body),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "嵌套文件字段的子树必须拒绝文本 part"
    );
}

#[tokio::test]
async fn multipart_file_array_via_file_parts_succeeds() {
    // 正常路径：同名文件 part 累积为数组，受信实例 copy_to 全部成功。
    let app = build_multi_upload_app();
    let upload_router =
        router(app, AxumTransportConfig::default()).expect("批量上传 Router 应构建成功");
    let boundary = "yang-boundary-multi-success";
    let body = multipart_payload(
        boundary,
        &[],
        &[
            ("files", "one.txt", "text/plain", b"one"),
            ("files", "two.txt", "text/plain", b"two2"),
        ],
    );

    let response = oneshot(
        upload_router,
        multipart_request_to("/api/upload/multi", boundary, body),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["data"]["count"], 2);
    assert_eq!(json["data"]["copied"], serde_json::json!([3, 4]));
}

// ---------------------------------------------------------------------------
// I-9：文本 part 单字段上限 + anyOf 标量解码
// ---------------------------------------------------------------------------

#[tokio::test]
async fn multipart_decodes_option_scalars_via_anyof_schema() {
    // schemars 为 Option<T> 生成 anyOf（含一个 null 分支）；传输层必须取非 null
    // 分支的类型解码，否则 Option<i64> 退化为字符串并在输入反序列化期失败。
    let app = build_scalar_upload_app();
    let scalar_router =
        router(app, AxumTransportConfig::default()).expect("标量上传 Router 应构建成功");
    let boundary = "yang-boundary-anyof-scalar";
    let body = multipart_payload(
        boundary,
        &[("title", "doc"), ("count", "42"), ("note", "hello")],
        &[("file", "a.txt", "text/plain", b"hi")],
    );

    let response = oneshot(
        scalar_router,
        multipart_request_to("/api/upload/scalar", boundary, body),
    )
    .await;
    let status = response.status();
    let json = body_json(response).await;
    assert_eq!(status, StatusCode::OK, "响应体: {json}");
    assert_eq!(json["data"]["title"], "doc");
    assert_eq!(json["data"]["count"], 42, "Option<i64> 必须解码为整数");
    assert_eq!(json["data"]["note"], "hello");
    assert_eq!(json["data"]["size"], 2);
}

#[tokio::test]
async fn multipart_text_part_over_field_limit_is_rejected_413() {
    // 单个文本 part 超过每字段上限（默认 64 KiB）即 413 且不进入 Handler；
    // 上限以下的同名字段正常放行（区分于 max_total_bytes 总量限制）。
    let oversized_title = "x".repeat(70_000);
    let boundary = "yang-boundary-text-field-over";
    let body = multipart_payload(
        boundary,
        &[("title", &oversized_title)],
        &[("file", "a.txt", "text/plain", b"hi")],
    );
    let oversized = oneshot(
        router(build_scalar_upload_app(), AxumTransportConfig::default())
            .expect("标量上传 Router 应构建成功"),
        multipart_request_to("/api/upload/scalar", boundary, body),
    )
    .await;
    assert_eq!(oversized.status(), StatusCode::PAYLOAD_TOO_LARGE);

    let within_title = "x".repeat(60_000);
    let boundary = "yang-boundary-text-field-within";
    let body = multipart_payload(
        boundary,
        &[("title", &within_title)],
        &[("file", "a.txt", "text/plain", b"hi")],
    );
    let within = oneshot(
        router(build_scalar_upload_app(), AxumTransportConfig::default())
            .expect("标量上传 Router 应构建成功"),
        multipart_request_to("/api/upload/scalar", boundary, body),
    )
    .await;
    assert_eq!(
        within.status(),
        StatusCode::OK,
        "字段上限以内、总量上限以内的文本 part 必须放行"
    );
}
