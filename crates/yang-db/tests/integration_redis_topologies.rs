#![allow(clippy::expect_used, clippy::unwrap_used)]
#![allow(dead_code)]
//! 真实 Redis 拓扑集成测试（TLS / Cluster / Sentinel）。
//!
//! 全部 `#[ignore]`，需要外部环境；由本地或 CI 显式启动对应拓扑后运行。
//! 推荐直接使用 `bash scripts/run_redis_topology_tests.sh` 一键搭建并执行。
//!
//! ```bash
//! # TLS（自签名 CA；insecure 用例需要 redis-tls-insecure-test feature）
//! REDIS_TLS_URL=rediss://127.0.0.1:6382 \
//!   cargo test -p yang-db --features redis-tls,redis-tls-insecure-test \
//!   --test integration_redis_topologies tls_ -- --ignored --test-threads=1
//!
//! # Cluster
//! REDIS_CLUSTER_URLS=redis://127.0.0.1:7000,redis://127.0.0.1:7001,redis://127.0.0.1:7002 \
//!   cargo test -p yang-db --features redis-cluster \
//!   --test integration_redis_topologies cluster_ -- --ignored --test-threads=1
//!
//! # Sentinel
//! REDIS_SENTINEL_URLS=redis://127.0.0.1:26379,redis://127.0.0.1:26380,redis://127.0.0.1:26381 \
//! REDIS_SENTINEL_MASTER=mymaster \
//!   cargo test -p yang-db --features redis-sentinel \
//!   --test integration_redis_topologies sentinel_ -- --ignored --test-threads=1
//! ```

#[cfg(feature = "redis-cluster")]
use yang_db::DbError;
use yang_db::{RedisClient, RedisConfig, RedisTopology};

fn env_required(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("必须设置环境变量 {name}"))
}

fn env_list(name: &str) -> Vec<String> {
    env_required(name)
        .split(',')
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn tls_url() -> String {
    let url = env_required("REDIS_TLS_URL");
    assert!(
        url.starts_with("rediss://"),
        "REDIS_TLS_URL 必须是 rediss://"
    );
    url
}

fn unique_key(suffix: &str) -> String {
    format!(
        "yang:topology:{}:{}:{}",
        suffix,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.subsec_nanos())
            .unwrap_or(0)
    )
}

#[cfg(feature = "redis-tls-insecure-test")]
#[tokio::test]
#[ignore = "需要 REDIS_TLS_URL 指向自签名 TLS Redis"]
async fn tls_insecure_connect_and_command() {
    let base = tls_url();
    let url = if base.contains("#insecure") {
        base
    } else {
        format!("{base}/#insecure")
    };
    let client = RedisClient::connect(url)
        .await
        .expect("rediss://#insecure 应能连接自签名 TLS Redis");
    let key = unique_key("tls");
    client.set(&key, "tls-value").await.expect("TLS SET 失败");
    assert_eq!(
        client.get(&key).await.expect("TLS GET 失败"),
        Some("tls-value".to_string())
    );
    client.del(&[key]).await.expect("TLS DEL 失败");
}

#[cfg(feature = "redis-tls")]
#[tokio::test]
#[ignore = "需要 REDIS_TLS_URL 且其证书不受系统信任"]
async fn tls_verification_rejects_untrusted_certificate() {
    let client = RedisClient::connect_with_config(
        tls_url(),
        RedisConfig::default()
            .with_connect_timeout(2)
            .with_wait_timeout(3)
            .with_reconnect_retries(0),
    )
    .await;
    assert!(
        client.is_err(),
        "默认配置必须拒绝不受信任的自签名证书，实际连接成功"
    );
}

#[cfg(feature = "redis-cluster")]
#[tokio::test]
#[ignore = "需要 REDIS_CLUSTER_URLS 指向可用集群"]
async fn cluster_topology_connect_and_command() {
    let urls = env_list("REDIS_CLUSTER_URLS");
    assert!(!urls.is_empty(), "REDIS_CLUSTER_URLS 不能为空");
    let config = RedisConfig::default()
        .with_connect_timeout(3)
        .with_wait_timeout(5)
        .with_response_timeout_ms(Some(3_000));
    let client = RedisClient::connect_topology_with_config(RedisTopology::Cluster { urls }, config)
        .await
        .expect("Cluster 连接失败");

    assert!(client.is_cluster(), "拓扑应为 Cluster");
    assert!(client.health_check().await.expect("PING 失败"));

    let key = unique_key("cluster");
    client
        .set(&key, "cluster-value")
        .await
        .expect("Cluster SET 失败");
    assert_eq!(
        client.get(&key).await.expect("Cluster GET 失败"),
        Some("cluster-value".to_string())
    );
    // Cluster 的 SCAN 游标是节点本地的，便捷遍历必须显式拒绝
    assert!(matches!(
        client.scan_all(None, Some(10)).await,
        Err(DbError::UnsupportedOperator(_))
    ));
    client.del(&[key]).await.expect("Cluster DEL 失败");
}

#[cfg(feature = "redis-sentinel")]
#[tokio::test]
#[ignore = "需要 REDIS_SENTINEL_URLS / REDIS_SENTINEL_MASTER"]
async fn sentinel_topology_connect_and_command() {
    let sentinel_urls = env_list("REDIS_SENTINEL_URLS");
    let master_name = env_required("REDIS_SENTINEL_MASTER");
    assert!(!sentinel_urls.is_empty(), "REDIS_SENTINEL_URLS 不能为空");
    let config = RedisConfig::default()
        .with_connect_timeout(3)
        .with_wait_timeout(5)
        .with_response_timeout_ms(Some(3_000));
    let client = RedisClient::connect_topology_with_config(
        RedisTopology::Sentinel {
            sentinel_urls,
            master_name,
        },
        config,
    )
    .await
    .expect("Sentinel 连接失败");

    assert!(!client.is_cluster(), "Sentinel 拓扑不应标记为 Cluster");
    assert!(client.health_check().await.expect("PING 失败"));

    let key = unique_key("sentinel");
    client
        .set(&key, "sentinel-value")
        .await
        .expect("Sentinel SET 失败");
    assert_eq!(
        client.get(&key).await.expect("Sentinel GET 失败"),
        Some("sentinel-value".to_string())
    );
    client.del(&[key]).await.expect("Sentinel DEL 失败");
}
