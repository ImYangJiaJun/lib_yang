# 生产级基础库修复记录

本文档记录基础库生产级审计中已经完成的修复点。每个完成点对应一次本地 git 提交；未完成的 RED 测试、探索结论或临时状态不作为完成点记录。

## 2026-07-15 - P4-02 TableConfig 与数据库 Schema

- 范围：新增 `SchemaColumn`、`SchemaIssue`/`SchemaIssueKind`、`SchemaValidationReport`、`TableConfig::validate_schema`，以及 MySQL `DatabaseInitializer::validate_table_config` 只读 introspection 入口。
- RED：两项纯契约测试产生 3 个缺类型/方法错误；实现后覆盖缺列、VARCHAR 容量不足、必填列允许 NULL、宽类型兼容与数据库额外列忽略。
- 关系边界：TableConfig 明确定义为运行期访问/校验/权限契约，不是 DDL 唯一真相；验证只检查声明字段能否由当前 schema 承载，不比较额外列、索引、默认值、触发器或存储选项，不生成 ALTER/回滚。
- 对抗性验证：真实 MySQL 首轮暴露 information_schema 大写列标签，第二轮暴露元数据 BLOB 解码；通过显式稳定别名与 CAST 修复后，真实 schema 精确报告 3 个问题，且验证前后列数保持 3，证明零写入（1 passed）。
- 文档：新增 `docs/TABLE_CONFIG_SCHEMA.md`，明确 ForeignKey 的可验证边界及自动 diff/ALTER 需独立 RFC 和灾难恢复设计。
- 门禁：yang-base lib 481 passed/8 ignored，doctest 74 passed/148 ignored，all-target/all-feature Clippy `-D warnings` 通过。

## 2026-07-15 - P4-01 迁移可验证性

- 范围：`DatabaseInitializer` 迁移表新增 checksum/status，公开 `MigrationPlan`/`MigrationPlanEntry`/`MigrationPlanStatus` 与只读 `plan_migrations`/`plan_all`，新增 checksum mismatch/in-progress 结构化错误和 `docs/MIGRATIONS.md`。
- RED：迁移记录分类测试产生 10 个缺函数/类型错误；实现后覆盖 Pending、Applied、ChecksumMismatch（含旧记录 NULL checksum）和 InProgress。
- dry-run/checksum：迁移表不存在时计划仍返回 Pending 且零写入；相同 module/version 内容变化在执行前返回 `MigrationChecksumMismatch`。旧版无 checksum 记录 fail-closed，需人工核对补录。
- 并发/恢复：执行前用 `(module_name, version)` 唯一键写 `running` 预留，成功后精确更新一行为 `applied`；竞争者只会看到 Applied 或 `MigrationInProgress`，失败尽力清理自身预留。真实 MySQL 8 双初始化器并发只执行一次业务 INSERT。
- 事务边界：文档明确 MySQL DML 可与记录共用事务，而 DDL 会隐式提交；running 状态显式暴露恢复边界。每个 migration 是一个独立语句，不做分号切割；MySQL/PostgreSQL `Database::init` 旧入口已 deprecated。
- 对抗性验证：真实 MySQL dry-run 零写、应用后计划、内容漂移拒绝、并发唯一执行共 1 项集成测试通过；yang-base lib 479 passed/8 ignored，doctest 74 passed/148 ignored，两库 all-target/all-feature Clippy 通过。

## 2026-07-15 - P3-05 支持矩阵与 non-goal

- 范围：重写 `crates/yang-db/README.md` 的开发状态，加入 MySQL 8/PostgreSQL 16/Redis 7 支持矩阵，并明确 SQLite、MSSQL 与数据库运维职责边界。
- RED：可执行文档契约 2 项均失败，分别确认 README 缺少支持/non-goal 边界且仍把查询、CRUD、事务、JOIN、聚合列为待实现。
- 决策：SQLite/MSSQL 仅在出现真实消费者后通过独立 RFC 评估驱动、类型映射、DDL、事务与 CI；`backup`/`restore`/`database-create` 不进入 QueryBuilder，优先使用数据库原生工具并由运维层治理。
- 对抗性验证：文档测试逐项锁定三种支持后端、两个 non-goal、RFC 成本维度、运维关键词与“不进入 QueryBuilder”约束，并阻止过期“待实现功能”清单回归；2 passed，all-target/all-feature Clippy `-D warnings` 通过。

## 2026-07-15 - P3-04 原子字段更新

