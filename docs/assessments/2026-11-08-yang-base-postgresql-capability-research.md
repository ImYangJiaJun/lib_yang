# PostgreSQL 能力调研：yang-base 的现状、缺口与路线图

**范围**：`crates/yang-base`（应用框架层）及其与 `crates/yang-db` 的衔接  
**基线**：yang-base 0.4.0 / yang-db 0.2.1；PostgreSQL 16（17/18 差异单独标注）  
**配套报告**：`docs/assessments/2026-11-08-postgresql-mature-capability-research.md`（yang-db 驱动层，本报告不重复其能力清单）  
**证据基线**：2026-11-08 本地代码审计 + PostgreSQL 官方文档 + 成熟框架对标 + 对抗核实

## 1. 摘要

**全栈定位**：`yang-db` 是「有基础、不成熟」——已有 `postgres` feature、`$N` 占位符、单表 CRUD、事务与受限 `RETURNING`/`ON CONFLICT`；`yang-base` 是「零 PG 支持、全链路 MySQL 绑定」——没有 `postgres` feature，表句柄、查询计划、动态行解码、内置 CRUD 与 schema 同步全部编译在 `#[cfg(feature = "mysql")]` 下，公开签名直接出现 `sqlx::MySqlPool` / `MySqlRow` / `MySqlConnection`。

本报告把「能力基线（成熟框架应该有什么）」与「当前实现（已经有什么）」分开：前者给出 40 项能力（P0 15、P1 19、P2 6），后者逐项给出 `file:line` 证据。结论是：yang-base 侧不存在「已有 PG 分支待开启」，所有执行路径都需要新增后端抽象；但表定义 DSL、`FieldType` 校验、`WhereCondition`、权限/租户/软删计划、`RelationBatchExecutor` trait、错误枚举是可复用的方言无关资产，因此不需要重写业务规则。

### 最高优先级缺口 Top 8

| # | 缺口 | 后果 |
|---|---|---|
| 1 | 无 `postgres` feature；`TableHandle::bind`/`TableQuery`/`Record` 固定 `MySqlPool`/`MySqlRow` | PG-only 构建下执行层整体不存在，无法编译出任何 PG 路径 |
| 2 | `Record` 只有 `FromRow<MySqlRow>`，按 MySQL 类型名解码 | 所有读路径（select/paginate/关联/内置 Action）不可用 |
| 3 | `table_query/plan.rs` 直接产出 MySQL `QueryBuilder`，且不注入 `field_types`、不归一 `= NULL` | PG 严格类型下类型不匹配或静默 0 行 |
| 4 | CRUD 返回契约是 `last_insert_id` + `u64` 主键，批量 upsert 只有 ODKU | UUID/text/numeric/非 `id` 主键无法回填；PG 无批量 upsert |
| 5 | `schema_sync` 5 个文件全部 MySQL：`information_schema`/`GET_LOCK`/`MODIFY COLUMN`/`ENGINE=InnoDB`/`AUTO_INCREMENT` | PG 启动同步完全不可用 |
| 6 | DDL 类型映射与标识符/转义是 MySQL 形状（`TINYINT(1)`、反引号、反斜杠双写、64 字符） | 生成的 DDL 在 PG 语法错误或语义改变 |
| 7 | `Tools`/`EngineConfig`/`DatabaseInitializer`/`ActionContext` 单后端生命周期；内置 CRUD 被 cfg 编译掉 | 无法配置、持有、健康检查或关闭 PG 资源 |
| 8 | 无任何 yang-base PG 测试、CI cell 或 docker-postgres 作业；能力矩阵易被误读为 base 已支持 PG | 「支持 PG」不可验收，回归无门禁 |

### 全栈实施顺序（结论）

1. **yang-db 先修 P0 契约**：保留 SQLSTATE、`RETURNING` 可配置（列/关闭/非 bigint 主键）、`ON CONFLICT` 显式目标与批量 upsert、typed NULL、公开 identifier API。
2. **yang-base 先做 MySQL-only 抽象重构**：引入 `AnyDatabase`（MySQL 单变体）、`Statement`、sealed `Executor`、`AnyTransaction`，现有 MySQL 测试零行为变化。
3. **接入 `postgres` feature 与单栈最小闭环**：配置/连接/Tools/Record/类型映射/identifier/CRUD/分页/软删/租户/事务。
4. **schema_sync 方言化**：抽 planner + `SchemaDriver`，先 MySQL 回归，再 PG 反射/渲染/锁/预检/dry-run。
5. **双后端契约测试与 CI 矩阵**：`none`/`mysql`/`postgres`/`mysql+postgres` 四档 + `docker-postgres` 跑 yang-base。
6. **文档与 semver**：能力矩阵增加「base 接入状态」列，公开 breaking 变更给弃用窗口。

## 2. 分层模型与依赖关系

| 层 | 主要位置 | 职责 | PG 能力落地归属 |
|---|---|---|---|
| 驱动/方言层 | `crates/yang-db/src/{dialect,capability,postgres}` | 连接池、事务原语、占位符、QueryBuilder、SQLSTATE、行绑定 | **yang-db**（base 只消费） |
| 定义层 | `src/table/{definition,field_type,field_config}.rs`、`src/definition/` | `Table`/`Field`/`FieldType`、校验、JSON Schema | yang-base（保持方言无关） |
| 查询层 | `src/table/table_query/` | 计划、过滤、分页、权限/租户/软删守卫 | yang-base（需后端执行器） |
| 记录层 | `src/table/record.rs` | 动态行 → JSON `Record` | yang-base（需 PG 解码器） |
| DDL 层 | `src/database/schema_sync/` | 反射、diff、渲染、锁、预检、dry-run | yang-base（需 PG driver） |
| 运行时层 | `src/{tools,config}.rs`、`src/action/context.rs`、`src/database/initializer.rs` | 资源槽、配置、事务上下文 | yang-base |
| 动作层 | `src/action/builtin/`、`src/definition/spec.rs` | 内置 CRUD、Action 注册 | yang-base（业务逻辑可复用） |

**必须先在 yang-db 落地的能力**：SQLSTATE 结构化（当前 `crates/yang-db/src/error.rs:163-186` 把 23000/23502/23503/23505/23514 统一抹平成 `ConstraintError(message)`）；`RETURNING` 可选列/可关闭/非 `CAST(... AS BIGINT)` 主键回填；`ON CONFLICT` 显式目标与批量 upsert；typed NULL 绑定；`PgQueryBuilder` 的公开构造入口或让 base 统一走 `PgDatabase::table()`；方言无关 `Identifier`/`QualifiedIdentifier` 公开；事务只读/可延迟与语句超时。

