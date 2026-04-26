# 实施计划: Redis 操作功能

## 概述

本实施计划基于设计文档和需求文档，将 Redis 操作功能的开发分为 4 个阶段，每个阶段包含明确的实施任务和测试任务。系统将为 yang-db 库提供完整的 Redis 数据库操作能力。

### 技术栈

- **语言**: Rust
- **异步运行时**: Tokio
- **Redis 客户端**: redis + deadpool-redis
- **序列化**: serde + serde_json
- **错误处理**: thiserror

### 实施原则

1. **类型安全优先**: 充分利用 Rust 类型系统，编译期捕获错误
2. **测试驱动**: 每个功能模块都包含单元测试和集成测试
3. **增量开发**: 每个阶段都能独立运行和测试
4. **文档完善**: 所有公开 API 都包含中文文档注释

## 任务列表

### 第一阶段：基础设施

本阶段实现 Redis 客户端、连接池管理、值类型系统和错误处理扩展。

- [x] 1. 实现 RedisValue 类型系统
  - [x] 1.1 定义 RedisValue 枚举
    - 定义 Nil、Int、Float、String、Bytes、Array、Bool 变体
    - 实现 Debug、Clone、PartialEq trait
    - _需求: REQ-002, REQ-003, REQ-004, REQ-005, REQ-006_
  
  - [x] 1.2 实现 RedisValue 的转换方法
    - 实现 as_string()、as_i64()、as_f64()、as_bool() 等方法
    - 实现 as_bytes()、as_array() 方法
    - 实现 is_nil() 方法
    - _需求: REQ-002, REQ-003_
  
  - [x] 1.3 实现 From trait 支持自动转换
    - 实现 From<String>、From<&str>
    - 实现 From<i64>、From<i32>、From<f64>
    - 实现 From<bool>、From<Vec<u8>>
    - _需求: NFR-003_
  
  - [x] 1.4 实现 redis::Value 到 RedisValue 的转换
    - 实现 From<redis::Value> for RedisValue
    - 处理所有 redis::Value 变体
    - 处理 UTF-8 字符串转换
    - _需求: NFR-002_
  
  - [x] 1.5 编写 RedisValue 单元测试
    - 测试所有类型转换
    - 测试边界条件
    - 测试错误情况
    - _需求: NFR-005_

- [x] 2. 扩展 DbError 错误类型
  - [x] 2.1 添加 Redis 相关错误变体
    - 添加 RedisConnectionError
    - 添加 RedisCommandError
    - 添加 RedisPoolError
    - 添加 RedisTypeConversionError
    - 添加 RedisTimeoutError
    - _需求: NFR-002_
  
  - [x] 2.2 实现错误转换 trait
    - 实现 From<redis::RedisError> for DbError
    - 实现 From<deadpool_redis::PoolError> for DbError
    - 确保错误信息包含中文描述
    - _需求: NFR-003_
  
  - [x] 2.3 编写错误处理单元测试
    - 测试错误转换
    - 测试错误消息
    - _需求: NFR-005_

- [x] 3. 实现 RedisConfig 配置结构
  - [x] 3.1 定义 RedisConfig 结构体
    - 定义 max_connections 字段
    - 定义 connect_timeout 字段
    - 定义 wait_timeout 字段
    - 定义 enable_logging 字段
    - _需求: REQ-001_
  
  - [x] 3.2 实现 Default trait
    - 设置合理的默认值
    - max_connections = 10
    - connect_timeout = 5
    - wait_timeout = 10
    - enable_logging = false
    - _需求: REQ-001_
  
  - [x] 3.3 编写 RedisConfig 单元测试
    - 测试默认配置
    - 测试自定义配置
    - _需求: NFR-005_

