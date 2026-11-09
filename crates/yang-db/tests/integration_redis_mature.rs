#![allow(clippy::expect_used, clippy::unwrap_used)]
//! 成熟 Redis 能力集成测试：二进制、命名空间/JSON、锁、Pipeline 容错、
//! Streams、Pub/Sub、SCAN 遍历、详细健康检查、命令级超时。
//!
//! 默认连接 `REDIS_TEST_URL`，未设置时回退到 `redis://127.0.0.1:6379`。
//! 测试不做软跳过：连接失败必须失败。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use yang_db::{KeyNamespace, RedisClient, RedisConfig, RedisValue};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn redis_url() -> String {
    std::env::var("REDIS_TEST_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
}

async fn client() -> RedisClient {
    RedisClient::connect(redis_url())
        .await
        .expect("连接测试 Redis 失败")
}

fn key(prefix: &str) -> String {
    format!(
        "yang:mature:{prefix}:{}:{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    )
}

#[tokio::test]
async fn binary_roundtrip_is_lossless() {
    let client = client().await;
    let cache_key = key("bytes");
    let hash_key = key("bytes-hash");
    let value = vec![0u8, 0xFF, 0x80, b'a'];

    client.set_bytes(&cache_key, &value).await.unwrap();
    assert_eq!(
        client.get_bytes(&cache_key).await.unwrap(),
        Some(value.clone())
    );
    // 字符串 API 对非 UTF-8 必须返回明确错误，而不是「键不存在」
    assert!(matches!(
        client.get(&cache_key).await,
        Err(yang_db::DbError::RedisTypeConversionError(_))
    ));

    client
        .hset_bytes(&hash_key, [0xFFu8], &value)
        .await
        .unwrap();
    assert_eq!(
        client.hget_bytes(&hash_key, [0xFFu8]).await.unwrap(),
        Some(value.clone())
    );
    assert_eq!(
        client.hgetall_bytes(&hash_key).await.unwrap(),
        vec![(vec![0xFF], value.clone())]
    );

    client.del(&[cache_key, hash_key]).await.unwrap();
}

#[tokio::test]
async fn namespace_and_json_roundtrip() {
    let namespace = KeyNamespace::new("yang", "test").expect("合法 namespace");
    let namespaced = client().await.with_namespace(namespace);
    let cache_key = key("json");

    namespaced
        .set_json(&cache_key, &42u64, Some(30))
        .await
        .unwrap();
    assert_eq!(
        namespaced.get_json::<u64>(&cache_key).await.unwrap(),
        Some(42)
    );

    // 原始客户端应看到带前缀的 key
    let raw = client().await;
    let prefixed = format!("yang:test:{cache_key}");
    assert_eq!(
        raw.get_bytes(&prefixed).await.unwrap(),
        Some(b"42".to_vec())
    );

    raw.del(&[prefixed]).await.unwrap();
}

#[tokio::test]
async fn lock_is_mutually_exclusive_and_owner_safe() {
    let client = client().await;
    let lock_key = key("lock");
    let lock = client.lock(&lock_key).with_ttl(Duration::from_secs(5));

    let guard = lock.try_acquire().await.unwrap().expect("首次获取应成功");
    assert!(
        lock.try_acquire().await.unwrap().is_none(),
        "第二次必须失败"
    );
    assert!(guard.release().await.unwrap(), "持有者释放应成功");

    let guard = lock.try_acquire().await.unwrap().expect("释放后应可获取");
    assert!(guard.extend(Duration::from_secs(5)).await.unwrap());
    guard.release().await.unwrap();
}

#[tokio::test]
async fn pipeline_tolerant_reports_each_command() {
    let client = client().await;
    let set_key = key("pipeline:set");
    let bad_key = key("pipeline:bad");

    client.set(&bad_key, "not-a-number").await.unwrap();

    let mut pipeline = client.pipeline();
    pipeline.set(&set_key, "value");
    pipeline.cmd(yang_db::redis_crate::cmd("INCR").arg(&bad_key).clone());
    pipeline.get(&set_key);

    let results = pipeline.execute_tolerant().await.unwrap();
    assert_eq!(results.len(), 3);
    assert!(results[0].is_ok());
    assert!(results[1].is_err(), "INCR 非数字必须返回单条错误");
    assert!(results[2].is_ok(), "错误不得吞掉后续命令结果");

    client.del(&[set_key, bad_key]).await.unwrap();
}

#[tokio::test]
async fn streams_consumer_group_flow() {
    let client = client().await;
    let stream = key("stream");
    let group = "grp";

    client
        .xgroup_create(&stream, group, "0", true)
        .await
        .unwrap();
    let id = client
        .xadd(&stream, "*", &[("field", "value")])
        .await
        .unwrap();
    assert_eq!(client.xlen(&stream).await.unwrap(), 1);

    let range = client.xrange(&stream, "-", "+", None).await.unwrap();
    assert_eq!(range.ids.len(), 1);
    assert_eq!(range.ids[0].id, id);
    assert_eq!(
        range.ids[0].get::<String>("field"),
        Some("value".to_string())
    );

    let reply = client
        .xread_group(
            std::slice::from_ref(&stream),
            &[">".to_string()],
            group,
            "consumer-1",
            Some(10),
            None,
            false,
        )
        .await
        .unwrap();
    assert_eq!(reply.keys.len(), 1);
    assert_eq!(reply.keys[0].ids.len(), 1);

    let pending = client.xpending(&stream, group).await.unwrap();
    assert_eq!(pending.count(), 1);

    assert_eq!(client.xack(&stream, group, &[id]).await.unwrap(), 1);
    assert_eq!(client.xpending(&stream, group).await.unwrap().count(), 0);

    // 再次消费后交给另一个 consumer 接管
    let _ = client
        .xadd(&stream, "*", &[("field", "second")])
        .await
        .unwrap();
    let reply = client
        .xread_group(
            std::slice::from_ref(&stream),
            &[">".to_string()],
            group,
            "consumer-1",
            Some(10),
            None,
            false,
        )
        .await
        .unwrap();
    let claimed_id = reply.keys[0].ids[0].id.clone();
    let claimed = client
        .xautoclaim(&stream, group, "consumer-2", 0, "0-0", Some(10))
        .await
        .unwrap();
    assert!(claimed.claimed.iter().any(|entry| entry.id == claimed_id));

    let info = client.xinfo_stream(&stream).await.unwrap();
    assert!(info.length >= 1);

    client.del(&[stream]).await.unwrap();
}

#[tokio::test]
async fn pubsub_publish_subscribe_roundtrip() {
    let client = client().await;
    let channel = key("chan");
    let mut subscriber = client.pubsub().await.unwrap();
    subscriber
        .subscribe(std::slice::from_ref(&channel))
        .await
        .unwrap();

    // 订阅确认与 PUBLISH 之间存在竞态：连续发布直到收到消息
    let publisher = client.clone();
    let publish_channel = channel.clone();
    let publish_task = tokio::spawn(async move {
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = publisher.publish(&publish_channel, "hello").await;
        }
    });

    let message = tokio::time::timeout(Duration::from_secs(5), subscriber.next_message())
        .await
        .expect("等待 Pub/Sub 消息超时")
        .unwrap()
        .expect("应收到消息");
    assert_eq!(message.channel, channel);
    assert_eq!(message.payload, b"hello");
    publish_task.abort();
}

