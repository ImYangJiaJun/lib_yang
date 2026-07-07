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
## 2026-07-07 - yang-base Action Request header 写入归一化

- 范围：`crates/yang-base/src/action/request.rs`
- 风险：`Request::header()`/`headers()` 允许 `Authorization` 与 `authorization` 等大小写变体同时存在，导致读取值取决于查询大小写，认证和中间件行为不确定。
- 修改：通过 builder 写入 header 时统一将名称归一化为 ASCII 小写；批量写入复用单个 `header()` 逻辑，同名大小写变体以后写值覆盖先写值。
- 兼容：这是有意的公共字段内容形态收紧；`Request.headers` 仍为 `HashMap<String, String>`，但经 builder 写入的 key 现在稳定为小写。
- 验证：`cargo test -p yang-base --lib test_header_`
## 2026-07-07 - yang-base Action Request Bearer scheme 大小写不敏感

- 范围：`crates/yang-base/src/action/request.rs`
- 风险：HTTP Authorization scheme 大小写不敏感，但 `Request::token()` 只接受精确 `Bearer `，导致 `bearer <token>` 或 `BEARER <token>` 被误判为未认证。
- 修改：新增私有 `parse_bearer_token()`，用 `split_once(' ')` 拆分 scheme/token，并对 scheme 使用 `eq_ignore_ascii_case("Bearer")`。
- 验证：`cargo test -p yang-base --lib test_token_accepts_case_insensitive_bearer_scheme`
## 2026-07-07 - yang-base Action Request Bearer token 空白边界校验

- 范围：`crates/yang-base/src/action/request.rs`
- 风险：`Request::token()` 之前会把 `Bearer ` 解析为 `Some("")`，也会把 `Bearer    token` 解析为带前导空格的 token，后续认证错误定位不稳定。
- 修改：`parse_bearer_token()` 改为按空白分段解析，仅接受 `Bearer <token>` 两段；多空格会被归一化，空 token 或额外分段返回 `None`。
- 验证：`cargo test -p yang-base --lib test_token_`
## 2026-07-07 - yang-base GlobalTools 重复工具注册 fail-fast

- 范围：`crates/yang-base/src/action/context.rs`、`crates/yang-base/src/action/__tests__/context_test.rs`、`crates/yang-base/src/action/__tests__/global_tools_concurrency_test.rs`
- 风险：`GlobalTools::register_tool()` 对同名工具静默覆盖，依赖注入配置错误会被延迟到运行期表现为工具实例不符合预期。
- 修改：`register_tool()` 改为返回 `Result<(), BaseError>`，同名重复注册返回 `BaseError::ConfigError("工具已注册: ...")`，不覆盖已有实例；并发测试同步为“首个注册成功，后续重复注册失败但不破坏 map”。
- 兼容：这是有意的破坏性 API 收紧；调用方现在必须处理注册失败。
- 验证：`cargo test -p yang-base --lib global_tools`
## 2026-07-07 - yang-base GlobalTools 工具名非空校验

- 范围：`crates/yang-base/src/action/context.rs`、`crates/yang-base/src/action/__tests__/context_test.rs`
- 风险：`GlobalTools::register_tool()` 允许空字符串或纯空白工具名进入注册表，后续按名称获取、审计和排错都缺少稳定标识。
- 修改：注册前校验 `name.trim().is_empty()`，空白名称返回 `BaseError::ConfigError("工具名称不能为空")`，不进入写锁和注册表。
- 验证：`cargo test -p yang-base --lib register_tool`
## 2026-07-07 - yang-base AppRouter 重复模块注册 fail-fast

