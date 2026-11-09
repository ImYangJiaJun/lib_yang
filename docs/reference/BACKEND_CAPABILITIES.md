# yang-db 后端能力契约

本文件说明 `yang_db::{MYSQL_CAPABILITIES, POSTGRES_CAPABILITIES, REDIS_CAPABILITIES}` 的公开契约。
代码中的 `BackendCapabilities` 常量是可机读的单一事实源；本表用于解释兼容性与安全边界。

## QueryBuilder 与事务能力

| 能力 | MySQL | PostgreSQL | Redis |
|---|---:|---:|---:|
| checked identifier | 是 | 是 | 不适用 |
| select / aggregate / join | 是 | 是 | 不适用 |
| insert / batch insert | 是 | 是 | 不适用 |
| update / batch update / delete | 是 | 是 | 不适用 |
| upsert | `ON DUPLICATE KEY UPDATE` | `ON CONFLICT` | 不适用 |
| 批量 upsert | `INSERT ... ON DUPLICATE KEY UPDATE` | `INSERT ... ON CONFLICT ... DO UPDATE`（显式冲突列 + `EXCLUDED`） | 不适用 |
| 显式冲突目标 | 否，由唯一键决定 | 是，`on_conflict` | 不适用 |
| `RETURNING` | 否 | 是 | 不适用 |
| 事务保存点 | 未声明 | 是，`SAVEPOINT` / `ROLLBACK TO SAVEPOINT`（`Transaction::savepoint`） | 不适用 |
| CTE（`WITH` / `WITH RECURSIVE`） | 未声明 | 是（`with` / `with_recursive`） | 不适用 |
| 窗口函数（`OVER (...)`） | 未声明 | 是（`window_expr`） | 不适用 |
| `FOR UPDATE SKIP LOCKED` | 未声明 | 是 | 不适用 |
| `COPY ... FROM STDIN` 批量导入 | 未声明 | 是（`copy_from_csv` / `copy_to_csv`） | 不适用 |
| `LISTEN` / `NOTIFY` | 未声明 | 是（`PgNotificationListener` / `notify`） | 不适用 |
| 范围 / 多范围（`RANGE` / `MULTIRANGE`） | 未声明 | 是，`FieldType::Range` / `FieldType::Multirange`，`where_range_contains` / `where_range_overlaps` | 不适用 |
| JSONB 路径查询（`jsonpath`） | 未声明 | 是，`where_json_path_exists` / `where_json_path_match`，路径与变量均为绑定参数 | 不适用 |
| 全文检索（`tsvector` / `tsquery`） | 未声明 | 是，`where_full_text` / `where_full_text_query`，固定 `simple` 配置 | 不适用 |
| 事务局部配置（`SET LOCAL`） | 未声明 | 是，`Transaction::set_local`（`set_config(..., true)`） | 不适用 |
| 两阶段提交 | 未声明 | 是，`Database::prepare_transaction` / `commit_prepared` / `rollback_prepared`，需 `max_prepared_transactions > 0` | 不适用 |
| 表级锁（`LOCK TABLE ... IN ... MODE`） | 未声明 | 是，`Transaction::lock_table` + 受控 `TableLockMode` | 不适用 |
| 执行计划（`EXPLAIN`） | 未声明 | 是，`Database::explain`，返回 `FORMAT JSON` 计划文本 | 不适用 |
| 服务端内省 | 未声明 | 是，`Database::server_version_num` / `supports` / `active_queries` / `lock_waits` | 不适用 |
| 事务内 CRUD builder | 是 | 是 | 不适用 |
| Pipeline / Lua | 不适用 | 不适用 | 是 |
| WATCH/MULTI/EXEC | 不适用 | 不适用 | 是 |

上述 Phase C 能力已实现并有真库集成测试，但**没有新增 `BackendCapability` 枚举变体**：
调用方使用对应的专用 API，并通过 `Database::supports(min_version)` 做服务端版本门控。
`supports()` 仍只回答 `POSTGRES_CAPABILITIES` 中已声明的通用能力。
范围/多范围谓词（`where_range_contains` / `where_range_overlaps`）目前只对绑定值追加
`::int4range` / `::int4multirange`；`tsrange`、`numrange`、`daterange` 等其他范围类型
可通过断言字段类型写路径（`FieldType::Range`）与原生 SQL（显式 `$1::tsrange`）使用，
谓词快捷 API 尚不支持这些类型。

