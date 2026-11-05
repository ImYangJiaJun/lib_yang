//! 云盘媒体下载与审批附件上传。
//!
//! 下载走 `open` 域，上传走 `upload` 域（`www.feishu.cn`），两者不可混用。
//! 上传无幂等键：重试会重复上传，由调用方的任务轮次负责去重。

use std::sync::Arc;

use serde::Deserialize;

use crate::client::ClientCore;
use crate::error::{Error, Result};
use crate::multipart::Multipart;
use crate::path::{validate_opaque_path_segment, validate_path_segment};
use crate::transport::{Method, Request, MEDIA_REQUEST_TIMEOUT_SECS, PULL_RETRY};

/// 审批附件上传路径（挂在 `upload` 域名下）。
pub const APPROVAL_FILE_UPLOAD_PATH: &str = "/approval/openapi/v2/file/upload";

/// 一个多维表格附件。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Attachment {
    /// 源文件 token。
    pub file_token: String,
    /// 文件名。
    pub name: String,
    /// 字节数。
    pub size: u64,
}

/// 解析/校验附件时的参数。
#[derive(Debug, Clone, Copy)]
pub struct UploadSpec<'a> {
    /// 文案标签（「图片」/「附件」）。
    pub label: &'a str,
    /// 上传类型（`image` / `attachment`）。
    pub file_type: &'static str,
    /// 单文件大小上限（字节）。
    pub max_bytes: usize,
}

/// 附件契约失败。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AttachmentError {
    /// 单元格不是附件数组。
    #[error("{label}必须是多维表格附件数组")]
    NotArray {
        /// 文案标签。
        label: String,
    },
    /// file_token 非法。
    #[error("{label}含非法 file_token")]
    InvalidToken {
        /// 文案标签。
        label: String,
    },
    /// 文件名非法。
    #[error("{label}含非法文件名")]
    InvalidName {
        /// 文案标签。
        label: String,
    },
    /// 文件大小非法。
    #[error("{label}的 size 必须在 1 字节至 {max_mib} MiB 之间")]
    InvalidSize {
        /// 文案标签。
        label: String,
        /// 上限（MiB）。
        max_mib: usize,
    },
}

/// 解析并校验投标单元格里的附件数组。
pub fn parse_attachments(
    cell: &serde_json::Value,
    spec: UploadSpec<'_>,
) -> std::result::Result<Vec<Attachment>, AttachmentError> {
    if cell.is_null() {
        return Ok(Vec::new());
    }
    let files: Vec<Attachment> =
        serde_json::from_value(cell.clone()).map_err(|_| AttachmentError::NotArray {
            label: spec.label.to_string(),
        })?;
    for file in &files {
        if !valid_file_token(&file.file_token) {
            return Err(AttachmentError::InvalidToken {
                label: spec.label.to_string(),
            });
        }
        if !valid_file_name(&file.name) {
            return Err(AttachmentError::InvalidName {
                label: spec.label.to_string(),
            });
        }
        if file.size == 0 || file.size > spec.max_bytes as u64 {
            return Err(AttachmentError::InvalidSize {
                label: spec.label.to_string(),
                max_mib: spec.max_bytes / (1024 * 1024),
            });
        }
    }
    Ok(files)
}

fn valid_file_token(token: &str) -> bool {
    !token.is_empty()
        && token.len() <= 128
        && token.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn valid_file_name(name: &str) -> bool {
    name.len() <= 1024
        && !name
            .chars()
            .any(|c| c.is_control() || matches!(c, '"' | '\\' | '/'))
        && name
            .rsplit_once('.')
            .is_some_and(|(stem, ext)| !stem.trim().is_empty() && !ext.trim().is_empty())
}

/// 图片 part 的 Content-Type 按扩展名猜。
pub fn image_content_type(name: &str) -> &'static str {
    let extension = name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase());
    match extension.as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        Some("heic") => "image/heic",
        _ => "application/octet-stream",
    }
}

