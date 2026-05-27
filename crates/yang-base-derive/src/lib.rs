//! yang-base 派生宏入口。

#![warn(missing_docs)]

use proc_macro::TokenStream;
use proc_macro_error::proc_macro_error;
use syn::{parse_macro_input, DeriveInput};

mod action;
mod table_entity;
mod util;

/// 为 struct 派生 `TypedAction` 实现，自动生成 `ActionMeta` 静态聚合。
///
/// # 必填属性
///
/// - `#[action(name = "...")]`：Action 唯一标识。
///
/// # 可选属性
///
/// - `#[action(display_name = "...")]`：用户可见名称（默认同 `name`）。
/// - `#[action(description = "...")]`：简介（默认空字符串）。
/// - `#[action(public)]`：标记为公开 Action（默认 `false`）。
/// - `#[action(permissions("perm:a", "perm:b"))]`：所需权限列表。
#[proc_macro_error]
#[proc_macro_derive(Action, attributes(action))]
pub fn derive_action(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    action::expand(input).into()
}

/// 为 struct 派生 `TableEntity` 实现，同时生成对应的 `<Name>Field` 和 `<Name>Where` 枚举。
///
/// # 必填属性
///
/// - `#[table(name = "...")]`：数据库表名。
/// - 至少一个字段标注 `#[entity(primary_key)]`。
///
/// # 可选表级属性
///
/// - `#[table(display_name = "...")]`：展示名，默认同表名。
/// - `#[table(soft_delete = "...")]`：软删除字段名（当前版本保留，暂不展开生成）。
///
/// # 可选字段级属性（`#[entity(...)]`）
///
/// - `primary_key`：标记主键字段（必须有且仅有一个）。
/// - `max_length = <n>`：字符串字段最大长度（默认 255）。
/// - `unique`：在 TableConfig 中添加该字段的唯一索引。
/// - `required = true/false`：覆盖"非 Option 默认必填"的推断。
/// - `column = "..."`: 指定列名（默认同字段名）。
/// - `skip`：跳过此字段，不出现在枚举和 TableConfig 中。
#[proc_macro_error]
#[proc_macro_derive(TableEntity, attributes(table, entity))]
pub fn derive_table_entity(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    table_entity::expand(input).into()
}
