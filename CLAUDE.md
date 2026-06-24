# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 沟通语言

- 所有面向用户展示的对话内容、思考过程、说明文字、commit message、PR 描述一律使用中文。
- 代码中的注释、文档字符串、用户可见的错误信息保持中文风格（与现有代码一致）。
- 工具调用的参数（例如 `bash` 命令、`grep` 模式）按需使用英文，不必强行翻译。

## 仓库总览

`lib_yang` 是一个 Rust workspace（`resolver = "2"`，edition `2021`，共享依赖集中在根 `Cargo.toml` 的 `[workspace.dependencies]`），包含四个 crate：

- `crates/yang-db/` — MySQL 查询构建器 + Redis 客户端，下游 crate 的数据访问基础。
- `crates/yang-base/` — 在 `yang-db` 之上的后端服务原语：插件、全局 DB/Redis、表配置/查询、Action 调度、路由，以及可选的 HTTP 客户端和 JWT Token。
- `crates/yang-base-derive/` — `yang-base` 的 proc-macro crate，提供 `#[derive(TableEntity)]` 与 `#[derive(Action)]`（类型化 Action 系统的派生基础设施）。仅被 `yang-base` 依赖。
- `crates/yang-pcg/` — UE5 / Roguelike 程序化地图生成库（确定性 PCG 管线）。与 db/base 解耦：PCG 不依赖它们。

每个 crate 根目录都有一份 `AGENTS.md`，部分子模块（`yang-base/src/action`、`yang-base/src/table`、`yang-pcg/src/terrain`）还有更细粒度的 `AGENTS.md`。**修改对应模块前先读那份 AGENTS.md**，里面记录了该模块的 hotspot、anti-pattern 和约定，本文件不重复展开。

## 常用命令

```bash
# 检查（CI 缺失，提交前最好都过一遍）
cargo fmt
cargo check
cargo clippy --all-targets --all-features -- -D warnings    # 项目唯一被 README 钦定的检查

# 单元测试（colocated 在 src/.../__tests__/）
cargo test --lib                       # 整个 workspace
cargo test --lib -p yang-base          # 单 crate
cargo test --lib -p yang-db
cargo test --lib -p yang-pcg

# 单测试或单测试函数
cargo test --lib -p <crate> <test_name>
cargo test --test <integration_test_file>

# Docker / 集成测试（默认 #[ignore]，必须单线程）
cargo test --test <name> -- --ignored --test-threads=1

# 示例
cargo run --example <name> -p <crate>
```

工具链上没有 `rust-toolchain.toml` / `rustfmt.toml` / `clippy.toml` / Makefile / justfile / Dockerfile / docker-compose / CI 配置 — 这是有意的现状，不要新增除非用户明确要求。

## 架构要点（多文件视角）

### yang-base 的请求执行链路
请求经由 `router::ModuleRouter::dispatch` 进入：先穿过注册的中间件链（`router::middleware` 的 `Middleware`/`Next` 洋葱模型，最外层），链尾 `authorize_and_dispatch` 做内置鉴权 + Action 派发。Action 现在是**端到端类型化**的（H-1 重构已落地）：

- 旧的对象安全 `Action` trait 已删除，`action/action_trait.rs` 只剩 `Permission`。
- 三层 trait 在 `action/typed.rs`：用户手写 `TypedHandler`（声明关联类型 `Input`/`Output`）→ 派生层 `TypedAction` → 类型擦除层 `DynAction`（注册表存 `Arc<dyn DynAction>`）。
- 字段名走 `#[derive(TableEntity)]` 生成的封闭 `<Name>Field` / `<Name>Where` 枚举，杜绝任意字符串列名拼接；`#[derive(Action)]` 生成 `TypedAction` impl 与 `ActionMeta`。两个派生宏都在 `yang-base-derive`。
- 六个内置 Action（add/put/del/get/select/table，`action/builtin/`）已泛型化为 `XxxAction<T: TableEntity>`；`ModuleRouter::table_typed::<T>()` 一行注册全套 CRUD。
- 启用 `token` feature 时另有认证内置 Action（`action/auth.rs`）：`LoginAction<V>`（凭证校验委托业务实现的 `CredentialVerifier`）、`RefreshAction`、`LogoutAction`。

Action 通过 `ActionContext`（user/tools/table 上下文）访问 `database::GlobalDatabase` / `GlobalRedis`，再用 `table::TableQuery` 做带权限校验的查询。注意 `action/AGENTS.md` 与 `docs/`、`docs/yang-base.md` 部分内容写于 H-1 重构之前，仍按旧 `Action` trait + `serde_json::Value` 描述——以 `action/typed.rs`、`action/mod.rs`、`builtin/` 的实际代码为准。

### 全局单例与 feature gate
`GlobalDatabase` / `GlobalRedis` 用 `OnceLock` 实现，重复初始化必须返回 `BaseError`，不要 panic。统一启动入口是 `database::DatabaseBundle::init(mysql_url, mysql_config, redis_url, redis_config)`（mysql feature 下），按固定顺序先 MySQL 再 Redis 初始化两个单例，任一失败即返回，避免"半初始化"——新代码组装应用时优先用它而非分别调用两个 `init()`。crate 通过 feature 切换功能：`token`（JWT）、`http`（reqwest 包装）、`mysql`、`validator`、`plugin-schema`，默认全开。修改这些模块时确认 feature gate 不被破坏。

