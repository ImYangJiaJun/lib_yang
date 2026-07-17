# yang-base 快速参考

## 初始化

```rust
use yang_base::database::{GlobalDatabase, GlobalRedis};
use yang_base::http::HttpClient;
use yang_base::token::TokenManager;
use yang_base::tools::ToolsBuilder;
use yang_db::{DatabaseConfig, redis::RedisConfig};

// MySQL
GlobalDatabase::init("mysql://root:pass@localhost/db", DatabaseConfig::default()).await?;

// Redis
GlobalRedis::init("redis://127.0.0.1:6379", RedisConfig::default()).await?;

// HTTP 客户端（注册进 Tools 资源槽，运行期经 tools.http()? 获取）
let tools = ToolsBuilder::new().http(HttpClient::new(30)?).build()?;

// Token 管理
TokenManager::init("secret-key")?;
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
// String
GlobalRedis::set("key", "value", Some(60)).await?;  // 60秒过期
let val = GlobalRedis::get("key").await?;

// Hash
GlobalRedis::hset("user:1", "name", "张三").await?;
let name = GlobalRedis::hget("user:1", "name").await?;
let all = GlobalRedis::hgetall("user:1").await?;

// List (队列)
GlobalRedis::lpush("queue", &["task1".to_string()]).await?;
let task = GlobalRedis::rpop("queue").await?;

// Set (集合)
GlobalRedis::sadd("tags", &["rust".to_string()]).await?;
let tags = GlobalRedis::smembers("tags").await?;

// Sorted Set (排行榜)
GlobalRedis::zadd("scores", &[(100.0, "player1".to_string())]).await?;
let top = GlobalRedis::zrange("scores", 0, 9).await?;

// 通用操作
GlobalRedis::del(&["key".to_string()]).await?;
GlobalRedis::exists(&["key".to_string()]).await?;
GlobalRedis::expire("key", 3600).await?;
GlobalRedis::ttl("key").await?;
GlobalRedis::keys("user:*").await?;
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
async fn get_with_cache<T>(key: &str, fetch: impl Future<Output = Result<T, BaseError>>) 
    -> Result<T, BaseError>
where
    T: Serialize + DeserializeOwned,
{
    // 查缓存
    if let Some(cached) = GlobalRedis::get(key).await? {
        if let Ok(value) = serde_json::from_str(&cached) {
            return Ok(value);
        }
    }
    
    // 查数据库
    let value = fetch.await?;
    
    // 写缓存
    GlobalRedis::set(key, &serde_json::to_string(&value)?, Some(300)).await?;
    
    Ok(value)
}
```

## 完整示例

```rust
#[tokio::main]
async fn main() -> Result<(), BaseError> {
    // 初始化
    GlobalDatabase::init("mysql://root:pass@localhost/db", DatabaseConfig::default()).await?;
    GlobalRedis::init("redis://127.0.0.1:6379", RedisConfig::default()).await?;
    
    // 查询用户（带缓存）
    let cache_key = "user:1";
    let user = if let Some(cached) = GlobalRedis::get(cache_key).await? {
        serde_json::from_str(&cached)?
    } else {
        let user: User = GlobalDatabase::table("users")?
            .where_and("id", "=", 1)
            .select().await?
            .into_iter().next()
            .ok_or(BaseError::RecordNotFound("用户1".into()))?;
        
        GlobalRedis::set(cache_key, &serde_json::to_string(&user)?, Some(300)).await?;
        user
    };
    
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

RedisConfig::new(
    10,     // max_connections
    5,      // connect_timeout
    300,    // idle_timeout
    false   // enable_logging
)
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
