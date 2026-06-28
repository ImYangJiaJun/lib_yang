# yang-base + yang-db 生产就绪度复核（再审）— 2026-06-27

> 范围：仅 `yang-base` + `yang-db`（`yang-pcg` 不在本轮范围）。分支 master，本机 cargo，全 feature。
> 上一轮基线：2026-06-24 审计（评分 75，后更新 v1.2 标 91）。

## 一、总判定与评分

| 项目 | 结论 |
|---|---|
| **生产就绪度** | **CONDITIONAL（有条件就绪）** |
| **综合评分** | **71 / 100** |
| **单元测试** | PASS — yang-base 379 / yang-db 325（合计 704），0 failed |
| **clippy 门禁（README 唯一钦定）** | **RED** — yang-db lib 7 errors（修复回归）+ test 131 errors，exit 101 |
| **cargo fmt** | **RED** — 80 文件漂移，exit 1 |
| **cargo audit** | **不可用** — cargo-audit 未安装，无本地依赖漏洞扫描 |

**核心结论**：单元测试 704 全绿证明功能稳定，且本轮确认大量历史高危项已真实修复。但「单测全绿 ≠ 生产就绪」——项目唯一被 README 钦定的质量门 `cargo clippy --all-targets --all-features -- -D warnings` 当前为 RED，且这是上轮修复扫尾引入的回归（对内部方法标了 `#[deprecated]` 却漏处理库内调用方）。该回归修复成本极低，但在它转绿之前不得宣称生产就绪。真实可达的 SQL 注入面在 yang-base 类型层（TableEntity 封闭枚举）下已收口；对直接消费 yang-db 公有 API 并传入外部字符串的调用方仍未缓解。

**转为 YES 的前置条件**：①clippy lib 7 errors 清零；②condition/裸 SQL 面补 checked 变体或文档化收口；③敏感 DTO 手写脱敏 Debug；④`cargo fmt --all` 消除漂移并补一次 cargo-audit。

## 二、各维度评分

| 维度 | 评分 | 一句话 |
|---|---|---|
| 安全（SQL 注入） | 67 | 写路径+值侧已防护；condition 静默回退与 order/group/join/value 裸 SQL 仍开放，yang-base 类型层已收口 |
| 认证 / 令牌 | 72 | 撤销/轮换/无匿名降级到位；6 个敏感 DTO Debug 明文（潜伏 CWE-312）、弱封装 pub 项仍在 |
| 性能 | 78 | pool recycle / HashSet O(1) 已修；bind 单次 clone 残留 |
| 逻辑正确性 | 80 | ErrorCategory/WATCH-Nil/重试重连/优雅关闭已修；PG Transaction 无 Drop |
| API / SemVer | 74 | 隔离级别可配、MySQL deprecated 完整；PG SqlValue 漏 non_exhaustive、PG execute 漏 deprecated、identifier 未重导出 |
| 并发 | 74 | WATCH 修复+隔离级别+多线程功能测试；无压力/竞态回归，TOCTOU 窄窗口 |
| 构建 / Lint 门禁 | 38 | clippy RED（修复回归）+ fmt RED + 无 cargo-audit |
| 文档 | 64 | AGENTS.md/yang-db.md/yang-base.md 多处 stale |

## 三、构建 / 测试 / Lint 实测 ground truth

- `cargo test --lib`：**PASS**。yang-base 379 passed / 0 failed / 8 ignored；yang-db 325 passed / 0 failed / 1 ignored。
- `cargo fmt --all -- --check`：**FAIL（exit 1）**。80 文件漂移（最重 `crates/yang-base/src/plugin/mod.rs` 约 290 diff 块）。项目有意无 CI，故仅代码卫生问题，不致流水线失败。
- `cargo clippy --all-targets --all-features -- -D warnings`：**FAIL（exit 101）**。
  - **LIB 7 errors**：`init()`/`create_table()`/`drop_table()` 内部调用自身 `#[deprecated]` 的 `execute()` 未加 `#[allow(deprecated)]`（MySQL `database.rs:281/289/300` + PG `database.rs:264/272/283`，共 6 处），外加 `doc_lazy_continuation`（`mysql/condition.rs:166`）。**这是修复扫尾引入的回归**：标了 deprecated 却漏处理库内调用方。
  - **LIB TEST 131 errors**：测试 `unwrap()/expect()`（既知 M-1）+ 大量 `where_*_unchecked`/`having_cond_unchecked` deprecated 调用。
  - 复核更正：`manual_range_contains` 子项**确实存在**——`error.rs:502` 的测试断言 `assert!(c >= 800000 && c < 900000)` 触发，属上述 LIB TEST 131 errors 之一；本轮仅更正 doc lint 的定位为 `mysql/condition.rs:166`（而非 `database.rs`）。
