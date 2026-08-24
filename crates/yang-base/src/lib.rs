//! yang-base - YANG 基础库
//!
//! 提供插件管理、数据库访问、HTTP 客户端和 JWT Token 管理等核心功能。
//!
//! # 模块
//!
//! - `plugin`：插件管理模块
//! - `database`：数据库管理模块（支持 MySQL 和 Redis）
//! - `http`：HTTP 客户端模块（需启用 `http` feature）
//! - `token`：JWT Token 管理模块（需启用 `token` feature）
//! - `error`：错误处理模块
//! - `table`：表配置系统模块
//! - `action`：Action 系统模块
//!
//! # Feature Gates
//!
//! - `token`：启用 JWT Token 管理功能（依赖 `jsonwebtoken`）
//! - `http`：启用 HTTP 客户端功能（依赖 `reqwest`、`serde_urlencoded`）
//! - `mysql`：启用 MySQL 数据库查询执行功能（依赖 `sqlx`）
//! - `redis`：启用应用实例持有的 Redis 客户端（`token` 会自动启用它）
//! - `validator`：启用正则表达式校验器（依赖 `regex`）
//! - `plugin-schema`：启用插件 JSON Schema 配置验证（依赖 `jsonschema`）
//! - `admin-metadata`：启用独立后台展示描述（不新增依赖、不改变 dispatch）
//! - `transport-axum`：启用 Axum 0.8 HTTP 传输适配器（依赖 `axum`、`tower-http`）
//!
//! # 快速开始
//!
//! ```rust,ignore
//! use yang_base::definition::{AppBuilder, AddonSpec};
//! use yang_base::tools::ToolsBuilder;
//! use yang_db::{Database, DatabaseConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let database = Database::connect(
//!         "mysql://user:pass@localhost/db",
//!         DatabaseConfig::default(),
//!     ).await?;
//!     let tools = ToolsBuilder::new().mysql(database).build()?;
//!     let app = AppBuilder::new()
//!         .addon(AddonSpec::new(yang_base::addon!("account")))
//!         .build(tools)?;
//!     app.tools().mysql().health_check().await?;
//!     Ok(())
//! }
//! ```

// 强制公开 API 文档覆盖率检查：所有公开项必须有文档注释
#![warn(missing_docs)]

/// 当前 yang-base crate 版本。
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

// 让派生宏内部的 `::yang_base::...` 路径在 yang-base 自身的测试中正常解析。
extern crate self as yang_base;

pub mod action;
#[cfg(feature = "admin-metadata")]
pub mod admin;
pub mod config;
pub mod database;
pub mod definition;
pub mod error;
#[cfg(feature = "http")]
pub mod http;
pub mod lifecycle;
pub mod observability;
pub mod plugin;
pub mod router;
pub mod table;
#[cfg(feature = "token")]
pub mod token;
pub mod tools;
pub mod transport;

// 重新导出插件系统的核心类型，方便用户直接使用
pub use plugin::{Plugin, PluginManager, PluginManagerBuilder, PluginRegistry};

// 重新导出统一错误类型与 Result 别名，下游可直接写 yang_base::Result<T>
pub use error::{BaseError, ErrorCategory, Result};

// 重新导出派生宏
pub use yang_base_derive::{params, Action};

/// 直接生成原生 [`definition::Fields`]；重复字段由临时结构体在编译期拒绝。
#[macro_export]
macro_rules! fields {
    ($($name:ident => $builder:expr),* $(,)?) => {{
        const _: () = {
            $($crate::definition::__validate_field_literal(stringify!($name));)*
        };
        #[allow(dead_code)]
        struct __YangFieldNames {
            $($name: (),)*
        }
        let _ = __YangFieldNames { $($name: (),)* };
        $crate::definition::Fields::new()
            $(.field(
                $crate::definition::FieldName::__from_validated_literal(stringify!($name)),
                $builder,
            ))*
    }};
}

/// 编译期校验的 AddonName 字面量。
#[macro_export]
macro_rules! addon {
    ($value:literal) => {{
        const _: () = $crate::definition::__validate_segment_literal($value);
        $crate::definition::AddonName::__from_validated_literal($value)
    }};
}

/// 编译期校验的 ModuleName 字面量。
#[macro_export]
macro_rules! module {
    ($value:literal) => {{
        const _: () = $crate::definition::__validate_ref_literal($value);
        $crate::definition::ModuleName::__from_validated_literal($value)
    }};
}

/// 编译期校验的 TableName 字面量。
#[macro_export]
macro_rules! table {
    ($value:literal) => {{
        const _: () = $crate::definition::__validate_segment_literal($value);
        $crate::definition::TableName::__from_validated_literal($value)
    }};
}

/// 编译期校验的限定 FieldRef 字面量，例如 `field!("users.id")`。
#[macro_export]
macro_rules! field {
    ($value:literal) => {{
        const _: () = $crate::definition::__validate_ref_literal($value);
        $crate::definition::FieldRef::__from_validated_literal($value)
    }};
}

/// 编译期校验的限定 ActionRef 字面量，例如 `action!("account.user.login")`。
#[macro_export]
macro_rules! action {
    ($value:literal) => {{
        const _: () = $crate::definition::__validate_ref_literal($value);
        $crate::definition::ActionRef::__from_validated_literal($value)
    }};
}

/// 编译期校验的 ActionName 字面量，例如 `action_name!("login")`。
#[macro_export]
macro_rules! action_name {
    ($value:literal) => {{
        const _: () = $crate::definition::__validate_segment_literal($value);
        $crate::definition::ActionName::__from_validated_literal($value)
    }};
}

/// 编译期校验的限定 ViewRef 字面量。
#[macro_export]
macro_rules! view {
    ($value:literal) => {{
        const _: () = $crate::definition::__validate_ref_literal($value);
        $crate::definition::ViewRef::__from_validated_literal($value)
    }};
}

/// 聚合原生 Modules；每个元素直接调用 Module::into_spec。
#[macro_export]
macro_rules! modules {
    ($($module:expr),* $(,)?) => {{
        $crate::definition::Modules::new()
            $(.module($module))*
    }};
}

/// 原子聚合 ActionSpec 与 Handler，不支持字符串 match。
#[macro_export]
macro_rules! actions {
    ($($spec:expr => $handler:expr),* $(,)?) => {{
        $crate::definition::Actions::new()
            $(.action($spec, $handler))*
    }};
    ($($handler:expr),* $(,)?) => {{
        $crate::definition::Actions::new()
            $(.native($handler))*
    }};
}
