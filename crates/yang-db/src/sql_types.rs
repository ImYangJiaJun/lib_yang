//! 方言无关的 SQL 语义类型。
//!
//! 这些类型只在 crate 内部流转，用类型区分受校验标识符、限定标识符和调用方明确
//! 信任的 SQL 表达式，避免一个 `String` 同时承担三种安全语义。

use crate::DbError;

/// 单段 SQL 标识符。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Identifier(String);

impl Identifier {
    /// 解析严格 ASCII 标识符 `[A-Za-z_][A-Za-z0-9_]*`。
    pub(crate) fn parse(value: &str) -> Result<Self, DbError> {
        let mut chars = value.chars();
        match chars.next() {
            Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
            _ => return Err(invalid_identifier(value)),
        }
        if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(invalid_identifier(value));
        }
        Ok(Self(value.to_string()))
    }

    #[cfg(any(feature = "mysql", feature = "postgres"))]
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// PostgreSQL 标识符的最大字节数（`NAMEDATALEN - 1` = 63）。
#[cfg(feature = "postgres")]
pub(crate) const POSTGRES_MAX_IDENTIFIER_BYTES: usize = 63;

#[cfg(feature = "postgres")]
impl Identifier {
    /// 按 PostgreSQL 规则解析单段标识符。
    ///
    /// 规则（输出始终以双引号包裹，因此大小写保持原样）：
    ///
    /// - 首字符必须是 Unicode 字母或 `_`；
    /// - 其余字符可以是 Unicode 字母/数字、`_`、`$`（`$` 不允许出现在首字符）；
    /// - 允许内嵌双引号 `"`，渲染时按 SQL 标准双写成 `""`；
    /// - 禁止 NUL 与控制字符（fail-closed）；
    /// - 长度上限 [`POSTGRES_MAX_IDENTIFIER_BYTES`] 字节（UTF-8 字节数，不是字符数）。
    ///
    /// **超长直接拒绝，不做截断。** PostgreSQL 会把超长标识符静默截断到 63 字节
    ///（加不加引号都一样）并只发一条 NOTICE，两个不同名字可能因此折叠为重名，
    /// 这与 checked identifier 入口的 fail-closed 契约冲突。
    pub(crate) fn parse_postgres(value: &str) -> Result<Self, DbError> {
        if value.len() > POSTGRES_MAX_IDENTIFIER_BYTES {
            return Err(invalid_identifier(value));
        }
        let mut chars = value.chars();
        match chars.next() {
            Some(c) if c.is_alphabetic() || c == '_' => {}
            _ => return Err(invalid_identifier(value)),
        }
        if !chars.all(is_postgres_identifier_continuation) {
            return Err(invalid_identifier(value));
        }
        Ok(Self(value.to_string()))
    }
}

/// PostgreSQL 标识符的非首字符判定：Unicode 字母/数字、`_`、`$` 或内嵌双引号。
///
/// NUL 与控制字符一律拒绝（`char::is_control` 覆盖 U+0000..U+001F 与
/// U+007F..U+009F）。
#[cfg(feature = "postgres")]
fn is_postgres_identifier_continuation(c: char) -> bool {
    !c.is_control() && (c.is_alphanumeric() || matches!(c, '_' | '$' | '"'))
}

/// 一到三段 SQL 标识符（`column` / `table.column` / `schema.table.column`）。
///
/// 段数上限由 [`QualifiedIdentifier::parse_with_max_parts`] 显式控制：表名最多
/// `schema.table` 两段，字段名最多 `schema.table.column` 三段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QualifiedIdentifier(Vec<Identifier>);

impl QualifiedIdentifier {
    /// 按字段名口径解析（最多三段）。
    pub(crate) fn parse(value: &str) -> Result<Self, DbError> {
        Self::parse_with_max_parts(value, 3)
    }

