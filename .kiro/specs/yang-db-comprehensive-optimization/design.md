# yang-db 综合优化设计文档

## 概述

本文档描述 yang-db 基础库综合优化的技术设计方案。yang-db 是一个 Rust 数据库操作库，提供类型安全的 MySQL 和 Redis 操作接口。

### 设计目标

1. **功能完善**：为 Redis 模块添加 Pipeline、事务、Lua 脚本等核心功能；为 MySQL 模块添加聚合函数、HAVING 子句、批量更新等高级特性
2. **性能优化**：通过批量操作、Pipeline、连接池复用等技术提升性能
3. **类型安全**：利用 Rust 类型系统保证编译时安全，防止常见错误
4. **向后兼容**：保持现有 API 不变，新功能以扩展方式添加
5. **测试覆盖**：提供完善的单元测试、集成测试和属性测试

### 优化范围

**阶段一（P0）- Redis 高优先级功能**：
- Pipeline 批量操作机制
- 事务支持（MULTI/EXEC/WATCH）
- 完善 String 操作（GETRANGE、SETRANGE、INCRBYFLOAT、PSETEX）
- 完善 List 操作（LINSERT、LREM、RPOPLPUSH、BLPOP、BRPOP）

**阶段二（P1）- MySQL 和 Redis 中优先级功能**：
- MySQL 聚合函数（AVG、MIN、MAX）
- MySQL HAVING 子句支持
- MySQL 批量 UPDATE 操作
- Redis Lua 脚本支持
- Redis Set 高级操作（SINTER、SUNION、SDIFF、SMOVE、SSCAN）
- Redis Sorted Set 高级操作（ZRANK、ZREVRANK、ZREVRANGE、ZREMRANGEBYRANK、ZREMRANGEBYSCORE、ZSCAN）
- Redis SCAN 迭代器支持

## 架构

### 整体架构

```
yang-db/
├── src/
│   ├── lib.rs                    # 库入口，导出公共 API
│   ├── error.rs                  # 统一错误类型定义
│   ├── mysql/                    # MySQL 模块
│   │   ├── mod.rs               # MySQL 模块入口
│   │   ├── database.rs          # 数据库连接管理
│   │   ├── query_builder.rs    # 查询构建器（扩展聚合函数、HAVING、批量更新）
│   │   ├── condition.rs         # 条件构建
│   │   ├── field.rs             # 字段类型定义
│   │   ├── transaction.rs       # 事务支持
│   │   └── init.rs              # 初始化逻辑
│   └── redis/                    # Redis 模块
│       ├── mod.rs               # Redis 模块入口
│       ├── client.rs            # Redis 客户端（扩展 String、List 操作）
│       ├── pipeline.rs          # Pipeline 批量操作（新增）
│       ├── transaction.rs       # 事务支持（新增）
│       ├── script.rs            # Lua 脚本支持（新增）
│       ├── scan.rs              # SCAN 迭代器（新增）
│       ├── config.rs            # 配置管理
│       └── value.rs             # 值类型转换
└── tests/                        # 集成测试
    ├── integration_mysql_*.rs   # MySQL 集成测试
    └── integration_redis_*.rs   # Redis 集成测试
```

### 模块职责

#### MySQL 模块

**QueryBuilder（查询构建器）**：
- 现有功能：SELECT、INSERT、UPDATE、DELETE、JOIN、WHERE、ORDER BY、GROUP BY
- 新增功能：
  - 聚合函数：`avg(field)`、`min(field)`、`max(field)`
  - HAVING 子句：`having(condition)`
  - 批量更新：`update_batch(records, where_field)`

**Database（数据库连接）**：
- 连接池管理（基于 sqlx）
- 事务支持
- 查询执行

#### Redis 模块

**RedisClient（Redis 客户端）**：
- 现有功能：String、Hash、List、Set、Sorted Set 基础操作
- 新增功能：
  - String：`getrange`、`setrange`、`incrbyfloat`、`psetex`
  - List：`linsert`、`lrem`、`rpoplpush`、`blpop`、`brpop`
  - Set：`sinter`、`sunion`、`sdiff`、`smove`、`sscan`
  - Sorted Set：`zrank`、`zrevrank`、`zrevrange`、`zremrangebyrank`、`zremrangebyscore`、`zscan`

