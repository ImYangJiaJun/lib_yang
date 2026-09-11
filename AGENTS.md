# AGENTS.md — lib_yang

**刷新日期:** 2026-09-05（对照 commit b8e511d 现场核实；上一版生成于 2026-07-16 / b65a50b）

## OVERVIEW
YANG Rust workspace：五个基础库 crate（`yang-db`、`yang-base` + `yang-base-derive` proc-macro、`yang-runtime`、`yang-pcg`）+ 独立嵌套应用 `project/yang-system` 用于本地联合调试。edition 2021、resolver 2、共享 `[workspace.dependencies]`，根工具链固定 1.97.1（`rust-toolchain.toml`），MSRV 1.80（CI `+1.80.0` 单独 job 校验）。所有注释、文档、commit message 中文风格。

## STRUCTURE
```text
lib_yang/
├── Cargo.toml                # workspace root: members=["crates/*"], exclude project/yang-system
├── clippy.toml               # disallowed-types 豁免（RUSTSEC-2026-0009）——NOTES 勿再写"无 clippy.toml"
├── deny.toml                 # cargo-deny 策略（advisories/licenses/sources）
├── rust-toolchain.toml       # channel 1.97.1
├── benchmarks/               # runtime-shadow 基准配置（CI performance-shadow job）
├── crates/
│   ├── yang-db/              # MySQL/PG 查询构建 + Redis 客户端（v0.1.6）
│   ├── yang-base/            # definition 内核、actions、tables、auth、token、HTTP、transport-axum（v0.2.2）
│   ├── yang-base-derive/     # #[derive(Action)]/#[derive(TableEntity)] + params! 宏（v0.2.1）
│   ├── yang-runtime/         # 配置源、可观测性、进程生命周期（v0.1.0）
│   └── yang-pcg/             # deterministic PCG map generator + UE5 adapter（v0.1.1）
├── project/yang-system/      # 嵌套独立 Git/Cargo 应用（有自己的 .git/AGENTS.md/compose.yaml）；不在 workspace
├── scripts/
│   ├── run_ci.py             # quick/full/integration 统一入口（唯一 CI 门禁入口）
│   ├── verify_ci_contract.py / verify_feature_isolation.py
│   ├── verify_dependency_policy.py   # supply-chain job 契约
│   └── run_performance_shadow.py     # performance-shadow job
└── docs/                     # BACKLOG.md + superpowers/{specs,plans,baselines}；根目录散落的历史 md 多为工作日志
```

## COMMANDS
提交前 `run_ci.py quick`；推送前 `run_ci.py full`；动了 DB 行为再跑 `integration`（需容器服务）。run_ci 全命令带 `--locked`。

```bash
python scripts/run_ci.py quick        # fmt --check + 分 crate test + clippy -D warnings + doctest
python scripts/run_ci.py full         # quick + cargo-deny + MSRV 1.80 check + 17 组 feature 矩阵
python scripts/run_ci.py integration  # 需 MySQL 8.0 / PostgreSQL 16 / Redis 7（带 --ignored 集成测试）

# 与 CI 对齐的分 crate 手拼命令（三组跑法不同，别漏 auxiliary）
cargo test --lib -p yang-db --locked
cargo test --lib -p yang-base --locked
cargo test --all-targets -p yang-base-derive -p yang-pcg -p yang-runtime --locked

cargo fmt --all -- --check
cargo clippy -p yang-db -p yang-base -p yang-base-derive -p yang-pcg -p yang-runtime \
  --all-targets --all-features --locked -- -D warnings
cargo check --workspace --all-targets --all-features --locked   # MSRV/feature 等效全量检查
cargo test --doc -p yang-db -p yang-base -p yang-runtime --locked

# 集成测试（需容器；Redis pipeline/script 两个测试不带 --ignored，服务可用时直接 --test-threads=1）
cargo test --test <name> -- --ignored --test-threads=1

# 契约自检
python scripts/verify_feature_isolation.py --self-test
python scripts/verify_ci_contract.py .github/workflows/ci.yml

# 示例（以实际 example 名为准，crates/*/examples/ 有；建议 --locked）
cargo run --example <name> -p <crate> --locked
```