- 范围：MySQL/PostgreSQL 普通 QueryBuilder 与 TransactionQueryBuilder 对称增加 `increment`/`decrement`；两条执行路径共享同一个 `SqlGenerator::build_arithmetic_update`。
- RED：双方言公共边界测试产生 4 个缺方法错误，确认原子更新能力不存在；实现后 4 项定向测试覆盖缺 WHERE、恶意字段、负增量及参数顺序。
- 安全语义：更新字段只接受单段标识符，表名和 WHERE 继续走 checked renderer；增量固定为 `i64` 并绑定，MySQL 为 `?` 后接 WHERE 参数，PostgreSQL 精确为增量 `$1`、条件 `$2`；普通与事务路径均 fail-closed 禁止无 WHERE 全表更新。
- 真实数据库：MySQL 8/PostgreSQL 16 均验证 `increment(-3)`、`decrement(2)`、事务内 `increment(5)` 并提交，最终值回到 10；对 `BIGINT::MAX + 1` 均返回数据库错误而非静默环绕（双方言各 1 passed）。
- 门禁：`yang-db` lib 397 passed/1 ignored，doctest 65 passed，all-target/all-feature Clippy `-D warnings` 通过。

## 2026-07-15 - P3-03 行锁与事务查询

- 范围：新增跨后端 `RowLock::{ForUpdate, ForShare}`；MySQL/PostgreSQL Transaction 对称提供 `select`、`select_locked`、`select_for_update`、`select_for_share`，普通 QueryBuilder 不公开锁入口。
- 复用边界：QueryBuilder 唯一负责 checked SELECT 与 typed params 渲染，Transaction 唯一负责在底层事务连接执行及附加受控锁子句；未给 TransactionQueryBuilder 增加第三套 SELECT 生成逻辑。
- RED：双方言锁渲染/拒绝测试产生 14 个类型或方法缺失错误；实现后 4 项定向测试精确覆盖 MySQL `?`、PostgreSQL `$1`、LIMIT 后锁位置与不支持形态。
- 对抗性约束：锁查询拒绝 DISTINCT/GROUP BY/HAVING/UNION，避免依赖方言差异或数据库运行时才报错；参数继续由原 typed binder 绑定，不开放锁 SQL 字符串。
- 真实并发：MySQL 8/PostgreSQL 16 中持锁事务读取余额后，竞争更新均在 250ms 超时并被取消；等待事务随后可回滚，持锁事务回滚释放锁，新事务更新/提交成功（双方言各 1 passed），覆盖阻塞、取消、回滚语义。
- 门禁：`yang-db` lib 393 passed/1 ignored，doctest 65 passed，all-target/all-feature Clippy `-D warnings` 通过。

## 2026-07-15 - P3-02 UNION / UNION ALL

- 范围：MySQL/PostgreSQL QueryBuilder 对称增加 `union`/`union_all`，复用原 `SqlGenerator` 递归渲染复合 SELECT，不引入 raw SQL 或第三套生成器。
- RED：双方言 4 项新测试产生 8 个缺方法编译错误，确认公共能力不存在；覆盖 `UNION ALL`、参数顺序、分支/根 ORDER-LIMIT 作用域及拒绝路径。
- 输出契约：所有 UNION 分支必须显式声明投影，拒绝列数未知的 `*`；左右输出列数不一致立即返回 `InvalidArgument`。分支 ORDER/LIMIT 位于分支括号内，根 ORDER/LIMIT 在全部 UNION 后生效。
- 对抗性验证：恶意分支表名在 checked renderer 中失败且不生成绑定参数；双方言精确 SQL/参数测试通过，PostgreSQL 跨分支占位符连续为 `$1/$2`。`yang-db` lib 389 passed/1 ignored，doctest 65 passed，all-target/all-feature Clippy 通过。
- 真实数据库：MySQL 8/PostgreSQL 16 分别执行带分支 DESC LIMIT 与根 ASC ORDER 的 UNION ALL，结果均为 `[(1), (4)]`（各 1 passed），证明两种方言作用域语法和绑定参数可执行。

## 2026-07-15 - P3-01 受控子查询与 EXISTS

- 范围：MySQL/PostgreSQL 对称提供 `Subquery`、`where_exists`、`where_not_exists`、`where_in_subquery`，支持绑定值条件与受控列对列关联，不接收外部裸 SQL。
- RED：双方言 QueryBuilder 测试因 `Subquery` 与三个查询入口缺失产生 6 个编译错误，确认公共能力不存在；实现后的首轮精确 SQL 断言又暴露多条件根节点会由既有 renderer 加括号，按真实契约修正快照。
- 安全边界：子查询表名只接受单段标识符，投影/条件/关联列只接受一至两段标识符；比较符收敛为固定枚举。空值、注释、分号、三段名、函数、NUL 和操作符注入均在 SQL 渲染前返回结构化错误。
- 对抗性验证：4 项双方言单测覆盖 EXISTS/NOT EXISTS/IN、关联列、外层/嵌套参数顺序和恶意结构；PostgreSQL 精确锁定 `$1` 至 `$4` 连续编号。`yang-db` lib 385 passed/1 ignored，doctest 65 passed，all-target/all-feature Clippy 通过。
- 真实数据库：临时 MySQL 8 与 PostgreSQL 16 容器分别执行关联 EXISTS 与绑定条件，均只返回预期用户（各 1 passed）；集成契约保存在 `integration_advanced_queries.rs`，默认离线套件标记 ignored。

