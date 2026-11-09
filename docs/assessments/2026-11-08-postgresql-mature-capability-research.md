# PostgreSQL 成熟能力调研：yang-db 的现状、缺口与路线图

**日期**：2026-11-08  
**范围**：`crates/yang-db` PostgreSQL 后端（`postgres` feature）  
**配套报告**：[`2026-11-08-yang-base-postgresql-capability-research.md`](2026-11-08-yang-base-postgresql-capability-research.md)（应用框架层 `crates/yang-base`；本报告只覆盖驱动/方言层）  
**基线**：PostgreSQL 16（以 17 补充版本差异）；sqlx 0.8.x  
**一句话结论**：已有可用的单表 CRUD/事务闭环，但距离“成熟 PG 基础库”仍差一个能力层，且正确性缺口集中在错误契约、RETURNING、表达式、NULL 绑定与类型映射。

## 1. 摘要

先澄清一个常见误判：yang-db **并非没有 PostgreSQL 支持**。`Cargo.toml` 的 `postgres` feature 默认启用并接入 `sqlx/postgres`；`src/postgres/` 已具备 identifier、field、condition、query_builder、database、transaction、init 七个模块；公开 API 覆盖 `$N` 占位符、单表 CRUD、批量 INSERT/UPDATE、`RETURNING`（受限）、`ON CONFLICT`（受限）、事务与隔离级别、两种行锁、连接池管理面，以及两个 PG 集成测试与一个示例。它的问题不是“从零开始”，而是“**有基础闭环，成熟度显著低于 MySQL，且若干 P0 语义是错的或不可表达的**”。

三个结构性观察：

1. **对称性已破**：MySQL 的 `query_builder` 已拆为 8 个子模块（连同 `mod.rs` 共 9 个文件），PG 仍是 3167 行单文件；MySQL 有 `select_expr/set_expr/where_expr/upsert_batch/upsert_batch_with_size/read_only_transaction`，PG 没有。能力声明 `POSTGRES_OPERATIONS` 也明确不声明 `BatchUpsert`，并有测试固定这一现状。
2. **正确性缺口比能力缺口更危险**：`SqlValue::Null` 在 WHERE 路径绑定为 `int4`，对 text/timestamptz/uuid 列会报 `operator does not exist`；`insert` 固定 `RETURNING CAST(<col> AS BIGINT)`，UUID/text/numeric 主键直接失败；`NUMERIC` 走 f64/String，超过 2^53 丢精度；SQLSTATE 只保留 message，40001/40P01 被归为不可重试。
3. **测试门禁不足**：CI 只跑 `integration_pg_crud_simple` 和 `integration_pg_transaction`；`integration_advanced_queries.rs` 的 4 个 PG 用例全部 `#[ignore]`；CRUD 测试的连接/建表失败仅 `println`，count/update/delete 断言包在 `if let Ok` 内，upsert 只断言 `is_ok`。这使“已支持”缺少回归证据。

本次调研结论：把 PG 做到“成熟基础库”需要三层工作——先修正确性与对称性（Phase A），再补 PG 特有常用能力（Phase B），最后做进阶与差异化（Phase C）。P0 缺口 11 项、P1 28 项、P2 11 项（详见第 5 节）。

### Top 8 最高优先级缺口

| # | 缺口 | P | 一句话影响 |
|---|---|:--:|---|
| 1 | SQLSTATE 结构化丢失；40001/40P01/57014/25P02/55P03 未分流 | P0 | 无法安全重试，无法识别事务中止/锁冲突/取消 |
| 2 | RETURNING 只支持 insert 单列 `CAST(... AS BIGINT)` | P0 | UUID/text/numeric 主键与非 id 主键表无法回填；update/delete/upsert 无返回行 |
| 3 | `select_expr/set_expr/where_expr` 全缺，`SqlExpr::postgres_render` 为 dead code | P0 | 服务端表达式、计算列、`NOW()` 类写入无法表达 |
| 4 | WHERE typed NULL 绑成 int4 | P0 | `where_and(field, Eq, None)` 在非整型列上直接报错 |
| 5 | NUMERIC/timestamptz/UUID 值模型缺失或降级 | P0/P1 | 金额丢精度、时区语义丢失、UUID 主键无法插入 |
| 6 | ON CONFLICT 形状窄且默认冲突列 `["id"]` | P0 | 非 id 唯一键、条件 upsert、部分唯一索引无法表达 |
| 7 | schema 限定、search_path、标识符边界缺失 | P0 | 无法表达 `schema.table`；search_path 未固定有劫持风险 |
| 8 | CTE/窗口/HAVING（聚合）不可用 | P1 | 复杂报表与分组 top-N 只能裸 SQL，真实消费者已在绕过库 |

> 说明：Top 8 按“正确性优先于能力、P0 优先于 P1”排序；测试门禁与连接会话基线虽未进 Top 8，但属于 Phase A/B 的硬验收项。

## 2. 调研方法与依据

三类证据，按可信度标注：

| 等级 | 来源 | 用途 | 局限 |
|---|---|---|---|
| A（规范） | PostgreSQL 16/17 官方文档：protocol、sql-select/insert/update/delete/merge/copy、datatype-*、errcodes、libpq、runtime-config、catalogs | 定义“PG 语义上应该有什么” | 文档是能力上限，不代表某驱动已实现 |
| B（实现对标） | sqlx 0.8.x（主对照）、tokio-postgres/postgres-types、diesel、sea-orm、cornucopia、refinery；跨语言 SQLAlchemy、Prisma、Drizzle、GORM、ActiveRecord、asyncpg | 判断“成熟库实际提供什么”、API 形状与测试实践 | 库的设计目标不同，不能直接等同 |
| C（本地审计） | `crates/yang-db/src/postgres/**`、`src/capability.rs`、`src/error.rs`、`tests/**`、`.github/workflows/ci.yml`、README 与 docs/reference | 确认“当前到底有什么” | 行号会漂移；本篇只在缺口矩阵引用关键位置 |

方法：

1. 按 8 个能力域列“能力基线”（应该有什么），再逐项对本地代码标注“已支持/部分/缺失”。
2. 只把“缺口与部分项”放进缺口矩阵，按统一后的 P 级排序，并区分“对称性缺口”与“PG 特有成熟度缺口”。
3. 对抗核实：对每条审计结论做代码/文档复核；被证伪或需降级的项在文中注明（例如 QueryBuilder 的 `from_pool` 属命名差异、NEW-39 已在代码修复）。
4. 不确定项标“待验证”，不把推测写成结论。

关键事实核验（本次已确认）：

- `postgres` feature 默认启用；`src/postgres/` 文件集合与任务描述一致；`query_builder.rs` 3167 行。
- `POSTGRES_OPERATIONS` 含 `Returning`/`ExplicitConflictTarget`，不含 `BatchUpsert`，且 `capability.rs` 有测试断言 `!supports(BatchUpsert)`。
- PG 公开 builder 方法中没有 `select_expr/set_expr/where_expr/upsert_batch/upsert_batch_with_size/read_only_transaction/where_not`；`having_cond` 仅 6 种字段比较；`join/left_join/right_join` 只接受两个 `FieldRef` 的等值 ON。
- `DatabaseConfig` 有 7 个字段但只有 3 个链式 setter；`RowLock` 仅 `ForUpdate/ForShare`；`Database::from_pool` 存在，`QueryBuilder::from_pool` 不存在。
- `SqlValue` 已 `#[non_exhaustive]`、`Transaction` 已实现 `Drop`（NEW-39 描述的缺失已不存在）；`docs/BACKLOG.md` 明细仍标 ⏳，属文档时效问题。

