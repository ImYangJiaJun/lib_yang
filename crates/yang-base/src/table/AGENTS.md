# yang-base/table — Table System

**Parent:** `crates/yang-base/AGENTS.md`

## OVERVIEW
Table-aware schema, validation, permissions, dynamic row decoding, and query building layer used by builtin actions and backend modules.

## STRUCTURE
```text
table/
├── mod.rs              # public exports
├── field_type.rs       # FieldType enum, MySQL mapping, JSON validation
├── field_config.rs     # FieldConfig, validators, permissions
├── table_config.rs     # TableConfig, indexes, timestamps, soft delete
├── query_params.rs     # filters/sorts/pagination request model
├── table_query.rs      # TableQuery chainable DB operations
├── dynamic_row.rs      # DynamicRow and BLOB base64 JSON conversion
├── validator.rs        # Validator enum and validation helpers
└── __tests__/          # colocated unit tests
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Field type behavior | `field_type.rs` | validation, SQL type mapping, JSON conversion |
| Field metadata | `field_config.rs` | labels, required/default/validators/permissions |
| Table schema | `table_config.rs` | builder-style table config, indexes, timestamps, soft delete |
| Query request model | `query_params.rs` | selected fields, filters, sort, pagination |
| DB query execution | `table_query.rs` | select/get/insert/update/delete/count/paginate |
| Dynamic rows | `dynamic_row.rs` | MySQL row -> JSON-ish map, BLOB encoded with base64 |
| Validators | `validator.rs` | enum validators; regex-backed variants are feature-sensitive |
| Tests | `__tests__/` | field/table/query/validator coverage |

## CODE MAP
| Symbol | Location | Role |
|--------|----------|------|
| `FieldType` | `field_type.rs` | String/Integer/BigInt/Float/Double/Boolean/Date/DateTime/Timestamp/Json/Text/Enum/ForeignKey/Blob |
| `FieldConfig` | `field_config.rs` | per-field name/type/display/permission/validator/default metadata |
| `FieldPermissions` | `field_config.rs` | role-based read/write/filter/sort controls |
| `TableConfig` | `table_config.rs` | table name, fields, indexes, default order, soft delete, timestamps |
| `QueryParams` | `query_params.rs` | request-facing filtering/sorting/pagination DTO |
| `TableQuery` | `table_query.rs` | table-aware query builder/executor |
| `DynamicRow` | `dynamic_row.rs` | dynamic MySQL row serialization bridge |
| `PaginatedResult` | `mod.rs` | page/page_size/total/total_pages/data container |

## CONVENTIONS
- Table config uses builder chaining (`TableConfig::new(...).field(...).soft_delete(...).timestamps(...)`).
- All SQL identifiers must pass `is_valid_identifier`; don't concatenate untrusted field/table names.
- Permission checks are role-based and happen before field selection/filtering/sorting/writing.
- `TableQuery` uses `Arc<TableConfig>` and `Arc<[String]>` to avoid cloning configs/roles.
- `DynamicRow` serializes BLOB bytes to base64 strings for JSON output.
- Soft delete updates the configured field instead of physical delete when `soft_delete_field` is set.

## FEATURE NOTES
- `mysql` feature enables async DB execution methods in `TableQuery`.
- `validator` feature enables stricter regex-backed validators; without it, some validators degrade to simpler checks.
- Date/DateTime/Timestamp validation now exists in `field_type.rs`; keep tests aligned when changing accepted formats.

## ANTI-PATTERNS
- Do not bypass `TableConfig::validate_field` or `TableQuery` permission checks for user-controlled field names.
- Do not add raw SQL string filters; use validated `QueryParams` / `where_*` patterns.
- Do not assume all `serde_json::Value` inputs are objects; builtin actions explicitly validate object shapes before insert/update.
- Do not make `DynamicRow` silently drop unsupported MySQL values; return structured errors or explicit conversions.
- Avoid adding more `#[allow(dead_code)]` around table internals; existing ones mark reserved getter/pool hooks.
