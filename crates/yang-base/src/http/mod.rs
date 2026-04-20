//! HTTP 客户端模块
//!
//! 提供灵活的 HTTP 请求构建和响应处理能力。
//!
//! # 主要组件
//!
//! - `HttpClient`：HTTP 客户端核心
//! - `RequestBuilder`：请求构建器
//! - `Response`：响应处理
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::http::HttpClient;
//!
//! // 初始化全局 HTTP 客户端
//! HttpClient::init_global(30)?;
//!
//! // 发起 GET 请求
//! let response = HttpClient::global()?
//!     .get("https://api.example.com/users")
//!     .bearer_token("your_token")
//!     .send()
//!     .await?;
//!
//! // 解析 JSON 响应
//! let users: Vec<User> = response.json().await?;
//! ```

mod client;
mod request;
mod response;

pub use client::HttpClient;
pub use request::RequestBuilder;
pub use response::Response;

#[cfg(test)]
mod __tests__;