- 范围：`crates/yang-base/src/router/app_router.rs`、`crates/yang-base/src/router/mod.rs`、`docs/yang-base.md`
- 风险：`AppRouter::register_module()` 对同名模块静默覆盖，应用启动阶段的路由配置错误会被延迟到运行期表现为错误模块处理请求。
- 修改：`register_module()` 改为返回 `Result<AppRouter, BaseError>`，重复模块名返回 `BaseError::ConfigError("模块已注册: ...")`，不覆盖已有模块；同步源码和 API 文档示例。
- 兼容：这是有意的破坏性 API 收紧；调用方现在必须处理模块注册失败。
- 验证：`cargo test -p yang-base --lib test_register_module_rejects_duplicate_module_name`
## 2026-07-07 - yang-base AppRouter 模块名非空校验

- 范围：`crates/yang-base/src/router/app_router.rs`
- 风险：`AppRouter::register_module()` 允许空字符串或纯空白模块名进入路由表，后续 dispatch、metrics 和日志都缺少稳定模块标识。
- 修改：注册模块前校验 `module_name.trim().is_empty()`，空白模块名返回 `BaseError::ConfigError("模块名称不能为空")`。
- 验证：`cargo test -p yang-base --lib test_register_module_`
## 2026-07-07 - yang-base ModuleRouter 重复 Action 注册 fail-fast

- 范围：`crates/yang-base/src/router/module_router.rs`、`crates/yang-base/src/router/__tests__/module_router_tests.rs`、`docs/yang-base.md`
- 风险：`ModuleRouter::register_action()` 对同名 Action 静默覆盖，路由启动配置错误会变成运行时 dispatch 到错误处理器。
- 修改：`register_action()` 改为返回 `Result<ModuleRouter, BaseError>`，重复 Action 名返回 `BaseError::ConfigError("Action 已注册: ...")`；`table_typed()` 使用 `?` 串联六个内置 Action 注册；同步 API 文档示例。
- 兼容：这是有意的破坏性 API 收紧；自定义 Action 注册调用方现在必须处理注册失败。
- 验证：`cargo test -p yang-base --lib register_action`
## 2026-07-07 - yang-base ModuleRouter Action 名非空校验

- 范围：`crates/yang-base/src/router/module_router.rs`、`crates/yang-base/src/router/__tests__/module_router_tests.rs`
- 风险：`ModuleRouter::register_action()` 允许空字符串或纯空白 Action 名进入路由表，dispatch、metrics 和日志会缺少稳定 Action 标识。
- 修改：注册 Action 前校验 `name.trim().is_empty()`，空白名称返回 `BaseError::ConfigError("Action 名称不能为空")`。
- 验证：`cargo test -p yang-base --lib register_action`
## 2026-07-07 - yang-base ActionContext 用户注入边界收紧

- 范围：`crates/yang-base/src/action/context.rs`、`crates/yang-base/src/action/__tests__/context_test.rs`、`crates/yang-base/tests/typed_action_integration.rs`、`docs/yang-base.md`
- 风险：`ActionContext::with_user()` 是公开方法，外部调用方可构造上下文并注入任意用户绕过 TokenAuthMiddleware，`ModuleRouter::authorize_and_dispatch()` 只检查上下文中是否已有用户。
- 修改：将 `with_user()` 降为 `pub(crate)`，新增只读 `authenticated_user()`；外部 CRUD 集成测试改为通过真实 access token 和 `TokenAuthMiddleware` 建立登录态；API 文档移除手动注入用户示例。
- 兼容：这是有意的破坏性 API 收紧；外部认证扩展不能再直接篡改 `ActionContext.user`，需走受控中间件路径。
- 验证：`cargo test -p yang-base --lib test_action_context_authenticated_user_getter`
- 验证：`cargo test -p yang-base --test typed_action_integration --no-run`
## 2026-07-07 - yang-base ModuleRouter Action 权限元数据非空校验

- 范围：`crates/yang-base/src/router/module_router.rs`、`crates/yang-base/src/router/__tests__/module_router_tests.rs`
- 风险：自定义 Action 的 `ActionMeta.permissions` 可包含空白权限名，注册成功后会在运行期表现为永远无法满足或错误信息不可定位的权限要求。
- 修改：`ModuleRouter::register_action()` 在注册阶段遍历 `meta.permissions`，发现空白权限名时返回 `BaseError::ConfigError("Action 权限名称不能为空")`。
- 验证：`cargo test -p yang-base --lib register_action`
## 2026-07-07 - yang-base ModuleRouter 默认权限名非空校验

