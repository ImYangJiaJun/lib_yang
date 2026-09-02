# yang-db — 数据库基础库文档

版本：0.1.6 | 许可：MIT OR Apache-2.0

## 概述

`yang-db` 是底层数据库抽象库，提供三个可独立选择的后端：

| 子系统 | 入口类型 | 说明 |
|--------|----------|------|
| **MySQL** | `Database` → `QueryBuilder` | 链式查询构建器，封装 sqlx |
| **PostgreSQL 16** | `postgres::Database` → `QueryBuilder` | 与 MySQL 对称的参数化查询与事务 API |
| **Redis** | `RedisClient` | 连接池客户端，封装 deadpool-redis |

三个后端通过统一的 `DbError` 错误类型汇聚错误，并通过 `BackendCapabilities` 暴露可机读能力、占位符风格与安全约束。

```toml
[dependencies]
yang-db = { path = "../yang-db" }
```

---

## 依赖关系

```
yang-db
├── sqlx (MySQL 驱动，带 runtime-tokio-rustls)
├── deadpool-redis (Redis 连接池)
├── redis (Redis 命令)
├── tokio (异步运行时)
├── serde / serde_json (序列化)
├── thiserror (错误派生)
├── chrono (时间类型)
└── base64 (BLOB 编解码)
```

### 0.1.6 查询能力摘要

- MySQL 8 与 PostgreSQL 16 均提供受控 `Subquery`（EXISTS/NOT EXISTS/IN）、`UNION`/`UNION ALL` 和显式 `RowLock`。
- `increment`/`decrement` 使用绑定的 `i64` 数值完成原子字段更新，并复用写操作的 WHERE 防护。
- `BackendCapabilities` 是后端能力的公开机读契约；应用不得仅凭方法名假定不同后端完全等价。
- checked identifier 与绑定参数是外部输入路径的默认边界；原生 SQL API 是显式逃生舱。

---

## 错误类型 (`DbError`)

```rust
pub enum DbError {
    // MySQL
    ConnectionError(String),       // 连接错误
    QueryError(String),            // 查询错误
    SqlSyntaxError(String),        // SQL 语法错误
    ConstraintError(String),       // 约束违反（唯一键、外键等）
    TypeConversionError(String),   // 类型转换失败
    SerializationError(String),    // 序列化失败
    DeserializationError(String),  // 反序列化失败
    TransactionError(String),      // 事务错误
    TableNotFound(String),         // 表不存在
    MissingWhereClause,            // 缺少 WHERE 条件（禁止全表更新/删除）
    MissingGroupByClause,          // HAVING 子句缺少 GROUP BY
    UnsupportedOperator(String),   // 不支持的操作符

    // Redis
    RedisConnectionError(String),      // Redis 连接失败
    RedisCommandError(String),         // Redis 命令执行失败
    RedisPoolError(String),            // Redis 连接池错误
    RedisTypeConversionError(String),  // Redis 类型转换失败
    RedisTimeoutError(String),         // Redis 操作超时

    Unknown(String),               // 未知错误
}

pub type Result<T> = std::result::Result<T, DbError>;
```

**自动 From 转换**：
- `sqlx::Error` → `DbError`（按错误码分类：23000→ConstraintError，42S02→TableNotFound，42000→SqlSyntaxError）
- `redis::RedisError` → `DbError`（按错误类型和消息内容分类）
- `deadpool_redis::PoolError` → `DbError`（按 Timeout/Closed/Backend 分类）

---

## MySQL 子系统

### DatabaseConfig

```rust
pub struct DatabaseConfig {
    pub max_connections: u32,  // 最大连接数，默认 10
    pub connect_timeout: u64,  // 连接超时（秒），默认 30
    pub idle_timeout: u64,     // 空闲超时（秒），默认 600
    pub enable_logging: bool,  // 是否启用 SQL 日志，默认 false
}
// 实现 Default
```

### Database（连接池管理器）

```rust
pub struct Database { /* 内部持有 MySqlPool */ }
```

