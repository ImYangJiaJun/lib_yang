# 实现计划：yang-db 综合优化（修订版）

## 概述

本实现计划基于 `IMPROVED_OPTIMIZATION_PLAN.md` 深度评审结果，对原优化方案进行了重大修正：

### 关键改进点

1. **架构性修正（CRITICAL）**：
   - Redis Pipeline 基于 redis-rs 的 `redis::pipe()` 原生实现
   - Redis Transaction 基于 redis-rs 的 `transaction_async()` 和 `pipe().atomic()`
   - Redis Lua Script 基于 redis-rs 的 `Script` 类型

2. **Bug 修复（P0 优先级）**：
   - 修复 RedisConfig 连接池参数不生效的 Bug
   - 为 insert_batch 添加自动分批处理

3. **遗漏功能补充**：
   - MySQL UPSERT（INSERT ON DUPLICATE KEY UPDATE）
   - MySQL IS NULL / IS NOT NULL 条件
   - Redis zrange_with_scores / zrevrange_with_scores
   - 连接池健康检查和指标暴露
   - Redis Pub/Sub 基础支持（提升为 P1）

4. **测试策略改进**：
   - 移除所有测试任务的可选标记（`*`），改为必选
   - Redis 测试覆盖率目标从 80% 提升到 85%

5. **设计决策优化**：
   - HAVING 子句只保留结构化 API（having_cond），移除裸字符串版本

### 优化阶段

**P0（必须修复/实现）**：
- Bug 修复（RedisConfig、insert_batch）
- Redis 架构重构（基于 redis-rs 原生能力）
- Redis String/List 操作完善

**P1（重要新增）**：
- MySQL 聚合函数、HAVING、批量更新、UPSERT、IS NULL
- Redis Set/ZSet 高级操作、SCAN、Pub/Sub
- 连接池健康检查

**P2（后续考虑）**：
- Redis Bitmap、HyperLogLog、Geo、Stream
- Redis Cluster 支持
- 性能基准报告、文档完善

## 任务列表

### P0：Bug 修复和架构重构

- [x] 1. 修复现有代码 Bug（CRITICAL）
  - [x] 1.1 修复 RedisConfig 连接池参数不生效
    - 定位问题：`src/redis/client.rs:62-93` 中 `connect_with_config` 未使用 config 参数
    - 修改 `Config` 构建逻辑，正确应用 `max_connections`、`wait_timeout` 等参数
    - 使用 `deadpool_redis::PoolConfig` 和 `Timeouts` 结构
    - 添加单元测试验证配置生效
    - _Bug 修复：IMPROVED_OPTIMIZATION_PLAN.md 第二章 Bug 1_

  - [x] 1.2 为 insert_batch 添加自动分批处理
    - 定位问题：`src/mysql/query_builder.rs:1400-1469` 无分批逻辑
    - 添加 `INSERT_BATCH_SIZE` 常量（默认 500）
    - 实现 `insert_chunk` 内部方法
    - 修改 `insert_batch` 使用 `chunks()` 分批处理
    - 累计返回受影响行数
    - 添加集成测试验证大批量插入（5000+ 条）
    - _Bug 修复：IMPROVED_OPTIMIZATION_PLAN.md 第二章 Bug 2_

