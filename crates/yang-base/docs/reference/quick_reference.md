# yang-base 快速参考
> **迁移提示（0.4.0）**：`GlobalRedis` / `GlobalDatabase` 已删除。Redis 新用法见
> [guides/redis.md](../guides/redis.md)；MySQL 请使用 `yang_db::Database` +
> `ToolsBuilder`。下文如出现这两个类型，均为历史 API，不能直接编译。


## 初始化

```rust
use yang_base::http::HttpClient;
use yang_base::tools::ToolsBuilder;
use yang_db::RedisConfig;

// Redis / HTTP 等资源统一由 ToolsBuilder 注入并冻结为 Tools
let tools = ToolsBuilder::new()
    .cache_from_url("redis://127.0.0.1:6379", RedisConfig::default())
    .await?
    .http(HttpClient::new(30)?)
    .build()?;

// 运行期从 Tools 获取（Action 内用 ctx.tools().cache()?）
let redis = tools.cache()?;
```

## MySQL 常用操作

```rust
// 查询
let users: Vec<User> = GlobalDatabase::table("users")?.select().await?;

// 条件查询
let users: Vec<User> = GlobalDatabase::table("users")?
    .where_and("age", ">=", 18)
    .select().await?;

// 插入
let id = GlobalDatabase::table("users")?.insert(&user_data).await?;

// 更新
let n = GlobalDatabase::table("users")?
    .where_and("id", "=", 1)
    .update(&update_data).await?;

// 删除
let n = GlobalDatabase::table("users")?
    .where_and("id", "=", 1)
    .delete().await?;

// 事务
let mut tx = GlobalDatabase::transaction().await?;
tx.table("users").insert(&data).await?;
tx.commit().await?;
```

## Redis 常用操作

```rust
let redis = tools.cache()?;

// String
redis.setex("key", 60, "value").await?;  // 60 秒过期
let val = redis.get("key").await?;

// Hash
redis.hset("user:1", "name", "张三").await?;
let name = redis.hget("user:1", "name").await?;
let all = redis.hgetall("user:1").await?;

// List / Set / Sorted Set
redis.lpush("queue", &["task1".to_string()]).await?;
let task = redis.rpop("queue").await?;
redis.sadd("tags", &["rust".to_string()]).await?;
let tags = redis.smembers("tags").await?;
redis.zadd("scores", &[(100.0, "player1".to_string())]).await?;
let top = redis.zrange("scores", 0, 9).await?;

// 通用操作
redis.del(&["key".to_string()]).await?;
redis.exists(&["key".to_string()]).await?;
redis.expire("key", 3600).await?;
redis.ttl("key").await?;
redis.scan_all(Some("user:*"), Some(100)).await?; // 生产安全，替代 KEYS

// 二进制安全
redis.set_bytes("blob", &[0u8, 0xFF, 0x80]).await?;
let blob = redis.get_bytes("blob").await?;

// JSON / 命名空间
use yang_db::KeyNamespace;
let redis = redis.clone().with_namespace(KeyNamespace::new("myapp", "prod")?);
redis.set_json("user:1", &user, Some(300)).await?;
let user: User = redis.get_json("user:1").await?.unwrap();

// 缓存门面（cache-aside / single-flight / 空值缓存）
use yang_base::cache::{Cache, CacheConfig};
let cache = Cache::new(redis, CacheConfig::default());
let user: User = cache
    .get_or_load("user:1", None, || async { load_user(1).await })
    .await?;

// 分布式锁
let guard = cache.client().lock("job:1")
    .with_ttl(std::time::Duration::from_secs(30))
    .acquire(std::time::Duration::from_secs(5))
    .await?;
guard.release().await?;

// 固定窗口限流
use yang_base::cache::RateLimiter;
let limiter = RateLimiter::new(cache.client().clone(), "api");
let decision = limiter.check("user:1", 100, std::time::Duration::from_secs(60)).await?;
```

## HTTP 请求

```rust
// 从 Tools（或 Action 内 ctx.http()?）获取客户端
let client = tools.http()?;

// GET / POST 等方法均返回 RequestBuilder，send() 后得到 Response
let resp = client.get("https://api.example.com/users").send().await?;

// 带请求头
let resp = client.get(url)
    .header("Authorization", "Bearer token")
    .send().await?
    .json::<ApiResponse>().await?;
```

## Token 管理

```rust
// 生成
let token = TokenManager::generate(&claims, 3600)?;  // 1小时

// 验证
let claims = TokenManager::verify::<Claims>(&token)?;
```

## 错误处理

```rust
use yang_base::error::BaseError;

match operation().await {
    Ok(result) => { /* 成功 */ },
    Err(BaseError::DatabaseNotInitialized) => { /* 数据库未初始化 */ },
    Err(BaseError::RedisNotInitialized) => { /* Redis 未初始化 */ },
    Err(e) => { /* 其他错误 */ },
}

// 获取错误码
let code = error.code();
```

## 缓存模式

```rust
use yang_base::cache::Cache;

async fn get_with_cache<T>(
    cache: &Cache,
    key: &str,
    fetch: impl Future<Output = Result<T, BaseError>>,
) -> Result<T, BaseError>
where
    T: Serialize + DeserializeOwned,
{
    // single-flight + TTL 抖动 + 空值策略由 Cache 统一处理
    cache.get_or_load(key, None, move || fetch).await
}
```

## 完整示例

```rust
use std::time::Duration;
use yang_base::cache::{Cache, CacheConfig};
use yang_base::tools::ToolsBuilder;
use yang_db::RedisConfig;

#[tokio::main]
async fn main() -> Result<(), BaseError> {
    let tools = ToolsBuilder::new()
        .cache_from_url("redis://127.0.0.1:6379", RedisConfig::default())
        .await?
        .build()?;

    let cache = Cache::new(
        tools.cache()?.clone(),
        CacheConfig {
            default_ttl: Duration::from_secs(300),
            ..CacheConfig::default()
        },
    );

    // 查询用户（cache-aside）
    let user: User = cache
        .get_or_load("user:1", None, || async { load_user(1).await })
        .await?;

    println!("用户: {:?}", user);
    Ok(())
}
```

## 配置建议

```rust
// 生产环境配置
DatabaseConfig {
    max_connections: 20,      // 根据负载调整
    connect_timeout: 10,      // 10秒
    idle_timeout: 300,        // 5分钟
    enable_logging: false,    // 生产环境关闭
}

RedisConfig::default()
    .with_max_connections(10)
    .with_connect_timeout(5)
    .with_wait_timeout(10)
    .with_enable_logging(false)
```

## 常见错误码

| 错误码 | 说明 |
|--------|------|
| 200001 | 数据库连接失败 |
| 200008 | 数据库未初始化 |
| 210001 | Redis 连接失败 |
| 210003 | Redis 未初始化 |
| 300002 | HTTP 请求失败 |
| 400005 | Token 已过期 |
| 700006 | 记录未找到 |

## 性能优化提示

1. **批量操作**：使用 `mset`、`lpush` 等批量方法
2. **连接池**：合理配置 `max_connections`
3. **缓存 TTL**：根据数据更新频率设置
4. **索引优化**：确保数据库查询字段有索引
5. **异步并发**：使用 `tokio::join!` 并发执行独立操作

```rust
// 并发查询
let (users, orders) = tokio::join!(
    GlobalDatabase::table("users")?.select::<User>(),
    GlobalDatabase::table("orders")?.select::<Order>()
);
```