- [x] 4. 实现 RedisClient 客户端
  - [x] 4.1 定义 RedisClient 结构体
    - 定义 pool 字段（deadpool_redis::Pool）
    - 定义 config 字段（RedisConfig）
    - _需求: REQ-001_
  
  - [x] 4.2 实现 connect() 方法
    - 解析 Redis URL
    - 创建连接池
    - 测试连接有效性
    - 返回 RedisClient 实例
    - _需求: REQ-001_
  
  - [x] 4.3 实现 connect_with_config() 方法
    - 接收自定义配置
    - 创建连接池
    - 应用配置参数
    - _需求: REQ-001_
  
  - [x] 4.4 实现 pool() 方法
    - 返回连接池引用
    - _需求: REQ-001_
  
  - [x] 4.5 实现 execute() 方法
    - 从连接池获取连接
    - 执行 Redis 命令
    - 转换返回值为 RedisValue
    - 处理错误情况
    - _需求: REQ-001_
  
  - [x] 4.6 编写 RedisClient 单元测试
    - 测试连接创建
    - 测试配置应用
    - 测试错误处理
    - _需求: NFR-005_
  
  - [x] 4.7 编写 RedisClient 集成测试
    - 测试实际 Redis 连接
    - 测试连接池功能
    - 测试命令执行
    - _需求: NFR-001, NFR-002_

- [x] 5. 第一阶段检查点
  - 确保所有测试通过
  - 确保代码通过 cargo clippy 检查
  - 确保代码通过 cargo fmt 检查
  - 如有问题请向用户询问

### 第二阶段：String 和 Hash 操作

本阶段实现 Redis String 和 Hash 类型的所有操作，直接在 RedisClient 上实现。

- [x] 6. 实现 String 操作方法
  - [x] 6.1 实现基本 String 操作
    - 实现 set() 方法
    - 实现 get() 方法
    - 实现 setex() 方法（带过期时间）
    - 实现 setnx() 方法（仅当不存在时设置）
    - 实现 getset() 方法
    - _需求: REQ-002_
  
  - [x] 6.2 实现批量 String 操作
    - 实现 mget() 方法
    - 实现 mset() 方法
    - _需求: REQ-002_
  
  - [x] 6.3 实现计数器操作
    - 实现 incr() 方法
    - 实现 incrby() 方法
    - 实现 decr() 方法
    - 实现 decrby() 方法
    - _需求: REQ-002_
  
  - [x] 6.4 实现字符串操作
    - 实现 append() 方法
    - 实现 strlen() 方法
    - _需求: REQ-002_
  
  - [x] 6.5 编写 String 操作单元测试
    - 测试所有操作的正常情况
    - 测试边界条件
    - 测试错误情况
    - _需求: NFR-005_
  
  - [x] 6.6 编写 String 操作集成测试
    - 测试完整的 String 操作流程
    - 测试过期时间功能
    - 测试计数器功能
    - _需求: REQ-002, NFR-001_

- [x] 7. 实现 Hash 操作方法
  - [x] 7.1 实现基本 Hash 操作
    - 实现 hset() 方法
    - 实现 hget() 方法
    - 实现 hdel() 方法
    - 实现 hexists() 方法
    - _需求: REQ-003_
  
  - [x] 7.2 实现批量 Hash 操作
    - 实现 hmset() 方法
    - 实现 hmget() 方法
    - 实现 hgetall() 方法
    - _需求: REQ-003_
  
  - [x] 7.3 实现 Hash 查询操作
    - 实现 hlen() 方法
    - 实现 hkeys() 方法
    - 实现 hvals() 方法
    - _需求: REQ-003_
  
  - [x] 7.4 实现 Hash 计数器操作
    - 实现 hincrby() 方法
    - 实现 hincrbyfloat() 方法
    - _需求: REQ-003_
  
  - [x] 7.5 编写 Hash 操作单元测试
    - 测试所有操作的正常情况
    - 测试边界条件
    - 测试错误情况
    - _需求: NFR-005_
  
  - [x] 7.6 编写 Hash 操作集成测试
    - 测试完整的 Hash 操作流程
    - 测试批量操作
    - 测试计数器功能
    - _需求: REQ-003, NFR-001_

- [x] 8. 第二阶段检查点
  - 确保所有测试通过
  - 确保 String 和 Hash 操作功能完整
  - 确保性能满足要求
  - 如有问题请向用户询问

### 第三阶段：List、Set 和 Sorted Set 操作

本阶段实现 Redis List、Set 和 Sorted Set 类型的所有操作，直接在 RedisClient 上实现。

