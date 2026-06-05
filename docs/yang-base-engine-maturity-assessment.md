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