**属于 yang-base 自身抽象问题的**：`TableHandle`/`TableQuery`/`Record` 公开签名绑定 sqlx 具体类型；`schema_sync` 没有方言缝；`Tools`/`Config` 单后端；`action/builtin` 整体 cfg 导致 PG 用户没有默认 CRUD。

**共享抽象 vs 双实现推荐**：共享「表定义 DSL、`FieldType` 逻辑类型、`WhereCondition`、权限/租户/软删计划、`SchemaSyncReport`/`SchemaPreflightReport` 契约、Action 业务逻辑、错误枚举」；**双实现只放在**「SQL 渲染、系统目录反射、DDL 锁与事务、行解码」四处。不要复制两套 base，否则权限/软删/租户规则必然漂移。

## 3. 调研方法与证据等级

| 等级 | 来源 | 用途 | 局限 |
|---|---|---|---|
| A 官方规范 | PostgreSQL 16/17/18 文档：datatype、sql-*、errcodes、protocol、catalogs、runtime-config；JSON Schema 2020-12；Cargo features | 定义「语义上应该有什么」 | 文档是能力上限，不代表驱动已实现 |
| B 成熟实现对标 | SeaORM、SQLAlchemy、Diesel、GORM、Hibernate、Prisma、Django、Rails、Alembic | 判断「成熟框架实际提供什么」、API 形状与测试实践 | 设计目标不同，不能直接等同 |
| C 本地审计 | 两份独立审计（A/B）对 `crates/yang-base` 逐文件核对，附 `file:line` | 确认「当前到底有什么」 | 行号会随重构漂移 |
| D 对抗核实 | 48 条声明逐条复核：47 条成立、1 条证伪；并做完整性批评 | 剔除错误事实、识别遗漏 | 无法替代真库行为测试 |

证据结论：代码事实为高置信；框架设计建议为中置信；「同进程双后端是否有真实消费者」「RLS 是否有真实消费者」「PG 18 支持窗口」标记为**待验证**。

## 4. P0/P1/P2 分级定义（框架层口径）

| 级别 | 定义 | 典型项 |
|---|---|---|
| **P0** | 缺失会使 PG 路径**不可编译、语义错误、静默错或安全边界失效**；是「敢声称支持 PG」的准入线，必须作为发布门禁 | feature/句柄/执行器、Record 解码、类型映射、identifier、RETURNING/upsert、schema_sync 反射与 DDL、SQLSTATE、测试与 CI |
| **P1** | 成熟应用框架应当提供的常用能力；缺失会让真实业务频繁绕裸 SQL或带来运维风险 | 统一事务/savepoint/retry、会话与池配置、LIKE/ILIKE、NULLS 排序、在线 DDL、复合类型解码、约束元数据、数据迁移策略 |
| **P2** | 进阶、差异化或依赖外部工具的能力；不影响单栈最小闭环 | 可观测性细化、多后端同时在线、RLS/GUC、版本门控、迁移历史/导出、性能基准 |

**状态口径**：`绝对缺失`＝没有任何 PG 代码路径；`硬编码`＝逻辑存在但写死 MySQL 类型/语法；`不对称`＝两后端都能跑但语义不同；`部分`＝有可复用逻辑但入口或边界不完整；`已具备`＝无需 PG 改造。

## 5. 能力基线全景

> 说明：本表是「成熟应用框架应该有什么」的基线，不是 yang-base 的已完成清单。「PG 特有」表示该能力源于 PostgreSQL 类型/语义或 PG 独有机制。

### A. 后端抽象与运行时

| 编号 | 能力 | 说明 | P | PG 特有 | yang-base 现状 |
|---|---|---|---|---|---|
| A1 | 运行时底座 | `postgres` feature + `AnyDatabase`（非穷尽 struct + 内部 enum）+ sealed `Executor` + `Statement`；`TableHandle`/`TableQuery` 公开签名去 sqlx 类型；配置与 `Tools` 单资源生命周期 | P0 | 否 | 缺失。`Cargo.toml:26,35,60,106` 仅 mysql；`definition.rs:1040,1116`、`table_query/mod.rs:139`、`build.rs:25` 固定 `MySqlPool`；`config.rs:34,38-39` 仅 `YANG_MYSQL_URL`；`tools.rs:69-70,114-125,203-216,251-254` 仅 mysql |
| A2 | 统一事务 | `AnyTransaction` + savepoint + 可重试事务 + 请求级传播；池与事务同构 | P1 | 否 | 缺失。`context.rs:469` 的 `begin_transaction` 返回 MySQL `Transaction`；无 savepoint/retry |
| A3 | 会话与连接池 | `search_path`/`TimeZone`/`DateStyle`/`application_name`/`sslmode`/statement cache | P1 | 是 | 缺失。仅 URL 透传；无 `after_connect` 钩子 |
| A4 | 能力注册表 | `BackendCapabilities.supports()` + 启动期必需能力 fail-fast + `UnsupportedCapability` | P1 | 否 | 缺失。`yang-db/src/capability.rs:87-104` 已有常量，base 从未调用 |
| A5 | 可观测性 | tracing/metrics 带 `backend` 标签、池指标、慢查询、受控 EXPLAIN | P2 | 否 | 缺失。`yang-runtime/src/observability/telemetry.rs:382-386` 仅 `tools.mysql()` |
| A6 | 多后端在线 | 动态路由、双后端同进程、公开类型演进空间 | P2 | 否 | 缺失；是否有真实消费者**待验证**，不建议首版实现 |

### B. 表定义与类型映射

