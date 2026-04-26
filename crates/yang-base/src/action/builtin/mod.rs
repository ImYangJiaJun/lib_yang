//! 内置 CRUD Actions 模块
//!
//! 提供标准的 CRUD 操作 Actions，包括新增、更新、删除、查询等。
//!
//! # 主要组件
//!
//! - `AddAction`：新增数据 Action
//! - `PutAction`：更新数据 Action
//! - `DelAction`：删除数据 Action
//! - `GetAction`：获取单条数据 Action
//! - `SelectAction`：查询列表 Action
//! - `TableAction`：获取表元数据 Action
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::action::builtin::{AddAction, GetAction, SelectAction};
//! use yang_base::table::TableConfig;
//! use std::sync::Arc;
//!
//! // 创建表配置
//! let table_config = Arc::new(TableConfig::new("users"));
//!
//! // 创建内置 Actions
//! let add_action = AddAction::new(table_config.clone());
//! let get_action = GetAction::new(table_config.clone());
//! let select_action = SelectAction::new(table_config.clone());
//! ```

mod add;
mod del;
mod get;
mod put;
mod select;
mod table;

pub use add::AddAction;
pub use del::DelAction;
pub use get::GetAction;
pub use put::PutAction;
pub use select::SelectAction;
pub use table::TableAction;

#[cfg(test)]
mod __tests__;