    /// 按给定段数上限解析：每段都必须通过严格 ASCII 标识符校验。
    pub(crate) fn parse_with_max_parts(value: &str, max_parts: usize) -> Result<Self, DbError> {
        let parts = value
            .split('.')
            .map(Identifier::parse)
            .collect::<Result<Vec<_>, _>>()?;
        if parts.is_empty() || parts.len() > max_parts {
            return Err(invalid_identifier(value));
        }
        Ok(Self(parts))
    }

    #[cfg(any(feature = "mysql", feature = "postgres"))]
    pub(crate) fn render(&self, quote: char) -> String {
        self.0
            .iter()
            .map(|identifier| quote_part(identifier, quote))
            .collect::<Vec<_>>()
            .join(".")
    }
}

#[cfg(feature = "postgres")]
impl QualifiedIdentifier {
    /// 按 PostgreSQL 规则解析字段名（最多三段）。
    pub(crate) fn parse_postgres(value: &str) -> Result<Self, DbError> {
        Self::parse_postgres_with_max_parts(value, 3)
    }

    /// 按 PostgreSQL 规则解析限定标识符，逐段校验（段数上限由调用方给定）。
    ///
    /// 每段都走 [`Identifier::parse_postgres`]，因此 63 字节上限、Unicode 字母、
    /// `$` 与内嵌双引号规则对每一段独立生效。
    pub(crate) fn parse_postgres_with_max_parts(
        value: &str,
        max_parts: usize,
    ) -> Result<Self, DbError> {
        let parts = value
            .split('.')
            .map(Identifier::parse_postgres)
            .collect::<Result<Vec<_>, _>>()?;
        if parts.is_empty() || parts.len() > max_parts {
            return Err(invalid_identifier(value));
        }
        Ok(Self(parts))
    }