**待验证项**（本次未在真库复现，不应作为结论）：

- NUMERIC 以 String 绑定时，对 numeric 列的赋值/比较是否稳定触发 `operator does not exist`（取决于参数 OID 与列类型解析）。
- `transaction_with_isolation` 在 BEGIN 后第二条 `SET TRANSACTION` 的额外往返与错误时机（语法正确，需集成测试确认）。
- sqlx `Json<T>` 默认按 JSONB 编码时，写入 `json` 列的具体错误形状。
- PG17 `transaction_timeout` 的终止方式与 SQLSTATE（官方文档只写“终止会话”）。

> 说明：任务给定的“已核实事实”作为硬约束；本节复核用于补充细节，不改变其结论。

## 3. 分级定义

统一口径（吸收六份调研的分层冲突后）：

| 级别 | 判定标准 | 典型例子 | 不包含 |
|---|---|---|---|
| P0 | 缺失或错误会使**基础查询/DML 语义不成立、静默出错或安全边界失效**；必须在“自称支持 PG”之前修掉 | SQLSTATE 结构化、typed NULL、RETURNING 主键回填、NUMERIC 精度、schema 限定、search_path | 只是“更方便”的 API、进阶语法 |
| P1 | **成熟完整的 PG 基础库应当提供**的常用能力；缺失不影响最小 CRUD，但会让真实业务频繁绕裸 SQL | CTE、窗口、COPY、SAVEPOINT、SKIP LOCKED、数组/UUID/JSONB、TLS 证书、会话超时 | 仅少数场景需要的扩展/运维能力 |
| P2 | **进阶、版本相关或扩展生态**能力；按消费者需求与版本门控逐步提供 | MERGE RETURNING、2PC、表锁、range/multirange、PostGIS/pgvector、EXPLAIN 集成 | 纯 DBA/集群运维 |

分层原则：

- 以“语义正确性”优先于“API 数量”；同样缺失时，会静默出错的排 P0，会显式报错的排 P1。
- 跨报告冲突按本表统一。例如 ARRAY/UUID/JSONB 在原调研中被列为 P0，本文降为 P1；SAVEPOINT 统一 P1；RANGE/MULTIRANGE 统一 P2；事务重试与 `server_version_num` 升为 P1。
- 迁移 runner/checksum/CLI 属 non-goal（见第 7 节），不进入 P0/P1 计数；DDL 只保留“逃生舱质量”层面的 P1。

## 4. 能力基线全景

> 阅读方式：P 级是“成熟 PG 基础库应该有的优先级”；状态是“yang-db 当前实现”。“部分”表示有入口但形状/类型/安全边界不足。

### 4.1 查询语言与 DML

| 能力 | 说明 | P | PG 特有 | yang-db 现状 |
|---|---|:--:|:--:|---|
| `$N` 占位符与参数重排 | 谓词/子查询/集合展开后编号连续；单语句不超过 65535 参数 | P0 | 是 | 已支持（基础）；`insert_batch` 不按列数缩批，宽表可能超限 |
| SELECT 投影表达式 + AS | 算术、CASE/COALESCE、函数、cast、别名 | P0 | 否 | 缺失（仅 FieldRef 与 5 种白名单聚合） |
| WHERE 布尔树 AND/OR/NOT | 任意嵌套、按树加括号、值全绑定 | P0 | 否 | 部分（AND/OR/IN/EXISTS/BETWEEN；无 NOT） |
| ORDER BY 多列/NULLS | ASC/DESC、NULLS FIRST/LAST；PG 默认 ASC NULLS LAST | P0 | 是 | 部分（多列 ASC/DESC；无 NULLS 控制） |
| LIMIT/OFFSET/FETCH | 参数化、LIMIT ALL、FETCH FIRST/WITH TIES | P0 | 是 | 部分（u64 文本内联；无 FETCH/WITH TIES） |
| DISTINCT 整行 | 整行去重 | P0 | 否 | 已支持 |
| UNION / UNION ALL | 分支括号与集合排序语义 | P0 | 否 | 已支持（基础；与 ORDER BY/LIMIT 的作用域需补测试） |
| IN / NOT IN + 空列表 | 空列表降级恒假；NOT IN 三值逻辑提示 | P0 | 否 | 部分（IN 与 IN 子查询有；NOT IN/空列表未处理） |
| EXISTS / NOT EXISTS | 相关子查询 | P0 | 否 | 已支持 |
| INNER/LEFT/RIGHT JOIN | 任意 ON 布尔表达式、`USING (...)` | P0 | 否 | 部分（仅两个 FieldRef 的等值 ON；无 USING） |
| INSERT 多行/DEFAULT | 多行 VALUES、单元格 DEFAULT、DEFAULT VALUES | P0 | 否 | 部分（多行有；DEFAULT/DEFAULT VALUES 缺） |
| RETURNING | INSERT/UPDATE/DELETE 多列/整行/别名 | P0 | 是 | 部分（仅 insert 单列 `CAST AS BIGINT`） |
| ON CONFLICT | 目标列/ON CONSTRAINT/索引谓词/DO UPDATE WHERE/EXCLUDED | P0 | 是 | 部分（列列表 + 全列 EXCLUDED；无 WHERE/约束名/白名单） |
| 服务端表达式入口 | select_expr/set_expr/where_expr | P0 | 否 | 缺失（`SqlExpr::postgres_render` 未接线） |
| CTE / WITH RECURSIVE | 多 CTE、MATERIALIZED/NOT MATERIALIZED | P1 | 否 | 缺失 |
| 窗口函数 OVER/frame | 命名窗口、ROWS/RANGE/GROUPS/EXCLUDE | P1 | 否 | 缺失 |
| GROUP BY 分组集 | GROUPING SETS/ROLLUP/CUBE/GROUP BY DISTINCT | P1 | 否 | 缺失 |
| HAVING | 聚合后过滤；可无 GROUP BY | P1 | 否 | 部分（仅字段比较；无 GROUP BY 时被拒；不能写 `HAVING COUNT(*) > 1`） |
| 聚合修饰 | FILTER、聚合内 ORDER BY/DISTINCT、有序集聚合 | P1 | 否 | 缺失 |
| DISTINCT ON | 每组取首行，校验最左 ORDER BY | P1 | 是 | 缺失 |
| FULL/CROSS/NATURAL JOIN、LATERAL | 全外/交叉/自然连接、横向子查询 | P1 | 否 | 缺失（JoinType 仅 Inner/Left/Right） |
| INTERSECT/EXCEPT、ANY/SOME/ALL | 集合运算与量词谓词 | P1 | 否 | 缺失 |
| VALUES / 表函数 / WITH ORDINALITY | FROM 子查询、unnest/generate_series | P1 | 否 | 缺失 |
| INSERT...SELECT / UPDATE FROM / DELETE USING | 联表 DML | P1 | 是 | 缺失 |
| MERGE（PG15+） | WHEN MATCHED/NOT MATCHED，顺序求值 | P1 | 否 | 缺失（需版本门控） |
| COPY FROM/TO | 流式导入导出、text/csv/binary | P1 | 是 | 缺失 |
| ILIKE / POSIX 正则 / IS DISTINCT FROM | 大小写不敏感、正则、NULL-safe 比较 | P1 | 是 | 缺失（LIKE 有） |
| JSONB / 数组运算符 | `->/->>/#>/@>/?`、`&&/@>/<@`、`= ANY($1)` | P1 | 是 | 缺失（JSONB 整值读写仅部分） |
| 行构造器/TABLESAMPLE/TRUNCATE/SEARCH-CYCLE/DML CTE/MERGE RETURNING/jsonpath/range/FTS | 进阶语法 | P2 | 是/否 | 缺失 |

