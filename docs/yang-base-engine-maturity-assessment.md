# yang-base 核心引擎成熟度评估

> 本文整合三轮评估的结论：搭建就绪度（7 维）、引擎成熟度（4 维）、补充核查（类型安全 + 并发，2 维）。
> 所有结论均基于对当前代码的 file:line 取证；评估时分支 `master`。

## 一、总判定

**`solid_core_with_holes`** —— yang-base 是一个安全属性扎实的「受保护单表 CRUD 引擎 + 成熟启动期生命周期」，但还不是完备的生产级核心引擎。

承重墙是好的：参数化绑定 + 标识符转义（注入防线）、字段级四类权限、软删 / WHERE 守卫、插件拓扑生命周期、幂等迁移、统一错误码体系，H-1 类型化重构在边缘真的落地了。

**距完备核心引擎，差四类运行期承重能力：**

1. 受保护层无法跑在事务里（连自身多步写都不原子）
2. 查询表达力封顶在 AND-only 单表（无 OR / JOIN / 聚合）
3. 类型化层钉死 MySQL（yang-db 已有 PG 后端，落后的恰是受保护层）
4. 请求执行热路径对运行时完全不可观测

一句话：引擎的「安全」和「启动期」已经成熟，缺的是「事务原子性、查询表达力、多后端、运行期可观测」这四根运行期承重柱。

## 二、定位认知

yang-base 是**后端原语库**，不是开箱即用框架。它刻意不含 HTTP/WS/gRPC 传输层——那是引擎边界之外、由调用方自带的。本文聚焦**引擎自身**的成熟度，不把「传输层缺失」算作引擎缺口。

## 三、已经成熟、可信赖的部分（不需再投入）

**受保护数据引擎本体**
- 12 种 WHERE 操作符（Eq/Ne/In/NotIn/Like/Gt/Gte/Lt/Lte/Between/IsNull/IsNotNull）全部参数化绑定
- 反引号标识符转义（`is_valid_identifier` / `quote_identifier`），列名拼接注入在类型层面被杜绝
- 字段级 read/write/filter/sort 四类权限强制（`validate_filter_field` / `ensure_fields_readable` / `can_*`）
- 类型化实体层封闭 `<Name>Field` / `<Name>Where` 枚举，杜绝任意列名拼接
- 软删除（自动 `IS NULL` 过滤 / `with_trashed` / 软删走 UPDATE）
- UPDATE 的 `MissingWhereClause` 守卫 + `allow_full_table` 全表守卫
- 自动 `updated_at` / 时间戳填充、INSERT 默认值填充
- offset 分页 + `PaginatedResult`（total/total_pages/has_next/has_prev）与 COUNT(*)

**启动期生命周期**
- 插件：依赖完整性校验 + Kahn 拓扑排序 + 循环依赖检测 + 拓扑逆序 `on_shutdown`，register/build 两阶段分离
- 统一初始化 `DatabaseBundle::init`（固定先 MySQL 再 Redis、fail-fast、诚实记录半初始化不可回滚）
- 幂等迁移（`_migrations` 版本表、事务/非事务两模式、参数化记录防注入）

**错误与响应**
- `BaseError` 60+ 变体按 9 域组织，`#[source]` 保留底层错误链（有测试验证 `source()` 可遍历）
- 稳定 6 位 `code()` 且每变体单测锁定数值，thiserror 中文 Display
- `ApiResponse` 形态统一，`success` 返回 `Result`（序列化失败不静默吞错）
- `Result<T, BaseError>` 别名导出，全局 `#![warn(missing_docs)]`

**并发基础纪律（真功夫）**
- 单例 `OnceLock::set` 语义，重复 init 返回结构化 Err、绝不覆盖、绝不 panic，无 TOCTOU
- 锁绝不跨 await（临界区纯同步、await 前释放）——从根上排除 async 死锁
- 异步共享态用 `tokio::sync::RwLock`，短临界区用 std 锁，选型正确
- `TokenManager` 无共享可变态，verify/generate 天然线程安全

**类型化边缘**
- 三层 trait 闭环（`TypedHandler` / `TypedAction` / blanket `DynAction`），编译期 Input/Output 契约
- 内置 CRUD 在 handler 边界是具体类型：add `Input=T`、get `Output=T`、select `Input=SelectQuery<T>`
- `serde_json::Value` 在合理开放边界（HTTP body / JWT custom claims / DynamicRow 解码）隔离得当

## 四、Tier-Core：完备核心引擎必备、当前缺失

按补齐杠杆排序。tier=core 表示「完备核心引擎承重柱」，缺了它引擎不完整。

### C1. 事务无法进入受保护层（最大承重缺口，effort L）
- **现状**：`ActionContext::table_query()` 永远注入 `db.pool().clone()`（context.rs:368-375）；`TableQuery` 仅持 `Option<Arc<MySqlPool>>`（table_query.rs:129）；所有 execute 走 `pool.as_ref()`，无接受 `&mut Transaction` 的执行变体（grep Transaction/begin 零命中）。
- **后果**：受权限/校验/软删保护的多步写无法原子提交。多表原子业务只能整段跌落 `GlobalDatabase::transaction`（global.rs:328）裸 SQL，丢掉全部保护。
- **杠杆**：同时是批量插入 / UPSERT 有意义的前置。最大承重缺口且仅 L，杠杆最高。

