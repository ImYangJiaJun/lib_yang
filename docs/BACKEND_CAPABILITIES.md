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
| 显式冲突目标 | 否，由唯一键决定 | 是，`on_conflict` | 不适用 |
| `RETURNING` | 否 | 是 | 不适用 |
| 事务内 CRUD builder | 是 | 是 | 不适用 |
| Pipeline / Lua | 不适用 | 不适用 | 是 |
| WATCH/MULTI/EXEC | 不适用 | 不适用 | 是 |

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