- [x] 2. 重构 Redis Pipeline（基于 redis-rs 原生能力）
  - [x] 2.1 创建基于 redis::pipe() 的 RedisPipeline
    - 在 `src/redis/pipeline.rs` 中定义 `RedisPipeline` 结构体
    - 字段：`pipe: redis::Pipeline`, `client: RedisClient`
    - 实现 `new(client)` 构造函数，使用 `redis::pipe()` 创建原生 Pipeline
    - _架构重构：IMPROVED_OPTIMIZATION_PLAN.md 第一章缺陷1_

  - [x] 2.2 实现 Pipeline 命令添加方法
    - 实现 `set()`, `get()`, `del()`, `incr()` 等基础命令
    - 使用 `self.pipe.set()`, `self.pipe.get()` 等原生方法
    - 实现 `hset()`, `hget()`, `lpush()`, `rpush()`, `sadd()`, `zadd()` 等
    - 实现 `cmd()` 方法支持自定义命令：`self.pipe.add_command(cmd)`
    - 所有方法返回 `&mut Self` 支持链式调用
    - _需求：1.2, 1.6_

  - [x] 2.3 实现类型安全的结果提取
    - 实现 `query<T: FromRedisValue>(self) -> Result<Vec<T>>` 方法
    - 使用 `self.pipe.query(&mut conn)` 执行并返回类型化结果
    - 实现 `execute(self) -> Result<Vec<RedisValue>>` 兼容模式
    - 将 `redis::Value` 转换为 `RedisValue`
    - 实现 `len()`, `is_empty()` 辅助方法
    - _需求：1.3, 1.4, 1.8_

  - [x] 2.4 编写 Pipeline 单元测试（必选）
    - 测试基础命令添加和执行
    - 测试链式调用
    - 测试类型化结果提取（`query::<String>()`）
    - 测试错误处理
    - _需求：1.1-1.8, 14.1, 14.4_

  - [x] 2.5 编写 Pipeline 集成测试（必选）
    - 使用 testcontainers 启动 Redis 容器
    - 测试批量 SET/GET 操作（100+ 命令）
    - 测试混合命令类型
    - 验证单次网络往返（性能测试）
    - _需求：1.1-1.8, 14.2, 14.8_

  - [x] 2.6 在 RedisClient 中添加 pipeline() 方法
    - 在 `src/redis/client.rs` 中实现 `pipeline()` 方法
    - 返回 `RedisPipeline::new(self.clone())`
    - 添加中文文档注释和使用示例
    - _需求：1.1, 16.1, 16.2_

- [x] 3. 重构 Redis Transaction（基于 redis-rs 原生能力）
  - [x] 3.1 基于 transaction_async 实现事务
    - 在 `src/redis/client.rs` 中实现 `transaction()` 方法
    - 签名：`pub async fn transaction<F, Fut, T>(&self, watched_keys: &[String], func: F) -> Result<T>`
    - 使用 `redis::transaction_async(&mut conn, watched_keys, |_conn, pipe| func(pipe))` 
    - 自动处理 WATCH/MULTI/EXEC/UNWATCH 流程
    - 自动重试 WATCH 冲突
    - _架构重构：IMPROVED_OPTIMIZATION_PLAN.md 第一章缺陷2_

  - [x] 3.2 实现 TransactionBuilder 辅助类型
    - 创建 `RedisTransactionBuilder` 包装 `redis::Pipeline`
    - 提供 `set()`, `get()`, `incr()`, `decrby()` 等方法
    - 提供 `atomic()` 方法开启 MULTI 模式
    - 提供 `ignore()` 方法忽略特定命令错误
    - _需求：2.2, 2.3_

  - [x] 3.3 编写事务单元测试（必选）
    - 测试基础事务执行
    - 测试 WATCH 键未修改的成功场景
    - 测试 WATCH 键被修改的冲突场景
    - 测试自动重试机制
    - _需求：2.1-2.10, 14.1, 14.4_

  - [x] 3.4 编写事务集成测试（必选）
    - 使用 testcontainers 启动 Redis 容器
    - 测试乐观锁实现（余额扣减示例）
    - 测试并发事务场景
    - 验证原子性
    - _需求：2.1-2.10, 14.2, 14.8_

  - [x] 3.5 添加文档和示例
    - 添加中文文档注释
    - 添加乐观锁使用示例
    - 说明自动重试机制
    - _需求：2.1, 16.1, 16.2_

