# YANG 基础库能力与成熟度评估

> 评估对象：`D:\code\lib_yang` 下的 `yang_*` workspace
> 源码快照：`c33b603f278b4b303142f9b1252e88da6c36e6bf`
> 评估日期：2026-07-26
> 评估方法：第一性原理约束、静态结构与调用链分析、测试/CI 契约核对、本机质量门禁
> 说明：文中的分数是用于排序和决策的工程量表，不是统计学测量。

## 一、结论先行

`yang-base + yang-db` 已经不是“基础工具集合”，而是一套有明确运行时模型的模块化应用内核：定义在启动期校验并冻结，运行期通过预解析句柄派发，数据访问在统一边界实施字段、租户和危险写保护。以“单体/模块化单体、MySQL 为应用层主数据库、Redis 提供会话与基础设施能力”为目标场景，它已达到 **工程完整、可进入受控试运行的水平**，但尚不称为 L4 生产候选，更不等同于已经由长期生产流量证明。

平台内核（`yang-base + yang-db + yang-base-derive`）评为 **3.8/5（L3 后段）**；整个 `yang_*` workspace 的治理成熟度评为 **3.5/5**。`yang-pcg` 是正交领域，不参与平台内核平均分。差距主要来自产品边界和治理尚未完全收敛：

- `Addon / Module / Action` 已经形成正确的唯一业务扩展主线，但旁边仍保留另一套 `PluginManager / PluginRegistry`，而 `definition::Plugins` 又表示“内部 Action 调用入口”，三个“插件”语义相互冲突。
- `yang-db` 同时支持 MySQL、PostgreSQL、Redis，但 `yang-base` 的受保护表访问仍明确是 MySQL 模型。能力矩阵需要把“底层可用”和“平台已承诺”区分开。
- CI 的稳定主门禁集中覆盖 `yang-base` 和 `yang-db`，没有把 `yang-migrate`、`yang-pcg`、`yang-base-derive` 作为同等级 workspace 成员治理。
- 已有性能基准代码，但没有形成可重复的基线比较、噪声阈值和合并阻断机制。

因此，下一阶段的主题不应是继续横向增加框架能力，而应是：**收窄承诺、统一概念、补齐治理、用基准守住单运行时热路径**。

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
| 性能设计 | 3.8 | 关键路径避免动态查找，但仍有角色集合重复分配等源码漂移 |
| 可读性与开发体验 | 3.8 | 原生 DSL 连贯；扩展概念和部分动态记录模型增加认知负担 |
| 测试与发布治理 | 3.4 | 核心 crate 门禁强，workspace 成员覆盖和性能回归治理不足 |
| 产品边界清晰度 | 3.2 | 应用层 MySQL 承诺、基础设施插件和独立 PCG 能力需重新分组 |
| **平台内核** | **3.8** | **L3 后段；工程闭环较完整，可进入受控试运行** |
| **workspace 治理** | **3.5** | **成员分组、发布和性能治理仍处于 L3** |

### 分 crate 判断

| crate | 定位 | 成熟度 | 关键判断 |
|---|---|---:|---|
| `yang-base` | 应用定义、派发、鉴权、传输、受保护数据访问 | 4.0 | 主干成熟，优先做概念和错误模型收敛 |
| `yang-db` | MySQL/PostgreSQL/Redis 低层能力 | 4.1 | 能力面完整，方言和能力边界显式 |
| `yang-base-derive` | Action 等派生宏 | 3.6 | 由主 crate 间接覆盖较多，但应成为显式 CI 对象 |
| `yang-migrate` | 受控源码迁移/codemod | 3.2 | 对当前职责足够；不能误当作生产数据库迁移引擎 |
| `yang-pcg` | 确定性程序化地图生成 | 3.8 | 自身设计完整，但与业务系统内核正交，应独立治理 |

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

### F-04：`ActionContext` 的角色缓存与实现已经漂移

**优先级：P2；影响：热路径分配、源码可信度。**

`ActionContext` 已声明 `cached_roles` 优化槽，但当前仍被允许为 dead code；`table_query()` 每次把角色 `HashSet` 克隆成 `Vec` 再转为 `Arc<[String]>`。连接池也在每次创建查询器时重新包装。

单次成本尚未被测量，不能称为性能瓶颈；但这是典型的“注释宣称优化已完成，运行代码没有消费它”，会破坏性能设计文档的可信度。

**建议路径：**

- 在身份注入时一次性构建不可变角色切片，所有 `TableQuery` 共享；
- 或删除无效缓存字段和 PERF 注释，先用基准证明无需优化；
- 记录每次 dispatch 的分配次数和 `table_query()` 微基准；
- 以同一机器、同一 profile、足够样本比较改动前后，不使用单次绝对值。

**验收条件：**

- 不再有为热路径优化预留却未消费的 dead field；
- 角色集合不会在同一请求的每次表查询中重复深拷贝；
- 吞吐、p50/p95、分配数均有基线，稳定热路径回归超过 3% 时阻断。

### F-05：workspace 成员没有同等级治理

**优先级：P1；影响：发布可靠性。**

当前稳定 CI/本地 `run_ci.py` 的主要 test、doc、Clippy、feature matrix 和 MSRV 对象是 `yang-base`、`yang-db`。`yang-base-derive` 会被间接编译，但 `yang-migrate` 和规模可观的 `yang-pcg` 没有进入同等级门禁。

**建议路径：**

- 二选一：
  1. 把平台相关 crate 纳入统一 workspace 门禁，并为 `yang-pcg` 增加独立 job；
  2. 把正交的 `yang-pcg` 拆成独立 workspace/repository，拥有自己的版本和 CI。
