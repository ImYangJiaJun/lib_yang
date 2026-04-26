//! Request 结构体单元测试

use crate::action::Request;
use serde_json::json;
use std::collections::HashMap;

#[test]
fn test_request_new() {
    // 测试创建新请求
    let body = json!({
        "username": "alice",
        "email": "alice@example.com"
    });

    let request = Request::new(body.clone());

    assert_eq!(request.body, body);
    assert!(request.headers.is_empty());
    assert!(request.query.is_empty());
    assert!(request.path_params.is_empty());
}

#[test]
fn test_request_default() {
    // 测试默认请求
    let request = Request::default();

    assert_eq!(request.body, serde_json::Value::Null);
    assert!(request.headers.is_empty());
    assert!(request.query.is_empty());
    assert!(request.path_params.is_empty());
}

#[test]
fn test_request_header() {
    // 测试添加单个请求头
    let request = Request::new(json!({}))
        .header("Content-Type", "application/json")
        .header("User-Agent", "TestClient/1.0");

    assert_eq!(request.get_header("Content-Type"), Some("application/json"));
    assert_eq!(request.get_header("User-Agent"), Some("TestClient/1.0"));
    assert_eq!(request.get_header("Non-Existent"), None);
}

#[test]
fn test_request_headers() {
    // 测试批量添加请求头
    let mut headers = HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    headers.insert("Authorization".to_string(), "Bearer token123".to_string());

    let request = Request::new(json!({})).headers(headers);

    assert_eq!(request.get_header("Content-Type"), Some("application/json"));
    assert_eq!(request.get_header("Authorization"), Some("Bearer token123"));
}

#[test]
fn test_request_query() {
    // 测试添加单个查询参数
    let request = Request::new(json!({}))
        .query("page", "1")
        .query("limit", "10")
        .query("sort", "name");

    assert_eq!(request.get_query("page"), Some("1"));
    assert_eq!(request.get_query("limit"), Some("10"));
    assert_eq!(request.get_query("sort"), Some("name"));
    assert_eq!(request.get_query("non-existent"), None);
}

#[test]
fn test_request_queries() {
    // 测试批量添加查询参数
    let mut query = HashMap::new();
    query.insert("page".to_string(), "1".to_string());
    query.insert("limit".to_string(), "10".to_string());

    let request = Request::new(json!({})).queries(query);

    assert_eq!(request.get_query("page"), Some("1"));
    assert_eq!(request.get_query("limit"), Some("10"));
}

#[test]
fn test_request_path_param() {
    // 测试添加单个路径参数
    let request = Request::new(json!({}))
        .path_param("id", "123")
        .path_param("action", "update");

    assert_eq!(request.get_path_param("id"), Some("123"));
    assert_eq!(request.get_path_param("action"), Some("update"));
    assert_eq!(request.get_path_param("non-existent"), None);
}

#[test]
fn test_request_path_params() {
    // 测试批量添加路径参数
    let mut path_params = HashMap::new();
    path_params.insert("id".to_string(), "123".to_string());
    path_params.insert("action".to_string(), "update".to_string());

    let request = Request::new(json!({})).path_params(path_params);

    assert_eq!(request.get_path_param("id"), Some("123"));
    assert_eq!(request.get_path_param("action"), Some("update"));
}

#[test]
fn test_request_token_with_bearer() {
    // 测试从 Authorization 头提取 Bearer Token
    let request = Request::new(json!({})).header("Authorization", "Bearer my_secret_token");

    assert_eq!(request.token(), Some("my_secret_token"));
}

#[test]
fn test_request_token_lowercase_header() {
    // 测试小写的 authorization 头
    let request = Request::new(json!({})).header("authorization", "Bearer lowercase_token");

    assert_eq!(request.token(), Some("lowercase_token"));
}

#[test]
fn test_request_token_without_bearer() {
    // 测试没有 Bearer 前缀的 Token
    let request = Request::new(json!({})).header("Authorization", "InvalidFormat");

    assert_eq!(request.token(), None);
}

#[test]
fn test_request_token_missing() {
    // 测试缺失 Authorization 头
    let request = Request::new(json!({}));

    assert_eq!(request.token(), None);
}

