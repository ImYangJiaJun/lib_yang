# yang-base 0.2.0 快速开始

本指南用一条完整主链路搭建 schema-first 模块：定义表、注册标准 CRUD、追加自定义 `Api`、组装 `AppRouter`，并在启动时同步 schema。

## 1. 添加依赖

默认 features 包含 MySQL、Redis、Token、HTTP、validator 与 plugin schema：

```toml
[dependencies]
yang-base = "0.2.0"
```

只启用核心模型和 MySQL：

```toml
[dependencies]
yang-base = { version = "0.2.0", default-features = false, features = ["mysql"] }
```

## 2. 定义 TableDefinition

应用通过 `Table` 和 `Field` 声明 schema；`build()` 是集中校验边界。

```rust
use yang_base::table::{col, Field, Table, TableDefinition};
use yang_base::BaseError;

fn users_table() -> Result<TableDefinition, BaseError> {
    Table::new("users")
        .label("用户表")
        .fields([
            Field::id("id").label("ID"),
            Field::string("username", 64)
                .label("用户名")
                .required()
                .length(3..=64)
                .unique()
                .filterable()
                .sortable(),
            Field::string("email", 128)
                .label("邮箱")
                .required()
                .email()
                .unique(),
            Field::boolean("active").default(true),
            Field::created_at("created_at"),
            Field::updated_at("updated_at"),
            Field::soft_delete("deleted_at"),
        ])
        .default_order(col("created_at").desc())
        .build()
}
```

返回的 `TableDefinition` 是运行时唯一表契约，用于字段权限、查询、内置 CRUD、JSON Schema 与数据库同步。

## 3. 注册标准 CRUD

启用 `mysql` 后，把主表绑定到模块并调用 `.crud()`：

```rust
use yang_base::router::ModuleRouter;
use yang_base::BaseError;

fn user_crud_module() -> Result<ModuleRouter, BaseError> {
    ModuleRouter::new("user", "用户管理")
        .table(users_table()?)
        .crud()
}
```

该调用原子注册：

| Action | Method | Path |
|---|---|---|
| `add` | POST | `/api/user` |
| `put` | PUT | `/api/user` |
| `del` | DELETE | `/api/user` |
| `get` | GET | `/api/user` |
| `select` | POST | `/api/user/query` |
| `table` | GET | `/api/user/schema` |

新增、更新和查询行统一使用 `Record`；主键输入使用 `serde_json::Value`，因此不把业务主键锁死为某个 Rust 整数类型。

权限也由 `.crud()` 一次生成：`add`、`put`、`del` 需要 `user:write`，`get`、`select`、`table` 需要 `user:read`。runtime dispatch 与 `ApiCatalog` 使用同一份授权契约；Catalog 的主键、记录和查询 schema 从当前 `users_table()` 生成，不是六个跨表复用的宽泛占位 schema。

## 4. 添加自定义 Api

自定义业务逻辑实现 `TypedHandler`，再通过 `#[derive(Action)]` 获得可注册的 `TypedAction`。注册时用 `Api` 同时指定 handler 与 HTTP 元数据：

```rust
use yang_base::action::TypedAction;
use yang_base::router::{Api, AppRouter, ModuleRouter};
use yang_base::BaseError;

fn build_app(profile_action: impl TypedAction) -> Result<AppRouter, BaseError> {
    let user_module = ModuleRouter::new("user", "用户管理")
        .table(users_table()?)
        .crud()?
        .api(
            Api::get("/api/user/profile", profile_action)
                .operation_id("user.profile")
                .tag("user"),
        )?;

    AppRouter::new().module(user_module)
}
```

批量自定义端点使用 `ModuleRouter::apis`。`Api::{get,post,put,patch,delete}` 会把不同具体类型的 Action 转为统一注册值。

Action 默认受保护；用 `#[action(..., public)]` 标记注册、登录等公开接口。公开与受保护端点可以放在同一个 `ModuleRouter`，因为 `TokenAuthMiddleware` 只作用于 `MiddlewareScope::ProtectedActions`；未改变 scope 的日志、限流和追踪中间件仍覆盖全部 Action。

路由模板在 `api` / `apis` 注册时按 Axum 0.8 校验。路径参数写成 `/api/user/{id}`，尾部通配写成 `/files/{*path}`；旧式 `:id` 和 `*path` 会立即返回配置错误。`AppRouter::catalog()` 还会在 transport 启动前检查跨模块模板与 operation id 冲突。

## 5. 使用 Record

`Record` 序列化为普通 JSON object，是动态行的公开模型：

```rust
use yang_base::table::Record;
use yang_base::BaseError;

fn record_example() -> Result<(), BaseError> {
    let mut row = Record::new()
        .set("username", "alice")
        .set("email", "alice@example.com");
    row.insert("active", true);

    let username: String = row.require("username")?;
    let nickname: Option<String> = row.optional("nickname")?;
    println!("{username}: {nickname:?}");
    Ok(())
}
```

- `set` 链式写入字段。
- `insert` 原地写入并返回旧值。
- `require::<T>` 读取必需字段。
- `optional::<T>` 把缺失或 `null` 映射为 `None`。
- `as_map` / `into_map` 提供只读或所有权形式的 JSON map 访问。

请求内的表访问优先使用 `ActionContext::table_query()`，它会带上当前用户角色、字段权限、软删除和 request id。

WHERE 条件在访问数据库前按字段类型验证，`IN` 和 `BETWEEN` 的值也逐项检查。`where_eq(field, Value::Null)` / `where_ne(field, Value::Null)` 会生成 `IS NULL` / `IS NOT NULL`；也可以显式使用 `where_null` / `where_not_null`。

## 6. 汇总目录并同步 schema

应用构建完成后，可以生成确定性的 API 目录：

```rust
let catalog = app.catalog()?;
```

`catalog()` 不只是导出文档：它完成跨模块路由校验，并为每个 CRUD Action 保存与具体 `TableDefinition` 对齐的 schema 和 `{module}:read` / `{module}:write` 权限。发生冲突或契约构建失败时，不应继续启动 HTTP adapter。

启动期由数据库初始化器汇总全部模块主表和附属表：

```rust
let report = initializer.sync_app_schema(&app).await?;
```

`sync_app_schema` 执行 additive 同步，只创建缺失表、列、主键和索引；已有类型、NULL、自增或主键不兼容时 fail-fast，不自动执行破坏性变更。

推荐启动顺序：

1. 读取应用配置。
2. 初始化 MySQL、Redis 等外部资源。
3. 注册插件并执行显式 migrations。
4. 构建 `AppRouter`。
5. 调用 `sync_app_schema(&app)`。
6. 从 `ApiCatalog` 构建 transport adapter。
7. 启动服务并注册优雅关闭钩子。

## 7. 主表与附属表

每个模块最多有一张驱动内置 CRUD 和 `ActionContext::table_query()` 的主表：

```rust
use yang_base::router::ModuleRouter;
use yang_base::BaseError;

fn user_module() -> Result<ModuleRouter, BaseError> {
    ModuleRouter::new("user", "用户管理")
        .table(users_table()?)
        .schema(user_sessions_table()?)
        .schema(user_audit_table()?)
        .crud()
}
```

`schema` 添加的定义只参与启动期汇总，不替换模块主表。

## 8. 下一步

- [表定义与字段能力](./table_config.md)
- [批量声明表字段](../examples/batch_field_config.md)
- [内置 CRUD Actions](../api/action_builtin.md)
- [项目结构与模块边界](../reference/project_structure.md)