- [x] 4. 重构 Redis Lua Script（基于 redis-rs Script 类型）
  - [x] 4.1 基于 redis::Script 实现脚本支持
    - 在 `src/redis/client.rs` 中实现 `script(&self, code: &str) -> redis::Script` 方法
    - 直接返回 `redis::Script::new(code)`
    - 实现 `eval_script<T>()` 方法执行脚本
    - 使用 `script.prepare_invoke()` 准备调用
    - 使用 `invocation.key()` 和 `invocation.arg()` 传递参数
    - 使用 `invocation.invoke(&mut conn)` 执行
    - _架构重构：IMPROVED_OPTIMIZATION_PLAN.md 第一章缺陷3_

  - [x] 4.2 编写 Lua 脚本单元测试（必选）
    - 测试简单脚本执行
    - 测试 keys 和 args 参数传递
    - 测试不同返回值类型
    - 测试 EVALSHA 自动回退
    - _需求：8.1-8.10, 14.1, 14.4_

  - [x] 4.3 编写 Lua 脚本集成测试（必选）
    - 使用 testcontainers 启动 Redis 容器
    - 测试复杂脚本（包含多个 Redis 操作）
    - 测试脚本原子性（并发场景）
    - 验证 EVALSHA 缓存和重用
    - _需求：8.1-8.10, 14.2, 14.8_

  - [x] 4.4 添加文档和示例
    - 添加中文文档注释
    - 添加原子操作示例
    - 说明 EVALSHA 自动优化
    - _需求：16.1, 16.2_

- [x] 5. 完善 Redis String 操作
  - [x] 5.1 实现 GETRANGE 和 SETRANGE 命令
    - 在 `src/redis/client.rs` 中实现 `getrange()` 方法
    - 实现 `setrange()` 方法
    - 处理负数索引
    - 添加中文文档注释和使用示例
    - _需求：3.1, 3.2, 3.3, 3.4, 3.10_

  - [x] 5.2 实现 INCRBYFLOAT 和 PSETEX 命令
    - 实现 `incrbyfloat()` 方法
    - 实现 `psetex()` 方法
    - 添加中文文档注释和使用示例
    - _需求：3.5, 3.6, 3.7, 3.8_

  - [x] 5.3 编写 String 操作单元测试（必选）
    - 测试 GETRANGE 正常和负数索引
    - 测试 SETRANGE 边界情况
    - 测试 INCRBYFLOAT 精度
    - 测试 PSETEX 过期时间
    - _需求：3.1-3.10, 14.1, 14.4_

  - [x] 5.4 编写 String 操作集成测试（必选）
    - 使用 testcontainers 启动 Redis 容器
    - 测试 GETRANGE/SETRANGE 组合
    - 测试 PSETEX 实际过期行为
    - _需求：3.1-3.10, 14.2, 14.8_

- [x] 6. 完善 Redis List 操作
  - [x] 6.1 实现 LINSERT 和 LREM 命令
    - 在 `src/redis/client.rs` 中实现 `linsert()` 方法
    - 实现 `lrem()` 方法，支持三种 count 模式
    - 添加中文文档注释和使用示例
    - _需求：4.1, 4.2, 4.3, 4.4_

  - [x] 6.2 实现 RPOPLPUSH 命令
    - 实现 `rpoplpush()` 方法
    - 处理源列表为空的情况
    - 添加中文文档注释和使用示例
    - _需求：4.5, 4.6_

  - [x] 6.3 实现 BLPOP 和 BRPOP 阻塞命令
    - 实现 `blpop()` 方法
    - 实现 `brpop()` 方法
    - 处理超时情况
    - 添加中文文档注释和使用示例
    - _需求：4.7, 4.8, 4.9, 4.10_

  - [x] 6.4 编写 List 操作单元测试（必选）
    - 测试 LINSERT BEFORE/AFTER
    - 测试 LREM 三种 count 模式
    - 测试 RPOPLPUSH 原子性
    - 测试 BLPOP/BRPOP 超时
    - _需求：4.1-4.11, 14.1, 14.4_

  - [x] 6.5 编写 List 操作集成测试（必选）
    - 使用 testcontainers 启动 Redis 容器
    - 测试 BLPOP/BRPOP 阻塞和唤醒
    - 测试 RPOPLPUSH 在队列间移动
    - _需求：4.1-4.11, 14.2, 14.8_

