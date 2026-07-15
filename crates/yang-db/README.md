# yang-db

YANG 数据库操作库，提供类型安全的 MySQL/PostgreSQL 查询构建器与 Redis 客户端。

## 功能特性

- 链式调用 API，提供流畅的开发体验
- 基于 sqlx 的异步数据库操作
- 类型安全，编译时捕获错误
- 防止 SQL 注入，使用参数化查询
- 支持事务管理
- 完善的错误处理和中文错误消息

## Feature 选择

默认启用 `mysql`、`postgres`、`redis`。只使用单一后端时应关闭默认 feature，避免编译和链接无关驱动：

```toml
# 仅 MySQL
yang-db = { version = "0.1.4", default-features = false, features = ["mysql"] }

# 仅 PostgreSQL
yang-db = { version = "0.1.4", default-features = false, features = ["postgres"] }

# 仅 Redis
yang-db = { version = "0.1.4", default-features = false, features = ["redis"] }
```

无 feature 组合仍提供 `DbError`、`DbErrorCategory`、`IsolationLevel` 与 `PoolStatus` 等后端无关契约。docs.rs 使用 all-features 构建完整 API。

## 支持矩阵与 non-goal

当前正式支持的后端如下：

| 后端 | 状态 | 主要范围 |
|---|---|---|
| MySQL 8 | 支持 | 参数化 CRUD、事务、子查询、UNION、行锁、原子更新 |
| PostgreSQL 16 | 支持 | 与 MySQL 对称的查询/事务 API，使用 `$N` 占位符与 `RETURNING` |
| Redis 7 | 支持 | 连接池、常用数据结构、Pipeline、WATCH 事务与 Lua |

- SQLite 与 MSSQL 是当前 non-goal，不提供 feature、驱动或兼容承诺。出现真实消费者后必须通过独立 RFC 评估驱动、类型映射、DDL、事务和 CI 成本，不能只复制方法清单。
- `backup`、`restore`、`database-create` 等数据库运维能力不进入 `QueryBuilder`。这类操作优先使用数据库原生工具，并由部署/运维层管理权限、审计、加密与生命周期。
- 原生 SQL 入口仅作为明确标注的逃生舱；面向外部输入的查询应使用 checked identifier API 和绑定参数。

## 依赖

- `sqlx`: 可选异步 SQL 工具包，按 feature 启用 MySQL/PostgreSQL
- `redis` / `deadpool-redis`: 仅 `redis` feature 启用
- `tokio`: 异步运行时
- `serde`: 序列化/反序列化
- `thiserror`: 错误处理
- `chrono`: 日期时间处理
- `log`: 日志记录

## 模块结构

- `database`: 数据库连接管理
- `query_builder`: 查询构建器
- `condition`: 查询条件和 SQL 值类型
- `field`: 字段类型和 JOIN/ORDER 子句
- `transaction`: 事务管理
- `error`: 错误类型定义
- `init`: 数据库初始化

## 核心类型

### DbError

统一的错误类型，包含：
- `ConnectionError`: 连接错误
- `QueryError`: 查询错误
- `SqlSyntaxError`: SQL 语法错误
- `ConstraintError`: 约束错误
- `TypeConversionError`: 类型转换错误
- `SerializationError`: 序列化错误
- `DeserializationError`: 反序列化错误
- `TransactionError`: 事务错误
- `TableNotFound`: 表不存在
- `MissingWhereClause`: 缺少 WHERE 条件
- `Unknown`: 未知错误

### SqlValue

SQL 值类型枚举，支持：
- `Null`: NULL 值
- `Bool`: 布尔值
- `Int`: 整数（i64）
- `Float`: 浮点数（f64）
- `String`: 字符串
- `Bytes`: 字节数组
- `Json`: JSON 值
- `DateTime`: 日期时间
- `Timestamp`: 时间戳

### FieldType

字段类型标记：
- `Standard`: 标准类型
- `Json`: JSON 类型
- `DateTime`: DATETIME 类型
- `Timestamp`: TIMESTAMP 类型
- `Decimal`: DECIMAL 类型
- `Blob`: BLOB 类型
- `Text`: TEXT 类型

## 开发状态

核心查询、CRUD、事务、JOIN、聚合与三后端 feature 隔离均已实现。当前能力边界以本 README、公开 API 文档和 `BackendCapabilities` 机读契约为准；新增方言或运维职责必须先更新这些契约。

## 测试

运行测试：

```bash
cargo test --lib
```

## 代码质量

```bash
# 检查代码
cargo check

# 代码格式化
cargo fmt

# 代码检查
cargo clippy --all-targets -- -D warnings
```
