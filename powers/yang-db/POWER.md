---
name: "yang-db"
displayName: "YANG-DB 数据库操作库"
description: "基于 Rust 的类型安全 MySQL 查询构建器，提供链式 API、参数化查询和完善的错误处理"
keywords: ["rust", "mysql", "database", "query-builder", "sqlx", "orm", "crud"]
author: "YANG Team"
---

# YANG-DB 数据库操作库

## 概述

yang-db 是一个基于 Rust 的类型安全 MySQL 数据库操作库。

### 核心特性

- ✅ **链式调用 API** - 流畅的开发体验
- ✅ **类型安全** - 编译时捕获错误
- ✅ **防 SQL 注入** - 参数化查询
- ✅ **异步支持** - 基于 tokio 和 sqlx
- ✅ **事务管理** - 完整的事务支持
- ✅ **特殊字段类型** - JSON、DATETIME、DECIMAL、BLOB 等
- ✅ **中文错误消息** - 友好的错误提示
- ✅ **安全设计** - UPDATE/DELETE 必须带 WHERE 条件

## 快速开始

### 1. 添加依赖

在 `Cargo.toml` 中添加：

```toml
[dependencies]
yang-db = { path = "crates/yang-db" }
tokio = { version = "1.51", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### 2. 连接数据库

```rust
use yang_db::Database;

#[tokio::main]
async fn main() -> Result<(), yang_db::DbError> {
    // 连接数据库
    let db = Database::connect("mysql://user:password@localhost:3306/database").await?;
    
    // 使用数据库...
    
    Ok(())
}
```

### 3. 基本 CRUD 操作

```rust
use yang_db::Database;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
struct User {
    id: i32,
    name: String,
    email: String,
    age: i32,
}

#[tokio::main]
async fn main() -> Result<(), yang_db::DbError> {
    let db = Database::connect("mysql://user:password@localhost:3306/test").await?;
    
    // 插入数据
    let user_id = db.table("users")
        .insert(&json!({
            "name": "张三",
            "email": "zhangsan@example.com",
            "age": 25
        }))
        .await?;
    
    // 查询单条记录
    let user: Option<User> = db.table("users")
        .where_and("id", "=", user_id)
        .find()
        .await?;
    
    // 查询多条记录
    let users: Vec<User> = db.table("users")
        .where_and("age", ">", 18)
        .order("name", true)
        .select()
        .await?;
    
    // 更新数据
    let affected = db.table("users")
        .where_and("id", "=", user_id)
        .update(&json!({"age": 26}))
        .await?;
    
    // 删除数据
    let affected = db.table("users")
        .where_and("id", "=", user_id)
        .delete()
        .await?;
    
    Ok(())
}
```

## 核心 API

### 数据库连接

#### Database::connect()
```rust
// 基本连接
let db = Database::connect("mysql://user:password@localhost:3306/database").await?;

// 自定义配置
use yang_db::DatabaseConfig;

let config = DatabaseConfig {
    max_connections: 20,
    connect_timeout: 30,
    idle_timeout: 600,
    enable_logging: true,
};

let db = Database::connect_with_config("mysql://...", config).await?;
```

### 查询构建器

#### 选择表
```rust
let builder = db.table("users");
```

#### 字段选择
```rust
// 选择所有字段
db.table("users").select::<User>().await?;

// 选择特定字段
db.table("users")
    .field("id")
    .field("name")
    .select::<UserBasic>().await?;

// 选择多个字段
db.table("users")
    .fields(&["id", "name", "email"])
    .select::<UserBasic>().await?;

// 去重查询
db.table("users")
    .field("city")
    .distinct()
    .select::<City>().await?;
```

#### WHERE 条件

```rust
// 基本条件
db.table("users")
    .where_and("age", ">", 18)
    .where_and("status", "=", 1)
    .select::<User>().await?;

// OR 条件
db.table("users")
    .where_or("status", "=", 1)
    .where_or("status", "=", 2)
    .select::<User>().await?;

// IN 条件
db.table("users")
    .where_in("id", vec![1, 2, 3, 5, 8])
    .select::<User>().await?;

