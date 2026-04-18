# UPDATE 操作示例

本文档展示了 yang-db 库中 `update()` 方法的使用示例。

## 基本用法

### 1. 更新单条记录

```rust
use yang_db::Database;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), yang_db::DbError> {
    // 连接数据库
    let db = Database::connect("mysql://root:password@localhost/test").await?;
    
    // 更新单个用户
    let update_data = json!({
        "name": "张三（已更新）",
        "email": "zhangsan_new@example.com",
        "age": 26
    });
    
    let affected_rows = db.table("users")
        .where_and("id", "=", 1)
        .update(&update_data)
        .await?;
    
    println!("更新成功，影响 {} 行", affected_rows);
    
    Ok(())
}
```

### 2. 批量更新多条记录

```rust
use yang_db::Database;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), yang_db::DbError> {
    let db = Database::connect("mysql://root:password@localhost/test").await?;
    
    // 批量更新状态
    let batch_update = json!({
        "status": 1,
        "updated_at": "2024-01-01 12:00:00"
    });
    
    let affected_rows = db.table("users")
        .where_and("age", ">", 18)
        .where_and("status", "=", 0)
        .update(&batch_update)
        .await?;
    
    println!("批量更新成功，影响 {} 行", affected_rows);
    
    Ok(())
}
```

### 3. 更新带 JSON 字段的数据

```rust
use yang_db::Database;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), yang_db::DbError> {
    let db = Database::connect("mysql://root:password@localhost/test").await?;
    
    // 更新订单，包含 JSON 字段
    let order_update = json!({
        "status": "completed",
        "metadata": {
            "completed_at": "2024-01-01 12:00:00",
            "completed_by": "admin"
        }
    });
    
    let affected_rows = db.table("orders")
        .json("metadata")  // 标记 metadata 字段为 JSON 类型
        .where_and("id", "=", 100)
        .update(&order_update)
        .await?;
    
    println!("订单更新成功，影响 {} 行", affected_rows);
    
    Ok(())
}
```

### 4. 使用多个条件更新

```rust
use yang_db::Database;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), yang_db::DbError> {
    let db = Database::connect("mysql://root:password@localhost/test").await?;
    
    // 使用多个条件
    let update_data = json!({
        "discount": 0.1
    });
    
    let affected_rows = db.table("products")
        .where_and("category", "=", "electronics")
        .where_and("price", ">", 1000)
        .where_and("stock", ">", 0)
        .update(&update_data)
        .await?;
    
    println!("更新了 {} 个产品的折扣", affected_rows);
    
    Ok(())
}
```

## 安全特性

### 防止全表更新

为了防止误操作，`update()` 方法要求必须提供 WHERE 条件。如果没有 WHERE 条件，会返回 `MissingWhereClause` 错误：

```rust
use yang_db::Database;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), yang_db::DbError> {
    let db = Database::connect("mysql://root:password@localhost/test").await?;
    
    // 尝试不带 WHERE 条件的更新（会返回错误）
    let result = db.table("users")
        .update(&json!({"status": 1}))
        .await;
    
    match result {
        Ok(_) => println!("更新成功"),
        Err(yang_db::DbError::MissingWhereClause) => {
            println!("错误：禁止全表更新，必须提供 WHERE 条件");
        }
        Err(e) => println!("其他错误: {}", e),
    }
    
    Ok(())
}
```

## 错误处理

### 处理约束错误

```rust
use yang_db::Database;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), yang_db::DbError> {
    let db = Database::connect("mysql://root:password@localhost/test").await?;
    
    let update_data = json!({
        "email": "duplicate@example.com"  // 假设这个邮箱已存在
    });
    
    match db.table("users")
        .where_and("id", "=", 1)
        .update(&update_data)
        .await
    {
        Ok(rows) => println!("更新成功，影响 {} 行", rows),
        Err(yang_db::DbError::ConstraintError(msg)) => {
            println!("约束错误: {}", msg);
            // 处理唯一约束冲突等情况
        }
        Err(e) => println!("更新失败: {}", e),
    }
    
    Ok(())
}
```

