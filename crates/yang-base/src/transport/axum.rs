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

use crate::action::{
    ApiResponse, Request, RequestId, RequestMeta, ResponseAttachment, TENANT_ID_HEADER,
};
use crate::definition::{
    schema_subtree_contains_binary, ActionHandle, ActionMediaType, ActionRef, BuiltApp,
    MultipartSpec,
};
use crate::error::{BaseError, ErrorCategory};
use axum::body::{to_bytes, Body};
use axum::extract::{
    ConnectInfo, DefaultBodyLimit, FromRequest, Multipart, Path, Request as AxumRequest, State,
};
use axum::http::{header, HeaderName, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{on, MethodFilter};
use axum::{Json, Router};
use serde_json::json;
use serde_json::{map::Entry, Map, Value};
use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tower::limit::ConcurrencyLimitLayer;
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
            headers: [
                "content-type",
                "authorization",
                "x-request-id",
                "x-step-up-proof",
                TENANT_ID_HEADER,
            ]
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
    /// 同时进入应用的最大请求数；`None`（默认）不额外限制。
    ///
    /// 超出部分在 Tower 层等待，并由 `request_timeout` 限制最长等待时间。
    pub max_concurrency: Option<usize>,
    /// 是否按 `Accept-Encoding` 协商压缩响应。默认开启。
    pub compression: bool,
}