- 范围：`crates/yang-base/src/router/module_router.rs`、`crates/yang-base/src/router/__tests__/module_router_tests.rs`、`crates/yang-base/src/router/mod.rs`、`docs/yang-base.md`
- 风险：`ModuleRouter::default_permissions()` 允许空字符串或纯空白权限名进入模块默认权限列表，后续鉴权失败信息和配置排错都缺少稳定权限标识。
- 修改：`default_permissions()` 改为返回 `Result<ModuleRouter, BaseError>`，配置阶段拒绝空白权限名并返回 `BaseError::ConfigError("默认权限名称不能为空")`；同步源码和 API 文档示例。
- 兼容：这是有意的破坏性 API 收紧；调用方现在必须处理默认权限配置错误。
- 验证：`cargo test -p yang-base --lib default_permissions`
## 2026-07-07 - yang-base RequestId 上游全零标识拒绝

- 范围：`crates/yang-base/src/action/request_id.rs`、`crates/yang-base/src/router/__tests__/request_id_middleware_tests.rs`
- 风险：上游 `X-Request-Id` 为全零值时会被解析为 `RequestId(0)` 并覆盖 `ActionContext` 已生成的运行期标识；全零值是典型无效/哨兵标识，会破坏日志、span、metrics 和审计串联。
- 修改：`RequestId::parse_hex()` 将解析结果 `0` 视为无效并返回 `None`，`RequestIdMiddleware` 因此保留已有默认生成值；新增中间件测试覆盖全零 header 不透传。
- 验证：`cargo test -p yang-base --lib request_id_middleware`
- 验证：`cargo test -p yang-base --lib action::request_id::tests`
## 2026-07-07 - yang-base Action Request query 空白键拒绝

- 范围：`crates/yang-base/src/action/request.rs`
- 风险：`Request::query()` 与 `queries()` 允许空字符串或纯空白 query key 写入请求上下文，调用方后续无法可靠区分参数缺失、错误写入和真实空 key。
- 修改：`query()` 在写入前拒绝空白 key；`queries()` 改为复用 `query()`，保证单个和批量写入行为一致。
- 验证：`cargo test -p yang-base --lib test_query_rejects_blank_keys`
## 2026-07-07 - yang-base Action Request path 参数空白键拒绝

- 范围：`crates/yang-base/src/action/request.rs`
- 风险：`Request::path_param()` 与 `path_params()` 允许空字符串或纯空白路径参数 key 写入请求上下文，路由参数配置错误会延迟为运行期缺参或错参，降低 Action 入参边界可靠性。
- 修改：`path_param()` 在写入前拒绝空白 key；`path_params()` 改为复用 `path_param()`，保证单个和批量写入行为一致。
- 验证：`cargo test -p yang-base --lib test_path_param_rejects_blank_keys`
## 2026-07-07 - yang-base Action Request header 空白名拒绝

- 范围：`crates/yang-base/src/action/request.rs`
- 风险：`Request::header()` 允许空字符串或纯空白 header 名进入请求上下文，认证、中间件和追踪逻辑依赖 header 索引时会遇到非 HTTP 语义 key。
- 修改：`header()` 在归一化写入前拒绝空白 header 名；`headers()` 继续复用 `header()`，批量写入同步继承该边界。
- 验证：`cargo test -p yang-base --lib test_header_rejects_blank_names`
- 验证：`cargo test -p yang-base --lib test_header_`
## 2026-07-07 - yang-base ActionContext path 参数名读取校验

