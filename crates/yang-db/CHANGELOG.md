# Changelog

## Unreleased

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

### 限制

- 部分索引与表达式索引目前仅由 schema 同步反射后保留（不会误删），暂不支持
  通过表定义 DSL 声明式创建；需要时请在外部直接执行 DDL。