| 编号 | 能力 | 说明 | P | PG 特有 | yang-base 现状 |
|---|---|---|---|---|---|
| B1 | `FieldType`→PG 类型 | 13 个变体到 PG 类型名；Boolean/Json/Enum/时间语义；未知类型显式拒绝 | P0 | 是 | 缺失。`render.rs:298-346` 输出 `INT`/`TINYINT(1)`/`DATETIME`/`JSON`/`ENUM` |
| B2 | 标识符与字面量 | 双引号、63 字节上限、大小写折叠、`schema.table`、`standard_conforming_strings` | P0 | 是 | 硬编码。`render.rs:431` 调 `yang_db::mysql::quote_identifier`；`:415-419` 64 字符；`:437` 反斜杠双写；`yang-db/src/sql_types.rs` 的 `TableRef` 仅接受单段标识符 |
| B3 | 自增与主键生成 | `AUTO_INCREMENT`→`GENERATED BY DEFAULT AS IDENTITY`；INSERT 省略列后 `RETURNING` 回填 | P0 | 是 | 缺失。`render.rs:354`；`definition.rs:112-119`；`schema_validation.rs:44-53,146-164` 以 `column_type` 含 `auto_increment` 判定 |
| B4 | 边界语义与上限 | `varchar` 字符数（PG 10,485,760）、`numeric(p,s)`、naive/UTC/微秒、`text` 容量 | P1 | 部分 | 部分。`field_type.rs:9` `MAX_VARCHAR_LENGTH=16_383`；`schema_validation.rs:238-284` 把 bool 归一为 1/0 |
| B5 | 专属类型层 | `Uuid`→`uuid`、`Blob`→`bytea`、`Array`→`element[]`、扩展类型注册 | P1 | 是 | 缺失。`field_type.rs:58-244` 无 Uuid/Blob/Array；`base64` 仅 mysql feature（`Cargo.toml:35,85`）；`uuid` 仅 token feature（`:100`） |
| B6 | Enum/CHECK/生成列/注释 | Enum 默认 `text/varchar + CHECK`；CHECK 的 `NOT VALID`；generated；`COMMENT ON` | P1 | 是 | 部分。`render.rs:147-215` 是 MySQL `CHECK_CONSTRAINTS` 归一；`model.rs` 无注释/generated 概念 |
| B7 | Collation/索引名/JSON Schema | 字段 collation、63 字节索引名、schema 内唯一、JSON Schema `format/items` | P1 | 部分 | 部分。无字段 collation；`render.rs:415-419` 64 字符；`definition.rs` 已有 decimal/date/enum 的 JSON Schema |
| B8 | 默认表达式 | `DefaultValue::Literal｜Expr` 白名单；`CURRENT_TIMESTAMP` 事务时间语义 | P2 | 是 | 缺失。`field_config.rs:37` 仅 `serde_json::Value` |

### C. Record 动态解码

| 编号 | 能力 | 说明 | P | PG 特有 | yang-base 现状 |
|---|---|---|---|---|---|
| C1 | PG 行解码与契约 | `FromRow<PgRow>`；按 `PgColumn::type_info().oid()`/`PgTypeKind` 分派；text/binary 双格式；统一 JSON 契约；区分 SQL NULL / jsonb null / 缺列 | P0 | 是 | 缺失。`record.rs:185` 仅 `MySqlRow`；`:268-361` 按 MySQL 类型名解码；`:157` 的 `From<Map>` 不能替代行解码；无 `null_kind` |
| C2 | 未知 OID/扩展 | 未知 OID 不静默置 Null；`register(oid, decoder)` 支持 citext/pgvector/hstore | P1 | 是 | 缺失 |
| C3 | 复合类型 | uuid/array/interval/range/composite/inet/cidr/macaddr 递归解码 | P1 | 是 | 缺失 |
| C4 | 列上下文/写路径 | 列名保真、重复列策略、解码错误带 `column/ordinal/oid`、反向编码显式 `$n::type` | P1 | 部分 | 缺失 |

### D. TableQuery/权限/租户/软删/关联加载

| 编号 | 能力 | 说明 | P | PG 特有 | yang-base 现状 |
|---|---|---|---|---|---|
| D1 | 查询计划与执行器 | 后端无关 `QueryPlan`/`Statement`；池/事务同构执行；`field_types` 注入；`NULL` 归一为 `IS NULL`/`IS NOT NULL` | P0 | 部分 | 缺失。`plan.rs:3,85-94,101-111` 直接返回 MySQL `QueryBuilder`；`:113-190` 从不调用 `.json()/.datetime()/.decimal()/.blob()`；`validation.rs:205-212` 注释称会归一，`dialect.rs:170-183` 实际无条件渲染 `=/!=` |
| D2 | CRUD 返回契约 | `insert_returning(PK/Columns/All)`；PG `INSERT ... RETURNING`；`upsert(conflict_columns)`→`ON CONFLICT` | P0 | 是 | 缺失。`write.rs:100,118-129` 依赖 `last_insert_id`；`:180-232` 仅 ODKU；`add.rs:18-28` 的 `InsertResult.id: u64` |
| D3 | 守卫与 schema 限定 | 权限/租户/软删覆盖 select/count/exists/paginate/relation/update/delete；`schema.table` | P0 | 部分 | 部分。`plan.rs:186` 软删守卫安全；`relation_loader.rs:81-113` 直接查目标表绕过守卫；`yang-db/src/reference.rs` 的 `TableRef` 拒绝 `schema.table` |
| D4 | LIKE/ILIKE | `contains`/`icontains` 显式区分；`ESCAPE`；collation 策略 | P1 | 是 | 部分。`filters.rs:188-206,536-588` 统一 `LIKE`；`dialect.rs:253-257` 无 `ILIKE` |
| D5 | 排序与分页 | `NULLS FIRST/LAST`、确定性排序、`COUNT` 与 `SELECT` 同快照、`EXISTS`、`IN` 分块/`ANY(array)` | P1 | 是 | 缺失/部分。无 NULLS 控制；无 EXISTS；500 元素 IN 展开；COUNT 与 SELECT 无事务包裹 |
| D6 | 事务与语句语义 | `statement_timeout`、`FOR UPDATE/SKIP LOCKED/NOWAIT`、隔离级别/只读/可延迟 | P1 | 是 | 部分。`yang-db` 有行锁原语，base 无超时/隔离配置入口 |
| D7 | 进阶 | RLS/`SET LOCAL`、keyset 分页、窗口 `COUNT(*) OVER()`、动态路由 | P2 | 是 | 缺失；RLS 与既有 non-goal 冲突，需 RFC |

### E. Schema Sync/DDL 反射

| 编号 | 能力 | 说明 | P | PG 特有 | yang-base 现状 |
|---|---|---|---|---|---|
| E1 | planner/driver 与反射模型 | 纯 planner + `SchemaDriver`；PG 用 `pg_catalog` 反射列/主键/索引/CHECK/FK/default/identity/comment；中立 `ExistingTableSchema` | P0 | 是 | 缺失。`inspect.rs:51-165` 全 MySQL `information_schema`；`model.rs:37-46`；`schema_validation.rs:5-18` 用 `column_type` 字符串 |
| E2 | PG DDL 渲染 | `CREATE TABLE` 无 ENGINE/内联索引；`ALTER COLUMN ... TYPE ... USING`；identity；无 `MODIFY COLUMN` | P0 | 是 | 缺失。`render.rs:98,113,311-346,354`；`plan.rs:253` |
| E3 | 事务化 DDL/锁/预检 | 单事务 DDL 可回滚；`pg_advisory_lock`；preflight 用 `::text`；plan SQL 预览与事务内 dry-run | P0 | 是 | 缺失。`sync.rs:132,140-157` 用 `DATABASE()`/`GET_LOCK`；`preflight.rs:43-105` 用 `CAST(... AS CHAR)`；无 SQL 预览 |
| E4 | 在线 DDL | `CREATE/DROP INDEX CONCURRENTLY`、`NOT VALID`+`VALIDATE`、`lock_timeout`、INVALID 索引自愈 | P1 | 是 | 缺失；且 `CONCURRENTLY` 不能与事务级 advisory lock 同批，需要专用连接上的 session lock |
| E5 | 版本/迁移工具 | `server_version_num` 门控（PG18 的 NOT NULL/generated 差异）；迁移历史/print-schema/影子库 | P2 | 是 | 缺失；迁移历史等与既有 non-goal 冲突，默认只留外部工具接口 |

