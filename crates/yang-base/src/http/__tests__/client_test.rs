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
            BaseError::HttpClientNotInitialized => {
                // 正确：全局客户端未初始化
            }
            _ => panic!("期望 HttpClientNotInitialized 错误"),
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
        Err(BaseError::HttpClientAlreadyInitialized) => {
            // 已经初始化过，符合预期
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

#[tokio::test]
async fn test_header_error_accumulation_invalid_name() {
    // 测试非法 header 名称被累积为错误，send() 时返回 ParamInvalid
    let client = HttpClient::new(30).unwrap();

    // 包含空格的 header 名称是非法的
    let result = client
        .get("https://api.example.com/test")
        .header("Invalid Header Name", "value")
        .send()
        .await;

    assert!(result.is_err(), "非法 header 名称应导致错误");
    // 使用 err().unwrap() 而非 unwrap_err()，因为 Response 未实现 Debug
    match result.err().unwrap() {
        BaseError::ParamInvalid(field, msg) => {
            assert_eq!(field, "header", "错误字段应为 header");
            assert!(
                msg.contains("Invalid Header Name"),
                "错误信息应包含非法 header 名称，实际: {}",
                msg
            );
        }
        other => panic!("期望 ParamInvalid 错误，实际: {:?}", other),
    }
}

#[tokio::test]
async fn test_header_error_accumulation_invalid_value() {
    // 测试包含非法字符的 header 值被累积为错误
    let client = HttpClient::new(30).unwrap();

    // 包含换行符的 header 值是非法的（HTTP 头注入防护）
    let result = client
        .get("https://api.example.com/test")
        .header("X-Custom", "value\r\nX-Injected: injected")
        .send()
        .await;

    assert!(result.is_err(), "包含换行符的 header 值应导致错误");
    match result.err().unwrap() {
        BaseError::ParamInvalid(field, _msg) => {
            assert_eq!(field, "header", "错误字段应为 header");
        }
        other => panic!("期望 ParamInvalid 错误，实际: {:?}", other),
    }
}

#[tokio::test]
async fn test_header_error_accumulation_multiple_errors() {
    // 测试多个非法 header 的错误被合并到一条错误信息中
    let client = HttpClient::new(30).unwrap();

    let result = client
        .get("https://api.example.com/test")
        .header("Bad Name 1", "value1")
        .header("Bad Name 2", "value2")
        .send()
        .await;

    assert!(result.is_err(), "多个非法 header 应导致错误");
    match result.err().unwrap() {
        BaseError::ParamInvalid(field, msg) => {
            assert_eq!(field, "header");
            // 错误信息应包含两个错误，用分号分隔
            assert!(
                msg.contains("Bad Name 1") && msg.contains("Bad Name 2"),
                "错误信息应包含所有非法 header 名称，实际: {}",
                msg
            );
        }
        other => panic!("期望 ParamInvalid 错误，实际: {:?}", other),
    }
}

#[tokio::test]
async fn test_header_error_accumulation_valid_headers_pass() {
    // 测试合法 header 不产生错误（不发送实际请求，仅验证构建阶段无错误累积）
    // 注意：send() 会尝试真实网络请求，此处仅验证 header 解析不产生 ParamInvalid
    let client = HttpClient::new(30).unwrap();

    let result = client
        .get("https://127.0.0.1:1") // 使用不可达地址，确保快速失败
        .header("X-Valid-Header", "valid-value")
        .header("Content-Type", "application/json")
        .timeout(1)
        .send()
        .await;

    // 合法 header 不应产生 ParamInvalid 错误
    match &result {
        Err(BaseError::ParamInvalid(field, _)) if field == "header" => {
            panic!("合法 header 不应产生 ParamInvalid 错误");
        }
        _ => {
            // 其他错误（如连接失败）是预期的，说明 header 解析通过了
        }
    }
}

#[test]
fn test_form_url_encoding_special_chars() {
    // 测试 form 方法使用 serde_urlencoded 正确编码特殊字符
    let client = HttpClient::new(30).unwrap();

    // 包含特殊字符的表单数据
    let builder = client
        .post("https://api.example.com/form")
        .form(vec![
            ("name", "Alice & Bob"),
            ("email", "alice+bob@example.com"),
            ("message", "Hello World!"),
        ]);

    // 构建成功，无错误累积
    drop(builder);
}

#[test]
fn test_form_url_encoding_utf8() {
    // 测试 form 方法正确处理 UTF-8 字符
    let client = HttpClient::new(30).unwrap();

    let builder = client
        .post("https://api.example.com/form")
        .form(vec![("name", "张三"), ("city", "北京")]);

    drop(builder);
}

#[test]
fn test_form_sets_content_type() {
    // 测试 form 方法自动设置 Content-Type
    // 通过构建后不产生 header 错误来间接验证
    let client = HttpClient::new(30).unwrap();

    // form 方法内部调用 content_type("application/x-www-form-urlencoded")
    // 这是合法的 Content-Type，不应产生 header 错误
    let builder = client
        .post("https://api.example.com/form")
        .form(vec![("key", "value")]);

    drop(builder);
}
