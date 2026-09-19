//! HTTP 请求构建器实现
//!
//! 提供链式调用接口构建 HTTP 请求。

use crate::error::BaseError;
use crate::http::response::Response;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Client, Method};
use serde::Serialize;
use std::time::Duration;

/// 判断 HTTP 方法是否幂等（可默认安全重试）。
///
/// 依据 RFC 9110：GET/HEAD/OPTIONS/TRACE 为安全方法，PUT/DELETE 幂等；
/// POST/PATCH 非幂等，重复发送可能造成重复写入（重复扣款、重复插入等）。
///
/// `reqwest::Method` 的 `PartialEq` 是手写实现，故这里用常量相等比较，
/// 不使用 `matches!` 的结构性常量模式匹配。
fn is_idempotent_method(method: &Method) -> bool {
    *method == Method::GET
        || *method == Method::HEAD
        || *method == Method::OPTIONS
        || *method == Method::TRACE
        || *method == Method::PUT
        || *method == Method::DELETE
}

fn redact_url_for_log(url: &str) -> String {
    let Ok(mut parsed_url) = reqwest::Url::parse(url) else {
        return "<invalid-url>".to_string();
    };

    parsed_url.set_query(None);
    if !parsed_url.username().is_empty() {
        let _ = parsed_url.set_username("***");
    }
    if parsed_url.password().is_some() {
        let _ = parsed_url.set_password(Some("***"));
    }

    parsed_url.to_string()
}

/// 请求级重试策略配置（L-4）。
///
/// 对临时性失败（连接错误、可重试的 5xx 等）按指数退避自动重试。
/// 默认不重试——只有显式调用 [`RequestBuilder::retry`] 才启用。
///
/// 注意：当前仅实现「重试 + 指数退避 + 抖动」。熔断（circuit breaker）尚未实现，
/// 如需熔断请在调用方或网关层处理。
///
/// 非幂等方法（POST/PATCH）默认不参与重试——即使启用了重试也只发送一次；
/// 需要重试时须显式设置 [`RetryConfig::retry_non_idempotent`] 为 `true`，
/// 由调用方自行承担重复写入（重复扣款/重复插入）的风险。
///
/// 重试等待受 [`RetryConfig::total_budget_ms`] 总预算钳制，避免长退避把单次
/// `send()` 拖成不可控的长尾；另有 [`RetryConfig::jitter_percent`] 抖动去同步
/// 并发重试。本配置只在调用方显式调用 [`RequestBuilder::retry`] 时生效，
/// 应用层应另外配置 transport 的 `request_timeout`
/// （`AxumTransportConfig::request_timeout`，默认 `None`）作为最终兜底。
///
/// # 示例
///
/// ```rust,ignore
/// use yang_base::http::RetryConfig;
///
/// let cfg = RetryConfig {
///     max_retries: 3,
///     retry_on: vec![502, 503, 504],
///     backoff_ms: 100, // 第 n 次重试前等待 backoff_ms * 2^(n-1) 毫秒
///     retry_non_idempotent: false, // POST/PATCH 不重试
///     total_budget_ms: 30_000,     // 单次 send() 内全部退避等待的总预算
///     jitter_percent: 25,          // 退避抖动幅度 ±25%
/// };
/// ```
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// 最大重试次数（不含首次请求）。0 等价于不重试。
    pub max_retries: u32,
    /// 命中这些 HTTP 状态码时重试（如 `[502, 503, 504]`）。
    pub retry_on: Vec<u16>,
    /// 初始退避毫秒数，按 `backoff_ms * 2^attempt` 指数增长。
    pub backoff_ms: u64,
    /// 是否允许非幂等方法（POST/PATCH）重试。默认 `false`。
    ///
    /// 默认关闭时，非幂等请求即使启用了重试也只发送一次，避免
    /// 「服务端已处理但响应超时/连接中断」时重复扣款、重复插入。
    /// 仅当调用方确认接口幂等（如带幂等键）时才可置为 `true`。
    pub retry_non_idempotent: bool,
    /// 单次 `send()` 内全部重试等待的总时间预算（毫秒）。
    ///
    /// 每次退避等待取 `min(抖动后退避, 剩余预算)`；预算耗尽后不再等待，
    /// 直接返回最后一次请求的结果。必须大于 0（禁止「无预算」后门），
    /// 上限 300_000 毫秒（`MAX_TOTAL_BUDGET_MS`）。
    pub total_budget_ms: u64,
    /// 单次退避的抖动幅度（百分比 `0..=100`）。0 表示不抖动。
    ///
    /// 抖动用于打散并发请求的同步重试（惊群）：实际上限为初始退避的
    /// ±`jitter_percent`%，取 `SystemTime` 纳秒 + 进程号 + 重试序号混合的
    /// 非密码学熵源，不引入额外依赖。
    pub jitter_percent: u8,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_on: vec![502, 503, 504],
            backoff_ms: 100,
            retry_non_idempotent: false,
            total_budget_ms: 30_000,
            jitter_percent: 25,
        }
    }
}

impl RetryConfig {
    const MAX_RETRIES: u32 = 10;
    const MAX_BACKOFF_MS: u64 = 60_000;
    const MAX_TOTAL_BUDGET_MS: u64 = 300_000;

