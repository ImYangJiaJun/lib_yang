# lib_yang — Project Knowledge Base

**Generated:** 2026-07-16
**Commit:** b65a50b
**Branch:** master

## OVERVIEW
YANG Rust workspace：包含 `yang-db`、`yang-base`、`yang-base-derive`、`yang-pcg` 四个基础库 crate，以及用于联合调试基础库的 `yang-system` 应用。

## STRUCTURE
```text
lib_yang/
├── Cargo.toml              # workspace root, resolver=2, shared deps/lints
├── crates/
│   ├── yang-db/            # MySQL query builder + Redis client
│   ├── yang-base/          # plugins, actions, tables, router, token, HTTP, global DB
│   └── yang-pcg/           # deterministic PCG map generator + UE5 adapter
├── project/
│   └── yang-system/        # independent nested Git/Cargo application for local integration
├── .kiro/                  # requirements/design/tasks specs + steering hooks
└── docs/                   # workspace API references and backlog
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| MySQL queries | `crates/yang-db/src/mysql/` | `query_builder.rs` is the 5.5k-line hotspot (~5,506 lines); `condition.rs` owns WHERE/HAVING expressions |
| Redis operations | `crates/yang-db/src/redis/` | `client.rs` is the main API; pipeline/transaction wrap `redis::pipe()` patterns |
| Backend globals | `crates/yang-base/src/database/` | `GlobalDatabase`, `GlobalRedis`, `DatabaseInitializer` |
| Plugin system | `crates/yang-base/src/plugin/mod.rs` | single-file plugin lifecycle/registry implementation |
| Action system | `crates/yang-base/src/action/` | child AGENTS.md covers trait, context, builtin CRUD |
| Table system | `crates/yang-base/src/table/` | child AGENTS.md covers FieldType/FieldConfig/TableQuery/DynamicRow |
| Router | `crates/yang-base/src/router/` | `ModuleRouter`, `AppRouter` |
| Tokens | `crates/yang-base/src/token/` | JWT `TokenManager`, feature-gated |
| HTTP client | `crates/yang-base/src/http/` | reqwest wrapper, feature-gated |
| 基础系统联调 | `project/yang-system/` | 独立嵌套仓库；固定 Git revision，临时 Cargo patch 联调本地库 |
| PCG generation | `crates/yang-pcg/src/generator.rs` | pipeline: topology -> layout -> terrain -> spawn -> chunks |
| PCG terrain | `crates/yang-pcg/src/terrain/` | child AGENTS.md covers strategies, fallback, known connectivity gaps |
| Specs/backlog | `.kiro/specs/`, `docs/BACKLOG.md` | requirements/design/tasks; some root summary docs are historical artifacts |
| API docs | `docs/yang-db.md`, `docs/yang-base.md` | broad generated API references |

## CODE MAP
| Symbol | Type | Location | Role |
|--------|------|----------|------|
| `QueryBuilder` | struct | `crates/yang-db/src/mysql/query_builder.rs` | MySQL CRUD/select/aggregate/batch SQL builder |
| `Condition` / `SqlValue` | enum | `crates/yang-db/src/mysql/condition.rs` | WHERE/HAVING expression tree and bind values |
| `RedisClient` | struct | `crates/yang-db/src/redis/client.rs` | Redis string/hash/list/set/zset/pubsub/script API |
| `Database` | struct | `crates/yang-db/src/mysql/database.rs` | sqlx MySQL pool wrapper and raw query entry |
| `PluginManagerBuilder` / `PluginRegistry` | structs | `crates/yang-base/src/plugin/mod.rs` | build-time registration, dependency checks, runtime registry |
| `TypedHandler` / `DynAction` | trait | `crates/yang-base/src/action/typed.rs` | H-1 类型化 action 系统：TypedHandler（用户实现）→ TypedAction（派生层）→ DynAction（注册表存储）；旧 Action trait 已删除，action_trait.rs 仅剩 Permission |
| `ActionContext` | struct | `crates/yang-base/src/action/context.rs` | request/user/tools/table context passed to actions |
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
- Do not weaken or delete ignored property tests in `yang-pcg`; they document known algorithm gaps.
- Do not hardcode credentials outside Docker/test examples; use `MYSQL_TEST_PASSWORD` or local ignored config.
- Do not document `RedisConfig` pool params or `insert_batch` auto-batching as currently broken; current code applies pool config and `insert_batch` batches at 500 rows by default.
- Builtin actions still use `serde_json::Value` in several paths; do not expand that pattern without a deliberate type-safety decision.
- Root summary markdown files are historical work logs unless referenced by AGENTS.md or docs.

## COMMANDS
```bash
cargo fmt
cargo check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib
cargo test --lib -p yang-base
cargo test --lib -p yang-db
cargo test --lib -p yang-pcg
cargo test --test <name> -- --ignored --test-threads=1
cargo run --example <name> -p <crate>
```

## NOTES
- No CI, `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`, Makefile, justfile, Dockerfile, or docker-compose file exists.
- LSP rust-analyzer was unavailable in this environment; CodeGraph is indexed and should be preferred for structural lookup.
- `.gitignore` includes `*/tests/`, which is unusual for Rust; be careful when reasoning about tracked integration tests.
- `.kiro/` contains real requirements/design/tasks context, not throwaway docs.
- Full Docker tests require MySQL 8.0 and Redis via testcontainers or the `.kiro/steering` standard containers.