### C2. 查询表达力封顶在 AND-only 单表
- **OR / 嵌套布尔（effort M）**：`append_where_to_sql` 分隔符写死 ` AND `（table_query.rs:1151），`where_conditions` 平铺 Vec（query_params.rs:346），`WhereOp` 无 OR（entity.rs:85）。`status=1 OR vip=true` 这类查询一律跌落原始 SQL。底层 yang-db 已有 `where_or`，属低成本桥接、极高杠杆。
- **JOIN / 关联预加载（effort XL）**：`RelationConfig`/`RelationType` 定义齐全（field_config.rs:101/295/594，含 1:1/1:N/M:N），但 table_query.rs 对 relation **零引用——死配置**。跨表读取一律退化为应用层 N+1 或原始 SQL。

### C3. 类型化层钉死 MySQL（effort XL）
- **现状**：`TableEntity` 约束 `FromRow<'r, MySqlRow>`（entity.rs:21），`TableQuery` 用 `MySqlPool`/`MySqlRow`/`MySqlArguments` 全路径（table_query.rs:129/1017/1353/1471）。
- **后果**：yang-db 近期已加 PostgreSQL 后端，**落后的恰是受保护层**。完备引擎（SeaORM/Django/GORM）均以多后端为基线。

### C4. 请求执行热路径零可观测性（一簇，effort ~L）
- **现状**：dispatch→authorize→handler 全程无 log/span/计时（module_router.rs:327-405、typed.rs:82-92）；依赖是 `log` 而非 `tracing`（grep tracing/instrument/span 零命中）；Request/ActionContext 无 `request_id`/`trace_id`（request.rs:48-60、context.rs:202-211）；全仓无 metrics/prometheus。
- **后果**：线上一次失败的 Action 派发不留任何痕迹，无法串联请求、无法量化 QPS/P99/错误率。

### C5. 错误缺引擎级分类 API（effort M）
- **现状**：`impl BaseError` 只有 `code()`（error/mod.rs:444），无 `category()`/`is_retryable()`/`is_client_error()`/`is_server_error()`（grep 零命中）。
- **后果**：这是弹性重试与任意下游适配的共同基座（仅指引擎自有分类语义；具体 HTTP status 映射属调用方传输层边界，不计入）。

### C6. 并发正确性无回归测试背书（effort L）
- **现状**：并发**设计**是真功夫（见第三节），但全仓 grep `loom`/`worker_threads`/stress 测试**零命中**。单例、熔断器、三处 RwLock 缓存、plugin TOCTOU 全部只有单线程测试。
- **后果**：「生产级并发」的宣称没有任何多线程验证背书，回归无法捕获竞态。

## 五、Tier-Important：生产常需但非必备

### 类型安全的两处真漏气
- **PutInput 值未按字段类型校验（effort M）**：`PutInput.data: Vec<(T::Field, serde_json::Value)>`（put.rs:27）—— 列名由 Field 枚举保证封闭安全，但配对的新值是任意 `serde_json::Value`，从不在编译期按字段类型校验。这是 CRUD 六件套里**唯一「列名 typed 但值 untyped」的破洞**：客户端可发 `{"id":1,"data":[["age","非数字"]]}`，只在运行期报错。可仿 `WhereCond` 由派生宏生成定型更新枚举收口。
- **typed 层 ↔ yang-db 写 API 的接缝（effort L）**：add/put 立刻 `serde_json::to_value(&input)` 摊平成 `HashMap<String, Value>`（add.rs:60-71 / put.rs:68-74）再喂给 yang-db 写 API。列名安全靠 `&'static str` 保住，但值类型在此接缝完全擦除。属架构边界（validator 运行期复检），但「端到端 typed」承诺在此即停。

### 弹性补齐
- **连接池自愈参数（effort S）**：`max_lifetime`/`test_before_acquire`/`min_connections` 全仓零命中，sqlx 原生支持只差接出。无它则 failover/wait_timeout 杀掉的连接会先失败一次才被替换。成本极低、收益高。
- **优雅停机 / drain / 连接池 close（effort M）**：`GLOBAL_DB`/`GLOBAL_REDIS` 用 OnceLock 持有、无 Drop/close/drain，无信号处理；插件 `on_shutdown` 不触及 DB/Redis 池。K8s 滚动更新会 RST 在途连接。
- **配置体系 env/文件分层（effort M）**：全仓无 `env::var`/`dotenv`/`figment`，连接串与池参数只能编程式硬传，违背 12-factor。

### 写路径与统计完备（底层 yang-db 都有，未桥接）
- 批量插入（effort M）：受保护写路径只能逐行、非原子
- UPSERT ON DUPLICATE/ON CONFLICT（effort M）
- 游标 / keyset 分页（effort M）：仅 LIMIT/OFFSET，深翻页线性退化、并发写下结果漂移
- GROUP BY / HAVING / 聚合 SUM/AVG/MAX/MIN（effort L）：受保护层仅 count()

### 观测细项
- 认证审计钩子（effort S）：Login/Refresh/Logout 安全路径全静默（auth.rs:192/355/430）
- MySQL `pool_status`（effort S）：Redis 侧已有，MySQL 侧仅 health_check，连接耗尽无据可查
- 慢查询日志（effort M）：`enable_logging` 仅 debug 打印 SQL 文本，无耗时/阈值
- 插件/模块 unregister 与热重载（effort M）：注册后不可变，任何变更需重启进程

