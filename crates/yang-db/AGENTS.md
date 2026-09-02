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
│   ├── isolation.rs        # IsolationLevel enum (NG-2)
│   ├── dialect.rs          # SQL 方言抽象：引号/占位符风格 + 条件树共享渲染（crate 内部）
│   ├── mysql/
│   │   ├── query_builder/  # SQL builder（mod.rs 定义类型，impl 按职责分散于 generator/builder/render/read/aggregate/write/predicate/bind）
│   │   ├── condition.rs     # Condition / SqlValue / SQL conversion
│   │   ├── database.rs      # sqlx MySQL pool wrapper
│   │   ├── transaction.rs   # MySQL transaction wrapper
│   │   ├── field.rs         # FieldType, JoinClause, OrderClause
│   │   ├── identifier.rs    # SQL标识符校验与转义 (DB-1)
│   │   └── init.rs          # migration config placeholder
│   ├── postgres/
│   │   ├── query_builder.rs # 2.1k-line PostgreSQL SQL builder
│   │   ├── condition.rs     # PgCondition / SqlValue
│   │   ├── database.rs      # sqlx PgPool wrapper
│   │   ├── transaction.rs   # PostgreSQL transaction wrapper
│   │   ├── field.rs         # PgFieldType, JoinClause, OrderClause
│   │   └── identifier.rs    # SQL标识符校验与转义 (PG方言)
│   └── redis/
│       ├── client.rs        # 2.2k-line Redis API surface
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
| Query building | `src/mysql/query_builder/` | select/find/value/count/sum/avg/min/max/insert/update/delete/upsert/batch；类型在 `mod.rs`，执行逻辑分属 `read.rs`/`aggregate.rs`/`write.rs` |
| Conditions | `src/mysql/condition.rs` | `Condition`, `SqlValue`, `condition_to_sql(_owned)` |
| Transactions | `src/mysql/transaction.rs` | `Transaction`, `TransactionQueryBuilder` |
| Redis operations | `src/redis/client.rs` | string/hash/list/set/zset/key/pubsub/lua/scan/health |
| Redis config | `src/redis/config.rs` | max connections, connect/wait timeout, logging |
| Redis pipeline | `src/redis/pipeline.rs` | wrapper around `redis::pipe()` |
| Redis transaction | `src/redis/transaction.rs` | optimistic locking with retry |
| Redis value mapping | `src/redis/value.rs` | conversion from `redis::Value` |
| Errors | `src/error.rs` | `DbError` variants and From impls |
| PostgreSQL connection/raw SQL | `src/postgres/database.rs` | `PgDatabase::connect`, query, execute |
| PostgreSQL query building | `src/postgres/query_builder.rs` | select/find/insert/update/delete/upsert/batch |
| PostgreSQL conditions | `src/postgres/condition.rs` | `PgCondition`, `pg_condition_to_sql` |
| PostgreSQL transactions | `src/postgres/transaction.rs` | `PgTransaction`, transaction query builder |
| PostgreSQL field types | `src/postgres/field.rs` | `PgFieldType`, `JoinClause`, `OrderClause` |
| Redis pool health | `src/redis/client.rs` | `RedisClient::pool_status()` → `PoolStatus` |
| Transaction isolation | `src/isolation.rs` | `IsolationLevel` enum, `as_sql()` |

## CODE MAP
| Symbol | Location | Role |
|--------|----------|------|
| `Database` | `src/mysql/database.rs` | MySQL pool and raw query entry |
| `QueryBuilder` | `src/mysql/query_builder/mod.rs` | main chainable SQL builder（impl 分散在同目录各职责文件中） |
| `Condition` | `src/mysql/condition.rs` | nested boolean condition tree |
| `SqlValue` | `src/mysql/condition.rs` | bind parameter representation |
| `SqlExpr` | `src/reference.rs` | 受控服务端标量表达式白名单（UNIX_TIMESTAMP 系列），渲染固定片段+绑定参数 |
| `Transaction` | `src/mysql/transaction.rs` | transaction lifecycle wrapper |
| `RedisClient` | `src/redis/client.rs` | Redis operation API |
| `RedisConfig` | `src/redis/config.rs` | pool/timeouts/logging config |
| `RedisPipeline` | `src/redis/pipeline.rs` | batched Redis commands |
| `RedisTransaction` | `src/redis/transaction.rs` | WATCH/MULTI/EXEC with retry |
| `RedisValue` | `src/redis/value.rs` | typed Redis response values |
| `DbError` | `src/error.rs` | database/Redis error type |
| `PgDatabase` | `src/postgres/database.rs` | PostgreSQL pool and raw query entry |
| `PgQueryBuilder` | `src/postgres/query_builder.rs` | PostgreSQL chainable SQL builder |
| `PgCondition` | `src/postgres/condition.rs` | PostgreSQL nested boolean condition tree |
| `PgFieldType` | `src/postgres/field.rs` | PostgreSQL field type markers |
| `PoolStatus` | `src/redis/client.rs` | Redis pool health snapshot |
| `IsolationLevel` | `src/isolation.rs` | SQL标准四级事务隔离 |
| `Dialect` / `CondNode` / `render_condition` | `src/dialect.rs` | crate 内部方言抽象；mysql/postgres 的 identifier/condition 为其薄封装 |

## CONVENTIONS
- Public API is re-exported from `src/lib.rs`; downstream crates often import directly from `yang_db`.
- `Result<T>` aliases `std::result::Result<T, DbError>`.
- `update()` and `delete()` require WHERE conditions; empty conditions return `MissingWhereClause`.
- Use checked operator APIs (`where_and`, `where_or`, `having_cond`) for user input.
- `insert_batch` auto-splits at 500 rows; use `insert_batch_with_size` for custom batch sizes.
- 服务端时间用 `SqlExpr` 白名单（`unix_timestamp()`/`unix_timestamp_add(s)`），经 `set_expr`（INSERT VALUES / UPDATE SET）、`where_expr`（列↔表达式比较）、`select_expr`（投影+受控别名）接入构造器；`insert_returning_id` 显式返回自增主键。事务内行锁 SELECT 的构建器用 `QueryBuilder::from_pool(db.pool(), ...)` 创建后交给 `Transaction::select_for_update`。
- Redis scripts use `redis::Script`; pipeline/transaction wrappers already build on `redis::pipe()`.

## HOTSPOTS
- `src/mysql/query_builder/`: 原 6.4k 行单文件已按职责拆分（generator/builder/render/read/aggregate/write/predicate/bind），耦合仍高，跨文件改动保持谨慎。
- `src/redis/client.rs`: 2.2k lines, large Redis operation surface.
- `src/postgres/query_builder.rs`: 2.1k lines, PostgreSQL SQL builder.
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
