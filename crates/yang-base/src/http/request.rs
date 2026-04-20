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
/// 提供链式调用接口构建 HTTP 请求
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
    /// reqwest 客户端
    client: Client,

    /// HTTP 方法
    method: Method,

    /// 请求 URL
    url: String,

    /// 请求头
    headers: HeaderMap,

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
    /// - `client`: reqwest 客户端
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
            query_params: Vec::new(),
            body: None,
            timeout,
            token,
        }
    }

    /// 设置请求头
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
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(value),
        ) {
            self.headers.insert(name, value);
        }
        self
    }

    /// 批量设置请求头
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
            if let (Ok(name), Ok(value)) = (
                HeaderName::from_bytes(name.as_bytes()),
                HeaderValue::from_str(value),
            ) {
                self.headers.insert(name, value);
            }
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
        let json_str =
            serde_json::to_vec(json).map_err(|e| BaseError::JsonSerializeFailed(e.to_string()))?;

        self.body = Some(json_str);
        self = self.content_type("application/json");

        Ok(self)
    }

    /// 设置表单请求体
    ///
    /// # 参数
    ///
    /// - `form`: 表单数据
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
        let form_str = form
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");

        self.body = Some(form_str.into_bytes());
        self = self.content_type("application/x-www-form-urlencoded");

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
    /// # 返回
    ///
    /// - `Ok(Response)`: 响应对象
    /// - `Err(BaseError)`: 请求失败
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
        // 构建请求
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
            .map_err(|e| BaseError::HttpRequestFailed(e.to_string()))?;

        Ok(Response::new(response))
    }
}