// BETWEEN 条件
db.table("users")
    .where_between("age", 18, 65)
    .select::<User>().await?;

// LIKE 条件
db.table("users")
    .where_and("email", "like", "%@example.com")
    .select::<User>().await?;
```

#### JOIN 操作

```rust
// INNER JOIN
db.table("users")
    .join("orders", "users.id = orders.user_id")
    .select::<UserOrder>().await?;

// LEFT JOIN
db.table("users")
    .left_join("orders", "users.id = orders.user_id")
    .select::<UserOrder>().await?;

// RIGHT JOIN
db.table("users")
    .right_join("orders", "users.id = orders.user_id")
    .select::<UserOrder>().await?;
```

#### 排序和分组

```rust
// 排序
db.table("users")
    .order("name", true)   // 升序
    .order("age", false)   // 降序
    .select::<User>().await?;

// 分组
db.table("orders")
    .field("user_id")
    .field("COUNT(*) as order_count")
    .group("user_id")
    .select::<UserOrderCount>().await?;
```

#### 分页

```rust
// 限制返回数量
db.table("users")
    .limit(10)
    .select::<User>().await?;

// 分页查询
db.table("users")
    .limit(10)
    .offset(20)
    .select::<User>().await?;
```

### 查询方法

#### find() - 查询单条记录
```rust
let user: Option<User> = db.table("users")
    .where_and("id", "=", 1)
    .find()
    .await?;

match user {
    Some(u) => println!("找到用户: {:?}", u),
    None => println!("用户不存在"),
}
```

#### select() - 查询多条记录
```rust
let users: Vec<User> = db.table("users")
    .where_and("status", "=", 1)
    .select()
    .await?;

println!("找到 {} 个用户", users.len());
```

#### value() - 查询单个字段值
```rust
// 查询字符串字段
let name: Option<String> = db.table("users")
    .where_and("id", "=", 1)
    .value("name")
    .await?;

// 查询数值字段
let age: Option<i32> = db.table("users")
    .where_and("id", "=", 1)
    .value("age")
    .await?;
```

#### count() - 统计记录数
```rust
let total: i64 = db.table("users").count().await?;
let active: i64 = db.table("users")
    .where_and("status", "=", 1)
    .count()
    .await?;
```

#### sum() - 计算字段总和
```rust
let total_amount: Option<f64> = db.table("orders")
    .where_and("status", "=", "completed")
    .sum("amount")
    .await?;

println!("总金额: {:.2}", total_amount.unwrap_or(0.0));
```

### 数据修改

#### insert() - 插入单条记录
```rust
use serde_json::json;

let user_id = db.table("users")
    .insert(&json!({
        "name": "张三",
        "email": "zhangsan@example.com",
        "age": 25
    }))
    .await?;

println!("插入成功，ID: {}", user_id);
```

#### insert_batch() - 批量插入
```rust
let users = vec![
    json!({"name": "张三", "email": "zhangsan@example.com", "age": 25}),
    json!({"name": "李四", "email": "lisi@example.com", "age": 30}),
    json!({"name": "王五", "email": "wangwu@example.com", "age": 28}),
];

let affected = db.table("users")
    .insert_batch(&users)
    .await?;

println!("批量插入成功，影响 {} 行", affected);
```

#### update() - 更新数据
```rust
// 必须提供 WHERE 条件
let affected = db.table("users")
    .where_and("id", "=", 1)
    .update(&json!({
        "name": "张三（已更新）",
        "age": 26
    }))
    .await?;

println!("更新了 {} 行", affected);
```

#### delete() - 删除数据
```rust
// 必须提供 WHERE 条件
let affected = db.table("users")
    .where_and("id", "=", 1)
    .delete()
    .await?;

println!("删除了 {} 行", affected);
```

## 特殊字段类型

### JSON 字段
```rust
let order_id = db.table("orders")
    .json("items")  // 标记 items 字段为 JSON 类型
    .insert(&json!({
        "user_id": 1,
        "total": 199.99,
        "items": [{"id": 1, "qty": 2}, {"id": 2, "qty": 1}]
    }))
    .await?;
