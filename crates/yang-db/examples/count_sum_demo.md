# count() 和 sum() 方法使用示例

本文档展示了 yang-db 查询构建器中 `count()` 和 `sum()` 方法的使用示例。

## count() 方法

`count()` 方法用于统计匹配条件的记录数量，生成 `COUNT(*)` SQL 语句。

### 基本用法

```rust
use yang_db::Database;

#[tokio::main]
async fn main() -> Result<(), yang_db::DbError> {
    let db = Database::connect("mysql://root:password@localhost/test").await?;

    // 统计所有用户数量
    let total_users = db.table("users")
        .count()
        .await?;
    println!("总用户数: {}", total_users);

    Ok(())
}
```

### 带条件的统计

```rust
// 统计活跃用户数量
let active_users = db.table("users")
    .where_and("status", "=", 1)
    .count()
    .await?;
println!("活跃用户数: {}", active_users);

// 统计符合多个条件的记录
let filtered_count = db.table("users")
    .where_and("status", "=", 1)
    .where_and("age", ">", 25)
    .count()
    .await?;
println!("符合条件的用户数: {}", filtered_count);
```

### 使用 IN 条件统计

```rust
// 统计状态为 0 或 1 的用户数量
let count = db.table("users")
    .where_in("status", vec![0, 1])
    .count()
    .await?;
println!("状态为 0 或 1 的用户数: {}", count);
```

### 使用 BETWEEN 条件统计

```rust
// 统计年龄在 25-35 之间的用户数量
let count = db.table("users")
    .where_between("age", 25, 35)
    .count()
    .await?;
println!("年龄在 25-35 之间的用户数: {}", count);
```

## sum() 方法

`sum()` 方法用于计算指定字段的总和，生成 `SUM(field)` SQL 语句。

### 基本用法

```rust
use yang_db::Database;

#[tokio::main]
async fn main() -> Result<(), yang_db::DbError> {
    let db = Database::connect("mysql://root:password@localhost/test").await?;

    // 计算所有订单的总金额
    let total_amount = db.table("orders")
        .sum("amount")
        .await?;

    match total_amount {
        Some(sum) => println!("订单总金额: {:.2}", sum),
        None => println!("没有订单或金额全为 NULL"),
    }

    Ok(())
}
```

### 带条件的求和

```rust
// 计算已完成订单的总金额
let completed_amount = db.table("orders")
    .where_and("status", "=", "completed")
    .sum("amount")
    .await?;

println!("已完成订单总金额: {:.2}", completed_amount.unwrap_or(0.0));
```

### 计算整数字段的总和

```rust
// 计算订单数量总和
let total_quantity = db.table("orders")
    .sum("quantity")
    .await?;

match total_quantity {
    Some(sum) => println!("订单数量总和: {:.0}", sum),
    None => println!("没有订单"),
}
```

### 使用多个条件求和

```rust
// 计算符合多个条件的订单金额总和
let filtered_sum = db.table("orders")
    .where_and("status", "=", "completed")
    .where_and("amount", ">", 100.0)
    .sum("amount")
    .await?;

println!("大额已完成订单总金额: {:.2}", filtered_sum.unwrap_or(0.0));
```

## 返回值说明

### count() 返回值

- `Ok(i64)`: 查询成功，返回记录数量（至少为 0）
- `Err(DbError)`: 查询执行失败

`count()` 方法总是返回一个数值，即使没有匹配的记录也会返回 0。

### sum() 返回值

- `Ok(Some(f64))`: 查询成功，返回字段总和
- `Ok(None)`: 查询成功，但没有匹配的记录或所有值为 NULL
- `Err(DbError)`: 查询执行失败

`sum()` 方法返回 `Option<f64>`，因为 MySQL 的 `SUM()` 函数在没有匹配记录时返回 NULL。

## 注意事项

1. **类型转换**: `sum()` 方法使用 `CAST(SUM(field) AS DOUBLE)` 将结果转换为浮点数，以统一处理整数和浮点数字段。

2. **NULL 值处理**: 
   - `count()` 不受 NULL 值影响，会统计所有记录（包括字段值为 NULL 的记录）
   - `sum()` 会忽略 NULL 值，只对非 NULL 值求和

3. **性能考虑**: 
   - 对于大表，`count()` 可能较慢，考虑添加适当的索引
   - `sum()` 在有索引的字段上性能更好

4. **精度问题**: 使用 `sum()` 计算金额时，建议在数据库中使用 DECIMAL 类型存储，避免浮点数精度问题。

## 完整示例

```rust
use yang_db::Database;

#[tokio::main]
async fn main() -> Result<(), yang_db::DbError> {
    // 连接数据库
    let db = Database::connect("mysql://root:password@localhost/test").await?;

    // 统计订单数量
    let order_count = db.table("orders")
        .where_and("status", "=", "completed")
        .count()
        .await?;
    println!("已完成订单数: {}", order_count);

    // 计算订单总金额
    let total_amount = db.table("orders")
        .where_and("status", "=", "completed")
        .sum("amount")
        .await?;
    
    if let Some(sum) = total_amount {
        println!("已完成订单总金额: {:.2}", sum);
        
        // 计算平均订单金额
        if order_count > 0 {
            let avg = sum / order_count as f64;
            println!("平均订单金额: {:.2}", avg);
        }
    } else {
        println!("没有已完成的订单");
    }

    Ok(())
}
```

## 测试验证

所有功能都经过了完整的测试验证：

1. **单元测试**: 验证 SQL 生成的正确性
2. **基于属性的测试**: 验证在各种输入下的正确性
3. **集成测试**: 验证与实际数据库交互的正确性

测试覆盖了以下场景：
- 基本的 count 和 sum 操作
- 带各种 WHERE 条件的统计和求和
- 空结果集的处理
- NULL 值的处理
- 整数和浮点数字段的求和
