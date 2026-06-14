//! SQL 标识符校验与转义（DB-1）。
//!
//! yang-db 的值侧已全部参数化（`?` 占位符 + 绑定），无注入风险；但标识符（表名、
//! 列名）此前全程裸 `push_str` 拼接，直接喂外部输入即可注入。本模块提供与 yang-base
//! 受保护层一致的校验/转义能力：
//!
//! - [`is_valid_identifier`]：仅允许 `[A-Za-z_][A-Za-z0-9_]*`，杜绝含空格/引号/分号/
//!   括号等的注入载荷或表达式。
//! - [`quote_identifier`]：校验后用反引号包裹并对内部反引号做加倍转义（MySQL 方言）。
//! - [`quote_qualified`]：支持 `表.列` 限定名，逐段校验并各自加引号。
//!
//! **使用边界**：写入路径（INSERT/UPDATE/UPSERT 的列名、各 DML 的表名）的标识符来自
//! 调用方提供的列名/表名，是真·标识符，统一在 SQL 生成收口处 quote。而 `field()`/
//! `order()`/`group()`/JOIN `ON` 按设计接受 SQL 表达式（如 `COUNT(*) AS c`、`a.b`、
//! `YEAR(d)`），属**可信输入**，不在此强制 quote——直接消费 yang-db 且需要喂入外部
//! 输入的调用方，应先用本模块的 `pub` 助手自行校验。

use crate::error::DbError;

/// 校验是否为合法的 SQL 标识符：首字符为字母或下划线，其余为字母/数字/下划线。
///
/// 空串、含空格/引号/分号/括号/点号等的字符串一律判为非法（点号限定名请用
/// [`quote_qualified`]）。纯 `char` 迭代，零分配。
pub fn is_valid_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// 校验并用反引号转义一个标识符（MySQL 方言）。
///
/// 合法标识符返回 `` `ident` ``（内部反引号加倍）；非法返回
/// [`DbError::InvalidArgument`]。
pub fn quote_identifier(ident: &str) -> Result<String, DbError> {
    if !is_valid_identifier(ident) {
        return Err(DbError::InvalidArgument(format!(
            "非法 SQL 标识符: {ident:?}"
        )));
    }
    // is_valid_identifier 已排除反引号，replace 仅为防御性冗余。
    Ok(format!("`{}`", ident.replace('`', "``")))
}

/// 校验并转义可能带限定前缀的标识符：`列` → `` `列` ``，`表.列` → `` `表`.`列` ``。
///
/// 各段分别校验并加引号；段数超过 2 或任一段非法返回 [`DbError::InvalidArgument`]。
pub fn quote_qualified(ident: &str) -> Result<String, DbError> {
    let parts: Vec<&str> = ident.split('.').collect();
    if parts.is_empty() || parts.len() > 2 {
        return Err(DbError::InvalidArgument(format!(
            "非法限定标识符: {ident:?}"
        )));
    }
    let quoted: Result<Vec<String>, DbError> = parts.iter().map(|p| quote_identifier(p)).collect();
    Ok(quoted?.join("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_identifier() {
        assert!(is_valid_identifier("users"));
        assert!(is_valid_identifier("user_id"));
        assert!(is_valid_identifier("_internal"));
        assert!(is_valid_identifier("a1"));
        // 非法
        assert!(!is_valid_identifier(""));
        assert!(!is_valid_identifier("1users"));
        assert!(!is_valid_identifier("user name"));
        assert!(!is_valid_identifier("user-name"));
        assert!(!is_valid_identifier("user;DROP"));
        assert!(!is_valid_identifier("a.b"));
        assert!(!is_valid_identifier("COUNT(*)"));
        assert!(!is_valid_identifier("a`b"));
    }

    #[test]
    fn test_quote_identifier() {
        assert_eq!(quote_identifier("users").unwrap(), "`users`");
        assert_eq!(quote_identifier("user_id").unwrap(), "`user_id`");
        assert!(quote_identifier("user;DROP").is_err());
        assert!(quote_identifier("a b").is_err());
    }

    #[test]
    fn test_quote_qualified() {
        assert_eq!(quote_qualified("name").unwrap(), "`name`");
        assert_eq!(quote_qualified("users.name").unwrap(), "`users`.`name`");
        assert!(quote_qualified("a.b.c").is_err());
        assert!(quote_qualified("users.COUNT(*)").is_err());
    }
}
