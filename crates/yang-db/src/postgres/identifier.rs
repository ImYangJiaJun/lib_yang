//! SQL 标识符校验与转义（PostgreSQL 方言）。
//!
//! 规则以 PostgreSQL 为准（P1-28），与 MySQL 入口不同构：
//!
//! - 首字符：Unicode 字母或 `_`；其余字符：Unicode 字母/数字、`_`、`$`
//!   （`$` 不允许出现在首字符，与 PostgreSQL 的未加引号标识符一致）；
//! - 允许内嵌双引号 `"`，输出时按 SQL 标准双写为 `""`；
//! - 禁止 NUL 与控制字符（fail-closed）；
//! - 长度上限 63 字节（UTF-8 字节数），**超长直接拒绝而非截断**，原因见
//!   `crate::sql_types` 中 `Identifier::parse_postgres` 的说明。
//!
//! 所有入口都先校验再渲染，调用方永远拿不到未经检查的拼接结果。MySQL 的
//! 标识符入口不受影响，仍走严格 ASCII 文法。

use crate::dialect;
use crate::error::DbError;
use crate::sql_types::{Identifier, QualifiedIdentifier, POSTGRES_MAX_IDENTIFIER_BYTES};

/// 校验是否为合法的 PostgreSQL 标识符。
///
/// 本 API 始终以双引号包裹标识符，因此内嵌双引号也被视为合法（渲染时双写）；
/// 判定口径与 [`quote_identifier`] 完全一致。
pub fn is_valid_identifier(s: &str) -> bool {
    // 63 字节上限必须先行判定：共享的严格文法不检查长度。
    if s.len() > POSTGRES_MAX_IDENTIFIER_BYTES {
        return false;
    }
    // 常见 ASCII 标识符走共享的零分配快路径；扩展规则（Unicode 字母、`$`、
    // 内嵌双引号）回退到 PostgreSQL 解析器。
    dialect::is_valid_identifier(s) || Identifier::parse_postgres(s).is_ok()
}

/// 校验并用双引号转义一个标识符（PostgreSQL 方言）。
///
/// 合法标识符返回 `"ident"`；含内嵌双引号时双写为 `""`；含控制字符、
/// 首字符为 `$`/数字或超过 63 字节时返回 [`DbError::InvalidArgument`]。
pub fn quote_identifier(ident: &str) -> Result<String, DbError> {
    Ok(QualifiedIdentifier::parse_postgres_with_max_parts(ident, 1)?.render_postgres())
}

/// 校验并转义可能带限定前缀的标识符：`name` → `"name"`，`users.name` → `"users"."name"`，
/// `schema.users.name` → `"schema"."users"."name"`（字段名最多三段）。
pub fn quote_qualified(ident: &str) -> Result<String, DbError> {
    Ok(QualifiedIdentifier::parse_postgres(ident)?.render_postgres())
}

/// 校验并转义表名：只接受 `table` 或 `schema.table`（最多两段）。
pub fn quote_table(ident: &str) -> Result<String, DbError> {
    // PostgreSQL 规则是权威校验（含每段 63 字节上限）；严格 ASCII 的常见形态
    // 复用共享渲染路径，输出与 PG 渲染完全一致。
    let parsed = QualifiedIdentifier::parse_postgres_with_max_parts(ident, 2)?;
    match dialect::quote_table(dialect::POSTGRES, ident) {
        Ok(quoted) => Ok(quoted),
        Err(_) => Ok(parsed.render_postgres()),
    }
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
        assert!(!is_valid_identifier("a`b"));
        assert!(!is_valid_identifier("a b"));
    }

    /// P1-28：Unicode 字母、非首字符 `$`、内嵌双引号、NUL/控制字符。
    #[test]
    fn postgres_identifier_rules() {
        assert!(is_valid_identifier("用户"));
        assert!(is_valid_identifier("a$b"));
        assert!(!is_valid_identifier("$a"));
        // 内嵌双引号现在合法（渲染时双写转义）。
        assert!(is_valid_identifier("a\"b"));
        assert!(!is_valid_identifier("a\0b"));
        assert!(!is_valid_identifier("a\nb"));
        assert!(!is_valid_identifier("\u{7f}"));
    }

    /// P1-28：63 字节上限按 UTF-8 字节计，超长拒绝而不是静默截断。
    #[test]
    fn postgres_identifier_63_byte_limit() {
        assert!(is_valid_identifier(&"a".repeat(63)));
        assert!(!is_valid_identifier(&"a".repeat(64)));
        assert!(is_valid_identifier(&"用".repeat(21)));
        assert!(!is_valid_identifier(&"用".repeat(22)));
    }

    #[test]
    fn test_quote_identifier() {
        assert_eq!(quote_identifier("users").unwrap(), "\"users\"");
        assert!(quote_identifier("user;DROP").is_err());
        assert!(quote_identifier("$a").is_err());
        assert!(quote_identifier("a\0b").is_err());
        assert!(quote_identifier(&"a".repeat(64)).is_err());
    }

    #[test]
    fn quote_postgres_unicode_and_embedded_quotes() {
        assert_eq!(quote_identifier("用户").unwrap(), "\"用户\"");
        assert_eq!(quote_identifier("a$b").unwrap(), "\"a$b\"");
        assert_eq!(quote_identifier("a\"b").unwrap(), "\"a\"\"b\"");
        assert_eq!(quote_qualified("模式.用户").unwrap(), "\"模式\".\"用户\"");
        assert_eq!(
            quote_qualified("audit.u.a\"b").unwrap(),
            "\"audit\".\"u\".\"a\"\"b\""
        );
        assert_eq!(quote_table("审计.事件").unwrap(), "\"审计\".\"事件\"");
    }

    #[test]
    fn test_quote_qualified() {
        assert_eq!(quote_qualified("name").unwrap(), "\"name\"");
        assert_eq!(quote_qualified("u.name").unwrap(), "\"u\".\"name\"");
        assert_eq!(
            quote_qualified("audit.u.name").unwrap(),
            "\"audit\".\"u\".\"name\""
        );
        assert!(quote_qualified("a.b.c.d").is_err());
    }

    #[test]
    fn test_quote_table() {
        assert_eq!(quote_table("users").unwrap(), "\"users\"");
        assert_eq!(quote_table("audit.users").unwrap(), "\"audit\".\"users\"");
        assert!(quote_table("a.b.c").is_err());
        assert!(quote_table("users;DROP").is_err());
    }
}
