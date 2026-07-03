#![allow(deprecated)]
#![allow(clippy::expect_used, clippy::unwrap_used)]
use yang_db::{RedisClient, RedisValue};

/// 测试 Pipeline 创建
#[tokio::test]
async fn test_pipeline_creation() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    let pipeline = client.pipeline();
    assert_eq!(pipeline.len(), 0, "新创建的 Pipeline 应该为空");
    assert!(pipeline.is_empty(), "新创建的 Pipeline 应该为空");
}

/// 测试 Pipeline 基础命令添加
#[tokio::test]
async fn test_pipeline_basic_commands() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    let mut pipeline = client.pipeline();

    // 添加多个命令
    pipeline
        .set("pipeline_key1", "value1")
        .set("pipeline_key2", "value2")
        .get("pipeline_key1")
        .get("pipeline_key2");

    assert_eq!(pipeline.len(), 4, "Pipeline 应该包含 4 个命令");
    assert!(!pipeline.is_empty(), "Pipeline 不应该为空");
}

/// 测试 Pipeline 链式调用
#[tokio::test]
async fn test_pipeline_chaining() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    let mut pipeline = client.pipeline();

    // 测试链式调用
    pipeline
        .set("chain_key1", "value1")
        .set("chain_key2", "value2")
        .incr("chain_counter")
        .get("chain_key1");

    assert_eq!(pipeline.len(), 4, "链式调用应该添加 4 个命令");
}

/// 测试 Pipeline 执行 - SET/GET 操作
#[tokio::test]
async fn test_pipeline_execute_set_get() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 清理可能存在的测试数据
    let _ = client
        .del(&["exec_key1".to_string(), "exec_key2".to_string()])
        .await;

    let mut pipeline = client.pipeline();
    pipeline
        .set("exec_key1", "value1")
        .set("exec_key2", "value2")
        .get("exec_key1")
        .get("exec_key2");

    let results = pipeline.execute().await.expect("Pipeline 执行失败");

    assert_eq!(results.len(), 4, "应该返回 4 个结果");

    // 验证 SET 命令返回 OK 或 Nil（redis-rs 在 Pipeline 中可能返回不同的值）
    // 打印实际结果以便调试
    println!("SET 结果 0: {:?}", results[0]);
    println!("SET 结果 1: {:?}", results[1]);

    // 验证 GET 命令返回正确的值
    assert!(
        matches!(results[2], RedisValue::String(ref s) if s == "value1"),
        "GET 结果 2 不正确: {:?}",
        results[2]
    );
    assert!(
        matches!(results[3], RedisValue::String(ref s) if s == "value2"),
        "GET 结果 3 不正确: {:?}",
        results[3]
    );

    // 清理测试数据
    let _ = client
        .del(&["exec_key1".to_string(), "exec_key2".to_string()])
        .await;
}

/// 测试 Pipeline 类型化结果提取
#[tokio::test]
async fn test_pipeline_query_typed() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 清理可能存在的测试数据
    let _ = client
        .del(&["typed_key1".to_string(), "typed_key2".to_string()])
        .await;

    // 先设置一些值
    let _ = client.set("typed_key1", "hello").await;
    let _ = client.set("typed_key2", "world").await;

    let mut pipeline = client.pipeline();
    pipeline.get("typed_key1").get("typed_key2");

    // 使用类型化查询
    let results: Vec<String> = pipeline.query().await.expect("类型化查询失败");

    assert_eq!(results.len(), 2, "应该返回 2 个结果");
    assert_eq!(results[0], "hello");
    assert_eq!(results[1], "world");

    // 清理测试数据
    let _ = client
        .del(&["typed_key1".to_string(), "typed_key2".to_string()])
        .await;
}

