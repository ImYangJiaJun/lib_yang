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
## 2026-07-06 - yang-base 默认排序复用排序权限校验

- 范围：`crates/yang-base/src/table/table_query.rs`
- 风险：显式 `order_by()` 会拒绝 `sortable(false)` 或无排序权限字段，但 `TableConfig::default_order` 在 SQL 构造时只检查字段存在，可能绕过同一条硬约束。
- 修改：抽出统一排序字段校验，显式排序和默认排序共同检查字段存在、`sortable` 开关和角色排序权限。
- 验证：`cargo test -p yang-base --lib test_default_order_rejects_unsortable_field`
## 2026-07-06 - yang-base SELECT * 强制字段读取权限

- 范围：`crates/yang-base/src/table/table_query.rs`
- 风险：显式 `select_fields()` 会校验字段读取权限，但默认读路径生成 `SELECT *` 时没有底层权限防线，库调用方绕过内置 Action 时可能返回用户无权读取的字段。
- 修改：将字段读取权限校验下沉到 SQL 构造层；`SELECT *` 要求当前角色可读取表内所有字段，显式字段也在构造 SQL 时再次校验。
- 验证：`cargo test -p yang-base --lib test_select_star_rejects_unreadable_field`
## 2026-07-06 - yang-base paginate 默认分页写回数据查询

- 范围：`crates/yang-base/src/table/table_query.rs`
- 风险：`paginate()` 在调用方未显式 `.page()` 时只把默认 `page/page_size` 用于返回元数据，数据查询仍可能不带 `LIMIT/OFFSET`，导致分页接口退化为全量读取。
- 修改：新增有效分页归一化 helper，`paginate()` 在 COUNT 和数据 SELECT 之间使用同一份分页状态，并采用 `query_params` 的默认 page size。
- 验证：`cargo test -p yang-base --lib test_effective_pagination_applies_default_limit_to_data_query_sql`
## 2026-07-06 - yang-base HTTP 客户端配置零值校验

- 范围：`crates/yang-base/src/http/client.rs`
- 风险：`HttpClientConfig` 允许 0 秒超时、0 空闲连接等无效配置进入 reqwest builder，可能导致请求立即失败或连接池行为退化，且错误暴露较晚。
- 修改：新增 `HttpClientConfig::validate()`，拒绝 `timeout_secs`、`pool_max_idle_per_host`、`pool_idle_timeout_secs` 的零值，并在 `HttpClient::with_config()` 开头 fail-fast。
- 验证：`cargo test -p yang-base --lib http_client_config`；`cargo test -p yang-base --lib test_with_config_rejects_invalid_config_before_building_client`
## 2026-07-07 - yang-base HTTP retry 策略边界校验

- 范围：`crates/yang-base/src/http/request.rs`
- 风险：`RetryConfig` 可接受过大的 `max_retries`、空 `retry_on`、0 毫秒或过大的退避，以及非法 HTTP 状态码；这些配置会导致请求热循环、长时间阻塞或无意义重试，并且错误暴露在网络调用之后。
- 修改：新增 `RetryConfig::validate()`，限制最大重试次数、退避时间和状态码范围；`RequestBuilder::send()` 在发起网络请求前 fail-fast 校验 retry 策略。
- 验证：`cargo test -p yang-base --lib retry_config`
## 2026-07-07 - yang-base 请求级 HTTP timeout 零值校验

- 范围：`crates/yang-base/src/http/request.rs`
- 风险：`HttpClientConfig` 已拒绝 0 秒超时，但 `RequestBuilder::timeout(0)` 仍可覆盖为 0 秒请求级超时，导致请求在发送后才以传输错误形式失败。
- 修改：`RequestBuilder::send()` 在网络调用前检查 `self.timeout.is_zero()`，对 0 秒超时返回 `BaseError::ParamInvalid("http.timeout_secs", ...)`。
- 验证：`cargo test -p yang-base --lib test_send_rejects_zero_request_timeout_before_network`
## 2026-07-07 - yang-base HTTP 熔断器配置零值校验

