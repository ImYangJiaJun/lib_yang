# yang-base / yang-db 基础库完善计划

**状态：** Proposed

**制定日期：** 2026-07-15

**范围：** `yang-base`、`yang-db`，以及与二者公共契约直接相关的 `yang-base-derive`

**对标基线：** `br-addon 0.2.20`、其依赖的 `br-db 1.8.71`

**目标：** 把当前“主体功能可用但门禁和系统集成不完整”的基础库，推进为可稳定支撑个人基础系统的生产级底座。

> 本文是本轮完善工作的执行基线。`docs/audit/` 保留历史事实，
> `docs/PRODUCTION_READINESS_LOG.md` 记录已经完成并验证的修复；历史文档中的评分、
> 行号和开放项不能直接当作当前事实，实施每一点前都必须重新核对源码和测试。

## 1. 结论先行

本项目不应以“复制 `br-addon` / `br-db` 的每一个公开方法”为完成标准。基础库的价值不来自
API 数量，而来自以下端到端闭环是否同时成立：

1. 配置错误能在启动边界暴露，而不是在运行中降级或产生伪成功。
2. 查询、权限、事务和迁移能保持数据正确性，并且不会用兼容接口掩盖真实错误。
3. Action、路由、请求、Schema 和 API 文档来自同一份契约，不发生多处手写漂移。
4. 未使用的数据库和重量级能力可以不编译、不初始化、不进入供应链。
5. 错误链、request id、tracing 和 metrics 足以解释一次失败经过了哪些边界。
6. 每个公开能力都有自动化证据，发布门禁在全部支持组合上为绿色。

因此，本计划采用以下总体顺序：

```text
恢复可信门禁
  -> 修复公共边界正确性与错误语义
    -> 收敛数据库方言和 feature 契约
      -> 建立 RequestMeta / ApiCatalog / OpenAPI 集成链
        -> 按真实工作负载补高级 SQL
          -> 完善迁移、可选后台元数据和发布治理
```

严格对标时暂不追求 SQLite、MSSQL 和一体化后台 UI；它们只有在出现真实消费者和验收场景后
才进入核心路线。否则会增加依赖、测试矩阵和维护成本，却不能提高当前系统的可靠性。

## 2. 第一性原理与系统不变量

### 2.1 完整性不是功能计数

基础库的“完整”定义为：目标系统中的关键旅程不存在必须绕开公共 API、吞掉错误、复制元数据
或注入未受控 SQL 才能完成的步骤。以下纵向旅程必须闭环：

- 启动：读取配置 -> 校验 -> 建立连接 -> 按依赖迁移插件 -> 注册模块和 Action。
- 请求：传输层适配 -> Request/RequestMeta -> 中间件 -> 鉴权 -> Action -> TableQuery。
- 数据：TableQuery -> yang-db 查询/事务 -> MySQL/PostgreSQL/Redis -> 结构化错误。
- 契约：TypedAction + ModuleRouter + 路由映射 -> ApiCatalog -> OpenAPI/API 清单。
- 运维：request id -> tracing/metrics -> 健康检查 -> 逆依赖关闭与连接池排空。

### 2.2 公共边界必须 fail-fast / fail-closed

- 无效配置、无效标识符、空条件组和非法权限元数据必须返回真实错误。
- 鉴权或字段权限存在歧义时，默认拒绝，不允许静默扩大读取范围。
- 历史 infallible API 可以暂时保留兼容，但只能返回固定、不可执行的安全结果，并引导调用方迁移到
  `Result` API；不能把未经验证的输入重新拼成 SQL。

### 2.3 类型应表达调用方意图

当前多个缺陷都源于 `&str` 同时表示简单列名、限定列名和可信 SQL 表达式。目标类型模型应明确区分：

- `Identifier`：单段标识符，如 `status`。
- `QualifiedIdentifier`：受限段数的限定标识符，如 `users.status`。
- `TrustedSqlExpr`：调用方明确声明的可信表达式，如 `COUNT(*) AS total`。
- `SqlValue`：只能进入绑定参数，不能被当作 SQL 片段。

安全 API 接收前两类；表达式逃生口必须在命名、文档和调用点上显式可见。

