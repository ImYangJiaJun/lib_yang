# yang-db — Database Abstraction Layer

**Parent:** lib_yang workspace

## OVERVIEW
Type-safe MySQL query builder and Redis client. Provides Database, QueryBuilder, Condition, Transaction (MySQL) plus RedisClient, pipeline, transactions (Redis). Consumed by yang-base.

## STRUCTURE
```
yang-db/
├── src/
│   ├── lib.rs           # Re-exports: DbError, Database, QueryBuilder, RedisClient, etc.
│   ├── error.rs         # DbError enum
│   ├── mysql/           # 7 files: Database, QueryBuilder, Condition, Transaction, types
│   └── redis/           # 6 files: RedisClient, RedisPipeline, RedisTransaction, types
├── tests/               # 16 integration test files
├── docs/                # API docs, testing guides
└── proptest-regressions/ # Property-based test regression files
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| MySQL CRUD | `src/mysql/database.rs` | `Database::table()`, `insert()`, `update()`, `delete()` |
| Query building | `src/mysql/query_builder.rs` | Chainable WHERE, ORDER, LIMIT, JOIN |
| Conditions | `src/mysql/condition.rs` | Condition enum + SqlValue types |
| Transactions | `src/mysql/transaction.rs` | MySQL transaction wrapper |
| Redis ops | `src/redis/client.rs` | 29+ Redis methods (string, hash, list, set, zset) |
| Redis pipeline | `src/redis/pipeline.rs` | Redis pipeline (should use redis::pipe) |
| Error types | `src/error.rs` | DbError (ConnectionError, QueryError, ConstraintError, etc.) |
| Test docs | `docs/testing/test_summary.md` | Test infrastructure overview |

## CONVENTIONS
- All public types re-exported at crate root (`pub use mysql::*`, `pub use redis::*`)
- `Result<T>` = `std::result::Result<T, DbError>`
- Type-safe: `QueryBuilder` is generic over return type — compile-time validation
- Edition: 2024 (BUG — should be 2021)
- 100% test pass rate (147 unit + 29 integration tests)

## ANTI-PATTERNS
- **CRITICAL BUG**: `RedisConfig` connection pool parameters never applied to pool
- **CRITICAL BUG**: `insert_batch` no auto-batching — large datasets hit `max_allowed_packet`
- Redis Pipeline/Transaction/Lua should use `redis::pipe()` / `redis::Script` APIs, not custom implementations
- `having()` method uses raw string (no SQL injection protection)
- `update_batch` uses CASE WHEN — JOIN approach is more optimal
- Missing: UPSERT, IS NULL/IS NOT NULL, pool health checks, blocking timeout config
