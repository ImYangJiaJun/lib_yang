# yang-base — Backend Services Library

**Parent:** lib_yang workspace

## OVERVIEW
Application-facing backend primitives built on `yang-db`: plugin lifecycle, global MySQL/Redis access, table schemas/querying, action dispatch, routing, optional HTTP client, optional JWT tokens, and unified errors.

## STRUCTURE
```text
yang-base/
├── src/
│   ├── lib.rs           # 11 public modules; feature-gated http/token
│   ├── config.rs        # 全局配置结构
│   ├── lifecycle.rs     # 应用生命周期钩子
│   ├── observability.rs # 可观测性配置（慢查询阈值 OnceLock 单例，无 tracing/metrics 集成）
│   ├── database/        # GlobalDatabase, GlobalRedis, DatabaseInitializer
│   ├── plugin/          # Plugin trait + managers/registry in one file
│   ├── action/          # child AGENTS.md: TypedHandler/TypedAction/DynAction, builtin CRUD
│   ├── table/           # child AGENTS.md: Table/Field/TableDefinition/Record/TableQuery
│   ├── router/          # Api + ModuleRouter + AppRouter + ApiCatalog
│   ├── http/            # reqwest wrapper, `http` feature
│   ├── token/           # JWT TokenManager + revocation, `token` feature
│   └── error/           # BaseError with numeric codes
├── tests/               # integration tests, Docker/manual where ignored
├── docs/                # api/guides/reference/examples
└── examples/            # database + field config examples
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Database init | `src/database/global.rs`, `src/database/global_redis.rs` | OnceLock singletons wrapping yang-db |
| DB/Redis bootstrap | `src/database/initializer.rs` | initialization/migration ordering |
| Plugin management | `src/plugin/mod.rs` | `Plugin`, `PluginManager`, `PluginManagerBuilder`, `PluginRegistry` |
| Custom actions | `src/action/` | child AGENTS.md covers trait/context/builtin CRUD |
| Table definitions | `src/table/definition.rs` | public schema-first `Table` / `Field` builders and immutable `TableDefinition` |
| Router/API registration | `src/router/api.rs`, `src/router/module_router.rs` | `Api` atomically binds an Action and route; `.table(...).crud()` installs builtin CRUD |
| HTTP requests | `src/http/` | feature-gated request builder/client/response |
| Token generation | `src/token/manager.rs` | feature-gated JWT generation/validation/refresh |
| Error handling | `src/error/mod.rs` | `BaseError` variants and codes |
| Full docs | `docs/`, `../../docs/yang-base.md` | generated/reference material |

## CODE MAP
| Symbol | Location | Role |
|--------|----------|------|
| `Plugin` | `src/plugin/mod.rs` | plugin extension trait |
| `PluginManagerBuilder` | `src/plugin/mod.rs` | build-time plugin registration and dependency checks |
| `PluginRegistry` | `src/plugin/mod.rs` | runtime immutable plugin lookup |
| `GlobalDatabase` | `src/database/global.rs` | global MySQL query/table access |
| `GlobalRedis` | `src/database/global_redis.rs` | global Redis delegate API |
| `TypedHandler` | `src/action/typed.rs` | user-written handler trait (Input/Output) |
| `TypedAction` | `src/action/typed.rs` | derived trait layer (H-1 typed system) |
| `DynAction` | `src/action/typed.rs` | type-erased dispatch layer |
| `Permission` | `src/action/action_trait.rs` | action permission type |
| `ActionContext` | `src/action/context.rs` | request/user/tools/table context |
| `Table` / `Field` | `src/table/definition.rs` | application-facing schema builders |
| `TableDefinition` | `src/table/definition.rs` | immutable runtime schema, permissions and JSON Schema source |
| `Record` | `src/table/dynamic_row.rs` | transparent dynamic row object used by builtin CRUD |
| `TableQuery` | `src/table/table_query.rs` | table-aware SQL query builder/executor |
| `FieldType` | `src/table/field_type.rs` | JSON/MySQL field type validation/mapping |
| `Api` | `src/router/api.rs` | Action + method/path/operation metadata single registration value |
| `ModuleRouter` | `src/router/module_router.rs` | action registration and dispatch |
| `TokenManager` | `src/token/manager.rs` | JWT encode/decode/refresh |
| `HttpClient` | `src/http/client.rs` | global reqwest client wrapper |
| `BaseError` | `src/error/mod.rs` | crate-wide structured errors |

## FEATURE GATES
| Feature | Enables |
|---------|---------|
| `token` | `src/token`, `TokenManager`, token-aware `GlobalTools` |
| `http` | `src/http` reqwest wrapper |
| `mysql` | sqlx-backed table/action/database execution |
| `validator` | stricter regex-backed field validators |
| `plugin-schema` | JSON Schema plugin config validation |

Default enables all features for compatibility.

## CONVENTIONS
- `yang-base` inherits workspace lints and sets `#![warn(missing_docs)]`.
- Most public docs and user-facing errors are Chinese; keep that style.
- Global singletons use `OnceLock`; repeated init should return `BaseError`, not panic.
- `action` and `table` are large enough to have child AGENTS.md; read those before editing either module.
- Unit tests are colocated in `__tests__/`; integration tests live in `tests/` and often require Docker.

## ANTI-PATTERNS
- Builtin actions are non-generic typed handlers over runtime `TableDefinition`: add/get/select use `Record`, dynamic primary keys live inside typed DTOs, and `.table(definition).crud()` installs the complete API set. Custom actions still follow `TypedHandler` → derived `TypedAction` → erased `DynAction`; do not use a bare `serde_json::Value` as the whole input/output contract.
- Register custom endpoints with `Api::{get,post,put,patch,delete}` and `ModuleRouter::api`/`apis`; do not split Action and route registration into separate calls.
- Plugin code is a 1.4k-line single file with existing unwraps; avoid adding new panic paths.
- Do not bypass `TableQuery`/`FieldPermissions` when handling user-selected fields.
- Do not add hardcoded production credentials; test/default Docker credentials belong only in test docs/examples.
- Token system supports revocation via Redis-based blacklist (`TokenManager::revoke_token` / `is_revoked` / `verify_token_checked` in `token/revocation.rs`). For auth paths that need logout/revoke support, use `verify_token_checked` instead of bare `verify_token` (which skips the blacklist check for backward compatibility).

## TESTING
```bash
cargo test --lib -p yang-base
cargo test --test <name> -- --ignored --test-threads=1
```
Docker tests use MySQL 8.0 via testcontainers or the `.kiro/steering` standard container. Prefer `.expect("具体上下文")` in tests over bare `.unwrap()`. Use `assert!(matches!(...))` for error-shape assertions.
