# 全仓库审查修复进度 — 2026-09-16（已完成）

> 承接 `docs/audit/2026-09-16-whole-repo-review.md`（64 项候选发现）。经核验工作流逐条
> 分析真实性后，**52 项判为真实（CONFIRMED/PARTIAL）**、**12 项判为误报/夸大（REFUTED）**。
> 核验详情（每条的 verdict/证据/修复方案）在：
> `C:\Users\16040\.claude\projects\D--code-lib-yang\c873c283-2199-4f15-8b44-ea55db8371a8\real-findings.md`
> （仓库外临时文件，约 2745 行；仓库内未提交）。

## 进度总览（最终）

| 状态 | 数量 |
|------|------|
| ✅ 已修复并提交 | **52** |
| ⛔ 误报/夸大（REFUTED，跳过） | 12 |
| **合计（真实 52 + 误报 12）** | **64** |

每项修复**独立提交**（commit message 带 finding ID，可溯源）。提交序列从 `fd1d3e1` 到
`f3a70bd`（均落在 `master`，未推送）。全部 52 项 + H3 均通过 `run_ci.py quick` 门禁
（fmt + 分 crate test + clippy `-D warnings` + doctest）。

## ✅ 已完成（52 项，按 crate）

**yang-pcg（13）**
- H8 缓存键纳入 constraints 摘要（`6472b3e`）
- M35 maze 门口 BFS 限内部格（`0bc9a9d`）
- M38 generate_chunk 预算 fail-closed（`081c686`）
- M39 ConfigDigest From→TryFrom（`fd1d3e1`）
- M40 排除区 exclude_rooms fail-closed + 默认值纠正（`c59b5a9`）
- M41 未接线旋钮标 RESERVED（`1b3385c`）
- M42 open_arena 空区间 panic 守卫（`289afba`）
- M43 doors O(E·R)→HashMap（`1b2140a`）
- M44 validate_spawn_spacing 迭代器入口（`82c6bce`）
- M45 tile 通道 O(T×R)→HashMap（`8d48fc8`）
- M46 maze 丢弃连通性洪水填充（`d55cb2d`）
- M47 validate_no_overlap 排序扫描（`987da84`）
- M48 build_chunks PcgResult + 去哨兵（`227f5f9`）

**yang-db（8）**
- H5 JOIN 表名渲染期 quote_identifier（`efece51`）
- M28 Redis 无效池参数标注不支持 + idle_timeout 语义（`8d1858f`）
- M29 raw-param u64 顶半区分类绑定（`b6ed5cc`）
- M30 纠正"已消除克隆"失真注释（`13d4841`）
- M31 set_expr 批量/upsert fail-closed（`e01169e`）
- M32 update_batch 按列数推导批大小（`8ddfd7d`）
- M33 where_and/or/having_cond 返回 Result（`46a5602`，**破坏性变更**）
- M34 标识符转义语义 + 失效文档链接（`09652ee`）

**yang-runtime（2）**
- M53 就绪探针共享 NOT_READY_CODE + fail_value（`634d307`）
- M54 shutdown 指标名纳入 RuntimeMetricNames（`4425260`）

**yang-base-derive（4）**
- H10 Radio query 字符串兜底（`725803e`）
- M49 params! 属性白名单（`1e1c82c`）
- M50 permission_mode 编译期报错（`fce1542`）
- M52 __private 重导出（`99f3f7a`）

**yang-base（25）**
- H3 附件流式返回 + 可配置大小上限（`caf3675`）
- H4 文件不存在不泄露服务器路径（`3fa3c74`）
- H6 写路径不再误跑读投影门禁（`2b0bd79`）
- H7 树构建加深度上限 + 显式工作栈 + 夹住 max_nodes（`32c893d`）
- M2 撤销水位线 TTL 取 max(access,refresh)（`0131106`）
- M3 StepUp proof 存储必填参数（`bb1abba`，**破坏性变更**）
- M4 移除 LogoutInput.refresh_token 死字段（`8dfb6a8`，**破坏性变更**）
- M5 new_symmetric 非 HMAC 返回 Err（`13e6084`）
- M6 弃用 refresh_access_token（`39a5a87`）
- M7 健康端点冲突改为构建期拒绝（`40b8ef6`）
- M10 非幂等方法默认不参与重试（`c3aab89`）
- M11 重试退避加总预算与抖动（`a6a493c`）
- M12 响应体流式读取加大小上限（`6b0df92`）
- M13 熔断器 host 状态加容量与空闲淘汰（`75dadf4`）
- M16 解码器 Schema 改持 Arc 避免每请求深拷贝（`a71cfab`）
- M17 读体失败区分 413 与 400（`8c60708`）
- M18 正则缓存迁入 Tools 并加上限（`7bd9306`）
- M19 函数索引 COLUMN_NAME 解码为 Option（`469feb5`）
- M20 UI 目录一次哈希与 views 预索引（`cb7920a`）
- M21 ValidationSpec min/max 服务端强制（`8aaf779`）
- M22 VARCHAR 超长构建期拒绝并收敛常量（`02ddf6f`）
- M23 CHECK 归一化只在引号外（`b63a143`）
- M24 TINYINT 仅 tinyint(1) 解码为 Bool（`26d28f1`）
- M26 消除 O(A×M) 并共享 Arc<TableDefinition>（`db686ca`）
- M27 topological_sort 镜像 registry 归一化（`f77140c`）