### 2.4 层次边界不能因对标而倒置

- `ActionMeta` 描述业务 Action，不应直接承载所有 HTTP 或后台 UI 细节。
- HTTP method/path/content type 属于路由适配层，应由 `RouteDescriptor` 描述。
- `ApiCatalog` 合并 Action、Module 和 Route 契约；OpenAPI 只是它的一个投影。
- 菜单、按钮、图标、页面布局属于可选后台展示层，不应成为核心 Action 执行的依赖。
- 请求耗时属于 tracing/metrics 观测结果，不应像普通输入一样写回 `Request`。

### 2.5 可选能力必须真正可选

一个 feature 只有同时满足以下条件才算成立：

1. 关闭后相关依赖不会进入依赖图。
2. 关闭后 crate 仍能独立编译和生成文档。
3. 合法 feature 组合有自动化检查。
4. 运行时不会引用被关闭能力的全局单例或配置字段。

## 3. 当前系统地图

```text
传输层适配器
  -> action::Request（body / headers / query / path_params）
  -> ActionContext
  -> AppRouter
     -> ModuleRouter
        -> Middleware
        -> DynAction / TypedAction
           -> TableQuery（字段权限、查询约束、软删除）
              -> yang-db QueryBuilder / Transaction
                 -> MySQL / PostgreSQL / Redis

注册侧：
Plugin -> PluginManagerBuilder / PluginRegistry -> DatabaseInitializer -> migration SQL
TypedAction -> ActionMeta(input_schema/output_schema) -> ModuleRouter

当前断点：
ActionMeta + ModuleRouter + 实际 HTTP 路由 -X-> ApiCatalog / OpenAPI
传输层 method/URI/peer metadata       -X-> RequestMeta
TableConfig                           -X-> 可校验的 schema/migration contract
```

已有优势必须保留：类型化 Action、JSON Schema、插件依赖拓扑、字段级权限、软删除保护、JWT 撤销、
HTTP retry/circuit breaker、结构化错误码、tracing/metrics、MySQL/PostgreSQL/Redis 常用能力。

## 4. 2026-07-15 当前门禁基线

以下结果由本轮在当前工作树直接运行得到：

| 命令 | 结果 | 结论 |
|---|---:|---|
| `cargo test --lib -p yang-db --quiet` | 354 passed / 2 failed / 1 ignored | MySQL 复杂 JOIN 查询被限定列名校验回归阻断 |
| `cargo test --lib -p yang-base --quiet` | 457 passed / 2 failed / 8 ignored | OR 渲染测试未显式选择可读字段，与新的 fail-closed `SELECT *` 契约冲突 |
| `cargo clippy -p yang-db -p yang-base --all-targets --all-features -- -D warnings` | failed | 12 个测试 `expect_err()`；2 个 `items_after_test_module` |

### 4.1 已确认的真实回归

提交 `b36702c` 为 WHERE/HAVING 条件新增标识符校验，但调用了只允许单段名称的
`quote_identifier()`。因此 `users.status`、`orders.total` 等合法限定列名被拒绝：

- `crates/yang-db/src/mysql/query_builder.rs:110`
- `crates/yang-db/src/postgres/query_builder.rs:125`
- `crates/yang-db/src/mysql/identifier.rs:50`
- `crates/yang-db/src/postgres/identifier.rs:32`

这不是测试陈旧，而是安全加固把“简单标识符”错误地当成了“所有条件字段”的语法。

### 4.2 已确认的契约/测试漂移

提交 `3b5def9` 把字段读取权限下沉到 SQL 生成层。默认 `SELECT *` 现在要求当前角色能读取表中
所有字段，这是正确的 fail-closed 行为。但两个 OR 语义测试使用普通用户和包含受限字段的公共 fixture，
又没有显式选择字段，于是测试在到达 OR 渲染断言前被权限拒绝：

- `crates/yang-base/src/table/table_query.rs` 的 `validate_all_fields_readable()`
- `crates/yang-base/src/table/__tests__/table_query_test.rs:1106`
- `crates/yang-base/src/table/__tests__/table_query_test.rs:1137`