- 对 `yang-base-derive` 增加显式 `trybuild`/编译失败用例门禁；
- 对 `yang-migrate` 增加自身单元测试、fixture 和幂等性门禁；
- 在各发布 crate 的 `Cargo.toml` 声明与 CI 一致的 `rust-version`。
- 审计后引入 `cargo deny` 或等价 RustSec 门禁，明确 advisory 例外的 owner 和到期日。

**验收条件：**

- `cargo test --workspace --all-targets` 或等价的分组 job 覆盖每个 workspace 成员；
- 每个可发布 crate 都有明确 MSRV、owner 和发布条件；
- 独立领域 crate 的失败不会被核心 crate 的绿色状态遮蔽。

### F-06：性能基准尚未形成回归治理

**优先级：P1；影响：核心设计承诺。**

`crates/yang-base/benches/runtime_baseline.rs` 已说明团队意识到热路径重要，但普通 CI 未执行稳定基线比较。没有固定 runner、历史基准和噪声处理时，无法证明“变更没有性能回退”。

**建议路径：**

- 在专用或可校准 runner 上记录 dispatch、typed call、UI catalog 投影和 TableQuery 构建；
- 同时记录吞吐、p50、p95、分配次数、锁等待；数据库路径另记 SQL 数量和序列化成本；
- PR 只做相对基线比较，采用重复采样和置信/噪声带；
- 稳定回归超过 3% 阻断，噪声区间内要求人工复核而非直接失败；
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

### F-08：可观测能力存在，但尚未形成平台级闭环

**优先级：P2；影响：运维和诊断。**

基础库已有 tracing、request id、慢查询阈值、health 和可选 metrics feature，但还缺少一份从能力启用、标签约束、导出到告警的规范。代码中同时存在 `log` 与 `tracing`，也增加了统一采集的成本。

**建议路径：**

- 新代码统一使用 `tracing`；
- 规定低基数标签：service、version、route/action、result，不把 user/tenant id 当 metrics label；
- 给下游应用提供 metrics exporter、结构化日志和 trace 接入示例；
- 把 health、readiness、slow query 和资源 close 事件纳入统一运维契约。

**验收条件：**

- 下游应用可用一个受支持示例完成结构化日志、metrics 和 trace 接入；
- 指标标签通过低基数检查；
- request id 能关联 dispatch、慢查询和资源故障；
- 本项属于平台治理建议；在 exporter/SLO 尚未成为公共承诺前，不作为核心库功能正确性的阻断项。

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
3. 把 `yang-base-derive`、`yang-migrate`、`yang-pcg` 纳入显式 CI，或先拆分 job。
4. 固化运行时基准协议和当前主分支基线。

**退出条件：** 每个 crate 的定位、支持矩阵、owner、门禁和性能基线均可查。

### 阶段 1：统一核心语义（3—6 周）

1. 重命名内部 `Plugins` 调用入口。
2. 将基础设施插件与业务 Addon 解耦，迁移数据库初始化使用者。
3. 为 RelationLoader、Tools 生命周期和扩展收敛补回归测试。

**退出条件：** 新开发者只沿 Addon 主线即可实现业务模块；旧插件语义不再进入新代码。

### 阶段 2：生产治理闭环（6—12 周）

1. 在稳定 runner 上启用性能相对比较和 3% 阻断规则。
2. 统一 tracing/metrics/health 接入规范。
3. 设计领域错误到稳定公共错误投影，处理同义变体。
4. 修正 `cached_roles` 源码/注释漂移，并用测量决定是否保留缓存。
5. 为所有发布 crate 声明 MSRV、SemVer 和发布检查。
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

- **源码已证实：** 构建期冻结、预绑定派发、租户/字段保护、扩展概念冲突、错误变体重复、角色缓存漂移；
- **测试/CI 契约已证实：** 核心 crate 有单元、属性、文档、feature matrix 和真实依赖集成门禁；
- **尚不能据此宣称：** 高并发生产负载、跨版本兼容稳定性、灾难恢复时间、长期无性能回归。

### 本机新鲜验证

| 命令 | 结果 | 覆盖 |
|---|---|---|
| `python scripts/run_ci.py quick` | 通过 | CI 契约、feature isolation 自检、rustfmt、Clippy all targets/all features、核心单元与文档测试 |
| `cargo test -p yang-base-derive -p yang-migrate -p yang-pcg --all-targets --locked` | 通过 | `yang-migrate` 4 项、`yang-pcg` 351 + 11 项；4 项 generation benchmark 被设计为 ignored；`yang-base-derive` 自身为 0 个直接单元测试 |

`quick` 的可见测试结果为：

- `yang-db`：387 个 library test 通过，64 个 doc test 通过；
- `yang-base`：540 个 library test 通过、4 个 ignored；38 个 doc test 通过、115 个 ignored；
- Clippy 和格式门禁通过。

这些证据强化了核心实现质量，也直接佐证了 F-05：额外 crate 当前可以单独通过，但没有进入同等级主门禁；`yang-base-derive` 仍主要依赖间接/调用方覆盖。

本轮没有执行根 workspace 的 MySQL/PostgreSQL/Redis 全套故障注入，也没有执行可比较性能基线，因此不作对应通过声明。当前环境也未安装 `cargo-audit`/`cargo-deny`，仓库主门禁未发现 RustSec advisory 扫描，Rust 依赖漏洞状态不作绿色声明。`yang-system` 的真实 MySQL/Redis 纵向集成另见系统评估。