    /// 验证请求级重试策略。
    ///
    /// `max_retries == 0` 表示显式禁用重试，此时 `retry_on` 与 `backoff_ms` 不参与发送。
    pub fn validate(&self) -> Result<(), BaseError> {
        if self.max_retries > Self::MAX_RETRIES {
            return Err(BaseError::ParamInvalid(
                "http.retry.max_retries".to_string(),
                format!("最大重试次数不能超过 {}", Self::MAX_RETRIES),
            ));
        }

        if self.max_retries == 0 {
            return Ok(());
        }

        if self.retry_on.is_empty() {
            return Err(BaseError::ParamInvalid(
                "http.retry.retry_on".to_string(),
                "启用重试时 retry_on 不能为空".to_string(),
            ));
        }

        if let Some(status) = self
            .retry_on
            .iter()
            .copied()
            .find(|status| !(100..=599).contains(status))
        {
            return Err(BaseError::ParamInvalid(
                "http.retry.retry_on".to_string(),
                format!("非法 HTTP 状态码: {status}"),
            ));
        }

        if self.backoff_ms == 0 {
            return Err(BaseError::ParamInvalid(
                "http.retry.backoff_ms".to_string(),
                "启用重试时初始退避时间必须大于 0 毫秒".to_string(),
            ));
        }

        if self.backoff_ms > Self::MAX_BACKOFF_MS {
            return Err(BaseError::ParamInvalid(
                "http.retry.backoff_ms".to_string(),
                format!("初始退避时间不能超过 {} 毫秒", Self::MAX_BACKOFF_MS),
            ));
        }

        if self.total_budget_ms == 0 || self.total_budget_ms > Self::MAX_TOTAL_BUDGET_MS {
            return Err(BaseError::ParamInvalid(
                "http.retry.total_budget_ms".to_string(),
                format!("重试总预算必须在 1..={} 毫秒内", Self::MAX_TOTAL_BUDGET_MS),
            ));
        }

        if self.jitter_percent > 100 {
            return Err(BaseError::ParamInvalid(
                "http.retry.jitter_percent".to_string(),
                "退避抖动百分比不能超过 100".to_string(),
            ));
        }

        Ok(())
    }
}

/// 对退避时长施加 ±`jitter_percent`% 抖动，避免并发请求同步重试（惊群）。
///
/// 纯整数运算：`span = base_ms / 100 * jitter_percent`，返回值落在
/// `[base_ms - span, base_ms + span]`。熵源仅用于打散去同步，非密码学用途，
/// 故直接复用 `SystemTime` 纳秒 + 进程号 + 重试序号，不引入 `rand` 等依赖。
fn jittered_backoff(base_ms: u64, jitter_percent: u8, attempt: u32) -> u64 {
    let jp = u64::from(jitter_percent.min(100));
    if base_ms == 0 || jp == 0 {
        return base_ms;
    }

    let span = base_ms / 100 * jp;
    if span == 0 {
        return base_ms;
    }

    let entropy = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::from(d.subsec_nanos()))
        ^ u64::from(std::process::id())
        ^ u64::from(attempt);

    base_ms
        .saturating_sub(span)
        .saturating_add(entropy % span.saturating_mul(2).saturating_add(1))
}

/// HTTP 请求构建器
///
/// 提供链式调用接口构建 HTTP 请求。
///
/// # Header 错误处理
///
/// `header`、`headers` 等方法在解析失败时不会立即返回错误，而是将错误信息
/// 累积到内部 `header_errors` 列表中。调用 `send()` 时，若存在累积的 header
/// 错误，则返回 `BaseError::ParamInvalid("header", ...)` 而不发送请求。
///
/// # 示例
///
/// ```rust,ignore
/// // tools 为启动期经 ToolsBuilder::http(...) 冻结的应用资源；Action 内则用 ctx.http()?
/// let response = tools.http()?
///     .get("https://api.example.com/users")
///     .header("X-Custom-Header", "value")
///     .query("page", "1")
///     .bearer_token("your_token")
///     .timeout(60)
///     .send()
///     .await?;
/// ```
pub struct RequestBuilder {
    /// reqwest 客户端（Arc 包装，clone 时复用同一连接池）
    client: Client,

    /// HTTP 方法
    method: Method,

    /// 请求 URL
    url: String,

    /// 请求头
    headers: HeaderMap,

    /// 累积的 header 解析错误列表
    ///
    /// `header`、`headers` 方法在解析失败时将错误描述追加到此列表，
    /// `send()` 时若非空则返回 `BaseError::ParamInvalid`。
    header_errors: Vec<String>,

    /// 查询参数
    query_params: Vec<(String, String)>,

    /// 请求体
    body: Option<Vec<u8>>,

    /// 超时时间
    timeout: Duration,

    /// 出站响应体最大允许字节数（来自 HttpClientConfig）
    max_response_bytes: usize,

    /// Token（可选）
    token: Option<String>,

    /// 重试策略（可选，默认不重试）
    retry: Option<RetryConfig>,

    /// 熔断器（可选，默认 None）。来自创建该构建器的 HttpClient，共享状态。
    circuit_breaker: Option<crate::http::CircuitBreaker>,
}

