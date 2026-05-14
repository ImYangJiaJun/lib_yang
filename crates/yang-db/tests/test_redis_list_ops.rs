use yang_db::RedisClient;

/// 测试 LINSERT 命令
#[tokio::test]
async fn test_linsert() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 创建测试列表
    client
        .rpush("test_linsert", &["a".to_string(), "c".to_string()])
        .await
        .expect("RPUSH 失败");

    // 测试在元素前插入
    let len = client
        .linsert("test_linsert", "BEFORE", "c", "b")
        .await
        .expect("LINSERT 失败");
    assert_eq!(len, 3, "列表长度应为 3");

    // 验证列表内容
    let list = client
        .lrange("test_linsert", 0, -1)
        .await
        .expect("LRANGE 失败");
    assert_eq!(list, vec!["a", "b", "c"], "列表应为 ['a', 'b', 'c']");

    // 测试在元素后插入
    let len = client
        .linsert("test_linsert", "AFTER", "b", "b2")
        .await
        .expect("LINSERT 失败");
    assert_eq!(len, 4, "列表长度应为 4");

    let list = client
        .lrange("test_linsert", 0, -1)
        .await
        .expect("LRANGE 失败");
    assert_eq!(
        list,
        vec!["a", "b", "b2", "c"],
        "列表应为 ['a', 'b', 'b2', 'c']"
    );

    // 测试 pivot 不存在的情况
    let len = client
        .linsert("test_linsert", "BEFORE", "nonexistent", "x")
        .await
        .expect("LINSERT 失败");
    assert_eq!(len, -1, "pivot 不存在应返回 -1");

    // 测试不存在的键
    let len = client
        .linsert("nonexistent_list", "BEFORE", "a", "x")
        .await
        .expect("LINSERT 失败");
    assert_eq!(len, 0, "不存在的键应返回 0");

    // 清理
    client.del(&["test_linsert".to_string()]).await.ok();
}

/// 测试 LREM 命令
#[tokio::test]
async fn test_lrem() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 创建测试列表
    client
        .rpush(
            "test_lrem",
            &[
                "a".to_string(),
                "b".to_string(),
                "a".to_string(),
                "c".to_string(),
                "a".to_string(),
            ],
        )
        .await
        .expect("RPUSH 失败");

    // 测试从头删除 2 个 "a"
    let removed = client.lrem("test_lrem", 2, "a").await.expect("LREM 失败");
    assert_eq!(removed, 2, "应删除 2 个元素");

    let list = client
        .lrange("test_lrem", 0, -1)
        .await
        .expect("LRANGE 失败");
    assert_eq!(list, vec!["b", "c", "a"], "列表应为 ['b', 'c', 'a']");

    // 重新创建列表
    client.del(&["test_lrem".to_string()]).await.ok();
    client
        .rpush(
            "test_lrem",
            &[
                "a".to_string(),
                "b".to_string(),
                "a".to_string(),
                "c".to_string(),
                "a".to_string(),
            ],
        )
        .await
        .expect("RPUSH 失败");

    // 测试从尾删除 2 个 "a"
    let removed = client.lrem("test_lrem", -2, "a").await.expect("LREM 失败");
    assert_eq!(removed, 2, "应删除 2 个元素");

    let list = client
        .lrange("test_lrem", 0, -1)
        .await
        .expect("LRANGE 失败");
    assert_eq!(list, vec!["a", "b", "c"], "列表应为 ['a', 'b', 'c']");

    // 重新创建列表
    client.del(&["test_lrem".to_string()]).await.ok();
    client
        .rpush(
            "test_lrem",
            &[
                "a".to_string(),
                "b".to_string(),
                "a".to_string(),
                "c".to_string(),
                "a".to_string(),
            ],
        )
        .await
        .expect("RPUSH 失败");

    // 测试删除所有 "a"
    let removed = client.lrem("test_lrem", 0, "a").await.expect("LREM 失败");
    assert_eq!(removed, 3, "应删除 3 个元素");

    let list = client
        .lrange("test_lrem", 0, -1)
        .await
        .expect("LRANGE 失败");
    assert_eq!(list, vec!["b", "c"], "列表应为 ['b', 'c']");

    // 测试删除不存在的元素
    let removed = client.lrem("test_lrem", 0, "x").await.expect("LREM 失败");
    assert_eq!(removed, 0, "不存在的元素应返回 0");

    // 清理
    client.del(&["test_lrem".to_string()]).await.ok();
}

