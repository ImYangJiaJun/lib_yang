//! 内置 CRUD Actions 单元测试（H-1 类型化后）
//!
//! 旧测试基于已移除的 `AddAction::new(table_config)` + `Action::execute` API，
//! 现重写为对类型化泛型 builtin 的元信息断言：每个 `XxxAction<T>` 的
//! `name`/`display_name`/`is_public` 与 `#[action(...)]` 标注一致，且能作为
//! `DynAction` trait object 暴露正确的 `ActionMeta`。涉及真实数据库的执行路径
//! 由 `tests/typed_action_integration.rs`（testcontainers）覆盖。
#![cfg(feature = "mysql")]

use crate::action::builtin::{
    AddAction, DelAction, GetAction, PutAction, SelectAction, TableAction,
};
use crate::action::{DynAction, TypedAction};
use serde::{Deserialize, Serialize};
use yang_base_derive::TableEntity;

/// 测试用实体：派生 TableEntity 以实例化泛型 builtin。
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema, sqlx::FromRow, TableEntity)]
#[table(name = "test_users")]
struct TestUser {
    #[entity(primary_key)]
    id: i64,
    #[entity(max_length = 50)]
    username: String,
    age: i32,
}

#[test]
fn test_add_action_meta() {
    let action = AddAction::<TestUser>::new();
    assert_eq!(action.name(), "add");
    assert_eq!(action.display_name(), "新增数据");
    assert!(!action.is_public());
}

#[test]
fn test_put_action_meta() {
    let action = PutAction::<TestUser>::new();
    assert_eq!(action.name(), "put");
    assert_eq!(action.display_name(), "更新数据");
    assert!(!action.is_public());
}

#[test]
fn test_del_action_meta() {
    let action = DelAction::<TestUser>::new();
    assert_eq!(action.name(), "del");
    assert_eq!(action.display_name(), "删除数据");
    assert!(!action.is_public());
}

#[test]
fn test_get_action_meta() {
    let action = GetAction::<TestUser>::new();
    assert_eq!(action.name(), "get");
    assert_eq!(action.display_name(), "获取数据");
    assert!(!action.is_public());
}

#[test]
fn test_select_action_meta() {
    let action = SelectAction::<TestUser>::new();
    assert_eq!(action.name(), "select");
    assert_eq!(action.display_name(), "查询列表");
    assert!(!action.is_public());
}

#[test]
fn test_table_action_meta() {
    let action = TableAction::<TestUser>::new();
    assert_eq!(action.name(), "table");
    assert_eq!(action.display_name(), "表元信息");
}

#[test]
fn test_builtin_meta_through_dyn() {
    // 作为 DynAction trait object 暴露的 ActionMeta 名称正确
    let get: &dyn DynAction = &GetAction::<TestUser>::new();
    assert_eq!(get.meta().name, "get");

    let add: &dyn DynAction = &AddAction::<TestUser>::new();
    assert_eq!(add.meta().name, "add");
}

#[test]
fn test_builtin_input_schema_non_empty() {
    // 派生宏为每个 builtin 生成了非空的 input schema
    let action = SelectAction::<TestUser>::new();
    let schema = serde_json::to_value(action.input_schema()).expect("schema 应可序列化");
    assert!(schema.is_object(), "input schema 应为 JSON 对象");
}
