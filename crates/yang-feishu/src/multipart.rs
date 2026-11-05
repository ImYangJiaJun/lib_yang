//! 最小 multipart/form-data 构造器（仅 crate 内部使用）。
//!
//! 不引入第三方 multipart 依赖；字段名与文件名在这里统一做 CRLF 注入校验。

/// 一个 multipart 表单。
pub(crate) struct Multipart {
    boundary: String,
    body: Vec<u8>,
    closed: bool,
}

impl Multipart {
    /// 新建表单（随机 boundary）。
    pub(crate) fn new() -> Self {
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

    fn ensure_open(&self) {
        assert!(!self.closed, "multipart 已结束");
    }

    /// 追加一个文本字段。
    pub(crate) fn text(&mut self, name: &str, value: &str) -> &mut Self {
        self.ensure_open();
        validate_token("字段名", name);
        self.push_line(&format!("--{}", self.boundary));
        self.push_line(&format!("Content-Disposition: form-data; name=\"{name}\""));
        self.push_line("");
        self.push_line(value);
        self
    }

    /// 追加一个文件字段。
    pub(crate) fn file(
        &mut self,
        name: &str,
        filename: &str,
        content_type: &str,
        bytes: &[u8],
    ) -> &mut Self {
        self.ensure_open();
        validate_token("字段名", name);
        validate_token("文件名", filename);
        self.push_line(&format!("--{}", self.boundary));
        self.push_line(&format!(
            "Content-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\""
        ));
        self.push_line(&format!("Content-Type: {content_type}"));
        self.push_line("");
        self.body.extend_from_slice(bytes);
        self.body.extend_from_slice(b"\r\n");
        self
    }

    /// 结束表单，返回 `(Content-Type, body)`。
    pub(crate) fn finish(mut self) -> (String, Vec<u8>) {
        self.closed = true;
        self.push_line(&format!("--{}--", self.boundary));
        (
            format!("multipart/form-data; boundary={}", self.boundary),
            self.body,
        )
    }
}

fn validate_token(label: &str, value: &str) {
    assert!(
        !value.is_empty()
            && !value
                .chars()
                .any(|c| c.is_control() || matches!(c, '"' | '\\' | '\r' | '\n')),
        "{label}含非法字符"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_multipart_with_text_and_file_parts() {
        let mut form = Multipart::new();
        form.text("file_name", "a.txt")
            .file("file", "a.txt", "text/plain", b"hello");
        let (content_type, body) = form.finish();
        assert!(content_type.starts_with("multipart/form-data; boundary=feishu-"));
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("name=\"file_name\""));
        assert!(text.contains("filename=\"a.txt\""));
        assert!(text.contains("hello"));
    }

    #[test]
    #[should_panic]
    fn rejects_crlf_in_names() {
        let mut form = Multipart::new();
        form.text("a\r\nX", "v");
    }
}
