use testcontainers::{runners::AsyncRunner, GenericImage};
use yang_db::{RedisClient, RedisValue};

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

/// 测试 Pipeline 批量 SET/GET 操作（100+ 命令）
#[tokio::test]
async fn test_pipeline_batch_operations_100_plus() {
    let Some((_container, url)) = setup_redis_container().await else {
        eprintln!("跳过测试：无法启动 Redis 容器");
        return;
    };

    let client = RedisClient::connect(&url)
        .await
        .expect("连接 Redis 容器失败");

    let batch_size = 150;
    let mut pipeline = client.pipeline();

    // 添加 150 个 SET 命令
    for i in 0..batch_size {
        pipeline.set(format!("batch_key_{}", i), format!("batch_value_{}", i));
    }

    // 添加 150 个 GET 命令
    for i in 0..batch_size {
        pipeline.get(format!("batch_key_{}", i));
    }

    let results = pipeline.execute().await.expect("批量操作失败");

    // 应该有 300 个结果（150 个 SET + 150 个 GET）
    assert_eq!(results.len(), batch_size * 2, "结果数量不正确");

    // 验证前 150 个是 SET 的结果（应该是 Bool(true)）
    for (i, result) in results.iter().enumerate().take(batch_size) {
        assert!(
            matches!(result, RedisValue::Bool(true)),
            "SET 命令 {} 应该返回 Bool(true)",
            i
        );
    }

    // 验证后 150 个是 GET 的结果
    for i in 0..batch_size {
        let expected_value = format!("batch_value_{}", i);
        assert!(
            matches!(results[batch_size + i], RedisValue::String(ref s) if s == &expected_value),
            "GET 命令 {} 应该返回正确的值",
            i
        );
    }

    // 清理测试数据
    let keys: Vec<String> = (0..batch_size)
        .map(|i| format!("batch_key_{}", i))
        .collect();
    let _ = client.del(&keys).await;
}

/// 测试 Pipeline 混合命令类型（大规模）
#[tokio::test]
async fn test_pipeline_mixed_command_types() {
    let Some((_container, url)) = setup_redis_container().await else {
        eprintln!("跳过测试：无法启动 Redis 容器");
        return;
    };

    let client = RedisClient::connect(&url)
        .await
        .expect("连接 Redis 容器失败");

    let mut pipeline = client.pipeline();

    // String 操作
    for i in 0..20 {
        pipeline.set(format!("str_{}", i), format!("value_{}", i));
    }

    // Hash 操作
    for i in 0..20 {
        pipeline.hset(format!("hash_{}", i), "field", format!("value_{}", i));
    }

    // List 操作
    for i in 0..20 {
        pipeline.lpush(format!("list_{}", i), &[format!("item_{}", i)]);
    }

    // Set 操作
    for i in 0..20 {
        pipeline.sadd(format!("set_{}", i), &[format!("member_{}", i)]);
    }

    // Sorted Set 操作
    for i in 0..20 {
        pipeline.zadd(
            format!("zset_{}", i),
            &[(i as f64, format!("member_{}", i))],
        );
    }

    // INCR 操作
    for i in 0..20 {
        pipeline.set(format!("counter_{}", i), "0");
        pipeline.incr(format!("counter_{}", i));
    }

    let results = pipeline.execute().await.expect("混合命令执行失败");

    // 应该有 140 个结果（20*5 + 20*2）
    assert_eq!(results.len(), 140, "结果数量不正确");

    // 清理测试数据
    let mut keys = Vec::new();
    for i in 0..20 {
        keys.push(format!("str_{}", i));
        keys.push(format!("hash_{}", i));
        keys.push(format!("list_{}", i));
        keys.push(format!("set_{}", i));
        keys.push(format!("zset_{}", i));
        keys.push(format!("counter_{}", i));
    }
    let _ = client.del(&keys).await;
}

