# yang-base

`yang-base` 0.2.0 是 YANG 后端基础库，提供构建期定义内核（Addon/Module/Action/fields/params → 冻结 Catalog + Registry）、类型化 Action、显式资源所有权（`Tools`）、schema-first 数据表、HTTP 客户端、JWT Token 管理和可选 Axum HTTP 传输。

当前应用侧的核心链路是：

```text
fields! / params! / #[derive(Action)]
  -> AppBuilder::build(tools)
  -> BuiltApp（DefinitionCatalog + Registry + TableDefinition）
  -> ActionContext + TableQuery
```

应用定义在构建期一次性校验冻结，请求期只剩 slot 派发、类型化反序列化与受控 SQL。

## Features

| Feature | 默认 | 能力 |
|---|---:|---|
| `mysql` | 是 | `Tools` MySQL 资源槽、schema 同步、`TableHandle`、`TableQuery` 与内置 CRUD |
| `redis` | 是 | `Tools` Redis 资源槽与 Redis 数据结构 API |
| `token` | 是 | JWT 签发、刷新与 Redis 撤销列表；自动启用 `redis` |
| `http` | 是 | 带超时、重试和熔断的 HTTP 客户端（`Tools` http 槽） |
| `validator` | 是 | Email、Phone、Regex 严格验证 |
| `plugin-schema` | 是 | 插件配置 JSON Schema 验证 |
| `metrics` | 否 | Action 指标门面，不绑定 exporter |
| `openapi` | 否 | 从 `DefinitionCatalog` 投影 OpenAPI 3.1 JSON |
| `admin-metadata` | 否 | 后台展示元数据；不改变 dispatch |
| `transport-axum` | 否 | Axum 0.8 HTTP 传输适配器（CORS/超时/压缩/健康端点/文件与重定向响应） |

`default-features = false` 保留插件、definition 内核、Action 和表定义核心，不引入数据库或网络驱动。

## 安装

发布版本：

```toml
[dependencies]
yang-base = "0.2.0"
```

同一 workspace 联调：

```toml
[dependencies]
yang-base = { path = "../yang-base" }
```

按需选择 feature：

```toml
# 仅核心模型
yang-base = { version = "0.2.0", default-features = false }

# 核心模型 + MySQL
yang-base = { version = "0.2.0", default-features = false, features = ["mysql"] }

# 核心模型 + Redis
yang-base = { version = "0.2.0", default-features = false, features = ["redis"] }
```

## 快速开始：定义表并注册 CRUD

```rust
use yang_base::router::{AppRouter, ModuleRouter};
use yang_base::table::{col, Field, Table};
use yang_base::BaseError;

fn build_router() -> Result<AppRouter, BaseError> {
    let users = Table::new("users")
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
            Field::created_at("created_at"),
            Field::updated_at("updated_at"),
            Field::soft_delete("deleted_at"),
        ])
        .default_order(col("created_at").desc())
        .build()?;

    let user_module = ModuleRouter::new("user", "用户管理")
        .table(users)
        .crud()?;

    AppRouter::new().module(user_module)
}
```

`.crud()` 原子注册六个标准 API：

| Action | Method | Path | 用途 |
|---|---|---|---|
| `add` | `POST` | `/api/user` | 插入一条 `Record` |
| `put` | `PUT` | `/api/user` | 按主键更新 `data` 中的字段 |
| `del` | `DELETE` | `/api/user` | 按主键删除或软删除 |
| `get` | `GET` | `/api/user` | 按主键读取一条 `Record` |
| `select` | `POST` | `/api/user/query` | where 树、排序与分页查询 |
| `table` | `GET` | `/api/user/schema` | 返回输入/输出 JSON Schema |

`.crud()` 同时生成运行时授权契约：`add`、`put`、`del` 需要 `user:write`，`get`、`select`、`table` 需要 `user:read`；把模块名替换为实际的 `module_name` 即得到其它模块的权限名。六个 Action 的 Catalog schema 不是通用 `Record` 占位符，而是从当前主表的 `TableDefinition` 生成，包含真实主键类型、可读/可写字段，以及允许筛选和排序的字段枚举。

`ModuleRouter::table` 绑定模块主表；只参与启动期 schema 汇总的附属表使用 `ModuleRouter::schema`。

## 自定义 API

自定义 Action 实现 `TypedHandler` 并派生 `Action`。注册时用 `Api` 将 handler、HTTP method、path、operation id、状态码和标签放在同一个值中：

```rust
use yang_base::action::TypedAction;
use yang_base::router::{Api, ModuleRouter};
use yang_base::BaseError;

fn build_system_module(
    health_action: impl TypedAction,
) -> Result<ModuleRouter, BaseError> {
    ModuleRouter::new("system", "系统").api(
        Api::get("/health", health_action)
            .operation_id("system.health")
            .tag("system"),
    )
}
```

多个端点使用 `ModuleRouter::apis`。`Api::{get,post,put,patch,delete}` 会先把具体 Action 擦除成统一注册值，因此数组或 `Vec<Api>` 可以容纳不同的 Action 类型。

公开与受保护 Action 可以注册在同一个 `ModuleRouter`。Action 默认受保护，只有带 `#[action(..., public)]` 的端点跳过认证；`TokenAuthMiddleware` 的 scope 固定为 `ProtectedActions`，因此登录、注册等公开端点不会被强制要求 Bearer Token，日志、限流等普通中间件仍默认覆盖全部端点。

