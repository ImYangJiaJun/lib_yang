# lib_yang 基础库成熟度与结构评估（对标 SCS br 生态与 GitHub 成熟库）

- **评估日期**：2026-09-02
- **源码快照**：`0de2853`（master，2026-08-25「简化接口定义方式」）
- **基线**：`2026-07-26-yang-foundation-assessment.md`（L4 入口 / 4.0/5）；本报告为增量复核 + 外部对标扩展，不重复已关闭项的取证
- **对象**：yang-db、yang-base、yang-base-derive、yang-runtime（yang-pcg 正交，不计入）
- **方法声明**：源码盘点（行数/feature/API 计数为实测）、SCS `scs/` 目录实证对标、GitHub 成熟库公开资料对标。分数为工程量表而非统计测量。

---

## 0. 结论先行

1. **成熟度判定**：维持 L4 入口（受控生产候选，4.0/5）。自 07-26 基线后的三笔增量提交（认证机制下沉 `8e0e073`、受控服务端时间表达式 `461988b`、接口简化 `0de2853`）方向均正确，无成熟度回退。**"自用/内部项目生产可用"有源码与测试证据支撑；"作为通用开源基础库对外成熟"不能声称**——缺外部用户、semver 发布史、第三方审计，这不是缺陷而是口径边界。
2. **对标 br 生态**：框架内核层（元数据驱动 CRUD、Action 派发、RBAC/租户、Tools 资源所有权、OpenAPI）已结构性对齐，且安全模型（参数绑定、fail-closed、step-up 重认证、tenant resolver 服务端校验）**强于 br**（br 存在明文配置口令、动态 JSON 遍布、全局单例 Tools）。差距不在内核而在**业务面广度**：br 有 30 个 addon / 217 张表 / 1227 个 API 的业务沉淀（支付、Excel、邮件、OSS、审批流），这些属于应用层组件而非基础库职责，不应以此否定基础库成熟度。
3. **结构判定**：未失控，但存在三处经实测确认的臃肿热点，按优先级：① yang-db `mysql/postgres` 双后端逐行复制（`identifier.rs` diff 仅剩引号与文档措辞）；② yang-db `query_builder.rs` 6,393 行（单 impl 块 ~2,300 行 / 54 个 pub 方法 + 内联测试 ~3,000 行）；③ yang-base 六个 1,500+ 行巨石文件。建议治理顺序见 §5。
4. **与 GitHub 成熟库对比**：工程治理（测试基建、CI 门禁、依赖管理、契约测试）处于上游水平，超过不少知名开源库的实际配置；架构路线上与 loco-rs 代表的组合路线有本质分野（见 §3.2），该分野由"对标 br"的目标决定，属合理选择。

---

## 1. 评估口径：从第一性原理定义"成熟可用"

对一个基础库，"成熟可用"的第一性分解为五个可验证维度，外加一个口径分层：

| 维度 | 核心问题 | 证据类型 |
|---|---|---|
| 正确性 | 行为是否符合契约 | 测试金字塔、property test、契约测试 |
| 失败可控 | 出错时是否可诊断、不扩散 | 结构化错误、fail-closed、无 panic 路径 |
| API 稳定 | 变更是否可预期 | 版本策略、兼容契约、变更日志 |
| 运维可见 | 线上是否可观测 | tracing/metrics/health |
| 演进成本 | 加需求/修 bug 的边际成本 | 结构清晰度、重复度、feature 耦合 |

**口径分层（关键）**：
- **L-A 自用生产级**：在自有项目（yang-system、SCS 迁移目标）中长期承载真实流量。→ 当前评估目标。
- **L-B 开源生态级**：外部用户采用、社区反馈闭环、semver 纪律、安全响应流程。→ 明确 non-goal；GitHub 成熟库（sqlx 26k+ commits 量级的外部验证）处于此层，直接对比"成熟度"不公平，只能对比**工程实践**。

## 2. 五维评估（增量复核，含实测数据）

### 2.1 正确性证据 —— 强（4.5/5）

实测（磁盘，不含 target）：

| 口径 | yang-db | yang-base | derive | runtime | 合计 |
|---|---|---|---|---|---|
| 生产代码行 | 13,354 | 32,654 | 714 | 1,404 | ≈48,126 |
| 测试总量行 | 14,813 | 21,921 | 0 | 218 | ≈36,952 |

