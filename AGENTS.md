# lib_yang — Project Knowledge Base

**Generated:** 2026-05-02
**Commit:** 10e3a1a
**Branch:** master

## OVERVIEW
YANG 后端框架 — Rust workspace with 3 crates providing database abstraction (MySQL/Redis), plugin-based backend services, and map generation utilities.

## STRUCTURE
```
lib_yang/
├── Cargo.toml              # Workspace root (resolver=2)
├── crates/
│   ├── yang-base/          # Backend services (plugin, HTTP, token, action, table, router)
│   ├── yang-db/            # Database abstraction (MySQL query builder + Redis client)
│   └── yang-pcg/           # Placeholder — map generation (unimplemented)
└── target/                 # Build artifacts (gitignored)
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| MySQL queries | `crates/yang-db/src/mysql/` | QueryBuilder, Database, Condition, Transaction |
| Redis operations | `crates/yang-db/src/redis/` | RedisClient, pipeline, transactions, pub/sub |
| Database connection init | `crates/yang-base/src/database/` | GlobalDatabase, GlobalRedis |
| HTTP client | `crates/yang-base/src/http/` | HttpClient with global singleton |
| JWT tokens | `crates/yang-base/src/token/` | TokenManager, Claims |
| Plugin system | `crates/yang-base/src/plugin/` | Plugin trait, PluginManager |
| Action system | `crates/yang-base/src/action/` | Action trait, builtin actions (select, get, etc.) |
| Table config | `crates/yang-base/src/table/` | Table structs, field configuration |
| Router | `crates/yang-base/src/router/` | Router configuration |
| Error types | `crates/yang-base/src/error/` + `crates/yang-db/src/error.rs` | BaseError (yang-base), DbError (yang-db) |
| Root-level docs | `DOCS_CLEANUP_PLAN.md`, `*.md` | Summaries of dependency updates, testcontainers fixes, optimization plans |

## CONVENTIONS
- **Edition**: yang-base = 2021; yang-db/yang-pcg use 2024 (non-standard, should be 2021)
- **License**: MIT OR Apache-2.0 (all crates)
- **Linting**: `cargo clippy --all-targets --all-features -- -D warnings` (must pass clean)
- **Formatting**: `cargo fmt` (Rust defaults, no custom rustfmt.toml)
- **Comments**: Chinese throughout
- **Tests**: `__tests__/` colocated for unit tests, `tests/` for integration tests; `#[ignore]` for Docker-dependent tests
- **Requirement tracing**: Test comments reference requirement numbers (e.g., `验证需求: 4.1`)

## ANTI-PATTERNS (THIS PROJECT)
- **CRITICAL**: `RedisConfig` connection pool params never applied — fix before touching Redis
- **CRITICAL**: `insert_batch` lacks auto-batching — large datasets exceed `max_allowed_packet`
- **TODO**: Plugin JSON Schema validation not implemented (`src/plugin/mod.rs:451`)
- **TODO**: `builtin/select.rs` and `builtin/get.rs` use `serde_json::Value` instead of concrete types
- **NEVER**: Use `unwrap()` in production code — tests have 20+ unwrap() calls to fix
- **NEVER**: Hardcode credentials — `.mcp.json` contains MySQL password "111111"
- `having()` method uses raw strings without SQL injection protection
- Redis Pipeline/Transaction/Lua should use `redis::pipe()`/`redis::Script` APIs, not custom impl

## COMMANDS
```bash
cargo clippy --all-targets --all-features -- -D warnings   # Lint (must pass)
cargo test --lib                                              # Unit tests (no Docker)
cargo test --lib -p yang-base                                 # yang-base unit tests
cargo test --lib -p yang-db                                   # yang-db unit tests
cargo test --test <name> -- --ignored --test-threads=1       # Integration tests (requires Docker)
cargo fmt                                                     # Format code
cargo check                                                   # Compile check
```

## NOTES
- **Edition 2024 bug**: yang-db and yang-pcg specify `edition = "2024"` — needs fixing to "2021"
- yang-pcg (`crates/yang-pcg/`) is a placeholder with 7 lines of code — safe to ignore
- No CI/CD configured — tests must be run locally
- Root-level `*.md` files are work-in-progress summaries, not formal documentation
- Integration tests require Docker + MySQL 8.0 (via testcontainers)
- Workspace has no shared dependency table — dependencies duplicated across crates
- Requires Docker for full test suite; unit tests run without Docker
