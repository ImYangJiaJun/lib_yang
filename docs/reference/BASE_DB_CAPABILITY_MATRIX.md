# yang-base / yang-db 支持能力矩阵

**基线日期**：2026-11-09（对照 commit bfa1563）

**开发版本**：yang-base 0.4.0 / yang-db 0.2.1

本矩阵是公开能力边界的简表；精确签名以 rustdoc 和源码为准，逐点验收证据见 `docs/plans/YANG_BASE_DB_COMPLETENESS_PLAN.md` 与 `docs/audit/PRODUCTION_READINESS_LOG.md`。

## 支持矩阵

| 范围 | 当前能力 | 约束与验证 |
|---|---|---|
| Table / schema | `Table` + `Field` 构建不可变 `TableDefinition`；字段校验、权限、索引、关系、JSON Schema、`Record` 动态行 | definition 单测、schema 快照、真实 MySQL schema 同步 |
| Action / Router | `TypedHandler` 类型化输入输出；`Api` 原子注册；`.table(...).crud()` 标准接口；transport-neutral `RequestMeta`；确定性 `ApiCatalog`；可选 OpenAPI 3.1 | 路由纵向契约、catalog/OpenAPI 测试、CRUD 集成测试 |
| 数据治理 | checksum migration、plan/dry-run、并发占位；`SchemaValidationReport` 只读验证；从 `AppRouter` 汇总 `TableDefinition` 做 additive schema 同步 | MySQL 8 双实例并发、幂等补列、危险漂移拒绝 |
| 插件 | 依赖拓扑、生命周期、配置 schema、迁移声明 | 循环/缺失依赖、配置反例与迁移漂移测试 |
| MySQL 8 | 参数化 CRUD、聚合、JOIN、受控子查询、复合查询、事务行锁、原子更新 | 单元/属性测试与真实数据库集成 |
| PostgreSQL 16 / 17 | 与 MySQL 对齐（非逐项对称）的单表查询/事务/CRUD 能力，使用 `$N` 和 `RETURNING`；CTE、窗口、保存点、批量 upsert、MERGE 已实现；PG17 由独立 CI job 验证 | SQL 形态对抗测试与真实数据库集成；base 接入范围与不对称项见下方「PostgreSQL 能力与 base 接入状态」「已知限制」 |
| Redis 7 | 连接池、String/Hash/List/Set/ZSet、Pipeline、WATCH 事务、Lua | feature 隔离、错误分类与真实 Redis 集成 |
| 运维可见性 | 错误分类、request id、慢查询 tracing、敏感字段脱敏、可选 metrics | 日志捕获、故障注入和 Debug 泄露反例 |
| 后台扩展 | `admin-metadata` 通过稳定 ID 引用 Action/Table/API operation | 零新增依赖，不持有或改变核心 dispatch |

## PostgreSQL 能力与 base 接入状态

“base 接入状态”指 yang-base 是否已提供对应封装；“yang-db 支持”来自
`POSTGRES_CAPABILITIES`，两者不能互相替代。未接入的能力仍可直接使用 yang-db 的
PostgreSQL API，但不能假定 `TableQuery` / 内置 CRUD 会按 PG 语义执行。

| 能力 | yang-db 支持 | base 接入状态 | 说明 |
|---|---:|---|---|
| 连接池与生命周期 | 是 | 已接入 | `AnyDatabase::postgres`；`Tools` 统一数据库槽与 `pool_status` |
| `Record` 动态行解码 | 是 | 已接入 | `integration_record_pg` 覆盖 PG 类型族 |
| 声明式 schema 同步 | 是 | 已接入 | 单事务 DDL + `pg_advisory_lock`；枚举字段 fail-closed |
| 参数化 CRUD / 聚合 / JOIN | 是 | 已接入 | `TableQuery` 的 MySQL/PG 执行分支 |
| `TableQuery` / 内置 CRUD | 是 | 已接入 | SELECT/分页/COUNT/软删/租户/权限/写入均有 PG 分支 |
| 事务 / 保存点 | 是 | 部分接入 | base 提供 `AnyTransaction`；保存点直接用 yang-db |
| `RETURNING`（INSERT） | 是 | 已接入 | `TableQuery::insert_returning(ReturningTarget::{Pk, Columns, All})`；UUID/text/numeric 主键可回填；MySQL 仅整型主键 |
| `ORDER BY ... NULLS FIRST/LAST` | 是 | 已接入 | `TableQuery::order_nulls`；PostgreSQL 渲染原生 NULLS，MySQL 用 `field IS NULL` 模拟 |
| update/delete/upsert `RETURNING` | 是 | 未接入 | 用 yang-db 的 `update_returning` / `delete_returning` / `upsert_returning` |
| `ON CONFLICT` / 批量 upsert | 是 | 已接入 | `TableQuery::{upsert_batch, upsert_batch_do_nothing}` 及 `_in_tx` 变体；冲突列必须显式传入 |
| 事务局部配置（`SET LOCAL`） | 是 | 已接入 | `ActionContext::{set_local, set_tenant_local}`；仅 PostgreSQL，MySQL 明确返回配置错误 |
| CTE / 窗口函数 / `SKIP LOCKED` | 是 | 未封装 | 可直接使用 yang-db QueryBuilder |
| `COPY ... FROM STDIN` | 是 | 未封装 | yang-db 提供 `copy_from_csv` / `copy_to_csv` |
| `LISTEN` / `NOTIFY` | 是 | 未封装 | yang-db 提供 `PgNotificationListener` / `notify` |
| range / multirange / jsonpath / 全文检索 | 是 | 未封装 | 用 yang-db 的 `FieldType::Range` / `where_range_*`、`where_json_path_*`、`where_full_text*` |
| `EXPLAIN` / `pg_locks` / `pg_stat_activity` 内省 | 是 | 未封装 | 用 yang-db 的 `Database::explain` / `lock_waits` / `active_queries` |
| 两阶段提交 / 表级锁 | 是 | 未封装 | 用 yang-db 的 `prepare_transaction` / `commit_prepared` / `rollback_prepared`、`Transaction::lock_table` |

