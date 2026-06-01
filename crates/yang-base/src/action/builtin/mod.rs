//! 内置 CRUD Actions（H-1 类型化后的版本）
#![cfg(feature = "mysql")]

pub mod add;
pub mod del;
pub mod get;
pub mod put;
pub mod select;
pub mod table;

pub use add::{AddAction, AffectedResult};
pub use del::DelAction;
pub use get::{GetAction, GetByPk};
pub use put::{PutAction, PutInput};
pub use select::{OrderByItem, SelectAction, SelectQuery, SelectResult};
pub use table::{EmptyInput, TableAction, TableSchemaResponse};

#[cfg(test)]
mod __tests__;