这里不能通过取消底层权限校验来“修绿”。低层 `SELECT *` 应继续 fail-closed；测试必须显式构造满足
测试意图的字段投影或角色。如果产品层希望“默认返回所有可读字段”，应新增命名清楚的显式 API，
而不是悄悄改变 `SELECT *` 的含义。

### 4.3 历史审计中的已漂移信息

实施计划不能直接复制 2026-06-27 审计。例如 PostgreSQL `Transaction` 当前已经存在 `Drop`，
`ActionContext.user` 也已经收紧为私有字段；这些不再是开放项。相反，以下事实仍存在：

- `yang-db` 没有 crate feature，MySQL、PostgreSQL、Redis 依赖总是一起进入依赖图。
- `GlobalRedis` 的连接和 42 个操作方法仍将 `DbError` 转成字符串，丢失 source chain。
- 插件初始化和迁移路径也有将底层错误转成字符串的边界。
- `ActionMeta` 已有输入/输出 Schema，但没有 route/API catalog/OpenAPI 投影。
- `action::Request` 只有 body、headers、query、path params，传输层上下文在适配边界丢失。
- `union`、`exists/subquery`、行锁和原子增减等能力没有公开、受控的 QueryBuilder API。

## 5. 缺陷根因与目标状态

| ID | 表面缺陷 | 第一性根因 | 目标状态 |
|---|---|---|---|
| D-01 | 限定列名导致复杂 JOIN 失败 | 一个 `String` 承担三种 SQL 语义，校验器只理解单段标识符 | 条件字段使用统一的严格限定标识符解析和方言渲染；恶意载荷仍被拒绝 |
| D-02 | OR 测试被字段权限阻断 | 安全契约升级后，测试 fixture 没有表达自己的最小权限前提 | 保留低层 fail-closed；测试显式投影；可选增加 `select_readable_fields` 产品层 API |
| D-03 | 测试与 Clippy 门禁为红 | 修复点只验证窄测试，没有完整回归门和提交前自动门禁 | 每一点窄 RED/GREEN，阶段结束跑完整 test/clippy/doc/feature matrix |
| D-04 | legacy 条件渲染可能 RAW 回退 | infallible 兼容签名无法表达校验错误，渲染和校验又是两条路径 | checked renderer 成为唯一内部实现；兼容 API 只做安全委托并逐步废弃 |
| D-05 | Redis/插件错误链被字符串化 | 上层错误变体不能统一持有底层 source，调用点就地拼接文本 | 基础设施错误变体保留 `#[source]`；展示文本与机器可诊断原因分离 |
| D-06 | yang-db 无 feature gate | workspace `sqlx` 一次启用两个后端，yang-base 又默认拉入完整 yang-db | MySQL/PostgreSQL/Redis 可独立编译；默认组合仅用于兼容 |
| D-07 | Action Schema 无法形成 API 文档 | Action 业务元数据与 route 传输元数据没有汇合模型 | `ApiCatalog = Module + Action + Route`，OpenAPI 从 catalog 确定性生成 |
| D-08 | Request 缺 method/URI/peer 等信息 | 传输适配层只复制了 payload，没有稳定的 transport metadata 契约 | 新增可选 `RequestMeta`；敏感字段有 Debug 脱敏；耗时仍由观测层负责 |
| D-09 | 高级 SQL 能力缺口 | QueryBuilder 以方法堆叠增长，MySQL/PG 同构代码让每项能力成本翻倍 | 先提取共用查询语义和 Dialect seam，再补实际需要的受控原语 |
| D-10 | TableConfig 与迁移无闭环 | 运行期表元数据和建表/迁移 SQL 是两份独立真相 | 先做迁移 checksum/漂移检测和 dry-run；自动 schema diff 另行 RFC |
| D-11 | 后台 UI 元数据缺失 | 对标库把执行、HTTP 和展示元数据放在同一 trait | 独立可选 admin metadata 投影，不污染核心 Action/Table 契约 |
| D-12 | 审计、BACKLOG、代码状态漂移 | 缺少一个可更新的主计划和机器门禁 | 本文跟踪工作包；完成点写修复日志；历史审计保持只读 |

## 6. 目标架构决策

### ADR-01：条件字段只接受标识符，不接受任意表达式

