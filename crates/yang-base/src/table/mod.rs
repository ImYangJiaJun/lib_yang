//! 表配置系统
//!
//! 提供数据表的配置、字段定义、验证和查询构建功能。
//!
//! # 模块
//!
//! 本模块公开重导出以下核心类型：
//!
//! - 字段类型与校验：[`FieldType`]、[`Validator`]（及自定义校验函数类型 [`ValidatorFn`]）
//! - 字段元数据：[`FieldConfig`]、[`FieldPermissions`]、[`RelationConfig`]（及 [`RelationType`]）
//! - 表结构：[`TableConfig`]、[`IndexConfig`]、[`SortOrder`]、[`TimestampFields`]
//! - 查询请求模型：[`QueryParams`]、[`PaginatedResult`]、[`WhereCondition`]
//! - 查询执行：[`TableQuery`]
//! - 动态行：[`DynamicRow`]
//! - 可选数据库列兼容验证：[`SchemaColumn`]、[`SchemaValidationReport`]
//! - 类型化实体（受 `mysql` feature gate）：`TableEntity`、[`SqlCondition`]、[`SqlOp`]、
//!   通用/字符串 where 操作符 [`WhereOp`]、[`StringWhereOp`] 等
//!
//! 其中 `TableEntity` 依赖 sqlx 的 `FromRow`，仅在启用 `mysql` feature 时可用。
//! `TableConfig` 是运行期访问、校验与权限契约，不是数据库 DDL 的唯一真相；
//! schema 验证只报告声明字段的缺失/类型/NULL 兼容问题，不生成 ALTER。
//!
//! # 示例
//!
//! ```rust
//! use yang_base::table::{FieldType, Validator};
//! use serde_json::json;
//!
//! // 创建字符串类型字段
//! let name_field = FieldType::String { max_length: 50 };
//!
//! // 创建整数类型字段
//! let age_field = FieldType::Integer;
//!
//! // 创建枚举类型字段
//! let status_field = FieldType::Enum {
//!     values: vec!["active".to_string(), "inactive".to_string()],
//! };
//!
//! // 创建验证器
//! let min_length_validator = Validator::MinLength(5);
//! let email_validator = Validator::Email;
//!
//! // 验证字段值
//! assert!(min_length_validator.validate("username", &json!("alice")).is_ok());
//! assert!(email_validator.validate("email", &json!("user@example.com")).is_ok());
//! ```

mod dynamic_row;
mod entity;
mod field_config;
mod field_type;
mod query_params;
mod schema_validation;
mod table_config;
mod table_query;
mod validator;

#[cfg(test)]
mod __tests__;

pub use dynamic_row::DynamicRow;
#[cfg(feature = "mysql")]
pub use entity::TableEntity;
pub use entity::{
    AsColumnName, Filter, IntoSqlCondition, SqlCondition, SqlOp, StringWhereOp, WhereOp,
};
pub use field_config::{FieldConfig, FieldPermissions, RelationConfig, RelationType};
pub use field_type::FieldType;
pub use query_params::{PaginatedResult, QueryParams, WhereCondition, MAX_QUERY_PAGE_SIZE};
pub use schema_validation::{SchemaColumn, SchemaIssue, SchemaIssueKind, SchemaValidationReport};
pub use table_config::{IndexConfig, SortOrder, TableConfig, TimestampFields};
pub use table_query::{TableQuery, MAX_TABLE_QUERY_PAGE_SIZE};
pub use validator::{Validator, ValidatorFn};
