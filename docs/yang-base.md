# yang-base — 后端基础库公共 API

版本：0.2.0 | 许可：MIT OR Apache-2.0

`yang-base` 提供 schema-first 数据表、类型化 Action、原子 API 注册、应用级路由目录、插件生命周期、MySQL/Redis 初始化、HTTP 客户端和 JWT Token 管理。

精确签名以 rustdoc 与源码为准；本文件描述 0.2.0 应用侧公共契约，不保留已删除 API 的历史教程。

## 安装与 feature

```toml
[dependencies]
yang-base = "0.2.0"
```

| Feature | 默认 | 能力 |
|---|---:|---|
| `mysql` | 是 | MySQL 全局访问、`TableHandle`、`TableQuery`、内置 CRUD 与 schema 同步 |
| `redis` | 是 | `GlobalRedis` 与 Redis 数据结构 API |
| `token` | 是 | JWT 签发、刷新、Redis 撤销列表；自动启用 `redis` |
| `http` | 是 | 带超时、重试和熔断的 HTTP 客户端 |
| `validator` | 是 | Email、Phone、Regex 严格校验 |
| `plugin-schema` | 是 | 插件配置 JSON Schema 校验 |
| `metrics` | 否 | Action 指标门面，不绑定 exporter |
| `openapi` | 否 | 从 `ApiCatalog` 投影 OpenAPI 3.1 JSON |
| `admin-metadata` | 否 | 后台展示元数据，不改变 dispatch |

`default-features = false` 保留插件、Action、Router 与表定义核心，不引入数据库或网络驱动。

## 核心模型

```text
Table + Field
  -> TableDefinition
  -> ModuleRouter::table / ModuleRouter::schema
  -> ModuleRouter::crud / ModuleRouter::api(Api)
  -> AppRouter
  -> ApiCatalog / OpenAPI 3.1

TableDefinition + MySQL pool
  -> TableHandle
  -> TableQuery
  -> Record
```

- `Table` / `Field` 是应用侧 schema 构建入口。
- `TableDefinition` 是校验后的不可变运行时定义。
- `Record` 是内置 CRUD 和动态查询统一使用的透明 JSON object。
- `Api` 把 Action 与 method/path/operation id/status/tags 放在同一注册值中。
- `RequestMeta` 保存 transport-neutral 的 method、URI、地址和白名单 headers。
- `ApiCatalog` 是模块、Action schema、授权要求与路由的确定性只读快照；内置 CRUD 的契约绑定具体 `TableDefinition`。

## Table：schema-first 表定义

```rust
use yang_base::table::{col, Field, Table, TableDefinition};

fn users_table() -> Result<TableDefinition, yang_base::BaseError> {
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
            Field::enumeration("status", ["active", "disabled"])
                .default("active")
                .filterable(),
            Field::created_at("created_at"),
            Field::updated_at("updated_at"),
            Field::soft_delete("deleted_at"),
        ])
        .default_order(col("created_at").desc())
        .build()
}
```

`Table::build()` 一次检查标识符、主键、重复字段、字段形态、默认值、索引引用、时间戳角色、权限范围与默认排序，然后生成 `TableDefinition`。

### Field 构造器

- 标量：`string`、`integer`、`bigint`、`float`、`double`、`boolean`
- 时间：`date`、`datetime`、`timestamp`
- 复杂值：`json`、`text`、`enumeration`
- 生成列：`id`、`created_at`、`updated_at`、`soft_delete`

常用修饰器：

- 结构：`label`、`required`、`nullable`、`default`、`primary_key`、`auto_increment`
- 校验：`length`、`min_length`、`max_length`、`min`、`max`、`email`、`phone`、`url`、`regex`
- 索引：字段级 `unique`、`unique_named`、`index`、`index_named`；表级 `unique`、`unique_named`、`index`、`index_named`
- 权限：`readable_by`、`writable_by`、`filterable_by`、`sortable_by`、`secret`
- 关系：`relation`、`relation_display_fields`

存储类型与关系元数据正交。普通外键列先按真实数据库类型构造，再附加关系，例如 `Field::bigint("user_id").relation("users", "id", RelationType::ManyToOne)`。