- [x] 7. 检查点 - P0 阶段完成验证
  - 确保所有 P0 测试通过
  - 确保代码通过 clippy 检查
  - 确保代码格式化正确
  - 询问用户是否有问题或需要调整

### P1：MySQL 和 Redis 中优先级功能

- [-] 8. 实现 MySQL 聚合函数扩展
  - [x] 8.1 实现 AVG 聚合函数
    - 在 `src/mysql/query_builder.rs` 中实现 `avg()` 方法
    - 生成 `SELECT AVG(field) FROM table` SQL 语句
    - 返回 `Option<f64>`
    - 处理空结果集和 NULL 值
    - 添加中文文档注释和使用示例
    - _需求：5.1, 5.2, 5.10_

  - [x] 8.2 实现 MIN 和 MAX 聚合函数
    - 实现 `min<T>()` 方法，支持泛型返回类型
    - 实现 `max<T>()` 方法，支持泛型返回类型
    - 处理空结果集
    - 添加中文文档注释和使用示例
    - _需求：5.3, 5.4, 5.5, 5.6, 5.10_

  - [x] 8.3 支持聚合函数与其他子句组合
    - 确保与 WHERE 子句组合
    - 确保与 GROUP BY 子句组合
    - 支持多个聚合函数
    - 添加组合使用示例
    - _需求：5.7, 5.8, 5.9_

  - [ ] 8.4 编写聚合函数单元测试（必选）
    - 测试 AVG 计算正确性
    - 测试 MIN/MAX 不同数据类型
    - 测试空结果集和 NULL 值
    - 测试与 WHERE/GROUP BY 组合
    - _需求：5.1-5.10, 14.1, 14.4_

  - [ ] 8.5 编写聚合函数集成测试（必选）
    - 使用 testcontainers 启动 MySQL 容器
    - 测试 AVG/MIN/MAX 计算结果
    - 测试多个聚合函数组合
    - 测试与 GROUP BY 组合
    - _需求：5.1-5.10, 14.2, 14.8_

- [x] 9. 实现 MySQL HAVING 子句支持
  - [x] 9.1 实现 HAVING 子句基础功能（仅结构化 API）
    - 在 `src/mysql/query_builder.rs` 中添加 `having_clause` 字段
    - 实现 `having_cond()` 方法，接受字段、运算符、值
    - 在 SQL 生成逻辑中添加 HAVING 子句（GROUP BY 之后，ORDER BY 之前）
    - **不实现** `having()` 裸字符串版本（防止 SQL 注入）
    - _需求：6.2, 6.3, 6.5, 6.8；设计优化：IMPROVED_OPTIMIZATION_PLAN.md 第四章优化1_

  - [x] 9.2 实现 HAVING 子句验证和错误处理
    - 验证 HAVING 必须与 GROUP BY 一起使用
    - 支持多个 HAVING 条件的 AND 连接
    - 使用参数化查询防止 SQL 注入
    - _需求：6.4, 6.6, 6.9, 6.10, 13.1_

  - [x] 9.3 支持 HAVING 与其他子句组合
    - 确保与 WHERE 同时使用
    - 确保 SQL 子句顺序正确
    - 添加中文文档注释和示例
    - _需求：6.7, 6.8, 16.1, 16.2_

  - [x] 9.4 编写 HAVING 子句单元测试（必选）
    - 测试基础 HAVING 条件
    - 测试与聚合函数组合
    - 测试没有 GROUP BY 时的错误
    - 测试多个 HAVING 条件
    - _需求：6.1-6.10, 14.1, 14.4_

  - [ ] 9.5 编写 HAVING 子句集成测试（必选）
    - 使用 testcontainers 启动 MySQL 容器
    - 测试 HAVING 过滤分组结果
    - 测试 WHERE + GROUP BY + HAVING 组合
    - _需求：6.1-6.10, 14.2, 14.8_

