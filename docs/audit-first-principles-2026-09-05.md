# lib_yang 基础库第一性原理审计

**日期:** 2026-09-05　**基线:** b8e511d　**方法:** 六路并行只读勘察 + 父代理逐文件复核（每项判定附证据 `文件:行`）

**修订:** 2026-09-05 第二轮六路并行只读复核修订。修正初版 **2 处实质性误判**（schema_sync TOCTOU 不成立；`TypedHandler` 默认路径并非死代码）及约 15 处证据行号/表述偏差，修正处以「复核修正」标注。

> 第一性原理 = 回到每层"为什么存在"的不可再约本质，逐一检验实现是否直接服务该本质。判定含义：**GOOD**（直接服务本质，代价合理）；**OK**（有取舍但当下合理）；**SUBOPTIMAL**（有更简单/更本质的做法，建议改进）。先给结论，再给证据。

---

## 0. 全局结论（先读这段）

**五个 crate 的整体架构是健康的、且明显优于这个体量的典型个人项目**：无循环依赖、构建期冻结 + 运行期只读的分层正确、PCG 完全解耦、安全边界普遍 fail-closed。真正的弱点不在"哪里坏了"，而在**两套过度**与**两类待决策**：

| # | 类型 | 问题 | 一句诊断 |
|---|------|------|----------|
| 1 | 过度 | `yang-db` 的 MySQL 侧有**多套表达式表示**：操作符映射三套并存（`ComparisonOperator`/`CompareOp`/字符串 op）、`Condition`（19 变体）、`SqlValue`、`SqlExpr`、builder 与 Subquery 内联 match | 条件从用户到 SQL 要经过多次映射 match，新增 op 需同步至少 3 处 |
| 2 | 过度 | `action/typed.rs` 存在 **3 层派发**（`Action` → blanket `TypedHandler` → `DynAction`），且两种 action 写法并存（`impl Action` vs 直接 `impl TypedHandler`） | 内置 6 个 CRUD action 走 TypedHandler 默认路径、UiCatalogAction 走 Action 原生路径，双写法增加认知负担，泛型收敛后本质只需 2 层 |
| 3 | 待决策 | `yang-base` 与 `yang-runtime` **各写一套配置加载**（`EngineConfig::from_env` vs `ConfigSource` 合成器），字段不共享 | 职责高度相似，语义漂移风险 |
| 4 | 待决策 | `yang-db` 的 SQL 方言抽象（`dialect.rs`）**只共享了标识符转义与条件渲染**，PG 的 CRUD/UPSERT/RETURNING 组装层独立 | 共享边界是 dialect.rs 模块文档自己声明的，但是否收敛未显式决策 |

**复核修正：** 初版曾将 schema_sync 的 preflight 与 apply 判为「分属两个锁域的 TOCTOU 竞态」并列为 P0，**复核确认不成立**——校验与执行在同一把 MySQL advisory lock、同一条连接内完成（详见 §4）。schema_sync 无竞态缺口，残留面仅是 advisory lock 不约束并发业务 DML（由 ALTER 约束失败 fail-closed 兜底）。

**优先级建议**：短期改 #2（统一 action 写法，收编派发层）；中期消 #1（合并表达式/操作符表示）；#3/#4 属于取舍问题，建议显式决策而非放任。

---

## 1. 总体架构（audit-architecture + 复核）

### 依赖图（全部实测，非文档声称）

```
yang-runtime ──→ yang-base ──→ yang-base-derive (proc-macro, 仅编译期)
      │              │
      └──────────→ yang-db  ←─┘   (yang-base 经 mysql/redis feature 依赖 yang-db)
yang-pcg  → 仅外部 crate（serde/rand/thiserror）—— 零内部依赖，完全孤立
```

