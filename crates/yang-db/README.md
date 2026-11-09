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
yang-db = { version = "0.2.1", default-features = false, features = ["mysql"] }

# 仅 PostgreSQL
yang-db = { version = "0.2.1", default-features = false, features = ["postgres"] }

# 仅 Redis
yang-db = { version = "0.2.1", default-features = false, features = ["redis"] }
```

无 feature 组合仍提供 `DbError`、`DbErrorCategory`、`IsolationLevel` 与 `PoolStatus` 等后端无关契约。docs.rs 使用 all-features 构建完整 API。

## 支持矩阵与 non-goal

当前正式支持的后端如下：

| 后端 | 状态 | 主要范围 |
|---|---|---|
| MySQL 8 | 支持 | 参数化 CRUD、事务、子查询、UNION、行锁、原子更新 |
| PostgreSQL 16 / 17 | 支持 | 与 MySQL 对称的查询/事务 API，使用 `$N` 占位符与 `RETURNING`；PG17 由独立 CI job 验证 |
| Redis 7 | 支持 | 连接池、常用数据结构、Pipeline、WATCH 事务与 Lua |

- SQLite 与 MSSQL 是当前 non-goal，不提供 feature、驱动或兼容承诺。出现真实消费者后必须通过独立 RFC 评估驱动、类型映射、DDL、事务和 CI 成本，不能只复制方法清单。
- `backup`、`restore`、`database-create` 等数据库运维能力不进入 `QueryBuilder`。这类操作优先使用数据库原生工具，并由部署/运维层管理权限、审计、加密与生命周期。
- 逻辑复制 / 复制槽（publication、subscription、`pg_replication_slots`）：属集群/DBA 运维，由数据库原生复制工具管理。
- FDW / 外部表（`postgres_fdw`、`file_fdw`、`IMPORT FOREIGN SCHEMA`）：跨库联邦与服务端文件访问的权限模型超出运行时查询构建器边界。
- 迁移 CLI / checksum runner / 自动 diff：由 yang-base 的加法 schema 同步子集或外部迁移工具承担，不新增独立迁移命令。
- 多主机 / 故障转移（`target_session_attrs` 等运行时选主）：由部署层连接串、代理或服务发现处理。
- PgBouncer / 事务池模式：不承诺 prepared statement 与事务池的兼容组合；部署层需选择会话池或关闭服务端预编译。
- 原生 SQL 入口仅作为明确标注的逃生舱；面向外部输入的查询应使用 checked identifier API 和绑定参数。

### PostgreSQL 能力与限制

PostgreSQL 当前提供的能力（`POSTGRES_CAPABILITIES` 声明的通用能力，以及通过专用 API
提供、由集成测试覆盖的进阶能力）：

- 参数化 CRUD、聚合、JOIN、批量插入/更新、条件删除。
- `ON CONFLICT` upsert：单行与批量均支持显式冲突目标，批量使用 `EXCLUDED`。
- `RETURNING`：插入、更新、删除可返回受影响行。
- 事务与保存点：`SAVEPOINT` / `ROLLBACK TO SAVEPOINT`、隔离级别、advisory lock。
- `WITH` / `WITH RECURSIVE`、窗口函数、`FOR UPDATE SKIP LOCKED`。
- schema 限定查询与 `search_path` 处理。
- 类型映射：`UUID`、`NUMERIC`、`TIMESTAMPTZ`、`JSON` / `JSONB`、数组、`INET` / `CIDR`、`BYTEA`。
- 进阶查询：range / multirange（`FieldType::Range` / `Multirange`，
  `where_range_contains` / `where_range_overlaps` 仅支持整数范围；其他范围类型
  用原生 SQL 显式转换）、jsonpath
  （`where_json_path_exists` / `where_json_path_match`）、全文检索
  （`where_full_text` / `where_full_text_query`，固定 `simple` 配置）。
- 事务进阶：`Transaction::set_local`（事务级 `SET LOCAL`，回滚后自动失效）、
  `Transaction::lock_table`（受控 `TableLockMode`）、两阶段提交
  `Database::{prepare_transaction, commit_prepared, rollback_prepared}`
  （需服务端 `max_prepared_transactions > 0`）。
- 可观测与内省：`Database::explain`（`EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)`）、
  `server_version_num` / `supports`（版本门控）、`active_queries` / `lock_waits`。

明确限制（不声明即不保证）：

- 上述 Phase C 能力通过专用 API 提供，**没有新增 `BackendCapability` 枚举变体**；
  `Database::supports(min_version)` 只用于服务端版本门控，不代表
  `POSTGRES_CAPABILITIES.supports()` 会返回 `true`。
- PostgreSQL 已封装 `COPY`（`copy_from_csv` / `copy_to_csv`）与 `LISTEN`/`NOTIFY`
  （`PgNotificationListener` / `Database::notify`），并在 `POSTGRES_CAPABILITIES` 中声明。
- MySQL 的能力契约不声明 `RETURNING`、显式冲突目标、保存点、CTE 与窗口函数；MySQL 8 自身的
  SQL 支持不代表 yang-db 已暴露对应 API。
- base 层未封装的进阶能力（CTE/窗口/COPY/LISTEN/range/jsonpath/FTS/EXPLAIN/内省/2PC/表锁）
  应直接使用 yang-db 的 PostgreSQL API。
- 通用方言能力用 `BackendCapabilities::supports()` 做运行时分支；Phase C 专用 API 用
  `Database::supports(min_version)` 做服务端版本门控。不要按数据库版本文档假定能力。

PostgreSQL 集成测试均为 `#[ignore]`，需要真实实例：

```bash
PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
  cargo test -p yang-db --no-default-features --features postgres \
  --test integration_pg_crud_simple -- --ignored --test-threads=1
```

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
