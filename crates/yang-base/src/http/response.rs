//! HTTP 响应处理实现
//!
//! 提供便捷的响应处理方法。

use crate::error::BaseError;
use reqwest::header::HeaderMap;
use reqwest::Response as ReqwestResponse;

/// HTTP 响应
///
/// 封装 reqwest::Response，提供便捷的响应处理方法
///
/// # 示例
///
/// ```rust,ignore
/// // tools 为启动期经 ToolsBuilder::http(...) 冻结的应用资源；Action 内则用 ctx.http()?
/// let response = tools.http()?
///     .get("https://api.example.com/users")
///     .send()
///     .await?;
///
/// // 检查状态码
/// if response.is_success() {
///     // 解析 JSON
///     let users: Vec<User> = response.json().await?;
/// }
/// ```
pub struct Response {
    /// reqwest 响应
    response: ReqwestResponse,
}

impl Response {
    /// 创建新的响应对象
    ///
    /// # 参数
    ///
    /// - `response`: reqwest 响应
    pub(crate) fn new(response: ReqwestResponse) -> Self {
        Self { response }
    }

    /// 获取状态码
    ///
    /// # 返回
    ///
    /// - HTTP 状态码
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let status = response.status();
    /// println!("状态码: {}", status);
    /// ```
    pub fn status(&self) -> u16 {
        self.response.status().as_u16()
    }

    /// 检查响应是否成功（2xx）
    ///
    /// # 返回
    ///
    /// - `true`: 状态码为 2xx
    /// - `false`: 状态码不是 2xx
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// if response.is_success() {
    ///     println!("请求成功");
    /// } else {
    ///     println!("请求失败，状态码: {}", response.status());
    /// }
    /// ```
    pub fn is_success(&self) -> bool {
        self.response.status().is_success()
    }

    /// 获取响应头
    ///
    /// # 返回
    ///
    /// - 响应头映射
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let headers = response.headers();
    /// if let Some(content_type) = headers.get("content-type") {
    ///     println!("Content-Type: {:?}", content_type);
    /// }
    /// ```
    pub fn headers(&self) -> &HeaderMap {
        self.response.headers()
    }

    /// 获取响应体为文本
    ///
    /// # 返回
    ///
    /// - `Ok(String)`: 响应文本
    /// - `Err(BaseError)`: 解析失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let text = response.text().await?;
    /// println!("响应内容: {}", text);
    /// ```
    pub async fn text(self) -> Result<String, BaseError> {
        self.response
            .text()
            .await
            .map_err(|e| BaseError::HttpResponseParseFailed(e.to_string()))
    }

    /// 获取响应体为字节流
    ///
    /// # 返回
    ///
    /// - `Ok(Vec<u8>)`: 响应字节
    /// - `Err(BaseError)`: 解析失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let bytes = response.bytes().await?;
    /// println!("响应大小: {} 字节", bytes.len());
    /// ```
    pub async fn bytes(self) -> Result<Vec<u8>, BaseError> {
        self.response
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| BaseError::HttpResponseParseFailed(e.to_string()))
    }

    /// 获取响应体为 JSON
    ///
    /// # 类型参数
    ///
    /// - `T`: 目标类型，必须实现 `serde::de::DeserializeOwned`
    ///
    /// # 返回
    ///
    /// - `Ok(T)`: 反序列化后的对象
    /// - `Err(BaseError)`: 解析失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// #[derive(Deserialize)]
    /// struct User {
    ///     id: u64,
    ///     name: String,
    /// }
    ///
    /// let user: User = response.json().await?;
    /// println!("用户: {} (ID: {})", user.name, user.id);
    /// ```
    pub async fn json<T: serde::de::DeserializeOwned>(self) -> Result<T, BaseError> {
        self.response
            .json()
            .await
            .map_err(|e| BaseError::JsonDeserializeFailed(e.to_string()))
    }
}
