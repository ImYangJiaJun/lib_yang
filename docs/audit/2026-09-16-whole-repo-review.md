# 全仓库代码审查报告 — lib_yang

- **日期**: 2026-09-16
- **范围**: 5 个 crate（`yang-base`、`yang-base-derive`、`yang-db`、`yang-pcg`、`yang-runtime`），约 305 个生产源文件、约 9.2 万行 Rust
- **方法**: 委托式审查（OCR 只负责文件选择与规则解析，审查由 6 个并行只读子代理按区域完成），规则覆盖所有权/生命周期、错误处理与 panic、`unsafe` 边界、并发/异步、性能、类型/API 设计、宏、安全敏感代码，并叠加 `AGENTS.md` 反模式
- **结论摘要**: **10 个严重问题 + 54 个中等问题 = 64 项**；另有若干误报澄清与表现良好的方面。**本轮未做任何代码修改。**

---

## 一、严重问题（High）

### H1 — 上传文件预览存在存储型 XSS
- **位置**: `crates/yang-base/src/transport/axum.rs:1010, :1074`
- **问题**: `Preview` 以 `text/html` / `image/svg+xml` 内联（`Content-Disposition: inline`）返回上传文件内容；`sanitize_filename` 保留了扩展名，且整个 crate 未设置 `X-Content-Type-Options: nosniff` 或 CSP。任何「预览上传文件」的 Action 都会在应用源上构成存储型 XSS。
- **建议**: 至少加 `X-Content-Type-Options: nosniff` + `Content-Security-Policy: sandbox; default-src 'none'`；预览优先用 `text/plain` / `application/octet-stream`。

### H2 — 上传临时目录信任边界可被伪造
- **位置**: `crates/yang-base/src/action/upload.rs:33`（`copy_to` 校验在 :68）
- **问题**: `temp_root` 是客户端可反序列化的 `#[serde(default)] Option<PathBuf>` 字段。构造体如 `{"path":"/etc/passwd","temp_root":"/"}` 可反序列化成「可信」实例，`canonicalize()`/`starts_with` 校验直接通过，导致任意本地文件拷贝。框架自身路径已被构建期校验挡住，但任何把客户端对象重新反序列化为含 `UploadedFile` 结构的 Action 会重新打开此洞。
- **建议**: 停止信任反序列化标记——把请求级临时根放进 `RequestContext`（传输层注入）并据此校验；或将 `temp_root` 设为不可从客户端反序列化（密封构造 + 自定义 `Deserialize` 恒返回 `None`）。

### H3 — 附件下载全量读入内存、无大小上限
- **位置**: `crates/yang-base/src/transport/axum.rs:987`
- **问题**: `file_response` 用 `tokio::fs::read(path)` 全量读入后交给 `Body::from`，1 GB 附件就是每请求 1 GB 分配，全 crate 无任何流式处理。
- **建议**: 用 `tokio_util::io::ReaderStream` + `Body::from_stream` 流式返回，`Content-Length` 取自 `metadata().len()`，并拒绝超过可配置上限的文件。

### H4 — 文件不存在时泄露服务器绝对路径
- **位置**: `crates/yang-base/src/transport/axum.rs:992`
- **问题**: `BaseError::RecordNotFound(format!("文件不存在: {}", path.display()))` 映射为 404 而非 5xx，`error_response` 会把含完整服务器路径的原始消息回给客户端。
- **建议**: 路径经 tracing 记日志，对外只返回通用「文件不存在」。