### F. 内置 CRUD/Action/Router

| 编号 | 能力 | 说明 | P | PG 特有 | yang-base 现状 |
|---|---|---|---|---|---|
| F1 | 内置 CRUD 去门控 | `action/builtin`、`sql_bridge`、`ModuleSpec::crud`、`Tables`、`ActionContext` 在 PG 下可用；`InsertResult` 主键泛化 | P0 | 否 | 缺失。`builtin/mod.rs:2`、`sql_bridge.rs:2`、`spec.rs:540-564`、`tables.rs:150-299`、`context.rs:383,469` 均 cfg mysql |
| F2 | 深模块/自定义 Action | 分页/树/关系编排与自定义 Action 走同一查询链路 | P1 | 否 | 部分。纯编排逻辑可复用，执行层缺失 |
| F3 | Router/定义 DSL | Router、definition builder、`WhereCondition` 方言无关 | 已具备 | 否 | 已具备。`router/mod.rs`、`definition/field.rs:994-999`、`query_params.rs:101-104` |

### G. 安全与错误映射

| 编号 | 能力 | 说明 | P | PG 特有 | yang-base 现状 |
|---|---|---|---|---|---|
| G1 | SQLSTATE 与能力错误 | `ErrorKind{Unique/FK/NotNull/Check/Serialization/Deadlock/QueryCanceled/InFailedSQLTransaction/ConnectionLost}`；`UnsupportedCapability` 显式返回 | P0 | 部分 | 部分/缺失。`error/mod.rs:393-420` 靠英文报文；`:1430-1439` 有 PG 23505 单测；无 `UnsupportedCapability` |
| G2 | 约束元数据 | `constraint`/`columns`/`detail`；上层据此返回 409/422/503 | P1 | 否 | 部分。`duplicate_key_field` 只截键名 |
| G3 | 可重试与中止态 | `is_retryable`；40001/40P01 重试；25P02 事务中止跟踪；22001/22003 映射 | P1 | 是 | 缺失。`DbError→BaseError` 未分类 |
| G4 | 绑定与引用 | 值只经 Bind；标识符白名单；日志脱敏 | P1 | 否 | 部分。值绑定已参数化；标识符引用绑定 MySQL 方言 |

### H. 测试/CI/文档/契约

| 编号 | 能力 | 说明 | P | PG 特有 | yang-base 现状 |
|---|---|---|---|---|---|
| H1 | 测试与 CI 门禁 | PG 单测/集成测试、双后端契约测试、`Mock`/`test_transaction`、CI feature 矩阵与 docker-postgres | P0 | 否 | 缺失。`tests/table_query_crud_test.rs:19` 等直接用 `MySqlPoolOptions`；`ci.yml:145-147` 无 `yang-base-postgres`；`:234-262` 只跑 yang-db |
| H2 | 文档与能力声明 | README/AGENTS/lib.rs feature 表；能力矩阵增加「base 接入状态」列 | P0 | 否 | 部分/风险。README 仅 mysql；`docs/reference/BASE_DB_CAPABILITY_MATRIX.md:18` 联合矩阵写「PostgreSQL 16 与 MySQL 对称」，易被读成 base 已支持 |
| H3 | 数据迁移与 semver | MySQL→PG 数据转换/校验/切流/回滚；breaking API 的弃用窗口与升级指南 | P1 | 否 | 缺失；可声明 non-goal，但必须写清外部替代路径 |
| H4 | 性能与前端契约 | 双后端微基准、EXPLAIN 回归、OpenAPI 3.1/UI Catalog/UUID/array 表单控件 | P2 | 否 | 缺失/部分。JSON Schema 有基础，但新类型契约未覆盖 |

## 6. 缺口矩阵

> P 级与第 5 节一致；只列缺失/部分项。证据行号以当前工作区为准，重构后会漂移。

### P0（15 项，发布准入线）