### 处理序列化错误

```rust
use yang_db::Database;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct UpdateData {
    name: String,
    age: i32,
}

#[tokio::main]
async fn main() -> Result<(), yang_db::DbError> {
    let db = Database::connect("mysql://root:password@localhost/test").await?;
    
    let update_data = UpdateData {
        name: "李四".to_string(),
        age: 30,
    };
    
    match db.table("users")
        .where_and("id", "=", 2)
        .update(&update_data)
        .await
    {
        Ok(rows) => println!("更新成功，影响 {} 行", rows),
        Err(yang_db::DbError::SerializationError(msg)) => {
            println!("序列化错误: {}", msg);
        }
        Err(e) => println!("更新失败: {}", e),
    }
    
    Ok(())
}
```

## 特殊字段类型

### 更新 DATETIME 字段

```rust
use yang_db::Database;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), yang_db::DbError> {
    let db = Database::connect("mysql://root:password@localhost/test").await?;
    
    let update_data = json!({
        "name": "王五",
        "last_login": "2024-01-01 12:00:00"
    });
    
    let affected_rows = db.table("users")
        .datetime("last_login")  // 标记为 DATETIME 类型
        .where_and("id", "=", 3)
        .update(&update_data)
        .await?;
    
    println!("更新成功，影响 {} 行", affected_rows);
    
    Ok(())
}
```

### 更新 DECIMAL 字段

```rust
use yang_db::Database;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), yang_db::DbError> {
    let db = Database::connect("mysql://root:password@localhost/test").await?;
    
    let update_data = json!({
        "price": 199.99,
        "discount_rate": 0.15
    });
    
    let affected_rows = db.table("products")
        .decimal("price")
        .decimal("discount_rate")
        .where_and("id", "=", 100)
        .update(&update_data)
        .await?;
    
    println!("更新成功，影响 {} 行", affected_rows);
    
    Ok(())
}
```

## 日志记录

启用日志记录可以帮助调试 UPDATE 操作：

```rust
use yang_db::{Database, DatabaseConfig};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), yang_db::DbError> {
    // 初始化日志
    env_logger::init();
    
    // 启用日志记录
    let config = DatabaseConfig {
        enable_logging: true,
        ..Default::default()
    };
    
    let db = Database::connect_with_config(
        "mysql://root:password@localhost/test",
        config
    ).await?;
    
    let update_data = json!({
        "status": 1
    });
    
    // 这将记录生成的 SQL 和参数
    let affected_rows = db.table("users")
        .where_and("id", "=", 1)
        .update(&update_data)
        .await?;
    
    println!("更新成功，影响 {} 行", affected_rows);
    
    Ok(())
}
```

日志输出示例：
```
[DEBUG] 执行 update() 操作，表: users
[DEBUG] 执行 update() SQL: UPDATE users SET status = ? WHERE id = ?
[DEBUG] 参数: [Int(1), Int(1)]
[DEBUG] update() 成功，影响 1 行
```

## 注意事项

1. **必须提供 WHERE 条件**：为了防止误操作，`update()` 方法要求必须提供至少一个 WHERE 条件
2. **返回受影响行数**：`update()` 方法返回受影响的行数，如果没有记录匹配条件，返回 0
3. **数据序列化**：更新数据必须能够序列化为 JSON 对象
4. **特殊字段类型**：对于 JSON、DATETIME、DECIMAL 等特殊字段，需要使用相应的类型标记方法
5. **错误处理**：建议使用 match 语句处理不同类型的错误
6. **事务支持**：可以在事务中使用 `update()` 方法，确保数据一致性

## 性能建议

1. **使用索引**：确保 WHERE 条件中的字段有索引，提高更新性能
2. **批量更新**：尽可能使用单个 UPDATE 语句更新多条记录，而不是循环调用
3. **避免更新不必要的字段**：只更新需要修改的字段，减少数据传输和处理
4. **使用事务**：对于多个相关的更新操作，使用事务确保原子性
