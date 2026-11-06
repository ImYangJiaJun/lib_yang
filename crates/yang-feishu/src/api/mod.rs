//! 官方 OpenAPI 全量端点薄封装。
//!
//! 由 `scripts/generate_openapi_endpoints.py` 从官方 SDK/llms 端点目录生成。
//! 每个子模块对应一个官方域；方法名带域前缀，统一挂在 [`ApiClient`] 上。
//! typed 核心链路仍使用 `bitable()` / `approval()` / `im()` 等客户端。

use std::sync::Arc;

use crate::client::ClientCore;

/// 通用 OpenAPI 客户端。
#[derive(Clone)]
pub struct ApiClient {
    // 只在启用生成域时被方法读取；默认空 feature 下保留装配面。
    #[allow(dead_code)]
    pub(crate) core: Arc<ClientCore>,
}

impl ApiClient {
    pub(crate) fn new(core: Arc<ClientCore>) -> Self {
        Self { core }
    }
}

/// 二进制响应上限（64 MiB）。
#[allow(dead_code)]
pub(crate) const MAX_BINARY_BYTES: usize = 64 * 1024 * 1024;

#[allow(dead_code)]
pub(crate) fn query_vec(query: &[(&str, &str)]) -> Vec<(String, String)> {
    query
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect()
}

#[allow(dead_code)]
pub(crate) fn join_url(base: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        path.to_string()
    } else {
        format!("{base}{path}")
    }
}

/// `acs` 端点。
#[cfg(feature = "api-acs")]
pub mod acs;

/// `admin` 端点。
#[cfg(feature = "api-admin")]
pub mod admin;

/// `aily` 端点。
#[cfg(feature = "api-aily")]
pub mod aily;

/// `apaas` 端点。
#[cfg(feature = "api-apaas")]
pub mod apaas;

/// `application` 端点。
#[cfg(feature = "api-application")]
pub mod application;

/// `approval` 端点。
#[cfg(feature = "api-approval")]
pub mod approval;

/// `attendance` 端点。
#[cfg(feature = "api-attendance")]
pub mod attendance;

/// `auth` 端点。
#[cfg(feature = "api-auth")]
pub mod auth;

/// `authen` 端点。
#[cfg(feature = "api-authen")]
pub mod authen;

/// `baike` 端点。
#[cfg(feature = "api-baike")]
pub mod baike;

/// `base` 端点。
#[cfg(feature = "api-base")]
pub mod base;

/// `bitable` 端点。
#[cfg(feature = "api-bitable")]
pub mod bitable;

/// `block` 端点。
#[cfg(feature = "api-block")]
pub mod block;

/// `board` 端点。
#[cfg(feature = "api-board")]
pub mod board;

/// `bot` 端点。
#[cfg(feature = "api-bot")]
pub mod bot;

/// `calendar` 端点。
#[cfg(feature = "api-calendar")]
pub mod calendar;

/// `cardkit` 端点。
#[cfg(feature = "api-cardkit")]
pub mod cardkit;

/// `compensation` 端点。
#[cfg(feature = "api-compensation")]
pub mod compensation;

/// `contact` 端点。
#[cfg(feature = "api-contact")]
pub mod contact;

/// `corehr` 端点。
#[cfg(feature = "api-corehr")]
pub mod corehr;

/// `directory` 端点。
#[cfg(feature = "api-directory")]
pub mod directory;

/// `docs` 端点。
#[cfg(feature = "api-docs")]
pub mod docs;

/// `document_ai` 端点。
#[cfg(feature = "api-document_ai")]
pub mod document_ai;

/// `docx` 端点。
#[cfg(feature = "api-docx")]
pub mod docx;

/// `drive` 端点。
#[cfg(feature = "api-drive")]
pub mod drive;

/// `ehr` 端点。
#[cfg(feature = "api-ehr")]
pub mod ehr;

