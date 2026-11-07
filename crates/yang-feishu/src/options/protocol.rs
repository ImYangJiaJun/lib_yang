//! 飞书审批「关联外部选项」接口的请求与响应契约。
//!
//! 顶层是 `{code, msg, data}`，`data.result` 在未配置 Key 时是对象、配置 Key 后是
//! base64 字符串。键名逐字对齐官方文档。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// 外部选项接口的请求体。
///
/// **刻意不设 `deny_unknown_fields`**：飞书新增字段不应打挂调用方。
#[derive(Debug, Clone, Deserialize)]
pub struct FeishuOptionsRequest {
    /// 内部 ID（v1 不消费）。
    #[serde(default)]
    pub user_id: Option<String>,
    /// 用户 user_id（v1 不消费）。
    #[serde(default)]
    pub employee_id: Option<String>,
    /// 校验请求来源的自定义取值。
    pub token: String,
    /// 联动选项参数。
    #[serde(default)]
    pub linkage_params: Option<BTreeMap<String, String>>,
    /// 分页标记。
    #[serde(default)]
    pub page_token: Option<String>,
    /// 搜索关键词。
    #[serde(default)]
    pub query: Option<String>,
    /// 语言环境（v1 不消费）。
    #[serde(default)]
    pub locale: Option<String>,
}

/// 未配置 Key 时 `data.result` 的内容。
#[derive(Debug, Clone, Serialize)]
pub struct FeishuResultBody {
    /// 选项列表。
    pub options: Vec<FeishuOption>,
    /// 国际化文案；**必须至少有一项**。
    #[serde(rename = "i18nResources")]
    pub i18n_resources: Vec<FeishuI18nResource>,
    /// 是否有下一页。
    #[serde(rename = "hasMore")]
    pub has_more: bool,
    /// 下一页游标。
    #[serde(rename = "nextPageToken", skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

impl FeishuResultBody {
    /// 空结果集：仍带默认语言（飞书要求 `i18nResources` 至少一项）。
    pub fn empty(default_locale: &str) -> Self {
        super::i18n::build_result_body(&[], default_locale, None)
    }
}

/// 单个选项。
#[derive(Debug, Clone, Serialize)]
pub struct FeishuOption {
    /// 选项唯一标识。
    pub id: String,
    /// `i18nResources.texts` 的匹配键。
    pub value: String,
    /// 是否默认选项。
    #[serde(rename = "isDefault", skip_serializing_if = "Option::is_none")]
    pub is_default: Option<bool>,
}

/// 国际化文案。
#[derive(Debug, Clone, Serialize)]
pub struct FeishuI18nResource {
    /// `zh_cn` / `en_us` / `ja_jp`。
    pub locale: String,
    /// 是否默认语言。
    #[serde(rename = "isDefault")]
    pub is_default: bool,
    /// 键为 `@i18n@<option_id>`。
    pub texts: BTreeMap<String, String>,
}

/// `data.result`：未配置 Key 时是对象，配置 Key 后是 base64 字符串。
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum FeishuResult {
    /// 明文结果。
    Plain(FeishuResultBody),
    /// 加密结果。
    Encrypted(String),
}

/// `data` 包装。
#[derive(Debug, Clone, Serialize)]
pub struct FeishuData {
    /// 请求结果内容。
    pub result: FeishuResult,
}

/// 外部选项响应包络 `{code, msg, data}`。
#[derive(Debug, Clone, Serialize)]
pub struct FeishuEnvelope {
    /// 错误码，非 0 表示失败。
    pub code: i32,
    /// 返回码描述（注意键是 `msg`，不是 `message`）。
    pub msg: String,
    /// 业务信息；失败时为 `null`。
    pub data: Option<FeishuData>,
}

impl FeishuEnvelope {
    /// 成功（明文）。
    pub fn ok(body: FeishuResultBody) -> Self {
        Self {
            code: 0,
            msg: "success!".to_string(),
            data: Some(FeishuData {
                result: FeishuResult::Plain(body),
            }),
        }
    }

    /// 成功（加密）。
    pub fn encrypted(cipher_base64: String) -> Self {
        Self {
            code: 0,
            msg: "success!".to_string(),
            data: Some(FeishuData {
                result: FeishuResult::Encrypted(cipher_base64),
            }),
        }
    }

    /// 业务失败。
    pub fn fail(code: i32, msg: impl Into<String>) -> Self {
        Self {
            code,
            msg: msg.into(),
            data: None,
        }
    }

    /// 序列化为响应体文本。
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_uses_msg_and_nested_result() {
        let body = FeishuResultBody {
            options: vec![FeishuOption {
                id: "1".into(),
                value: "@i18n@1".into(),
                is_default: Some(true),
            }],
            i18n_resources: vec![FeishuI18nResource {
                locale: "zh_cn".into(),
                is_default: true,
                texts: BTreeMap::from([("@i18n@1".to_string(), "A".to_string())]),
            }],
            has_more: false,
            next_page_token: None,
        };
        let json: serde_json::Value =
            serde_json::from_str(&FeishuEnvelope::ok(body).to_json().unwrap()).unwrap();
        assert_eq!(json["code"], 0);
        assert_eq!(json["msg"], "success!");
        assert!(json["data"]["result"]["options"].is_array());
        assert!(json["data"]["result"].get("nextPageToken").is_none());
        assert!(json.get("message").is_none());
    }

    #[test]
    fn encrypted_result_is_a_string() {
        let json: serde_json::Value = serde_json::from_str(
            &FeishuEnvelope::encrypted("cipher".into())
                .to_json()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(json["data"]["result"], "cipher");
    }

    #[test]
    fn request_tolerates_unknown_fields_and_requires_token() {
        let request: FeishuOptionsRequest =
            serde_json::from_str(r#"{"token":"t","extra":1}"#).unwrap();
        assert_eq!(request.token, "t");
        assert!(serde_json::from_str::<FeishuOptionsRequest>(r#"{"extra":1}"#).is_err());
    }
}
