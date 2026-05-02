# yang-base — Backend Services Library

**Parent:** lib_yang workspace

## OVERVIEW
Provides plugin management, database access (MySQL/Redis), HTTP client, JWT tokens, action system, table configuration, and error handling — the application-facing layer built on yang-db.

## STRUCTURE
```
yang-base/
├── src/
│   ├── lib.rs           # 8 public modules exported
│   ├── database/        # GlobalDatabase + GlobalRedis init
│   ├── plugin/          # Plugin trait + PluginManager (TODO: JSON Schema)
│   ├── action/          # Action trait + builtin actions
│   │   └── builtin/     # select, get, insert, update, delete
│   ├── http/            # HttpClient (reqwest wrapper)
│   ├── token/           # JWT TokenManager
│   ├── table/           # Table struct definitions
│   ├── router/          # Router configuration
│   └── error/           # BaseError type
├── tests/               # Integration tests (9 files, Docker required)
├── docs/                # API docs, guides, reference
└── examples/            # Example usage (4 files)
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Database init | `src/database/mod.rs` | GlobalDatabase::init(), GlobalRedis::init() |
| Plugin management | `src/plugin/mod.rs` | Plugin trait, init_sql(), migration_sql() |
| Custom actions | `src/action/mod.rs` + `src/action/builtin/` | Implement Action trait |
| HTTP requests | `src/http/mod.rs` | Global singleton HttpClient |
| Token generation/validation | `src/token/mod.rs` | TokenManager with JWT |
| Table configuration | `src/table/mod.rs` | Table struct, field definitions |
| Error handling | `src/error/mod.rs` | BaseError with error codes |
| Full documentation | `docs/api/`, `docs/guides/` | Usage guides, quick reference, Redis guide |

## CONVENTIONS
- Depends on `yang-db` for all database types (DatabaseConfig, RedisConfig, etc.)
- Uses global singletons: `GlobalDatabase`, `GlobalRedis`, `HttpClient`
- All public modules re-exported from `lib.rs`
- Error type: `BaseError` (extends `std::error::Error`)

## ANTI-PATTERNS
- **TODO**: Plugin JSON Schema validation not implemented
- **TODO**: `builtin/select.rs` and `builtin/get.rs` use `serde_json::Value` — needs concrete types
- Test files use excessive `unwrap()` — replace with proper error handling
