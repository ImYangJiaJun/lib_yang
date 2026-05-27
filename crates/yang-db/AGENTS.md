# yang-db — Database Abstraction Layer

**Parent:** lib_yang workspace

## OVERVIEW
Type-safe MySQL query builder plus Redis client. Exposes `Database`, `QueryBuilder`, `Condition`, transactions, `RedisClient`, pipelines, Redis transactions, script helpers, config, values, and `DbError`. Consumed by `yang-base`.

## STRUCTURE
```text
yang-db/
├── src/
│   ├── lib.rs              # crate-level re-exports + Result<T>
│   ├── error.rs            # DbError conversions
│   ├── mysql/
│   │   ├── query_builder.rs # 4.8k-line SQL builder hotspot
│   │   ├── condition.rs     # Condition / SqlValue / SQL conversion
│   │   ├── database.rs      # sqlx MySQL pool wrapper
│   │   ├── transaction.rs   # MySQL transaction wrapper
│   │   ├── field.rs         # FieldType, JoinClause, OrderClause
│   │   └── init.rs          # migration config placeholder
│   └── redis/
│       ├── client.rs        # 2k-line Redis API surface
│       ├── config.rs        # RedisConfig + pool params
│       ├── pipeline.rs      # RedisPipeline wrapper over redis::pipe()
│       ├── transaction.rs   # WATCH/MULTI/EXEC helper
│       └── value.rs         # RedisValue conversions
├── tests/                  # integration tests; many Docker-dependent/ignored
├── docs/                   # examples/testing/reference docs
└── proptest-regressions/   # query_builder regression corpus
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| MySQL connection/raw SQL | `src/mysql/database.rs` | `Database::connect`, `table`, `query`, `execute`, params helpers |
| Query building | `src/mysql/query_builder.rs` | select/find/value/count/sum/avg/min/max/insert/update/delete/upsert/batch |
| Conditions | `src/mysql/condition.rs` | `Condition`, `SqlValue`, `condition_to_sql(_owned)` |
| Transactions | `src/mysql/transaction.rs` | `Transaction`, `TransactionQueryBuilder` |
| Redis operations | `src/redis/client.rs` | string/hash/list/set/zset/key/pubsub/lua/scan/health |
| Redis config | `src/redis/config.rs` | max connections, connect/wait timeout, logging |
| Redis pipeline | `src/redis/pipeline.rs` | wrapper around `redis::pipe()` |
| Redis transaction | `src/redis/transaction.rs` | optimistic locking with retry |
| Redis value mapping | `src/redis/value.rs` | conversion from `redis::Value` |
| Errors | `src/error.rs` | `DbError` variants and From impls |

## CODE MAP
| Symbol | Location | Role |
|--------|----------|------|
| `Database` | `src/mysql/database.rs` | MySQL pool and raw query entry |
| `QueryBuilder` | `src/mysql/query_builder.rs` | main chainable SQL builder |
| `Condition` | `src/mysql/condition.rs` | nested boolean condition tree |
| `SqlValue` | `src/mysql/condition.rs` | bind parameter representation |
| `Transaction` | `src/mysql/transaction.rs` | transaction lifecycle wrapper |
| `RedisClient` | `src/redis/client.rs` | Redis operation API |
| `RedisConfig` | `src/redis/config.rs` | pool/timeouts/logging config |
| `RedisPipeline` | `src/redis/pipeline.rs` | batched Redis commands |
| `RedisTransaction` | `src/redis/transaction.rs` | WATCH/MULTI/EXEC with retry |
| `RedisValue` | `src/redis/value.rs` | typed Redis response values |
| `DbError` | `src/error.rs` | database/Redis error type |

## CONVENTIONS
- Public API is re-exported from `src/lib.rs`; downstream crates often import directly from `yang_db`.
- `Result<T>` aliases `std::result::Result<T, DbError>`.
- `update()` and `delete()` require WHERE conditions; empty conditions return `MissingWhereClause`.
- Use checked operator APIs (`where_and`, `where_or`, `having_cond`) for user input.
- `insert_batch` auto-splits at 500 rows; use `insert_batch_with_size` for custom batch sizes.
- Redis scripts use `redis::Script`; pipeline/transaction wrappers already build on `redis::pipe()`.

## HOTSPOTS
- `src/mysql/query_builder.rs`: 4.8k lines, largest file; touches almost every SQL behavior.
- `src/redis/client.rs`: 2k lines, large Redis operation surface.
- `src/mysql/condition.rs`: complex expression tree and SQL conversion.
- `src/mysql/transaction.rs`: separate tx-bound query builder; operator behavior differs in places.

## ANTI-PATTERNS
- Do not add new production `unwrap()`/`expect()` just because crate lints allow them.
- Avoid `_unchecked` query helpers unless the operator was already validated; `having_cond_unchecked` can panic.
- Do not reintroduce stale docs claiming `RedisConfig` pool params are unused; `connect_with_config` applies pool/timeouts now.
- Do not reintroduce stale docs claiming `insert_batch` lacks batching; it defaults to 500-row batches now.
- Do not hardcode `root:111111` outside local Docker/test examples.
- Do not split `query_builder.rs` opportunistically during feature work; refactor only with focused tests because coupling is high.

## TESTING
```bash
cargo test --lib -p yang-db
cargo test --test <name> -- --ignored --test-threads=1
```
- `src/mysql/__tests__/` holds colocated SQL-generator/batch tests.
- `query_builder.rs` and `condition.rs` contain property tests.
- `tests/` has MySQL/Redis integration tests; Docker-dependent tests are ignored.
- `proptest-regressions/mysql/query_builder.txt` is intentional.
