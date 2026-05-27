# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 沟通语言

- 所有面向用户展示的对话内容、思考过程、说明文字、commit message、PR 描述一律使用中文。
- 代码中的注释、文档字符串、用户可见的错误信息保持中文风格（与现有代码一致）。
- 工具调用的参数（例如 `bash` 命令、`grep` 模式）按需使用英文，不必强行翻译。

## 仓库总览

`lib_yang` 是一个 Rust workspace（`resolver = "2"`，edition `2021`），包含三个互相依赖的 crate：

- `crates/yang-db/` — MySQL 查询构建器 + Redis 客户端，下游 crate 的数据访问基础。
- `crates/yang-base/` — 在 `yang-db` 之上的后端服务原语：插件、全局 DB/Redis、表配置/查询、Action 调度、路由，以及可选的 HTTP 客户端和 JWT Token。
- `crates/yang-pcg/` — UE5 / Roguelike 程序化地图生成库（确定性 PCG 管线）。三者解耦：PCG 不依赖 db/base。

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
请求经由 `router::ModuleRouter` 分发到对应模块的 `Action`（`action::Action` trait），Action 拿到 `ActionContext`（包含 user/tools/table 上下文），通过 `database::GlobalDatabase` / `GlobalRedis` 访问数据，再用 `table::TableQuery` 做带权限校验的查询。该路径上几乎所有的 builtin action 仍然在用 `serde_json::Value` 传值，不要在没有明确类型安全决策的情况下扩散这种用法。

### 全局单例与 feature gate
`GlobalDatabase` / `GlobalRedis` 用 `OnceLock` 实现，重复初始化必须返回 `BaseError`，不要 panic。crate 通过 feature 切换功能：`token`（JWT）、`http`（reqwest 包装）、`mysql`、`validator`、`plugin-schema`，默认全开。修改这些模块时确认 feature gate 不被破坏。

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

`RuntimeChunked` 走 `chunked::generate_chunk`；`HybridPrecompute` 用 `generate_topology_only` + `fill_chunk_details`。RNG 派生标签（`topology` / `layout` / `terrain` / `spawn` 以及每个房间的 item/enemy 流名）是确定性契约的一部分，**改名等于破坏 seed 复现性和黄金测试**。UE 相关概念只能放在 `src/ue/` 下，core 模块（generator/topology/layout/terrain/spawn）不允许混入 UE 类型。`set_debug(true)` 不得改变 gameplay 输出，debug 数据走 side channel。

### 已知 Gap（不要"修"成被动失败）
`yang-pcg/src/tests_task27/property_tests.rs` 中有三个 `#[ignore]` 的 property test，分别记录布局重叠、地形连通性、spawn 间距的真实未满足不变量。**不要为了让测试通过去删/弱化它们**，那些 ignore 是文档。

## 全局约定

- 注释和公开文档大多是中文，新代码保持中文风格；`yang-base` 启用 `#![warn(missing_docs)]`。
- `yang-db` / `yang-pcg` 的 clippy 配置 allow 了 `unwrap_used` / `expect_used`，**这不是"可以加生产 panic"的许可**。已有 hotspot 包括 `query_builder.rs`、`plugin/mod.rs`、`validation.rs`、`grammar/selector.rs`，不要继续往里塞新的 unwrap/expect。
- 单元测试 colocated 在 `src/.../__tests__/`；集成测试在 crate 根 `tests/`；`yang-pcg` 还有 `tests_task26/` `tests_task27/` `chunked_tests.rs`。
- 需求追踪注释统一是 `验证需求: X.Y`，沿用此格式。
- `proptest-regressions/` 目录（`yang-db`、`yang-pcg`）是有意保留的回归语料，不要删。
- `.gitignore` 含 `*/tests/` 这一不寻常项，推断"哪些集成测试被跟踪"时要小心。
- 不要在 Docker/test 示例之外硬编码凭据；测试用 `MYSQL_TEST_PASSWORD` 等环境变量或 ignored 配置。
- Token 系统**没有**吊销/黑名单机制，不要在文档/注释里暗示登出会让现有 JWT 失效（除非确实在做这个特性）。

## 仓库结构里的"陈旧"信号

- 根目录有若干 `*_SUMMARY.md` / `DOCS_*` / `API_COMPATIBILITY_CHECK_SUMMARY.md` 等是**历史工作日志**，不是当前规范源，不要把它们当 spec。
- `.kiro/specs/` 下的 requirements/design/tasks 是**真**的产品/设计源，PCG 部分的真理在 `.kiro/specs/ue5-roguelike-map-generator/`。
- `crates/yang-pcg/INSTALL.md.md` 是双扩展名遗留产物。
- `docs/yang-db.md` / `docs/yang-base.md` 是较广的生成参考，速查可看；细节以代码为准。

## CodeGraph 索引

仓库已通过 `codegraph init` 建立 `.codegraph/`。结构性问题（"X 在哪/谁调用 X/X 调了什么/改 X 会炸什么/X 怎么走到 Y"）一律优先用 `codegraph_*` MCP 工具；详细规则见 `.cursor/rules/codegraph.mdc`，本文件不重复。仅在做"字面文本"搜索（字符串内容、注释、日志文案）或文件已被打开时才用 grep/read。
