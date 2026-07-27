# YANG 基础库能力与成熟度评估

> 评估对象：`D:\code\lib_yang` 下的 `yang_*` workspace
> 初评源码快照：`c33b603f278b4b303142f9b1252e88da6c36e6bf`
> 复评源码快照：`f899decdf086c43edf63a3cad9b6cc4f0617fc29`
> 后端收尾快照：`14c3ed8d032b52a76212cefa6fed212cd46db2c4`
> 评估日期：2026-07-26
> 后端收尾日期：2026-07-27
> 评估方法：第一性原理约束、静态结构与调用链分析、测试/CI 契约核对、本机质量门禁
> 说明：文中的分数是用于排序和决策的工程量表，不是统计学测量。

## 一、结论先行

`yang-base + yang-db` 已经不是“基础工具集合”，而是一套有明确运行时模型的模块化应用内核：定义在启动期校验并冻结，运行期通过预解析句柄派发，数据访问在统一边界实施字段、租户和危险写保护。以“单体/模块化单体、MySQL 为应用层主数据库、Redis 提供会话与基础设施能力”为目标场景，它已达到 **L4 入口的受控生产候选水平**，但仍不等同于已经由长期生产流量、灾备演练和稳定性能门禁证明。

平台内核（`yang-base + yang-db + yang-base-derive`）复评为 **4.0/5（L4 入口）**；整个 `yang_*` workspace 的治理成熟度复评为 **4.0/5**。`yang-pcg` 是正交领域，不参与平台内核平均分。当前差距已经从“基础门禁不完整”收敛为“产品边界、概念与长期治理尚未完全统一”：

- `Addon / Module / Action` 已经形成正确的唯一业务扩展主线，但旁边仍保留另一套 `PluginManager / PluginRegistry`，而 `definition::Plugins` 又表示“内部 Action 调用入口”，三个“插件”语义相互冲突。
- `yang-db` 同时支持 MySQL、PostgreSQL、Redis，但 `yang-base` 的受保护表访问仍明确是 MySQL 模型。能力矩阵需要把“底层可用”和“平台已承诺”区分开。
- workspace 全成员测试、Clippy、MSRV 和依赖策略已经进入统一门禁，初评中的成员治理缺口已经关闭。
- Action/SQL/Redis 已具备统一 trace 关联边界；`yang-system` 已提供结构化日志、Prometheus、OTLP 和 readiness/SLO 的真实消费实现。
- 请求角色缓存的实现漂移已经修复：认证时一次性冻结角色切片，后续 `TableQuery` 复用该切片。
- 性能治理已有可重复的 shadow baseline、噪声协议和 CI 产物，但尚未在稳定 runner 上启用“稳定回归超过 3%”的合并阻断。

因此，下一阶段的主题不应是继续横向增加框架能力，而应是：**收窄承诺、统一概念、补齐治理、用基准守住单运行时热路径**。

### 复评增量

本轮不是按代码量重新打分，而是逐项核对初评问题是否形成了“实现 + 门禁 + 失败方式”闭环：

| 初评问题 | 复评状态 | 已落地证据 | 剩余边界 |
|---|---|---|---|
| F-05 workspace 治理 | **已完成** | `467b9b3` 全成员 full gate；`5798a78` 统一 MSRV；`ef7a925` Rust 依赖策略 | 继续维护 advisory 例外 owner 与到期日 |
| F-06 性能回归治理 | **部分完成** | `0489625` runtime shadow baseline、重复采样、噪声带和 CI 产物 | 稳定 runner 与 >3% 阻断仍待落地 |
| F-04 请求角色缓存 | **已完成** | `14c3ed8` 在认证边界一次性构建稳定角色切片，查询构建仅 `Arc::clone` | `TableQuery` 内部角色集合成本继续由 F-06 度量 |
| F-08 可观测性闭环 | **基础闭环已完成** | `22e2a41`、`a79054a`、`30611d4`；下游系统已接入日志、metrics、trace、readiness 与告警规则 | 真实告警路由、阈值校准和滚动发布演练 |
| 认证/授权扩展点 | **能力增强** | `b3f5ae4` application claims validator；`f899dec` 授权新鲜度错误分类 | 生产效果仍依赖下游 writer/read-path 完整接入 |
| 事务与受保护定位能力 | **能力增强** | `1927893` 事务 mutation handler；`89c8659` 受保护主键定位 | 仍需坚持单一原生运行链，避免业务旁路扩散 |

