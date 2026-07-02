# 第一性原理全量文档复核报告 — 2026-06-29

## 概要

对三份 lib_yang 缺陷/优化对照文档执行第一性原理逐条复核：32 批次 Sonnet agent 各自打开源码逐条核验 ~198 条目（展开为 278 个独立断言），每条非 CONFIRMED 判定再经独立 Sonnet 二审尝试推翻初审，Opus 终审综合。

**核验方法**：每一条目均亲自打开被引源码核实（`codegraph_explore` / `codegraph_search` / `Read` / `Grep`），绝不相信文档自身的任何断言或 file:line 引用——审计文档本身也会出错。

---

## 一、整体结果

```
278 条断言经逐条对抗式复核
├── 250 条 (89.9%) ✓ CONFIRMED — 断言与代码完全相符
├──  12 条 (4.3%)  ✗ 真实不准确 — 已通过二审确认，需更正
├──  12 条 (4.3%)  ⊗ 审计误判 — 初审判为不准确但二审推翻（假阳性，已排除）
└──   4 条 (1.4%)  ? 二审未覆盖 — 因 API 429 限流未完成对抗复核
```

**整体可信度：89.9%。** 更正本文所列 12 条不准确项后，三份文档可作为生产就绪度改进的可靠基线。

### 涉及文档

| 文档 | 路径 | 条目类型 |
|---|---|---|
| yang-base / yang-db 优化对照指南 | `docs/yang-base-db-optimization-guide.md` | AUTH / LOGIC / QRY / ERR / PERF / API / PLUG / TEST / CONC / OVF / SUP / LEAK |
| yang-pcg 优化对照指南 | `docs/yang-pcg-optimization-guide.md` | OPT-D / OPT-L / OPT-R / OPT-P / OPT-A / OPT-S / OPT-T / OPT-Q |
| yang-base + yang-db 再审报告 | `docs/audit/2026-06-27-yang-base-db-reaudit.md` | 评分 / FIXED / OPEN / NEW / REGRESSION |

---

## 二、12 条真实不准确（需更正）

### §2.1 yang-base-db 优化对照指南（6 条）

