//! 派生 TableEntity 实现验证（Task 4 之后）
#![cfg(feature = "mysql")]

use crate::table::entity::*;
use serde::{Deserialize, Serialize};

#[derive(
    Debug, Deserialize, Serialize, schemars::JsonSchema, sqlx::FromRow,
    yang_base_derive::TableEntity,
)]
#[table(name = "test_users")]
pub struct TestUser {
    #[entity(primary_key)]
    pub id: i64,
    #[entity(max_length = 50, unique)]
    pub username: String,
}

#[test]
fn test_where_op_deserialize() {
    let json = r#"{"op":"eq","value":42}"#;
    let op: WhereOp<i64> = serde_json::from_str(json).unwrap();
    assert!(matches!(op, WhereOp::Eq(42)));
}

#[test]
fn test_where_op_in_deserialize() {
    let json = r#"{"op":"in","value":[1,2,3]}"#;
    let op: WhereOp<i64> = serde_json::from_str(json).unwrap();
    match op {
        WhereOp::In(vs) => assert_eq!(vs, vec![1i64, 2, 3]),
        other => panic!("expected WhereOp::In, got {:?}", other),
    }
}

#[test]
fn test_test_user_where_deserialize() {
    let json = r#"{"field":"id","cond":{"op":"eq","value":42}}"#;
    let cond: TestUserWhere = serde_json::from_str(json).unwrap();
    let sql_cond = cond.into_sql_condition();
    assert_eq!(sql_cond.column, "id");
    assert!(matches!(sql_cond.op, SqlOp::Eq));
}

#[test]
fn test_invalid_field_rejected() {
    let json = r#"{"field":"unknown","cond":{"op":"eq","value":42}}"#;
    let result: Result<TestUserWhere, _> = serde_json::from_str(json);
    assert!(result.is_err(), "未知字段名必须反序列化失败");
}

#[test]
fn test_string_like_works() {
    let json = r#"{"field":"username","cond":{"op":"like","value":"%alice%"}}"#;
    let cond: TestUserWhere = serde_json::from_str(json).unwrap();
    let sql_cond = cond.into_sql_condition();
    assert_eq!(sql_cond.column, "username");
    assert!(matches!(sql_cond.op, SqlOp::Like));
    assert_eq!(sql_cond.params[0].as_str(), Some("%alice%"));
}

#[test]
fn test_derived_table_constants() {
    use crate::table::TableEntity;
    assert_eq!(TestUser::TABLE_NAME, "test_users");
    assert_eq!(TestUser::PK_FIELD, "id");
}

#[test]
fn test_derived_table_config() {
    use crate::table::TableEntity;
    let cfg = TestUser::table_config();
    assert_eq!(cfg.table_name, "test_users");
    assert_eq!(cfg.primary_key, "id");
    assert!(cfg.fields.contains_key("id"), "应包含 id 字段");
    assert!(cfg.fields.contains_key("username"), "应包含 username 字段");
    // unique 字段应该在 unique_indexes 中
    assert_eq!(cfg.unique_indexes.len(), 1, "username 标记 unique，应有 1 个唯一索引");
}