## 2026-07-15 - P2-04 Phase 2 纵向契约测试

- 范围：最小 DirectoryPlugin 与真实 TypedAction 的 crate 内纵向契约测试，不修改生产 dispatch；覆盖完整 RequestMeta 适配输入，且不连接数据库。
- 正向旅程：PluginManagerBuilder 注册/构建 -> ModuleRouter Action/route/认证中间件注册 -> AppRouter dispatch -> 权限校验 -> typed body 提取 -> `ActionContext::table_query()` 无连接构造 -> ApiResponse -> ApiCatalog -> OpenAPI。
- 对抗性旅程：重复插件注册返回 `PluginAlreadyRegistered`；移除认证中间件后私有 Action 返回 `Unauthorized`；把 `query` 从 String 改为 number 后在 handler 前返回 `ParamInvalid("body", ..)`。同时精确比较 OpenAPI request schema 与 catalog 中运行时 input RootSchema，防止文档复制漂移。
- 验证：纵向正反用例 2 项通过；`cargo fmt --all -- --check`；yang-db lib（381 passed，1 ignored）；yang-base lib（478 passed，8 ignored）；yang-db doctest（65 passed）；yang-base doctest（74 passed，148 ignored）；两库 all-target/all-feature Clippy `-D warnings` 与 yang-base Rust 1.80 all-feature check 均通过。
- Phase 2 结果：Action 运行时 Schema 与 OpenAPI 使用同一 catalog 对象；缺 route、重复 route/operation 在注册或 catalog 构建期失败；公开性只存在于 ActionMeta，不在 RouteDescriptor 复制，因此不存在双源冲突。

## 2026-07-15 - P2-03 OpenAPI 3.1 投影

- 范围：`yang-base/openapi` 可选 feature、`OpenApiInfo`、`ApiCatalog::to_openapi`、完整 JSON snapshot 和 ApiCatalog 文档。
- 风险：运行时 Schema、路由与权限虽已汇合，但缺少可发布契约；若投影重新读取 Action 或复制 method/path，会产生第二真相，公开 catalog 被修改后还可能让重复 operation/route 静默覆盖。
- RED：在仅启用 openapi 的最小组合中从 ApiCatalog 投影，产生 2 个编译错误，确认 OpenApiInfo 与 to_openapi 公共边界缺失。
- 修复：feature-gated 纯投影生成 OpenAPI 3.1 JSON，不新增依赖；input/output RootSchema 直接取自 ActionDescriptor，映射 operation/tags/content types/success status、ApiResponse 成功/错误 envelope、bearer security、公开性与权限扩展。投影前重新校验 route、method 与 operation id，并自行确定性排序。
- 对抗性验证：完整 OpenAPI JSON snapshot 覆盖公开/私有 Action、Schema、安全、权限和 400/401/403/500；另验证手工篡改 catalog 为不支持的 CONNECT method 或重复 operation id 均失败。仅 openapi 的 no-default 测试 2 项通过，完全关闭 openapi 的 no-default check 通过。
- 门禁：`cargo test -p yang-base --lib --all-features --locked`（476 passed，8 ignored）；doctest（74 passed，148 ignored）；all-target/all-feature Clippy `-D warnings` 与 Rust 1.80 all-feature check 均通过。

## 2026-07-15 - P2-02 确定性 ApiCatalog

- 范围：`yang-base::router` 的 `RouteDescriptor`、`ActionDescriptor`、`ModuleDescriptor`、`ApiCatalog`，ModuleRouter route 绑定/只读快照，AppRouter 全局 catalog 与文档。
- 风险：ActionMeta 已持有运行时 Schema/权限/公开性，但 method/path 等传输信息没有唯一来源；HashMap 枚举不稳定，外部若直接接触注册表会破坏 dispatch 不变量，重复 route/operation 和漏绑 route 只能推迟到适配层暴露。
- RED：两个真实 TypedAction 从公开注册入口构造 catalog，产生 10 个编译错误，确认 RouteDescriptor、register_route、module descriptor 与 app catalog 全部缺失。
- 修复：RouteDescriptor 独立保存 method/path/operation id/content types/success status/tags，ActionMeta 不重复传输字段；ModuleRouter 私有保存 action-route 绑定，`descriptor()` 生成 owned snapshot 并合并 Schema、权限和公开性；AppRouter `catalog()` 跨模块校验并按模块/Action 名称排序，运行时注册表和 middleware 不暴露。
- 对抗性验证：5 项 catalog 测试覆盖缺 route、模块内重复 route、重复 operation id、跨模块两类冲突、构造后恶意改写公开 descriptor 再注册、逆序注册稳定排序和 Schema 同源；无默认 feature 下同样通过。
- 门禁：`cargo test -p yang-base --lib --all-features --locked`（474 passed，8 ignored）；doctest（74 passed，148 ignored）；all-target/all-feature Clippy `-D warnings` 与 Rust 1.80 all-feature check 均通过。

