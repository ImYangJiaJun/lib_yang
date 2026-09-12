# ApiCatalog 契约

`ApiCatalog` 是运行时私有路由注册表的只读快照，不参与 dispatch，也不依赖 `openapi` feature。
它把 `ActionMeta` 的 Schema、权限、公开性与独立 `RouteDescriptor` 合并，供 CLI、后台管理、
契约测试和后续 OpenAPI 投影复用。

```rust
use yang_base::router::{AppRouter, ModuleRouter, RouteDescriptor};

let users = ModuleRouter::new("users", "用户")
    .register_action(SearchAction)?
    .register_route(
        "search",
        RouteDescriptor::new("POST", "/users/search", "users.search")?
            .with_tags(vec!["users".to_string()])?,
    )?;
let catalog = AppRouter::new().register_module(users)?.catalog()?;
```

约束：

- 每个注册 Action 必须显式绑定一个 route，缺失时 catalog 构建失败。
- `(method, path)` 和 `operation_id` 在整个 AppRouter 内唯一；模块内和跨模块冲突都失败。
- method 规范化为大写；path 必须是无 query/fragment/空白的绝对路径。
- request/response content type 非空且不重复；成功状态码必须在 `100..=599`。
- `RouteDescriptor` 的公开字段在注册时会重新完整校验，构造后修改不能绕过约束。
- 模块按名称排序，Action 按名称排序；相同注册内容产生稳定的 catalog 顺序。
- `ActionMeta` 不保存 method/path，`RouteDescriptor` 是传输路由的唯一来源。

`ModuleRouter::descriptor()` 和 `AppRouter::catalog()` 返回 owned snapshot。调用方可以处理快照，
但无法借此修改运行时 Action、route 或 middleware 注册表。

## OpenAPI 3.1 投影

启用 `yang-base/openapi` feature 后，可从同一个 catalog 生成 OpenAPI JSON；关闭 feature 时投影
模块不参与编译，也不增加依赖。

```rust
use yang_base::router::OpenApiInfo;

let document = catalog.to_openapi(
    OpenApiInfo::new("YANG API", "1.0.0").with_description("服务 API"),
)?;
let json = serde_json::to_string_pretty(&document)?;
```

投影使用 OpenAPI `3.1.0`：input/output RootSchema 直接来自 ActionMeta 快照；成功响应按
`ApiResponse { code, message, data }` 包装；400/401/403/500 复用统一错误 Schema。私有 Action
声明 `bearerAuth`，公开 Action 显式声明空 security；权限和公开性同时写入 `x-permissions`、
`x-permission-mode`、`x-public`。投影阶段会重新验证 route 和 operation 唯一性，公开快照被修改
后也不会静默覆盖文档节点。