/// 测试 RPOPLPUSH 命令
#[tokio::test]
async fn test_rpoplpush() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 创建源列表
    client
        .rpush(
            "test_source",
            &["a".to_string(), "b".to_string(), "c".to_string()],
        )
        .await
        .expect("RPUSH 失败");

    // 测试移动元素
    let elem = client
        .rpoplpush("test_source", "test_dest")
        .await
        .expect("RPOPLPUSH 失败");
    assert_eq!(elem, Some("c".to_string()), "应移动元素 'c'");

    // 验证源列表
    let source = client
        .lrange("test_source", 0, -1)
        .await
        .expect("LRANGE 失败");
    assert_eq!(source, vec!["a", "b"], "源列表应为 ['a', 'b']");

    // 验证目标列表
    let dest = client
        .lrange("test_dest", 0, -1)
        .await
        .expect("LRANGE 失败");
    assert_eq!(dest, vec!["c"], "目标列表应为 ['c']");

    // 再次移动
    let elem = client
        .rpoplpush("test_source", "test_dest")
        .await
        .expect("RPOPLPUSH 失败");
    assert_eq!(elem, Some("b".to_string()), "应移动元素 'b'");

    let dest = client
        .lrange("test_dest", 0, -1)
        .await
        .expect("LRANGE 失败");
    assert_eq!(dest, vec!["b", "c"], "目标列表应为 ['b', 'c']");

    // 测试空列表
    client.del(&["test_source".to_string()]).await.ok();
    let elem = client
        .rpoplpush("test_source", "test_dest")
        .await
        .expect("RPOPLPUSH 失败");
    assert_eq!(elem, None, "空列表应返回 None");

    // 测试同一列表（循环移动）
    client
        .rpush(
            "test_circular",
            &["1".to_string(), "2".to_string(), "3".to_string()],
        )
        .await
        .expect("RPUSH 失败");

    let elem = client
        .rpoplpush("test_circular", "test_circular")
        .await
        .expect("RPOPLPUSH 失败");
    assert_eq!(elem, Some("3".to_string()), "应移动元素 '3'");

    let list = client
        .lrange("test_circular", 0, -1)
        .await
        .expect("LRANGE 失败");
    assert_eq!(list, vec!["3", "1", "2"], "列表应为 ['3', '1', '2']");

    // 清理
    client
        .del(&[
            "test_source".to_string(),
            "test_dest".to_string(),
            "test_circular".to_string(),
        ])
        .await
        .ok();
}

/// 测试 BLPOP 命令
#[tokio::test]
async fn test_blpop() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 清理之前的测试数据
    client
        .del(&["test_blpop".to_string(), "test_blpop2".to_string()])
        .await
        .ok();

    // 创建测试列表
    client
        .rpush("test_blpop", &["a".to_string(), "b".to_string()])
        .await
        .expect("RPUSH 失败");

    // 测试立即弹出（列表非空）
    let result = client
        .blpop(&["test_blpop".to_string()], 0)
        .await
        .expect("BLPOP 失败");
    assert_eq!(
        result,
        Some(("test_blpop".to_string(), "a".to_string())),
        "应弹出元素 'a'"
    );

    // 测试多个键（按顺序检查）
    client
        .rpush("test_blpop2", &["x".to_string()])
        .await
        .expect("RPUSH 失败");

    let result = client
        .blpop(&["nonexistent".to_string(), "test_blpop2".to_string()], 0)
        .await
        .expect("BLPOP 失败");
    assert_eq!(
        result,
        Some(("test_blpop2".to_string(), "x".to_string())),
        "应从第二个键弹出"
    );

    // 注意：不测试超时情况，因为会导致连接池超时
    // 如需测试超时，请使用更长的连接池超时设置

    // 清理
    client
        .del(&["test_blpop".to_string(), "test_blpop2".to_string()])
        .await
        .ok();
}

