# 内置 CRUD Actions

本文对应 `yang-base` 0.2.0。启用 `mysql` 后，`ModuleRouter::table(TableDefinition).crud()` 会为模块原子注册六个标准 API。应用不需要逐个构造或绑定内置 Action。

## 注册

```rust
use yang_base::router::ModuleRouter;
use yang_base::table::{Field, Table};
use yang_base::BaseError;

fn user_module() -> Result<ModuleRouter, BaseError> {
    let users = Table::new("users")
        .label("用户表")
        .fields([
            Field::id("id"),
            Field::string("username", 64).required().unique(),
            Field::string("email", 128).required().email().unique(),
            Field::created_at("created_at"),
            Field::updated_at("updated_at"),
            Field::soft_delete("deleted_at"),
        ])
        .build()?;

    ModuleRouter::new("user", "用户管理")
        .table(users)
        .crud()
}
```

`.crud()` 要求模块已经绑定主表，否则返回 `BaseError::TableDefinitionNotSet`。主表定义同时驱动字段校验、权限、软删除、查询和 JSON Schema。

## API 契约

| Action | Method | Path | 输入 | 输出 |
|---|---|---|---|---|
| `add` | `POST` | `/api/{module}` | `Record` | `InsertResult { affected, id }` |
| `put` | `PUT` | `/api/{module}` | `PutInput { id, data: Record }` | `AffectedResult { affected }` |
| `del` | `DELETE` | `/api/{module}` | `GetByPk { id }` | `AffectedResult { affected }` |
| `get` | `GET` | `/api/{module}` | `GetByPk { id }` | `Record` |
| `select` | `POST` | `/api/{module}/query` | `SelectQuery` | `SelectResult { items: Vec<Record>, ... }` |
| `table` | `GET` | `/api/{module}/schema` | `{}` | `TableSchemaResponse` |

`Record` 序列化为普通 JSON object；它是新增、更新数据和动态查询结果的统一行模型。

## 权限与 Catalog

`.crud()` 按模块名生成权限，而不是把内置 Action 当成无权限公共接口：

- `add`、`put`、`del`：`{module}:write`
- `get`、`select`、`table`：`{module}:read`

runtime dispatch 与 `ApiCatalog` 共享这份契约。Catalog 的 schema 也绑定当前主表：`get` / `del` / `put` 使用真实主键字段类型，`add` / `put` 只包含允许写入的字段，读取结果不包含 secret 或无读权限字段，`select` 的筛选/排序字段枚举来自该表的权限元数据。

## 请求与响应示例

### add

请求体直接是记录对象：

```json
{
  "username": "alice",
  "email": "alice@example.com"
}
```

响应数据：

```json
{
  "affected": 1,
  "id": 42
}
```

### put

```json
{
  "id": 42,
  "data": {
    "email": "new-alice@example.com"
  }
}
```

`data` 不能为空；字段存在性、类型和写权限由主表定义校验。

### del 与 get

两者都接受动态类型的主键值：

```json
{
  "id": 42
}
```

`del` 在表定义包含软删除字段时执行标记更新，否则执行物理删除。`get` 返回一条 `Record`；记录不存在时返回 `BaseError::RecordNotFound`。

### select

```json
{
  "page": 1,
  "page_size": 20,
  "order_by": [
    { "field": "created_at", "direction": "desc" }
  ],
  "count_total": true
}
```

- `page` 从 1 开始，默认 1。
- `page_size` 默认 10，范围为 1..=100。
- `where` 接受 `WhereCondition` 布尔树。
- `order_by` 的字段和方向会经过字段权限校验。
- 只有 `count_total` 为 `true` 时，响应中的 `total` 才有值。

WHERE 的字段、操作符和值会在 SQL 生成前按 `TableDefinition` 校验，`IN` 和 `BETWEEN` 逐项验证。`Eq` / `Ne` 与 JSON `null` 的比较分别生成 `IS NULL` / `IS NOT NULL`；显式 `IsNull` / `IsNotNull` 具有相同语义。

响应数据：

```json
{
  "items": [
    { "id": 42, "username": "alice" }
  ],
  "page": 1,
  "page_size": 20,
  "total": 1
}
```

### table

请求输入为 `{}`，响应数据为：

```json
{
  "table_name": "users",
  "primary_key": "id",
  "input_schema": { "type": "object" },
  "output_schema": { "type": "object" }
}
```

输入、输出 JSON Schema 来自同一个不可变表定义；禁止读取或写入的字段不会进入对应 schema。

## 在 CRUD 之外添加 API

自定义端点使用 `Api` 把类型化 Action 与 HTTP 元数据绑定为一个注册值：

```rust
use yang_base::action::TypedAction;
use yang_base::router::{Api, ModuleRouter};
use yang_base::table::TableDefinition;
use yang_base::BaseError;

fn user_module(
    users: TableDefinition,
    profile_action: impl TypedAction,
) -> Result<ModuleRouter, BaseError> {
    ModuleRouter::new("user", "用户管理")
        .table(users)
        .crud()?
        .api(
            Api::get("/api/user/profile", profile_action)
                .operation_id("user.profile")
                .tag("user"),
        )
}
```

标准 CRUD 与自定义 `Api` 最终都进入同一个 `ApiCatalog`，供 HTTP 适配、文档和可选 OpenAPI 3.1 投影使用。

`Api` 路径在注册期按 Axum 0.8 校验：动态段使用 `{id}` / `{*path}`，旧式 `:id` / `*path` 会立即失败。`AppRouter::catalog()` 还会拒绝跨模块的匹配模板和 operation id 冲突，transport adapter 只消费已经验证的目录。

完整启动流程参见[快速开始](../guides/quick_start.md)。
