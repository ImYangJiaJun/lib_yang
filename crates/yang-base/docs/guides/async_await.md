# Rust 异步编程：.await 和 ? 操作符详解

## 基础概念

### 1. `.await` 操作符

`.await` 用于等待异步操作（Future）完成。

#### 示例 1：基本使用

```rust
use tokio::time::{sleep, Duration};

async fn fetch_data() -> String {
    // 模拟网络请求，等待 2 秒
    sleep(Duration::from_secs(2)).await;
    "数据".to_string()
}

#[tokio::main]
async fn main() {
    println!("开始获取数据...");
    
    // .await 会等待 fetch_data 完成
    let data = fetch_data().await;
    
    println!("获取到数据: {}", data);
}
```

**输出：**
```
开始获取数据...
（等待 2 秒）
获取到数据: 数据
```

#### 示例 2：不使用 .await 会怎样？

```rust
async fn fetch_data() -> String {
    "数据".to_string()
}

#[tokio::main]
async fn main() {
    // ❌ 错误：这只是获取了 Future，并没有执行
    let future = fetch_data();
    
    // future 的类型是 impl Future<Output = String>
    // 而不是 String
    
    // ✅ 正确：使用 .await 获取实际结果
    let data = fetch_data().await;
}
```

### 2. `?` 操作符

`?` 用于简化错误处理，自动传播错误。

#### 示例 1：不使用 ?

```rust
fn read_file() -> Result<String, std::io::Error> {
    let file_result = std::fs::read_to_string("file.txt");
    
    let content = match file_result {
        Ok(content) => content,
        Err(e) => return Err(e),  // 手动返回错误
    };
    
    Ok(content)
}
```

#### 示例 2：使用 ?

```rust
fn read_file() -> Result<String, std::io::Error> {
    // ? 会自动处理错误
    let content = std::fs::read_to_string("file.txt")?;
    Ok(content)
}
```

### 3. `.await?` 组合使用

在异步函数中处理可能失败的异步操作。

#### 示例 1：数据库查询

```rust
use yang_base::database::GlobalDatabase;
use yang_base::error::BaseError;

// ❌ 不使用 .await?（繁琐）
async fn get_user_verbose(user_id: i32) -> Result<User, BaseError> {
    // 第一步：获取 Future
    let query_future = GlobalDatabase::table("users")?
        .where_and("id", "=", user_id)
        .select::<User>();
    
    // 第二步：等待 Future 完成
    let query_result = query_future.await;
    
    // 第三步：处理可能的错误
    let users = match query_result {
        Ok(users) => users,
        Err(e) => return Err(e.into()),
    };
    
    // 第四步：获取第一个用户
    let user = users.into_iter().next()
        .ok_or(BaseError::RecordNotFound("用户不存在".to_string()))?;
    
    Ok(user)
}

// ✅ 使用 .await?（简洁）
async fn get_user(user_id: i32) -> Result<User, BaseError> {
    let users = GlobalDatabase::table("users")?
        .where_and("id", "=", user_id)
        .select::<User>()
        .await?;  // 等待并处理错误
    
    let user = users.into_iter().next()
        .ok_or(BaseError::RecordNotFound("用户不存在".to_string()))?;
    
    Ok(user)
}
```

#### 示例 2：Redis 操作

```rust
use yang_base::database::GlobalRedis;
use yang_base::error::BaseError;

// 设置缓存
async fn set_cache(key: &str, value: &str) -> Result<(), BaseError> {
    // .await? 做了两件事：
    // 1. .await - 等待 Redis 操作完成
    // 2. ? - 如果失败，立即返回错误
    GlobalRedis::set(key, value, Some(300)).await?;
    Ok(())
}

// 获取缓存
async fn get_cache(key: &str) -> Result<Option<String>, BaseError> {
    // 同样，等待并处理错误
    let value = GlobalRedis::get(key).await?;
    Ok(value)
}
```

## 错误处理流程

### 示例：完整的错误处理链

```rust
use yang_base::database::{GlobalDatabase, GlobalRedis};
use yang_base::error::BaseError;

async fn get_user_with_cache(user_id: i32) -> Result<User, BaseError> {
    let cache_key = format!("user:{}", user_id);
    
    // 步骤 1：尝试从 Redis 获取缓存
    // .await? 会：
    // - 等待 Redis 操作完成
    // - 如果 Redis 连接失败，立即返回 Err
    if let Some(cached) = GlobalRedis::get(&cache_key).await? {
        // 步骤 2：反序列化缓存数据
        // ? 会：
        // - 如果反序列化失败，立即返回 Err
        return Ok(serde_json::from_str(&cached)?);
    }
    
    // 步骤 3：缓存未命中，查询数据库
    // .await? 会：
    // - 等待数据库查询完成
    // - 如果查询失败，立即返回 Err
    let users = GlobalDatabase::table("users")?
        .where_and("id", "=", user_id)
        .select::<User>()
        .await?;
    
    let user = users.into_iter().next()
        .ok_or(BaseError::RecordNotFound(format!("用户 {}", user_id)))?;
    
    // 步骤 4：写入缓存
    // .await? 会：
    // - 等待 Redis 写入完成
    // - 如果写入失败，立即返回 Err
    let user_json = serde_json::to_string(&user)?;
    GlobalRedis::set(&cache_key, user_json, Some(300)).await?;
    
    Ok(user)
}
```

### 错误传播示例