## 2026-07-15 - P2-01 transport-neutral RequestMeta

- 范围：`yang-base` 的 `RequestMeta`、`ActionContext` 兼容构造/注入、`Request` Debug 与规范 header 助手，以及当前 API 文档。
- 风险：Action 上下文只能携带 body/headers/query/path params，传输适配器无法稳定传递 method、原始 URI、scheme、peer/local address；派生 Debug 会直接输出 Authorization、Cookie、User-Agent 等 header 值，新增 URI/地址后还会扩大日志泄漏面。
- RED：从公开入口新增缺失/存在/注入/Debug 测试后产生 4 个编译错误，分别确认 `RequestMeta` 未导出、`ActionContext.request_meta` 不存在、兼容构造未提供默认元数据、builder 注入方法缺失。
- 修复：新增不依赖 Web 框架的 `RequestMeta`，可选字段覆盖 method、original URI、scheme、`SocketAddr` peer/local address 和确定性 extensions；作为 `ActionContext` sidecar 默认构造并通过 `with_request_meta` 注入，保持 `Request::new(body)` 行为与公开字段不变。`Request` 自定义 Debug 对 headers/query/path values 脱敏，`RequestMeta` 对 URI、地址和 extension values 脱敏；User-Agent/Cookie 只从规范 headers 提供只读助手，不重复存储；未加入 handle time。
- 对抗性验证：缺失/存在/sidecar 注入 3 条路径及 8 种敏感载荷泄漏负例通过；默认 feature request 回归 45 项通过，无默认 feature RequestMeta 3 项通过。
- 门禁：`cargo test -p yang-base --lib --all-features --locked`（469 passed，8 ignored）；doctest（74 passed，148 ignored）；all-target/all-feature Clippy `-D warnings` 与 Rust 1.80 all-feature check 均通过。

## 2026-07-15 - P1-04 后端能力与统一管理面契约

- 范围：`yang-db` 的 MySQL/PostgreSQL QueryBuilder/Transaction/Redis 能力表、三后端 `capabilities/health_check/close/is_closed/pool_status` 管理面、`yang-base` Redis 停机编排，以及 `docs/BACKEND_CAPABILITIES.md`。
- 风险：后端能力只能靠实现细节推断，PostgreSQL 方言能力可能被误认为跨后端可用；SQL 健康检查返回 `Result<()>`，Redis 关闭为同步调用且健康检查把连接池/命令故障吞成 `Ok(false)`，导致统一编排和故障诊断失真。
- RED：编译期管理面契约产生 6 个错误，确认三后端均缺少 `capabilities()`、MySQL/PostgreSQL `health_check` 返回类型不符、Redis `close` 不是 Future；关闭 Redis 池的对抗测试进一步确认旧实现会吞掉实际连接池错误。
- 修复：新增可机读 `BackendCapabilities`、`BackendCapability`、`BackendKind`、`PlaceholderStyle`、`SafetyConstraint` 及三后端静态常量；显式区分 PostgreSQL `RETURNING`/冲突目标、MySQL 原生 upsert 和 Redis 原生 Pipeline/WATCH/Lua。三后端统一为 `health_check().await -> Result<bool, DbError>`、`close().await`、`is_closed() -> bool`、`pool_status() -> PoolStatus`，Redis 基础设施错误改为结构化传播。
- 对抗性验证：能力正反例与编译期签名测试 4 项通过；关闭后的 MySQL/PostgreSQL/Redis 健康检查 3 项均验证为具体连接池错误；`cargo test -p yang-db --lib --all-features --locked`（381 passed，1 ignored），`cargo test -p yang-base --lib --all-features --locked`（465 passed，8 ignored），无 feature 能力测试 4 项通过。
- 门禁：`cargo test --doc -p yang-db --all-features --locked`（65 passed），`cargo test --doc -p yang-base --all-features --locked`（74 passed，148 ignored）；两库 all-target/all-feature Clippy `-D warnings` 通过；Rust 1.80 all-feature check 通过。

## 2026-07-15 - P1-03 yang-db/yang-base 独立 feature 矩阵

