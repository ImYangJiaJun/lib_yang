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

## 2026-07-06 - yang-base TableQuery 底层分页上限

- 范围：`crates/yang-base/src/table/table_query.rs`
- 风险：内置 `SelectAction` 限制 `page_size <= 100`，但底层 `TableQuery::page()` 只拒绝 0。自定义 action 或库调用方直接使用 `ctx.table_query()?.page(...)` 时可绕过上层限制，发起超大查询，造成数据库压力或应用内存风险。
- 修复：新增 `MAX_TABLE_QUERY_PAGE_SIZE = 100`，并在 `TableQuery::page()` 底层统一拒绝超过上限的 `page_size`。
- 对抗性验证：新增单元测试证明 `TableQuery::page(1, 101)` 会返回 `BaseError::ParamInvalid("page_size", ...)`。
- 已运行验证：`cargo test -p yang-base --lib test_page_rejects_page_size_above_production_limit`
- 已运行验证：`cargo test -p yang-base --lib test_paginated_result_new`

## 2026-07-06 - yang-base QueryParams 分页归一化上限

- 范围：`crates/yang-base/src/table/query_params.rs`、`crates/yang-base/src/table/table_query.rs`、`crates/yang-base/src/table/mod.rs`
- 风险：`QueryParams::normalize()` 只修正 `page=0`，不处理 `page_size=0` 或超大 `page_size`。作为可反序列化 DTO，它可能在进入 `TableQuery` 前被上层独立归一化；如果不处理 page_size，会形成和底层执行边界不一致的分页行为。
- 修复：新增 `DEFAULT_QUERY_PAGE_SIZE = 10` 与 `MAX_QUERY_PAGE_SIZE = 100`，`normalize()` 将 `page_size=0` 归一化为默认值，并将超过上限的 `page_size` 截到 100。
- 修复：`MAX_TABLE_QUERY_PAGE_SIZE` 改为复用 `MAX_QUERY_PAGE_SIZE`，并通过 `table::mod` 重导出分页上限常量，避免调用方重复硬编码。
- 对抗性验证：新增单元测试覆盖 `page=0/page_size=0` 和 `page_size=101` 的归一化结果。
- 已运行验证：`cargo test -p yang-base --lib test_query_params_normalize_clamps_invalid_pagination`
- 已运行验证：`cargo test -p yang-base --lib test_page_rejects_page_size_above_production_limit`

## 2026-07-06 - yang-pcg Combat 敌人预算饱和加法

- 范围：`crates/yang-pcg/src/spawn/budget.rs`
- 风险：`RoomType::Combat` 使用 `base + room.difficulty`，在 debug 构建中会因 `u16` 溢出 panic，在 release 构建中存在回绕风险；Boss/Elite 分支已使用饱和运算。
- 修改：将 Combat 分支改为 `base.saturating_add(room.difficulty)`，保持敌人预算上限在 `u16::MAX`。
- 验证：`cargo test -p yang-pcg --lib test_combat_enemy_budget_saturates_on_overflow`
## 2026-07-06 - yang-pcg RoomBounds 极端坐标宽整数计算

- 范围：`crates/yang-pcg/src/model/geometry.rs`
- 风险：`RoomBounds::width`、`height`、`center` 直接使用 `i32` 加减，极端合法坐标会在 debug 构建中溢出 panic，在 release 构建中得到错误几何结果。
- 修改：将宽度、高度、中心点的中间计算提升到 `i64`，保持对外返回类型不变，并补充极端坐标回归测试。
- 验证：`cargo test -p yang-pcg --lib test_room_bounds_`
## 2026-07-06 - yang-pcg RangeU16 闭区间采样包含最大上界

- 范围：`crates/yang-pcg/src/topology/graph.rs`
- 风险：`sample_range_u16` 用 `range.max.saturating_add(1)` 构造半开区间，当 `range.max == u16::MAX` 时会把合法上界永久排除，破坏闭区间采样语义，并影响拓扑、布局和点位数量采样。
- 修改：将采样区间提升为 `u32` 后再构造半开区间，采样结果再转回 `u16`。
- 验证：`cargo test -p yang-pcg --lib test_sample_range_u16_includes_u16_max_upper_bound`