| 编号 | 缺口 | 类型 | 关键证据（file:line） | 影响 |
|---|---|---|---|---|
| P0-01 | 运行时底座：feature/句柄/执行器/配置/Tools | 绝对缺失 | `Cargo.toml:26,35,60,106`；`definition.rs:1040,1116`；`table_query/mod.rs:139`；`build.rs:25`；`config.rs:34,38-39`；`tools.rs:69-70,114-125,203-216,251-254`；`initializer.rs:3`；`database/mod.rs:22-33` | PG-only 构建不存在 `TableHandle`/`TableQuery`/`DatabaseInitializer`；运行期无法持有 PG 池 |
| P0-02 | `FieldType`→PG 类型映射与未知类型拒绝 | 硬编码 | `render.rs:298-346,316,319`；`field_type.rs:58-244` | `CREATE TABLE` 产出 `TINYINT(1)`/`DATETIME`/`ENUM`；PG 语法或语义错误 |
| P0-03 | 标识符规则与字面量转义 | 硬编码+不对称 | `render.rs:415-419,431,437`；`yang-db/src/sql_types.rs` 单段 `TableRef` | 反引号/64 字符/反斜杠双写在 PG 下错误；`schema.table` 无法表达 |
| P0-04 | `auto_increment`→identity 与读回 | 硬编码 | `render.rs:354`；`schema_validation.rs:44-53,146-164`；`definition.rs:112-119` | `AUTO_INCREMENT` 非法；serial/identity 表每次启动误判 `AutoIncrementMismatch` |
| P0-05 | `Record` PG 解码与 JSON/NULL 契约 | 绝对缺失 | `record.rs:185,268-361`；`read.rs:50,169,202,260` | PG 行无法转 `Record`；select/paginate/关联/内置 Action 读路径全部不可用；SQL NULL 与 jsonb null 混淆 |
| P0-06 | 查询计划/执行器/`field_types`/NULL 归一 | 绝对缺失+缺陷 | `plan.rs:3,24-29,85-94,101-111,113-190`；`dialect.rs:170-183` | 无 PG QueryBuilder 入口；`= NULL` 静默 0 行；日期/JSON/Decimal/bytea 按默认 OID 绑定可能 `operator does not exist` |
| P0-07 | CRUD 返回契约与冲突目标 | 绝对缺失 | `write.rs:100,118-129,180-199,204-232`；`add.rs:18-28` | `last_insert_id` 在 PG 不存在；UUID/text/numeric/非 `id` 主键无法回填；PG 无批量 upsert |
| P0-08 | 守卫全覆盖与 schema 限定 | 部分+硬编码 | `plan.rs:186`；`relation_loader.rs:81-113`；`yang-db/src/reference.rs` | 关联加载绕过租户/软删；PG schema 隔离不可用，安全边界与 MySQL 不对称 |
| P0-09 | schema_sync 反射与中立模型 | 绝对缺失 | `inspect.rs:51-165`；`model.rs:37-46`；`schema_validation.rs:5-18`；`plan.rs:11,15` | PG 启动同步不可用；即使替换内省，`column_type`/自增判定仍误报 |
| P0-10 | PG DDL 渲染 | 绝对缺失 | `render.rs:98,113,311-346,354`；`plan.rs:253` | `ENGINE=InnoDB`/内联 KEY/`AUTO_INCREMENT`/`MODIFY COLUMN` 在 PG 直接失败 |
| P0-11 | 事务化 DDL/advisory lock/预检/dry-run | 绝对缺失 | `sync.rs:132,140-157`；`preflight.rs:43-105` | `GET_LOCK`/`DATABASE()` 不存在；无失败回滚与演练；`CAST AS CHAR` 截断主键 |
| P0-12 | 内置 CRUD/Action 去门控与 `InsertResult` | 绝对缺失 | `builtin/mod.rs:2`；`sql_bridge.rs:2`；`spec.rs:540-564`；`tables.rs:150-299`；`context.rs:383,469` | PG-only 构建没有默认可注册 CRUD；表查询/事务方法消失 |
| P0-13 | SQLSTATE 结构化与 `UnsupportedCapability` | 部分/绝对缺失 | `error/mod.rs:393-420,1430-1439`；`yang-db/src/error.rs:163-186` | 只能靠英文报文猜唯一键；FK/NOT NULL/CHECK/序列化失败无法分流；能力缺失退化为错误 SQL 或静默 |
| P0-14 | PG 测试与 CI 门禁 | 绝对缺失 | `ci.yml:145-147,234-262`；`tests/table_query_crud_test.rs:19`；`tests/schema_sync_integration_test.rs:38` | 无 PG 回归证据；任何「支持 PG」声明不可验收 |
| P0-15 | 文档与能力矩阵修正 | 部分/风险 | `README.md:20,55-56`；`AGENTS.md:76,85`；`src/lib.rs:19`；`docs/reference/BASE_DB_CAPABILITY_MATRIX.md:18` | 外部读者可能把 yang-db 的 PG 能力误读为 yang-base 已支持；缺少限制与接入状态 |

### P1（19 项，成熟框架能力）

| 编号 | 缺口 | 类型 | 关键证据（file:line） | 影响 |
|---|---|---|---|---|
| P1-01 | 统一事务/savepoint/retry/请求级传播 | 缺失 | `context.rs:469`；无 savepoint/retry API | 多语句 Action 无法跨后端；40001 不能重试；嵌套回滚不可表达 |
| P1-02 | 会话初始化与连接池配置 | 缺失 | 无 `after_connect`；`config.rs` 仅 URL | `search_path`/`TimeZone`/SSL/`application_name` 不可控；schema 租户难实现 |
| P1-03 | 能力注册表与启动 fail-fast | 缺失 | `yang-db/src/capability.rs:87-104` 未被调用 | 能力差异到运行期才由错误 SQL 暴露 |
| P1-04 | 边界语义与上限 | 部分/不对称 | `field_type.rs:9`；`schema_validation.rs:238-284` | PG 下 `varchar` 上限误拒；bool 默认值 `1/0` 与 PG `TRUE/FALSE` 不兼容 |
| P1-05 | 专属类型层与扩展类型 | 缺失 | `field_type.rs:58-244`；`Cargo.toml:35,85,100` | UUID/bytea/array 无法声明；postgres-only 构建缺 base64/uuid 接线 |
| P1-06 | Enum/CHECK/生成列/注释 | 部分 | `render.rs:147-215`；`model.rs` 无相关字段 | CHECK 读回与 PG 语法不同；生成列不可只读保护；注释不迁移 |
| P1-07 | Collation/索引名/JSON Schema | 部分 | `render.rs:415-419` | 大小写不敏感唯一约束无法表达；索引名超 63 字节冲突；前端约束与数据库不一致 |
| P1-08 | 未知 OID 与扩展 decoder | 缺失 | `record.rs` 无注册入口 | pgvector/citext/hstore 等扩展类型无法解码 |
| P1-09 | 复合类型解码 | 缺失 | `record.rs` 无 array/range/composite 分支 | 数组/范围/复合/interval 无法输出 JSON |
| P1-10 | 列上下文与写路径反编码 | 缺失 | `record.rs` 无列元数据；`plan.rs` 不注入类型 | 宽表/JOIN 解码错误难定位；写路径 cast 依赖服务端推断 |
| P1-11 | LIKE/ILIKE 与 collation 策略 | 部分/不对称 | `filters.rs:188-206,536-588`；`dialect.rs:253-257` | 同一 `search` 在 MySQL（`_ci`）与 PG（`LIKE` 敏感）结果不同 |
| P1-12 | 排序/分页一致性 | 部分 | `plan.rs` 无 NULLS；`read.rs` COUNT 与 SELECT 分离 | 页序不稳定；`total` 与 `items` 快照不一致；深 OFFSET 无警告 |
| P1-13 | 事务/语句语义 | 部分 | `context.rs:469`；无 `statement_timeout` | 长查询无法取消；隔离级别/只读/行锁不可配 |
| P1-14 | 在线 DDL | 缺失 | `sync.rs` 无 concurrent/NOT VALID | 大表加索引/约束长时间阻塞；INVALID 索引无自愈 |
| P1-15 | Tables 深模块与自定义 Action | 部分 | `tables.rs:150-299` | 分页/树/关系编排在 PG 下不可用 |
| P1-16 | 约束元数据与业务分流 | 部分 | `error/mod.rs:400-420` | 只拿键名；FK/NOT NULL/CHECK 无法映射到 409/422 |
| P1-17 | 可重试与中止态 | 缺失 | `error/mod.rs` 无 `is_retryable` | 40001/40P01 不可重试；25P02 产生级联错误 |
| P1-18 | 绑定与引用安全 | 部分 | 值绑定已参数化；标识符仅 MySQL 方言 | 安全边界依赖方言引用实现，跨后端不一致 |
| P1-19 | 数据迁移与 semver | 缺失 | 无迁移工具；无公开 API 升级策略 | 存量 MySQL 迁 PG 无路径；`InsertResult.id` 等改动会 breaking |