**RedisPipeline（Pipeline）**：
- 批量命令添加
- 批量执行
- 结果按序返回

**RedisTransaction（事务）**：
- MULTI/EXEC/DISCARD
- WATCH/UNWATCH
- 原子性保证

**RedisScript（Lua 脚本）**：
- 脚本加载和缓存
- 脚本执行（EVAL/EVALSHA）
- 参数传递

**RedisScan（SCAN 迭代器）**：
- SCAN/HSCAN/SSCAN/ZSCAN
- 游标管理
- 迭代器 API

### 依赖关系

```
┌─────────────────────────────────────────────────────────┐
│                      应用层                              │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│                    yang-db 公共 API                      │
│  Database, RedisClient, QueryBuilder, Pipeline, etc.    │
└─────────────────────────────────────────────────────────┘
                          │
          ┌───────────────┴───────────────┐
          ▼                               ▼
┌──────────────────────┐       ┌──────────────────────┐
│    MySQL 模块         │       │    Redis 模块         │
│  - QueryBuilder      │       │  - RedisClient       │
│  - Database          │       │  - RedisPipeline     │
│  - Transaction       │       │  - RedisTransaction  │
│  - Condition         │       │  - RedisScript       │
└──────────────────────┘       │  - RedisScan         │
          │                    └──────────────────────┘
          ▼                               │
┌──────────────────────┐                 ▼
│       sqlx           │       ┌──────────────────────┐
│  - MySqlPool         │       │  deadpool-redis      │
│  - MySqlConnection   │       │  - Pool              │
│  - Query             │       │  - Connection        │
└──────────────────────┘       └──────────────────────┘
                                          │
                                          ▼
                                ┌──────────────────────┐
                                │       redis          │
                                │  - Cmd               │
                                │  - Value             │
                                └──────────────────────┘
```

## 组件和接口

### MySQL 组件

#### 1. QueryBuilder 聚合函数扩展

```rust
impl<'a> QueryBuilder<'a> {
    /// 计算字段平均值
    ///
    /// # 参数
    /// - field: 字段名
    ///
    /// # 返回
    /// - Ok(Some(f64)): 平均值
    /// - Ok(None): 没有匹配记录或字段值全为 NULL
    ///
    /// # 示例
    /// ```rust
    /// let avg_age = db.table("users")
    ///     .where_and("status", "=", 1)
    ///     .avg("age")
    ///     .await?;
    /// ```
    pub async fn avg(self, field: &str) -> Result<Option<f64>, DbError>;

    /// 获取字段最小值
    ///
    /// # 参数
    /// - field: 字段名
    ///
    /// # 返回
    /// - Ok(Some(T)): 最小值
    /// - Ok(None): 没有匹配记录
    ///
    /// # 示例
    /// ```rust
    /// let min_price: Option<f64> = db.table("products")
    ///     .min("price")
    ///     .await?;
    /// ```
    pub async fn min<T>(self, field: &str) -> Result<Option<T>, DbError>
    where
        T: for<'r> sqlx::Decode<'r, sqlx::MySql> + sqlx::Type<sqlx::MySql> + Send + Unpin;

    /// 获取字段最大值
    ///
    /// # 参数
    /// - field: 字段名
    ///
    /// # 返回
    /// - Ok(Some(T)): 最大值
    /// - Ok(None): 没有匹配记录
    ///
    /// # 示例
    /// ```rust
    /// let max_score: Option<i32> = db.table("scores")
    ///     .max("score")
    ///     .await?;
    /// ```
    pub async fn max<T>(self, field: &str) -> Result<Option<T>, DbError>
    where
        T: for<'r> sqlx::Decode<'r, sqlx::MySql> + sqlx::Type<sqlx::MySql> + Send + Unpin;
}
```

#### 2. QueryBuilder HAVING 子句支持

```rust
impl<'a> QueryBuilder<'a> {
    /// 添加 HAVING 条件
    ///
    /// # 参数
    /// - condition: HAVING 条件表达式
    ///
    /// # 返回
    /// 返回 self 以支持链式调用
    ///
    /// # 注意
    /// - HAVING 必须与 GROUP BY 一起使用
    /// - HAVING 条件可以使用聚合函数
    ///
    /// # 示例
    /// ```rust
    /// let result = db.table("orders")
    ///     .field("user_id")
    ///     .field("SUM(amount) as total")
    ///     .group("user_id")
    ///     .having("SUM(amount) > 1000")
    ///     .select::<OrderSummary>()
    ///     .await?;
    /// ```
    pub fn having(mut self, condition: &str) -> Self;