- **测试/生产比 ≈ 0.73**，测试与生产同量级。测试基建组合为：proptest（condition/query_builder/sql_types）+ testcontainers 集成（MySQL/PG/Redis 各一套，`#[ignore]` 单线程）+ trybuild 编译失败用例（4 对）+ insta schema 快照 + 契约测试（`release_docs_contract.rs` 等）+ `verify_feature_isolation.py` feature 隔离。
- 对比锚点：多数成熟开源库具备单测+集成测试，但同时配齐 property test + 编译期负例测试 + 快照测试 + CI 契约脚本的并不多见。此维度是当前最强项。
- 已知缺口（源码已证实）：yang-runtime 仅 1 个 78 行契约测试，observability 双链路基本无自动化验证；M-1（测试代码 ~870 处 unwrap/expect）仍开放。

### 2.2 失败可控 —— 良好（4.0/5）

- 安全边界（源码已证实）：值始终参数绑定（`?`/`$N`）、标识符走 checked API、无 WHERE 的 UPDATE/DELETE fail-closed、统一 `InvalidArgument`；租户解析 fail-closed（header 仅声明、resolver 服务端校验）；step-up challenge 绑定用户+Action+资源+短过期。
- 错误模型：thiserror 结构化错误 + `PcgError` 式错误码惯例；F-03（BaseError 同义变体膨胀）仍为 P1/P2 开放项。
- 扣分项：BACKLOG M-1 测试 panic 面；`unsafe_code = "warn"` 而非 deny（测试豁免是合理动机，但生产路径依赖自律）。

### 2.3 API 稳定 —— 中上（3.5/5）

- 有 `VERSIONING.md` 版本兼容策略、API 契约文档、宏级编译期校验（`table!`/`field!`/`action!` 等 9 个宏）——方向正确。
- 不能声称稳定的部分（尚不能宣称）：0.1.x/0.2.x 阶段本就允许 breaking；无 CHANGELOG 机械化校验；文档版本漂移实测存在（`docs/yang-db.md` 标 0.1.4 vs Cargo.toml 0.1.5，`docs/yang-base.md`/README 标 0.2.0 vs 0.2.1），契约测试未卡住版本号一致性。
- 评分低于 07-26 基线预期的部分即文档同步纪律。

### 2.4 运维可见 —— 良好（4.0/5）

- tracing + tracing-log 桥接、metrics 门面（feature 可选）、OpenTelemetry OTLP、Prometheus exporter、`capabilities()/health_check()/pool_status()` 三后端对齐管理面——组合拳完整，接近 loco-rs/生产模板级配置。
- 缺口：性能治理半完工——shadow 基线存在（B-01），但 >3% 回归阻断未启用（B-07 开放）；本机 runner 变异系数 5–8% 不满足阻断方差条件（尚不能宣称有性能回归门禁）。

### 2.5 演进成本 —— 中等（3.0/5）—— 本评估主要短板

- 依赖治理好：workspace 统一版本、optional 化、MSRV 1.80 pin（tempfile/proptest/uuid 版本注释说明原因）、`deny.toml`、依赖图单向无环 derive→db→base→runtime。
- 但结构臃肿直接抬高兴建成本，详见 §4。`mysql` feature 106 处 `#[cfg]` 渗透 table/database/action，意味着"关掉 mysql 的最小构建"维护成本持续存在（有脚本兜底，属受控债）。

**综合：4.0/5，维持 L4 入口。**

## 3. 外部对标

### 3.1 对标 SCS br 生态（内核层逐项）

br 生态实证（`scs/scs-api`，~15.7 万行 Rust）：自研 `br-web-server`/`br-addon`/`br-fields` 等 14+ crate，三层字符串寻址插件树（addon→module→action），一份 fields 元数据同时驱动 DDL + 校验 + 管理后台 UI + Swagger，feature 门控 addon 组合交付。

