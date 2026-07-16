//! Schema-first DSL 构建后的内部表配置。

use super::FieldConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 查询与 schema 同步共享的编译后表契约。
#[derive(Debug, Clone)]
pub(crate) struct TableConfig {
    pub(crate) table_name: String,
    pub(crate) display_name: String,
    pub(crate) primary_key: String,
    pub(crate) fields: HashMap<String, FieldConfig>,
    #[cfg_attr(not(any(feature = "mysql", test)), allow(dead_code))]
    pub(crate) unique_indexes: Vec<IndexConfig>,
    #[cfg_attr(not(any(feature = "mysql", test)), allow(dead_code))]
    pub(crate) indexes: Vec<IndexConfig>,
    #[cfg_attr(not(any(feature = "mysql", test)), allow(dead_code))]
    pub(crate) default_order: Vec<(String, SortOrder)>,
    pub(crate) soft_delete_field: Option<String>,
    pub(crate) timestamp_fields: Option<TimestampFields>,
}

impl TableConfig {
    pub(crate) fn get_field(&self, field_name: &str) -> Option<&FieldConfig> {
        self.fields.get(field_name)
    }
}

/// 编译后的索引配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct IndexConfig {
    pub(crate) name: Option<String>,
    pub(crate) fields: Vec<String>,
}

impl IndexConfig {
    pub(crate) fn new(name: Option<String>, fields: Vec<String>) -> Self {
        Self { name, fields }
    }
}

/// 排序方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum SortOrder {
    /// 升序排序。
    Asc,
    /// 降序排序。
    Desc,
}

/// 编译后的自动时间戳字段配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TimestampFields {
    pub(crate) created_at: Option<String>,
    pub(crate) updated_at: Option<String>,
    pub(crate) deleted_at: Option<String>,
}

impl TimestampFields {
    pub(crate) fn new(
        created_at: Option<String>,
        updated_at: Option<String>,
        deleted_at: Option<String>,
    ) -> Self {
        Self {
            created_at,
            updated_at,
            deleted_at,
        }
    }
}
