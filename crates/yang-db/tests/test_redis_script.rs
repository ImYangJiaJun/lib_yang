#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Lua 脚本真实集成测试。
//!
//! 连接 `REDIS_TEST_URL`（未设置时回退 `redis://127.0.0.1:6379`）；
//! 连接失败必须失败，不做软跳过。

use yang_db::RedisClient;

fn redis_url() -> String {
    std::env::var("REDIS_TEST_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
}

async fn client() -> RedisClient {
    RedisClient::connect(redis_url())
        .await
        .expect("连接测试 Redis 失败")
}

fn key(suffix: &str) -> String {
    format!("yang:test:script:{suffix}:{}", std::process::id())
}

#[tokio::test]
async fn test_script_creation_and_execution() {
    let client = client().await;
    let script = client.script(r#"return "Hello, Lua!""#);
    let value: String = client.eval_script(&script, &[], &[]).await.unwrap();
    assert_eq!(value, "Hello, Lua!");
}

#[tokio::test]
async fn test_script_with_keys_and_args() {
    let client = client().await;
    let script = client.script(r#"return KEYS[1] .. ":" .. ARGV[1]"#);
    let value: String = client
        .eval_script(&script, &["prefix".to_string()], &["suffix".to_string()])
        .await
        .unwrap();
    assert_eq!(value, "prefix:suffix");
}

#[tokio::test]
async fn test_script_return_number() {
    let client = client().await;
    let script = client.script("return 42");
    let value: i64 = client.eval_script(&script, &[], &[]).await.unwrap();
    assert_eq!(value, 42);
}

#[tokio::test]
async fn test_script_return_array() {
    let client = client().await;
    let script = client.script(r#"return {1, 2, 3, "four"}"#);
    let value: Vec<String> = client.eval_script(&script, &[], &[]).await.unwrap();
    assert_eq!(value, vec!["1", "2", "3", "four"]);
}

#[tokio::test]
async fn test_script_returns_nil() {
    let client = client().await;
    let cache_key = key("missing");
    client.del(std::slice::from_ref(&cache_key)).await.unwrap();
    let script = client.script("return redis.call('GET', KEYS[1])");
    let value: Option<String> = client
        .eval_script(&script, &[cache_key], &[])
        .await
        .unwrap();
    assert_eq!(value, None);
}

#[tokio::test]
async fn test_script_syntax_error_is_reported() {
    let client = client().await;
    let script = client.script("this is not valid lua");
    let result: Result<i64, _> = client.eval_script(&script, &[], &[]).await;
    assert!(result.is_err(), "非法脚本必须返回错误");
}

#[tokio::test]
async fn test_script_evalsha_caching() {
    let client = client().await;
    let cache_key = key("evalsha");
    client.del(std::slice::from_ref(&cache_key)).await.unwrap();
    let script = client.script("return redis.call('INCR', KEYS[1])");

    let first: i64 = client
        .eval_script(&script, std::slice::from_ref(&cache_key), &[])
        .await
        .unwrap();
    let second: i64 = client
        .eval_script(&script, std::slice::from_ref(&cache_key), &[])
        .await
        .unwrap();
    assert_eq!((first, second), (1, 2), "EVALSHA 缓存应复用同一脚本");
    client.del(&[cache_key]).await.unwrap();
}

#[tokio::test]
async fn test_script_with_loop() {
    let client = client().await;
    let script = client.script(
        r#"
        local total = 0
        for i = 1, 10 do
            total = total + i
        end
        return total
        "#,
    );
    let value: i64 = client.eval_script(&script, &[], &[]).await.unwrap();
    assert_eq!(value, 55);
}

#[tokio::test]
async fn test_script_conditional_update() {
    let client = client().await;
    let cache_key = key("balance");
    client.set(&cache_key, "100").await.unwrap();
    let script = client.script(
        r#"
        local balance = tonumber(redis.call('GET', KEYS[1]) or 0)
        local amount = tonumber(ARGV[1])
        if balance >= amount then
            redis.call('DECRBY', KEYS[1], amount)
            return 1
        end
        return 0
        "#,
    );
    let success: i64 = client
        .eval_script(
            &script,
            std::slice::from_ref(&cache_key),
            &["30".to_string()],
        )
        .await
        .unwrap();
    assert_eq!(success, 1);
    assert_eq!(
        client.get(&cache_key).await.unwrap(),
        Some("70".to_string())
    );
    client.del(&[cache_key]).await.unwrap();
}

#[tokio::test]
async fn test_script_type_error_does_not_leak_payload() {
    let client = client().await;
    let secret = "eyJhbGciOiJIUzI1NiJ9.super-secret-token";
    let script = client.script(r#"return ARGV[1]"#);
    let result: Result<i64, _> = client
        .eval_script(&script, &[], &[secret.to_string()])
        .await;
    let error = result.expect_err("字符串结果按 i64 解码必须失败");
    assert!(
        !format!("{error}").contains("super-secret-token"),
        "类型错误不得回显原始响应: {error}"
    );
}