| br 内核能力 | lib_yang 对应 | 判定 |
|---|---|---|
| fields 元数据 → 建表/校验/UI | definition `FieldType`/`FieldConfig` + schema_sync + UiCatalog（schema_version + revision） | ✅ 对齐且带版本化投影，强于 br |
| 字符串寻址三级插件树 | Catalog/Registry 构建期校验 + slot 预解析 | ✅ 对齐，且改静态校验（优于 br 运行期 match） |
| RBAC 标签/多租户 | action/auth + TenantResolver（fail-closed） | ✅ 对齐，信任模型更严 |
| Tools 全局资源（br 为全局单例） | ToolsBuilder→Tools 显式所有权 | ✅ 强于 br（可测试性、多实例） |
| Swagger/API 清单 | ApiCatalog + OpenAPI 3.1（feature） | ✅ 对齐 |
| Token/JWT | token/（含 revocation） | ✅ 对齐，br 未见撤销机制 |
| 文件上传/OSS | MultipartSpec/UploadedFile（越界拒绝） | ✅ 内核具备，OSS 业务组件缺 |
| 后台任务（br 为线程 sleep 轮询，无 cron） | 无 | ⚠️ 双缺：br 都只有轮询。建议作为**应用层**组件在 yang-system 验证后再下沉 |
| 审批流（br 为嵌入业务的 put_submit/review 状态机） | 无 | non-goal（业务层） |
| Excel/支付/邮件/发票解析 | 无 | non-goal（业务层，br-pay 等是独立 crate） |
| 代码生成器 | 无 | br 同样没有（仅 API 清单生成），非差距 |
| 动态 JSON 贯穿（json::JsonValue） | serde_json::Value 残留于 builtin action | ⚠️ 共同的债；AGENTS.md 已冻结不扩散 |

**结论**：内核层无结构性缺口。br 的护城河是业务沉淀量，复制路径是"在 yang-system 上逐个迁 addon"，而不是给基础库加能力。

### 3.2 对标 GitHub 成熟库（工程实践层）

| 维度 | 成熟库实践 | lib_yang 现状 | 判定 |
|---|---|---|---|
| 查询构建 | sqlx（运行期校验 SQL）/SeaORM（SeaQuery AST + ActiveRecord）/Diesel（编译期 schema） | yang-db 自建 builder，站在 sqlx 连接池上，定位≈SeaQuery 子集 | 合理：br-db 对齐要求受控 SQL 子集；但不与 SeaQuery 对标方法数量（已是既定 non-goal） |
| 全栈框架路线 | loco-rs「Rust on Rails」：**组合**社区 crate（axum+sea-orm+sidekiq-rs…）+ 约定 | yang-base：**自研**元数据内核（definition/action/table） | 路线分野由"替代 br 生态"目标决定，合理；代价是所有内核代码自维护，loco 路线把维护外包给社区 |
| 进程组装 | loco boot / spring-rs plugin | yang-runtime（1,544 行薄组装，`publish=false`） | 对齐且更克制 |
| 观测 | tracing/metrics/OTel 三件套 | 全套具备 | 持平 |
| CI 治理 | fmt/clippy/doc-test/MSRV/feature 矩阵/docker 服务 | 全部具备 + feature 隔离脚本 + 契约校验脚本 | 上游水平 |
| 发布纪律 | CHANGELOG、crates.io 发布、semver 工具（cargo-semver-checks） | 无 crates.io 发布、文档版本漂移存在 | 差距，但属 L-B 口径，L-A 下不阻塞 |

**第一性结论**：自用库的成熟度不取决于"像不像 sqlx"，而取决于§1 五维证据。与成熟库对比的真正价值是**借用其治理工具**（建议引入 cargo-semver-checks 类检查补齐 2.3 短板），而非追赶其 API 广度。

## 4. 结构臃肿评估（实测热点）

### 4.1 确认臃肿（建议治理）

