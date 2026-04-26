//! ApiResponse 集成测试

use crate::action::ApiResponse;
use crate::error::BaseError;
use serde_json::json;

#[test]
fn test_success_response_with_user_data() {
    let user_data = json!({
        "id": 1,
        "username": "alice",
        "email": "alice@example.com",
        "roles": ["admin", "user"]
    });

    let response = ApiResponse::success(user_data, "用户信息获取成功");

    assert_eq!(response.code, 0);
    assert_eq!(response.message, "用户信息获取成功");
    assert!(response.data.is_some());

    let data = response.data.unwrap();
    assert_eq!(data["id"], 1);
    assert_eq!(data["username"], "alice");
}

#[test]
fn test_success_response_with_list() {
    let users = json!([
        { "id": 1, "name": "Alice" },
        { "id": 2, "name": "Bob" },
        { "id": 3, "name": "Charlie" }
    ]);

    let response = ApiResponse::success(users, "用户列表查询成功");

    assert_eq!(response.code, 0);
    assert!(response.data.is_some());

    let data = response.data.unwrap();
    assert!(data.is_array());
    assert_eq!(data.as_array().unwrap().len(), 3);
}

#[test]
fn test_success_response_with_affected_rows() {
    let result = json!({ "affected": 5 });
    let response = ApiResponse::success(result, "批量更新成功");

    assert_eq!(response.code, 0);
    assert_eq!(response.message, "批量更新成功");

    let data = response.data.unwrap();
    assert_eq!(data["affected"], 5);
}

#[test]
fn test_fail_response_with_various_codes() {
    // 参数错误
    let response = ApiResponse::fail(400001, "参数缺失: username");
    assert_eq!(response.code, 400001);
    assert!(response.data.is_none());

    // 权限错误
    let response = ApiResponse::fail(403001, "权限不足");
    assert_eq!(response.code, 403001);

    // 业务错误
    let response = ApiResponse::fail(500001, "用户名已存在");
    assert_eq!(response.code, 500001);
}

#[test]
fn test_from_error_plugin_errors() {
    let error = BaseError::PluginNotFound("test_plugin".to_string());
    let response = ApiResponse::from_error(error);

    assert_eq!(response.code, 100002);
    assert!(response.message.contains("test_plugin"));
    assert!(response.data.is_none());
}

#[test]
fn test_from_error_database_errors() {
    let error = BaseError::DatabaseQueryFailed("连接超时".to_string());
    let response = ApiResponse::from_error(error);

    assert_eq!(response.code, 200003);
    assert!(response.message.contains("连接超时"));
}

#[test]
fn test_from_error_field_validation_errors() {
    let error = BaseError::FieldRequired("username".to_string());
    let response = ApiResponse::from_error(error);

    assert_eq!(response.code, 600006);
    assert!(response.message.contains("username"));

    let error = BaseError::ValidationFailed("email".to_string(), "格式不正确".to_string());
    let response = ApiResponse::from_error(error);

    assert_eq!(response.code, 600005);
    assert!(response.message.contains("email"));
    assert!(response.message.contains("格式不正确"));
}

#[test]
fn test_response_serialization_format() {
    let response = ApiResponse::success(json!({ "count": 10 }), "查询成功");
    let json_str = serde_json::to_string(&response).unwrap();

    // 验证 JSON 格式
    assert!(json_str.contains("\"code\":0"));
    assert!(json_str.contains("\"message\":\"查询成功\""));
    assert!(json_str.contains("\"data\""));
    assert!(json_str.contains("\"count\":10"));
}

#[test]
fn test_fail_response_serialization_omits_data() {
    let response = ApiResponse::fail(500001, "服务器错误");
    let json_str = serde_json::to_string(&response).unwrap();

    // 验证 data 字段被省略
    assert!(json_str.contains("\"code\":500001"));
    assert!(json_str.contains("\"message\":\"服务器错误\""));
    assert!(!json_str.contains("\"data\""));
}

#[test]
fn test_response_chain_with_error_handling() {
    // 模拟一个可能失败的操作
    fn get_user(id: i32) -> Result<serde_json::Value, BaseError> {
        if id <= 0 {
            return Err(BaseError::ValidationFailed(
                "id".to_string(),
                "ID 必须大于 0".to_string(),
            ));
        }

        Ok(json!({
            "id": id,
            "name": "Alice"
        }))
    }

    // 成功情况
    match get_user(1) {
        Ok(user) => {
            let response = ApiResponse::success(user, "获取成功");
            assert_eq!(response.code, 0);
        }
        Err(e) => {
            let response = ApiResponse::from_error(e);
            assert_ne!(response.code, 0);
        }
    }

    // 失败情况
    match get_user(-1) {
        Ok(user) => {
            let response = ApiResponse::success(user, "获取成功");
            assert_eq!(response.code, 0);
        }
        Err(e) => {
            let response = ApiResponse::from_error(e);
            assert_eq!(response.code, 600005); // ValidationFailed
            assert!(response.message.contains("ID 必须大于 0"));
        }
    }
}

#[test]
fn test_all_error_types_have_valid_codes() {
    // 测试所有错误类型都能正确转换为响应
    let errors = vec![
        BaseError::PluginNotFound("test".to_string()),
        BaseError::DatabaseQueryFailed("test".to_string()),
        BaseError::HttpTimeout,
        BaseError::TokenExpired,
        BaseError::JsonSerializeFailed("test".to_string()),
        BaseError::FieldRequired("test".to_string()),
        BaseError::ValidationFailed("field".to_string(), "reason".to_string()),
        BaseError::Unknown("test".to_string()),
    ];

    for error in errors {
        let response = ApiResponse::from_error(error);
        assert_ne!(response.code, 0, "错误响应的 code 不应该为 0");
        assert!(response.data.is_none(), "错误响应不应该包含 data");
        assert!(!response.message.is_empty(), "错误响应应该包含消息");
    }
}
