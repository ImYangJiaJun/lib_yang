//! Redis key 命名空间。
//!
//! 统一生成 `app:env[:version]:key` 形式的 key，避免多应用/多环境共用同一个
//! Redis 时发生键冲突。段名只允许 ASCII 字母数字与 `-_.`，防止冒号注入。

use crate::{DbError, Result};

/// 结构化 key 命名空间。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyNamespace {
    prefix: String,
}

impl KeyNamespace {
    /// 创建 `app:env:` 命名空间。
    pub fn new(app: impl Into<String>, env: impl Into<String>) -> Result<Self> {
        let app = app.into();
        let env = env.into();
        validate_segment("app", &app)?;
        validate_segment("env", &env)?;
        Ok(Self {
            prefix: format!("{app}:{env}:"),
        })
    }

    /// 创建带版本段的 `app:env:version:` 命名空间。
    pub fn new_with_version(
        app: impl Into<String>,
        env: impl Into<String>,
        version: impl Into<String>,
    ) -> Result<Self> {
        Self::new(app, env)?.with_version(version)
    }

    /// 追加版本段（如 `v2`）。
    pub fn with_version(mut self, version: impl Into<String>) -> Result<Self> {
        let version = version.into();
        validate_segment("version", &version)?;
        self.prefix.push_str(&version);
        self.prefix.push(':');
        Ok(self)
    }

    /// 命名空间前缀（含结尾冒号）。
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// 生成带命名空间的二进制 key。
    pub fn key(&self, key: impl AsRef<[u8]>) -> Vec<u8> {
        let key = key.as_ref();
        let mut out = Vec::with_capacity(self.prefix.len() + key.len());
        out.extend_from_slice(self.prefix.as_bytes());
        out.extend_from_slice(key);
        out
    }

    /// 生成带命名空间的字符串 key。
    pub fn key_str(&self, key: impl AsRef<str>) -> String {
        format!("{}{}", self.prefix, key.as_ref())
    }
}

fn validate_segment(label: &str, value: &str) -> Result<()> {
    if value.is_empty() {
        return Err(DbError::InvalidArgument(format!(
            "Redis namespace {label} 不能为空"
        )));
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        return Err(DbError::InvalidArgument(format!(
            "Redis namespace {label} 只允许 ASCII 字母数字与 -_."
        )));
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn builds_expected_prefixes() {
        let namespace = KeyNamespace::new("app", "prod")
            .expect("合法命名空间")
            .with_version("v2")
            .expect("合法版本");
        assert_eq!(namespace.prefix(), "app:prod:v2:");
        assert_eq!(namespace.key_str("user:1"), "app:prod:v2:user:1");
        assert_eq!(namespace.key(b"bin\xff"), b"app:prod:v2:bin\xff");
    }

    #[test]
    fn rejects_empty_and_unsafe_segments() {
        for result in [
            KeyNamespace::new("", "prod"),
            KeyNamespace::new("app", ""),
            KeyNamespace::new("app:evil", "prod"),
            KeyNamespace::new("app", "pro d"),
            KeyNamespace::new("app", "prod").and_then(|ns| ns.with_version("v:2")),
        ] {
            assert!(matches!(result, Err(DbError::InvalidArgument(_))));
        }
    }
}
