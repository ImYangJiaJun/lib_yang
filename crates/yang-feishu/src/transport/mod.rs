//! 飞书出站传输抽象、失败分类与重试引擎。
//!
//! # 方向说明
//!
//! 本 crate 只负责**出站**（我们调飞书开放平台），不处理飞书来调我们的入站凭证。
//!
//! # 判成败的唯一判据是响应体的业务码
//!
//! 大量飞书错误是 **HTTP 200 + 非 0 业务码**（`1254003`、`1254040`、`1254302` 等），
//! 反之可重试的 `1254607` 又是 HTTP 400。状态码只在**拿不到业务码**时兜底。
//!
//! # 失败分类顺序
//!
//! 429 → 业务码 → 状态码兜底。token 族必须先于 401/403 判定：`99991663` 常以
//! HTTP 400 返回，若先判 401/403 会把「应刷新 token」误判成终态。

pub mod codes;
#[cfg(feature = "http")]
pub mod http;

use std::collections::BTreeMap;
use std::time::Duration;

use crate::error::{Error, FailureKind, Result};

use codes::{
    CODES_PERMISSION_DENIED, CODES_RETRYABLE, CODES_RETRYABLE_APPROVAL, CODES_TOKEN_EXPIRED,
    CODES_TOKEN_MALFORMED, CODE_INSTANCE_NOT_FOUND, CODE_MESSAGE_RATE_LIMITED, CODE_RATE_LIMITED,
    CODE_SUBSCRIPTION_EXISTED, CODE_TOO_LARGE_RESPONSE, CODE_TOO_MANY_REQUEST, CODE_UUID_CONFLICT,
};

/// 出站 HTTP 方法。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// GET。
    Get,
    /// POST。
    Post,
    /// PUT。
    Put,
    /// PATCH。
    Patch,
    /// DELETE。
    Delete,
}

/// 传输中立的请求描述。
///
/// `query` 以**键值对**携带而不是预拼进 URL：百分号编码交给真传输，上层不手写编码。
///
/// 手写 `Debug`：`bearer_token` / `json_body` / `raw_body` 可能含凭证与租户数据，
/// 任何一次 `tracing::debug!(?request)` 都不能把它们落到日志里。
#[derive(Clone, PartialEq, Eq)]
pub struct Request {
    /// HTTP 方法。
    pub method: Method,
    /// 完整 URL（不含 query）。
    pub url: String,
    /// 查询参数键值对。
    pub query: Vec<(String, String)>,
    /// `Authorization: Bearer` 值。取 tenant_access_token 的请求必须为 `None`。
    pub bearer_token: Option<String>,
    /// JSON 请求体。
    pub json_body: Option<serde_json::Value>,
    /// 原始请求体：`(Content-Type, bytes)`。
    pub raw_body: Option<(String, Vec<u8>)>,
    /// 二进制响应上限；JSON 调用沿用客户端默认值。
    pub binary_limit: Option<usize>,
    /// 请求级超时（秒）。
    pub timeout_secs: Option<u64>,
    /// 失败后能否安全重发。
    pub idempotent: bool,
}

impl std::fmt::Debug for Request {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Request")
            .field("method", &self.method)
            .field("url", &self.url)
            .field("query", &self.query)
            .field(
                "bearer_token",
                &self.bearer_token.as_ref().map(|_| "<已隐藏>"),
            )
            .field("json_body", &self.json_body.as_ref().map(|_| "<已隐藏>"))
            .field(
                "raw_body",
                &self.raw_body.as_ref().map(|(content_type, body)| {
                    (content_type, format!("<{} 字节已隐藏>", body.len()))
                }),
            )
            .field("binary_limit", &self.binary_limit)
            .field("timeout_secs", &self.timeout_secs)
            .field("idempotent", &self.idempotent)
            .finish()
    }
}

/// 传输中立的响应描述。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// HTTP 状态码。
    pub status: u16,
    /// 文本响应体。
    pub body: String,
    /// 二进制响应体（仅在 `binary_limit` 且响应不是 JSON 时填充）。
    pub bytes: Option<Vec<u8>>,
    /// 响应头，键一律小写。
    pub headers: BTreeMap<String, String>,
}