所以分数维持在 L4 入口，不因为完成若干实现点就虚增成熟度。F-01、F-02、F-03 仍是概念、承诺与错误边界的核心收敛项；F-06 的稳定 runner 和长期样本仍是从“受控候选”走向成熟平台的硬门槛。

## 二、成熟度口径

| 等级 | 含义 |
|---|---|
| L1 | 只有骨架，核心路径未闭环 |
| L2 | 可运行原型，依赖约定和人工兜底 |
| L3 | 工程闭环，具备测试和明确边界 |
| L4 | 受控生产候选，关键故障模式有防线 |
| L5 | 成熟平台，兼容、运维、性能和演进均有长期治理 |

### 综合评分

| 维度 | 评分 | 判断 |
|---|---:|---|
| 核心运行时模型 | 4.5 | 构建期解析、冻结、预绑定和单次解码方向正确 |
| 数据安全边界 | 4.2 | 字段权限、租户域、软删除、参数化查询和危险写保护完整 |
| 错误与故障模型 | 3.5 | 错误码和错误链较完整，但中心错误枚举膨胀且有同义变体 |
| 性能设计 | 4.0 | 关键路径避免动态查找并已有 shadow baseline，但阻断门槛尚未启用 |
| 可读性与开发体验 | 3.8 | 原生 DSL 连贯；扩展概念和部分动态记录模型增加认知负担 |
| 测试与发布治理 | 4.2 | workspace 全成员、MSRV、依赖策略已统一；性能目前为非阻断 shadow |
| 产品边界清晰度 | 3.3 | 应用层 MySQL 承诺、基础设施插件和独立 PCG 能力仍需重新分组 |
| **平台内核** | **4.0** | **L4 入口；可作为受控生产候选，尚缺长期运行证明** |
| **workspace 治理** | **4.0** | **全成员质量门禁已闭环，性能阻断与发布长期治理待完成** |

### 分 crate 判断

| crate | 定位 | 成熟度 | 关键判断 |
|---|---|---:|---|
| `yang-base` | 应用定义、派发、鉴权、传输、受保护数据访问 | 4.1 | 主干成熟并支持应用声明校验，优先做概念和错误模型收敛 |
| `yang-db` | MySQL/PostgreSQL/Redis 低层能力 | 4.1 | 能力面完整，方言和能力边界显式 |
| `yang-base-derive` | Action 等派生宏 | 3.8 | 已成为显式 CI 对象，直接编译失败契约仍可继续加强 |
| `yang-migrate` | 受控源码迁移/codemod | 3.5 | 已纳入统一门禁；不能误当作生产数据库迁移引擎 |
| `yang-pcg` | 确定性程序化地图生成 | 3.9 | 已纳入统一门禁，但与业务系统内核正交，宜独立发布治理 |

## 三、从第一性原理建立评估基线

基础库是否“完善”，不取决于 API 数量，而取决于以下不变量能否持续成立：

1. **定义只有一个事实源。** 同一 Action 的参数、权限、路由、UI 元数据和运行处理器不能长期双写。
2. **错误尽量在启动期暴露。** 名称冲突、依赖缺失、路由冲突、引用错误和中间件顺序错误不应进入请求期。
3. **运行热路径只消费冻结产物。** 请求期不做兼容转换、不重建注册表、不按字符串遍历查找处理器。
4. **安全默认值位于不可绕过的边界。** 租户条件、字段读写、密文字段、全表更新/删除保护不能依赖每个业务 Action 自觉实现。
5. **能力承诺必须可验证。** “支持 PostgreSQL”“支持插件”“可生产使用”都要有端到端使用者和对应门禁，而不只是某个底层类型存在。
6. **故障必须可诊断、可恢复。** 错误链、稳定错误码、资源关闭、健康检查和请求关联是运行时契约的一部分。
7. **抽象必须减少总复杂度。** 如果一个抽象只消除几行重复，却增加第二条运行链或第二套术语，它就是负收益。
8. **性能承诺需要比较基线。** 单次跑分快不等于没有回归；必须比较吞吐、p50/p95、分配、锁竞争以及数据库/序列化开销。
9. **结构正确不等于生产正确。** 冻结 Registry 不能自动证明策略无误、并发安全、隔离无旁路或故障后可恢复。
10. **演进本身必须有契约。** API、Catalog、数据库 Schema、feature 和 MSRV 都要定义兼容窗口及失败方式。

