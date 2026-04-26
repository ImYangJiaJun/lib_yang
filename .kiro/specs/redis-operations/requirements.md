# 需求文档: Redis 操作功能

## 1. 功能需求

### 1.1 Redis 客户端管理

**需求 ID**: REQ-001  
**优先级**: 高  
**描述**: 提供 Redis 客户端管理功能，支持连接池管理和配置

**验收标准**:
- 支持通过 URL 连接到 Redis 服务器
- 支持自定义连接池配置（最大连接数、超时时间等）
- 连接池能够自动管理连接的创建和释放
- 支持连接健康检查
- 连接失败时提供清晰的错误信息

**用户故事**:
```
作为开发者
我想要连接到 Redis 服务器
以便我可以执行 Redis 操作
```

**示例**:
```rust
// 基本连接
let client = RedisClient::connect("redis://127.0.0.1:6379").await?;

// 自定义配置连接
let config = RedisConfig {
    max_connections: 20,
    connect_timeout: 5,
    wait_timeout: 10,
    enable_logging: true,
};
let client = RedisClient::connect_with_config("redis://127.0.0.1:6379", config).await?;
```

### 1.2 String 类型操作

**需求 ID**: REQ-002  
**优先级**: 高  
**描述**: 支持 Redis String 类型的所有常用操作

**验收标准**:
- 支持 SET/GET 基本操作
- 支持 SETEX（设置过期时间）
- 支持 SETNX（仅当键不存在时设置）
- 支持 MGET/MSET（批量操作）
- 支持 INCR/DECR（计数器操作）
- 支持 APPEND（字符串追加）
- 所有操作都是类型安全的
- 所有操作都是异步的

**用户故事**:
```
作为开发者
我想要存储和获取字符串值
以便我可以实现缓存、会话存储等功能
```

**示例**:
```rust
// 基本操作
client.string().set("key", "value").await?;
let value = client.string().get("key").await?;

// 过期时间
client.string().setex("session:abc", "token", 3600).await?;

// 计数器
let count = client.string().incr("page:views").await?;
```

### 1.3 Hash 类型操作

**需求 ID**: REQ-003  
**优先级**: 高  
**描述**: 支持 Redis Hash 类型的所有常用操作

**验收标准**:
- 支持 HSET/HGET 单字段操作
- 支持 HMSET/HMGET 多字段操作
- 支持 HGETALL 获取所有字段
- 支持 HDEL 删除字段
- 支持 HEXISTS 检查字段存在性
- 支持 HLEN 获取字段数量
- 支持 HINCRBY/HINCRBYFLOAT 字段计数器
- 所有操作都是类型安全的

**用户故事**:
```
作为开发者
我想要存储结构化数据
以便我可以高效地管理对象属性
```

**示例**:
```rust
// 设置用户信息
client.hash().hset("user:1", "name", "张三").await?;
client.hash().hset("user:1", "age", 25).await?;

// 获取所有字段
let user_data = client.hash().hgetall("user:1").await?;
```

### 1.4 List 类型操作

**需求 ID**: REQ-004  
**优先级**: 中  
**描述**: 支持 Redis List 类型的所有常用操作

**验收标准**:
- 支持 LPUSH/RPUSH 推入元素
- 支持 LPOP/RPOP 弹出元素
- 支持 LRANGE 范围查询
- 支持 LLEN 获取长度
- 支持 LINDEX 索引访问
- 支持 LSET 设置元素
- 支持 LTRIM 修剪列表
- 可用于实现队列、栈等数据结构

**用户故事**:
```
作为开发者
我想要使用列表数据结构
以便我可以实现消息队列、任务队列等功能
```

**示例**:
```rust
// 消息队列
client.list().rpush("queue:tasks", vec!["task1", "task2"]).await?;
let task = client.list().lpop("queue:tasks").await?;
```

### 1.5 Set 类型操作

**需求 ID**: REQ-005  
**优先级**: 中  
**描述**: 支持 Redis Set 类型的所有常用操作

**验收标准**:
- 支持 SADD 添加成员
- 支持 SREM 删除成员
- 支持 SMEMBERS 获取所有成员
- 支持 SISMEMBER 检查成员存在性
- 支持 SCARD 获取集合大小
- 支持 SPOP 随机弹出
- 支持 SRANDMEMBER 随机获取
- 自动去重