- `cargo audit`：**不可用**（no such command: audit）。历史 rsa RUSTSEC-2023-0071（经 sqlx-mysql 引入）未被检测。
- `cargo tree -d`：存在多版本依赖（windows-sys 等），与历史 S-H11/S-L10 一致。

## 四、相对 2026-06-24 审计的四类清单

### 4.1 已确认修复（FIXED）

**安全 / SQL（yang-db）**
- PG INSERT RETURNING 列名已 quote_identifier（`postgres/query_builder.rs:1493-1497`）。
- 聚合 sum/avg/min/max field 已 quote_identifier（mysql 1764/1830/1912/1994；pg 1414/1429/1445/1461）。
- build_select 表名已 quote_identifier（mysql 151；pg 166）。
- drop_table/table_exists 已参数化/转义（mysql 298-317；pg 281-292）。
- 值侧绑定全部 ?/$N 占位符，无回归。
- MySQL Database::execute/query、Transaction::execute 均标 `#[deprecated]` 且有 `*_with_params` 替代。

**认证 / Token（yang-base）**
- S-H7：refresh_access_token 已用 verify_token_checked（`manager.rs:507`）。
- S-H8：RefreshAction 已 rotate_refresh_token 完整轮换并返回 TokenPairResponse（`auth.rs:461-470`）。
- S-NEW-AUTH-4：GetAction/SelectAction 改 ok_or_else(Unauthorized)，无匿名降级（`get.rs:52-54`、`select.rs:155-158`）。

**逻辑 / 性能 / API**
- L-H1/H2 ErrorCategory 修正（`error/mod.rs:677/688/691`）。
- L-NEW-1 RedisTransaction::exec 重试中重新获取连接（`redis/transaction.rs:344-375`）。
- L-NEW-2 graceful_shutdown 解绑 mysql feature（`lifecycle.rs:70/82`）。
- WATCH-Nil 冲突走 exec + is_watch_conflict 不再静默吞（`redis/transaction.rs:334`，is_watch_conflict 调用处）。
- PG build_update 非整型列内联 NULL 字面量（`postgres/query_builder.rs:518-524`）。
- P-H1 Redis pool recycle 改 idle_timeout(300s)（`redis/client.rs:89`）。
- P-H6 FieldPermissions 改 HashSet O(1)（`field_config.rs:8`）。
- DbError 已有 code()/category()/is_retryable() + `#[non_exhaustive]`。
- NG-2 隔离级别可配（`isolation.rs` + transaction_with_isolation, mysql:258 pg:241）。
- L-C1(MySQL) Transaction Drop warn 已实现（`mysql/transaction.rs:223-231`）。
- L-C2(MySQL) GlobalDatabase::init 经 DatabaseConnection 保留 source 链（`global.rs:92`）。
- A-C3 三个测试文件已 token/mysql feature gate。

### 4.2 仍开放（OPEN / PARTIAL）

**HIGH（影响生产姿态）**
- condition_to_sql_owned 用 safe_quote_identifier，对非法标识符仅 `log::warn` 后输出 RAW 字段（`mysql/condition.rs:171-176`，pg 同构）——为 JOIN 表达式 `a.b` 的设计回退，同一回退也放行恶意载荷。
- build_order_by/build_group_by/build_joins(表名+整段 ON)/value(field) 全程不转义（mysql 244-283/1655，pg 同构）；公有入口 join()/order_by()/group_by()/value() 在 yang-db 层不转义。
- create_table(create_sql)/init(sql_script) 执行任意 DDL，无 `#[deprecated]`、无安全属性；其内部调用 `#[deprecated]` execute() 未 `#[allow]`——即 clippy lib 7 errors 来源。
- S-M8..M12：6 个敏感 DTO 仍 `#[derive(Debug)]` 明文（`auth.rs:55/67/76/84/94`、`token/mod.rs:85`），仅 TokenManager 本体手写遮蔽。当前无生产 `{:?}` 调用（审计走 FNV-1a 指纹），属潜伏 CWE-312。

**MEDIUM**
- L-C1 PG 端 Transaction 无 `impl Drop`（`postgres/transaction.rs` 零 Drop）——sqlx 底层仍自动回滚，仅缺诊断日志，与 MySQL 不对称。
- PG SqlValue 漏 `#[non_exhaustive]`（`postgres/condition.rs:10-11`，MySQL 孪生已保护）；PG Transaction::execute 漏标 `#[deprecated]`。