- `where_*`、`having_*` 接受 `Identifier` / `QualifiedIdentifier`。
- 聚合、函数和自定义片段走明确命名的 `*_expr(TrustedSqlExpr)` 逃生口。
- MySQL 和 PostgreSQL 共享语法校验，只由 Dialect 决定反引号/双引号和占位符。
- QueryBuilder 内部只调用返回 `Result` 的 checked renderer。
- 0.1.x 保留兼容方法并 deprecated；破坏性删除放到 0.2.0。

### ADR-02：`SELECT *` 保持 fail-closed

- 低层 `TableQuery` 的 `SELECT *` 只有在全部字段均可读时才成功。
- 显式字段请求中出现无权限字段必须报错，不能静默丢弃。
- 如内置列表 Action 需要便利行为，新增显式的“投影全部可读字段”路径；字段顺序必须确定，
  不得依赖 `HashMap` 迭代顺序。
- 权限错误选择哪个字段必须可预测，便于测试和日志聚合。

### ADR-03：Action 与 HTTP Route 分层

- `ActionMeta` 继续保存 name/display/description/permission/schema。
- `RouteDescriptor` 保存 method/path/content types/success status/tags。
- `ModuleDescriptor` 保存模块显示信息和默认安全策略。
- `ApiCatalog` 在注册完成后构建不可变快照，并校验重复 operation/path、缺 route、Schema 冲突。
- `openapi` feature 只负责把 Catalog 投影成 OpenAPI，不参与运行时 dispatch。

### ADR-04：请求元数据和观测数据分离

`RequestMeta` 最小集合：method、original URI、scheme、peer/local address、可选 extensions。
User-Agent 和 Cookie 优先从规范化 headers 提供只读助手，避免重复存储。请求接收时间可以保存在
内部 context 中；handle time 只能由 tracing/metrics 在完成后计算。

### ADR-05：错误变体保留 source

- 连接、查询、事务、Redis、迁移、插件回调等基础设施边界使用带 `#[source]` 的结构化错误。
- 只有用户输入校验、外部协议兼容或真正没有可持有错误类型时才使用 `String`。
- 错误码保持稳定；新增 source 变体前先定义兼容映射和 SemVer 影响。

### ADR-06：能力对齐，不追求方言伪一致

MySQL/PostgreSQL 对同一语义提供一致入口，但允许返回 `UnsupportedCapability` 或以 capability trait
声明差异。不能为了 API 对称而模拟数据库不具备的事务或锁语义。

## 7. 分阶段实施计划

状态取值：`PENDING`、`IN_PROGRESS`、`DONE`、`DEFERRED`。每个工作包完成后更新本节状态，
并在 `docs/PRODUCTION_READINESS_LOG.md` 追加真实验证命令。

### Phase 0：恢复可信基线

#### P0-01 修复限定条件字段回归 — `DONE` — effort S

**范围**

- MySQL/PostgreSQL `validate_condition_field`、condition checked renderer、相关测试。

**RED 证据**

- 当前两个 MySQL 复杂查询测试稳定失败。
- 新增 PostgreSQL 对称测试，确保不是只修 MySQL 表象。

**实现约束**

1. 合法 `field` 和 `table.field` 都能通过。
2. 空段、三段名称、分号、注释、引号、函数表达式仍被拒绝。
3. 校验和渲染必须共用同一个解析结果，不能“先宽松校验、后 RAW 渲染”。
4. WHERE 与 HAVING、MySQL 与 PostgreSQL 测试矩阵对称。

**完成定义**

- 两个现有失败测试转绿。
- 新增两方言合法/恶意限定名测试。
- `try_to_sql()` 返回真实错误；`to_sql()` 仍只返回不可执行哨兵。

#### P0-02 固化字段权限投影契约 — `DONE` — effort S/M

**范围**

- `TableQuery::build_select_sql`、OR 语义测试、内置 SelectAction 的默认投影行为。

**实施步骤**

1. 给两个 OR 测试显式选择只与 OR 断言有关的可读字段，或使用全字段可读 fixture。
2. 保留并扩充 `SELECT *` 无权字段 fail-closed 回归测试。
3. 让无权字段错误确定性选择字段，消除 HashMap 顺序导致的错误文本漂移。
4. 调查内置 SelectAction 的产品语义；如需要默认可用，新增显式 `select_readable_fields()`，
   并测试“显式请求受限字段仍报错”。