- [x] 10. 实现 MySQL 批量 UPDATE 操作
  - [x] 10.1 实现批量更新基础框架
    - 在 `src/mysql/query_builder.rs` 中实现 `update_batch()` 方法
    - 接受记录列表和 where_field 参数
    - 解析记录列表，提取字段名和值
    - 生成 CASE WHEN 批量更新 SQL
    - _需求：7.1, 7.2, 7.3, 7.4_

  - [x] 10.2 实现批量更新 SQL 生成逻辑
    - 为每个字段生成 CASE WHEN 语句
    - 生成 WHERE IN 子句
    - 使用参数化查询
    - _需求：7.3, 7.5, 7.8_

  - [x] 10.3 实现批量更新分批处理
    - 实现分批逻辑（默认每批 1000 条）
    - 循环处理每批记录
    - 累计受影响行数
    - 添加错误处理
    - _需求：7.6, 7.7, 7.9_

  - [x] 10.4 优化批量更新性能
    - 使用事务包装
    - 添加中文文档注释和示例
    - 说明性能优势
    - _需求：7.10, 12.2, 12.3, 16.1, 16.2_

  - [x] 10.5 编写批量更新单元测试（必选）
    - 测试小批量更新（< 10 条）
    - 测试中等批量更新（100 条）
    - 测试大批量更新（1000+ 条）
    - 测试 SQL 注入防护
    - _需求：7.1-7.10, 14.1, 14.4_

  - [ ] 10.6 编写批量更新集成测试（必选）
    - 使用 testcontainers 启动 MySQL 容器
    - 执行批量更新
    - 验证更新结果正确性
    - 测量性能，与逐条更新对比
    - _需求：7.1-7.10, 14.2, 14.8_

- [x] 11. 实现 MySQL UPSERT 操作（遗漏功能补充）
  - [x] 11.1 实现 UPSERT 方法
    - 在 `src/mysql/query_builder.rs` 中实现 `upsert<T>()` 方法
    - 生成 `INSERT INTO ... ON DUPLICATE KEY UPDATE ...` SQL
    - 自动提取所有字段用于 UPDATE 子句
    - 使用参数化查询
    - 添加中文文档注释和示例
    - _遗漏功能：IMPROVED_OPTIMIZATION_PLAN.md 第三章遗漏1_

  - [x] 11.2 编写 UPSERT 单元测试（必选）
    - 测试插入新记录
    - 测试更新已存在记录
    - 测试幂等性
    - _需求：14.1, 14.4_

  - [ ] 11.3 编写 UPSERT 集成测试（必选）
    - 使用 testcontainers 启动 MySQL 容器
    - 测试实际 UPSERT 行为
    - 验证主键冲突时的更新
    - _需求：14.2, 14.8_

- [x] 12. 实现 MySQL IS NULL / IS NOT NULL 条件（遗漏功能补充）
  - [x] 12.1 实现 NULL 条件方法
    - 在 `src/mysql/query_builder.rs` 中实现 `where_null()` 方法
    - 实现 `where_not_null()` 方法
    - 生成 `field IS NULL` 和 `field IS NOT NULL` SQL
    - 添加中文文档注释和示例
    - _遗漏功能：IMPROVED_OPTIMIZATION_PLAN.md 第三章遗漏5_

  - [x] 12.2 编写 NULL 条件单元测试（必选）
    - 测试 IS NULL 条件
    - 测试 IS NOT NULL 条件
    - 测试与其他条件组合
    - _需求：14.1, 14.4_

  - [ ] 12.3 编写 NULL 条件集成测试（必选）
    - 使用 testcontainers 启动 MySQL 容器
    - 测试实际 NULL 值过滤
    - _需求：14.2, 14.8_

