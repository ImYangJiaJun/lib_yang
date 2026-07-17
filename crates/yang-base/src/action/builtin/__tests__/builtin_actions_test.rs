//! Schema-first 内置 CRUD Actions 单元测试。
//!
//! 这里验证无泛型 builtin 的静态元信息，以及 `DynAction::dispatch` 对动态 DTO
//! 和当前上下文 `TableDefinition` 的运行期契约。真实数据库执行路径由集成测试覆盖。
#![cfg(feature = "mysql")]

use crate::action::builtin::{
    AddAction, DelAction, GetAction, PutAction, SelectAction, TableAction,
};
use crate::action::{ActionContext, DynAction, Request, TypedAction, User};
use crate::error::BaseError;
use crate::table::{Field, Table, TableDefinition};
#[cfg(feature = "token")]
use crate::token::TokenManager;
use crate::tools::ToolsBuilder;
#[cfg(feature = "token")]
use jsonwebtoken::Algorithm;
use serde_json::json;
use std::sync::Arc;

fn test_definition() -> TableDefinition {
    Table::new("test_users")
        .label("测试用户")
        .fields([Field::id("id"), Field::string("username", 50).required()])
        .build()
        .expect("测试表定义应有效")
}

fn make_ctx(body: serde_json::Value) -> ActionContext {
    #[cfg(feature = "token")]
    let tools = Arc::new(
        ToolsBuilder::new()
            .token(TokenManager::new_symmetric(
                "test_secret_key",
                Algorithm::HS256,
                "test_issuer".to_string(),
                "test_audience".to_string(),
                3600,
                86400,
            ))
            .build()
            .expect("测试 Tools 应构建成功"),
    );
    #[cfg(not(feature = "token"))]
    let tools = Arc::new(ToolsBuilder::new().build().expect("测试 Tools 应构建成功"));

    ActionContext::new(Request::new(body), tools)
}

#[test]
fn test_builtin_action_meta_without_entity_generics() {
    let add = AddAction::new();
    let put = PutAction::new();
    let del = DelAction::new();
    let get = GetAction::new();
    let select = SelectAction::new();
    let table = TableAction::new();

    let cases: [(&dyn DynAction, &str, &str); 6] = [
        (&add, "add", "新增数据"),
        (&put, "put", "更新数据"),
        (&del, "del", "删除数据"),
        (&get, "get", "获取数据"),
        (&select, "select", "查询列表"),
        (&table, "table", "表元信息"),
    ];

    for (action, name, display_name) in cases {
        assert_eq!(action.meta().name, name);
        assert_eq!(action.meta().display_name, display_name);
        assert!(!action.meta().is_public);
    }
}

#[test]
fn test_builtin_input_schema_non_empty() {
    let action = SelectAction::new();
    let schema = serde_json::to_value(action.input_schema()).expect("schema 应可序列化");
    assert!(schema.is_object(), "input schema 应为 JSON 对象");
    assert!(
        schema.pointer("/properties/order_by").is_some(),
        "SelectQuery schema 应暴露字符串排序字段"
    );
}

#[tokio::test]
async fn test_table_action_dispatch_uses_bound_definition() {
    let ctx = make_ctx(json!({})).with_table_definition(test_definition());
    let action = TableAction::new();

    let response = action.dispatch(ctx).await.expect("table dispatch 应成功");
    let data = response.data.expect("table dispatch 应返回 data");

    assert_eq!(data["table_name"], "test_users");
    assert_eq!(data["primary_key"], "id");
    assert_eq!(
        data["input_schema"]["properties"]["username"]["maxLength"],
        50
    );
    assert_eq!(
        data["output_schema"]["properties"]["username"]["type"],
        "string"
    );
}

#[tokio::test]
async fn test_table_action_dispatch_scopes_schema_to_current_user_roles() {
    let definition = Table::new("team_notes")
        .fields([
            Field::id("id"),
            Field::string("title", 64).required(),
            Field::string("admin_note", 255)
                .readable_by(["admin"])
                .writable_by(["admin"]),
            Field::string("member_note", 255)
                .readable_by(["member"])
                .writable_by(["member"]),
        ])
        .build()
        .expect("角色字段表定义应有效");
    let ctx = make_ctx(json!({}))
        .with_user(User::new(7, "member").with_roles(["member"]))
        .with_table_definition(definition);

    let response = TableAction::new()
        .dispatch(ctx)
        .await
        .expect("table dispatch 应成功");
    let data = response.data.expect("table dispatch 应返回 data");

    for schema_name in ["input_schema", "output_schema"] {
        let properties = &data[schema_name]["properties"];
        assert!(properties.get("title").is_some());
        assert!(properties.get("member_note").is_some());
        assert!(properties.get("admin_note").is_none());
    }
}

#[tokio::test]
async fn test_add_dispatch_rejects_non_record_body() {
    let result = AddAction::new().dispatch(make_ctx(json!([]))).await;
    match result {
        Err(BaseError::ParamInvalid(field, _)) => assert_eq!(field, "body"),
        Err(other) => panic!("期望 ParamInvalid，实际: {other:?}"),
        Ok(_) => panic!("数组请求体不应通过 Record 输入契约"),
    }
}

#[tokio::test]
async fn test_put_dispatch_rejects_empty_record() {
    let result = PutAction::new()
        .dispatch(make_ctx(json!({"id": "user-1", "data": {}})))
        .await;
    match result {
        Err(BaseError::ParamInvalid(field, _)) => assert_eq!(field, "data"),
        Err(other) => panic!("期望 ParamInvalid，实际: {other:?}"),
        Ok(_) => panic!("空更新对象不应通过 PutInput 契约"),
    }
}

#[tokio::test]
async fn test_select_dispatch_accepts_dynamic_where_and_string_order() {
    let result = SelectAction::new()
        .dispatch(make_ctx(json!({
            "page": 0,
            "where": {"type": "eq", "field": "username", "value": "alice"},
            "order_by": [{"field": "username"}]
        })))
        .await;
    match result {
        Err(BaseError::ParamInvalid(field, _)) => assert_eq!(field, "page/page_size"),
        Err(other) => panic!("期望分页校验错误，实际: {other:?}"),
        Ok(_) => panic!("page=0 不应通过分页校验"),
    }
}

#[tokio::test]
async fn test_select_dispatch_checks_auth_before_count() {
    let result = SelectAction::new()
        .dispatch(make_ctx(json!({"count_total": true})))
        .await;
    match result {
        Err(BaseError::Unauthorized(message)) => assert_eq!(message, "需要登录"),
        Err(other) => panic!("期望鉴权错误，实际: {other:?}"),
        Ok(_) => panic!("未登录请求不应进入 COUNT 路径"),
    }
}