**完成定义**

- OR 测试真正断言 OR SQL，而不是被无关权限前置条件阻断。
- 权限测试同时覆盖全部可读、部分不可读、零可读、显式越权四种情况。
- 不削弱底层字段权限。

#### P0-03 清理 all-target Clippy 门禁 — `DONE` — effort S

**范围**

- 12 个 `expect_err()` lint。
- `http/request.rs`、`table/dynamic_row.rs` 的 test module 布局。

**原则**

- 测试代码遵循与门禁相符的 lint 契约；不通过 crate 级大范围 `allow` 隐藏问题。
- 如果项目决定测试允许 `expect`，应在 workspace 中明确配置 test target 策略，而不是逐点摇摆。

**完成定义**

- `cargo clippy -p yang-db -p yang-base --all-targets --all-features -- -D warnings` 通过。

#### P0-04 建立持续门禁 — `DONE` — effort M

**内容**

- 添加 CI：fmt、lib tests、all-target clippy、doc tests、feature matrix。
- Docker 依赖测试单独 job，明确 MySQL 8/PostgreSQL/Redis 版本和串行要求。
- 固定 MSRV 1.80 检查，并增加当前 stable 检查。
- 提交前只跑窄验证；合并前必须跑阶段完整门禁。

**Phase 0 退出条件 G0**

- `yang-base` / `yang-db` lib tests 零失败。
- all-target/all-feature Clippy 零 warning。
- CI 能复现本机命令，且不依赖未跟踪文件。

### Phase 1：公共边界正确性和诊断能力

#### P1-01 收敛 SQL 标识符与 checked renderer — `DONE` — effort L

1. 引入 `Identifier`、`QualifiedIdentifier`、`TrustedSqlExpr` 内部类型。
2. MySQL/PostgreSQL 条件渲染统一为 `Result<RenderedCondition, DbError>`。
3. QueryBuilder 移除“单独校验一次、legacy renderer 再渲染一次”的双轨。
4. deprecated infallible 条件函数只委托 checked 实现，失败时返回固定 fail-closed 片段；不得 RAW 回退。
5. 为 field/order/group/join ON 明确“标识符 API”和“可信表达式 API”。
6. 属性测试覆盖注释、引号、Unicode 边界、空段、限定段数和占位符顺序。

#### P1-02 保留基础设施错误链 — `DONE` — effort M

1. 为 Redis 连接与操作统一使用持有 `yang_db::DbError` 的 BaseError 变体。
2. 迁移执行错误保留 module/version/checksum 和底层 source。
3. 插件生命周期错误保留插件名、阶段和 source；避免 `Box<dyn Error> -> String` 提前扁平化。
4. 增加 `Error::source()` 链测试和错误码稳定性测试。
5. 日志展示仍可格式化，但不能替代结构化错误值。

#### P1-03 建立 yang-db feature 矩阵 — `DONE` — effort L

**目标 feature**

```toml
default = ["mysql", "postgres", "redis"]
mysql = ["dep:sqlx", "sqlx/mysql"]
postgres = ["dep:sqlx", "sqlx/postgres"]
redis = ["dep:deadpool-redis", "dep:redis"]
```

实际实现需先调整 workspace `sqlx` feature，避免根依赖提前把两个后端全部打开。`yang-base` 应以
`default-features = false` 依赖 `yang-db`，再由自己的 `mysql` / `redis` feature 精确转发。

**必须验证的组合**

- yang-db：none、mysql、postgres、redis、all。
- yang-base：none、token、http、mysql、redis、默认 all。
- docs.rs 所需组合和 README 示例组合。

#### P1-04 定义后端能力契约 — `DONE` — effort M

- 建立 MySQL/PostgreSQL QueryBuilder/Transaction capability 表。
- 同义操作统一名称、错误和安全约束。
- 方言特有能力显式标注，不用条件编译让同一方法悄悄改变语义。
- Redis 的 health/close/status 与 SQL 后端在管理面统一返回结构，但不强行伪装成关系数据库。