| 方法 | 签名 | 说明 |
|------|------|------|
| `connect` | `async fn connect(url: &str) -> Result<Self>` | 使用默认配置连接 |
| `connect_with_config` | `async fn connect_with_config(url: &str, config: DatabaseConfig) -> Result<Self>` | 使用自定义配置连接 |
| `table` | `fn table(&self, table_name: &str) -> QueryBuilder<'_>` | 创建查询构建器 |
| `query` | `async fn query<T: FromRow>(sql: &str) -> Result<Vec<T>>` | 原生 SELECT 查询 |
| `execute` | `async fn execute(sql: &str) -> Result<u64>` | 原生 INSERT/UPDATE/DELETE |
| `query_with_params` | `async fn query_with_params<T: FromRow>(sql: &str, params: Vec<Value>) -> Result<Vec<T>>` | 参数化 SELECT（防注入） |
| `execute_with_params` | `async fn execute_with_params(sql: &str, params: Vec<Value>) -> Result<u64>` | 参数化写操作（防注入） |
| `transaction` | `async fn transaction() -> Result<Transaction>` | 开始事务 |
| `init` | `async fn init(sql_script: &str) -> Result<()>` | 批量执行初始化 SQL |
| `create_table` | `async fn create_table(create_sql: &str) -> Result<()>` | 创建表 |
| `drop_table` | `async fn drop_table(table_name: &str) -> Result<()>` | 删除表 |
| `table_exists` | `async fn table_exists(table_name: &str) -> Result<bool>` | 检查表是否存在 |

**连接字符串格式**：`mysql://user:password@host:port/database`

### Transaction（事务）

```rust
pub struct Transaction { /* 内部持有 sqlx::Transaction */ }
```

| 方法 | 签名 | 说明 |
|------|------|------|
| `commit` | `async fn commit(self) -> Result<()>` | 提交事务（消费 self） |
| `rollback` | `async fn rollback(self) -> Result<()>` | 回滚事务（消费 self） |
| `execute` | `async fn execute(&mut self, sql: &str) -> Result<u64>` | 事务内原生执行 |
| `execute_with_params` | `async fn execute_with_params(&mut self, sql: &str, params: Vec<Value>) -> Result<u64>` | 事务内参数化执行 |
| `query_with_params` | `async fn query_with_params<T: FromRow>(&mut self, sql, params) -> Result<Vec<T>>` | 事务内参数化查询 |
| `table` | `fn table(&mut self, table_name: &str) -> TransactionQueryBuilder<'_>` | 事务内查询构建器 |

### Condition（WHERE 条件）

```rust
pub enum Condition {
    Eq(String, SqlValue),                    // field = value
    Ne(String, SqlValue),                    // field != value
    Gt(String, SqlValue),                    // field > value
    Lt(String, SqlValue),                    // field < value
    Gte(String, SqlValue),                   // field >= value
    Lte(String, SqlValue),                   // field <= value
    In(String, Vec<SqlValue>),               // field IN (...)
    Between(String, SqlValue, SqlValue),     // field BETWEEN a AND b
    Like(String, String),                    // field LIKE pattern
    IsNull(String),                          // field IS NULL
    IsNotNull(String),                       // field IS NOT NULL
    And(Vec<Condition>),                     // (cond1 AND cond2 AND ...)
    Or(Vec<Condition>),                      // (cond1 OR cond2 OR ...)
}
```

**自由函数**：
- `condition_to_sql(condition: &Condition, params: &mut Vec<SqlValue>) -> String`（借用）
- `condition_to_sql_owned(condition: Condition, params: &mut Vec<SqlValue>) -> String`（消费）

### SqlValue（SQL 参数类型）

```rust
pub enum SqlValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    Json(JsonValue),
    DateTime(NaiveDateTime),
    Timestamp(i64),
}
```

**From 转换**（自动推断类型）：
`i32`, `i64`, `u64`, `f64`, `f32`, `String`, `&str`, `bool`, `Vec<u8>`, `JsonValue`, `NaiveDateTime`, `Option<T: Into<SqlValue>>`

### QueryBuilder（链式查询构建器）

`Database::table(name)` 返回 `QueryBuilder<'_>`，所有构建方法消费并返回 `Self`（链式调用）。

#### 字段选择

| 方法 | 说明 |
|------|------|
| `.field("name")` | 添加单个字段 |
| `.fields(&["id", "name"])` | 批量添加字段 |
| `.distinct()` | 添加 DISTINCT |
| `.json("config")` | 将字段标记为 JSON 类型（反序列化时自动解析） |
| `.datetime("created_at")` | 标记为 DateTime 类型 |
| `.timestamp("ts")` | 标记为 Timestamp 类型 |
| `.decimal("price")` | 标记为 Decimal 类型 |
| `.blob("data")` | 标记为 BLOB 类型（base64 编解码） |
| `.text("content")` | 标记为 Text 类型 |