impl RequestBuilder {
    /// 创建新的请求构建器
    ///
    /// # 参数
    ///
    /// - `client`: reqwest 客户端（Arc 包装，clone 时复用同一连接池）
    /// - `method`: HTTP 方法
    /// - `url`: 请求 URL
    /// - `timeout`: 超时时间
    /// - `max_response_bytes`: 出站响应体最大允许字节数
    /// - `token`: 默认 Token
    /// - `circuit_breaker`: 熔断器（可选，来自 HttpClient）
    pub(crate) fn new(
        client: Client,
        method: Method,
        url: String,
        timeout: Duration,
        max_response_bytes: usize,
        token: Option<String>,
        circuit_breaker: Option<crate::http::CircuitBreaker>,
    ) -> Self {
        Self {
            client,
            method,
            url,
            headers: HeaderMap::new(),
            header_errors: Vec::new(),
            query_params: Vec::new(),
            body: None,
            timeout,
            max_response_bytes,
            token,
            retry: None,
            circuit_breaker,
        }
    }

    /// 设置请求头
    ///
    /// 若 header 名称或值解析失败，错误信息将被累积到内部错误列表，
    /// 不会立即返回错误。调用 `send()` 时若存在累积错误则返回 `BaseError::ParamInvalid`。
    ///
    /// # 参数
    ///
    /// - `name`: 请求头名称
    /// - `value`: 请求头值
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let response = client
    ///     .get("https://api.example.com/users")
    ///     .header("X-Custom-Header", "value")
    ///     .send()
    ///     .await?;
    /// ```
    pub fn header(mut self, name: &str, value: &str) -> Self {
        // 解析 header 名称
        let header_name = match HeaderName::from_bytes(name.as_bytes()) {
            Ok(n) => n,
            Err(e) => {
                // 累积错误，不立即返回
                self.header_errors
                    .push(format!("非法 header 名称 '{}': {}", name, e));
                return self;
            }
        };

        // 解析 header 值
        let header_value = match HeaderValue::from_str(value) {
            Ok(v) => v,
            Err(e) => {
                // 累积错误，不立即返回
                self.header_errors
                    .push(format!("非法 header 值 '{}': {}", value, e));
                return self;
            }
        };

        self.headers.insert(header_name, header_value);
        self
    }

    /// 批量设置请求头
    ///
    /// 若任意 header 名称或值解析失败，错误信息将被累积到内部错误列表，
    /// 不会立即返回错误。调用 `send()` 时若存在累积错误则返回 `BaseError::ParamInvalid`。
    ///
    /// # 参数
    ///
    /// - `headers`: 请求头列表
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let response = client
    ///     .get("https://api.example.com/users")
    ///     .headers(vec![
    ///         ("X-Custom-Header-1", "value1"),
    ///         ("X-Custom-Header-2", "value2"),
    ///     ])
    ///     .send()
    ///     .await?;
    /// ```
    pub fn headers(mut self, headers: Vec<(&str, &str)>) -> Self {
        for (name, value) in headers {
            // 解析 header 名称
            let header_name = match HeaderName::from_bytes(name.as_bytes()) {
                Ok(n) => n,
                Err(e) => {
                    // 累积错误，继续处理其余 header
                    self.header_errors
                        .push(format!("非法 header 名称 '{}': {}", name, e));
                    continue;
                }
            };

            // 解析 header 值
            let header_value = match HeaderValue::from_str(value) {
                Ok(v) => v,
                Err(e) => {
                    // 累积错误，继续处理其余 header
                    self.header_errors
                        .push(format!("非法 header 值 '{}': {}", value, e));
                    continue;
                }
            };

            self.headers.insert(header_name, header_value);
        }
        self
    }

    /// 设置 Content-Type
    ///
    /// # 参数
    ///
    /// - `content_type`: Content-Type 值
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let response = client
    ///     .post("https://api.example.com/users")
    ///     .content_type("application/json")
    ///     .send()
    ///     .await?;
    /// ```
    pub fn content_type(self, content_type: &str) -> Self {
        self.header("Content-Type", content_type)
    }

    /// 设置 Bearer Token
    ///
    /// # 参数
    ///
    /// - `token`: Token 字符串
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let response = client
    ///     .get("https://api.example.com/users")
    ///     .bearer_token("your_token")
    ///     .send()
    ///     .await?;
    /// ```
    pub fn bearer_token(mut self, token: &str) -> Self {
        self.token = Some(token.to_string());
        self
    }

    /// 设置 User-Agent
    ///
    /// # 参数
    ///
    /// - `user_agent`: User-Agent 值
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let response = client
    ///     .get("https://api.example.com/users")
    ///     .user_agent("MyApp/1.0")
    ///     .send()
    ///     .await?;
    /// ```
    pub fn user_agent(self, user_agent: &str) -> Self {
        self.header("User-Agent", user_agent)
    }

    /// 添加查询参数
    ///
    /// # 参数
    ///
    /// - `key`: 参数名
    /// - `value`: 参数值
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let response = client
    ///     .get("https://api.example.com/users")
    ///     .query("page", "1")
    ///     .query("limit", "10")
    ///     .send()
    ///     .await?;
    /// ```
    pub fn query(mut self, key: &str, value: &str) -> Self {
        self.query_params.push((key.to_string(), value.to_string()));
        self
    }

