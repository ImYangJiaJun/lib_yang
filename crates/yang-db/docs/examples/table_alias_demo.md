# 表别名支持演示

本文档演示 MySQL 查询构建器中表别名的使用方法和生成的 SQL 语句。

## 功能概述

表别名允许为表指定简短的替代名称，在以下场景中特别有用：
- 多表连接查询
- 自连接查询
- 简化长表名的引用
- 避免字段名冲突

## 使用方法

### 1. 基本表别名

```rust
use yang_db::Database;

// 使用 AS 关键字指定表别名
let builder = db.table("users AS u")
    .field("u.id")
    .field("u.name")
    .field("u.email");

// 生成的 SQL:
// SELECT u.id, u.name, u.email FROM users AS u
```

### 2. JOIN 查询中的表别名

```rust
// 主表和连接表都使用别名
let builder = db.table("orders AS o")
    .field("o.id")
    .field("o.order_no")
    .field("u.name")
    .join("users AS u", "o.user_id = u.id");

// 生成的 SQL:
// SELECT o.id, o.order_no, u.name 
// FROM orders AS o 
// INNER JOIN users AS u ON o.user_id = u.id
```

### 3. 多表连接

```rust
// 多个表都使用别名
let builder = db.table("orders AS o")
    .field("o.order_no")
    .field("u.name")
    .field("p.product_name")
    .join("users AS u", "o.user_id = u.id")
    .join("products AS p", "o.product_id = p.id")
    .where_and("o.status", "=", "active");

// 生成的 SQL:
// SELECT o.order_no, u.name, p.product_name 
// FROM orders AS o 
// INNER JOIN users AS u ON o.user_id = u.id 
// INNER JOIN products AS p ON o.product_id = p.id 
// WHERE o.status = ?
```

### 4. 自连接查询

```rust
// 同一个表使用不同的别名进行自连接
let builder = db.table("employees AS e1")
    .field("e1.name AS employee_name")
    .field("e2.name AS manager_name")
    .left_join("employees AS e2", "e1.manager_id = e2.id");

// 生成的 SQL:
// SELECT e1.name AS employee_name, e2.name AS manager_name 
// FROM employees AS e1 
// LEFT JOIN employees AS e2 ON e1.manager_id = e2.id
```

### 5. 带别名的 WHERE 条件

```rust
// 在 WHERE 条件中使用表别名
let builder = db.table("orders AS o")
    .field("o.*")
    .join("users AS u", "o.user_id = u.id")
    .where_and("u.status", "=", "active")
    .where_and("o.created_at", ">", "2024-01-01");

// 生成的 SQL:
// SELECT o.* 
// FROM orders AS o 
// INNER JOIN users AS u ON o.user_id = u.id 
// WHERE u.status = ? AND o.created_at > ?
```

### 6. 带别名的 ORDER BY 和 GROUP BY

```rust
// 在排序和分组中使用表别名
let builder = db.table("orders AS o")
    .field("u.name")
    .field("COUNT(o.id) AS order_count")
    .join("users AS u", "o.user_id = u.id")
    .group("u.name")
    .order("order_count", false);

// 生成的 SQL:
// SELECT u.name, COUNT(o.id) AS order_count 
// FROM orders AS o 
// INNER JOIN users AS u ON o.user_id = u.id 
// GROUP BY u.name 
// ORDER BY order_count DESC
```

## 属性测试验证

属性 33 的基于属性的测试验证了以下特性：

1. **表别名正确传递**：带别名的表名（如 "table AS t"）能正确保存到查询构建器中
2. **SQL 生成正确**：生成的 SQL 包含完整的表别名语法
3. **JOIN 中的别名**：JOIN 子句中的表别名正确生成
4. **字段引用别名**：字段选择中使用的别名（如 "t.id"）正确出现在 SQL 中
5. **ON 条件使用别名**：JOIN 的 ON 条件中使用的别名正确生成

测试运行 100 次迭代，使用随机生成的表名和别名，确保功能在各种输入下都能正常工作。

## 最佳实践

1. **使用简短有意义的别名**：如 `users AS u`、`orders AS o`
2. **保持别名一致性**：在整个查询中使用相同的别名
3. **避免别名冲突**：确保不同表使用不同的别名
4. **明确字段来源**：在多表查询中，使用 `alias.field` 格式明确字段来源
5. **自连接必须使用别名**：同一个表的自连接必须使用不同的别名

## 注意事项

- 表别名在 SQL 标准中使用 `AS` 关键字，但某些数据库也支持省略 `AS`
- 别名区分大小写（取决于数据库配置）
- 别名只在当前查询中有效，不影响表的实际名称
- 使用别名后，必须在所有引用该表的地方使用别名而不是原表名