当前核心内核基本满足 1—4、6，主要差距集中在第 5、7、8、10 项。

## 四、已经成熟的能力

### 4.1 启动期编译、运行期冻结

`crates/yang-base/src/definition/builder.rs` 中的 `AppBuilder::build` 依次完成：

- Addon、Module、所有权和依赖唯一性校验；
- 模块内容、中间件顺序、字段引用和路由校验；
- 参数字段继承、定义排序和运行时 View 编译；
- Action 句柄解析、强类型内部调用绑定；
- `Catalog / Registry / Tools / TableDefinition` 冻结。

`Registry::dispatch` 直接通过构建期 slot 取得唯一处理器，再注入已经解析的模块、Action、表定义、策略和中间件。强类型内部调用也走预解析句柄，不需要把结构体重新序列化成 JSON，更没有请求期字符串查找链。

这条链满足“配置期付出复杂度、运行期保持简单”的核心目标，也是基础库最有价值的能力。

### 4.2 单一原生定义语言已经形成

`Addon → Module → Action / Table / View`、`params!`、`fields!` 和派生宏已经提供一致的开发节奏。业务系统可以在一个原生模型中同时得到：

- 运行时处理器；
- 参数与返回类型；
- 路由；
- 权限策略；
- 表和字段规则；
- UI Catalog。

这比“后端 DTO + 路由配置 + 权限配置 + 前端表单 Schema”四份事实源更接近逻辑最简。后续应继续强化这一条链，而不是引入兼容适配器或第二注册表。该方向也与既有设计约束一致，参见 [`BR 体验兼容的 YANG 原生核心设计`](../superpowers/specs/2026-07-17-br-experience-compatible-yang-core-design.md)。

### 4.3 受保护数据访问边界扎实

`crates/yang-base/src/action/context.rs` 与 `crates/yang-base/src/table/` 将运行身份、角色、表定义和租户上下文注入 `TableQuery`。已具备：

- 可读、可写、可筛选、可排序字段约束；
- secret 字段默认不投影；
- 普通租户自动追加 tenant key 条件；
- 软删除处理；
- 嵌套 `AND / OR` 条件；
- 受控标识符和参数化值；
- 无 `WHERE` 的更新/删除拒绝；
- 事务内 `select / insert / update / delete` 路径。

这些能力处于业务 Action 下面的公共执行边界，因而比散落在 service/repository 中的手工检查更可靠。

### 4.4 数据层能力面完整且边界较清楚

`crates/yang-db/src/mysql/`、`postgres/` 和 `redis/` 提供：

- MySQL/PostgreSQL 查询构建、事务、聚合、Join、Upsert、Union 和子查询；
- 严格标识符类型，避免把表达式误当成表名或字段名；
- Redis pipeline、事务和 Lua 脚本；
- 显式 `BackendCapabilities`，使方言差异可见。

这意味着 `yang-db` 已经具备独立低层库的厚度。需要治理的不是功能缺口，而是上层平台究竟承诺哪些组合。

### 4.5 生命周期与故障表达有良好基础

`crates/yang-base/src/tools.rs` 把数据库、Redis、Token、HTTP 和扩展配置冻结在应用资源中，生命周期状态区分 `Running / Closing / Closed`，关闭操作串行化且可重复调用。`BaseError` 提供错误链、稳定分类/编码，传输层能够屏蔽服务端内部错误，并用 request id 串联诊断。

资源没有依赖进程级可变单例，这对并行测试、多应用实例和可预测关闭非常重要。

### 4.6 测试深度优于一般基础库

核心代码存在大量单元、属性、文档和真实依赖集成测试，包括：