- [x] 9. 实现 List 操作方法
  - [x] 9.1 实现 List 推入/弹出操作
    - 实现 lpush() 方法
    - 实现 rpush() 方法
    - 实现 lpop() 方法
    - 实现 rpop() 方法
    - _需求: REQ-004_
  
  - [x] 9.2 实现 List 查询操作
    - 实现 lrange() 方法
    - 实现 llen() 方法
    - 实现 lindex() 方法
    - _需求: REQ-004_
  
  - [x] 9.3 实现 List 修改操作
    - 实现 lset() 方法
    - 实现 ltrim() 方法
    - _需求: REQ-004_
  
  - [x] 9.4 编写 List 操作单元测试
    - 测试所有操作的正常情况
    - 测试边界条件
    - 测试错误情况
    - _需求: NFR-005_
  
  - [x] 9.5 编写 List 操作集成测试
    - 测试完整的 List 操作流程
    - 测试队列功能
    - 测试栈功能
    - _需求: REQ-004, NFR-001_

- [x] 10. 实现 Set 操作方法
  - [x] 10.1 实现 Set 基本操作
    - 实现 sadd() 方法
    - 实现 srem() 方法
    - 实现 smembers() 方法
    - 实现 sismember() 方法
    - 实现 scard() 方法
    - _需求: REQ-005_
  
  - [x] 10.2 实现 Set 随机操作
    - 实现 spop() 方法
    - 实现 srandmember() 方法
    - _需求: REQ-005_
  
  - [x] 10.3 编写 Set 操作单元测试
    - 测试所有操作的正常情况
    - 测试去重功能
    - 测试错误情况
    - _需求: NFR-005_
  
  - [x] 10.4 编写 Set 操作集成测试
    - 测试完整的 Set 操作流程
    - 测试标签系统场景
    - _需求: REQ-005, NFR-001_

- [x] 11. 实现 Sorted Set 操作方法
  - [x] 11.1 实现 Sorted Set 基本操作
    - 实现 zadd() 方法
    - 实现 zrem() 方法
    - 实现 zscore() 方法
    - 实现 zcard() 方法
    - _需求: REQ-006_
  
  - [x] 11.2 实现 Sorted Set 范围查询
    - 实现 zrange() 方法
    - 实现 zrangebyscore() 方法
    - 实现 zcount() 方法
    - _需求: REQ-006_
  
  - [x] 11.3 实现 Sorted Set 计数器操作
    - 实现 zincrby() 方法
    - _需求: REQ-006_
  
  - [x] 11.4 编写 Sorted Set 操作单元测试
    - 测试所有操作的正常情况
    - 测试排序功能
    - 测试错误情况
    - _需求: NFR-005_
  
  - [x] 11.5 编写 Sorted Set 操作集成测试
    - 测试完整的 Sorted Set 操作流程
    - 测试排行榜场景
    - _需求: REQ-006, NFR-001_

- [x] 12. 第三阶段检查点
  - 确保所有测试通过
  - 确保 List、Set、Sorted Set 操作功能完整
  - 确保性能满足要求
  - 如有问题请向用户询问

### 第四阶段：通用操作和优化

本阶段实现通用键操作、性能优化和文档完善。

- [x] 13. 实现通用键操作
  - [x] 13.1 实现 del() 方法
    - 支持删除单个或多个键
    - 返回删除的键数量
    - _需求: REQ-007_
  
  - [x] 13.2 实现 exists() 方法
    - 支持检查单个或多个键
    - 返回存在的键数量
    - _需求: REQ-007_
  
  - [x] 13.3 实现 expire() 方法
    - 设置键的过期时间（秒）
    - 返回是否设置成功
    - _需求: REQ-007_
  
  - [x] 13.4 实现 ttl() 方法
    - 获取键的剩余生存时间
    - 返回秒数（-1 表示永不过期，-2 表示不存在）
    - _需求: REQ-007_
  
  - [x] 13.5 实现 persist() 方法
    - 移除键的过期时间
    - 返回是否移除成功
    - _需求: REQ-007_
  
  - [x] 13.6 实现 keys() 方法
    - 支持模式匹配查找键
    - 返回匹配的键列表
    - 添加性能警告注释
    - _需求: REQ-007_
  
  - [x] 13.7 编写通用键操作单元测试
    - 测试所有操作的正常情况
    - 测试边界条件
    - 测试错误情况
    - _需求: NFR-005_
  
  - [x] 13.8 编写通用键操作集成测试
    - 测试完整的键管理流程
    - 测试过期时间功能
    - _需求: REQ-007, NFR-001_

