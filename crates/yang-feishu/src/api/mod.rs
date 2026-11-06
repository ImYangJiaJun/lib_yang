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
    pub(crate) core: Arc<ClientCore>,
}

impl ApiClient {
    pub(crate) fn new(core: Arc<ClientCore>) -> Self {
        Self { core }
    }
}

/// 二进制响应上限（64 MiB）。
pub(crate) const MAX_BINARY_BYTES: usize = 64 * 1024 * 1024;

pub(crate) fn query_vec(query: &[(&str, &str)]) -> Vec<(String, String)> {
    query
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect()
}

pub(crate) fn join_url(base: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        path.to_string()
    } else {
        format!("{base}{path}")
    }
}

/// `acs` 端点。
pub mod acs;

/// `admin` 端点。
pub mod admin;

/// `aily` 端点。
pub mod aily;

/// `apaas` 端点。
pub mod apaas;

/// `application` 端点。
pub mod application;

/// `approval` 端点。
pub mod approval;

/// `attendance` 端点。
pub mod attendance;

/// `auth` 端点。
pub mod auth;

/// `authen` 端点。
pub mod authen;

/// `baike` 端点。
pub mod baike;

/// `base` 端点。
pub mod base;

/// `bitable` 端点。
pub mod bitable;

/// `block` 端点。
pub mod block;

/// `board` 端点。
pub mod board;

/// `bot` 端点。
pub mod bot;

/// `calendar` 端点。
pub mod calendar;

/// `cardkit` 端点。
pub mod cardkit;

/// `compensation` 端点。
pub mod compensation;

/// `contact` 端点。
pub mod contact;

/// `corehr` 端点。
pub mod corehr;

/// `directory` 端点。
pub mod directory;

/// `docs` 端点。
pub mod docs;

/// `document_ai` 端点。
pub mod document_ai;

/// `docx` 端点。
pub mod docx;

/// `drive` 端点。
pub mod drive;

/// `ehr` 端点。
pub mod ehr;

/// `ephemeral` 端点。
pub mod ephemeral;

/// `event` 端点。
pub mod event;

/// `face_verify` 端点。
pub mod face_verify;

/// `helpdesk` 端点。
pub mod helpdesk;

/// `hire` 端点。
pub mod hire;

/// `human_authentication` 端点。
pub mod human_authentication;

/// `im` 端点。
pub mod im;

/// `jssdk` 端点。
pub mod jssdk;

/// `lingo` 端点。
pub mod lingo;

/// `mail` 端点。
pub mod mail;

/// `mdm` 端点。
pub mod mdm;

/// `meeting_room` 端点。
pub mod meeting_room;

/// `message` 端点。
pub mod message;

/// `minutes` 端点。
pub mod minutes;

/// `moments` 端点。
pub mod moments;

/// `okr` 端点。
pub mod okr;

/// `optical_char_recognition` 端点。
pub mod optical_char_recognition;

/// `passport` 端点。
pub mod passport;

/// `pay` 端点。
pub mod pay;

/// `payroll` 端点。
pub mod payroll;

/// `performance` 端点。
pub mod performance;

/// `personal_settings` 端点。
pub mod personal_settings;

/// `report` 端点。
pub mod report;

/// `search` 端点。
pub mod search;

/// `security_and_compliance` 端点。
pub mod security_and_compliance;

/// `sheets` 端点。
pub mod sheets;

/// `slides_ai` 端点。
pub mod slides_ai;

/// `spark` 端点。
pub mod spark;

/// `speech_to_text` 端点。
pub mod speech_to_text;

/// `suite` 端点。
pub mod suite;

/// `task` 端点。
pub mod task;

/// `tenant` 端点。
pub mod tenant;

/// `translation` 端点。
pub mod translation;

/// `trust_party` 端点。
pub mod trust_party;

/// `unified_kms` 端点。
pub mod unified_kms;

/// `user` 端点。
pub mod user;

/// `vc` 端点。
pub mod vc;

/// `verification` 端点。
pub mod verification;

/// `wiki` 端点。
pub mod wiki;

/// `workplace` 端点。
pub mod workplace;
