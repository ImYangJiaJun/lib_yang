//! yang-base Action 派生宏入口。

#![warn(missing_docs)]

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod action;
mod params;

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
#[proc_macro_derive(Action, attributes(action))]
pub fn derive_action(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    action::expand(input).into()
}

/// 一次声明强类型输入结构与其唯一原生 Params 定义。
#[proc_macro]
pub fn params(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as params::ParamsInput);
    params::expand(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
