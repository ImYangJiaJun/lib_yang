# GlobalDatabase 实现说明

## 概述

GlobalDatabase 是对 yang-db::Database 的封装，提供线程安全的全局数据库访问接口。

## 实现特性

### 1. 全局单例模式

使用 `std::sync::OnceLock` 实现线程安全的全局单例：

```rust
static GLOBAL_DB: OnceLock<Database> = OnceLock::new();
```

### 2. 核心方法

#### init - 初始化全局数据库

```rust
pub async fn init(url: &str, config: DatabaseConfig) -> Result<(), BaseError>
```

- 使用 `yang-db::Database::connect_with_config` 创建数据库连接
- 只能调用一次，重复调用返回 `DatabaseAlreadyInitialized` 错误
- 连接失败返回 `DatabaseConnectionFailed` 错误

#### get - 获取数据库实例

```rust
pub fn get() -> Result<&'static Database, BaseError>
```

- 返回全局数据库实例的静态引用
- 未初始化时返回 `DatabaseNotInitialized` 错误

#### table - 创建查询构建器

```rust
pub fn table(table_name: &str) -> Result<QueryBuilder<'static>, BaseError>
```

- 调用 `yang-db::Database::table` 方法
- 返回 yang-db 的 QueryBuilder，支持链式调用

#### query - 执行 SELECT 查询

```rust
pub async fn query<T>(sql: &str) -> Result<Vec<T>, BaseError>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin
```

- 调用 `yang-db::Database::query` 方法
- 适用于原生 SQL 查询

#### execute - 执行 INSERT/UPDATE/DELETE

```rust
pub async fn execute(sql: &str) -> Result<u64, BaseError>
```

- 调用 `yang-db::Database::execute` 方法
- 返回受影响的行数

#### transaction - 开始事务

```rust
pub async fn transaction() -> Result<Transaction, BaseError>
```

- 调用 `yang-db::Database::transaction` 方法
- 返回 yang-db 的 Transaction 对象

## 使用示例

### 基本使用

```rust
use yang_base::database::GlobalDatabase;
use yang_db::DatabaseConfig;

// 初始化
GlobalDatabase::init("mysql://user:pass@localhost/db", DatabaseConfig::default()).await?;

// 查询
let users = GlobalDatabase::table("users")?
    .field("id")
    .field("name")
    .select::<User>()
    .await?;

// 原生查询
let result: Vec<User> = GlobalDatabase::query("SELECT * FROM users").await?;

// 执行语句
let affected = GlobalDatabase::execute("DELETE FROM users WHERE id = 1").await?;
```

### 事务使用

```rust
// 开始事务
let mut tx = GlobalDatabase::transaction().await?;

// 在事务中执行操作
tx.execute("INSERT INTO users (name) VALUES ('Alice')").await?;
tx.execute("INSERT INTO logs (action) VALUES ('user_created')").await?;

// 提交事务
tx.commit().await?;
```

## 错误处理

所有方法都返回 `Result<T, BaseError>`，主要错误类型：

- `DatabaseNotInitialized`: 数据库未初始化
- `DatabaseAlreadyInitialized`: 数据库已初始化（重复初始化）
- `DatabaseConnectionFailed`: 数据库连接失败
- `DatabaseQueryFailed`: 查询执行失败
- `DatabaseExecuteFailed`: 语句执行失败
- `DatabaseTransactionFailed`: 事务操作失败

## 测试

### 单元测试

位于 `src/database/global.rs` 的 `tests` 模块：

- `test_database_not_initialized`: 测试未初始化时的错误处理
- `test_table_not_initialized`: 测试未初始化时调用 table 方法

### 集成测试

位于 `tests/database_test.rs`：

- `test_global_database_init`: 测试数据库初始化（需要真实数据库）

### 示例代码

位于 `examples/database_example.rs`：

- 完整的使用示例，包括初始化、查询、事务等操作

## 设计决策

### 为什么使用 OnceLock？

1. **线程安全**: OnceLock 提供线程安全的单次初始化
2. **零成本抽象**: 初始化后访问没有额外开销
3. **标准库支持**: 不需要额外依赖

### 为什么封装 yang-db？

1. **统一接口**: 提供全局访问点，简化使用
2. **错误转换**: 将 yang-db 错误转换为 BaseError
3. **日志记录**: 在关键操作点添加日志
4. **扩展性**: 未来可以添加连接池监控、性能统计等功能

## 相关需求

此实现满足以下需求：

- 需求 6.1: 提供全局静态访问接口
- 需求 6.2: 存储数据库连接实例
- 需求 6.3: 封装 yang-db 的 Database 类型
- 需求 6.4: 返回可用的数据库连接
- 需求 6.5: 未初始化时返回错误
- 需求 13.1: 提供 table 方法返回 QueryBuilder
- 需求 13.2: 提供 query 方法执行原生查询
- 需求 13.3: 提供 execute 方法执行原生语句
- 需求 13.4: 提供 transaction 方法开始事务
- 需求 13.5: 封装所有 yang-db 的 Database 方法
- 需求 15.2: 使用线程安全的方式存储数据库连接
- 需求 15.3: 正确处理并发请求

## 后续任务

下一步需要实现：

- DatabaseInitializer 结构体（任务 4.2）
- 数据库管理集成测试（任务 4.3）