- SQL 标识符和注入边界；
- 查询生成属性测试；
- MySQL/PostgreSQL/Redis 实际集成；
- 强类型 Action 调用；
- TableQuery CRUD、分页和事务；
- feature 隔离矩阵；
- MSRV、Clippy、rustfmt 和文档测试。

这使许多安全和正确性结论不只停留在接口设计层。

## 五、关键问题与改进建议

### F-01：扩展机制存在三套相近语义

**优先级：P1；影响：可读性、长期演进、公共 API。**

当前同时存在：

1. 产品级 `Addon / Module / Action`；
2. `crates/yang-base/src/plugin/` 的 `PluginManager / PluginRegistry`；
3. `crates/yang-base/src/definition/plugins.rs` 的 `Plugins`，实际职责是发起强类型内部 Action 调用。

`DatabaseInitializer::initialize_all` 又直接接收旧 `PluginManager`。这会让使用者无法从名称判断“插件”究竟是产品模块、基础设施生命周期扩展，还是内部服务调用器。

**建议路径：**

- 明确 `Addon` 是唯一业务扩展单元；
- 把 `definition::Plugins` 重命名为 `ActionCaller`、`Actions` 或 `Services`；
- 盘点 `PluginManager` 的真实使用者：
  - 若只承载基础设施生命周期，将其改名并收窄为 `InfrastructureExtensions`；
  - 若没有生产使用者，先标记 deprecated，再在下一个主版本删除；
- 数据库初始化改为消费显式 migration/initializer 清单，不再借用“插件列表”表达迁移顺序；
- 全程禁止把两套注册机制挂到同一次请求派发链。

**验收条件：**

- 公共文档中每个扩展概念只有一个职责定义；
- 新建业务模块只需理解 Addon 主线；
- `BuiltApp` 内只有一个 Action Registry；
- 基准证明重命名/收敛没有引入请求期转换或超过约定阈值的回归。

### F-02：底层多数据库能力与平台承诺没有分层表达

**优先级：P1；影响：架构边界、用户预期、测试成本。**

`yang-db` 同时支持 MySQL 和 PostgreSQL，但 `yang-base` 的 `TableQuery`、事务入口和动态记录读取仍是 MySQL 类型。这不是缺陷；对当前 `yang-system` 而言，显式选择 MySQL 反而更简单。问题在于文档或 feature 名称可能让使用者把“底层驱动可用”理解为“完整平台后端可替换”。

**建议路径：**

- 当前版本正式声明：`yang-base` 应用平台链以 MySQL 为首个完整实现；
- `yang-db` 保持独立的 MySQL/PostgreSQL/Redis 能力；
- 提供一张端到端能力矩阵，区分“连接/查询构建器可用”和“TableQuery/Schema/UI/集成测试全链可用”；
- 只有出现真实 PostgreSQL 产品需求后，才设计中立 executor；不要先为“看起来通用”改造成泛型蔓延的抽象层。

**验收条件：**

- README、Cargo feature 和 CI 对同一能力给出一致承诺；
- 未达到端到端覆盖的后端不得标记为平台级支持；
- 若未来加入 PostgreSQL 平台链，必须有与 MySQL 对等的租户、权限、事务和集成测试。

### F-03：中心 `BaseError` 已经承担过多领域

**优先级：P2；影响：SemVer、可读性、维护成本。**

`crates/yang-base/src/error/mod.rs` 已覆盖插件、数据库、Redis、HTTP、Token、字段、Action 等大量领域，并存在明确的同义或近义变体，例如：

- `DatabaseConnectionFailed` / `DatabaseConnectionDbError`；
- `DatabaseMigrationFailed` / `MigrationFailed` / `MigrationExecutionFailed`；
- `RedisOperationFailed` / `RedisOperationDbError`。

这使任何新增基础设施能力都扩大一个公共枚举的变更半径。

**建议路径：**

- 保留稳定的传输层错误投影：`code / category / retryable / public_message`；
- 内部分拆 `DatabaseError`、`TokenError`、`DefinitionError`、`ActionError` 等领域错误；
- 用 `From` 在边界投影到公共错误，不在每个调用点手工映射；
- 先标记同义变体 deprecated，建立生成或表驱动的 code/category 测试，再在主版本清理；
- 不要丢失 `source` 错误链。