#[tokio::test]
async fn pubsub_pattern_subscription() {
    let client = client().await;
    let channel = key("pattern-chan");
    let pattern = format!("{}*", &channel[..channel.len() - 3]);
    let mut subscriber = client.pubsub().await.unwrap();
    subscriber
        .psubscribe(std::slice::from_ref(&pattern))
        .await
        .unwrap();

    let publisher = client.clone();
    let publish_channel = channel.clone();
    let publish_task = tokio::spawn(async move {
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = publisher.publish(&publish_channel, "pattern").await;
        }
    });

    let message = tokio::time::timeout(Duration::from_secs(5), subscriber.next_message())
        .await
        .expect("等待模式订阅消息超时")
        .unwrap()
        .expect("应收到消息");
    assert_eq!(message.pattern.as_deref(), Some(pattern.as_str()));
    assert_eq!(message.payload, b"pattern");
    publish_task.abort();
}

#[tokio::test]
async fn scan_all_and_hscan_all_cover_everything() {
    let client = client().await;
    let prefix = key("scan");
    let hash_key = format!("{prefix}:hash");

    for index in 0..50 {
        client
            .set(format!("{prefix}:item:{index}"), "v")
            .await
            .unwrap();
        client
            .hset(
                &hash_key,
                format!("field-{index}"),
                format!("value-{index}"),
            )
            .await
            .unwrap();
    }

    let keys = client
        .scan_all(Some(&format!("{prefix}:item:*")), Some(10))
        .await
        .unwrap();
    assert_eq!(keys.len(), 50);

    let pairs = client
        .hscan_all(hash_key.clone(), None, Some(10))
        .await
        .unwrap();
    assert_eq!(pairs.len(), 50);

    let mut cleanup = keys;
    cleanup.push(hash_key);
    client.del(&cleanup).await.unwrap();
}

#[tokio::test]
async fn detailed_health_check_reads_info() {
    let client = client().await;
    let health = client.health_check_detailed().await.unwrap();
    assert!(health.ping);
    assert!(health.role.is_some(), "INFO 应返回 role");
    assert!(health.connected_clients.is_some());
}

#[tokio::test]
async fn command_response_timeout_is_enforced() {
    // 接受 TCP 连接但永不响应，模拟网络黑洞/卡死的 Redis。
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("绑定本地监听失败");
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((stream, _)) = listener.accept().await {
            held.push(stream);
        }
    });

    let config = RedisConfig::default()
        .with_connect_timeout(1)
        .with_wait_timeout(2)
        .with_response_timeout_ms(Some(100))
        .with_reconnect_retries(0);
    let result = RedisClient::connect_with_config(format!("redis://{address}"), config).await;
    assert!(
        result.is_err(),
        "无响应服务器必须在 response timeout 内失败"
    );
}

#[tokio::test]
async fn close_and_wait_is_idempotent() {
    let client = client().await;
    client.close_and_wait(Duration::from_secs(1)).await;
    assert!(client.is_closed());
    // 再次关闭不应 panic
    client.close_and_wait(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn execute_escape_hatch_still_works() {
    let client = client().await;
    let cache_key = key("escape");
    let mut cmd = yang_db::redis_crate::cmd("SET");
    cmd.arg(&cache_key).arg("raw");
    client.execute(&cmd).await.unwrap();
    assert_eq!(
        client.get(&cache_key).await.unwrap(),
        Some("raw".to_string())
    );
    assert_eq!(client.del(&[cache_key]).await.unwrap(), 1);
}

#[tokio::test]
async fn value_debug_never_prints_payload() {
    let client = client().await;
    let cache_key = key("debug");
    client.set(&cache_key, "super-secret-token").await.unwrap();
    let value: RedisValue = client
        .execute(yang_db::redis_crate::cmd("GET").arg(&cache_key))
        .await
        .unwrap();
    let debug = format!("{value:?}");
    assert!(!debug.contains("super-secret-token"));
    client.del(&[cache_key]).await.unwrap();
}