| # | 位置 | 实测 | 问题本质 | 建议 |
|---|---|---|---|---|
| S1 | yang-db `mysql/` vs `postgres/` | `identifier.rs` 81 vs 53 行，diff 仅剩引号字符与文档措辞；condition.rs 1477 vs 685（PG 为 MySQL 子集移植） | 双后端复制粘贴，修 bug 必然漂移 | 抽 `dialect` 模块参数化引号/占位符/upsert 语法；两后端共享 Condition/SqlValue。**优先级最高**：改动机械、收益持续 |
| S2 | yang-db `mysql/query_builder.rs` | 6,393 行：`SqlGenerator`(62–987) + QueryBuilder 单 impl 块 ~2,300 行 54 个 pub 方法 + 内联测试 ~3,000 行 | 单类型承载 select/write/aggregate/batch/upsert 全职责 | 拆 `select.rs`/`write.rs`/`aggregate.rs` 子模块（同类型分文件 impl，不动公开 API）；内联测试迁 `__tests__/`（自身约定已存在） |
| S3 | yang-base 巨石文件 | table_query.rs 3,231（含 ~10 处 cfg(test) 段）、ui.rs 2,160、builder.rs 2,147、auth/mod.rs 1,708（Login/Refresh/Logout + 中间件 + 5 trait）、plugin/mod.rs 1,669（Plugin/Manager/Builder/Registry 一文件）、schema_sync.rs 1,550 | 同上，单文件职责超载 | 按文件逐个拆子模块；`schema_sync_tests.rs` 迁入 `__tests__/`（现放 `src/database/` 根，违反自身约定） |
| S4 | yang-base feature 矩阵 | 11 features、187 处 `#[cfg(feature)]`（mysql 106 / validator 36 / token 34）；`token` 一个 feature 拖 7 依赖 | 编译期组合复杂度 | 不删 feature（隔离是优点），但把 `token` 拆出 `token-core`（JWT+撤销）与 `token-redis`（存储后端）两级，降低最小构建面 |
| S5 | 新旧两代插件并存 | plugin/（旧生命周期）vs definition Addon（新链路） | F-01 开放项，双语义并存抬升认知成本 | 按 CORE 队列既定计划收敛，本报告不重复展开 |

### 4.2 不算臃肿（明确不改）

- **crate 边界（4+1）**：derive/db/base/runtime 单向无环，职责正交。**不建议再拆 crate**——yang-base 的 185 struct/689 pub fn 属于"一个框架"的内聚体量，拆散只会制造跨 crate 版本同步成本。
- **feature 数量本身**：11 个 feature 对标 br-addon 的 feature 组合交付模式，是必要的交付机制；问题在粒度（S4）不在数量。
- **测试体量**：36,952 行测试是资产不是赘肉。

### 4.3 杂物清单（低成本顺手项）

- `INSTALL.md.md`（扩展名重复，且 exclude 规则写成 `INSTALL.md` 匹配不上）。
- `examples/` 中 `test_min_max.rs` 命名像测试混入 examples。
- 文档版本漂移（docs/yang-db.md 0.1.4 vs 实际 0.1.5 等）——建议契约测试加版本号一致性断言。
- BACKLOG.md 状态表与 2026-07-15 对账节矛盾，引用前需对账。

## 5. 优化路径（按投入产出排序）

1. **S1 dialect 抽象**（yang-db，纯机械重构，消除一整类双端漂移风险）。
2. **S2 query_builder.rs 拆分 + 测试迁出**（同类型分文件 impl，零 API 变更）。
3. **文档版本一致性契约**（半小时级，堵 2.3 的实测漏洞）。
4. **S3 yang-base 巨石拆分**（按 table_query → ui → builder → auth → plugin → schema_sync 顺序，每个独立提交，遵循"一点一提交"协议）。
5. **S4 token feature 分层**（需 feature 隔离脚本回归验证）。
6. **B-07 性能回归阻断**（需先解决 runner 方差，属环境治理而非代码）。
7. **F-01/F-03 收敛**（沿用 CORE 长期队列）。

全程非目标：不追 br 的业务组件广度（支付/Excel/邮件属应用层）；不追 SeaORM/SeaQuery 的 API 数量；不拆 crate；不为"像开源库"而发版。

## 6. 证据边界声明

- **源码已证实**：§2 行数与文件结构、§4 全部热点、文档版本漂移、feature/cfg 计数。
- **测试已证实（历史基线引用）**：704+ 单测全绿、proptest/testcontainers 套件存在（06-27 再审记录）；本次未重新全量跑测试套件。
- **尚不能宣称**：长期真实流量承载、性能回归门禁（B-07）、对外开源级 API 稳定性、yang-runtime observability 链路的自动化验证。