    /// 添加 HAVING 条件（带参数）
    ///
    /// # 参数
    /// - field: 聚合函数表达式（如 "COUNT(*)"、"SUM(amount)"）
    /// - op: 比较运算符（"="、">"、"<"、">="、"<="、"!="）
    /// - value: 比较值
    ///
    /// # 示例
    /// ```rust
    /// let result = db.table("orders")
    ///     .field("user_id")
    ///     .field("COUNT(*) as order_count")
    ///     .group("user_id")
    ///     .having_cond("COUNT(*)", ">", 5)
    ///     .select::<UserOrderCount>()
    ///     .await?;
    /// ```
    pub fn having_cond<V>(mut self, field: &str, op: &str, value: V) -> Self
    where
        V: Into<SqlValue>;
}
```

#### 3. QueryBuilder 批量 UPDATE 支持

```rust
impl<'a> QueryBuilder<'a> {
    /// 批量更新记录
    ///
    /// # 参数
    /// - records: 要更新的记录列表（JSON 格式）
    /// - where_field: 用于匹配记录的字段名（通常是主键，如 "id"）
    ///
    /// # 返回
    /// - Ok(u64): 受影响的行数
    /// - Err(DbError): 更新失败
    ///
    /// # 实现策略
    /// 使用 CASE WHEN 语句实现批量更新：
    /// ```sql
    /// UPDATE table_name
    /// SET
    ///   field1 = CASE
    ///     WHEN id = ? THEN ?
    ///     WHEN id = ? THEN ?
    ///     ...
    ///   END,
    ///   field2 = CASE
    ///     WHEN id = ? THEN ?
    ///     WHEN id = ? THEN ?
    ///     ...
    ///   END
    /// WHERE id IN (?, ?, ...)
    /// ```
    ///
    /// # 性能优化
    /// - 自动分批处理（默认每批 1000 条）
    /// - 使用参数化查询防止 SQL 注入
    /// - 单次数据库往返完成批量更新
    ///
    /// # 示例
    /// ```rust
    /// let updates = vec![
    ///     json!({"id": 1, "name": "张三", "age": 25}),
    ///     json!({"id": 2, "name": "李四", "age": 30}),
    ///     json!({"id": 3, "name": "王五", "age": 28}),
    /// ];
    ///
    /// let affected = db.table("users")
    ///     .update_batch(&updates, "id")
    ///     .await?;
    /// ```
    pub async fn update_batch<T>(
        self,
        records: &[T],
        where_field: &str,
    ) -> Result<u64, DbError>
    where
        T: serde::Serialize;
}
```

### Redis 组件

#### 1. RedisClient String 操作扩展

```rust
impl RedisClient {
    /// GETRANGE - 获取字符串的子串
    ///
    /// # 参数
    /// - key: 键
    /// - start: 起始偏移量（0 表示第一个字符，-1 表示最后一个字符）
    /// - end: 结束偏移量（包含）
    ///
    /// # 返回
    /// - Ok(String): 子串内容
    ///
    /// # 示例
    /// ```rust
    /// // 假设 key "mykey" 的值是 "Hello World"
    /// let substr = client.getrange("mykey", 0, 4).await?;  // "Hello"
    /// let substr = client.getrange("mykey", -5, -1).await?; // "World"
    /// ```
    pub async fn getrange(
        &self,
        key: impl Into<String>,
        start: i64,
        end: i64,
    ) -> Result<String>;