#### WHERE 条件

```rust
// 检查版本：操作符不合法时返回 Err(DbError::UnsupportedOperator)
.where_and(field, op, value)  -> Result<Self>
.where_or(field, op, value)   -> Result<Self>
.having_cond(field, op, value) -> Result<Self>

// 非检查版本：操作符不合法时 panic
.where_and_unchecked(field, op, value) -> Self
.where_or_unchecked(field, op, value)  -> Self
.having_cond_unchecked(field, op, value) -> Self
```

**支持的操作符**：`=`、`!=`、`>`、`<`、`>=`、`<=`、`like`/`LIKE`

快捷条件方法（无需指定操作符）：
```rust
.where_in(field, Vec<V: Into<SqlValue>>)         // IN (...)
.where_between(field, start, end)                 // BETWEEN
.where_null(field)                                // IS NULL
.where_not_null(field)                            // IS NOT NULL
```

#### 排序 / 分组 / 分页

```rust
.order(field, asc: bool)   // ORDER BY field ASC/DESC
.group(field)              // GROUP BY field
.limit(n: u64)             // LIMIT n
.offset(n: u64)            // OFFSET n
```

#### JOIN

```rust
.join(table, on)        // INNER JOIN table ON on
.left_join(table, on)   // LEFT JOIN table ON on
.right_join(table, on)  // RIGHT JOIN table ON on
```

#### 查询执行（SELECT）

```rust
// 单条查询
.find::<T>() -> Result<Option<T>>
// 多条查询
.select::<T>() -> Result<Vec<T>>
// 单个字段值
.value::<T>(field: &str) -> Result<Option<T>>
// 聚合
.count() -> Result<i64>
.sum(field) -> Result<Option<f64>>
.avg(field) -> Result<Option<f64>>
.min::<T>(field) -> Result<Option<T>>
.max::<T>(field) -> Result<Option<T>>
// 调试
.to_sql() -> String
```

所有查询类型 `T` 需实现 `sqlx::FromRow`。

#### 写操作

```rust
// 插入（data: &impl Serialize）
.insert(data) -> Result<u64>                            // 返回 last_insert_id
// 批量插入（自动分批，默认 batch_size=500）
.insert_batch(data: &[T]) -> Result<u64>
.insert_batch_with_size(data: &[T], batch_size: usize) -> Result<u64>
// 更新（需配合 .where_and() 设置条件，否则返回 MissingWhereClause）
.update(data) -> Result<u64>                            // 返回 affected_rows
// 批量更新（CASE WHEN，自动分批，默认 batch_size=1000）
.update_batch(records: &[T], where_field: &str) -> Result<u64>
// 删除（需配合 .where_and() 设置条件，否则返回 MissingWhereClause）
.delete() -> Result<u64>
// Upsert（INSERT ... ON DUPLICATE KEY UPDATE）
.upsert(data) -> Result<u64>
```

> **安全设计**：`update` 和 `delete` 在没有 WHERE 条件时返回 `MissingWhereClause` 错误，禁止全表操作。

#### SQL 注入防护

- **值**：所有用户值通过 `?` 占位符参数化绑定，不拼接进 SQL 字符串（注入安全）。
- **标识符（表名/列名）**：yang-db 在 `mysql/identifier.rs`（反引号方言）和 `postgres/identifier.rs`（双引号方言）中各自实现了标识符转义，均独立导出 `is_valid_identifier`、`quote_identifier`、`quote_qualified`，不依赖 yang-base。写路径（INSERT/UPDATE/DELETE/UPSERT 的表名与列名）在 SQL 生成时调用 `quote_identifier`，无效标识符返回 `DbError::InvalidArgument`。`condition_to_sql_owned` 使用 `safe_quote_identifier`，对包含 `.`、`(` 等的表达式（如 `a.b`、`COUNT(*)`）会降级为原始值并打 `log::warn`。`field()`/`order()`/`group()`/JOIN ON 有意接受 SQL 表达式、不强制 quote；**通过这些路径传入外部输入的调用方，仍须手动用 `quote_identifier`/`quote_qualified` 校验**，上层 yang-base 类型层（`TableEntity` 派生的封闭字段枚举）是阻止任意字符串列名的推荐防线。

#### 完整示例