/// 测试 BRPOP 命令
#[tokio::test]
async fn test_brpop() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 清理之前的测试数据
    client
        .del(&["test_brpop".to_string(), "test_brpop2".to_string()])
        .await
        .ok();

    // 创建测试列表
    client
        .rpush("test_brpop", &["a".to_string(), "b".to_string()])
        .await
        .expect("RPUSH 失败");

    // 测试立即弹出（列表非空）
    let result = client
        .brpop(&["test_brpop".to_string()], 0)
        .await
        .expect("BRPOP 失败");
    assert_eq!(
        result,
        Some(("test_brpop".to_string(), "b".to_string())),
        "应弹出元素 'b'"
    );

    // 测试多个键
    client
        .rpush("test_brpop2", &["y".to_string()])
        .await
        .expect("RPUSH 失败");

    let result = client
        .brpop(&["nonexistent".to_string(), "test_brpop2".to_string()], 0)
        .await
        .expect("BRPOP 失败");
    assert_eq!(
        result,
        Some(("test_brpop2".to_string(), "y".to_string())),
        "应从第二个键弹出"
    );

    // 注意：不测试超时情况，因为会导致连接池超时
    // 如需测试超时，请使用更长的连接池超时设置

    // 清理
    client
        .del(&["test_brpop".to_string(), "test_brpop2".to_string()])
        .await
        .ok();
}

/// 测试 List 操作的边界情况
#[tokio::test]
async fn test_list_ops_edge_cases() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 测试空列表的 LINSERT
    let len = client
        .linsert("empty_list", "BEFORE", "a", "x")
        .await
        .expect("LINSERT 失败");
    assert_eq!(len, 0, "空列表应返回 0");

    // 测试空列表的 LREM
    let removed = client.lrem("empty_list", 0, "a").await.expect("LREM 失败");
    assert_eq!(removed, 0, "空列表应返回 0");

    // 测试空列表的 RPOPLPUSH
    let elem = client
        .rpoplpush("empty_list", "dest")
        .await
        .expect("RPOPLPUSH 失败");
    assert_eq!(elem, None, "空列表应返回 None");

    // 测试单元素列表
    client
        .rpush("single_elem", &["only".to_string()])
        .await
        .expect("RPUSH 失败");

    let elem = client
        .rpoplpush("single_elem", "dest")
        .await
        .expect("RPOPLPUSH 失败");
    assert_eq!(elem, Some("only".to_string()), "应移动唯一元素");

    let len = client.llen("single_elem").await.expect("LLEN 失败");
    assert_eq!(len, 0, "源列表应为空");

    // 清理
    client
        .del(&[
            "empty_list".to_string(),
            "single_elem".to_string(),
            "dest".to_string(),
        ])
        .await
        .ok();
}

/// 测试 List 操作的并发安全性
#[tokio::test]
async fn test_list_ops_concurrent() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    let mut handles = vec![];

    // 创建 10 个并发任务，每个向列表添加元素
    for i in 0..10 {
        let client_clone = client.clone();
        let handle = tokio::spawn(async move {
            client_clone
                .rpush("concurrent_list", &[format!("item_{}", i)])
                .await
        });
        handles.push(handle);
    }

    // 等待所有任务完成
    for handle in handles {
        handle.await.expect("任务执行失败").expect("RPUSH 失败");
    }

    // 验证列表长度
    let len = client.llen("concurrent_list").await.expect("LLEN 失败");
    assert_eq!(len, 10, "列表应包含 10 个元素");

    // 清理
    client.del(&["concurrent_list".to_string()]).await.ok();
}

/// 测试阻塞操作的超时行为
///
/// 注意：由于连接池的超时设置，此测试使用较短的超时时间
#[tokio::test]
#[ignore] // 此测试需要较长时间，默认忽略
async fn test_blocking_ops_timeout() {
    use yang_db::RedisConfig;
    // 使用更长的超时设置
    let config = RedisConfig::new(10, 60, 60, false);
    let client = RedisClient::connect_with_config("redis://127.0.0.1:6379", config)
        .await
        .expect("连接失败");

    // 测试 BLPOP 超时
    let start = std::time::Instant::now();
    let result = client
        .blpop(&["timeout_test".to_string()], 1)
        .await
        .expect("BLPOP 失败");
    let elapsed = start.elapsed();

    assert_eq!(result, None, "应超时返回 None");
    assert!(
        elapsed.as_secs() >= 1 && elapsed.as_secs() <= 2,
        "超时时间应约为 1 秒"
    );

    // 测试 BRPOP 超时
    let start = std::time::Instant::now();
    let result = client
        .brpop(&["timeout_test".to_string()], 1)
        .await
        .expect("BRPOP 失败");
    let elapsed = start.elapsed();

    assert_eq!(result, None, "应超时返回 None");
    assert!(
        elapsed.as_secs() >= 1 && elapsed.as_secs() <= 2,
        "超时时间应约为 1 秒"
    );
}