### P2（6 项，进阶与边界）

| 编号 | 缺口 | 类型 | 关键证据（file:line） | 影响 |
|---|---|---|---|---|
| P2-01 | 可观测性细化 | 缺失 | `yang-runtime/src/observability/telemetry.rs:382-386` | PG 池、慢查询、SQLSTATE 指标不可见 |
| P2-02 | 多后端同时在线/动态路由 | 缺失，需求待验证 | 无 `DatabaseRouter`；公开类型未定 `#[non_exhaustive]` | 过早实现会把 P0 从 15 项膨胀到数十项 |
| P2-03 | 默认表达式白名单 | 缺失 | `field_config.rs:37` | `now()`/`gen_random_uuid()` 无法声明；只能写死字面量 |
| P2-04 | RLS/`SET LOCAL`/keyset/窗口 COUNT | 缺失，边界待定 | 无；与 GRANT/RLS non-goal 冲突 | 高阶多租户安全能力无入口；需 RFC 决定策略管理归属 |
| P2-05 | 版本门控与迁移工具 | 缺失，边界待定 | 无 `server_version_num`；无迁移历史 | PG18 差异未门控；迁移 CLI/checksum 与 non-goal 冲突 |
| P2-06 | 性能/前端契约 | 缺失/部分 | 无基准；JSON Schema 未覆盖 uuid/array/bytea | 迁移后可能静默劣化；表单/OpenAPI 契约不完整 |

## 7. 关键架构决策（含取舍建议）

### 7.1 后端选择：enum wrapper vs trait object

**建议：公开 `#[non_exhaustive] struct AnyDatabase { inner: AnyDatabaseKind }`（Clone、Send+Sync），内部私有 `enum AnyDatabaseKind { MySql(..), Postgres(..) }`；另定义 `pub(crate) sealed trait Executor` 做静态分派。**  
理由：async trait + `QueryBuilder<'a>` 生命周期 + `FromRow<R>` 泛型使公开 trait object 难对象安全且丢失类型信息；两套平行 base 会让权限/软删/租户规则漂移；`sqlx::Any` 抹掉 `RETURNING`/JSONB/数组/SQLSTATE，禁止作为统一层。SeaORM 从公开 enum 演进为 `non_exhaustive` struct + 内部 enum，说明公开枚举必须留演进空间；Diesel 的 `Database<B: Backend>` 零成本但要求编译期固定后端，只作 P2 特化。

### 7.2 单例 Tools 如何容纳多后端

**建议：首版只持有一个 `Option<AnyDatabase>`，`Tools::database()`/`health_check()`/`close()` 走同一生命周期；不新增 `postgres` 专用槽。** 若未来出现真实同进程双后端消费者（**待验证**），再增加 `BackendRegistry` 按 `BackendKind` 路由，并明确跨后端 JOIN/事务直接拒绝。现状 `Tools::extension`（`tools.rs:162,340`）可临时存放 PG 资源，但没有健康检查/关闭语义，不能当作契约。

### 7.3 FieldType：可移植子集还是 PG 专属变体

**建议：核心 `FieldType` 保持可移植子集（String/Integer/BigInt/Float/Double/Decimal/Boolean/Date/DateTime/Timestamp/Json/Text/Enum）；PG 专属类型（Uuid/Bytea/Array/Interval/Range/原生 Enum）放入 `#[cfg(feature = "postgres")]` 的 `PgFieldType` 或嵌套变体。** `Timestamp` 保持 `bigint`（Unix 秒），`DateTime` 映射 `timestamptz`；需要无时区墙上时间时新增显式变体，禁止同一变体静默切换。构建期按 feature 校验在 `mysql+postgres` 同时开启时不可判定，应把方言校验推迟到 `bind`/`schema_sync`，或给 `TableDefinition` 增加显式 target backend。未知类型必须显式报错，禁止静默降级为 `text`。

### 7.4 Record 解码：类型注册表

**建议：`OID → decoder` 注册表 + `PgTypeKind` 递归分派；binary 优先、text 兜底；未知 OID 返回带 `column/ordinal/oid/type_name` 的结构化错误，绝不静默置 Null；提供 `RecordDecoder::register(oid, fn(&PgValueRef) -> Result<Value>)` 扩展点。** 共享一个 JSON 归一器：bool→bool、int→number、numeric→十进制字符串、timestamptz→UTC RFC3339、bytea→Base64、数组→JSON Array、SQL NULL 与 jsonb 字面量 `null` 用 `null_kind` sidecar 区分。类型化 `FromRow` 双轨保留为内部/P2，不作为首版公开 API。

### 7.5 schema_sync 模型如何方言化

**建议：纯 planner（desired vs actual → change list）+ `SchemaDriver { introspect, render, lock, preflight, ddl_transactional }`；共享 `ExistingTableSchema` 枚举模型；MySQL driver 保留 `information_schema`/`GET_LOCK`，PG driver 用 `pg_catalog`/`pg_advisory_lock`。** 不要把 1,659 行逐文件复制成两套，也不要在每个调用点散落 `match backend`。`SchemaSyncReport`/`SchemaPreflightReport` 契约保持不变，只替换执行体。

### 7.6 feature 组合如何避免爆炸

**建议：`db` 元 feature + `mysql`/`postgres` 子 feature；`default` 保留 `mysql` 以兼容；核心模块（定义、计划、守卫）始终编译，后端实现按 feature 裁剪；`mysql+postgres` 必须同时编译通过。** CI 矩阵至少覆盖 `none`、`mysql`、`postgres`、`mysql+postgres`；`--all-features` 与 MSRV 也要覆盖。`base64` 与 `uuid` 当前分别只在 `mysql`/`token` feature 下启用，PG 的 `bytea`/`uuid` 需要独立接线。公开 enum 的变体若随 feature 条件编译会破坏下游 match 形态，这是选择「struct + 内部 enum」而非公开 enum 的另一理由。

### 7.7 错误契约如何跨层

**建议：yang-db 保留 SQLSTATE 与约束名，base 映射为 `ErrorKind` + `constraint/columns/detail` + `is_retryable()`；能力缺失返回 `UnsupportedCapability { backend, capability, suggestion }`；禁止继续用英文报文做主要判定。** `duplicate_key_field` 只保留为兼容兜底并标注不可依赖。23503/23502/23514 映射 409/422，40001/40P01 可重试，25P02 立即标记事务不可用，57014 映射独立 Timeout。

## 8. 三阶段路线图

### Phase A：打通 PG 单栈最小闭环