**Phase 1 退出条件 G1**

- 所有内部 SQL 生成路径都经过 checked renderer。
- 基础设施错误的 `source()` 可追到驱动错误。
- 每个声明支持的 feature 组合可独立 check/test/doc。

### Phase 2：请求契约、API Catalog 与 OpenAPI

#### P2-01 引入 RequestMeta — `DONE` — effort M

- 新增 transport-neutral `RequestMeta`，不直接依赖具体 Web 框架类型。
- 提供兼容构造器，现有 `Request::new(body)` 行为不变。
- 新字段通过 builder/adapter 注入；对地址、URI、headers 的 Debug 输出执行脱敏。
- 为 method/URI/peer address 缺失和存在两种路径写测试。
- 不加入 handle_time；由 dispatch span/metrics 记录。

#### P2-02 建立 ApiCatalog — `DONE` — effort L

- 为 AppRouter/ModuleRouter 提供只读 descriptor 枚举，不暴露可变注册表。
- 新增 `RouteDescriptor` 和注册期冲突校验。
- 合并 `ActionMeta` 的 input/output Schema、权限和公开性。
- Catalog 构建必须确定性排序，便于 snapshot 和缓存。
- Catalog 生成不依赖 `openapi` feature，可供 CLI、后台管理和测试复用。

#### P2-03 OpenAPI 投影 — `DONE` — effort L

- 可选 `openapi` feature，从 ApiCatalog 生成 OpenAPI 3.x 文档。
- 映射 request/response schema、bearer security、public/private、tags、operation id、错误响应。
- 不在 ActionMeta 重复存 method/path；以 RouteDescriptor 为唯一来源。
- 使用 snapshot 测试确保模块、Action、权限或 Schema 变化会显式更新契约。

#### P2-04 纵向契约测试 — `DONE` — effort M

构造一个最小示例插件，验证：注册 -> 路由 -> 鉴权 -> 类型化输入 -> TableQuery -> ApiResponse ->
ApiCatalog/OpenAPI。该测试不需要真实数据库；数据库执行另由集成测试覆盖。

**Phase 2 退出条件 G2**

- 同一个 Action 的运行时 Schema 和 OpenAPI Schema 来自同一对象。
- 重复 route/operation、缺失 route、公开性冲突在注册期失败。
- RequestMeta 可由至少一个真实传输适配器完整构造。

### Phase 3：按工作负载补齐 yang-db 高级能力

这一阶段禁止按 `br-db` 方法清单机械复制。每个能力必须先提供至少一个本项目消费者或最小业务用例。

#### P3-01 子查询与 EXISTS — `DONE` — effort L

- `where_exists` / `where_not_exists`。
- `where_in_subquery`。
- 子查询使用 QuerySpec/受控 builder，不接收外部裸 SQL。
- 验证两方言占位符编号和参数顺序。

#### P3-02 UNION / UNION ALL — `DONE` — effort L

- 校验列数量/输出契约，组合查询各自持有参数。
- limit/order 的作用域必须明确并覆盖方言差异测试。

#### P3-03 行锁与事务查询能力 — `DONE` — effort M/L

- `for_update`、共享锁或方言能力枚举。
- 锁 API 只能在事务/支持的查询类型中使用。
- 扩展 TransactionQueryBuilder 前先定义与普通 QueryBuilder 的最小复用边界，避免第三套 SQL 生成器。
- 使用真实 MySQL/PostgreSQL 并发测试验证阻塞、回滚和取消行为。

#### P3-04 原子字段更新 — `DONE` — effort M

- `increment` / `decrement` / 受控 column expression。
- 字段必须是受验证标识符，增量必须走绑定参数。
- 覆盖溢出、负数、无 WHERE 全表保护和事务内调用。

#### P3-05 明确不支持项 — `DONE` — effort S

- 在 README/API 文档列出当前支持 MySQL/PostgreSQL/Redis。
- SQLite/MSSQL 保持 non-goal；出现真实消费者时单独 RFC，评估驱动、类型映射、DDL、事务和 CI 成本。
- backup/database-create 等运维能力不进入 QueryBuilder；优先交给数据库原生工具。

**Phase 3 退出条件 G3**