**附：本次顺带修复（非 64 项之一）**
- 兼容性契约测试版本断言 0.2.1/0.1.5 → 0.2.2/0.1.6（`f3a70bd`，M3/M4 版本号残留）
- validator 测试 RegexCache 导入按 feature 门控（`4829805`，M18 引入的 feature 隔离回归）

## 🔀 跨仓库同步（project/yang-system，独立 Git 仓库，分支 `main`）

上游破坏性变更（M33 的 `where_and/where_or/having_cond → Result`、M54 的
`RuntimeMetricNames` 新增 3 字段）会破坏 yang-system 编译，已在该仓库单独修复并提交：

- `500d36c` — M33：27 处链式 `where_and` 补 `?`（authz_version.rs 20、password_reset 6、
  repository.rs 1）；Cargo.lock 同步 tokio-util/http-body-util 依赖边。
- `2f95cec` — M54：`YANG_SYSTEM_METRIC_NAMES` 补 shutdown 三字段，`ShutdownBudget` 加
  `with_metric_names`，Prometheus 规则/演练 `yang_runtime_shutdown_phase_total` →
  `yang_system_shutdown_phase_total`。

yang-system 侧 `cargo check`、`cargo clippy --all-targets --all-features -- -D warnings`、
`cargo test --lib`（163 passed）、`cargo fmt --check` 均通过。

## ⛔ 误报/夸大（REFUTED，已跳过）

H1、H2、H9、M1、M8、M9、M14、M15、M25、M36、M37、M51

（理由详见 real-findings.md 各条 `adversarial` 字段；H1/H2/H9 是"无在库触发路径/应用层
责任"降级，M51 是死 `unreachable!()` 有 guard 不会触发。）

## ⚠️ 遗留事项（非本批 64 项范围，建议单列）

1. **release 契约测试因缺失文档失败**（仓库既有状态，本批未引入）：
   `crates/yang-base/tests/release_candidate_contract.rs`（2 failed）与
   `release_docs_contract.rs`（3 failed）读取 `docs/VERSIONING.md`、
   `docs/BASE_DB_CAPABILITY_MATRIX.md` 等文件，这些文件在仓库中不存在。
   属 release 流程文档缺失，与本审计无关，建议由发布流程补齐文档后再启用。
2. **破坏性变更需按 CONTRIBUTING 走版本边界并记 CHANGELOG**：M3、M4、M33（均已随
   版本号 0.2.2 落地，CHANGELOG 记录与否需确认）。
3. **集成测试**（MySQL/PostgreSQL/Redis 容器，`#[ignore]`）本批未实际运行，仅保证编译；
   动了 DB 行为（H5/M29/M31/M32/M33/M24/M19）的推送前需按 AGENTS.md 跑 `integration`。

## 验证状态

- ✅ `cargo check --workspace --all-targets --all-features --locked`（全 feature 全 target）
- ✅ `python scripts/run_ci.py quick`（fmt + 分 crate test + clippy `-D warnings` + doctest）
- ✅ feature 矩阵 17 组合（`check` + `test --lib` + `test --doc`，均 `-Dwarnings`）全通过
- ⚠️ `python scripts/run_ci.py full`：本机未安装 `cargo-deny` 与 `rustup 1.80.0` 工具链，
  止于 cargo-deny 阶段；其前的 fmt/test/clippy/doctest/依赖策略各阶段均已通过。推送前需
  在装有这两件工具的环境补跑 `full`。
- 核验结果源文件 `real-findings.md` 仍在用户家目录（仓库外，未提交）。
