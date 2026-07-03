#![allow(deprecated)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
#![allow(dead_code)]
#![allow(unused_results)]
use yang_db::RedisClient;

/// 集成测试：String 操作完整流程
#[tokio::test]
async fn integration_test_string_operations() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 场景：管理用户会话数据
    let session_key = "session:user:1001";

    // 1. 设置会话数据，100 毫秒后过期
    client
        .psetex(session_key, 100, "user_session_data")
        .await
        .expect("PSETEX 失败");

    // 2. 读取会话数据
    let session_data = client
        .get(session_key)
        .await
        .expect("GET 失败")
        .expect("会话应存在");
    assert_eq!(session_data, "user_session_data");

    // 3. 等待会话过期
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

    let session_data = client.get(session_key).await.expect("GET 失败");
    assert_eq!(session_data, None, "会话应已过期");

    // 场景：处理文本内容
    let content_key = "content:article:2001";

    // 1. 设置文章内容
    client
        .set(content_key, "Hello World! This is a test article.")
        .await
        .expect("SET 失败");

    // 2. 获取标题（前 12 个字符）
    let title = client
        .getrange(content_key, 0, 11)
        .await
        .expect("GETRANGE 失败");
    assert_eq!(title, "Hello World!");

    // 3. 修改部分内容
    client
        .setrange(content_key, 13, "That")
        .await
        .expect("SETRANGE 失败");

    let updated_content = client
        .get(content_key)
        .await
        .expect("GET 失败")
        .expect("内容应存在");
    assert!(updated_content.contains("That is a test"));

    // 场景：管理商品价格
    let price_key = "price:product:3001";

    // 1. 设置初始价格
    client.set(price_key, "99.99").await.expect("SET 失败");

    // 2. 价格上涨 10.5
    let new_price = client
        .incrbyfloat(price_key, 10.5)
        .await
        .expect("INCRBYFLOAT 失败");
    assert!((new_price - 110.49).abs() < 0.01);

    // 3. 价格下降 5.49（促销）
    let final_price = client
        .incrbyfloat(price_key, -5.49)
        .await
        .expect("INCRBYFLOAT 失败");
    assert!((final_price - 105.0).abs() < 0.01);

    // 清理
    client
        .del(&[
            session_key.to_string(),
            content_key.to_string(),
            price_key.to_string(),
        ])
        .await
        .ok();
}

/// 集成测试：List 操作完整流程
#[tokio::test]
async fn integration_test_list_operations() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 场景：任务队列管理
    let queue_key = "queue:tasks";

    // 1. 添加任务到队列
    client
        .rpush(
            queue_key,
            &[
                "task1".to_string(),
                "task2".to_string(),
                "task3".to_string(),
            ],
        )
        .await
        .expect("RPUSH 失败");

    // 2. 在 task2 前插入优先任务
    client
        .linsert(queue_key, "BEFORE", "task2", "priority_task")
        .await
        .expect("LINSERT 失败");

    let tasks = client.lrange(queue_key, 0, -1).await.expect("LRANGE 失败");
    assert_eq!(tasks, vec!["task1", "priority_task", "task2", "task3"]);

    // 3. 删除已完成的任务
    client.lrem(queue_key, 1, "task1").await.expect("LREM 失败");

    // 场景：消息队列处理
    let inbox_key = "inbox:user:1001";
    let processing_key = "processing:user:1001";

    // 1. 添加消息到收件箱
    client
        .rpush(
            inbox_key,
            &["msg1".to_string(), "msg2".to_string(), "msg3".to_string()],
        )
        .await
        .expect("RPUSH 失败");

    // 2. 将消息从收件箱移到处理队列
    let msg = client
        .rpoplpush(inbox_key, processing_key)
        .await
        .expect("RPOPLPUSH 失败")
        .expect("应有消息");
    assert_eq!(msg, "msg3");

    // 3. 验证队列状态
    let inbox_len = client.llen(inbox_key).await.expect("LLEN 失败");
    assert_eq!(inbox_len, 2);

    let processing_len = client.llen(processing_key).await.expect("LLEN 失败");
    assert_eq!(processing_len, 1);

    // 场景：实时通知系统
    let notification_key = "notifications:user:2001";

    // 1. 添加通知
    client
        .rpush(notification_key, &["notification1".to_string()])
        .await
        .expect("RPUSH 失败");

    // 2. 阻塞等待通知（应立即返回）
    let result = client
        .blpop(&[notification_key.to_string()], 0)
        .await
        .expect("BLPOP 失败");
    assert!(result.is_some());

    // 注意：不测试等待新通知的超时情况，因为会导致连接池超时

    // 清理
    client
        .del(&[
            queue_key.to_string(),
            inbox_key.to_string(),
            processing_key.to_string(),
            notification_key.to_string(),
        ])
        .await
        .ok();
}