- 范围：`crates/yang-base/src/action/context.rs`、`crates/yang-base/src/action/__tests__/context_test.rs`
- 风险：`Request.path_params` 是 public 字段，外部可绕过 builder 直接写入空白 key；`ActionContext::path_param("")` 之前会读取该值并返回成功，导致 Action 读取侧接受无效参数名。
- 修改：`ActionContext::path_param()` 在读取前校验 `key.trim().is_empty()`，空白参数名返回 `BaseError::ParamInvalid("", "路径参数名不能为空")`。
- 验证：`cargo test -p yang-base --lib test_action_context_path_param_rejects_blank_key`
- 验证：`cargo test -p yang-base --lib test_action_context_path_param`
## 2026-07-07 - yang-base ModuleRouter 默认权限重复名校验

- 范围：`crates/yang-base/src/router/module_router.rs`
- 风险：`ModuleRouter::default_permissions()` 允许重复权限名进入默认权限列表，导致配置冗余、错误信息重复，并掩盖启动阶段的权限配置错误。
- 修改：配置默认权限时用 `HashSet` 检测重复项，发现重复权限名返回 `BaseError::ConfigError("默认权限重复: ...")`。
- 验证：`cargo test -p yang-base --lib test_default_permissions_rejects_duplicate_permission_name`
- 验证：`cargo test -p yang-base --lib default_permissions`
## 2026-07-07 - yang-base Action Request header 空白名读取拒绝

- 范围：`crates/yang-base/src/action/request.rs`
- 风险：`Request.headers` 是 public 字段，外部可绕过 `header()` builder 直接写入空白 key；`get_header("")` 之前会读取该值，导致读取侧接受非 HTTP 语义 header 名。
- 修改：`get_header()` 在读取前校验空白名称，空白 header 名直接返回 `None`，保留合法名称的大小写不敏感查找。
- 验证：`cargo test -p yang-base --lib test_get_header_rejects_blank_names`
- 验证：`cargo test -p yang-base --lib test_header_`
## 2026-07-07 - yang-base Action Request query 空白键读取拒绝

- 范围：`crates/yang-base/src/action/request.rs`
- 风险：`Request.query` 是 public 字段，外部可绕过 `query()` builder 直接写入空白 key；`get_query("")` 之前会读取该值，导致读取侧接受无效 query 名。
- 修改：`get_query()` 在读取前校验空白 key，空白 query 名直接返回 `None`，合法 query 读取行为不变。
- 验证：`cargo test -p yang-base --lib test_get_query_rejects_blank_keys`
- 验证：`cargo test -p yang-base --lib query_rejects_blank_keys`
## 2026-07-07 - yang-base Action Request path 参数空白键读取拒绝

- 范围：`crates/yang-base/src/action/request.rs`
- 风险：`Request.path_params` 是 public 字段，外部可绕过 `path_param()` builder 直接写入空白 key；`get_path_param("")` 之前会读取该值，导致读取侧接受无效路径参数名。
- 修改：`get_path_param()` 在读取前校验空白 key，空白路径参数名直接返回 `None`，合法路径参数读取行为不变。
- 验证：`cargo test -p yang-base --lib test_get_path_param_rejects_blank_keys`
- 验证：`cargo test -p yang-base --lib path_param_rejects_blank_keys`
## 2026-07-07 - yang-base ActionContext body 参数空白名读取校验

- 范围：`crates/yang-base/src/action/context.rs`、`crates/yang-base/src/action/__tests__/context_test.rs`
- 风险：`ActionContext::param_optional_strict("")` 之前会读取 `Request.body` 中的空 key 并返回成功，导致旧 body 参数读取入口接受无效参数名。
- 修改：`param_optional_strict()` 在读取前校验空白参数名，空白 key 返回 `BaseError::ParamInvalid("", "参数名不能为空")`，缺失的合法参数仍返回 `Ok(None)`。
- 验证：`cargo test -p yang-base --lib test_action_context_param_optional_strict_rejects_blank_key`
- 验证：`cargo test -p yang-base --lib test_action_context_param_optional_strict`
## 2026-07-07 - yang-pcg Grid2D 索引溢出保护