- 反向依赖实测为零（代码层面）：`yang_pcg` 在 base/db 中零出现；`yang_base` 在 yang-db 中仅出现于 5 处中文 doc 注释（如 `mysql/database.rs:244`）；`yang_runtime` 在 base 中仅 1 处 JSON schema 扩展键字符串 `"x-yang-runtime-validators"`（`table/definition.rs:1643`）。均为非代码引用，**无循环、无反向依赖。**（复核修正：初版"零出现"字面不准确，结论不变。）
- 版本经根 `Cargo.toml [workspace.dependencies]` 统一（约 40 个依赖集中管理），仅 argon2/rand_core/hmac/http-uri（yang-base）、crc32fast/rand/rand_pcg（yang-pcg）、deadpool-redis/redis（yang-db）等少量局部 pin。
- feature 全部使用 `dep:` 显式语法，**无隐式 feature 依赖**（`crates/yang-base/Cargo.toml:29-56`；`openapi`/`admin-metadata` 为空 feature 属正常）。
- 注：yang-base 对 yang-db 的 manifest 依赖是无条件 path 依赖，能力经 `mysql`/`redis` feature 门控（`yang-base/Cargo.toml:35,38`）；yang-runtime 亦直接依赖 yang-db（`yang-runtime/Cargo.toml:27`）。

### 判定表（节选）

| 决策 | 判定 | 证据 | 理由 |
|------|------|------|------|
| 分层方向 db→base→runtime | **GOOD** | `crates/yang-base/Cargo.toml:60`、`crates/yang-runtime/Cargo.toml:26-27` | 低层无业务依赖，高层按需聚合 |
| `yang-pcg` 完全解耦 | **GOOD** | `crates/yang-pcg/Cargo.toml:32-46`（仅 6 个外部 crate） | 纯算法 crate，可独立发布 |
| 资源所有权集中 Tools（替代 Global 单例） | **GOOD** | `crates/yang-base/src/tools.rs`（`ToolsBuilder`:254 / `Tools`:67） | 构建期显式所有权从结构上消灭进程级资源全局单例；残留 `OnceLock` 均为元数据/regex 缓存（如 `action/functional.rs:49-51`、`table/validator.rs:24-34`），非资源单例 |
| `yang-runtime` 依赖 `yang-base` | OK（有意的应用胶水层） | `crates/yang-runtime/Cargo.toml:7`（`publish=false`）、:26 | 作为 `yang-system` 的胶水合理；若想成为可复用"运行层"则 observability 应下沉 |
| 配置加载分两处 | **SUBOPTIMAL** | `crates/yang-base/src/config.rs:83`（`EngineConfig::from_env`，读取 `YANG_MYSQL_URL` 等固定变量；:32 为结构体定义）vs `crates/yang-runtime/src/config.rs`（无具体字段的通用 ConfigSources 合成器） | 语义重叠、字段不共享，漂移风险 |
| SQL 方言共享 | **SUBOPTIMAL** | `crates/yang-db/src/dialect.rs:12-13`（模块文档自声明共享边界） | 复核修正：dialect **已部分覆盖** PG——标识符转义经 `postgres/identifier.rs:18,23` → `dialect::quote_identifier(POSTGRES, ...)`，条件渲染经 `postgres/condition.rs:302` → `dialect::render_condition(POSTGRES, ...)`；独立的只是 CRUD/UPSERT/RETURNING 组装层（`postgres/query_builder.rs`），而非"未覆盖 PG 构建器" |

---

## 2. yang-db（MySQL/PG 查询构建 + Redis）

### 职责本质
查询构建层的本质：**把 Rust 类型安全地映射为 SQL 文本 + 绑定值序列，防注入、防误用**。Redis 客户端本质：命令序列化 + 连接池 + 错误传播。

### 判定表