- [x] 13. 检查点 - MySQL 功能完成验证
  - 确保所有 MySQL 测试通过
  - 确保代码通过 clippy 检查
  - 确保代码格式化正确
  - 询问用户是否有问题或需要调整

### P1：Redis 中优先级功能（续）

- [x] 14. 完善 Redis Set 操作
  - [x] 14.1 实现 Set 集合运算命令
    - 在 `src/redis/client.rs` 中实现 `sinter()` 方法（交集）
    - 实现 `sunion()` 方法（并集）
    - 实现 `sdiff()` 方法（差集）
    - 支持多个集合参数
    - 添加中文文档注释和使用示例
    - _需求：9.1, 9.2, 9.3, 9.4, 9.5, 9.6_

  - [x] 14.2 实现 SMOVE 和 SSCAN 命令
    - 实现 `smove()` 方法，原子性地移动成员
    - 实现 `sscan()` 方法，支持游标迭代
    - 支持 pattern 和 count 参数
    - 添加中文文档注释和使用示例
    - _需求：9.7, 9.8, 9.9, 9.10_

  - [x] 14.3 编写 Set 操作单元测试（必选）
    - 测试 SINTER/SUNION/SDIFF 基础功能
    - 测试多个集合的运算
    - 测试空集合和不存在集合
    - _需求：9.1-9.11, 14.1, 14.4_

  - [ ] 14.4 编写 Set 操作集成测试（必选）
    - 使用 testcontainers 启动 Redis 容器
    - 测试集合运算的数学属性（交换律、结合律）
    - 测试 SMOVE 原子性
    - 测试 SSCAN 大数据集迭代
    - _需求：9.1-9.11, 14.2, 14.8_

- [x] 15. 完善 Redis Sorted Set 操作
  - [x] 15.1 实现排名查询命令
    - 在 `src/redis/client.rs` 中实现 `zrank()` 方法（升序排名）
    - 实现 `zrevrank()` 方法（降序排名）
    - 处理成员不存在的情况（返回 None）
    - 添加中文文档注释和使用示例
    - _需求：10.1, 10.2, 10.3, 10.4_

  - [x] 15.2 实现逆序范围查询（含分数）
    - 实现 `zrevrange()` 方法，支持 with_scores 参数
    - 实现 `zrange_with_scores()` 方法（带分数的升序查询）
    - 实现 `zrevrange_with_scores()` 方法（带分数的降序查询）
    - 返回 `Vec<(String, f64)>`（成员名, 分数）
    - _需求：10.5, 10.6；遗漏功能：IMPROVED_OPTIMIZATION_PLAN.md 第三章遗漏2_

  - [x] 15.3 实现范围删除命令
    - 实现 `zremrangebyrank()` 方法，按排名范围删除
    - 实现 `zremrangebyscore()` 方法，按分数范围删除
    - 返回删除的成员数量
    - 添加中文文档注释和使用示例
    - _需求：10.7, 10.8_

  - [x] 15.4 实现 ZSCAN 迭代命令
    - 实现 `zscan()` 方法，支持游标迭代
    - 返回游标、成员和分数列表
    - 支持 pattern 和 count 参数
    - _需求：10.9, 10.10_

  - [x] 15.5 编写 Sorted Set 操作单元测试（必选）
    - 测试 ZRANK/ZREVRANK 排名查询
    - 测试 ZREVRANGE 逆序查询
    - 测试 zrange_with_scores / zrevrange_with_scores
    - 测试 ZREMRANGEBYRANK/ZREMRANGEBYSCORE
    - _需求：10.1-10.11, 14.1, 14.4_

  - [ ] 15.6 编写 Sorted Set 操作集成测试（必选）
    - 使用 testcontainers 启动 Redis 容器
    - 测试排名查询的正确性
    - 测试排序不变性（分数从小到大）
    - 测试范围删除的准确性
    - _需求：10.1-10.11, 14.2, 14.8_