## CI（.github/workflows/ci.yml）
`supply-chain`（verify_dependency_policy + cargo-deny）→ `stable`（契约自检 → fmt → 分 crate test → clippy → doctest）→ `msrv`（1.80 check）→ `feature-matrix`（17 组合，check/test/doctest，`-Dwarnings`）→ `docker-mysql`（mysql:8.0+redis:7）→ `docker-postgres`（PG 16）→ `docker-redis`（redis:7）。`performance-shadow` 非阻塞采集（continue-on-error）。**改 CI 必须同步 `verify_ci_contract.py` 与 `run_ci.py`。**

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| MySQL 查询 | `crates/yang-db/src/mysql/query_builder/` | 按职责拆分子模块；`condition.rs` owns WHERE/HAVING 表达式 |
| PostgreSQL 查询 | `crates/yang-db/src/postgres/` | 共享 `crates/yang-db/src/dialect.rs`（SQL dialect 抽象，非复制） |
| Redis 操作 | `crates/yang-db/src/redis/` | `client.rs` 是主 API |
| 资源所有权 | `crates/yang-base/src/tools.rs` | `ToolsBuilder` → `Tools`；Global 单例已删除 |
| 定义内核 | `crates/yang-base/src/definition/` | `builder/` 子模块化（app/catalog/compile/handle/project/registry/validate） |
| Action 系统 | `crates/yang-base/src/action/` | child AGENTS.md；`auth/` 子模块化（login/refresh/password/middleware…） |
| auth/认证 | `crates/yang-base/src/action/auth/` | browser_session/email_verification/rate_limit/audit 等子模块 |
| HTTP 传输 | `crates/yang-base/src/transport/` | transport-axum feature：`axum.rs`（router 入口）+ client_ip.rs |
| HTTP 客户端 | `crates/yang-base/src/http/` | reqwest wrapper, feature-gated |
| Table 系统 | `crates/yang-base/src/table/` | child AGENTS.md；`table_query/` 子模块化（build/read/write/filters/plan/validation…） |
| definition ui | `crates/yang-base/src/definition/ui/` | 已拆 action/catalog/demo/hints/module/table |
| schema 同步 | `crates/yang-base/src/database/schema_sync/` | inspect/model/plan/preflight/render/sync 子模块 |
| Tokens | `crates/yang-base/src/token/` | JWT TokenManager, feature-gated |
| 基础系统联调 | `project/yang-system/` | 独立嵌套仓库（相对路径依赖 ../../crates 联调本地库）；进入该目录独立跑 Cargo |
| PCG generation | `crates/yang-pcg/src/generator.rs` | pipeline: topology -> layout -> terrain -> spawn -> chunks |
| PCG terrain | `crates/yang-pcg/src/terrain/` | child AGENTS.md；`pcg 全部测试放 auxiliary 组跑 --all-targets` |
| Specs/backlog | `docs/BACKLOG.md`, `docs/superpowers/` | requirements/design/tasks；根目录散落 summary md 多为历史日志 |

