# yang-base/action — Action System

**Parent:** `crates/yang-base/AGENTS.md`

## OVERVIEW
Action extension point for backend operations. Wraps request data, current user, global tools, table config, unified responses, and builtin CRUD actions.

## STRUCTURE
```text
action/
├── mod.rs              # public re-exports
├── action_trait.rs     # Permission only（行为契约已迁移到 typed.rs）
├── auth.rs             # token feature：LoginAction / RefreshAction / LogoutAction
├── context.rs          # ActionContext, GlobalTools, User
├── meta.rs             # ActionMeta 聚合体（由 #[derive(Action)] 生成）
├── request.rs          # JSON body, headers, query, path params
├── request_id.rs       # 进程内 request_id（u128，日志/span/metrics/审计串联）
├── response.rs         # ApiResponse success/fail helpers
├── sql_bridge.rs       # WhereCondition → TableQuery 桥接（count_with_tree）
├── typed.rs            # TypedHandler / TypedAction / DynAction 三层 trait
├── builtin/            # add, put, del, get, select, table（H-1 类型化版本）
└── __tests__/          # colocated unit tests
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Define custom action | `typed.rs` | 实现 `TypedHandler::handle`，声明关联类型 `Input`/`Output`；加 `#[derive(Action)]` 获得元信息 |
| Extract params | `context.rs` | `param`, `param_optional`, `param_optional_strict`, `path_param`, `query_param`, `param_or` |
| Current user/roles | `context.rs` | `User`, `has_permission`, `has_role`, `user_roles_slice` |
| Global tools | `context.rs` | `GlobalTools` OnceLock singleton; optional `TokenManager` with `token` feature |
| Request wrapper | `request.rs` | chain `header`, `query`, `path_param`; `token()` handles Bearer token |
| Response wrapper | `response.rs` | `ApiResponse::success`, `success_value`, `fail`, `from_error` |
| Builtin CRUD | `builtin/*.rs` | 泛型 table-backed Actions：`AddAction<T>`, `PutAction<T>`, `DelAction<T>`, `GetAction<T>`, `SelectAction<T>`, `TableAction<T>` |
| Builtin tests | `builtin/__tests__/builtin_actions_test.rs` | CRUD action behavior |

## TYPED ACTION SYSTEM（H-1 重构已落地）

旧的对象安全 `Action` trait 已删除，现在是三层 trait 体系：

### 1. `TypedHandler` — 用户实现（`typed.rs`）
```rust
#[async_trait]
pub trait TypedHandler: Send + Sync + 'static {
    type Input: serde::de::DeserializeOwned + schemars::JsonSchema + Send;
    type Output: serde::Serialize + schemars::JsonSchema + Send;

    async fn handle(&self, ctx: ActionContext, input: Self::Input) -> Result<Self::Output, BaseError>;
}
```
- 唯一需要**手写**的 trait。`Input` / `Output` 是编译期契约，不在运行时拆 `serde_json::Value`。
- `Input` 自动从请求体反序列化（`ctx.extract_input()` 在 `DynAction::dispatch` blanket 中调用）。
- `Output` 自动序列化进 `ApiResponse::success`。

### 2. `TypedAction` — 元信息层（由 `#[derive(Action)]` 派生）
```rust
pub trait TypedAction: TypedHandler {
    fn name(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn permissions(&self) -> &'static [Permission];
    fn is_public(&self) -> bool;
    fn input_schema(&self) -> &'static schemars::schema::RootSchema;
    fn output_schema(&self) -> &'static schemars::schema::RootSchema;
    fn meta_static(&self) -> &'static ActionMeta;
}
```
- **不要手写** — 由 `#[derive(Action)]` 自动生成。聚合所有元信息到 `ActionMeta`（定义在 `meta.rs`）。

### 3. `DynAction` — 类型擦除层（router 存 `Arc<dyn DynAction>`）
```rust
pub trait DynAction: Send + Sync + 'static {
    async fn dispatch(&self, ctx: ActionContext) -> Result<ApiResponse, BaseError>;
    fn meta(&self) -> &'static ActionMeta;
}
```
- 通过 `blanket impl<T: TypedAction> DynAction for T` 自动桥接：所有 `TypedAction` 自动是 `DynAction`。
- `dispatch()` 里做了：`ctx.extract_input()` → `self.handle()` → `ApiResponse::success()`，外加 tracing span 和 metrics（`feature = "metrics"` 时）。

