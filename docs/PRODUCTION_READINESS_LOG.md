# 生产级基础库修复记录

本文档记录基础库生产级审计中已经完成的修复点。每个完成点对应一次本地 git 提交；未完成的 RED 测试、探索结论或临时状态不作为完成点记录。

## 2026-07-06 - yang-db QueryBuilder SQL 调试接口错误暴露

- 范围：`crates/yang-db/src/mysql/query_builder.rs`、`crates/yang-db/src/postgres/query_builder.rs`
- 风险：公开 `to_sql()` 在 SQL 生成失败时吞掉真实错误，并拼出简化 SQL。非法表名或缺少 `GROUP BY` 的查询会被伪装成成功生成的 SQL，既影响调试判断，也可能在日志/上层拼接中泄漏未校验输入。
- 修复：新增 `try_to_sql() -> Result<String, DbError>`，让调用方可以拿到 `InvalidArgument`、`MissingGroupByClause` 等真实错误。
- 兼容：保留 `to_sql() -> String` 签名；失败时返回固定不可执行哨兵 `/* SQL generation failed */`，不再包含未校验表名或不完整查询结构。
- 对抗性验证：新增 MySQL/PostgreSQL 各 3 个单元测试，覆盖非法表名错误暴露、缺少 `GROUP BY` 错误暴露、旧降级路径不再泄漏 `DROP TABLE` 载荷。
- 已运行验证：`cargo test -p yang-db --lib try_to_sql`
- 已运行验证：`cargo test -p yang-db --lib to_sql_does_not_fallback_to_raw_untrusted_table`

## 2026-07-06 - yang-db RedisConfig 生产配置 fail-fast 校验

- 范围：`crates/yang-db/src/redis/config.rs`、`crates/yang-db/src/redis/client.rs`
- 风险：`RedisConfig` 的 builder 允许明显非法配置，例如 `max_connections = 0`、`min_connections > max_connections`、零秒超时、`idle_timeout <= connect_timeout`。这些配置如果直接进入建池流程，会在运行期表现为连接池创建失败、立即超时或连接被过早回收，错误定位晚且不稳定。
- 修复：新增 `RedisConfig::validate()`，把非法配置统一映射为 `DbError::InvalidArgument`。
- 修复：`RedisClient::connect_with_config()` 在创建连接池和发起连接前先调用 `validate()`，保证非法配置 fail-fast，不触发网络连接。
- 对抗性验证：新增配置校验测试，覆盖默认配置可用、零连接数、`min_connections` 超过 `max_connections`、零秒超时、`idle_timeout` 不大于 `connect_timeout`。
- 对抗性验证：新增连接入口测试，确认非法配置在连接前直接返回 `InvalidArgument`。
- 已运行验证：`cargo test -p yang-db --lib validate_rejects`
- 已运行验证：`cargo test -p yang-db --lib connect_with_config_rejects_invalid_config_before_connecting`
- 已运行验证：`cargo test -p yang-db --lib validate_accepts_default_config`

## 2026-07-06 - yang-db MySQL/PostgreSQL DatabaseConfig 生产配置 fail-fast 校验

- 范围：`crates/yang-db/src/mysql/database.rs`、`crates/yang-db/src/postgres/database.rs`
- 风险：MySQL/PostgreSQL 的 `DatabaseConfig` 允许明显非法配置，例如 `max_connections = 0`、`min_connections > max_connections`、零秒超时、`idle_timeout <= connect_timeout`。这些配置如果直接进入 sqlx 建池流程，会在运行时才暴露为连接池错误、立即超时或连接生命周期异常。
- 修复：MySQL/PostgreSQL 分别新增 `DatabaseConfig::validate()`，把非法配置统一映射为 `DbError::InvalidArgument`。
- 修复：MySQL/PostgreSQL 的 `Database::connect_with_config()` 在创建 sqlx pool 和发起连接前先调用 `validate()`，保证非法配置 fail-fast。
- 对抗性验证：新增 MySQL/PostgreSQL 单元测试，覆盖默认配置可用、非法池大小、非法超时、连接入口在联网前拒绝非法配置。
- 已运行验证：`cargo test -p yang-db --lib database_config_validate_rejects_invalid_pool_size`
- 已运行验证：`cargo test -p yang-db --lib database_config_validate_rejects_invalid_timeouts`
- 已运行验证：`cargo test -p yang-db --lib database_config_validate_accepts_default_config`
- 已运行验证：`cargo test -p yang-db --lib test_connect_with_config_rejects_invalid_config_before_connecting`
