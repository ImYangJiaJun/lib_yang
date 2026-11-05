//! IM 消息：发送富文本 post 与编辑已发送消息。

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::client::ClientCore;
use crate::error::{Error, FailureKind, Result};
use crate::multipart::Multipart;
use crate::path::validate_opaque_path_segment;
use crate::transport::{
    Method, Request, MEDIA_REQUEST_TIMEOUT_SECS, MESSAGE_REQUEST_TIMEOUT_SECS, MESSAGE_RETRY,
};

/// 消息接收人类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiveIdType {
    /// `open_id`。
    OpenId,
    /// `user_id`。
    UserId,
    /// `union_id`。
    UnionId,
    /// `email`。
    Email,
}

impl ReceiveIdType {
    /// 查询参数值。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenId => "open_id",
            Self::UserId => "user_id",
            Self::UnionId => "union_id",
            Self::Email => "email",
        }
    }
}

/// 消息接收人。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MessageRecipient {
    /// 接收人类型。
    pub receive_id_type: ReceiveIdType,
    /// 接收人 ID 或邮箱。
    pub receive_id: String,
}

impl MessageRecipient {
    /// 校验接收人形态。
    pub fn validate(&self) -> Result<()> {
        let id = self.receive_id.trim();
        let valid_type = match self.receive_id_type {
            ReceiveIdType::OpenId => id.is_ascii() && id.starts_with("ou_") && id.len() > 3,
            ReceiveIdType::UnionId => id.is_ascii() && id.starts_with("on_") && id.len() > 3,
            ReceiveIdType::UserId => id.is_ascii(),
            ReceiveIdType::Email => id.split_once('@').is_some_and(|(local, domain)| {
                !local.is_empty() && domain.contains('.') && !domain.contains('@')
            }),
        };
        if id.is_empty()
            || id.len() > 320
            || id.chars().any(|c| c.is_whitespace() || c.is_control())
            || !valid_type
        {
            return Err(Error::fatal(
                "请提供与 receive_id_type 对应的有效 ID 或邮箱，不能传姓名",
            ));
        }
        Ok(())
    }
}