    /// 批量添加查询参数
    ///
    /// # 参数
    ///
    /// - `params`: 查询参数列表
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let response = client
    ///     .get("https://api.example.com/users")
    ///     .queries(vec![("page", "1"), ("limit", "10")])
    ///     .send()
    ///     .await?;
    /// ```
    pub fn queries(mut self, params: Vec<(&str, &str)>) -> Self {
        for (key, value) in params {
            self.query_params.push((key.to_string(), value.to_string()));
        }
        self
    }

    /// 设置 JSON 请求体
    ///
    /// # 参数
    ///
    /// - `json`: 可序列化为 JSON 的数据
    ///
    /// # 返回
    ///
    /// - `Ok(Self)`: 设置成功
    /// - `Err(BaseError)`: 序列化失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// #[derive(Serialize)]
    /// struct User {
    ///     name: String,
    ///     email: String,
    /// }
    ///
    /// let user = User {
    ///     name: "Alice".to_string(),
    ///     email: "alice@example.com".to_string(),
    /// };
    ///
    /// let response = client
    ///     .post("https://api.example.com/users")
    ///     .json(&user)?
    ///     .send()
    ///     .await?;
    /// ```
    pub fn json<T: Serialize>(mut self, json: &T) -> Result<Self, BaseError> {
        let json_bytes =
            serde_json::to_vec(json).map_err(|e| BaseError::JsonSerializeFailed(e.to_string()))?;

        self.body = Some(json_bytes);
        self = self.content_type("application/json");

        Ok(self)
    }

    /// 设置表单请求体
    ///
    /// 使用 `serde_urlencoded` 对表单数据进行 URL 编码，正确处理特殊字符、
    /// 空格、UTF-8 字符等，并自动设置 `Content-Type: application/x-www-form-urlencoded`。
    ///
    /// # 参数
    ///
    /// - `form`: 表单数据（键值对列表）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let response = client
    ///     .post("https://api.example.com/login")
    ///     .form(vec![("username", "alice"), ("password", "secret")])
    ///     .send()
    ///     .await?;
    /// ```
    pub fn form(mut self, form: Vec<(&str, &str)>) -> Self {
        // 使用 serde_urlencoded 进行标准 URL 编码，正确处理特殊字符
        match serde_urlencoded::to_string(&form) {
            Ok(encoded) => {
                self.body = Some(encoded.into_bytes());
                // 自动设置 Content-Type
                self = self.content_type("application/x-www-form-urlencoded");
            }
            Err(e) => {
                // 编码失败时累积错误
                self.header_errors.push(format!("表单数据编码失败: {}", e));
            }
        }
        self
    }

    /// 设置原始字节请求体
    ///
    /// # 参数
    ///
    /// - `body`: 字节数据
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let response = client
    ///     .post("https://api.example.com/upload")
    ///     .body(vec![0x00, 0x01, 0x02])
    ///     .send()
    ///     .await?;
    /// ```
    pub fn body(mut self, body: Vec<u8>) -> Self {
        self.body = Some(body);
        self
    }

    /// 设置文本请求体
    ///
    /// # 参数
    ///
    /// - `text`: 文本数据
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let response = client
    ///     .post("https://api.example.com/notes")
    ///     .text("Hello, World!")
    ///     .send()
    ///     .await?;
    /// ```
    pub fn text(mut self, text: &str) -> Self {
        self.body = Some(text.as_bytes().to_vec());
        self = self.content_type("text/plain");
        self
    }