**验收条件：**

- 同一种故障只对应一个规范错误语义；
- 传输错误码在重构前后保持兼容；
- 新领域能力不再要求修改一个超大枚举的多个映射函数。

### F-04：`ActionContext` 的角色缓存与实现漂移（已完成）

**优先级：P2；影响：热路径分配、源码可信度。**

初评确认 `ActionContext` 虽声明 `cached_roles`，但 `with_user()` 没有填充它，`table_query()` 也没有消费它；源码注释与真实运行路径不一致。

`14c3ed8` 已在受信认证注入边界对角色去重并稳定排序，冻结为 `Arc<[String]>`；普通与系统表查询都只克隆 `Arc`。同时删除 dead-code 豁免并加入缓存内容回归测试。该改动静态上消除了同一请求每次查询时的 `HashSet → Vec → Arc` 重建，但没有把本机噪声跑分包装成性能收益结论。

**已完成路径：**

- 身份注入时一次性构建不可变角色切片，所有 `TableQuery` 共享；
- 重复角色由 `User.roles` 去重，缓存再排序，避免对外观察到请求内漂移；
- 聚焦单测、`yang-base` 全 library test 与 all-target/all-feature Clippy 均通过。

**验收结果与剩余边界：**

- ✅ 不再有为热路径优化预留却未消费的 dead field；
- ✅ 角色切片不会在同一请求的每次表查询中重复深拷贝；
- ◐ 吞吐、p50/p95 与稳定 3% 阻断属于 F-06，必须在低方差 runner 上完成。

### F-05：workspace 成员没有同等级治理（已完成）

**优先级：P1；影响：发布可靠性。**

复评时该问题已经关闭：full gate 显式覆盖每个 workspace 成员的 all-target tests 和 Clippy；所有成员继承统一 `workspace.package.rust-version`，MSRV 门禁会核对成员集合；`cargo deny` 与仓库策略脚本共同检查 advisory、license、source 和例外到期信息。

**已完成路径：**

- `467b9b3` 将辅助 crate 的 all-target tests、全 workspace Clippy 和 CI 契约核对纳入 full gate；
- `5798a78` 统一声明并验证 workspace MSRV；
- `ef7a925` 增加 `cargo deny` 与依赖策略自检，例外必须携带理由、复核日期和退出条件。

**验收条件：**

- full gate 的测试与 Clippy 包集合必须与 workspace 成员集合完全一致；
- 每个成员显式继承统一 MSRV，依赖策略配置能被仓库脚本自检；
- 正交领域 crate 的失败不能被核心 crate 的绿色状态遮蔽。

### F-06：性能基准尚未形成回归治理（部分完成）

**优先级：P1；影响：核心设计承诺。**

`0489625` 已将 runtime benchmark 推进为 shadow baseline：配置明确场景、重复采样、相对比较和噪声处理，CI 保存报告但暂不阻断。它解决了“没有可重复协议”的问题；由于共享 runner 噪声和历史样本仍需校准，目前仍不能证明所有合并都没有性能回退。

**建议路径：**

- 保持现有 dispatch、typed call、UI catalog 投影和 TableQuery 场景的 shadow 数据连续；
- 在专用或可校准 runner 上积累稳定历史样本，并补充分配数、锁等待；数据库路径另记 SQL 数量和序列化成本；
- 从非阻断 shadow 分阶段升级为相对基线门禁；
- 稳定回归超过 3% 时阻断，噪声区间内要求人工复核而非直接失败；
- 基线升级必须带原因和评审，不允许“更新基线让红灯变绿”。

**验收条件：**

- 任一核心运行时 PR 都能回答“相对主分支变化多少”；
- 结果可重跑，原始数据作为构建产物保留；
- 性能结论不依赖开发机的一次跑分。

### F-07：动态 `Record` 应保持在通用管理边界

**优先级：P2；影响：类型安全、业务可读性。**

`Record` 对动态表格、通用 CRUD 和 Catalog 驱动 UI 是合理抽象，但若扩散到领域服务，会把字段拼写、必填约束和返回结构退化到运行时。

