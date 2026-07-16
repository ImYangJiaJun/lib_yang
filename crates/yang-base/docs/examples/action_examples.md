# 类型化 Action 与 API 注册示例

`yang-base` 0.2.0 的业务 Action 实现 `TypedHandler`，再通过 `#[derive(Action)]` 生成元数据。注册时使用单个 `Api` 同时绑定 handler 与 HTTP method/path，应用不再分别维护 Action 注册表和 route 注册表。

## 定义公开 Action

```rust
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use yang_base::action::{ActionContext, TypedHandler};
use yang_base::{Action, BaseError};

#[derive(Debug, Default, Deserialize, JsonSchema)]
struct HealthInput {}

#[derive(Debug, Serialize, JsonSchema)]
struct HealthOutput {
    status: &'static str,
}

#[derive(Action)]
#[action(
    name = "health",
    display_name = "健康检查",
    description = "返回进程状态",
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
        _input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Ok(HealthOutput { status: "ok" })
    }
}
```

没有 `public` 的 Action 默认受保护；可用 `permissions("user:read")` 声明额外业务权限。输入输出都应使用 serde + schemars 强类型 DTO，只有动态数据库记录使用 `Record`。

## 原子注册与鉴权范围

```rust
use yang_base::action::{TokenAuthMiddleware, User};
use yang_base::router::{Api, ModuleRouter};

let auth = TokenAuthMiddleware::new(|claims| {
    User::new(0, claims.sub.clone())
});

let module = ModuleRouter::new("system", "系统")
    .middleware(auth)
    .api(
        Api::get("/health", HealthAction)
            .operation_id("system.health")
            .tag("system"),
    )?;
```

`TokenAuthMiddleware` 固定使用 `MiddlewareScope::ProtectedActions`，所以它会跳过上例的公开健康检查，但会验证同一模块中的受保护 Action。普通中间件默认是 `AllActions`，日志、限流和请求追踪仍可覆盖两类端点。

`Api` 路由按 Axum 0.8 校验：路径参数使用 `{id}`，尾部通配使用 `{*path}`；旧式 `:id` / `*path` 和匹配冲突会在 transport 启动前返回配置错误。批量注册使用 `ModuleRouter::apis([Api::get(...), Api::post(...)])`。

标准表接口不需要逐个创建 Action：先用 `.table(definition)` 绑定主表，再调用 `.crud()`。它会自动注册六个 API、生成 `{module}:read` / `{module}:write` 权限，并把该表的主键、字段和查询约束写入 `ApiCatalog`。