    /// 以 PostgreSQL 双引号渲染，内嵌双引号按 SQL 标准双写转义。
    pub(crate) fn render_postgres(&self) -> String {
        self.0
            .iter()
            .map(|identifier| format!("\"{}\"", identifier.as_str().replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(".")
    }
}

/// 一次完整且已校验的条件渲染结果。
#[cfg(any(feature = "mysql", feature = "postgres"))]
#[derive(Debug)]
pub(crate) struct RenderedCondition<T> {
    pub(crate) sql: String,
    pub(crate) params: Vec<T>,
}

#[cfg(any(feature = "mysql", feature = "postgres"))]
fn quote_part(identifier: &Identifier, quote: char) -> String {
    format!("{quote}{}{quote}", identifier.as_str())
}

fn invalid_identifier(value: &str) -> DbError {
    DbError::InvalidArgument(format!("非法 SQL 标识符: {value:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn identifier_types_reject_adversarial_boundaries() {
        for invalid in [
            "",
            ".id",
            "users.",
            "a.b.c.d",
            "users.id--",
            "用户",
            "a\0b",
            "a b",
            "a\"b",
            "a`b",
        ] {
            assert!(QualifiedIdentifier::parse(invalid).is_err(), "{invalid:?}");
        }

        assert!(Identifier::parse("users_1").is_ok());
        assert!(QualifiedIdentifier::parse("users.id").is_ok());
        assert!(QualifiedIdentifier::parse("public.users.id").is_ok());
        // 表名口径只接受 schema.table 两段。
        assert!(QualifiedIdentifier::parse_with_max_parts("public.users", 2).is_ok());
        assert!(QualifiedIdentifier::parse_with_max_parts("a.b.c", 2).is_err());
    }

    #[cfg(any(feature = "mysql", feature = "postgres"))]
    #[test]
    fn qualified_identifier_renders_both_dialects() {
        let identifier = match QualifiedIdentifier::parse("users.id") {
            Ok(identifier) => identifier,
            Err(error) => panic!("合法限定标识符不应失败: {error}"),
        };
        assert_eq!(identifier.render('`'), "`users`.`id`");
        assert_eq!(identifier.render('"'), "\"users\".\"id\"");
    }

    /// P1-28：PostgreSQL 标识符允许 Unicode 字母、非首字符 `$`、内嵌双引号，
    /// 并执行 63 字节上限（超长拒绝而非截断）。
    #[cfg(feature = "postgres")]
    #[test]
    fn postgres_identifiers_accept_unicode_dollar_and_embedded_quotes() {
        assert!(Identifier::parse_postgres("用户").is_ok());
        assert!(Identifier::parse_postgres("名前_1").is_ok());
        assert!(Identifier::parse_postgres("a$b").is_ok());
        assert!(Identifier::parse_postgres("col$").is_ok());
        assert!(Identifier::parse_postgres("a\"b").is_ok());
        assert!(Identifier::parse_postgres("_x$1").is_ok());

        // 首字符不能是 `$`、数字、引号或空格。
        for invalid in ["$leading", "1abc", "\"lead", " a"] {
            assert!(
                Identifier::parse_postgres(invalid).is_err(),
                "{invalid:?} 应被拒绝"
            );
        }
        // NUL 与控制字符 fail-closed。
        for invalid in ["a\0b", "a\nb", "a\tb", "\u{7f}"] {
            assert!(
                Identifier::parse_postgres(invalid).is_err(),
                "{invalid:?} 应被拒绝"
            );
        }
        // 其它标点仍拒绝（只放开任务要求的类别）。
        for invalid in ["a;b", "a b", "a`b", "a'b", "a-b"] {
            assert!(
                Identifier::parse_postgres(invalid).is_err(),
                "{invalid:?} 应被拒绝"
            );
        }
    }

    #[cfg(feature = "postgres")]
    #[test]
    fn postgres_identifier_63_byte_limit_is_rejected_not_truncated() {
        // 63 字节可接受，64 字节拒绝。
        assert!(Identifier::parse_postgres(&"a".repeat(63)).is_ok());
        assert!(Identifier::parse_postgres(&"a".repeat(64)).is_err());
        // 中文按 UTF-8 3 字节计：21 字 = 63 字节可接受，22 字 = 66 字节拒绝。
        assert!(Identifier::parse_postgres(&"用".repeat(21)).is_ok());
        assert!(Identifier::parse_postgres(&"用".repeat(22)).is_err());
    }

    #[cfg(feature = "postgres")]
    #[test]
    #[allow(clippy::expect_used)]
    fn postgres_identifiers_render_with_doubled_quotes() {
        let unicode =
            QualifiedIdentifier::parse_postgres("模式.用户").expect("Unicode 限定标识符应合法");
        assert_eq!(unicode.render_postgres(), "\"模式\".\"用户\"");

        let quoted = QualifiedIdentifier::parse_postgres("a\"b").expect("内嵌双引号应合法");
        assert_eq!(quoted.render_postgres(), "\"a\"\"b\"");

        let qualified_quoted =
            QualifiedIdentifier::parse_postgres("t.a\"b").expect("限定名每段都允许内嵌双引号");
        assert_eq!(qualified_quoted.render_postgres(), "\"t\".\"a\"\"b\"");

        assert!(QualifiedIdentifier::parse_postgres("a.b.c.d").is_err());
        assert!(QualifiedIdentifier::parse_postgres_with_max_parts("a.b", 1).is_err());
        assert!(QualifiedIdentifier::parse_postgres_with_max_parts("a.b", 2).is_ok());
    }

    proptest! {
        #[test]
        fn accepted_identifiers_always_match_the_strict_grammar(value in any::<String>()) {
            if QualifiedIdentifier::parse(&value).is_ok() {
                let parts: Vec<_> = value.split('.').collect();
                prop_assert!(parts.len() <= 3);
                for part in parts {
                    let mut chars = part.chars();
                    prop_assert!(matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_'));
                    prop_assert!(chars.all(|c| c.is_ascii_alphanumeric() || c == '_'));
                }
            }
        }
    }
}