/// 构造 multipart 请求体；文件名必须在原语内部再次校验（防 CRLF 注入）。
pub fn multipart(
    name: &str,
    bytes: &[u8],
    file_type: &str,
) -> std::result::Result<(String, Vec<u8>), AttachmentError> {
    if !valid_file_name(name) {
        return Err(AttachmentError::InvalidName {
            label: file_type.to_string(),
        });
    }
    let content_type = if file_type == "image" {
        image_content_type(name)
    } else {
        "application/octet-stream"
    };
    let boundary = format!("feishu-{}", uuid::Uuid::new_v4());
    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"name\"\r\n\r\n{name}\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"type\"\r\n\r\n{file_type}\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"content\"; filename=\"{name}\"\r\nContent-Type: {content_type}\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    Ok((format!("multipart/form-data; boundary={boundary}"), body))
}

/// 云盘/附件客户端。
#[derive(Clone)]
pub struct DriveClient {
    pub(crate) core: Arc<ClientCore>,
}

impl DriveClient {
    pub(crate) fn new(core: Arc<ClientCore>) -> Self {
        Self { core }
    }

    /// 下载一个媒体文件；要求状态 200 且长度与记录一致。
    pub async fn download_media(
        &self,
        file_token: &str,
        extra: Option<&str>,
        expected_size: u64,
        max_bytes: usize,
    ) -> Result<Vec<u8>> {
        validate_path_segment("file_token", file_token)
            .map_err(|error| Error::fatal(error.to_string()))?;
        let mut query = Vec::new();
        if let Some(extra) = extra {
            query.push(("extra".to_string(), extra.to_string()));
        }
        let request = Request {
            method: Method::Get,
            url: format!(
                "{}/open-apis/drive/v1/medias/{file_token}/download",
                self.core.endpoints().open.as_str()
            ),
            query,
            bearer_token: None,
            json_body: None,
            raw_body: None,
            binary_limit: Some(max_bytes),
            timeout_secs: Some(MEDIA_REQUEST_TIMEOUT_SECS),
            idempotent: true,
        };
        let response = self.core.send_tenant(request, &PULL_RETRY).await?;
        response
            .bytes
            .filter(|bytes| response.status == 200 && bytes.len() as u64 == expected_size)
            .ok_or_else(|| Error::retry("附件/图片下载不是完整二进制文件或大小与记录不一致"))
    }

    /// 上传一个审批附件，返回 file code。
    pub async fn upload_approval_file(
        &self,
        name: &str,
        bytes: &[u8],
        file_type: &str,
    ) -> Result<String> {
        let (content_type, body) =
            multipart(name, bytes, file_type).map_err(|error| Error::fatal(error.to_string()))?;
        let request = Request {
            method: Method::Post,
            url: format!(
                "{}{APPROVAL_FILE_UPLOAD_PATH}",
                self.core.endpoints().upload.as_str()
            ),
            query: Vec::new(),
            bearer_token: None,
            json_body: None,
            raw_body: Some((content_type, body)),
            binary_limit: None,
            timeout_secs: Some(MEDIA_REQUEST_TIMEOUT_SECS),
            // 上传无幂等键；重试交给任务下一轮。
            idempotent: false,
        };
        let response = self.core.send_tenant(request, &PULL_RETRY).await?;
        let value: serde_json::Value = serde_json::from_str(&response.body)
            .map_err(|_| Error::retry("审批附件/图片上传响应不是 JSON"))?;
        value
            .get("data")
            .and_then(|data| data.get("code"))
            .and_then(serde_json::Value::as_str)
            .filter(|code| {
                value.get("code").and_then(serde_json::Value::as_i64) == Some(0)
                    && !code.trim().is_empty()
            })
            .map(str::to_string)
            .ok_or_else(|| Error::retry("审批附件/图片上传响应缺少成功标记或 file code"))
    }

    /// 上传文件到云空间（`/files/upload_all`），返回 `file_token`。
    pub async fn upload_file(
        &self,
        file_name: &str,
        parent_type: &str,
        parent_node: &str,
        bytes: &[u8],
    ) -> Result<String> {
        self.upload_all(
            "/open-apis/drive/v1/files/upload_all",
            file_name,
            parent_type,
            parent_node,
            bytes,
        )
        .await
    }

