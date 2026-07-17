//! 传输适配器：把冻结的 [`BuiltApp`](crate::definition::BuiltApp) 暴露到具体网络协议。
//!
//! 框架核心（Catalog / Registry / Action）不知道任何传输协议的存在；每个协议
//! 一个薄适配器，负责协议解析、横切中间件与响应映射。当前提供：
//!
//! - [`axum`]：Axum 0.8 HTTP 适配器（`transport-axum` feature）

/// Axum 0.8 HTTP 传输适配器。
#[cfg(feature = "transport-axum")]
pub mod axum;