| 步骤 | 内容 | 可测试验收标准 |
|---|---|---|
| A0 | MySQL-only 抽象重构：`AnyDatabase`（MySQL 单变体）、`Statement`、sealed `Executor`、`AnyTransaction` | 现有 MySQL 单测/集成测试与 CI 全绿；公开 API 除类型别名外零 breaking；`TableQuery` 行为快照不变 |
| A1 | `postgres` feature + `EngineConfig` URL scheme + `Tools` 资源槽 + 连接/健康检查/关闭 | `cargo check -p yang-base --no-default-features --features postgres` 通过；PG 容器能 connect/health/close |
| A2 | `Record` PG 解码 + 13 变体类型映射 + identifier/字面量转义 | 13 个 `FieldType` 的 round-trip 测试；非法标识符/超长名/单引号字面量测试；未知类型返回 ConfigError |
| A3 | 查询计划/读/写/`insert_returning`/`upsert(conflict_columns)` | PG CRUD、分页、软删、租户、事务回滚集成测试；`INSERT ... RETURNING` 回填 UUID/text/非 `id` 主键 |
| A4 | 内置 CRUD 去门控 + `ActionContext` + `Tables` | `ModuleSpec::crud()` 在 PG-only 构建可用；内置 add/get/put/del/select 契约测试通过 |
| A5 | PG 测试 + 双后端契约 + CI + 文档 | CI 新增 `yang-base-postgres`/`yang-base-all` cell 与 `docker-postgres` 作业；能力矩阵出现「base 接入状态」列 |

**Phase A 退出条件**：`--no-default-features --features postgres` 与 `--features mysql,postgres` 都能编译并通过契约测试；同一套 CRUD/分页/软删/租户用例在 MySQL 8 与 PG 16 容器上结果一致；MySQL 回归零变化。

### Phase B：达到成熟框架能力

| 步骤 | 内容 | 可测试验收标准 |
|---|---|---|
| B1 | schema_sync PG：反射/渲染/plan/preflight/advisory lock/事务化 DDL/dry-run | PG 16 容器首次建表、二次同步 no-op、加列/索引/CHECK/FK 成功；两实例并发启动被串行化；dry-run 后系统目录无痕迹；中途失败整体回滚 |
| B2 | 事务语义：savepoint/retry/`statement_timeout`/隔离级别/只读/行锁 | 40001/40P01 重试测试；57014 映射 Timeout；savepoint 回滚内层不影响外层；`FOR UPDATE SKIP LOCKED` 并发测试 |
| B3 | 查询语义：ILIKE、NULLS、确定性分页、COUNT 同快照、EXISTS、`ANY(array)` | 双后端结果集对比；分页无重复/遗漏；`total` 与 `items` 一致；大小写策略显式测试 |
| B4 | 守卫/关联/可观测性：关联加载复用目标表守卫、backend 标签、池指标 | 软删/租户/字段权限在关联加载生效；tracing/metrics 带 `backend=postgres`；PG 池指标可查 |

**Phase B 退出条件**：PG 启动同步可重复、可回滚、可演练；事务与查询语义有双后端契约测试；身份/租户/软删守卫在 coverage 报告中没有绕过出口。

### Phase C：进阶与差异化

| 步骤 | 内容 | 可测试验收标准 |
|---|---|---|
| C1 | 新类型：Uuid/Blob/Array + 扩展类型注册（citext/pgvector 等） | uuid/bytea/array round-trip；未注册扩展类型返回含 OID 的错误；扩展存在性检查纳入 schema_sync |
| C2 | RLS/`SET LOCAL search_path`（仅 RFC 通过后） | 非 superuser/非 owner 角色下租户隔离测试；策略创建/变更/删除可逆；否则明确标记 non-goal |
| C3 | 多后端同时在线/动态路由（仅有真实消费者后） | 双后端编译与运行时路由测试；跨后端 JOIN/事务显式拒绝 |
| C4 | 迁移/切换与 semver 衔接 | MySQL→PG 数据迁移与校验报告（或声明 external）；公开 breaking 变更的弃用窗口与升级指南 |

**Phase C 退出条件**：新增能力不破坏 MySQL 行为；所有 P0 项有真库证据；文档与能力矩阵与实际实现一致。

## 9. 非目标与与 yang-db 报告的分工边界

**与 yang-db 报告的分工**：yang-db 负责「驱动/方言层」——占位符、连接池、事务原语、`RETURNING` 机制、`ON CONFLICT` builder、SQLSTATE 保留、typed NULL、参数上限、identifier API；yang-base 负责「应用框架层」——`FieldType` 映射、`Record` 解码、查询计划、schema sync、Action/守卫、CI/文档。本报告不重复 yang-db 的 `$N`、池、事务、`RETURNING` 细节，只在 P0 依赖处引用。

**明确 non-goal（沿用仓库既有口径，含本轮新增边界）**：

- SQLite/MSSQL：无 feature、无驱动承诺。
- backup/restore/database-create：部署运维层能力。
- 通用 ORM/任意 SQL AST、逐方法兼容 br-db。
- 迁移 CLI/checksum runner/自动 diff/print-schema/影子库：默认不进本 crate；如需，必须另立 RFC，不与「支持 PG」捆绑。
- GRANT/RLS 策略管理：默认不管理；base 只提供 `SET LOCAL`/策略输入。若实现 RLS，需把该 non-goal 改掉。
- 触发器：默认不生成 `BEFORE UPDATE`；`updated_at` 由框架写路径维护。
- 原生 `CREATE TYPE ... AS ENUM`：默认用 `varchar + CHECK`；原生 enum 作为 P2 可选路径。
- `xmax` 区分 upsert 的 inserted/updated：系统列、非官方语义，不写入契约；批量 upsert 只承诺 `attempted/affected/returned_rows`。
- MySQL→PG 数据迁移工具：若不做，必须写清外部替代路径与验收口径。
- 同进程双后端同时在线：仅有真实消费者时才做。

**已识别的报告冲突与处理**：① `BASE_DB_CAPABILITY_MATRIX.md:18` 的联合矩阵需加「yang-base 接入状态」列，避免把 yang-db 的 PG 能力误读为 base 已支持；② 软删部分唯一索引会让 MySQL/PG 行为不同，需二选一（统一状态列参与唯一键，或明确记录差异并双后端测试）；③ MySQL `DATETIME` 输出无小数秒/无时区，PG `timestamptz` 要求 UTC RFC3339，二者不能同时声称「双后端一致」，需在 Phase A 决定是否 breaking 改 MySQL 输出；④ `Timestamp` 保持 `bigint` 与 `DateTime` 映射 `timestamptz` 的边界必须在文档中写死。

## 10. 参考来源

**PostgreSQL 官方文档（16/17/18）**