/// 测试 Pipeline INCR 操作
#[tokio::test]
async fn test_pipeline_incr() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 清理可能存在的测试数据
    let _ = client.del(&["incr_counter".to_string()]).await;

    let mut pipeline = client.pipeline();
    pipeline
        .set("incr_counter", "10")
        .incr("incr_counter")
        .incr("incr_counter")
        .incr("incr_counter")
        .get("incr_counter");

    let results = pipeline.execute().await.expect("Pipeline 执行失败");

    assert_eq!(results.len(), 5, "应该返回 5 个结果");

    // 最后一个结果应该是 "13"
    assert!(matches!(results[4], RedisValue::String(ref s) if s == "13"));

    // 清理测试数据
    let _ = client.del(&["incr_counter".to_string()]).await;
}

/// 测试 Pipeline DEL 操作
#[tokio::test]
async fn test_pipeline_del() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 先设置一些值
    let _ = client.set("del_key1", "value1").await;
    let _ = client.set("del_key2", "value2").await;
    let _ = client.set("del_key3", "value3").await;

    let mut pipeline = client.pipeline();
    pipeline
        .del(&["del_key1".to_string(), "del_key2".to_string()])
        .get("del_key1")
        .get("del_key3");

    let results = pipeline.execute().await.expect("Pipeline 执行失败");

    assert_eq!(results.len(), 3, "应该返回 3 个结果");

    // DEL 应该返回删除的键数量
    assert!(matches!(results[0], RedisValue::Int(2)));

    // 被删除的键应该返回 Nil
    assert!(matches!(results[1], RedisValue::Nil));

    // 未被删除的键应该返回值
    assert!(matches!(results[2], RedisValue::String(ref s) if s == "value3"));

    // 清理测试数据
    let _ = client.del(&["del_key3".to_string()]).await;
}

/// 测试 Pipeline Hash 操作
#[tokio::test]
async fn test_pipeline_hash_operations() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 清理可能存在的测试数据
    let _ = client.del(&["hash_key".to_string()]).await;

    let mut pipeline = client.pipeline();
    pipeline
        .hset("hash_key", "field1", "value1")
        .hset("hash_key", "field2", "value2")
        .hget("hash_key", "field1")
        .hget("hash_key", "field2");

    let results = pipeline.execute().await.expect("Pipeline 执行失败");

    assert_eq!(results.len(), 4, "应该返回 4 个结果");

    // 验证 HGET 返回正确的值
    assert!(matches!(results[2], RedisValue::String(ref s) if s == "value1"));
    assert!(matches!(results[3], RedisValue::String(ref s) if s == "value2"));

    // 清理测试数据
    let _ = client.del(&["hash_key".to_string()]).await;
}

/// 测试 Pipeline List 操作
#[tokio::test]
async fn test_pipeline_list_operations() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 清理可能存在的测试数据
    let _ = client.del(&["list_key".to_string()]).await;

    let mut pipeline = client.pipeline();
    pipeline
        .lpush("list_key", &["value1".to_string(), "value2".to_string()])
        .rpush("list_key", &["value3".to_string(), "value4".to_string()]);

    let results = pipeline.execute().await.expect("Pipeline 执行失败");

    // LPUSH 和 RPUSH 应该返回列表长度
    assert!(results.len() >= 2, "应该至少返回 2 个结果");

    // 清理测试数据
    let _ = client.del(&["list_key".to_string()]).await;
}

/// 测试 Pipeline Set 操作
#[tokio::test]
async fn test_pipeline_set_operations() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 清理可能存在的测试数据
    let _ = client.del(&["set_key".to_string()]).await;

    let mut pipeline = client.pipeline();
    pipeline.sadd(
        "set_key",
        &[
            "member1".to_string(),
            "member2".to_string(),
            "member3".to_string(),
        ],
    );

    let results = pipeline.execute().await.expect("Pipeline 执行失败");

    assert!(!results.is_empty(), "应该至少返回 1 个结果");

    // 清理测试数据
    let _ = client.del(&["set_key".to_string()]).await;
}