    /// SETRANGE - 从指定偏移量开始替换字符串内容
    ///
    /// # 参数
    /// - key: 键
    /// - offset: 起始偏移量
    /// - value: 要设置的值
    ///
    /// # 返回
    /// - Ok(i64): 修改后字符串的长度
    ///
    /// # 示例
    /// ```rust
    /// client.set("mykey", "Hello World").await?;
    /// let len = client.setrange("mykey", 6, "Redis").await?; // "Hello Redis"
    /// ```
    pub async fn setrange(
        &self,
        key: impl Into<String>,
        offset: i64,
        value: impl Into<String>,
    ) -> Result<i64>;

    /// INCRBYFLOAT - 将键的浮点数值增加指定数量
    ///
    /// # 参数
    /// - key: 键
    /// - increment: 增量（可以是负数）
    ///
    /// # 返回
    /// - Ok(f64): 增加后的值
    ///
    /// # 示例
    /// ```rust
    /// client.set("price", "10.5").await?;
    /// let new_price = client.incrbyfloat("price", 2.3).await?; // 12.8
    /// ```
    pub async fn incrbyfloat(
        &self,
        key: impl Into<String>,
        increment: f64,
    ) -> Result<f64>;

    /// PSETEX - 设置键值并指定毫秒级过期时间
    ///
    /// # 参数
    /// - key: 键
    /// - milliseconds: 过期时间（毫秒）
    /// - value: 值
    ///
    /// # 示例
    /// ```rust
    /// // 设置键值，100 毫秒后过期
    /// client.psetex("session", 100, "data").await?;
    /// ```
    pub async fn psetex(
        &self,
        key: impl Into<String>,
        milliseconds: i64,
        value: impl Into<String>,
    ) -> Result<()>;
}
```

#### 2. RedisClient List 操作扩展

```rust
impl RedisClient {
    /// LINSERT - 在列表的指定元素前或后插入新元素
    ///
    /// # 参数
    /// - key: 键
    /// - before_after: "BEFORE" 或 "AFTER"
    /// - pivot: 参考元素
    /// - value: 要插入的值
    ///
    /// # 返回
    /// - Ok(i64): 插入后列表的长度，-1 表示 pivot 不存在
    ///
    /// # 示例
    /// ```rust
    /// client.rpush("mylist", &["a".to_string(), "c".to_string()]).await?;
    /// client.linsert("mylist", "BEFORE", "c", "b").await?; // ["a", "b", "c"]
    /// ```
    pub async fn linsert(
        &self,
        key: impl Into<String>,
        before_after: &str,
        pivot: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<i64>;

    /// LREM - 删除列表中的指定元素
    ///
    /// # 参数
    /// - key: 键
    /// - count: 删除数量
    ///   - count > 0: 从头到尾删除 count 个匹配元素
    ///   - count < 0: 从尾到头删除 |count| 个匹配元素
    ///   - count = 0: 删除所有匹配元素
    /// - value: 要删除的值
    ///
    /// # 返回
    /// - Ok(i64): 被删除元素的数量
    ///
    /// # 示例
    /// ```rust
    /// client.rpush("mylist", &["a".to_string(), "b".to_string(), "a".to_string()]).await?;
    /// let removed = client.lrem("mylist", 2, "a").await?; // 删除 2 个 "a"
    /// ```
    pub async fn lrem(
        &self,
        key: impl Into<String>,
        count: i64,
        value: impl Into<String>,
    ) -> Result<i64>;

    /// RPOPLPUSH - 从源列表尾部弹出元素并插入到目标列表头部
    ///
    /// # 参数
    /// - source: 源列表键
    /// - destination: 目标列表键
    ///
    /// # 返回
    /// - Ok(Some(String)): 被移动的元素
    /// - Ok(None): 源列表为空
    ///
    /// # 示例
    /// ```rust
    /// client.rpush("list1", &["a".to_string(), "b".to_string()]).await?;
    /// let elem = client.rpoplpush("list1", "list2").await?; // "b"
    /// ```
    pub async fn rpoplpush(
        &self,
        source: impl Into<String>,
        destination: impl Into<String>,
    ) -> Result<Option<String>>;

    /// BLPOP - 阻塞式地从列表头部弹出元素
    ///
    /// # 参数
    /// - keys: 键列表（按顺序检查）
    /// - timeout: 超时时间（秒），0 表示无限等待
    ///
    /// # 返回
    /// - Ok(Some((String, String))): (键名, 元素值)
    /// - Ok(None): 超时
    ///
    /// # 示例
    /// ```rust
    /// let result = client.blpop(&["queue1".to_string(), "queue2".to_string()], 5).await?;
    /// if let Some((key, value)) = result {
    ///     println!("从 {} 弹出: {}", key, value);
    /// }
    /// ```
    pub async fn blpop(
        &self,
        keys: &[String],
        timeout: i64,
    ) -> Result<Option<(String, String)>>;

    /// BRPOP - 阻塞式地从列表尾部弹出元素
    ///
    /// # 参数
    /// - keys: 键列表（按顺序检查）
    /// - timeout: 超时时间（秒），0 表示无限等待
    ///
    /// # 返回
    /// - Ok(Some((String, String))): (键名, 元素值)
    /// - Ok(None): 超时
    ///
    /// # 示例
    /// ```rust
    /// let result = client.brpop(&["queue1".to_string()], 10).await?;
    /// ```
    pub async fn brpop(
        &self,
        keys: &[String],
        timeout: i64,
    ) -> Result<Option<(String, String)>>;
}
```

#### 3. RedisPipeline（Pipeline 批量操作）

```rust
/// Redis Pipeline 批量操作
///
/// Pipeline 允许将多个命令打包发送到 Redis 服务器，减少网络往返次数，提高性能。
///
/// # 特性
/// - 支持所有 Redis 命令
/// - 按添加顺序返回结果
/// - 单次网络往返执行所有命令
/// - 非原子性（与事务不同）
///
/// # 示例
/// ```rust
/// let mut pipeline = client.pipeline();
/// pipeline.set("key1", "value1");
/// pipeline.set("key2", "value2");
/// pipeline.get("key1");
/// pipeline.incr("counter");
///
/// let results = pipeline.execute().await?;
/// ```
pub struct RedisPipeline {
    client: RedisClient,
    commands: Vec<redis::Cmd>,
}

impl RedisPipeline {
    /// 创建新的 Pipeline
    pub fn new(client: RedisClient) -> Self;