/// `ephemeral` 端点。
#[cfg(feature = "api-ephemeral")]
pub mod ephemeral;

/// `event` 端点。
#[cfg(feature = "api-event")]
pub mod event;

/// `face_verify` 端点。
#[cfg(feature = "api-face_verify")]
pub mod face_verify;

/// `helpdesk` 端点。
#[cfg(feature = "api-helpdesk")]
pub mod helpdesk;

/// `hire` 端点。
#[cfg(feature = "api-hire")]
pub mod hire;

/// `human_authentication` 端点。
#[cfg(feature = "api-human_authentication")]
pub mod human_authentication;

/// `im` 端点。
#[cfg(feature = "api-im")]
pub mod im;

/// `jssdk` 端点。
#[cfg(feature = "api-jssdk")]
pub mod jssdk;

/// `lingo` 端点。
#[cfg(feature = "api-lingo")]
pub mod lingo;

/// `mail` 端点。
#[cfg(feature = "api-mail")]
pub mod mail;

/// `mdm` 端点。
#[cfg(feature = "api-mdm")]
pub mod mdm;

/// `meeting_room` 端点。
#[cfg(feature = "api-meeting_room")]
pub mod meeting_room;

/// `message` 端点。
#[cfg(feature = "api-message")]
pub mod message;

/// `minutes` 端点。
#[cfg(feature = "api-minutes")]
pub mod minutes;

/// `moments` 端点。
#[cfg(feature = "api-moments")]
pub mod moments;

/// `okr` 端点。
#[cfg(feature = "api-okr")]
pub mod okr;

/// `optical_char_recognition` 端点。
#[cfg(feature = "api-optical_char_recognition")]
pub mod optical_char_recognition;

/// `passport` 端点。
#[cfg(feature = "api-passport")]
pub mod passport;

/// `pay` 端点。
#[cfg(feature = "api-pay")]
pub mod pay;

/// `payroll` 端点。
#[cfg(feature = "api-payroll")]
pub mod payroll;

/// `performance` 端点。
#[cfg(feature = "api-performance")]
pub mod performance;

/// `personal_settings` 端点。
#[cfg(feature = "api-personal_settings")]
pub mod personal_settings;

/// `report` 端点。
#[cfg(feature = "api-report")]
pub mod report;

/// `search` 端点。
#[cfg(feature = "api-search")]
pub mod search;

/// `security_and_compliance` 端点。
#[cfg(feature = "api-security_and_compliance")]
pub mod security_and_compliance;

/// `sheets` 端点。
#[cfg(feature = "api-sheets")]
pub mod sheets;

/// `slides_ai` 端点。
#[cfg(feature = "api-slides_ai")]
pub mod slides_ai;

/// `spark` 端点。
#[cfg(feature = "api-spark")]
pub mod spark;

/// `speech_to_text` 端点。
#[cfg(feature = "api-speech_to_text")]
pub mod speech_to_text;

/// `suite` 端点。
#[cfg(feature = "api-suite")]
pub mod suite;

/// `task` 端点。
#[cfg(feature = "api-task")]
pub mod task;

/// `tenant` 端点。
#[cfg(feature = "api-tenant")]
pub mod tenant;

/// `translation` 端点。
#[cfg(feature = "api-translation")]
pub mod translation;

/// `trust_party` 端点。
#[cfg(feature = "api-trust_party")]
pub mod trust_party;

/// `unified_kms` 端点。
#[cfg(feature = "api-unified_kms")]
pub mod unified_kms;

/// `user` 端点。
#[cfg(feature = "api-user")]
pub mod user;

/// `vc` 端点。
#[cfg(feature = "api-vc")]
pub mod vc;

/// `verification` 端点。
#[cfg(feature = "api-verification")]
pub mod verification;

/// `wiki` 端点。
#[cfg(feature = "api-wiki")]
pub mod wiki;

/// `workplace` 端点。
#[cfg(feature = "api-workplace")]
pub mod workplace;
