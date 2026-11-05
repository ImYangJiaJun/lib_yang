//! URL 路径段校验。
//!
//! 保留**两套语义**（设计 §0.3 #10）：
//!
//! - [`validate_path_segment`]：严格白名单，只放 ASCII 字母数字与 `-` `_`。
//!   用于多维表格的 `app_token` / `table_id` / `view_id` / `record_id`。
//! - [`validate_opaque_path_segment`]：宽松黑名单，只拒绝 `/ ? # 空格 \`。
//!   用于审批 `instance_code` / `approval_code` / `department_id`——这些值历史上
//!   允许 `.` `:` `@` 等字符，收紧会改变既有入参行为。
//!
//! 不要为了「共用一份」把多维表格的路径穿越防护降级。

/// 路径段长度上限（字节）。
pub const MAX_PATH_SEGMENT_LEN: usize = 128;

/// 路径段校验失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathError {
    /// 参数名（用于定位是哪一个值非法）。
    pub name: String,
    /// 可读原因。
    pub reason: String,
}

impl PathError {
    fn new(name: &str, reason: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            reason: reason.into(),
        }
    }
}

impl std::fmt::Display for PathError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} {}", self.name, self.reason)
    }
}

impl std::error::Error for PathError {}

/// 严格校验一个要拼进 URL 路径段的值（多维表格用）。
///
/// 白名单只放 ASCII 字母数字与 `-` `_`；base token、table id、record id 都落在
/// 这个集合内。`page_token` 是服务端下发的不透明游标，**不能**用这个函数校验。
pub fn validate_path_segment(name: &str, value: &str) -> std::result::Result<(), PathError> {
    if value.is_empty() || value.len() > MAX_PATH_SEGMENT_LEN {
        return Err(PathError::new(
            name,
            format!("长度必须在 1..={MAX_PATH_SEGMENT_LEN} 字节"),
        ));
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(PathError::new(
            name,
            "只能包含 ASCII 字母、数字、连字符与下划线",
        ));
    }
    Ok(())
}

/// 宽松校验一个要拼进 URL 路径段的值（审批/通讯录用）。
///
/// 只拒绝 `/ ? # 空格 \`，保留审批侧历史上允许的 `.` `:` `@` 等字符。
pub fn validate_opaque_path_segment(name: &str, value: &str) -> std::result::Result<(), PathError> {
    if value.is_empty() {
        return Err(PathError::new(name, "不能为空"));
    }
    if value.len() > MAX_PATH_SEGMENT_LEN {
        return Err(PathError::new(
            name,
            format!("超长（{} > {MAX_PATH_SEGMENT_LEN}）", value.len()),
        ));
    }
    if value.contains(['/', '?', '#', ' ', '\\']) {
        return Err(PathError::new(name, format!("含非法字符：{value}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_segment_accepts_bitable_ids() {
        assert!(validate_path_segment("app_token", "bascnCMII2ORej2RItqpZZUNMIe").is_ok());
        assert!(validate_path_segment("table_id", "tblxI2tWaxP5dG7p").is_ok());
        assert!(validate_path_segment("record_id", "recABC_123-x").is_ok());
    }

    #[test]
    fn strict_segment_rejects_traversal_and_punctuation() {
        for value in ["../evil", "a/b", "a?b", "a#b", "a b", "a\\b", "a.b", "中文"] {
            assert!(
                validate_path_segment("id", value).is_err(),
                "应拒绝 {value:?}"
            );
        }
        assert!(validate_path_segment("id", "").is_err());
        assert!(validate_path_segment("id", &"a".repeat(129)).is_err());
    }

    #[test]
    fn opaque_segment_keeps_dots_colons_and_at() {
        for value in ["4202AD96-9EC1.2026", "user:123", "a@b", "部门-1"] {
            assert!(
                validate_opaque_path_segment("id", value).is_ok(),
                "应接受 {value:?}"
            );
        }
        for value in ["../evil", "a/b", "a?b", "a#b", "a b", "a\\b", ""] {
            assert!(
                validate_opaque_path_segment("id", value).is_err(),
                "应拒绝 {value:?}"
            );
        }
    }
}
