# Changelog

## 0.3.1

### 修复

- PostgreSQL 参数绑定缓存改为按「SQL 文本 + 参数类型组合」分桶：未类型化 NULL
  使用 OID 0，并生成 128 位绑定类型签名附加到 SQL 注释中，避免首次 PARSE 定型后
  出现 22P03 / 08P01 / 22021，以及 FLOAT8 按 INT8 解码导致的静默错值。
- 事务与 QueryBuilder 的 NULL 从 TEXT / INT4 改为 OID 0，支持
  `::numeric` / `::uuid` / `::timestamptz` 等任意显式 cast。
- 新增单连接池回归 `integration_pg_bind_signature`：db NULL→TEXT、tx NULL→INT、
  QueryBuilder INT→FLOAT。

### 兼容新增

- MySQL / PostgreSQL 的 `Database` 实现 `Clone`，用于多数据源共享同一连接池。
- SQL 前会附加 `/*yang-bind:...*/` 注释参与 sqlx 语句缓存键；断言精确 SQL 文本的
  下游测试需同步。`postgres::params` 为 crate 内部模块，不新增公共 API。

## 0.3.0

### 破坏性变更

- `postgres::query_builder::WindowFunction` 移除 `PartialEq` / `Eq` 派生
  （仅保留 `Debug` / `Clone`）。需要比较窗口函数表达式的下游代码请改为比较
  渲染后的 SQL 或自行提供比较逻辑。
- `postgres::query_builder::FrameMode` 新增 `Groups` 变体（`GROUPS` 窗口帧），
  且未标注 `#[non_exhaustive]`。下游对 `FrameMode` 的穷举 `match` 必须补上
  `FrameMode::Groups` 分支才能通过编译。
- `table::DatabaseRelationExecutor::new` 签名由 `new(database)` 改为
  `new(database, roles)`：数据库句柄改为跨后端 `AnyDatabase`，并新增 `roles`
  参数用于字段读权限裁剪；同时新增 `target` / `with_tenant` /
  `without_tenant_scope` 守卫入口，未登记目标表定义或未绑定租户值时
  fail-closed。
- `database::SchemaSyncReport`（已标注 `#[non_exhaustive]`）新增
  `statements` 与 `deferred_statements` 字段：分别携带事务内 DDL 与事务提交后
  执行的 deferred DDL（`VALIDATE CONSTRAINT`、并发索引等），供 dry-run 审计。
  使用结构体字面量构造下游需改用 `Default` + 字段赋值。

### 兼容增强

- 新增 `RedisClient::set_with_options_outcome` 与 `SetOutcome`：区分 `SET NX/XX`
  条件写是否真正生效；`set_with_options` 的既有返回契约保持不变。
- PostgreSQL `DISCARD ALL` 后同步清理 sqlx 客户端语句缓存，避免连接因失效缓存
  被 hard-close，池不再退化为每次借出都新建连接。
- `insert()` 在解码阶段拒绝负值主键；Pool 路径走既有回滚分支，不再出现
  「返回 Err 但行已提交」。
- 事务与查询日志改为只记录参数数量，不记录参数值。

### 限制

- 部分索引与表达式索引目前仅由 schema 同步反射后保留（不会误删），暂不支持
  通过表定义 DSL 声明式创建；需要时请在外部直接执行 DDL。
