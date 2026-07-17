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
//! use yang_base::tools::ToolsBuilder;
//!
//! // 启动期：把客户端注册进应用资源并冻结
//! let tools = ToolsBuilder::new().http(HttpClient::new(30)?).build()?;
//!
//! // 运行期：从 Tools（或 Action 内 ctx.http()?）获取客户端发起 GET 请求
//! let response = tools.http()?
//!     .get("https://api.example.com/users")
//!     .bearer_token("your_token")
//!     .send()
//!     .await?;
//!
//! // 解析 JSON 响应
//! let users: Vec<User> = response.json().await?;
//! ```

mod circuit_breaker;
mod client;
mod request;
mod response;

pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
pub use client::{HttpClient, HttpClientConfig};
pub use request::{RequestBuilder, RetryConfig};
pub use response::Response;

#[cfg(test)]
mod __tests__;