**用户故事**:
```
作为开发者
我想要使用集合数据结构
以便我可以实现标签系统、去重等功能
```

**示例**:
```rust
// 标签系统
client.set().sadd("article:1:tags", vec!["rust", "database"]).await?;
let has_rust = client.set().sismember("article:1:tags", "rust").await?;
```

### 1.6 Sorted Set 类型操作

**需求 ID**: REQ-006  
**优先级**: 中  
**描述**: 支持 Redis Sorted Set 类型的所有常用操作

**验收标准**:
- 支持 ZADD 添加成员及分数
- 支持 ZREM 删除成员
- 支持 ZSCORE 获取分数
- 支持 ZRANGE 按索引范围查询
- 支持 ZRANGEBYSCORE 按分数范围查询
- 支持 ZCARD 获取大小
- 支持 ZCOUNT 统计范围内成员数
- 支持 ZINCRBY 增加分数
- 成员按分数自动排序

**用户故事**:
```
作为开发者
我想要使用有序集合数据结构
以便我可以实现排行榜、优先级队列等功能
```

**示例**:
```rust
// 排行榜
client.sorted_set().zadd("leaderboard", vec![
    (100.0, "player1"),
    (200.0, "player2"),
]).await?;
let top10 = client.sorted_set().zrange("leaderboard", 0, 9, true).await?;
```

### 1.7 通用键操作

**需求 ID**: REQ-007  
**优先级**: 高  
**描述**: 支持 Redis 通用键操作

**验收标准**:
- 支持 DEL 删除键
- 支持 EXISTS 检查键存在性
- 支持 EXPIRE 设置过期时间
- 支持 TTL 获取剩余生存时间
- 支持 PERSIST 移除过期时间
- 支持 KEYS 模式匹配查找键
- 所有操作支持批量处理

**用户故事**:
```
作为开发者
我想要管理 Redis 键的生命周期
以便我可以实现缓存过期、键清理等功能
```

**示例**:
```rust
// 设置过期时间
client.expire("session:abc", 3600).await?;

// 检查剩余时间
let ttl = client.ttl("session:abc").await?;

// 删除键
client.del(vec!["key1", "key2"]).await?;
```

## 2. 非功能需求

### 2.1 性能需求

**需求 ID**: NFR-001  
**优先级**: 高  
**描述**: 系统性能要求

**验收标准**:
- 单个操作延迟 < 10ms（本地 Redis）
- 支持至少 10,000 QPS（并发操作）
- 连接池能够高效复用连接
- 批量操作性能优于单个操作
- 内存使用合理，无内存泄漏

### 2.2 可靠性需求

**需求 ID**: NFR-002  
**优先级**: 高  
**描述**: 系统可靠性要求

**验收标准**:
- 连接断开时自动重连
- 操作失败时提供清晰的错误信息
- 支持连接超时配置
- 支持操作超时配置
- 异常情况下不会导致程序崩溃

### 2.3 可用性需求

**需求 ID**: NFR-003  
**优先级**: 高  
**描述**: API 易用性要求

**验收标准**:
- API 设计符合 Rust 惯例
- 提供链式调用支持
- 类型安全，编译期捕获错误
- 所有公开 API 都有中文文档注释
- 提供完整的使用示例
- 错误信息清晰易懂

### 2.4 兼容性需求

**需求 ID**: NFR-004  
**优先级**: 中  
**描述**: 系统兼容性要求

**验收标准**:
- 支持 Redis 5.0 及以上版本
- 支持 Rust 1.70 及以上版本
- 与现有 yang-db MySQL 功能无冲突
- 错误处理统一使用 DbError
- 异步运行时统一使用 tokio

### 2.5 可维护性需求

**需求 ID**: NFR-005  
**优先级**: 中  
**描述**: 代码可维护性要求

**验收标准**:
- 代码结构清晰，模块化设计
- 所有代码通过 clippy 检查
- 所有代码通过 fmt 格式化
- 单元测试覆盖率 > 80%
- 集成测试覆盖所有主要功能
- 代码注释完整，易于理解

## 3. 约束条件