```

### DATETIME 字段
```rust
let user_id = db.table("users")
    .datetime("last_login")
    .insert(&json!({
        "name": "张三",
        "last_login": "2024-01-01 12:00:00"
    }))
    .await?;
```

### TIMESTAMP 字段
```rust
let log_id = db.table("logs")
    .timestamp("created_at")
    .insert(&json!({
        "message": "用户登录",
        "created_at": 1704096000  // Unix 时间戳
    }))
    .await?;
```

### DECIMAL 字段
```rust
let product_id = db.table("products")
    .decimal("price")
    .decimal("discount_rate")
    .insert(&json!({
        "name": "商品A",
        "price": 199.99,
        "discount_rate": 0.15
    }))
    .await?;
```

### BLOB 字段
```rust
let file_id = db.table("files")
    .blob("content")
    .insert(&json!({
        "name": "document.pdf",
        "content": "base64_encoded_content_here"
    }))
    .await?;
```

### TEXT 字段
```rust
let article_id = db.table("articles")
    .text("content")
    .insert(&json!({
        "title": "文章标题",
        "content": "这是一篇很长的文章内容..."
    }))
    .await?;
```

## 事务管理

```rust
use yang_db::Database;

#[tokio::main]
async fn main() -> Result<(), yang_db::DbError> {
    let db = Database::connect("mysql://...").await?;
    
    // 开始事务
    let mut tx = db.transaction().await?;
    
    // 在事务中执行操作
    let user_id = tx.table("users")
        .insert(&json!({"name": "张三", "balance": 1000}))
        .await?;
    
    let order_id = tx.table("orders")
        .insert(&json!({"user_id": user_id, "amount": 100}))
        .await?;
    
    tx.table("users")
        .where_and("id", "=", user_id)
        .update(&json!({"balance": 900}))
        .await?;
    
    // 提交事务
    tx.commit().await?;
    
    // 或者回滚事务
    // tx.rollback().await?;
    
    Ok(())
}
```

## 原生 SQL 支持

```rust
// 执行原生 SELECT 查询
let users: Vec<User> = db.query("SELECT * FROM users WHERE age > 18").await?;

