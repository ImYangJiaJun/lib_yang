//! Action 请求媒体类型与受限 multipart 上传契约。

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 默认允许的非文件表单字段数量。
pub const DEFAULT_MULTIPART_MAX_FIELDS: u16 = 64;
/// 默认允许的文件数量。
pub const DEFAULT_MULTIPART_MAX_FILES: u16 = 8;
/// 默认单文件上限：10 MiB。
pub const DEFAULT_MULTIPART_MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
/// 默认整个 multipart 请求上限：32 MiB。
pub const DEFAULT_MULTIPART_MAX_TOTAL_BYTES: u64 = 32 * 1024 * 1024;

/// Action 接受的请求媒体类型。
///
/// 未知值必须降级为 JSON，不能在消费者侧擅自启用文件上传。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ActionMediaType {
    /// `multipart/form-data`，必须同时提供 [`MultipartSpec`]。
    Multipart,
    /// `application/json`，也是未知值的安全降级。
    #[default]
    #[serde(other)]
    Json,
}

/// 上传临时文件的生命周期。
///
/// 当前只允许请求作用域：Handler 返回后框架删除临时文件；业务若需持久化，必须在
/// Handler 内显式复制或移动到自己的受控存储。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum UploadLifecycle {
    /// 请求结束即清理的临时文件。
    #[default]
    #[serde(other)]
    RequestScoped,
}

/// `multipart/form-data` 的资源与类型限制。
///
/// `allowed_content_types` 只校验客户端声明的媒体类型，不能替代 Handler 对文件魔数、
/// 内容和业务格式的验证。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MultipartSpec {
    /// 非文件表单字段数量上限。
    pub max_fields: u16,
    /// 文件字段数量上限。
    pub max_files: u16,
    /// 单文件字节上限。
    pub max_file_bytes: u64,
    /// 整个 multipart body 的字节上限。
    pub max_total_bytes: u64,
    /// 允许的精确 MIME 类型；不允许空集合或通配符。
    pub allowed_content_types: Vec<String>,
    /// 临时文件生命周期。
    pub lifecycle: UploadLifecycle,
}

impl MultipartSpec {
    /// 使用安全默认资源上限创建上传契约。
    pub fn new<I, S>(allowed_content_types: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            max_fields: DEFAULT_MULTIPART_MAX_FIELDS,
            max_files: DEFAULT_MULTIPART_MAX_FILES,
            max_file_bytes: DEFAULT_MULTIPART_MAX_FILE_BYTES,
            max_total_bytes: DEFAULT_MULTIPART_MAX_TOTAL_BYTES,
            allowed_content_types: allowed_content_types.into_iter().map(Into::into).collect(),
            lifecycle: UploadLifecycle::RequestScoped,
        }
    }

    /// 设置非文件表单字段数量上限。
    #[must_use]
    pub fn max_fields(mut self, max_fields: u16) -> Self {
        self.max_fields = max_fields;
        self
    }

    /// 设置文件字段数量上限。
    #[must_use]
    pub fn max_files(mut self, max_files: u16) -> Self {
        self.max_files = max_files;
        self
    }

    /// 设置单文件字节上限。
    #[must_use]
    pub fn max_file_bytes(mut self, max_file_bytes: u64) -> Self {
        self.max_file_bytes = max_file_bytes;
        self
    }

    /// 设置整个 multipart body 的字节上限。
    #[must_use]
    pub fn max_total_bytes(mut self, max_total_bytes: u64) -> Self {
        self.max_total_bytes = max_total_bytes;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_media_and_lifecycle_values_fail_closed() {
        let media: ActionMediaType =
            serde_json::from_str("\"websocket\"").expect("未知请求媒体类型应安全解析");
        let lifecycle: UploadLifecycle =
            serde_json::from_str("\"permanent\"").expect("未知上传生命周期应安全解析");

        assert_eq!(media, ActionMediaType::Json);
        assert_eq!(lifecycle, UploadLifecycle::RequestScoped);
    }

    #[test]
    fn multipart_defaults_are_bounded() {
        let spec = MultipartSpec::new(["application/pdf"]);
        assert_eq!(spec.max_fields, DEFAULT_MULTIPART_MAX_FIELDS);
        assert_eq!(spec.max_files, DEFAULT_MULTIPART_MAX_FILES);
        assert_eq!(spec.max_file_bytes, DEFAULT_MULTIPART_MAX_FILE_BYTES);
        assert_eq!(spec.max_total_bytes, DEFAULT_MULTIPART_MAX_TOTAL_BYTES);
        assert_eq!(spec.lifecycle, UploadLifecycle::RequestScoped);
    }
}
