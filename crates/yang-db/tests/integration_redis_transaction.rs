use yang_db::RedisClient;

/// 测试基础事务执行
///
/// 验证事务能够正确执行多个命令并返回结果
#[tokio::test]
async fn test_basic_transaction() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 清理测试数据
    let _ = client
        .del(&["tx_key1".to_string(), "tx_key2".to_string()])
        .await;

    // 执行事务：设置两个键
    let mut tx = client.transaction();
    tx.set("tx_key1", "value1").set("tx_key2", "value2");

    let result: (String, String) = tx.exec().await.expect("事务执行失败");

    // 验证结果
    assert_eq!(result.0, "OK");
    assert_eq!(result.1, "OK");

    // 验证数据已写入
    let value1 = client.get("tx_key1").await.expect("获取 tx_key1 失败");
    let value2 = client.get("tx_key2").await.expect("获取 tx_key2 失败");
    assert_eq!(value1, Some("value1".to_string()));
    assert_eq!(value2, Some("value2".to_string()));

    // 清理测试数据
    let _ = client
        .del(&["tx_key1".to_string(), "tx_key2".to_string()])
        .await;
}

/// 测试事务中的计数器操作
///
/// 验证事务能够正确执行 INCR 等数值操作
#[tokio::test]
async fn test_transaction_counter() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 清理测试数据
    let _ = client.del(&["tx_counter".to_string()]).await;

    // 初始化计数器
    client.set("tx_counter", "0").await.expect("设置计数器失败");

    // 执行事务：增加计数器 3 次
    let mut tx = client.transaction();
    tx.incr("tx_counter").incr("tx_counter").incr("tx_counter");

    let result: (i64, i64, i64) = tx.exec().await.expect("事务执行失败");

    // 验证结果
    assert_eq!(result.0, 1);
    assert_eq!(result.1, 2);
    assert_eq!(result.2, 3);

    // 验证最终值
    let final_value = client.get("tx_counter").await.expect("获取计数器失败");
    assert_eq!(final_value, Some("3".to_string()));

    // 清理测试数据
    let _ = client.del(&["tx_counter".to_string()]).await;
}

/// 测试 WATCH 键未修改的成功场景
///
/// 验证当 WATCH 的键未被修改时，事务能够成功执行
#[tokio::test]
async fn test_transaction_watch_success() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 清理测试数据
    let _ = client.del(&["watched_key".to_string()]).await;

    // 初始化被监视的键
    client
        .set("watched_key", "100")
        .await
        .expect("设置 watched_key 失败");

    // 读取当前值
    let value: String = client
        .get("watched_key")
        .await
        .expect("获取 watched_key 失败")
        .unwrap();
    let current: i64 = value.parse().unwrap_or(0);

    // 执行事务：监视 watched_key，设置新值
    let mut tx = client.transaction();
    tx.watch(&["watched_key".to_string()]);
    tx.set("watched_key", (current + 50).to_string());

    let result: (String,) = tx.exec().await.expect("事务执行失败");

    // 验证结果
    assert_eq!(result.0, "OK");

    // 验证最终值
    let final_value = client
        .get("watched_key")
        .await
        .expect("获取 watched_key 失败");
    assert_eq!(final_value, Some("150".to_string()));

    // 清理测试数据
    let _ = client.del(&["watched_key".to_string()]).await;
}

/// 测试乐观锁实现：余额扣减
///
/// 验证使用 WATCH 机制实现乐观锁的余额扣减场景
#[tokio::test]
async fn test_transaction_optimistic_lock_balance() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 清理测试数据
    let _ = client.del(&["balance".to_string()]).await;

    // 初始化余额
    client.set("balance", "1000").await.expect("设置余额失败");

    // 读取当前余额
    let balance_str: String = client.get("balance").await.expect("获取余额失败").unwrap();
    let balance: i64 = balance_str.parse().unwrap_or(0);

    // 检查余额是否足够
    if balance >= 100 {
        // 执行事务：扣减余额
        let mut tx = client.transaction();
        tx.watch(&["balance".to_string()]);
        tx.set("balance", (balance - 100).to_string());

        let result: (String,) = tx.exec().await.expect("事务执行失败");

        // 验证结果
        assert_eq!(result.0, "OK");
    }

    // 验证最终余额
    let final_balance = client.get("balance").await.expect("获取余额失败");
    assert_eq!(final_balance, Some("900".to_string()));

    // 清理测试数据
    let _ = client.del(&["balance".to_string()]).await;
}