## CODE MAP
| Symbol | Type | Location | Role |
|--------|------|----------|------|
| `QueryBuilder` | struct | `crates/yang-db/src/mysql/query_builder/mod.rs` | MySQL CRUD/select/aggregate/batch SQL builder |
| `Condition` / `SqlValue` | enums | `crates/yang-db/src/mysql/condition.rs` | WHERE/HAVING expression tree and bind values |
| `Database` | struct | `crates/yang-db/src/mysql/database.rs`（同类在 `postgres/database.rs`） | sqlx pool wrapper + raw query 入口 |
| `RedisClient` | struct | `crates/yang-db/src/redis/client.rs` | Redis 各数据结构 API |
| `AppBuilder` / `BuiltApp` | structs | `crates/yang-base/src/definition/builder/app.rs` | 构建期组装/校验 → 冻结 Catalog/Registry/Tools |
| `ToolsBuilder` / `Tools` | structs | `crates/yang-base/src/tools.rs` | 应用资源显式所有权与生命周期 |
| `Action` / `TypedHandler` / `DynAction` | traits | `crates/yang-base/src/action/typed.rs` | Action::index → TypedHandler → TypedAction(derive) → DynAction |
| `ApiResponse` / `ResponseBody` | structs | `crates/yang-base/src/action/response.rs` | 统一响应 + 文件/预览/重定向附件 |
| `ActionContext` | struct | `crates/yang-base/src/action/context.rs` | request/user/tools/table context |
| `UiCatalog` | struct | `crates/yang-base/src/definition/ui/catalog.rs` | 请求级 UI 目录投影（原表列在 action/ui_catalog.rs，那是 UiCatalogAction） |
| `TenantResolver` / `TenantResolverMiddleware` | trait/struct | `crates/yang-base/src/action/tenant.rs` | 可信租户解析（header 仅声明，resolver 服务端校验） |
| `StepUpManager` / `StepUpMiddleware` | structs | `crates/yang-base/src/action/step_up.rs` | 敏感操作重认证（challenge/proof） |
| `MultipartSpec` / `UploadedFile` | structs | `crates/yang-base/src/definition/media.rs`、`crates/yang-base/src/action/upload.rs` | 受限 multipart 契约与上传句柄 |
| `router` / `serve` | functions | `crates/yang-base/src/transport/axum.rs` | Axum 0.8 传输适配器入口 |
| `TableQuery` | struct | `crates/yang-base/src/table/table_query/mod.rs` | table-aware query builder with permissions |
| `FieldType` | enum | `crates/yang-base/src/table/field_type.rs` | JSON/MySQL field validation and type mapping |
| `MapGenerator` | struct | `crates/yang-pcg/src/generator.rs` | PCG orchestration entry point |
| `PcgError` | enum | `crates/yang-pcg/src/error.rs` | PCG structured error codes/context |
| `run_full_validation` | fn(pub(crate)) | `crates/yang-pcg/src/validation.rs` | reachability/overlap/connectivity/spawn invariant report（仅 crate 内使用） |
| `PluginManagerBuilder` / `PluginRegistry` | structs | `crates/yang-base/src/plugin/builder.rs`、`registry.rs` | build-time registration + runtime registry |

## CONVENTIONS
- edition `2021`（各 crate `edition.workspace = true`）；`project/yang-system` 从根 workspace 排除，Cargo 命令须进该目录跑。
- Comments/public docs/commit 中文风格；`yang-base` `#![warn(missing_docs)]`。
- Unit tests colocated 在 `__tests__/`；integration tests 在 crate `tests/`；`yang-pcg` 另有 `tests_task26/`、`tests_task27/`、`chunked_tests.rs`。
- 需求追踪注释用 `验证需求: X.Y` 前缀。
- Docker tests `#[ignore]`、单线程跑；`proptest-regressions/` 是 `yang-db`/`yang-pcg` 的意图性保留。

## ANTI-PATTERNS（本项目）
- 不新增生产 `unwrap()`/`expect()`，即使 crate lint 允许（现热点：`mysql/query_builder/`、`plugin/`、`validation.rs`、`grammar/selector.rs`）。
- 不用 `_unchecked` 查询助手，除非调用方已验证运算符；优先 `having_cond`/`where_and`/`where_or` Result 返回 API。
- 资源一律经 `ToolsBuilder` 注册、`Tools` 获取；禁止在 yang-base 新增进程级全局单例（`static OnceLock`/`lazy_static`）。
- 不删/弱化 `yang-pcg` 忽略的属性测试（文档化算法缺口）。
- 不硬编码凭据；用 `MYSQL_TEST_PASSWORD` 或本地忽略配置。
- 不把 `RedisConfig` pool 参数 / `insert_batch` 自动批处理写成坏的（当前已生效，`insert_batch` 默认 500 行批处理）。
- Builtin actions 部分路径仍用 `serde_json::Value`；未经类型安全决策不扩大该模式。
- 不新增进程级 `auth` 全局（auth 子模块化了；看 `action/AGENTS.md` 与 `action/auth/mod.rs` 约定）。

## NOTES
- `CONTRIBUTING.md` 是提交规范；改 `Cargo.lock`/feature 必须在推送前跑 `python scripts/run_ci.py full`。
- 根 `rust-toolchain.toml` 固定 1.97.1；CI 的 `msrv` job 用 `+1.80.0` check——两件事不是一回事。
- CI 跑在 `.github/workflows/ci.yml`；`deny.toml` 驱动 supply-chain job；`benchmarks/runtime-shadow.toml` 驱动 performance-shadow。
- `.gitignore` 含 `*/tests/`（对 Rust 反常），reasoning about tracked integration tests 时要小心。
- 每个 crate 根与部分子模块（`yang-base/src/action|table`、`yang-pcg/src/terrain`）各有一份 AGENTS.md；**动手前先读目标模块的 AGENTS.md**。
