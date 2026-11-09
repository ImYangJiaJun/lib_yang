# yang-base Redis 使用指南

> `GlobalRedis` 已在 0.4.0 删除。Redis 资源通过 `ToolsBuilder` 显式注入，运行期经
> `Tools::cache()` / `ActionContext::tools().cache()` 获取 `yang_db::RedisClient`。

## 初始化

```rust
use yang_base::tools::ToolsBuilder;
use yang_db::RedisConfig;

let tools = ToolsBuilder::new()
    .cache_from_url("redis://127.0.0.1:6379", RedisConfig::default())
    .await?
    .build()?;

let redis = tools.cache()?;
```

`cache_from_url` 建连失败会返回 `BaseError::RedisConnectionFailed`（保留底层错误链）。
需要自定义池参数时，先构造 `RedisConfig`：

```rust
let config = RedisConfig::default()
    .with_max_connections(20)
    .with_connect_timeout(5)
    .with_wait_timeout(10)
    .with_response_timeout_ms(Some(3_000))
    .with_reconnect_retries(2);
```

`min_connections` / `max_lifetime` / `test_before_acquire` 不被 deadpool 0.12 支持，
设置后 `validate()` 会显式返回 `InvalidArgument`（不再静默忽略）。

## 基础操作

```rust
// String
redis.setex("session:1", 3600, "data").await?;
let value = redis.get("session:1").await?;

// Hash
redis.hset("user:1", "name", "Alice").await?;
let name = redis.hget("user:1", "name").await?;
let all = redis.hgetall("user:1").await?;

// List / Set / Sorted Set
redis.lpush("queue", &["task1".to_string()]).await?;
let task = redis.rpop("queue").await?;
redis.sadd("tags", &["rust".to_string()]).await?;
redis.zadd("scores", &[(100.0, "alice".to_string())]).await?;

// 通用
redis.expire("session:1", 600).await?;
redis.scan_all(Some("user:*"), Some(100)).await?; // 生产安全，替代 KEYS
```

## 二进制安全

```rust
let blob = vec![0u8, 0xFF, 0x80];
redis.set_bytes("blob", &blob).await?;
assert_eq!(redis.get_bytes("blob").await?, Some(blob));

redis.hset_bytes("hash", [0xFFu8], &[0x00, 0x01]).await?;
let pairs = redis.hgetall_bytes("hash").await?;
```

字符串 API 遇到非 UTF-8 会返回 `DbError::RedisTypeConversionError`，不会静默当成
「键不存在」。

## JSON、命名空间与类型化读写

```rust
use yang_db::KeyNamespace;

let redis = redis.clone().with_namespace(KeyNamespace::new("myapp", "prod")?);
redis.set_json("user:1", &user, Some(300)).await?;
let user: User = redis.get_json("user:1").await?.unwrap();

// 自定义 codec
redis.set_typed(&MyCodec, "user:2", &user, None).await?;
```

## Pipeline / 事务 / 脚本

```rust
// Pipeline：任一命令失败会整体报错
let mut pipeline = redis.pipeline();
pipeline.set("k1", "v1").get("k1");
let values = pipeline.execute().await?;

// 容错 Pipeline：逐条返回结果，单条错误不吞掉其他命令
let mut pipeline = redis.pipeline();
pipeline.set("k1", "v1");
pipeline.cmd(redis::cmd("INCR").arg("not-a-number").clone());
let results = pipeline.execute_tolerant().await?;
assert!(results[1].is_err());

// WATCH/MULTI/EXEC 乐观锁
let mut tx = redis.transaction();
tx.watch(&["balance".to_string()]);
tx.set("balance", "100");
let _: (String,) = tx.exec().await?;

// Lua（EVALSHA + NOSCRIPT 自动 SCRIPT LOAD 后重试）
let script = redis.script("return redis.call('INCR', KEYS[1])");
let value: i64 = redis.eval_script(&script, &["counter".to_string()], &[]).await?;
```

