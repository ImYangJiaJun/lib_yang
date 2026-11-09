#![allow(clippy::expect_used, clippy::unwrap_used)]
//! yang-base 通用缓存门面与限流器集成测试。
//!
//! 默认连接 `REDIS_TEST_URL`，未设置时回退到 `redis://127.0.0.1:6379`；
//! 连接失败必须失败（不软跳过）。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use yang_base::cache::{Cache, CacheConfig, RateLimiter};
use yang_base::error::BaseError;
use yang_db::RedisClient;

fn redis_url() -> String {
    std::env::var("REDIS_TEST_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
}

async fn client() -> RedisClient {
    RedisClient::connect(redis_url())
        .await
        .expect("连接测试 Redis 失败")
}

fn key(suffix: &str) -> String {
    format!("yang-base:cache-test:{suffix}:{}", std::process::id())
}

#[tokio::test]
async fn get_or_load_caches_value() {
    let cache = Cache::new(client().await, CacheConfig::default());
    let cache_key = key("basic");
    cache.delete(&cache_key).await.unwrap();

    let loads = Arc::new(AtomicUsize::new(0));
    for _ in 0..2 {
        let loads = loads.clone();
        let value: u64 = cache
            .get_or_load(
                &cache_key,
                Some(Duration::from_secs(30)),
                move || async move {
                    loads.fetch_add(1, Ordering::SeqCst);
                    Ok(7u64)
                },
            )
            .await
            .unwrap();
        assert_eq!(value, 7);
    }
    assert_eq!(loads.load(Ordering::SeqCst), 1, "第二次应命中缓存");
    cache.delete(&cache_key).await.unwrap();
}

#[tokio::test]
async fn single_flight_collapses_concurrent_misses() {
    let cache = Arc::new(Cache::new(
        client().await,
        CacheConfig {
            single_flight: true,
            ..CacheConfig::default()
        },
    ));
    let cache_key = key("single-flight");
    cache.delete(&cache_key).await.unwrap();

    let loads = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(tokio::sync::Barrier::new(20));
    let mut tasks = Vec::new();
    for _ in 0..20 {
        let cache = cache.clone();
        let loads = loads.clone();
        let barrier = barrier.clone();
        let cache_key = cache_key.clone();
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            let counter = loads.clone();
            cache
                .get_or_load::<u64, _, _>(&cache_key, None, move || {
                    let counter = counter.clone();
                    async move {
                        counter.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        Ok(9u64)
                    }
                })
                .await
        }));
    }
    for task in tasks {
        assert_eq!(task.await.unwrap().unwrap(), 9);
    }
    assert_eq!(
        loads.load(Ordering::SeqCst),
        1,
        "20 个并发 miss 只应回源一次"
    );
    cache.delete(&cache_key).await.unwrap();
}

#[tokio::test]
async fn optional_loader_caches_null() {
    let cache = Cache::new(client().await, CacheConfig::default());
    let cache_key = key("null");
    cache.delete(&cache_key).await.unwrap();

    let loads = Arc::new(AtomicUsize::new(0));
    for _ in 0..2 {
        let loads = loads.clone();
        let value = cache
            .get_or_load_optional::<u64, _, _>(&cache_key, None, move || async move {
                loads.fetch_add(1, Ordering::SeqCst);
                Ok::<Option<u64>, BaseError>(None)
            })
            .await
            .unwrap();
        assert_eq!(value, None);
    }
    assert_eq!(loads.load(Ordering::SeqCst), 1, "空值也应缓存，避免穿透");
    cache.delete(&cache_key).await.unwrap();
}

#[tokio::test]
async fn fail_open_only_applies_to_plain_cache() {
    // fail-open：Redis 关闭后按 miss 处理
    let open_client = client().await;
    let open_cache = Cache::new(
        open_client.clone(),
        CacheConfig {
            fail_open: true,
            ..CacheConfig::default()
        },
    );
    open_client.close().await;
    assert_eq!(
        open_cache.get::<u64>(&key("fail-open")).await.unwrap(),
        None
    );

    // fail-closed：Redis 关闭后必须返回错误
    let closed_client = client().await;
    let strict_cache = Cache::new(
        closed_client.clone(),
        CacheConfig {
            fail_open: false,
            ..CacheConfig::default()
        },
    );
    closed_client.close().await;
    assert!(strict_cache.get::<u64>(&key("fail-closed")).await.is_err());
}

#[tokio::test]
async fn tools_health_check_includes_redis() {
    let tools = yang_base::tools::ToolsBuilder::new()
        .cache(client().await)
        .build()
        .expect("Tools 应构建成功");
    let health = tools.health_check().await;
    assert!(health.is_healthy());
    assert!(
        health
            .resources
            .iter()
            .any(|resource| resource.resource == "cache" && resource.healthy),
        "Tools 健康检查应包含 Redis cache"
    );
    tools.close().await;
}

#[tokio::test]
async fn rate_limiter_enforces_fixed_window() {
    let limiter = RateLimiter::new(client().await, key("limit"));
    let subject = key("subject");
    let window = Duration::from_secs(30);

    let first = limiter.check(&subject, 2, window).await.unwrap();
    assert!(first.allowed);
    assert_eq!(first.current, 1);

    let second = limiter.check(&subject, 2, window).await.unwrap();
    assert!(second.allowed);
    assert_eq!(second.current, 2);

    let third = limiter.check(&subject, 2, window).await.unwrap();
    assert!(!third.allowed, "超过上限必须拒绝");
    assert!(third.reset_after_ms > 0);
}