### 错误硬化
- `#[non_exhaustive]`（effort S）：error/mod.rs:31 缺失，新增变体即 SemVer 破坏性变更
- **非测试代码 panic 收口（effort S）**：`entity.rs:237` 在类型化 WhereOp 序列化热路径 `expect`（**最值得修，恰在受保护查询路径上**）、`circuit_breaker.rs:82/106/124` 锁中毒 `expect`、`validator.rs:42/52` 静态正则 expect
- ApiResponse 类型擦除 + 结构化错误明细被拍扁（effort L+M）：`data: Option<Value>` 擦除强类型 Output；`from_error` 只取 `code()+to_string()`，丢弃 `ValidationFailed` 等变体携带的字段/原因

### 并发运营成熟度
- **锁中毒策略自相矛盾（effort S）**：circuit_breaker 用 `.expect`（中毒即 panic），context/validator/client 用 `into_inner()` 恢复——同一 crate 两套相反策略，且熔断器恰是最关键路径
- **plugin register check-then-insert TOCTOU（effort M）**：plugin/mod.rs:248-265 读锁查重→await→写锁 insert，并发同名注册会都通过、第二个静默覆盖
- **dispatch 无背压/并发上限（effort M）**：无 Semaphore/有界队列，突发流量靠连接池被动限流而非主动背压

## 六、Tier-Nice：锦上添花

- 受保护层字段表达式/别名/计算列/DISTINCT（effort M）：`select_fields` 仅对裸列名 quote_identifier
- 子查询 / EXISTS / IN(SELECT) 与多列 RETURNING（effort L）：insert 仅回 last_insert_id 单值
- 大结果集流式读取（effort M）：`select()` 一次性 `fetch_all` 进 Vec，无 `.stream()`/分块游标
- DB/Redis 瞬时错误重试/退避（effort M）：依赖 core 的 `is_retryable` 分类
- 统一健康/就绪聚合端点（effort S）：DB/Redis health_check 分散，无聚合 readiness gate
- 事件/钩子总线（effort L）：除中间件洋葱链外无 emit/subscribe
- 错误人体工学构造器与 `.context()` 链（effort M）：仅能裸 `BaseError::Variant("...".to_string())` 构造
- 内置 Action Output DTO 一致性（effort S）：`AffectedResult` 定义在 add.rs 却被 put/del 反向依赖；成功文案 typed.rs:86 硬编码「成功」
- 错误码与变体双射 / 去重（effort S）：`DatabaseConnectionFailed` 与 `...DbError` 同码 200001、Redis 同码 210004、Migration 双变体
- extract_input 的 schema 校验闸（effort S）：input_schema 已生成但未当运行期校验闸；每次 dispatch 克隆整个 body
- cancellation 半状态收口（effort M）：熔断器探测名额消耗后若 await 被取消卡在 HalfOpen；LogoutAction 两次 revoke 之间取消留半登出会话
- 单例 connect-then-set 并发资源浪费（effort S）：并发 init 会各自建池，后者 set 失败但池已建

## 七、建议补齐顺序（按依赖与杠杆）

1. **事务传播进受保护层**（core L）—— 最大承重且仅 L，杠杆最高，是批量/UPSERT 前置。给 TableQuery 增加接受 `&mut yang_db::Transaction` 的执行变体，ActionContext 暴露事务作用域 API。
2. **OR / 嵌套布尔桥接**（core M）—— `where_conditions` 引入 Or/Group 变体，桥接 yang-db 既有 `where_or`。消除一大类「有 OR 就跌落原始 SQL」，低成本高杠杆。
3. **可观测性热路径一揽子**（core ~L）—— 切 tracing 并 span 化 dispatch→authorize→handler，入口生成并透传 request_id，挂 Action 计时/错误/吞吐 metrics + MySQL pool_status + 认证审计钩子 + 慢查询阈值日志。顺带带走多个 important。
4. **错误分类 + 契约硬化 + panic 收口**（core M + important S×3）—— `category()`/`is_retryable()`/`is_client_error()`/`is_server_error()`，标 `#[non_exhaustive]`，收掉 `entity.rs:237` 等可达 panic。为第 5 步重试做前置。
5. **弹性补齐**（important）—— 池自愈参数（S）+ 优雅停机 drain/close（M）+ 瞬时错误重试退避（M，依赖第 4 步）+ 配置体系 from_env/文件分层（M）。
6. **写路径与统计完备**（important）—— 批量插入、UPSERT、游标分页、GROUP BY/聚合，经受保护层桥接到 yang-db 既有能力（批量/UPSERT 复用第 1 步事务）。
7. **数据库无关性**（core XL）—— 抽象 `TableEntity` 脱离 `FromRow<MySqlRow>`、`TableQuery` 泛型化脱离 MySqlPool，打通 PG。放在 OR/事务/写路径成形后一次性抽象（会重写第 2/6 步部分 MySQL 专用代码，是有意接受的张力）。
8. **JOIN / 关联预加载**（core XL，压轴）—— 把闲置的 `RelationConfig` 真正接入查询构建，复用第 2 步布尔基座。
9. **nice 收尾** —— 字段表达式/别名/DISTINCT、子查询/多列 RETURNING、流式读取、健康聚合端点、事件总线、错误工效糖与码双射去重。

## 八、并发回归测试（横切，建议尽早补）

C6 是横切所有改动的承重项：补 `#[tokio::test(flavor = "multi_thread")]` + stress / 必要处 `loom` 验证，覆盖 OnceLock 单例、熔断器状态机、三处 RwLock 缓存、plugin register TOCTOU。在动第 1、3 步前先有并发回归网，避免新增并发路径无背书。

## 九、维度成熟度速查