### yang-db 的 SQL/Redis hotspot
- `crates/yang-db/src/mysql/query_builder.rs` 是 4.8k 行的巨型文件，几乎所有 SQL 行为都从这里出。**不要在做功能时顺手拆分它**，耦合很高，拆分必须配套测试。
- `update()` / `delete()` 没有 WHERE 必返回 `MissingWhereClause`；处理用户输入时用 `where_and` / `where_or` / `having_cond` 等 checked API，不要用 `_unchecked` 系列除非 operator 已经验证过（`having_cond_unchecked` 会 panic）。
- `insert_batch` 默认 500 行批量；想自定义用 `insert_batch_with_size`。
- `RedisConfig` 的连接池/超时参数已经被 `connect_with_config` 应用，遇到声称这些字段未生效的旧文档要修正。

### yang-pcg 生成管线（确定性是契约）
```
GenerationRequest
  -> validate_request + config.normalize
  -> topology::generate_topology     RNG.derive("topology")
  -> layout::solve_layout            RNG.derive("layout")
  -> terrain::generate_terrains      RNG.derive("terrain")
  -> spawn::generate_spawns          RNG.derive("spawn")
  -> ue::streaming::build_chunks
  -> validate_result
  -> debug 模式下：run_full_validation
```

`RuntimeChunked` 走 `chunked::generate_chunk`；`HybridPrecompute` 用 `generate_topology_only` + `fill_chunk_details`。RNG 派生标签（`topology` / `layout` / `terrain` / `spawn` 以及每个房间的 item/enemy 流名）是确定性契约的一部分，**改名等于破坏 seed 复现性和黄金测试**。注意确定性是**逐生成模式**成立的：三种模式 RNG 派生路径不同（`OfflineFullFloor` 用 `terrain`，分块/混合路径用 `terrain:chunk:{chunk}:{room}`），**同一 seed 跨模式会产出不同地图**——这是按需出块的必要设计。`seed: None` 会**从 config 派生确定性种子**（`ConfigDigest::seed_from_config`），相同 config 仍复现同图；想要不同结果就显式给 seed 或改 config。UE 相关概念只能放在 `src/ue/` 下，core 模块（generator/topology/layout/terrain/spawn）不允许混入 UE 类型。`set_debug(true)` 不得改变 gameplay 输出，debug 数据走 side channel。

### 历史 Gap 已修复（2026-06 起）
`yang-pcg/src/tests_task27/property_tests.rs` 曾有三个 `#[ignore]` 的 property test（布局重叠、地形连通性、spawn 间距）。**这三项已被构造性算法修复并解除 ignore**（提交 `0ff2979`/`3650a4d`/`f2aef14`），现 6 个 property test 全部启用、`cargo test --lib -p yang-pcg` = 305 passed / 0 ignored / clippy 干净。这些不变量现由生产路径的全量硬校验（`generator.rs` 的 `backend.validate(... FullFloor)`，失败返回 `Err`）+ 已启用的 proptest 共同守护，**仍然不要删/弱化它们**，但它们守护的是真不变量、不再是"已知 gap"。`docs/BACKLOG.md` 给出各项当前真相。

## 全局约定

- 注释和公开文档大多是中文，新代码保持中文风格；`yang-base` 启用 `#![warn(missing_docs)]`。
- `yang-db` / `yang-pcg` 的 clippy 配置 allow 了 `unwrap_used` / `expect_used`，**这不是"可以加生产 panic"的许可**。已有 hotspot 包括 `query_builder.rs`、`plugin/mod.rs`、`validation.rs`、`grammar/selector.rs`，不要继续往里塞新的 unwrap/expect。
- 单元测试 colocated 在 `src/.../__tests__/`；集成测试在 crate 根 `tests/`；`yang-pcg` 还有 `tests_task26/` `tests_task27/` `chunked_tests.rs`。
- 需求追踪注释统一是 `验证需求: X.Y`，沿用此格式。
- `proptest-regressions/` 目录（`yang-db`、`yang-pcg`）是有意保留的回归语料，不要删。
- `.gitignore` 含 `*/tests/` 这一不寻常项，推断"哪些集成测试被跟踪"时要小心。
- 不要在 Docker/test 示例之外硬编码凭据；测试用 `MYSQL_TEST_PASSWORD` 等环境变量或 ignored 配置。
- Token 系统现已提供基于 Redis 的吊销/黑名单机制（`TokenManager::revoke_token` / `is_revoked` / `verify_token_checked`，见 `token/revocation.rs`）。`verify_token` 本身**不查**黑名单（保持向后兼容）；需要支持登出/撤销的鉴权路径必须用 `verify_token_checked`。

## 仓库结构里的"陈旧"信号

- 根目录有若干 `*_SUMMARY.md` / `DOCS_*` / `API_COMPATIBILITY_CHECK_SUMMARY.md` 等是**历史工作日志**，不是当前规范源，不要把它们当 spec。
- `docs/BACKLOG.md` 是带状态标记（✅/🟨/⏳）的问题追踪表，记录了 C/H/M/L 各项的修复进展。遇到"某文档说 X 坏了/缺失"时先查这里——很多旧描述（`serde_json::Value` Action、RedisConfig 静默失效、edition 2024、缺撤销机制等）都已修复，BACKLOG 给出当前真相。唯一仍 ⏳ 未处理的是 M-1（测试里 `unwrap()` 过多）。
- `.kiro/specs/` 目录可能不存在（已移除或迁移），不要依赖其内容。
- `docs/yang-db.md` / `docs/yang-base.md` 是较广的生成参考，速查可看；细节以代码为准。

## CodeGraph 索引

仓库已通过 `codegraph init` 建立 `.codegraph/`。结构性问题（"X 在哪/谁调用 X/X 调了什么/改 X 会炸什么/X 怎么走到 Y"）一律优先用 `codegraph_*` MCP 工具；详细规则见 `.cursor/rules/codegraph.mdc`，本文件不重复。仅在做"字面文本"搜索（字符串内容、注释、日志文案）或文件已被打开时才用 grep/read。