/// 集成测试：String 和 List 操作混合场景
#[tokio::test]
async fn integration_test_mixed_operations() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 场景：电商订单处理系统
    let order_id = "order:5001";
    let order_status_key = format!("{}:status", order_id);
    let order_items_key = format!("{}:items", order_id);
    let order_total_key = format!("{}:total", order_id);

    // 1. 创建订单
    client
        .set(&order_status_key, "pending")
        .await
        .expect("SET 失败");

    // 2. 添加订单商品
    client
        .rpush(
            &order_items_key,
            &[
                "item:1001".to_string(),
                "item:1002".to_string(),
                "item:1003".to_string(),
            ],
        )
        .await
        .expect("RPUSH 失败");

    // 3. 设置订单总额
    client.set(&order_total_key, "0").await.expect("SET 失败");

    // 4. 累加商品价格
    client
        .incrbyfloat(&order_total_key, 29.99)
        .await
        .expect("INCRBYFLOAT 失败");
    client
        .incrbyfloat(&order_total_key, 49.99)
        .await
        .expect("INCRBYFLOAT 失败");
    client
        .incrbyfloat(&order_total_key, 19.99)
        .await
        .expect("INCRBYFLOAT 失败");

    let total = client
        .get(&order_total_key)
        .await
        .expect("GET 失败")
        .expect("总额应存在");
    let total_value: f64 = total.parse().expect("解析失败");
    assert!((total_value - 99.97).abs() < 0.01);

    // 5. 移除一个商品
    client
        .lrem(&order_items_key, 1, "item:1002")
        .await
        .expect("LREM 失败");

    // 6. 更新总额
    client
        .incrbyfloat(&order_total_key, -49.99)
        .await
        .expect("INCRBYFLOAT 失败");

    // 7. 更新订单状态
    client
        .set(&order_status_key, "confirmed")
        .await
        .expect("SET 失败");

    // 8. 验证最终状态
    let status = client
        .get(&order_status_key)
        .await
        .expect("GET 失败")
        .expect("状态应存在");
    assert_eq!(status, "confirmed");

    let items = client
        .lrange(&order_items_key, 0, -1)
        .await
        .expect("LRANGE 失败");
    assert_eq!(items.len(), 2);

    let final_total = client
        .get(&order_total_key)
        .await
        .expect("GET 失败")
        .expect("总额应存在");
    let final_total_value: f64 = final_total.parse().expect("解析失败");
    assert!((final_total_value - 49.98).abs() < 0.01);

    // 清理
    client
        .del(&[order_status_key, order_items_key, order_total_key])
        .await
        .ok();
}

/// 集成测试：高并发场景
#[tokio::test]
async fn integration_test_high_concurrency() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 场景：高并发计数器和队列
    let counter_key = "concurrent:counter";
    let queue_key = "concurrent:queue";

    // 初始化
    client.set(counter_key, "0").await.expect("SET 失败");

    let mut handles = vec![];

    // 创建 20 个并发任务
    for i in 0..20 {
        let client_clone = client.clone();
        let counter_key = counter_key.to_string();
        let queue_key = queue_key.to_string();

        let handle = tokio::spawn(async move {
            // 增加计数器
            client_clone
                .incrbyfloat(&counter_key, 1.5)
                .await
                .expect("INCRBYFLOAT 失败");

            // 添加到队列
            client_clone
                .rpush(&queue_key, &[format!("item_{}", i)])
                .await
                .expect("RPUSH 失败");
        });

        handles.push(handle);
    }

    // 等待所有任务完成
    for handle in handles {
        handle.await.expect("任务执行失败");
    }

    // 验证结果
    let counter_value = client
        .get(counter_key)
        .await
        .expect("GET 失败")
        .expect("计数器应存在");
    let counter: f64 = counter_value.parse().expect("解析失败");
    assert!((counter - 30.0).abs() < 0.01, "计数器应为 30.0");

    let queue_len = client.llen(queue_key).await.expect("LLEN 失败");
    assert_eq!(queue_len, 20, "队列应包含 20 个元素");

    // 清理
    client
        .del(&[counter_key.to_string(), queue_key.to_string()])
        .await
        .ok();
}

/// 集成测试：错误处理和边界情况
#[tokio::test]
async fn integration_test_error_handling() {
    let client = RedisClient::connect("redis://127.0.0.1:6379")
        .await
        .expect("连接失败");

    // 测试不存在的键
    let value = client.get("nonexistent_key").await.expect("GET 失败");
    assert_eq!(value, None);

    let substr = client
        .getrange("nonexistent_key", 0, 10)
        .await
        .expect("GETRANGE 失败");
    assert_eq!(substr, "");

    // 测试空列表操作
    let elem = client
        .rpoplpush("empty_list1", "empty_list2")
        .await
        .expect("RPOPLPUSH 失败");
    assert_eq!(elem, None);

    // 测试 LINSERT 在不存在的 pivot
    let len = client
        .linsert("test_list", "BEFORE", "nonexistent", "value")
        .await
        .expect("LINSERT 失败");
    assert_eq!(len, 0);

    // 注意：不测试超时的阻塞操作，因为会导致连接池超时
    // 如需测试超时，请使用更长的连接池超时设置
}
