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

/// 一段或两段 SQL 标识符（`column` / `table.column`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum QualifiedIdentifier {
    Unqualified(Identifier),
    Qualified(Identifier, Identifier),
}

impl QualifiedIdentifier {
    pub(crate) fn parse(value: &str) -> Result<Self, DbError> {
        match value.split_once('.') {
            None => Ok(Self::Unqualified(Identifier::parse(value)?)),
            Some((qualifier, name)) => Ok(Self::Qualified(
                Identifier::parse(qualifier)?,
                Identifier::parse(name)?,
            )),
        }
    }

    #[cfg(any(feature = "mysql", feature = "postgres"))]
    pub(crate) fn render(&self, quote: char) -> String {
        match self {
            Self::Unqualified(name) => quote_part(name, quote),
            Self::Qualified(qualifier, name) => {
                format!(
                    "{}.{}",
                    quote_part(qualifier, quote),
                    quote_part(name, quote)
                )
            }
        }
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
            "a.b.c",
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
    }

    #[test]
    fn qualified_identifier_renders_both_dialects() {
        let identifier = match QualifiedIdentifier::parse("users.id") {
            Ok(identifier) => identifier,
            Err(error) => panic!("合法限定标识符不应失败: {error}"),
        };
        assert_eq!(identifier.render('`'), "`users`.`id`");
        assert_eq!(identifier.render('"'), "\"users\".\"id\"");
    }

    proptest! {
        #[test]
        fn accepted_identifiers_always_match_the_strict_grammar(value in any::<String>()) {
            if QualifiedIdentifier::parse(&value).is_ok() {
                let parts: Vec<_> = value.split('.').collect();
                prop_assert!((1..=2).contains(&parts.len()));
                for part in parts {
                    let mut chars = part.chars();
                    prop_assert!(matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_'));
                    prop_assert!(chars.all(|c| c.is_ascii_alphanumeric() || c == '_'));
                }
            }
        }
    }
}
