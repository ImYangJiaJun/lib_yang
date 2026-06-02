# yang-base-derive

`yang-base` 的过程宏（proc-macro）crate，提供类型化 Action 系统的派生基础设施。

> 一般不需要直接依赖本 crate。`yang-base` 已通过 `pub use yang_base_derive::{Action, TableEntity};`
> 重导出这两个派生宏，下游直接写 `yang_base::TableEntity` / `yang_base::Action` 即可。

## 提供的派生宏

### `#[derive(TableEntity)]`

为 struct 生成表实体的全部类型化基础设施：

- `<Name>Field` 枚举 —— 每个字段一个变体，impl `AsColumnName`，提供封闭的列名集合，杜绝任意字符串列名拼接。
- `<Name>Where` 枚举 —— 按列类型生成 `WhereOp<T>`（字符串列用 `StringWhereOp`），impl `IntoSqlCondition`。
- `impl TableEntity` —— 关联类型 `Pk` / `Field` / `WhereCond`，常量 `TABLE_NAME` / `PK_FIELD`，以及 `OnceLock` 缓存的运行时 `TableConfig`。

编译期校验：必须恰好一个 `#[entity(primary_key)]`（多个或零个直接编译失败），仅支持具名字段 struct。

属性：

- 表级 `#[table(name = "...", display_name = "...", soft_delete = "...")]`
- 字段级 `#[entity(primary_key, max_length = N, unique, required = bool, column = "...", skip)]`

### `#[derive(Action)]`

为 Action struct 生成 `TypedAction` impl 与静态 `ActionMeta` 聚合体：

- impl `TypedAction`（`name` / `display_name` / `description` / `is_public` / `permissions`）。
- 用 `schemars::schema_for!` 从 `TypedHandler::Input` / `Output` 生成静态 JSON Schema。
- 聚合出 `OnceLock<ActionMeta>`，供 `ModuleRouter::dispatch` 读取以做鉴权与派发。

依赖手写的 `TypedHandler`（声明 `Input` / `Output` 关联类型 + `handle` 业务逻辑）；本宏只补元数据与 trait 胶水。

属性：`#[action(name = "...", display_name = "...", description = "...", public, permissions("a", "b"))]`，其中 `name` 必填。

## 使用示例

```rust
use yang_base::{TableEntity, Action};
use yang_base::action::TypedHandler;

#[derive(TableEntity, serde::Serialize, serde::Deserialize, schemars::JsonSchema, sqlx::FromRow)]
#[table(name = "users")]
struct User {
    #[entity(primary_key)]
    id: i64,
    #[entity(max_length = 50, unique)]
    name: String,
}
```

内置六个 CRUD Action（add/put/del/get/select/table）即用本 crate 派生，
通过 `ModuleRouter::table_typed::<User>()` 一行注册全套。

## 许可证

MIT OR Apache-2.0

