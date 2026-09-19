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
async fn test_tools_http_client_workflow() {
    // 经 Tools 资源槽获取客户端（替代原全局单例用法）
    let tools = crate::tools::ToolsBuilder::new()
        .http(HttpClient::new(30).unwrap())
        .build()
        .expect("注册 HTTP 客户端后应构建成功");

    let client = tools.http().expect("已配置时应返回 HTTP 客户端");

    // 使用客户端创建请求
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

// ──────────────────────────────────────────────────────────────────────────────
// 重试策略（L-4）
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_retry_config_default() {
    let cfg = crate::http::RetryConfig::default();
    assert_eq!(cfg.max_retries, 3);
    assert_eq!(cfg.backoff_ms, 100);
    assert_eq!(cfg.retry_on, vec![502, 503, 504]);
    // 默认 3 次重试的退避合计仅 ~0.7s，远小于总预算，故默认行为不受钳制影响
    assert_eq!(cfg.total_budget_ms, 30_000);
    assert_eq!(cfg.jitter_percent, 25);
}

#[test]
fn test_request_builder_with_retry() {
    let client = HttpClient::new(30).unwrap();
    // .retry() 是链式 setter，应能与其他链式方法组合
    let builder = client
        .get("https://api.example.com/data")
        .retry(crate::http::RetryConfig {
            max_retries: 5,
            retry_on: vec![500, 503],
            backoff_ms: 10,
            // 本用例仅验证链式组合，显式 opt-in 非幂等重试以免语义被默认值改变
            retry_non_idempotent: false,
            // 退避预算与抖动沿用默认值（10ms 退避下 span 向下取整为 0，不抖动）
            total_budget_ms: 30_000,
            jitter_percent: 25,
        })
        .header("X-Test", "1");
    drop(builder);
}

/// 验证传输错误（连接失败）会触发重试并最终返回 Err，且不 panic。
/// 标记 ignore：发起真实连接，依赖网络栈行为，默认跳过。
#[tokio::test]
#[ignore = "依赖网络栈行为，发起真实连接；默认跳过"]
async fn test_retry_exhausts_on_connection_error() {
    let client = HttpClient::new(1).unwrap();
    let result = client
        .get("http://127.0.0.1:1/never")
        .retry(crate::http::RetryConfig {
            max_retries: 2,
            // 非空：空 retry_on 会被 validate() 拒绝，永远走不到「重试耗尽」路径
            retry_on: vec![503],
            backoff_ms: 1,
            // GET 幂等，无需 opt-in；显式写出避免新增字段后语义歧义
            retry_non_idempotent: false,
            // 1ms 退避下抖动 span 为 0，预算充足，重试路径不被钳制
            total_budget_ms: 30_000,
            jitter_percent: 25,
        })
        .send()
        .await;
    assert!(result.is_err(), "连接失败重试耗尽后应返回 Err");
}

// ──────────────────────────────────────────────────────────────────────────────
// 响应体大小上限（M12）
// ──────────────────────────────────────────────────────────────────────────────

/// 起一个本地 TCP 监听器，接受一次连接后返回指定响应（状态行 + 头 + 空行 + 体）。
///
/// 返回监听地址；服务任务随测试结束时由运行时回收。
async fn serve_once(
    status_line: &'static str,
    headers: String,
    body: Vec<u8>,
) -> std::net::SocketAddr {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("本地监听端口应可绑定");
    let addr = listener.local_addr().expect("应能取得监听地址");

    tokio::spawn(async move {
        let Ok((mut stream, _)) = listener.accept().await else {
            return;
        };
        // 读走请求，避免客户端写请求时收到 EPIPE
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf).await;

        let mut resp = Vec::new();
        resp.extend_from_slice(status_line.as_bytes());
        resp.extend_from_slice(headers.as_bytes());
        resp.extend_from_slice(b"\r\n");
        resp.extend_from_slice(&body);
        let _ = stream.write_all(&resp).await;
        let _ = stream.shutdown().await;
    });

    addr
}

/// Content-Length 预检分支：显式返回 `Content-Length: 1024` 的响应，
/// 应快速失败并携带 `actual == Some(1024)`。
#[tokio::test]
async fn test_response_bytes_exceeds_limit_via_content_length() {
    let addr = serve_once(
        "HTTP/1.1 200 OK\r\n",
        "Content-Type: text/plain\r\nContent-Length: 1024\r\nConnection: close\r\n".to_string(),
        vec![b'x'; 1024],
    )
    .await;

    let client = HttpClient::with_config(crate::http::HttpClientConfig {
        max_response_bytes: 64,
        ..crate::http::HttpClientConfig::default()
    })
    .expect("合法配置应创建客户端");

    let response = client
        .get(&format!("http://{addr}/large"))
        .send()
        .await
        .expect("请求应成功（响应体读取前）");

    let err = match response.bytes().await {
        Ok(_) => panic!("超限响应体应被拒绝"),
        Err(err) => err,
    };

    match err {
        crate::error::BaseError::HttpResponseTooLarge { limit, actual } => {
            assert_eq!(limit, 64);
            assert_eq!(actual, Some(1024));
        }
        other => panic!("期望 HttpResponseTooLarge，实际: {other:?}"),
    }
}

/// 流式分支（无 Content-Length，close 定界）：运行中累计触发上限，
/// 应携带 `actual == None`。
#[tokio::test]
async fn test_response_bytes_exceeds_limit_streaming() {
    let addr = serve_once(
        "HTTP/1.1 200 OK\r\n",
        "Content-Type: text/plain\r\nConnection: close\r\n".to_string(),
        vec![b'y'; 1024],
    )
    .await;

    let client = HttpClient::with_config(crate::http::HttpClientConfig {
        max_response_bytes: 64,
        ..crate::http::HttpClientConfig::default()
    })
    .expect("合法配置应创建客户端");

    let response = client
        .get(&format!("http://{addr}/large"))
        .send()
        .await
        .expect("请求应成功（响应体读取前）");

    let err = match response.bytes().await {
        Ok(_) => panic!("超限响应体应被拒绝"),
        Err(err) => err,
    };

    match err {
        crate::error::BaseError::HttpResponseTooLarge { limit, actual } => {
            assert_eq!(limit, 64);
            assert_eq!(actual, None);
        }
        other => panic!("期望 HttpResponseTooLarge，实际: {other:?}"),
    }
}

/// 边界：body 恰好等于上限时应成功（上限判断是 `>` 而非 `>=`）。
#[tokio::test]
async fn test_response_bytes_exactly_at_limit_succeeds() {
    let limit = 1024usize;
    let headers =
        format!("Content-Type: text/plain\r\nContent-Length: {limit}\r\nConnection: close\r\n");
    let addr = serve_once("HTTP/1.1 200 OK\r\n", headers, vec![b'z'; limit]).await;

    let client = HttpClient::with_config(crate::http::HttpClientConfig {
        max_response_bytes: limit,
        ..crate::http::HttpClientConfig::default()
    })
    .expect("合法配置应创建客户端");

    let response = client
        .get(&format!("http://{addr}/exact"))
        .send()
        .await
        .expect("请求应成功");

    let body = response
        .bytes()
        .await
        .expect("恰好等于上限的响应体应成功读取");
    assert_eq!(body.len(), limit);
}