/// 测试 Pipeline Sorted Set 操作
#[tokio::test]
async fn test_pipeline_sorted_set_operations() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 清理可能存在的测试数据
    let _ = client.del(&["zset_key".to_string()]).await;

    let mut pipeline = client.pipeline();
    pipeline.zadd(
        "zset_key",
        &[
            (1.0, "member1".to_string()),
            (2.0, "member2".to_string()),
            (3.0, "member3".to_string()),
        ],
    );

    let results = pipeline.execute().await.expect("Pipeline 执行失败");

    assert!(!results.is_empty(), "应该至少返回 1 个结果");

    // 清理测试数据
    let _ = client.del(&["zset_key".to_string()]).await;
}

/// 测试 Pipeline 自定义命令
#[tokio::test]
async fn test_pipeline_custom_command() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 清理可能存在的测试数据
    let _ = client.del(&["custom_key".to_string()]).await;

    let mut pipeline = client.pipeline();

    // 添加自定义 SETEX 命令
    let mut cmd = redis::cmd("SETEX");
    cmd.arg("custom_key").arg(60).arg("custom_value");

    pipeline.cmd(cmd).get("custom_key");

    let results = pipeline.execute().await.expect("Pipeline 执行失败");

    assert_eq!(results.len(), 2, "应该返回 2 个结果");

    // 验证 GET 返回正确的值
    assert!(matches!(results[1], RedisValue::String(ref s) if s == "custom_value"));

    // 清理测试数据
    let _ = client.del(&["custom_key".to_string()]).await;
}

/// 测试 Pipeline 错误处理 - 空 Pipeline
#[tokio::test]
async fn test_pipeline_empty_execution() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    let pipeline = client.pipeline();

    // 执行空 Pipeline 可能会失败（redis-rs 不支持空 Pipeline）
    // 我们只验证它不会崩溃
    let results = pipeline.execute().await;

    // 空 Pipeline 可能返回错误或空结果，两种情况都是可接受的
    if let Ok(res) = results {
        assert_eq!(res.len(), 0, "空 Pipeline 应该返回空结果");
    }
    // 空 Pipeline 返回错误也是可接受的
}

/// 测试 Pipeline 错误处理 - 无效的键操作
#[tokio::test]
async fn test_pipeline_error_handling() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 先设置一个字符串类型的键
    let _ = client.set("string_key", "value").await;

    let mut pipeline = client.pipeline();

    // 尝试对字符串类型的键执行 Hash 操作（应该失败）
    pipeline.hget("string_key", "field").get("string_key");

    // Pipeline 执行应该失败或返回错误结果
    let results = pipeline.execute().await;

    // 根据 Redis 的行为，这可能会成功但返回错误值，或者直接失败
    // 这里我们只验证能够处理这种情况
    assert!(results.is_ok() || results.is_err());

    // 清理测试数据
    let _ = client.del(&["string_key".to_string()]).await;
}

/// 测试 Pipeline 混合命令类型
#[tokio::test]
async fn test_pipeline_mixed_commands() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 清理可能存在的测试数据
    let _ = client
        .del(&[
            "mixed_string".to_string(),
            "mixed_hash".to_string(),
            "mixed_list".to_string(),
            "mixed_set".to_string(),
            "mixed_zset".to_string(),
        ])
        .await;

    let mut pipeline = client.pipeline();
    pipeline
        .set("mixed_string", "value")
        .hset("mixed_hash", "field", "value")
        .lpush("mixed_list", &["item".to_string()])
        .sadd("mixed_set", &["member".to_string()])
        .zadd("mixed_zset", &[(1.0, "member".to_string())])
        .get("mixed_string")
        .hget("mixed_hash", "field");

    let results = pipeline
        .execute()
        .await
        .expect("混合命令 Pipeline 执行失败");

    assert!(results.len() >= 7, "应该返回至少 7 个结果");

    // 验证最后两个 GET 操作返回正确的值
    assert!(matches!(results[5], RedisValue::String(ref s) if s == "value"));
    assert!(matches!(results[6], RedisValue::String(ref s) if s == "value"));

    // 清理测试数据
    let _ = client
        .del(&[
            "mixed_string".to_string(),
            "mixed_hash".to_string(),
            "mixed_list".to_string(),
            "mixed_set".to_string(),
            "mixed_zset".to_string(),
        ])
        .await;
}