    /// 设置超时时间
    ///
    /// # 参数
    ///
    /// - `timeout_secs`: 超时时间（秒）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let response = client
    ///     .get("https://api.example.com/users")
    ///     .timeout(60)
    ///     .send()
    ///     .await?;
    /// ```
    pub fn timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout = Duration::from_secs(timeout_secs);
        self
    }

    /// 设置出站响应体最大允许字节数。
    ///
    /// 与 `HttpClientConfig::max_response_bytes` 语义一致：读取响应体时超限
    /// 返回 `BaseError::HttpResponseTooLarge`。设为 0 会在 `send()` 前被拒绝。
    ///
    /// # 参数
    ///
    /// - `bytes`: 响应体最大允许字节数（必须大于 0）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let response = client
    ///     .get("https://api.example.com/users")
    ///     .max_response_bytes(1024 * 1024)
    ///     .send()
    ///     .await?;
    /// ```
    pub fn max_response_bytes(mut self, bytes: usize) -> Self {
        self.max_response_bytes = bytes;
        self
    }

    /// 设置请求级重试策略（L-4）。
    ///
    /// 默认不重试。启用后，对连接错误与命中 `retry_on` 的状态码按指数退避重试。
    ///
    /// 非幂等方法（POST/PATCH）默认不参与重试，需
    /// [`RetryConfig::retry_non_idempotent`] 置为 `true` 才显式承担重复写风险。
    ///
    /// # 参数
    ///
    /// - `config`: 重试策略
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::http::RetryConfig;
    ///
    /// let response = client
    ///     .get("https://api.example.com/users")
    ///     .retry(RetryConfig::default())
    ///     .send()
    ///     .await?;
    /// ```
    pub fn retry(mut self, config: RetryConfig) -> Self {
        self.retry = Some(config);
        self
    }

    /// 发送请求
    ///
    /// 在发送前检查累积的 header 错误。若存在任何 header 解析错误，
    /// 则返回 `BaseError::ParamInvalid("header", ...)` 而不发送请求。
    ///
    /// # 返回
    ///
    /// - `Ok(Response)`: 响应对象
    /// - `Err(BaseError::ParamInvalid)`: 存在非法 header
    /// - `Err(BaseError::HttpRequestFailed)`: 请求发送失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let response = client
    ///     .get("https://api.example.com/users")
    ///     .send()
    ///     .await?;
    /// ```
    pub async fn send(self) -> Result<Response, BaseError> {
        // 检查累积的 header 错误，若非空则提前返回错误
        if !self.header_errors.is_empty() {
            return Err(BaseError::ParamInvalid(
                "header".to_string(),
                self.header_errors.join("; "),
            ));
        }

        if self
            .query_params
            .iter()
            .any(|(key, _)| key.trim().is_empty())
        {
            return Err(BaseError::ParamInvalid(
                "query".to_string(),
                "query 参数名称不能为空".to_string(),
            ));
        }

        let parsed_url = reqwest::Url::parse(&self.url).map_err(|err| {
            BaseError::ParamInvalid("url".to_string(), format!("非法 HTTP URL: {}", err))
        })?;
        if !matches!(parsed_url.scheme(), "http" | "https") {
            return Err(BaseError::ParamInvalid(
                "url".to_string(),
                "HTTP 客户端仅支持 http 和 https URL".to_string(),
            ));
        }

        if let Some(token) = &self.token {
            let auth_value = format!("Bearer {}", token);
            if let Err(err) = HeaderValue::from_str(&auth_value) {
                return Err(BaseError::ParamInvalid(
                    "authorization".to_string(),
                    format!("非法 bearer token，无法构造 Authorization 头: {}", err),
                ));
            }
        }

        if self.timeout.is_zero() {
            return Err(BaseError::ParamInvalid(
                "http.timeout_secs".to_string(),
                "HTTP 请求超时时间必须大于 0 秒".to_string(),
            ));
        }

        if self.max_response_bytes == 0 {
            return Err(BaseError::ParamInvalid(
                "http.max_response_bytes".to_string(),
                "HTTP 响应体上限必须大于 0 字节".to_string(),
            ));
        }

        if let Some(retry) = &self.retry {
            retry.validate()?;
        }

        let retry = self.retry.clone();

        // 解析目标 host 用于熔断分键；无熔断器或解析失败时为 None（按无熔断处理）。
        let host = self
            .circuit_breaker
            .as_ref()
            .and_then(|_| parsed_url.host_str().map(|h| h.to_string()));

        // 无重试策略：单次发送（与原行为一致，仅多一层熔断准入）
        let Some(retry) = retry else {
            return self.send_guarded(host.as_deref()).await;
        };

        // 非幂等方法未显式 opt-in：退化为单次发送。
        // 该闸门同时覆盖状态码重试分支与传输错误重试分支——POST/PATCH 一旦命中
        // `retry_on` 或连接中断同样会重放已生效的写入，须一并拦截。
        if !is_idempotent_method(&self.method) && !retry.retry_non_idempotent {
            return self.send_guarded(host.as_deref()).await;
        }

        // 有重试策略：最多发送 1 + max_retries 次。
        // `remaining_budget_ms` 是本轮 `send()` 内全部退避等待共享的总预算。
        let mut attempt: u32 = 0;
        let mut remaining_budget_ms = retry.total_budget_ms;
        loop {
            let result = self.send_guarded(host.as_deref()).await;

            let should_retry = match &result {
                // 命中可重试状态码
                Ok(resp) => retry.retry_on.contains(&resp.status()),
                // 连接/超时等传输错误也重试
                Err(BaseError::HttpRequestFailed(_)) => true,
                // 熔断打开（HttpCircuitBreakerOpen）等其它错误不重试
                Err(_) => false,
            };

            if !should_retry || attempt >= retry.max_retries {
                return result;
            }

            // 指数退避 backoff_ms * 2^attempt，叠加抖动，再受剩余预算钳制：
            // 预算耗尽（钳制后为 0）时不再等待，直接返回最后一次结果，避免
            // 「重试 + 长退避」把单次 send() 拖成长尾。
            let base = retry.backoff_ms.saturating_mul(1u64 << attempt.min(20));
            let sleep_ms =
                jittered_backoff(base, retry.jitter_percent, attempt).min(remaining_budget_ms);
            if sleep_ms == 0 {
                return result;
            }

            tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
            remaining_budget_ms -= sleep_ms;
            attempt += 1;
        }
    }

    /// 在熔断器准入检查下发送一次请求，并按结果记录成功/失败。
    ///
    /// - 准入：若熔断器对该 host 处于 Open（且未冷却），直接返回
    ///   `BaseError::HttpCircuitBreakerOpen`，不实际发请求。
    /// - 记账：传输错误与 5xx 视为失败，2xx/3xx/4xx 视为成功。
    ///
    /// 当 `host` 为 `None`（无熔断器或 URL 解析不出 host）时，等价于直接 `send_once`。
    async fn send_guarded(&self, host: Option<&str>) -> Result<Response, BaseError> {
        let breaker = self.circuit_breaker.as_ref();

        if let (Some(breaker), Some(host)) = (breaker, host) {
            if !breaker.allow(host) {
                return Err(BaseError::HttpCircuitBreakerOpen(host.to_string()));
            }
        }

        let result = self.send_once().await;

        if let (Some(breaker), Some(host)) = (breaker, host) {
            match &result {
                Ok(resp) if resp.status() >= 500 => breaker.on_failure(host),
                Ok(_) => breaker.on_success(host),
                Err(_) => breaker.on_failure(host),
            }
        }

        result
    }

    /// 构建并发送一次请求（不含重试）。
    ///
    /// 因重试需要重复发送，这里借用 `&self` 并克隆可复用的请求部件
    /// （headers / query / body 均可 clone；`client` 为 `Arc`，clone 复用连接池）。
    async fn send_once(&self) -> Result<Response, BaseError> {
        // 构建请求
        // 注意：self.client.clone() 是 Arc::clone，复用同一底层连接池，不创建新的 TCP 连接池
        let mut request = self
            .client
            .request(self.method.clone(), &self.url)
            .timeout(self.timeout);

        // 添加请求头
        request = request.headers(self.headers.clone());

        // 添加 Token
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }

        // 添加查询参数
        if !self.query_params.is_empty() {
            request = request.query(&self.query_params);
        }

        // 添加请求体
        if let Some(body) = &self.body {
            request = request.body(body.clone());
        }

        // 发送请求（NG-1：记录方法/URL/状态码/耗时，便于排查外部 API 慢响应与失败）。
        // URL 日志脱敏：移除 query，并隐藏 userinfo，避免泄漏 token/password。
        let start = std::time::Instant::now();
        let result = request.send().await;
        let elapsed_ms = start.elapsed().as_millis();
        let log_url = redact_url_for_log(&self.url);
        match &result {
            Ok(resp) => {
                tracing::debug!(
                    target: "yang_base::http",
                    method = %self.method,
                    url = %log_url,
                    status = resp.status().as_u16(),
                    elapsed_ms = elapsed_ms as u64,
                    "HTTP 出站请求完成"
                );
            }
            Err(e) => {
                tracing::warn!(
                    target: "yang_base::http",
                    method = %self.method,
                    url = %log_url,
                    elapsed_ms = elapsed_ms as u64,
                    error = %e,
                    "HTTP 出站请求失败"
                );
            }
        }
        let response = result.map_err(BaseError::HttpRequestFailed)?;

        Ok(Response::new(response, self.max_response_bytes))
    }
}