**建议路径：**

- 通用管理页面和基础 CRUD 可以继续使用 `Record`；
- 资金、身份、授权、状态机等领域行为使用强类型 input/output；
- 在 Action 边界完成动态记录与领域类型转换；
- 文档明确“动态记录是 UI/数据通道，不是默认领域模型”。

**验收条件：**

- 安全敏感和状态机 Action 的输入输出均为强类型；
- `Record` 的允许使用边界写入架构检查或 review checklist；
- 动态到强类型的转换错误具有稳定错误语义和回归测试。

### F-08：可观测能力已形成基础闭环

**优先级：P2；影响：运维和诊断。**

基础库现在用 `tracing` 串联可信 request/action/actor/tenant 维度，并覆盖 Action 派发与 SQL/Redis 受保护边界；下游 `yang-system` 已实现固定字段 JSON 日志、低基数 Prometheus 指标、OTLP/W3C TraceContext、独立管理面、readiness 单一预算和 Prometheus 告警规则。它证明了基础能力可以沿单一运行链被真实应用消费，而不是停留在可选 feature。

**已完成路径：**

- 新代码使用 `tracing`，Action 与数据库边界携带同一 request id；
- metrics 固定低基数 service/action/result 等标签，user/tenant 只进入 trace/log 字段；
- 下游系统提供 exporter、结构化日志、trace、health/readiness 与告警规则的可执行样例；
- 生命周期关闭与依赖退化具有有界状态和诊断原因。

**剩余运营验收：**

- 在真实 Prometheus/Alertmanager 环境验证规则加载与告警路由；
- 用稳定负载校准延迟、连接池和授权传播阈值；
- 在多副本滚动发布中验证 readiness 撤销、在途请求排空和关闭时限。

## 六、能力边界：哪些“不做”反而更成熟

以下事项当前不建议实施：

- 不为保留旧 BR 内部实现而增加兼容运行时、双 Registry 或请求期定义转换；
- 不为了“数据库无关”把整个 Action/Service 泛型化；
- 不把 `Record` 推广为所有领域对象；
- 不继续增加第四种“插件”概念；
- 不把 `yang-pcg` 强行包装成业务系统内核的一部分；
- 不在普通共享 GitHub runner 上用一次 benchmark 绝对值决定回归；
- 不为了消除少量 SQL 而强迫通用查询 DSL 表达所有领域 Join 和锁语义。

逻辑最简不是文件最少或抽象最少，而是：**每个概念只有一个权威职责，每个运行请求只有一条真实执行链。**

## 七、分阶段改进路径

### 阶段 0：冻结边界与度量（1—2 周）

1. 发布端到端能力矩阵，明确 `yang-base` 当前平台链是 MySQL。
2. 记录公共扩展概念和生产使用者，决定 `PluginManager` 的保留/收窄/废弃方向。
3. ✅ 已把 `yang-base-derive`、`yang-migrate`、`yang-pcg` 纳入显式 CI。
4. ◐ 已固化运行时 shadow 基准协议；稳定 runner 的阻断基线待完成。

**退出条件：** 每个 crate 的定位、支持矩阵、owner、门禁和性能基线均可查。

### 阶段 1：统一核心语义（3—6 周）

1. 重命名内部 `Plugins` 调用入口。
2. 将基础设施插件与业务 Addon 解耦，迁移数据库初始化使用者。
3. 为 RelationLoader、Tools 生命周期和扩展收敛补回归测试。

**退出条件：** 新开发者只沿 Addon 主线即可实现业务模块；旧插件语义不再进入新代码。

### 阶段 2：生产治理闭环（6—12 周）

1. 在稳定 runner 上启用性能相对比较和 3% 阻断规则。
2. ✅ 已完成 tracing/metrics/health 基础接入规范；继续执行真实告警和滚动发布演练。
3. 设计领域错误到稳定公共错误投影，处理同义变体。
4. ✅ 已修正 `cached_roles` 源码/注释漂移；稳定性能判断继续归入 F-06。
5. ◐ 所有 workspace 成员已统一声明并校验 MSRV；SemVer 与发布检查仍待闭环。
6. 生成 capability ledger：每项能力链接实现、测试、feature 和使用者。

