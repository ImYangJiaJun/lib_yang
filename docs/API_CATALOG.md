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