- 范围：workspace/两库 Cargo feature、后端模块 gate、跨后端 `PoolStatus`、无 feature 测试 gate、README/docs.rs、CI feature matrix 与依赖隔离校验器。
- 风险：workspace `sqlx` 无条件启用 MySQL/PostgreSQL，`yang-db` 的 SQL/Redis 依赖和模块均不可裁剪，`yang-base` 又通过默认依赖静默拉入全部后端；原 CI 的 `yang-db-current` 无法证明单 feature 独立。关闭默认 feature 后，测试 target 还会调用不存在的 MySQL 方法或断言只在 validator feature 下成立的严格语义。
- 修复：`yang-db` 建立 none/mysql/postgres/redis/all feature，驱动均改为 optional 并 gate 模块/转换；workspace `sqlx` 移除后端默认泄漏。`yang-base` 关闭 `yang-db` 默认 feature，精确转发 mysql/redis，token 显式依赖 redis；DatabaseBundle 仅在 mysql+redis 下公开。把 `PoolStatus` 上移为后端中立根类型，解除 SQL 对 Redis 模块的反向依赖；docs.rs 固定 all-features，README 给出最小组合。
- RED：首次 `yang-base --no-default-features --lib` 出现 65 个缺方法编译错误；修正测试 gate 后又暴露 5 个 validator fallback 断言错误。首次 mysql-only `yang-db` 因 SQL pool status 引用已关闭的 Redis 模块失败；mysql-only doctest 又因缺少 `sqlx/derive` 失败 5 项。
- 对抗性验证：新增 validator 关闭时 Regex fail-closed、Email/Phone 文档化 fallback 测试；`verify_feature_isolation.py` 对每条 required/forbidden 依赖做恶意删改自测，并用 `cargo tree -e normal` 证明 none/单后端不会夹带 sqlx-mysql、sqlx-postgres、Redis、HTTP 或 JWT 的无关组合。
- 已运行验证：yang-db none/mysql/postgres/redis/all 五组均在 `-Dwarnings` 下通过 check、lib test、doctest；all 为 374 passed/1 ignored，65 doctests，其余组合按实际 gate 运行 27/273/75/84 项单测与 0/25/7/33 项 doctest。
- 已运行验证：yang-base none/token/http/mysql/redis/validator/plugin-schema/metrics/default/all 十组均在 `RUSTFLAGS=-Dwarnings`、`RUSTDOCFLAGS=-Dwarnings` 下通过 check、lib test、doctest；none 为 231 passed，default/all 为 465 passed/8 ignored、74 doctests/148 ignored。
- 已运行验证：`python scripts/verify_feature_isolation.py --self-test`；CI contract 自测/实测与 workflow YAML 解析。
- 已运行验证：`cargo +1.80.0 check -p yang-db -p yang-base --all-features --locked`；`cargo clippy -p yang-db -p yang-base --all-targets --all-features --locked -- -D warnings`。

## 2026-07-15 - P1-02 基础设施结构化错误链

- 范围：`yang-base` 的 `BaseError`、`GlobalRedis`、`DatabaseInitializer`、插件生命周期接口及其示例/测试。
- 风险：GlobalRedis 的连接与 44 个操作入口把 `yang_db::DbError` 提前转为字符串；迁移执行只保留 module/version/reason，无法关联具体 SQL 内容；插件 register/init/shutdown 回调也立即把动态错误扁平化，导致 `Error::source()` 在基础设施边界中断。
- 修复：`RedisConnectionFailed` 与 `RedisOperationFailed` 直接持有 `#[source] DbError`，所有 GlobalRedis 入口统一传递结构化错误；新增带 module/version/FNV-1a checksum/source 的 `MigrationExecutionFailed`；新增 `PluginError`、`PluginLifecycleStage` 与 `PluginLifecycleFailed`，真实生命周期路径保留插件名、阶段和原始 error object。旧错误码按阶段复用，现有兼容变体与错误码不删除。
- RED：新增 source 链测试首次无法编译，明确暴露 Redis 变体仍要求 `String`、迁移/插件结构化变体不存在；调用链检查同时确认 44 个 Redis 操作、两条迁移执行路径和四条插件生命周期路径存在 `to_string()` 扁平化。
- 对抗性验证：覆盖 Redis 连接/命令错误、迁移 module/version/checksum/source、插件 register/init/shutdown 阶段；真实 manager/builder/registry 路径验证回调失败未注册、逆序 shutdown 继续执行且首个错误仍保留 source；`code()`/`code_str()` 对新旧结构化变体保持一致。
- 已运行验证：错误链、迁移 checksum、register/shutdown 定向单测与 `plugin_test` 失败路径均通过。
- 已运行验证：`cargo test -p yang-base --lib --locked`（465 passed，0 failed，8 ignored）；`cargo test -p yang-base --test plugin_test --locked`（17 passed）。
- 已运行验证：`cargo test --doc -p yang-base --locked`（74 passed，148 ignored）；`cargo clippy -p yang-base --all-targets --all-features --locked -- -D warnings`。
- 已运行验证：`cargo +1.80.0 check -p yang-base --all-features --locked`。
- 后续基线：额外运行 `cargo test -p yang-base --no-default-features --lib --locked` 暴露 65 个测试 target 缺少 feature gating，已留给 P1-03 处理；默认特性与 all-feature 门禁不受影响。