`ModuleRouter::api` / `apis` 会立即校验路由；`AppRouter::catalog()` 再校验跨模块冲突。动态段必须使用 Axum 0.8 的 `/users/{id}` 与 `/files/{*path}` 语法，旧式 `:id` / `*path` 会在 transport 启动前返回配置错误。同一路径可以注册不同 HTTP method，但匹配集合相同的模板会被视为冲突。

## 动态行：Record

`Record` 是内置 CRUD 与动态查询统一使用的行对象，序列化后就是普通 JSON object：

```rust
use yang_base::table::Record;

let mut row = Record::new()
    .set("username", "alice")
    .set("email", "alice@example.com");
row.insert("active", true);

let username: String = row.require("username").expect("username 必须是字符串");
let nickname: Option<String> = row.optional("nickname").expect("nickname 类型应有效");
```

- `set`：链式写入字段。
- `insert`：原地写入并返回旧值。
- `require::<T>`：必需字段的类型化读取。
- `optional::<T>`：缺失或 `null` 返回 `None`。
- `as_map` / `into_map`：以只读引用或所有权形式访问底层 JSON 字段映射。

## 表字段能力

常用 `Field` 构造器：

- `id`、`string`、`integer`、`bigint`、`float`、`double`、`boolean`
- `date`、`datetime`、`timestamp`
- `json`、`text`、`enumeration`
- `created_at`、`updated_at`、`soft_delete`

常用修饰器：

- 结构：`label`、`required`、`nullable`、`default`、`primary_key`、`auto_increment`
- 校验：`length`、`min_length`、`max_length`、`min`、`max`、`email`、`phone`、`url`、`regex`
- 索引：`unique`、`unique_named`、`index`、`index_named`
- 权限：`readable_by`、`writable_by`、`filterable_by`、`sortable_by`、`secret`
- 关联：`relation`、`relation_display_fields`

字段存储类型与关系元数据正交。普通外键列先按真实数据库类型构造，再附加关系，例如 `Field::bigint("user_id").relation("users", "id", RelationType::ManyToOne)`。

`Table::build()` 是集中校验边界：它检查表/字段标识符、主键、重复字段、索引引用、时间戳角色、默认排序和字段形态，然后生成不可变 `TableDefinition`。

完整示例见 [表定义指南](docs/guides/table_config.md) 和 [batch_field_config.rs](examples/batch_field_config.rs)。

## 数据库与 Redis

启用默认 feature 后，MySQL/Redis 客户端经 `ToolsBuilder` 注册进应用资源并由每个 `BuiltApp` 显式持有（无进程级全局单例）。初始化配置类型来自 `yang-db`；直接构造这些客户端的应用还需声明匹配的依赖：

```toml
yang-db = { version = "0.1.4", default-features = false, features = ["mysql", "redis"] }
```

初始化示例：

```rust
use std::sync::Arc;
use yang_base::tools::ToolsBuilder;
use yang_base::BaseError;
use yang_db::{redis::RedisConfig, Database, DatabaseConfig, RedisClient};

async fn build_tools() -> Result<Arc<yang_base::tools::Tools>, BaseError> {
    let database = Database::connect_with_config(
        "mysql://root:password@localhost/mydb",
        DatabaseConfig::default(),
    )
    .await?;
    let cache = RedisClient::connect_with_config(
        "redis://127.0.0.1:6379",
        RedisConfig::default(),
    )
    .await?;

    let tools = ToolsBuilder::new()
        .mysql(database)
        .cache(cache)
        .build()?;
    Ok(Arc::new(tools))
}
```

面向请求的单表访问优先走 `ActionContext::table_query()`；它携带字段权限、软删除、慢查询阈值和 request id。`Tools::db()` 返回的是不受保护的连接池，只适合初始化、系统任务和明确承担授权责任的底层操作。

`TableQuery` 会在 SQL 生成前按 `TableDefinition` 校验 WHERE 字段、筛选权限、操作符和值类型；`IN` 与 `BETWEEN` 的每个值都逐项校验。`where_eq(field, null)` / `where_ne(field, null)` 分别生成 `IS NULL` / `IS NOT NULL`，也可以显式调用 `where_null` / `where_not_null`，不会生成语义错误的 `= NULL` 或 `!= NULL`。

## 目录

```text
src/
├── action/       # Action(业务 trait) / TypedHandler / TypedAction / DynAction 与 builtin CRUD
├── definition/   # AppBuilder / BuiltApp / Catalog / Registry / fields! / params!
├── tools.rs      # ToolsBuilder / Tools（db/cache/token/http + 类型化扩展）
├── database/     # DatabaseInitializer（迁移治理与启动期 schema 同步）
├── router/       # 洋葱中间件（RequestId / 鉴权）
├── table/        # Table、Field、TableDefinition、Record、TableQuery
├── plugin/       # 旧代插件生命周期与依赖管理
├── transport/    # 可选 Axum 0.8 HTTP 适配器（transport-axum）
├── http/         # 可选 HTTP 客户端
├── token/        # 可选 JWT 与撤销列表
└── error/        # BaseError
```

## 验证

```bash
cargo fmt --check
cargo test --lib -p yang-base
cargo check -p yang-base --examples --all-features
cargo clippy -p yang-base --all-targets --all-features -- -D warnings
```

需要真实 MySQL/Redis 的 ignored 测试应在容器就绪后单线程运行。

## 许可证

MIT OR Apache-2.0