## 已知限制

以下限制是**当前实现的有意边界**，不是待办清单；涉及安全边界的项一律 fail-closed。

| 限制 | 影响范围 | 当前行为 | 替代路径 |
|---|---|---|---|
| 部分索引 / 表达式索引不支持 DSL 声明 | `schema_sync`、`Table`/`Field` 索引 DSL | PostgreSQL 侧完整反射 `pg_index.indpred` 与 `pg_get_indexdef` 表达式键并保留；但 DSL 只能声明普通列索引，声明比较与反射结果永不相等，同名漂移会 fail-closed 拒绝 | 用 yang-db 的 `execute_script`/raw DDL 或外部迁移工具单独管理 |
| migration runner 未提供 | `yang-db`、`yang-base` | `MigrationConfig` 仅为占位；`DatabaseInitializer` 只做加法 schema 同步，不维护版本表/checksum | 外部迁移工具（如 sqlx migrate、refinery）或人工 SQL |
| 租户守卫未绑定租户时 fail-closed | 关联预加载（`RelationLoader`） | 目标表声明 `tenant_key` 但执行器未绑定租户值时返回 `BaseError::ConfigError`，不返回任何行 | 确需系统级跨租户读取时必须显式调用 `RelationLoader::without_tenant_scope` |
| MySQL 不支持 PG 专属语义 | `yang-db` / `TableQuery` 的 PG-only 入口 | MySQL 上调用 PG 专属行锁、`ON CONFLICT` 冲突谓词、range/数组绑定等入口时返回明确的 `BaseError::ConfigError`，不静默降级 | 按后端能力分支调用；或改用 PostgreSQL 后端 |

## schema-first 边界

- 应用表通过 `Table::build()` 进入 `TableDefinition`；失败时不会得到部分可用定义。
- `ModuleRouter::table` 绑定运行期 CRUD 主表，`ModuleRouter::schema` 声明只参与启动同步的附属表。
- 内置 CRUD 使用非泛型 handler 与 `Record`，主键和 schema 在运行期从绑定定义读取。
- 自定义 Action 与 HTTP 元数据通过 `Api` 同时注册，避免 handler、route 与 operation id 漂移。
- `AppRouter::catalog()` 与 `AppRouter::table_definitions()` 分别汇总 API 和 schema 两条确定性只读视图。
- schema 同步只覆盖库支持的表、列、主键和索引子集，不是任意 DDL 或破坏性 migration 引擎。

## 与 br-addon / br-db 的设计差异

`br-addon` 与 `br-db` 仅作为能力盘点参考，不是要复制的兼容目标：

- 本项目按真实消费者与已验证风险决定 API，不以另一个库的方法数量作为完成度。
- 数据入口默认走 checked identifier、绑定参数和受控 SQL 构造；RAW/native SQL 只作为显式逃生舱，不接受不可信输入。
- schema-first 定义是应用层当前结构的事实源，但插件 migration 继续承载历史演进和超出内置同步子集的变更。
- Action 保持 transport-neutral，后台展示元数据通过可选 feature 引用稳定 ID，不侵入 dispatch。
- 方言能力由 `BackendCapabilities` 明示；MySQL/PostgreSQL API 对称不等于 SQL 语义无差异。

## 明确 non-goal

- SQLite、MSSQL：当前没有 feature、驱动或兼容承诺；只有出现真实消费者并完成独立 RFC 后才评估。
- backup、restore、database-create：属于部署/运维层，优先使用数据库原生工具处理权限、审计、加密和生命周期。
- 通用 ORM、任意 SQL AST、破坏性或任意 schema migration：不在当前职责内。
- `br-addon` / `br-db` 的逐方法兼容与 drop-in replacement：不提供承诺。
- 逻辑复制 / 复制槽（publication、subscription、`pg_replication_slots`）：属集群/DBA 运维，
  由数据库原生复制工具管理；不进入运行时 QueryBuilder。
- FDW / 外部表（`postgres_fdw`、`file_fdw`、`IMPORT FOREIGN SCHEMA`）：跨库联邦与服务端文件
  访问的权限模型超出基础库边界。
- 迁移 CLI / checksum runner / 自动 diff：由 yang-base 的加法 schema 同步子集或外部迁移工具承担，
  不新增独立迁移命令。
- 多主机 / 故障转移（`target_session_attrs` 等运行时选主）：由部署层连接串、代理或服务发现处理。
- PgBouncer / 事务池模式：不承诺 prepared statement 与事务池的兼容组合；部署层需选择会话池或
  关闭服务端预编译。