- https://www.postgresql.org/docs/16/datatype.html
- https://www.postgresql.org/docs/16/datatype-character.html
- https://www.postgresql.org/docs/16/datatype-numeric.html
- https://www.postgresql.org/docs/16/datatype-datetime.html
- https://www.postgresql.org/docs/16/datatype-json.html
- https://www.postgresql.org/docs/16/datatype-binary.html
- https://www.postgresql.org/docs/16/datatype-uuid.html
- https://www.postgresql.org/docs/16/datatype-enum.html
- https://www.postgresql.org/docs/16/arrays.html
- https://www.postgresql.org/docs/16/rangetypes.html
- https://www.postgresql.org/docs/16/rowtypes.html
- https://www.postgresql.org/docs/16/ddl-constraints.html
- https://www.postgresql.org/docs/16/ddl-identity-columns.html
- https://www.postgresql.org/docs/16/ddl-generated-columns.html
- https://www.postgresql.org/docs/16/ddl-schemas.html
- https://www.postgresql.org/docs/16/ddl-rowsecurity.html
- https://www.postgresql.org/docs/16/sql-createtable.html
- https://www.postgresql.org/docs/16/sql-altertable.html
- https://www.postgresql.org/docs/16/sql-createindex.html
- https://www.postgresql.org/docs/16/sql-insert.html
- https://www.postgresql.org/docs/16/sql-select.html
- https://www.postgresql.org/docs/16/sql-update.html
- https://www.postgresql.org/docs/16/sql-savepoint.html
- https://www.postgresql.org/docs/16/sql-set.html
- https://www.postgresql.org/docs/16/sql-set-transaction.html
- https://www.postgresql.org/docs/16/sql-syntax-lexical.html
- https://www.postgresql.org/docs/16/functions-matching.html
- https://www.postgresql.org/docs/16/functions-admin.html
- https://www.postgresql.org/docs/16/functions-info.html
- https://www.postgresql.org/docs/16/functions-datetime.html
- https://www.postgresql.org/docs/16/functions-window.html
- https://www.postgresql.org/docs/16/transaction-iso.html
- https://www.postgresql.org/docs/16/errcodes-appendix.html
- https://www.postgresql.org/docs/16/protocol-message-formats.html
- https://www.postgresql.org/docs/16/protocol-flow.html
- https://www.postgresql.org/docs/16/runtime-config-client.html
- https://www.postgresql.org/docs/16/runtime-config-preset.html
- https://www.postgresql.org/docs/16/runtime-config-compatible.html
- https://www.postgresql.org/docs/16/infoschema-columns.html
- https://www.postgresql.org/docs/16/catalog-pg-class.html
- https://www.postgresql.org/docs/16/catalog-pg-attribute.html
- https://www.postgresql.org/docs/16/catalog-pg-constraint.html
- https://www.postgresql.org/docs/16/catalog-pg-index.html
- https://www.postgresql.org/docs/16/catalog-pg-attrdef.html
- https://www.postgresql.org/docs/16/catalog-pg-description.html
- https://www.postgresql.org/docs/16/catalog-pg-type.html
- https://www.postgresql.org/docs/16/pgstatstatements.html
- https://www.postgresql.org/docs/16/sql-explain.html
- https://www.postgresql.org/docs/18/catalog-pg-constraint.html
- https://wiki.postgresql.org/wiki/Transactional_DDL_in_PostgreSQL:_A_Competitive_Analysis

**驱动与框架实现对标**

- https://docs.rs/sqlx/latest/sqlx/trait.FromRow.html
- https://docs.rs/sqlx/latest/sqlx/trait.Row.html
- https://docs.rs/sqlx/latest/sqlx/postgres/types/index.html
- https://docs.rs/sqlx/latest/sqlx/postgres/enum.PgTypeKind.html
- https://docs.rs/sqlx/latest/sqlx/postgres/struct.PgTypeInfo.html
- https://docs.rs/sqlx/latest/sqlx/postgres/struct.PgValueRef.html
- https://docs.rs/sqlx/latest/sqlx/postgres/struct.PgColumn.html
- https://docs.rs/sqlx/latest/sqlx/postgres/struct.PgConnectOptions.html
- https://docs.rs/sqlx/latest/sqlx/macro.migrate.html
- https://docs.rs/sea-orm/latest/sea_orm/trait.ConnectionTrait.html
- https://docs.rs/sea-orm/latest/sea_orm/struct.Statement.html
- https://docs.rs/sea-orm/latest/sea_orm/struct.DatabaseTransaction.html
- https://docs.rs/sea-orm/latest/sea_orm/struct.QueryResult.html
- https://docs.rs/diesel/latest/diesel/backend/trait.Backend.html
- https://docs.rs/diesel/latest/diesel/connection/trait.Connection.html
- https://docs.sqlalchemy.org/en/20/core/engines.html
- https://docs.sqlalchemy.org/en/20/core/type_basics.html
- https://docs.sqlalchemy.org/en/20/orm/queryguide/dml.html
- https://docs.djangoproject.com/en/5.1/topics/db/multi-db/
- https://docs.djangoproject.com/en/5.1/ref/models/querysets/
- https://guides.rubyonrails.org/active_record_querying.html
- https://api.rubyonrails.org/v7.1.2/classes/ActiveRecord/Persistence/ClassMethods.html
- https://www.prisma.io/docs/reference/api-reference/prisma-schema-reference
- https://www.prisma.io/docs/orm/prisma-schema/data-model/multi-schema
- https://www.prisma.io/docs/orm/prisma-migrate/understanding-prisma-migrate/shadow-database
- https://alembic.sqlalchemy.org/en/latest/autogenerate.html
- https://docs.rs/testcontainers-modules/latest/testcontainers_modules/
- https://json-schema.org/draft/2020-12/json-schema-validation.html
- https://doc.rust-lang.org/cargo/reference/features.html

**本地证据（绝对路径）**

- `D:\code\lib_yang\crates\yang-base\Cargo.toml`
- `D:\code\lib_yang\crates\yang-base\src\table\definition.rs`
- `D:\code\lib_yang\crates\yang-base\src\table\record.rs`
- `D:\code\lib_yang\crates\yang-base\src\table\table_query\`
- `D:\code\lib_yang\crates\yang-base\src\database\schema_sync\`
- `D:\code\lib_yang\crates\yang-base\src\{tools,config}.rs`
- `D:\code\lib_yang\crates\yang-base\src\action\context.rs`
- `D:\code\lib_yang\crates\yang-base\tests\`
- `D:\code\lib_yang\.github\workflows\ci.yml`
- `D:\code\lib_yang\docs\reference\BASE_DB_CAPABILITY_MATRIX.md`
