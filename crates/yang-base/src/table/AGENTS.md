# yang-base/table — Table System

**Parent:** `crates/yang-base/AGENTS.md`

## OVERVIEW
Schema-first table definitions, field validation and permissions, dynamic `Record` rows, schema compatibility checks, and guarded MySQL query execution.

## STRUCTURE
```text
table/
├── mod.rs                # public re-exports
├── definition.rs         # Table / Field builders, TableDefinition, TableHandle
├── field_type.rs         # FieldType enum, MySQL mapping, JSON validation
├── field_config.rs       # internal field metadata, permissions and relations
├── table_config.rs       # internal normalized schema metadata
├── query_params.rs       # filters, sorting and pagination request model
├── table_query/          # permission-aware query builder and execution（mod.rs 持 TableQuery 结构体与 MAX_TABLE_QUERY_PAGE_SIZE；impl 按职责分散于 build/filters/validation/plan/read/write；sql_render.rs 与 sql_param.rs 为 cfg(test) SQL 文本渲染与参数类型）
├── record.rs             # Record and MySQL row decoding
├── schema_validation.rs  # live-schema compatibility report
├── validator.rs          # Validator enum and validation helpers
└── __tests__/            # colocated unit tests
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Define an application table | `definition.rs` | Compose `Table::new(...).fields([...]).build()` |
| Define fields | `definition.rs` | `Field` constructors plus validation, permission, index and relation modifiers |
| Inspect immutable metadata | `definition.rs` | `TableDefinition` and `FieldMetadata` read-only views |
| Bind a database | `definition.rs` | `TableDefinition::bind` creates `TableHandle` when `mysql` is enabled |
| Dynamic rows | `record.rs` | `Record` is the transparent JSON object used by queries and builtin CRUD |
| Query request model | `query_params.rs` | selected fields, Boolean where trees, sort and pagination |
| DB query execution | `table_query/` | select/get/insert/update/delete/count/page operations（读路径 `read.rs`，写路径 `write.rs`，计划编译 `plan.rs`，链式入口 `filters.rs`，权限校验 `validation.rs`） |
| Schema drift | `schema_validation.rs` | additive/compatible/destructive issue classification |
| Validators | `validator.rs` | built-in validators; regex-backed variants depend on `validator` |

## CODE MAP
| Symbol | Location | Role |
|--------|----------|------|
| `Table` | `definition.rs` | schema-first table builder; `build` performs cross-field validation |
| `Field` | `definition.rs` | typed field builder with validation, permissions, indexes and relations |
| `TableDefinition` | `definition.rs` | immutable runtime schema and JSON Schema source |
| `FieldMetadata` | `definition.rs` | read-only view returned by `TableDefinition::field(s)` |
| `TableHandle` | `definition.rs` | a definition bound to a MySQL pool |
| `Record` | `record.rs` | transparent dynamic row with typed `require` / `optional` reads |
| `FieldType` | `field_type.rs` | String/Integer/BigInt/Float/Double/Boolean/Date/DateTime/Timestamp/Json/Text/Enum storage types |
| `RelationType` | `field_config.rs` | OneToOne/OneToMany/ManyToOne/ManyToMany relation metadata |
| `WhereCondition` | `query_params.rs` | validated Boolean filter tree |
| `TableQuery` | `table_query/mod.rs` | guarded query builder and executor（impl 分散在同目录职责文件中） |
| `SchemaValidationReport` | `schema_validation.rs` | deterministic live-schema compatibility result |

## APPLICATION CONTRACT
```rust
use yang_base::table::{col, Field, Table};

let users = Table::new("users")
    .label("用户表")
    .fields([
        Field::id("id"),
        Field::string("username", 64)
            .required()
            .unique()
            .filterable()
            .sortable(),
        Field::created_at("created_at"),
    ])
    .default_order(col("created_at").desc())
    .build()?;
```

- Application code constructs schemas only through `Table` and `Field`.
- `build` is the validation boundary and returns an immutable `TableDefinition`.
- A module binds its primary definition with `ModuleRouter::table`; extra startup-only schemas use `ModuleRouter::schema`.
- Builtin CRUD and `TableQuery` exchange dynamic rows as `Record`.

## CONVENTIONS
- All SQL identifiers must pass the centralized identifier validator; never concatenate untrusted table or field names.
- Permission checks happen before field selection, filtering, sorting or writes.
- Use `Field::id`, `created_at`, `updated_at` and `soft_delete` for generated-column semantics instead of recreating their flags manually.
- Keep storage types and relation metadata orthogonal: define a normal foreign-key column with `Field::bigint("user_id").relation("users", "id", RelationType::ManyToOne)`.
- Use `col("name")` for table-level indexes and default ordering.
- `Record` serializes as a plain JSON object; use `require::<T>` and `optional::<T>` for typed reads.
- Soft delete updates the declared soft-delete field instead of physically deleting a row.

## FEATURE NOTES
- `mysql` enables `TableHandle` and async execution methods in `TableQuery`.
- `validator` enables strict regex-backed Email/Phone/Regex validation.
- Date/DateTime/Timestamp validation lives in `field_type.rs`; keep accepted formats and tests aligned.

## ANTI-PATTERNS
- Do not expose or construct the normalized internal metadata structs from application code; keep `Table` / `Field` as the public declaration boundary.
- Do not bypass `TableDefinition` or `TableQuery` permission checks for user-controlled field names.
- Do not add raw SQL string filters; use validated `WhereCondition` / `where_*` APIs.
- Do not assume every `serde_json::Value` is an object; use `Record` for dynamic row-shaped payloads.
- Do not make row decoding silently drop unsupported MySQL values; return structured errors or explicit conversions.