## 缓存门面（Cache）

```rust
use yang_base::cache::{Cache, CacheConfig};

let cache = Cache::new(redis.clone(), CacheConfig::default());

// cache-aside：miss 回填，single-flight 防击穿，TTL 自动抖动
let user: User = cache
    .get_or_load("user:1", None, || async { load_user(1).await })
    .await?;

// 允许缓存空值，防穿透
let maybe: Option<User> = cache
    .get_or_load_optional("user:2", None, || async { Ok(None) })
    .await?;
```

`CacheConfig::fail_open = true` 仅适用于纯缓存场景；token 撤销、MFA、鉴权限流等
安全路径必须保持 fail-closed（默认值）。

## 分布式锁

```rust
use std::time::Duration;

let guard = redis
    .lock("job:nightly")
    .with_ttl(Duration::from_secs(30))
    .acquire(Duration::from_secs(5))
    .await?;

// ... 临界区 ...

guard.release().await?; // 仅 owner token 匹配时删除
```

锁 TTL 是进程崩溃时的兜底；长时间任务应设置足够大的 TTL 或显式 `extend`。

## 限流

```rust
use yang_base::cache::RateLimiter;
use std::time::Duration;

let limiter = RateLimiter::new(redis.clone(), "api");
let decision = limiter.check("user:1", 100, Duration::from_secs(60)).await?;
if !decision.allowed {
    // 返回 429，reset_after_ms 可用于 Retry-After
}
```

## Pub/Sub 与 Streams

```rust
// 订阅（专用长连接，断线自动重连重订阅）
let mut subscriber = redis.pubsub().await?;
subscriber.subscribe(&["events".to_string()]).await?;
if let Some(message) = subscriber.next_message().await? {
    println!("{}: {:?}", message.channel, message.payload);
}

// Streams + 消费者组
redis.xgroup_create("orders", "workers", "0", true).await?;
let id = redis.xadd("orders", "*", &[("order", "42")]).await?;
let reply = redis
    .xread_group(&["orders".to_string()], &[">".to_string()], "workers", "w1", Some(10), None, false)
    .await?;
redis.xack("orders", "workers", &[id]).await?;
```

## 可观测性与韧性

- `response_timeout_ms`：命令级响应超时（`None` 表示不限制）。
- `reconnect_retries` / `reconnect_max_delay_ms`：连接断开时的重试与指数退避。
- `health_check_detailed()`：PING + INFO（role / clients / memory / replication）。
- `close_and_wait(deadline)`：停机时等待借出连接归还。
- feature `metrics`：输出 `db.client.operation.duration` 与错误计数。

## 环境变量（EngineConfig）

| 变量 | 含义 |
|---|---|
| `YANG_REDIS_URL` | Redis 连接串 |
| `YANG_REDIS_MAX_CONNECTIONS` | 最大连接数 |
| `YANG_REDIS_CONNECT_TIMEOUT` | 建连超时（秒） |
| `YANG_REDIS_WAIT_TIMEOUT` | 获取连接超时（秒） |
| `YANG_REDIS_IDLE_TIMEOUT` | 归还回收钩子超时（秒） |
| `YANG_REDIS_RESPONSE_TIMEOUT_MS` | 命令级响应超时（0 表示关闭） |
| `YANG_REDIS_RECONNECT_RETRIES` | 断线重试次数 |
| `YANG_REDIS_RECONNECT_MAX_DELAY_MS` | 重试最大退避 |
| `YANG_REDIS_PIPELINE_MAX_COMMANDS` | Pipeline 命令数上限 |
| `YANG_REDIS_ENABLE_LOGGING` | 是否打印脱敏连接目标 |

TLS 需要 `yang-db/redis-tls` feature（`rediss://`）；Cluster/Sentinel 分别需要
`yang-db/redis-cluster` / `yang-db/redis-sentinel`。
