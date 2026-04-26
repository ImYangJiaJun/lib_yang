use yang_db::{RedisClient, RedisConfig, RedisValue};

/// 测试 Redis 连接
///
/// 需要本地 Redis 服务器运行在 127.0.0.1:6379
#[tokio::test]
async fn test_redis_connect() {
    let result = RedisClient::connect("redis://127.0.0.1:6379").await;
    assert!(result.is_ok(), "Redis 连接失败: {:?}", result.err());
}

/// 测试使用自定义配置连接
#[tokio::test]
async fn test_redis_connect_with_config() {
    let config = RedisConfig::new(5, 3, 5, false);
    let result = RedisClient::connect_with_config("redis://127.0.0.1:6379", config).await;
    assert!(result.is_ok(), "使用自定义配置连接失败: {:?}", result.err());
}

/// 测试连接池功能
#[tokio::test]
async fn test_redis_pool() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    let pool = client.pool();
    let conn_result = pool.get().await;
    assert!(conn_result.is_ok(), "从连接池获取连接失败");
}

/// 测试执行 PING 命令
#[tokio::test]
async fn test_redis_execute_ping() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    let cmd = redis::cmd("PING");
    let result = client.execute(&cmd).await;
    assert!(result.is_ok(), "执行 PING 命令失败: {:?}", result.err());

    let value = result.unwrap();
    assert!(matches!(value, RedisValue::String(_)));
}

/// 测试执行 SET 和 GET 命令
#[tokio::test]
async fn test_redis_execute_set_get() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // SET 命令
    let mut set_cmd = redis::cmd("SET");
    set_cmd.arg("test_key").arg("test_value");
    let set_result = client.execute(&set_cmd).await;
    assert!(
        set_result.is_ok(),
        "执行 SET 命令失败: {:?}",
        set_result.err()
    );

    // GET 命令
    let mut get_cmd = redis::cmd("GET");
    get_cmd.arg("test_key");
    let get_result = client.execute(&get_cmd).await;
    assert!(
        get_result.is_ok(),
        "执行 GET 命令失败: {:?}",
        get_result.err()
    );

    let value = get_result.unwrap();
    if let RedisValue::String(s) = value {
        assert_eq!(s, "test_value");
    } else {
        panic!("期望返回字符串类型");
    }

    // 清理测试数据
    let mut del_cmd = redis::cmd("DEL");
    del_cmd.arg("test_key");
    let _ = client.execute(&del_cmd).await;
}

/// 测试并发连接
#[tokio::test]
async fn test_redis_concurrent_connections() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    let mut handles = vec![];

    // 创建 10 个并发任务
    for i in 0..10 {
        let client_clone = client.clone();
        let handle = tokio::spawn(async move {
            let mut cmd = redis::cmd("SET");
            cmd.arg(format!("concurrent_key_{}", i))
                .arg(format!("value_{}", i));
            client_clone.execute(&cmd).await
        });
        handles.push(handle);
    }

    // 等待所有任务完成
    for handle in handles {
        let result = handle.await.expect("任务执行失败");
        assert!(result.is_ok(), "并发执行失败: {:?}", result.err());
    }

    // 清理测试数据
    for i in 0..10 {
        let mut del_cmd = redis::cmd("DEL");
        del_cmd.arg(format!("concurrent_key_{}", i));
        let _ = client.execute(&del_cmd).await;
    }
}

/// 测试错误处理 - 无效的连接 URL
#[tokio::test]
async fn test_redis_invalid_url() {
    let result = RedisClient::connect("redis://invalid-host:9999").await;
    assert!(result.is_err(), "应该连接失败");
}

/// 测试错误处理 - 无效的命令
#[tokio::test]
async fn test_redis_invalid_command() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 执行一个不存在的命令
    let cmd = redis::cmd("INVALID_COMMAND");
    let result = client.execute(&cmd).await;
    assert!(result.is_err(), "应该执行失败");
}