```rust
use yang_db::{Database, DatabaseConfig, Condition, SqlValue};

let db = Database::connect_with_config(
    "mysql://root:pass@localhost/mydb",
    DatabaseConfig { max_connections: 20, enable_logging: true, ..Default::default() },
).await?;

// 查询
#[derive(sqlx::FromRow)]
struct User { id: i64, name: String }

let users = db.table("users")
    .fields(&["id", "name"])
    .where_and("status", "=", 1)?
    .where_and("age", ">=", 18)?
    .order("created_at", false)
    .limit(10)
    .select::<User>()
    .await?;

// 插入
#[derive(serde::Serialize)]
struct NewUser { name: String, email: String }

let id = db.table("users")
    .insert(&NewUser { name: "Alice".into(), email: "alice@example.com".into() })
    .await?;

// 更新
db.table("users")
    .where_and("id", "=", id)?
    .update(&serde_json::json!({ "status": 1 }))
    .await?;

// 事务
let mut tx = db.transaction().await?;
tx.execute("INSERT INTO logs (msg) VALUES ('hello')").await?;
tx.table("users")
    .where_and("id", "=", 1)
    .update(&serde_json::json!({ "updated": true }))
    .await?;
tx.commit().await?;
```

---

## Redis 子系统

### RedisConfig

```rust
pub struct RedisConfig {
    pub max_connections: usize,      // 最大连接数
    pub min_connections: usize,      // 最小连接数
    pub connect_timeout: u64,        // 连接超时（秒）
    pub wait_timeout: u64,           // 等待可用连接超时（秒）
    pub idle_timeout: u64,           // 空闲超时（秒）
    pub max_lifetime: Option<u64>,   // 连接最大生命周期（秒），None=不限
    pub test_before_acquire: bool,   // 获取连接前 PING 检测
    pub enable_logging: bool,        // 是否启用日志
}
```

各字段均可通过 Builder 方法（`with_max_connections`、`with_min_connections` 等）配置。注意：`connect_with_config` 当前仅将 `max_connections`/`wait_timeout`/`connect_timeout`/`idle_timeout`/`enable_logging` 五个字段应用到 deadpool 连接池；`min_connections`/`max_lifetime`/`test_before_acquire` 暂未被连接层消费（`max_lifetime_duration()` 标 `#[allow(dead_code)]`）。

### RedisClient（核心客户端）

```rust
pub struct RedisClient { /* 内部持有 deadpool_redis::Pool */ }
```

**连接**：
```rust
RedisClient::connect(url) -> Result<Self>
RedisClient::connect_with_config(url, config) -> Result<Self>
```

**连接字符串格式**：`redis://host:port` 或 `redis://:password@host:port`

#### String 操作

| 方法 | 签名 | 说明 |
|------|------|------|
| `set` | `(key, value) -> Result<()>` | SET |
| `get` | `(key) -> Result<Option<String>>` | GET |
| `setex` | `(key, seconds, value) -> Result<()>` | SETEX（带过期） |
| `setnx` | `(key, value) -> Result<bool>` | SETNX（不存在时设置） |
| `getset` | `(key, value) -> Result<Option<String>>` | GETSET（设置并返回旧值） |
| `mget` | `(keys: &[String]) -> Result<Vec<Option<String>>>` | MGET 批量获取 |
| `mset` | `(pairs: &[(String, String)]) -> Result<()>` | MSET 批量设置 |
| `incr` | `(key) -> Result<i64>` | INCR |
| `incrby` | `(key, increment: i64) -> Result<i64>` | INCRBY |
| `incrbyfloat` | `(key, increment: f64) -> Result<f64>` | INCRBYFLOAT |
| `decr` | `(key) -> Result<i64>` | DECR |
| `decrby` | `(key, decrement: i64) -> Result<i64>` | DECRBY |
| `append` | `(key, value) -> Result<i64>` | APPEND（返回追加后长度） |
| `strlen` | `(key) -> Result<i64>` | STRLEN |
| `getrange` | `(key, start, end) -> Result<String>` | GETRANGE |
| `setrange` | `(key, offset, value) -> Result<i64>` | SETRANGE |
| `psetex` | `(key, milliseconds, value) -> Result<()>` | PSETEX（毫秒级过期） |

#### Hash 操作

