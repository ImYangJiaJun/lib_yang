//! Axum 0.8 HTTP 传输适配器。
//!
//! 把冻结的 [`BuiltApp`] 一次性暴露为生产可用的 HTTP 服务：
//!
//! - 启动时从 Catalog 逐 Action 生成真实路由（方法 + 路径模板），请求期零字符串查找；
//! - 请求体上限（413）、非法 JSON / query（400）在派发前拒绝；
//! - `x-request-id` 上游透传（沿用 [`RequestId::parse_hex`] 语义，失败则新生成）；
//! - 成功状态码按 Action 声明的 `success_status`，错误按 [`BaseError`] 类别映射，
//!   5xx 对外统一遮蔽 message（内部细节只进 tracing）；
//! - [`ResponseAttachment`] 映射：文件下载 / 预览 / 302 重定向；
//! - tower-http 横切：tracing、显式白名单 CORS、超时、压缩；
//! - `/health/live` 与 `/health/ready`（对接 [`Tools::health_check`](crate::tools::Tools)）。
//!
//! # 示例
//!
//! ```rust,ignore
//! use std::sync::Arc;
//! use yang_base::transport::axum::{serve, AxumTransportConfig};
//!
//! let app = Arc::new(build_app()?);
//! serve("0.0.0.0:8080".parse()?, app, AxumTransportConfig::default()).await?;
//! ```

use crate::action::{ApiResponse, Request, RequestId, RequestMeta, ResponseAttachment};
use crate::definition::{ActionHandle, ActionRef, BuiltApp};
use crate::error::{BaseError, ErrorCategory};
use axum::body::{to_bytes, Body};
use axum::extract::{ConnectInfo, Path, Request as AxumRequest, State};
use axum::http::{header, HeaderName, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{on, MethodFilter};
use axum::{Json, Router};
use serde_json::json;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

/// 默认请求体上限：2 MiB。
const DEFAULT_MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

/// CORS 配置（显式白名单语义）。
///
/// 安全约束在构建期强制：`origins` 含 `"*"` 时禁止同时开启 `credentials`
/// （该组合被浏览器规范拒绝，且是常见的配置性安全事故）。
#[derive(Debug, Clone)]
pub struct CorsConfig {
    /// 允许的来源白名单；空列表表示不启用 CORS 层；`["*"]` 表示任意来源。
    pub origins: Vec<String>,
    /// 允许的方法。
    pub methods: Vec<String>,
    /// 允许的请求头。
    pub headers: Vec<String>,
    /// 是否允许携带凭据（Cookie / Authorization）。
    pub credentials: bool,
    /// 预检结果缓存秒数。
    pub max_age_secs: u64,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            origins: Vec::new(),
            methods: ["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"]
                .iter()
                .map(|method| (*method).to_string())
                .collect(),
            headers: ["content-type", "authorization", "x-request-id"]
                .iter()
                .map(|name| (*name).to_string())
                .collect(),
            credentials: false,
            max_age_secs: 86400,
        }
    }
}

/// Axum 传输配置。
#[derive(Debug, Clone)]
pub struct AxumTransportConfig {
    /// 请求体字节上限；超过返回 413。默认 2 MiB。
    pub max_body_bytes: usize,
    /// CORS 配置；默认不启用 CORS 层。
    pub cors: CorsConfig,
    /// 单请求总超时；`None`（默认）关闭超时层。
    pub request_timeout: Option<Duration>,
    /// 是否按 `Accept-Encoding` 协商压缩响应。默认开启。
    pub compression: bool,
}

impl Default for AxumTransportConfig {
    fn default() -> Self {
        Self {
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            cors: CorsConfig::default(),
            request_timeout: None,
            compression: true,
        }
    }
}

/// 路由共享状态。
#[derive(Clone)]
struct HttpState {
    app: Arc<BuiltApp>,
    /// 实际监听地址（`serve` 填入；`router` 单独构建时为 None）。
    local_addr: Option<SocketAddr>,
    max_body_bytes: usize,
}

/// 由冻结应用构建 Axum [`Router`]（不绑定端口，可用 tower `oneshot` 直接驱动测试）。
///
/// # 返回
///
/// - `Ok(Router)`：含健康端点、Catalog 全部 Action 路由与已配置横切层
/// - `Err(BaseError::ConfigError)`：CORS 配置非法（如通配来源 + credentials）、
///   Action 未预编译到 Registry、或路由方法不受支持
pub fn router(app: Arc<BuiltApp>, config: AxumTransportConfig) -> Result<Router, BaseError> {
    router_with_addr(app, config, None)
}