## 2026-07-15 - P1-01 SQL 语义类型与唯一 checked renderer

- 范围：`crates/yang-db/src/sql_types.rs`、MySQL/PostgreSQL 的 `identifier`、`condition` 与 `query_builder` 模块。
- 风险：公开的 infallible 条件渲染器仍可能在后半棵条件树失败前写入参数，并存在 RAW SQL 回退；field/order/group/join ON 也用同一种 `String` 表示标识符和可信表达式，调用边界不清晰。
- 修复：引入内部 `Identifier`、`QualifiedIdentifier`、`TrustedSqlExpr` 与 `RenderedCondition<T>`；两种方言只通过返回 `Result<RenderedCondition, DbError>` 的 checked renderer 生成条件，QueryBuilder 直接消费完整渲染结果。deprecated 兼容函数仅委托 checked 路径，失败统一返回 `/* invalid condition */ 1 = 0` 且不修改参数；新增 field/order/group/join ON 的显式 checked identifier API，原字符串 API 明确为可信表达式入口。
- RED：混合“合法条件 + 恶意后置字段”的条件树会在返回错误前追加前缀参数；legacy API 会把拒绝的字段载荷作为 RAW SQL 输出；缺少显式安全 API 时，外部字段名只能进入可信表达式入口。
- 对抗性验证：两方言对称覆盖空段、三段、SQL 注释、单双引号、反引号、Unicode、NUL、空白和函数表达式；属性测试证明所有被接受的标识符均符合严格 ASCII 一段/两段语法；验证失败渲染的参数事务性以及 PostgreSQL 从既有参数偏移继续编号。
- 已运行验证：P1-01 定向测试（12 passed），包括 partial params、legacy fail-closed、PostgreSQL placeholder order、两方言 safe identifier API 与属性测试。
- 已运行验证：`cargo test -p yang-db --lib --locked`（374 passed，0 failed，1 ignored）；`cargo test --doc -p yang-db --locked`（65 passed）。
- 已运行验证：`cargo test -p yang-base --lib --locked`（461 passed，0 failed，8 ignored）。
- 已运行验证：`cargo +1.80.0 check -p yang-db -p yang-base --all-features --locked`；`cargo clippy -p yang-db -p yang-base --all-targets --all-features --locked -- -D warnings`。

## 2026-07-15 - P0-04 stable/MSRV/feature/Docker 持续门禁

- 范围：`.github/workflows/ci.yml`、`scripts/verify_ci_contract.py`、workspace 依赖锁定、两库 doctest、Redis 事务错误分类、`typed_action_integration` 的真实服务依赖。
- 风险：仓库没有 CI，锁文件已漂移到要求 Rust 1.82-1.88/Edition 2024 的依赖，声明的 MSRV 1.80 无法编译；doc tests 存在 19 个过期示例；MySQL typed-action job 未初始化鉴权黑名单所需 Redis，配置出来的 job 实际为红。
- 修复：建立 stable fmt/lib/clippy/doc、Rust 1.80、九组 feature matrix，以及 MySQL 8/PostgreSQL 16/Redis 7 串行集成 job；全部 Cargo 命令使用 `--locked`。把 Redis/testcontainers/JWT/HTTP 等依赖收敛到支持 1.80 的兼容版本，并按真实旧版 API 修正 Redis/JWT 边界；同步修复 doctest 示例和 `non_exhaustive` 配置构造入口；typed-action 集成测试同时启动 MySQL 8 与 Redis 7。
- RED：真实 `cargo +1.80.0 check` 依次暴露 Edition 2024 manifest、高于 1.80 的 `rust-version` 及 JWT/Redis API 差异；首次 doctest 为 `yang-db` 13 失败、`yang-base` 6 失败；首次 typed-action Docker 运行返回 `RedisNotInitialized`。
- 对抗性验证：CI contract self-test 逐一删除每个必需 job/命令/镜像/串行参数并确认校验器拒绝；Redis 错误测试区分 EXECABORT/TypeError/ResponseError；过期 JWT 证明安全验证失败而显式 unsafe 解析仍成功；真实容器验证 Redis pipeline 100+ 批量/并发/错误恢复、Lua 脚本和 typed-action 全 CRUD 链路。
- 已运行验证：`python scripts/verify_ci_contract.py --self-test`；workflow contract 校验与 YAML 解析。
- 已运行验证：`cargo +1.80.0 check -p yang-db -p yang-base --all-features --locked`。
- 已运行验证：`cargo test --lib -p yang-db --locked`（363 tests，0 failed，1 ignored）；`cargo test --lib -p yang-base --locked`（461 passed，0 failed，8 ignored）。
- 已运行验证：`cargo clippy -p yang-db -p yang-base --all-targets --all-features --locked -- -D warnings`；两库 doc tests（65 passed；74 passed、148 ignored）；九组 feature matrix 均在 `RUSTFLAGS=-Dwarnings` 下通过。
- 已运行验证：Redis pipeline（9 passed）、Redis script（13 passed）、typed-action MySQL 8 + Redis 7（1 passed）真实 Docker 集成。
- 说明：三个 TableQuery Docker 测试文件按测试逐个启动容器，整文件串行执行超过本地工具 120 秒上限；CI job 已保留无超时的串行执行，发布候选阶段再记录完整结果。

