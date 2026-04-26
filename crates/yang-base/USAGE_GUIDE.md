# yang-base 完整使用指南

yang-base 是一个功能丰富的 Rust 基础库，提供插件管理、数据库访问、HTTP 客户端、Token 管理等核心功能。

## 目录

1. [数据库管理](#1-数据库管理)
   - [MySQL 数据库](#11-mysql-数据库)
   - [Redis 缓存](#12-redis-缓存)
2. [插件管理](#2-插件管理)
3. [HTTP 客户端](#3-http-客户端)
4. [Token 管理](#4-token-管理)
5. [错误处理](#5-错误处理)
6. [表配置系统](#6-表配置系统)
7. [Action 系统](#7-action-系统)
8. [完整示例](#8-完整示例)

---

## 1. 数据库管理

### 1.1 MySQL 数据库

#### 初始化

```rust
use yang_base::database::GlobalDatabase;
use yang_db::DatabaseConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 使用默认配置
    GlobalDatabase::init(
        "mysql://root:password@localhost:3306/mydb",
        DatabaseConfig::default()
    ).await?;
    
    // 或使用自定义配置
    let config = DatabaseConfig {
        max_connections: 20,
        connect_timeout: 10,
        idle_timeout: 300,
        enable_logging: true,
    };
    GlobalDatabase::init("mysql://root:password@localhost:3306/mydb", config).await?;
    
    Ok(())
}
```

#### 查询构建器

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
struct User {
    id: i32,
    name: String,
    email: String,
    age: Option<i32>,
}

// 查询所有用户
let users: Vec<User> = GlobalDatabase::table("users")?
    .select()
    .await?;

// 条件查询
let active_users: Vec<User> = GlobalDatabase::table("users")?
    .where_and("status", "=", 1)
    .where_and("age", ">=", 18)
    .select()
    .await?;

// 选择特定字段
let users: Vec<User> = GlobalDatabase::table("users")?
    .field("id")
    .field("name")
    .field("email")
    .select()
    .await?;

// 排序和分页
let users: Vec<User> = GlobalDatabase::table("users")?
    .order_by("created_at", false) // false = DESC
    .limit(10)
    .offset(0)
    .select()
    .await?;

// JOIN 查询
let results = GlobalDatabase::table("users")?
    .join("orders", "users.id = orders.user_id")
    .select::<User>()
    .await?;
```

#### 插入数据

```rust
use serde_json::json;

// 插入单条记录
let user_data = json!({
    "name": "张三",
    "email": "zhangsan@example.com",
    "age": 25
});

let user_id = GlobalDatabase::table("users")?
    .insert(&user_data)
    .await?;

println!("插入的用户 ID: {}", user_id);
```

#### 更新数据

```rust
// 更新数据（必须有 WHERE 条件）
let update_data = json!({
    "age": 26,
    "updated_at": chrono::Utc::now()
});

let affected = GlobalDatabase::table("users")?
    .where_and("id", "=", user_id)
    .update(&update_data)
    .await?;

println!("更新了 {} 条记录", affected);
```

#### 删除数据

```rust
// 删除数据（必须有 WHERE 条件）
let affected = GlobalDatabase::table("users")?
    .where_and("id", "=", user_id)
    .delete()
    .await?;

println!("删除了 {} 条记录", affected);
```

#### 事务处理

```rust
// 开始事务
let mut tx = GlobalDatabase::transaction().await?;

// 在事务中执行操作
let user_data = json!({"name": "李四", "email": "lisi@example.com"});
let user_id = tx.table("users").insert(&user_data).await?;

let order_data = json!({"user_id": user_id, "amount": 100.0});
tx.table("orders").insert(&order_data).await?;

// 提交事务
tx.commit().await?;

// 或回滚事务
// tx.rollback().await?;
```

#### 原生 SQL

```rust
// 执行查询
let users: Vec<User> = GlobalDatabase::query(
    "SELECT * FROM users WHERE age > 18"
).await?;

// 执行语句（INSERT/UPDATE/DELETE）
let affected = GlobalDatabase::execute(
    "UPDATE users SET status = 1 WHERE age > 18"
).await?;
```

---

### 1.2 Redis 缓存

#### 初始化

```rust
use yang_base::database::GlobalRedis;
use yang_db::redis::RedisConfig;

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
```

#### String 操作

```rust
// 设置值
GlobalRedis::set("user:name", "张三", None).await?;

// 设置带过期时间的值（60秒后过期）
GlobalRedis::set("session:12345", "user_data", Some(60)).await?;

// 获取值
if let Some(name) = GlobalRedis::get("user:name").await? {
    println!("用户名: {}", name);
}

// 删除键
GlobalRedis::del(&["key1".to_string(), "key2".to_string()]).await?;

// 检查键是否存在
let count = GlobalRedis::exists(&["user:name".to_string()]).await?;
if count > 0 {
    println!("键存在");
}

// 设置过期时间
GlobalRedis::expire("user:name", 3600).await?; // 1小时后过期

// 获取剩余生存时间
let ttl = GlobalRedis::ttl("user:name").await?;
println!("剩余 {} 秒", ttl);

// 移除过期时间
GlobalRedis::persist("user:name").await?;

// 查找匹配的键
let keys = GlobalRedis::keys("user:*").await?;
```

#### Hash 操作

```rust
// 设置用户信息
GlobalRedis::hset("user:1", "name", "张三").await?;
GlobalRedis::hset("user:1", "email", "zhangsan@example.com").await?;
GlobalRedis::hset("user:1", "age", "25").await?;

// 获取单个字段
if let Some(name) = GlobalRedis::hget("user:1", "name").await? {
    println!("用户名: {}", name);
}

// 获取所有字段
let user_data = GlobalRedis::hgetall("user:1").await?;
for (field, value) in user_data {
    println!("{}: {}", field, value);
}

// 检查字段是否存在
let exists = GlobalRedis::hexists("user:1", "name").await?;

// 获取字段数量
let count = GlobalRedis::hlen("user:1").await?;

// 删除字段
GlobalRedis::hdel("user:1", &["age".to_string()]).await?;
```

#### List 操作（队列）

```rust
// 任务队列 - 从左侧推入
GlobalRedis::lpush("tasks", &[
    "task1".to_string(),
    "task2".to_string(),
    "task3".to_string()
]).await?;

// 从右侧弹出（FIFO 队列）
while let Some(task) = GlobalRedis::rpop("tasks").await? {
    println!("处理任务: {}", task);
}

// 从右侧推入
GlobalRedis::rpush("logs", &["log1".to_string()]).await?;

// 从左侧弹出
let log = GlobalRedis::lpop("logs").await?;

// 获取列表长度
let len = GlobalRedis::llen("tasks").await?;

// 获取范围内的元素
let all_tasks = GlobalRedis::lrange("tasks", 0, -1).await?;
```

#### Set 操作（集合）

```rust
// 添加标签
GlobalRedis::sadd("article:1:tags", &[
    "rust".to_string(),
    "redis".to_string(),
    "database".to_string()
]).await?;

// 检查成员是否存在
let is_member = GlobalRedis::sismember("article:1:tags", "rust").await?;

// 获取所有成员
let tags = GlobalRedis::smembers("article:1:tags").await?;

// 获取成员数量
let count = GlobalRedis::scard("article:1:tags").await?;

// 移除成员
GlobalRedis::srem("article:1:tags", &["database".to_string()]).await?;
```

#### Sorted Set 操作（排行榜）

```rust
// 添加分数
GlobalRedis::zadd("leaderboard", &[
    (100.0, "player1".to_string()),
    (200.0, "player2".to_string()),
    (150.0, "player3".to_string()),
]).await?;

// 获取排名（从低到高）
let players = GlobalRedis::zrange("leaderboard", 0, -1).await?;

// 获取前10名
let top10 = GlobalRedis::zrange("leaderboard", 0, 9).await?;

// 获取成员数量
let count = GlobalRedis::zcard("leaderboard").await?;

// 移除成员
GlobalRedis::zrem("leaderboard", &["player1".to_string()]).await?;
```

#### 缓存模式

```rust
// 查询时先查缓存
async fn get_user(user_id: i32) -> Result<User, BaseError> {
    let cache_key = format!("user:{}", user_id);
    
    // 先查 Redis
    if let Some(cached) = GlobalRedis::get(&cache_key).await? {
        // 反序列化缓存数据
        return Ok(serde_json::from_str(&cached)?);
    }
    
    // 缓存未命中，查询数据库
    let user = GlobalDatabase::table("users")?
        .where_and("id", "=", user_id)
        .select::<User>()
        .await?
        .into_iter()
        .next()
        .ok_or(BaseError::RecordNotFound(format!("用户 {}", user_id)))?;
    
    // 写入缓存（5分钟过期）
    let user_json = serde_json::to_string(&user)?;
    GlobalRedis::set(&cache_key, user_json, Some(300)).await?;
    
    Ok(user)
}
```

---

## 2. 插件管理

### 定义插件

```rust
use yang_base::plugin::{Plugin, PluginMetadata, PluginContext};
use yang_base::error::BaseError;
use async_trait::async_trait;

pub struct MyPlugin {
    name: String,
}

#[async_trait]
impl Plugin for MyPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "my_plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "我的插件".to_string(),
            dependencies: vec![], // 依赖的其他插件
        }
    }
    
    async fn init(&mut self, context: &PluginContext) -> Result<(), BaseError> {
        println!("插件 {} 初始化", self.name);
        // 初始化逻辑
        Ok(())
    }
    
    async fn shutdown(&mut self) -> Result<(), BaseError> {
        println!("插件 {} 关闭", self.name);
        // 清理逻辑
        Ok(())
    }
}
```

### 使用插件管理器

```rust
use yang_base::plugin::PluginManager;

let mut manager = PluginManager::new();

// 注册插件
let plugin = Box::new(MyPlugin { name: "my_plugin".to_string() });
manager.register(plugin)?;

// 初始化所有插件
manager.init_all().await?;

// 获取插件
if let Some(plugin) = manager.get("my_plugin") {
    // 使用插件
}

// 关闭所有插件
manager.shutdown_all().await?;
```

---

## 3. HTTP 客户端

### 初始化

```rust
use yang_base::http::HttpClient;

// 初始化全局 HTTP 客户端（30秒超时）
HttpClient::init_global(30)?;
```

### GET 请求

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct ApiResponse {
    code: i32,
    message: String,
    data: serde_json::Value,
}

// 发送 GET 请求
let response: ApiResponse = HttpClient::get("https://api.example.com/users")
    .await?;

println!("响应: {:?}", response);
```

### POST 请求

```rust
#[derive(Debug, Serialize)]
struct CreateUserRequest {
    name: String,
    email: String,
}

let request = CreateUserRequest {
    name: "张三".to_string(),
    email: "zhangsan@example.com".to_string(),
};

// 发送 POST 请求
let response: ApiResponse = HttpClient::post(
    "https://api.example.com/users",
    &request
).await?;
```

### 带请求头的请求

```rust
use yang_base::http::HttpClient;

let client = HttpClient::get_global()?;
let response = client
    .get("https://api.example.com/protected")
    .header("Authorization", "Bearer token123")
    .send()
    .await?
    .json::<ApiResponse>()
    .await?;
```

---

## 4. Token 管理

### 初始化

```rust
use yang_base::token::TokenManager;

// 使用密钥初始化
TokenManager::init("your-secret-key-here")?;
```

### 生成 Token

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    user_id: i32,
    username: String,
    role: String,
}

let claims = Claims {
    user_id: 1,
    username: "zhangsan".to_string(),
    role: "admin".to_string(),
};

// 生成 Token（24小时过期）
let token = TokenManager::generate(&claims, 24 * 3600)?;
println!("Token: {}", token);
```

### 验证 Token

```rust
// 验证并解析 Token
match TokenManager::verify::<Claims>(&token) {
    Ok(claims) => {
        println!("用户 ID: {}", claims.user_id);
        println!("用户名: {}", claims.username);
        println!("角色: {}", claims.role);
    }
    Err(e) => {
        println!("Token 验证失败: {}", e);
    }
}
```

---

## 5. 错误处理

### 错误类型

```rust
use yang_base::error::BaseError;

// 所有错误都是 BaseError 枚举类型
match some_operation().await {
    Ok(result) => println!("成功: {:?}", result),
    Err(BaseError::DatabaseNotInitialized) => {
        println!("数据库未初始化");
    }
    Err(BaseError::RedisNotInitialized) => {
        println!("Redis 未初始化");
    }
    Err(BaseError::PluginNotFound(name)) => {
        println!("插件未找到: {}", name);
    }
    Err(e) => {
        println!("其他错误: {}", e);
    }
}
```

### 错误码

```rust
// 获取错误码（用于 API 响应）
let error = BaseError::DatabaseNotInitialized;
let code = error.code(); // 返回 200008
```

### 错误转换

```rust
use yang_db::DbError;

// yang_db::DbError 自动转换为 BaseError
let db_result: Result<Vec<User>, DbError> = /* ... */;
let base_result: Result<Vec<User>, BaseError> = db_result.map_err(Into::into);
```

---

## 6. 表配置系统

### 定义表配置

```rust
use yang_base::table::{TableConfig, FieldConfig, FieldType};

let table_config = TableConfig {
    name: "users".to_string(),
    label: "用户表".to_string(),
    fields: vec![
        FieldConfig {
            name: "id".to_string(),
            label: "ID".to_string(),
            field_type: FieldType::Integer,
            required: true,
            readonly: true,
            ..Default::default()
        },
        FieldConfig {
            name: "name".to_string(),
            label: "姓名".to_string(),
            field_type: FieldType::String,
            required: true,
            max_length: Some(50),
            ..Default::default()
        },
        FieldConfig {
            name: "email".to_string(),
            label: "邮箱".to_string(),
            field_type: FieldType::String,
            required: true,
            ..Default::default()
        },
    ],
    ..Default::default()
};
```

### 字段验证

```rust
use serde_json::json;

let data = json!({
    "name": "张三",
    "email": "zhangsan@example.com"
});

// 验证数据
table_config.validate(&data)?;
```

---

## 7. Action 系统

### 定义 Action

```rust
use yang_base::action::{Action, ActionContext, ActionResponse};
use yang_base::error::BaseError;
use async_trait::async_trait;
use serde_json::Value;

pub struct GetUserAction;

#[async_trait]
impl Action for GetUserAction {
    fn name(&self) -> &str {
        "get_user"
    }
    
    fn description(&self) -> &str {
        "获取用户信息"
    }
    
    async fn execute(&self, context: &ActionContext) -> Result<ActionResponse, BaseError> {
        // 获取参数
        let user_id: i32 = context.get_param("user_id")?;
        
        // 查询用户
        let user = GlobalDatabase::table("users")?
            .where_and("id", "=", user_id)
            .select::<User>()
            .await?
            .into_iter()
            .next()
            .ok_or(BaseError::RecordNotFound(format!("用户 {}", user_id)))?;
        
        // 返回响应
        Ok(ActionResponse::success(serde_json::to_value(user)?))
    }
}
```

### 使用 Action

```rust
use yang_base::action::ActionRegistry;

let mut registry = ActionRegistry::new();

// 注册 Action
registry.register(Box::new(GetUserAction))?;

// 执行 Action
let context = ActionContext::new(json!({"user_id": 1}));
let response = registry.execute("get_user", &context).await?;

println!("响应: {:?}", response);
```

---

## 8. 完整示例

### Web 应用示例

```rust
use yang_base::database::{GlobalDatabase, GlobalRedis};
use yang_base::http::HttpClient;
use yang_base::token::TokenManager;
use yang_base::error::BaseError;
use yang_db::{DatabaseConfig, redis::RedisConfig};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
struct User {
    id: i32,
    name: String,
    email: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TokenClaims {
    user_id: i32,
    username: String,
}

#[tokio::main]
async fn main() -> Result<(), BaseError> {
    // 1. 初始化所有组件
    GlobalDatabase::init(
        "mysql://root:password@localhost/mydb",
        DatabaseConfig::default()
    ).await?;
    
    GlobalRedis::init(
        "redis://127.0.0.1:6379",
        RedisConfig::default()
    ).await?;
    
    HttpClient::init_global(30)?;
    TokenManager::init("my-secret-key")?;
    
    // 2. 用户登录
    let user = login("zhangsan", "password123").await?;
    
    // 3. 生成 Token
    let claims = TokenClaims {
        user_id: user.id,
        username: user.name.clone(),
    };
    let token = TokenManager::generate(&claims, 24 * 3600)?;
    
    // 4. 缓存用户信息
    let cache_key = format!("user:{}", user.id);
    GlobalRedis::set(&cache_key, &serde_json::to_string(&user)?, Some(3600)).await?;
    
    // 5. 记录登录日志
    GlobalRedis::lpush("login_logs", &[
        format!("用户 {} 于 {} 登录", user.name, chrono::Utc::now())
    ]).await?;
    
    println!("登录成功！Token: {}", token);
    
    Ok(())
}

async fn login(username: &str, password: &str) -> Result<User, BaseError> {
    // 查询用户
    let users = GlobalDatabase::table("users")?
        .where_and("name", "=", username)
        .select::<User>()
        .await?;
    
    let user = users.into_iter().next()
        .ok_or(BaseError::UserNotFound(username.to_string()))?;
    
    // 验证密码（实际应用中应该使用哈希）
    // verify_password(password, &user.password_hash)?;
    
    Ok(user)
}
```

### 缓存服务示例

```rust
use std::time::Duration;

pub struct CacheService;

impl CacheService {
    /// 获取或设置缓存
    pub async fn get_or_set<T, F, Fut>(
        key: &str,
        ttl: i64,
        fetch_fn: F,
    ) -> Result<T, BaseError>
    where
        T: Serialize + for<'de> Deserialize<'de>,
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, BaseError>>,
    {
        // 先查缓存
        if let Some(cached) = GlobalRedis::get(key).await? {
            if let Ok(value) = serde_json::from_str::<T>(&cached) {
                return Ok(value);
            }
        }
        
        // 缓存未命中，执行获取函数
        let value = fetch_fn().await?;
        
        // 写入缓存
        let json = serde_json::to_string(&value)?;
        GlobalRedis::set(key, json, Some(ttl)).await?;
        
        Ok(value)
    }
    
    /// 删除缓存
    pub async fn delete(key: &str) -> Result<(), BaseError> {
        GlobalRedis::del(&[key.to_string()]).await?;
        Ok(())
    }
    
    /// 批量删除缓存
    pub async fn delete_pattern(pattern: &str) -> Result<(), BaseError> {
        let keys = GlobalRedis::keys(pattern).await?;
        if !keys.is_empty() {
            GlobalRedis::del(&keys).await?;
        }
        Ok(())
    }
}

// 使用示例
async fn get_user_with_cache(user_id: i32) -> Result<User, BaseError> {
    let cache_key = format!("user:{}", user_id);
    
    CacheService::get_or_set(&cache_key, 300, || async move {
        GlobalDatabase::table("users")?
            .where_and("id", "=", user_id)
            .select::<User>()
            .await?
            .into_iter()
            .next()
            .ok_or(BaseError::RecordNotFound(format!("用户 {}", user_id)))
    }).await
}
```

---

## 常见问题

### Q: 如何处理数据库连接失败？

```rust
match GlobalDatabase::init(url, config).await {
    Ok(_) => println!("数据库连接成功"),
    Err(BaseError::DatabaseConnectionFailed(msg)) => {
        eprintln!("数据库连接失败: {}", msg);
        // 重试或退出
    }
    Err(e) => eprintln!("其他错误: {}", e),
}
```

### Q: 如何在多线程环境中使用？

所有全局组件（GlobalDatabase、GlobalRedis、HttpClient、TokenManager）都是线程安全的，可以在多线程环境中直接使用。

```rust
use tokio::task;

let handles: Vec<_> = (0..10).map(|i| {
    task::spawn(async move {
        let user = get_user_with_cache(i).await?;
        Ok::<_, BaseError>(user)
    })
}).collect();

for handle in handles {
    let user = handle.await??;
    println!("用户: {:?}", user);
}
```

### Q: 如何优雅关闭？

```rust
// 在应用关闭时，确保所有资源被正确释放
// GlobalDatabase 和 GlobalRedis 会在 Drop 时自动清理连接池
// 如果使用了插件管理器，需要手动关闭
plugin_manager.shutdown_all().await?;
```

---

## 最佳实践

1. **初始化顺序**：先初始化数据库和 Redis，再初始化其他组件
2. **错误处理**：使用 `?` 操作符传播错误，在顶层统一处理
3. **缓存策略**：合理设置 TTL，避免缓存雪崩
4. **连接池配置**：根据实际负载调整 `max_connections`
5. **日志记录**：开启 `enable_logging` 便于调试
6. **安全性**：Token 密钥使用环境变量，不要硬编码
7. **性能优化**：使用批量操作减少网络往返

---

## 更多资源

- [Redis 功能详细指南](./REDIS_GUIDE.md)
- [Action 系统示例](./src/action/ACTION_EXAMPLES.md)
- [yang-db 文档](../yang-db/README.md)