### 4.2 数据类型与映射

| 能力 | 说明 | P | PG 特有 | yang-db 现状 |
|---|---|:--:|:--:|---|
| OID 驱动列元数据 | RowDescription 的 OID/格式码；未知 OID 可回溯 | P0 | 是 | 缺失（无 PgValueRef 级降级出口） |
| typed NULL / 参数类型 | None 以正确 OID 编码；必要时 `$1::type` | P0 | 是 | 部分（Null 绑 int4；insert/update 内联 NULL 规避） |
| 基础标量 | bool/i16/i32/i64/f32/f64/String/bytes | P0 | 否 | 部分（Int 统一 i64、Float 统一 f64；无 i16/i32/f32 区分） |
| NUMERIC 全精度 | BigDecimal/Decimal，scale/NaN/Infinity | P0 | 否 | 部分（小于 2^53 走 f64，超出走字符串；无 BigDecimal） |
| 日期时间族 | date/time/timetz/timestamp/timestamptz/interval/infinity | P0 | 是 | 部分（仅 `%Y-%m-%d %H:%M:%S` NaiveDateTime；无时区/interval/date/time） |
| UUID | 16 字节二进制、`gen_random_uuid()` | P1 | 是 | 缺失（sqlx uuid feature 未启用） |
| JSON/JSONB | 映射、JSON null vs SQL NULL、操作符 | P1 | 是 | 部分（serde_json::Value 整值；无操作符/JSON null 区分） |
| ARRAY | 一维/多维、NULL 元素、`= ANY($1)` | P1 | 是 | 缺失（JSON 数组会被当作 JSONB） |
| RANGE/MULTIRANGE | 六种内置 range + multirange、Bound 语义 | P2 | 是 | 缺失 |
| 网络类型 | inet/cidr/macaddr/macaddr8 | P1 | 是 | 缺失 |
| BYTEA | Vec<u8>、hex/binary | P1 | 否 | 已支持（base64 失败静默按 UTF-8 字节写入，需修） |
| ENUM | 按类型名/label 编解码，排序为声明序 | P1 | 是 | 缺失 |
| DOMAIN/COMPOSITE | typbasetype/typrelid 递归解析 | P2 | 是 | 缺失 |
| 位串/oid/reg*/几何 | bit/varbit、oid/regclass、point 等 | P2 | 是 | 缺失 |
| 扩展类型 | PostGIS/pgvector/ltree/hstore/citext/money/xml/tsvector/pg_lsn | P2 | 是 | 缺失（建议先声明“透传/不支持”清单） |
| binary/text 双格式 | 两种格式等价编解码 | P1 | 是 | 部分（依赖 sqlx 默认；无格式选择/测试） |
| 会话 GUC 基线 | TimeZone/DateStyle/IntervalStyle/bytea_output | P1 | 是 | 缺失 |
| client_encoding/UTF-8 | SQL_ASCII 不得直接当 String | P1 | 否 | 缺失 |
| 类型 round-trip 测试矩阵 | NULL/极值/NaN/Inf/数组/range | P1 | 是 | 缺失 |

### 4.3 DDL、模式与迁移

| 能力 | 说明 | P | PG 特有 | yang-db 现状 |
|---|---|:--:|:--:|---|
| DDL 事务化执行 | 整组 DDL 失败回滚 | P1 | 是 | 部分（`create_table` 裸 SQL、autocommit；`init` 已 deprecated） |
| dollar-quote 脚本执行器 | 识别 `$$/$tag$`、字符串/注释中的分号 | P1 | 是 | 缺失（`init` 按分号切分，函数/DO 块会坏） |
| 标识符引用/折叠/63 字节 | 双引号、未引用折叠小写、超长截断 | P1 | 是 | 部分（始终加引号，但只接受 ASCII，拒绝 `$`/Unicode/内嵌引号） |
| schema 限定名 | `schema.table`、三段名 | P0 | 是 | 缺失（Identifier 仅 1-2 段；表名字面量禁止 `.`） |
| 结构化建表/约束/IDENTITY/生成列 | 列类型白名单、命名约束、NOT VALID/VALIDATE | P2 | 否 | 缺失（non-goal 路径：保持 raw escape hatch） |
| ALTER TABLE/索引/分区/视图/物化视图 | 子命令、部分/表达式/INCLUDE/CONCURRENTLY、ATTACH/DETACH | P2 | 是 | 缺失 |
| 扩展管理 | CREATE/ALTER/DROP EXTENSION、可用性校验 | P2 | 是 | 缺失 |
| 内省 | pg_catalog/information_schema 反射 | 非目标/P2 | 是 | 缺失（迁移与 diff 由 yang-base/外部工具承担） |
| 迁移 runner/checksum/advisory lock | 版本表、单事务、并发互斥 | 非目标 | 否 | 缺失（`MigrationConfig` 仅占位） |
| GRANT/RLS/角色 | 权限、策略、默认权限 | 非目标/P2 | 是 | 缺失（部署/SRE 层） |

### 4.4 事务与并发

| 能力 | 说明 | P | PG 特有 | yang-db 现状 |
|---|---|:--:|:--:|---|
| 事务边界与 Drop | begin/commit/rollback；Drop 回滚并 warn | P0 | 否 | 已支持 |
| 隔离级别 | RC/RR/Serializable（RU 视为 RC） | P0 | 否 | 已支持（`SET TRANSACTION` 在 BEGIN 后第二条；无 READ ONLY/DEFERRABLE） |
| SAVEPOINT/嵌套事务 | 局部回滚、释放 | P1 | 否 | 缺失（可经 `executor()` 手写逃生） |
| read_only_transaction | READ ONLY，服务端拒绝写入 | P1 | 是 | 缺失 |
| 事务中止态 25P02 | 暴露 is_aborted，引导 ROLLBACK/SAVEPOINT | P1 | 是 | 缺失 |
| 四种行锁 + OF | FOR UPDATE/NO KEY UPDATE/SHARE/KEY SHARE | P1 | 是 | 部分（仅 FOR UPDATE/FOR SHARE） |
| NOWAIT / SKIP LOCKED | 队列消费、快速失败 | P1 | 是 | 缺失 |
| advisory lock | 会话级/事务级、共享/排他 | P1（迁移互斥）/P2 | 是 | 缺失 |
| 40001/40P01 重试 | 整事务重跑、退避/抖动 | P1 | 是 | 缺失（归 Server，不可重试） |
| statement/lock/idle 超时 | 语句、锁等待、悬挂事务上限 | P1 | 是 | 缺失 |
| CancelToken/查询取消 | CancelRequest + 57014 映射 | P1 | 是 | 缺失 |
| 2PC/表锁/DEFERRABLE/transaction_timeout | 进阶事务控制 | P2 | 是 | 缺失 |
| 游标/流式结果 | DECLARE/FETCH、背压 | P1 | 是 | 缺失 |

### 4.5 连接与运行时