| 维度 | 成熟度 | 一句话 |
|------|--------|--------|
| 数据 / 查询完备性 | partial | 稳健单表 CRUD，但无事务/OR/JOIN/聚合/多后端 |
| 类型安全完整性 | partial（偏成熟）| 边缘兑现，put 值 untyped + 写库接缝 + 响应擦除三处漏气 |
| 可观测性 / 可运维 | skeletal | 冷路径有日志，请求热路径完全不可观测 |
| 并发正确性 | partial | 设计是真功夫，缺统一中毒策略 + 回归测试 + 停机 |
| 生命周期 / 弹性 | partial | 启动期扎实，运行期/停机期弹性几乎空白 |
| 错误体系 / API | partial | 覆盖广有错误码，缺传输分类 + non_exhaustive + 类型保真 |

> 评估方法：内联探查锁定缺口候选 → 三轮 subagent 工作流并行核查（就绪度 7 维 + 成熟度 4 维 + 补跑 2 维）→ 每条结论 file:line 取证。本文为评估快照，代码演进后需复核对应 file:line。

---

## 十、yang-db 底层复核（2026-06-05 追加）

> 本节为对照本文档、对底层 crate `yang-db` 的独立复核：核实文档涉及 yang-db 能力的论断是否属实，并补充文档未触及的 yang-db 自身问题。方法同前——工作流并行审查（MySQL 查询构建 / 执行写 / 连接事务、PostgreSQL 后端、Redis、错误/导出/测试、文档论断逐条核对 7 维）+ 对抗式复核，每条结论 file:line 取证；共 67 条经核保留、1 条被否决。所引行号以复核时 `master` 为准。

**一句话结论**：文档对 yang-db **能力归属**的论断绝大多数属实（where_or / PG 后端 / 批量插入 / UPSERT / 聚合 / RedisConfig 生效 / 池自愈参数缺失 / Redis pool_status 等全部坐实），「缺口在 yang-base 受保护层未桥接」的定性正确。**两处分类/归属需修正**（游标分页、MySQL health_check 归属）。但 yang-db 自身另有一批文档未覆盖的问题，其中 **3 项 high**（标识符裸拼注入面、Redis WATCH 冲突被静默吞、PG 单行 UPDATE 不内联 NULL）值得优先处理。

### 10.1 文档论断核实 —— 属实（doc-confirmed）

下列文档论断经 file:line 取证**确认与代码一致**，可信赖：

| 文档论断 | 核实结论 | 取证（yang-db） |
|---|---|---|
| 底层 yang-db 已有 `where_or`（低成本桥接） | 属实，且为 checked API（非法 op 返回 `UnsupportedOperator` 不 panic） | `mysql/query_builder.rs:984`、`postgres/query_builder.rs:1003`、`condition.rs:46`(Condition::Or) |
| yang-db 近期已加 PostgreSQL 后端 | 属实，7 文件齐全、lib.rs 以 `Pg*` 重导出，commit 8577348（2026-06-03） | `lib.rs:8/25-29`、`postgres/` 整目录 |
| `insert_batch` 默认 500 | 属实，两后端 `INSERT_BATCH_SIZE=500`，可用 `insert_batch_with_size` 自定义 | `mysql/query_builder.rs:47/2064`、`postgres/query_builder.rs:52/1411` |
| batch_size==0 在任何 DB 操作前返回错误（不 panic） | 属实，0 值/空切片均提前返回 `SerializationError` | `mysql/query_builder.rs:2114-2119/2132-2136`，单测 `:5272` |
| UPSERT（ON DUPLICATE / ON CONFLICT）底层 yang-db 都有 | 属实，双后端齐备且参数化正确 | MySQL `query_builder.rs:640-687/2518`、PG `postgres/query_builder.rs:633-711/1675` |
| GROUP BY / HAVING / 聚合 SUM/AVG/MAX/MIN 底层都有 | 属实（受保护层仅 count()，措辞建议明确主语） | `mysql/query_builder.rs:1141/1249/1589/1645/1709/1787/1867` |
| insert 仅回 `last_insert_id` 单值（无多列 RETURNING） | 属实，PG 也仅取单个可配置 RETURNING 列 | MySQL `query_builder.rs:1977-1981`、PG `postgres/query_builder.rs:1372-1388` |
| `having_cond_unchecked` 会 panic | 属实，已标 `#[deprecated]`；`where_and/or_unchecked` 同样 panic（未标弃用） | `mysql/query_builder.rs:962/1040/1192-1198`、PG `:930/942/960` |
| update/delete 无 WHERE 返回 `MissingWhereClause` | 属实，两后端 query_builder + transaction 路径都强制 | `mysql/query_builder.rs:2288/2370`、`transaction.rs:395/460`、`error.rs:32` |
| RedisConfig 连接池/超时参数已被 `connect_with_config` 应用 | 属实，max_connections→max_size、wait_timeout→wait、connect_timeout→create/recycle | `redis/client.rs:78-86`、`config.rs:68-75` |
| Redis 侧已有 `pool_status` | 属实，转发 deadpool `pool.status()`；Redis 还另有 health_check | `redis/client.rs:1742-1750/7-16/1730` |
| `max_lifetime`/`test_before_acquire`/`min_connections` 全仓零命中 | 属实，两 DB 后端均只接出 max_connections/acquire_timeout/idle_timeout（源码零命中） | `mysql/database.rs:98-103`、`postgres/database.rs:97-102` |
| 12 种 WHERE 操作符全部参数化绑定（值侧） | 属实（针对 yang-base 受保护层）；yang-db condition 层值侧同样全 `?` 绑定无注入 | `mysql/condition.rs:156-204`、bind 宏 `query_builder.rs:18-41` |
| PG $N 占位符编号正确性 | 属实，递增连续、与绑定顺序严格一致，事务路径复用同一 generator 编号一致 | `postgres/condition.rs:137-140`、`postgres/query_builder.rs:62-65/482-494/421-447` |