### H5 — JOIN 表名未经标识符校验直达 SQL（SQL 注入隐患）
- **位置**: `crates/yang-db/src/mysql/query_builder/builder.rs:437,454,471`；PG 对应 `postgres/query_builder.rs:1362,1378,1394`
- **问题**: `join`/`left_join`/`right_join` 用 `format!("`{}`", table.as_str())` 拼接，`build_joins` 原样 `push_str`；而 `build_select/insert/update/delete/drop_table` 均在渲染时经 `quote_identifier(...)?` 重新校验。参数类型 `&TableRef` 文档声称「构建期校验完成」，但 `TableRef::__from_validated_owned`（`reference.rs:34`）是 `pub` 且不做任何校验，且已在运行时被 `yang-base/src/table/relation_loader.rs:29` 调用。含反引号的表名可注入任意 SQL 到 JOIN 子句，使 `BackendCapability::Join` + `SafetyConstraint::CheckedIdentifiers` 的契约失效。当前仓库内调用方传的都是已校验名，故为**潜伏 sink 而非现网漏洞**——但这是唯一无校验的表名路径。
- **建议**: 让 `quote_identifier(table.as_str())?` 成为 JOIN 表名来源（`join*` 返回 `Result`，与 `where_expr` 一致），或让 `TableRef::__from_validated_owned` 返回 `Result` 并校验。

### H6 — 写路径误跑读投影门禁，write-only 角色 INSERT 被拒
- **位置**: `crates/yang-base/src/table/table_query/plan.rs:100-112`（经 `write.rs:56,113,296,486,492,515,520`）
- **问题**: `apply_db_plan` 无条件解析读投影 `default_read_fields()`；当角色无任何「非隐藏且可读」字段时返回 `FieldPermissionDenied(_, "*", "当前角色没有可读字段")`。`insert/insert_returning_id/update/delete` 及其 `_in_tx` 变体都经此路径，导致合法的 write-only 配置（如字段全部 `writable_by(["ingest"])`）在 INSERT 时被读权限错误拒绝。写路径上该投影不仅是多余门禁，更是死计算（`yang-db` 的写只用 `table/field_types/expr_assignments`）。
- **建议**: 拆分 plan 编译器——写路径只构建查询构建器 + WHERE/租户/软删守卫；`count`（确会泄露表基数）保留 `ensure_readable_projection`。

### H7 — 树构建按深度无限递归 → 栈溢出
- **位置**: `crates/yang-base/src/table/tables.rs:250-272`
- **问题**: `build_tree` 逐层递归 `build_node`，唯一限制是**节点数**（`DEFAULT_TREE_MAX_NODES = 10_000`），不是深度。1 万节点线性链即约 1 万层嵌套帧，每帧持有 `TableTreeNode`/`Vec`/`Result` 临时量，在 release 下可撑爆 2 MiB 的 tokio worker 栈，debug 更糟；`ViewSpec::max_nodes` 仅 `Some(0)` 才拒绝，View 还能任意抬高上限。
- **建议**: 改成显式工作栈循环，或在节点上限之外加深度上限。

### H8 — PCG 缓存键遗漏 constraints，命中返回错误地图
- **位置**: `crates/yang-pcg/src/cache/key.rs:31-39`
- **问题**: 缓存键为 `schema_version:algorithm_version:seed:config_digest:scope`，其中 `config_digest` 只哈希 `GenerationConfig`；但 `GenerationRequest.constraints` 会实质性改变输出（`apply_room_constraints` 改写 room_type→terrain 策略、spawn 规则、敌人预算、保留区；`apply_spawn_constraints` 删除 spawn 点）。相同 seed+config、不同 constraints 的两请求得到相同 key、不同地图，`ResultCache` 静默返回错误的楼层。
- **建议**: 把 constraints 的哈希纳入 `CacheKey`（如 FNV-1a 序列化 `&[Constraint]`，仿照 `ConfigDigest`），或让 `for_full_floor` 接收 `&GenerationRequest`。