### TableDefinition

只读定义提供：

- `name()`、`label()`、`primary_key()`、`field_count()`
- `field(name)`、`fields()` 返回 `FieldMetadata`
- `input_schema()`、`output_schema()`
- `soft_delete_field()`
- `validate_schema(columns) -> SchemaValidationReport`
- `bind(Arc<MySqlPool>) -> TableHandle`（`mysql` feature）

定义不可原地修改。schema 变化应修改声明代码、重新构建应用，并交由启动期同步器处理允许的 additive 变更。

## Record 与 TableQuery

`Record` 序列化为普通 JSON object：

```rust
use yang_base::table::Record;

let mut row = Record::new()
    .set("username", "alice")
    .set("email", "alice@example.com");
row.insert("active", true);

let username: String = row.require("username")?;
let nickname: Option<String> = row.optional("nickname")?;
let json_object = row.into_map();
```

公共方法包括 `new`、`set`、`insert`、`get`、`require`、`optional`、`as_map` 与 `into_map`。SQL 行解码、内置 CRUD 输入输出和 `TableQuery` 终端方法都使用同一个类型。

启用 `mysql` 后，通过 `TableHandle::query(roles)` 创建受保护查询：

```rust
use std::sync::Arc;
use yang_base::table::Record;

let handle = users.bind(Arc::new(pool.clone()));

let rows: Vec<Record> = handle
    .query(["admin"])
    .where_eq("status", "active")?
    .order_by("created_at", yang_base::table::SortOrder::Desc)?
    .all()
    .await?;

let affected = handle
    .query(["admin"])
    .insert(Record::new().set("username", "alice"))
    .await?;
```

主要终端方法：

- 读取：`all`、`optional`、`one`、`paginate_records`
- 写入：`insert`、`insert_returning_id`、`update`、`delete`
- 事务：对应的 `*_in_tx` 入口

字段选择、筛选、排序和写入在 SQL 执行前经过字段存在性、类型与角色权限校验。`ActionContext::table_query()` 还会注入当前用户角色、request id、慢查询阈值与全局 MySQL pool。

WHERE 值不会作为无类型 JSON 直接进入 SQL。`TableQuery` 按字段存储类型校验每个叶子条件，`IN` 与 `BETWEEN` 也逐项验证；无效的日期时间、超出 `i64` 的 `BigInt`、非对象/数组的 `Json` 等都会在访问数据库前失败。`where_eq(field, Value::Null)` 与 `where_ne(field, Value::Null)` 分别规范化为 `IS NULL` 和 `IS NOT NULL`，等价的显式入口是 `where_null` 与 `where_not_null`。

## Action：类型化业务操作

Action 分为三层：

1. 应用实现 `TypedHandler`，声明 `Input` / `Output`。
2. `#[derive(Action)]` 生成 `TypedAction` 和静态 `ActionMeta`。
3. blanket impl 自动生成 object-safe `DynAction`，供 Router 擦除存储。

```rust
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use yang_base::action::{ActionContext, TypedHandler};
use yang_base::{Action, BaseError};

#[derive(Deserialize, JsonSchema, Default)]
struct HealthInput {}

#[derive(Serialize, JsonSchema)]
struct HealthOutput {
    status: &'static str,
}

#[derive(Action)]
#[action(
    name = "health",
    display_name = "健康检查",
    description = "返回进程健康状态",
    public
)]
struct HealthAction;

#[async_trait]
impl TypedHandler for HealthAction {
    type Input = HealthInput;
    type Output = HealthOutput;

    async fn handle(
        &self,
        _ctx: ActionContext,
        _input: HealthInput,
    ) -> Result<HealthOutput, BaseError> {
        Ok(HealthOutput { status: "ok" })
    }
}
```

自定义 DTO 应保持强类型；只有确实动态的表行使用 `Record`。

## Api 与 Router

`Api` 是 Action 与传输元数据的原子注册单元：