| 能力 | 说明 | P | PG 特有 | yang-db 现状 |
|---|---|:--:|:--:|---|
| 池配置与健康检查 | min/max/acquire/idle/max_lifetime/test_before_acquire | P1 | 否 | 已支持（`connect_timeout` 被复用为 acquire_timeout；无法区分池满与建连慢） |
| after_connect/before_acquire | 每连接初始化、借出钩子 | P1 | 否 | 缺失 |
| application_name/search_path/options | StartupMessage 会话默认值 | P1 | 是 | 缺失（只能经连接串间接设置） |
| statement cache / PgBouncer | 容量、清理、事务池兼容开关 | P1 | 是 | 缺失 |
| DISCARD ALL 归还清理 | 清 search_path/LISTEN/临时表/prepared/advisory lock | P1 | 是 | 缺失 |
| 池指标与错误区分 | waiting/size/idle；PoolTimedOut vs PoolClosed | P1 | 否 | 部分（waiting 恒 0；两者都映射 ConnectionError 且可重试） |
| TLS | 六种 sslmode、CA、客户端证书、verify-full | P1 | 是 | 部分（仅连接串 sslmode；无证书配置/测试） |
| keepalives/tcp_user_timeout | 半开连接回收 | P1 | 是 | 缺失 |
| server_version_num/能力门控 | 按版本启用语法 | P1 | 是 | 缺失 |
| 多主机/target_session_attrs | 主从选择、按序重试 | P2 | 是 | 缺失 |
| 优雅关闭/重连语义 | 排空在途连接、事务不可跨连接恢复 | P1 | 否 | 已支持（close/is_closed）；断线语义无测试 |
| client_encoding/会话 GUC | UTF-8 校验、TimeZone/DateStyle | P1 | 是 | 缺失 |

### 4.6 安全与错误

| 能力 | 说明 | P | PG 特有 | yang-db 现状 |
|---|---|:--:|:--:|---|
| SQLSTATE 结构化 | 5 字符码 + constraint/schema/table/column/detail/hint/position | P0 | 是 | 部分（只映射少数码，其余拼进 message；结构体不携带 code） |
| 约束谓词 | unique/fk/check/not-null/exclusion（23505/23503/23514/23502/23P01） | P1 | 是 | 部分（前 4 类归 ConstraintError；无 23P01；无谓词 API） |
| 参数化值 | 值不进入 SQL 文本 | P0 | 否 | 已支持 |
| checked identifier | 外部标识符走白名单/引用 | P0 | 否 | 部分（ASCII-only；未利用 PG 引用能力） |
| search_path 固定 | 防可写 schema 劫持；对象尽量 schema 限定 | P0 | 是 | 缺失 |
| SET LOCAL 上下文/RLS 输入 | 租户、角色、超时的事务级设置 | P1/P2 | 是 | 缺失（RLS 管理本身归部署层） |
| SCRAM/channel_binding | 安全密码认证与通道绑定 | P1 | 是 | 部分（依赖 sqlx 默认；无显式配置） |
| 原生 SQL 逃生舱 | 明确 deprecated/信任边界 | P1 | 否 | 部分（query/execute/init 已 deprecated；create_table 未标） |
| Blob 输入静默回退 | base64 失败应报错或显式 raw 前缀 | P1 | 否 | 部分（静默按 UTF-8 字节写入） |
| 能力声明一致性 | 声明与实现一致、可机读 | P1 | 否 | 部分（`ExplicitTrustedExpressions` 在 PG 无对应入口；枚举无 PG 专属变体） |

### 4.7 可观测与运维接口

| 能力 | 说明 | P | PG 特有 | yang-db 现状 |
|---|---|:--:|:--:|---|
| 统一管理面 | capabilities/health_check/close/is_closed/pool_status | P1 | 否 | 已支持 |
| 日志/tracing/脱敏 | SQL、参数、耗时、SQLSTATE、连接 ID | P1 | 否 | 部分（有 operation 级 tracing；无 SQLSTATE/行数/慢查询阈值） |
| LISTEN/NOTIFY | 独立监听连接、重连重订阅 | P1 | 是 | 缺失 |
| COPY | FROM STDIN/TO STDOUT 流式 | P1 | 是 | 缺失 |
| advisory lock | 分布式互斥/迁移锁 | P1 | 是 | 缺失 |
| EXPLAIN/计划诊断 | EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) | P2 | 是 | 缺失 |
| 锁/会话内省 | pg_locks/pg_stat_activity 阻塞链 | P2 | 是 | 缺失 |
| 逻辑复制/复制槽 | publication/subscription、WAL 位点 | 非目标 | 是 | 缺失（建议明确声明） |
| FDW/外部表 | postgres_fdw/file_fdw | 非目标 | 是 | 缺失（建议明确声明） |
| 备份/恢复/建库 | pg_dump/restore、CREATE DATABASE | 非目标 | 是 | 缺失（现有 non-goal） |

### 4.8 测试与工具链

| 能力 | 说明 | P | PG 特有 | yang-db 现状 |
|---|---|:--:|:--:|---|
| PG 集成测试进 CI | 默认门禁执行，失败即红 | P1 | 是 | 部分（CI 仅 2 个文件；advanced 4 例全 `#[ignore]`） |
| 断言强度 | 不允许连接/建表失败静默通过 | P1 | 否 | 部分（CRUD 测试存在 `println` 后 return、`if let Ok` 包裹） |
| testcontainers 一键复现 | 每测试容器、自动清理 | P1 | 是 | 缺失（PG 测试固定 localhost:5433） |
| PG 专项单测 | build_insert/update/upsert/returning/SQLSTATE | P1 | 否 | 部分（有 33 个内联 builder 单测；缺 update_batch/SQLSTATE/RETURNING 类型） |
| 属性测试 | 类型 encode→decode 往返、边界值 | P2 | 是 | 缺失 |
| SQL 快照 | insta 快照 SQL + 参数序列 | P2 | 否 | 缺失 |
| 多版本矩阵 | 至少 PG 16/17；记录 14/15 差异 | P1 | 是 | 部分（CI 固定 postgres:16-alpine） |
| feature 矩阵 | runtime × TLS × 类型 feature | P2 | 否 | 部分（有 lib 级矩阵，无 PG 类型/TLS 组合） |
| 编译期 SQL/离线元数据 | query!/prepare --check | 非目标/P2 | 否 | 缺失（开发工作流，不进运行时基线） |
| 迁移 CLI/代码生成 | migrate add/run、db pull | 非目标 | 否 | 缺失（外部工具/ORM 层） |

## 5. 与 yang-db 现状的缺口矩阵

口径：

- 只列“缺失/部分”；“已支持”不再重复。
- `对称性` = MySQL 有而 PG 无（或入口命名/形状不一致）；`PG 特有` = PG 语义要求而两端都没有。
- 证据只给关键 `file:line`；行号以当前工作树为准。

### 5.1 P0 缺口（11 项）