impl Default for AxumTransportConfig {
    fn default() -> Self {
        Self {
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            cors: CorsConfig::default(),
            request_timeout: None,
            max_concurrency: None,
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

#[derive(Clone)]
enum RequestDecoder {
    Json,
    Multipart {
        spec: MultipartSpec,
        input_schema: Value,
    },
}

struct DecodedBody {
    value: Value,
    resource_guard: Option<Arc<tempfile::TempDir>>,
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

/// 绑定地址并启动 HTTP 服务，直至收到关闭信号（Ctrl+C 或 Unix SIGTERM）后优雅停机。
pub async fn serve(
    bind: SocketAddr,
    app: Arc<BuiltApp>,
    config: AxumTransportConfig,
) -> Result<(), BaseError> {
    serve_with_shutdown(
        bind,
        app,
        config,
        crate::lifecycle::wait_for_shutdown_signal(),
    )
    .await
}

/// 使用调用方提供的关闭触发器启动 HTTP 服务。
///
/// 与 [`serve`] 的唯一区别是信号所有权交给调用方，使上层可以从同一个关闭
/// 事件开始，为 HTTP drain、后台任务和资源释放共享一个进程级总预算。
pub async fn serve_with_shutdown<S>(
    bind: SocketAddr,
    app: Arc<BuiltApp>,
    config: AxumTransportConfig,
    shutdown: S,
) -> Result<(), BaseError>
where
    S: Future<Output = ()> + Send + 'static,
{
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
    .with_graceful_shutdown(shutdown)
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
                let (decoder, body_limit) = match action.request_media_type {
                    ActionMediaType::Json => (RequestDecoder::Json, state.max_body_bytes),
                    ActionMediaType::Multipart => {
                        let spec = action.multipart.clone().ok_or_else(|| {
                            BaseError::ConfigError(format!(
                                "multipart Action 缺少资源限制: {reference}"
                            ))
                        })?;
                        let action_limit = usize::try_from(spec.max_total_bytes).map_err(|_| {
                            BaseError::ConfigError(format!(
                                "multipart Action 请求上限超出平台 usize: {reference}"
                            ))
                        })?;
                        if action_limit > state.max_body_bytes {
                            return Err(BaseError::ConfigError(format!(
                                "multipart Action {reference} 的 max_total_bytes={} 超过 AxumTransportConfig.max_body_bytes={}",
                                spec.max_total_bytes, state.max_body_bytes
                            )));
                        }
                        (
                            RequestDecoder::Multipart {
                                spec,
                                input_schema: action.input_schema.clone(),
                            },
                            action_limit,
                        )
                    }
                };
                let method_router = on(
                    method_filter,
                    move |State(state): State<HttpState>,
                          ConnectInfo(peer): ConnectInfo<SocketAddr>,
                          Path(path_params): Path<HashMap<String, String>>,
                          request: AxumRequest| {
                        let decoder = decoder.clone();
                        async move {
                            dispatch_request(
                                state,
                                peer,
                                path_params,
                                request,
                                handle,
                                success_status,
                                decoder,
                            )
                            .await
                        }
                    },
                )
                .layer(DefaultBodyLimit::max(body_limit));
                router = router.route(&path, method_router);
            }
        }
    }

    let mut router = router.with_state(state);

    // 横切层：后调用者先生效，TraceLayer 置于最外层以覆盖全部处理耗时。
    if config.compression {
        router = router.layer(CompressionLayer::new());
    }
    if let Some(max_concurrency) = config.max_concurrency {
        if max_concurrency == 0 {
            return Err(BaseError::ConfigError(
                "HTTP max_concurrency 必须大于 0".to_string(),
            ));
        }
        router = router.layer(ConcurrencyLimitLayer::new(max_concurrency));
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
    decoder: RequestDecoder,
) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let headers: HashMap<String, String> = request
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_string(), value.to_string()))
        })
        .collect();
    // 每个进入 Action 传输边界的请求都只有一个确定 request_id：合法上游值规范化
    // 后沿用，否则在读取请求体前生成。这样即使 query/body 解码失败，调用方仍能用
    // 响应头关联服务端日志。
    let request_id = headers
        .get("x-request-id")
        .and_then(|raw| RequestId::parse_hex(raw))
        .unwrap_or_default();
    let query = match uri.query() {
        Some(raw) => match serde_urlencoded::from_str::<HashMap<String, String>>(raw) {
            Ok(query) => query,
            Err(_) => {
                return with_request_id_header(
                    error_response(
                        StatusCode::BAD_REQUEST,
                        BaseError::ParamInvalid(
                            "query".to_string(),
                            "查询参数编码无效".to_string(),
                        ),
                    ),
                    request_id,
                );
            }
        },
        None => HashMap::new(),
    };
    let decoded = match decode_request_body(&state, request, decoder).await {
        Ok(decoded) => decoded,
        Err((status, error)) => {
            return with_request_id_header(error_response(status, error), request_id);
        }
    };

    let mut action_request = Request::new(decoded.value)
        .headers(headers)
        .queries(query)
        .path_params(path_params);
    if let Some(resource_guard) = decoded.resource_guard {
        action_request = action_request.retain_resource(resource_guard);
    }
    let request_meta = RequestMeta::new()
        .with_method(method.to_string())
        .with_original_uri(uri.to_string())
        .with_scheme(uri.scheme_str().unwrap_or("http"))
        .with_peer_addr(peer);
    let request_meta = match state.local_addr {
        Some(addr) => request_meta.with_local_addr(addr),
        None => request_meta,
    };
    let context = state
        .app
        .context(action_request)
        .with_request_meta(request_meta)
        .with_request_id(request_id);

    let response = match state.app.dispatch_context(handle, context).await {
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
    };
    with_request_id_header(response, request_id)
}