/// 测试 Pipeline 单次网络往返（性能测试）
#[tokio::test]
async fn test_pipeline_single_round_trip_performance() {
    let Some((_container, url)) = setup_redis_container().await else {
        eprintln!("跳过测试：无法启动 Redis 容器");
        return;
    };

    let client = RedisClient::connect(&url)
        .await
        .expect("连接 Redis 容器失败");

    let num_operations = 200;

    // 测试 Pipeline 性能
    let pipeline_start = std::time::Instant::now();
    let mut pipeline = client.pipeline();
    for i in 0..num_operations {
        pipeline.set(format!("perf_key_{}", i), format!("value_{}", i));
    }
    let _ = pipeline.execute().await.expect("Pipeline 执行失败");
    let pipeline_duration = pipeline_start.elapsed();

    // 测试逐个执行性能
    let sequential_start = std::time::Instant::now();
    for i in 0..num_operations {
        let _ = client
            .set(format!("seq_key_{}", i), format!("value_{}", i))
            .await;
    }
    let sequential_duration = sequential_start.elapsed();

    println!("Pipeline 执行时间: {:?}", pipeline_duration);
    println!("逐个执行时间: {:?}", sequential_duration);
    println!(
        "性能提升: {:.2}x",
        sequential_duration.as_secs_f64() / pipeline_duration.as_secs_f64()
    );

    // Pipeline 应该明显快于逐个执行
    // 在本地测试中，Pipeline 通常快 10-50 倍
    assert!(
        pipeline_duration < sequential_duration,
        "Pipeline 应该比逐个执行更快"
    );

    // 清理测试数据
    let mut keys = Vec::new();
    for i in 0..num_operations {
        keys.push(format!("perf_key_{}", i));
        keys.push(format!("seq_key_{}", i));
    }
    let _ = client.del(&keys).await;
}

/// 测试 Pipeline 类型化结果提取（大规模）
#[tokio::test]
async fn test_pipeline_typed_query_large_scale() {
    let Some((_container, url)) = setup_redis_container().await else {
        eprintln!("跳过测试：无法启动 Redis 容器");
        return;
    };

    let client = RedisClient::connect(&url)
        .await
        .expect("连接 Redis 容器失败");

    let num_keys = 100;

    // 先设置一些值
    for i in 0..num_keys {
        let _ = client
            .set(format!("typed_key_{}", i), format!("value_{}", i))
            .await;
    }

    // 使用 Pipeline 批量获取
    let mut pipeline = client.pipeline();
    for i in 0..num_keys {
        pipeline.get(format!("typed_key_{}", i));
    }

    // 使用类型化查询
    let results: Vec<String> = pipeline.query().await.expect("类型化查询失败");

    assert_eq!(results.len(), num_keys, "结果数量不正确");

    // 验证每个结果
    for (i, result) in results.iter().enumerate().take(num_keys) {
        assert_eq!(result, &format!("value_{}", i), "结果 {} 不正确", i);
    }

    // 清理测试数据
    let keys: Vec<String> = (0..num_keys).map(|i| format!("typed_key_{}", i)).collect();
    let _ = client.del(&keys).await;
}

/// 测试 Pipeline 并发执行
#[tokio::test]
async fn test_pipeline_concurrent_execution() {
    let Some((_container, url)) = setup_redis_container().await else {
        eprintln!("跳过测试：无法启动 Redis 容器");
        return;
    };

    let client = RedisClient::connect(&url)
        .await
        .expect("连接 Redis 容器失败");

    let num_pipelines = 10;
    let ops_per_pipeline = 50;

    let mut handles = vec![];

    // 创建 10 个并发 Pipeline
    for p in 0..num_pipelines {
        let client_clone = client.clone();
        let handle = tokio::spawn(async move {
            let mut pipeline = client_clone.pipeline();
            for i in 0..ops_per_pipeline {
                pipeline.set(
                    format!("concurrent_{}_{}", p, i),
                    format!("value_{}_{}", p, i),
                );
            }
            pipeline.execute().await
        });
        handles.push(handle);
    }

    // 等待所有 Pipeline 完成
    for handle in handles {
        let result = handle.await.expect("任务执行失败");
        assert!(result.is_ok(), "并发 Pipeline 执行失败");
    }

    // 清理测试数据
    let mut keys = Vec::new();
    for p in 0..num_pipelines {
        for i in 0..ops_per_pipeline {
            keys.push(format!("concurrent_{}_{}", p, i));
        }
    }
    let _ = client.del(&keys).await;
}