/// 出站传输。
///
/// 实现**不做重试**：重试统一由 client 内部的发送循环负责。两处都开会让一次逻辑调用
/// 被放大成 N×M 次真实请求。
#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    /// 发送一次请求。
    async fn send(&self, request: Request) -> Result<Response>;
}

/// 可替换的睡眠，唯一目的是可测性。
#[async_trait::async_trait]
pub trait Sleeper: Send + Sync {
    /// 睡眠指定时长。
    async fn sleep(&self, duration: Duration);
}

/// 生产实现。
#[derive(Debug, Default, Clone, Copy)]
pub struct TokioSleeper;

#[async_trait::async_trait]
impl Sleeper for TokioSleeper {
    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

/// 自建重试策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// 总尝试次数（含首次）。1 表示不重试。
    pub max_attempts: u64,
    /// 指数退避的基准毫秒数。
    pub base_backoff_ms: u64,
    /// 指数退避的上限毫秒数。
    pub max_backoff_ms: u64,
    /// 采信服务端建议等待的上限毫秒数。
    pub max_retry_after_ms: u64,
}

/// 换取 token：尝试少、退避短。
///
/// 不变式：`TOKEN_RETRY.worst_case_seconds(TOKEN_REQUEST_TIMEOUT_SECS) < LOCK_TTL_SECONDS`。
pub const TOKEN_RETRY: RetryPolicy = RetryPolicy {
    max_attempts: 3,
    base_backoff_ms: 200,
    max_backoff_ms: 1_000,
    max_retry_after_ms: 2_000,
};

/// 单次换取 token 的请求超时（秒）。
pub const TOKEN_REQUEST_TIMEOUT_SECS: u64 = 5;

/// 拉取记录：在锁外执行，可以放宽。
pub const PULL_RETRY: RetryPolicy = RetryPolicy {
    max_attempts: 3,
    base_backoff_ms: 500,
    max_backoff_ms: 4_000,
    max_retry_after_ms: 30_000,
};

/// 单页拉取的请求超时（秒）。
pub const PULL_REQUEST_TIMEOUT_SECS: u64 = 20;

/// 部门名解析：best-effort 富化，一次即止。
pub const DEPARTMENT_RETRY: RetryPolicy = RetryPolicy {
    max_attempts: 1,
    base_backoff_ms: 0,
    max_backoff_ms: 0,
    max_retry_after_ms: 0,
};

/// 部门名解析的请求超时（秒）。
pub const DEPARTMENT_REQUEST_TIMEOUT_SECS: u64 = 5;

/// IM 消息：重试 3 次，退避 250ms 起、1000ms 封顶。
pub const MESSAGE_RETRY: RetryPolicy = RetryPolicy {
    max_attempts: 3,
    base_backoff_ms: 250,
    max_backoff_ms: 1_000,
    max_retry_after_ms: 1_000,
};

/// IM 单次请求的超时（秒）。
pub const MESSAGE_REQUEST_TIMEOUT_SECS: u64 = 2;

/// 审批主链路的请求超时（秒）。
pub const APPROVAL_REQUEST_TIMEOUT_SECS: u64 = 20;

/// 附件下载/上传的请求超时（秒）。
pub const MEDIA_REQUEST_TIMEOUT_SECS: u64 = 120;

impl RetryPolicy {
    /// 单轮的最坏耗时上界（秒）。
    pub fn worst_case_seconds(&self, request_timeout_secs: u64) -> u64 {
        request_timeout_secs * self.max_attempts
            + (self.max_retry_after_ms / 1_000) * self.max_attempts.saturating_sub(1)
    }