| # | 缺口 | 类别 | 证据 | 影响 |
|---|---|---|---|---|
| P0-1 | SQLSTATE 只保留 message，丢失 code/constraint/schema/table/column/detail/hint | PG 特有 | `src/error.rs:171-184` | 下游只能解析本地化字符串；约束定位与审计不可用 |
| P0-2 | 40001/40P01/57014/25P02/55P03 未分流；`is_retryable()` 为 false | PG 特有 | `src/error.rs:120-152`、`:202-204` | 串行化失败/死锁无法安全重试；池关闭也被判可重试 |
| P0-3 | RETURNING 仅 insert 单列 `CAST(... AS BIGINT)`；update/delete/upsert/batch 无 RETURNING | PG 特有 | `src/postgres/query_builder.rs:1801-1806`、`:1203-1206` | UUID/text/numeric 主键无法回填；无法返回受影响行 |
| P0-4 | `select_expr/set_expr/where_expr` 缺失；`SqlExpr::postgres_render` 为 dead code | 对称性 + PG 正确性 | `src/reference.rs:372-382`；`src/postgres/query_builder.rs` 无入口 | 计算列、服务端时间写入、表达式谓词无法表达 |
| P0-5 | WHERE 的 `SqlValue::Null` 绑定为 `Option<i32>` | PG 特有 | `src/postgres/query_builder.rs:23-45`、`:555-558` | 非整型列比较报 `operator does not exist` |
| P0-6 | NUMERIC 以 f64/String 绑定；大于 2^53 丢精度、text→numeric 亲和问题 | PG 特有 | `src/postgres/query_builder.rs:907-934`、`:965-981` | 金额/科学计算不可信 |
| P0-7 | 时间类型仅 `%Y-%m-%d %H:%M:%S` NaiveDateTime；无 timestamptz/date/time/timetz/interval/infinity | PG 特有 | `src/postgres/query_builder.rs:877-895`；`src/postgres/condition.rs:107-119` | 时区语义丢失；带偏移/毫秒/ISO8601 输入报错 |
| P0-8 | ON CONFLICT 仅冲突列 + 全列 `EXCLUDED`；默认 `["id"]`；无 ON CONSTRAINT/DO UPDATE WHERE/部分索引/白名单 | PG 特有 | `src/postgres/query_builder.rs:769-864`、`:1055` | 非 id 唯一键、条件 upsert 不可表达；存在隐性陷阱 |
| P0-9 | schema 限定与三段名缺失；表名字面量禁止 `.`；`table_exists` 固定 `current_schema()` | PG 特有 | `src/reference.rs:127-165`、`src/sql_types.rs:14-24`、`src/postgres/database.rs:359-371` | 无法表达 `schema.table`，多 schema 应用不可用 |
| P0-10 | search_path 未固定、无 `SET LOCAL` 会话上下文 | PG 特有 | `src/postgres/database.rs:30-45`、`:165-189` | 可写 schema 劫持；池化下租户上下文易泄漏 |
| P0-11 | 布尔条件树无 NOT；`where_and/or` 只能拼扁平条件 | 对称性 | `src/postgres/query_builder.rs:1212-1398` | 否定条件只能用反向运算符，复杂谓词需裸 SQL |

### 5.2 P1 缺口（28 项）

| # | 缺口 | 类别 | 证据 | 影响 |
|---|---|---|---|---|
| P1-1 | UUID 类型/绑定缺失；sqlx uuid feature 未启用 | PG 特有 | 根 `Cargo.toml:75`；`query_builder.rs:1803` | UUID 主键表无法 insert 取回 |
| P1-2 | ARRAY 映射/`= ANY($1)`/数组运算符缺失 | PG 特有 | `postgres/condition.rs:107-119`、`query_builder.rs:980` | 数组列不可写；JSON 数组被误当 JSONB |
| P1-3 | JSONB 运算符/路径/JSON null 语义缺失 | PG 特有 | `query_builder.rs:874-980` | 只能整值读写，无法 containment/路径查询 |
| P1-4 | ENUM/DOMAIN/COMPOSITE/网络/位串/oid 编解码缺失 | PG 特有 | 无入口；`FieldType` 仅 7 变体 | 自定义类型只能当 text，排序/约束语义丢失 |
| P1-5 | CTE / WITH RECURSIVE 缺失 | 对称性 | `query_builder.rs` 状态无 CTE 字段 | 报表预聚合/树查询只能裸 SQL |
| P1-6 | 窗口函数/frame 缺失 | 对称性 | `reference.rs:223-239`（SelectExpr 仅 5 聚合） | top-N/累计/排名只能用原生 SQL（真实消费者已如此） |
| P1-7 | HAVING 仅 6 种字段比较；无 GROUP BY 时被拒；无 GROUPING SETS/FILTER/有序集 | 对称性 | `query_builder.rs:221-225`、`:1365-1398` | `HAVING COUNT(*) > 1` 不可表达 |
| P1-8 | DISTINCT ON / FULL / CROSS / NATURAL / LATERAL / USING 缺失 | PG 特有 | `postgres/field.rs:25-31`、`query_builder.rs:302-326` | 每组取首行、全外连接、横向 top-N 不可表达 |
| P1-9 | INTERSECT/EXCEPT、ANY/SOME/ALL 缺失 | 对称性 | builder 状态仅 unions | 集合差/交与量词查询需裸 SQL |
| P1-10 | INSERT...SELECT / UPDATE FROM / DELETE USING 缺失 | PG 特有 | 无对应 builder | 联表迁移/批量更新需裸 SQL |
| P1-11 | MERGE（PG15+）缺失 | PG 特有 | 无 builder | 复杂 upsert/同步需多条语句或裸 SQL |
| P1-12 | COPY FROM/TO 缺失 | PG 特有 | 无 `PgCopyIn/Out` | 大批量导入导出性能差一个数量级 |
| P1-13 | ILIKE/POSIX 正则/IS DISTINCT FROM 缺失 | PG 特有 | `CompareOp` 无对应操作 | 大小写不敏感搜索/NULL-safe 比较需裸 SQL |
| P1-14 | 批量 UPSERT/`upsert_batch_with_size` 缺失；`insert_returning_id` 命名不对称；Config setter/`QueryBuilder::from_pool` 不对称 | 对称性 | `capability.rs:128-139`；`mysql/query_builder/write.rs:461,499` | 批量同步只能逐条；配置/事务入口不齐 |
| P1-15 | SAVEPOINT/嵌套事务缺失 | 对称性 | `postgres/transaction.rs:11-278` | 局部失败只能整事务回滚 |
| P1-16 | `read_only_transaction` 缺失 | 对称性 | `postgres/database.rs` 无入口 | 只读副本/报表无数据库强制保护 |
| P1-17 | 行锁仅 FOR UPDATE/FOR SHARE；无 OF/NOWAIT/SKIP LOCKED/NO KEY UPDATE/KEY SHARE | PG 特有 | `src/row_lock.rs:6-22` | 队列消费、FK 密集并发不可安全表达 |
| P1-18 | advisory lock 缺失 | PG 特有 | 无封装 | 分布式锁/迁移互斥只能裸 SQL |
| P1-19 | statement_timeout/lock_timeout/idle_in_transaction + CancelToken 缺失 | PG 特有 | `DatabaseConfig` 无字段；无 after_connect | 慢查询/锁等待无上界，无法取消 |
| P1-20 | after_connect/application_name/search_path/statement_cache/DISCARD ALL/池指标缺失；PoolTimedOut 与 PoolClosed 不区分 | PG 特有/运行时 | `database.rs:30-45`、`:177-186`、`:226-233` | 会话状态泄漏；池耗尽/关闭误判可重试 |
| P1-21 | TLS 仅连接串 sslmode，无 CA/客户端证书/verify-full 配置与测试 | PG 特有 | `database.rs:165-189`；根 `Cargo.toml:75` | 生产 TLS 校验不可验证，存在降级风险 |
| P1-22 | server_version_num 门控、GUC 基线、client_encoding 校验缺失 | PG 特有 | 无探针/初始化代码 | 跨版本语法与 locale/编码漂移 |
| P1-23 | 游标/流式结果缺失 | 对称性/运行时 | 查询 API 以 Vec 为主 | 大结果集全量进内存 |
| P1-24 | LISTEN/NOTIFY 缺失 | PG 特有 | 无 `PgListener` | 事件驱动/缓存失效无法用库内 API |
| P1-25 | PG 测试门禁弱：CI 仅 2 文件；advanced 4 例 `#[ignore]`；断言可静默通过；无 testcontainers；仅 PG16 | 测试 | `.github/workflows/ci.yml:234-262`；`tests/integration_pg_crud_simple.rs` | 能力声明缺少回归证据，发布门禁不可信 |
| P1-26 | 能力声明与实现不一致：`ExplicitTrustedExpressions` 无 PG 表达式入口；`BackendCapability` 无 PG 专属变体 | 文档/契约 | `capability.rs:143-149`、`:161-176` | 调用方无法机读判断 COPY/SKIP LOCKED/数组等能力 |
| P1-27 | DDL 事务化/dollar-quote 脚本执行器缺失；`create_table` 裸 SQL 未标 deprecated | DDL/逃生舱 | `database.rs:325-345`；`postgres/init.rs:5-21` | 初始化脚本在函数/DO 块处被分号切坏；DDL 失败不可整组回滚 |
| P1-28 | 标识符仅 ASCII：拒绝 `$`、Unicode、内嵌双引号；无 63 字节规则 | 安全/可用性 | `sql_types.rs:14-24`；`postgres/identifier.rs:10-25` | 中文/含 `$` 标识符无法使用；引用能力未发挥 |