### H9 — Boss 保留区半径下限 2，小房间外墙环被吃掉
- **位置**: `crates/yang-pcg/src/terrain/carve.rs:124`（及 :143-154）
- **问题**: `radius: (width.min(height) / 4).max(2)`、`center = (width/2, height/2)`；当 `w == 5` 时 `dx = -2, dy = 0` 已满足 `dx²+dy² ≤ 4`，第 0 列（及远端列/行）变成 `Reserved`。`Reserved` 在 `terrain/grid.rs:15` 与 `validation.rs:308` 中是可通行的，于是房间外墙变成通向室外的门洞，且验证层不拦截。当 `room_size.min_width/min_height < 8`（config 校验只要求 ≥ 4，套件最小房间 6×6）即可达。
- **建议**: 半径夹到内部，如 `radius = ((width.min(height) - 3) / 4).max(1)`，并额外跳过边界环上的圆格（`x==0||y==0||x==w-1||y==h-1`）。

### H10 — `Radio` 参数对原始文本做 JSON 解析，String/枚举型永远解码失败
- **位置**: `crates/yang-base-derive/src/params.rs:243-247`
- **问题**: 其他 builder 都是把原始文本直接映射（`Str/Text/Decimal/Password → String`、`Int/... → parse::<i64>()`、`Switch → bool`），唯独 `Radio` 走 `serde_json::from_str::<#value_type>(raw)`。对 `Radio::<i8>` 可行（`"5"` 是合法 JSON），但对 `Radio::<String>`/`Radio::<UserStatus>`（受支持的 VARCHAR-Radio 形式，见 `Radio::varchar`、`project/yang-system/src/addon/account/user/table.rs:114`），未加引号的 query 值不是合法 JSON，`?status=active` 恒失败。这也与框架自身契约矛盾（`definition/openapi.rs:234-252` 生成的 schema 是 `{"type":"string"}`）。当前只用到 `Radio::<i8>`，但对整类值类型是错误代码生成。
- **建议**: 按声明值类型选解析策略（string/enum → `serde_json::from_value::<T>(Value::String(raw.clone()))`；numeric/bool 保留文本解析），或先试原始 token 再回退 JSON；补 query/path `Radio` 的 trybuild/ui 用例。

---

## 二、中等问题（Medium，54 条）

### 认证 / Token（6）
- **M1** `action/auth/logout.rs:75`（:34 `public`、:90 `revoke_by_subject`）— `LogoutAction` 为 `public`，「只能撤销自己的 Token」检查只在 `if let Some(bearer_token)` 内；匿名调用方凭任一签名有效未过期 token 即可触发 `revoke_by_subject` 终结整个 subject 的会话，未证明所有权。→ 要求必带 bearer 做所有权比对（匿名分支只按 `jti` 撤销单 token），并对 `input.token` 做 Access 类型检查。
- **M2** `token/revocation.rs:223-225` — subject watermark TTL 只绑 `refresh_token_expiry`；当 `access_token_expiry > refresh_token_expiry` 时，撤销前签发的 access token 会活得比 watermark 久，key 过期后「复活」已撤销会话。→ 构造时校验 `access <= refresh`，且/或 watermark TTL 取 `max(access, refresh)`。
- **M3** `action/step_up.rs:300` — `StepUpMiddleware::new` 默认用进程内 `InMemoryStepUpProofStore`；多实例部署忘记 `with_proof_store(RedisStepUpProofStore)` 时，实例 A 消费过的单次 proof 可在实例 B 重放。→ 让 proof store 成为必填构造参数，或默认 Redis、显式 opt-in 内存。
- **M4** `action/auth/logout.rs:17-19` 及 `dto.rs:106-114` — `LogoutInput.refresh_token` 是死字段，handler 从不读取；文档/DTO 却声称会一并拉黑。→ 删字段改文档，或真正黑名单 refresh 的 `jti`。
- **M5** `token/manager.rs:232-234` — `new_symmetric` 在非 HMAC 算法上 `.expect` 崩溃（启动期）；`new_symmetric_keyring` 对同一条件返回 `Err`。→ 换成 `validate_hmac_algorithm(algorithm)?`。
- **M6** `token/manager.rs:755-768` — `refresh_access_token` 是公开、非轮换的刷新路径，旧 refresh token 仍完全可用，重新引入 `rotate_refresh_token` 要堵的重放窗口（未接入 `RefreshAction`，仅 `#[ignore]` 测试用）。→ 弃用并指向 `rotate_refresh_token`，或改成轮换。