    /// 第 `attempt` 次（1 起）失败后的等待毫秒数。
    pub fn backoff_ms(&self, attempt: u64, retry_after_seconds: Option<u64>) -> u64 {
        if let Some(seconds) = retry_after_seconds {
            return seconds
                .saturating_mul(1_000)
                .min(self.max_retry_after_ms)
                .min(self.max_backoff_ms.max(self.max_retry_after_ms));
        }
        let shift = attempt.saturating_sub(1).min(20);
        self.base_backoff_ms
            .saturating_mul(1u64 << shift)
            .min(self.max_backoff_ms)
    }
}

/// 一次失败后该怎么办。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// 等待这么多毫秒后重发。
    RetryAfter(u64),
    /// 放弃，把失败返回给调用方。
    Give,
}

/// 判成败的唯一入口。返回 `None` 表示成功。
pub fn classify(
    status: u16,
    headers: &BTreeMap<String, String>,
    body: &str,
) -> Option<FailureKind> {
    if status == 429 {
        return Some(FailureKind::Retry {
            retry_after_seconds: rate_limit_reset_seconds(headers),
        });
    }

    if let Some(code) = business_code(body) {
        if code == 0 {
            if (200..300).contains(&status) {
                return None;
            }
        } else if CODES_TOKEN_EXPIRED.contains(&code) {
            return Some(FailureKind::TokenExpired);
        } else if CODES_TOKEN_MALFORMED.contains(&code) {
            return Some(FailureKind::Fatal { code });
        } else if code == CODE_RATE_LIMITED
            || code == CODE_TOO_MANY_REQUEST
            || code == CODE_MESSAGE_RATE_LIMITED
            || CODES_RETRYABLE.contains(&code)
            || CODES_RETRYABLE_APPROVAL.contains(&code)
        {
            return Some(FailureKind::Retry {
                retry_after_seconds: rate_limit_reset_seconds(headers),
            });
        } else if code == CODE_UUID_CONFLICT {
            return Some(FailureKind::UuidConflict);
        } else if code == CODE_INSTANCE_NOT_FOUND {
            return Some(FailureKind::InstanceNotFound);
        } else if (500..=599).contains(&status) {
            // 服务端 5xx 即使带业务码也按瞬态失败重试；只按业务码判 Fatal 会把
            // 网关/后端超时误判成永久的配置错误。
            return Some(FailureKind::Retry {
                retry_after_seconds: rate_limit_reset_seconds(headers),
            });
        } else {
            return Some(FailureKind::Fatal { code });
        }
    }

    match status {
        200..=299 => None,
        500..=599 => Some(FailureKind::Retry {
            retry_after_seconds: None,
        }),
        _ => Some(FailureKind::Fatal { code: 0 }),
    }
}

/// 频控建议等待秒数：先 `x-ogw-ratelimit-reset`，再 `Retry-After`。
pub fn rate_limit_reset_seconds(headers: &BTreeMap<String, String>) -> Option<u64> {
    headers
        .get("x-ogw-ratelimit-reset")
        .or_else(|| headers.get("retry-after"))
        .and_then(|value| value.trim().parse::<u64>().ok())
}

/// 从响应体里取业务码。非 JSON、缺 `code`、非整数一律 `None`。
pub fn business_code(body: &str) -> Option<i32> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    i32::try_from(value.get("code")?.as_i64()?).ok()
}

/// 截断响应体用于日志与 `last_error`（按字符截断，中文不会切在 UTF-8 边界）。
pub fn summarize(status: u16, body: &str) -> String {
    const LIMIT: usize = 500;
    let total = body.chars().count();
    let head: String = body.chars().take(LIMIT).collect();
    if total > LIMIT {
        format!("HTTP {status}: {head}…（共 {total} 字符，已截断）")
    } else {
        format!("HTTP {status}: {head}")
    }
}