## 2026-07-15 - P0-03 all-target/all-feature Clippy 门禁

- 范围：`crates/yang-db/src/mysql/database.rs`、`crates/yang-db/src/postgres/database.rs`、`crates/yang-db/src/redis/config.rs`、`crates/yang-base/src/http/request.rs`、`crates/yang-base/src/table/dynamic_row.rs`
- 风险：all-target Clippy 因 12 个测试 `expect_err()` 和两个位于生产项之前的 test module 失败，导致 lint 门禁无法用于持续集成；绕过 lint 会让测试 target 与 workspace 的 panic 契约继续漂移。
- 修复：把错误变体断言改为直接匹配 `Result::Err`，意外成功仍会使断言失败；把 HTTP request 与 DynamicRow 的测试模块移到文件末尾，不增加任何 crate/module 级 lint 豁免。
- RED：`cargo clippy -p yang-db -p yang-base --all-targets --all-features -- -D warnings` 报告 12 个 `clippy::expect_used` 和 2 个 `clippy::items_after_test_module`。
- 对抗性验证：保留所有非法池大小、超时、retry、URL、bearer token、空 query key 和空白动态列名断言，证明门禁修复没有删除或弱化负向测试。
- 已运行验证：`cargo clippy -p yang-db -p yang-base --all-targets --all-features -- -D warnings`
- 已运行验证：`cargo test -p yang-db --lib database_config_validate_rejects`
- 已运行验证：`cargo test -p yang-db --lib test_validate_rejects`
- 已运行验证：`cargo test -p yang-base --lib retry_config`
- 已运行验证：`cargo test -p yang-base --lib get_rejects_blank_column_name`

## 2026-07-15 - P0-02 字段权限投影契约与确定性错误

- 范围：`crates/yang-base/src/table/table_query.rs`、`crates/yang-base/src/table/__tests__/table_query_test.rs`
- 风险：两个 OR 语义测试默认生成 `SELECT *`，却使用含管理员专属字段的 `user` fixture，正确的 fail-closed 字段权限会先于 OR SQL 断言触发；同时全字段权限校验直接遍历 `HashMap`，多个受限字段时错误文本不确定。
- 修复：OR/嵌套测试显式选择断言涉及的 `id/name/email` 可读字段；`build_select_sql()` 与整实体 Action 的 `ensure_fields_readable()` 共用同一权限校验，并固定返回字典序最小的不可读字段。
- 产品语义：`SelectAction<T>` 返回完整实体 `T`，缺字段投影无法保证反序列化契约，因此继续要求全部字段可读，不新增静默投影可读字段的默认路径；显式请求受限字段仍返回 `FieldPermissionDenied`。
- RED：`cargo test -p yang-base --lib` 稳定失败于两个 OR 测试；`cargo test -p yang-base --lib test_unreadable_field_errors_are_deterministic` 证明错误字段受 HashMap 顺序影响。
- 对抗性验证：覆盖全部可读、部分不可读、零可读、显式越权四种投影情况，并连续重建 64 次多受限字段配置，验证底层 SELECT 与整实体 Action 均固定报告 `a_secret`。
- 已运行验证：`cargo test -p yang-base --lib test_unreadable_field_errors_are_deterministic`
- 已运行验证：`cargo test -p yang-base --lib test_select_projection_permission_matrix`
- 已运行验证：`cargo test -p yang-base --lib test_where_or_renders_parenthesized_or`
- 已运行验证：`cargo test -p yang-base --lib test_nested_or_and_groups`
- 已运行验证：`cargo test -p yang-base --lib`（461 passed，0 failed，8 ignored）

## 2026-07-15 - P0-01 MySQL/PostgreSQL 限定条件字段回归