### HTTP / Transport / Action（11）
- **M7** `transport/axum.rs:282` — 与框架自身健康端点路由冲突时 panic 而非 `Err`（`validate_routes` 只去重 Action 间路径）。→ 在构建校验里保留 `/health/live`、`/health/ready` 并拒绝。
- **M8** `transport/axum.rs:473` — 204/304 仍按 `(status, Json(response))` 构建，附带 JSON body。→ 1xx/204/304 时返回空 body 并去掉 `Content-Type`/`Content-Length`。
- **M9** `transport/axum.rs:964` — `ResponseBody::Redirect` 接受任意 `Location`，无同源/相对约束，开放重定向。→ 写头前要求相对路径或显式白名单。
- **M10** `http/request.rs:664` — 重试循环对传输错误无条件重试，包括非幂等 POST/PATCH/DELETE（可能双重扣款/插入）。→ 默认仅重试幂等方法，body 请求需显式 opt-in。
- **M11** `http/request.rs:677` — 退避无总预算、无抖动，`max_retries=10, backoff_ms=60_000` 可在 handler 内睡数小时。→ 加总预算 deadline + 抖动。
- **M12** `http/response.rs:130` — `text()/bytes()/json()` 无大小上限全量缓冲上游 body，恶意上游可耗尽内存。→ 加 `max_response_bytes`，查 `Content-Length`，流式读 + 运行中上限。
- **M13** `http/circuit_breaker.rs:148` — 每 host 状态 map 无界（只在成功路径移除）。→ 加 LRU/`retain` 清理或 host 数上限。
- **M14** `http/client.rs:228` — 出站客户端无 SSRF 防护、无重定向策略控制（可打 `169.254.169.254` 等内网）。→ 暴露 `redirect(Policy)` 与可选 `deny_private_networks`。
- **M15** `action/context.rs:147` — 手写 `Clone for ActionContext` 静默丢弃 `request_context` 与 `resource_guards`（上传临时目录被删，后续 `copy_to` 失败）；`router/middleware.rs` 仍称 ctx 故意不 `Clone`，文档已失效。→ 移除 `Clone`（或正确克隆 guard 与类型化值）并更新契约。
- **M16** `transport/axum.rs:266` — `decoder.clone()` 每请求深拷贝 Action 的 JSON Schema（multipart 路径）。→ 解码器持 `Arc<Value>`。
- **M17** `transport/axum.rs:530` — 所有 `to_bytes` 失败都报 413「请求体过大」，掩盖真实 IO/中止错误。→ downcast `LengthLimitError` 区分 413 与 400。