| 决策 | 判定 | 证据 | 理由 |
|------|------|------|------|
| 值一律走绑定，列名经 `quote_identifier` 校验 | **GOOD** | `generator.rs:340-345, 365`；INSERT 列头/表名逐项 quote、值全部 `?` 占位 + `params`（`generator.rs:455` 起） | 注入面只在标识符一侧，且已封闭 |
| 比较操作符白名单校验（`ComparisonOperator::parse`） | **GOOD** | `condition.rs:26-36`（六操作符白名单） | 非法 op 返回 `Err(DbError::UnsupportedOperator)` 而非拼进 SQL |
| `SqlExpr` 受控服务端表达式（UPDATE SET / INSERT VALUES / SELECT 投影显式携带） | **GOOD** | `query_builder/mod.rs:84-87`（`expr_assignments`/`select_exprs`） | "允许少量 SQL 表达式"的口子收窄到显式类型，比裸字符串安全 |
| 操作符映射**三套并存** | **SUBOPTIMAL** | `condition.rs:26-36`（`ComparisonOperator` enum+`parse`）vs `reference.rs:176`（`CompareOp` enum）vs `Subquery::where_value` 手写 match（`condition.rs:77-85`）；builder.rs 另有三处内联 match（:154-179 `where_and`、:202-216 `where_or`、:405-419 `having_cond`） | 复核修正：比初版"两套并存"更多——同一操作符语义实际有三处映射表，新增 op 需同步多处 |
| 表达式类型过多 | **SUBOPTIMAL** | `ComparisonOperator` / `CompareOp` / `Condition`（**19 变体**，`condition.rs:121-160`，复核修正初版"8+ 变体"低估）/ `SqlValue` / `SqlExpr` / builder 内联 match / `Subquery` 内联 match | 从用户条件到 SQL 经多层转换，类型间转换代码散布 |
| `QueryExecutor` pool/transaction 双态 | GOOD | `mod.rs:60-63` | 正确表达"构建器可能执行于事务"，且 builder 必须从不借用事务的 pool 创建（`mod.rs:99-100` `from_pool` 文档锚点） |
| `unions` 先存储后渲染 | OK | `mod.rs:82`（`Vec<(UnionOperator, Box<QueryBuilder>)>`，渲染在 `generator.rs:169-177`） | 体量小（2 op），为它引入 SQL AST 不值 |
| Redis client 主 API 单一 | GOOD | `redis/client.rs`（2237 行主 API，另有 config/pipeline/transaction/value 辅助模块） | 结构简单 |
| 已知 panic 热点（现存，勿新增） | 受限 OK | `aggregate.rs:50-51`（`unwrap_or(0)`，注释声明 COUNT 语义）、`generator.rs:464` 等（`unwrap_or(&Value::Null)`） | 全部是 `unwrap_or` 兜底而非裸 unwrap；`query_builder/` 目录 grep 无裸 `unwrap()`/`expect(`，符合项目自身规范 |

### 结论
**注入防护是可靠的**：值绑定、列名 quote、op 白名单三层都到位，且比多数个人项目严谨。主要代价是**表达式层数**：建议把 `Subquery` 的字段方法改为复用 `Condition` 构造器，统一三套 op 映射为 `Condition` 变体 + `parse` 一处入口，可删掉约 4 处重复 match（builder.rs 三处 + Subquery 一处）。

### 改进建议
1. **统一表达式入口**：收敛 `ComparisonOperator`/`CompareOp`/字符串 op 三套表示，删除 `Subquery::where_value` 手写 match（`condition.rs:77-85`）与 `builder.rs:154-179/202-216/405-419` 的内联映射 match，全部走同一条构造路径（复核修正：初版误写 `where_and` 在 mod.rs，实际在 builder.rs）。
2. **清理悬空文档注释**：`query_builder/mod.rs:33-37` 有一段描述"消除三处重复映射 match 的共享助手"的文档注释，但该助手函数在 mod.rs 中并不存在（注释挂在 `enum UnionOperator` 上方，疑为重构残留）——复核新发现，建议删除或落实。
3. **dialect 边界显式决策**：dialect 已共享标识符转义与条件渲染（见 §1），剩下独立的 PG 组装层要么并入 dialect，要么显式注释声明为"够用即可"的次要目标，避免两份构建逻辑继续漂移。
4. `aggregate.rs:50` 的 `unwrap_or(0)` 有注释声明理由，可保留；但不建议新增同款（遵守仓库 anti-pattern）。

---

## 3. yang-base 内核（definition 构建 + Tools + action 派发）

### 职责本质
框架内核只有四条本质路径：**注册(declare) → 校验(validate) → 编译/冻结(compile/freeze) → 请求分发(dispatch)**。第一性检验 = 每层间接、每次运行时重复构建、每次跨层拷贝都必须为这四条路径服务。

### 判定表

