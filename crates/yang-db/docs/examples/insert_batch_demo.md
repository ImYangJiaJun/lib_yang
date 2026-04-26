# insert_batch() 方法使用示例

本文档展示如何使用 `insert_batch()` 方法进行批量数据插入。

## 基本用法

### 批量插入普通数据

```rust
use yang_db::Database;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), yang_db::DbError> {
    // 连接数据库
    let db = Database::connect("mysql://root:password@localhost/test").await?;

    // 准备批量插入的数据
    let users = vec![
        json!({"name": "张三", "email": "zhangsan@example.com", "age": 25}),
        json!({"name": "李四", "email": "lisi@example.com", "age": 30}),
        json!({"name": "王五", "email": "wangwu@example.com", "age": 28}),
    ];

    // 批量插入
    let affected_rows = db.table("users")
        .insert_batch(&users)
        .await?;

    println!("批量插入成功，影响 {} 行", affected_rows);

    Ok(())
}
```

### 批量插入带 JSON 字段的数据

```rust
use yang_db::Database;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), yang_db::DbError> {
    let db = Database::connect("mysql://root:password@localhost/test").await?;

    // 准备带 JSON 字段的数据
    let orders = vec![
        json!({
            "user_id": 1,
            "total": 199.99,
            "items": [{"id": 1, "qty": 2}, {"id": 2, "qty": 1}]
        }),
        json!({
            "user_id": 2,
            "total": 299.99,
            "items": [{"id": 3, "qty": 1}, {"id": 4, "qty": 3}]
        }),
        json!({
            "user_id": 3,
            "total": 399.99,
            "items": [{"id": 5, "qty": 2}]
        }),
    ];

    // 批量插入，标记 items 字段为 JSON 类型
    let affected_rows = db.table("orders")
        .json("items")
        .insert_batch(&orders)
        .await?;

    println!("批量插入订单成功，影响 {} 行", affected_rows);

    Ok(())
}
```

## 性能对比

### 单条插入（低效）

```rust
// 不推荐：多次单条插入
for user in &users {
    db.table("users").insert(user).await?;
}
```

### 批量插入（高效）

```rust
// 推荐：一次批量插入
db.table("users").insert_batch(&users).await?;
```

批量插入的优势：
- 只需要一次数据库往返
- 减少网络开销
- 提高插入性能
- 适合大量数据的插入场景

## 生成的 SQL 语句

对于上面的用户批量插入示例，生成的 SQL 语句如下：

```sql
INSERT INTO users (name, email, age) VALUES 
    (?, ?, ?),
    (?, ?, ?),
    (?, ?, ?)
```

参数列表：
```
["张三", "zhangsan@example.com", 25, "李四", "lisi@example.com", 30, "王五", "wangwu@example.com", 28]
```

## 注意事项

1. **字段一致性**：所有记录必须具有相同的字段结构，字段顺序以第一条记录为准
2. **缺失字段**：如果某条记录缺少字段，将使用 NULL 值
3. **空数据检查**：传入空数组会返回错误
4. **返回值**：返回受影响的行数，而不是最后插入的 ID（与单条插入不同）
5. **事务支持**：批量插入在单个事务中执行，要么全部成功，要么全部失败

## 错误处理

```rust
use yang_db::Database;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), yang_db::DbError> {
    let db = Database::connect("mysql://root:password@localhost/test").await?;

    let users = vec![
        json!({"name": "张三", "email": "zhangsan@example.com"}),
        json!({"name": "李四", "email": "lisi@example.com"}),
    ];

    match db.table("users").insert_batch(&users).await {
        Ok(affected_rows) => {
            println!("批量插入成功，影响 {} 行", affected_rows);
        }
        Err(e) => {
            eprintln!("批量插入失败: {}", e);
            // 处理错误，例如：
            // - 约束违反（重复的唯一键）
            // - 数据类型不匹配
            // - 表不存在
        }
    }

    Ok(())
}
```

## 使用结构体批量插入

```rust
use yang_db::Database;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct User {
    name: String,
    email: String,
    age: i32,
}

#[tokio::main]
async fn main() -> Result<(), yang_db::DbError> {
    let db = Database::connect("mysql://root:password@localhost/test").await?;

    // 使用结构体
    let users = vec![
        User {
            name: "张三".to_string(),
            email: "zhangsan@example.com".to_string(),
            age: 25,
        },
        User {
            name: "李四".to_string(),
            email: "lisi@example.com".to_string(),
            age: 30,
        },
    ];

    let affected_rows = db.table("users")
        .insert_batch(&users)
        .await?;

    println!("批量插入成功，影响 {} 行", affected_rows);

    Ok(())
}
```

## 最佳实践

1. **批量大小**：建议每批插入 100-1000 条记录，避免单次插入过多数据
2. **分批处理**：对于大量数据，可以分批插入

```rust
// 分批插入大量数据
let chunk_size = 500;
for chunk in users.chunks(chunk_size) {
    db.table("users").insert_batch(chunk).await?;
}
```

3. **错误恢复**：批量插入失败时，考虑记录失败的批次，便于重试
4. **性能监控**：使用日志记录批量插入的执行时间和影响行数
