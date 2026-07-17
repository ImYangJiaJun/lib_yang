//! YANG 原生定义内核。
//!
//! 业务侧的 Addon、Module、Action、Fields、Params 与 View 最终都构建为本模块的
//! 不可变定义。名称和引用只在构建期解析；请求运行时使用预解析的 slot，不再按
//! 字符串遍历 Addon/Module/Action。

mod builder;
mod error;
mod field;
mod interface;
mod name;
#[cfg(feature = "openapi")]
mod openapi;
mod param;
mod plugins;
mod spec;
mod ui;
mod view;

pub use builder::{
    ActionHandle, AppBuilder, BuiltApp, DefinitionCatalog, Registry, TypedActionHandle,
};
pub use error::BuildError;
pub use field::FieldSpec;
pub use field::{
    AccessRule, AccessSpec, Decimal, Fields, Int, IntoFieldSpec, Key, Password, PresentationSpec,
    Radio, StorageSpec, Str, Switch, Table, TableSpec, Text, Timestamp, TimestampMode, Tree,
    ValidationSpec,
};
pub use interface::{Actions, Addon, Module, Modules};
pub use name::{
    ActionName, ActionRef, AddonName, FieldName, FieldRef, ModuleName, TableName, ViewName, ViewRef,
};
#[doc(hidden)]
pub use name::{
    __validate_field_literal, __validate_qualified_literal, __validate_ref_literal,
    __validate_segment_literal,
};
#[cfg(feature = "openapi")]
pub use openapi::OpenApiInfo;
pub use param::{ParamInput, Params};
pub use plugins::{ActionLink, Plugins};
pub use spec::{
    ActionSpec, AddonSpec, FieldKind, HttpMethod, ModuleSpec, ParamSource, ParamSpec, RouteSpec,
    ViewSpec,
};
pub use ui::{
    ActionDemoParamSchema, ActionDemoSchema, ActionResponseKind, FormFieldSchema, FormSchema,
    TableColumnSchema, TableViewSchema, UiCatalog, UiParamSource, WidgetHint, UI_SCHEMA_VERSION,
};
pub use view::CompiledTableView;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod trybuild_tests;
