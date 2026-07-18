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
///
/// # 受信边界（C-1）
///
/// 实例只能由 multipart 传输层在服务端构造的 JSON 反序列化得到：传输层在构造处注入
/// `temp_root`（请求作用域临时目录）。客户端 JSON 缺省该字段，反序列化后为 `None`，
/// [`copy_to`](Self::copy_to) 一律拒绝，防止普通 JSON Action 伪造 `path` 读取本地文件。
/// 构建期 `AppBuilder` 另有双向校验：含二进制文件字段的输入必须声明 multipart，
/// multipart Action 必须至少声明一个文件字段。
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UploadedFile {
    field_name: String,
    original_filename: String,
    content_type: String,
    size: u64,
    path: PathBuf,
    /// 受信临时根目录；仅 multipart 传输层在服务端构造的 JSON 中注入，
    /// 客户端反序列化（缺省）为 `None`。
    #[serde(default)]
    temp_root: Option<PathBuf>,
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
    ///
    /// 安全校验（fail-closed）：实例必须持有受信临时根（即来自 multipart 传输层），
    /// 且 `path` 与临时根经 `canonicalize` 解析后必须位于临时根之内，否则拒绝复制；
    /// 复制使用校验时解析出的规范路径，消除符号链接交换（TOCTOU）竞争窗口。
    pub async fn copy_to(&self, destination: impl AsRef<Path>) -> Result<u64, BaseError> {
        let temp_root = self.temp_root.as_ref().ok_or_else(|| {
            BaseError::PermissionDenied(
                "上传文件缺少受信临时根（疑似不可信反序列化实例），拒绝复制".to_string(),
            )
        })?;
        let root = temp_root.canonicalize().map_err(|error| {
            BaseError::PermissionDenied(format!("受信临时根校验失败，拒绝复制: {error}"))
        })?;
        let source = self.path.canonicalize().map_err(|error| {
            BaseError::PermissionDenied(format!("上传临时文件校验失败，拒绝复制: {error}"))
        })?;
        if !source.starts_with(&root) {
            return Err(BaseError::PermissionDenied(
                "上传文件路径越出受信临时根，拒绝复制".to_string(),
            ));
        }
        tokio::fs::copy(&source, destination)
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
            .field("trusted", &self.temp_root.is_some())
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

    /// 创建带唯一名字的测试临时目录（不依赖 tempfile，兼容无 transport-axum 的测试矩阵）。
    fn unique_test_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "yang_upload_test_{tag}_{}_{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("测试临时目录应创建成功");
        dir
    }

    #[test]
    fn uploaded_file_projects_as_openapi_binary_input() {
        let schema = schemars::schema_for!(UploadedFile);
        let value = serde_json::to_value(schema).expect("UploadedFile schema 应可序列化");

        assert_eq!(value["type"], "string");
        assert_eq!(value["format"], "binary");
        assert!(value.get("path").is_none(), "临时路径不得进入输入契约");
    }

    #[tokio::test]
    async fn copy_to_rejects_deserialized_instance_without_trusted_temp_root() {
        // C-1：客户端经 JSON 通道伪造的实例没有受信临时根，copy_to 必须 fail-closed 拒绝。
        // 以当前可执行文件为受害路径：修复前 copy_to 会以服务进程权限直接复制它（漏洞复现）。
        let victim = std::env::current_exe().expect("当前可执行文件路径应可获取");
        let forged: UploadedFile = serde_json::from_value(serde_json::json!({
            "field_name": "file",
            "original_filename": "payload.bin",
            "content_type": "application/octet-stream",
            "size": 1,
            "path": victim,
        }))
        .expect("伪造实例应可反序列化");
        let dir = unique_test_dir("untrusted");
        let destination = dir.join("stolen.bin");

        let error = forged
            .copy_to(&destination)
            .await
            .expect_err("无受信临时根的实例必须拒绝复制");
        assert!(
            matches!(error, BaseError::PermissionDenied(_)),
            "无受信临时根应返回 PermissionDenied: {error}"
        );
        assert!(!destination.exists(), "拒绝复制不得产生目标文件");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn copy_to_rejects_path_escaping_trusted_temp_root() {
        // C-1：即使实例持有受信临时根，越出根目录的路径也必须拒绝（防 symlink/../ 逃逸）。
        let root = unique_test_dir("root");
        let outside = unique_test_dir("outside");
        let secret = outside.join("secret.txt");
        std::fs::write(&secret, b"top-secret").expect("越界文件应写入成功");
        let file = UploadedFile {
            field_name: "file".to_string(),
            original_filename: "secret.txt".to_string(),
            content_type: "text/plain".to_string(),
            size: 10,
            path: secret,
            temp_root: Some(root.clone()),
        };
        let destination = root.join("copy.txt");

        let error = file
            .copy_to(&destination)
            .await
            .expect_err("越出受信临时根的路径必须拒绝");
        assert!(
            matches!(error, BaseError::PermissionDenied(_)),
            "越界路径应返回 PermissionDenied: {error}"
        );
        assert!(!destination.exists(), "拒绝复制不得产生目标文件");
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[tokio::test]
    async fn copy_to_copies_file_inside_trusted_temp_root() {
        // 受信临时根内的合法文件应正常复制（canonicalize 双端解析后 starts_with 通过）。
        let root = unique_test_dir("trusted");
        let source = root.join("yang-upload-1");
        std::fs::write(&source, b"hello").expect("源文件应写入成功");
        let file = UploadedFile {
            field_name: "file".to_string(),
            original_filename: "upload.bin".to_string(),
            content_type: "application/octet-stream".to_string(),
            size: 5,
            path: source,
            temp_root: Some(root.clone()),
        };
        let destination = root.join("saved.bin");

        let copied = file
            .copy_to(&destination)
            .await
            .expect("受信临时根内的文件应复制成功");
        assert_eq!(copied, 5);
        assert_eq!(
            std::fs::read(&destination).expect("目标文件应可读"),
            b"hello"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn transport_injected_temp_root_deserializes_into_trusted_instance() {
        // transport 在服务端构造的 JSON 会注入 temp_root；缺省（客户端 JSON）则不得获得。
        let trusted: UploadedFile = serde_json::from_value(serde_json::json!({
            "field_name": "file",
            "original_filename": "a.txt",
            "content_type": "text/plain",
            "size": 3,
            "path": "/tmp/scope/yang-upload-1",
            "temp_root": "/tmp/scope",
        }))
        .expect("transport 注入的 temp_root 应可反序列化");
        assert_eq!(
            trusted.temp_root.as_deref(),
            Some(Path::new("/tmp/scope")),
            "transport 注入的临时根必须保留"
        );

        let forged: UploadedFile = serde_json::from_value(serde_json::json!({
            "field_name": "file",
            "original_filename": "a.txt",
            "content_type": "text/plain",
            "size": 3,
            "path": "/etc/passwd",
        }))
        .expect("缺省 temp_root 应可反序列化");
        assert!(
            forged.temp_root.is_none(),
            "客户端 JSON 反序列化的实例不得获得受信临时根"
        );
    }
}