- 范围：`crates/yang-pcg/src/model/terrain.rs`
- 风险：`Grid2D::get()` 与 `set()` 使用 `u32` 乘加计算行优先索引，异常大尺寸下 debug 会 panic，release 可能整数回绕后错误命中 `data[0]` 等位置。
- 修改：坐标先做负值和边界检查，再使用 `usize::checked_mul()`/`checked_add()` 计算索引；溢出时 `get()` 返回 `None`，`set()` 返回 `false`。
- 验证：`cargo test -p yang-pcg grid_`
## 2026-07-07 - yang-base DynamicRow 空白列名读取拒绝

- 范围：`crates/yang-base/src/table/dynamic_row.rs`
- 风险：`DynamicRow.columns` 是 public map，外部或解码路径若写入空字符串/纯空白列名，`DynamicRow::get()` 之前会返回该值，导致表行读取侧接受无效列名。
- 修改：`DynamicRow::get()` 在读取前校验空白列名，空白 key 直接返回 `None`，合法列读取行为不变。
- 验证：`cargo test -p yang-base --lib get_rejects_blank_column_name`
## 2026-07-07 - yang-pcg Grid2D 可失败构造与尺寸上限

- 范围：`crates/yang-pcg/src/model/terrain.rs`、`crates/yang-pcg/src/terrain/carve.rs`、`crates/yang-pcg/src/terrain/maze.rs`、`crates/yang-pcg/src/terrain/organic.rs`、`crates/yang-pcg/src/terrain/open_arena.rs`、`crates/yang-pcg/src/terrain/pillar.rs`、`crates/yang-pcg/src/model/__tests__/terrain_test.rs`、`crates/yang-pcg/src/model/__tests__/result_test.rs`
- 风险：`Grid2D::new()` 之前用 `width * height` 直接计算容量，异常尺寸可能在 debug 下 panic，或在 release/64 位平台上尝试巨量分配导致 OOM。
- 修改：`Grid2D::new()` 改为返回 `PcgResult<Grid2D<T>>`，使用 checked 乘法并引入单网格最大格子数 `1_048_576`；terrain 策略调用点通过 `?` 传播构造错误，测试中的合法尺寸显式 `expect`。
- 兼容：这是有意的破坏性 API 收紧；调用方必须处理网格构造失败。
- 验证：`cargo test -p yang-pcg grid_new_rejects_excessive_size`
- 验证：`cargo test -p yang-pcg grid_`
- 验证：`cargo test -p yang-pcg test_grid2d`
- 验证：`cargo test -p yang-pcg test_generation_result_full_json_roundtrip`

## 2026-07-07 - yang-base TableConfig 字段注册空白名称校验

- 范围：`crates/yang-base/src/table/table_config.rs`、`crates/yang-base-derive/src/table_entity.rs`、table 相关单元测试、integration test 构造器与 `batch_field_config` 示例。
- 风险：此前 `TableConfig::field` / `fields` / `fields_from_iter` 会接受空白字段名，导致无效字段进入表配置并推迟到查询构建阶段暴露。
- 修改：字段注册 builder 改为返回 `Result<Self, BaseError>`，统一拒绝 `name.trim().is_empty()` 的 `FieldConfig`；合法调用点显式 `expect`；`TableEntity` 派生宏对空白列名做编译期 `abort!`，并适配 fallible builder。
- 兼容：这是公开 API 破坏性变更，调用方需要在字段注册链上使用 `?` 或带上下文的 `expect`。
- 验证：`cargo test -p yang-base --lib test_table_config_field_rejects_blank_name`；`cargo test -p yang-base --lib table_config`；`cargo test -p yang-base --test table_query_paginate_test --no-run`；`cargo test -p yang-base --test table_query_crud_test --no-run`；`cargo test -p yang-base --test table_query_transaction_test --no-run`；`cargo check -p yang-base --example batch_field_config`。

## 2026-07-07 - yang-base TableQuery 空字段选择拒绝