/// 测试并发事务场景
///
/// 验证多个并发事务能够正确执行
#[tokio::test]
async fn test_transaction_concurrent() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 清理测试数据
    let _ = client.del(&["concurrent_counter".to_string()]).await;

    // 初始化计数器
    client
        .set("concurrent_counter", "0")
        .await
        .expect("设置计数器失败");

    let mut handles = vec![];

    // 创建 10 个并发事务，每个事务增加计数器
    for _ in 0..10 {
        let client_clone = client.clone();
        let handle = tokio::spawn(async move {
            // 读取当前值
            let value: String = client_clone
                .get("concurrent_counter")
                .await
                .ok()
                .flatten()
                .unwrap_or_else(|| "0".to_string());
            let current: i64 = value.parse().unwrap_or(0);

            // 使用 WATCH 监视计数器，实现乐观锁
            let mut tx = client_clone.transaction();
            tx.watch(&["concurrent_counter".to_string()]);
            tx.set("concurrent_counter", (current + 1).to_string());

            let result: Result<(String,), _> = tx.exec().await;
            result
        });
        handles.push(handle);
    }

    // 等待所有任务完成
    let mut success_count = 0;
    for handle in handles {
        let result = handle.await.expect("任务执行失败");
        if result.is_ok() {
            success_count += 1;
        }
    }

    // 至少应该有一些事务成功
    assert!(success_count > 0, "应该至少有一些事务成功");

    // 验证最终计数器值
    let final_value = client
        .get("concurrent_counter")
        .await
        .expect("获取计数器失败");
    let final_count: i64 = final_value.unwrap().parse().unwrap_or(0);

    // 由于 WATCH 机制，最终计数应该大于 0
    println!("成功事务数: {}, 最终计数: {}", success_count, final_count);
    assert!(final_count > 0, "最终计数应该大于 0");

    // 清理测试数据
    let _ = client.del(&["concurrent_counter".to_string()]).await;
}

/// 测试事务原子性
///
/// 验证事务中的所有命令要么全部执行，要么全部不执行
#[tokio::test]
async fn test_transaction_atomicity() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 清理测试数据
    let _ = client
        .del(&[
            "atomic_key1".to_string(),
            "atomic_key2".to_string(),
            "atomic_key3".to_string(),
        ])
        .await;

    // 执行事务：设置三个键
    let mut tx = client.transaction();
    tx.set("atomic_key1", "value1")
        .set("atomic_key2", "value2")
        .set("atomic_key3", "value3");

    let result: (String, String, String) = tx.exec().await.expect("事务执行失败");

    // 验证所有命令都成功
    assert_eq!(result.0, "OK");
    assert_eq!(result.1, "OK");
    assert_eq!(result.2, "OK");

    // 验证所有键都已设置
    let value1 = client
        .get("atomic_key1")
        .await
        .expect("获取 atomic_key1 失败");
    let value2 = client
        .get("atomic_key2")
        .await
        .expect("获取 atomic_key2 失败");
    let value3 = client
        .get("atomic_key3")
        .await
        .expect("获取 atomic_key3 失败");

    assert_eq!(value1, Some("value1".to_string()));
    assert_eq!(value2, Some("value2".to_string()));
    assert_eq!(value3, Some("value3".to_string()));

    // 清理测试数据
    let _ = client
        .del(&[
            "atomic_key1".to_string(),
            "atomic_key2".to_string(),
            "atomic_key3".to_string(),
        ])
        .await;
}

/// 测试空事务
///
/// 验证空事务（不包含任何命令）能够正常执行
#[tokio::test]
async fn test_empty_transaction() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 执行空事务
    let tx = client.transaction();
    let result: () = tx.exec().await.expect("空事务执行失败");

    // 空事务应该成功返回
    assert_eq!(result, ());
}

/// 测试事务中的 Hash 操作
///
/// 验证事务能够正确执行 Hash 相关命令
#[tokio::test]
async fn test_transaction_hash_operations() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 清理测试数据
    let _ = client.del(&["tx_hash".to_string()]).await;

    // 执行事务：设置多个 Hash 字段
    let mut tx = client.transaction();
    tx.hset("tx_hash", "field1", "value1")
        .hset("tx_hash", "field2", "value2")
        .hset("tx_hash", "field3", "value3");

    let result: (i64, i64, i64) = tx.exec().await.expect("事务执行失败");

    // 验证结果（HSET 返回 1 表示新字段，0 表示更新）
    assert!(result.0 == 1 || result.0 == 0);
    assert!(result.1 == 1 || result.1 == 0);
    assert!(result.2 == 1 || result.2 == 0);

    // 验证数据已写入
    let value1 = client
        .hget("tx_hash", "field1")
        .await
        .expect("获取 field1 失败");
    let value2 = client
        .hget("tx_hash", "field2")
        .await
        .expect("获取 field2 失败");
    let value3 = client
        .hget("tx_hash", "field3")
        .await
        .expect("获取 field3 失败");

    assert_eq!(value1, Some("value1".to_string()));
    assert_eq!(value2, Some("value2".to_string()));
    assert_eq!(value3, Some("value3".to_string()));

    // 清理测试数据
    let _ = client.del(&["tx_hash".to_string()]).await;
}
