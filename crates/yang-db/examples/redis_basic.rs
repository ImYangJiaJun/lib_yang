//! Redis 基础用法：连接、二进制、Pipeline、Lua、锁与健康检查。
//!
//! 运行：
//! ```bash
//! REDIS_URL=redis://127.0.0.1:6379 cargo run --example redis_basic -p yang-db
//! ```

use std::time::Duration;
use yang_db::{RedisClient, RedisConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let config = RedisConfig::default()
        .with_response_timeout_ms(Some(3_000))
        .with_reconnect_retries(2)
        .with_enable_logging(true);

    let client = RedisClient::connect_with_config(url, config).await?;

    // 基础字符串
    client.set("yang:demo:key", "value").await?;
    println!("GET = {:?}", client.get("yang:demo:key").await?);

    // 二进制安全
    let blob = vec![0u8, 0xFF, 0x80];
    client.set_bytes("yang:demo:blob", &blob).await?;
    println!(
        "GET bytes = {:?}",
        client.get_bytes("yang:demo:blob").await?
    );

    // Pipeline：一条错误可被 tolerant 模式单独报告
    let mut pipeline = client.pipeline();
    pipeline.set("yang:demo:p1", "v1");
    pipeline.get("yang:demo:p1");
    println!("Pipeline = {:?}", pipeline.execute().await?);

    // Lua
    let script = client.script("return redis.call('INCR', KEYS[1])");
    let counter: i64 = client
        .eval_script(&script, &["yang:demo:counter".to_string()], &[])
        .await?;
    println!("counter = {counter}");

    // 分布式锁
    let lock = client
        .lock("yang:demo:lock")
        .with_ttl(Duration::from_secs(10));
    if let Some(guard) = lock.try_acquire().await? {
        println!("lock acquired");
        guard.release().await?;
    }

    // 健康检查
    println!("health = {:?}", client.health_check_detailed().await?);

    client
        .del(&[
            "yang:demo:key".to_string(),
            "yang:demo:blob".to_string(),
            "yang:demo:p1".to_string(),
            "yang:demo:counter".to_string(),
        ])
        .await?;
    client.close_and_wait(Duration::from_secs(2)).await;
    Ok(())
}