- [x] 16. 实现 Redis SCAN 迭代器支持
  - [x] 16.1 实现 SCAN 命令
    - 在 `src/redis/client.rs` 中实现 `scan()` 方法
    - 接受 cursor、pattern、count 参数
    - 返回新游标和键列表
    - 处理游标为 0 的情况（迭代完成）
    - _需求：11.1, 11.2, 11.3_

  - [ ] 16.2 实现便捷的迭代器 API
    - 在 `src/redis/` 目录下创建 `scan.rs` 文件
    - 定义 `RedisScanIterator` 结构体，封装游标管理
    - 实现 async 迭代器模式（`async_stream` 或手动 Stream）
    - 支持 pattern 和 count 参数
    - 在 `RedisClient` 中添加 `scan_iter()` 方法
    - _需求：11.4, 11.5, 11.10_

  - [x] 16.3 添加 SCAN 文档和示例
    - 添加中文文档注释
    - 添加使用示例（基础迭代和 pattern 过滤）
    - 说明 SCAN 的非阻塞特性和数据变化影响
    - _需求：11.6, 11.9, 16.1, 16.2_

  - [x] 16.4 编写 SCAN 单元测试（必选）
    - 测试基础 SCAN 迭代
    - 测试 pattern 过滤
    - 测试 count 参数
    - 测试游标管理
    - _需求：11.1-11.10, 14.1, 14.4_

  - [ ] 16.5 编写 SCAN 集成测试（必选）
    - 使用 testcontainers 启动 Redis 容器
    - 插入大量键（1000+）
    - 测试完整迭代（所有键至少返回一次）
    - 测试迭代器 API 便捷性
    - _需求：11.1-11.10, 14.2, 14.8_

- [x] 17. 实现 Redis Pub/Sub + 连接池健康检查
  - [x] 17.1 实现 Pub/Sub 发布功能
    - 在 `src/redis/client.rs` 中实现 `publish()` 方法
    - 返回接收到消息的订阅者数量
    - 添加中文文档注释和使用示例
    - _遗漏功能：IMPROVED_OPTIMIZATION_PLAN.md_

  - [x] 17.2 实现连接池健康检查
    - 实现 `health_check()` 方法
    - 使用 PING 命令 + 连接池状态检查
    - 实现 `pool_status()` 方法暴露 deadpool 状态
    - 添加中文文档注释
    - _遗漏功能：IMPROVED_OPTIMIZATION_PLAN.md 第三章遗漏3_

  - [x] 17.3 编写 Pub/Sub 和健康检查单元测试（必选）
    - 测试 PUBLISH 基本功能
    - 测试 health_check 返回值
    - 测试 pool_status 信息
    - _需求：14.1, 14.4_

  - [ ] 17.4 编写 Pub/Sub 集成测试（必选）
    - 使用 testcontainers 启动 Redis 容器
    - 测试发布/订阅基本流程
    - 测试多频道订阅
    - _需求：14.2, 14.8_

- [x] 18. 检查点 - Redis P1 功能完成验证
  - 确保所有 Redis P1 测试通过
  - 确保代码通过 clippy 检查
  - 确保代码格式化正确
  - 询问用户是否有问题或需要调整

### 最终整合

- [ ] 19. 完善 lib.rs 导出
  - 在 `src/lib.rs` 中导出 `RedisPipeline` 和 `RedisTransaction`
  - 确保所有新类型的导出路径正确
  - _需求：15.1, 15.2_

- [ ] 20. 最终验证
  - 运行所有单元测试：`cargo test --lib`
  - 运行所有集成测试：`cargo test --test '*'`
  - 运行 `cargo clippy --all-targets -- -D warnings`
  - 运行 `cargo fmt --check`
  - 检查向后兼容性：现有 API 签名不变
  - _需求：14.1-14.10, 15.1-15.8, 17.2, 17.3_