### Definition / Table / Schema-sync / Plugin（10）
- **M18** `table/validator.rs:24,28,34` — 进程级 `OnceLock` 单例（`EMAIL_REGEX`/`PHONE_REGEX`/`REGEX_CACHE`），违反「禁止进程级全局单例」；`REGEX_CACHE`（:331-364）是无界、永不淘汰的 `HashMap<String, Regex>`，key 来自 `Validator::Regex`（可反序列化）。→ 移入 `Tools` 资源槽或加 LRU + 上限。
- **M19** `database/schema_sync/inspect.rs:94-107` — 函数索引（MySQL 8.0.13+）使 `COLUMN_NAME` 为 NULL，解码进非可选 `String` 抛 `ColumnDecode`，`load_existing_schema` 以晦涩错误中止启动。→ 解码为 `Option<String>` 并跳过 NULL 行。
- **M20** `definition/ui/catalog.rs:44,55,66` — UI catalog 每请求序列化+哈希 3 次（前两次摘要被丢弃）；`registry.rs:244-250` 每模块重扫 views，O(modules×views)/请求。→ 投影链末端只算一次 revision，views 按 `module_id` 预索引。
- **M21** `definition/field.rs:285-293` — `ValidationSpec::minimum/maximum` 从未服务端强制（`min_length/max_length/pattern` 都映射到了 validator），只进 action 参数与 UI 表单，且无 builder setter。→ 映射为 `Validator::Min/Max` 或明确文档化缺口。
- **M22** `table/definition.rs:1154-1160` vs `schema_sync/render.rs:252-261` — VARCHAR 超过 MySQL 上限时 `build()` 通过、启动时 `ConfigError` 崩溃（`1..=16383` 只在 `render_column` 校验）。→ 在 `validate_field_shape` 强制 `1..=16383`。
- **M23** `schema_sync/render.rs:146-168` — `normalize_check_expression` 在字符串字面量内部也去空白/反引号，`CHECK (a='x y')` 与 `CHECK (a='xy')` 归一化相同，变更被静默跳过。→ 只在引号外归一化（扫描时跟踪引号态）。
- **M24** `table/record.rs:238-248` vs `schema_validation.rs:196-199` — TINYINT 恒解码为 `Bool`，而 `FieldType::Integer` 声明兼容 `tinyint`，声明整型字段返回 true/false 而非数字。→ 两者对齐（Integer 兼容收窄到 ≥ smallint，或除 `tinyint(1)` 外解码为数字）。
- **M25** `table/tables.rs:160-180` + `table_query/filters.rs:523-534` — 树 prefetch `max_nodes` 无上限，`table_tree_view` 透传 `max_nodes+1`，`max_nodes(5_000_000)` 会 `LIMIT 5_000_001` 并全量缓冲后才被节点上限拒绝（正是节点上限要防的 OOM）。→ 构建期夹到硬上限，或流式 + 硬上限。
- **M26** `definition/builder/compile.rs:711-716,739-747` — `build_registry` 每 action 线性扫所有 modules 并重建完整 `TableDefinition`（重复校验+分配），O(A×M) 且进程期保留 A 份副本。→ 预建 `module name → &ModuleSpec` 索引，每表共享一个 `Arc<TableDefinition>`。
- **M27** `plugin/manager.rs:256-263`（对照 :275-311）— `topological_sort` 用未归一化名称且循环只 `log::error!` 后降级，依赖串与注册名不一致（如空白）会断边、in-degree 恒正、`shutdown()` 顺序错。→ 镜像 registry 实现（归一化 key、循环作为错误上报）。

### yang-db（7）
- **M28** `redis/client.rs:77-87` — `RedisConfig` 的 `min_connections`/`max_lifetime`/`test_before_acquire` 三个池参数被静默忽略（deadpool 0.12.3 只有 `max_size/timeouts/queue_mode`），`recycle` 被误当「空闲连接 TTL」（实为回收钩子超时）。→ 删三个无效字段（或标记不支持并停止校验），重写 `idle_timeout` 语义。
- **M29** `mysql/database.rs:533-535`（及 `transaction.rs`、PG 对应）— raw-param 助手 `as_i64() -> else as_f64()`，超过 `i64::MAX` 的 u64（雪花 ID 等）静默绑成 f64，写 NUMERIC/DECIMAL 时舍入。`SqlValue::from(u64)`（`condition.rs:176-187`）已有正确分支，raw-SQL 路径不一致。→ 加 `as_u64` 分支或统一走 `SqlValue`。
- **M30** `mysql/query_builder/generator.rs:215,224,293,301`（PG 对应）— `append_condition` 按值消费，`build_where/having` 深拷贝整个条件树（含 `Vec<SqlValue>`/子查询），注释却称已消除克隆。→ 加借用入口 `render_condition_ref`。
- **M31** `mysql/query_builder/write.rs:246,300,775,865` — `set_expr` 只被 `insert`/`update` 消费，`upsert`/`insert_batch*`/`update_batch` 静默丢弃（`builder.rs:42-52` 文档却声称生效）。→ 四条路径在 `!expr_assignments.is_empty()` 时返回 `InvalidArgument`（fail-closed）。
- **M32** `mysql/query_builder/write.rs:17`（PG :55）— `update_batch` 固定 1000 行，CASE-WHEN 占位符 `2*records*fields+records`，宽表（>~32 列）超协议 65,535 参数上限直接失败。→ 加 `update_batch_with_size` 或按列数推导分块。
- **M33** `mysql/query_builder/builder.rs:173,213,416`（PG 对应）— `CompareOp::Like` 遇非字符串值回退 `format!("{other:?}")`，发出 `` `name` LIKE 'Int(5)' `` 永不匹配的错误模式且 Debug 文本泄露。→ 改 `Result` 返回或加 checked 变体，报 `UnsupportedOperator`。
- **M34** `mysql/identifier.rs:9` — 文档称 `quote_identifier`「对内部反引号加倍转义」，实现实为拒绝（`Identifier::parse` 拒绝任何反引号）；另有 `builder.rs:426,477,488` 等指向不存在方法 `join_on_identifiers/order_identifier/...` 的失效文档链接、`write.rs:192` 关于 NULL 的过时说明、`AGENTS.md:104` 指向已不存在的 `having_cond_unchecked`。→ 澄清「拒绝而非转义」，删/补方法引用，刷新批量与 AGENTS 说明。

