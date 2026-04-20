//! HTTP 客户端集成测试
//!
//! 测试 HttpClient 的完整功能流程

use crate::http::HttpClient;

#[tokio::test]
async fn test_http_client_full_workflow() {
    // 创建客户端
    let client = HttpClient::new(30).unwrap();

    // 设置默认 Token
    client.set_default_token("test_token_123".to_string());

    // 测试 GET 请求构建
    let get_builder = client
        .get("https://httpbin.org/get")
        .header("X-Test-Header", "test-value")
        .query("param1", "value1")
        .query("param2", "value2")
        .timeout(60);

    // 验证构建器创建成功
    drop(get_builder);

    // 测试 POST 请求构建
    use serde::Serialize;

    #[derive(Serialize)]
    struct TestPayload {
        name: String,
        value: i32,
    }

    let payload = TestPayload {
        name: "test".to_string(),
        value: 42,
    };

    let post_builder = client
        .post("https://httpbin.org/post")
        .json(&payload)
        .unwrap()
        .bearer_token("override_token");

    drop(post_builder);

    // 测试 PUT 请求
    let put_builder = client.put("https://httpbin.org/put").text("Hello, World!");

    drop(put_builder);

    // 测试 DELETE 请求
    let delete_builder = client.delete("https://httpbin.org/delete");

    drop(delete_builder);

    // 测试 PATCH 请求
    let patch_builder = client
        .patch("https://httpbin.org/patch")
        .form(vec![("key1", "value1"), ("key2", "value2")]);

    drop(patch_builder);
}

#[tokio::test]
async fn test_global_client_workflow() {
    // 尝试初始化全局客户端（可能已经被其他测试初始化）
    let _ = HttpClient::init_global(30);

    // 获取全局客户端
    let client = HttpClient::global();
    assert!(client.is_ok());

    let client = client.unwrap();

    // 使用全局客户端创建请求
    let builder = client.get("https://httpbin.org/get").query("test", "value");

    drop(builder);
}

#[test]
fn test_request_builder_content_types() {
    let client = HttpClient::new(30).unwrap();

    // 测试 JSON content type
    use serde::Serialize;

    #[derive(Serialize)]
    struct Data {
        field: String,
    }

    let data = Data {
        field: "value".to_string(),
    };

    let json_builder = client.post("https://api.example.com/data").json(&data);
    assert!(json_builder.is_ok());

    // 测试 form content type
    let form_builder = client
        .post("https://api.example.com/form")
        .form(vec![("key", "value")]);
    drop(form_builder);

    // 测试 text content type
    let text_builder = client
        .post("https://api.example.com/text")
        .text("plain text");
    drop(text_builder);

    // 测试自定义 content type
    let custom_builder = client
        .post("https://api.example.com/custom")
        .content_type("application/xml")
        .body(b"<xml></xml>".to_vec());
    drop(custom_builder);
}

#[test]
fn test_request_builder_authentication() {
    let client = HttpClient::new(30).unwrap();

    // 测试 Bearer Token
    let bearer_builder = client
        .get("https://api.example.com/protected")
        .bearer_token("my_access_token");
    drop(bearer_builder);

    // 测试自定义 Authorization header
    let custom_auth_builder = client
        .get("https://api.example.com/protected")
        .header("Authorization", "Basic dXNlcjpwYXNz");
    drop(custom_auth_builder);
}

#[test]
fn test_request_builder_timeout() {
    let client = HttpClient::new(30).unwrap();

    // 测试自定义超时
    let builder = client.get("https://api.example.com/slow").timeout(120);
    drop(builder);

    // 测试默认超时（30秒）
    let default_builder = client.get("https://api.example.com/fast");
    drop(default_builder);
}

#[test]
fn test_multiple_query_params() {
    let client = HttpClient::new(30).unwrap();

    // 测试多个查询参数
    let builder = client
        .get("https://api.example.com/search")
        .query("q", "rust")
        .query("page", "1")
        .query("limit", "10")
        .query("sort", "relevance");
    drop(builder);

    // 测试批量查询参数
    let batch_builder = client.get("https://api.example.com/search").queries(vec![
        ("q", "rust"),
        ("page", "1"),
        ("limit", "10"),
        ("sort", "relevance"),
    ]);
    drop(batch_builder);
}

#[test]
fn test_multiple_headers() {
    let client = HttpClient::new(30).unwrap();

    // 测试多个请求头
    let builder = client
        .get("https://api.example.com/data")
        .header("X-Custom-1", "value1")
        .header("X-Custom-2", "value2")
        .header("X-Custom-3", "value3")
        .user_agent("MyApp/1.0");
    drop(builder);

    // 测试批量请求头
    let batch_builder = client.get("https://api.example.com/data").headers(vec![
        ("X-Custom-1", "value1"),
        ("X-Custom-2", "value2"),
        ("X-Custom-3", "value3"),
    ]);
    drop(batch_builder);
}