| 决策 | 判定 | 证据 | 理由 |
|------|------|------|------|
| `AppBuilder` 构建期交叉校验 + `build()` 冻结为 `BuiltApp` | **GOOD** | `builder/app.rs:113`（`#[must_use]` 防忘 build）、`build()` 在 app.rs:131-173 依次调用 8 个 validate 函数（unique_addons/dependencies/module_ownership/unique_modules/module_contents/middleware_order/references/routes，均在 `builder/validate.rs`） | 校验全部发生在运行期之前；冻结结构 `Arc<Registry>`+`Arc<Tools>`（app.rs:29-30） |
| Registry/Tools 用 `Arc` 共享，请求期零查找 | **GOOD** | `builder/app.rs:87-90`（`context()` 每次请求仅 `Arc::clone`）；dispatch 走构建期 `ActionHandle` slot 直取 handler（`builder/registry.rs:297-300`） | 请求热路径不按名解析；`Registry::resolve()` 的按名查找仍存在（registry.rs:100-102），仅服务显式内部调用句柄解析 |
| 资源生命周期状态机 RUNNING→CLOSING→CLOSED | **GOOD** | `tools.rs:22-24, 103-108`（`ensure_running`），`close()` 幂等且逆序关闭（:231-248） | 关闭后取资源返回 `Err`，fail-fast 而非静默 |
| 扩展/配置用 `TypeId→Box<dyn Any>` 类型化映射 | GOOD | `tools.rs:26, 76-77`（`TypeMap = HashMap<TypeId, Box<dyn Any + Send + Sync>>`），取用见 :153-178 | 类型安全 + 低频访问，代价（动态分发）只在扩展点 |
| Action 三层 trait（`Action` → blanket `TypedHandler` → `DynAction`） | **SUBOPTIMAL** | `action/typed.rs:19-53`（Action）、55-86（blanket 桥接）、88-123（TypedHandler）、126+（TypedAction）、194+（DynAction）+ :230 blanket | 用户手写 1 层；blanket impl 让 `TypedHandler` 只剩"旧接口兼容"意义 |
| `handle_future` 双路径与两种 action 写法并存 | **SUBOPTIMAL** | `typed.rs:71-77`（blanket 内原生覆盖，直接调 `Action::index`）vs :115-122（默认调 async-trait `handle`） | 复核修正：默认路径**不是死代码**——全部 6 个内置 CRUD Action 直接 `impl TypedHandler`（`builtin/add.rs:53`、`get.rs:43`、`put.rs:51`、`del.rs:44`、`select.rs:94`、`table.rs:51`），只有 `ui_catalog.rs:36` 走 Action 原生路径；且默认路径是**单次装箱**，二次装箱仅当"实现 Action 却未覆盖 handle_future"；`Action` 本身也是 `#[async_trait]`（typed.rs:20），两条路径各有一次装箱。真正的问题是两种 action 写法并存、语义重复 |
| `params()` 默认经 `ParamInput` 单一事实 | **GOOD** | `typed.rs:33-35` | 输入定义不重复 |
| UI 目录按身份投影、secret fail-closed | **GOOD** | 入口 `builder/app.rs:74-84`；实际投影在 `builder/registry.rs:151-287`（action/view/module 全经 `policy.allows(context)` 过滤）；secret fail-closed 在 `builder/project.rs:64`（`!column.secret && access_rule_allows(...)`）、:83（`write_only: column.secret \|\| !readable`） | 目录不是安全边界但仍做了 fail-closed，且注明租户隔离在 TableQuery 层强制 |

### 结论
**构建期/运行期分界是这个框架最正确的设计**：所有校验在冻结前完成，运行期只有 `Arc::clone` + dispatch。成本集中在**派发链的层数与双写法并存**。建议：统一两种 action 写法——例如全部收敛为 `#[derive(Action)]` 原生路径，将 `TypedHandler` 降为私有实现细节；或反向统一。目标使用户只面对一种写法，而非"删除死代码"（默认路径在被实际使用，见上表复核修正）。

