//! HTTP 客户端核心实现
//!
//! 提供全局 HTTP 客户端和请求构建能力。

use crate::error::BaseError;
use crate::http::request::RequestBuilder;
use reqwest::{Client, Method};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

/// 全局 HTTP 客户端实例
static GLOBAL_HTTP_CLIENT: OnceLock<HttpClient> = OnceLock::new();

/// HTTP 客户端
///
/// 提供 HTTP 请求构建和发送能力
///
/// # 示例
///
/// ```rust,ignore
/// use yang_base::http::HttpClient;
///
/// // 初始化全局客户端
/// HttpClient::init_global(30)?;
///
/// // 使用全局客户端发起请求
/// let response = HttpClient::global()?
///     .get("https://api.example.com/users")
///     .send()
///     .await?;
/// ```
#[derive(Clone, Debug)]
pub struct HttpClient {
    /// reqwest 客户端
    client: Client,

    /// 默认超时时间
    default_timeout: Duration,

    /// 默认 Token（可选）
    default_token: Arc<RwLock<Option<String>>>,
}

impl HttpClient {
    /// 创建新的 HTTP 客户端
    ///
    /// # 参数
    ///
    /// - `timeout_secs`: 默认超时时间（秒）
    ///
    /// # 返回
    ///
    /// - `Ok(HttpClient)`: 客户端实例
    /// - `Err(BaseError)`: 创建失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let client = HttpClient::new(30)?;
    /// ```
    pub fn new(timeout_secs: u64) -> Result<Self, BaseError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .map_err(|e| BaseError::HttpClientCreateFailed(e.to_string()))?;

        Ok(Self {
            client,
            default_timeout: Duration::from_secs(timeout_secs),
            default_token: Arc::new(RwLock::new(None)),
        })
    }

    /// 初始化全局 HTTP 客户端
    ///
    /// # 参数
    ///
    /// - `timeout_secs`: 默认超时时间（秒）
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 初始化成功
    /// - `Err(BaseError)`: 初始化失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// HttpClient::init_global(30)?;
    /// ```
    pub fn init_global(timeout_secs: u64) -> Result<(), BaseError> {
        let client = Self::new(timeout_secs)?;

        GLOBAL_HTTP_CLIENT
            .set(client)
            .map_err(|_| BaseError::HttpClientCreateFailed("全局客户端已初始化".to_string()))?;

        log::info!("全局 HTTP 客户端已初始化，超时时间: {} 秒", timeout_secs);
        Ok(())
    }

    /// 获取全局 HTTP 客户端
    ///
    /// # 返回
    ///
    /// - `Ok(&HttpClient)`: 客户端实例
    /// - `Err(BaseError)`: 客户端未初始化
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let client = HttpClient::global()?;
    /// ```
    pub fn global() -> Result<&'static HttpClient, BaseError> {
        GLOBAL_HTTP_CLIENT
            .get()
            .ok_or(BaseError::HttpClientCreateFailed(
                "全局客户端未初始化".to_string(),
            ))
    }

    /// 设置默认 Token
    ///
    /// # 参数
    ///
    /// - `token`: Token 字符串
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let mut client = HttpClient::new(30)?;
    /// client.set_default_token("your_token".to_string());
    /// ```
    pub fn set_default_token(&self, token: String) {
        let mut default_token = self.default_token.write().unwrap();
        *default_token = Some(token);
        log::debug!("已设置默认 Token");
    }

    /// 获取默认 Token
    fn get_default_token(&self) -> Option<String> {
        let default_token = self.default_token.read().unwrap();
        default_token.clone()
    }

    /// 创建请求构建器
    ///
    /// # 参数
    ///
    /// - `method`: HTTP 方法
    /// - `url`: 请求 URL
    ///
    /// # 返回
    ///
    /// - `RequestBuilder`: 请求构建器
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use reqwest::Method;
    ///
    /// let response = client
    ///     .request(Method::GET, "https://api.example.com/users")
    ///     .send()
    ///     .await?;
    /// ```
    pub fn request(&self, method: Method, url: &str) -> RequestBuilder {
        RequestBuilder::new(
            self.client.clone(),
            method,
            url.to_string(),
            self.default_timeout,
            self.get_default_token(),
        )
    }

    /// GET 请求
    ///
    /// # 参数
    ///
    /// - `url`: 请求 URL
    ///
    /// # 返回
    ///
    /// - `RequestBuilder`: 请求构建器
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let response = client
    ///     .get("https://api.example.com/users")
    ///     .send()
    ///     .await?;
    /// ```
    pub fn get(&self, url: &str) -> RequestBuilder {
        self.request(Method::GET, url)
    }

    /// POST 请求
    ///
    /// # 参数
    ///
    /// - `url`: 请求 URL
    ///
    /// # 返回
    ///
    /// - `RequestBuilder`: 请求构建器
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let response = client
    ///     .post("https://api.example.com/users")
    ///     .json(&user)?
    ///     .send()
    ///     .await?;
    /// ```
    pub fn post(&self, url: &str) -> RequestBuilder {
        self.request(Method::POST, url)
    }

    /// PUT 请求
    ///
    /// # 参数
    ///
    /// - `url`: 请求 URL
    ///
    /// # 返回
    ///
    /// - `RequestBuilder`: 请求构建器
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let response = client
    ///     .put("https://api.example.com/users/1")
    ///     .json(&user)?
    ///     .send()
    ///     .await?;
    /// ```
    pub fn put(&self, url: &str) -> RequestBuilder {
        self.request(Method::PUT, url)
    }

    /// DELETE 请求
    ///
    /// # 参数
    ///
    /// - `url`: 请求 URL
    ///
    /// # 返回
    ///
    /// - `RequestBuilder`: 请求构建器
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let response = client
    ///     .delete("https://api.example.com/users/1")
    ///     .send()
    ///     .await?;
    /// ```
    pub fn delete(&self, url: &str) -> RequestBuilder {
        self.request(Method::DELETE, url)
    }

    /// PATCH 请求
    ///
    /// # 参数
    ///
    /// - `url`: 请求 URL
    ///
    /// # 返回
    ///
    /// - `RequestBuilder`: 请求构建器
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let response = client
    ///     .patch("https://api.example.com/users/1")
    ///     .json(&updates)?
    ///     .send()
    ///     .await?;
    /// ```
    pub fn patch(&self, url: &str) -> RequestBuilder {
        self.request(Method::PATCH, url)
    }
}