/// 绑定地址并启动 HTTP 服务，直至收到关闭信号（Ctrl-C）后优雅停机。
pub async fn serve(
    bind: SocketAddr,
    app: Arc<BuiltApp>,
    config: AxumTransportConfig,
) -> Result<(), BaseError> {
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .map_err(|error| BaseError::ConfigError(format!("绑定 HTTP 地址失败 {bind}: {error}")))?;
    let local_addr = listener
        .local_addr()
        .map_err(|error| BaseError::ConfigError(format!("读取 HTTP 监听地址失败: {error}")))?;
    let router = router_with_addr(app, config, Some(local_addr))?;
    tracing::info!(address = %local_addr, "HTTP 服务已启动");
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .map_err(|error| BaseError::ConfigError(format!("HTTP 服务运行失败: {error}")))
}

fn router_with_addr(
    app: Arc<BuiltApp>,
    config: AxumTransportConfig,
    local_addr: Option<SocketAddr>,
) -> Result<Router, BaseError> {
    let state = HttpState {
        app,
        local_addr,
        max_body_bytes: config.max_body_bytes,
    };

    let mut router = Router::new()
        .route("/health/live", axum::routing::get(live))
        .route("/health/ready", axum::routing::get(ready));

    for addon in state.app.catalog().addons() {
        for module in &addon.modules {
            for action in module.actions() {
                let method_filter = method_filter(action.route.method.as_str())?;
                let path = action.route.path.clone();
                let reference = ActionRef::new(module.name.clone(), action.name.clone());
                let handle = state.app.registry().resolve(&reference).ok_or_else(|| {
                    BaseError::ConfigError(format!("Action 未预编译到 Registry: {reference}"))
                })?;
                let success_status = action.success_status;
                router = router.route(
                    &path,
                    on(
                        method_filter,
                        move |State(state): State<HttpState>,
                              ConnectInfo(peer): ConnectInfo<SocketAddr>,
                              Path(path_params): Path<HashMap<String, String>>,
                              request: AxumRequest| {
                            async move {
                                dispatch_request(
                                    state,
                                    peer,
                                    path_params,
                                    request,
                                    handle,
                                    success_status,
                                )
                                .await
                            }
                        },
                    ),
                );
            }
        }
    }

    let mut router = router.with_state(state);

    // 横切层：后调用者先生效，TraceLayer 置于最外层以覆盖全部处理耗时。
    if config.compression {
        router = router.layer(CompressionLayer::new());
    }
    if let Some(timeout) = config.request_timeout {
        router = router.layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            timeout,
        ));
    }
    if let Some(cors) = cors_layer(&config.cors)? {
        router = router.layer(cors);
    }
    Ok(router.layer(TraceLayer::new_for_http()))
}

