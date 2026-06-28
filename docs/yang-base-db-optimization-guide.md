# yang-base / yang-db / yang-base-derive 优化对照指南

> 生成于 2026-06-27 reaudit。本文件由 7 路镜头评审 + 对抗式核验整合而成，作为后续优化时的**逐条对照清单**。
> 配合使用：`docs/BACKLOG.md`（带 ✅/🟨/⏳ 状态的既有跟踪表，是"某项是否已修"的真相源）、各模块 `AGENTS.md`（hotspot 与 anti-pattern）。
> 范围：`crates/yang-base`、`crates/yang-db`、`crates/yang-base-derive`。`yang-pcg` 不在本轮范围。

---

## 0. 总览

- **保留优化项**：76 条（已剔除核验为非问题/会引入回归的项）。
- **主题分布**：

| 主题 | 条数 |
|---|---|
| 认证与令牌安全 | 11 |
| 逻辑正确性与数据访问 | 5 |
| 查询构建 DoS 防护与 MySQL/PG 抽象 | 6 |
| 错误分类与可观测性语义 | 4 |
| 性能与分配 | 16 |
| API 与 SemVer 演进 | 18 |
| 插件系统 | 5 |
| 测试与可测性 | 11 |

### 核验后被剔除 / 修正的项（透明记录）

