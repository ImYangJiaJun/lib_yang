//! HTTP 请求构建器实现
//!
//! 提供链式调用接口构建 HTTP 请求。

use crate::error::BaseError;
use crate::http::response::Response;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Client, Method};
use serde::Serialize;
use std::time::Duration;

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
    pub(crate) fn new(
        client: Client,
        method: Method,
        url: String,
        timeout: Duration,
        token: Option<String>,
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
                self.header_errors
                    .push(format!("表单数据编码失败: {}", e));
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

        // 构建请求
        // 注意：self.client.clone() 是 Arc::clone，复用同一底层连接池，不创建新的 TCP 连接池
        let mut request = self
            .client
            .request(self.method, &self.url)
            .timeout(self.timeout);

        // 添加请求头
        request = request.headers(self.headers);

        // 添加 Token
        if let Some(token) = self.token {
            request = request.bearer_auth(token);
        }

        // 添加查询参数
        if !self.query_params.is_empty() {
            request = request.query(&self.query_params);
        }

        // 添加请求体
        if let Some(body) = self.body {
            request = request.body(body);
        }

        // 发送请求
        let response = request
            .send()
            .await
            .map_err(BaseError::HttpRequestFailed)?;

        Ok(Response::new(response))
    }
}