### 5.3 P2 缺口（11 项）

| # | 缺口 | 类别 | 证据/说明 | 影响 |
|---|---|---|---|---|
| P2-1 | 行构造器/行级比较、BETWEEN SYMMETRIC、TABLESAMPLE、TRUNCATE | 查询 | 无入口 | 进阶查询需裸 SQL |
| P2-2 | jsonpath、range/multirange 运算符、全文检索 tsvector/tsquery | 查询/类型 | 无入口 | 路径查询、时段重叠、全文检索不可用 |
| P2-3 | MERGE RETURNING/merge_action（PG17）、SEARCH/CYCLE、数据修改 CTE | 查询 | 需版本门控 | 递归排序/环检测/DML CTE 需裸 SQL |
| P2-4 | 2PC、表级 LOCK TABLE、DEFERRABLE、transaction_timeout（PG17） | 事务 | 无入口 | 分布式事务/迁移锁/长事务上限不可控 |
| P2-5 | 多主机/target_session_attrs、PgBouncer 模式、large object | 运行时 | 无配置 | 故障转移/事务池/大对象需外部方案 |
| P2-6 | 扩展类型：PostGIS/pgvector/ltree/hstore/citext/money/xml/tsvector/pg_lsn/几何 | 类型 | 无映射 | 只能文本透传或自定义 Type |
| P2-7 | 分区/排他约束/表继承/注释/高级索引 DDL | DDL | non-goal 路径 | 由外部迁移工具承担 |
| P2-8 | EXPLAIN/pg_locks/pg_stat_activity 诊断 | 可观测 | 无接口 | 计划与阻塞链需外部工具 |
| P2-9 | 属性测试/SQL 快照/故障注入/多版本矩阵扩展 | 测试 | 仅部分单测 | 边界回归不足 |
| P2-10 | 版本门控总表（PG14-18）与 PG18+ 策略 | 版本 | 无 feature×min_version 表 | 可能在旧版本生成非法 SQL |
| P2-11 | 文档时效：BACKLOG NEW-39/40 仍标待处理（代码已修）；`yang-db.md` 签名/章节过期 | 文档 | `docs/BACKLOG.md:609-625`；`docs/reference/yang-db.md` | 排期重复/误导使用者 |

> 计数：P0 11、P1 28、P2 11，共 50 项。计数只针对本矩阵条目，不等于能力基线总项数。

### 5.4 两类缺口的性质差异

- **对称性缺口**（P0-4、P0-11、P1-5/6/7/9/14/15/16/23/28）：多数不是“PG 做不到”，而是 PG 侧未跟进 MySQL 的 API 演进。修复成本相对低，但需要先决定“共享抽象还是双实现”，否则会继续漂移。
- **PG 特有成熟度缺口**（P0-1/2/3/5/6/7/8/9/10，P1-1/2/3/4/8/10/11/12/13/17/18/19/20/21/22/24）：必须按 PG 语义单独设计，不能从 MySQL 复制。它们决定“能不能称为成熟的 PG 基础库”。
- **结构性原因**：PG `query_builder.rs` 3167 行单文件内含独立 SqlGenerator/build_*；MySQL 已拆 9 文件。新增能力需双写，`ColumnExpr` 仍以 `cfg(mysql)` 排除 PG，已发生 `select_expr/upsert_batch` 漂移。建议 Phase A 先做“可共享的中间层”（值类型、SQLSTATE、表达式节点），而不是直接复制 MySQL 代码。

## 6. 三阶段路线图

### Phase A：对称性与正确性补齐（P0）

**目标**：让 PG 的单表 CRUD/事务在语义上正确、安全、可回归；把“会静默出错”的路径清零。

| 工作项 | 范围 | 验收标准（可测试） |
|---|---|---|
| A1 SQLSTATE 契约 | `DbError` 增加 `sqlstate/constraint/schema/table/column/detail/hint`；分类 23505/23503/23502/23514/23P01/40001/40P01/57014/25P02/55P03/25006/42501/22P02/22003/08xxx | 单测断言每个码映射到独立变体/谓词；`is_retryable()` 仅 40001/40P01/08xxx/53xxx 为 true；PoolClosed 不可重试 |
| A2 RETURNING 重构 | 支持多列/整行/别名；按列类型解码；insert/update/delete/upsert/batch 的 returning；`insert_returning_id` 作为兼容别名 | UUID/text/numeric/非 id 主键表 insert 返回正确值；update/delete returning 集成测试；`returning()` 显式类型不再被 CAST BIGINT 覆盖 |
| A3 表达式入口 | 接线 `SqlExpr::postgres_render`；新增 `select_expr/set_expr/where_expr`；显式 trusted 入口与 checked identifier 分离 | 单测验证渲染；集成测试 `COALESCE`/`NOW()`/`CAST` 投影与写入；未 trusted 输入仍被拒绝 |
| A4 typed NULL | `SqlValue::Null` 不再固定 int4；支持 `$n::type` 或 TypedValue；修正 WHERE/IN/raw 参数 | text/timestamptz/uuid/numeric 列的 `Eq None`/`IN (NULL)` 执行成功；无 `operator does not exist` |
| A5 值类型第一层 | NUMERIC（BigDecimal/Decimal feature）、timestamptz/date/time/interval、UUID；启用 sqlx 对应 feature | 38 位 NUMERIC 往返；timestamptz 保留 UTC；UUID 主键 CRUD；date/time/interval 边界（含 infinity 显式策略） |
| A6 ON CONFLICT 完整化 | ON CONSTRAINT、DO UPDATE WHERE、更新列白名单、部分索引谓词、冲突后 RETURNING；移除默认 `["id"]` 或改为显式必填 | 非 id 唯一键、条件 upsert、部分唯一索引各一条集成测试；缺目标时构建期报错 |
| A7 schema/search_path | 支持 `schema.table` 三段名；连接配置 `search_path`；`SET LOCAL` 上下文 helper | schema 限定 CRUD 在 search_path 为空时通过；SET LOCAL 在归还池后不泄漏（复用连接断言） |
| A8 布尔 NOT | `where_not`/`Condition::Not`；嵌套括号保序 | `NOT (a OR b AND c)` SQL 快照 + 真值测试；与 AND/OR 组合正确 |
| A9 测试门禁 | PG 测试全部进 CI；连接/建表失败 panic；count/update/delete 断言去掉 `if let Ok`；引入 testcontainers；PG 16/17 双版本 | CI 红灯可复现任何失败；本地 `cargo test --features postgres` 一键起容器；PG 17 job 通过 |

