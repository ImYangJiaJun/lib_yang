use std::time::Instant;
use yang_db::{RedisClient, RedisConfig};

/// 测试连接池最大连接数配置
///
/// 验证 max_connections 参数是否生效
#[tokio::test]
async fn test_max_connections_config() {
    // 创建一个只允许 2 个连接的配置
    let config = RedisConfig::new(2, 5, 10, false);
    let client = RedisClient::connect_with_config("redis://127.0.0.1:6379", config)
        .await
        .expect("连接失败");

    let pool = client.pool();

    // 获取 2 个连接（应该成功）
    let conn1 = pool.get().await.expect("获取第一个连接失败");
    let conn2 = pool.get().await.expect("获取第二个连接失败");

    // 尝试获取第 3 个连接（应该等待或超时）
    let start = Instant::now();
    let conn3_result = tokio::time::timeout(std::time::Duration::from_secs(2), pool.get()).await;

    // 应该超时，因为连接池已满
    assert!(conn3_result.is_err(), "应该超时，因为连接池最大连接数为 2");

    let elapsed = start.elapsed();
    assert!(elapsed.as_secs() >= 1, "应该等待至少 1 秒才超时");

    // 释放连接
    drop(conn1);
    drop(conn2);
}

/// 测试等待超时配置
///
/// 验证 wait_timeout 参数是否生效
#[tokio::test]
async fn test_wait_timeout_config() {
    // 创建一个等待超时为 2 秒的配置
    let config = RedisConfig::new(1, 5, 2, false);
    let client = RedisClient::connect_with_config("redis://127.0.0.1:6379", config)
        .await
        .expect("连接失败");

    let pool = client.pool();

    // 获取唯一的连接
    let _conn1 = pool.get().await.expect("获取连接失败");

    // 尝试获取第二个连接，应该在约 2 秒后超时
    let start = Instant::now();
    let conn2_result = pool.get().await;
    let elapsed = start.elapsed();

    // 应该失败（超时）
    assert!(
        conn2_result.is_err(),
        "应该超时，因为连接池已满且等待超时为 2 秒"
    );

    // 验证等待时间接近配置的超时时间（允许 ±1 秒误差）
    assert!(
        elapsed.as_secs() >= 1 && elapsed.as_secs() <= 3,
        "等待时间应该接近 2 秒，实际: {} 秒",
        elapsed.as_secs()
    );
}

/// 测试连接超时配置
///
/// 验证 connect_timeout 参数是否生效
#[tokio::test]
async fn test_connect_timeout_config() {
    // 创建一个连接超时为 1 秒的配置
    let config = RedisConfig::new(10, 1, 10, false);

    // 尝试连接到一个不存在的主机
    let start = Instant::now();
    let result = RedisClient::connect_with_config("redis://192.0.2.1:6379", config).await;
    let elapsed = start.elapsed();

    // 应该失败
    assert!(result.is_err(), "连接到无效主机应该失败");

    // 验证失败时间接近配置的超时时间（允许 ±2 秒误差，因为可能有重试）
    assert!(
        elapsed.as_secs() <= 5,
        "连接超时应该在 5 秒内，实际: {} 秒",
        elapsed.as_secs()
    );
}

/// 测试默认配置
///
/// 验证默认配置的参数值
#[tokio::test]
async fn test_default_config() {
    let config = RedisConfig::default();
    assert_eq!(config.max_connections, 10, "默认最大连接数应为 10");
    assert_eq!(config.connect_timeout, 5, "默认连接超时应为 5 秒");
    assert_eq!(config.wait_timeout, 10, "默认等待超时应为 10 秒");
    assert!(!config.enable_logging, "默认应禁用日志");
}

/// 测试自定义配置
///
/// 验证自定义配置的参数值
#[tokio::test]
async fn test_custom_config() {
    let config = RedisConfig::new(20, 10, 15, true);
    assert_eq!(config.max_connections, 20, "最大连接数应为 20");
    assert_eq!(config.connect_timeout, 10, "连接超时应为 10 秒");
    assert_eq!(config.wait_timeout, 15, "等待超时应为 15 秒");
    assert!(config.enable_logging, "应启用日志");
}

/// 测试配置克隆
///
/// 验证配置可以正确克隆
#[test]
fn test_config_clone() {
    let config = RedisConfig::new(15, 8, 12, true);
    let cloned = config.clone();

    assert_eq!(config.max_connections, cloned.max_connections);
    assert_eq!(config.connect_timeout, cloned.connect_timeout);
    assert_eq!(config.wait_timeout, cloned.wait_timeout);
    assert_eq!(config.enable_logging, cloned.enable_logging);
}

/// 测试并发连接数限制
///
/// 验证连接池能够正确限制并发连接数
#[tokio::test]
async fn test_concurrent_connection_limit() {
    // 创建一个最大连接数为 3 的配置
    let config = RedisConfig::new(3, 5, 10, false);
    let client = RedisClient::connect_with_config("redis://127.0.0.1:6379", config)
        .await
        .expect("连接失败");

    let pool = client.pool();

    // 获取 3 个连接
    let conn1 = pool.get().await.expect("获取连接 1 失败");
    let conn2 = pool.get().await.expect("获取连接 2 失败");
    let conn3 = pool.get().await.expect("获取连接 3 失败");

    // 尝试获取第 4 个连接（应该等待）
    let conn4_future = pool.get();

    // 在另一个任务中释放一个连接
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        drop(conn1); // 释放连接 1
                     // 确保连接被返回到池中
        let _ = pool_clone.get().await;
    });

    // 现在应该能够获取第 4 个连接（因为连接 1 被释放了）
    let start = Instant::now();
    let conn4_result = tokio::time::timeout(std::time::Duration::from_secs(5), conn4_future).await;
    let elapsed = start.elapsed();

    assert!(conn4_result.is_ok(), "应该能够获取连接，因为有连接被释放");

    // 验证等待时间大约为 1 秒（允许 ±1 秒误差）
    assert!(
        elapsed.as_secs() <= 3,
        "等待时间应该接近 1 秒，实际: {} 秒",
        elapsed.as_secs()
    );

    // 清理
    drop(conn2);
    drop(conn3);
    drop(conn4_result);
}

/// 测试日志配置
///
/// 验证 enable_logging 参数
#[tokio::test]
async fn test_logging_config() {
    // 测试启用日志
    let config_with_logging = RedisConfig::new(10, 5, 10, true);
    let result =
        RedisClient::connect_with_config("redis://127.0.0.1:6379", config_with_logging).await;
    assert!(result.is_ok(), "启用日志时连接应该成功");

    // 测试禁用日志
    let config_without_logging = RedisConfig::new(10, 5, 10, false);
    let result =
        RedisClient::connect_with_config("redis://127.0.0.1:6379", config_without_logging).await;
    assert!(result.is_ok(), "禁用日志时连接应该成功");
}