### 改进建议
1. **收编派发层/统一写法**：收敛 `impl Action` 与直接 `impl TypedHandler` 双轨制（涉及 `typed.rs` + `builtin/*` 6 个 action + `ui_catalog.rs`），同步简化 `handle_future` 双路径；注意收益是"减一种写法、减一层认知负担"，不是"删死代码"。
2. **Request 生命周期文档化**：`context.rs:149` 的 `ActionContext::new` 每次请求新建（经 app.rs:87-90）；`cached_roles` 注释在 context.rs:138-140（PERF-13：`with_user()` 时一次性构建，后续 `table_query()` 仅 `Arc::clone` O(1)，实现见 :258-264、:373-376）——确认角色缓存不会因用户中途变化而陈旧即可（若会，需要失效策略）。
3. `builder/` 已拆 7 个职责文件（app/catalog/compile/handle/project/registry/validate）+ mod.rs，共 8 个 `.rs` 文件（复核修正初版口径），目录组织本身 GOOD，无需再动。

---

## 4. yang-base 外围（auth、安全原语、table、schema_sync）

### 职责本质
安全代码的本质 = **fail-closed + 无 TOCTOU**；表查询的本质 = 权限裁剪 + 类型化字段 + SQL 生成；schema_sync 的本质 = **幂等收敛（diff→plan→apply，重复执行结果一致）**。

### 判定表与证据

| 决策 | 判定 | 证据 | 理由 |
|------|------|------|------|
| 密码用 Argon2 + 受控并发 + PHC 格式 | **GOOD** | `action/auth/password.rs:3-6`（注释明确"内存困难算法…阻塞工作线程"→ 信号量限制全局并发）、:22-26（`max_concurrency > 0` 校验）、:74（`spawn_blocking`） | 内存困难算法正确选型；并发上限有威胁模型注释 |
| 上传路径 canonicalize + `starts_with(root)` 防穿越 | **GOOD** | `action/upload.rs:68-88`（双端 canonicalize + 前缀校验，越界 `PermissionDenied`；:85 用规范路径执行 copy 消除 symlink 交换窗口） | 无 `..` 拼接漏洞；无 temp_root（伪造 JSON 实例）时 fail-closed 拒绝 copy；有三个对应测试 |
| 租户解析：header 仅候选，resolver 服务端校验，`TenantResolution` 代数消除非法状态 | **GOOD** | `action/tenant.rs:3-4`（"不可信候选"注释）、:16-26（Tenant/System 两变体）、:135-152（header 仅解析为正整数候选） | fail-closed + 用类型消灭 `Option+bool` 组合 |
| Token 三重校验（签名+过期+黑名单）短路 | **GOOD** | doc 注释 `middleware.rs:22-23`；实际代码 :159-165（`scope()` 默认 `ProtectedActions`）、:173-186（缺 token → `Unauthorized`）、:189（`verify_token_checked`）、:191-194（token_type 必须为 Access） | `public` action 需显式声明才绕过 |
| schema_sync preflight 与 apply **同锁域** | **GOOD**（复核修正：初版误判为 TOCTOU） | `schema_sync/sync.rs:117-167`：`sync_table_definitions` 先 `GET_LOCK`（:140-150，30s 超时）→ 调 `sync_locked`（:152）→ `sync_locked` 内部依次 `plan_locked` → `preflight_locked` → DDL 执行（:170-208）→ 全部完成后才 `RELEASE_LOCK`（:153-166） | 校验与执行在同一把 advisory lock、同一条连接内，锁不在中间释放，不存在初版断言的竞态窗口；公开的 `preflight_table_definitions`（:88-114）不持锁，但文档明确定位为只读预检 |
| schema_sync plan/changes 双账本断言 | GOOD | `schema_sync/plan.rs:22-27`（`plan.statements.len() != plan.changes.len()` 即报"schema 计划内部不一致"） | 语句与变更一一对应是收敛性的内部不变量 |
| 表权限校验在查询构建器（字段级 `validate` 模块） | **GOOD**（复核补证完成） | `table/table_query/mod.rs:46`（`mod validation`）；`table_query/validation.rs` 实现 `validate_read_field`（字段存在性 + `can_read(user_roles_set)` 字段级读权限），并覆盖筛选/排序权限与 WHERE 条件树递归校验（QRY-1/QRY-2 上限） | 初版"需补证"已复核：校验入口机制成立 |

### 安全热点（按严重度，复核修订）