```rust
// 函数 A 调用函数 B，函数 B 调用函数 C
// 错误会自动向上传播

async fn function_c() -> Result<String, BaseError> {
    // 如果这里失败，错误会返回给 function_b
    GlobalRedis::get("key").await?
        .ok_or(BaseError::RecordNotFound("键不存在".to_string()))
}

async fn function_b() -> Result<String, BaseError> {
    // 如果 function_c 失败，错误会返回给 function_a
    let value = function_c().await?;
    Ok(value)
}

async fn function_a() -> Result<String, BaseError> {
    // 如果 function_b 失败，错误会返回给调用者
    let value = function_b().await?;
    Ok(value)
}

#[tokio::main]
async fn main() {
    // 在这里处理最终的错误
    match function_a().await {
        Ok(value) => println!("成功: {}", value),
        Err(e) => eprintln!("错误: {}", e),
    }
}
```

## 并发操作

### 示例 1：顺序执行（使用 .await）

```rust
async fn sequential_operations() -> Result<(), BaseError> {
    // 操作按顺序执行，总时间 = 时间1 + 时间2 + 时间3
    let user = get_user(1).await?;      // 等待 100ms
    let orders = get_orders(1).await?;  // 等待 100ms
    let profile = get_profile(1).await?; // 等待 100ms
    // 总时间：约 300ms
    
    Ok(())
}
```

### 示例 2：并发执行（使用 tokio::join!）

```rust
async fn concurrent_operations() -> Result<(), BaseError> {
    // 操作并发执行，总时间 = max(时间1, 时间2, 时间3)
    let (user_result, orders_result, profile_result) = tokio::join!(
        get_user(1),      // 100ms
        get_orders(1),    // 100ms
        get_profile(1)    // 100ms
    );
    // 总时间：约 100ms（并发执行）
    
    let user = user_result?;
    let orders = orders_result?;
    let profile = profile_result?;
    
    Ok(())
}
```

## 常见模式

### 模式 1：链式调用

```rust
async fn chain_example() -> Result<User, BaseError> {
    GlobalDatabase::table("users")?
        .where_and("status", "=", 1)
        .where_and("age", ">=", 18)
        .order_by("created_at", false)
        .limit(10)
        .select::<User>()
        .await?  // 最后才 .await?
        .into_iter()
        .next()
        .ok_or(BaseError::RecordNotFound("用户不存在".to_string()))
}
```

### 模式 2：提前返回

```rust
async fn early_return_example(user_id: i32) -> Result<User, BaseError> {
    // 先检查缓存
    let cache_key = format!("user:{}", user_id);
    if let Some(cached) = GlobalRedis::get(&cache_key).await? {
        // 找到缓存，提前返回
        return Ok(serde_json::from_str(&cached)?);
    }
    
    // 缓存未命中，继续查询数据库
    let user = GlobalDatabase::table("users")?
        .where_and("id", "=", user_id)
        .select::<User>()
        .await?
        .into_iter()
        .next()
        .ok_or(BaseError::RecordNotFound(format!("用户 {}", user_id)))?;
    
    Ok(user)
}
```

### 模式 3：循环中使用

```rust
async fn loop_example() -> Result<(), BaseError> {
    let user_ids = vec![1, 2, 3, 4, 5];
    
    for user_id in user_ids {
        // 每次循环都会等待操作完成
        let user = GlobalDatabase::table("users")?
            .where_and("id", "=", user_id)
            .select::<User>()
            .await?;  // 等待当前查询完成
        
        println!("用户: {:?}", user);
    }
    
    Ok(())
}
```

## 错误类型转换

### 自动转换

```rust
use yang_db::DbError;
use yang_base::error::BaseError;

async fn auto_conversion() -> Result<User, BaseError> {
    // DbError 自动转换为 BaseError（因为实现了 From trait）
    let users = GlobalDatabase::table("users")?
        .select::<User>()
        .await?;  // DbError -> BaseError（自动）
    
    Ok(users.into_iter().next().unwrap())
}
```

### 手动转换

```rust
async fn manual_conversion() -> Result<User, BaseError> {
    let users = GlobalDatabase::table("users")?
        .select::<User>()
        .await
        .map_err(|e| BaseError::DatabaseQueryFailed(e.to_string()))?;
    
    Ok(users.into_iter().next().unwrap())
}
```

## 总结

### `.await` 的作用
1. ✅ 等待异步操作完成
2. ✅ 获取 Future 的结果
3. ✅ 非阻塞（不会阻塞线程）
4. ✅ 只能在 async 函数中使用

### `?` 的作用
1. ✅ 简化错误处理
2. ✅ 自动传播错误
3. ✅ 提取 Ok 值
4. ✅ 自动类型转换

### `.await?` 的作用
1. ✅ 等待异步操作完成
2. ✅ 如果成功，提取结果
3. ✅ 如果失败，立即返回错误
4. ✅ 代码简洁，易于阅读

### 使用建议

```rust
// ✅ 推荐：使用 .await?
async fn recommended() -> Result<User, BaseError> {
    let user = get_user(1).await?;
    Ok(user)
}

// ❌ 不推荐：手动处理（太繁琐）
async fn not_recommended() -> Result<User, BaseError> {
    let future = get_user(1);
    let result = future.await;
    let user = match result {
        Ok(u) => u,
        Err(e) => return Err(e),
    };
    Ok(user)
}
```

记住：**`.await?` = 等待 + 错误处理**，是 Rust 异步编程中最常用的模式！
