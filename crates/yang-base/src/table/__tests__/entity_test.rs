//! 手写 TableEntity 实现验证（步骤 2，无派生宏）
#![cfg(feature = "mysql")]

use crate::table::entity::*;
use crate::table::{TableConfig, FieldConfig, FieldType};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema, sqlx::FromRow)]
pub struct TestUser {
    pub id: i64,
    pub username: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TestUserField { Id, Username }

impl AsColumnName for TestUserField {
    fn column_name(&self) -> &'static str {
        match self {
            TestUserField::Id => "id",
            TestUserField::Username => "username",
        }
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(tag = "field", content = "cond", rename_all = "snake_case")]
pub enum TestUserWhere {
    Id(WhereOp<i64>),
    Username(StringWhereOp),
}

impl IntoSqlCondition for TestUserWhere {
    fn into_sql_condition(self) -> SqlCondition {
        match self {
            TestUserWhere::Id(op) => op.to_sql_condition("id"),
            TestUserWhere::Username(op) => op.to_sql_condition("username"),
        }
    }
}

impl TableEntity for TestUser {
    type Pk = i64;
    type Field = TestUserField;
    type WhereCond = TestUserWhere;
    const TABLE_NAME: &'static str = "test_users";
    const PK_FIELD: &'static str = "id";
    fn table_config() -> &'static TableConfig {
        static C: OnceLock<TableConfig> = OnceLock::new();
        C.get_or_init(|| TableConfig::new("test_users")
            .primary_key("id")
            .field(FieldConfig::new("id", FieldType::BigInt))
            .field(FieldConfig::new("username", FieldType::String { max_length: 50 })))
    }
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
    assert!(matches!(op, WhereOp::In(_)));
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
