//! 内置 CRUD Actions 单元测试

use crate::action::builtin::{
    AddAction, DelAction, GetAction, PutAction, SelectAction, TableAction,
};
use crate::action::{Action, ActionContext, GlobalTools, Request};
use crate::error::BaseError;
use crate::table::{FieldConfig, FieldType, TableConfig};
use crate::token::TokenManager;
use jsonwebtoken::Algorithm;
use serde_json::json;
use std::sync::Arc;

/// 创建测试用的 TokenManager
fn create_test_token_manager() -> TokenManager {
    TokenManager::new_symmetric(
        "test_secret_key",
        Algorithm::HS256,
        "test_issuer".to_string(),
        "test_audience".to_string(),
        3600,
        86400,
    )
}

/// 创建测试用的 GlobalTools
fn create_test_tools() -> Arc<GlobalTools> {
    Arc::new(GlobalTools::new(create_test_token_manager()))
}

/// 创建测试用的表配置
fn create_test_table_config() -> Arc<TableConfig> {
    Arc::new(
        TableConfig::new("test_users")
            .field(FieldConfig::new("id", FieldType::Integer))
            .field(FieldConfig::new(
                "username",
                FieldType::String { max_length: 50 },
            ))
            .field(FieldConfig::new(
                "email",
                FieldType::String { max_length: 100 },
            ))
            .field(FieldConfig::new("age", FieldType::Integer))
            .primary_key("id"),
    )
}

#[test]
fn test_add_action_name() {
    let table_config = create_test_table_config();
    let action = AddAction::new(table_config);

    assert_eq!(action.name(), "add");
    assert_eq!(action.display_name(), "新增数据");
    assert!(!action.is_public());
}

#[test]
fn test_put_action_name() {
    let table_config = create_test_table_config();
    let action = PutAction::new(table_config);

    assert_eq!(action.name(), "put");
    assert_eq!(action.display_name(), "更新数据");
    assert!(!action.is_public());
}

#[test]
fn test_del_action_name() {
    let table_config = create_test_table_config();
    let action = DelAction::new(table_config);

    assert_eq!(action.name(), "del");
    assert_eq!(action.display_name(), "删除数据");
    assert!(!action.is_public());
}

#[test]
fn test_get_action_name() {
    let table_config = create_test_table_config();
    let action = GetAction::new(table_config);

    assert_eq!(action.name(), "get");
    assert_eq!(action.display_name(), "获取数据");
    assert!(!action.is_public());
}

#[test]
fn test_select_action_name() {
    let table_config = create_test_table_config();
    let action = SelectAction::new(table_config);

    assert_eq!(action.name(), "select");
    assert_eq!(action.display_name(), "查询列表");
    assert!(!action.is_public());
}

#[test]
fn test_table_action_name() {
    let table_config = create_test_table_config();
    let action = TableAction::new(table_config);

    assert_eq!(action.name(), "table");
    assert_eq!(action.display_name(), "获取表元数据");
    assert!(action.is_public()); // TableAction 是公开的
}

#[test]
fn test_add_action_params_schema() {
    let table_config = create_test_table_config();
    let action = AddAction::new(table_config);

    let schema = action.params_schema();
    assert!(schema.is_some());

    let schema_value = schema.unwrap();
    assert_eq!(schema_value["type"], "object");
    assert!(schema_value["properties"]["data"].is_object());
    assert_eq!(schema_value["required"][0], "data");
}

#[test]
fn test_select_action_params_schema() {
    let table_config = create_test_table_config();
    let action = SelectAction::new(table_config);

    let schema = action.params_schema();
    assert!(schema.is_some());

    let schema_value = schema.unwrap();
    assert_eq!(schema_value["type"], "object");
    assert!(schema_value["properties"]["fields"].is_object());
    assert!(schema_value["properties"]["where"].is_object());
    assert!(schema_value["properties"]["order_by"].is_object());
    assert!(schema_value["properties"]["page"].is_object());
    assert!(schema_value["properties"]["page_size"].is_object());
}

