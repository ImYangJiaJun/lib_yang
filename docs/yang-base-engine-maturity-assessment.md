# yang-base 核心引擎成熟度评估

> 本文整合三轮评估的结论：搭建就绪度（7 维）、引擎成熟度（4 维）、补充核查（类型安全 + 并发，2 维）。
> 所有结论均基于对当前代码的 file:line 取证；评估时分支 `master`。
>
> **2026-06-13 二次审计已并入**：60 agent 并行全量扫描 + 对抗式复核，确认 10 项 ✅ 全部真落地（2 处子项有设计偏差非退化）、校正大批 file:line 漂移（见 11.7）、修正 M-1 数字（73→实测 ≈418）、新发现 **24 条文档此前未覆盖的问题（NEW-1~24，见第十三节）**。代码已较快照演进（`query_builder.rs` 5345 行、事务文件多处变动），引用具体行号时以 11.7 校正表与第十三节为准。

## 一、总判定

**`solid_core_with_holes`** —— yang-base 是一个安全属性扎实的「受保护单表 CRUD 引擎 + 成熟启动期生命周期」，但还不是完备的生产级核心引擎。

承重墙是好的：参数化绑定 + 标识符转义（注入防线）、字段级四类权限、软删 / WHERE 守卫、插件拓扑生命周期、幂等迁移、统一错误码体系，H-1 类型化重构在边缘真的落地了。

**距完备核心引擎，差四类运行期承重能力：**

1. ~~受保护层无法跑在事务里（连自身多步写都不原子）~~ ✅ **已修复**（C1，2026-06-09）
2. 查询表达力封顶在 AND-only 单表（~~无 OR~~ ✅ 已有 OR/嵌套布尔，仍无 JOIN / 聚合）
3. 类型化层钉死 MySQL（yang-db 已有 PG 后端，落后的恰是受保护层）
4. ~~请求执行热路径对运行时完全不可观测~~ ✅ **已修复**（C4，2026-06-09）

一句话：引擎的「安全」和「启动期」已经成熟，四根运行期承重柱中**事务原子性（C1）和可观测性（C4）已落地**，剩余查询表达力（JOIN/聚合）和多后端（PG）两根待补。**整体完成度约 65%**（核心承重项 6/6 中 4 项落地；Important 12 项中 3 项落地；yang-db 21 项中 3 项修复；Nice 12 项全未动）。

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
- GROUP BY / HAVING / 聚合 SUM/AVG/MAX/MIN（effort L）：受保护层仅 count()

### 写路径需新建（yang-db 与 yang-base 双层皆缺）
- 游标 / keyset 分页（effort M→L）：两层都仅 LIMIT/OFFSET，深翻页线性退化、并发写下结果漂移。**非桥接**，需先在 query_builder 新建 seek 分页（`mysql/query_builder.rs:1255/1261`、`postgres/query_builder.rs:1149/1155`），再经受保护层接出。（分类修正见 10.2 / 11.4 DOC-1）

### 观测细项
- 认证审计钩子（effort S）：Login/Refresh/Logout 安全路径全静默（auth.rs:192/355/430）。**✅ 已由 C4 落地**：`AuthAuditHook`+`TracingAuditHook` 注入三动作，token 仅记指纹（2026-06-09）
- MySQL `pool_status`（effort S）：Redis 侧已有（`redis/client.rs:1742`）。**归属澄清**：yang-base 层有 `health_check`（`global.rs:349`，`SELECT 1`）但无 pool_status；yang-db 的 `mysql::Database` 则 health_check / pool_status **两者皆缺**（`mysql/database.rs:88-263`）。补 yang-db pool_status 时建议同补 health_check（薄包 `pool.size()/num_idle()`），连接耗尽方有据可查（见 11.4 DOC-2）。**✅ 已由 C4 落地**：`Database::pool_status`/`health_check` 下沉 yang-db，`GlobalDatabase::pool_status` 转发（2026-06-09）
- 慢查询日志（effort M）：`enable_logging` 仅 debug 打印 SQL 文本，无耗时/阈值。**✅ 已由 C4 落地**：`TableQuery` 执行边界 `timed` 计时 + `ObservabilityConfig` 可配阈值，超阈值 `tracing::warn`（2026-06-09）
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
| 数据 / 查询完备性 | partial | 稳健单表 CRUD + 事务传播 + OR/嵌套布尔，但无 JOIN/聚合/多后端 |
| 类型安全完整性 | partial（偏成熟）| 边缘兑现，put 值 untyped + 写库接缝 + 响应擦除三处漏气 |
| 可观测性 / 可运维 | **partial**（原 skeletal） | 请求热路径已 span 化 + request_id + 慢查询计时 + 审计钩子 + pool_status；仍缺 HTTP 出站日志、metrics exporter 接入 |
| 并发正确性 | **partial+**（改善） | 设计是真功夫，中毒策略已统一（I10），12 项 stress 回归测试已补（C6）；仍缺停机 drain、背压 |
| 生命周期 / 弹性 | partial | 启动期扎实，运行期/停机期弹性几乎空白（I2/I3/I4 全 ⏳） |
| 错误体系 / API | **partial+**（改善） | `ErrorCategory` + `is_retryable()` + `#[non_exhaustive]` 已落地（C5），yang-db `DbError` 同步补齐（DB-7）；仍缺类型保真响应 + 码双射去重 |

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

### 10.4 深度复核新增缺口（2026-06-06 追加）

> 本轮以工作流多维查漏 + 对抗式验证（每条须 isReal && isNew 双判方保留）补充，确认 4 项文档此前未单独枚举的缺口。均经 file:line 取证。

| ID | 缺口 | 层 | 类型 | 严重度 | 取证 / 后果 |
|----|------|----|------|--------|------|
| NG-1 | HTTP 客户端出站请求无日志/计时（method/url/status/duration 全无） | base | gap | Medium | `http/request.rs:608-638` send_once 全程无埋点；外部 API 失败/慢响应不可排查。属 C4 同主题但未枚举的出站位置 |
| NG-2 | PG/MySQL 事务隔离级别不可配置（裸 `pool.begin()`，无 SET TRANSACTION 入口） | db | gap | Medium | `postgres/database.rs:148`、`mysql/database.rs:149`；全 crate 无 isolation 配置。InnoDB 默认 RR、PG 默认 RC，跨后端隔离差异静默隐埋竞态 |
| NG-3 | PG `FieldType::Decimal` 降级为 f64，写 NUMERIC 列丢任意精度 | db | bug | Medium | `postgres/query_builder.rs:744-749` 经 as_f64→SqlValue::Float；财务字段受损。MySQL 后端同模式 |
| NG-4 | `SqlParam::from_json` 对 Object/Array 的错误路径无单测 | base | test-gap | Medium | `table_query.rs:2318-2321` 经 where_eq 可达 `DatabaseQueryFailed`，两处测试目录零覆盖。错误处理本身正确（不 panic），仅缺回归网 |

## 十一、施行进度追踪表

> 统一追踪本文识别的所有补齐项与修复项。状态：⏳ 待办 / 🟨 进行中 / ✅ 已完成 / ⛔ 暂缓。初始状态均为 ⏳（评估快照，尚未动工）。「层」标明工作落点：base=yang-base 受保护层，db=yang-db 底层，doc=文档。完成时回填 commit 与日期。
>
> **最近全量复核**：2026-06-13（workflow `wf_a4760338-2eb`，21 agent 并行扫描）。确认所有 ✅ 项代码落地属实，未发现新的已完成项变更。

### 11.1 yang-base 核心承重项（第四节 Tier-Core）