#### OVERVIEW-COUNT（severity: MEDIUM）
- **位置**：第 11 行
- **问题**：声称「保留优化项：87 条（76 条原始 + 11 条补全）」，正文 grep 统计实有 **86 条**（AUTH 11 + LOGIC 5 + QRY 6 + ERR 4 + PERF 15 + API 18 + PLUG 5 + TEST 11 + CONC 2 + OVF 2 + SUP 3 + LEAK 4 = 86）。PERF-1 已被驳回（dispatch() O(#actions) 非问题），不应计入保留项。
- **证据**：`grep '^### (AUTH|LOGIC|QRY|ERR|PERF|API|PLUG|TEST|CONC|OVF|SUP|LEAK)-'` 全文命中 86 个标题。PERF 类从 PERF-2 始共 15 条；PERF-1 仅出现在「核验后被剔除」清单（第 31 行）。
- **更正**：第 11 行 `87 条（76 条原始 …）` → `86 条（75 条原始 …）`

#### DIST-TABLE-PERF（severity: MEDIUM）
- **位置**：第 20 行分布表
- **问题**：分布表标「性能与分配 \| 16」，但 Section 6 正文仅 PERF-2 至 PERF-16 共 **15 条**。PERF-1 已明确驳回，计入分布表与文档自身的剔除声明矛盾。
- **更正**：第 20 行 `性能与分配 | 16` → `性能与分配 | 15`

#### MATRIX-PERF2-ID（severity: MEDIUM）
- **位置**：高杠杆速览矩阵 P0/P1 × M 单元格
- **问题**：矩阵写「PERF-2（User 权限 HashSet）」，但 PERF-2 实际内容是「verify_token_checked 两次 Redis 读串行（应 pipeline）」（P2）。User 权限 HashSet 是 **PERF-4**（P1）。编号与内容错位。
- **证据**：PERF-2 源码为 `revocation.rs:175-188`（Redis pipeline）；PERF-4 源码为 `context.rs:41-43, 60-67`（User::has_permission/has_role O(n) Vec 扫描）。
- **更正**：P0/P1 × M 单元格 `PERF-2（User 权限 HashSet）` → `PERF-4（User 权限 HashSet）`；P2 × S 单元格追加 `· PERF-2（Redis pipeline）`

#### MATRIX-QRY4-PRIORITY（severity: HIGH）
- **位置**：高杠杆速览矩阵
- **问题**：矩阵将 QRY-4 放在 **P0/P1 × L/XL** 列，但条目正文标注 **P2 + effort M**（窄方案仅做 SqlParam→SqlValue 替换，单文件 59 处引用，非大重构）。优先级和 effort 同时错位，可能误导排期决策。
- **证据**：条目正文第 200 行标注 `MEDIUM · P2 · effort M（窄方案）`；窄方案仅消除 SqlParam（6 变体）改用 SqlValue（9 变体），59 处引用限于 `table_query.rs` 单文件。
- **更正**：矩阵 L/XL 列删除 QRY-4，仅保留 QRY-3；P2 × M 列补充 QRY-4

#### MATRIX-PLUG3-EFFORT（severity: MEDIUM）
- **位置**：高杠杆速览矩阵 P2 × S 列
- **问题**：矩阵将 PLUG-3 放在 S 列，但条目标注 effort M。源码核实确为 M 级：涉及 `graceful_shutdown` 签名改动（破坏性公共 API）、`initialize_all` 连锁波及、11+ 测试迁移、文档示例更新。
- **证据**：`lifecycle.rs:70` graceful_shutdown 接受 `Option<&PluginManager>`；`database/initializer.rs:190` initialize_all 同样引用 PluginManager；`plugin/mod.rs` 有 11 个测试使用 `PluginManager::new()`；`lib.rs:35` 快速开始示例也使用 PluginManager。
- **更正**：将 PLUG-3 从 S 列移至 M 列

#### MATRIX-TEST7-EFFORT（severity: MEDIUM）
- **位置**：高杠杆速览矩阵 P2 × S 列
- **问题**：矩阵将 TEST-7（PG 事务 Drop）放在 S 列，但条目标注 effort M。更优解含并发隔离测试（`tokio::spawn` 双 task + PG 隔离级别 + Docker），远超 S 级。MySQL 侧有 Drop impl 而 PG 侧缺失。
- **证据**：`crates/yang-db/src/postgres/transaction.rs` 全 659 行无 `impl Drop for Transaction`；MySQL 侧 `mysql/transaction.rs:223-231` 有 Drop impl。
- **更正**：将 TEST-7 从 S 列移至 M 列

---

### §2.2 yang-pcg 优化对照指南（4 条）

#### s0-guide-ref-claude-md（severity: LOW）
- **位置**：第 15 行
- **问题**：引用不存在的文件 `crates/yang-pcg/CLAUDE.md`。该 crate 从未有过 CLAUDE.md，RNG 确定性契约声明实际记载于 `crates/yang-pcg/AGENTS.md` 第 84 行。
- **证据**：Glob 搜索 `crates/yang-pcg/CLAUDE.md` 零结果。`crates/yang-pcg/AGENTS.md` 第 82-86 行 CONVENTIONS 区块声明 RNG stream names 是确定性契约的一部分。
- **更正**：`crates/yang-pcg/CLAUDE.md 已声明` → `crates/yang-pcg/AGENTS.md（第 84 行）已声明`

#### s0-guide-ref-audit（severity: LOW）
- **位置**：第 13 行
- **问题**：引用 `PRODUCTION_AUDIT_2026-06-24.md` 缺少路径前缀。同段第 12 行引用 `docs/BACKLOG.md` 带有 `docs/` 前缀。审计文件实际位于 `crates/yang-pcg/docs/`，读者按第 12 行模式去 `docs/` 查找会失败。
- **证据**：Glob 确认 `PRODUCTION_AUDIT_2026-06-24.md` 位于 `crates/yang-pcg/docs/`，仓库根 `docs/` 下无任何 `PRODUCTION_AUDIT*` 文件。
- **更正**：`PRODUCTION_AUDIT_2026-06-24.md` → `crates/yang-pcg/docs/PRODUCTION_AUDIT_2026-06-24.md`

#### OPT-L-05（severity: LOW）
- **位置**：条目代码位置引用
- **问题**：条目称 `grammar/selector.rs:130-174` 覆盖 `compute_adjusted_weight` 和 `select`，但 `select` 函数实际在 **89-127** 行，不在 130-174 范围内。错误消息生成代码（109-113 行）被行号范围遗漏。行为断言（NaN 绕过短路 + 错误消息无区分）完全准确。
- **证据**：`selector.rs:89-127` 为 select 函数（含 109-113 的 total_weight 检查与错误消息）；`132-174` 为 compute_adjusted_weight（NaN 绕过点）。128-131 是函数间空白。
- **更正**：位置改为分立列出 `grammar/selector.rs:89-127`（select）+ `grammar/selector.rs:132-174`（compute_adjusted_weight），或合并为 `89-174`

#### OPT-R-12（severity: MEDIUM）
- **位置**：条目核心断言
- **问题**：条目称「传入含 NaN f32 的未归一化 config 时 panic」，但 **serde_json 1.0.140 默认行为下 f32::NAN 被序列化为 JSON null 而不会触发 Err**，`.expect()` 不会 panic。真正的风险是 NaN 被静默序列化为 null 后产生**错误的哈希/种子**，而非 panic。核心触发条件描述有事实错误。
- **证据**：根 `Cargo.toml:20` 指定 `serde_json = "1.0.140"` 无 `arbitrary_precision` feature。serde_json 默认 Serializer::serialize_f32 对 NaN/Infinity 写 null 字节而不返回错误。生产路径 `generator.rs:51-57` 先 `validate_request` 后 `digest`，故安全。
- **更正**：将「传入含 NaN f32 的未归一化 config 时 panic」改为准确表述：若 serde_json 序列化因任何原因失败（虽对全派生 Serialize 的 GenerationConfig 概率极低），`.expect()` 直接 panic 而非返回 PcgError。注意 serde_json 默认行为下 f32::NAN 被序列化为 JSON null 而不触发错误（NaN 不会导致 panic，但会基于含 null 的 JSON 产生错误的哈希/种子）。生产路径先调 `validate_request` 故安全。

---

### §2.3 再审报告（2 条）

#### 4.2-L3（severity: MEDIUM）
- **位置**：§4.2「仍开放」清单
- **问题**：再审报告称「约 30 个 Redis 操作方法使用 e.to_string() 截断错误链」，实测为 **42 个独立方法 / 43 处调用点**（set 含 setex/set 两分支）。偏差约 40%，在审计文档中属于有意义的精度不足。核心实质主张（已存在 RedisOperationDbError 变体但未被使用、From 路径被绕过）验证通过。
- **证据**：grep `global_redis.rs` 的 map_err 模式命中 43 行，对应 42 个方法：`health_check, set(setex+set), get, del, exists, expire, ttl, persist, keys, hset, hget, hdel, hgetall, hexists, hlen, lpush, rpush, lpop, rpop, llen, lrange, sadd, srem, sismember, smembers, scard, zadd, zrem, zcard, zrange, incr, decr, incrby, decrby, incrbyfloat, hincrby, hincrbyfloat, mget, mset, zrange_with_scores, zrevrange, zincrby`。`error.rs:139` 确有 `RedisOperationDbError(#[source] yang_db::DbError)` 变体。
- **更正**：将「约 30 个」改为「42 个 Redis 操作方法（共 43 处调用点，其中 set 含 setex/set 两分支）使用 `.map_err(|e| BaseError::RedisOperationFailed(e.to_string()))` 绕开 From 路径，未用已存在的 RedisOperationDbError(#[source] yang_db::DbError) 变体。另 init(global_redis.rs:107) 使用 RedisConnectionFailed(e.to_string()) 截断连接错误链，含 TODO(P1-4)。」

#### 4.2-L5（severity: LOW）
- **位置**：§4.2「仍开放」清单
- **问题**：(1) pg/transaction.rs bind 分支跨越 `bind_execute_param`（552-560）和 `bind_scalar_param`（566-581）两个函数，审计范围 552-576 漏了 577 行 Json clone 调用；(2) MySQL Json 走 `to_string()`（额外序列化分配）而 PG Json 走 `clone()`（Arc-less Value 的引用计数 clone），统称「clone」对 MySQL 不精确。
- **证据**：`pg/transaction.rs:552/554/556` 为 bind_execute_param 的 String/Bytes/Json，`:575/576/577` 为 bind_scalar_param 的同组。`mysql/query_builder.rs:34` 和 `mysql/transaction.rs:555` 的 Json 为 `to_string()`（序列化分配，非 clone）。
- **更正**：更正为 `mysql query_builder.rs:30/32/34（Json 为 to_string()）、pg query_builder.rs:35/37/39（Json 为 clone()）、事务路径 mysql transaction.rs:553/554/555（Json 为 to_string()）、pg transaction.rs:552/554/556 + :575/576/577（Json 为 clone()）`

---

## 三、12 条审计误判（假阳性，已排除）

以下条目初审被判为不准确，但二审独立核实后**推翻**——条目本身准确，无需修改。

| # | ID | 初审判定 | 二审结论 | 误判原因 |
|---|---|---|---|---|
| 1 | MATRIX-AUTH5-EFFORT | effort S→M | **WRONG**：矩阵 S 列正确，条目自身 effort M 才是错的 | 二审核实 AUTH-5 实际改动仅 ~5 行 + 3 行调用替换，确为 S 级 |
| 2 | MATRIX-PLUG4-EFFORT | effort S→M | **WRONG**：矩阵 S 列正确，条目 effort M 偏高 | 编译器驱动机械替换，5 文件 20 个触摸点，60-90 分钟 |
| 3 | MATRIX-TEST5-EFFORT | effort S→M | **WRONG**：矩阵 S 列正确，条目 effort M 偏高 | graceful_shutdown 仅 ~25 行，close() 均为 no-op，零外部依赖 |
| 4 | QRY-3 | 条目需补注 | **WRONG**：条目已含核验修正信息和精确数字 | 第 193 行已有「核验下调（HIGH→MEDIUM，含失实修正）」 |
| 5 | TEST-9 | Relaxed 描述有误导 | **WRONG**：描述性括号被过度解读 | 文档从未暗示 Relaxed ordering 有安全风险 |
| 6 | OPT-R-09 | 行号偏移/表述夸大 | **WRONG**：行号精确，表述准确 | 初审混淆了 load_config 签名行和 read_to_string 行 |
| 7 | fmt-drift-hunk-count | per-file 数字未核实 | **CONFIRMED**：全部 4 个文件 hunk 数精确一致 | 二审 `cargo fmt --all -- --check` 逐文件实测 |
| 8 | dim-score-correctness-80 | 行号引用不存在 | **WRONG**：初审混淆了跨 crate 的同名文件 | yang-base `error/mod.rs`(1140行) ≠ yang-db `error.rs`(506行) |
| 9 | dim-score-docs-64 | UNVERIFIABLE | **WRONG**：底层 staleness 事实可通过代码对照证伪/证实 | 二审找到 4 项具体可复现 staleness 证据 |
| 10 | 4.2-H2 | 行号区间不精确 | **WRONG**：区间写法是标准技术写作手法 | 每个行号经逐行核实均精确命中 |
| 11 | 4.2-H4 | 编号范围歧义 | **CONFIRMED**：核心主张完全成立 | 「S-M8..M12」标签歧义不等于事实错误 |
| 12 | s0-guide-test-count | 优化指南数字过时 | **WRONG**：错指了 stale 文档 | 优化指南 309 正确，`crates/yang-pcg/AGENTS.md` 的 307 才过时 |

### 系统性问题：effort 标注系统性偏高

本次复核暴露出 effort 标注存在**系统性偏高**倾向：AUTH-5（条目标 M 实为 S）、PLUG-4（条目标 M 实为 S）、TEST-5（条目标 M 实为 S）三个条目在初审中均被误判为「矩阵 S 列放错了、应移至 M」——但实际上条目自身的 effort M 才是错的。**建议对全部条目的 effort 做一次性对齐审计。**

---

## 四、跨文档一致性问题

1. **yang-base-db 优化指南内部计数不一致**：总览称 87 条/性能 16 条，正文实有 86 条/性能 15 条（PERF-1 已驳回），导致总计数与分布表同时虚增 1。

2. **矩阵 effort 标注与条目正文 effort 标注存在 5 处不一致**（AUTH-5/P2-S vs M、PLUG-3/P2-S vs M、PLUG-4/P2-S vs M、TEST-5/P2-S vs M、TEST-7/P2-S vs M），其中 3 处经二审确认系条目正文 effort 标注偏高，2 处（PLUG-3、TEST-7）确系矩阵放置偏低。

3. **PCG 优化指南引用不存在的文件**：`crates/yang-pcg/CLAUDE.md` 从未存在（正确文件为 `crates/yang-pcg/AGENTS.md`），`PRODUCTION_AUDIT_2026-06-24.md` 路径缺少 `crates/yang-pcg/docs/` 前缀。

4. **AGENTS.md 测试计数过时**：`crates/yang-pcg/AGENTS.md` 第 91 行写 307 passed，实测为 **309**。优化指南第 24 行和第 938 行的 309 正确。

5. **跨 crate 同名文件歧义风险**：再审报告引用代码位置时未注明完整 crate 路径（如 `error/mod.rs`），初审因此混淆了 yang-base 的 `error/mod.rs`（1140 行，`BaseError::category()`）与 yang-db 的 `error.rs`（506 行，`DbError`）——建议以后引用代码位置时统一加注完整 crate 路径。

6. **反向误判模式**：当两份文档数字不一致时（如 s0-guide-test-count），初审错误指认了正确的文档为 stale，实际 stale 的是另一份——提示未来审核需要跨文档交叉验证而非单独判断。

---

## 五、4 条二审未覆盖项

以下条目初审均判为 PARTIAL 或 UNVERIFIABLE，但二审因 API 429 限流未能执行对抗复核。状态标记：**待后续复核**。

| ID | 文档 | 初审判定 | 初审提出的问题 |
|---|---|---|---|
| s1.2-excl-qc05 | pcg 指南 §1.2 | PARTIAL | 「4 处冗余 summarize_connectivity」实际至少 5-6 处 |
| OPT-R-06 | pcg 指南 | PARTIAL | critical_cursor_x 溢出的数值示例 1100×2000 数学上不可能触发 |
| OPT-T-07 | pcg 指南 | PARTIAL | NaN/Inf obstacle_density 已被 range check 拒绝，缺少显式测试但非功能缺口 |
| 4.3-NEW-3 | 再审报告 | UNVERIFIABLE | cargo-audit 为独立安装的环境工具，无法仅从代码验证 |

---

## 六、方法论说明

1. **逐条对照源码**：每一条目均调 `codegraph_explore` / `codegraph_search` / `Read` / `Grep` 亲自打开被引代码核实 file:line 存在性 + 行为断言真伪。
2. **对抗式二审**：每条非 CONFIRMED 判定的条目由**另一个独立的** Sonnet agent 执行「尝试推翻初审」——排除审计自身放大/误判/过度挑剔的假阳性。
3. **Opus 终审综合**：汇总全部核验 + 二审结果，识别跨文档不一致与系统性倾向。
4. **Clippy/Fmt ground truth**：构建类断言使用 `cargo clippy --all-targets --keep-going` 和 `cargo fmt --all -- --check` 实测数据而非二手引用。
5. **四条目二审未覆盖**的初审结论应视为**未经验证**，不直接采纳也不直接否决。