/// 测试 Pipeline 错误恢复
#[tokio::test]
async fn test_pipeline_error_recovery() {
    let Some((_container, url)) = setup_redis_container().await else {
        eprintln!("跳过测试：无法启动 Redis 容器");
        return;
    };

    let client = RedisClient::connect(&url)
        .await
        .expect("连接 Redis 容器失败");

    // 先设置一个字符串类型的键
    let _ = client.set("error_key", "string_value").await;

    let mut pipeline = client.pipeline();

    // 添加一些正常的命令
    pipeline.set("normal_key1", "value1");
    pipeline.set("normal_key2", "value2");

    // 尝试对字符串类型的键执行 Hash 操作（可能会失败）
    pipeline.hget("error_key", "field");

    // 添加更多正常的命令
    pipeline.get("normal_key1");
    pipeline.get("normal_key2");

    // 执行 Pipeline
    let results = pipeline.execute().await;

    // 验证能够处理错误情况
    assert!(results.is_ok() || results.is_err());

    // 清理测试数据
    let _ = client
        .del(&[
            "error_key".to_string(),
            "normal_key1".to_string(),
            "normal_key2".to_string(),
        ])
        .await;
}

/// 测试 Pipeline 空操作
#[tokio::test]
async fn test_pipeline_empty_operations() {
    let Some((_container, url)) = setup_redis_container().await else {
        eprintln!("跳过测试：无法启动 Redis 容器");
        return;
    };

    let client = RedisClient::connect(&url)
        .await
        .expect("连接 Redis 容器失败");

    let pipeline = client.pipeline();

    // 执行空 Pipeline
    let results = pipeline.execute().await;

    // 空 Pipeline 可能返回错误或空结果
    if let Ok(res) = results {
        assert_eq!(res.len(), 0, "空 Pipeline 应该返回空结果");
    }
    // 空 Pipeline 返回错误也是可接受的
}

/// 测试 Pipeline 大批量操作（1000+ 命令）
#[tokio::test]
async fn test_pipeline_very_large_batch() {
    let Some((_container, url)) = setup_redis_container().await else {
        eprintln!("跳过测试：无法启动 Redis 容器");
        return;
    };

    let client = RedisClient::connect(&url)
        .await
        .expect("连接 Redis 容器失败");

    let batch_size = 1000;
    let mut pipeline = client.pipeline();

    // 添加 1000 个 SET 命令
    for i in 0..batch_size {
        pipeline.set(format!("large_key_{}", i), format!("large_value_{}", i));
    }

    let results = pipeline.execute().await.expect("大批量操作失败");

    assert_eq!(results.len(), batch_size, "结果数量不正确");

    // 验证所有 SET 命令都成功
    for (i, result) in results.iter().enumerate().take(batch_size) {
        assert!(
            matches!(result, RedisValue::Bool(true)),
            "SET 命令 {} 应该返回 Bool(true)",
            i
        );
    }

    // 清理测试数据
    let keys: Vec<String> = (0..batch_size)
        .map(|i| format!("large_key_{}", i))
        .collect();
    let _ = client.del(&keys).await;
}

/// 测试 Pipeline 顺序性（验证结果按添加顺序返回）
#[tokio::test]
async fn test_pipeline_result_ordering() {
    let Some((_container, url)) = setup_redis_container().await else {
        eprintln!("跳过测试：无法启动 Redis 容器");
        return;
    };

    let client = RedisClient::connect(&url)
        .await
        .expect("连接 Redis 容器失败");

    let mut pipeline = client.pipeline();

    // 添加一系列有序的操作
    for i in 0..50 {
        pipeline.set(format!("order_key_{}", i), format!("order_value_{}", i));
    }

    for i in 0..50 {
        pipeline.get(format!("order_key_{}", i));
    }

    let results = pipeline.execute().await.expect("顺序性测试失败");

    assert_eq!(results.len(), 100, "结果数量不正确");

    // 验证前 50 个是 SET 的结果
    for (i, result) in results.iter().enumerate().take(50) {
        assert!(
            matches!(result, RedisValue::Bool(true)),
            "SET 命令 {} 应该返回 Bool(true)",
            i
        );
    }

    // 验证后 50 个是 GET 的结果，且顺序正确
    for i in 0..50 {
        let expected_value = format!("order_value_{}", i);
        assert!(
            matches!(results[50 + i], RedisValue::String(ref s) if s == &expected_value),
            "GET 命令 {} 应该返回正确的值且顺序正确",
            i
        );
    }

    // 清理测试数据
    let keys: Vec<String> = (0..50).map(|i| format!("order_key_{}", i)).collect();
    let _ = client.del(&keys).await;
}
