#![allow(deprecated)]
#![allow(clippy::expect_used, clippy::unwrap_used)]
//! Lua 脚本集成测试
//!
//! 使用 testcontainers 测试 Redis Lua 脚本的实际执行

use testcontainers::{runners::AsyncRunner, GenericImage};
use yang_db::RedisClient;

/// 启动 Redis 容器并返回连接 URL
async fn setup_redis_container() -> Option<(testcontainers::ContainerAsync<GenericImage>, String)> {
    let redis_image = GenericImage::new("redis", "7-alpine").with_wait_for(
        testcontainers::core::WaitFor::message_on_stdout("Ready to accept connections"),
    );

    match redis_image.start().await {
        Ok(container) => {
            let port = container.get_host_port_ipv4(6379).await.ok()?;
            let url = format!("redis://127.0.0.1:{}", port);

            // 等待 Redis 完全启动
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

            Some((container, url))
        }
        Err(e) => {
            eprintln!("无法启动 Redis 容器: {}. 跳过集成测试。", e);
            None
        }
    }
}

#[tokio::test]
async fn test_simple_script_execution() {
    let Some((_container, url)) = setup_redis_container().await else {
        return;
    };
    let client = RedisClient::connect(&url).await.unwrap();

    // 创建简单脚本：返回字符串
    let script = client.script(r#"return "Hello, Lua!""#);

    // 执行脚本
    let result: String = client.eval_script(&script, &[], &[]).await.unwrap();

    assert_eq!(result, "Hello, Lua!");
}

#[tokio::test]
async fn test_script_with_keys_parameter() {
    let Some((_container, url)) = setup_redis_container().await else {
        return;
    };
    let client = RedisClient::connect(&url).await.unwrap();

    // 设置初始值
    client.set("test_key", "test_value").await.unwrap();

    // 创建脚本：读取 KEYS[1]
    let script = client.script(r#"return redis.call('GET', KEYS[1])"#);

    // 执行脚本
    let result: String = client
        .eval_script(&script, &["test_key".to_string()], &[])
        .await
        .unwrap();

    assert_eq!(result, "test_value");
}

#[tokio::test]
async fn test_script_with_args_parameter() {
    let Some((_container, url)) = setup_redis_container().await else {
        return;
    };
    let client = RedisClient::connect(&url).await.unwrap();

    // 创建脚本：使用 ARGV 参数
    let script = client.script(
        r#"
        redis.call('SET', KEYS[1], ARGV[1])
        return redis.call('GET', KEYS[1])
        "#,
    );

    // 执行脚本
    let result: String = client
        .eval_script(&script, &["my_key".to_string()], &["my_value".to_string()])
        .await
        .unwrap();

    assert_eq!(result, "my_value");

    // 验证值确实被设置了
    let value = client.get("my_key").await.unwrap();
    assert_eq!(value, Some("my_value".to_string()));
}

#[tokio::test]
async fn test_script_return_number() {
    let Some((_container, url)) = setup_redis_container().await else {
        return;
    };
    let client = RedisClient::connect(&url).await.unwrap();

    // 创建脚本：返回数字
    let script = client.script(r#"return 42"#);

    // 执行脚本
    let result: i64 = client.eval_script(&script, &[], &[]).await.unwrap();

    assert_eq!(result, 42);
}

#[tokio::test]
async fn test_script_atomic_counter_increment() {
    let Some((_container, url)) = setup_redis_container().await else {
        return;
    };
    let client = RedisClient::connect(&url).await.unwrap();

    // 初始化计数器
    client.set("counter", "0").await.unwrap();

    // 创建脚本：原子性地增加计数器
    let script = client.script(
        r#"
        local current = redis.call('GET', KEYS[1])
        if not current then
            current = 0
        end
        local new_value = tonumber(current) + tonumber(ARGV[1])
        redis.call('SET', KEYS[1], new_value)
        return new_value
        "#,
    );

    // 执行脚本
    let result: i64 = client
        .eval_script(&script, &["counter".to_string()], &["10".to_string()])
        .await
        .unwrap();

    assert_eq!(result, 10);

    // 再次执行
    let result: i64 = client
        .eval_script(&script, &["counter".to_string()], &["5".to_string()])
        .await
        .unwrap();

    assert_eq!(result, 15);

    // 验证最终值
    let final_value = client.get("counter").await.unwrap().unwrap();
    assert_eq!(final_value, "15");
}

#[tokio::test]
async fn test_script_conditional_update() {
    let Some((_container, url)) = setup_redis_container().await else {
        return;
    };
    let client = RedisClient::connect(&url).await.unwrap();

    // 初始化余额
    client.set("balance", "100").await.unwrap();

    // 创建脚本：只有余额足够时才扣减
    let script = client.script(
        r#"
        local balance = tonumber(redis.call('GET', KEYS[1]) or 0)
        local amount = tonumber(ARGV[1])
        if balance >= amount then
            redis.call('DECRBY', KEYS[1], amount)
            return 1
        else
            return 0
        end
        "#,
    );

    // 测试成功扣减
    let success: i64 = client
        .eval_script(&script, &["balance".to_string()], &["50".to_string()])
        .await
        .unwrap();
    assert_eq!(success, 1);

    let balance = client.get("balance").await.unwrap().unwrap();
    assert_eq!(balance, "50");

    // 测试失败扣减（余额不足）
    let success: i64 = client
        .eval_script(&script, &["balance".to_string()], &["100".to_string()])
        .await
        .unwrap();
    assert_eq!(success, 0);

    // 余额应该保持不变
    let balance = client.get("balance").await.unwrap().unwrap();
    assert_eq!(balance, "50");
}

#[tokio::test]
async fn test_script_multiple_keys() {
    let Some((_container, url)) = setup_redis_container().await else {
        return;
    };
    let client = RedisClient::connect(&url).await.unwrap();

    // 创建脚本：原子性地增加两个计数器
    let script = client.script(
        r#"
        redis.call('INCR', KEYS[1])
        redis.call('INCR', KEYS[2])
        local v1 = redis.call('GET', KEYS[1])
        local v2 = redis.call('GET', KEYS[2])
        return {v1, v2}
        "#,
    );

    // 执行脚本
    let result: Vec<String> = client
        .eval_script(
            &script,
            &["counter1".to_string(), "counter2".to_string()],
            &[],
        )
        .await
        .unwrap();

    assert_eq!(result, vec!["1", "1"]);

    // 再次执行
    let result: Vec<String> = client
        .eval_script(
            &script,
            &["counter1".to_string(), "counter2".to_string()],
            &[],
        )
        .await
        .unwrap();

    assert_eq!(result, vec!["2", "2"]);
}

#[tokio::test]
async fn test_script_return_array() {
    let Some((_container, url)) = setup_redis_container().await else {
        return;
    };
    let client = RedisClient::connect(&url).await.unwrap();

    // 创建脚本：返回数组
    let script = client.script(r#"return {1, 2, 3, "four"}"#);

    // 执行脚本，返回混合类型数组
    let result: Vec<redis::Value> = client.eval_script(&script, &[], &[]).await.unwrap();

    assert_eq!(result.len(), 4);
}

#[tokio::test]
async fn test_script_with_loop() {
    let Some((_container, url)) = setup_redis_container().await else {
        return;
    };
    let client = RedisClient::connect(&url).await.unwrap();

    // 创建脚本：计算 1 到 N 的和
    let script = client.script(
        r#"
        local sum = 0
        for i = 1, tonumber(ARGV[1]) do
            sum = sum + i
        end
        return sum
        "#,
    );

    // 计算 1 到 10 的和
    let result: i64 = client
        .eval_script(&script, &[], &["10".to_string()])
        .await
        .unwrap();

    assert_eq!(result, 55); // 1+2+3+...+10 = 55
}

#[tokio::test]
async fn test_script_atomicity() {
    let Some((_container, url)) = setup_redis_container().await else {
        return;
    };
    let client = RedisClient::connect(&url).await.unwrap();

    // 初始化两个账户
    client.set("account_a", "100").await.unwrap();
    client.set("account_b", "50").await.unwrap();

    // 创建脚本：原子性地转账
    let script = client.script(
        r#"
        local from_balance = tonumber(redis.call('GET', KEYS[1]))
        local amount = tonumber(ARGV[1])
        
        if from_balance >= amount then
            redis.call('DECRBY', KEYS[1], amount)
            redis.call('INCRBY', KEYS[2], amount)
            return 1
        else
            return 0
        end
        "#,
    );

    // 执行转账：从 account_a 转 30 到 account_b
    let success: i64 = client
        .eval_script(
            &script,
            &["account_a".to_string(), "account_b".to_string()],
            &["30".to_string()],
        )
        .await
        .unwrap();

    assert_eq!(success, 1);

    // 验证余额
    let balance_a = client.get("account_a").await.unwrap().unwrap();
    let balance_b = client.get("account_b").await.unwrap().unwrap();
    assert_eq!(balance_a, "70");
    assert_eq!(balance_b, "80");
}

#[tokio::test]
async fn test_script_evalsha_caching() {
    let Some((_container, url)) = setup_redis_container().await else {
        return;
    };
    let client = RedisClient::connect(&url).await.unwrap();

    // 创建脚本
    let script = client.script(r#"return "cached""#);

    // 第一次执行（会使用 EVAL）
    let result1: String = client.eval_script(&script, &[], &[]).await.unwrap();
    assert_eq!(result1, "cached");

    // 第二次执行（应该使用 EVALSHA）
    let result2: String = client.eval_script(&script, &[], &[]).await.unwrap();
    assert_eq!(result2, "cached");

    // redis-rs 的 Script 会自动处理 EVALSHA 和 EVAL 的回退
    // 这里我们只验证多次执行都能成功
}

#[tokio::test]
async fn test_script_error_handling() {
    let Some((_container, url)) = setup_redis_container().await else {
        return;
    };
    let client = RedisClient::connect(&url).await.unwrap();

    // 创建包含错误的脚本（尝试对非数字进行算术运算）
    let script = client.script(
        r#"
        redis.call('SET', KEYS[1], 'not_a_number')
        return tonumber(redis.call('GET', KEYS[1])) + 10
        "#,
    );

    // 执行脚本应该返回错误
    let result: Result<i64, _> = client
        .eval_script(&script, &["test_key".to_string()], &[])
        .await;

    // 验证返回了错误
    assert!(result.is_err());
}

#[tokio::test]
async fn test_script_with_table_operations() {
    let Some((_container, url)) = setup_redis_container().await else {
        return;
    };
    let client = RedisClient::connect(&url).await.unwrap();

    // 设置多个键
    client.set("key1", "value1").await.unwrap();
    client.set("key2", "value2").await.unwrap();
    client.set("key3", "value3").await.unwrap();

    // 创建脚本：批量获取多个键的值
    let script = client.script(
        r#"
        local result = {}
        for i = 1, #KEYS do
            table.insert(result, redis.call('GET', KEYS[i]))
        end
        return result
        "#,
    );

    // 执行脚本
    let result: Vec<String> = client
        .eval_script(
            &script,
            &["key1".to_string(), "key2".to_string(), "key3".to_string()],
            &[],
        )
        .await
        .unwrap();

    assert_eq!(result, vec!["value1", "value2", "value3"]);
}
