//! Schema 快照测试（H-1 验收）
//!
//! 用 insta 固定派生宏生成的 JSON Schema 形态：实体行类型、`SelectQuery<T>`
//! 查询契约。schema 漂移（字段增删、where 条件结构变化）会让快照失败，
//! 从而显式暴露契约变更。
//!
//! 升级 schemars 或有意调整契约后，用 `cargo insta review` 复核并接受新快照。
#![cfg(feature = "mysql")]

use serde::{Deserialize, Serialize};
use yang_base::action::builtin::SelectQuery;
use yang_base_derive::TableEntity;

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema, sqlx::FromRow, TableEntity)]
#[table(name = "snapshot_users")]
struct SnapshotUser {
    #[entity(primary_key)]
    id: i64,
    #[entity(max_length = 50)]
    username: String,
    age: i32,
}

#[test]
fn snapshot_entity_schema() {
    let schema = schemars::schema_for!(SnapshotUser);
    insta::assert_json_snapshot!("entity_snapshot_user", schema);
}

#[test]
fn snapshot_select_query_schema() {
    let schema = schemars::schema_for!(SelectQuery<SnapshotUser>);
    insta::assert_json_snapshot!("select_query_snapshot_user", schema);
}