### yang-pcg（14）
- **M35** `terrain/maze.rs:157-244` — `connect_doorways_to_maze` BFS 无边界环排除，回溯把路径上 `Wall` 转 `Floor`，会在外墙环上多凿洞。→ 仿照 `carve_orthogonal_path`（:322-326）把 BFS 限制在内部格（`1..w-1`），绝不 `set` 环上。
- **M36** `ue/streaming.rs:37-43` — chunk 归属只按房间中心 `div_euclid`，跨 chunk 房间对相邻 chunk 不可见。→ 按 AABB 覆盖的每个 chunk 归属房间。
- **M37** `chunked.rs:251-260,445-453` — terrain 策略降级路径未跑 spawn 生成，房间变空；全量路径（`terrain/mod.rs:54-67`）同样的降级却会 spawn，同 seed 不同模式结果不一致。→ 降级后补跑公共 spawn 块。
- **M38** `chunked.rs:382-395` — 预算中止静默，`GenerationResult` 无 `partial` 标志，调用方拿到看似完整的截断结果。→ 加 partial 标志或返回 `Err`。
- **M39** `digest.rs:178-182` — `From<&GenerationConfig> for ConfigDigest` 在 NaN 上 `expect` panic（唯一生产 panic 路径）。→ 删除该 `From` 或改 `PcgResult`。
- **M40** `constraint/mod.rs:21-24` — `ExclusionZoneConstraint::exclude_rooms`（默认 true）校验但从未应用，`exclusion.rs:28` 只读 `exclude_spawns`。→ 实现房间过滤，或 `exclude_rooms==true` 时返回 `PcgError::constraint`。
- **M41** `config.rs:21,421,479,703-706`（及 `model/terrain.rs:178-180`）— 多个已校验旋钮生产从不读取：`dead_end_count`、`corridor.max_turns`、`terrain.min_walkable_ratio`、`capability_flags.grammar_support/debug_output`、`AnchorConstraint.target_grid_pos`、`RuntimeContext.focus_position/interest_radius`、`ReservedZone.allow_items/allow_enemies`。→ 标记 reserved/deprecated 或接线关键项。
- **M42** `terrain/open_arena.rs:58-59` — `random_range(1, width-1)` 在 2 格维度 panic，兄弟 `carve.rs:183` 有守卫、这里没有。→ 早期返回（`width<=3||height<=3`）。
- **M43** `layout/doors.rs:15-22` — 每边两次 O(R) 线性 `find`，锚点生成 O(E·R)，`room_count.max=4096` 时约 1600 万次字符串比较；`corridors.rs:22-25` 已用 `HashMap` 解决同样问题。→ 建一次 `HashMap<&str, &Room>`。
- **M44** `backend/topdown/mod.rs:82-87` + `validation.rs:648-653` — 每次生成深拷贝所有 spawn（复制 id/room_id/tag 等字符串）只为传 `&[SpawnPoint]`。→ `validate_spawn_spacing` 改收迭代器/切片引用。
- **M45** `ue/adapter.rs:243-283` — tile 通道导出 O(terrains×rooms)，每 tile 建 `BTreeMap`+`String` key。→ 建一次 `HashMap<&str,&Room>` 并提升 key 复用。
- **M46** `terrain/maze.rs:62` — `summarize_connectivity` 总被 `terrain/mod.rs:77-81` 覆写，是丢弃的重复洪水填充。→ 删本地 summary 与 `force_connect_doorways`。
- **M47** `validation.rs:148-167` — `validate_no_overlap` 是 O(R²) 平铺扫描，`room_count.max=4096` 时约 840 万次 AABB 测试。→ 按 `min.x` 排序扫描或分桶，硬校验不变。
- **M48** `ue/streaming.rs:50-55,72-90` — 为取 `&[Room]` 全量克隆每个房间；无房间有 bounds 时返回倒置哨兵 `min=(i32::MAX,…)` 且无错误。→ 改迭代器引用 + `PcgResult<RoomBounds>`。

