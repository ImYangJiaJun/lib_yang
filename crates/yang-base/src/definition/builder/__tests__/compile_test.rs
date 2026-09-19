//! `build_registry` 的构建期回归测试：锁定表定义「每表一份、Arc 共享」语义。

use crate::action::{ActionContext, TypedHandler};
use crate::definition::{
    ActionName, AddonName, AddonSpec, FieldKind, FieldName, FieldSpec, HttpMethod, ModuleName,
    ModuleSpec, RouteSpec, TableName, TableSpec,
};
use crate::definition::{ActionSpec, BuildError};
use crate::error::BaseError;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::super::compile::build_registry;

#[derive(Debug, Deserialize, JsonSchema)]
struct NoopInput {}

#[derive(Debug, Serialize, JsonSchema)]
struct NoopOutput {}

#[derive(crate::Action)]
#[action(name = "first", public)]
struct FirstAction;

#[derive(crate::Action)]
#[action(name = "second", public)]
struct SecondAction;

#[async_trait]
impl TypedHandler for FirstAction {
    type Input = NoopInput;
    type Output = NoopOutput;

    async fn handle(
        &self,
        _ctx: ActionContext,
        _input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Ok(NoopOutput {})
    }
}

#[async_trait]
impl TypedHandler for SecondAction {
    type Input = NoopInput;
    type Output = NoopOutput;

    async fn handle(
        &self,
        _ctx: ActionContext,
        _input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Ok(NoopOutput {})
    }
}

fn module_name(value: &str) -> ModuleName {
    ModuleName::new(value).expect("测试模块名应有效")
}

fn table_name(value: &str) -> TableName {
    TableName::new(value).expect("测试表名应有效")
}

fn field_name(value: &str) -> FieldName {
    FieldName::new(value).expect("测试字段名应有效")
}

fn addon_name(value: &str) -> AddonName {
    AddonName::new(value).expect("测试 Addon 名应有效")
}

fn action_name(value: &str) -> ActionName {
    ActionName::new(value).expect("测试 Action 名应有效")
}

fn get_action(name: &str, path: &str, operation_id: &str) -> ActionSpec {
    ActionSpec::new(
        action_name(name),
        RouteSpec::new(HttpMethod::Get, path, operation_id),
    )
    .public(true)
}

/// 同一模块的两个 Action 必须共享同一份 `Arc<TableDefinition>`：
/// 既锁住「每表只编译一次」（消除 O(A×M) 重复编译），也锁住运行期零拷贝共享。
#[test]
fn build_registry_shares_one_table_definition_across_module_actions() {
    let module = ModuleSpec::new(module_name("org.post"))
        .table(
            TableSpec::new(table_name("org_post"))
                .field(FieldSpec::new(field_name("id"), FieldKind::Key))
                .field(FieldSpec::new(field_name("title"), FieldKind::Str).required(true)),
        )
        .action(
            get_action("first", "/org/posts/first", "org.post.first"),
            FirstAction,
        )
        .action(
            get_action("second", "/org/posts/second", "org.post.second"),
            SecondAction,
        );
    let addons = vec![AddonSpec::new(addon_name("org")).module(module)];

    let registry = build_registry(&addons).expect("合法模块应构建成功");

    assert_eq!(registry.handlers.len(), 2);
    let first = registry.handlers[0]
        .table_definition
        .as_ref()
        .expect("Action 应携带模块主表定义");
    let second = registry.handlers[1]
        .table_definition
        .as_ref()
        .expect("Action 应携带模块主表定义");
    assert!(
        Arc::ptr_eq(first, second),
        "同模块两个 Action 应共享同一份 Arc<TableDefinition>"
    );
}

/// 无主表的模块不应携带表定义，且不得因缓存查找而产生构建期错误。
#[test]
fn build_registry_leaves_table_definition_empty_without_module_table() {
    let module = ModuleSpec::new(module_name("org.echo")).action(
        get_action("first", "/org/echo", "org.echo.first"),
        FirstAction,
    );
    let addons = vec![AddonSpec::new(addon_name("org")).module(module)];

    let registry = build_registry(&addons).expect("无主表模块应构建成功");

    assert_eq!(registry.handlers.len(), 1);
    assert!(registry.handlers[0].table_definition.is_none());
}

/// 非法主表仍须在构建期被拒绝，且错误指向具体表名（缓存预建不得吞掉该校验）。
#[test]
fn build_registry_still_rejects_invalid_module_table() {
    let module =
        ModuleSpec::new(module_name("org.bad")).table(TableSpec::new(table_name("org_bad")));
    let addons = vec![AddonSpec::new(addon_name("org")).module(module)];

    let error = build_registry(&addons).expect_err("空主表应被构建期拒绝");

    assert!(matches!(
        error,
        BuildError::InvalidFieldDefinition { ref table, .. } if table == "org_bad"
    ));
}
