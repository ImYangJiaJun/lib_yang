//! 面向前端运行时的版本化 UI 契约。
//!
//! 本模块只定义声明式数据，不包含组件路径、脚本或权限判定。请求级权限过滤由
//! 上层 projector 在构造 [`UiCatalog`] 前完成，避免把未授权 Action 暴露给前端。
//!
//! # 租户维度决策
//!
//! 目录投影当前不包含租户维度，这是有意决策：同一身份在不同租户下看到相同的
//! 契约结构，租户隔离由数据层（`tenant_key` 字段、租户中间件与 TableQuery 数据
//! 范围）独立强制。如此可避免目录 revision 随租户组合爆炸，也避免租户存在性经
//! 契约差异泄漏。后续若确需按租户裁剪视图，接入点为请求级投影入口
//! [`BuiltApp::ui_catalog`](crate::definition::BuiltApp::ui_catalog)（其实现位于
//! `definition/builder.rs` 的 registry 投影），在那里按请求上下文裁剪即可，
//! 本模块的契约类型无需变更。

mod action;
mod catalog;
mod demo;
mod hints;
mod module;
mod table;

pub use action::{
    ActionConfirmation, ActionPresentationSchema, ActionPresentationSpec, AvailabilityHint,
};
pub use catalog::UiCatalog;
pub use demo::{ActionDemoParamSchema, ActionDemoSchema, UiParamSource};
pub use hints::{
    ActionInteraction, ActionPlacement, ActionResponseKind, AvailabilityState, SortDirection,
    WidgetHint,
};
pub use module::{
    AccountIdentitySchema, AccountIdentitySpec, ModulePresentationSchema, ModulePresentationSpec,
};
pub use table::{
    FormFieldSchema, FormFieldValidationSchema, FormSchema, RelationOptionsSchema,
    TableColumnSchema, TableQuerySchema, TableSortSchema, TableSortSpec, TableViewSchema,
    TreeViewSchema,
};

/// 当前 UI 契约版本。
pub const UI_SCHEMA_VERSION: &str = "2.3";

#[cfg(test)]
mod __tests__;