### 10.2 文档论断核实 —— 需修正（doc-inaccuracy / 归属澄清）

| 文档论断 | 问题 | 修正 | 取证 |
|---|---|---|---|
| 「游标 / keyset 分页」列在「写路径与统计完备（底层 yang-db 都有，未桥接）」小节下（第 99/102/143 行） | **分类错误**：与批量/UPSERT/聚合不同，游标分页 yang-db 底层**也没有**，只有 LIMIT/OFFSET。条目正文「仅 LIMIT/OFFSET」已自承，但小标题「都有」与路线图第 6 步「桥接到既有能力」会误导——这是真·新建工作量，非桥接 | 从「底层都有」小节移出，标注 yang-db 与 yang-base 双层都缺，需在 query_builder 新建 seek 分页 | `mysql/query_builder.rs:1255/1261`、`postgres/query_builder.rs:1149/1155`；全仓无 keyset/seek/cursor SQL 分页 |
| 「MySQL 侧仅 health_check」「MySQL pool_status：Redis 侧已有」（第 107 行） | **归属含糊**：在 yang-base 层（GlobalDatabase）字面成立；但若读作 yang-db 层则不准——yang-db 的 `mysql::Database` **既无 health_check 也无 pool_status**，health_check 是 yang-base 用 `SELECT 1` 实现的 | 注明 health_check 属 yang-base `global.rs:349`；yang-db 的 MySQL Database 两者皆缺，补 pool_status 时需同时补 health_check（可包 `pool.size()/num_idle()`） | `mysql/database.rs:79-281`（无两方法）、`yang-base global.rs:349`、`redis/client.rs:1730/1742` |
| `docs/yang-db.md:276-277` 称 yang-db 用 `quote_identifier()`/`is_valid_identifier()` 转义/校验标识符 | **不实**（旁证，非本文档错）：这两个函数只在 yang-base `table_query.rs:140/167`，yang-db crate 内零命中。直接消费 yang-db 者会误以为标识符已自动转义 | 修正 `yang-db.md` 注入防护小节：yang-db 仅对**值**参数化，标识符安全由调用方/上层保证 | `yang-base table_query.rs:140/167`；yang-db 内零命中 |
| health_check 文档声明可返回 `Err(DbError): 无法获取连接`（Redis） | **不实**：实现 `Err(_) => Ok(false)` 吞掉全部错误，Err 分支不可达，恒返回 Ok | 修文档：任何异常都返回 `Ok(false)` 不返回 Err；或修代码区分「PING 失败」与「连不上」 | `redis/client.rs:1726-1736` |

> **被否决（1 条）**：有审查臂提出「文档 12 操作符清单含 yang-db 不存在的 NotIn」属 doc-inaccuracy，经复核**否决**——文档第 28 行的操作符清单是对 yang-base 受保护层的论断，`WhereOp::NotIn` 在 yang-base `entity.rs:101` 真实存在且全参数化（`table_query.rs:1219-1231`），不经 yang-db 的 Condition 枚举。文档此处准确。

### 10.3 yang-db 自身问题 —— 文档未覆盖（undocumented）

文档全程评估 yang-base，未把作为数据访问基座的 yang-db 自身缺陷纳入。下列为本轮新发现、经对抗式复核保留的 yang-db 问题（按严重度）：

**High（优先处理）**

1. **标识符全程裸拼接，public API 直接暴露注入面**（safety）。`SqlGenerator::append` 仅 `push_str` 无转义；表名/列名/order_by 字段/group_by 字段/select 字段/JOIN table 与**整段 ON 子句**/UPDATE SET 列名/INSERT 列名全部裸拼。`db.table(name)`/`.field()`/`.order()`/`.where_and(field,..)`/`.join(table,on)` 均收任意 `&str`，绕过 yang-base 类型层直接喂外部输入即可注入（order_by 动态排序列、JOIN ON 自由 SQL 是最大面）。yang-db 内不存在 `quote_identifier`/`is_valid_identifier`。值侧已全参数化（安全）。取证：`mysql/query_builder.rs:88-90/145/150/243-245/266-268/282/341/490`；PG 同构 `postgres/condition.rs:172`、`postgres/query_builder.rs:377/379/488/604-606`。
2. **Redis WATCH 事务冲突在兼容模式 `execute()` 下被静默吞成空结果**（bug）。WATCH 键被并发改写导致 EXEC 返回 Nil 时，`execute()` 以 `Vec<redis::Value>` 接收，`from_redis_value(Nil)→Ok(vec![])`，不触发冲突重试分支，调用方误以为事务成功而写入实际未生效——破坏乐观锁语义。`()` 返回类型同样受影响。取证：`redis/transaction.rs:368-371/318-340`。
3. **PG 单行 `update()` 不内联 NULL，对非整型列设 NULL 运行时报错**（bug）。同文件其余 4 个写入构建器（insert/insert_batch/update_batch/upsert）都显式内联 `NULL` 字面量（注释明示 PG 拒绝未类型化绑定 NULL），唯独 `build_update` 的 SET 循环无条件 `push_placeholder`（绑定为 INT4 的 NULL）。`SET text_col = $1` 会报 column-type 不匹配——内部不一致的大概率运行时 bug。取证：`postgres/query_builder.rs:486-487`（对比 `369-372/437-444/582-596/666-672`）。