- 范围：`crates/yang-base/src/table/table_query.rs` 与 `crates/yang-base/src/table/__tests__/table_query_test.rs`。
- 风险：此前 `TableQuery::select_fields(&[])` 会接受空字段列表并写入 `QueryParams.fields = Some([])`，后续可能构造无意义或非法的 SELECT 字段片段。
- 修改：`select_fields` 入口显式拒绝空列表，返回 `BaseError::ParamInvalid("fields", "查询字段列表不能为空")`。
- 兼容：合法非空字段选择行为不变。
- 验证：`cargo test -p yang-base --lib test_select_fields_rejects_empty_list`；`cargo test -p yang-base --lib select_fields`。

## 2026-07-07 - yang-base TableQuery 空 IN 列表拒绝

- 范围：`crates/yang-base/src/table/table_query.rs` 与 `crates/yang-base/src/table/__tests__/table_query_test.rs`。
- 风险：此前 `where_in` / `where_not_in` 接受空 `Vec`，会把 `WhereCondition::In/NotIn` 的值列表置空，后续可能渲染为非法或语义不明确的 SQL。
- 修改：`where_in` 对空列表返回 `BaseError::ParamInvalid("values", "IN 列表不能为空")`；`where_not_in` 对空列表返回 `BaseError::ParamInvalid("values", "NOT IN 列表不能为空")`；保留原最大长度限制。
- 兼容：非空列表、权限校验和最大长度限制行为不变。
- 验证：`cargo test -p yang-base --lib rejects_empty_values`；`cargo test -p yang-base --lib where_in`；`cargo test -p yang-base --lib where_not_in`。

## 2026-07-07 - yang-base WhereCondition 递归空 IN 列表拒绝

- 范围：`crates/yang-base/src/table/table_query.rs` 与 `crates/yang-base/src/table/__tests__/table_query_test.rs`。
- 风险：即使 `where_in` / `where_not_in` 入口拒绝空列表，调用方仍可通过 `where_tree`、`where_or` 或 `where_and` 直接提交 `WhereCondition::In/NotIn` 空值列表，绕过入口校验。
- 修改：`validate_condition_tree` 的 IN/NOT IN 叶子校验新增空列表拒绝，递归入口与便捷入口保持一致。
- 兼容：非空 IN/NOT IN 条件、空 AND/OR 组拒绝和最大长度限制行为不变。
- 验证：`cargo test -p yang-base --lib empty_in_values`；`cargo test -p yang-base --lib empty_not_in_values`；`cargo test -p yang-base --lib test_empty_groups_rejected`。

## 2026-07-07 - yang-base TableQuery 空白 contains 关键词拒绝

- 范围：`crates/yang-base/src/table/table_query.rs` 与 `crates/yang-base/src/table/__tests__/table_query_test.rs`。
- 风险：此前 `where_contains` 会把空白关键词包装成 `%   %` 或 `%%` 类 LIKE 条件，调用方可能误以为存在有效筛选，实际形成近似全匹配或低选择性查询。
- 修改：`where_contains` 在转义通配符前按 `keyword.trim().is_empty()` 拒绝空白关键词，返回 `BaseError::ParamInvalid("keyword", "搜索关键词不能为空")`。
- 兼容：非空关键词的通配符转义、长度上限和字段权限校验保持不变。
- 验证：`cargo test -p yang-base --lib test_where_contains_rejects_blank_keyword`；`cargo test -p yang-base --lib where_like`。

## 2026-07-07 - yang-db SQL 生成拒绝空 IN 条件

- 范围：`crates/yang-db/src/mysql/query_builder.rs` 与 `crates/yang-db/src/postgres/query_builder.rs`。
- 风险：底层 `QueryBuilder::where_in` 保持历史 infallible builder 形态，空列表此前会进入条件树并在 SQL 生成阶段形成非法或无意义的 `IN ()`。
- 修改：MySQL 与 PostgreSQL 的 `SqlGenerator` 在构建 WHERE/HAVING 前递归校验条件树，遇到空 `Condition::In` 返回 `DbError::InvalidArgument`；`try_to_sql` 暴露真实错误，兼容的 `to_sql` 仍会降级为不可执行哨兵。
- 兼容：`where_in` 方法签名不变；非空 IN、非法表名、缺少 GROUP BY 的错误行为不变。
- 验证：`cargo test -p yang-db --lib test_try_to_sql_rejects_empty_in_condition`；`cargo test -p yang-db --lib test_try_to_sql_surfaces`。