#[cfg(test)]
mod retry_config_tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn test_redact_url_for_log_removes_query_and_userinfo() {
        let redacted = redact_url_for_log("https://user:secret@example.com/path?token=secret&x=1");

        assert_eq!(redacted, "https://***:***@example.com/path");
        assert!(!redacted.contains("token"));
        assert!(!redacted.contains("secret"));
        assert!(!redacted.contains("user"));
    }

    #[test]
    fn test_retry_config_validate_rejects_unsafe_values() {
        let invalid_configs = [
            RetryConfig {
                max_retries: 11,
                ..RetryConfig::default()
            },
            RetryConfig {
                max_retries: 1,
                retry_on: Vec::new(),
                ..RetryConfig::default()
            },
            RetryConfig {
                max_retries: 1,
                backoff_ms: 0,
                ..RetryConfig::default()
            },
            RetryConfig {
                max_retries: 1,
                backoff_ms: 60_001,
                ..RetryConfig::default()
            },
            RetryConfig {
                max_retries: 1,
                retry_on: vec![99],
                ..RetryConfig::default()
            },
            RetryConfig {
                max_retries: 1,
                retry_on: vec![600],
                ..RetryConfig::default()
            },
        ];

        for config in invalid_configs {
            assert!(matches!(
                config.validate(),
                Err(BaseError::ParamInvalid(_, _))
            ));
        }
    }

    #[test]
    fn test_retry_config_validate_rejects_bad_budget_and_jitter() {
        let cases = [
            // 总预算不能为 0（不留「无预算」后门）
            (
                "http.retry.total_budget_ms",
                RetryConfig {
                    max_retries: 1,
                    total_budget_ms: 0,
                    ..RetryConfig::default()
                },
            ),
            // 总预算不能超过上限
            (
                "http.retry.total_budget_ms",
                RetryConfig {
                    max_retries: 1,
                    total_budget_ms: RetryConfig::MAX_TOTAL_BUDGET_MS + 1,
                    ..RetryConfig::default()
                },
            ),
            // 抖动百分比不能超过 100
            (
                "http.retry.jitter_percent",
                RetryConfig {
                    max_retries: 1,
                    jitter_percent: 101,
                    ..RetryConfig::default()
                },
            ),
        ];

        for (expected_field, config) in cases {
            let err = match config.validate() {
                Ok(()) => panic!("非法 {expected_field} 应被 validate() 拒绝"),
                Err(err) => err,
            };
            assert!(
                matches!(&err, BaseError::ParamInvalid(field, _) if field == expected_field),
                "期望 ParamInvalid({expected_field})，实际 {err:?}"
            );
        }
    }

    #[test]
    fn test_retry_config_default_and_boundaries_are_valid() {
        assert!(
            RetryConfig::default().validate().is_ok(),
            "默认配置必须通过校验"
        );

        // 边界：预算取上限、抖动取 100 均为合法值
        assert!(RetryConfig {
            max_retries: 1,
            total_budget_ms: RetryConfig::MAX_TOTAL_BUDGET_MS,
            jitter_percent: 100,
            ..RetryConfig::default()
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn test_jittered_backoff_identity_and_bounds() {
        // jitter_percent = 0：恒等返回 base
        for base in [0_u64, 1, 99, 100, 1_000, u64::MAX] {
            assert_eq!(
                jittered_backoff(base, 0, 0),
                base,
                "jitter_percent=0 时退避时长不应改变"
            );
        }

        // base = 0：任何抖动都只能得到 0
        assert_eq!(jittered_backoff(0, 100, 3), 0);

        // span 向下取整为 0（base < 100）：不做抖动，避免下溢
        assert_eq!(jittered_backoff(50, 1, 0), 50);

        // jitter_percent = 100：结果落在 [base - span, base + span]
        let base = 1_000_u64;
        let span = base / 100 * 100;
        for attempt in 0..64 {
            let got = jittered_backoff(base, 100, attempt);
            assert!(
                (base - span..=base + span).contains(&got),
                "第 {attempt} 次抖动的退避 {got} 应落在 [{}, {}]",
                base - span,
                base + span
            );
        }
    }

    /// 起一个对每个请求都回 `503` 的本地监听器，用于验证重试次数与耗时。
    ///
    /// 返回 `(监听地址, 请求计数)`；监听线程随测试结束由进程回收。
    fn spawn_503_listener() -> (
        std::net::SocketAddr,
        std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) {
        use std::io::{Read, Write};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("本地监听端口应可绑定");
        let addr = listener.local_addr().expect("应能取得监听地址");
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_for_thread = Arc::clone(&counter);

        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                counter_for_thread.fetch_add(1, Ordering::SeqCst);
                // 读走请求再回 503，避免客户端写请求时收到 EPIPE
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(
                    b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
                let _ = stream.flush();
            }
        });

        (addr, counter)
    }

    /// 预算钳制：总预算小于首轮退避时只睡剩余预算，随后立即返回最后一次结果。
    #[tokio::test]
    async fn test_send_clamps_backoff_wait_to_total_budget() {
        let (addr, counter) = spawn_503_listener();
        let client = crate::http::HttpClient::new(30).expect("HTTP client should be valid");

        let started = std::time::Instant::now();
        let result = client
            .get(&format!("http://{addr}/always-503"))
            .retry(RetryConfig {
                max_retries: 3,
                retry_on: vec![503],
                backoff_ms: 1_000,
                jitter_percent: 0,
                total_budget_ms: 5,
                ..RetryConfig::default()
            })
            .send()
            .await;
        let elapsed = started.elapsed();

        // 预算耗尽不等于丢结果：仍须把最后一次响应（503）返回给调用方
        let resp = match result {
            Ok(resp) => resp,
            Err(err) => panic!("预算耗尽后应返回最后一次响应而非错误: {err:?}"),
        };
        assert_eq!(resp.status(), 503);

        // 未钳制时退避合计 1000 + 2000 + 4000 = 7000ms；钳制后只有 1 次 5ms 等待
        assert!(
            elapsed < Duration::from_millis(500),
            "退避等待应被总预算钳制（未钳制需约 7s），实际耗时 {elapsed:?}"
        );

        // 等监听线程计数落定：预算耗尽后不应再发第 3 次请求
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(
            counter.load(Ordering::SeqCst),
            2,
            "预算耗尽后应停止重试（首次 + 1 次重试共 2 次请求）"
        );
    }

    #[tokio::test]
    async fn test_send_rejects_invalid_retry_config_before_network() {
        let builder = RequestBuilder::new(
            Client::new(),
            Method::GET,
            "http://127.0.0.1:1".to_string(),
            Duration::from_secs(30),
            10 * 1024 * 1024,
            None,
            None,
        )
        .retry(RetryConfig {
            max_retries: 11,
            ..RetryConfig::default()
        });

        let err = match builder.send().await {
            Ok(_) => panic!("无效 retry 配置应在网络请求前被拒绝"),
            Err(err) => err,
        };

        assert!(
            matches!(err, BaseError::ParamInvalid(field, _) if field == "http.retry.max_retries")
        );
    }

    #[tokio::test]
    async fn test_send_rejects_zero_request_timeout_before_network() {
        let builder = RequestBuilder::new(
            Client::new(),
            Method::GET,
            "http://127.0.0.1:1".to_string(),
            Duration::from_secs(30),
            10 * 1024 * 1024,
            None,
            None,
        )
        .timeout(0);

        let err = match builder.send().await {
            Ok(_) => panic!("0 秒请求超时应在网络请求前被拒绝"),
            Err(err) => err,
        };

        assert!(matches!(err, BaseError::ParamInvalid(field, _) if field == "http.timeout_secs"));
    }

    #[tokio::test]
    async fn test_send_rejects_blank_query_key_before_network() {
        let client = crate::http::HttpClient::new(30).expect("HTTP client should be valid");
        let result = client
            .get("https://api.example.com/users")
            .query("   ", "value")
            .send()
            .await;

        let err = match result {
            Err(err) => err,
            Ok(_) => panic!("空白 query key 应在发送前被拒绝"),
        };

        assert!(matches!(
            err,
            crate::error::BaseError::ParamInvalid(field, _) if field == "query"
        ));
    }

    #[tokio::test]
    async fn test_send_rejects_invalid_bearer_token_before_network() {
        let builder = RequestBuilder::new(
            Client::new(),
            Method::GET,
            "http://127.0.0.1:1".to_string(),
            Duration::from_secs(30),
            10 * 1024 * 1024,
            Some("bad\r\ntoken".to_string()),
            None,
        );

        let err = match builder.send().await {
            Ok(_) => panic!("非法 bearer token 应在网络请求前被拒绝"),
            Err(err) => err,
        };

        assert!(matches!(err, BaseError::ParamInvalid(field, _) if field == "authorization"));
    }

    #[tokio::test]
    async fn test_send_rejects_invalid_url_before_network() {
        let invalid_urls = ["not a url", "ftp://example.com/file"];

        for url in invalid_urls {
            let builder = RequestBuilder::new(
                Client::new(),
                Method::GET,
                url.to_string(),
                Duration::from_secs(30),
                10 * 1024 * 1024,
                None,
                None,
            );

            let err = match builder.send().await {
                Ok(_) => panic!("非法 URL 应在网络请求前被拒绝"),
                Err(err) => err,
            };

            assert!(matches!(err, BaseError::ParamInvalid(field, _) if field == "url"));
        }
    }

    #[test]
    fn test_is_idempotent_method_classification() {
        // 安全/幂等方法：默认允许重试
        for method in [
            Method::GET,
            Method::HEAD,
            Method::OPTIONS,
            Method::TRACE,
            Method::PUT,
            Method::DELETE,
        ] {
            assert!(
                is_idempotent_method(&method),
                "{method} 应被判定为幂等/安全方法"
            );
        }

        // 非幂等方法：默认不参与重试
        for method in [Method::POST, Method::PATCH] {
            assert!(
                !is_idempotent_method(&method),
                "{method} 不应被判定为幂等方法"
            );
        }
    }

    /// 起一个只计数、读完即断开的本地监听器，用于统计实际发起的请求次数。
    ///
    /// 返回 `(监听地址, 连接计数)`；监听线程随测试结束由进程回收。
    fn spawn_closing_listener() -> (
        std::net::SocketAddr,
        std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) {
        use std::io::Read;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("本地监听端口应可绑定");
        let addr = listener.local_addr().expect("应能取得监听地址");
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_for_thread = Arc::clone(&counter);

        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                counter_for_thread.fetch_add(1, Ordering::SeqCst);
                // 读走请求后直接关闭连接，制造传输错误（而非可重试状态码）
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
            }
        });

        (addr, counter)
    }

    /// 非幂等的 POST 未 opt-in 时，即使启用了重试也只发送一次。
    /// 标记 ignore：发起真实连接，依赖网络栈行为，默认跳过。
    #[tokio::test]
    #[ignore = "依赖网络栈行为，发起真实连接；默认跳过"]
    async fn test_post_not_retried_without_opt_in() {
        let (addr, counter) = spawn_closing_listener();
        let client = crate::http::HttpClient::new(1).expect("HTTP client should be valid");

        let result = client
            .post(&format!("http://{addr}/never"))
            .retry(RetryConfig {
                max_retries: 3,
                retry_on: vec![503],
                backoff_ms: 1,
                retry_non_idempotent: false,
                ..RetryConfig::default()
            })
            .send()
            .await;

        assert!(result.is_err(), "监听器直接断开，应返回传输错误");
        // 给监听线程留出计数落定的时间
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "非幂等 POST 未 opt-in 时只应发送一次"
        );
    }

    /// 非幂等的 POST 显式 opt-in 后，重试仍然生效（耗尽 max_retries）。
    /// 标记 ignore：发起真实连接，依赖网络栈行为，默认跳过。
    #[tokio::test]
    #[ignore = "依赖网络栈行为，发起真实连接；默认跳过"]
    async fn test_post_retried_when_opted_in() {
        let (addr, counter) = spawn_closing_listener();
        let client = crate::http::HttpClient::new(1).expect("HTTP client should be valid");

        let result = client
            .post(&format!("http://{addr}/never"))
            .retry(RetryConfig {
                max_retries: 3,
                retry_on: vec![503],
                backoff_ms: 1,
                retry_non_idempotent: true,
                ..RetryConfig::default()
            })
            .send()
            .await;

        assert!(result.is_err(), "监听器直接断开，应返回传输错误");
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(
            counter.load(Ordering::SeqCst),
            4,
            "opt-in 后应发送 1 + max_retries 次"
        );
    }
}
