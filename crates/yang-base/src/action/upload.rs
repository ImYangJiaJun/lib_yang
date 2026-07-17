//! 请求作用域上传文件句柄。

use crate::error::BaseError;
use schemars::gen::SchemaGenerator;
use schemars::schema::{InstanceType, Schema, SchemaObject, SingleOrVec};
use schemars::JsonSchema;
use serde::Deserialize;
use std::fmt;
use std::path::{Path, PathBuf};

/// 由受信传输层创建的请求作用域上传文件。
///
/// 临时路径只在当前 Handler 执行期间有效。需要持久化时调用 [`copy_to`](Self::copy_to)
/// 或使用受控存储服务显式转存；保存本路径供请求结束后使用是无效的。
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UploadedFile {
    field_name: String,
    original_filename: String,
    content_type: String,
    size: u64,
    path: PathBuf,
}

impl UploadedFile {
    /// 返回 multipart 字段名。
    pub fn field_name(&self) -> &str {
        &self.field_name
    }

    /// 返回去除客户端路径片段后的原始文件名。
    pub fn original_filename(&self) -> &str {
        &self.original_filename
    }

    /// 返回已通过 Action 白名单校验的客户端声明 MIME 类型。
    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    /// 返回实际接收的文件字节数。
    pub const fn size(&self) -> u64 {
        self.size
    }

    /// 返回只在当前请求期间有效的临时路径。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 将上传内容显式复制到业务拥有的目标位置。
    pub async fn copy_to(&self, destination: impl AsRef<Path>) -> Result<u64, BaseError> {
        tokio::fs::copy(&self.path, destination)
            .await
            .map_err(BaseError::from)
    }
}

impl fmt::Debug for UploadedFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UploadedFile")
            .field("field_name", &self.field_name)
            .field("original_filename", &self.original_filename)
            .field("content_type", &self.content_type)
            .field("size", &self.size)
            .field("path", &"[REQUEST_SCOPED_TEMP_FILE]")
            .finish()
    }
}

impl JsonSchema for UploadedFile {
    fn schema_name() -> String {
        "UploadedFile".to_string()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        Schema::Object(SchemaObject {
            instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::String))),
            format: Some("binary".to_string()),
            ..SchemaObject::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uploaded_file_projects_as_openapi_binary_input() {
        let schema = schemars::schema_for!(UploadedFile);
        let value = serde_json::to_value(schema).expect("UploadedFile schema 应可序列化");

        assert_eq!(value["type"], "string");
        assert_eq!(value["format"], "binary");
        assert!(value.get("path").is_none(), "临时路径不得进入输入契约");
    }
}