**退出条件：** “支持什么、谁在使用、什么测试证明、性能是否退化”都能从一处回答。

### 阶段 3：由真实需求驱动扩展

只有在出现明确产品使用者后，再决定：

- PostgreSQL 是否升级为完整平台后端；
- 是否需要独立 migration runner；
- metrics/OpenAPI/admin metadata 是否进入默认平台 bundle；
- PCG 是否单独版本化和发布。

## 八、建议的架构决策

| 决策 | 建议 |
|---|---|
| 业务扩展模型 | `Addon / Module / Action` 唯一主线 |
| 运行时 | 单一原生 Registry，启动期编译，运行期冻结 |
| 应用主数据库 | 当前明确 MySQL；PostgreSQL 保持 `yang-db` 低层能力 |
| 领域数据 | 强类型优先，动态 `Record` 只服务通用数据/UI 边界 |
| 错误模型 | 领域错误内部化，稳定公共错误在边界投影 |
| 性能门槛 | 相对基线；稳定热路径回归 >3% 阻断 |
| PCG | 独立领域、独立 CI/版本治理 |

## 九、验证记录与结论边界

本评估区分三种证据：

- **源码已证实：** 构建期冻结、预绑定派发、租户/字段保护、扩展概念冲突、错误变体重复，以及角色缓存漂移已关闭；
- **测试/CI 契约已证实：** 核心 crate 有单元、属性、文档、feature matrix 和真实依赖集成门禁；
- **尚不能据此宣称：** 高并发生产负载、跨版本兼容稳定性、灾难恢复时间、长期无性能回归。

### 本机新鲜验证

| 命令 | 结果 | 覆盖 |
|---|---|---|
| `python scripts/run_ci.py quick` | 通过 | CI 契约、feature isolation 自检、rustfmt、Clippy all targets/all features、核心单元与文档测试 |
| `cargo test -p yang-base-derive -p yang-migrate -p yang-pcg --all-targets --locked`（初评） | 通过 | `yang-migrate` 4 项、`yang-pcg` 351 + 11 项；4 项 generation benchmark 被设计为 ignored；`yang-base-derive` 自身为 0 个直接单元测试 |
| full gate 契约（复评） | 已落地 | workspace 全成员 tests/Clippy 集合、统一 MSRV、依赖策略与 CI 配置均有自检 |
| runtime shadow baseline（复评） | 已落地、非阻断 | 重复采样、噪声带、相对报告和 CI 产物；稳定 >3% 阻断尚未启用 |
| `cargo test -p yang-base --lib --locked`（后端收尾） | 通过 | 559 项通过、4 项按真实 Redis 条件 ignored；包含请求角色缓存回归 |
| `cargo clippy -p yang-base --all-targets --all-features --locked -- -D warnings`（后端收尾） | 通过 | F-04 修复及全部 `yang-base` targets/features 无 Clippy 错误 |

初评 `quick` 的可见测试结果为：

- `yang-db`：387 个 library test 通过，64 个 doc test 通过；
- `yang-base`：540 个 library test 通过、4 个 ignored；38 个 doc test 通过、115 个 ignored；
- Clippy 和格式门禁通过。

初评暴露的 F-05 已由后续提交关闭；复评仍保留“派生宏直接负例可加强”的判断，但它不再等同于 workspace 成员未进入主门禁。

后端收尾在同一会话比较 `0489625` 与当前实现时，目标场景曾出现 `+3.31%`、外层变异系数 `5.02%`；后续采样目标变为 `+6.82%`、外层变异系数升至 `8.10%`，同时无关场景也发生两位数漂移。因此这些数据只能证明本机 runner 不满足 3% 阻断的方差条件，不能证明回归或收益。`c734758` 与 `14c3ed8` 的提交依据是消除重复解析/分配和恢复源码不变量，不以该噪声跑分宣称性能提升。

本轮仍没有据此宣称长期生产负载、灾备或跨版本兼容已经成熟。Rust 依赖策略现已进入仓库门禁，但被审计例外仍须按 owner、复核日期和退出条件持续治理。`yang-system` 的真实 MySQL/Redis 纵向集成另见系统评估。
