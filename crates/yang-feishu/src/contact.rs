//! 通讯录：部门名解析与「姓名 → open_id」。

use std::sync::Arc;

use serde::Deserialize;

use crate::client::ClientCore;
use crate::error::{Error, Result};
use crate::path::validate_opaque_path_segment;
use crate::transport::{
    decode_data, Method, Request, APPROVAL_REQUEST_TIMEOUT_SECS, DEPARTMENT_REQUEST_TIMEOUT_SECS,
    DEPARTMENT_RETRY, PULL_RETRY,
};

/// 搜索用户的 URL（只接受 user_access_token）。
pub fn applicant_search_url() -> String {
    format!("{}/open-apis/search/v1/user", crate::FEISHU_OPEN_BASE)
}

/// 部门查询 URL。
pub fn department_url(department_id: &str) -> Result<String> {
    validate_opaque_path_segment("department_id", department_id)
        .map_err(|error| Error::fatal(error.to_string()))?;
    Ok(format!(
        "{}/open-apis/contact/v3/departments/{department_id}",
        crate::FEISHU_OPEN_BASE
    ))
}

/// 部门 ID 形态 → `department_id_type` 查询参数。
pub fn department_id_type(department_id: &str) -> &'static str {
    if department_id.starts_with("od-") {
        "open_department_id"
    } else {
        "department_id"
    }
}

/// 姓名 → open_id 的解析结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicantResolution {
    /// 唯一命中。
    Resolved {
        /// 命中人的 open_id。
        open_id: String,
    },
    /// 无命中。
    NotFound,
    /// 多个同名命中；`candidates` 为「姓名(open_id)」描述串。
    Ambiguous {
        /// 候选描述串。
        candidates: Vec<String>,
    },
}

/// 搜索接口 `data`（列表键两种形态都收）。
#[derive(Debug, Deserialize)]
struct ApplicantSearchData {
    #[serde(default, alias = "users")]
    items: Vec<SearchUser>,
}

/// 搜索命中的单个用户。
#[derive(Debug, Deserialize)]
struct SearchUser {
    #[serde(default)]
    open_id: String,
    #[serde(default, alias = "localized_name")]
    name: String,
}

fn classify_applicants(name: &str, users: &[SearchUser]) -> ApplicantResolution {
    let exact: Vec<&SearchUser> = users
        .iter()
        .filter(|user| user.name.trim() == name)
        .collect();
    match exact.as_slice() {
        [only] => ApplicantResolution::Resolved {
            open_id: only.open_id.clone(),
        },
        [] => match users {
            [only] => ApplicantResolution::Resolved {
                open_id: only.open_id.clone(),
            },
            _ => ApplicantResolution::NotFound,
        },
        many => ApplicantResolution::Ambiguous {
            candidates: many
                .iter()
                .map(|user| format!("{}({})", user.name.trim(), user.open_id))
                .collect(),
        },
    }
}

fn with_base(mut url: String, base: &str) -> String {
    if let Some(rest) = url.strip_prefix(crate::FEISHU_OPEN_BASE) {
        url = format!("{base}{rest}");
    }
    url
}

/// 通讯录客户端。
#[derive(Clone)]
pub struct ContactClient {
    pub(crate) core: Arc<ClientCore>,
}

impl ContactClient {
    pub(crate) fn new(core: Arc<ClientCore>) -> Self {
        Self { core }
    }

    fn open_base(&self) -> String {
        self.core.endpoints().open.as_str().to_string()
    }