    /// 添加 SET 命令
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self;

    /// 添加 GET 命令
    pub fn get(&mut self, key: impl Into<String>) -> &mut Self;

    /// 添加 DEL 命令
    pub fn del(&mut self, keys: &[String]) -> &mut Self;

    /// 添加 INCR 命令
    pub fn incr(&mut self, key: impl Into<String>) -> &mut Self;

    /// 添加 HSET 命令
    pub fn hset(
        &mut self,
        key: impl Into<String>,
        field: impl Into<String>,
        value: impl Into<String>,
    ) -> &mut Self;

    /// 添加 HGET 命令
    pub fn hget(&mut self, key: impl Into<String>, field: impl Into<String>) -> &mut Self;

    /// 添加 LPUSH 命令
    pub fn lpush(&mut self, key: impl Into<String>, values: &[String]) -> &mut Self;

    /// 添加 RPUSH 命令
    pub fn rpush(&mut self, key: impl Into<String>, values: &[String]) -> &mut Self;

    /// 添加 SADD 命令
    pub fn sadd(&mut self, key: impl Into<String>, members: &[String]) -> &mut Self;

    /// 添加 ZADD 命令
    pub fn zadd(&mut self, key: impl Into<String>, members: &[(f64, String)]) -> &mut Self;

    /// 添加自定义命令
    pub fn cmd(&mut self, cmd: redis::Cmd) -> &mut Self;

    /// 执行 Pipeline 中的所有命令
    ///
    /// # 返回
    /// - Ok(Vec<RedisValue>): 按命令添加顺序返回的结果列表
    /// - Err(DbError): 执行失败
    ///
    /// # 错误处理
    /// 如果某个命令失败，返回错误并指明失败的命令索引
    pub async fn execute(self) -> Result<Vec<RedisValue>>;