**Phase A 退出条件**：P0 矩阵 11 项全部有代码 + 单测 + 集成测试；`POSTGRES_CAPABILITIES` 更新并通过契约测试；文档标注 PG 与 MySQL 的差异；无新增 dead code。

### Phase B：成熟能力（PG 特有）

**目标**：把“成熟 PG 基础库常用能力”补齐到可用、可测、可声明。

| 工作项 | 范围 | 验收标准（可测试） |
|---|---|---|
| B1 查询能力 | CTE/递归、窗口/frame、HAVING/分组集/聚合修饰、DISTINCT ON、FULL/CROSS/NATURAL/LATERAL/USING、INTERSECT/EXCEPT、ANY/ALL、VALUES/表函数 | 每项一个 SQL 快照 + 一个 PG16/17 集成测试；窗口 top-N 与递归树用例通过 |
| B2 DML 能力 | 批量 UPSERT、INSERT...SELECT、UPDATE FROM、DELETE USING、MERGE（PG15+ 门控）、COPY FROM/TO | `upsert_batch` 与逐条结果一致；MERGE 在 PG16 工作、PG15 明确拒绝；COPY 导入 10k 行内存有界 |
| B3 事务能力 | SAVEPOINT、read_only_transaction、四种行锁/OF/NOWAIT/SKIP LOCKED、advisory lock、40001/40P01 重试 helper、statement/lock/idle 超时、CancelToken、游标/流式 | 保存点局部回滚；只读事务写报 25006；双消费者 SKIP LOCKED 无重复；超时映射 57014/55P03；取消后连接可复用或明确丢弃 |
| B4 连接运行时 | after_connect、application_name/search_path/options、statement_cache_size、DISCARD ALL、池指标、TLS 证书/verify-full、server_version_num、GUC 基线、client_encoding | 借出前后会话状态断言；PgBouncer 模式无 prepared 冲突；verify-full 失败/成功双用例；版本探针驱动门控 |
| B5 类型第二层 | ARRAY（含 `= ANY`）、JSONB 运算符、ENUM/DOMAIN/COMPOSITE、网络类型、BYTEA 格式修正 | 类型往返测试矩阵；数组 containment/JSONB `@>`/enum 排序各一条；base64 失败显式报错 |
| B6 运维接口 | LISTEN/NOTIFY、查询耗时/行数/SQLSTATE 日志、慢查询阈值、健康检查指标 | 两连接通知测试（提交投递、回滚不投递）；断线重订阅；日志断言含 SQLSTATE 且不含敏感参数 |
| B7 契约与文档 | 扩展 `BackendCapability`（BatchUpsert/Savepoint/SkipLocked/Copy/ListenNotify/Cte/Window/Returning 形状）；更新 README/矩阵/PG 使用章节 | `supports()` 契约测试；文档契约测试从“关键词 contains”升级为“符号/签名存在”；BACKLOG 对账 |

**Phase B 退出条件**：P1 矩阵 28 项中至少 80% 有实现与测试；剩余项有明确排期或 non-goal 说明；PG 16/17 CI 全绿；能力声明与实现零偏差。

### Phase C：进阶与差异化

**目标**：覆盖进阶 PG 语义与扩展生态，形成与同类基础库的差异化；不追求“全语法覆盖”。

| 工作项 | 范围 | 验收标准（可测试） |
|---|---|---|
| C1 查询进阶 | 行构造器/BETWEEN SYMMETRIC/TABLESAMPLE/TRUNCATE、jsonpath、range/multirange、FTS、MERGE RETURNING/merge_action（PG17）、SEARCH/CYCLE、DML CTE | 每项带版本门控的集成测试；PG17 与 PG16 行为差异断言；range overlap/GiST 用例 |
| C2 事务进阶 | 2PC、表锁、DEFERRABLE、transaction_timeout、多主机/target_session_attrs、PgBouncer | 2PC 在 `max_prepared_transactions>0` 环境通过；主从选择双容器测试；超时归类正确 |
| C3 类型生态 | PostGIS/pgvector/ltree/hstore/citext/money/xml/tsvector/pg_lsn/几何；自定义 Type 注册与 OID 缓存失效 | 可选 feature `postgres-ext`；扩展类型往返；未注册类型返回含类型名的显式错误 |
| C4 可观测 | EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)、pg_locks/pg_stat_activity 阻塞链、slow query 指标 | 计划 JSON 可解析；阻塞链接口返回结构化数据；指标可导出 |
| C5 质量工程 | proptest 往返、insta SQL 快照、故障注入（kill backend/断网/池耗尽）、PG 14-18 版本矩阵、feature×TLS 矩阵 | CI 矩阵全绿；故障注入后池可恢复、LISTEN 重订阅、事务不静默重放 |
| C6 文档与发布 | 版本门控总表、升级指南、非目标清单、能力矩阵自动生成 | 文档与代码符号一致性测试；每个 non-goal 有理由与替代路径 |

**Phase C 退出条件**：P2 矩阵项按消费者需求排序落地；高级能力默认关闭或版本门控，不增加基础路径复杂度；发布说明明确“支持到哪一版 PG、哪些能力是可选”。

**跨阶段约束**：

- 所有新增 SQL 生成必须有 checked identifier/绑定参数两条安全边界；trusted 表达式必须显式标注。
- 所有版本相关语法必须经 `server_version_num` 门控，禁止在 PG16 生成 PG17 语法。
- 所有能力声明必须同步 `capability.rs` 与 `docs/reference/BACKEND_CAPABILITIES.md`，并由测试固定。
- 测试优先：每个 Phase 的新增能力先写失败用例，再实现；PG 集成测试不得静默跳过。

## 7. 非目标与明确不做

与现有 non-goal 对齐（`crates/yang-db/README.md:41-43`、`docs/reference/BASE_DB_CAPABILITY_MATRIX.md`）：

| 非目标 | 理由 | 替代路径 |
|---|---|---|
| SQLite / MSSQL | 无真实消费者；驱动/类型/DDL/CI 成本高 | 出现消费者后独立 RFC |
| backup / restore / database-create | 集群级操作、权限/审计/加密属部署层；`CREATE DATABASE` 不能进事务 | pg_dump/pg_restore、云平台/运维工具 |
| 通用 ORM / 任意 SQL AST | 与项目的 schema-first/受控构造边界冲突；维护成本高 | 结构化 builder + raw escape hatch |
| 破坏性或任意 schema migration | 自动 diff 无法判断重命名/删除意图；不可逆 | 人工迁移、yang-base schema_sync（加法子集）、外部迁移工具 |
| br-addon / br-db 逐方法兼容 | 不以方法数量作为完成度 | 按真实消费者与已验证风险演进 |