    /// 上传素材到云空间（`/medias/upload_all`），返回 `file_token`。
    pub async fn upload_media(
        &self,
        file_name: &str,
        parent_type: &str,
        parent_node: &str,
        bytes: &[u8],
    ) -> Result<String> {
        self.upload_all(
            "/open-apis/drive/v1/medias/upload_all",
            file_name,
            parent_type,
            parent_node,
            bytes,
        )
        .await
    }

    async fn upload_all(
        &self,
        path: &str,
        file_name: &str,
        parent_type: &str,
        parent_node: &str,
        bytes: &[u8],
    ) -> Result<String> {
        let mut form = Multipart::new();
        form.text("file_name", file_name)
            .text("parent_type", parent_type)
            .text("parent_node", parent_node)
            .text("size", &bytes.len().to_string())
            .file("file", file_name, "application/octet-stream", bytes);
        let (content_type, body) = form.finish();
        let response = self
            .core
            .send_tenant(
                Request {
                    method: Method::Post,
                    url: format!("{}{path}", self.core.endpoints().open.as_str()),
                    query: Vec::new(),
                    bearer_token: None,
                    json_body: None,
                    raw_body: Some((content_type, body)),
                    binary_limit: None,
                    timeout_secs: Some(MEDIA_REQUEST_TIMEOUT_SECS),
                    // 上传无幂等键，重试交给任务下一轮。
                    idempotent: false,
                },
                &PULL_RETRY,
            )
            .await?;
        let value: serde_json::Value = serde_json::from_str(&response.body)
            .map_err(|_| Error::retry("云空间上传响应不是 JSON"))?;
        value["data"]["file_token"]
            .as_str()
            .filter(|token| !token.trim().is_empty())
            .map(str::to_owned)
            .ok_or_else(|| Error::retry("云空间上传响应缺少 file_token"))
    }

    /// 下载云空间文件；`max_bytes` 是二进制上限。
    pub async fn download_file(&self, file_token: &str, max_bytes: usize) -> Result<Vec<u8>> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| Error::fatal(error.to_string()))?;
        let response = self
            .core
            .send_tenant(
                Request {
                    method: Method::Get,
                    url: format!(
                        "{}/open-apis/drive/v1/files/{file_token}/download",
                        self.core.endpoints().open.as_str()
                    ),
                    query: Vec::new(),
                    bearer_token: None,
                    json_body: None,
                    raw_body: None,
                    binary_limit: Some(max_bytes),
                    timeout_secs: Some(MEDIA_REQUEST_TIMEOUT_SECS),
                    idempotent: true,
                },
                &PULL_RETRY,
            )
            .await?;
        response
            .bytes
            .filter(|_| response.status == 200)
            .ok_or_else(|| Error::retry("云空间文件下载不是完整二进制文件"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attachment_metadata_is_validated() {
        let spec = UploadSpec {
            label: "附件",
            file_type: "attachment",
            max_bytes: 50 * 1024 * 1024,
        };
        let files = parse_attachments(
            &serde_json::json!([{"file_token": "boxcn123", "name": "a.pdf", "size": 3}]),
            spec,
        )
        .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(files.len(), 1);

        for bad in [
            serde_json::json!({"file_token": "boxcn123", "name": "a.pdf", "size": 3}),
            serde_json::json!([{"file_token": "../x", "name": "a.pdf", "size": 3}]),
            serde_json::json!([{"file_token": "boxcn123", "name": "a", "size": 3}]),
            serde_json::json!([{"file_token": "boxcn123", "name": "a.pdf", "size": 0}]),
        ] {
            assert!(parse_attachments(&bad, spec).is_err());
        }
    }

    #[test]
    fn multipart_carries_type_and_guessed_content_type() {
        let (content_type, body) =
            multipart("a.png", b"abc", "image").unwrap_or_else(|error| panic!("{error}"));
        assert!(content_type.starts_with("multipart/form-data; boundary=feishu-"));
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("name=\"type\"\r\n\r\nimage"));
        assert!(text.contains("Content-Type: image/png"));
        assert!(multipart("a\nb.png", b"abc", "image").is_err());
    }

    #[test]
    fn image_content_type_falls_back_to_octet_stream() {
        assert_eq!(image_content_type("x.JPG"), "image/jpeg");
        assert_eq!(image_content_type("x.unknown"), "application/octet-stream");
    }
}