| 方法 | 签名 | 说明 |
|------|------|------|
| `hset` | `(key, field, value) -> Result<bool>` | HSET（true=新字段） |
| `hget` | `(key, field) -> Result<Option<String>>` | HGET |
| `hmset` | `(key, fields: &[(String, String)]) -> Result<()>` | HMSET 批量设置 |
| `hmget` | `(key, fields: &[String]) -> Result<Vec<Option<String>>>` | HMGET 批量获取 |
| `hdel` | `(key, fields: &[String]) -> Result<i64>` | HDEL |
| `hexists` | `(key, field) -> Result<bool>` | HEXISTS |
| `hgetall` | `(key) -> Result<Vec<(String, String)>>` | HGETALL |
| `hlen` | `(key) -> Result<i64>` | HLEN |
| `hkeys` | `(key) -> Result<Vec<String>>` | HKEYS |
| `hvals` | `(key) -> Result<Vec<String>>` | HVALS |
| `hincrby` | `(key, field, increment: i64) -> Result<i64>` | HINCRBY |
| `hincrbyfloat` | `(key, field, increment: f64) -> Result<f64>` | HINCRBYFLOAT |

#### List 操作

| 方法 | 签名 | 说明 |
|------|------|------|
| `lpush` | `(key, values: &[String]) -> Result<i64>` | LPUSH（返回列表长度） |
| `rpush` | `(key, values: &[String]) -> Result<i64>` | RPUSH |
| `lpop` | `(key) -> Result<Option<String>>` | LPOP |
| `rpop` | `(key) -> Result<Option<String>>` | RPOP |
| `lrange` | `(key, start, stop) -> Result<Vec<String>>` | LRANGE |
| `llen` | `(key) -> Result<i64>` | LLEN |
| `lindex` | `(key, index) -> Result<Option<String>>` | LINDEX |
| `lset` | `(key, index, value) -> Result<()>` | LSET |
| `ltrim` | `(key, start, stop) -> Result<()>` | LTRIM |
| `linsert` | `(key, "BEFORE"\|"AFTER", pivot, value) -> Result<i64>` | LINSERT |
| `lrem` | `(key, count, value) -> Result<i64>` | LREM |
| `rpoplpush` | `(source, destination) -> Result<Option<String>>` | RPOPLPUSH |
| `blpop` | `(keys: &[String], timeout) -> Result<Option<(String, String)>>` | BLPOP 阻塞弹出 |
| `brpop` | `(keys: &[String], timeout) -> Result<Option<(String, String)>>` | BRPOP 阻塞弹出 |

#### Set 操作

| 方法 | 签名 | 说明 |
|------|------|------|
| `sadd` | `(key, members: &[String]) -> Result<i64>` | SADD |
| `srem` | `(key, members: &[String]) -> Result<i64>` | SREM |
| `smembers` | `(key) -> Result<Vec<String>>` | SMEMBERS |
| `sismember` | `(key, member) -> Result<bool>` | SISMEMBER |
| `scard` | `(key) -> Result<i64>` | SCARD |
| `spop` | `(key) -> Result<Option<String>>` | SPOP 随机弹出 |
| `srandmember` | `(key) -> Result<Option<String>>` | SRANDMEMBER |
| `sinter` | `(keys: &[String]) -> Result<Vec<String>>` | SINTER 交集 |
| `sunion` | `(keys: &[String]) -> Result<Vec<String>>` | SUNION 并集 |
| `sdiff` | `(keys: &[String]) -> Result<Vec<String>>` | SDIFF 差集 |
| `smove` | `(source, dest, member) -> Result<bool>` | SMOVE |
| `sscan` | `(key, cursor, pattern?, count?) -> Result<(i64, Vec<String>)>` | SSCAN 游标扫描 |

#### Sorted Set（有序集合）操作