/// 按 CORS 配置构建白名单层；空 origins 表示不启用。
fn cors_layer(config: &CorsConfig) -> Result<Option<CorsLayer>, BaseError> {
    if config.origins.is_empty() {
        return Ok(None);
    }
    let wildcard = config.origins.iter().any(|origin| origin == "*");
    if wildcard && config.credentials {
        return Err(BaseError::ConfigError(
            "CORS 禁止通配来源与 credentials 同用（浏览器规范拒绝该组合）".to_string(),
        ));
    }

    let methods = config
        .methods
        .iter()
        .map(|method| {
            Method::from_bytes(method.as_bytes()).map_err(|_| {
                BaseError::ConfigError(format!("CORS 配置包含非法 HTTP 方法: {method}"))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let headers = config
        .headers
        .iter()
        .map(|name| {
            HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| BaseError::ConfigError(format!("CORS 配置包含非法请求头: {name}")))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let layer = CorsLayer::new()
        .allow_methods(methods)
        .allow_headers(headers)
        .max_age(Duration::from_secs(config.max_age_secs));
    let layer = if wildcard {
        layer.allow_origin(Any)
    } else {
        let origins = config
            .origins
            .iter()
            .map(|origin| {
                HeaderValue::from_str(origin)
                    .map_err(|_| BaseError::ConfigError(format!("CORS 配置包含非法来源: {origin}")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        layer.allow_origin(origins)
    };
    let layer = if config.credentials {
        layer.allow_credentials(true)
    } else {
        layer
    };
    Ok(Some(layer))
}

async fn live() -> impl IntoResponse {
    Json(ApiResponse::success_value(
        json!({"status": "live"}),
        "服务存活",
    ))
}

async fn ready(State(state): State<HttpState>) -> Response {
    let health = state.app.tools().health_check().await;
    if health.is_healthy() {
        (
            StatusCode::OK,
            Json(ApiResponse::success_value(
                json!({"status": "ready"}),
                "服务就绪",
            )),
        )
            .into_response()
    } else {
        tracing::warn!(?health, "就绪检查失败");
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::fail(900001, "服务尚未就绪")),
        )
            .into_response()
    }
}

async fn dispatch_request(
    state: HttpState,
    peer: SocketAddr,
    path_params: HashMap<String, String>,
    request: AxumRequest,
    handle: ActionHandle,
    success_status: u16,
) -> Response {
    let (parts, body) = request.into_parts();
    let body = match to_bytes(body, state.max_body_bytes).await {
        Ok(body) => body,
        Err(_) => {
            return error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                BaseError::ParamInvalid("body".to_string(), "请求体过大".to_string()),
            )
        }
    };
    let body = if body.is_empty() {
        json!({})
    } else {
        match serde_json::from_slice(&body) {
            Ok(value) => value,
            Err(_) => {
                return error_response(
                    StatusCode::BAD_REQUEST,
                    BaseError::ParamInvalid("body".to_string(), "请求体必须是 JSON".to_string()),
                )
            }
        }
    };

    let headers: HashMap<String, String> = parts
        .headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_string(), value.to_string()))
        })
        .collect();
    let query = match parts.uri.query() {
        Some(raw) => match serde_urlencoded::from_str::<HashMap<String, String>>(raw) {
            Ok(query) => query,
            Err(_) => {
                return error_response(
                    StatusCode::BAD_REQUEST,
                    BaseError::ParamInvalid("query".to_string(), "查询参数编码无效".to_string()),
                )
            }
        },
        None => HashMap::new(),
    };

    // 上游 request_id 透传：解析失败（非十六进制/超长）时降级为新生成。
    let upstream_request_id = headers
        .get("x-request-id")
        .and_then(|raw| RequestId::parse_hex(raw));

    let action_request = Request::new(body)
        .headers(headers)
        .queries(query)
        .path_params(path_params);
    let request_meta = RequestMeta::new()
        .with_method(parts.method.to_string())
        .with_original_uri(parts.uri.to_string())
        .with_scheme(parts.uri.scheme_str().unwrap_or("http"))
        .with_peer_addr(peer);
    let request_meta = match state.local_addr {
        Some(addr) => request_meta.with_local_addr(addr),
        None => request_meta,
    };
    let mut context = state
        .app
        .context(action_request)
        .with_request_meta(request_meta);
    if let Some(request_id) = upstream_request_id {
        context = context.with_request_id(request_id);
    }

    match state.app.dispatch_context(handle, context).await {
        Ok(response) => match response.attachment.clone() {
            Some(attachment) => attachment_response(attachment).await,
            None => {
                let status = StatusCode::from_u16(success_status).unwrap_or(StatusCode::OK);
                (status, Json(response)).into_response()
            }
        },
        Err(error) => {
            let status = status_for_error(&error);
            if status.is_server_error() {
                tracing::error!(error = %error, code = error.code(), "请求处理失败");
            }
            error_response(status, error)
        }
    }
}

/// 把附件响应映射为真实 HTTP 响应。
async fn attachment_response(attachment: ResponseAttachment) -> Response {
    match attachment {
        ResponseAttachment::Redirect { url } => match HeaderValue::from_str(&url) {
            Ok(location) => (StatusCode::FOUND, [(header::LOCATION, location)]).into_response(),
            Err(_) => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                BaseError::ConfigError("重定向地址包含非法字符".to_string()),
            ),
        },
        ResponseAttachment::Download { path, filename } => {
            file_response(&path, Disposition::Attachment(&filename)).await
        }
        ResponseAttachment::Preview { path } => file_response(&path, Disposition::Inline).await,
    }
}

/// 文件响应的 Content-Disposition 形态。
enum Disposition<'a> {
    /// 下载：attachment + 安全编码的 filename。
    Attachment(&'a str),
    /// 预览：inline + 按扩展名推断的 Content-Type。
    Inline,
}

async fn file_response(path: &std::path::Path, disposition: Disposition<'_>) -> Response {
    let bytes = match tokio::fs::read(path).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return error_response(
                StatusCode::NOT_FOUND,
                BaseError::RecordNotFound(format!("文件不存在: {}", path.display())),
            )
        }
    };

    let mut response = Response::new(Body::from(bytes));
    let headers = response.headers_mut();
    match disposition {
        Disposition::Attachment(filename) => {
            headers.insert(
                header::CONTENT_DISPOSITION,
                content_disposition_value(filename),
            );
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/octet-stream"),
            );
        }
        Disposition::Inline => {
            headers.insert(
                header::CONTENT_DISPOSITION,
                HeaderValue::from_static("inline"),
            );
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static(mime_for(path)),
            );
        }
    }
    response
}

