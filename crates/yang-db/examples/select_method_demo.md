# select() 方法使用示例

本文档展示了 `yang-db` 库中 `select()` 方法的各种使用场景。

## 基本用法

### 查询所有记录

```rust
use yang_db::Database;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
struct User {
    id: i32,
    name: String,
    age: i32,
}

async fn example() -> Result<(), yang_db::DbError> {
    let db = Database::connect("mysql://root:password@localhost/test").await?;
    
    // 查询所有用户
    let users: Vec<User> = db.table("users")
        .select()
        .await?;
    
    println!("找到 {} 个用户", users.len());
    for user in users {
        println!("用户: {:?}", user);
    }
    
    Ok(())
}
```

### 使用 WHERE 条件查询

```rust
// 查询活跃用户
let active_users: Vec<User> = db.table("users")
    .where_and("status", "=", 1)
    .select()
    .await?;

// 查询年龄大于 18 的用户
let adult_users: Vec<User> = db.table("users")
    .where_and("age", ">", 18)
    .select()
    .await?;

// 使用多个条件
let filtered_users: Vec<User> = db.table("users")
    .where_and("status", "=", 1)
    .where_and("age", ">=", 18)
    .where_and("age", "<=", 65)
    .select()
    .await?;
```

## 高级用法

### 排序查询结果

```rust
// 按名称升序排序
let sorted_users: Vec<User> = db.table("users")
    .order("name", true)
    .select()
    .await?;

// 按年龄降序排序
let sorted_by_age: Vec<User> = db.table("users")
    .order("age", false)
    .select()
    .await?;

// 多字段排序
let multi_sorted: Vec<User> = db.table("users")
    .order("status", false)  // 先按状态降序
    .order("age", true)      // 再按年龄升序
    .select()
    .await?;
```

### 限制返回数量

```rust
// 获取前 10 条记录
let top_10: Vec<User> = db.table("users")
    .limit(10)
    .select()
    .await?;

// 分页查询：跳过前 20 条，获取接下来的 10 条
let page_2: Vec<User> = db.table("users")
    .limit(10)
    .offset(20)
    .select()
    .await?;
```

### 选择特定字段

```rust
#[derive(Debug, sqlx::FromRow)]
struct UserBasic {
    name: String,
    age: i32,
}

// 只查询 name 和 age 字段
let basic_info: Vec<UserBasic> = db.table("users")
    .field("name")
    .field("age")
    .select()
    .await?;
```

### 使用 IN 条件

```rust
// 查询特定 ID 的用户
let specific_users: Vec<User> = db.table("users")
    .where_in("id", vec![1, 2, 3, 5, 8])
    .select()
    .await?;

// 查询特定类别的产品
let categories = vec!["电子产品", "家具", "图书"];
let products: Vec<Product> = db.table("products")
    .where_in("category", categories)
    .select()
    .await?;
```

### 使用 BETWEEN 条件

```rust
// 查询年龄在 18-65 之间的用户
let working_age_users: Vec<User> = db.table("users")
    .where_between("age", 18, 65)
    .select()
    .await?;

// 查询价格在 100-1000 之间的产品
let mid_range_products: Vec<Product> = db.table("products")
    .where_between("price", 100.0, 1000.0)
    .select()
    .await?;
```

### 去重查询

```rust
#[derive(Debug, sqlx::FromRow)]
struct City {
    city: String,
}

// 查询所有不重复的城市
let cities: Vec<City> = db.table("users")
    .field("city")
    .distinct()
    .select()
    .await?;
```

## 处理空结果

`select()` 方法在没有匹配记录时返回空的 `Vec`，而不是错误：

```rust
let users: Vec<User> = db.table("users")
    .where_and("name", "=", "不存在的用户")
    .select()
    .await?;

if users.is_empty() {
    println!("没有找到匹配的用户");
} else {
    println!("找到 {} 个用户", users.len());
}
```

## 错误处理

```rust
match db.table("users").select::<User>().await {
    Ok(users) => {
        println!("查询成功，找到 {} 条记录", users.len());
        for user in users {
            println!("  - {:?}", user);
        }
    }
    Err(e) => {
        eprintln!("查询失败: {}", e);
    }
}
```

## select() vs find()

- `select()`: 返回 `Vec<T>`，可能包含多条记录或空列表
- `find()`: 返回 `Option<T>`，最多返回一条记录，自动添加 `LIMIT 1`

```rust
// 使用 find() 查询单条记录
let user: Option<User> = db.table("users")
    .where_and("id", "=", 1)
    .find()
    .await?;

// 使用 select() 查询多条记录
let users: Vec<User> = db.table("users")
    .where_and("status", "=", 1)
    .select()
    .await?;
```

## 性能提示

1. **使用字段选择**：只查询需要的字段可以减少数据传输量
2. **添加索引**：为常用的 WHERE 条件字段添加数据库索引
3. **使用 LIMIT**：对于大数据集，使用 LIMIT 限制返回数量
4. **避免 SELECT ***：明确指定需要的字段
5. **使用连接池**：Database 内部使用连接池，可以复用连接

## 完整示例

```rust
use yang_db::{Database, DbError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
struct User {
    id: i32,
    name: String,
    age: i32,
    email: String,
    status: i32,
}

#[tokio::main]
async fn main() -> Result<(), DbError> {
    // 连接数据库
    let db = Database::connect("mysql://root:password@localhost/test").await?;
    
    // 查询活跃用户，按年龄排序，限制 10 条
    let users: Vec<User> = db.table("users")
        .where_and("status", "=", 1)
        .where_and("age", ">=", 18)
        .order("age", true)
        .limit(10)
        .select()
        .await?;
    
    println!("找到 {} 个活跃成年用户:", users.len());
    for user in users {
        println!("  - {} ({}岁): {}", user.name, user.age, user.email);
    }
    
    Ok(())
}
```

## 注意事项

1. 确保结构体字段与数据库表字段匹配
2. 使用 `#[derive(sqlx::FromRow)]` 自动实现行映射
3. 对于可能为 NULL 的字段，使用 `Option<T>` 类型
4. 查询大量数据时考虑使用分页
5. 始终处理可能的错误情况
