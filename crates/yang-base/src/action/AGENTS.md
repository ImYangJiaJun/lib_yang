# yang-base/action — Action System

**Parent:** `crates/yang-base/AGENTS.md`

## OVERVIEW
Action extension point for backend operations. Wraps request data, current user, global tools, table config, unified responses, and builtin CRUD actions.

## STRUCTURE
```text
action/
├── mod.rs              # public re-exports
├── action_trait.rs     # Action trait + Permission
├── context.rs          # ActionContext, GlobalTools, User
├── request.rs          # JSON body, headers, query, path params
├── response.rs         # ApiResponse success/fail helpers
├── builtin/            # add, put, del, get, select, table
└── __tests__/          # colocated unit tests
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Define custom action | `action_trait.rs` | implement `Action::execute`, `name`; optional metadata/schema/permissions |
| Extract params | `context.rs` | `param`, `param_optional`, `param_optional_strict`, `path_param`, `query_param`, `param_or` |
| Current user/roles | `context.rs` | `User`, `has_permission`, `has_role`, `user_roles_slice` |
| Global tools | `context.rs` | `GlobalTools` OnceLock singleton; optional `TokenManager` with `token` feature |
| Request wrapper | `request.rs` | chain `header`, `query`, `path_param`; `token()` handles Bearer token |
| Response wrapper | `response.rs` | `ApiResponse::success`, `success_value`, `fail`, `from_error` |
| Builtin CRUD | `builtin/*.rs` | table-backed add/put/del/get/select/table actions |
| Builtin tests | `builtin/__tests__/builtin_actions_test.rs` | CRUD action behavior |

## ACTION TRAIT
Required:
- `execute(&self, ActionContext) -> Result<ApiResponse, BaseError>`
- `name(&self) -> &str`

Optional defaults:
- `display_name()` -> `name()`
- `description()` -> `""`
- `permissions()` -> `&[]`
- `params_schema()` -> `None`
- `is_public()` -> `false`

## BUILTIN ACTIONS
| Action | File | Input | Output |
|--------|------|-------|--------|
| `AddAction` | `builtin/add.rs` | body `data` object | affected row count |
| `PutAction` | `builtin/put.rs` | id + update data | affected row count |
| `DelAction` | `builtin/del.rs` | primary key/path id | affected row count |
| `GetAction` | `builtin/get.rs` | primary key/path id | one row as JSON value |
| `SelectAction` | `builtin/select.rs` | filters/sort/page params | paginated JSON data |
| `TableAction` | `builtin/table.rs` | none | table metadata |

## CONVENTIONS
- Keep action metadata Chinese-facing: display names/descriptions are user-visible.
- Prefer `context.param_optional_strict` when type mismatch should be an error; `param_optional` silently returns `None` and logs a warning.
- Use `ApiResponse::success` for serializable Rust types and `success_value` only when data is already a `serde_json::Value`.
- Initialize `GlobalTools` once before `ActionContext::new_with_global_tools`; repeated init returns `BaseError::ConfigError`.
- Table-backed actions require `ActionContext::with_table_config` or `context.table_query()` fails with `TableConfigNotSet`.

## ANTI-PATTERNS
- Builtin actions use `serde_json::Value` for several inputs/outputs. Do not expand this pattern casually; prefer typed structs for new custom actions.
- Do not add public actions by overriding `is_public()` unless the route really needs no authentication.
- Avoid bare `unwrap()` in tests; project convention prefers `.expect("具体上下文")` or `assert!(matches!(...))`.
- `GlobalTools` uses lock recovery with `unwrap_or_else(|p| p.into_inner())`; do not replace it with plain `unwrap()` on poisoned locks.