**LOW / PARTIAL**
- parse_token_unsafe 已 `#[deprecated]` 但仍 pub（`manager.rs:434-442`）。
- ActionContext.user 仍 pub，仅注释警告（`context.rs:213`）。
- L-C2/NEW-B GlobalRedis::init(`global_redis.rs:107`，含 TODO(P1-4)) 与 health_check(:157) 用 `e.to_string()` 截断错误链；约 30 个 Redis 操作方法绕开 `From` 路径，未用已存在的 RedisOperationDbError 变体。
- identifier 工具未在 lib.rs 重导出，仅 `yang_db::mysql::quote_identifier` 子路径可用。
- P-H4/P-NEW-1 String/Bytes/JSON bind 单次 clone 未消除（每值在绑定宏处 clone 一次；mysql 30/32、pg 35/37、事务路径 mysql 553/554、pg 552-576）。
- A-C2 Redis close() 仍同步 fn，与 MySQL async close 不一致（`redis/client.rs:137`）。
- A-H1 health_check 返回类型不一致（MySQL/PG Result<()> vs Redis Result<bool>）。
- L-L4 u64 as i64 非饱和转换（`revocation.rs:83/131`，溢出需 2^63 秒不可达，code smell）。
- L-L2 leeway 硬编码 0（复核更正：实为正确的严格姿态，非缺陷）；sub/jti 不在 required_spec_claims（但 TokenClaims 字段非 Optional，serde 反序列化已兜底）。
- L-M6 verify_token_checked 两次独立 Redis GET 非原子（`revocation.rs:178/182`，亚毫秒 TOCTOU 窗口，JWT-over-Redis 标准权衡）。
- C6 仅 happy-path 并发功能测试，全仓 grep stress 零命中——无高迭代压力/竞态回归。

### 4.3 新发现（NEW）

- RefreshAction 对同一 refresh token 双重 verify_token_checked（`auth.rs:461` + rotate 内 `manager.rs:555`），每次刷新 4 次 Redis 往返 + 两次校验间窄 TOCTOU。
- `cargo fmt` 漂移规模为 80 文件（上轮断言「约 40+」显著低估）。
- cargo-audit 工具链缺失，无任何本地依赖漏洞扫描手段。

### 4.4 回归（REGRESSION）

- **clippy lib 7 errors**：上轮对 execute()/query() 加 `#[deprecated]` 时漏处理 init()/create_table()/drop_table() 这些库内调用方，导致 lib 自身在 `-D warnings` 下编译失败。这是本轮判定 CONDITIONAL 的首要原因。
- PG Transaction::execute 漏标 `#[deprecated]`——MySQL 对应已标，方言不一致。

## 五、按优先级的生产阻断项与修复路线

1. **[必修，门禁] clippy lib 回归**：为 `init()/create_table()/drop_table()` 内对 `self.execute()` 的调用加 `#[allow(deprecated)]`（或改走未弃用的内部私有执行函数），并修 `condition.rs:166` doc 缩进。目标：`cargo clippy --all-targets --all-features -- -D warnings` 转绿。
2. **[高，安全] condition + 裸 SQL 面收口**：为 condition_to_sql_owned 增 checked 变体（非法标识符返回 `InvalidArgument` 而非 RAW 回退）；为 value()/create_table()/init() 补 `#[deprecated]` 或 doc 安全警告；join ON / order / group 提供 quoted 变体；lib.rs 重导出 quote_identifier/quote_qualified。
3. **[高，安全] 敏感 DTO 脱敏 Debug**：为 6 个 DTO 手写 Debug 输出占位符或用 secrecy 包装 password/token 字段。
4. **[中] PG 对称性**：PG Transaction 补 `impl Drop`（warn）；PG SqlValue 补 `#[non_exhaustive]`；PG Transaction::execute 补 `#[deprecated]`。
5. **[低] 可观测性与卫生**：error/mod.rs 新增 RedisConnectionDbError 变体并让 GlobalRedis 各方法经 `From` 保留 source 链；`cargo fmt --all` 消除 80 文件漂移；安装 cargo-audit 跑一次并记录 rsa RUSTSEC-2023-0071 处置。
6. **[低] 封装收紧**：parse_token_unsafe → pub(crate)/feature gate；ActionContext.user → pub(crate) + 受控 setter。

## 六、正向发现

- 值侧端到端参数化，写路径标识符防护落地，注入面的主战场已被 yang-base TableEntity 封闭枚举从类型层堵死——这是本库自身消费者的核心安全保障。
- Token 撤销/黑名单机制完整（revoke/is_revoked/verify_token_checked + 完整轮换 + 登出双 token 撤销）。
- 错误分类体系成熟（code/category/is_retryable + `#[non_exhaustive]`，错误码表与代码精确一致）。
- Redis 弹性修复扎实（WATCH-Nil 不再静默吞、重试重连、优雅关闭解绑 feature）。
- 隔离级别可配且无注入面；单元测试 704 全绿守护回归。

---

*本报告基于 2026-06-27 本机实测与三轮对抗式代码复核。评分采用加权：构建/Lint 门禁与安全维度权重较高，因 README 钦定的 clippy 门为生产就绪的硬前置。*
