//! HTTP 请求构建器实现
//!
//! 提供链式调用接口构建 HTTP 请求。

use crate::error::BaseError;
use crate::http::response::Response;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Client, Method};
use serde::Serialize;
use std::time::Duration;

/// 请求级重试策略配置（L-4）。
///
/// 对临时性失败（连接错误、可重试的 5xx 等）按指数退避自动重试。
/// 默认不重试——只有显式调用 [`RequestBuilder::retry`] 才启用。
///
/// 注意：当前仅实现「重试 + 指数退避」。熔断（circuit breaker）尚未实现，
/// 如需熔断请在调用方或网关层处理。
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
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_on: vec![502, 503, 504],
            backoff_ms: 100,
        }
    }
}

impl RetryConfig {
    const MAX_RETRIES: u32 = 10;
    const MAX_BACKOFF_MS: u64 = 60_000;

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

        Ok(())
    }
}

#[cfg(test)]
mod retry_config_tests {
    use super::*;

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
            let err = config
                .validate()
                .expect_err("不安全 retry 配置应被拒绝");

            assert!(matches!(err, BaseError::ParamInvalid(_, _)));
        }
    }

    #[tokio::test]
    async fn test_send_rejects_invalid_retry_config_before_network() {
        let builder = RequestBuilder::new(
            Client::new(),
            Method::GET,
            "http://127.0.0.1:1".to_string(),
            Duration::from_secs(30),
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

        assert!(matches!(err, BaseError::ParamInvalid(field, _) if field == "http.retry.max_retries"));
    }

    #[tokio::test]
    async fn test_send_rejects_zero_request_timeout_before_network() {
        let builder = RequestBuilder::new(
            Client::new(),
            Method::GET,
            "http://127.0.0.1:1".to_string(),
            Duration::from_secs(30),
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
    async fn test_send_rejects_invalid_bearer_token_before_network() {
        let builder = RequestBuilder::new(
            Client::new(),
            Method::GET,
            "http://127.0.0.1:1".to_string(),
            Duration::from_secs(30),
            Some("bad\r\ntoken".to_string()),
            None,
        );

        let err = match builder.send().await {
            Ok(_) => panic!("非法 bearer token 应在网络请求前被拒绝"),
            Err(err) => err,
        };

        assert!(matches!(err, BaseError::ParamInvalid(field, _) if field == "authorization"));
    }
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
/// use yang_base::http::HttpClient;
///
/// let response = HttpClient::global()?
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
    /// - `token`: 默认 Token
    /// - `circuit_breaker`: 熔断器（可选，来自 HttpClient）
    pub(crate) fn new(
        client: Client,
        method: Method,
        url: String,
        timeout: Duration,
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

    /// 设置请求级重试策略（L-4）。
    ///
    /// 默认不重试。启用后，对连接错误与命中 `retry_on` 的状态码按指数退避重试。
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

        if let Some(retry) = &self.retry {
            retry.validate()?;
        }

        let retry = self.retry.clone();

        // 解析目标 host 用于熔断分键；无熔断器或解析失败时为 None（按无熔断处理）。
        let host = self
            .circuit_breaker
            .as_ref()
            .and_then(|_| reqwest::Url::parse(&self.url).ok())
            .and_then(|u| u.host_str().map(|h| h.to_string()));

        // 无重试策略：单次发送（与原行为一致，仅多一层熔断准入）
        let Some(retry) = retry else {
            return self.send_guarded(host.as_deref()).await;
        };

        // 有重试策略：最多发送 1 + max_retries 次
        let mut attempt: u32 = 0;
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

            // 指数退避：backoff_ms * 2^attempt
            let backoff = retry.backoff_ms.saturating_mul(1u64 << attempt.min(20));
            if backoff > 0 {
                tokio::time::sleep(Duration::from_millis(backoff)).await;
            }
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
        // URL 仅记 self.url（不含 query 参数，query 经 reqwest 单独拼接，避免泄漏敏感参数）。
        let start = std::time::Instant::now();
        let result = request.send().await;
        let elapsed_ms = start.elapsed().as_millis();
        match &result {
            Ok(resp) => {
                tracing::debug!(
                    target: "yang_base::http",
                    method = %self.method,
                    url = %self.url,
                    status = resp.status().as_u16(),
                    elapsed_ms = elapsed_ms as u64,
                    "HTTP 出站请求完成"
                );
            }
            Err(e) => {
                tracing::warn!(
                    target: "yang_base::http",
                    method = %self.method,
                    url = %self.url,
                    elapsed_ms = elapsed_ms as u64,
                    error = %e,
                    "HTTP 出站请求失败"
                );
            }
        }
        let response = result.map_err(BaseError::HttpRequestFailed)?;

        Ok(Response::new(response))
    }
}
