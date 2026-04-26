# DELETE 语句生成演示

本文档演示 yang-db 库中 DELETE 语句的生成和使用。

## 功能概述

DELETE 操作用于从数据库表中删除匹配条件的记录。为了防止误操作，DELETE 操作必须提供 WHERE 条件，否则会返回 `MissingWhereClause` 错误。

## 基本用法

### 1. 删除单条记录

```rust
use yang_db::Database;

#[tokio::main]
async fn main() -> Result<(), yang_db::DbError> {
    let db = Database::connect("mysql://root:password@localhost/test").await?;
    
    // 删除 ID 为 1 的用户
    let affected_rows = db.table("users")
        .where_and("id", "=", 1)
        .delete()
        .await?;
    
    println!("删除成功，影响 {} 行", affected_rows);
    
    Ok(())
}
```

生成的 SQL：
```sql
DELETE FROM users WHERE id = ?
```

### 2. 删除多条记录

```rust
// 删除所有状态为 0 且最后登录时间早于 2020 年的用户
let affected_rows = db.table("users")
    .where_and("status", "=", 0)
    .where_and("last_login", "<", "2020-01-01")
    .delete()
    .await?;

println!("批量删除成功，影响 {} 行", affected_rows);
```

生成的 SQL：
```sql
DELETE FROM users WHERE (status = ? AND last_login < ?)
```

### 3. 使用 IN 条件删除

```rust
// 删除指定 ID 列表的用户
let affected_rows = db.table("users")
    .where_in("id", vec![1, 2, 3, 4, 5])
    .delete()
    .await?;

println!("批量删除成功，影响 {} 行", affected_rows);
```

生成的 SQL：
```sql
DELETE FROM users WHERE id IN (?, ?, ?, ?, ?)
```

### 4. 使用 BETWEEN 条件删除

```rust
// 删除年龄在 18 到 25 之间的用户
let affected_rows = db.table("users")
    .where_between("age", 18, 25)
    .delete()
    .await?;

println!("删除成功，影响 {} 行", affected_rows);
```

生成的 SQL：
```sql
DELETE FROM users WHERE age BETWEEN ? AND ?
```

### 5. 使用 OR 条件删除

```rust
// 删除状态为 0 或 -1 的用户
let affected_rows = db.table("users")
    .where_or("status", "=", 0)
    .where_or("status", "=", -1)
    .delete()
    .await?;

println!("删除成功，影响 {} 行", affected_rows);
```

生成的 SQL：
```sql
DELETE FROM users WHERE (status = ? OR status = ?)
```

## 安全特性

### 防止全表删除

为了防止误操作，DELETE 操作必须提供 WHERE 条件。如果没有 WHERE 条件，会返回 `MissingWhereClause` 错误：

```rust
// 尝试不带 WHERE 条件的删除（会返回错误）
let result = db.table("users")
    .delete()
    .await;

match result {
    Ok(_) => println!("删除成功"),
    Err(yang_db::DbError::MissingWhereClause) => {
        println!("错误：禁止全表删除，必须提供 WHERE 条件");
    }
    Err(e) => println!("其他错误: {}", e),
}
```

### 外键约束处理

如果删除操作违反外键约束，会返回 `ConstraintError`：

```rust
// 尝试删除有关联数据的记录
let result = db.table("users")
    .where_and("id", "=", 1)
    .delete()
    .await;

match result {
    Ok(rows) => println!("删除成功，影响 {} 行", rows),
    Err(yang_db::DbError::ConstraintError(msg)) => {
        println!("约束错误: {}", msg);
        // 可能需要先删除关联数据
    }
    Err(e) => println!("其他错误: {}", e),
}
```

## 复杂条件示例

### 组合多个条件

```rust
// 删除满足多个条件的记录
let affected_rows = db.table("orders")
    .where_and("status", "=", "cancelled")
    .where_and("created_at", "<", "2023-01-01")
    .where_and("total", "<", 10.0)
    .delete()
    .await?;

println!("删除了 {} 个已取消的小额订单", affected_rows);
```

### 使用 LIKE 条件

```rust
// 删除邮箱包含特定域名的用户
let affected_rows = db.table("users")
    .where_and("email", "like", "%@spam.com")
    .delete()
    .await?;

println!("删除了 {} 个垃圾邮箱用户", affected_rows);
```

## 日志记录

启用日志后，DELETE 操作会记录详细信息：

```rust
// 启用日志
env_logger::init();

let db = Database::connect_with_config(
    "mysql://root:password@localhost/test",
    DatabaseConfig {
        enable_logging: true,
        ..Default::default()
    }
).await?;

// 执行删除操作
let affected_rows = db.table("users")
    .where_and("id", "=", 1)
    .delete()
    .await?;
```

日志输出示例：
```
[DEBUG] 执行 delete() 操作，表: users
[DEBUG] 执行 delete() SQL: DELETE FROM users WHERE id = ?
[DEBUG] 参数: [Int(1)]
[DEBUG] delete() 成功，影响 1 行
```

## 性能建议

1. **使用索引字段作为 WHERE 条件**：确保 WHERE 条件中的字段有索引，提高删除性能
2. **批量删除时使用 IN 条件**：相比多次单条删除，使用 IN 条件一次性删除多条记录性能更好
3. **大批量删除时分批处理**：避免一次性删除大量数据导致锁表时间过长
4. **在事务中执行删除**：如果需要删除多个表的关联数据，使用事务确保数据一致性

## 错误处理

DELETE 操作可能返回以下错误：

- `MissingWhereClause`：缺少 WHERE 条件
- `QueryError`：查询执行失败
- `ConstraintError`：违反外键约束
- `ConnectionError`：数据库连接失败

建议使用 match 语句处理不同类型的错误：

```rust
match db.table("users").where_and("id", "=", 1).delete().await {
    Ok(rows) => println!("删除成功，影响 {} 行", rows),
    Err(yang_db::DbError::MissingWhereClause) => {
        eprintln!("错误：必须提供 WHERE 条件");
    }
    Err(yang_db::DbError::ConstraintError(msg)) => {
        eprintln!("约束错误: {}", msg);
    }
    Err(yang_db::DbError::QueryError(msg)) => {
        eprintln!("查询错误: {}", msg);
    }
    Err(e) => {
        eprintln!("未知错误: {}", e);
    }
}
```

## 总结

DELETE 操作的关键特性：

1. ✅ 必须提供 WHERE 条件，防止误删除
2. ✅ 支持所有条件操作符（=, !=, >, <, >=, <=, IN, BETWEEN, LIKE）
3. ✅ 支持 AND/OR 组合条件
4. ✅ 参数化查询，防止 SQL 注入
5. ✅ 返回受影响的行数
6. ✅ 完善的错误处理和日志记录
7. ✅ 支持在事务中执行

## 相关文档

- [WHERE 子句验证](./where_clause_verification.md)
- [UPDATE 操作演示](./update_demo.md)
- [事务管理](../README.md#事务管理)