**Medium**

4. **`insert_batch` 多批次（>500 行）非原子**（bug）。每个 chunk 独立 `execute(pool)` 无事务包裹，第 N 批失败时前 N-1 批已落库无法回滚；而同文件 `update_batch` 用单事务包裹所有 chunk——同 crate 两个批量写 API 原子性语义不一致。取证：`mysql/query_builder.rs:2141-2152/2221`（对比 `2464-2490`）。
5. **无接受 `&mut Transaction` 的执行变体**（undocumented-gap）。事务内只能逐条 insert/update/delete（`TransactionQueryBuilder` 仅这三个方法），批量/upsert/select/聚合在事务上下文不可用。文档把「事务执行变体」定位为 yang-base 缺口，根因实在 yang-db。取证：`mysql/transaction.rs:310/378/446`、`query_builder.rs:1973/2221/2546`。
6. **`table_exists`/`drop_table`/`init` 用 `format!` 裸拼表名/脚本**（safety）。`table_exists` 把表名插进单引号字面量未转义（含单引号即可注入）；PG 同名方法已用 `$1` 参数化——同库内不一致。当前无调用方传用户输入（潜在面）。取证：`mysql/database.rs:184-190/177/154-167`（对比 PG `postgres/database.rs:182-193`）。
7. **`DbError` 无 `code()`/`category()`/`is_retryable()`，未标 `#[non_exhaustive]`**（undocumented-gap）。文档错误体系章节只评 yang-base 的 `BaseError`，但 `DbError`（18 变体）比它更欠缺——连稳定错误码都没有，新增变体即 SemVer 破坏；下游包装时丢失可重试性语义。取证：`error.rs:2-59`。
8. **`From<redis::RedisError>` 靠 Display 子串分类，顺序相关且 "type" 匹配过宽**（bug，复核下调为 low 影响）。`contains("Connection")` 先于 `contains("timeout")`，`contains("type")` 会误判任何含 "type" 的消息；redis 升级改文案即静默失配。应改用 `RedisError::kind()`。唯一消费者把五个 Redis 变体统一坍缩，仅改人类可读前缀、不影响控制流。取证：`error.rs:117-128`。
9. **PG 事务 insert 硬编码 `RETURNING CAST(id AS BIGINT)`**（bug）。忽略 `.returning()` 覆盖，非 id 主键表在事务内 insert 必失败，而非事务路径可用 `.returning("uid")`——双重对等性缺口。取证：`postgres/transaction.rs:335`（对比 `query_builder.rs:921-924/1372-1376`）。
10. **PG 事务原生查询助手把浮点参数绑为字符串**（bug）。照搬 MySQL（text→number 宽容），但 PG 类型严格，`WHERE price = $1`（numeric 列）会报 `operator does not exist: numeric = text`。取证：`postgres/transaction.rs:570-571/607-609`。
11. **MySQL 集成测试 `if let Ok(db)` 静默放过，DB 不可用时假绿**（test-gap）。既不标 `#[ignore]` 也不在连接失败时 fail，无 MySQL 时全部「通过」却什么都没验证；连纯 SQL 生成断言也锁在 `if let Ok` 内。对比 PG 测试正确用 `#[ignore]`、aggregate 测试用 testcontainers——同 crate 三套互斥风格。取证：`integration_database.rs:54/70/85`、`integration_crud_simple.rs:17` 等。

**Low**

12. **LIKE 通配符 `%`/`_` 由调用方自带且不转义**（undocumented-gap）。pattern 走 `?` 参数（无 SQL 注入），但 `%{input}%` 里用户输入的 `%`/`_` 会被当通配符导致意外宽匹配；代码/文档无提示。取证：`mysql/condition.rs:198-202`。
13. **非字符串值用 LIKE 时把值的 Debug 表示当 pattern**（bug）。`where_and("name","like",5i32)` 产出 pattern `"Int(5)"` 而非 `5`，语义错误且静默。取证：`mysql/query_builder.rs:939-942/1003-1005`、`transaction.rs:279-282`。
14. **`SerializationError` 被挪用为参数校验错误**（undocumented-gap）。batch_size==0 / 空数据 / 非 JSON 对象都复用 `SerializationError`，污染「序列化失败」语义，调用方无法按变体区分。建议新增 `InvalidArgument`。取证：`mysql/query_builder.rs:2116-2118/2133-2135`、PG `1426/1441`。
15. **Transaction 无自定义 Drop，未显式 commit/rollback 时依赖 sqlx 尽力回滚**（undocumented-gap）。语义正确但未文档化「drop 即回滚是尽力而为」，调用方易误以为有强保证。commit/rollback 后再操作返回 `TransactionError`（健壮，无 panic）。取证：`mysql/transaction.rs:7-46/54-59`。
16. **Redis 二进制（非 UTF-8）值读回为 None，与「键不存在」不可区分**（undocumented-gap）。typed 便捷方法统一过 `as_string()`；可经 public `execute()`+`as_bytes()` 逃生舱读取，但 sugar 不暴露。取证：`redis/client.rs:261-282`、`value.rs:39-44/87-92`。
17. **RESP3 协议下 Map/Set/Push 被降级为 Debug 字符串，HGETALL/SMEMBERS 静默返回空**（undocumented-gap）。默认 RESP2 正常（潜在 bug）；URL 显式 `?protocol=resp3` 时数据被静默吞。取证：`redis/value.rs:178-205`、`client.rs:660-682`。
18. **PG 两套后端大段平行实现，NULL 内联分叉即漂移实证**（undocumented-gap）。无共享抽象靠人工同步，单行 update NULL 漏改正是「改一处忘改另一处」。建议补跨后端一致性断言测试。取证：`postgres/query_builder.rs` 与 `mysql/query_builder.rs` 同名方法平行。
19. **事务 commit/rollback 集成测试 DB 不可用时静默 return；Redis 测试 connect 硬 `.expect()` 无 `#[ignore]`**（test-gap）。两套相反策略（MySQL 假绿、Redis 真红）都偏离离线可跑预期；`cargo test`（非 --lib）无 Redis 时变红。取证：`integration_database.rs:192-247`、`tests/test_redis_*.rs:6`。
20. **MySQL 集成测试硬编码 `root:111111` 凭据，未用 `MYSQL_TEST_PASSWORD`**（safety，违约定）。PG 测试遵守约定走 `PG_TEST_URL`，MySQL 全部明文硬编码进 const 且 git 跟踪。取证：`integration_database.rs:9` 等。
21. **error.rs 的 `From` 映射（sqlx SQLSTATE / redis 分类）几乎无单测，`MissingGroupByClause` Display 从未断言**（test-gap）。取证：`error.rs:62-112/114-129/160-172`。