// 执行原生 INSERT/UPDATE/DELETE
let affected = db.execute("UPDATE users SET status = 1 WHERE age > 18").await?;
```

## 数据库管理

```rust
// 初始化数据库（执行 SQL 脚本）
db.init(r#"
    CREATE TABLE IF NOT EXISTS users (
        id INT PRIMARY KEY AUTO_INCREMENT,
        name VARCHAR(100) NOT NULL,
        email VARCHAR(100) UNIQUE NOT NULL,
        age INT
    );
    
    CREATE TABLE IF NOT EXISTS orders (
        id INT PRIMARY KEY AUTO_INCREMENT,
        user_id INT NOT NULL,
        total DECIMAL(10, 2) NOT NULL
    );
"#).await?;

// 创建表
db.create_table(r#"
    CREATE TABLE products (
        id INT PRIMARY KEY AUTO_INCREMENT,
        name VARCHAR(100) NOT NULL,
        price DECIMAL(10, 2) NOT NULL
    )
"#).await?;

// 删除表
db.drop_table("products").await?;

// 检查表是否存在
if db.table_exists("users").await? {
    println!("users 表存在");
}
```

## 错误处理

### 错误类型

yang-db 提供了详细的错误类型：

```rust
pub enum DbError {
    ConnectionError(String),      // 连接错误
    QueryError(String),            // 查询错误
    SqlSyntaxError(String),        // SQL 语法错误
    ConstraintError(String),       // 约束错误
    TypeConversionError(String),   // 类型转换错误
    SerializationError(String),    // 序列化错误
    DeserializationError(String),  // 反序列化错误
    TransactionError(String),      // 事务错误
    TableNotFound(String),         // 表不存在
    MissingWhereClause,            // 缺少 WHERE 条件
    Unknown(String),               // 未知错误
}
```

### 错误处理示例

```rust
use yang_db::DbError;

match db.table("users").where_and("id", "=", 1).find::<User>().await {
    Ok(Some(user)) => println!("找到用户: {:?}", user),
    Ok(None) => println!("用户不存在"),
    Err(DbError::ConnectionError(msg)) => eprintln!("连接错误: {}", msg),
    Err(DbError::QueryError(msg)) => eprintln!("查询错误: {}", msg),
    Err(DbError::MissingWhereClause) => eprintln!("缺少 WHERE 条件"),
    Err(e) => eprintln!("其他错误: {}", e),
}
```

## 安全特性

### 1. 防止 SQL 注入
所有查询都使用参数化查询，自动防止 SQL 注入：

```rust
// 安全：参数会被正确转义
let user_input = "'; DROP TABLE users; --";
db.table("users")
    .where_and("name", "=", user_input)
    .select::<User>()
    .await?;
```

### 2. 防止全表更新/删除
UPDATE 和 DELETE 操作必须提供 WHERE 条件：

```rust
// 这会返回 MissingWhereClause 错误
let result = db.table("users")
    .update(&json!({"status": 1}))
    .await;

// 必须提供 WHERE 条件
let result = db.table("users")
    .where_and("age", ">", 18)
    .update(&json!({"status": 1}))
    .await?;
```

## 日志记录

启用日志可以帮助调试和监控：

```rust
use yang_db::{Database, DatabaseConfig};

#[tokio::main]
async fn main() -> Result<(), yang_db::DbError> {
    // 初始化日志
    env_logger::init();
    
    // 启用数据库日志
    let config = DatabaseConfig {
        enable_logging: true,
        ..Default::default()
    };
    
    let db = Database::connect_with_config("mysql://...", config).await?;
    
    // 所有操作都会记录日志
    let users: Vec<User> = db.table("users")
        .where_and("status", "=", 1)
        .select()
        .await?;
    
    Ok(())
}
```

日志输出示例：
```
[DEBUG] 执行 select() 查询: SELECT * FROM users WHERE status = ?
[DEBUG] 参数: [Int(1)]
[DEBUG] select() 查询成功，返回 5 条记录
```

## 性能优化建议

### 1. 使用连接池
Database 内部使用连接池，可以复用连接：

```rust
let config = DatabaseConfig {
    max_connections: 20,  // 增加连接池大小
    ..Default::default()
};
```

### 2. 批量操作
使用 `insert_batch()` 而不是多次 `insert()`：

```rust
// 好：一次批量插入
db.table("users").insert_batch(&users).await?;

// 不好：多次单条插入
for user in &users {
    db.table("users").insert(user).await?;
}
```

### 3. 只查询需要的字段
```rust
// 好：只查询需要的字段
db.table("users")
    .fields(&["id", "name"])
    .select::<UserBasic>()
    .await?;

// 不好：查询所有字段
db.table("users").select::<User>().await?;
```

### 4. 使用索引
确保 WHERE 条件中的字段有索引：

```sql
CREATE INDEX idx_users_status ON users(status);
CREATE INDEX idx_users_age ON users(age);
```

### 5. 使用 LIMIT
对于大数据集，使用 LIMIT 限制返回数量：

```rust
db.table("users")
    .limit(100)
    .select::<User>()
    .await?;
```

## 常见模式

### 模式 1：分页查询
```rust
async fn get_users_page(
    db: &Database,
    page: u64,
    page_size: u64
) -> Result<Vec<User>, DbError> {
    let offset = (page - 1) * page_size;
    
    db.table("users")
        .order("id", true)
        .limit(page_size)
        .offset(offset)
        .select()
        .await
}
```

### 模式 2：条件构建
```rust
async fn search_users(
    db: &Database,
    name: Option<&str>,
    min_age: Option<i32>,
    status: Option<i32>
) -> Result<Vec<User>, DbError> {
    let mut builder = db.table("users");
    
    if let Some(n) = name {
        builder = builder.where_and("name", "like", format!("%{}%", n));
    }
    
    if let Some(age) = min_age {
        builder = builder.where_and("age", ">=", age);
    }
    
    if let Some(s) = status {
        builder = builder.where_and("status", "=", s);
    }
    
    builder.select().await
}
```

### 模式 3：事务中的错误处理
```rust
async fn transfer_money(
    db: &Database,
    from_user: i32,
    to_user: i32,
    amount: f64
) -> Result<(), DbError> {
    let mut tx = db.transaction().await?;
    
    // 扣款
    let affected = tx.table("users")
        .where_and("id", "=", from_user)
        .where_and("balance", ">=", amount)
        .update(&json!({"balance": format!("balance - {}", amount)}))
        .await?;
    
    if affected == 0 {
        tx.rollback().await?;
        return Err(DbError::QueryError("余额不足".to_string()));
    }
    
    // 加款
    tx.table("users")
        .where_and("id", "=", to_user)
        .update(&json!({"balance": format!("balance + {}", amount)}))
        .await?;
    
    tx.commit().await?;
    Ok(())
}
```

## 测试建议

### 1. 使用测试数据库
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    async fn setup_test_db() -> Database {
        let db = Database::connect("mysql://root:password@localhost/test_db")
            .await
            .expect("无法连接测试数据库");
        
        // 清理测试数据
        db.execute("TRUNCATE TABLE users").await.ok();
        
        db
    }
    
    #[tokio::test]
    async fn test_insert_user() {
        let db = setup_test_db().await;
        
        let user_id = db.table("users")
            .insert(&json!({"name": "测试用户", "age": 25}))
            .await
            .expect("插入失败");
        
        assert!(user_id > 0);
    }
}
```

### 2. 测试事务回滚
```rust
#[tokio::test]
async fn test_transaction_rollback() {
    let db = setup_test_db().await;
    
    let mut tx = db.transaction().await.unwrap();
    
    tx.table("users")
        .insert(&json!({"name": "临时用户", "age": 30}))
        .await
        .unwrap();
    
    tx.rollback().await.unwrap();
    
    // 验证数据未插入
    let count: i64 = db.table("users").count().await.unwrap();
    assert_eq!(count, 0);
}
```

## 故障排查

### 问题 1：连接超时
```rust
// 增加连接超时时间
let config = DatabaseConfig {
    connect_timeout: 60,  // 60 秒
    ..Default::default()
};
```

### 问题 2：连接池耗尽
```rust
// 增加最大连接数
let config = DatabaseConfig {
    max_connections: 50,
    ..Default::default()
};
```

### 问题 3：查询超时
```rust
// 使用 tokio::time::timeout
use tokio::time::{timeout, Duration};

let result = timeout(
    Duration::from_secs(30),
    db.table("users").select::<User>()
).await;

match result {
    Ok(Ok(users)) => println!("查询成功"),
    Ok(Err(e)) => eprintln!("查询错误: {}", e),
    Err(_) => eprintln!("查询超时"),
}
```

## 最佳实践总结

1. ✅ **始终使用参数化查询** - 防止 SQL 注入
2. ✅ **UPDATE/DELETE 必须带 WHERE** - 防止误操作
3. ✅ **使用事务保证数据一致性** - 多个相关操作应在事务中执行
4. ✅ **启用日志记录** - 便于调试和监控
5. ✅ **处理所有错误** - 使用 match 或 ? 操作符
6. ✅ **使用批量操作** - 提高性能
7. ✅ **只查询需要的字段** - 减少数据传输
8. ✅ **为常用查询字段添加索引** - 提高查询性能
9. ✅ **使用连接池** - 复用数据库连接
10. ✅ **编写测试** - 确保代码质量

## 相关资源

- **项目位置**: `crates/yang-db/`
- **示例代码**: `crates/yang-db/examples/`
- **测试代码**: `crates/yang-db/tests/`
- **文档**: `crates/yang-db/README.md`

## 依赖库

- [sqlx](https://github.com/launchbadge/sqlx) - 异步 SQL 工具包
- [tokio](https://tokio.rs/) - 异步运行时
- [serde](https://serde.rs/) - 序列化/反序列化
- [chrono](https://github.com/chronotope/chrono) - 日期时间处理

---

**类型：** Knowledge Base Power（知识库型）  
**无需 MCP 配置** - 这是 Rust 库使用指南