### yang-runtime / yang-base-derive（6）
- **M49** `yang-base-derive/src/params.rs:92-106` — 除 `param` 外任意字段属性被透传进生成结构，但解码按 Rust 字段名插 key，`#[serde(rename="userId")]` 生成永不可解码结构（`deny_unknown_fields` 下更糟）。→ 拒绝 `param`/`doc` 外属性，或让外部 key 与重命名同源。
- **M50** `yang-base-derive/src/action.rs:246-249` — 非法 `permission_mode`（如 `"Any"`）静默变 `All`，多权限时把 OR 变 AND，合法调用方得 403 且无构建提示。→ `match` 明确 `all/any`，其他分支 `syn::Error::new_spanned(...).into_compile_error()`。
- **M51** `yang-base-derive/src/params.rs:119` — `Source::Body => unreachable!()` 是 derive 路径唯一 panic（当前死代码、不会触发，但一旦 guard 与 match 解耦就变编译器中止）。→ 去掉该分支，用周边 `if/else` 或返回 spanned error。
- **M52** `yang-base-derive/src/action.rs:286`（及 `params.rs:146-150,179`）— 展开硬编码 `::schemars::`/`::serde::`/`::serde_json::`，`yang-base` 未 re-export，消费方须自声明且大版本不一致时报晦涩错误。→ `yang-base` 加 `#[doc(hidden)] pub mod __private` re-export 并改用之。
- **M53** `yang-runtime/src/observability/telemetry.rs:263-271` — not-ready 响应硬编码错误码 `900001` 并重实现 `yang-base` 已有的响应信封（`transport/axum.rs:390`、`error/mod.rs:628`），两 crate 重复、分类变更即失同步。→ 统一到一个构造函数或共享 `pub const NOT_READY_CODE`。
- **M54** `yang-runtime/src/shutdown.rs:64,136,138` — shutdown 指标硬编码 `yang_runtime_shutdown_*`，绕过 `RuntimeMetricNames` 间接层（`logging.rs:96-108`），同一进程内前缀割裂、按前缀建的告警无 shutdown 覆盖。→ 把命名集合线程进 `ShutdownBudget::new`。

---

## 三、误报澄清