> **附澄清**：`.gitignore` 的 `*/tests/` 仅匹配顶层 crate 的 `tests/`，`crates/yang-db/tests/` 两层深不受影响——17 个测试文件全部被跟踪（已 `git check-ignore` 实测）。CLAUDE.md 的告诫对 yang-db 不构成实际影响。

## 十一、施行进度追踪表

> 统一追踪本文识别的所有补齐项与修复项。状态：⏳ 待办 / 🟨 进行中 / ✅ 已完成 / ⛔ 暂缓。初始状态均为 ⏳（评估快照，尚未动工）。「层」标明工作落点：base=yang-base 受保护层，db=yang-db 底层，doc=文档。完成时回填 commit 与日期。

### 11.1 yang-base 核心承重项（第四节 Tier-Core）

| ID | 项目 | 层 | effort | 优先级 | 状态 | 备注 / commit |
|----|------|----|--------|--------|------|------|
| C1 | 事务传播进受保护层（TableQuery 接受 `&mut Transaction`） | base+db | L | P0 | ⏳ | 最大承重，批量/UPSERT 前置；根因含 db 侧无事务执行变体（见 DB-5） |
| C2a | OR / 嵌套布尔桥接（`where_conditions` 引入 Or/Group） | base | M | P0 | ⏳ | db 侧 `where_or` 已就绪，低成本桥接 |
| C2b | JOIN / 关联预加载（接入闲置 `RelationConfig`） | base | XL | P3 | ⏳ | 压轴，复用 C2a 布尔基座 |
| C3 | 类型化层脱离 MySQL（`TableEntity`/`TableQuery` 泛型化打通 PG） | base | XL | P2 | ⏳ | db 侧 PG 后端已就绪 |
| C4 | 请求热路径可观测性（tracing/span/request_id/metrics/慢查询/审计钩子） | base | ~L | P1 | ⏳ | 一揽子，带走多个 important |
| C5 | 错误引擎级分类 API（`category()`/`is_retryable()`/`is_client/server_error()`） | base | M | P1 | ⏳ | 弹性重试基座；db 侧 DbError 同缺（见 DB-7） |
| C6 | 并发正确性回归测试（multi_thread + stress + loom） | base | L | P1 | ⏳ | 横切，动 C1/C4 前先有网 |

### 11.2 yang-base 生产常需项（第五节 Tier-Important，节选）

| ID | 项目 | 层 | effort | 状态 | 备注 |
|----|------|----|--------|------|------|
| I1 | PutInput 值按字段类型校验（派生定型更新枚举） | base | M | ⏳ | 唯一「列名 typed 值 untyped」破洞 |
| I2 | 连接池自愈参数（min_connections/max_lifetime/test_before_acquire） | db | S | ⏳ | sqlx 原生支持只差接出，成本极低（见 DB 复核确认） |
| I3 | 优雅停机 / drain / 连接池 close | base+db | M | ⏳ | OnceLock 无 Drop，K8s 滚动会 RST |
| I4 | 配置体系 env/文件分层（from_env） | base | M | ⏳ | 违背 12-factor |
| I5 | 批量插入桥接受保护层 | base | M | ⏳ | db 侧已有，复用 C1 事务（注意 DB-4 非原子） |
| I6 | UPSERT 桥接受保护层 | base | M | ⏳ | db 侧已有 |
| I7 | 游标 / keyset 分页 | base+db | M→L | ⏳ | **db 底层也缺**（修正后），需先在 query_builder 新建 |
| I8 | GROUP BY / HAVING / 聚合桥接受保护层 | base | L | ⏳ | db 侧已有 |
| I9 | `#[non_exhaustive]` + 非测试代码 panic 收口（entity.rs:237 等） | base | S | ⏳ | entity.rs:237 在受保护查询热路径 |
| I10 | 锁中毒策略统一（circuit_breaker 与 context/validator 相反） | base | S | ⏳ | 熔断器恰是最关键路径 |
| I11 | plugin register check-then-insert TOCTOU | base | M | ⏳ | 并发同名注册静默覆盖 |
| I12 | dispatch 背压/并发上限（Semaphore） | base | M | ⏳ | 无主动背压 |