async fn decode_request_body(
    state: &HttpState,
    request: AxumRequest,
    decoder: RequestDecoder,
) -> Result<DecodedBody, (StatusCode, BaseError)> {
    match decoder {
        RequestDecoder::Json => {
            let body = to_bytes(request.into_body(), state.max_body_bytes)
                .await
                .map_err(|_| {
                    (
                        StatusCode::PAYLOAD_TOO_LARGE,
                        BaseError::ParamInvalid("body".to_string(), "请求体过大".to_string()),
                    )
                })?;
            let value = if body.is_empty() {
                json!({})
            } else {
                serde_json::from_slice(&body).map_err(|_| {
                    (
                        StatusCode::BAD_REQUEST,
                        BaseError::ParamInvalid(
                            "body".to_string(),
                            "请求体必须是 JSON".to_string(),
                        ),
                    )
                })?
            };
            Ok(DecodedBody {
                value,
                resource_guard: None,
            })
        }
        RequestDecoder::Multipart { spec, input_schema } => {
            decode_multipart(state, request, &spec, &input_schema).await
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MultipartPartKind {
    Text,
    File,
}

async fn decode_multipart(
    state: &HttpState,
    request: AxumRequest,
    spec: &MultipartSpec,
    input_schema: &Value,
) -> Result<DecodedBody, (StatusCode, BaseError)> {
    let content_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let media_type = content_type.split(';').next().unwrap_or_default().trim();
    if !media_type.eq_ignore_ascii_case("multipart/form-data") {
        return Err(multipart_error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "body",
            "请求 Content-Type 必须是 multipart/form-data",
        ));
    }
    if request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|length| length > spec.max_total_bytes)
    {
        return Err(multipart_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "body",
            "multipart 请求体过大",
        ));
    }

    let scope = Arc::new(tempfile::tempdir().map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            BaseError::IoError(format!("创建上传临时目录失败: {error}")),
        )
    })?);
    let mut multipart = Multipart::from_request(request, state)
        .await
        .map_err(|rejection| {
            let status = rejection.status();
            let status = if status == StatusCode::PAYLOAD_TOO_LARGE {
                status
            } else {
                StatusCode::BAD_REQUEST
            };
            multipart_error(status, "body", "multipart 请求格式无效")
        })?;
    let mut body = Map::new();
    let mut kinds = HashMap::new();
    let mut field_count = 0_u16;
    let mut file_count = 0_u16;
    let mut payload_bytes = 0_u64;

    while let Some(mut field) = multipart.next_field().await.map_err(|error| {
        let status = error.status();
        let status = if status == StatusCode::PAYLOAD_TOO_LARGE {
            status
        } else {
            StatusCode::BAD_REQUEST
        };
        multipart_error(status, "body", "multipart 字段读取失败")
    })? {
        let name = field
            .name()
            .filter(|name| !name.is_empty() && name.len() <= 255)
            .map(str::to_string)
            .ok_or_else(|| {
                multipart_error(
                    StatusCode::BAD_REQUEST,
                    "body",
                    "multipart 字段名缺失或过长",
                )
            })?;
        let raw_filename = field.file_name().map(str::to_string);

        if let Some(raw_filename) = raw_filename {
            file_count = file_count.checked_add(1).ok_or_else(|| {
                multipart_error(StatusCode::PAYLOAD_TOO_LARGE, &name, "文件数量超过上限")
            })?;
            if file_count > spec.max_files {
                return Err(multipart_error(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    &name,
                    "文件数量超过上限",
                ));
            }
            let content_type = field
                .content_type()
                .map(|value| {
                    value
                        .split(';')
                        .next()
                        .unwrap_or_default()
                        .trim()
                        .to_ascii_lowercase()
                })
                .ok_or_else(|| {
                    multipart_error(
                        StatusCode::UNSUPPORTED_MEDIA_TYPE,
                        &name,
                        "上传文件缺少 Content-Type",
                    )
                })?;
            if !spec
                .allowed_content_types
                .iter()
                .any(|allowed| allowed == &content_type)
            {
                return Err(multipart_error(
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    &name,
                    "上传文件 Content-Type 不在 Action 白名单中",
                ));
            }

            let named = tempfile::Builder::new()
                .prefix("yang-upload-")
                .tempfile_in(scope.path())
                .map_err(|error| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        BaseError::IoError(format!("创建上传临时文件失败: {error}")),
                    )
                })?;
            let (file, path) = named.keep().map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    BaseError::IoError(format!("持有上传临时文件失败: {}", error.error)),
                )
            })?;
            let mut output = tokio::fs::File::from_std(file);
            let mut file_bytes = 0_u64;
            while let Some(chunk) = field.chunk().await.map_err(|error| {
                let status = error.status();
                let status = if status == StatusCode::PAYLOAD_TOO_LARGE {
                    status
                } else {
                    StatusCode::BAD_REQUEST
                };
                multipart_error(status, &name, "上传文件读取失败")
            })? {
                file_bytes = checked_payload_size(file_bytes, chunk.len(), spec.max_file_bytes)
                    .map_err(|_| {
                        multipart_error(StatusCode::PAYLOAD_TOO_LARGE, &name, "单文件大小超过上限")
                    })?;
                payload_bytes =
                    checked_payload_size(payload_bytes, chunk.len(), spec.max_total_bytes)
                        .map_err(|_| {
                            multipart_error(
                                StatusCode::PAYLOAD_TOO_LARGE,
                                "body",
                                "multipart 请求体过大",
                            )
                        })?;
                output.write_all(&chunk).await.map_err(|error| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        BaseError::IoError(format!("写入上传临时文件失败: {error}")),
                    )
                })?;
            }
            output.flush().await.map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    BaseError::IoError(format!("刷新上传临时文件失败: {error}")),
                )
            })?;
            drop(output);

            // 服务端构造上传文件句柄：注入受信临时根，标记该实例允许 copy_to。
            // 伪造通道已双向封闭：构建期强制 binary 字段只走 multipart；本通道内
            // 目标子树含文件字段的文本 part 一律拒绝（见上方文本分支），因此携带
            // temp_root 的文件字段 JSON 对象只能由本传输层生成。
            let value = json!({
                "field_name": name,
                "original_filename": sanitize_filename(&raw_filename),
                "content_type": content_type,
                "size": file_bytes,
                "path": path,
                "temp_root": scope.path()
            });
            insert_multipart_value(&mut body, &mut kinds, name, MultipartPartKind::File, value)?;
        } else {
            field_count = field_count.checked_add(1).ok_or_else(|| {
                multipart_error(StatusCode::PAYLOAD_TOO_LARGE, &name, "表单字段数量超过上限")
            })?;
            if field_count > spec.max_fields {
                return Err(multipart_error(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    &name,
                    "表单字段数量超过上限",
                ));
            }
            let mut bytes = Vec::new();
            let mut field_bytes = 0_u64;
            while let Some(chunk) = field.chunk().await.map_err(|error| {
                let status = error.status();
                let status = if status == StatusCode::PAYLOAD_TOO_LARGE {
                    status
                } else {
                    StatusCode::BAD_REQUEST
                };
                multipart_error(status, &name, "表单字段读取失败")
            })? {
                field_bytes =
                    checked_payload_size(field_bytes, chunk.len(), spec.max_text_field_bytes)
                        .map_err(|_| {
                            multipart_error(
                                StatusCode::PAYLOAD_TOO_LARGE,
                                &name,
                                "表单字段大小超过上限",
                            )
                        })?;
                payload_bytes =
                    checked_payload_size(payload_bytes, chunk.len(), spec.max_total_bytes)
                        .map_err(|_| {
                            multipart_error(
                                StatusCode::PAYLOAD_TOO_LARGE,
                                "body",
                                "multipart 请求体过大",
                            )
                        })?;
                bytes.extend_from_slice(&chunk);
            }
            let text = String::from_utf8(bytes).map_err(|_| {
                multipart_error(
                    StatusCode::BAD_REQUEST,
                    &name,
                    "multipart 文本字段必须是 UTF-8",
                )
            })?;
            // C-1 复审：含文件字段（format: binary，$ref 递归解析）的子树必须以文件
            // part 到达；否则文本 part 可原样透传 JSON 走私 temp_root 伪造受信实例。
            if text_part_targets_binary(input_schema, &name) {
                return Err(multipart_error(
                    StatusCode::BAD_REQUEST,
                    &name,
                    "包含文件字段的字段必须以文件形式上传",
                ));
            }
            let value = decode_text_value(input_schema, &name, text)?;
            insert_multipart_value(&mut body, &mut kinds, name, MultipartPartKind::Text, value)?;
        }
    }

    Ok(DecodedBody {
        value: Value::Object(body),
        resource_guard: Some(scope),
    })
}