- 范围：`crates/yang-base/src/http/circuit_breaker.rs`、`crates/yang-base/src/http/client.rs`
- 风险：`CircuitBreakerConfig` 可接受 0 次失败阈值、0 秒冷却或 0 次恢复成功阈值，破坏 Closed/Open/HalfOpen 状态机语义，并可能让非法策略进入客户端运行期。
- 修改：新增 `CircuitBreakerConfig::validate()`，拒绝熔断器零值策略；`HttpClientConfig::validate()` 在构建客户端前同步校验嵌套熔断器配置。
- 验证：`cargo test -p yang-base --lib circuit_breaker_config`
## 2026-07-07 - yang-base CircuitBreaker 构造器 fail-fast

- 范围：`crates/yang-base/src/http/circuit_breaker.rs`、`crates/yang-base/src/http/client.rs`、`crates/yang-base/src/http/__tests__/circuit_breaker_test.rs`、`crates/yang-base/src/http/__tests__/circuit_breaker_concurrency_test.rs`
- 风险：虽然 `HttpClientConfig` 路径已校验熔断器配置，公开的 `CircuitBreaker::new` 仍可被库调用方直接传入非法策略，绕过 fail-fast 边界。
- 修改：将 `CircuitBreaker::new` 改为返回 `Result<CircuitBreaker, BaseError>` 并内部调用 `CircuitBreakerConfig::validate()`；`HttpClient::with_config()` 使用 `transpose()?` 传播构造错误。
- 兼容：这是一次有意的破坏性 API 收紧；当前库尚未正式使用，优先保证基础库边界正确。
- 验证：`cargo test -p yang-base --lib circuit_breaker`
## 2026-07-07 - yang-base HTTP bearer token 头部 fail-fast 校验

- 范围：`crates/yang-base/src/http/request.rs`
- 风险：`content_type`、`user_agent` 等显式 header 已在发送前校验，但默认 token 或 `bearer_token()` 设置的值可能包含非法控制字符，之前会在 reqwest 构造/发送阶段暴露为传输错误，错误类型不准确且定位较晚。
- 修改：`RequestBuilder::send()` 在网络调用前构造并校验 `Authorization: Bearer <token>` 的 header 值，非法 token 返回 `BaseError::ParamInvalid("authorization", ...)`，且错误消息不回显 token 原文。
- 验证：`cargo test -p yang-base --lib test_send_rejects_invalid_bearer_token_before_network`
## 2026-07-07 - yang-base HTTP URL 参数 fail-fast 校验

- 范围：`crates/yang-base/src/http/request.rs`
- 风险：非法 URL 或非 `http/https` scheme 之前会交给 reqwest 在发送阶段处理，并被包装成 `HttpRequestFailed`，对调用方来说错误类型不准确，也会让熔断器 host 分键在非法 URL 上退化为无分键。
- 修改：`RequestBuilder::send()` 在网络调用前解析 URL，并仅允许 `http`/`https` scheme；解析后的 URL 复用于熔断器 host 分键。
- 验证：`cargo test -p yang-base --lib test_send_rejects_invalid_url_before_network`
## 2026-07-07 - yang-base HTTP 出站 URL 日志脱敏

- 范围：`crates/yang-base/src/http/request.rs`
- 风险：出站请求日志原样记录 `self.url`，当调用方直接传入带 query 参数或 userinfo 的 URL 时，可能把 token、password 等敏感信息写入日志。
- 修改：新增私有 `redact_url_for_log()`，日志记录前移除 query，并将 URL username/password 替换为 `***`；成功和失败日志统一使用脱敏 URL。
- 验证：`cargo test -p yang-base --lib test_redact_url_for_log_removes_query_and_userinfo`
## 2026-07-07 - yang-base Action Request header 大小写不敏感读取

- 范围：`crates/yang-base/src/action/request.rs`
- 风险：HTTP header 名大小写不敏感，但 `Action Request::get_header()` 只做精确匹配，`token()` 也只识别 `Authorization`/`authorization` 两种写法。路由或测试构造中出现混合大小写 header 时，认证 token 可能被误判为缺失。
- 修改：`get_header()` 保留精确命中快路径，并增加 `eq_ignore_ascii_case` fallback；`token()` 复用 `get_header("authorization")`。
- 验证：`cargo test -p yang-base --lib test_header_lookup_is_case_insensitive`
