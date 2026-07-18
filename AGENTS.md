# lib_yang — Project Knowledge Base

**Generated:** 2026-07-16
**Commit:** b65a50b
**Branch:** master

## OVERVIEW
YANG Rust workspace：包含 `yang-db`、`yang-base`（+ `yang-base-derive` 宏）、`yang-migrate`、`yang-pcg` 五个基础库 crate，以及用于联合调试基础库的 `yang-system` 应用。

## STRUCTURE
```text
lib_yang/
├── Cargo.toml              # workspace root, resolver=2, shared deps/lints
├── crates/
│   ├── yang-db/            # MySQL query builder + Redis client
│   ├── yang-base/          # definition 内核、actions、tables、Tools、token、HTTP、transport-axum
│   ├── yang-base-derive/   # #[derive(Action)] 与 params! 宏
│   ├── yang-migrate/       # br-to-yang 迁移 codemod
│   └── yang-pcg/           # deterministic PCG map generator + UE5 adapter
├── project/
│   └── yang-system/        # independent nested Git/Cargo application for local integration
└── docs/                   # workspace API references and backlog
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| MySQL queries | `crates/yang-db/src/mysql/` | `query_builder.rs` is the 5.5k-line hotspot (~5,506 lines); `condition.rs` owns WHERE/HAVING expressions |
| Redis operations | `crates/yang-db/src/redis/` | `client.rs` is the main API; pipeline/transaction wrap `redis::pipe()` patterns |
| 资源所有权 | `crates/yang-base/src/tools.rs` | `ToolsBuilder` → `Tools`：mysql/cache/token/http + 类型化 extension/config；GlobalDatabase/GlobalRedis 已删除 |
| 定义内核 | `crates/yang-base/src/definition/` | `AppBuilder`/`BuiltApp`/Catalog/Registry，构建期校验 + slot 预解析 |
| HTTP 传输 | `crates/yang-base/src/transport/` | `transport-axum` feature：Axum 0.8 适配器、CORS/超时/压缩、文件/重定向响应 |
| Plugin system | `crates/yang-base/src/plugin/mod.rs` | 旧代插件生命周期；新链路用 definition Addon 组织业务 |
| Action system | `crates/yang-base/src/action/` | child AGENTS.md covers trait, context, builtin CRUD |
| Table system | `crates/yang-base/src/table/` | child AGENTS.md covers FieldType/FieldConfig/TableQuery/DynamicRow |
| Tokens | `crates/yang-base/src/token/` | JWT `TokenManager`, feature-gated |
| HTTP client | `crates/yang-base/src/http/` | reqwest wrapper, feature-gated；经 `Tools::http()`/`ctx.http()?` 获取 |
| 基础系统联调 | `project/yang-system/` | 独立嵌套仓库；相对路径直接依赖 ../../crates 联调本地库 |
| PCG generation | `crates/yang-pcg/src/generator.rs` | pipeline: topology -> layout -> terrain -> spawn -> chunks |
| PCG terrain | `crates/yang-pcg/src/terrain/` | child AGENTS.md covers strategies, fallback, known connectivity gaps |
| Specs/backlog | `docs/BACKLOG.md`, `docs/superpowers/` | requirements/design/tasks; some root summary docs are historical artifacts |
| API docs | `docs/yang-db.md`, `docs/yang-base.md` | broad generated API references |

## CODE MAP
| Symbol | Type | Location | Role |
|--------|------|----------|------|
| `QueryBuilder` | struct | `crates/yang-db/src/mysql/query_builder.rs` | MySQL CRUD/select/aggregate/batch SQL builder |
| `Condition` / `SqlValue` | enum | `crates/yang-db/src/mysql/condition.rs` | WHERE/HAVING expression tree and bind values |
| `RedisClient` | struct | `crates/yang-db/src/redis/client.rs` | Redis string/hash/list/set/zset/pubsub/script API |
| `Database` | struct | `crates/yang-db/src/mysql/database.rs` | sqlx MySQL pool wrapper and raw query entry |
| `AppBuilder` / `BuiltApp` | structs | `crates/yang-base/src/definition/builder.rs` | 构建期组装/校验 → 冻结 Catalog/Registry/Tools |
| `ToolsBuilder` / `Tools` | structs | `crates/yang-base/src/tools.rs` | 应用资源显式所有权与生命周期 |
| `Action` / `TypedHandler` / `DynAction` | traits | `crates/yang-base/src/action/typed.rs` | 业务 Action::index → TypedHandler → TypedAction（derive）→ DynAction 擦除派发 |
| `ApiResponse` / `ResponseBody` | structs | `crates/yang-base/src/action/response.rs` | 统一响应 + 文件/预览/重定向附件 |
| `PluginManagerBuilder` / `PluginRegistry` | structs | `crates/yang-base/src/plugin/mod.rs` | build-time registration, dependency checks, runtime registry |
| `ActionContext` | struct | `crates/yang-base/src/action/context.rs` | request/user/tools/table context passed to actions |
| `UiCatalog` / `UiCatalogAction` | structs | `crates/yang-base/src/definition/ui.rs`、`crates/yang-base/src/action/ui_catalog.rs` | 请求级 UI 目录投影（ActionDemo/TableView/Form/Tree schema，schema_version + revision） |
| `WidgetHint` / `ActionPresentation` / `AvailabilityHint` | enums/structs | `crates/yang-base/src/definition/ui.rs` | 字段控件提示（可降级）、Action 位置×交互展示语义、非安全性可用提示 |
| `RelationOptionsRequest` / `RelationOptionsResponse` | structs | `crates/yang-base/src/table/relation_options.rs` | 关系选择器统一 options 契约（search/selected/filter/page/limit → {value,label}） |
| `TenantResolver` / `TenantResolverMiddleware` | trait/struct | `crates/yang-base/src/action/tenant.rs` | 可信租户解析（header 仅为声明，resolver 服务端校验，fail-closed） |
| `StepUpManager` / `StepUpMiddleware` | structs | `crates/yang-base/src/action/step_up.rs` | 敏感操作请求级重认证（challenge/proof 绑定用户+Action+资源+短过期） |
| `MultipartSpec` / `UploadedFile` | structs | `crates/yang-base/src/definition/media.rs`、`crates/yang-base/src/action/upload.rs` | 受限 multipart 契约与上传文件句柄（临时根受信、copy_to 越界拒绝） |
| `router` / `serve` | functions | `crates/yang-base/src/transport/axum.rs` | Axum 0.8 传输适配器入口 |
| `TableQuery` | struct | `crates/yang-base/src/table/table_query.rs` | table-aware query builder with permissions |
| `FieldType` | enum | `crates/yang-base/src/table/field_type.rs` | JSON/MySQL field validation and type mapping |
| `MapGenerator` | struct | `crates/yang-pcg/src/generator.rs` | PCG orchestration entry point |
| `GenerationConfig` | struct | `crates/yang-pcg/src/config.rs` | PCG config normalization and defaults |
| `PcgError` | enum | `crates/yang-pcg/src/error.rs` | PCG structured error codes/context |
| `run_full_validation` | function | `crates/yang-pcg/src/validation.rs` | reachability/overlap/connectivity/spawn invariant report |

## CONVENTIONS
- Edition is `2021` for all crates.
- `project/yang-system` 从根 workspace 排除；应进入该目录独立运行 Cargo 命令。
- Comments and public docs are mostly Chinese; preserve that style in Rust code and tests.
- `yang-base` inherits workspace lints and has `#![warn(missing_docs)]`.
- `yang-db` and `yang-pcg` override clippy to allow `unwrap_used`/`expect_used`; do not treat that as permission for new production panics.
- Unit tests are colocated in `__tests__/`; integration tests live in crate `tests/`; `yang-pcg` also has `tests_task26/`, `tests_task27/`, and `chunked_tests.rs`.
- Requirement tracing comments use `验证需求: X.Y`.
- Docker tests are `#[ignore]` and should run single-threaded.
- `proptest-regressions/` is intentional for `yang-db` and `yang-pcg`.

