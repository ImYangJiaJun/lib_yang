//! SQL 标识符校验与转义（DB-1，PostgreSQL 方言）。
//!
//! 与 MySQL 版同构，区别仅在转义字符：PostgreSQL 标识符用双引号包裹、内部双引号加倍。
//! 详见 `crate::mysql::identifier` 的模块说明与使用边界。

use crate::error::DbError;
use crate::sql_types::{Identifier, QualifiedIdentifier};

/// 校验是否为合法的 SQL 标识符：首字符为字母或下划线，其余为字母/数字/下划线。
pub fn is_valid_identifier(s: &str) -> bool {
    Identifier::parse(s).is_ok()
}

/// 校验并用双引号转义一个标识符（PostgreSQL 方言）。
///
/// 合法标识符返回 `"ident"`（内部双引号加倍）；非法返回 [`DbError::InvalidArgument`]。
pub fn quote_identifier(ident: &str) -> Result<String, DbError> {
    Ok(QualifiedIdentifier::Unqualified(Identifier::parse(ident)?).render('"'))
}

/// 校验并转义可能带限定前缀的标识符：`列` → `"列"`，`表.列` → `"表"."列"`。
pub fn quote_qualified(ident: &str) -> Result<String, DbError> {
    Ok(QualifiedIdentifier::parse(ident)?.render('"'))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_identifier() {
        assert!(is_valid_identifier("users"));
        assert!(is_valid_identifier("_x"));
        assert!(!is_valid_identifier(""));
        assert!(!is_valid_identifier("1a"));
        assert!(!is_valid_identifier("a;b"));
        assert!(!is_valid_identifier("a\"b"));
    }

    #[test]
    fn test_quote_identifier() {
        assert_eq!(quote_identifier("users").unwrap(), "\"users\"");
        assert!(quote_identifier("user;DROP").is_err());
    }

    #[test]
    fn test_quote_qualified() {
        assert_eq!(quote_qualified("name").unwrap(), "\"name\"");
        assert_eq!(quote_qualified("u.name").unwrap(), "\"u\".\"name\"");
        assert!(quote_qualified("a.b.c").is_err());
    }
}