#[test]
fn test_request_token_empty_bearer() {
    // 测试空的 Bearer Token
    let request = Request::new(json!({})).header("Authorization", "Bearer ");

    assert_eq!(request.token(), Some(""));
}

#[test]
fn test_request_chain_methods() {
    // 测试链式调用
    let request = Request::new(json!({
        "username": "alice",
        "password": "secret"
    }))
    .header("Content-Type", "application/json")
    .header("Authorization", "Bearer token123")
    .query("page", "1")
    .query("limit", "10")
    .path_param("id", "123");

    // 验证请求体
    assert_eq!(request.body["username"], "alice");
    assert_eq!(request.body["password"], "secret");

    // 验证请求头
    assert_eq!(request.get_header("Content-Type"), Some("application/json"));
    assert_eq!(request.token(), Some("token123"));

    // 验证查询参数
    assert_eq!(request.get_query("page"), Some("1"));
    assert_eq!(request.get_query("limit"), Some("10"));

    // 验证路径参数
    assert_eq!(request.get_path_param("id"), Some("123"));
}

#[test]
fn test_request_complex_body() {
    // 测试复杂的请求体
    let body = json!({
        "user": {
            "name": "Alice",
            "age": 30,
            "roles": ["admin", "user"]
        },
        "metadata": {
            "timestamp": 1234567890,
            "source": "web"
        }
    });

    let request = Request::new(body.clone());

    assert_eq!(request.body, body);
    assert_eq!(request.body["user"]["name"], "Alice");
    assert_eq!(request.body["user"]["age"], 30);
    assert_eq!(request.body["user"]["roles"][0], "admin");
}

#[test]
fn test_request_overwrite_header() {
    // 测试覆盖请求头
    let request = Request::new(json!({}))
        .header("Content-Type", "text/plain")
        .header("Content-Type", "application/json");

    assert_eq!(request.get_header("Content-Type"), Some("application/json"));
}

#[test]
fn test_request_overwrite_query() {
    // 测试覆盖查询参数
    let request = Request::new(json!({}))
        .query("page", "1")
        .query("page", "2");

    assert_eq!(request.get_query("page"), Some("2"));
}

#[test]
fn test_request_overwrite_path_param() {
    // 测试覆盖路径参数
    let request = Request::new(json!({}))
        .path_param("id", "123")
        .path_param("id", "456");

    assert_eq!(request.get_path_param("id"), Some("456"));
}

#[test]
fn test_request_extend_headers() {
    // 测试扩展请求头（不覆盖已有的）
    let request = Request::new(json!({}))
        .header("Content-Type", "application/json")
        .headers({
            let mut h = HashMap::new();
            h.insert("Authorization".to_string(), "Bearer token".to_string());
            h.insert("User-Agent".to_string(), "TestClient".to_string());
            h
        });

    assert_eq!(request.get_header("Content-Type"), Some("application/json"));
    assert_eq!(request.get_header("Authorization"), Some("Bearer token"));
    assert_eq!(request.get_header("User-Agent"), Some("TestClient"));
}

#[test]
fn test_request_extend_queries() {
    // 测试扩展查询参数
    let request = Request::new(json!({})).query("page", "1").queries({
        let mut q = HashMap::new();
        q.insert("limit".to_string(), "10".to_string());
        q.insert("sort".to_string(), "name".to_string());
        q
    });

    assert_eq!(request.get_query("page"), Some("1"));
    assert_eq!(request.get_query("limit"), Some("10"));
    assert_eq!(request.get_query("sort"), Some("name"));
}

#[test]
fn test_request_extend_path_params() {
    // 测试扩展路径参数
    let request = Request::new(json!({}))
        .path_param("id", "123")
        .path_params({
            let mut p = HashMap::new();
            p.insert("action".to_string(), "update".to_string());
            p.insert("version".to_string(), "v1".to_string());
            p
        });

    assert_eq!(request.get_path_param("id"), Some("123"));
    assert_eq!(request.get_path_param("action"), Some("update"));
    assert_eq!(request.get_path_param("version"), Some("v1"));
}