- **代码库实际没有 `unsafe` 块。** 全 crate `unsafe {`/`unsafe fn`/`transmute`/`from_raw`/`get_unchecked`/`set_len`/`as_ptr` 命中为零；`schema_sync/plan.rs` 与 `token/manager.rs` 里的 `unsafe` 命中分别是局部变量名 `unsafe_issues` 和 sqlx 的 `try_get_unchecked`（非 Rust `unsafe`，且已被 `type_name == "DECIMAL"` 正确门控）。`unsafe_code` 为 `deny`。
- **`#[derive(TableEntity)]` 并不存在** —— `yang-base-derive` 只有 `Action` derive 与 `params!`；`AGENTS.md:19` 仍写着 `#[derive(Action)]/#[derive(TableEntity)]`，是过时描述（docs 契约测试只查 README/docs/VERSIONING/BACKLOG，不查 AGENTS.md，故该行存留）。
- **`yang-runtime` 无信号处理** —— 信号接线/处理顺序在 `project/yang-system/src/bootstrap.rs`（不在审查范围），`ShutdownBudget` 只是截止时间预算。

---

## 四、表现良好的方面

- **认证硬规则成立**：所有授权路径（`TokenAuthMiddleware`、`RefreshAction`）都走 `verify_token_checked`；`verify_token` 仅用于被撤销 token 本身；`verify_token_checked` 失败关闭（`RedisNotInitialized`/`TokenRevocationStateInvalid`），无静默降级，且有测试钉住。
- **密码学健壮**：单算法白名单（无算法混淆）、HMAC 密钥强制 ≥32 字节、kid 校验、`Debug`/错误/日志全部手工脱敏、TOTP 常数时间比较、Argon2 走 `spawn_blocking`+信号量+`OsRng`+等时哑哈希。
- **客户端 IP 信任链不可伪造**：CIDR 拒绝 `/0`、v4 映射 v6 归一化、右到左走到首个不可信跳即停、header 字节/跳数上限、畸形输入回退 TCP 对端。
- **yang-db 值侧参数化统一且强制**：所有叶子值经 `Dialect::push_param`，标识符经 `QualifiedIdentifier` 解析+方言引用，空 `IN/AND/OR` fail-closed，`render_condition` 先入本地缓冲、失败不留脏参数（有单测钉住）。
- **PCG 确定性契约钉死**：`StableRng::derive` 是 `(seed, label)` 的纯函数，稳定性测试锁住 hash/流锚点，debug 流证明隔离；生成端欧氏距离 ≥ 校验端曼哈顿距离，生成器不可能产出校验拒绝的违规。
- **schema-sync 保守**：advisory lock 双路径正确 `GET_LOCK/RELEASE_LOCK`、重命名拒绝新旧列对歧义、非空表拒绝 required/auto-increment、FK 二阶段应用、preflight 全程参数化/标识符引用。
- **proc-macro 错误处理基本 panic-free**：除 M51 的死 `unreachable!()` 外，所有非法输入都走 `syn::Error::new_spanned`/`darling`/`into_compile_error`，trybuild 夹具证明 span 落在用户 token 上。

---

## 五、修复优先级建议（供决策，非自动执行）

1. **先修安全类 High**：H1（存储型 XSS）、H2（上传信任边界）、H5（SQL 注入 sink）——这三项直接影响安全边界，但修复都涉及「行为/契约变更」，需你拍板具体方案。
2. **再修确定性/数据正确性 High**：H8（缓存键漏 constraints）、H6（写路径误读门禁）、H9（boss 区吃墙）、H10（Radio 解码）、H7（栈溢出）、H3（内存）、H4（路径泄露）。
3. **中等问题按域批量处理**：认证语义（M1-M3）、HTTP 硬上限（M12/M13/M14/M11/M10）、yang-db 正确性（M29/M31/M33/M32）、PCG 正确性（M35/M36/M37/M38/M40）。
4. 每批修复后跑 `python scripts/run_ci.py full`（含 clippy `-D warnings` 与 feature 矩阵），涉及 DB 行为再跑 `integration`。

> 本报告为只读审查结果，未对仓库做任何改动。清单中的行号以审查当时为准，动手前请按当前代码复核。
