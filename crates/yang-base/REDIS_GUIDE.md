# yang-base Redis 功能使用指南

## 概述

yang-base 现在支持全局 Redis 访问，通过 `GlobalRedis` 提供线程安全的 Redis 操作接口。

## 初始化

```rust
use yang_base::database::GlobalRedis;
use yang_db::redis::RedisConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 使用默认配置
    GlobalRedis::init("redis://127.0.0.1:6379", RedisConfig::default()).await?;
    
    // 或使用自定义配置
    let config = RedisConfig::new(
        10,    // max_connections
        5,     // connect_timeout (秒)
        300,   // idle_timeout (秒)
        true   // enable_logging
    );
    GlobalRedis::init("redis://127.0.0.1:6379", config).await?;
    
    Ok(())
}
```

## String 操作

```rust
// 设置值
GlobalRedis::set("key", "value", None).await?;

// 设置带过期时间的值（10秒后过期）
GlobalRedis::set("key", "value", Some(10)).await?;

// 获取值
let value: Option<String> = GlobalRedis::get("key").await?;

// 删除键
GlobalRedis::del(&["key1".to_string(), "key2".to_string()]).await?;

// 检查键是否存在
let count = GlobalRedis::exists(&["key".to_string()]).await?;

// 设置过期时间
GlobalRedis::expire("key", 60).await?;

// 获取剩余生存时间
let ttl = GlobalRedis::ttl("key").await?;

// 移除过期时间
GlobalRedis::persist("key").await?;

// 查找匹配的键
let keys = GlobalRedis::keys("user:*").await?;
```

## Hash 操作

```rust
// 设置字段
GlobalRedis::hset("user:1", "name", "Alice").await?;

// 获取字段
let name: Option<String> = GlobalRedis::hget("user:1", "name").await?;

// 删除字段
GlobalRedis::hdel("user:1", &["age".to_string()]).await?;

// 获取所有字段和值
let all: Vec<(String, String)> = GlobalRedis::hgetall("user:1").await?;

// 检查字段是否存在
let exists = GlobalRedis::hexists("user:1", "name").await?;

// 获取字段数量
let count = GlobalRedis::hlen("user:1").await?;
```

## List 操作

```rust
// 从左侧推入
GlobalRedis::lpush("queue", &["task1".to_string(), "task2".to_string()]).await?;

// 从右侧推入
GlobalRedis::rpush("queue", &["task3".to_string()]).await?;

// 从左侧弹出
let task: Option<String> = GlobalRedis::lpop("queue").await?;

// 从右侧弹出
let task: Option<String> = GlobalRedis::rpop("queue").await?;

// 获取列表长度
let len = GlobalRedis::llen("queue").await?;

// 获取范围内的元素
let tasks: Vec<String> = GlobalRedis::lrange("queue", 0, -1).await?;
```

## Set 操作

```rust
// 添加成员
GlobalRedis::sadd("tags", &["rust".to_string(), "redis".to_string()]).await?;

// 移除成员
GlobalRedis::srem("tags", &["redis".to_string()]).await?;

// 检查成员是否存在
let is_member = GlobalRedis::sismember("tags", "rust").await?;

// 获取所有成员
let members: Vec<String> = GlobalRedis::smembers("tags").await?;

// 获取成员数量
let count = GlobalRedis::scard("tags").await?;
```

## Sorted Set 操作

```rust
// 添加成员（分数，成员）
GlobalRedis::zadd("leaderboard", &[
    (100.0, "player1".to_string()),
    (200.0, "player2".to_string()),
]).await?;

// 移除成员
GlobalRedis::zrem("leaderboard", &["player1".to_string()]).await?;

// 获取成员数量
let count = GlobalRedis::zcard("leaderboard").await?;

// 获取排名范围（按分数从小到大）
let players: Vec<String> = GlobalRedis::zrange("leaderboard", 0, -1).await?;
```

## 错误处理

所有 Redis 操作都返回 `Result<T, BaseError>`，可以使用 `?` 操作符进行错误传播：

```rust
use yang_base::error::BaseError;

async fn example() -> Result<(), BaseError> {
    GlobalRedis::set("key", "value", None).await?;
    let value = GlobalRedis::get("key").await?;
    Ok(())
}
```

## 完整示例

```rust
use yang_base::database::GlobalRedis;
use yang_base::error::BaseError;
use yang_db::redis::RedisConfig;

#[tokio::main]
async fn main() -> Result<(), BaseError> {
    // 初始化 Redis
    GlobalRedis::init("redis://127.0.0.1:6379", RedisConfig::default()).await?;
    
    // 用户会话管理
    let session_id = "session:12345";
    GlobalRedis::set(session_id, "user_data", Some(3600)).await?; // 1小时过期
    
    // 缓存用户信息
    GlobalRedis::hset("user:1", "name", "Alice").await?;
    GlobalRedis::hset("user:1", "email", "alice@example.com").await?;
    
    // 任务队列
    GlobalRedis::lpush("tasks", &["task1".to_string(), "task2".to_string()]).await?;
    let task = GlobalRedis::rpop("tasks").await?;
    
    // 标签系统
    GlobalRedis::sadd("article:1:tags", &["rust".to_string(), "redis".to_string()]).await?;
    
    // 排行榜
    GlobalRedis::zadd("scores", &[(100.0, "player1".to_string())]).await?;
    
    Ok(())
}
```

## 注意事项

1. **初始化顺序**：必须先调用 `GlobalRedis::init()` 才能使用其他方法
2. **线程安全**：`GlobalRedis` 是线程安全的，可以在多个线程中使用
3. **连接池**：内部使用连接池管理 Redis 连接，自动处理连接复用
4. **错误处理**：所有操作都可能失败，请妥善处理错误
5. **类型转换**：大部分方法返回 `String` 类型，需要时请自行转换

## 与 MySQL 配合使用

```rust
use yang_base::database::{GlobalDatabase, GlobalRedis};
use yang_db::{DatabaseConfig, redis::RedisConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化 MySQL
    GlobalDatabase::init(
        "mysql://root:password@localhost/mydb",
        DatabaseConfig::default()
    ).await?;
    
    // 初始化 Redis
    GlobalRedis::init(
        "redis://127.0.0.1:6379",
        RedisConfig::default()
    ).await?;
    
    // 先查 Redis 缓存
    if let Some(cached) = GlobalRedis::get("user:1").await? {
        println!("从缓存获取: {}", cached);
    } else {
        // 缓存未命中，查询数据库
        let user = GlobalDatabase::table("users")?
            .where_and("id", "=", 1)
            .select::<User>()
            .await?;
        
        // 写入缓存
        GlobalRedis::set("user:1", &serde_json::to_string(&user)?, Some(300)).await?;
    }
    
    Ok(())
}
```