- 每个新增高级能力都有两方言单测和至少一个真实数据库集成测试。
- 不新增默认接受外部裸 SQL 的便利入口。
- QueryBuilder 与 TransactionQueryBuilder 不出现新的语义分叉。

### Phase 4：迁移治理与可选系统元数据

#### P4-01 迁移可验证性 — `DONE` — effort L

- 迁移记录增加 checksum；同 module/version 内容变化时启动失败。
- 明确一次 migration 的事务边界和不支持事务 DDL 的处理策略。
- 提供 `plan` / `dry-run`，输出待执行版本但不修改数据库。
- 并发启动时用数据库约束/锁防止同一迁移重复执行。
- 初始化 SQL 的分号切割旧 API deprecated；复杂脚本以逐 migration 语句或专用执行器处理。

#### P4-02 TableConfig 与 Schema 的关系 — `PENDING` — effort M

- 明确 TableConfig 是运行期访问/权限契约，不自动声称是数据库 DDL 的唯一真相。
- 增加“TableConfig 字段是否能由当前迁移提供”的可选验证接口。
- 自动 schema diff、自动 ALTER 和回滚生成不在本阶段实现，需独立 RFC 和灾难恢复设计。

#### P4-03 可选后台元数据 — `PENDING` — effort L

- 在独立模块或 crate 定义 menu/icon/group/button/list/tree/form 等展示描述。
- 通过 Action/Table/ApiCatalog 稳定 ID 建立引用，不改变核心 dispatch。
- 未启用时零运行时和依赖成本。
- 审核流属于业务插件能力，不作为所有 Action 的基础字段。

**Phase 4 退出条件 G4**

- 修改已执行迁移会被 checksum 检测。
- 并发初始化不会重复应用迁移。
- 核心 crate 在关闭 admin metadata 后依赖图不包含展示层依赖。

### Phase 5：发布和文档收口

#### P5-01 版本与兼容策略 — `PENDING` — effort M

- 0.1.x：恢复门禁、增加兼容 API、deprecated 不安全/含糊入口。
- 0.2.0：删除 RAW fallback，启用明确的 identifier/expr 类型边界，完成 feature 拆分。
- 每个 breaking change 提供迁移示例和替代 API。

#### P5-02 文档统一 — `PENDING` — effort M

- README、`docs/yang-base.md`、`docs/yang-db.md`、示例和 feature 表与代码同步。
- `docs/BACKLOG.md` 中已完成或已失效条目标记来源和日期，不重写历史审计。
- 增加“支持能力矩阵”和“与 br-addon/br-db 的设计差异”，说明哪些缺口是明确 non-goal。

#### P5-03 发布候选验证 — `PENDING` — effort M

- clean checkout 全门禁。
- MSRV/stable、feature matrix、doc tests、真实数据库集成、依赖审计。
- `cargo package` 检查实际打包内容，确保示例、许可证、README 和必要测试资产存在。
- 生成发布前 capability report，与本文完成定义逐项对账。

## 8. 工作包执行协议

每个工作包按以下固定节奏执行，避免再次出现“窄测试通过但全库门禁回归”：

1. 检查工作树，识别并隔离已有用户修改。
2. 用 CodeGraph 定位公共边界、调用方和影响范围。
3. 写一个从真实公开入口触发的 RED 测试，并记录失败原因。
4. 只实现关闭该失败模式所需的最小改动。
5. 运行窄 GREEN 测试和一个邻近回归切片。
6. 到达阶段边界时运行完整验证矩阵。
7. 更新本文状态和 `docs/PRODUCTION_READINESS_LOG.md`，记录实际命令与结果。
8. 每个完成点单独创建本地提交，不混入无关格式化或其他工作树改动。

如果一个工作包连续三种修补方案都不能关闭同一失败模式，应停止打补丁，回到 ADR/架构层重新评估。

## 9. 验证矩阵

### 每个工作包

```powershell
cargo test -p <crate> --lib <narrow_test_name>
cargo test -p <crate> --lib <nearby_regression_prefix>
```

### 每个 Phase 退出门

