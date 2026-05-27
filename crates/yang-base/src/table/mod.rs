//! 表配置系统
//!
//! 提供数据表的配置、字段定义、验证和查询构建功能。
//!
//! # 模块
//!
//! - `field_type`：字段类型定义
//! - `validator`：字段验证器
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

pub mod entity;
mod dynamic_row;
mod field_config;
mod field_type;
mod query_params;
mod table_config;
mod table_query;
mod validator;

#[cfg(test)]
mod __tests__;

pub use dynamic_row::DynamicRow;
pub use entity::{
    AsColumnName, IntoSqlCondition, SqlCondition, SqlOp,
    WhereOp, StringWhereOp,
};
#[cfg(feature = "mysql")]
pub use entity::TableEntity;
pub use field_config::{FieldConfig, FieldPermissions, RelationConfig, RelationType};
pub use field_type::FieldType;
pub use query_params::{PaginatedResult, QueryParams, WhereCondition};
pub use table_config::{IndexConfig, SortOrder, TableConfig, TimestampFields};
pub use table_query::TableQuery;
pub use validator::{Validator, ValidatorFn};
