//! Schema-first DSL 构建后的内部表配置。

use super::FieldConfig;
use crate::datasource::DatasourceId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 查询与 schema 同步共享的编译后表契约。
#[derive(Debug, Clone)]
pub(crate) struct TableConfig {
    pub(crate) table_name: String,
    /// 表所属逻辑数据源；默认 `default`。
    pub(crate) datasource: DatasourceId,
    #[cfg_attr(not(feature = "mysql"), allow(dead_code))]
    pub(crate) table_ref: yang_db::TableRef,
    pub(crate) display_name: String,
    /// 兼容字段：单主键时等于唯一主键；复合主键时等于首个主键。
    pub(crate) primary_key: String,
    /// 完整主键列（按声明顺序）；空表示无主键。
    pub(crate) primary_keys: Vec<String>,
    pub(crate) fields: HashMap<String, FieldConfig>,
    /// 框架不建模、不读写、也不改写的存量列（仅 PostgreSQL）。
    ///
    /// 只用于校验列确实存在且类型符合声明；运行期 CRUD/UI 完全忽略。
    #[cfg_attr(not(any(feature = "mysql", feature = "postgres")), allow(dead_code))]
    pub(crate) opaque_columns: Vec<OpaqueColumnConfig>,
    #[cfg_attr(not(feature = "mysql"), allow(dead_code))]
    pub(crate) field_refs: HashMap<String, yang_db::FieldRef>,
    #[cfg_attr(not(any(feature = "mysql", test)), allow(dead_code))]
    pub(crate) unique_indexes: Vec<IndexConfig>,
    #[cfg_attr(not(any(feature = "mysql", test)), allow(dead_code))]
    pub(crate) indexes: Vec<IndexConfig>,
    #[cfg_attr(not(feature = "mysql"), allow(dead_code))]
    pub(crate) checks: Vec<CheckConfig>,
    #[cfg_attr(not(feature = "mysql"), allow(dead_code))]
    pub(crate) foreign_keys: Vec<ForeignKeyConfig>,
    #[cfg_attr(not(any(feature = "mysql", test)), allow(dead_code))]
    pub(crate) default_order: Vec<(String, SortOrder)>,
    pub(crate) soft_delete_field: Option<String>,
    pub(crate) timestamp_fields: Option<TimestampFields>,
}

/// 编译后的 CHECK 约束。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckConfig {
    pub(crate) name: String,
    pub(crate) expression: String,
}

/// 编译后的外键约束；删除和更新规则固定为 RESTRICT。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ForeignKeyConfig {
    pub(crate) name: String,
    pub(crate) columns: Vec<String>,
    pub(crate) referenced_table: String,
    pub(crate) referenced_columns: Vec<String>,
}

impl TableConfig {
    pub(crate) fn get_field(&self, field_name: &str) -> Option<&FieldConfig> {
        self.fields.get(field_name)
    }

    #[cfg(feature = "mysql")]
    pub(crate) fn get_field_ref(&self, field_name: &str) -> Option<&yang_db::FieldRef> {
        self.field_refs.get(field_name)
    }
}

/// 框架不建模的存量列声明。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpaqueColumnConfig {
    pub(crate) name: String,
    /// 与 PostgreSQL `format_type` 输出比较的受信类型文本（大小写/空白不敏感）。
    pub(crate) sql_type: String,
}

/// 编译后的索引配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct IndexConfig {
    pub(crate) name: Option<String>,
    pub(crate) fields: Vec<String>,
    /// 表达式键（已校验的受信 SQL）；与 `fields` 至少有一个非空。
    pub(crate) expressions: Vec<String>,
    /// PostgreSQL 部分索引谓词（`WHERE <predicate>`）。
    pub(crate) predicate: Option<String>,
    /// PostgreSQL 15+ 唯一索引的 `NULLS NOT DISTINCT` 语义；MySQL 明确不支持。
    pub(crate) nulls_not_distinct: bool,
}

impl IndexConfig {
    pub(crate) fn new(name: Option<String>, fields: Vec<String>, nulls_not_distinct: bool) -> Self {
        Self {
            name,
            fields,
            expressions: Vec::new(),
            predicate: None,
            nulls_not_distinct,
        }
    }

    pub(crate) fn with_expressions(mut self, expressions: Vec<String>) -> Self {
        self.expressions = expressions;
        self
    }

    pub(crate) fn with_predicate(mut self, predicate: Option<String>) -> Self {
        self.predicate = predicate;
        self
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
