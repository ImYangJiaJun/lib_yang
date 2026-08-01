//! YANG 服务进程通用运行时。
//!
//! 本 crate 只承载与具体业务无关的启动期配置、生命周期和可观测性机制。

pub mod config;
pub mod observability;
pub mod shutdown;