```rust
use yang_base::router::{Api, AppRouter, ModuleRouter};

let system_module = ModuleRouter::new("system", "系统")
    .api(
        Api::get("/health", HealthAction)
            .operation_id("system.health")
            .tag("system"),
    )?;

let user_module = ModuleRouter::new("user", "用户管理")
    .table(users_table()?)
    .crud()?;

let app = AppRouter::new().modules([system_module, user_module])?;
```

`Api::{get,post,put,patch,delete}` 支持链式设置：

- `operation_id`
- `status` / `created`
- `tag` / `tags`
- `request_content_types` / `response_content_types`

`ModuleRouter::api` 和 `apis` 统一检查 Action 名、权限、method/path、operation id 及模块内冲突。路由模板按 Axum 0.8 校验：动态段使用 `{id}`，尾部通配使用 `{*path}`；`:id` / `*path` 在注册期直接失败。相同 path 的不同 HTTP method 可以共存，但匹配集合相同的模板会冲突。`AppRouter::module` / `modules` 检查模块名，`AppRouter::catalog()` 在 transport 构建前完成跨模块 route 与 operation id 校验。

### 标准 CRUD

主表通过 `ModuleRouter::table(TableDefinition)` 绑定；只参与启动期 schema 汇总的附属表使用 `ModuleRouter::schema(TableDefinition)`。随后 `.crud()` 注册六个非泛型内置 Action：

| Action | Method | Path | 输入 | 输出 |
|---|---|---|---|---|
| `add` | POST | `/api/{module}` | `Record` | `InsertResult` |
| `put` | PUT | `/api/{module}` | `{ "id": Value, "data": Record }` | `AffectedResult` |
| `del` | DELETE | `/api/{module}` | `{ "id": Value }` | `AffectedResult` |
| `get` | GET | `/api/{module}` | `{ "id": Value }` | `Record` |
| `select` | POST | `/api/{module}/query` | `SelectQuery` | `SelectResult { items: Vec<Record>, ... }` |
| `table` | GET | `/api/{module}/schema` | `{}` | `TableSchemaResponse` |

调用 `.crud()` 前必须绑定主表，否则返回 `BaseError::TableDefinitionNotSet`（700009）。

`.crud()` 还会按模块名生成最小权限：`add`、`put`、`del` 需要 `{module}:write`，`get`、`select`、`table` 需要 `{module}:read`。这些权限同时用于 runtime dispatch 和 Catalog。Catalog 的输入/输出 JSON Schema 来自主表定义：主键使用真实字段类型，记录只暴露允许读写的字段，查询字段枚举只包含允许筛选/排序且非 secret 的字段。

### RequestMeta 与 dispatch

`RequestMeta` 是独立的 transport-neutral sidecar，保存 method、original URI、scheme、peer/local address、白名单 headers 和可选 extensions。Web 框架适配层应在构造 `ActionContext` 时填充它；Action 不直接依赖 Axum 等具体框架。

`ModuleRouter::dispatch` 的顺序是：中间件链 → Action/模块权限检查 → 类型化输入反序列化 → handler → `ApiResponse`。`AppRouter::dispatch(module, action, context)` 提供应用级两段式分发。

Action 默认受保护，`#[action(..., public)]` 显式标记公开端点。公开与受保护 Action 可以共存于同一个 `ModuleRouter`：普通中间件默认使用 `MiddlewareScope::AllActions`，而 `TokenAuthMiddleware` 固定使用 `MiddlewareScope::ProtectedActions`，所以登录、注册、刷新等公开端点跳过强制 Bearer 认证，受保护端点仍完成 Token 校验和 `User` 注入。

### ApiCatalog 与 OpenAPI

```rust
let catalog = app.catalog()?;
```

`ApiCatalog` 按模块和 Action 名确定性排序，包含 `ActionDescriptor`、输入/输出 JSON Schema、权限与唯一 `RouteDescriptor`。它是 HTTP 适配、文档和后台展示引用 operation id 的单一事实源；构建失败意味着 transport 不应启动。

启用 `openapi` 后：

```rust
use yang_base::router::OpenApiInfo;

let document = catalog.to_openapi(
    OpenApiInfo::new("My API", "1.0.0")
        .with_description("服务 API"),
)?;
```