### 注册流程
```
用户: impl TypedHandler for MyAction + #[derive(Action)]
   → 派生宏生成 TypedAction impl + ActionMeta
   → blanket impl 自动获得 DynAction
   → Arc<dyn DynAction> 存入 router 注册表
```

## BUILTIN ACTIONS
| Action | File | Input | Output |
|--------|------|-------|--------|
| `AddAction<T>` | `builtin/add.rs` | `T`（整行数据） | `InsertResult` |
| `PutAction<T>` | `builtin/put.rs` | `PutInput<T>`（主键 + 更新字段） | `AffectedResult` |
| `DelAction<T>` | `builtin/del.rs` | `GetByPk<T::Pk>`（主键） | `AffectedResult` |
| `GetAction<T>` | `builtin/get.rs` | `GetByPk<T::Pk>`（主键） | `T` |
| `SelectAction<T>` | `builtin/select.rs` | `SelectQuery<T>`（筛选/排序/分页） | `SelectResult<T>` |
| `TableAction<T>` | `builtin/table.rs` | `EmptyInput` | `TableSchemaResponse` |

所有内置 Action 是泛型 `T: TableEntity`，通过 `ModuleRouter::table_typed::<T>()` 一行注册全套 CRUD。

## CONVENTIONS
- Keep action metadata Chinese-facing: display names/descriptions are user-visible.
- Prefer `context.param_optional_strict` when type mismatch should be an error; `param_optional` silently returns `None` and logs a warning.
- Use `ApiResponse::success` for serializable Rust types and `success_value` only when data is already a `serde_json::Value`.
- Initialize `GlobalTools` once before `ActionContext::new_with_global_tools`; repeated init returns `BaseError::ConfigError`.
- Table-backed actions require `ActionContext::with_table_config` or `context.table_query()` fails with `TableConfigNotSet`.
- 新 Action 优先使用类型化输入/输出（`TypedHandler`），避免 `serde_json::Value` 拆箱。

## ANTI-PATTERNS
- 旧代码中曾大量使用 `serde_json::Value` 做 I/O；H-1 之后内置 Action 已全部类型化。新自定义 Action 必须用 `TypedHandler`，不要回退到 `serde_json::Value`。
- Do not add public actions by overriding `is_public()` unless the route really needs no authentication.
- Avoid bare `unwrap()` in tests; project convention prefers `.expect("具体上下文")` or `assert!(matches!(...))`.
- `GlobalTools` uses lock recovery with `unwrap_or_else(|p| p.into_inner())`; do not replace it with plain `unwrap()` on poisoned locks.

## SECURITY: GlobalDatabase 绕过风险 (S-NEW-AUTH-5)

自定义 Action 作者注意：`GlobalDatabase::get()` 返回的是**未经权限校验**的原始数据库连接池。
通过它可直接构造 `yang_db::QueryBuilder` 绕过 `TableQuery` 提供的所有保护层：

- **字段级读写权限**（`ensure_fields_readable` / `ensure_fields_writable`）
- **软删除过滤**（`deleted_at IS NULL`）
- **WHERE 条件校验**（字段存在性、筛选权限）
- **慢查询阈值告警**（`slow_query_threshold`）
- **request_id 串联**（日志/审计链路断裂）

**正确做法**：始终通过 `ctx.table_query()?` 获取受保护的 `TableQuery`，它自动注入当前用户角色、
连接池、慢查询阈值和 request_id。仅在以下场景才考虑直接使用 `GlobalDatabase`：

1. 跨模块数据迁移/修复脚本（离线、无用户上下文）
2. 系统健康检查（不需要行级权限）
3. 必须在事务中混合多表操作且 `begin_transaction` 已提供受保护入口

即使在这些场景中，也应显式注入 `request_id` 并自行实现必要的权限校验。
