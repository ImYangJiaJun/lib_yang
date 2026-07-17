//! 传输无关的 Action 中间件。
//!
//! 路由定义、Catalog 与运行时分派均由 [`crate::definition`] 的唯一
//! `AppBuilder -> BuiltApp` 链路负责；本模块只保留可复用的洋葱中间件抽象。

pub mod middleware;
pub use middleware::{Middleware, MiddlewareScope, Next, RequestIdMiddleware};

#[cfg(test)]
mod __tests__;