#[tokio::test]
async fn test_add_action_param_missing() {
    let table_config = create_test_table_config();
    let action = AddAction::new(table_config.clone());

    // 创建没有 data 参数的请求
    let request = Request::new(json!({}));
    let tools = create_test_tools();
    let context = ActionContext::new(request, tools).with_table_config(table_config);

    // 执行 action
    let result = action.execute(context).await;

    // 应该返回参数缺失错误
    assert!(result.is_err());
    match result.unwrap_err() {
        BaseError::ParamMissing(param) => assert_eq!(param, "data"),
        _ => panic!("期望 ParamMissing 错误"),
    }
}

#[tokio::test]
async fn test_add_action_param_invalid_type() {
    let table_config = create_test_table_config();
    let action = AddAction::new(table_config.clone());

    // 创建 data 参数类型错误的请求（应该是对象，但传入了字符串）
    let request = Request::new(json!({
        "data": "invalid"
    }));
    let tools = create_test_tools();
    let context = ActionContext::new(request, tools).with_table_config(table_config);

    // 执行 action
    let result = action.execute(context).await;

    // 应该返回参数无效错误
    assert!(result.is_err());
    match result.unwrap_err() {
        BaseError::ParamInvalid(param, _) => assert_eq!(param, "data"),
        _ => panic!("期望 ParamInvalid 错误"),
    }
}

#[tokio::test]
async fn test_put_action_param_missing() {
    let table_config = create_test_table_config();
    let action = PutAction::new(table_config.clone());

    // 创建没有主键参数的请求
    let request = Request::new(json!({
        "data": { "username": "alice" }
    }));
    let tools = create_test_tools();
    let context = ActionContext::new(request, tools).with_table_config(table_config);

    // 执行 action
    let result = action.execute(context).await;

    // 应该返回参数缺失错误
    assert!(result.is_err());
    match result.unwrap_err() {
        BaseError::ParamMissing(param) => assert_eq!(param, "id"),
        _ => panic!("期望 ParamMissing 错误"),
    }
}

#[tokio::test]
async fn test_del_action_param_missing() {
    let table_config = create_test_table_config();
    let action = DelAction::new(table_config.clone());

    // 创建没有主键参数的请求
    let request = Request::new(json!({}));
    let tools = create_test_tools();
    let context = ActionContext::new(request, tools).with_table_config(table_config);

    // 执行 action
    let result = action.execute(context).await;

    // 应该返回参数缺失错误
    assert!(result.is_err());
    match result.unwrap_err() {
        BaseError::ParamMissing(param) => assert_eq!(param, "id"),
        _ => panic!("期望 ParamMissing 错误"),
    }
}

#[tokio::test]
async fn test_get_action_param_missing() {
    let table_config = create_test_table_config();
    let action = GetAction::new(table_config.clone());

    // 创建没有主键参数的请求
    let request = Request::new(json!({}));
    let tools = create_test_tools();
    let context = ActionContext::new(request, tools).with_table_config(table_config);

    // 执行 action
    let result = action.execute(context).await;

    // 应该返回参数缺失错误
    assert!(result.is_err());
    match result.unwrap_err() {
        BaseError::ParamMissing(param) => assert_eq!(param, "id"),
        _ => panic!("期望 ParamMissing 错误"),
    }
}

#[tokio::test]
async fn test_table_action_execute() {
    let table_config = create_test_table_config();
    let action = TableAction::new(table_config.clone());

    // 创建请求
    let request = Request::new(json!({}));
    let tools = create_test_tools();
    let context = ActionContext::new(request, tools).with_table_config(table_config);

    // 执行 action
    let result = action.execute(context).await;

    // 应该返回成功
    assert!(result.is_ok());

    let response = result.unwrap();
    assert_eq!(response.code, 0);
    assert_eq!(response.message, "获取表元数据成功");

    // 验证返回的元数据
    let data = response.data.unwrap();
    assert_eq!(data["table_name"], "test_users");
    assert_eq!(data["primary_key"], "id");
    assert!(data["fields"].is_array());

    let fields = data["fields"].as_array().unwrap();
    assert_eq!(fields.len(), 4); // id, username, email, age
}

#[test]
fn test_all_actions_implement_send_sync() {
    // 测试所有 Actions 实现了 Send + Sync
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<AddAction>();
    assert_send_sync::<PutAction>();
    assert_send_sync::<DelAction>();
    assert_send_sync::<GetAction>();
    assert_send_sync::<SelectAction>();
    assert_send_sync::<TableAction>();
}
