# yang-base 0.2.0 项目结构

`yang-base` 是应用侧后端基础库。0.2.0 的核心边界是 schema-first：应用声明表和字段，构建不可变定义，再由 Router、数据库初始化器与 CRUD 共享同一份契约。

## 核心数据流

```text
Table + Field
  -> Table::build()
  -> TableDefinition
  -> ModuleRouter::table(...).crud()
  -> AppRouter
     ├── table_definitions() -> DatabaseInitializer::sync_app_schema(...)
     └── catalog()           -> HTTP adapter / OpenAPI

Custom TypedHandler
  -> TypedAction / DynAction
  -> Api::{get, post, put, patch, delete}
  -> ModuleRouter::api(...)

Request
  -> ActionContext
  -> TableQuery
  -> Record / typed Action output
```

## 源码目录

```text
crates/yang-base/
├── src/
│   ├── action/          # TypedHandler、类型擦除层、上下文与内置 CRUD
│   ├── database/        # MySQL/Redis 全局访问、初始化与 schema 同步
│   ├── error/           # BaseError 与稳定错误码
│   ├── http/            # 可选 HTTP 客户端
│   ├── plugin/          # 插件注册、依赖和生命周期
│   ├── router/          # Api、ModuleRouter、AppRouter、ApiCatalog
│   ├── table/           # Table、Field、TableDefinition、Record、TableQuery
│   ├── token/           # 可选 JWT 与 Redis 撤销列表
│   ├── admin.rs         # 可选后台展示元数据
│   ├── config.rs        # 全局配置
│   ├── lifecycle.rs     # 应用生命周期钩子
│   ├── observability.rs # 慢查询等可观测性配置
│   └── lib.rs           # 模块导出与 feature 边界
├── tests/               # 集成测试和 feature 契约测试
├── examples/            # 可运行示例
└── docs/                # API、指南、示例与参考文档
```

## 模块职责

### table

公开应用模型：

- `Table`：表声明 builder。
- `Field`：存储类型、校验、权限、索引和关系元数据 builder。
- `TableDefinition`：`build()` 生成的不可变运行时契约。
- `Record`：内置 CRUD 和动态查询使用的透明 JSON object。
- `TableHandle` / `TableQuery`：绑定数据库后的权限感知查询入口。

内部归一化结构只服务于 schema 校验、同步与查询执行，不是应用声明入口。

### action

自定义 Action 采用三层结构：

1. 应用实现 `TypedHandler`，固定 `Input` 和 `Output`。
2. `#[derive(Action)]` 生成 `TypedAction` 元数据和桥接实现。
3. Router 以 `DynAction` 保存异构 Action，并在 dispatch 时恢复类型化输入输出。

`ActionContext` 提供认证用户、`RequestMeta`、全局工具和当前模块的 `TableDefinition` / `TableQuery`。

### router

- `Api`：把 Action、HTTP method、path、operation id、状态码和标签绑定为一个值。
- `ModuleRouter`：拥有模块主表、附属 schema、Action、API 路由、中间件和默认权限。
- `AppRouter`：汇总模块、应用级 dispatch 与全部表定义。
- `ApiCatalog`：确定性的 Action/API 描述源；内置 CRUD 的 schema 与权限绑定具体主表，可选投影 OpenAPI 3.1。

主表使用 `ModuleRouter::table`；仅参与启动期 schema 汇总的附属表使用 `ModuleRouter::schema`。启用 `mysql` 后，`.crud()` 一次注册六个标准 API，并自动为写接口配置 `{module}:write`、为读接口配置 `{module}:read`。

公开与受保护 Action 可以位于同一 `ModuleRouter`。`TokenAuthMiddleware` 只应用于 `MiddlewareScope::ProtectedActions`，普通中间件默认覆盖全部 Action。路由在模块注册和应用 Catalog 构建期按 Axum 0.8 的 `{name}` / `{*name}` 模板语法校验，transport adapter 不再承担发现非法模板或冲突的职责。

### database

- `GlobalDatabase` / `GlobalRedis`：进程级数据库客户端入口。
- `DatabaseInitializer`：插件 migration、schema 验证和应用表 additive 同步。
- `SchemaValidationReport`：记录兼容、缺失和破坏性差异。

应用启动时应先完成底层数据库与插件初始化，再调用 `sync_app_schema(&app)`。同步器只创建缺失对象；不兼容差异会 fail-fast，不自动执行破坏性变更。

### plugin、token、http

- `plugin` 负责构建期注册、依赖排序、配置 schema 和生命周期。
- `token` 负责 JWT 签发、验证、刷新和可选撤销检查。
- `http` 提供带超时、重试和熔断的出站请求客户端。

这些模块与 Action/Table 主链路解耦，通过 feature 按需启用。

## 推荐应用目录

```text
src/
├── tables/
│   ├── mod.rs
│   └── users.rs       # fn users_table() -> Result<TableDefinition>
├── actions/
│   ├── mod.rs
│   └── profile.rs     # TypedHandler + #[derive(Action)]
├── router.rs          # ModuleRouter + Api + AppRouter
├── bootstrap.rs       # database/plugin init + sync_app_schema
└── main.rs            # transport adapter and lifecycle
```

职责边界：

- `tables` 只声明 schema，不持有连接池。
- `actions` 只实现类型化业务输入输出，通过 `ActionContext` 访问能力。
- `router` 原子绑定 Action 与 HTTP 元数据，并组合标准 CRUD。
- `bootstrap` 汇总外部资源初始化与 schema 同步顺序。
- transport adapter 只负责协议转换和构造 `RequestMeta`。

`TableQuery` 是用户可控查询的类型与授权边界：WHERE 字段、操作符和每个值都要匹配 `TableDefinition`，与 `null` 的 `Eq` / `Ne` 比较规范化为 `IS NULL` / `IS NOT NULL`。

## Feature 边界

| Feature | 主要能力 |
|---|---|
| `mysql` | 表绑定、`TableQuery` 执行、内置 CRUD、schema 同步 |
| `redis` | `GlobalRedis` 与 Redis 数据结构 API |
| `token` | JWT 与撤销列表；自动启用 `redis` |
| `http` | 出站 HTTP 客户端 |
| `validator` | 严格 Email、Phone、Regex 校验 |
| `plugin-schema` | 插件配置 JSON Schema 校验 |
| `openapi` | `ApiCatalog` 到 OpenAPI 3.1 |
| `admin-metadata` | 对核心稳定 ID 的只读展示引用 |

## 验证位置

- 模块单元测试：`src/**/__tests__/`
- crate 集成测试：`tests/`
- feature 组合：`tests/feature_contract.rs`
- 发布文档契约：`tests/release_docs_contract.rs`
- 示例编译：`cargo check -p yang-base --examples --all-features`

常用验证：

```bash
cargo fmt --check
cargo test --lib -p yang-base
cargo test -p yang-base --test release_docs_contract
cargo check -p yang-base --examples --all-features
cargo clippy -p yang-base --all-targets --all-features -- -D warnings
```

## 继续阅读

- [快速开始](../guides/quick_start.md)
- [表定义指南](../guides/table_config.md)
- [内置 CRUD Actions](../api/action_builtin.md)
- [批量声明表字段](../examples/batch_field_config.md)