- 范围：`crates/yang-db/src/mysql/condition.rs`、`crates/yang-db/src/mysql/query_builder.rs`、`crates/yang-db/src/postgres/condition.rs`、`crates/yang-db/src/postgres/query_builder.rs`
- 风险：QueryBuilder 先用仅支持单段字段的 `quote_identifier()` 校验 WHERE/HAVING，再交给可回退原始字符串的 legacy renderer；合法 `table.field` 被错误拒绝，校验和渲染也不是同一条安全路径。
- 修复：MySQL/PostgreSQL QueryBuilder 的 WHERE/HAVING 直接委托 checked renderer；checked renderer 使用 `quote_qualified()` 对单段或两段标识符逐段校验并按方言转义，不再执行“前置校验 + RAW 渲染”的双轨逻辑。
- RED：`cargo test -p yang-db --lib test_try_to_sql_accepts_qualified_where_and_having_identifiers`，两方言测试均以 `InvalidArgument("非法 SQL 标识符: \"users.status\"")` 失败。
- 对抗性验证：两方言对称覆盖合法 `field`/`table.field`，以及空段、三段、分号、注释、引号、函数表达式；同时验证 `try_to_sql()` 返回真实错误且 `to_sql()` 只返回固定不可执行哨兵。
- 已运行验证：`cargo test -p yang-db --lib test_try_to_sql_accepts_qualified_where_and_having_identifiers`
- 已运行验证：`cargo test -p yang-db --lib test_try_to_sql_rejects_malicious_qualified_where_and_having_identifiers`
- 已运行验证：`cargo test -p yang-db --lib test_select_complex_query`
- 已运行验证：`cargo test -p yang-db --lib test_sql_generator_complex_query`
- 已运行验证：`cargo test -p yang-db --lib`（360 passed，0 failed，1 ignored）

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

## 2026-07-07 - Action 请求参数 key 边界空格规范化

- 问题：`Request::header`、`Request::query`、`Request::path_param` 只用 `trim()` 判断空白，但存储时保留边界空格，导致 `token()`、`get_query()`、`get_path_param()` 等正常读取失败。
- 修改：写入和读取 header/query/path 参数 key 时统一先 `trim()`；header key 在 trim 后继续做 ASCII 小写规范化。
- 验证：先新增 header/query/path 三个 trim 行为用例并确认失败，再实现规范化，随后运行 `cargo test -p yang-base --lib trims` 确认通过。

## 2026-07-07 - ActionContext 路径参数 key 查询规范化

- 问题：`Request::path_param` 已对 key 做边界空格规范化，但 `ActionContext::path_param` 直接用调用方传入的原始 key 查底层 map，导致 `context.path_param(" id ")` 返回 `ParamMissing`。
- 修改：`ActionContext::path_param` 在空白校验、查找和错误信息中统一使用 trim 后的 key。
- 验证：先新增 `test_action_context_path_param_trims_lookup_key` 并确认失败，再实现规范化，随后运行该单测确认通过。

## 2026-07-07 - GlobalTools 工具名称规范化

- 问题：`GlobalTools::register_tool` 只用 `trim()` 判断空白，但注册、读取和重复检测仍使用原始名称，导致带边界空格的工具名注册后无法用规范名读取，也可能绕过重复注册检查。
- 修改：`register_tool` 和 `get_tool` 均使用 trim 后的工具名称；空白名称仍被拒绝或返回 None。
- 验证：先新增 `test_global_tools_trims_tool_names_for_register_and_get` 并确认失败，再实现名称规范化，随后运行该单测确认通过。

## 2026-07-07 - 插件名称规范化与空名称拒绝

- 问题：`PluginManager`、`PluginManagerBuilder`、`PluginRegistry` 以 `Plugin::name()` 或查询参数原样作为索引，带边界空格的插件名可能注册后无法用规范名查找，也可能绕过重复注册；配置加载同样可能用原始名称存储。
- 修改：新增内部插件名规范化逻辑，注册时 trim 并拒绝空名称；运行期/构建期查找、配置加载、配置读取和构建期依赖检查均使用规范化名称；构建期拓扑排序改为基于内部规范化 key 排序。
- 验证：先新增 `plugin_names` 相关测试并确认失败，再实现规范化；随后运行 `cargo test -p yang-base --lib plugin_names`、`cargo test -p yang-base --lib topological`、`cargo test -p yang-base --lib dependency` 确认通过。

## 2026-07-07 - PCG Grid2D 拒绝零维度

- 问题：`Grid2D::new(0, n)` 或 `Grid2D::new(n, 0)` 会成功创建空网格，但地形网格作为房间基础结构要求宽高均为正数，零维度会把配置错误推迟到后续算法阶段。
- 修改：`Grid2D::new` 在计算尺寸和分配前拒绝 0 宽或 0 高，返回 `PcgError::terrain`。
- 验证：先新增 `grid_new_rejects_zero_dimensions` 并确认失败，再实现非零校验，随后运行该单测确认通过。