1. **[低] schema_sync 并发业务 DML 面**（复核修正：替换初版的"TOCTOU 竞态"）：advisory lock 只串行化 schema 同步者本身；preflight 与 ALTER 之间的并发业务写入不受锁约束，可能写入违反新约束的行——但 ALTER 本身会因约束失败而报错，fail-closed 兜底。这不是锁域竞态，属可接受的设计边界；建议仅在运维文档中写明"schema 变更窗口期避免并发写入"，或对校验做显式的冲突行预扫描。
2. **[低] `auth/password.rs` 的 Argon2 参数**：确用 `Argon2::default()`（:39 hash、:54 verify），未显式调优 m_cost/t_cost/p_cost，依赖 argon2 crate 的 RFC 9106 推荐默认值（19 MiB / 2 / 1）；建议把参数放入配置并可调（`max_concurrency` 已可调，内存/迭代建议跟随）。
3. **[低] public action 清单**：`middleware.rs:159-165` 依赖"标记为 public 才绕过"，属 fail-open 面——建议构建期统计 public action 并输出到审计日志/启动警告，防止误标。

### 结论
**外围安全设计是这个仓库最见功力的部分**：几乎每个边界都选对了默认（拒绝）方向，且文档注释把威胁模型写得清清楚楚。复核修正：初版所称"schema_sync 竞态窗口"这一结构性缺口**不存在**——锁域设计正确；剩余风险项均为低危加固建议。

---

## 5. yang-base-derive + yang-runtime

### 职责本质
proc-macro 的本质：**编译期把声明式输入扩展为与手写等价、无隐藏 panic、错误 span 指向用户输入的代码**。运行时本质：确定性启动顺序 + 优雅关闭（级联 + 超时兜底 + 幂等）+ 结构化可观测。

### 判定表

| 决策 | 判定 | 证据 | 理由 |
|------|------|------|------|
| 宏错误全部 `syn::Error::new_spanned`（指向用户 token） | **GOOD** | `crates/yang-base-derive/src/action.rs:103, 126, 135, 145` 等（还有 :159/168/176/183/190/194/201/236 同模式）；darling 错误经 `e.write_errors()`（:96）转编译错误 | `into_compile_error()` 让 IDE 报错落在用户代码上。小保留：span 统一指向 `input.ident`（struct 名），而非出问题的具体属性 token |
| 宏内默认值用 `unwrap_or_else/default`（编译期值） | GOOD | `action.rs:109-115` | 无运行期 unwrap 泄漏到生成代码（人工核查全部 `quote!` 块：生成代码只用 `OnceLock::get_or_init`/`map_err` 等） |
| `params.rs` 手写 `Parse`（非 `synstructure`） | GOOD | `derive/src/params.rs:30-45`（ParamsInput）、:47-59（ParamField） | 输入形状简单，手写 parse 更可控 |
| 优雅关闭：共享绝对截止时间 + 阶段命名 + 每阶段超时记账 + metrics | **GOOD** | `runtime/src/shutdown.rs:105`（`timeout_at(window.deadline, future)`）、:83（`&'static str` 阶段名）、:128-147（`record_phase` 发 counter/histogram） | 复核修正措辞：并非每阶段独立超时，而是所有阶段共享 `begin()` 设定的总预算绝对截止时间（`ShutdownWindow.deadline`，:56-58，:1 文档注释确认此设计）；duration/remaining_budget 记账确为每阶段进行 |
| 配置来源优先级固定（secret > env > 文件） | GOOD | `runtime/src/config.rs:1-4`（文档注释；初版引用的 :5-8 是 use 行）；`parse_with_sources`（:183-210）硬编码顺序：TOML 文件 → `apply_environment`（:200）→ `apply_secrets`（:201-203） | 与 base 的 `from_env` 职责重叠（见 §1 判定表） |
| `#[derive(Action)]` 与 `params!` 能力边界 | OK | `lib.rs:23-27`（derive 只生成 `TypedAction` impl + `ActionMeta` 静态聚合）vs `lib.rs:30-35`（params! 生成输入 struct + `ParamInput` impl） | 一个做元数据、一个做输入 struct，当前划分清晰；未审计是否可合并为单一宏（取舍项） |