| 方法 | 签名 | 说明 |
|------|------|------|
| `zadd` | `(key, members: &[(f64, String)]) -> Result<i64>` | ZADD（score, member） |
| `zrem` | `(key, members: &[String]) -> Result<i64>` | ZREM |
| `zscore` | `(key, member) -> Result<Option<f64>>` | ZSCORE |
| `zcard` | `(key) -> Result<i64>` | ZCARD |
| `zrank` | `(key, member) -> Result<Option<i64>>` | ZRANK（从低到高排名） |
| `zrevrank` | `(key, member) -> Result<Option<i64>>` | ZREVRANK（从高到低排名） |
| `zrange` | `(key, start, stop) -> Result<Vec<String>>` | ZRANGE（低分到高分） |
| `zrevrange` | `(key, start, stop) -> Result<Vec<String>>` | ZREVRANGE（高分到低分） |
| `zrange_with_scores` | `(key, start, stop) -> Result<Vec<(String, f64)>>` | ZRANGE WITHSCORES |
| `zrevrange_with_scores` | `(key, start, stop) -> Result<Vec<(String, f64)>>` | ZREVRANGE WITHSCORES |
| `zrangebyscore` | `(key, min: f64, max: f64) -> Result<Vec<String>>` | ZRANGEBYSCORE |
| `zcount` | `(key, min, max) -> Result<i64>` | ZCOUNT |
| `zincrby` | `(key, increment: f64, member) -> Result<f64>` | ZINCRBY |
| `zremrangebyrank` | `(key, start, stop) -> Result<i64>` | ZREMRANGEBYRANK |
| `zremrangebyscore` | `(key, min, max) -> Result<i64>` | ZREMRANGEBYSCORE |
| `zscan` | `(key, cursor, pattern?, count?) -> Result<(i64, Vec<(String, f64)>)>` | ZSCAN |

#### 通用键操作

| 方法 | 签名 | 说明 |
|------|------|------|
| `del` | `(keys: &[String]) -> Result<i64>` | DEL（返回删除数量） |
| `exists` | `(keys: &[String]) -> Result<i64>` | EXISTS（返回存在数量） |
| `expire` | `(key, seconds: i64) -> Result<bool>` | EXPIRE |
| `ttl` | `(key) -> Result<i64>` | TTL（-1=永不过期，-2=不存在） |
| `persist` | `(key) -> Result<bool>` | PERSIST（移除过期时间） |
| `keys` | `(pattern) -> Result<Vec<String>>` | KEYS（生产环境慎用） |
| `scan` | `(cursor, pattern?, count?) -> Result<(i64, Vec<String>)>` | SCAN 游标扫描（推荐替代 KEYS） |

#### Pub/Sub、Lua、健康检查

```rust
// 发布消息
client.publish(channel, message) -> Result<i64>  // 返回订阅者数量

// Lua 脚本
let script = client.script("return redis.call('GET', KEYS[1])");
let result: String = client.eval_script(&script, &["mykey".into()], &[]).await?;

// 原始命令
client.execute(&redis::cmd("PING")) -> Result<RedisValue>

// 统一管理面：错误不会被吞成 Ok(false)
client.health_check().await -> Result<bool, DbError>
client.close().await;
client.is_closed() -> bool

// 连接池状态
let status: PoolStatus = client.pool_status();
// PoolStatus { max_size, size, available, waiting }
```

### RedisPipeline（非原子批量操作）

通过 `client.pipeline()` 获取，执行多条命令一次性发送到服务器（非原子）：

```rust
let mut pipeline = client.pipeline();
pipeline
    .set("k1", "v1")
    .set("k2", "v2")
    .get("k1")
    .incr("counter")
    .hset("user:1", "name", "Alice");

let results: Vec<RedisValue> = pipeline.execute().await?;
// 或者
let typed: Vec<String> = pipeline.query::<String>().await?;
```

**支持的命令**：`set`, `get`, `del`, `incr`, `hset`, `hget`, `lpush`, `rpush`, `sadd`, `zadd`, 以及 `.cmd(redis::Cmd)` 添加自定义命令。

### RedisTransaction（WATCH/MULTI/EXEC 乐观锁事务）

通过 `client.transaction()` 获取：

```rust
let mut tx = client.transaction();
tx.watch(&["balance".into()]);  // WATCH key（乐观锁）
tx.set("balance", "900")
  .incr("version");

// exec 自动重试（WATCH 冲突时，最多 100 次）
let (balance, version): (String, i64) = tx.exec().await?;
// 或者获取原始 RedisValue
let results: Vec<RedisValue> = tx.execute().await?;
```

**支持的命令**：同 `RedisPipeline` + `.watch(keys)` + `.decrby(key, decrement)`。

**自动重试**：`exec()` 检测到 WATCH 冲突（nil 响应）时自动重试，最多 100 次。

---

## 已知问题

| 问题 | 影响 | 状态 |
|------|------|------|
| `insert_batch` 无自动分批（**已修复**，现有 `insert_batch_with_size`） | 大数据集可能超过 max_allowed_packet | 已修复 |
| `having_cond` 中操作符验证 | `having_cond_unchecked` 不检查操作符合法性 | 低优先级 |
| 原生 SQL 逃生舱 | 调用方必须自行保证 SQL 来源可信并完成审计 | 明确边界 |
