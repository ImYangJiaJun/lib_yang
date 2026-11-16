//! 逻辑数据源标识。
//!
//! 模块只声明逻辑数据源名（如 `default`、`project_a_pg`），具体后端、URL 与连接池
//! 由部署配置绑定；这样同一模块可以在 MySQL 与 PostgreSQL 之间切换，而不需要改
//! 声明代码。

use crate::error::BaseError;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

/// 逻辑数据源名称。
///
/// 允许小写 ASCII 字母开头，后接小写字母、数字或下划线，长度 1..=32。
/// `default` 是保留名，表示应用的默认数据源。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DatasourceId(Arc<str>);

impl DatasourceId {
    /// 默认数据源的保留名。
    pub const DEFAULT_NAME: &'static str = "default";

    /// 校验并创建逻辑数据源名。
    pub fn new(value: impl Into<String>) -> Result<Self, BaseError> {
        let value = value.into();
        let bytes = value.as_bytes();
        let valid = !bytes.is_empty()
            && bytes.len() <= 32
            && bytes[0].is_ascii_lowercase()
            && bytes
                .iter()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_');
        if !valid {
            return Err(BaseError::ConfigError(format!(
                "非法数据源名称 {value:?}：必须是小写字母开头、仅含小写字母/数字/下划线，长度 1..=32"
            )));
        }
        Ok(Self(Arc::from(value.as_str())))
    }

    /// 返回 `default` 数据源名。
    pub fn default_source() -> Self {
        Self(Arc::from(Self::DEFAULT_NAME))
    }

    /// 是否为默认数据源。
    pub fn is_default(&self) -> bool {
        self.as_str() == Self::DEFAULT_NAME
    }

    /// 返回名称字符串。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DatasourceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Default for DatasourceId {
    fn default() -> Self {
        Self::default_source()
    }
}

impl FromStr for DatasourceId {
    type Err = BaseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_lowercase_names_and_default() {
        for name in ["default", "pg_main", "project_a_pg", "a1"] {
            DatasourceId::new(name).expect("合法数据源名应通过");
        }
        assert!(DatasourceId::default_source().is_default());
    }

    #[test]
    fn rejects_invalid_names() {
        for name in ["", "Default", "1pg", "pg-main", "pg main", "数据源"] {
            assert!(
                DatasourceId::new(name).is_err(),
                "非法数据源名 {name:?} 应被拒绝"
            );
        }
        assert!(DatasourceId::new("a".repeat(33)).is_err());
    }
}