/// 生成「跨轮去重」用的稳定文案：抹掉每轮变化的 `log_id` 值。
pub fn dedup_key(message: &str) -> String {
    const KEY: &str = "\"log_id\"";
    let mut out = String::with_capacity(message.len());
    let mut rest = message;
    while let Some(start) = rest.find(KEY) {
        out.push_str(&rest[..start]);
        let bytes = rest.as_bytes();
        let mut cursor = start + KEY.len();
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        let mut value_end = None;
        if bytes.get(cursor) == Some(&b':') {
            cursor += 1;
            while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            if bytes.get(cursor) == Some(&b'"') {
                let mut end = cursor + 1;
                while end < bytes.len() && bytes[end] != b'"' {
                    end += 1;
                }
                value_end = Some((end + 1).min(bytes.len()));
            }
        }
        match value_end {
            Some(end) => rest = &rest[end..],
            None => {
                out.push_str(&rest[start..cursor]);
                rest = &rest[cursor..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// 纯函数：把「失败类别 + 已用尝试数 + 是否幂等 + 策略」折成处置。
pub fn disposition(
    kind: &FailureKind,
    attempt: u64,
    idempotent: bool,
    policy: &RetryPolicy,
) -> Disposition {
    match kind {
        FailureKind::TokenExpired
        | FailureKind::UuidConflict
        | FailureKind::InstanceNotFound
        | FailureKind::Fatal { .. } => Disposition::Give,
        FailureKind::Retry {
            retry_after_seconds,
        } => {
            if !idempotent || attempt >= policy.max_attempts {
                return Disposition::Give;
            }
            Disposition::RetryAfter(policy.backoff_ms(attempt, *retry_after_seconds))
        }
    }
}

fn is_idempotent_outcome(code: Option<i32>) -> bool {
    code == Some(CODE_SUBSCRIPTION_EXISTED)
}

/// **唯一的重试入口**（crate 内部使用）。
///
/// 成功返回响应；失败返回最后一次的 [`Error`]，并保留分类让上层区分
/// 「token 失效」与「别放弃」。
pub(crate) async fn send_with_retry(
    transport: &dyn Transport,
    sleeper: &dyn Sleeper,
    request: &Request,
    policy: RetryPolicy,
) -> Result<Response> {
    let started = std::time::Instant::now();
    let mut attempt: u64 = 0;
    loop {
        attempt += 1;
        tracing::debug!(method = ?request.method, attempt, "飞书出站调用开始");
        let mut http_status = None;
        let mut code = None;
        let (kind, message) = match transport.send(request.clone()).await {
            Ok(response) => {
                http_status = Some(response.status);
                code = business_code(&response.body);
                match classify(response.status, &response.headers, &response.body) {
                    None => {
                        tracing::debug!(
                            method = ?request.method,
                            attempt,
                            http_status,
                            business_code = code,
                            duration_ms = started.elapsed().as_millis() as u64,
                            "飞书出站调用成功"
                        );
                        return Ok(response);
                    }
                    Some(kind) => (kind, summarize(response.status, &response.body)),
                }
            }
            Err(error) => (error.kind, error.message),
        };

        match disposition(&kind, attempt, request.idempotent, &policy) {
            Disposition::RetryAfter(wait_ms) => {
                tracing::warn!(
                    method = ?request.method,
                    http_status,
                    business_code = code,
                    attempt,
                    max_attempts = policy.max_attempts,
                    wait_ms,
                    ?kind,
                    duration_ms = started.elapsed().as_millis() as u64,
                    "飞书出站调用失败，退避后重试"
                );
                sleeper.sleep(Duration::from_millis(wait_ms)).await;
            }
            Disposition::Give => {
                if is_idempotent_outcome(code) {
                    tracing::debug!(
                        method = ?request.method,
                        http_status,
                        business_code = code,
                        attempt,
                        response = %message,
                        "飞书出站调用结束，幂等结果交给上层处理"
                    );
                } else if matches!(kind, FailureKind::InstanceNotFound) {
                    tracing::debug!(
                        method = ?request.method,
                        http_status,
                        business_code = code,
                        attempt,
                        max_attempts = policy.max_attempts,
                        ?kind,
                        response = %message,
                        duration_ms = started.elapsed().as_millis() as u64,
                        "飞书出站调用结束，实例暂不可查询，交给上层处理"
                    );
                    #[cfg(feature = "metrics")]
                    metrics::counter!("feishu_outbound_request_total", "result" => "failed")
                        .increment(1);
                } else {
                    tracing::warn!(
                        method = ?request.method,
                        http_status,
                        business_code = code,
                        attempt,
                        max_attempts = policy.max_attempts,
                        ?kind,
                        response = %message,
                        duration_ms = started.elapsed().as_millis() as u64,
                        "飞书出站调用结束，失败交给上层处理"
                    );
                    #[cfg(feature = "metrics")]
                    metrics::counter!("feishu_outbound_request_total", "result" => "failed")
                        .increment(1);
                }
                return Err(Error::new(kind, message));
            }
        }
    }
}

/// 取出飞书信封里的 `data` 并序列化为文本（审批/通讯录共用）。
#[allow(dead_code)] // 只在启用 typed 域 feature 时使用
pub(crate) fn decode_data(response: &str) -> Result<String> {
    #[derive(serde::Deserialize)]
    struct Envelope {
        #[serde(default)]
        data: Option<serde_json::Value>,
    }
    let envelope: Envelope = serde_json::from_str(response)
        .map_err(|error| Error::fatal(format!("响应信封解析失败: {error}")))?;
    let data = envelope
        .data
        .ok_or_else(|| Error::fatal("响应信封里没有 data"))?;
    serde_json::to_string(&data).map_err(|error| Error::fatal(format!("data 序列化失败: {error}")))
}

/// 权限类业务码（宿主可用它补运维文案）。
pub fn is_permission_denied_code(code: i32) -> bool {
    CODES_PERMISSION_DENIED.contains(&code)
}

/// 过大响应业务码（宿主可用它提示降低 page_size）。
pub fn is_too_large_response_code(code: i32) -> bool {
    code == CODE_TOO_LARGE_RESPONSE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    #[test]
    fn success_requires_zero_business_code() {
        assert_eq!(classify(200, &headers(&[]), r#"{"code":0}"#), None);
        assert_eq!(
            classify(200, &headers(&[]), r#"{"code":1254302}"#),
            Some(FailureKind::Fatal { code: 1254302 })
        );
    }

    #[test]
    fn token_codes_win_over_http_status() {
        assert_eq!(
            classify(400, &headers(&[]), r#"{"code":99991663}"#),
            Some(FailureKind::TokenExpired)
        );
        assert_eq!(
            classify(401, &headers(&[]), r#"{"code":99991663}"#),
            Some(FailureKind::TokenExpired)
        );
        assert_eq!(
            classify(400, &headers(&[]), r#"{"code":99991661}"#),
            Some(FailureKind::Fatal { code: 99991661 })
        );
    }

    #[test]
    fn rate_limit_reads_reset_header() {
        assert_eq!(
            classify(429, &headers(&[("x-ogw-ratelimit-reset", "7")]), ""),
            Some(FailureKind::Retry {
                retry_after_seconds: Some(7)
            })
        );
        assert_eq!(
            classify(200, &headers(&[]), r#"{"code":1254290}"#),
            Some(FailureKind::Retry {
                retry_after_seconds: None
            })
        );
    }

    #[test]
    fn approval_semantic_codes_are_distinct() {
        assert_eq!(
            classify(200, &headers(&[]), r#"{"code":60012}"#),
            Some(FailureKind::UuidConflict)
        );
        assert_eq!(
            classify(200, &headers(&[]), r#"{"code":1390003}"#),
            Some(FailureKind::InstanceNotFound)
        );
        assert_eq!(
            classify(200, &headers(&[]), r#"{"code":1395001}"#),
            Some(FailureKind::Retry {
                retry_after_seconds: None
            })
        );
    }

    #[test]
    fn unknown_business_errors_are_fatal() {
        assert_eq!(
            classify(200, &headers(&[]), r#"{"code":1254030}"#),
            Some(FailureKind::Fatal { code: 1254030 })
        );
        assert_eq!(
            classify(400, &headers(&[]), r#"{"code":1254024}"#),
            Some(FailureKind::Fatal { code: 1254024 })
        );
    }

    #[test]
    fn server_errors_with_business_codes_are_retryable() {
        assert_eq!(
            classify(503, &headers(&[]), r#"{"code":20072}"#),
            Some(FailureKind::Retry {
                retry_after_seconds: None
            })
        );
        assert_eq!(
            classify(200, &headers(&[]), r#"{"code":1254291}"#),
            Some(FailureKind::Retry {
                retry_after_seconds: None
            })
        );
    }

    #[test]
    fn status_is_only_the_fallback() {
        assert_eq!(classify(200, &headers(&[]), "not json"), None);
        assert_eq!(
            classify(503, &headers(&[]), "not json"),
            Some(FailureKind::Retry {
                retry_after_seconds: None
            })
        );
        assert_eq!(
            classify(404, &headers(&[]), "not json"),
            Some(FailureKind::Fatal { code: 0 })
        );
    }

    #[test]
    fn backoff_grows_then_clamps_and_server_hint_wins() {
        let policy = PULL_RETRY;
        assert_eq!(policy.backoff_ms(1, None), 500);
        assert_eq!(policy.backoff_ms(2, None), 1_000);
        assert_eq!(policy.backoff_ms(10, None), 4_000);
        assert_eq!(policy.backoff_ms(1, Some(2)), 2_000);
        assert_eq!(policy.backoff_ms(1, Some(999)), 30_000);
    }

    #[test]
    fn disposition_never_retries_non_idempotent_or_terminal() {
        let policy = PULL_RETRY;
        let retry = FailureKind::Retry {
            retry_after_seconds: None,
        };
        assert_eq!(disposition(&retry, 1, false, &policy), Disposition::Give);
        assert_eq!(disposition(&retry, 3, true, &policy), Disposition::Give);
        assert!(matches!(
            disposition(&retry, 1, true, &policy),
            Disposition::RetryAfter(_)
        ));
        assert_eq!(
            disposition(&FailureKind::TokenExpired, 1, true, &policy),
            Disposition::Give
        );
        assert_eq!(
            disposition(&FailureKind::UuidConflict, 1, true, &policy),
            Disposition::Give
        );
        assert_eq!(
            disposition(&FailureKind::InstanceNotFound, 1, true, &policy),
            Disposition::Give
        );
    }

    #[test]
    fn summarize_truncates_on_char_boundaries() {
        let body = "中".repeat(600);
        let summary = summarize(200, &body);
        assert!(summary.contains("共 600 字符，已截断"));
        assert!(summary.starts_with("HTTP 200: 中"));
    }

    #[test]
    fn dedup_key_ignores_volatile_log_id() {
        let left = r#"{"code":1,"msg":"x","log_id":"a"}"#;
        let right = r#"{"code":1,"msg":"x","log_id":"b"}"#;
        assert_eq!(dedup_key(left), dedup_key(right));
        assert!(!dedup_key(left).contains("\"a\""));
        assert!(!dedup_key(left).contains("log_id"));
    }

    #[test]
    fn request_debug_redacts_credentials_and_bodies() {
        let request = Request {
            method: Method::Post,
            url: "https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal".into(),
            query: vec![("page_token".into(), "p1".into())],
            bearer_token: Some("secret-bearer-token".into()),
            json_body: Some(serde_json::json!({"app_secret": "super-secret"})),
            raw_body: Some(("text/plain".into(), b"secret-raw-body".to_vec())),
            binary_limit: None,
            timeout_secs: Some(5),
            idempotent: true,
        };
        let debug = format!("{request:?}");
        assert!(!debug.contains("secret-bearer-token"));
        assert!(!debug.contains("super-secret"));
        assert!(!debug.contains("secret-raw-body"));
        assert!(debug.contains("已隐藏"));
    }

    #[test]
    fn lock_ttl_outlives_token_refresh_worst_case() {
        assert!(TOKEN_RETRY.worst_case_seconds(TOKEN_REQUEST_TIMEOUT_SECS) < 30);
    }
}
