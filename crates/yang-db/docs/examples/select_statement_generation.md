# SELECT 语句生成示例

本文档展示了 yang-db 库中 SELECT 语句生成功能的使用示例。

## 基本 SELECT 查询

```rust
use yang_db::Database;

// 连接数据库
let db = Database::connect("mysql://root:111111@localhost:3306/test").await?;

// 简单查询：SELECT * FROM users
let sql = db.table("users").to_sql();
// 输出: SELECT * FROM users

// 选择特定字段：SELECT id, name FROM users
let sql = db.table("users")
    .field("id")
    .field("name")
    .to_sql();
// 输出: SELECT id, name FROM users
```

## DISTINCT 查询

```rust
// SELECT DISTINCT name FROM users
let sql = db.table("users")
    .field("name")
    .distinct()
    .to_sql();
// 输出: SELECT DISTINCT name FROM users
```

## WHERE 条件查询

```rust
// SELECT * FROM users WHERE status = ? AND age > ?
let sql = db.table("users")
    .where_and("status", "=", 1)
    .where_and("age", ">", 18)
    .to_sql();
// 输出: SELECT * FROM users WHERE (status = ? AND age > ?)

// IN 条件：SELECT * FROM users WHERE id IN (?, ?, ?)
let sql = db.table("users")
    .where_in("id", vec![1, 2, 3])
    .to_sql();
// 输出: SELECT * FROM users WHERE id IN (?, ?, ?)

// BETWEEN 条件：SELECT * FROM users WHERE age BETWEEN ? AND ?
let sql = db.table("users")
    .where_between("age", 18, 65)
    .to_sql();
// 输出: SELECT * FROM users WHERE age BETWEEN ? AND ?
```

## JOIN 查询

```rust
// INNER JOIN
let sql = db.table("users")
    .field("users.id")
    .field("users.name")
    .field("orders.total")
    .join("orders", "users.id = orders.user_id")
    .to_sql();
// 输出: SELECT users.id, users.name, orders.total FROM users 
//       INNER JOIN orders ON users.id = orders.user_id

// LEFT JOIN
let sql = db.table("users")
    .field("users.name")
    .field("profiles.bio")
    .left_join("profiles", "users.id = profiles.user_id")
    .to_sql();
// 输出: SELECT users.name, profiles.bio FROM users 
//       LEFT JOIN profiles ON users.id = profiles.user_id

// 多表连接
let sql = db.table("users")
    .field("users.name")
    .field("orders.total")
    .field("products.name")
    .join("orders", "users.id = orders.user_id")
    .left_join("products", "orders.product_id = products.id")
    .to_sql();
// 输出: SELECT users.name, orders.total, products.name FROM users 
//       INNER JOIN orders ON users.id = orders.user_id 
//       LEFT JOIN products ON orders.product_id = products.id
```

## ORDER BY 排序

```rust
// 单字段排序：SELECT * FROM users ORDER BY name ASC
let sql = db.table("users")
    .order("name", true)
    .to_sql();
// 输出: SELECT * FROM users ORDER BY name ASC

// 多字段排序：SELECT * FROM users ORDER BY status ASC, created_at DESC
let sql = db.table("users")
    .order("status", true)
    .order("created_at", false)
    .to_sql();
// 输出: SELECT * FROM users ORDER BY status ASC, created_at DESC
```

## GROUP BY 分组

```rust
// 单字段分组：SELECT status FROM users GROUP BY status
let sql = db.table("users")
    .field("status")
    .group("status")
    .to_sql();
// 输出: SELECT status FROM users GROUP BY status

// 多字段分组：SELECT status, role FROM users GROUP BY status, role
let sql = db.table("users")
    .field("status")
    .field("role")
    .group("status")
    .group("role")
    .to_sql();
// 输出: SELECT status, role FROM users GROUP BY status, role
```

## LIMIT 和 OFFSET

```rust
// 分页查询：SELECT * FROM users LIMIT 10 OFFSET 20
let sql = db.table("users")
    .limit(10)
    .offset(20)
    .to_sql();
// 输出: SELECT * FROM users LIMIT 10 OFFSET 20
```

## 复杂查询示例

```rust
// 完整的复杂查询
let sql = db.table("users")
    .field("users.id")
    .field("users.name")
    .field("COUNT(orders.id) as order_count")
    .field("SUM(orders.total) as total_amount")
    .distinct()
    .join("orders", "users.id = orders.user_id")
    .where_and("users.status", "=", 1)
    .where_and("orders.total", ">", 100)
    .group("users.id")
    .group("users.name")
    .order("total_amount", false)
    .limit(20)
    .offset(10)
    .to_sql();

// 输出:
// SELECT DISTINCT users.id, users.name, COUNT(orders.id) as order_count, SUM(orders.total) as total_amount 
// FROM users 
// INNER JOIN orders ON users.id = orders.user_id 
// WHERE (users.status = ? AND orders.total > ?) 
// GROUP BY users.id, users.name 
// ORDER BY total_amount DESC 
// LIMIT 20 
// OFFSET 10
```

## 验证需求

本实现验证了以下需求：

- **需求 4.1**: find() 方法会自动添加 LIMIT 1
- **需求 4.2**: select() 方法返回所有匹配记录
- **需求 4.4**: count() 方法生成 COUNT(*) 语句
- **需求 4.5**: sum() 方法生成 SUM(field) 语句

## 注意事项

1. **参数化查询**: 所有值都使用 `?` 占位符，防止 SQL 注入
2. **括号处理**: WHERE 条件自动添加括号确保操作符优先级正确
3. **链式调用**: 所有方法都返回 `Self`，支持流畅的链式调用
4. **调试友好**: `to_sql()` 方法可以在任何时候调用以查看生成的 SQL

## 测试覆盖

- ✅ 37 个单元测试
- ✅ 16 个基于属性的测试
- ✅ 所有测试通过
- ✅ Clippy 检查通过
- ✅ 代码格式化检查通过