    /// 清空 Pipeline 中的所有命令
    pub fn clear(&mut self);

    /// 获取 Pipeline 中的命令数量
    pub fn len(&self) -> usize;

    /// 检查 Pipeline 是否为空
    pub fn is_empty(&self) -> bool;
}
```

#### 4. RedisTransaction（事务支持）

```rust
/// Redis 事务
///
/// Redis 事务通过 MULTI/EXEC 命令实现，保证一组命令的原子性执行。
/// 支持 WATCH 命令实现乐观锁。
///
/// # 特性
/// - 原子性：事务内的所有命令要么全部执行，要么全部不执行
/// - 隔离性：事务执行期间，其他客户端的命令不会插入
/// - WATCH 支持：监视键的变化，实现乐观锁
///
/// # 示例
/// ```rust
/// let mut tx = client.multi().await?;
/// tx.set("key1", "value1");
/// tx.incr("counter");
/// tx.get("key1");
///
/// let results = tx.exec().await?;
/// ```
///
/// # WATCH 示例
/// ```rust
/// // 监视键，实现乐观锁
/// let mut tx = client.multi().await?;
/// tx.watch(&["balance".to_string()]).await?;
///
/// let balance: i64 = client.get("balance").await?.unwrap().parse()?;
/// if balance >= 100 {
///     tx.decrby("balance", 100);
///     tx.incr("purchase_count");
///     tx.exec().await?;
/// } else {
///     tx.discard().await?;
/// }
/// ```
pub struct RedisTransaction {
    client: RedisClient,
    commands: Vec<redis::Cmd>,
    watched_keys: Vec<String>,
    started: bool,
}

impl RedisTransaction {
    /// 创建新的事务
    pub fn new(client: RedisClient) -> Self;

    /// 监视一个或多个键
    ///
    /// # 参数
    /// - keys: 要监视的键列表
    ///
    /// # 返回
    /// - Ok(()): 监视成功
    /// - Err(DbError): 监视失败
    ///
    /// # 注意
    /// - 必须在 MULTI 之前调用
    /// - 如果被监视的键在 EXEC 之前被修改，事务将被取消
    pub async fn watch(&mut self, keys: &[String]) -> Result<()>;

    /// 取消所有键的监视
    pub async fn unwatch(&mut self) -> Result<()>;

    /// 添加 SET 命令
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self;

    /// 添加 GET 命令
    pub fn get(&mut self, key: impl Into<String>) -> &mut Self;

    /// 添加 DEL 命令
    pub fn del(&mut self, keys: &[String]) -> &mut Self;

    /// 添加 INCR 命令
    pub fn incr(&mut self, key: impl Into<String>) -> &mut Self;

    /// 添加 DECRBY 命令
    pub fn decrby(&mut self, key: impl Into<String>, decrement: i64) -> &mut Self;

    /// 添加 HSET 命令
    pub fn hset(
        &mut self,
        key: impl Into<String>,
        field: impl Into<String>,
        value: impl Into<String>,
    ) -> &mut Self;

    /// 添加自定义命令
    pub fn cmd(&mut self, cmd: redis::Cmd) -> &mut Self;

    /// 执行事务
    ///
    /// # 返回
    /// - Ok(Vec<RedisValue>): 按命令添加顺序返回的结果列表
    /// - Err(DbError): 执行失败或事务被取消（WATCH 的键被修改）
    ///
    /// # 注意
    /// - 如果 WATCH 的键被修改，返回 TransactionAborted 错误
    /// - 执行后事务自动结束，不能再添加命令
    pub async fn exec(self) -> Result<Vec<RedisValue>>;

    /// 取消事务
    ///
    /// # 返回
    /// - Ok(()): 取消成功
    /// - Err(DbError): 取消失败
    ///
    /// # 注意
    /// - 清空所有已添加的命令
    /// - 取消所有 WATCH 的键
    pub async fn discard(self) -> Result<()>;

    /// 获取事务中的命令数量
    pub fn len(&self) -> usize;

    /// 检查事务是否为空
    pub fn is_empty(&self) -> bool;
}
```
```