fn checked_payload_size(current: u64, added: usize, limit: u64) -> Result<u64, ()> {
    let added = u64::try_from(added).map_err(|_| ())?;
    let next = current.checked_add(added).ok_or(())?;
    (next <= limit).then_some(next).ok_or(())
}

fn insert_multipart_value(
    body: &mut Map<String, Value>,
    kinds: &mut HashMap<String, MultipartPartKind>,
    name: String,
    kind: MultipartPartKind,
    value: Value,
) -> Result<(), (StatusCode, BaseError)> {
    if kinds.get(&name).is_some_and(|existing| *existing != kind) {
        return Err(multipart_error(
            StatusCode::BAD_REQUEST,
            &name,
            "同名 multipart 字段不能混用文本和文件",
        ));
    }
    kinds.entry(name.clone()).or_insert(kind);
    match body.entry(name) {
        Entry::Vacant(entry) => {
            entry.insert(value);
        }
        Entry::Occupied(mut entry) => match entry.get_mut() {
            Value::Array(values) => values.push(value),
            existing => {
                let first = std::mem::take(existing);
                *existing = Value::Array(vec![first, value]);
            }
        },
    }
    Ok(())
}

/// 判定 multipart 文本 part 的目标字段子树是否声明二进制文件字段（`format: "binary"`，
/// 本地 `$ref` 递归解析）；字段不在 schema 中时放行（由输入反序列化拒绝未知字段）。
fn text_part_targets_binary(input_schema: &Value, name: &str) -> bool {
    input_schema
        .get("properties")
        .and_then(|properties| properties.get(name))
        .is_some_and(|property| schema_subtree_contains_binary(input_schema, property))
}

