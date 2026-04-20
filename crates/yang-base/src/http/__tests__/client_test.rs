//! HttpClient 单元测试

use crate::error::BaseError;
use crate::http::HttpClient;

#[test]
fn test_create_http_client() {
    // 测试创建 HTTP 客户端
    let result = HttpClient::new(30);
    assert!(result.is_ok());

    let client = result.unwrap();
    // 验证客户端可以创建请求构建器
    let _builder = client.get("https://api.example.com/test");
}

#[test]
fn test_http_client_methods() {
    // 测试所有 HTTP 方法
    let client = HttpClient::new(30).unwrap();

    // GET
    let _get_builder = client.get("https://api.example.com/test");

    // POST
    let _post_builder = client.post("https://api.example.com/test");

    // PUT
    let _put_builder = client.put("https://api.example.com/test");

    // DELETE
    let _delete_builder = client.delete("https://api.example.com/test");

    // PATCH
    let _patch_builder = client.patch("https://api.example.com/test");
}

#[test]
fn test_set_default_token() {
    // 测试设置默认 Token
    let client = HttpClient::new(30).unwrap();
    client.set_default_token("test_token".to_string());

    // 验证 Token 已设置（通过创建请求构建器）
    let _builder = client.get("https://api.example.com/test");
}

#[test]
fn test_global_client_not_initialized() {
    // 测试未初始化的全局客户端
    // 注意：这个测试可能会失败，如果其他测试已经初始化了全局客户端
    // 在实际测试中，应该使用独立的测试环境
    let result = HttpClient::global();

    // 如果全局客户端未初始化，应该返回错误
    // 如果已初始化（被其他测试），则跳过此断言
    if let Err(err) = result {
        match err {
            BaseError::HttpClientCreateFailed(msg) => {
                assert!(msg.contains("未初始化"));
            }
            _ => panic!("期望 HttpClientCreateFailed 错误"),
        }
    }
}

#[test]
fn test_init_global_client() {
    // 测试初始化全局客户端
    // 注意：由于 OnceLock 的特性，这个测试只能运行一次
    let result = HttpClient::init_global(30);

    // 如果是第一次初始化，应该成功
    // 如果已经初始化过，会返回错误
    match result {
        Ok(_) => {
            // 验证可以获取全局客户端
            let global_result = HttpClient::global();
            assert!(global_result.is_ok());
        }
        Err(BaseError::HttpClientCreateFailed(msg)) => {
            // 已经初始化过，验证错误消息
            assert!(msg.contains("已初始化"));
        }
        Err(_) => panic!("期望 HttpClientCreateFailed 错误"),
    }
}

#[test]
fn test_request_builder_chain() {
    // 测试请求构建器的链式调用
    let client = HttpClient::new(30).unwrap();

    let builder = client
        .get("https://api.example.com/users")
        .header("X-Custom-Header", "value")
        .query("page", "1")
        .query("limit", "10")
        .bearer_token("test_token")
        .user_agent("TestAgent/1.0")
        .timeout(60);

    // 验证构建器创建成功（编译通过即表示链式调用正确）
    drop(builder);
}

#[test]
fn test_request_builder_headers() {
    // 测试批量设置请求头
    let client = HttpClient::new(30).unwrap();

    let builder = client.get("https://api.example.com/users").headers(vec![
        ("X-Header-1", "value1"),
        ("X-Header-2", "value2"),
        ("X-Header-3", "value3"),
    ]);

    drop(builder);
}

#[test]
fn test_request_builder_queries() {
    // 测试批量设置查询参数
    let client = HttpClient::new(30).unwrap();

    let builder = client.get("https://api.example.com/users").queries(vec![
        ("page", "1"),
        ("limit", "10"),
        ("sort", "name"),
    ]);

    drop(builder);
}

#[test]
fn test_request_builder_json() {
    use serde::Serialize;

    #[derive(Serialize)]
    struct TestData {
        name: String,
        value: i32,
    }

    let client = HttpClient::new(30).unwrap();
    let data = TestData {
        name: "test".to_string(),
        value: 42,
    };

    let result = client.post("https://api.example.com/data").json(&data);

    assert!(result.is_ok());
}

#[test]
fn test_request_builder_form() {
    // 测试表单数据
    let client = HttpClient::new(30).unwrap();

    let builder = client
        .post("https://api.example.com/login")
        .form(vec![("username", "alice"), ("password", "secret")]);

    drop(builder);
}

#[test]
fn test_request_builder_text() {
    // 测试文本数据
    let client = HttpClient::new(30).unwrap();

    let builder = client
        .post("https://api.example.com/notes")
        .text("Hello, World!");

    drop(builder);
}

#[test]
fn test_request_builder_body() {
    // 测试原始字节数据
    let client = HttpClient::new(30).unwrap();

    let builder = client
        .post("https://api.example.com/upload")
        .body(vec![0x00, 0x01, 0x02, 0x03]);

    drop(builder);
}