返回确定性的 OpenAPI 3.1 JSON，并保留成功状态码、content type、权限和公开性元数据。

## Database 与 schema 治理

### 全局访问

- `GlobalDatabase`：MySQL `OnceLock` 单例、查询、执行、事务与健康检查。
- `GlobalRedis`：Redis `OnceLock` 单例及 String/Hash/List/Set/ZSet 等操作。
- `DatabaseBundle`：按 MySQL → Redis 顺序统一初始化；任一步失败应中止启动。

直接使用初始化配置时，应用还需依赖 `yang-db` 0.1.4 的 `DatabaseConfig` / `RedisConfig`。

### DatabaseInitializer

`DatabaseInitializer` 负责：

- 插件初始化 SQL 与 migration checksum
- migration plan / dry-run / 并发占位 / 漂移检测
- `validate_table_definition(&TableDefinition) -> SchemaValidationReport`
- `sync_app_schema(&AppRouter)` 启动期 schema 同步

推荐启动顺序：

```rust
let initializer = DatabaseInitializer::new(database, true);
initializer.initialize_all(&plugin_manager).await?;

let report = initializer.sync_app_schema(&app).await?;
```

`sync_app_schema` 从应用定义汇总数据表，在数据库级 advisory lock 下执行保数据演进：创建缺失结构，支持显式列改名、受控列修改、唯一索引、CHECK 与外键。可能受旧数据影响的变更先只读预检并报告表、对象和主键；任何问题都会阻止整批 DDL。未知列、索引和约束不会被自动删除。

## Plugin

- `Plugin`：名称、版本、依赖、配置 schema、初始化 SQL、迁移声明和生命周期回调。
- `PluginManagerBuilder`：构建期注册并检查缺失/循环依赖。
- `PluginRegistry`：构建完成后的确定性只读注册表。
- `PluginManager`：需要运行时动态注册时使用的有锁版本。

应用表定义是当前 schema-first 结构契约；版本化迁移 API 仅为旧插件兼容入口，新应用应优先使用声明式预检与同步。

## Token（`token` feature）

`TokenManager` 支持对称/非对称 JWT、Access/Refresh Token、issuer/audience/algorithm 校验、刷新轮换与 Redis 撤销列表。

- `verify_token`：验证签名、时间、issuer、audience；不查询撤销列表。
- `verify_token_checked`：在标准验证后检查 token blacklist 与 subject 水位线；需要登出/强制下线语义时使用。
- `LoginAction`、`RefreshAction`、`LogoutAction`：内置公开认证 Action；业务方注入凭证或 claims 解析策略。

## HTTP（`http` feature）

`HttpClient` / `RequestBuilder` 支持 GET/POST/PUT/PATCH/DELETE、header、query、JSON、form、text、Bearer token、每请求 timeout、指数退避重试和按 host 隔离的熔断器。

客户端实例经 `ToolsBuilder::http(...)` 注册进应用资源，运行期通过 `Tools::http()` 或 `ActionContext::http()` 获取；自定义客户端使用 `HttpClient::with_config(HttpClientConfig)`。

## 错误与可观测性

`BaseError` 是 `#[non_exhaustive]` 的统一错误类型，提供：

- `code()` / `code_str()` 稳定错误码
- 错误分类与 `source()` 链
- 数据库、Redis、HTTP、Token、字段权限、Action、配置和 I/O 错误

Action dispatch 使用 tracing span；启用 `metrics` 后记录请求计数、错误码和 handler duration。`TableQuery` 携带 request id 与慢查询阈值，面向请求的查询不要绕过它直接访问原始 pool。

## 可选后台元数据（`admin-metadata` feature）

`AdminMetadataRegistry` 只保存稳定 ID、展示类型与对 Action/Table/API operation 的引用。它不持有 handler，不改变权限或 dispatch，也不引入额外依赖。

## 验证命令

```bash
cargo test --lib -p yang-base
cargo test -p yang-base --test release_docs_contract
cargo check -p yang-base --examples --all-features
cargo clippy -p yang-base --all-targets --all-features -- -D warnings
```

需要真实 MySQL/Redis 的 ignored 测试应在容器就绪后单线程运行。