- **❌ 已驳回（核验为非问题）**：`dispatch()` 每次请求 O(#actions) 线性扫描安全守卫（PERF-1 / 质量 Q-5 / 架构 ARCH-11，三镜头同一问题）。`module_router.rs:345` 完整条件是 `self.middlewares.is_empty() && self.actions.values().any(...)`，`&&` 短路使右侧扫描**仅在零中间件的异常配置下**执行；任何挂载了认证中间件的生产路由器中该扫描从不运行。**不要把它当性能阻断处理。** 若仍想做，仅作为 NIT 级清理（加 `has_private_actions: bool` 字段）。
- **🔧 已用核验者修正方案替换**（原方案会引入回归）：AUTH-2(SEC-2)、QRY-4(ARCH-2)、AUTH-11(ARCH-4)、API-2(SEMI-2)、TEST-2(T-SPAN-1)。各条目下用「⚠️ 修正」标注。

### 阅读说明

- 每条含：`id` / 严重度 / 优先级 / effort / 是否 breaking / 位置 / 现状 / 更优解 / ✅ 验收勾选项。
- 严重度采用核验后的 `adjustedSeverity`；未单独核验的低危项标注 `confidence`，落地前请自行用 codegraph/Read 复核 file:line。
- effort：S（<半天）/ M（1–2 天）/ L（数天）/ XL（跨 crate 大重构）。
- 标注「已跟踪」的项命中 `BACKLOG.md` 既有条目，本处仅给更优重构方案，**不重复登记为新阻断**。

---

## 1. 高杠杆速览矩阵（优先级 × effort）

> Top 项：先做"高优先级 × 低 effort"，再排"高价值 × 大重构"。

| 优先级＼effort | S（快） | M（中） | L/XL（大重构） |
|---|---|---|---|
| **P0/P1 阻断** | AUTH-1（token_type 中间件校验）· AUTH-3（verify_token 误分类）· LOGIC-1（path_param 数值恒失败）· PLUG-1（插件注册 TOCTOU）· ERR-1（is_client_error 漏 NotFound）· API-1（4 个配置类型补 non_exhaustive）· API-6（DatabaseConfig builder 补全）· AUTH-6（token_type 改枚举）· TEST-1/3/4（慢查询 warn / Observability 单例 / RequestId 中间件测试） | AUTH-2（轮转原子化，修正）· LOGIC-2（软删除字段写权限）· PERF-2（User 权限 HashSet）· TEST-2（span 字段断言，修正） | QRY-3（Dialect trait 去重）· QRY-4（TableQuery 与 QueryBuilder 统一，修正为窄方案） |
| **P2** | AUTH-4（Logout 所有权）· AUTH-5（RefreshAction 双重 verify）· AUTH-7（Validator::Url SSRF 文档/SafeUrl）· QRY-1/2（LIKE / In DoS 上限）· LOGIC-5（EXECABORT 枚举匹配）· ERR-2（错误码去重）· PERF-3（quote_identifier 去 replace）· PERF-6（build_where 去 to_vec）· API-3/4（TokenClaims/RedisValue non_exhaustive）· PLUG-2/3/4（拓扑环检测/双轨废弃/回调改 BaseError）· TEST-5/6/7（停机顺序/PG 事务 drop） | AUTH-11（ctx.user 封装，修正）· QRY-5/6（SqlParam→SqlValue / QueryBuilder 去生命周期）· PERF-5（condition_to_sql 借用遍历）· API-7（TokenManager builder）· ARCH 服务定位/JSON 往返 | — |
| **P3 / NIT** | 一批分配优化（PERF-7~12）· API ergonomics（ERG-5/6/8、merge 项）· 测试补强（TEST-8~11） | — | — |

---

## 2. 认证与令牌安全

### AUTH-1 — TokenAuthMiddleware 缺 token_type 校验，Refresh Token 可直接鉴权
- **严重度 HIGH · P0 · effort S · 非 breaking**
- 位置：`crates/yang-base/src/action/auth.rs:647-675`（`TokenAuthMiddleware::handle`，注入用户在 671-674）
- 现状：`verify_token_checked` 通过签名+过期+黑名单后直接 `build_user`，**未检查 `claims.token_type == "access"`**。Refresh/Access 同密钥同 iss/aud，Refresh Token 可无障碍通过所有受保护端点。
- 更优解：在 671 行 `build_user` 前插入 `if claims.token_type != "access" { return Err(BaseError::TokenTypeInvalid("期望 access token".into())); }`。用 `TokenTypeInvalid`（400006，已存在）而非 `Unauthorized`，与 RefreshAction 的现有模式一致。不影响 Refresh/Logout（它们自行校验）。
- ✅ 验收：用一枚有效 Refresh Token 请求挂载该中间件的端点，返回 `TokenTypeInvalid`；新增该用例测试。

### AUTH-2 — Refresh Token 轮转非原子，并发双重使用可一换二
- **严重度 HIGH · P1 · effort M · 非 breaking**
- 位置：`crates/yang-base/src/token/revocation.rs:75-84`（`revoke_claims`，SETEX 幂等写）+ `manager.rs:549-569`（`rotate_refresh_token`）
- 现状：轮转步骤 1（verify→读黑名单）与步骤 3（revoke→SETEX 写）之间无原子保证；并发同 token 双请求均通过读检查、均写入、均产出新 token 对。
- ⚠️ **修正方案**（原"把 `revoke_claims` 整体改 SET NX EX"会破坏 `revoke_token` 登出的幂等契约——核验否决）：仅在轮转路径单独走 compare-and-swap，**不改 `revoke_claims` 的 SETEX 语义**。新增私有 `try_revoke_once(jti, ttl) -> bool`（`SET key val NX EX ttl`，需先给 `GlobalRedis` 加 `set_nx_ex`），`rotate_refresh_token` 改用它；返回 false 抛 `TokenRevoked`。登出路径（`revoke_token`）保持幂等 `Ok(())` 不变。
- ✅ 验收：并发两次轮转同一 refresh token，仅一次成功、另一次 `TokenRevoked`；重复登出仍返回 `Ok(())`。

### AUTH-3 — verify_token 用构造函数指针绕过 From 映射，过期 Token 误报 TokenVerifyFailed
- **严重度 HIGH · P0 · effort S · breaking（错误变体变化）**
- 位置：`crates/yang-base/src/token/manager.rs:387-388`
- 现状：`.map_err(BaseError::TokenVerifyFailed)` 把含 `ExpiredSignature` 在内的所有错误统一包成 `TokenVerifyFailed`，而 `From<jsonwebtoken::Error>`（`error/mod.rs:412-422`）本可正确分流到 `TokenExpired`/`TokenVerifyFailed`/`TokenParseFailed`。调用方无法区分"已过期，提示重登"与"签名无效，安全告警"。`verify_token_checked` 继承此缺陷。
- 更优解：改为裸 `?` 或 `.map_err(BaseError::from)`，复用已有 From。同步更新 `manager_test.rs:177/284`、`revocation.rs:61/172` 对 `TokenVerifyFailed` 的过期断言，以及 `# Errors` 文档。
- ✅ 验收：过期 token 返回 `BaseError::TokenExpired`；测试断言三种变体各自命中。

### AUTH-4 — LogoutAction 公开且无所有权校验，任意持有者可强制登出受害者
- **严重度 MEDIUM · P2 · effort S · breaking（选项 A）/ 非 breaking（选项 B）** · confidence high
- 位置：`crates/yang-base/src/action/auth.rs:514-520`（`#[action(public)]`）+ `554-559`（handle）
- 现状：public，从 body 取任意 token 调 `revoke_token`，仅验签名+过期，不校验调用者与 token `sub` 一致。攻击者可使受害者无感登出（可用性 DoS / 静默会话操控）。
- 更优解：**选项 A（推荐）**去掉 public，经 TokenAuthMiddleware 鉴权后校验 `input.token.sub == ctx.user.id_str()`，否则 `PermissionDenied`。**选项 B（非 breaking）**保持 public 但撤销前解析 token 并与请求头 Bearer 的 sub 比对。
- ✅ 验收：A 实现下，用他人 token 登出返回 `PermissionDenied`；自己登出成功。

### AUTH-5 — RefreshAction 双重调用 verify_token_checked，4 次冗余 Redis RTT
- **严重度 MEDIUM · P2 · effort M · 非 breaking** · confidence high
- 位置：`crates/yang-base/src/action/auth.rs:461-471` + `manager.rs:549`
- 现状：`handle` 先 `verify_token_checked`（2 次 Redis 读）取 sub，随后 `rotate_refresh_token` 内部又 `verify_token_checked`（再 2 读）+ revoke（1 写），共 5 次，理论最小 3 次。
- 更优解：新增 `rotate_refresh_token_from_claims(old_claims, custom)`，跳过内部二次 verify，直接 `revoke_claims(old_claims)` + `generate_token_pair`。`handle` 改为 verify→类型校验→resolver→rotate_from_claims。保留旧 API 兼容。与 AUTH-2 协同（共用 try_revoke_once）。
- ✅ 验收：一次刷新 Redis 操作降至 3 次（计数断言或 mock）。

### AUTH-6 — TokenClaims.token_type 用 String，应为封闭枚举 TokenType
- **严重度 MEDIUM · P1 · effort S · breaking（API 签名）** · confidence high
- 位置：`crates/yang-base/src/token/mod.rs`（`token_type: String`），比较散落于 `auth.rs`（`!= "refresh"`）
- 现状：字符串拼写错误（`"Refresh"`）编译期不可见，校验散落易漏更新。
- 更优解：`#[serde(rename_all="lowercase")] enum TokenType { Access, Refresh }`，序列化值与现行字符串一致（已签发 token 仍可反序列化，无需数据迁移）。所有比较改 `== TokenType::Refresh`。与 AUTH-1、API-3 协同落地。
- ✅ 验收：`cargo check` 通过；旧 token 反序列化测试不变；故意拼错值无法编译。

### AUTH-7 — Validator::Url 仅校验协议前缀，形成 SSRF 虚假门禁
- **严重度 MEDIUM · P2 · effort S（文档）/ M（SafeUrl）· 非 breaking** · confidence high
- 位置：`crates/yang-base/src/table/validator.rs:297-311`
- 现状：只判断是否以 `http://`/`https://` 开头，不解析 host、不过滤内网；`http://169.254.169.254/...` 等可通过。
- 更优解：(1) 立即在文档注释标注"仅校验格式、不提供 SSRF 防护、禁止直接用于出站请求"；(2) 增 `Validator::SafeUrl { allowed_hosts }` 解析 host 白名单；HttpClient `with_config` 增可选 `allowed_hosts` 在 `send_once` 前校验。
- ✅ 验收：文档已警告；SafeUrl 对 `127.0.0.1`/`localhost`/链路本地地址返回 `ValidationFailed`。

### AUTH-8 — TokenManager::new_symmetric 未拦截非对称算法
- **严重度 LOW · P3 · effort S · breaking（改 Result）** · confidence high
- 位置：`crates/yang-base/src/token/manager.rs:153-170`
- 现状：接受 RS/ES 等算法不报错，首次签发才失败，难定位。
- 更优解：白名单 `[HS256,HS384,HS512]`，不匹配在构造期返回 `Err(ConfigError(...))`（或 panic 若不便改签名）。fail-fast。
- ✅ 验收：`new_symmetric(..., RS256, ...)` 构造期即报错。

### AUTH-9 — subject_min_iat 水位线解析失败静默 fail-open，无告警
- **严重度 LOW · P3 · effort S · 非 breaking** · confidence high
- 位置：`crates/yang-base/src/token/revocation.rs:151`
- 现状：`raw.parse::<u64>().ok()` 把损坏值转 None，水位线被静默跳过，应失效的 token 重新有效，无任何日志。
- 更优解：保留 fail-open 语义但解析失败时 `tracing::warn!(sub, raw, "水位线解析失败，视为无水位线")`。
- ✅ 验收：写入损坏水位线值后能在订阅者中捕获 warn。

### AUTH-10 — Redis 黑名单 key 中 sub 无字符集约束
- **严重度 LOW · P3 · effort S · 非 breaking** · confidence medium
- 位置：`crates/yang-base/src/token/revocation.rs:34-35`（`subject_min_iat_key`）
- 现状：`format!("token:user:{sub}:min_iat")`，sub 含 `:` 等会产生 key 歧义/超长。
- 更优解：签发前校验 subject 仅含 `[a-zA-Z0-9_:.-]` 且 ≤128；或对 sub 取 SHA-256 前缀做 key 段，固定长度、杜绝特殊字符。
- ✅ 验收：含 `:` 的 sub 被拒签或 key 经摘要化、长度恒定。

### AUTH-11 — ActionContext.user 为 pub 字段，可绕过认证中间件注入伪用户
- **严重度 MEDIUM · P2 · effort S→M · breaking** · confidence high
- 位置：`crates/yang-base/src/action/context.rs:214`（`pub user`）+ `277`（`pub with_user`）
- 现状：任何持 `&mut ctx` 的代码可 `ctx.user = Some(fake)` 绕过校验。
- ⚠️ **修正方案**（原"setter 降为 `pub(in crate::router)`"会破坏 `TokenAuthMiddleware`——它在 `crate::action::auth`，且用直接字段赋值——核验否决）：将字段改 `pub(crate)`、`with_user` 改 `pub(crate)`，并提供只读 `pub fn authenticated_user(&self) -> Option<&User>`。这样 crate 内（含 auth 中间件）可注入、外部 crate 不能篡改。承认下游自定义 Middleware 无法注入认证用户的取舍（外部扩展需另设受控 API）。定位为纵深防御/封装提升，非可直接触发漏洞。
- ✅ 验收：外部 crate 无法构造/赋值 `ctx.user`；crate 内中间件仍可注入；`authenticated_user()` 可读。

---

## 3. 逻辑正确性与数据访问

### LOGIC-1 — path_param<T> 对数值/布尔类型恒返回 ParamInvalid
- **严重度 HIGH · P0 · effort S · 非 breaking** · 核验确认
- 位置：`crates/yang-base/src/action/context.rs:379`
- 现状：`serde_json::Value::String(raw.clone())` 再 `from_value::<T>`，而 i64/bool/enum 的 Deserialize 不接受 JSON String，`ctx.path_param::<i64>("id")` 无论输入是否合法都失败。测试仅覆盖 `::<String>`。
- 更优解：先 `from_str::<Value>(raw).unwrap_or_else(|_| Value::String(raw.clone()))` 再 `from_value::<T>`。`"123"→Number→i64 Ok`，`"true"→Bool`，`"hello"→fallback String`。补 `path_param::<i64>` 单测。
- ✅ 验收：`path_param::<i64>("123")==Ok(123)`、`::<bool>("true")==Ok(true)`、`::<String>` 行为不变。

### LOGIC-2 — 软删除调用 self.update() 触发 FieldPermissionDenied（soft_delete_field 系统只读时）
- **严重度 HIGH · P1 · effort M · 非 breaking** · 核验确认
- 位置：`crates/yang-base/src/table/table_query.rs:2542-2550`（delete/delete_in_tx）→ `2244`（update）→ `2344-2350`（validate_update_data_impl）
- 现状：软删除构造 `{soft_delete_field: ts}` 走 `self.update()`，对该字段做 `can_write(user_roles)` 校验；若字段系统只读则软删除返回 `Err(BaseError::FieldPermissionDenied)`（`table_query.rs:2344-2350`，经 `?` 传回调用方，对 delete 是非直观错误类型，并非静默吞错）。与 `updated_at`（在 build_update_sql 自动追加、绕过写权限）不一致。`del.rs:20-21` 注释已自承这是副作用而非删除授权把关点。
- 更优解：软删除分支直接走 `build_update_sql_impl`（保留 WHERE 守卫、字段存在性检查、updated_at 自动追加），跳过 `validate_update_data_impl` 的用户写权限检查——与 updated_at 自动写入语义对称。delete_in_tx 同理。
- ✅ 验收：soft_delete_field 配置为系统只读时 delete 成功并写入时间戳；WHERE 缺失仍返回 `MissingWhereClause`。

### LOGIC-3 — SelectAction COUNT 在鉴权检查之前执行
- **严重度 LOW · P3 · effort S · 非 breaking** · 核验下调（HIGH→LOW）
- 位置：`crates/yang-base/src/action/builtin/select.rs:144-158`
- 现状：`count_total==true` 时 COUNT 先于 155 行 `ctx.user` 检查执行。核验澄清：路由层 `authorize_and_dispatch`（`module_router.rs:414`）已对非公开 Action 在 dispatch 前校验用户，正常路径 `ctx.user` 必非 None；信息泄露论断无效（仍返回 Unauthorized，count 被丢弃）。仅 `is_public=true` 误注册或直接调 handle 的 edge case 浪费一次 DB RTT。
- 更优解：把 `ctx.user` 检查 + `ensure_fields_readable` 提到 COUNT 块前，COUNT 改用 `table_query_for_user(user)`，与 GetAction 顺序一致。属一致性/效率清理，非安全阻断。
- ✅ 验收：未认证直接调用 SelectAction 不再触发 DB COUNT；正常路径结果不变。

### LOGIC-4 — count_internal 返回 usize（32 位平台截断）+ 与 SelectResult.total 类型不一致
- **严重度 LOW · P3 · effort S · breaking（改公共字段类型）** · confidence high
- 位置：`crates/yang-base/src/table/table_query.rs:1277`（count_internal）；`query_params.rs:600`（PaginatedResult.total: usize）vs `select.rs:98`（SelectResult.total: Option<u64>）
- 现状：i64→usize→u64 双重转换；32 位平台 >2^32 行截断；两个公共 total 字段类型不对称。
- 更优解：`count_internal` 返回 u64（`u64::try_from(count).unwrap_or(0)`）；`PaginatedResult.total`/`total_pages` 改 u64；公开 `count()` 直接委托，去掉冗余 `as u64`。
- ✅ 验收：类型链 i64→u64 直达；分页类型与 SelectResult 一致；32 位编译无截断告警。

### LOGIC-5 — RedisTransaction::exec 用字符串子串匹配 EXECABORT
- **严重度 MEDIUM · P2 · effort S · 非 breaking** · confidence high
- 位置：`crates/yang-db/src/redis/transaction.rs:359`
- 现状：`err_msg.contains("EXECABORT")` 作为 WATCH 冲突备用检测，版本升级/i18n 可能失效。
- 更优解：优先 `matches!(e.kind(), redis::ErrorKind::ExecAbortError)`；若该 crate 版本未定义则退 `e.is_connection_dropped()` 等 API，最后才降级字符串匹配并加注释。
- ✅ 验收：用枚举匹配检测 abort；保留字符串兜底带说明。

---

## 4. 查询构建 DoS 防护与 MySQL/PG 抽象

### QRY-1 — StringWhereOp::Like 无通配符限制也无长度上限（全表扫描 DoS）
- **严重度 MEDIUM · P2 · effort S · 非 breaking** · confidence high
- 位置：`crates/yang-base/src/table/entity.rs:133` + `table_query.rs:1404-1407`（render）/`496`（where_like）
- 现状：用户 JSON 直传 pattern，`%`/`_` 原样进 MySQL；`"%"` 匹配全表，超长 `_` 串触发昂贵回溯。
- 更优解：`where_like` 对 pattern 加长度上限（如 ≤128）超出 `ParamInvalid`；提供 `where_contains(field, literal)` 自动转义 `%`/`_`（`LIKE ... ESCAPE '\\'`）；`FieldConfig` 增 `allow_wildcard_like: bool` 默认 false。
- ✅ 验收：超长 pattern 被拒；`where_contains` 字面量搜索不被通配符污染。

### QRY-2 — WhereCondition::In/NotIn 列表无元素上限（超大参数 DoS）
- **严重度 MEDIUM · P2 · effort S · 非 breaking** · confidence high
- 位置：`crates/yang-base/src/table/entity.rs:127` + `table_query.rs:1396-1402`；对比 `select.rs:133` 已对 page_size 限 1..=100
- 现状：`{"in":[...10万项...]}` 直接生成 10 万绑定参数的 IN 子句，阻塞连接、CPU 峰值。
- 更优解：在 `validate_condition_tree`/`render_condition` 对 In/NotIn 列表加上限（如 ≤500）超出 `ParamInvalid`；可在 TableConfig/FieldConfig 配 `max_in_size`。
- ✅ 验收：超限 IN 列表返回 `ParamInvalid`。

### QRY-3 — MySQL / PG query_builder 重复，缺 Dialect trait 抽象（已跟踪邻域）
- **严重度 MEDIUM · P1 · effort L · 非 breaking** · 核验下调（HIGH→MEDIUM，含失实修正）
- 位置：`crates/yang-db/src/{mysql,postgres}/query_builder.rs`、`condition.rs`、`identifier.rs`
- 现状（核验修正）：PG `query_builder.rs` 实测 **2146 行**（非 5000），真实重复约 **1200–1500 行**（非"8000 行/80%"）。`build_upsert`（mysql:681 vs pg:689）与 `condition_to_sql_owned` 占位符/绑定语义**结构性不同**，非"完全相同"。CLAUDE.md 已明确该文件高耦合、拆分须配套测试。
- 更优解：引入 `Dialect` trait 抽取差异点——**关键**：占位符不是纯格式函数，需 `fn push_param(params:&mut Vec<SqlValue>, v:SqlValue)->String`（MySQL 返回 `"?"`，PG 返回 `format!("${}", params.len())`）；另含 `quote_char`、`build_upsert_suffix`、`bind_json_as_string`、`supports_returning`。`SqlGenerator<D:Dialect>` 单套实现，`pub type PgQueryBuilder = QueryBuilder<PgDialect>` 保持 API 兼容；配集成测试并行回归。
- ✅ 验收：MySQL/PG 共用一套生成器；两方言集成测试全绿；公共 API 不变。

### QRY-4 — TableQuery 自带 SQL 生成器，与 yang-db QueryBuilder 完全不复用
- **严重度 MEDIUM · P2 · effort M（窄方案）· 非 breaking** · 核验确认问题、否决原方案
- 位置：`crates/yang-base/src/table/table_query.rs`（约 2842 行，含 build_select/insert/update/delete_sql，无任何 QueryBuilder/SqlValue 调用）
- 现状：权限校验与 SQL 渲染交织于同一巨文件；yang-db 的 SQL 修复不惠及此路径。
- ⚠️ **修正方案**（原"ValidatedQuery 委托 `db.table().where_condition(c)`"——该 API 不存在；`QueryBuilder::insert/update` 要求 `T:Serialize` 不接受 HashMap；"方言/缓存同步"无依据——核验否决）：**收窄为低风险第一步**——消除 `SqlParam`、改用 `yang_db::SqlValue`（见 QRY-5），统一参数绑定。**在 yang-db 公开提供 `where_condition(Condition)` API 之前，不做 WHERE 构建委托**。完全拆分为 PermissionGate + 委托是合理长期方向，但当前 yang-db 公共 API 不支持、不应强行。
- ✅ 验收：TableQuery 参数类型统一为 SqlValue；不引入对不存在 API 的依赖。

### QRY-5 — SqlParam（6 变体）与 SqlValue（9 变体）平行类型，缺 DateTime/Bytes/Json/Timestamp
- **严重度 MEDIUM · P2 · effort S · 非 breaking** · confidence high（核验确认变体数与盲区）
- 位置：`crates/yang-base/src/table/table_query.rs:2789`（SqlParam）/`2816`（from_json 对 Array/Object 返回 Err）
- 现状：TableQuery 的 WHERE/INSERT 对时间戳/BLOB/JSON 列报错或需字符串绕过，无类型保障。
- 更优解：先把 `SqlParam` 扩到与 SqlValue 同等变体并在 `from_json` 处理（复用 `json_value_to_sql_value` 逻辑）；中期直接用 `yang_db::SqlValue` 替换 SqlParam（QRY-4 的前置）。独立可落地。
- ✅ 验收：DateTime/Bytes/Json 列经 WHERE/INSERT 不再报错。

### QRY-6 — QueryBuilder<'a> 持有 &'a MySqlPool 生命周期，限制可移动/可存储性
- **严重度 MEDIUM · P2 · effort M · breaking** · confidence high
- 位置：`crates/yang-db/src/mysql/query_builder.rs:897-918`（PG 同构）
- 现状：`&'a MySqlPool` 使构建器不能存 Arc、不能跨 await 边界移动；而 MySqlPool 内部已是 Arc，引用无内存收益。
- 更优解：pool 字段改 `MySqlPool`（clone O(1)），去掉 `'a`；`Database::table()` 返回 owned `QueryBuilder`。PG 同步。
- ✅ 验收：QueryBuilder 可存入结构体/跨 await；无性能回退。

---

## 5. 错误分类与可观测性语义

### ERR-1 — BaseError::is_client_error 漏 NotFound 分类
- **严重度 MEDIUM · P1 · effort S · breaking（行为变化）** · 核验下调（HIGH→MEDIUM，修正变体数）
- 位置：`crates/yang-base/src/error/mod.rs:730-731`（is_client_error）/`739-744`（is_server_error）
- 现状：`NotFound` 类别两侧都不含，使 `RecordNotFound/UserNotFound/ActionNotFound/FieldNotFound/TableConfigNotSet`（**5 个**变体，核验更正：`PluginNotFound` 已归 Server）对两个判定均返回 false。HTTP 404 应属客户端错误。注：该行为已在枚举 docstring(mod.rs:430-431) 显式声明，当前生产无调用者。
- 更优解：`is_client_error` 加入 `ErrorCategory::NotFound`；同步更新 docstring 与 `test_error_categories`。
- ✅ 验收：上述 5 个变体 `is_client_error()==true`；测试断言补齐。

### ERR-2 — 两对 BaseError 变体共享同一 error code
- **严重度 MEDIUM · P2 · effort S · breaking（新增子码）** · confidence high
- 位置：`crates/yang-base/src/error/mod.rs:494-495`（200001）/`510-511`（210004）
- 现状：`DatabaseConnectionFailed(String)` 与 `DatabaseConnectionDbError(DbError)` 同 200001；`RedisOperationFailed/RedisOperationDbError` 同 210004，告警/客户端无法区分来源。
- 更优解：为 `*DbError` 变体分配独立子码（如 200011/210011），更新 code()/code_str()/测试/文档。
- ✅ 验收：四个变体 code 两两不同；唯一性测试通过。

### ERR-3 — BaseError 两个语义重叠的数据库连接失败变体
- **严重度 LOW · P3 · effort S · breaking** · confidence high
- 位置：`crates/yang-base/src/error/mod.rs:74-78`
- 现状：`DatabaseConnectionFailed(String)`（无 #[source]，丢错误链）与 `DatabaseConnectionDbError(#[source] DbError)` 并存，match 易漏。
- 更优解：合并为唯一 `DatabaseConnectionFailed(#[source] DbError)`，纯字符串场景用 `DbError::ConnectionError(msg)` 包装；更新 `database/initializer.rs` 等构造点。
- ✅ 验收：仅一个连接失败变体且保留 #[source]。

### ERR-4 — DbError Display 测试漏 8 个新变体
- **严重度 NIT · P3 · effort S · 非 breaking** · confidence high
- 位置：`crates/yang-db/src/error.rs:261-280`（`test_error_display_chinese`）
- 现状：errors 向量仅 11 项，漏 `InvalidArgument/MissingGroupByClause/UnsupportedOperator/Redis* (5)`。
- 更优解：补全至 18 个变体，或改 `strum::EnumIter` 遍历。
- ✅ 验收：Display 测试覆盖全部变体。

---

## 6. 性能与分配

### PERF-2 — verify_token_checked 两次 Redis 读串行（应 pipeline）
- **严重度 MEDIUM · P2 · effort S · 非 breaking** · 核验下调（HIGH→MEDIUM）
- 位置：`crates/yang-base/src/token/revocation.rs:178-185`
- 现状：`is_revoked`(EXISTS) 与 `subject_min_iat`(GET) 串行 = 2×RTT。
- 更优解（核验建议优于原 try_join!）：用已实现的 `GlobalRedis::pipeline()`（`client.rs:173`）把 EXISTS+GET 合一条 pipeline → 1 RTT、单连接。**注意**：现状对已黑名单 token 会短路跳过第二次读，pipeline/try_join 会失去该早返；黑名单 token 稀少，可接受。
- ✅ 验收：一次校验仅 1 次 Redis 往返（pipeline）；有效 token 路径延迟下降。

### PERF-3 — quote_identifier 在校验后仍 replace 分配新 String
- **严重度 MEDIUM · P2 · effort S · 非 breaking** · confidence high
- 位置：`crates/yang-db/src/mysql/identifier.rs:44` / `postgres/identifier.rs:27`
- 现状：`is_valid_identifier` 只许 `[A-Za-z0-9_]`，`replace('`',"``")` 必为空操作却仍分配。
- 更优解：通过校验后直接 `format!("`{ident}`")`；`quote_qualified` 的 ≤2 元素可 match len 避免 Vec。
- ✅ 验收：去 replace 后行为等价；微基准分配减少。

### PERF-4 — User::has_permission/has_role O(n) Vec 扫描在鉴权热路径
- **严重度 MEDIUM · P1 · effort M · breaking（字段类型）** · confidence high
- 位置：`crates/yang-base/src/action/context.rs:61-67`；`check_permissions` 在其上 O(M×N)
- 现状：`permissions/roles: Vec<String>`，每条非公开请求至少一次线性扫描。
- 更优解：改 `HashSet<String>`，has_* 退化 O(1)；提供 `User::new(roles, permissions)`、字段私有化；更新所有 `User{..}` 构造点（多在测试/builtin）。
- ✅ 验收：has_permission O(1)；构造点改用 new。

### PERF-5 — condition_to_sql 借用 API 全树 clone 后转 owned
- **严重度 MEDIUM · P2 · effort M · 非 breaking** · confidence high
- 位置：`crates/yang-db/src/mysql/condition.rs:151` / `postgres/condition.rs:163`
- 现状：`condition_to_sql(&Condition)` 直接 `clone()` 整棵树再递归消费。
- 更优解：新增 `write_condition_to_sql(cond:&Condition, out:&mut String, params:&mut Vec<SqlValue>)` 引用遍历，仅压参时 clone SqlValue；`condition_to_sql` 委托之。MySQL/PG 各一套。是 PERF-6/8/9 的共同基座。
- ✅ 验收：复杂嵌套条件不再深克隆整树；两方言一致。

### PERF-6 — build_where/build_having 多条件时 to_vec() 克隆全部
- **严重度 MEDIUM · P2 · effort S · 非 breaking** · confidence high
- 位置：`crates/yang-db/src/mysql/query_builder.rs:221-222`/`297-299`
- 现状：`Condition::And(conditions.to_vec())` 克隆每个条件，And 分支再 collect Vec<String>。
- 更优解：内联拼接（push ` WHERE (` → 逐条 ` AND ` → `)`），配合 PERF-5 的 write_condition_to_sql；单独改也至少省外层 to_vec。
- ✅ 验收：多条件 WHERE 无外层克隆。

### PERF-7 — Condition::In 占位符用中间 Vec<&str> 构建
- **严重度 LOW · P3 · effort S · 非 breaking** · confidence high
- 位置：`crates/yang-db/src/mysql/condition.rs:213`
- 现状：`vec!["?"; count].join(", ")` 多分配一个 Vec。
- 更优解：预分配 String 直接 push（`with_capacity(count*3)`）。
- ✅ 验收：IN 占位符构建无中间 Vec。

### PERF-8 — condition_to_sql_owned And/Or 分支 collect Vec<String> 再 join
- **严重度 LOW · P3 · effort M · 非 breaking** · confidence high
- 位置：`crates/yang-db/src/mysql/condition.rs:239-243`（And collect）/`255-259`（Or collect） / `postgres/condition.rs:249-253`
- 现状：N 子条件产生 N+1 次 String 分配。
- 更优解：直接写入单 String（配 PERF-5 自然消除）。
- ✅ 验收：N 子条件分配降至常数级缓冲。

### PERF-9 — PG push_placeholder 每参一次 format! 分配
- **严重度 LOW · P3 · effort M · 非 breaking（私有）** · confidence high
- 位置：`crates/yang-db/src/postgres/condition.rs:149`
- 现状：每参 `format!("${}", len)` 产生短命 String。
- 更优解：改返回 `usize` 索引，调用方 `write!(&mut sql, "${idx}")` 直写；配 PERF-5 重构自然落地。
- ✅ 验收：30 参 INSERT 不再产生 30 个临时 String。

### PERF-10 — generate_token_pair 两次独立时间戳系统调用
- **严重度 LOW · P3 · effort S · 非 breaking** · confidence medium
- 位置：`crates/yang-base/src/token/manager.rs:259-353`
- 现状：access/refresh 各调一次 `current_unix_timestamp()`。
- 更优解：新增 `generate_*_token_at(now)`，pair 取一次 now 传入；额外利于测试注入时间。
- ✅ 验收：一次 pair 仅一次时间调用；access/refresh iat 相同。

### PERF-11 — 每次生成 Token 重建 Header 并分配 "JWT" String
- **严重度 LOW · P3 · effort S · 非 breaking** · confidence high
- 位置：`crates/yang-base/src/token/manager.rs:279-281/320-322`
- 现状：每次 `header.typ = Some("JWT".to_string())`。
- 更优解：在 TokenManager 预存 `jwt_header: Header`，生成时 `.clone()`。
- ✅ 验收：Header 构造从签发路径移除。

### PERF-12 — insert_batch_with_size 仅为取 len 把 chunks collect 成 Vec
- **严重度 NIT · P3 · effort S · 非 breaking** · confidence high
- 位置：`crates/yang-db/src/mysql/query_builder.rs:2259`
- 现状：`data.chunks(batch_size).collect::<Vec<_>>()` 仅为判断单批次+迭代。
- 更优解：`chunk_count = data.len().div_ceil(batch_size)` 分支，直接在 `chunks()` 迭代器上处理。
- ✅ 验收：去掉 `Vec<&[T]>` 分配；批量行为不变。

### PERF-13 — table_query() 每次 handle 分配新 Arc<[String]> 角色副本
- **严重度 LOW · P3 · effort S · 非 breaking** · confidence high（架构 ARCH-12）
- 位置：`crates/yang-base/src/action/context.rs:407`
- 现状：`Arc::from(self.user_roles_slice().to_vec())` 每次写操作重新 Arc 化角色。
- 更优解：`with_user()` 时把角色一次性包成 `Arc<[String]>` 缓存（或 User.roles 本身改 `Arc<[String]>`），table_query 直接 `Arc::clone` O(1)。
- ✅ 验收：table_query 不再每次分配角色 Vec/Arc。

### PERF-14 — RedisValue::as_string() 强制堆分配，缺借用版
- **严重度 MEDIUM · P2 · effort S · 非 breaking** · confidence high（质量 Q-6）
- 位置：`crates/yang-db/src/redis/value.rs:39-43`
- 现状：`as_string(&self)->Option<String>` 对内部 String clone。
- 更优解：新增 `as_str(&self)->Option<&str>`，`as_string` 复用之。纯新增。
- ✅ 验收：只读调用方可用 as_str 零分配；as_string 兼容。

### PERF-15 — RequestId::parse_hex 无连字符输入仍 to_string 分配
- **严重度 NIT · P3 · effort S · 非 breaking** · confidence high（质量 Q-9）
- 位置：`crates/yang-base/src/action/request_id.rs:64-66`
- 现状：else 分支 `s.to_string()` 多余分配。
- 更优解：用 `Cow<str>`（含 `-` 走 Owned，否则 Borrowed），非 UUID 路径零分配。
- ✅ 验收：短十六进制 request-id 解析无分配。

### PERF-16 — ActionContext::extract_input 每次 clone 整个请求体 JSON
- **严重度 LOW · P3 · effort M · 非 breaking** · confidence high（质量 Q-10）
- 位置：`crates/yang-base/src/action/context.rs:311`
- 现状：`from_value(self.request.body.clone())` 大 body 时克隆显著。
- 更优解：在 `TypedAction::dispatch` blanket impl 中 `std::mem::take(&mut ctx.request.body)` 后 `from_value`，避免 clone（ctx 已移入 async 块）。仅在 body 已知较大时优先。
- ✅ 验收：dispatch 路径不再克隆 body；现有功能不变。

---

## 7. API 与 SemVer 演进

### API-1 — TableConfig/FieldConfig/IndexConfig/TimestampFields 缺 #[non_exhaustive]
- **严重度 MEDIUM · P1 · effort S · breaking** · 核验下调（HIGH→MEDIUM）
- 位置：`table_config.rs:39`(TableConfig)/`field_config.rs:52`(FieldConfig)/`table_config.rs:505`(IndexConfig)/`544`(TimestampFields)
- 现状：四个高频配置类型全 pub 字段无 `#[non_exhaustive]`，而同仓 `DatabaseConfig`、`FieldType` 等已标。
- 更优解：四处各加 `#[non_exhaustive]`；均已有 builder/库内构造路径，不需新增 constructor。越早加代价越低。
- ✅ 验收：外部 crate 无法 struct literal 构造这四类；builder 链不受影响。

### API-2 — ActionMeta/User/ApiResponse 缺 #[non_exhaustive]
- **严重度 MEDIUM · P1 · effort S（User/ApiResponse）/ M（ActionMeta）· breaking** · 核验确认问题、否决"一行加属性"
- 位置：`meta.rs:9`(ActionMeta)/`context.rs:31`(User)/`response.rs:37`(ApiResponse)
- ⚠️ **修正方案**（核验否决对 ActionMeta 直接加属性）：`User`、`ApiResponse` 可直接加 `#[non_exhaustive]`（crate 内 struct literal 不受限）。**ActionMeta 必须先**在 yang-base 加 `ActionMeta::new(...)`，再改 `yang-base-derive/src/action.rs:112-120` 的宏生成代码改用该构造函数，**最后**才加属性——否则所有 `#[derive(Action)]` 用户编译报 E0639。
- ✅ 验收：三类型加属性后 `cargo check` 全绿；derive 宏生成代码改走构造函数。

### API-3 — TokenClaims/AuthAuditEvent/VerifiedSubject 缺 #[non_exhaustive]
- **严重度 MEDIUM · P2 · effort S · breaking** · confidence high
- 位置：`token/mod.rs`(TokenClaims)、`action/auth.rs`(AuthAuditEvent, VerifiedSubject)
- 现状：JWT claim 持续演进，扩展字段会破坏下游 struct literal / 穷举 match。
- 更优解：三类各加 `#[non_exhaustive]`；补 `TokenClaims::builder()/new`、`AuthAuditEvent::success/failure` 工厂。与 AUTH-6 协同。
- ✅ 验收：加属性后测试改走工厂；新增字段不 break。

### API-4 — RedisValue 枚举缺 #[non_exhaustive]
- **严重度 MEDIUM · P2 · effort S · breaking** · confidence high
- 位置：`crates/yang-db/src/redis/value.rs`
- 现状：RESP3 新类型（Map/Set/Double/Push…）升级时会强制 break 所有下游穷举 match。
- 更优解：加 `#[non_exhaustive]`，下游 `_` arm 落 `Unknown`；可将 `Unknown` 更名 `Unsupported`。
- ✅ 验收：枚举加属性；下游 match 含 `_` 兜底。

### API-5 — PoolStatus/ObservabilityConfig/HttpClientConfig/CircuitBreakerConfig 缺 #[non_exhaustive]
- **严重度 LOW · P3 · effort S · breaking** · confidence high
- 位置：`redis/client.rs`(PoolStatus)、`observability.rs`、`http/client.rs`、`http/circuit_breaker.rs`
- 现状：常见演进字段（idle_count/proxy_url/half_open_max_calls 等）会破坏 struct literal。
- 更优解：全部加属性；`CircuitBreakerConfig`/`PoolStatus` 补 builder 或 `Default` 入口。
- ✅ 验收：四类加属性；提供非字面量构造路径。

### API-6 — DatabaseConfig builder 不完整（max_connections 等缺 with_*）
- **严重度 MEDIUM · P1 · effort S · 非 breaking** · confidence high
- 位置：`crates/yang-db/src/mysql/database.rs:55-83`
- 现状：仅 3 个 with_*，最关键的 `max_connections`/`connect_timeout`/`idle_timeout`/`enable_logging` 须字段直赋；而 `RedisConfig` 已全覆盖；`DatabaseConfig` 又有 `#[non_exhaustive]` 禁字面量，代价更大。
- 更优解：补四个 `with_*` 方法，与 RedisConfig 对称。纯新增。
- ✅ 验收：`DatabaseConfig::default().with_max_connections(20).with_connect_timeout(10)` 可链式编译。

### API-7 — TokenManager::new_symmetric/new_asymmetric 6-7 位置参数，缺 builder
- **严重度 MEDIUM · P2 · effort M · 非 breaking** · confidence high
- 位置：`crates/yang-base/src/token/manager.rs`（new_symmetric/new_asymmetric）
- 现状：6-7 位置参数易传错；`issuer/audience: String` 强制 `.to_string()`；新配置项继续恶化。
- 更优解：引入 `TokenManagerConfig`（issuer/audience: `impl Into<String>` + 各 with_*），`from_symmetric(secret, alg, config)` 仅 3 参；保留旧方法 `#[deprecated]`。
- ✅ 验收：新 API 3 参；旧方法标 deprecated 仍可用。

### API-8 — ModuleRouter::default_permissions 取 Vec<String>（合并 ERG-3 / 质量 Q-12）
- **严重度 LOW · P2 · effort S · 非 breaking** · confidence high
- 位置：`crates/yang-base/src/router/module_router.rs:187`
- 现状：调用方须对每个字面量 `.to_string()`。
- 更优解：改 `impl IntoIterator<Item = impl Into<String>>`，`["a","b"]` 与 `vec![...]` 均可。
- ✅ 验收：`default_permissions(["perm:read","perm:write"])` 编译；旧 Vec<String> 兼容。

### API-9 — with_table_config 与 table_config 重复 setter（合并 ERG-4 / 质量 Q-11）
- **严重度 LOW · P3 · effort S · 非 breaking** · confidence high
- 位置：`crates/yang-base/src/router/module_router.rs:137-167`
- 现状：两等价 pub 方法；`table_config` 与字段同名易误认 getter。
- 更优解：保留 `with_table_config`，`table_config` 标 `#[deprecated(note="请用 with_table_config")]`；文档只示范前者。
- ✅ 验收：`table_config` 触发 deprecated 警告；文档单一。

### API-10 — ModuleRouter builder 方法缺 #[must_use]
- **严重度 NIT · P3 · effort S · 非 breaking** · confidence high（质量 Q-13）
- 位置：`crates/yang-base/src/router/module_router.rs:105-305`
- 现状：`router.with_table_config(cfg);`（忘重绑定）无警告，静默丢配置。
- 更优解：struct 或各 consume-self 方法加 `#[must_use="builder 返回新实例，忽略将丢失配置"]`。
- ✅ 验收：忽略 builder 返回值触发 clippy 警告。

### API-11 — EngineConfig.database_url 与 DatabaseBundle::init(mysql_url) 命名不一致
- **严重度 LOW · P3 · effort S · breaking** · confidence medium
- 位置：`crates/yang-base/src/config.rs`(EngineConfig) + `database/bundle.rs`
- 现状：generic `database_url` + 读 `DATABASE_URL`，未来加 PG 支持时歧义。
- 更优解：`#[cfg(feature="mysql")]` 字段改名 `mysql_url`，环境变量优先 `MYSQL_URL`、fallback `DATABASE_URL`(deprecated)。在加 PG 字段前完成成本最低。
- ✅ 验收：字段/环境变量与 mysql 语义对齐；DATABASE_URL 兼容读取。

### API-12 — Permission 缺 Display / From<&str>
- **严重度 LOW · P2 · effort S · 非 breaking** · confidence high
- 位置：`crates/yang-base/src/action/action_trait.rs`（Permission）
- 现状：`format!("{}", perm)` 不编译，须 `perm.name()`；无法 `.into()` 构造。
- 更优解：实现 `Display`（写 name）+ `From<&'static str>`/`From<String>`。纯新增。
- ✅ 验收：`format!("{perm}")` 与 `"x".into()` 可用。

### API-13 — HttpClient::init_global 只暴露 timeout，与 HttpClientConfig 不对称
- **严重度 LOW · P2 · effort S · 非 breaking** · confidence high
- 位置：`crates/yang-base/src/http/client.rs`（init_global）
- 现状：无法配置 max_connections/UA 等；与 `GlobalDatabase/Redis::init_with_config` 不对称。
- 更优解：新增 `init_global_with_config(HttpClientConfig)`，`init_global` 改为其薄包装。
- ✅ 验收：可经全局初始化配置全部字段。

### API-14 — 权限模型仅 AND 语义，无法表达 OR / 角色组
- **严重度 MEDIUM · P2 · effort S · 非 breaking** · confidence high（架构 ARCH-8）
- 位置：`crates/yang-base/src/router/module_router.rs:419-446/517-521`
- 现状：default_permissions 与 Action 权限均 `.all()`，业务被迫在 handle 内重写 OR 判断。
- 更优解：`TypedAction::permission_mode()->PermissionMode{All|Any}` 默认 All，授权按 mode 选 all/any；Permission 增 `group: Option<&'static str>`（组内 OR、组间 AND）。保留默认值，非 breaking。
- ✅ 验收：Any 模式下满足任一权限即放行；默认行为不变。

### API-15 — #[derive(Action)] 对泛型 Action 每个单态化生成独立空 Permission static
- **严重度 LOW · P3 · effort S · 非 breaking** · confidence high（架构 ARCH-9）
- 位置：`crates/yang-base-derive/src/action.rs:67-88`
- 现状：`AddAction<User>` 与 `<Product>` 各有独立 `PERMS: OnceLock`，空权限也各占一份；10 表×6 内置=60 份。
- 更优解：与 T 无关的常量用关联 `const`/直接返回 `&[]`（空时零分配），非空才 OnceLock。
- ✅ 验收：空权限 Action 不再各占 OnceLock。

### API-16 — TableEntity derive 的 OnceLock<TableConfig> 缺 Send+Sync 静态断言
- **严重度 LOW · P3 · effort S · 非 breaking** · confidence high（架构 ARCH-13）
- 位置：`crates/yang-base-derive/src/table_entity.rs:197-209`
- 现状：未来 TableConfig 引入非 Send+Sync 字段时报难定位的编译错误。
- 更优解：宏展开加 `const _: fn() = || { fn a<T:Send+Sync>(){} a::<TableConfig>(); };`。
- ✅ 验收：TableConfig 失去 Send+Sync 时在宏处给清晰诊断。

### API-17 — param_optional_strict 是 pub + deprecated + dead_code
- **严重度 NIT · P3 · effort S · breaking** · confidence high（NIT-1）
- 位置：`crates/yang-base/src/action/request.rs`
- 现状：已知无用、将删的方法仍 pub，增文档噪音。
- 更优解：降 `pub(crate)`（或若无 crate 内调用直接删）。
- ✅ 验收：不再出现在公共 API 文档。

### API-18 — ApiResponse::from_error 按值消费 BaseError
- **严重度 NIT · P3 · effort S · breaking** · confidence medium（NIT-2）
- 位置：`crates/yang-base/src/action/response.rs:197-199`
- 现状：`from_error(error: BaseError)` 消费 error，调用方想同时日志须 clone。
- 更优解：改 `from_error(error: &BaseError)`（code()/to_string() 均不需所有权）。
- ✅ 验收：转换后仍可 `tracing::error!` 原 error。

---

## 8. 插件系统

### PLUG-1 — PluginManager::register 双重锁 check-then-insert TOCTOU
- **严重度 HIGH · P1 · effort S · 非 breaking** · 核验确认
- 位置：`crates/yang-base/src/plugin/mod.rs:248-265`
- 现状：读锁 contains_key 检查（253 释放）→ 无锁 `on_register().await`（256-259）→ 写锁 insert（263）。并发同名两任务均过检查、各执行 on_register、竞争写锁，第二次静默覆盖，首个 on_register 副作用被孤立，与"已注册返回 PluginAlreadyRegistered"矛盾。注：`PluginManagerBuilder::register` 单线程构建路径无此问题。
- 更优解（推荐两阶段）：无锁调 on_register → 取写锁内**二次 contains_key 校验** → insert，避免持写锁跨 await。
- ✅ 验收：并发注册同名插件，一个成功、另一个 `PluginAlreadyRegistered`，无静默覆盖。

### PLUG-2 — PluginManager::topological_sort 静默吞循环依赖
- **严重度 MEDIUM · P2 · effort S · 非 breaking** · confidence high
- 位置：`crates/yang-base/src/plugin/mod.rs:393-439`
- 现状：Kahn 后环节点赋 `usize::MAX` 静默置末尾；`PluginRegistry::compute_topological_sort` 能返回 `PluginCircularDependency`(100006) 但旧路径绕过，关闭时顺序不确定。
- 更优解：Kahn 后检查 `sorted.len() != plugins.len()`，私有方法可 `log::error!` 记录环，或改为返回 Result 与 Registry 对齐。
- ✅ 验收：存在环时有错误/告警，不再静默。

### PLUG-3 — PluginManager 与 PluginManagerBuilder+PluginRegistry 双轨并存，旧 API 弱却未 deprecated
- **严重度 MEDIUM · P2 · effort M · breaking** · confidence high
- 位置：`crates/yang-base/src/lib.rs:80`；`plugin/mod.rs:196-535`
- 现状：旧 `PluginManager` 有 TOCTOU(PLUG-1)、不检测环(PLUG-2)、每次 get_all 持锁重排；`lifecycle::graceful_shutdown` 仅接受 `Option<&PluginManager>`；lib.rs 示例仍用旧 API。
- 更优解：`PluginManager` 加 `#[deprecated(note="改用 PluginManagerBuilder+PluginRegistry")]`；`graceful_shutdown` 改接 `&PluginRegistry` 或抽 `PluginLifecycle` trait；更新示例。
- ✅ 验收：旧 API 标 deprecated；shutdown 支持 Registry。

### PLUG-4 — Plugin 三个生命周期回调返回 Box<dyn Error>，丢错误链
- **严重度 MEDIUM · P2 · effort M · breaking** · confidence high
- 位置：`crates/yang-base/src/plugin/mod.rs:138,150,161`
- 现状：`on_register/on_init/on_shutdown` 返回 `Box<dyn Error>`，调用处 `.to_string()` 转换丢 `#[source]` 链，违 yang-base BaseError 约定。
- 更优解：改返回 `Result<(), BaseError>`，调用处直接传播；插件包装第三方错误用 `BaseError::Unknown`。需改所有插件实现与测试。
- ✅ 验收：回调返回 BaseError；错误链经 source() 可追踪。

### PLUG-5 — Plugin::dependencies() 默认实现每次分配新 Vec
- **严重度 LOW · P3 · effort M · breaking** · confidence high
- 位置：`crates/yang-base/src/plugin/mod.rs:94-96`
- 现状：`fn dependencies(&self)->Vec<&str>{Vec::new()}` 每次拓扑排序对每插件分配空 Vec。
- 更优解：改 `fn dependencies(&self)->&[&str]{ &[] }`，实现方用静态切片。所有实现同步改。
- ✅ 验收：dependencies 无堆分配；实现返回切片。

---

## 9. 测试与可测性

### TEST-1 — 慢查询 warn 分支从未被断言（合并 T-INTE-1 薄弱断言）
- **严重度 MEDIUM · P1 · effort S→M · 非 breaking** · 核验下调（HIGH→MEDIUM）
- 位置：`crates/yang-base/src/table/table_query.rs:2698-2730`(timed) + `tests/table_query_transaction_test.rs:515-547`
- 现状：唯一覆盖测试设 `Duration::from_nanos(0)` 进 warn 分支但只断言结果正确，从不验证 warn 发出；无 subscriber；`tracing-subscriber`/`tracing_test` 不在 dev-deps。warn 条件改反则测试仍绿。
- 更优解：(S) dev-dep 加 `tracing-subscriber`/`tracing_test`，新增 `test_slow_query_warn_fires` 断言 WARN 含 table/op；(M) 把决策抽为纯函数 `emit_slow_log(elapsed, table, op)` 便于无 Docker 单测，timed 保留为集成调用点。
- ✅ 验收：threshold=0 时断言捕获含"慢查询"+表名的 WARN 事件。

### TEST-2 — 所有 tracing span/record 调用零 subscriber 断言
- **严重度 MEDIUM · P1 · effort M · 非 breaking** · 核验确认问题、修正方案
- 位置：`module_router.rs:381-388`、`middleware.rs:142-143`、`typed.rs:88-104`
- 现状：dispatch 根 span + record request_id、handle span、RequestIdMiddleware record 全在测试中静默 no-op；字段名拼错/record 移位不会失败。
- ⚠️ **修正方案**（核验否决原方案）：(1) 用 `tracing_subscriber::fmt().with_test_writer().try_init()`——**必须 try_init()**，并行测试下 `.init()` 会 panic；(2) `tracing_test`/`logs_contain` 只能捕获 `info!/debug!` 文本事件，**无法读 span 字段值**——要断言 `request_id` 为 32 位十六进制，需自实现 `tracing::Subscriber` 的 `new_span/record` + `Visit` 把字段写入 `Vec<FieldCapture>`（工作量大于"M"的字面印象）。
- ✅ 验收：自定义 recorder 断言 dispatch span 名、request_id 字段格式、action 值。

### TEST-3 — ObservabilityConfig 全局单例无法在同进程测 init→get 链路
- **严重度 MEDIUM · P1 · effort S（独立集成测试）· 非 breaking** · confidence high
- 位置：`crates/yang-base/src/observability.rs:66-90`
- 现状：`get_returns_default_when_uninitialized` 不做断言；`double_init` 用局部 OnceLock 替身，真实 init→get→ActionContext 注入链零测试。
- 更优解：(短期) 新增 `tests/observability_test.rs`（独立进程→新鲜 OnceLock）唯一调 init(阈值) 后断言 `get().slow_query_threshold==Some(阈值)`；(长期) 改依赖注入 `ActionContext` 持 `Arc<ObservabilityConfig>`。
- ✅ 验收：独立集成测试验证 init 后 get 读回正确阈值。

### TEST-4 — RequestIdMiddleware 无专属测试
- **严重度 MEDIUM · P1 · effort S · 非 breaking** · confidence high
- 位置：`crates/yang-base/src/router/middleware.rs:122-146`
- 现状：现有中间件测试只用 ShortCircuit/OrderRecording，三条路径（合法头透传 / 小写头容忍 / 缺失或非法回退生成值）全未覆盖。
- 更优解：新增 `test_request_id_middleware_propagates_header` 等，用 RecordingMiddleware 读 `ctx.request_id`（crate 内 pub(crate) 可访问）断言三路径。
- ✅ 验收：三条路径各有用例且通过。

### TEST-5 — lifecycle::graceful_shutdown 完全无测试
- **严重度 MEDIUM · P2 · effort M · 非 breaking** · confidence high
- 位置：`crates/yang-base/src/lifecycle.rs:70-96`
- 现状：plugins→Redis→MySQL 停机顺序（启动逆序）与"插件失败不阻断 drain"语义均无测试。
- 更优解：(1) 注入故意返回 Err 的测试插件，断言 `graceful_shutdown(Some(&pm))` 返回 Err（失败被传播）；(2) plugins=None 时返回 Ok；顺序可用共享 `Vec` 记录器验证。
- ✅ 验收：插件失败传播且后续 drain 不被阻断有测试覆盖。

### TEST-6 — Plugin shutdown 仅验证 Ok，未验证逆拓扑顺序
- **严重度 MEDIUM · P2 · effort S · 非 breaking** · confidence high
- 位置：`crates/yang-base/src/plugin/mod.rs:1127-1138`（注释 812 称"拓扑逆序"）
- 现状：A←B 测试两插件 on_shutdown 均空实现，无法区分顺序；`plugins.reverse()` 被移除不会被发现。
- 更优解：`test_shutdown_calls_in_reverse_topological_order`：A←B←C 各向 `Arc<Mutex<Vec<String>>>` push 名字，断言 `["c","b","a"]`。
- ✅ 验收：shutdown 顺序断言为逆依赖序。

### TEST-7 — yang-db PG Transaction 无 drop-without-commit / 并发隔离测试
- **严重度 MEDIUM · P2 · effort M · 非 breaking · 已跟踪 NEW-39（本处给更优验收测试，不重复登记）** · confidence high
- 位置：`crates/yang-db/src/postgres/transaction.rs`
- 现状：MySQL Transaction 有 Drop（rollback on drop），PG 遗漏（NEW-39）；现有 PG 集成测试无 drop-without-commit 与并发事务用例。
- 更优解：在 `tests/integration_database.rs`（需 Docker）加 ① `test_pg_transaction_rollback_on_drop`（开事务 insert→drop 不 commit→select 断言不存在，作为 Drop impl 落地验收）② `test_pg_transaction_concurrent_isolation`（两 task 并发 select-then-update 同行验证无脏读）。
- ✅ 验收：drop-without-commit 行未落库；并发用例通过。

### TEST-8 — LoginAction generate_token_pair 失败路径的审计 on_failure 未测
- **严重度 LOW · P2 · effort M · 非 breaking** · confidence medium
- 位置：`crates/yang-base/src/action/auth.rs:317-327`
- 现状：测试仅 DummyVerifier(永远 Ok)，`audit.on_failure` 携带的 subject/error_code 未验证。
- 更优解：`test_login_audit_on_token_generation_failure`：用签名失败的 TokenManager(私钥/公钥格式错误) dispatch LoginAction，断言 RecordingHook 捕获 on_failure 且 error_code 非空、subject 正确。需 GlobalTools 支持注入非法 TokenManager。
- ✅ 验收：token 生成失败时 on_failure 被调用且字段正确。

### TEST-9 — RequestId::generate() 并发唯一性无压测
- **严重度 LOW · P3 · effort S · 非 breaking** · confidence high
- 位置：`crates/yang-base/src/action/request_id.rs:31-36,94-99`
- 现状：仅顺序 a<b，无并发唯一性断言（COUNTER 用 Relaxed）。
- 更优解：`test_generate_unique_under_concurrency`：64 线程×1000 次收集 HashSet 断言 len==64000。回归保护防误改为非原子/引入 Mutex。
- ✅ 验收：并发生成无碰撞。

### TEST-10 — token_fingerprint 无冲突/雪崩 property test
- **严重度 LOW · P3 · effort S · 非 breaking** · confidence medium
- 位置：`crates/yang-base/src/action/auth.rs:184-196,712-722`
- 现状：仅测两固定值稳定/不同，无相似输入扩散性与边界测试。
- 更优解：对 100 对仅末字符不同的字符串断言指纹全不同；测空串/单字节不 panic。防退化为 `s.len()`。
- ✅ 验收：1-bit 差异输入指纹不同；边界不 panic。

### TEST-11 — CircuitBreaker Mutex 中毒恢复路径无测试
- **严重度 LOW · P3 · effort M · 非 breaking** · confidence medium
- 位置：`crates/yang-base/src/http/circuit_breaker.rs:82,106,123`
- 现状：`unwrap_or_else(|p| p.into_inner())` 恢复分支为死代码，改 `.unwrap()` 测试仍绿。
- 更优解：`#[cfg(test)] pub fn states_lock_for_test` 注入持锁 panic 毒化，再跨线程调 allow/on_success/on_failure 验证不 panic、返回合理值。
- ✅ 验收：锁毒化后断路器仍可用。

---

## 10. 建议落地顺序

> 原则：先封堵可触发的安全/正确性阻断（多为 S effort），再补可测性护栏，最后做大重构与分配优化。

**第 1 批 — 安全/正确性阻断（P0/P1，多为 S）**
1. AUTH-1 token_type 中间件校验 · AUTH-3 verify_token 误分类 · LOGIC-1 path_param 数值
2. PLUG-1 插件注册 TOCTOU · LOGIC-2 软删除字段写权限 · AUTH-2 轮转原子化（修正方案）
3. ERR-1 is_client_error 漏 NotFound

**第 2 批 — 可测性护栏（P1，锁住上面修复不回退）**
4. TEST-1/2/3/4（慢查询 warn / span 字段断言 / Observability 单例 / RequestId 中间件）
5. TEST-5/6/7（停机顺序 · 插件 shutdown 逆序 · PG 事务 drop=NEW-39 验收）

**第 3 批 — 高频 ergonomics 与 SemVer 加固（P1/P2，S）**
6. API-1/2 non_exhaustive（API-2 先改 derive 宏再加属性）· API-6 DatabaseConfig builder · AUTH-6 token_type 枚举 + API-3
7. AUTH-4/5/7 · QRY-1/2 查询 DoS 上限 · LOGIC-5 EXECABORT · ERR-2

**第 4 批 — 性能与分配（P2/P3）**
8. PERF-2 Redis pipeline · PERF-4 权限 HashSet · PERF-3/6 SQL 分配 · PERF-14 RedisValue as_str
9. PERF-5 引入 `write_condition_to_sql` 借用遍历（基座）→ 顺带消除 PERF-7/8/9
10. 其余 PERF-10~16、PLUG-2~5、剩余 API ergonomics

**第 5 批 — 大重构（P1 价值高但 L/XL，需配套回归）**
11. QRY-5 SqlParam→SqlValue（窄、低风险，先行）
12. QRY-3 Dialect trait 去重（配并行集成测试）· QRY-6 QueryBuilder 去生命周期
13. QRY-4 TableQuery 与 QueryBuilder 统一——**仅在 yang-db 暴露 `where_condition(Condition)` API 之后**推进，否则停在 QRY-5 的参数统一即可

> 每批结束跑 `cargo fmt && cargo clippy --all-targets --all-features -- -D warnings && cargo test --lib`，并回写 `docs/BACKLOG.md` 状态。
