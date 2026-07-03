#![allow(deprecated)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
#![allow(dead_code)]
#![allow(unused_results)]
use yang_db::RedisClient;

/// 测试 GETRANGE 命令
#[tokio::test]
async fn test_getrange() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 设置测试数据
    client
        .set("test_getrange", "Hello World")
        .await
        .expect("SET 失败");

    // 测试正常范围
    let substr = client
        .getrange("test_getrange", 0, 4)
        .await
        .expect("GETRANGE 失败");
    assert_eq!(substr, "Hello", "子串应为 'Hello'");

    // 测试负数索引
    let substr = client
        .getrange("test_getrange", -5, -1)
        .await
        .expect("GETRANGE 失败");
    assert_eq!(substr, "World", "子串应为 'World'");

    // 测试完整范围
    let substr = client
        .getrange("test_getrange", 0, -1)
        .await
        .expect("GETRANGE 失败");
    assert_eq!(substr, "Hello World", "子串应为完整字符串");

    // 测试超出范围
    let substr = client
        .getrange("test_getrange", 0, 100)
        .await
        .expect("GETRANGE 失败");
    assert_eq!(substr, "Hello World", "超出范围应返回完整字符串");

    // 测试不存在的键
    let substr = client
        .getrange("nonexistent_key", 0, 10)
        .await
        .expect("GETRANGE 失败");
    assert_eq!(substr, "", "不存在的键应返回空字符串");

    // 清理
    client.del(&["test_getrange".to_string()]).await.ok();
}

/// 测试 SETRANGE 命令
#[tokio::test]
async fn test_setrange() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 设置测试数据
    client
        .set("test_setrange", "Hello World")
        .await
        .expect("SET 失败");

    // 测试替换部分内容
    let len = client
        .setrange("test_setrange", 6, "Redis")
        .await
        .expect("SETRANGE 失败");
    assert_eq!(len, 11, "字符串长度应为 11");

    let value = client
        .get("test_setrange")
        .await
        .expect("GET 失败")
        .expect("键应存在");
    assert_eq!(value, "Hello Redis", "内容应为 'Hello Redis'");

    // 测试从头开始替换
    client
        .setrange("test_setrange", 0, "Hi")
        .await
        .expect("SETRANGE 失败");
    let value = client
        .get("test_setrange")
        .await
        .expect("GET 失败")
        .expect("键应存在");
    assert_eq!(value, "Hillo Redis", "内容应为 'Hillo Redis'");

    // 测试扩展字符串（超出原长度）
    let len = client
        .setrange("test_setrange", 20, "End")
        .await
        .expect("SETRANGE 失败");
    assert!(len >= 23, "字符串长度应至少为 23");

    // 测试不存在的键（会创建新键）
    let len = client
        .setrange("new_key", 0, "New")
        .await
        .expect("SETRANGE 失败");
    assert_eq!(len, 3, "新键长度应为 3");

    // 清理
    client
        .del(&["test_setrange".to_string(), "new_key".to_string()])
        .await
        .ok();
}

/// 测试 INCRBYFLOAT 命令
#[tokio::test]
async fn test_incrbyfloat() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 设置初始值
    client.set("test_float", "10.5").await.expect("SET 失败");

    // 测试增加正数
    let result = client
        .incrbyfloat("test_float", 2.3)
        .await
        .expect("INCRBYFLOAT 失败");
    assert!((result - 12.8).abs() < 0.001, "结果应约为 12.8");

    // 测试增加负数（减少）
    let result = client
        .incrbyfloat("test_float", -5.5)
        .await
        .expect("INCRBYFLOAT 失败");
    assert!((result - 7.3).abs() < 0.001, "结果应约为 7.3");

    // 测试不存在的键（从 0 开始）
    let result = client
        .incrbyfloat("new_float", std::f64::consts::PI)
        .await
        .expect("INCRBYFLOAT 失败");
    assert!(
        (result - std::f64::consts::PI).abs() < 0.001,
        "结果应约为 π"
    );

    // 测试大数值
    let result = client
        .incrbyfloat("test_float", 1000.123)
        .await
        .expect("INCRBYFLOAT 失败");
    assert!(result > 1000.0, "结果应大于 1000");

    // 清理
    client
        .del(&["test_float".to_string(), "new_float".to_string()])
        .await
        .ok();
}