    /// 解析部门名称；失败一律返回 `None` 并留 warn（调用方退化写原始 ID）。
    pub async fn resolve_department_name(&self, department_id: &str) -> Option<String> {
        let department_id = department_id.trim();
        if department_id.is_empty() {
            return None;
        }
        let url = match department_url(department_id) {
            Ok(url) => with_base(url, &self.open_base()),
            Err(error) => {
                tracing::warn!(error = %error, department_id, "部门 ID 非法，跳过部门名解析");
                return None;
            }
        };
        let request = Request {
            method: Method::Get,
            url,
            query: vec![(
                "department_id_type".to_string(),
                department_id_type(department_id).to_string(),
            )],
            bearer_token: None,
            json_body: None,
            raw_body: None,
            binary_limit: None,
            timeout_secs: Some(DEPARTMENT_REQUEST_TIMEOUT_SECS),
            idempotent: true,
        };
        match self.core.send_tenant(request, &DEPARTMENT_RETRY).await {
            Ok(response) => {
                let data = match decode_data(&response.body) {
                    Ok(data) => data,
                    Err(error) => {
                        tracing::warn!(error = %error, department_id, "部门响应解析失败，回填退化为写部门 ID");
                        return None;
                    }
                };
                let name = serde_json::from_str::<serde_json::Value>(&data)
                    .ok()
                    .and_then(|value| {
                        value
                            .get("department")
                            .and_then(|department| department.get("name"))
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string)
                    })
                    .map(|name| name.trim().to_string())
                    .filter(|name| !name.is_empty());
                if name.is_none() {
                    tracing::warn!(
                        department_id,
                        "部门接口响应里没有部门名，回填退化为写部门 ID"
                    );
                }
                name
            }
            Err(error) => {
                tracing::warn!(
                    error = %error.message,
                    department_id,
                    department_id_type = department_id_type(department_id),
                    "部门名称解析失败，回填退化为写部门 ID（无 contact scope 或部门不在可见范围时属预期）"
                );
                None
            }
        }
    }

    /// 按姓名解析 open_id；只接受 user_access_token。
    pub async fn resolve_applicant(
        &self,
        bearer_token: &str,
        name: &str,
    ) -> Result<ApplicantResolution> {
        let name = name.trim();
        if name.is_empty() {
            return Ok(ApplicantResolution::NotFound);
        }
        let request = Request {
            method: Method::Get,
            url: with_base(applicant_search_url(), &self.open_base()),
            query: vec![
                ("query".to_string(), name.to_string()),
                ("page_size".to_string(), "20".to_string()),
            ],
            bearer_token: Some(bearer_token.to_string()),
            json_body: None,
            raw_body: None,
            binary_limit: None,
            timeout_secs: Some(APPROVAL_REQUEST_TIMEOUT_SECS),
            idempotent: true,
        };
        let response = self.core.send_raw(request, &PULL_RETRY).await?;
        let payload = decode_data(&response.body)?;
        let data: ApplicantSearchData = serde_json::from_str(&payload)
            .map_err(|error| Error::fatal(format!("搜索用户响应解析失败: {error}")))?;
        Ok(classify_applicants(name, &data.items))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(name: &str, open_id: &str) -> SearchUser {
        SearchUser {
            open_id: open_id.to_string(),
            name: name.to_string(),
        }
    }

    #[test]
    fn classifies_unique_exact_duplicate_and_fuzzy_hits() {
        assert_eq!(
            classify_applicants("张三", &[user("张三", "ou_1")]),
            ApplicantResolution::Resolved {
                open_id: "ou_1".into()
            }
        );
        assert!(matches!(
            classify_applicants("张三", &[user("张三", "ou_1"), user("张三", "ou_2")]),
            ApplicantResolution::Ambiguous { .. }
        ));
        assert_eq!(
            classify_applicants("张三", &[user("张三丰", "ou_1")]),
            ApplicantResolution::Resolved {
                open_id: "ou_1".into()
            }
        );
        assert_eq!(
            classify_applicants("张三", &[user("李四", "ou_1"), user("王五", "ou_2")]),
            ApplicantResolution::NotFound
        );
    }

    #[test]
    fn department_id_type_uses_the_od_prefix() {
        assert_eq!(department_id_type("od-123"), "open_department_id");
        assert_eq!(department_id_type("db59"), "department_id");
    }

    #[test]
    fn department_url_rejects_path_injection() {
        assert!(department_url("od-1").is_ok());
        assert!(department_url("../evil").is_err());
    }
}
