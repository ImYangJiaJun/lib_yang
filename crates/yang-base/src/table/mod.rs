//! Schema-first 表系统
//!
//! 提供数据表的配置、字段定义、验证和查询构建功能。
//!
//! # 模块
//!
//! 本模块公开重导出以下核心类型：
//!
//! - 字段类型与校验：[`FieldType`]、[`Validator`]（及自定义校验函数类型 [`ValidatorFn`]）
//! - Schema-first 定义：[`Table`]、[`Field`]、[`TableDefinition`]
//! - 查询请求模型：[`QueryParams`]、[`PaginatedResult`]、[`WhereCondition`]
//! - 查询执行：[`TableQuery`]
//! - 动态记录：[`Record`]
//! - 可选数据库列兼容验证：[`SchemaColumn`]、[`SchemaValidationReport`]
//!
//! [`TableDefinition`] 是运行期访问、校验与权限契约，也是可选 additive schema
//! 同步的声明来源；同步器只创建缺失表/列/主键/索引，绝不删除或改写已有结构。
//!
//! # 示例
//!
//! ```rust
//! use yang_base::table::{Field, Record, Table};
//!
//! # fn main() -> yang_base::Result<()> {
//! let users = Table::new("users")
//!     .label("用户")
//!     .fields(vec![
//!         Field::id("id"),
//!         Field::string("username", 64).required().unique(),
//!         Field::string("password_hash", 255)
//!             .required()
//!             .secret()
//!             .readable_by(["system"])
//!             .writable_by(["system"]),
//!         Field::created_at("created_at"),
//!         Field::updated_at("updated_at"),
//!     ])
//!     .build()?;
//!
//! let input = Record::new().set("username", "alice");
//! assert_eq!(users.name(), "users");
//! assert_eq!(input.get("username").and_then(|value| value.as_str()), Some("alice"));
//! # Ok(())
//! # }
//! ```

mod definition;
mod field_config;
mod field_type;
mod query_params;
mod record;
mod schema_validation;
mod table_config;
mod table_query;
mod validator;

#[cfg(test)]
mod __tests__;

#[cfg(feature = "mysql")]
pub use definition::TableHandle;
pub use definition::{col, ColumnName, Field, FieldMetadata, Order, Table, TableDefinition};
pub(crate) use field_config::FieldConfig;
pub use field_config::RelationType;
pub use field_type::FieldType;
pub use query_params::{PaginatedResult, QueryParams, WhereCondition, MAX_QUERY_PAGE_SIZE};
pub use record::Record;
pub use schema_validation::{SchemaColumn, SchemaIssue, SchemaIssueKind, SchemaValidationReport};
pub use table_config::SortOrder;
pub(crate) use table_config::TableConfig;
pub use table_query::{TableQuery, MAX_TABLE_QUERY_PAGE_SIZE};
pub use validator::{Validator, ValidatorFn};