| ID | 项目 | 层 | effort | 优先级 | 状态 | 备注 / commit |
|----|------|----|--------|--------|------|------|
| C1 | 事务传播进受保护层（TableQuery 接受 `&mut Transaction`） | base+db | L | P0 | ✅ | 最大承重，批量/UPSERT 前置；根因含 db 侧无事务执行变体（见 DB-5）。`*_in_tx`（insert/insert_returning_id/update/delete/select）+ `ActionContext::begin_transaction`，run_execute/run_fetch_all 泛型收敛；9 项事务集成测试通过（2026-06-09） |
| C2a | OR / 嵌套布尔桥接（`where_conditions` 引入 Or/Group） | base | M | P0 | ✅ | `WhereCondition` 加 `And`/`Or` 递归变体 + `#[non_exhaustive]`（合并 I9）；`append_where_to_sql` 重构为递归 `render_condition`（组括号包裹，空 Or→`1=0`/空 And→`1=1`，深度上限 32 返 `ParamInvalid` 不爆栈）；`TableQuery::where_or`/`where_and`/`where_tree` 递归权限下钻；类型化层新增 `Filter<W>`（untagged 布尔树）+ `SqlCondition::into_where_condition`，`SelectQuery.where` 线格式由数组改为布尔树（**破坏性**，已确认）；附带修复 typed_action 集成测试假绿（漏设 table_config）。8 项单测 + 真实 MySQL OR 组 e2e 通过（2026-06-09） |
| C2b | JOIN / 关联预加载（接入闲置 `RelationConfig`） | base | XL | P3 | ⏳ | 压轴，复用 C2a 布尔基座 |
| C3 | 类型化层脱离 MySQL（`TableEntity`/`TableQuery` 泛型化打通 PG） | base | XL | P2 | ⏳ | db 侧 PG 后端已就绪 |
| C4 | 请求热路径可观测性（tracing/span/request_id/metrics/慢查询/审计钩子） | base | ~L | P1 | ✅ | 一揽子 7 子项全落地（2026-06-09）：tracing 设核心依赖 + dispatch/authorize/handle 三段 span；`RequestId`(u128)+`ActionContext.request_id`(#[non_exhaustive])+`RequestIdMiddleware` 透传 `X-Request-Id`；feature-gated `metrics`（DynAction 边界，`BaseError::code_str` 静态 label）；MySQL `pool_status`/`health_check` 下沉 yang-db 经 GlobalDatabase 暴露；`TableQuery` 慢查询计时（`timed` 包裹 11 处执行点，None 短路）；`AuthAuditHook`+`TracingAuditHook` 注入 Login/Refresh/Logout（token 仅记指纹）；`ObservabilityConfig` 单例收口。371 lib 测试 + 11 e2e 通过；三 feature 组合 clippy 干净 |
| C5 | 错误引擎级分类 API（`category()`/`is_retryable()`/`is_client/server_error()`） | base | M | P1 | ✅ | 弹性重试基座。`ErrorCategory{Client,Auth,NotFound,Conflict,Transient,Server}`（#[non_exhaustive]）；`BaseError` 加 `category()` 为单一事实源，`is_retryable()`/`is_client_error()`/`is_server_error()` 全委托派生。`is_server_error` 含 Transient+Conflict（按 §12.5 设计）。`BaseError` 已标 `#[non_exhaustive]`。导出 `ErrorCategory`。4 项新测试（category 覆盖/is_retryable/is_client_error/is_server_error）。yang-base 375 lib 测试通过（2026-06-09） |
| C6 | 并发正确性回归测试（multi_thread + stress + loom） | base | L | P1 | ✅ | 横切，动 C1/C4 前先有网。已补 12 项 stress 测试：circuit_breaker（Arc&lt;Mutex&gt; 计数不丢/共享）、REGEX_CACHE（同/异 pattern 并发编译）、GlobalTools（register/get 争用）、plugin register TOCTOU（锁定当前契约，I11 后收紧）。未引入 loom（multi_thread+stress 已覆盖回归网，loom 需 cfg 改写锁类型，留待按需）（2026-06-09） |

### 11.2 yang-base 生产常需项（第五节 Tier-Important，节选）

| ID | 项目 | 层 | effort | 状态 | 备注 |
|----|------|----|--------|------|------|
| I1 | PutInput 值按字段类型校验（派生定型更新枚举） | base | M | ⏳ | 唯一「列名 typed 值 untyped」破洞 |
| I2 | 连接池自愈参数（min_connections/max_lifetime/test_before_acquire） | db | S | ✅ | DatabaseConfig（MySQL+PG）接出三参 + 链式 with_*，标 #[non_exhaustive]（2026-06-14，commit 2d0e315） |
| I3 | 优雅停机 / drain / 连接池 close | base+db | M | ✅ | yang-db Database/RedisClient::close()/is_closed()；yang-base GlobalDatabase/GlobalRedis::close()；新增 lifecycle.rs（wait_for_shutdown_signal + graceful_shutdown plugin→redis→db）（2026-06-14，commit 2d0e315） |
| I4 | 配置体系 env/文件分层（from_env） | base | M | ✅ | 新增 config.rs EngineConfig::from_env()（YANG_ 前缀，纯 std::env，ConfigError 不 panic）（2026-06-14，commit 2d0e315） |
| I5 | 批量插入桥接受保护层 | base | M | ⏳ | db 侧已有，复用 C1 事务（DB-4 已修复为多批单事务原子） |
| I6 | UPSERT 桥接受保护层 | base | M | ⏳ | db 侧已有 |
| I7 | 游标 / keyset 分页 | base+db | M→L | ⏳ | **db 底层也缺**（修正后），需先在 query_builder 新建 |
| I8 | GROUP BY / HAVING / 聚合桥接受保护层 | base | L | ⏳ | db 侧已有 |
| I9 | `#[non_exhaustive]` + 非测试代码 panic 收口（entity.rs:237 等） | base | S | ✅ | `BaseError` 已标 `#[non_exhaustive]`；`entity.rs:237` 的 `to_v` expect 改 `unwrap_or_else`+`log::error!`（降级为 Null 不崩）；`circuit_breaker.rs` 三处锁 expect 改 `unwrap_or_else(|p| p.into_inner())`（I10 一并）。validator.rs 静态正则 expect 保留（编译时常量，标准实践）。（2026-06-09） |
| I10 | 锁中毒策略统一（circuit_breaker 与 context/validator 相反） | base | S | ✅ | 三处 `expect` 统一改 `unwrap_or_else(|p| p.into_inner())`（随 I9 一并），与 context/validator 一致（2026-06-09） |
| I11 | plugin register check-then-insert TOCTOU | base | M | ⏳ | 并发同名注册静默覆盖 |
| I12 | dispatch 背压/并发上限（Semaphore） | base | M | ⏳ | 无主动背压 |
| M-1 | 测试代码 `unwrap()`/`expect()` 过多 | **db**（仅 yang-db 启用该 lint） | M | ⏳ | **数字修正（2026-06-13 实测）**：原文「73 个」严重过时。`cargo clippy -p yang-db --all-targets --all-features` 实测 `unwrap_used` 144 + `expect_used` 274 ≈ **418 条**。yang-base 未在 `[lints]` 启用 `unwrap_used`/`expect_used`（workspace 根仅 `large_enum_variant`），故 yang-base 测试 unwrap 对 clippy 门禁**零贡献**，scope 应为 `db` 而非 `base+db`。生产代码已无裸 unwrap（I9/I10 收口），M-1 纯属测试清理，**非正确性/panic 风险**，优先级可下调 |

### 11.3 yang-db 自身问题（第十节新发现）

| ID | 项目 | 类型 | 严重度 | 状态 | 取证 |
|----|------|------|--------|------|------|
| DB-1 | 标识符全程裸拼接，public API 注入面（补 quote/校验，JOIN ON 标注可信） | safety | High | ✅ | 新增 mysql/postgres `identifier` 模块（is_valid_identifier/quote_identifier/quote_qualified，导出 pub）；写入路径表名+列名 quote。WHERE 字段（Condition 路径）为已知剩余面（2026-06-14，commit 717a191） |
| DB-2 | Redis WATCH 冲突在 `execute()` 被静默吞成空结果（显式检测 EXEC Nil） | bug | High | ✅ | exec() 先解码 redis::Value，is_watch_conflict 判定重试再 from_redis_value（2026-06-14，commit 5296536） |
| DB-3 | PG 单行 `update()` 不内联 NULL，非整型列设 NULL 运行时报错 | bug | High | ✅ | build_update SET 循环对 SqlValue::Null 内联 NULL 字面量（2026-06-14，commit 5296536） |
| DB-4 | `insert_batch` 多批次非原子（比照 `update_batch` 单事务包裹） | bug | Medium | ✅ | 单批走 pool、多批单事务包裹整体回滚（MySQL+PG）（2026-06-14，commit e15dc3f） |
| DB-5 | 无接受 `&mut Transaction` 的执行变体（C1 的 db 侧前置） | gap | Medium | ✅ | `mysql/transaction.rs:310/378/446`；已加 `#[doc(hidden)] executor()` 逃生舱（MySQL+PG 对称），借出 `&mut MySqlConnection`/`&mut PgConnection`（2026-06-09） |
| DB-6 | `table_exists`/`drop_table`/`init` 裸拼表名（table_exists 改 `?` 绑定，对齐 PG） | safety | Medium | ✅ | MySQL table_exists 改 ? 绑定；MySQL+PG drop_table 用 quote_identifier（2026-06-14，commit e15dc3f） |
| DB-7 | `DbError` 无 code()/category()/is_retryable()，未标 `#[non_exhaustive]` | gap | Medium | ✅ | `DbError` 标 `#[non_exhaustive]` + `DbErrorCategory`；`code()`（8xxxxx 段，含新增 InvalidArgument 800013）；`category()` 单一事实源；`is_retryable()` 委托（2026-06-09 + B1 e3115c4 补 InvalidArgument） |
| DB-8 | `From<RedisError>` 靠 Display 子串分类（改用 `kind()`） | bug | Medium→Low | ✅ | `error.rs:117-128` 改用协议层枚举判定（2026-06-09） |
| DB-9 | PG 事务 insert 硬编码 `RETURNING CAST(id AS BIGINT)`（补 returning setter） | bug | Medium | ✅ | TransactionQueryBuilder 加 returning 字段（默认 id）+ setter，RETURNING 列经 quote（2026-06-14，commit e15dc3f） |
| DB-10 | PG 事务原生助手把浮点绑为字符串（改 `bind(f)`） | bug | Medium | ✅ | bind_json_param_tx/_as_tx 的 Number/Float arm 改 bind(f) 绑原生 f64（2026-06-14，commit e15dc3f） |
| DB-11 | MySQL 集成测试 `if let Ok(db)` 假绿（改 `#[ignore]`/testcontainers） | test-gap | Medium | 🟨 | query_builder.rs 的 create_test_pool/_sync 改 connect_lazy，离线 lib 测试不再挂死（2026-06-14，commit e3115c4）；集成测试 #[ignore] 化仍待 Batch 10 |
| DB-12 | LIKE 通配符 `%`/`_` 不转义（文档说明 + 可选 like_literal） | gap | Low | ⏳ | `mysql/condition.rs:198-202` |
| DB-13 | 非字符串值用 LIKE 取 Debug 表示（类型不匹配返回 Err） | bug | Low | ⏳ | `mysql/query_builder.rs:939-942`、`transaction.rs:279-282` |
| DB-14 | `SerializationError` 被挪用为参数校验（新增 `InvalidArgument`） | gap | Low | ✅ | DbError 新增 InvalidArgument（code 800013, Client 类）；批量列集校验用之（2026-06-14，commit e3115c4） |
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
| DOC-1 | 「游标/keyset 分页」移出「底层都有」小节，标注双层都缺 | ✅ | 本文第五节已拆分「写路径需新建」小节 + 路线图第 6 步措辞 |
| DOC-2 | 注明 MySQL health_check 属 yang-base，db 的 Database 两者皆缺 | ✅ | 本文第五节「观测细项」pool_status 条已补归属澄清 |
| DOC-3 | 修正 `docs/yang-db.md` 关于 quote/校验的归属 | ✅ | yang-db.md 注入防护小节已重写（值参数化 vs 标识符由上层保证） |
| DOC-4 | 修正 Redis `health_check` 文档（恒返回 Ok，不返回 Err） | ✅ | `redis/client.rs:1722-1729` doc 注释已删除不可达 Err 行 |

### 11.5 深度复核新增缺口（第 10.4 节）

| ID | 项目 | 层 | 类型 | 严重度 | 状态 | 取证 |
|----|------|----|------|--------|------|------|
| NG-1 | HTTP 客户端出站请求日志/计时 | base | gap | Medium | ✅ | http send_once 包裹计时，成功 debug/失败 warn 记 method/url/status/elapsed_ms（2026-06-14，commit afdd1d1） |
| NG-2 | PG/MySQL 事务隔离级别可配置（SET TRANSACTION 入口） | db | gap | Medium | ✅ | 新增 isolation 模块 IsolationLevel + MySQL/PG transaction_with_isolation()（2026-06-14，commit e15dc3f） |
| NG-3 | PG `FieldType::Decimal` 精度丢失（降级 f64，应绑 NUMERIC） | db | bug | Medium | ✅ | Decimal 仅 |v|<2^53 走 Float 否则字符串保精度（MySQL+PG，不引依赖）（2026-06-14，commit e15dc3f） |
| NG-4 | `from_json` Object/Array 错误路径补单测 | base | test-gap | Medium | ⏳ | `table_query.rs:2318-2321` |

> 本表为活文档，随补齐推进回填状态/commit/日期。优先级建议遵循第七节顺序（事务→OR→可观测→错误分类→弹性→写路径→多后端→JOIN），yang-db 的 3 个 High（DB-1/2/3）可独立于 yang-base 节奏先行修复。

### 11.6 全量复核总结（2026-06-13，二次审计已更新）

> 首次复核（21 agent）确认已完成项落地。**二次审计（2026-06-13，workflow `wf_98b60c08` + `wf_bd92c883`，共 60 agent 并行 + 对抗式复核）** 进一步逐条核实 ✅/⏳ 论断真实性、校正 file:line 漂移、并发现 **24 条文档此前未覆盖的新问题**（见第十三节）。

**已完成项验证结论**：10 项 ✅ 全部经代码取证确认落地，**无虚标、无退化**。两处子项与设计稿有偏差（非退化，详见下）：
- **C4-metrics**：埋点确在 `DynAction::dispatch` 边界且 `code_str` 静态 label 正确，但实际只发 `action`+`status`(+`code`) 标签，**缺 `module`**（§12.4 设计稿要求 `{module,action,status}`，但 `ActionMeta` 无 module 字段，dispatch 边界拿不到模块名）。见新问题 **NEW-2**。
- **C4-pool_status**：`pool_status` 确经 `GlobalDatabase::pool_status` 转发；但 `GlobalDatabase::health_check`（global.rs:349）仍自执行 `SELECT 1`，**未转发**到 yang-db 的 `Database::health_check`。文档正文（行 109）措辞精确未声称转发，无误。
- **C4-reqid 透传失效**：根 span 的 `request_id` 字段以**具体值**声明而非 `Empty`，使 `RequestIdMiddleware` 的 `record` 透传成为 no-op（见新问题 **NEW-1**）。

**M-1 数字修正**：原文「73 个 unwrap_used」严重过时，yang-db 实测 ≈418 条（144 unwrap + 274 expect），且 yang-base 根本未启用该 lint。详见 11.2 表 M-1 行。

**当前完成度分解**（含二次审计新增）：

| 层级 | 总项数 | ✅ 已完成 | ⏳ 待办 | 完成率 |
|------|--------|----------|---------|--------|
| yang-base Core（C1-C6） | 6 | 4 | 2（C2b/C3，均为 XL） | 67% |
| yang-base Important（I1-I12+M-1） | 13 | 3 | 10 | 23% |
| yang-db 自身（DB-1~DB-21） | 21 | 3 | 18 | 14% |
| 深度复核新增（NG-1~NG-4） | 4 | 0 | 4 | 0% |
| **二次审计新增（NEW-1~NEW-24）** | **24** | **0** | **24** | **0%** |
| Nice（N1-N12） | 12 | 0 | 12 | 0% |
| 文档修正（DOC-1~4） | 4 | 4 | 0 | 100% |

**优先级重排建议**（基于 2026-06-13 二次审计的实际杠杆）：

1. **安全/数据正确性优先**（新发现）—— NEW-4（空布尔组绕过全表写守卫）、NEW-3（filterable/sortable 死配置可绕过）、NEW-6（按用户撤销同秒 off-by-one）、NEW-10/12/13（批量列集/类型提示/u64 截断静默写错）
2. **yang-db P0 三件套**（DB-1/2/3）—— 生产阻断级，可独立于 yang-base 先行修复
3. **可观测性补全**（新发现）—— NEW-1（reqid 透传失效，一行修复）、NEW-2（metrics 缺 module）、NEW-7（X-Request-Id 仅接受 ≤32 hex）、NEW-14（PG Database 缺 pool_status/health_check）
4. **弹性三件套**（I2 连接池自愈 + I3 优雅停机 + I4 from_env 配置）—— K8s 场景刚需
5. **yang-db Medium bug 批量清**（DB-4/6/9/10 + 新发现 NEW-15/16）—— 均为单文件单行级修改
6. **M-1 测试 unwrap 清理** + 安全机制测试补背书（NEW-21/22）—— CI 门禁前提
7. **写路径桥接**（I5/I6 批量插入+UPSERT）—— 复用 C1 事务，杠杆高

### 11.7 file:line 漂移校正表（2026-06-13 二次审计）

> 代码自评估快照后持续演进（`query_builder.rs` 现 5345 行、`mysql/transaction.rs` 615 行、`postgres/transaction.rs` 636 行），文档多处 file:line 已过时。下列为二次审计取证的当前正确行号，**结论均不变，仅行号校正**。复核本文任何条目时以此表为准。

| 文档条目 | 文档旧引用 | 当前正确 file:line（2026-06-13） |
|---|---|---|
| C1 设计草图（§12.1） | `table_query.rs:1615/1669/1881/2142`、`context.rs:358`、`mysql/transaction.rs:192` | `select_in_tx:1611`/`insert_in_tx:1915`/`insert_returning_id_in_tx:1989`/`update_in_tx:2239`/`delete_in_tx:2536`；`context.rs:430`（begin_transaction）；`mysql/transaction.rs:210`（executor） |
| C2a（§12.2 锚点） | `query_params.rs:96-251`、`table_query.rs:1130`、`entity.rs:140` | `query_params.rs:96-276`（含 And:260/Or:272）；`table_query.rs:1339`(render_condition)/1443(render_group)/1007-1054(where_or/and/tree)/934(MAX_WHERE_DEPTH=32)；`entity.rs:322`(Filter)/189(into_where_condition)/343 |
| C4 慢查询（§12.4） | `timed` 包裹「11 处」 | 实为 **12 处** `Self::timed(`：table_query.rs:1235/1586/1628/1805/1894/1929/1973/2003/2218/2253/2515/2553；`timed` 定义 2656 |
| C4-metrics 标签（§12.4 行 411） | `yang_action_requests_total{module,action,status}` | 实现仅 `{action,status}`（typed.rs:105/109-128），**缺 module**（见 NEW-2） |
| I9 panic 收口（行 284/§12.5） | `entity.rs:237` | `entity.rs:281-288`（to_v 已改 unwrap_or_else+log::error） |
| DB-5（表/§12.1） | `mysql/transaction.rs:310/378/446`、`192` | executor 定义 `mysql/transaction.rs:210` / `postgres/transaction.rs:206` |
| DB-6（表/§12.7） | `mysql/database.rs:184-190/177` | `table_exists:215-225`（仍裸拼 `'{}'`）/`drop_table:208-211`（裸拼 `` `{}` ``）/`init:186-199`；PG `drop_table:175-178` **同样仍裸拼**，可一并补入 |
| DB-7（行 431） | `error.rs:1-59` | 实现跨 `error.rs:1-145`（枚举 1-63、DbErrorCategory 65-82、code/category/is_retryable 84-145） |
| DB-9（表/§12.9） | `postgres/transaction.rs:335`（及 200/212） | 硬编码 RETURNING 在 `postgres/transaction.rs:350`，insert 定义 :325；事务版无 `.returning()` setter（是**缺失**而非「忽略」） |
| DB-10（表/§12.9） | `postgres/transaction.rs:570-571/607-609` | `bind_json_param_tx:584-586`、`bind_json_param_as_tx:622-624`（对照主 builder :533 已 `bind(*f)`） |
| DB-13（表/§12.9） | `mysql/query_builder.rs:939-942`、`transaction.rs:279-282/278` | MySQL `query_builder.rs:936-942`(format@941) 与 1000-1004(format@1004)；PG `postgres/transaction.rs:292-297`(format@296) |
| NG-2（表） | `mysql/database.rs:149` | 真正 `pool.begin()` 在 `mysql/database.rs:181`（:149 现为 health_check 的 SELECT 1）；PG `postgres/database.rs:148` 仍准 |
| NG-4（表） | `table_query.rs:2318-2321` | from_json 错误路径在 `table_query.rs:2794-2797`（from_json 整体 2774-2800）；where_eq 入口 1357（旧引 2318-2321 实为 UPDATE 写权限校验，无关） |
| DOC-3（yang-db.md:277 内嵌） | `table/table_query.rs:140/167` | `is_valid_identifier:151`、`quote_identifier:178`（assessment.md:202/277 亦沿用旧 140/167，待同步） |

## 十二、优雅解决方案设计

> 为本文识别的每一个问题至少附一个贴合现有架构的优雅方案。每条给出：核心思路（为何优雅）、API/代码草图要点、涉及文件、兼容性/SemVer、effort。约束统一遵循本仓约定：中文注释、checked API 优先（禁新增生产 panic）、保持 feature gate、鉴权热路径零分配、向后兼容优先。

**I5｜批量插入桥接受保护层（自建多行 INSERT，复用 C1 事务保原子）** · effort M
不转调 yang-db `insert_batch`（裸拼标识符 + 零权限校验，会绕过受保护层）。在 yang-base 新增 `build_insert_batch_sql`：逐行复用 `prepare_and_validate_insert`（权限/默认值/时间戳/必填），列名走 `quote_identifier`，拼成单条 `(?,?),(?,?)...` 多值 INSERT 一次绑定。原子性复用 C1：`insert_batch` 内部 `begin_transaction → insert_batch_in_tx → commit`，多批次同事务，天然规避 DB-4。文件 `table_query.rs`（新增 `build_insert_batch_sql` + `insert_batch`/`insert_batch_in_tx`）。`#[cfg(feature="mysql")]`，minor，依赖 C1。

**I6｜UPSERT 桥接受保护层（自建 ON DUPLICATE KEY UPDATE）** · effort M
同 I5 理由不转调 yang-db。新增 `build_upsert_sql`：复用 `prepare_and_validate_insert` 校验，列走 `quote_identifier`，生成 `INSERT ... ON DUPLICATE KEY UPDATE col=VALUES(col)`，提供 `upsert`/`upsert_in_tx`。返回 rows_affected（1=插入/2=更新）。MySQL 专用语法，未来 C3 多后端化时按后端分派（PG `ON CONFLICT`）。minor，依赖 C1。

**DB-4｜insert_batch 多批次单事务包裹（对齐 update_batch）** · effort S
`insert_batch_with_size` 当前每 chunk 独立 `execute(pool)`，第 N 批失败前 N-1 批已落库；而 `update_batch` 已用单 `pool.begin()` 包裹所有 chunk（自相矛盾）。把 `insert_chunk` 执行目标从 `&MySqlPool` 抽成 `sqlx::Executor` 泛型：单批次走 pool 直执行（免事务开销），多批次开一个 tx 包裹后 commit，比照 `update_batch`。PG 侧对称。文件 `mysql/query_builder.rs:2138-2236`、`postgres/query_builder.rs:1411`。签名不变，行为从「部分提交」变「全成功或全回滚」——严格改进。

### 12.2 查询表达力（C2a / C2b / I7 / I8）

**C2a｜WhereCondition 引入递归 And/Or 组节点 + 类型化层 `Filter<W>` 布尔树** · effort M
关键认知：读路径 `build_select_sql` 直接用 sqlx 生成 SQL，**不经** yang-db QueryBuilder，所以 `where_or` 对受保护层无用——必须在 yang-base 自身渲染层做布尔树。(1) 给平铺 `WhereCondition` 追加递归 `And(Vec)/Or(Vec)`，把 `append_where_to_sql` 的扁平循环重构为递归 `render_condition`，组节点括号包裹，12 个叶子分支零改动，顶层 Vec 仍隐式 AND（向后兼容）；(2) 类型化层新增泛型 `Filter<W>` 布尔树包装 `T::WhereCond`，`SelectQuery` 加可选 `filter` 字段。组节点权限校验递归下钻；递归深度设上限（如 32）防深嵌爆栈，checked 返回 `ParamInvalid` 不 panic。**务必同时给 `WhereCondition` 打 `#[non_exhaustive]`（合并 I9）**否则加变体是 minor-breaking。文件 `query_params.rs:96-251`、`table_query.rs:1130`、`entity.rs:140`、`sql_bridge.rs`、`builtin/select.rs`。

**C2b｜RelationConfig 接入：批量二次查询式预加载（避免 N+1，逐表过权限），而非裸 SQL JOIN** · effort XL
`RelationConfig`/`RelationType` 定义齐全但 `table_query.rs` 零引用（死配置）。**不做裸 JOIN**：裸 JOIN 会让被关联表的列绕过 `ensure_fields_readable`/`validate_filter_field`，且 yang-db `join(table,on)` 的 ON 是裸拼注入面（DB-1），引进受保护层等于开后门。改走关联预加载：主查询后收集外键值，对每个关联表用其自身 `TableConfig` 走带权限校验的 `where_in` 二次查询再 stitch——天然 `1+N_relations` 条（非 N+1），每条参数化零注入；1:1/1:N 用子表 FK in 父键集，M:N 多一跳中间表。前置：需 `ModuleRouter` 补 `table_config_of(name) -> Option<Arc<TableConfig>>` 反查 registry。设 `max_preload_depth` 防深链。`#[cfg(feature="mysql")]`，纯增量。

**I7｜读路径新建 keyset/seek 分页：行值元组比较 + 不透明游标，PK 兜底全序** · effort M（base）/ L（双层对称）
双层都只有 LIMIT/OFFSET，keyset 是真·新建。用 `(k1,k2,..) > (?,?,..)` 行值比较把「翻下一页」变等值 seek，深翻页 O(offset) 退化消失、并发写不漂移。游标做成不透明 `base64(JSON)`（编码末行排序键 + 方向），解码失败走 `BaseError::ParamInvalid`（不 panic）。强约束：自动把 `PK_FIELD` 追加为末位 tiebreaker 保全序。与 offset 分页并存（新增 `seek` 方法，`page()` 不动，二者同设返回 `ParamInvalid`）。列名走 `quote_identifier`、值走 `SqlParam` 绑定。软删过滤子句需与 seek 的 WHERE 正确 AND 合并；排序列建议 NOT NULL。文件 `query_params.rs:333`、`table_query.rs:1390/826`。

**I8｜TableQuery 新增白名单聚合 + GROUP BY/HAVING，输出 DynamicRow，配套 AggregateAction** · effort L
受保护层仅 `count()`；读路径不经 yang-db 故需自建。聚合规格全走封闭枚举/白名单（`AggFn::{Count,Sum,Avg,Min,Max}`），**绝不接受裸表达式字符串**：作用列走 `quote_identifier`，别名走 `is_valid_identifier`，GROUP BY 列必须 filterable + 在 TableConfig，HAVING 作用在聚合别名上、值参数化绑定。输出用既有 `DynamicRow` 承载异构结果，避免为每种聚合造类型。零新增注入面、零新增 panic（全 checked）。聚合结果列不过 `ensure_fields_readable`（非实体字段），文档需说明输出是 DynamicRow 非 T。`#[cfg(feature="mysql")]`，纯增量。文件 `table_query.rs:1061/1390`、新增 `table/aggregate.rs`、`action/builtin/aggregate.rs`。

### 12.3 类型化层多后端（C3 / DB-18）

**C3｜封闭 `Backend` trait + `TableQuery<B=MySql>` 泛型化，TableEntity 脱钩 MySqlRow（主方案）** · effort XL
引入 sealed `Backend` trait 作方言/驱动单一抽象点，把 sqlx 的 Pool/Row/Arguments 与方言行为（占位符、标识符引用、自增取回）收口到关联类型与少量方法；`TableQuery` 升级 `TableQueryGeneric<B: Backend>`，`type TableQuery = TableQueryGeneric<MySqlBackend>` 保旧名。`TableEntity` 去掉 `FromRow<MySqlRow>` 超 trait，行解码约束下沉到 `select::<T>()` 调用点（`T: FromRow<B::Row>`），实体回归纯数据契约。B 是零大小类型参数，单态化后零分配零虚调用，鉴权热路径不受影响。`postgres` 全程 `#[cfg]`，默认不开。**风险**：泛型渗透到 builtin 六件套——建议 builtin 暂固定 `MySqlBackend`，PG 走显式 `table_typed_with::<T, PgBackend>()`，避免一次性炸全链。SemVer minor-risky。

**C3（拆分第一步）｜最小脱钩：仅 TableEntity 去 FromRow + 约束下沉到调用点** · effort M
把 XL 拆成可独立发版的第一刀：只移除 `TableEntity` 的 `FromRow<MySqlRow>` 超 trait（约束放到真正需要的 `select::<T>()` 边界），不立刻泛型化 TableQuery。移除后同一实体 struct 可被 MySQL/PG 路径复用，为后续泛型化扫清类型障碍，且本步零运行时变更。现有 `#[derive(sqlx::FromRow, TableEntity)]` 全部继续编译。文件 `entity.rs:16-26`。patch~minor。

**DB-18｜跨后端一致性断言测试：基于 to_sql 的方言归一化等价校验（纯离线，无活库）** · effort S
两后端都有纯函数 `to_sql()`/`SqlGenerator::build_*`（不触网）。建一组「同一逻辑查询 → 两后端各自 SQL → 归一化后断言等价」测试：归一化器吃掉合法方言差（`?`↔`$N`、反引号↔双引号、`ON DUPLICATE`↔`ON CONFLICT`、CAST），剩余差异即漂移红线。复用 `sql_generator_prealloc_test.rs` 离线范式，`connect_lazy` 造惰性池不连库，可进默认 `cargo test`。纯新增测试，零 API/SemVer 风险。新增 `crates/yang-db/tests/cross_backend_consistency.rs`。

**DB-18（治本备选）｜提炼共享方言 trait（SqlDialect）抽掉两 SqlGenerator 公共骨架** · effort L
漂移根因是两份 SqlGenerator 各写 build_*。提炼 `pub(crate) trait SqlDialect`（仅承载占位符/标识符引用/UPSERT 子句/CAST 等真正不同的原子），公共骨架上移到泛型 `SqlGenerator<D>`，两后端只实现各自 Dialect，漂移在编译期消除。**红线**：CLAUDE.md 禁顺手拆 query_builder.rs——必须以上一条离线一致性测试做回归网后再动，仅抽方言原子不动文件物理切分，分后端小步迁移每步 `cargo test --lib` 绿。对外 API 无变化（pub(crate)），patch。风险高，可标 backlog。

### 12.4 可观测性热路径（C4 / 慢查询 / 审计 / pool_status）

**C4-tracing｜引入 tracing + dispatch→authorize→handler 三段 span 化（log 经 tracing-log 桥接）** · effort M
把 tracing 作 yang-base 核心依赖，启用其 `log` 兼容特性 + `tracing-log`，使现有 `log::*` 宏自动流入 tracing 订阅者，无需逐处改写、对未接订阅者的旧调用方完全无感。仅在三个稳定接缝开 span：dispatch 根 span（module/action）、authorize child span（is_public/granted）、DynAction::dispatch handler span。span 用静态名 + 借用字段 + `.instrument`，成功路径无 `format!`/无 collect，满足零分配。tracing 设无条件核心依赖（横切能力），不引入新 cfg。minor。文件 `module_router.rs:327-405`、`typed.rs:82-92`。（备选 `C4-tracing-alt`：用 `#[tracing::instrument]` 宏属性，改动更少但 blanket impl 上无法按 Action 定制字段、须 `skip(context)` 防泄漏。）

**C4-reqid｜request_id 透传：ActionContext 增字段 + RequestIdMiddleware 生成并注入 span** · effort M
request_id 放 `ActionContext`（一次派发的运行期标识，而非传输输入），不污染 `Request`。内置 `RequestIdMiddleware` 作洋葱链最外层：缺失时生成、存在（上游 `X-Request-Id`）则透传，再 `span.record` 进根 span 串联日志/metrics/审计。类型用轻量 `RequestId(u128)`（时间高位|计数器低位），避免把 uuid 拉进非 token 构建。**破坏点**：`ActionContext` pub 字段新增会破坏字面量构造——在 `new`/`new_with_global_tools` 默认填充，并建议标 `#[non_exhaustive]`。生成是单次整数运算无堆分配，满足零分配。minor-breaking + non_exhaustive。

**C4-metrics｜Action 计时/错误/吞吐 metrics：feature-gated `metrics` 门面 + DynAction::dispatch 边界埋点** · effort M
引入 `metrics` crate（门面，运行期由调用方挂 exporter），新增 opt-in feature `metrics` 默认关闭（零依赖增量、完全向后兼容）。埋点统一放 `DynAction::dispatch`（所有 Action 唯一必经边界）：计数器 `yang_action_requests_total{module,action,status}`、直方图 `yang_action_duration_seconds`、错误计数 `yang_action_errors_total{...,code}`。label 全用 `&'static str`（module/action 来自 meta，code 用新增 `code_str()` 返回静态映射，**切勿 `to_string()` 当 label**——高基数+分配）。feature 关闭时 `#[cfg]` 整段消失。minor。文件 `typed.rs:82`、`error/mod.rs:444`。

**pool_status｜MySQL pool_status + health_check 下沉 yang-db 并经 GlobalDatabase 暴露** · effort S
消除「同 crate Redis 两者俱全、MySQL 皆缺」的不对称。yang-db `mysql::Database` 补 `pool_status()`（包 `MySqlPool::size()/num_idle()`）与 `health_check()`（下沉 `SELECT 1`），复用/提升 `PoolStatus` 到 crate 顶层（重导出保 `redis::PoolStatus` 旧路径）；`GlobalDatabase` 补 `pool_status()` 转发，与 `GlobalRedis::pool_status` 对称。sqlx 原生支持只差接出，零新依赖。mysql gate 内。纯新增，minor。文件 `mysql/database.rs:118`、`lib.rs`、`global.rs:349`。

**慢查询｜受保护层 TableQuery 执行边界计时 + 可配阈值，超阈值 warn** · effort M
给 TableQuery 增 `slow_threshold: Option<Duration>`（由 `ActionContext.table_query()` 从全局观测配置注入），抽一个内部 `timed(op, fut)` 辅助封装 6 处执行点，超阈值 `tracing::warn!{table, op, elapsed_ms, request_id}`。SQL 文本默认不记（防泄漏/高基数）。`None`（默认）时计时分支整体短路，热路径仅一次 `Instant::now()` 无分配。**不下沉 query_builder.rs**（遵守约定）。保持 `new` 签名不变，加 `with_slow_threshold()` 链式 setter。minor。文件 `table_query.rs:129/1081/1374/1641/1687/1907/2174`、`context.rs:358`。

**审计钩子｜AuthAuditHook trait 注入 Login/Refresh/Logout** · effort S
三条安全路径（`auth.rs` Login/Refresh/Logout）全静默。定义 object-safe `AuthAuditHook`（成功/失败两类事件），经 `LoginAction<V, A=TracingAuditHook>` 等构造参数注入（与既有 `CredentialVerifier` 注入同构），默认 `TracingAuditHook`（发 tracing event）。事件含 request_id/subject/结果/错误码，**绝不记凭据明文/token 原文**（只记指纹）。保留 `new(verifier)` 用默认 hook，加 `with_audit(verifier, hook)`，旧调用不破坏。token feature gate 内。minor。

**NG-config｜统一 ObservabilityConfig 收口可观测开关** · effort S
上述 tracing/metrics/慢查询/审计若各自配置会散落多处。补轻量 `ObservabilityConfig { slow_query_threshold, ... }`，随 `OnceLock` 单例（重复 init 返 Err 不 panic），由 `ActionContext` 读取下发。把 C4 一揽子运行期旋钮收敛到单一可测试入口，与 `DatabaseBundle::init` 统一入口理念一致。全新增，minor。新增 `crates/yang-base/src/observability.rs`。

### 12.5 错误分类与契约（C5 / DB-7 / DB-8 / DB-14 / I9 / 码双射）

**C5｜BaseError 引擎级分类 API（category/is_retryable/is_client_error/is_server_error）** · effort M
在 `impl BaseError` 新增零分配纯 `match self` 分类方法。引入 `pub enum ErrorCategory{Client,Auth,NotFound,Conflict,Transient,Server}`（引擎自有语义，**不掺 HTTP status**——HTTP 映射属调用方传输层边界），`category()` 为单一事实源，三个 `is_*` 全委托它派生避免漂移。DbError 携带变体透传 `map_db_category(e)`（衔接 DB-7）。`ErrorCategory` 自带 `#[non_exhaustive]`。`is_client`/`is_server` 非互斥补集（Transient 属 server 但可重试），文档需注明按 category 派生。纯新增，无 SemVer 破坏。文件 `error/mod.rs:415-525`。

**DB-7｜DbError 补 code()/category()/is_retryable() 并标 #[non_exhaustive]** · effort S
yang-db `DbError`（18 变体）连稳定码都没有。补稳定 `code()`（8xxxxx 段独立命名空间）、`category() -> DbErrorCategory{Client,Conflict,NotFound,Transient,Server}`、`is_retryable()`。分类信息已在 `From<sqlx::Error>` 的 SQLSTATE 精确分桶里，`category()` 只是显式化：Connection/RedisConnection/RedisTimeout/RedisPool→Transient，Constraint→Conflict，TableNotFound→NotFound，SqlSyntax/MissingWhere/MissingGroupBy/UnsupportedOperator→Client。BaseError 携带变体透传 `e.category()`，两层语义一致。`#[non_exhaustive]` 对跨 crate match 是破坏（需 `_` 臂）——同 crate 内 yang-base 的穷举 match 不受影响。文件 `error.rs:1-59`。

**DB-8｜From<RedisError> 由 Display 子串改 kind() 精确分类** · effort S
现状 `format!` 成串后 `contains("Connection")`/`contains("type")` 模糊匹配——顺序相关、`"type"` 过宽误判、redis 升级改文案即静默失配。改用稳定的 `RedisError::kind()` 与 `is_timeout()`/`is_connection_dropped()`/`is_io_error()`：timeout 优先 → connection/io → TypeError → 兜底 Command。控制流从「脆弱人类可读串」迁到「协议层枚举」。行为兼容（同样五选一变体），仅分类更准，无 API 变化。需补单测构造各 kind 验证落桶（衔接 DB-21）。文件 `error.rs:114-130`。

**DB-14｜新增 DbError::InvalidArgument，解耦被挪用的 SerializationError** · effort S
`batch_size==0`/空数据切片/非 JSON 对象都复用 `SerializationError`，污染「序列化失败」语义。新增 `InvalidArgument(String)` 专表参数校验失败，归 DB-7 的 Client 类（不可重试、调用方过错），是 checked-API 哲学的体现。yang-base 的 `From<DbError>` 穷举 match 须新增 `InvalidArgument(_)` 臂（建议归 `DatabaseExecuteFailed` 同桶保上层码不变）。配合 DB-7 的 `#[non_exhaustive]` 后破坏性已前置吸收。文件 `error.rs`、`mysql/query_builder.rs:2116/2133`、`postgres/query_builder.rs:1426/1441`。

**I9｜BaseError 标 #[non_exhaustive] + 收口三处可达 panic** · effort S（2A）/ M（2B）
两件打包。(1) 给 `BaseError` 加 `#[non_exhaustive]` 杜绝下游 match 无 `_` 臂时「加变体即破坏」。(2) panic 收口，重点 `entity.rs:237` 的 `to_v` 在受保护查询热路径 `expect`，任意 Serialize 缺陷会 panic 整个 dispatch。**方案 2A（推荐先行，S）**：`to_v` 改 `unwrap_or_else` 落 Null 同时 `log::error!`，把「不可能发生」降级为「发生也不崩」，零签名变化。**方案 2B（彻底，M）**：`IntoSqlCondition::into_sql_condition` 返回 `Result` 沿 `select.rs:143` 一路 `?` 上抛——trait 破坏性但派生宏使用者透明。锁中毒统一 `into_inner()` 恢复（`circuit_breaker.rs:82/106/124`，顺带解 I10）。`#[non_exhaustive]` 对下游 match 是 minor 破坏，同 crate 不受影响。

**码双射｜错误码↔变体双射去重：单一事实源 + 唯一性回归网** · effort S
现状 `DatabaseConnectionFailed`/`...DbError` 同码 200001、Redis 同码 210004（String 兜底版 vs DbError 携带版，故意同源同码），及 Migration 双变体真冗余。本质不是「同码非法」而是「无机制保证一致性」。(1) 把「允许同码的成对变体」建白名单表，用穷举全变体的回归测试断言「码要么唯一、要么在白名单内」，让双射意图变可执行契约；(2) 真冗余的 Migration 双变体 `#[deprecated]` 其一收敛；(3) 文档化每段码语义。现有码值全不变（测试锁定），`#[deprecated]` 仅告警不破坏。文件 `error/mod.rs:444-524` + 测试区。

### 12.6 端到端类型保真（I1 / 写库接缝 / put值untyped / 响应擦除）

**I1｜派生 `<Name>Set` 定型更新枚举，PutInput.data 收口为 `Vec<EntitySet>`** · effort M
`put.rs:27` 的 `data: Vec<(T::Field, serde_json::Value)>`——列名 typed（封闭枚举安全），值裸 `Value` 编译期不校验，是 CRUD 六件套唯一「列名 typed 值 untyped」破洞。完全复刻已验证的 `<Name>Where` 模式：派生 `<Name>Set` 枚举，每字段一变体内层为该字段 Rust 类型（`Option<T>` 字段保留 Option 以支持置 NULL），serde `#[serde(tag="field",content="value")]`，反序列化时 `value` 直接按字段类型解析——`age` 收到字符串即在 serde 层报错，**校验提前到反序列化边界**。`TableEntity` 加关联类型 `type SetField: IntoUpdatePair`。**破坏**：JSON 顶层从 `[["age",30]]` 变 `[{"field":"age","value":30}]`（面向用户协议变更）+ trait 加关联类型；建议保留旧路径 `#[deprecated]` 给过渡期。文件 `entity.rs`、`yang-base-derive/table_entity.rs:116-209`、`put.rs:23-74`。

**写库接缝｜typed→yang-db 接缝用 IntoColumnMap 构建 HashMap，消除「序列化摊平」值类型擦除** · effort M
`add.rs:60` 的 `serde_json::to_value(&input)` 把整个实体摊平成 `Value::Object` 再遍历成 `HashMap`，列名靠 `&'static str` 保住但值类型在 `to_value` 这步全擦除，`_ =>` 还留运行期不可达分支。承认 `HashMap<String,Value>` 是 yang-db 边界契约（多后端/动态列的现实），但把「实体→HashMap」从「serde 盲摊平」升级为「经类型化 `IntoColumnMap`」：派生宏按字段静态展开 `(列名, to_value(self.field))`，列名来自 `&'static str`、值来自具体字段类型，擦除点收敛到单一可审计调用，消除不可达分支。逐字段 `to_value` 失败须 `try` 收集返回 `Result`（禁 unwrap）。不动 `table_query.rs:1881` 签名、不碰 query_builder.rs。基本向后兼容。

**put值untyped（I1 反序列化视角强化）｜SetField 值校验补 serde 边界 + 运行期类型断言双保险** · effort S
与 I1 同根，聚焦「校验在哪层」。`<Name>Set` 把校验提前到 serde 是主防线，但两残留缝隙显式收口：(1) `FieldType::Json` 字段内层仍是 Value（业务语义本就任意 JSON，可接受）；(2) 数值精度/范围 serde 只拦一部分。让 `into_update_pair` 后仍流经既有 `validate_update_data_impl`（`table_query.rs:1950`，免费）做 FieldType 复检，形成「serde 静态收口 + 运行期兜底」双层。`extract_input` 错误细化为 `ValidationFailed(field, reason)`（内容变化非签名变化，兼容）。需确保 `deny_unknown_fields` 对 SetField 同样开启。

**响应擦除｜ApiResponse 增 success_typed 类型保真 + details 结构化错误明细** · effort M
两子问题分治。(A) `data: Option<Value>` 类型擦除：根因是 `DynAction::dispatch` 返回 `Result<ApiResponse, BaseError>`，整条洋葱链 + `Arc<dyn DynAction>` 注册表都按此对象安全签名建立——泛型化 `ApiResponse<T>` 会击穿运行期擦除派发管线，**判定不可行**。优雅解不是消除擦除，而是确保擦除前 `Output: Serialize + JsonSchema`（`typed.rs:23` 已有 bound），类型契约在 `output_schema()` 静态保真，补一个「data 必满足 output_schema」不变量测试背书。(B) `from_error` 明细丢弃：`fail(code, to_string())` 把 `ValidationFailed(field,reason)` 等结构化变体拍扁成串。给 `ApiResponse` 增 `details: Option<Value>`（`skip_serializing_if`，零成本兼容），`from_error` 经新增 `BaseError::detail_payload()` 抽取 `{field, reason}`。`typed.rs`/`middleware.rs`/`module_router.rs` 签名均不动。高度向后兼容（构造器封装，仅手写字面量者需补 `details: None`）。

### 12.7 注入面与标识符安全（DB-1 / DB-6 / DB-12 / DB-20）

**DB-1｜新增 yang-db 标识符校验/转义模块 + SQL 生成层统一加引号（JOIN ON 标可信），保留表达式逃生舱** · effort L
把 yang-base 已验证的 `is_valid_identifier`/`quote_identifier` 下沉为 yang-db 自有能力，在唯一的 SQL 生成收口（`SqlGenerator::build_*`）施加，而非散落每个链式 setter——不动 query_builder 巨型结构，不改 `field()`/`order()`/`join()` 签名（仍返回 Self）。校验在 build 期（`build_select`/`build_update`/`build_delete` 已返回 Result）；无 Result 的 `build_order_by`/`build_group_by`/`build_joins` 改返回 Result（pub(crate) 无 SemVer 影响）。`is_valid_identifier` 纯 char 迭代零分配。**张力处理**：`field("COUNT(*)")`/`field("u.name AS n")` 等表达式用法——quote 走「限定标识符感知」（`a.b → \`a\`.\`b\``），对含括号/空格/AS 的表达式提供显式逃生舱 `field_expr()`/`order_raw()`（标 `# Safety`）；JOIN ON 本质自由 SQL 表达式，按评估结论标「可信输入」，仅对 JOIN 的 table 名 quote。**行为破坏点**：依赖 `field()` 传表达式者会在 build 期得 `InvalidIdentifier` err，需迁到逃生舱；新增 DbError 变体建议与 DB-7 的 `#[non_exhaustive]` 一并落地。PG 侧同构（双引号）。

**DB-1（最小侵入备选）｜仅收口表名/JOIN 表名 quote + 校验助手设为 pub 供上层复用** · effort S
若不愿承担表达式用法的行为破坏，退一步：只对「几乎不可能是表达式」的位置（FROM 表名、JOIN 目标表名、drop_table 表名）强制 quote+校验，列/排序/分组字段保持原样，但把 `is_valid_identifier`/`quote_identifier`/`quote_qualified` 提升为 yang-db 的 **pub** 工具函数，让直接消费 yang-db、绕过 yang-base 类型层者能显式校验外部输入；setter doc 明确「列名/ON 为可信输入，外部输入请先 quote」。零行为破坏、零 SemVer 风险，同时解决 DOC-3 文档不实。适合作为先行小步，再迭代到主方案。

**DB-6｜table_exists 改 ? 绑定对齐 PG；drop_table 用 quote_identifier；init 文档化为可信 DDL** · effort S
按「值 vs 标识符 vs 脚本」三类各用最合适手段：(1) `table_exists` 是 DML 查询，`information_schema.tables.table_name` 可作绑定参数——照搬 PG 写法用 `?` 绑定，消除字面量注入且跨后端一致；(2) `drop_table` 是 DDL，MySQL 不支持 DDL 占位符，走 `quote_identifier` 校验+转义（复用 DB-1 模块），非法名 Err；(3) `init` 按 `;` 切分执行，本质是开发者提供的迁移脚本运行器，维持现状但补 doc 明确「入参为可信 DDL，朴素切分不处理字符串内分号」。文件 `mysql/database.rs:176/182/153`。合法表名行为不变。

**DB-12｜新增 like 通配符转义助手 + like_literal 子串匹配 API，文档化默认 LIKE 为原始 pattern** · effort M
区分两种意图：现有 `where_and(field,'like',pat)` 传入的是「完整 LIKE pattern」（调用方自带 `%/_`，是特性），保持不变兼容。新增「把用户输入当字面子串匹配」场景：`escape_like_pattern()` 转义 `% _ \`，新增 `where_like_contains`/`starts_with`/`ends_with` 生成 `col LIKE ? ESCAPE '\'` 并绑转义后的 `%term%`。把「意外宽匹配」陷阱显式化，ESCAPE 子句让转义在 SQL 层确定生效、跨 collation 稳定。新增 `Condition::LikeEscaped` 变体建议同标 `#[non_exhaustive]`。PG 侧同构。文件 `mysql/condition.rs:38/198`、`postgres/condition.rs:172`。

**DB-20｜MySQL 测试连接串改读 MYSQL_TEST_URL/MYSQL_TEST_PASSWORD 环境变量，对齐 PG** · effort S
照搬 PG 的 `test_db_url()` 模式：把 6 个文件硬编码的 `const TEST_DB_URL` 换成读环境变量带本地默认值的辅助函数，密码优先从 `MYSQL_TEST_PASSWORD` 注入，整串可被 `MYSQL_TEST_URL` 覆盖。消除 git 跟踪的明文凭据，符合 CLAUDE.md 约定，默认值保留维持本地一键可跑。可抽到 `tests/common/` 共享模块。仅测试代码，零生产影响。与 DB-11 正交可同批。

### 12.8 测试可信度（DB-11 / DB-19 / DB-21 / NG-4）

**DB-11｜MySQL 集成测试改 #[ignore] + 环境变量统一连接助手，纯 SQL 断言移出 if let Ok** · effort M
现状 `if let Ok(db) = result {...}` 包裹全部断言：无 MySQL 时整块被跳过测试仍判过——「假绿」。对齐同 crate 已有的 PG 范式（`#[ignore]` + `PG_TEST_URL`）：给所有触库测试加 `#[ignore]`，连接失败直接 `.expect()` 让 `--ignored` 模式真红；同时把不触库的纯 SQL 生成断言从 `if let Ok` 拆出作为离线测试保留。三套互斥风格收敛为两套有意义的（离线纯逻辑 / `--ignored` 触库）。顺带消化 DB-20。仅测试代码，无 API 影响。

**DB-19｜Redis 集成测试加 #[ignore] + REDIS_TEST_URL 助手，离线套件不再变红** · effort S
`test_redis_*.rs` 直接 connect 后硬 `assert!`/`expect`、且无 `#[ignore]`：无 Redis 时 `cargo test` 整批变红（与 MySQL 假绿反向，但同样偏离「离线可跑」）。对齐 PG/DB-11 范式：所有触 Redis 测试加 `#[ignore]`，提供 `redis_test_url()` 读 `REDIS_TEST_URL`。保留硬 expect（`--ignored` 下连不上即真红）。仅测试代码，无 SemVer 影响。

**DB-21｜为 error.rs 的 From 映射与 MissingGroupByClause Display 补单测** · effort S
`From<sqlx::Error>`（SQLSTATE→变体）、`From<redis::RedisError>`（Display 子串分类）几乎无单测，`MissingGroupByClause` Display 从未断言。补三组：(1) 遗漏变体 Display 断言；(2) 用 `redis::RedisError::from((kind, msg))` 构造真实错误断言落桶；(3) sqlx 侧测可直接构造的分支（RowNotFound/PoolClosed/ColumnNotFound 等），SQLSTATE 分支留待 DB-18 集成测试覆盖。是 DB-8（改 kind() 分类）的安全网，应先于 DB-8 落地。纯新增测试。文件 `error.rs:51/62-112/114-129/153-332`。

**NG-4｜为 from_json 的 Object/Array 错误路径补单测（经 where_eq + build_select_sql_for_test）** · effort S
`SqlParam::from_json` 是私有 fn 但有现成公共触发链：`where_eq(field, Value::Object) → build_select_sql_for_test()`（已存在的 `#[cfg(test)]` 助手）会调 `append_where_to_sql → from_json`，对 Object/Array 返回 `DatabaseQueryFailed`。在 `__tests__/table_query_test.rs` 加测试，复用 `create_test_table_config`，断言返回 Err 且匹配该变体，覆盖 `{"field":"id","value":{"x":1}}` 误发场景。纯测试补充，零生产改动、零风险。文件 `table_query.rs:1160/2199/2318-2321`。

### 12.9 数据库正确性 bug（DB-2 / DB-3 / DB-9 / DB-10 / DB-13）

**DB-2｜exec() 先解码为 redis::Value 并显式检测 EXEC Nil → 触发 WATCH 重试** · effort M
根因：atomic pipe 在 WATCH 冲突时 EXEC 回 Nil，而 `from_redis_value(Nil)` 对 `Vec<T>`/`()` 都解码成 `Ok(空)`，命中 Ok 分支直接返回，乐观锁语义被破坏；现状靠 `err_msg.contains("nil")` 检测是脆弱的（冲突压根不产生 Err）。把「冲突检测」从「解析错误字符串」下沉到「协议层值检测」：先统一解码为 `redis::Value`，`watched_keys` 非空且整体为 `Value::Nil` 判定为 WATCH 冲突并重试，确认非冲突后再 `from_redis_value::<T>`。无监视键时仍允许 Nil 透传（不影响普通 pipeline）。修 bug，签名不变。文件 `redis/transaction.rs:318-344`。

**DB-3｜PG build_update SET 循环对 SqlValue::Null 内联 NULL 字面量** · effort S
根因：`build_update` SET 循环无条件 `push_placeholder`，NULL 被压成参数；PG 未类型化绑定 NULL 默认按 INT4，对 text/timestamp 列报类型不匹配。同文件 insert/insert_batch/upsert/update_batch 都已显式内联 NULL，唯独单行 update 漏改（内部不一致实锤）。对齐 `build_insert` 的 match 模式：`SqlValue::Null => push_str("NULL")`，其余走占位符。占位符编号自动收缩、WHERE 子句自洽。纯 bug 修复。文件 `postgres/query_builder.rs:482-491`。

**DB-9｜TransactionQueryBuilder 补 returning 字段 + .returning() setter，去掉硬编码 id** · effort S
根因：事务内 insert 硬编码 `RETURNING CAST(id AS BIGINT)`，忽略主键列名，非 id 主键表事务内 insert 必失败；而非事务路径已有 `.returning()`（对等性缺口）。对齐非事务 builder：同名 `returning` 字段（默认 `"id"`）+ 同名 setter + 同样 `CAST(col AS BIGINT)`，两路径 API 对称。纯新增 + 默认值保持，向后兼容 minor。文件 `postgres/transaction.rs:200/212/335`。

**DB-10｜PG 事务原生助手浮点直接 bind(f64) 而非 to_string()** · effort S
根因：`bind_json_param_tx`/`bind_json_param_as_tx` 照搬 MySQL 的「float→string 容错」，但 PG 类型严格，numeric/float8 列 `WHERE price = $1` 绑文本报 `operator does not exist: numeric = text`；主 builder 的 `bind_value_match` 宏对 `SqlValue::Float` 已是 `bind(*f)`（漂移）。两处各改一行 `query.bind(f)` 与主 builder 对齐，一并改误导注释。bug 修复，签名不变。文件 `postgres/transaction.rs:570-571/607-609`。

**DB-13｜LIKE 遇非字符串值返回类型错误，杜绝 Debug 表示当 pattern** · effort S
根因：MySQL `where_and`/`where_or` 与 PG 事务 `where_and` 在 `op=like` 且值非 String 时 `format!("{:?}", sql_value)`，把 `Int(5)` 当 pattern——语义错误且静默。LIKE 本就要求字符串 pattern，类型不符应显式报错：MySQL 侧返回 `Err(TypeConversionError)`，PG 事务侧用既有「延迟错误」`self.error` 机制返回，两端都不新增 panic（checked API 优先）。旧行为是 bug（产出永不匹配的 pattern），无合理调用方依赖。patch。文件 `mysql/query_builder.rs:937/1003`、`postgres/transaction.rs:278`。

### 12.10 Redis/事务收尾（DB-15 / DB-16 / DB-17）

**DB-15｜Transaction 文档化「drop 即 sqlx 尽力回滚」+ 可选 Drop 日志告警（不改语义）** · effort S
`Transaction` 仅持 `Option<SqlxTransaction>`，未显式 commit/rollback 时依赖 sqlx 自身 Drop 做尽力回滚——语义正确但未文档化，调用方易误以为有强保证。Rust 的 Drop 不能 async，正确做法不是自写回滚，而是：(1) 类型级 doc 显式声明契约；(2) 加轻量 `Drop` impl，仅当 `tx` 仍 `Some` 且 `enable_logging` 时打一条 warn。把隐式行为变可观测+文档化，零行为破坏、零 panic、热路径不受影响（Drop 中只读 `tx.is_some()` 不消费）。文件 `mysql/transaction.rs:7-20` + PG 对称。

**DB-16｜新增 get_bytes 逃生舱方法，二进制值不再退化为 None** · effort S
`as_string()` 仅匹配 `String` 变体，对 `Bytes`（非 UTF-8）返回 None，导致 `get()` 把二进制值与「键不存在」(Nil) 都得到 `Ok(None)` 不可区分。底层 `From<redis::Value>` 已正确保留 Bytes，仅缺 sugar：加 `RedisValue::into_bytes(self) -> Option<Vec<u8>>`（Nil→None，String/Bytes→Some 字节，零 clone）+ client `get_bytes(key) -> Result<Option<Vec<u8>>>`，语义明确「None 仅代表键不存在」。`get()` 刻意不变。纯新增，minor。文件 `redis/value.rs:39-44`、`redis/client.rs:277-282`。

**DB-17｜From<redis::Value> 将 Map/Set 摊平为 Array，修复 RESP3 下 HGETALL/SMEMBERS 静默返空** · effort S
现状把 RESP3 的 `Map`/`Set` 降级为 `format!("{:?}")` 串，而 `hgetall`/`collect_string_array` 假设结果是 Array 再 `as_array()`——RESP3 下拿到 String，`as_array()` 返回 None，静默返回空 Vec。在 From 转换处把 Map 摊平为交替 `[k1,v1,k2,v2,...]` Array（正好匹配 RESP2 HGETALL 线格式与 `step_by(2)` 解析）、Set 转元素 Array，上层所有解析对 RESP2/RESP3 无感一致，无需改任何命令方法。默认 RESP2 不走该分支故无影响；RESP3 从「静默返空」变正确返回。向后兼容修 bug。文件 `redis/value.rs:178-189`。

### 12.11 弹性与生命周期（I2 / I3 / I4）

**I2｜DatabaseConfig 接出连接池自愈三参（min_connections / max_lifetime / test_before_acquire）** · effort S
sqlx `PoolOptions` 原生支持这三个自愈参数，本仓只差透传。MySQL/PG 对称改造，默认值保持当前行为（0/None/false）故运行期语义对未改配置者完全不变。min_connections 维持热连接避免冷启动惊群；max_lifetime 让连接在 failover/wait_timeout 杀掉前主动轮换；test_before_acquire 借出前 PING 把「先失败再替换」变成透明自愈。**破坏点**：`DatabaseConfig` 字段全 pub 且文档/测试用字面量构造，新增字段是源码级破坏——建议补 `with_*` 链式方法并在 CHANGELOG 标注（字面量构造者需 `..Default::default()`），或加 `#[non_exhaustive]` 一次性收口。max_lifetime 用 Option + checked 透传无 panic。文件 `mysql/database.rs:26/93`、`postgres/database.rs:25/92`。

**I3｜yang-db 补连接池优雅 drain/close 原语（Database::close / RedisClient::close）** · effort S
停机根因在底层池无法主动 drain：sqlx `Pool::close()` 停止发新连接、等待在途归还后关闭（正是 K8s 滚动需要的语义），deadpool 同理。把 drain 能力建在持有池的那一层（yang-db），yang-base 只编排。补 `Database::close()`/`is_closed()`（PG 对称）与 `RedisClient::close()`/`is_closed()`。纯新增、幂等、无 panic（close 不返回 Result）。close 后再用会返回 PoolClosed 类错误而非 panic（期望行为，文档说明）。minor。文件 `mysql/database.rs:74`、`postgres/database.rs:73`、`redis/client.rs:22/1742`。

**I3｜yang-base 编排式优雅停机 + 信号处理（EngineShutdown：plugin → redis → db 顺序 drain）** · effort M
在 yang-db 原语之上提供单一停机入口，按「与启动相反」顺序收尾：先 `PluginManager::shutdown`（业务先停接活），再关 Redis，最后 drain MySQL（与 `DatabaseBundle::init` 先 MySQL 后 Redis 严格逆序）。配一个 tokio 信号 helper（ctrl_c + unix SIGTERM），让 K8s SIGTERM 触发 drain 而非 RST 在途连接。与 `DatabaseBundle` 的「统一初始化入口」形成对称的「统一停机入口」。**约束**：信号 helper 的 `signal()` 失败必须改 checked（log + 降级为仅 ctrl_c），不得 `.expect`（违反禁新增 panic）。OnceLock 不重置但 close 是原地 drain，文档说明「停机后不应再 dispatch」。minor。文件 `database/bundle.rs:37`、`global.rs`、新增 `lifecycle.rs`。

**I4｜yang-base 分层配置体系 EngineConfig（默认 < TOML 文件 < 环境变量，12-factor）** · effort M
聚合配置类型 `EngineConfig`，三层覆盖：内置默认 → 可选 TOML 文件 → 环境变量（优先级最高）。用 serde 反序列化文件（workspace 已有），env 覆盖走 checked 解析（失败返回 `BaseError::ConfigError`，复用既有变体零新增）；TOML 收在新的可选 `config` feature 后（toml = optional dep），默认不开则退化为纯 env。`from_env()` 直接产出可喂给 `DatabaseBundle::init` 的子配置，打通「一行启动」。不引 figment/config 重依赖。所有解析 checked→ConfigError（禁 panic）。env 前缀统一 `YANG_`。轻量替代：只落 `from_env`（纯 std，降为 S）。minor。新增 `crates/yang-base/src/config.rs`。

### 12.12 锦上添花（N1–N12，第六节 Tier-Nice）

**N1｜受保护层字段表达式/别名/计算列/DISTINCT** · effort M
`select_fields` 只能裸列名。引入封闭的、与权限系统对齐的投影项枚举 `SelectItem<T>{Field/Aliased/Distinct/Agg}`，列引用沿用 `T::Field` 封闭枚举杜绝任意字符串注入，表达式只允许白名单聚合函数作用在已校验字段上，别名限 `[A-Za-z0-9_]`，DISTINCT 作查询级标志。`query_params` 增 `projection: Option<Vec<String>>`（已转义）+ `distinct`。旧 `select_fields` 保留，向后兼容 minor。（与 I8 聚合同源，可合并设计。）

**N2｜子查询/EXISTS/IN(SELECT) 与多列 RETURNING** · effort L
(a) `WhereCondition` 增 `InSubquery{field, sql, params}`/`Exists{sql, params}`，子查询是受过权限校验的 TableQuery 产出的 `(sql, params)` 片段，父查询拼接时占位符重编号，复用 `build_select_sql` 不动 query_builder。(b) MySQL 无 RETURNING，`insert_returning<T>()` 在同一事务里 INSERT 后按自增主键 SELECT 回整行（PG 走真 RETURNING），语义统一为「返回插入后实体」。`WhereCondition` 加变体须配 `#[non_exhaustive]`（合并 I9）。依赖事务进受保护层（C1）。

**N3｜大结果集流式读取** · effort M
`select()` 一次性 `fetch_all` 撑爆内存。不引入 sqlx `.stream()`（其借用 pool 的 lifetime 难穿过受保护层 API），而是 `for_each_chunk<T,F>(chunk, f)`：基于主键游标分块循环 fetch，每块回调处理，内存上界 = chunk_size（复用 I7 keyset）。`select()` 不变，向后兼容 minor。游标分块在并发写下仍可能漏/重，文档注明用于离线批处理而非强一致快照。

**N4｜DB/Redis 瞬时错误重试/退避** · effort M
前置是错误可分类（DB-7/C5 的 `is_retryable()`）。在 yang-base 暴露与具体 IO 解耦的 `retry(policy, op)` 组合子：对返回 `BaseError` 的 async 闭包做指数退避 + 抖动，**只对 `is_retryable()` 的变体重试**。分类下沉到错误本身（单一真相），重试是纯函数式组合子。不自动套用（避免对非幂等写重试），由调用方显式包裹，文档强调只对幂等操作或带去重键的写使用。sleep 用 tokio。新增 `crates/yang-base/src/resilience/retry.rs`。minor。

**N5｜统一健康/就绪聚合端点** · effort S
`HealthReport::probe()` 并行探测各已初始化子系统（MySQL `SELECT 1` / Redis PING），区分 liveness 与 readiness，各探测受 feature gate 控制（未启用则跳过），未初始化单例报 `Unconfigured` 而非 `Down` 避免误判。返回结构化报告便于序列化成 `/healthz`、`/readyz`，**不绑 HTTP 框架**（保持 crate 中立），由下游 web 层挂端点。纯增量 minor。新增 `crates/yang-base/src/health/mod.rs`。

**N6｜事件/钩子总线** · effort L
除中间件洋葱链外无 emit/subscribe。轻量同步事件总线：泛型 `Event` trait + 类型键索引订阅者表（`TypeId → Vec<handler>`）。锁中毒用 `into_inner()` 恢复（与 context/validator 一致，不像 circuit_breaker 用 expect），`emit` 不在持锁期间 await（克隆 handler 列表后释放锁再调）。先做同步 typed-hook 版满足审计/慢查询钩子。**热路径零分配**：默认空订阅者时 emit 仅一次读锁+空 Vec 检查，可进一步用 OnceLock 缓存「有无订阅者」标志规避读锁。同步 handler 不得阻塞（文档强调）。纯增量 minor。新增 `crates/yang-base/src/event/mod.rs`。

**N7｜错误人体工学构造器与 .context() 链** · effort M
只能裸 `BaseError::Variant("...".to_string())` 构造，调用点充斥 `.map_err(|e| BaseError::X(e.to_string()))`。两层糖：(a) 语义化构造器 `config`/`unknown`/`param`（接受 `impl Into<String>`）省 `.to_string()`；(b) `ResultExt::context` trait 给任意 Result 链上人类可读上下文。**约束**：不能破坏结构化变体（ValidationFailed 等）携带的字段与错误码——`context` 默认走 Unknown 会丢码，文档建议仅在最外层用；需保码场景提供 `.context_keep()` 仅日志不改变体。不引 anyhow（保持零额外依赖）。纯增量 minor。文件 `error/mod.rs:415`。

**N8｜内置 Action Output DTO 一致性 + 成功文案** · effort S
两个味道：(1) `AffectedResult` 定义在 `add.rs` 却被 `put.rs`/`del.rs` 反向 `use`（寄居错位）；(2) blanket `DynAction::dispatch` 硬编码 `ApiResponse::success(output, "成功")`（`typed.rs:86`），所有 Action 文案一刀切。把共享 DTO 收敛到 `action/builtin/dto.rs` 单一归属（保留 `pub use` + `#[deprecated]` 兼容），`TypedAction` 提供可覆盖 `success_message()` 默认 `"成功"`，dispatch 改用 `self.success_message()`，派生宏支持 `#[action(success_message="新增成功")]`。有默认实现不破坏手写者。minor。文件 `add.rs:13`、`put.rs:4`、`del.rs:4`、`typed.rs:34-87`。

**N9｜错误码与变体双射/去重** · effort S
与 12.5「码双射」同源，从 Tier-Nice 视角强化：撞码本身可接受（同类对外同码），真正的债是缺不变量守护 + 重复变体。(a) 补列举全变体的双射体检测试，登记「允许同码」白名单，新增变体引入未登记撞码即测试失败；(b) `MigrationFailed` `#[deprecated]` 收敛到 `DatabaseMigrationFailed`；(c) enum 标 `#[non_exhaustive]`（评估亦点名）。`#[non_exhaustive]` 是 SemVer 破坏需显式标注，deprecate 不删兼容，撞码不改对外码稳定。文件 `error/mod.rs:31/92-97/444-524` + 测试区。

**N10｜extract_input 的 schema 校验闸 + 避免整体克隆** · effort S
两个独立小债。(1) `input_schema` 已生成却未当运行期校验闸——`feature="plugin-schema"/"validator"` 时可在 handler 前用 jsonschema 校验 body 命中 schema，违例转 `ValidationFailed`（放 feature gate，默认不开避免热路径开销与误拒）。(2) `extract_input` 每次 `from_value(body.clone())` 深克隆整个 body——dispatch 独占 ctx，改 `take_input(&mut self)` 用 `std::mem::take` 取走 body 免深克隆。take 后 body 变 Null 不可二次提取（文档注明）。minor。文件 `context.rs:268`、`typed.rs:82`。

**N11｜cancellation 半状态收口** · effort M
两处取消安全洞。(a) 熔断器 `allow_at` 在 Open 冷却结束插入 HalfOpen 放行探测，若随后 `.await` 被取消，HalfOpen 名额已消耗却永无收尾，卡在 HalfOpen——用 RAII 探测守卫 `Probe`，`Drop` 时若既未成功也未失败则回滚 Open（借 Drop 在取消时也执行收口）。(b) `LogoutAction` 两次 revoke 之间被取消会留半登出——并发 `tokio::join!` 拉黑两 token 缩小窗口，revoke 本身幂等故重试安全（文档化）。顺带把 `circuit_breaker.rs:82/106/124` 的 `.expect` 统一改 `into_inner`（消除中毒即 panic，呼应 I9/I10）。pub API 不变，patch/minor。文件 `circuit_breaker.rs:81-145`、`auth.rs:355-371`。

**N12｜单例 connect-then-set 并发资源浪费** · effort S
`GlobalDatabase::init` 先 `connect_with_config` 建池再 `OnceLock::set`，并发 init 会各自建池、后者 set 失败但池已建（连接已占）；`GlobalTools::init` 同模式。**方案 A**：用 `tokio::sync::OnceCell` 的 `get_or_try_init` 把建池纳入临界区，只建一次，其余等待复用——但语义微调（重复 init 从报 `AlreadyInitialized` 变复用首池）。**方案 B（零行为变更，推荐）**：用 `Mutex<()>` 包住 connect+set 序列，set 已存在则跳过建池，保留报错语义。若依赖旧报错行为则 A 是行为破坏需 CHANGELOG 标注。tokio sync 已在 workspace。minor。文件 `database/global.rs:45/91-125`、`action/context.rs:26/141-166`。

---

> **方案集完整性**：截至本节，第 8-10 节 yang-base（C1-C6 等）、yang-db（DB-1..DB-21）、第六节 Tier-Nice（N1-N12）、文档勘误（DOC-1..4）与深度复核新增（NG-1..4）的每一个问题，均已在 12.1-12.12 给出至少一个贴合架构、标注兼容性与 effort 的优雅方案。落地时按第七节优先级顺序推进，每条完成回填 11.4/11.5 状态表。

### 12.1 事务与原子写（C1 / DB-5 / I5 / I6 / DB-4）

**DB-5｜Transaction 暴露受控 sqlx 执行器逃生舱（C1 的 db 侧前置）** · effort S
开 `#[doc(hidden)] fn executor(&mut self) -> Option<&mut sqlx::MySqlConnection>`，把内部 sqlx 连接借给同 workspace 上层执行其自构建的参数化语句。yang-base 本就直接依赖 sqlx，泄漏连接与现有耦合一致，无需新抽象；值由调用方 `?` 绑定，标识符安全由 yang-base 的 quote 保证。PG 侧对称返回 `&mut PgConnection`。文件 `mysql/transaction.rs:192`、`postgres/transaction.rs`。纯新增，向后兼容。

**C1｜TableQuery 增 `*_in_tx` 终端变体 + 执行器泛型化** · effort L
TableQuery 已是「构建/执行分离」结构，把各终端方法重复的 bind+execute 收敛进一个对 `sqlx::Executor` 泛型的私有 `run_execute`（`&MySqlPool` 与 `&mut MySqlConnection` 都实现），现有 insert/update/delete/select 改调它（行为零变化），再加一组 `*_in_tx(&mut yang_db::Transaction, ...)`。事务不进 builder 字段（规避 `&mut` 生命周期与链式 move 冲突），只在终端调用点传入，builder 链完全不动；权限/校验/软删/MissingWhere 守卫全部复用。`ActionContext::begin_transaction()` 转发 `GlobalDatabase::transaction`。复用 `DbError::TransactionError` 不新增 BaseError 变体。文件 `table_query.rs:1615/1669/1881/2142`、`context.rs:358`。全部新增、`#[cfg(feature="mysql")]`，向后兼容 minor；依赖 DB-5。

---

## 十三、二次审计新发现问题（2026-06-13）

> 本节为 2026-06-13 二次审计（workflow `wf_98b60c08-4f0` 8 透镜 + `wf_bd92c883-2f9` 3 透镜，共 60 agent）经**对抗式复核（isReal && isNew 双判，18+10=28 候选 → 24 确认，4 项被否决/降级）** 保留的、文档第一~十二节**确实未覆盖**的新问题。每条均经 file:line 取证 + 独立复核臂确认非现有条目换皮。编号 NEW-1..NEW-24，按层与严重度组织。

### 13.1 施行进度追踪表（新问题）

| ID | 问题 | 层 | 类型 | 严重度 | 状态 | 取证 |
|----|------|----|------|--------|------|------|
| NEW-1 | dispatch 根 span `request_id` 以具体值声明非 `Empty`，RequestIdMiddleware 透传 `record` 成 no-op，上游 X-Request-Id 不生效 | base | bug | Medium | ✅ | 根 span 改 `tracing::field::Empty` 声明 + 进入后先 record 默认值（2026-06-14，commit afdd1d1） |
| NEW-2 | Action metrics 缺 `module` 标签，跨模块同名内置 Action（user::add / order::add）指标碰撞 | base | gap | Medium | ✅ | `ActionContext.module` 字段 + `with_module`，`ModuleRouter::dispatch` 注入，typed.rs 三 metrics 补 module 标签（2026-06-14，commit afdd1d1） |
| NEW-3 | `FieldConfig.filterable/.sortable` 是死配置——查询路径从不读，`.filterable(false)` 字段仍可被筛选/排序 | base | gap | Medium | ✅ | `validate_filter_field`/`order_by` 权限校验前先断言字段级开关（2026-06-14，commit 625a266） |
| NEW-4 | 空/恒真布尔组（`where_and(vec![])`）使 `where_conditions` 非空，静默绕过 MissingWhereClause 全表写守卫，生成 `WHERE (1=1)` | base | safety | Medium | ✅ | `validate_condition_tree` 对空 And/Or 组返回 ParamInvalid（覆盖 where_and/or/tree+嵌套）（2026-06-14，commit 625a266） |
| NEW-5 | `page/page_size` 无上界，`offset=(page-1)*page_size` 乘法可溢出（debug panic / release 回绕），与注释「直接构造也安全」矛盾 | base | bug | Low | ⏳ | `table_query.rs:856-866/1703-1708`、`query_params.rs:387/393` |
| NEW-6 | 按用户批量撤销同秒 off-by-one（`iat < min_iat` 严格小于），改密/强制下线当秒签发的 Token 可绕过撤销 | base | safety | Medium | ✅ | 抽 `iat_revoked_by_watermark` 用 `<=`，消除同秒旁路（2026-06-14，commit e9993eb） |
| NEW-7 | 上游 `X-Request-Id` 仅接受 ≤32 位十六进制，标准 UUID/traceparent 被静默丢弃，跨服务串联失效 | base | gap | Medium | ✅ | `parse_hex` 先去连字符再 hex 解析，接受标准 UUID；u128 承载限制已文档化（2026-06-14，commit afdd1d1） |
| NEW-8 | 非事务迁移模式 execute-then-record 非原子，崩溃/记录失败致迁移已应用未登记会重跑 | base | bug | Medium | ⏳ | `initializer.rs:366-371`（vs 事务模式 409-421） |
| NEW-9 | 批量 INSERT/UPDATE 列集仅取首条记录，异构记录的列被静默丢弃/置 NULL（`WHEN id=NULL` 永不匹配） | db | bug | Medium | ✅ | build_insert_batch/build_update_batch 校验所有记录列集一致否则 InvalidArgument（MySQL+PG）（2026-06-14，commit e3115c4） |
| NEW-10 | `json_value_to_sql_value` 的 FieldType 类型提示在值形态不匹配时被静默忽略，跌落默认转换 | db | bug | Medium | ✅ | DateTime/Timestamp/Blob/Text 形态不匹配返 TypeConversionError（NULL 放行），MySQL+PG（2026-06-14，commit e3115c4） |
| NEW-11 | `SqlValue` 的 `From<u64>` 用 `v as i64` 截断，u64 > i64::MAX（BIGINT UNSIGNED/雪花 ID）静默环绕为负 | db | bug | Medium | ✅ | `From<u64>` 对 >i64::MAX 走 String（MySQL+PG）（2026-06-14，commit e3115c4） |
| NEW-12 | PG `Database` 缺 `pool_status()`/`health_check()`，与 MySQL/Redis 后端不对称（C4 落地漂移） | db | gap | Medium | ✅ | PG Database 补 pool_status/health_check，与 MySQL 对称（2026-06-14，commit afdd1d1） |
| NEW-13 | PG 事务原生 SQL 助手把 JSON 数组/对象绑为文本串，对 JSONB 列报类型错；与非事务路径绑原生 JSONB 不一致 | db | bug | Medium | ✅ | bind_json_param_tx/_as_tx 的 JSON arm 改 `bind(other.clone())` 绑原生 JSONB（2026-06-14，commit e3115c4） |
| NEW-14 | Token 撤销/黑名单 6 个安全 API 零行为测试（仅 2 条 key 格式断言），登出/强制下线安全保证无背书 | base | test-gap | Low | ⏳ | `revocation.rs:181-194`（测试）/53/65/88/115/138/165（API） |
| NEW-15 | 认证 Action（Login/Refresh/Logout）+ 审计钩子 + `token_fingerprint` 全链路零测试，C4「已完成」测试维度无背书 | base | test-gap | Medium | ⏳ | `auth.rs:188/203/251/413/504`；唯一 token e2e 仅跑 CRUD（`typed_action_integration.rs:123-185`） |
| NEW-16 | 公开枚举 `FieldType` 未标 `#[non_exhaustive]`，新增字段类型即 SemVer 破坏；I9/C5 硬化未覆盖它 | base | gap | Low | ✅ | `FieldType` 已标 `#[non_exhaustive]`（2026-06-14，commit 8e06e6e） |
| NEW-17 | 集合类命令（smembers/lrange/hgetall/sinter…）对二进制/非 UTF-8 元素静默丢弃整条，结果数组比真实短、与 LLEN/SCARD/HLEN 不一致 | db | bug | Low | ⏳ | `client.rs:1931-1936`(collect_string_array)/664-682(hgetall)、`value.rs:39-44` |
| NEW-18 | 批量命令（del/exists/mget/sadd…）传入空切片下发无参命令，触发 `wrong number of arguments` 错误而非 no-op | db | bug | Low | 🟨 | pipeline/tx 的 push/sadd/zadd 已随 NEW-21 加空切片短路；client.rs 的 del/exists/mget 仍待办 |
| NEW-19 | Redis WATCH 事务无法做读-改-写：命令 build 期固定，重试只重放同一批固定值，乐观锁退化为 CAS-set | db | bug | Medium | ✅ | `watch()` 文档化「仅 CAS、不支持读-改-写」，指引 Lua/业务层重试（闭包 API 侵入过大）（2026-06-14，commit e15dc3f） |
| NEW-20 | Redis WATCH 冲突重试无指数退避/抖动，争用下以 RTT 为节奏最多重发 100 次放大 Redis 压力 | db | perf | Low | ⏳ | `transaction.rs:304-343`（continue 无 sleep/backoff；非 CPU 忙等，每轮有网络 RTT） |
| NEW-21 | 多元素 `lpush/rpush/sadd/zadd` 展开为 N 条独立命令，破坏 pipeline/tx「每命令一个结果」契约，按索引取结果错位 | db | bug | Medium | ✅ | 改用变参单命令（zadd 用 zadd_multiple），空切片 no-op（pipeline+transaction）（2026-06-14，commit e15dc3f） |
| NEW-22 | 经 SET 写入的数字读回恒为 `RedisValue::String`，`as_i64()`/`as_f64()` 永远返回 None（BulkString 从不解析数值） | db | gap | Low | ⏳ | `value.rs:165-172`（BulkString→String）、as_i64 51-56/as_f64 63-68 仅 match Int/Float |
| NEW-23 | `RedisConfig` 不校验退化参数（max_connections/connect_timeout/wait_timeout=0），无 checked 错误，0 值产生不可用客户端且错误信息不透明 | db | safety | Low | ⏳ | `config.rs:53-65`、`client.rs:78-91/94-97`（违 checked API 优先） |
| NEW-24 | `test_logging_config` 为空断言假绿测试，true/false 两分支断言相同，未验证 `enable_logging` 行为 | db | test-gap | Low | ⏳ | `tests/test_redis_config.rs:186-199`、`client.rs:105-107`（与 DB-19 不同轴：断言缺失 vs 硬 expect） |

**新问题严重度分布**：High 0 / Medium 13 / Low 11。**层分布**：base 10、db 14。**类型**：bug 11、gap 6、safety 3、test-gap 3、perf 1。

> 严重度校准说明：本轮无 High——确认的安全/正确性缺口均需调用方主动触发（空布尔组、异构批量数据、u64 顶半区、同秒撤销窗口）或仅在退化配置/二进制值下显现，不构成无条件生产阻断或外部可控注入；故顶格 Medium。被对抗复核**否决/降级**的 4 项：Redis 集合二进制丢弃（Medium→Low，与 DB-16/17 校准）、WATCH 重试「紧自旋」定性（误判，实为有 RTT 的无退避，Medium→Low）、Token 撤销测试缺口（High→Low，机制本身正确仅缺回归网）、RedisConfig 校验（Medium→Low，返回 Err 非 panic）。

### 13.1.1 施行进度（2026-06-14 大批量修复）

> 用户授权一次性修复全部 ⏳ 项（除 C2b/C3 两个 XL 架构重构），按 §11.6 重排优先级分批推进，每子任务单独提交 master（不推送）。实现+提交串行（共享工作树），核查用只读 workflow 并行复核每项当前 file:line。

**已完成批次**（截至 2026-06-14）：

| Batch | 范围 | commit | 状态 |
|-------|------|--------|------|
| 1 | 安全/正确性：NEW-3/4/6/9/10/11/13 + DB-14 + DB-11(部分) | 625a266 / e9993eb / e3115c4 | ✅ |
| 2 | yang-db P0 三件套：DB-1/2/3 | 5296536 / 717a191 | ✅ |
| 3 | 可观测性：NEW-1/2/7/12 + NG-1 | afdd1d1 | ✅ |
| 4 | 弹性：I2/I3/I4 | 2d0e315 | ✅ |
| 5 | yang-db Medium：DB-4/6/9/10 + NEW-19/21 + NG-2/3 | e15dc3f | ✅ |
| 6 | 错误硬化：NEW-16（FieldType non_exhaustive） | 8e06e6e | 🟨 进行中（码双射/N7 待办） |

**累计**：本轮已落地 NEW-1/2/3/4/6/7/9/10/11/12/13/16/19/21（14 项）、DB-1/2/3/4/6/9/10/14（8 项）、I2/I3/I4（3 项）、NG-1/2/3（3 项）。DB-11、NEW-18 为部分完成（🟨）。

**测试与门禁**：yang-base 381 lib（all-features）、yang-db 321 lib 全绿；两 crate production clippy（`--lib -D warnings`）干净。注意 yang-db **全量** `-D warnings` 仍因 M-1（~418 测试 unwrap/expect）报错——属未启动的测试清理项，非本轮回归。

**剩余待办**（按 §11.6 优先级）：
- Batch 6 续：码双射去重（MigrationFailed `#[deprecated]` + 双射回归测试）、N7 错误工效构造器
- Batch 7：写路径桥接 I5（批量插入）/I6（UPSERT）/I7（keyset 分页）/I8（聚合）+ N1（字段表达式/DISTINCT）
- Batch 8：类型保真 I1（PutInput 定型枚举）/写库接缝/响应擦除 + I11（plugin TOCTOU）/I12（dispatch 背压）
- Batch 9：Redis/Low 收尾 DB-12/13/15/16/17 + NEW-5/8/17/18(续)/20/22/23
- Batch 10：测试背书 DB-19/21 + NEW-14/15/24 + NG-4 + DB-18 跨后端一致性 + Nice N2~N12
- C2b（JOIN）/C3（多后端）两个 XL 架构项按约定不在本轮，留待单独立项



### 13.2 yang-base 层详情（NEW-1~8 / 14~16）

**NEW-1｜request_id 透传失效（一行修复，杠杆最高）** · safety-adjacent bug
`module_router.rs:355-360` 的 dispatch 根 span 把 `request_id` 字段以**具体 display 值**声明（`request_id = %context.request_id`），而 tracing 语义下只有以 `tracing::field::Empty` 声明的字段才能被后续 `record` 更新。`middleware.rs:138-143` 的 `RequestIdMiddleware` 解析上游 `X-Request-Id` → `with_request_id` 替换 ctx → `Span::current().record("request_id", ...)`，但对已赋值字段是 no-op。**后果**：上游传入 X-Request-Id 时，根 span 仍显示本地生成的旧 id，而下游 handler/table_query 读到新值——同一请求出现两个 request_id，跨服务串联在 span 层断裂。同文件注释（353-354）明写「先以 Empty 占位」，与代码自相矛盾。**修复**：把 359 行改为 `request_id = tracing::field::Empty`。

**NEW-2｜Action metrics 缺 module 标签** · gap
`typed.rs:105/109-128` 三个 metrics 宏仅带 `action`(+status/code)，无 `module`；埋点在 blanket `impl<T:TypedAction> DynAction::dispatch` 边界，只能取 `self.meta()`，而 `ActionMeta`(meta.rs:9-24) 无 module 字段。**后果**：user::add 与 order::add 在 `yang_action_requests_total{action="add"}` 下合并计数，丧失按资源/表区分 QPS/错误率/P99 的能力，偏离 §12.4 C4 设计承诺的 `{module,action,status}`。**修复**：`ActionMeta` 增 module 字段，由 `table_typed`/register 注入，dispatch 边界补 label。

**NEW-3｜filterable/sortable 死配置（被误以为已关闭的查询面）** · gap/safety-adjacent
`field_config.rs:91/96` 的两个 bool + docstring（以 password 字段举例）承诺「filterable=false 不能用于 WHERE / sortable=false 不能用于 ORDER BY」，但 `validate_filter_field`(table_query.rs:919-926) 只查 `permissions.can_filter`、`order_by`(824-830) 只查 `can_sort`，**从不读这两个 bool**（grep 全树仅 setter 自身赋值）。又因 `filterable_roles`/`sortable_roles` 空 = 放行所有角色，`.filterable(false)` 字段在默认配置下仍可被任意筛选。**后果**：开发者标记敏感字段期望禁筛，实际无效，攻击者可对这些列做 WHERE/ORDER BY 探测（二分 LIKE 探密码哈希、按内部列排序泄序）。**修复**：`validate_filter_field`/`order_by` 在权限校验前先断言 `field_config.filterable`/`.sortable`，或移除这两个开关并文档化「筛选/排序仅由角色权限控制」。

**NEW-4｜空布尔组绕过全表写守卫** · safety
`where_and(vec![])` 把 `WhereCondition::And{conditions:vec![]}` 推入 `where_conditions`（validate_condition_tree 对空组遍历 0 次直接 Ok），使其 `len=1≠空`。UPDATE 守卫 `table_query.rs:2411`（`where_conditions.is_empty() && !allow_full_table`）判为非空 → 不触发；render_group(1452-1454) 对空 And 渲染 `1=1`，最终 `UPDATE ... WHERE (1=1)` 全表写。DELETE 同理（2613）。**后果**：程序拼接条件时「过滤列表为空」未特判即可让 MissingWhereClause/allow_full_table 守卫形同虚设，全表静默改写/软删且不报错。**修复**：`validate_condition_tree` 对空 And/Or 组返回 `ParamInvalid`，或写守卫改为「`where_conditions` 中所有条件渲染后均为恒真常量时也视作全表」。

**NEW-5｜分页 offset 乘法溢出** · bug（Low）
`table_query.rs:1707` `let offset = page.saturating_sub(1) * page_size;`——saturating_sub 仅防 page==0 下溢，乘法对接近 usize::MAX 的入参无保护：debug panic（违禁生产 panic 约定）、release 静默回绕出错误 OFFSET。`page()`(856-866) 只校验 `==0` 无上界；`QueryParams.page/page_size` 为 pub 可绕过入口直接构造（内置 SelectAction 的 `page_size<=100` 限制只覆盖内置路径）。注释（1705-1706）自称「直接构造也安全」与实际矛盾。**修复**：`offset` 改 `checked_mul` 失败返 `ParamInvalid`，`page()` 加合理上界。

**NEW-6｜按用户撤销同秒 off-by-one** · safety
`revocation.rs:116` 写水位线 `min_iat = now`（秒级），`172-175` 用严格 `claims.iat < min_iat` 判定；签发侧 `manager.rs:273/312` `iat = now`（同秒级）。同一秒内签发的 Token `iat == min_iat`，`<` 为 false → 不撤销。**后果**：改密/强制下线发生在第 T 秒，攻击者（持泄漏凭据）在第 T 秒登录拿 iat=T 的 Token 可越过批量撤销，1 秒窗口内撤销失效。**修复**：判定改 `<=`，或水位线写 `now+1`（取过撤一侧）。

**NEW-7｜X-Request-Id 仅接受 ≤32 位十六进制** · gap
`request_id.rs:51-57` `parse_hex`：`len > 32` 或含非 hex 字符即 None；`middleware.rs:132-141` 解析失败无声保留本地新 id。**后果**：36 位带连字符 UUID、W3C traceparent、字母数字 trace id 全被拒绝静默丢弃，跨服务链路无法串联。根因是 `RequestId` 为 u128，结构上无法承载任意字符串 id。文档 C4-reqid（标 ✅）只称「透传」未记此限制。**修复**：要么文档化「仅支持 ≤32 hex」，要么把 RequestId 改为可承载任意上游字符串（如 `Cow<'static,str>`）+ 本地生成回退。

**NEW-8｜非事务迁移 execute-then-record 非原子** · bug
`initializer.rs:366-371` 非事务模式 `execute(sql)` 与 `record_migration` 是两步独立调用无事务包裹（对照事务模式 409-421 在同一 tx 内）。**后果**：SQL 成功但 record 失败/两步间崩溃 → 未写 `_migrations` → 下次启动重跑，对非幂等迁移（无 IF NOT EXISTS 的 ALTER / 种子 INSERT）报错或重复插入。文档把「幂等迁移（事务/非事务两模式）」列入成熟项，未记此缺口。**修复**：非事务模式也应「先 record（标记 in-progress）再 execute 再标 done」或文档化「非事务迁移必须自身幂等」。

**NEW-14｜Token 撤销 API 零行为测试** · test-gap（Low）
`revocation.rs:181-194` 测试模块仅 2 条 key 格式断言，6 个 pub async 安全 API（revoke_token/revoke_claims/is_revoked/revoke_by_subject/subject_min_iat/verify_token_checked）无行为测试。机制本身正确（故 Low），但 `iat<min_iat` 边界（见 NEW-6）、`ttl==0` 短路、脏数据返 None「避免误杀」等关键分支若回归无测试可捕获。**修复**：补 `#[ignore]` Redis 集成测试覆盖六 API 的成功/失败/边界路径。

**NEW-15｜认证 Action + 审计钩子全链路零测试** · test-gap
`auth.rs` 的 Login/Refresh/Logout 三 Action、AuthAuditHook/TracingAuditHook、`token_fingerprint`（声称「绝不泄漏原文」）在 `__tests__/` 与 `tests/` 命中为 0；唯一 token e2e 仅跑六件 CRUD。**后果**：C4「认证审计钩子 ✅ 已落地」在测试维度无背书——审计是否真在成功/失败两路触发、事件是否只带指纹不带明文、FNV-1a 指纹是否稳定，回归静默通过。**修复**：补 auth Action 的 mock-verifier 单测 + 审计事件捕获断言（验证不含 token 原文）。

**NEW-16｜FieldType 缺 #[non_exhaustive]** · gap（Low）
`field_type.rs:59-60` 公开枚举 `FieldType`（经 table/mod.rs:66 导出）无 `#[non_exhaustive]`，而 I9/C5 已为 BaseError/ErrorCategory/WhereCondition 加上。**后果**：未来新增字段类型（Decimal/Uuid/Json 细分）对所有在 FieldType 上穷举 match 的下游 crate 构成破坏性变更，须 major，与文档「加变体不破坏」目标自相矛盾。**修复**：标 `#[non_exhaustive]`（与同主题硬化一并）。

### 13.3 yang-db 层详情（NEW-9~13 / 17~24）

**NEW-9｜批量写列集仅取首条记录** · bug
`mysql/query_builder.rs:398` `fields` 取自 `data_list[0].keys()`，循环里 `430` 缺失列 `unwrap_or(&Value::Null)`、首条没有的额外列被完全忽略；build_update_batch 同构（561-565/592/629），缺失 id 生成 `WHEN id=NULL`（永不匹配，该行静默不更新）。全程无记录间列一致性校验。**后果**：传入字段集不一致的批量数据时部分列静默丢弃/写 NULL，与逐行 insert 行为不一致，隐蔽数据正确性 bug。**修复**：取所有记录列名并集，或校验列集一致否则返 `InvalidArgument`（配合 DB-14）。

**NEW-10｜FieldType 类型提示静默 fallthrough** · bug
`mysql/query_builder.rs:724-728`（Timestamp 仅 as_i64 才 return）/750-754（Text 仅 as_str）/738-748（Blob）/711-722（DateTime 非字符串跌穿，而坏字符串却报 Err——分支内自相矛盾），值形态不匹配时跌穿到 763 默认 match。**后果**：`.timestamp(f)`/`.text(f)`/`.blob(f)` 显式标注的字段类型期望在 shape 稍偏即静默退化为通用绑定，无告警。与 NG-3（Decimal 降 f64）是不同侧面。**修复**：类型提示分支在 shape 不匹配时返 `TypeConversionError` 而非跌穿。

**NEW-11｜From<u64> 截断环绕** · bug
`mysql/condition.rs:62-66` `From<u64>` 用 `v as i64`（wrapping 语义），u64 > i64::MAX（BIGINT UNSIGNED 高半区、无符号雪花 ID）静默环绕成负数后 `bind(*i)`。**后果**：以 u64 传大值时写入/查询条件被静默改成负数，错误匹配/写错数据无报错。**修复**：`From<u64>` 对 > i64::MAX 走 `SqlValue::String`（MySQL 接受十进制串）或新增 `SqlValue::UInt(u64)` 变体。

**NEW-12｜PG Database 缺 pool_status/health_check** · gap
`postgres/database.rs:78-281` 整个 impl 无 `pool_status`/`health_check`，而 MySQL（132/149）与 Redis（client.rs:1730/1742）都有。**后果**：C4 把这两个方法下沉 yang-db 时漏掉 PG 后端，直接消费 PgDatabase 者无法探活、连接耗尽时无池快照，三后端能力不对称。**修复**：PG `Database` 补 `pool_status`（包 `PgPool::size()/num_idle()`）与 `health_check`（`SELECT 1`），与 MySQL 对称。

**NEW-13｜PG 事务 JSON 参数绑文本串** · bug
`postgres/transaction.rs:596/634` 的 `other` arm `query.bind(other.to_string())` 把 JSON 数组/对象绑为 text，而非事务路径 `database.rs:315` `bind(other.clone())` 绑原生 JSONB。**后果**：对 JSONB 列，事务内 `execute_with_params` 发 text 报 `column is of type jsonb but expression is of type text`，非事务同语句成功——PG 内部 tx↔非tx 行为分叉。DB-10 只修同函数的浮点 arm，不触及此 arm。**修复**：`other` arm 改 `bind(other.clone())` 与非事务对齐。

**NEW-17｜集合命令二进制元素静默丢条** · bug（Low）
`client.rs:1931-1936` `collect_string_array` 用 `filter_map(|v| v.as_string())`，被 smembers/sinter/sunion/sdiff/lrange/zrange/hkeys/hvals/scan 共用；`as_string()`(value.rs:39-44) 对非 UTF-8 Bytes 返 None 被整条剔除。`hgetall`(664-682) 更严重：field 与 value 双双 as_string 成功才 push。**后果**：存二进制成员时返回 Vec 比真实短、与 LLEN/SCARD/HLEN 计数不一致且无错误（与 DB-16 单值 None 失败模式不同）。**修复**：collect 改保留 Bytes 的变体，或提供 `*_bytes` 系列（配合 DB-16 的 get_bytes）。

**NEW-18｜批量命令空切片下发无参命令** · bug（Low）
`client.rs:1584-1591`（del）/1597-1604（exists）/334（mget）/1062（sadd）等均 `for x in slice { cmd.arg(x) }` 后直执行，无空输入短路。**后果**：空切片本应 no-op，但 Redis 对无参 DEL/SADD 返回 `ERR wrong number of arguments` → `RedisCommandError`，调用方在「待处理列表恰为空」边界拿到错误而非 Ok(0)。**修复**：各批量方法开头加 `if keys.is_empty() { return Ok(默认值) }`。

**NEW-19｜WATCH 事务无法读-改-写** · bug
`transaction.rs:88-91` watch() 仅记键名，命令在 build 期（101-261）把固定值写入 pipe，exec()(292-344) 重试分支裸 `continue` 重放同一 pipe，WATCH 与 EXEC 间无读取被监视键/重算的钩子。**后果**：典型乐观锁「WATCH balance; GET; compute; SET」结构上无法表达，退化为固定值 CAS-set；文档却宣称「基于 WATCH/MULTI/EXEC 实现乐观锁」并给 `tx.watch(&["balance"])` 示例，误导调用方。与 DB-2 不同根（DB-2 是冲突检测被吞）。**修复**：提供接受闭包的 `watch_then(keys, |conn| async {...})` API 在冲突时重跑闭包，或文档明确「仅支持固定值 CAS」。

**NEW-20｜WATCH 重试无退避** · perf（Low）
`transaction.rs:304-343` 冲突分支 `retries+=1` 后直接 continue，无 sleep/backoff/jitter（最多 100 次）。**注**：非 CPU 忙等——每轮有 WATCH+EXEC 两次网络 RTT 且 .await 让出，原报告「紧自旋」定性已被复核否决。真问题是争用下以 RTT 节奏重发放大 Redis 压力。**修复**：重试间加指数退避 + 抖动（依赖 tokio::time）。

**NEW-21｜多元素 push/sadd 破坏 pipeline 结果契约** · bug
`pipeline.rs:171-225`、`transaction.rs:195-249` 的 lpush/rpush/sadd/zadd 对 values 切片逐元素 `add_command`（而非 redis-rs 变参单命令），N 元素展开为 N 条命令各返一个结果。但 query()/execute() 文档（pipeline.rs:259/301）承诺「每命令一个结果」，len()(345) 按底层命令计数。**后果**：`pipeline.set(a).sadd("s",&["x","y","z"]).get(b)` 期望 3 个结果实得 5 个，按索引取结果整体错位，len() 与方法调用次数不符。**修复**：用变参一次性 `self.pipe.lpush(key, values)` 生成单命令。

**NEW-22｜SET 写入的数字读回恒为 String** · gap（Low）
`value.rs:165-172` From<redis::Value> 对 BulkString 一律先转 String，从不解析数值；仅 `redis::Value::Int/Double` 才映射 Int/Float。而 GET/HGET 数字在 RESP 中是 BulkString。as_i64(51-56)/as_f64(63-68) 只 match Int/Float。**后果**：`get("counter")` 即便存 42 也得 `String("42")`，as_i64() 返 None，调用方必须自己 parse，与便捷访问器预期相悖。**修复**：as_i64/as_f64 对 String 变体回退 `s.parse().ok()`。

**NEW-23｜RedisConfig 不校验退化参数** · safety（Low）
`config.rs:53-65` `RedisConfig::new` 对 max_connections/connect_timeout/wait_timeout 直接赋值无校验，`client.rs:78-91` 原样喂入 PoolConfig。max_connections=0 → deadpool 信号量 0 permit，pool.get()(94-97) 永远失败，以不透明 `RedisConnectionError("获取连接失败")` 报错而非明确配置错误。**后果**：0 值产生看似构造成功却不可用的客户端，违 checked API 优先，排障困难。**修复**：`new`/`connect_with_config` 对 0 值返 `InvalidConfig`/checked 错误。

**NEW-24｜test_logging_config 假绿** · test-gap（Low）
`tests/test_redis_config.rs:186-199` enable_logging=true 与 =false 两分支断言完全相同（均只 `assert!(result.is_ok())`），从不检查日志产生；而 enable_logging 仅 gate `client.rs:105-107` 一行 log::info!。**后果**：对 enable_logging 提供虚假覆盖信心，日志逻辑回归仍绿（与 DB-19 硬 expect/无 #[ignore] 不同轴，是断言缺失）。**修复**：用 log 捕获断言验证 true 分支产生日志、false 分支静默。

> 本节为活文档，随修复推进回填 13.1 表状态/commit/日期。新问题修复优先级见 11.6「优先级重排建议」第 1、3 项。