“未声明”表示对应后端没有通过 `BackendCapabilities` 或 QueryBuilder 暴露该能力，不代表
底层数据库本身不支持。调用方只能以 `supports()` / `enforces()` 的返回值判断，不得按数据库
版本文档假定能力已可用。PostgreSQL 的 COPY 与 LISTEN/NOTIFY 已实现并在
`POSTGRES_CAPABILITIES` 中声明；MySQL 不声明这些方言能力。

MySQL 与 PostgreSQL 的同义操作使用相同方法名；方言差异只通过显式能力和显式方法暴露。
同一个方法不会因为 feature 或后端不同而悄悄改变含义。Redis 明确声明为非关系后端，不提供
伪 SQL、QueryBuilder 或关系事务能力。

SQL 值始终使用驱动绑定参数。MySQL 使用 `?`，PostgreSQL 使用 `$1`、`$2`。外部标识符必须
进入 checked identifier API；`field`、`group` 等可信 SQL 表达式入口与其显式分离。缺少 WHERE
的 UPDATE/DELETE fail-closed，非法参数统一返回 `DbError::InvalidArgument`。驱动、连接池和命令
故障保留为对应的结构化 `DbError`，不降格为字符串成功值或 `Ok(false)`。

## 统一管理面

| 方法 | MySQL | PostgreSQL | Redis |
|---|---|---|---|
| `capabilities()` | `&'static BackendCapabilities` | 同左 | 同左 |
| `health_check().await` | `Result<bool, DbError>` | 同左 | 同左 |
| `close().await` | `()` | `()` | `()` |
| `is_closed()` | `bool` | `bool` | `bool` |
| `pool_status()` | `PoolStatus` | `PoolStatus` | `PoolStatus` |

健康检查成功时返回 `Ok(true)`；获取连接或执行命令失败时必须返回 `Err(DbError)`。`Ok(false)`
只保留给协议层明确返回非健康响应的情况。三个后端都以 async 形态公开 `close`，即使 Redis
底层的关闭动作是同步完成，也不把该实现细节泄漏给生命周期编排层。

`PoolStatus` 的字段统一为 `max_size`、`size`、`available`、`waiting`。sqlx 当前不暴露等待者
数量，因此 MySQL/PostgreSQL 的 `waiting` 为 0；Redis 返回 deadpool 的实际等待者数量。

## 明确 non-goal

以下能力不进入 `BackendCapabilities`，也不作为基础库的兼容承诺。调用方需要时应使用数据库
原生工具或独立 RFC，不要把“数据库本身支持”误读为“yang-db 已封装”。

| 能力 | 边界与替代路径 |
|---|---|
| 逻辑复制 / 复制槽（publication、subscription、`pg_replication_slots`） | 属集群/DBA 运维，需要 `wal_level`、复制权限与长期状态；由数据库原生复制工具管理。 |
| FDW / 外部表（`postgres_fdw`、`file_fdw`、`IMPORT FOREIGN SCHEMA`） | 跨库联邦与服务端文件访问，权限模型超出运行时查询构建器边界。 |
| backup / restore / database-create | 部署运维层职责；使用 `pg_dump` / `pg_restore` 或平台工具处理权限、审计、加密与生命周期。 |
| 迁移 CLI / checksum runner / 自动 diff | 不在运行时查询库内实现；schema 变更由 yang-base 的加法同步子集或外部迁移工具承担。 |
| 多主机 / 故障转移（`target_session_attrs` 等运行时选主） | 由部署层的连接串、代理或服务发现处理；连接选项以 sqlx 暴露范围为准。 |
| PgBouncer / 事务池模式 | 不承诺 prepared statement 与事务池的兼容组合；需要在部署层选择会话池或关闭服务端预编译。 |

版本相关语法以实际连接的 PostgreSQL 服务端版本为准：本仓库集成测试基线为 PostgreSQL 16，
CI 额外声明 PostgreSQL 17。任何新增版本门控能力都必须在文档中标明最低版本，不能假定
“CI 能跑”等于“所有版本可用”。

