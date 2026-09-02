# yang-base / yang-db 支持能力矩阵

**基线日期**：2026-07-16

**开发版本**：yang-base 0.2.1 / yang-db 0.1.5

本矩阵是公开能力边界的简表；精确签名以 rustdoc 和源码为准，逐点验收证据见 `YANG_BASE_DB_COMPLETENESS_PLAN.md` 与 `PRODUCTION_READINESS_LOG.md`。

## 支持矩阵

| 范围 | 当前能力 | 约束与验证 |
|---|---|---|
| Table / schema | `Table` + `Field` 构建不可变 `TableDefinition`；字段校验、权限、索引、关系、JSON Schema、`Record` 动态行 | definition 单测、schema 快照、真实 MySQL schema 同步 |
| Action / Router | `TypedHandler` 类型化输入输出；`Api` 原子注册；`.table(...).crud()` 标准接口；transport-neutral `RequestMeta`；确定性 `ApiCatalog`；可选 OpenAPI 3.1 | 路由纵向契约、catalog/OpenAPI 测试、CRUD 集成测试 |
| 数据治理 | checksum migration、plan/dry-run、并发占位；`SchemaValidationReport` 只读验证；从 `AppRouter` 汇总 `TableDefinition` 做 additive schema 同步 | MySQL 8 双实例并发、幂等补列、危险漂移拒绝 |
| 插件 | 依赖拓扑、生命周期、配置 schema、迁移声明 | 循环/缺失依赖、配置反例与迁移漂移测试 |
| MySQL 8 | 参数化 CRUD、聚合、JOIN、受控子查询、复合查询、事务行锁、原子更新 | 单元/属性测试与真实数据库集成 |
| PostgreSQL 16 | 与 MySQL 对称的查询/事务能力，使用 `$N` 和 `RETURNING` | SQL 形态对抗测试与真实数据库集成 |
| Redis 7 | 连接池、String/Hash/List/Set/ZSet、Pipeline、WATCH 事务、Lua | feature 隔离、错误分类与真实 Redis 集成 |
| 运维可见性 | 错误分类、request id、慢查询 tracing、敏感字段脱敏、可选 metrics | 日志捕获、故障注入和 Debug 泄露反例 |
| 后台扩展 | `admin-metadata` 通过稳定 ID 引用 Action/Table/API operation | 零新增依赖，不持有或改变核心 dispatch |

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