### 3.1 技术约束

- 必须使用 `redis` crate 作为底层 Redis 客户端
- 必须使用 `deadpool-redis` 进行连接池管理
- 必须使用 `tokio` 作为异步运行时
- 必须集成到现有 `yang-db` crate 中
- 错误类型必须使用现有的 `DbError` 枚举

### 3.2 设计约束

- API 设计必须与现有 MySQL API 风格一致
- 所有操作必须是异步的
- 必须支持类型安全的值转换
- 必须提供中文文档和错误消息
- 必须遵循 Rust 命名规范

### 3.3 测试约束

- 所有测试必须放在 `__tests__` 文件夹中
- 集成测试需要本地 Redis 服务器
- 测试必须能够独立运行
- 测试必须清理测试数据

**测试环境配置**:
- Redis 服务器地址: 127.0.0.1:6379
- Redis 密码: 无
- Docker 容器名: Redis
- 操作系统: Windows 11

**测试前准备**:
```bash
# 确保 Redis 容器运行
docker start Redis

# 验证 Redis 可访问
docker exec -it Redis redis-cli PING
```

**测试后清理**:
```bash
# 清空测试数据库
docker exec -it Redis redis-cli FLUSHDB
```

## 4. 依赖关系

### 4.1 外部依赖

- Redis 服务器（5.0+）
- redis crate (1.1.0)
- deadpool-redis crate (0.23.0)
- tokio crate (1.51.0)

### 4.2 内部依赖

- yang-db::error::DbError
- yang-db 现有模块结构

## 5. 验收测试

### 5.1 功能测试

- [ ] 能够成功连接到 Redis 服务器
- [ ] 能够执行所有 String 操作
- [ ] 能够执行所有 Hash 操作
- [ ] 能够执行所有 List 操作
- [ ] 能够执行所有 Set 操作
- [ ] 能够执行所有 Sorted Set 操作
- [ ] 能够执行所有通用键操作
- [ ] 错误处理正确

### 5.2 性能测试

- [ ] 单操作延迟满足要求
- [ ] 并发操作 QPS 满足要求
- [ ] 连接池性能满足要求
- [ ] 批量操作性能优于单操作

### 5.3 可靠性测试

- [ ] 连接断开后能够自动重连
- [ ] 超时配置生效
- [ ] 异常情况不会崩溃
- [ ] 错误信息清晰

### 5.4 兼容性测试

- [ ] 在 Redis 5.0 上正常工作
- [ ] 在 Redis 6.0 上正常工作
- [ ] 在 Redis 7.0 上正常工作
- [ ] 与 MySQL 功能无冲突

## 6. 里程碑

### 阶段 1: 基础设施（1-2 天）
- 实现 RedisClient 和连接池管理
- 实现 RedisValue 类型系统
- 实现错误处理扩展
- 编写基础单元测试

### 阶段 2: String 和 Hash 操作（2-3 天）
- 实现 StringOps 所有操作
- 实现 HashOps 所有操作
- 编写单元测试和集成测试
- 编写使用示例

### 阶段 3: List、Set 和 Sorted Set 操作（2-3 天）
- 实现 ListOps 所有操作
- 实现 SetOps 所有操作
- 实现 SortedSetOps 所有操作
- 编写单元测试和集成测试

### 阶段 4: 通用操作和优化（1-2 天）
- 实现通用键操作
- 性能优化
- 文档完善
- 最终测试

## 7. 风险和缓解措施

### 风险 1: Redis 版本兼容性问题
**影响**: 中  
**概率**: 低  
**缓解措施**: 
- 使用 Redis 5.0 的稳定特性
- 在多个 Redis 版本上测试
- 文档中明确版本要求

### 风险 2: 性能不达标
**影响**: 中  
**概率**: 低  
**缓解措施**:
- 使用连接池减少连接开销
- 支持批量操作
- 进行性能测试和优化

### 风险 3: 与现有代码冲突
**影响**: 高  
**概率**: 低  
**缓解措施**:
- 遵循现有代码风格
- 使用统一的错误类型
- 充分的集成测试

---

**文档版本**: 1.0.0  
**创建日期**: 2026-04-25  
**最后更新**: 2026-04-25