/// 按 input_schema 中字段声明的类型把文本 part 解码为 JSON 值。
///
/// 类型来源按序回退：字段子 schema 的平铺 `type`（含 schemars nullable 形态
/// `"type": ["integer", "null"]`），再到 schemars `anyOf`（含 null 分支的
/// `Option<T>` 形态），取第一个非 null 类型。
/// 已知限制：本地 `$ref`（如 `Option<UploadedFile>` 的 `#/definitions/...` 分支）
/// 暂不解析——该分支没有 `type`，字段按字符串处理；指向二进制子树的文本
/// part 已在 [`text_part_targets_binary`] 处先行拒绝，其余 `$ref` 字段的解码
/// 精度损失由输入反序列化兜底报错。
fn decode_text_value(
    input_schema: &Value,
    name: &str,
    text: String,
) -> Result<Value, (StatusCode, BaseError)> {
    let property = input_schema
        .get("properties")
        .and_then(|properties| properties.get(name));
    let schema_type = property
        .and_then(non_null_type)
        .or_else(|| property.and_then(any_of_non_null_type));
    match schema_type {
        Some("integer") => text
            .parse::<i64>()
            .map(Value::from)
            .map_err(|_| multipart_error(StatusCode::BAD_REQUEST, name, "表单字段必须是整数")),
        Some("number") => text
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .ok_or_else(|| {
                multipart_error(StatusCode::BAD_REQUEST, name, "表单字段必须是有限数值")
            }),
        Some("boolean") => match text.as_str() {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            _ => Err(multipart_error(
                StatusCode::BAD_REQUEST,
                name,
                "表单字段必须是 true 或 false",
            )),
        },
        Some("object" | "array") => serde_json::from_str(&text)
            .map_err(|_| multipart_error(StatusCode::BAD_REQUEST, name, "表单字段 JSON 格式无效")),
        _ => Ok(Value::String(text)),
    }
}

/// 取子 schema 的非 null 类型：兼容平铺字符串（`"type": "integer"`）与 schemars
/// nullable 形态（`"type": ["integer", "null"]`）。
fn non_null_type(schema: &Value) -> Option<&str> {
    match schema.get("type") {
        Some(Value::String(kind)) if kind != "null" => Some(kind.as_str()),
        Some(Value::Array(kinds)) => kinds
            .iter()
            .filter_map(Value::as_str)
            .find(|kind| *kind != "null"),
        _ => None,
    }
}