### 结论
派生宏与运行时的质量**显著高于同类个人项目**：span 定位、compile_error 模式、超时账本都是"被工业界验证过才值得抄"的做法。grep 全 crate 无 `unwrap()`/`expect()`/`panic!`；唯一例外是 `params.rs:119` 的 `unreachable!()`——位于宏侧（proc-macro 进程）代码、由外层 `if !matches!(source, Source::Body)` 守卫、不进入生成代码（复核补充）。yang-runtime 的 panic 全部位于 `#[cfg(test)]` 与集成测试，生产代码干净。改进空间主要在**文档化能力契约**（derive 宏的 README 与代码的对应）与配置层的合并决策。

---

## 6. yang-pcg（确定性 PCG）

### 职责本质
确定性 PCG = **纯函数管线 + 固定种子消费顺序**（同配置同种子 → 字节级同图）；质量 = 各阶段不变量（可达/重叠/连通/spawn 合法）。

### 判定表与确定性核查

| 决策 | 判定 | 证据 | 理由 |
|------|------|------|------|
| `seed:None` 时从 config 派生确定性种子（非系统时间） | **GOOD** | `generator.rs:58-68`（`ConfigDigest::seed_and_digest_from_config`） | 注释 + 测试 `test_generate_with_none_seed_is_deterministic`（`generator.rs:320-335`，两次生成导出 JSON 字符串 `assert_eq!`，字节级断言）双锚点 |
| 根种子派生阶段种子（`root_rng.derive("topology"/"layout"/"terrain"/"spawn")`） | **GOOD** | `generator.rs:77, 84, 90, 101`（复核修正初版行号 70/77/85/93，偏移 6-11 行） | 阶段隔离 + 消费顺序固定 |
| 自定义 `StableRng`（非 thread_rng）贯穿全管线 | **GOOD** | `generator.rs:68` → `rng.rs:162` `StableRng`（底层 `rand_pcg::Pcg64`） | 全 crate grep `thread_rng/entropy/SystemTime/OsRng/from_entropy` **零命中**；另有 `rng.rs:827-879` 哈希/种子稳定性固化测试 |
| `Instant` 仅用于 debug 计时 | GOOD（限 generator.rs 范围） | `generator.rs:76, 83, 89, 100`（`debug_enabled.then(Instant::now)`） | 复核修正边界：此论断仅对 generator.rs 成立——`chunked.rs:172, 373` 的 `Instant::now()` 用于**时间预算强制执行**（功能性用途），不影响确定性结论，但初版的泛化表述需限定 |
| 约束校验在生成前（`validate_constraints`） | GOOD | `generator.rs:70`（种子派生 61-68 之后、拓扑阶段 78 之前） | fail-fast |
| HashMap/HashSet 用于**查找**而非迭代序 | GOOD | `layout/corridors.rs:21-25`（注释声明 O(1) 查找，仅 `.get()`，输出序由 edges enumerate 决定）、`cache/store.rs:32` | 未发现依赖迭代序的输出路径 |
| HybridPrecompute 模式显式拒绝单阶段调用 | GOOD | `generator.rs:51-56`（返回 `Err(PcgError::config(...))` 要求两阶段调用；初版写的 :57 是块尾空行） | 用错误代替静默错误结果 |

### 结论
**确定性是这层做得最扎实的**：种子架构（根种子 + 阶段派生 + 配置派生的缺省种子 + 字节级测试）是对"确定性 PCG"本质的教科书级回答，且对每处可能破坏确定性的来源（时间、熵、容器迭代序）都有注释或测试锚点。没有发现真正的确定性缺陷。

### 改进建议
1. **可选**：`cache/store.rs:32` 的 `HashMap<CacheKey, GenerationResult>` 当前公开 API 仅 `insert`/`get`/`contains`（:40/:44/:48），若未来支持"缓存遍历/清空统计"，注意不要输出迭代序。
2. `chunked.rs:149-150` 的 HashSet 预建（注释"O(1) 查找替代 Vec::contains 的 O(N)"）是纯性能优化，仅用于 :157/:165 的 `contains` 过滤；若分块路径有"结果顺序=某集合顺序"的需求，需确认排序兜底。

---

## 7. 汇总：全局 Top 改进清单（按 ROI 排序，复核修订版）