- [x] 14. 性能优化和测试
  - [x] 14.1 连接池性能优化
    - 调整连接池参数
    - 测试不同配置下的性能
    - _需求: NFR-001_
  
  - [x] 14.2 批量操作性能测试
    - 测试 mget/mset 性能
    - 测试 hmget/hmset 性能
    - 确保批量操作优于单操作
    - _需求: NFR-001_
  
  - [x] 14.3 并发操作性能测试
    - 测试多线程并发访问
    - 测试连接池在高并发下的表现
    - 确保 QPS 满足要求
    - _需求: NFR-001_
  
  - [x] 14.4 内存使用测试
    - 测试长时间运行的内存使用
    - 确保无内存泄漏
    - _需求: NFR-001_

- [x] 15. 文档和示例完善
  - [x] 15.1 完善 API 文档注释
    - 为所有公开 API 添加中文文档注释
    - 包含参数说明、返回值说明
    - 包含使用示例
    - _需求: NFR-003_
  
  - [x] 15.2 编写使用示例
    - 编写基本连接示例
    - 编写各数据类型操作示例
    - 编写实际应用场景示例（缓存、队列、排行榜等）
    - _需求: NFR-003_
  
  - [x] 15.3 更新 README.md
    - 添加 Redis 功能说明
    - 添加快速开始指南
    - 添加 API 概览
    - _需求: NFR-003_
  
  - [x] 15.4 编写迁移指南
    - 说明如何从其他 Redis 客户端迁移
    - 提供常见用法对比
    - _需求: NFR-003_

- [x] 16. 第四阶段检查点
  - 确保所有测试通过
  - 确保性能满足要求
  - 确保文档完整
  - 如有问题请向用户询问

## 注意事项

### 测试策略

- 每个功能模块都包含单元测试和集成测试
- 集成测试需要本地 Redis 服务器（默认 127.0.0.1:6379）
- 测试必须清理测试数据，避免影响其他测试
- 单元测试覆盖率目标 > 80%

### 测试环境配置

**本地 Redis 测试服务器**:
- **地址**: 127.0.0.1:6379
- **密码**: 无
- **容器名**: Redis
- **平台**: Windows 11 + Docker

**连接字符串**:
```rust
let client = RedisClient::connect("redis://127.0.0.1:6379").await?;
```

**验证 Redis 连接**:
```bash
# 进入 Redis 容器
docker exec -it Redis redis-cli

# 测试连接
127.0.0.1:6379> PING
PONG

# 查看所有键
127.0.0.1:6379> KEYS *

# 清空测试数据
127.0.0.1:6379> FLUSHDB
```

**Docker 命令参考**:
```bash
# 启动 Redis 容器
docker start Redis

# 停止 Redis 容器
docker stop Redis

# 查看 Redis 日志
docker logs Redis

# 进入 Redis CLI
docker exec -it Redis redis-cli
```

### 依赖关系

- 第二阶段依赖第一阶段的基础设施
- 第三阶段依赖第一阶段的基础设施
- 第四阶段依赖所有前置阶段

### 代码规范

- 所有代码必须通过 cargo clippy 检查
- 所有代码必须通过 cargo fmt 格式化
- 所有公开 API 必须包含中文文档注释
- 所有测试必须放在 tests/ 目录或模块内的 tests 子模块中

### 性能要求

- 单操作延迟 < 10ms（本地 Redis）
- 并发操作 QPS > 10,000
- 批量操作性能优于单操作
- 连接池能够高效复用连接

### 兼容性要求

- 支持 Redis 5.0 及以上版本
- 支持 Rust 1.70 及以上版本
- 与现有 MySQL 功能无冲突
- 错误处理统一使用 DbError

## 完成标准

当所有任务完成后，系统应该：

1. 提供完整的 Redis 数据库操作能力
2. 支持所有五种 Redis 数据类型
3. 提供类型安全的 API
4. 包含完整的文档和示例
5. 通过所有单元测试和集成测试
6. 满足性能和可靠性要求
7. 与现有 yang-db 功能无缝集成

---

**文档版本**: 1.0.0  
**创建日期**: 2026-04-25  
**最后更新**: 2026-04-25