## 2026-07-07 - yang-db SQL 生成拒绝空布尔条件组

- 范围：`crates/yang-db/src/mysql/query_builder.rs` 与 `crates/yang-db/src/postgres/query_builder.rs`。
- 风险：底层条件树允许 `Condition::And(vec![])` / `Condition::Or(vec![])` 进入 SQL 生成，历史渲染可能退化为恒真/恒假片段，尤其恒真组会让调用方误以为存在有效 WHERE。
- 修改：MySQL 与 PostgreSQL 的 `SqlGenerator` 递归条件校验新增空 AND/OR 组拒绝，统一返回 `DbError::InvalidArgument`。
- 兼容：非空布尔条件组和空 IN 拒绝逻辑保持不变。
- 验证：`cargo test -p yang-db --lib test_try_to_sql_rejects_empty_boolean_condition`；`cargo test -p yang-db --lib test_try_to_sql_rejects_empty_in_condition`。

## 2026-07-07 - yang-db checked 条件转换拒绝空条件

- 范围：`crates/yang-db/src/mysql/condition.rs` 与 `crates/yang-db/src/postgres/condition.rs`。
- 风险：`condition_to_sql_owned_checked` 是返回 `Result` 的安全转换入口，但此前仍会把空 IN 折叠为 `1 = 0`、空 AND 折叠为 `1 = 1`、空 OR 折叠为 `1 = 0`，与 checked API 的显式错误语义不一致。
- 修改：MySQL 与 PostgreSQL 的 checked 条件转换对空 IN/AND/OR 返回 `DbError::InvalidArgument`；legacy `condition_to_sql` / `condition_to_sql_owned` 继续保持原常量折叠兼容行为。
- 兼容：只收紧 checked API；非空条件和 legacy 空 IN 渲染保持不变。
- 验证：`cargo test -p yang-db --lib test_checked_rejects_empty`；`cargo test -p yang-db --lib test_condition_in_empty`。

## 2026-07-07 - yang-db SQL 生成拒绝非法条件字段标识符

- 范围：`crates/yang-db/src/mysql/query_builder.rs` 与 `crates/yang-db/src/postgres/query_builder.rs`。
- 风险：`try_to_sql()` 是显式错误面，但此前 `SqlGenerator::validate_condition` 只拒绝空 IN/空布尔组，未校验 WHERE/HAVING 叶子条件字段；非法字段名可继续进入 legacy 条件渲染路径。
- 修改：MySQL 与 PostgreSQL 的条件树校验新增叶子字段标识符校验，使用各自方言的 `quote_identifier` 判断合法性；`field()`/`group()`/`order()` 的可信表达式入口保持不变。
- 兼容：合法条件字段、非法表名、缺少 GROUP BY、空 IN 和空布尔组的既有错误行为保持不变。
- 验证：`cargo test -p yang-db --lib test_try_to_sql_rejects_invalid_condition_identifier`；`cargo test -p yang-db --lib test_try_to_sql_surfaces`；`cargo test -p yang-db --lib test_try_to_sql_rejects_empty`；`cargo test -p yang-db --lib where_and`。

## 2026-07-07 - HTTP 请求 query 参数空 key 校验

- 问题：`RequestBuilder::query` 允许空白 query key 进入发送阶段，与 `action::Request` 的空 key 处理不一致，也会把明显无效的调用方输入推迟到网络层暴露。
- 修改：`RequestBuilder::send` 在解析 URL 和发送网络请求前扫描 query 参数名，发现空白 key 时返回 `BaseError::ParamInvalid("query", ...)`。
- 验证：先新增 `test_send_rejects_blank_query_key_before_network` 并确认失败，再实现前置校验，随后运行该单测确认通过。