/// 构造安全的 `attachment` 处置值：ASCII 兜底名 + RFC 5987 `filename*` 原样保留中文名。
fn content_disposition_value(filename: &str) -> HeaderValue {
    // 兜底名剔除引号与空白控制字符，防止头注入
    let fallback: String = filename
        .chars()
        .map(|ch| match ch {
            '"' | '\r' | '\n' | '\\' => '_',
            ch if ch.is_ascii() => ch,
            _ => '_',
        })
        .collect();
    let fallback = if fallback.trim_matches('_').is_empty() {
        "download".to_string()
    } else {
        fallback
    };
    let encoded = percent_encode(filename.as_bytes());
    let value = format!("attachment; filename=\"{fallback}\"; filename*=UTF-8''{encoded}");
    // 上面的构造已剔除全部非法头字符；失败时退化为固定值而非 panic
    HeaderValue::from_str(&value)
        .unwrap_or_else(|_| HeaderValue::from_static("attachment; filename=\"download\""))
}

/// RFC 5987 attr-char 百分号编码。
fn percent_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for byte in bytes {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'!'
            | b'#'
            | b'$'
            | b'&'
            | b'+'
            | b'-'
            | b'.'
            | b'^'
            | b'_'
            | b'`'
            | b'|'
            | b'~' => out.push(*byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// 按扩展名推断预览 Content-Type（未知类型按二进制流）。
fn mime_for(path: &std::path::Path) -> &'static str {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase());
    match extension.as_deref() {
        Some("txt") | Some("log") | Some("md") | Some("csv") => "text/plain",
        Some("html") | Some("htm") => "text/html",
        Some("json") => "application/json",
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("mp4") => "video/mp4",
        Some("mp3") => "audio/mpeg",
        _ => "application/octet-stream",
    }
}

fn method_filter(method: &str) -> Result<MethodFilter, BaseError> {
    match method {
        "GET" => Ok(MethodFilter::GET),
        "POST" => Ok(MethodFilter::POST),
        "PUT" => Ok(MethodFilter::PUT),
        "PATCH" => Ok(MethodFilter::PATCH),
        "DELETE" => Ok(MethodFilter::DELETE),
        "HEAD" => Ok(MethodFilter::HEAD),
        "OPTIONS" => Ok(MethodFilter::OPTIONS),
        "TRACE" => Ok(MethodFilter::TRACE),
        other => Err(BaseError::ConfigError(format!(
            "不支持的 HTTP 方法: {other}"
        ))),
    }
}

fn status_for_error(error: &BaseError) -> StatusCode {
    match error {
        BaseError::Unauthorized(_)
        | BaseError::InvalidPassword
        | BaseError::TokenKeyInvalid(_)
        | BaseError::TokenGenerateFailed(_)
        | BaseError::TokenVerifyFailed(_)
        | BaseError::TokenParseFailed(_)
        | BaseError::TokenExpired
        | BaseError::TokenRevoked
        | BaseError::TokenTypeInvalid(_) => StatusCode::UNAUTHORIZED,
        BaseError::PermissionDenied(_) | BaseError::FieldPermissionDenied(_, _, _) => {
            StatusCode::FORBIDDEN
        }
        _ => match error.category() {
            ErrorCategory::Client => StatusCode::BAD_REQUEST,
            ErrorCategory::Auth => StatusCode::UNAUTHORIZED,
            ErrorCategory::NotFound => StatusCode::NOT_FOUND,
            ErrorCategory::Conflict => StatusCode::CONFLICT,
            ErrorCategory::Transient => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        },
    }
}

fn error_response(status: StatusCode, error: BaseError) -> Response {
    // 5xx 对外遮蔽内部细节（只进 tracing），4xx 保留业务 message
    let response = if status.is_server_error() {
        let message = if status == StatusCode::SERVICE_UNAVAILABLE {
            "服务暂时不可用"
        } else {
            "服务器内部错误"
        };
        ApiResponse::fail(error.code(), message)
    } else {
        ApiResponse::from_error(&error)
    };
    (status, Json(response)).into_response()
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(error = %error, "监听关闭信号失败");
    }
    tracing::info!("收到关闭信号");
}