/// 测试 PSETEX 命令
#[tokio::test]
async fn test_psetex() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 设置键值，1000 毫秒后过期
    client
        .psetex("test_psetex", 1000, "temporary")
        .await
        .expect("PSETEX 失败");

    // 立即检查键是否存在
    let value = client.get("test_psetex").await.expect("GET 失败");
    assert_eq!(value, Some("temporary".to_string()), "键应存在且值正确");

    // 检查 TTL（应该小于等于 1000 毫秒，即 1 秒）
    let ttl = client.ttl("test_psetex").await.expect("TTL 失败");
    assert!(ttl > 0 && ttl <= 1, "TTL 应在 0-1 秒之间");

    // 等待过期
    tokio::time::sleep(tokio::time::Duration::from_millis(1100)).await;

    // 检查键是否已过期
    let value = client.get("test_psetex").await.expect("GET 失败");
    assert_eq!(value, None, "键应已过期");

    // 测试短过期时间（100 毫秒）
    client
        .psetex("test_psetex_short", 100, "very_temporary")
        .await
        .expect("PSETEX 失败");

    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

    let value = client.get("test_psetex_short").await.expect("GET 失败");
    assert_eq!(value, None, "键应已过期");
}

/// 测试 String 操作的边界情况
#[tokio::test]
async fn test_string_ops_edge_cases() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 测试空字符串
    client.set("empty_key", "").await.expect("SET 失败");
    let value = client.get("empty_key").await.expect("GET 失败");
    assert_eq!(value, Some("".to_string()), "空字符串应正确存储");

    // 测试 GETRANGE 空字符串
    let substr = client
        .getrange("empty_key", 0, 10)
        .await
        .expect("GETRANGE 失败");
    assert_eq!(substr, "", "空字符串的子串应为空");

    // 测试 SETRANGE 偏移量为 0
    client
        .setrange("empty_key", 0, "A")
        .await
        .expect("SETRANGE 失败");
    let value = client.get("empty_key").await.expect("GET 失败");
    assert_eq!(value, Some("A".to_string()), "应正确设置");

    // 测试 INCRBYFLOAT 零增量
    client.set("zero_test", "5.5").await.expect("SET 失败");
    let result = client
        .incrbyfloat("zero_test", 0.0)
        .await
        .expect("INCRBYFLOAT 失败");
    assert!((result - 5.5).abs() < 0.001, "零增量应保持原值");

    // 清理
    client
        .del(&["empty_key".to_string(), "zero_test".to_string()])
        .await
        .ok();
}

/// 测试 String 操作的并发安全性
#[tokio::test]
async fn test_string_ops_concurrent() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 初始化计数器
    client.set("concurrent_float", "0").await.expect("SET 失败");

    let mut handles = vec![];

    // 创建 10 个并发任务，每个增加 1.0
    for _ in 0..10 {
        let client_clone = client.clone();
        let handle =
            tokio::spawn(async move { client_clone.incrbyfloat("concurrent_float", 1.0).await });
        handles.push(handle);
    }

    // 等待所有任务完成
    for handle in handles {
        handle
            .await
            .expect("任务执行失败")
            .expect("INCRBYFLOAT 失败");
    }

    // 验证最终结果
    let value = client
        .get("concurrent_float")
        .await
        .expect("GET 失败")
        .expect("键应存在");
    let final_value: f64 = value.parse().expect("解析失败");
    assert!((final_value - 10.0).abs() < 0.001, "最终值应为 10.0");

    // 清理
    client.del(&["concurrent_float".to_string()]).await.ok();
}