| 优先级 | 动作 | 位置 | 收益 | 成本 |
|--------|------|------|------|------|
| ~~P0~~ | ~~schema_sync 消除 TOCTOU~~ | ~~sync.rs~~ | **复核撤回**：竞态不存在，校验与执行同锁同连接（§4）；可选仅文档化"变更窗口避免并发写入" | — |
| **P1** | 统一 action 写法、收编派发层：收敛 `impl Action` / 直接 `impl TypedHandler` 双轨制，`TypedHandler` 降为内部细节 | `crates/yang-base/src/action/typed.rs` + `action/builtin/*` + `action/ui_catalog.rs` | 减一层抽象、减一种写法（注意：非"删死代码"，默认路径在被 6 个内置 action 使用） | 1-2 天 + 全量测试 |
| **P1** | 统一表达式/操作符构造入口，删约 4 处重复 match（三套 op 表示收敛为一套） | `crates/yang-db/src/mysql/condition.rs`、`reference.rs:176`、`query_builder/builder.rs:154-179/202-216/405-419` | 新 op 只改一处 | 1-2 天 |
| **P2** | 显式决策配置层：`EngineConfig::from_env` 下沉或抽 `yang-config` | `crates/yang-base/src/config.rs:83`、`crates/yang-runtime/src/config.rs` | 消除语义漂移 | 设计讨论先行 |
| **P2** | public action 构建期统计 + 启动审计日志 | `crates/yang-base/src/action/auth/middleware.rs:159-165` 配套 builder | 防误标 public 扩大攻击面 | 半天 |
| **P3** | PG 方言边界显式化：决策是否把 PG 组装层并入 dialect，或注释声明为次要目标 | `crates/yang-db/src/dialect.rs:12-13`、`postgres/query_builder.rs` | 停止两份漂移 | 大，需权衡 |
| **P3** | 清理 `query_builder/mod.rs:33-37` 悬空文档注释（描述不存在的共享助手） | `crates/yang-db/src/mysql/query_builder/mod.rs` | 消除误导 | 分钟级 |

**一句话收束（复核修订）**：你的代码"最优"程度的分界线不在正确性（正确性普遍优秀），而在**抽象层数与写法统一**——安全与确定性相关的决策几乎全对（schema_sync 的锁域设计经复核也是对的），需要动手的是把三层派发削成两层、把三套 op 表示收敛成一套、把两种 action 写法统一成一种；配置层与 PG 方言边界各需一次显式决策。

---

## 附：方法与局限

- 第一轮：六路子任务并行勘察 + 父代理对关键判定逐文件复核；所有判定表条目均带 `文件:行` 证据（复核时修正了 3 处子任务误报：`run_full_validation` 为 `pub(crate)`、`UiCatalog` 定义在 `definition/ui/catalog.rs`、`TableQuery` 在 `table/table_query/mod.rs`）。
- 第二轮（2026-09-05 复核修订）：六路并行只读复核全部判定，确认 HEAD 即基线 b8e511d（工作区仅有 schema_sync render.rs CHECK 表达式归一化等未提交小改，不影响结论）。修正 2 处实质性误判（schema_sync TOCTOU 不成立；TypedHandler 默认路径非死代码）及约 15 处证据行号/表述偏差（Condition 变体数、where_and 归属文件、PCG 阶段种子行号、shutdown 超时语义、Instant 论断边界、dialect 覆盖范围等），正文以「复核修正」标注。附录三处更正声明经复核确认属实：`run_full_validation` 为 `pub(crate) fn`（`crates/yang-pcg/src/validation.rs:623`）；`UiCatalog` 在 `definition/ui/catalog.rs:13`（`action/ui_catalog.rs` 里的是 `UiCatalogAction`）；`TableQuery` 在 `table/table_query/mod.rs:106`。
- **未覆盖/抽样范围**（诚实声明）：`crates/yang-pcg/src/terrain/` 策略细节与 `export/`（确定性主链已核，分支未逐行）；`yang-base/src/http`、`token`、`router/` 中间件链其余部分；`yang-system`（独立嵌套仓库，非本 workspace）；docs 历史工件。宏展开产物与 schema_sync 全部执行路径未跑测试验证（两轮均为只读审计，未执行 cargo）。