/// 从 `anyOf` 分支中取第一个带非 null `type` 的分支类型（schemars 为 `Option<T>`
/// 生成的形态）；`$ref` 分支无 `type` 键，自然跳过。
fn any_of_non_null_type(schema: &Value) -> Option<&str> {
    schema
        .get("anyOf")?
        .as_array()?
        .iter()
        .find_map(non_null_type)
}

fn sanitize_filename(raw: &str) -> String {
    let leaf = raw.rsplit(['/', '\\']).next().unwrap_or_default().trim();
    let mut safe = String::new();
    for character in leaf.chars().filter(|character| !character.is_control()) {
        if safe.len() + character.len_utf8() > 255 {
            break;
        }
        safe.push(character);
    }
    if safe.is_empty() || matches!(safe.as_str(), "." | "..") {
        "upload.bin".to_string()
    } else {
        safe
    }
}

fn multipart_error(status: StatusCode, field: &str, message: &str) -> (StatusCode, BaseError) {
    (
        status,
        BaseError::ParamInvalid(field.to_string(), message.to_string()),
    )
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
        #[cfg(feature = "token")]
        BaseError::StepUpRequired(_) => StatusCode::PRECONDITION_REQUIRED,
        BaseError::RateLimitExceeded { .. } => StatusCode::TOO_MANY_REQUESTS,
        BaseError::Unauthorized(_)
        | BaseError::InvalidPassword
        | BaseError::TokenKeyInvalid(_)
        | BaseError::TokenGenerateFailed(_)
        | BaseError::TokenVerifyFailed(_)
        | BaseError::TokenParseFailed(_)
        | BaseError::TokenExpired
        | BaseError::TokenRevoked
        | BaseError::AuthorizationStale
        | BaseError::AuthorizationVersionInvalid
        | BaseError::TokenTypeInvalid(_) => StatusCode::UNAUTHORIZED,
        BaseError::AuthorizationCheckUnavailable => StatusCode::SERVICE_UNAVAILABLE,
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

fn with_request_id_header(mut response: Response, request_id: RequestId) -> Response {
    // RequestId::Display 固定输出 32 位 ASCII hex；仍通过 HeaderValue 校验守住
    // HTTP 边界，避免未来展示格式演进时把非法字节写入响应头。
    match HeaderValue::from_str(&request_id.to_string()) {
        Ok(value) => {
            response
                .headers_mut()
                .insert(HeaderName::from_static("x-request-id"), value);
        }
        Err(error) => {
            tracing::error!(%request_id, %error, "request_id 无法写入 HTTP 响应头");
        }
    }
    response
}

fn error_response(status: StatusCode, error: BaseError) -> Response {
    // 5xx 对外遮蔽内部细节（只进 tracing），4xx 保留业务 message
    #[cfg(feature = "token")]
    if let BaseError::StepUpRequired(challenge) = &error {
        let mut response = ApiResponse::from_error(&error);
        response.data = Some(serde_json::json!({
            "challenge": challenge.challenge,
            "expires_in": challenge.expires_in,
        }));
        return (status, Json(response)).into_response();
    }

    if let BaseError::RateLimitExceeded {
        retry_after_seconds,
    } = &error
    {
        let mut response = (status, Json(ApiResponse::from_error(&error))).into_response();
        if let Ok(value) = HeaderValue::from_str(&retry_after_seconds.to_string()) {
            response.headers_mut().insert(header::RETRY_AFTER, value);
        }
        return response;
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorization_freshness_errors_have_stable_http_semantics() {
        assert_eq!(
            status_for_error(&BaseError::AuthorizationStale),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            status_for_error(&BaseError::AuthorizationVersionInvalid),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            status_for_error(&BaseError::AuthorizationCheckUnavailable),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
}
