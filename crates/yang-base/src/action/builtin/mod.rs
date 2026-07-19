//! 基于运行时表定义与 [`crate::table::Record`] 的内置 CRUD Actions。
#![cfg(feature = "mysql")]

pub mod add;
mod catalog_schema;
pub mod del;
pub mod get;
pub mod put;
pub mod relation_options;
pub mod select;
pub mod table;

pub use add::{AddAction, AffectedResult};
pub use del::DelAction;
pub use get::{GetAction, GetByPk};
pub use put::{PutAction, PutInput};
pub use relation_options::RelationOptionsAction;
pub use select::{OrderByItem, SelectAction, SelectQuery, SelectResult};
pub use table::{EmptyInput, TableAction, TableSchemaResponse};

pub(crate) use catalog_schema::{crud_contracts, BuiltinActionContract};

#[cfg(test)]
mod __tests__;
