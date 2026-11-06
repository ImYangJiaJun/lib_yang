//! 最小 multipart/form-data 构造器。
//!
//! 不引入第三方 multipart 依赖；字段名、字段值、文件名与 `Content-Type`
//! 在这里统一做 CRLF 注入校验。校验失败返回 [`MultipartError`]，不 panic。

/// multipart 构造失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultipartError {
    /// 可读原因。
    pub message: String,
}

impl std::fmt::Display for MultipartError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for MultipartError {}

impl From<MultipartError> for crate::error::Error {
    fn from(error: MultipartError) -> Self {
        crate::error::Error::fatal(error.to_string())
    }
}

/// 一个 multipart 表单。
pub struct Multipart {
    boundary: String,
    body: Vec<u8>,
    closed: bool,
}

impl Multipart {
    /// 新建表单（随机 boundary）。
    pub fn new() -> Self {
        Self {
            boundary: format!("feishu-{}", uuid::Uuid::new_v4()),
            body: Vec::new(),
            closed: false,
        }
    }

    fn push_line(&mut self, line: &str) {
        self.body.extend_from_slice(line.as_bytes());
        self.body.extend_from_slice(b"\r\n");
    }

    fn ensure_open(&self) -> Result<(), MultipartError> {
        if self.closed {
            return Err(MultipartError {
                message: "multipart 已结束".to_string(),
            });
        }
        Ok(())
    }

    /// 追加一个文本字段。
    pub fn text(&mut self, name: &str, value: &str) -> Result<&mut Self, MultipartError> {
        self.ensure_open()?;
        validate_token("字段名", name)?;
        validate_value("字段值", value)?;
        self.push_line(&format!("--{}", self.boundary));
        self.push_line(&format!("Content-Disposition: form-data; name=\"{name}\""));
        self.push_line("");
        self.push_line(value);
        Ok(self)
    }

    /// 追加一个文件字段。
    pub fn file(
        &mut self,
        name: &str,
        filename: &str,
        content_type: &str,
        bytes: &[u8],
    ) -> Result<&mut Self, MultipartError> {
        self.ensure_open()?;
        validate_token("字段名", name)?;
        validate_token("文件名", filename)?;
        validate_token("Content-Type", content_type)?;
        self.push_line(&format!("--{}", self.boundary));
        self.push_line(&format!(
            "Content-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\""
        ));
        self.push_line(&format!("Content-Type: {content_type}"));
        self.push_line("");
        self.body.extend_from_slice(bytes);
        self.body.extend_from_slice(b"\r\n");
        Ok(self)
    }

    /// 结束表单，返回 `(Content-Type, body)`。
    pub fn finish(mut self) -> (String, Vec<u8>) {
        self.closed = true;
        self.push_line(&format!("--{}--", self.boundary));
        (
            format!("multipart/form-data; boundary={}", self.boundary),
            self.body,
        )
    }
}

impl Default for Multipart {
    fn default() -> Self {
        Self::new()
    }
}

fn validate_token(label: &str, value: &str) -> Result<(), MultipartError> {
    if value.is_empty()
        || value
            .chars()
            .any(|c| c.is_control() || matches!(c, '"' | '\\' | '\r' | '\n'))
    {
        return Err(MultipartError {
            message: format!("{label}含非法字符: {value:?}"),
        });
    }
    Ok(())
}

fn validate_value(label: &str, value: &str) -> Result<(), MultipartError> {
    if value
        .chars()
        .any(|c| matches!(c, '\r' | '\n') || c.is_control())
    {
        return Err(MultipartError {
            message: format!("{label}含换行或控制字符: {value:?}"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_multipart_with_text_and_file_parts() {
        let mut form = Multipart::new();
        form.text("file_name", "a.txt")
            .unwrap_or_else(|error| panic!("{error}"))
            .file("file", "a.txt", "text/plain", b"hello")
            .unwrap_or_else(|error| panic!("{error}"));
        let (content_type, body) = form.finish();
        assert!(content_type.starts_with("multipart/form-data; boundary=feishu-"));
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("name=\"file_name\""));
        assert!(text.contains("filename=\"a.txt\""));
        assert!(text.contains("hello"));
    }

    #[test]
    fn rejects_crlf_and_control_characters() {
        let mut form = Multipart::new();
        assert!(form.text("a\r\nX", "v").is_err());
        assert!(form.text("name", "v\r\nX").is_err());
        assert!(form.file("file", "a.txt", "text/plain\r\nX", b"v").is_err());
    }
}