## ANTI-PATTERNS (THIS PROJECT)
- Do not add production `unwrap()`/`expect()` even where crate lints allow them; existing hotspots include `query_builder.rs`, `plugin/mod.rs`, `validation.rs`, and `grammar/selector.rs`.
- Do not use `_unchecked` query helpers unless caller already validated operators; prefer `having_cond`, `where_and`, `where_or` Result-returning APIs.
- 资源一律经 `ToolsBuilder` 注册、`Tools` 获取；禁止在 yang-base 新增进程级全局单例（`static OnceLock`/`lazy_static`）。
- Do not weaken or delete ignored property tests in `yang-pcg`; they document known algorithm gaps.
- Do not hardcode credentials outside Docker/test examples; use `MYSQL_TEST_PASSWORD` or local ignored config.
- Do not document `RedisConfig` pool params or `insert_batch` auto-batching as currently broken; current code applies pool config and `insert_batch` batches at 500 rows by default.
- Builtin actions still use `serde_json::Value` in several paths; do not expand that pattern without a deliberate type-safety decision.
- Root summary markdown files are historical work logs unless referenced by AGENTS.md or docs.

## COMMANDS
```bash
# 提交前快速门禁；推送前使用 full；数据库行为变更再运行 integration
python scripts/run_ci.py quick
python scripts/run_ci.py full
python scripts/run_ci.py integration
cargo fmt
cargo check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib
cargo test --lib -p yang-base
cargo test --lib -p yang-db
cargo test --lib -p yang-pcg
cargo test --test <name> -- --ignored --test-threads=1
cargo run --example <name> -p <crate>
python scripts/verify_feature_isolation.py --self-test
python scripts/verify_ci_contract.py .github/workflows/ci.yml
```

## NOTES
- CI 位于 `.github/workflows/ci.yml`（fmt/test/clippy/doc-test 门禁、MSRV 1.80、feature 矩阵、docker 服务）；`rust-toolchain.toml` 固定默认开发/CI 工具链，无 `rustfmt.toml`/`clippy.toml`/Makefile/Dockerfile。
- `CONTRIBUTING.md` 是提交规范；`scripts/run_ci.py` 是本地 CI 统一入口。`Cargo.lock` 或 feature 变更必须在推送前运行 `python scripts/run_ci.py full`。
- LSP rust-analyzer was unavailable in this environment; CodeGraph is indexed and should be preferred for structural lookup.
- `.gitignore` includes `*/tests/`, which is unusual for Rust; be careful when reasoning about tracked integration tests.
- Full Docker tests require MySQL 8.0 and Redis via testcontainers or standard containers.
