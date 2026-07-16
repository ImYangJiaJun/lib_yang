//! Schema-first 表定义的 JSON Schema 快照测试。
//!
//! 预期结构直接内联在测试中，避免迁移期间继续依赖旧实体派生类型；
//! 字段增删、必填语义或读写 Schema 变化仍会显式导致测试失败。
#![cfg(feature = "mysql")]

use serde_json::json;
use yang_base::table::{Field, Table, TableDefinition};

fn snapshot_user_table() -> TableDefinition {
    Table::new("snapshot_users")
        .fields([
            Field::id("id"),
            Field::string("username", 50).required(),
            Field::integer("age").required(),
        ])
        .build()
        .expect("snapshot_users 表定义应有效")
}

#[test]
fn snapshot_input_schema() {
    assert_eq!(
        snapshot_user_table().input_schema(),
        json!({
            "type": "object",
            "title": "snapshot_users",
            "properties": {
                "age": { "type": "integer", "title": "age" },
                "username": { "type": "string", "maxLength": 50, "title": "username" }
            },
            "required": ["age", "username"],
            "additionalProperties": false
        })
    );
}

#[test]
fn snapshot_output_schema() {
    assert_eq!(
        snapshot_user_table().output_schema(),
        json!({
            "type": "object",
            "title": "snapshot_users",
            "properties": {
                "age": { "type": "integer", "title": "age" },
                "id": { "type": "integer", "title": "id" },
                "username": { "type": "string", "maxLength": 50, "title": "username" }
            },
            "required": ["age", "id", "username"],
            "additionalProperties": false
        })
    );
}