建议新增声明的 non-goal（本次调研发现的空白，避免“看起来支持”）：

| 建议 non-goal | 理由 |
|---|---|
| 逻辑复制/复制槽（publication/subscription、pg_replication_slots） | 属集群/DBA 运维；需要 wal_level/权限与长期状态，不应进运行时 QueryBuilder |
| FDW/外部表（postgres_fdw/file_fdw、IMPORT FOREIGN SCHEMA） | 跨库联邦与服务器文件访问，权限模型复杂，超出基础库边界 |
| 事件触发器/规则系统（EVENT TRIGGER、CREATE RULE） | 低频、易被误用；应由迁移/运维工具管理 |
| VACUUM/CLUSTER/REINDEX/ANALYZE 等维护命令 | 不能在事务块内执行，属运维调度 |
| 服务端 COPY PROGRAM/文件、服务端 lo_import/lo_export | 服务端文件系统权限，默认仅超级用户，安全风险高 |
| 认证协议实现（GSSAPI/LDAP/PAM/peer） | 由 libpq/sqlx 与部署环境承担；库只暴露连接配置 |
| 迁移 CLI/checksum runner/自动 diff | 与现有 non-goal 一致；由 yang-base 或外部工具承担 |
| GRANT/RLS/角色管理 | 部署/SRE 职责；库只提供 `SET LOCAL` 与策略输入入口 |
| 编译期 SQL 校验/离线元数据/代码生成 | 开发工作流/ORM 生态能力，不是运行时基础库职责 |
| 结构化 DDL 构建器（CREATE TABLE/ALTER 全语法） | 现有 `create_table` 是明确逃生舱；结构化 DDL 成本高且与迁移 non-goal 重叠，若需要应独立 RFC |

> 注：non-goal 不等于“永远不做”，而是“当前版本不承诺、不进入公开能力矩阵”；新增前必须有真实消费者与 RFC。

## 8. 参考来源

官方文档（PostgreSQL 16，另注 17）：

- 协议与消息：https://www.postgresql.org/docs/16/protocol-message-formats.html
- 扩展查询/取消/流水线：https://www.postgresql.org/docs/16/protocol-flow.html
- libpq 参数与 SSL：https://www.postgresql.org/docs/16/libpq-connect.html 、https://www.postgresql.org/docs/16/libpq-ssl.html
- SELECT/ORDER/LIMIT/UNION/WITH/WINDOW：https://www.postgresql.org/docs/16/sql-select.html 、https://www.postgresql.org/docs/16/queries-order.html 、https://www.postgresql.org/docs/16/queries-union.html 、https://www.postgresql.org/docs/16/queries-with.html
- INSERT/UPDATE/DELETE/RETURNING/ON CONFLICT：https://www.postgresql.org/docs/16/sql-insert.html 、https://www.postgresql.org/docs/16/sql-update.html 、https://www.postgresql.org/docs/16/sql-delete.html
- MERGE（17）：https://www.postgresql.org/docs/17/sql-merge.html
- COPY：https://www.postgresql.org/docs/17/sql-copy.html
- 事务/保存点/隔离/锁：https://www.postgresql.org/docs/16/sql-set-transaction.html 、https://www.postgresql.org/docs/16/sql-savepoint.html 、https://www.postgresql.org/docs/16/transaction-iso.html 、https://www.postgresql.org/docs/16/sql-lock.html
- advisory lock/序列：https://www.postgresql.org/docs/16/functions-admin.html
- 超时/客户端 GUC/DISCARD：https://www.postgresql.org/docs/16/runtime-config-client.html 、https://www.postgresql.org/docs/16/sql-discard.html
- 错误码：https://www.postgresql.org/docs/16/errcodes-appendix.html
- 类型：https://www.postgresql.org/docs/16/datatype-numeric.html 、https://www.postgresql.org/docs/16/datatype-datetime.html 、https://www.postgresql.org/docs/16/datatype-json.html 、https://www.postgresql.org/docs/16/datatype-character.html 、https://www.postgresql.org/docs/16/datatype-binary.html 、https://www.postgresql.org/docs/16/datatype-net-types.html 、https://www.postgresql.org/docs/16/datatype-enum.html 、https://www.postgresql.org/docs/16/datatype-oid.html
- 数组/范围/全文检索：https://www.postgresql.org/docs/16/arrays.html 、https://www.postgresql.org/docs/16/rangetypes.html 、https://www.postgresql.org/docs/16/functions-textsearch.html
- 标识符/schema：https://www.postgresql.org/docs/16/sql-syntax-lexical.html 、https://www.postgresql.org/docs/16/ddl-schemas.html
- 索引/分区/DDL：https://www.postgresql.org/docs/16/sql-createindex.html 、https://www.postgresql.org/docs/16/ddl-partitioning.html 、https://www.postgresql.org/docs/16/ddl-generated-columns.html
- 系统目录：https://www.postgresql.org/docs/16/catalog-pg-type.html 、https://www.postgresql.org/docs/16/catalog-pg-attribute.html 、https://www.postgresql.org/docs/16/catalog-pg-index.html

Rust 生态：

- sqlx PostgreSQL types：https://docs.rs/sqlx/0.8.6/sqlx/postgres/types/index.html
- sqlx PgConnectOptions：https://docs.rs/sqlx/latest/sqlx/postgres/struct.PgConnectOptions.html
- sqlx PoolOptions：https://docs.rs/sqlx/latest/sqlx/pool/struct.PoolOptions.html
- sqlx Migrator：https://docs.rs/sqlx/latest/sqlx/migrate/struct.Migrator.html
- sqlx PgListener：https://docs.rs/sqlx/latest/sqlx/postgres/struct.PgListener.html
- tokio-postgres：https://docs.rs/tokio-postgres/latest/tokio_postgres/struct.Client.html
- postgres-types：https://docs.rs/postgres-types/latest/postgres_types/
- diesel Connection：https://docs.rs/diesel/latest/diesel/connection/trait.Connection.html
- sea-orm：https://docs.rs/sea-orm/latest/sea_orm/
- cornucopia：https://github.com/cornucopia-rs/cornucopia
- refinery：https://docs.rs/refinery/latest/refinery/
- testcontainers-rs：https://github.com/testcontainers/testcontainers-rs
- proptest：https://docs.rs/proptest/latest/proptest/
- insta：https://docs.rs/insta/latest/insta/

跨语言对标：

- SQLAlchemy PostgreSQL dialect：https://docs.sqlalchemy.org/en/20/dialects/postgresql.html
- SQLAlchemy reflection：https://docs.sqlalchemy.org/en/20/core/reflection.html
- Prisma raw SQL：https://www.prisma.io/docs/orm/v6/prisma-client/using-raw-sql/raw-queries
- Prisma logging：https://www.prisma.io/docs/orm/prisma-client/observability-and-logging/logging
- asyncpg API：https://magicstack.github.io/asyncpg/current/api/index.html
- PgBouncer features：https://www.pgbouncer.org/features.html

本地证据：

- `crates/yang-db/src/postgres/**`、`src/capability.rs`、`src/error.rs`、`src/reference.rs`、`src/sql_types.rs`、`src/row_lock.rs`
- `crates/yang-db/tests/integration_pg_*.rs`、`.github/workflows/ci.yml`
- `crates/yang-db/README.md`、`docs/reference/BACKEND_CAPABILITIES.md`、`docs/reference/BASE_DB_CAPABILITY_MATRIX.md`、`docs/BACKLOG.md`