### 11.3 yang-db 自身问题（第十节新发现）

| ID | 项目 | 类型 | 严重度 | 状态 | 取证 |
|----|------|------|--------|------|------|
| DB-1 | 标识符全程裸拼接，public API 注入面（补 quote/校验，JOIN ON 标注可信） | safety | High | ⏳ | `mysql/query_builder.rs:88-90/150/243-245/266-268`、PG 同构 |
| DB-2 | Redis WATCH 冲突在 `execute()` 被静默吞成空结果（显式检测 EXEC Nil） | bug | High | ⏳ | `redis/transaction.rs:368-371/318-340` |
| DB-3 | PG 单行 `update()` 不内联 NULL，非整型列设 NULL 运行时报错 | bug | High | ⏳ | `postgres/query_builder.rs:486-487` |
| DB-4 | `insert_batch` 多批次非原子（比照 `update_batch` 单事务包裹） | bug | Medium | ⏳ | `mysql/query_builder.rs:2141-2152`（对比 2464-2490） |
| DB-5 | 无接受 `&mut Transaction` 的执行变体（C1 的 db 侧前置） | gap | Medium | ⏳ | `mysql/transaction.rs:310/378/446` |
| DB-6 | `table_exists`/`drop_table`/`init` 裸拼表名（table_exists 改 `?` 绑定，对齐 PG） | safety | Medium | ⏳ | `mysql/database.rs:184-190/177`（对比 PG `:182-193`） |
| DB-7 | `DbError` 无 code()/category()/is_retryable()，未标 `#[non_exhaustive]` | gap | Medium | ⏳ | `error.rs:2-59` |
| DB-8 | `From<RedisError>` 靠 Display 子串分类（改用 `kind()`） | bug | Medium→Low | ⏳ | `error.rs:117-128` |
| DB-9 | PG 事务 insert 硬编码 `RETURNING CAST(id AS BIGINT)`（补 returning setter） | bug | Medium | ⏳ | `postgres/transaction.rs:335` |
| DB-10 | PG 事务原生助手把浮点绑为字符串（改 `bind(f)`） | bug | Medium | ⏳ | `postgres/transaction.rs:570-571/607-609` |
| DB-11 | MySQL 集成测试 `if let Ok(db)` 假绿（改 `#[ignore]`/testcontainers） | test-gap | Medium | ⏳ | `integration_database.rs:54/70/85` 等 |
| DB-12 | LIKE 通配符 `%`/`_` 不转义（文档说明 + 可选 like_literal） | gap | Low | ⏳ | `mysql/condition.rs:198-202` |
| DB-13 | 非字符串值用 LIKE 取 Debug 表示（类型不匹配返回 Err） | bug | Low | ⏳ | `mysql/query_builder.rs:939-942`、`transaction.rs:279-282` |
| DB-14 | `SerializationError` 被挪用为参数校验（新增 `InvalidArgument`） | gap | Low | ⏳ | `mysql/query_builder.rs:2116-2118/2133-2135` |
| DB-15 | Transaction 无自定义 Drop，未文档化「drop 即尽力回滚」 | gap | Low | ⏳ | `mysql/transaction.rs:7-46` |
| DB-16 | Redis 二进制值读回 None 与「键不存在」不可区分（补 get_bytes） | gap | Low | ⏳ | `redis/client.rs:261-282`、`value.rs:39-44` |
| DB-17 | RESP3 下 Map/Set/Push 降级为 Debug 串，HGETALL/SMEMBERS 静默返空 | gap | Low | ⏳ | `redis/value.rs:178-205`、`client.rs:660-682` |
| DB-18 | PG/MySQL 两套后端平行实现易漂移（补跨后端一致性断言测试） | gap | Low | ⏳ | `postgres/` 与 `mysql/query_builder.rs` 同名方法 |
| DB-19 | Redis 测试 connect 硬 `.expect()` 无 `#[ignore]`，无 Redis 即变红 | test-gap | Low | ⏳ | `tests/test_redis_*.rs:6` |
| DB-20 | MySQL 测试硬编码 `root:111111`，未用 `MYSQL_TEST_PASSWORD` | safety | Low | ⏳ | `integration_database.rs:9` 等 |
| DB-21 | error.rs 的 From 映射几乎无单测，`MissingGroupByClause` 文案未断言 | test-gap | Low | ⏳ | `error.rs:62-112/114-129/160-172` |

### 11.4 文档自身修正项（第十节 10.2）

| ID | 项目 | 状态 | 备注 |
|----|------|------|------|
| DOC-1 | 「游标/keyset 分页」移出「底层都有」小节，标注双层都缺 | ⏳ | 本文第 99/102/143 行 |
| DOC-2 | 注明 MySQL health_check 属 yang-base，db 的 Database 两者皆缺 | ⏳ | 本文第 107 行 |
| DOC-3 | 修正 `docs/yang-db.md:276-277` 关于 quote/校验的归属 | ⏳ | yang-db.md 注入防护小节 |
| DOC-4 | 修正 Redis `health_check` 文档（恒返回 Ok，不返回 Err） | ⏳ | `redis/client.rs:1726-1736` doc |

> 本表为活文档，随补齐推进回填状态/commit/日期。优先级建议遵循第七节顺序（事务→OR→可观测→错误分类→弹性→写路径→多后端→JOIN），yang-db 的 3 个 High（DB-1/2/3）可独立于 yang-base 节奏先行修复。