/// 消息 ID 直接进 URL 路径，只放 ASCII 字母数字与 `_` `-`。
pub fn valid_message_id(message_id: &str) -> bool {
    !message_id.is_empty()
        && message_id.len() <= 128
        && message_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

fn invalid_response() -> Error {
    Error::new(FailureKind::Fatal { code: 0 }, "消息调用未获得有效确认")
}

fn with_base(mut url: String, base: &str) -> String {
    if let Some(rest) = url.strip_prefix(crate::FEISHU_OPEN_BASE) {
        url = format!("{base}{rest}");
    }
    url
}

/// IM 客户端。
#[derive(Clone)]
pub struct ImClient {
    pub(crate) core: Arc<ClientCore>,
}

impl ImClient {
    pub(crate) fn new(core: Arc<ClientCore>) -> Self {
        Self { core }
    }

    fn open_base(&self) -> String {
        self.core.endpoints().open.as_str().to_string()
    }

    async fn send_message(&self, request: Request) -> Result<serde_json::Value> {
        let request = Request {
            url: with_base(request.url, &self.open_base()),
            ..request
        };
        let response = self.core.send_tenant(request, &MESSAGE_RETRY).await?;
        let value: serde_json::Value =
            serde_json::from_str(&response.body).map_err(|_| invalid_response())?;
        if value["code"].as_i64() != Some(0) {
            return Err(invalid_response());
        }
        Ok(value)
    }

    /// 发送富文本 post，返回 message_id。`content` 由调用方构造。
    pub async fn send_post(
        &self,
        recipient: &MessageRecipient,
        content: &serde_json::Value,
        uuid: &str,
    ) -> Result<String> {
        recipient.validate()?;
        let content = serde_json::to_string(content).map_err(|_| invalid_response())?;
        let request = Request {
            method: Method::Post,
            url: format!("{}/open-apis/im/v1/messages", crate::FEISHU_OPEN_BASE),
            query: vec![(
                "receive_id_type".to_string(),
                recipient.receive_id_type.as_str().to_string(),
            )],
            bearer_token: None,
            json_body: Some(serde_json::json!({
                "receive_id": recipient.receive_id.trim(),
                "msg_type": "post",
                "content": content,
                "uuid": uuid,
            })),
            raw_body: None,
            binary_limit: None,
            timeout_secs: Some(MESSAGE_REQUEST_TIMEOUT_SECS),
            // 同一 uuid 去重；230020 频控可安全重试。
            idempotent: true,
        };
        let value = self.send_message(request).await?;
        value["data"]["message_id"]
            .as_str()
            .filter(|id| !id.trim().is_empty())
            .map(str::to_owned)
            .ok_or_else(invalid_response)
    }

    /// 编辑已发送的 post（同一条最多编辑 20 次）。
    pub async fn edit_post(&self, message_id: &str, content: &serde_json::Value) -> Result<()> {
        if !valid_message_id(message_id) {
            return Err(invalid_response());
        }
        let content = serde_json::to_string(content).map_err(|_| invalid_response())?;
        let request = Request {
            method: Method::Put,
            url: format!(
                "{}/open-apis/im/v1/messages/{message_id}",
                crate::FEISHU_OPEN_BASE
            ),
            query: Vec::new(),
            bearer_token: None,
            json_body: Some(serde_json::json!({
                "msg_type": "post",
                "content": content,
            })),
            raw_body: None,
            binary_limit: None,
            timeout_secs: Some(MESSAGE_REQUEST_TIMEOUT_SECS),
            idempotent: true,
        };
        self.send_message(request).await.map(|_| ())
    }

    /// 上传消息文件，返回 `file_key`。
    pub async fn upload_file(
        &self,
        file_name: &str,
        file_type: &str,
        bytes: &[u8],
    ) -> Result<String> {
        let mut form = Multipart::new();
        form.text("file_type", file_type)
            .text("file_name", file_name)
            .file("file", file_name, "application/octet-stream", bytes);
        let (content_type, body) = form.finish();
        let response = self
            .core
            .send_tenant(
                Request {
                    method: Method::Post,
                    url: format!("{}/open-apis/im/v1/files", self.open_base()),
                    query: Vec::new(),
                    bearer_token: None,
                    json_body: None,
                    raw_body: Some((content_type, body)),
                    binary_limit: None,
                    timeout_secs: Some(MEDIA_REQUEST_TIMEOUT_SECS),
                    // 上传无幂等键，重试交给任务下一轮。
                    idempotent: false,
                },
                &MESSAGE_RETRY,
            )
            .await?;
        response_key(&response.body, "file_key")
    }

    /// 上传消息图片，返回 `image_key`。
    pub async fn upload_image(&self, image_type: &str, bytes: &[u8]) -> Result<String> {
        let mut form = Multipart::new();
        form.text("image_type", image_type)
            .file("image", "image.png", "image/png", bytes);
        let (content_type, body) = form.finish();
        let response = self
            .core
            .send_tenant(
                Request {
                    method: Method::Post,
                    url: format!("{}/open-apis/im/v1/images", self.open_base()),
                    query: Vec::new(),
                    bearer_token: None,
                    json_body: None,
                    raw_body: Some((content_type, body)),
                    binary_limit: None,
                    timeout_secs: Some(MEDIA_REQUEST_TIMEOUT_SECS),
                    idempotent: false,
                },
                &MESSAGE_RETRY,
            )
            .await?;
        response_key(&response.body, "image_key")
    }

    /// 下载消息里的图片/文件资源。
    pub async fn download_message_resource(
        &self,
        message_id: &str,
        file_key: &str,
        resource_type: &str,
    ) -> Result<Vec<u8>> {
        if !valid_message_id(message_id) {
            return Err(Error::fatal("消息 ID 非法"));
        }
        validate_opaque_path_segment("file_key", file_key)
            .map_err(|error| Error::fatal(error.to_string()))?;
        let response = self
            .core
            .send_tenant(
                Request {
                    method: Method::Get,
                    url: format!(
                        "{}/open-apis/im/v1/messages/{message_id}/resources/{file_key}",
                        self.open_base()
                    ),
                    query: vec![("type".to_string(), resource_type.to_string())],
                    bearer_token: None,
                    json_body: None,
                    raw_body: None,
                    binary_limit: Some(200 * 1024 * 1024),
                    timeout_secs: Some(MEDIA_REQUEST_TIMEOUT_SECS),
                    idempotent: true,
                },
                &MESSAGE_RETRY,
            )
            .await?;
        response
            .bytes
            .filter(|_| response.status == 200)
            .ok_or_else(|| Error::retry("消息资源下载不是完整二进制文件"))
    }
}

fn response_key(body: &str, key: &str) -> Result<String> {
    let value: serde_json::Value = serde_json::from_str(body).map_err(|_| invalid_response())?;
    value["data"][key]
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(invalid_response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipient_requires_explicit_type_and_valid_identifier() {
        let open_id = MessageRecipient {
            receive_id_type: ReceiveIdType::OpenId,
            receive_id: "ou_123".into(),
        };
        assert!(open_id.validate().is_ok());
        assert!(MessageRecipient {
            receive_id_type: ReceiveIdType::OpenId,
            receive_id: "张三".into(),
        }
        .validate()
        .is_err());
        assert!(MessageRecipient {
            receive_id_type: ReceiveIdType::Email,
            receive_id: "a@b.com".into(),
        }
        .validate()
        .is_ok());
        assert!(MessageRecipient {
            receive_id_type: ReceiveIdType::Email,
            receive_id: "a@b".into(),
        }
        .validate()
        .is_err());
    }

    #[test]
    fn message_id_rejects_path_injection() {
        assert!(valid_message_id("om_123-abc"));
        assert!(!valid_message_id("../evil"));
        assert!(!valid_message_id(""));
        assert!(!valid_message_id(&"a".repeat(129)));
    }
}