```powershell
cargo fmt --all -- --check
cargo test --lib -p yang-db
cargo test --lib -p yang-base
cargo clippy -p yang-db -p yang-base --all-targets --all-features -- -D warnings
cargo test --doc -p yang-db
cargo test --doc -p yang-base
```

### feature 拆分完成后

```powershell
cargo check -p yang-db --no-default-features
cargo check -p yang-db --no-default-features --features mysql
cargo check -p yang-db --no-default-features --features postgres
cargo check -p yang-db --no-default-features --features redis
cargo check -p yang-db --all-features

cargo check -p yang-base --no-default-features
cargo check -p yang-base --no-default-features --features token
cargo check -p yang-base --no-default-features --features http
cargo check -p yang-base --no-default-features --features mysql
cargo check -p yang-base --all-features
```

### 真实基础设施验证

- MySQL 8：CRUD、JOIN 限定字段、事务隔离、行锁、迁移并发。
- PostgreSQL：CRUD、占位符顺序、RETURNING、事务隔离、行锁、迁移并发。
- Redis：pool config、pipeline、WATCH/MULTI/EXEC、script、错误 source、关闭流程。
- 所有 Docker/外部依赖测试单线程执行，并记录服务版本。

## 10. 优先级总表

| 顺序 | 工作包 | 性质 | 阻塞关系 |
|---:|---|---|---|
| 1 | P0-01 限定字段回归 | 正确性 | 阻塞所有发布判断 |
| 2 | P0-02 字段权限契约 | 安全/测试真实性 | 阻塞 yang-base 全量测试 |
| 3 | P0-03 Clippy | 工程门禁 | 阻塞 CI |
| 4 | P0-04 CI | 防回归 | 阻塞后续并行扩展 |
| 5 | P1-01 checked renderer | 安全/架构 | 阻塞高级 SQL |
| 6 | P1-02 错误链 | 可诊断性 | 阻塞生产级故障分析 |
| 7 | P1-03/P1-04 feature 与 capability | 依赖/契约 | 阻塞可靠发布矩阵 |
| 8 | P2 RequestMeta/ApiCatalog/OpenAPI | 系统集成 | 补齐 yang-base 最大对标缺口 |
| 9 | P3 高级 SQL | 业务能力 | 依赖统一 SQL 语义 |
| 10 | P4 迁移/admin metadata | 运维/可选产品层 | 依赖前述稳定契约 |
| 11 | P5 发布收口 | 发布治理 | 依赖 G0-G4 全部通过 |

## 11. 明确不做或延后

- 不做 `br-addon` / `br-db` 的 drop-in API 兼容层。
- 不因为对标库存在就立即增加 SQLite/MSSQL。
- 不把任意 SQL 字符串包装成“高级查询支持”。
- 不让 OpenAPI 类型侵入 Action 运行时核心。
- 不把菜单、按钮、页面布局放进所有 Action 的必需元数据。
- 不在没有备份、漂移检测和回滚设计前实现自动 ALTER/schema diff。
- 不用删除测试、放宽权限或全局 `allow` 把门禁变绿。

## 12. 最终完成定义

只有同时满足以下条件，才可把 `yang-base` / `yang-db` 标记为本目标下的“完善”：

1. G0-G4 全部通过，clean checkout 可复现。
2. 支持的每个 feature 组合都能编译，公开文档与 feature 行为一致。
3. MySQL/PostgreSQL/Redis 的声明能力都有真实基础设施验证。
4. SQL 标识符、值和可信表达式在类型/API 层明确分离。
5. 字段权限和认证默认 fail-closed，便利 API 不改变安全含义。
6. 基础设施错误链能够跨 yang-db -> yang-base 保留到调用方。
7. Action 运行时契约、ApiCatalog 和 OpenAPI 不重复手写 Schema。
8. 插件迁移有 checksum、并发保护、可预览计划和确定性失败行为。
9. 发布包、README、API 文档、示例和支持矩阵同步。
10. 所有明确不支持的能力都有 documented non-goal，而不是处于含糊的“可能支持”状态。

达到这些条件后，项目即使仍不支持 SQLite、MSSQL 或一体化后台 UI，也已经是一个边界清楚、
行为可靠、可诊断、可演进的基础系统库；这比表面复制对标项目的全部方法更接近真正的功能完善。
