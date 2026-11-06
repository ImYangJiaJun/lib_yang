//! yang-feishu —— 飞书开放平台协议客户端。
//!
//! 只包含**平台契约**：出站传输、令牌、多维表格、审批、通讯录、IM、云盘/附件、
//! 事件入站、外部选项协议与加解密原语。审批派发状态机、字段映射、数据源拉取编排、
//! 控制台 Action、产品表与中文运维文案都留在宿主。
//!
//! # 快速开始
//!
//! ```rust,ignore
//! use std::sync::Arc;
//! use yang_feishu::{Endpoints, OpenApiClient, OpenApiConfig};
//! use yang_feishu::auth::MemoryTokenCache;
//! use yang_feishu::transport::TokioSleeper;
//!
//! let client = OpenApiClient::builder(
//!     OpenApiConfig::static_credentials("cli_xxx", "secret")
//!         .endpoints(Endpoints::feishu())
//!         .cache_namespace("demo"),
//! )
//! .transport(Arc::new(yang_feishu::transport::http::ReqwestTransport::new()?))
//! .sleeper(Arc::new(TokioSleeper))
//! .token_cache(Arc::new(MemoryTokenCache::default()))
//! .build()?;
//!
//! let snapshot = client
//!     .bitable()
//!     .list_all_records("bascn...", "tbl...", None, None)
//!     .await?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Feature Gates
//!
//! - `http`：启用 [`transport::http::ReqwestTransport`]（独立使用者）。
//! - `redis`：启用 [`auth::redis::RedisTokenCache`]（独立使用者）；宿主已有
//!   Redis 时实现 [`auth::TokenCache`] 即可。
//! - `metrics`：启用 `feishu_outbound_request_total` / `feishu_outbound_token_total`。
//! - `full`：以上三者。
//!
//! # 全量官方端点
//!
//! [`OpenApiClient::api`] 返回 [`api::ApiClient`]，覆盖官方 SDK、llms 文档与历史
//! 版本中的 1713 个 REST 端点，按 66 个域生成 2372 个薄封装方法；typed 核心链路
//! 仍走 `OpenApiClient::bitable` / `OpenApiClient::approval` 等客户端。
//!
//! 每个生成域由 `api-<domain>` feature 门控；typed 模块由 `bitable` /
//! `approval` / `contact` / `im` / `drive` 门控。默认不启用任何域，按需开启。
//!
//! # 平台事实（最容易踩的坑）
//!
//! - 成败判据是响应体业务码，**不是 HTTP 状态码**；大量错误是 HTTP 200 + 非 0 码。
//! - `TokenExpired` 必须先于 401/403 判定；`99991663` 常以 HTTP 400 返回。
//! - 频控等待在 `x-ogw-ratelimit-reset`，没有 `Retry-After`。
//! - `has_more=true` 但缺 `page_token` 必须硬失败；`code=0` 但缺 `data` 必须报错。
//! - 60012 = uuid 已创建（按 uuid 反查），1390003 = 实例不存在（不是终态），
//!   1390007 = 已订阅（幂等成功）。
//! - 金额控件 `currency` 只能从审批定义 `option.currencyRange` 取。
//! - 外部选项 `linkage_params` 的真机值是文案，不是 `@i18n@<option_id>`。
//!
//! # 已知缺口
//!
//! - `X-Lark-Signature` 未实现（需要原始请求体；`EventConfig::require_signature`
//!   置 `true` 会显式失败）。
//! - Lark 国际版 `upload` 域未用真实租户验证。
//! - 关联审批的 AppLink 解析需要调用方提供 `FormInput::approval_app_id`；传 `None`
//!   时只接受 36 位实例 Code。附件搬运编排、`bitablePerm` extra 与 200ms pacing
//!   属于宿主产品语义，不在本 crate。
//!
//! # 0.x 兼容性
//!
//! 公开 DTO 与枚举暂不标 `#[non_exhaustive]`：宿主既有测试与结构体字面量要能
//! drop-in 编译。破坏性变更走 minor；等 1.0 再统一收紧。

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// 当前 crate 版本。
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 飞书开放平台域名（中国站）。
pub const FEISHU_OPEN_BASE: &str = "https://open.feishu.cn";

pub mod api;
#[cfg(feature = "approval")]
pub mod approval;
pub mod auth;
#[cfg(feature = "bitable")]
pub mod bitable;
pub mod client;
#[cfg(feature = "contact")]
pub mod contact;
#[cfg(feature = "contact")]
mod contact_endpoints;
pub mod crypto;
#[cfg(feature = "drive")]
pub mod drive;
#[cfg(feature = "drive")]
mod drive_endpoints;
pub mod endpoints;
pub mod error;
pub mod event;
#[cfg(feature = "im")]
pub mod im;
#[cfg(feature = "im")]
mod im_endpoints;
pub mod multipart;
pub mod options;
pub mod path;
pub mod transport;

pub use api::ApiClient;
pub use client::{OpenApiClient, OpenApiClientBuilder, OpenApiConfig};
pub use endpoints::{BaseUrl, Endpoints};
pub use error::{Error, FailureKind, Result};
pub use transport::{
    disposition, Disposition, Method, Request, Response, RetryPolicy, Sleeper, TokioSleeper,
    Transport,
};
