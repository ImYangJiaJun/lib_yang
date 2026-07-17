# yang-base — Backend Services Library

**Parent:** lib_yang workspace

## OVERVIEW
Application-facing backend primitives built on `yang-db`: build-time definition kernel (Addon/Module/Action/fields/params → frozen Catalog + Registry), typed action dispatch, table schemas/querying, explicit resource ownership (`Tools`), optional JWT tokens, optional Axum HTTP transport, and unified errors.

## STRUCTURE
```text
yang-base/
├── src/
│   ├── lib.rs           # 13 public modules; feature-gated http/token/transport-axum 等
│   ├── config.rs        # EngineConfig（YANG_* 环境变量）
│   ├── lifecycle.rs     # 优雅停机（插件 → Tools 逆序）
│   ├── observability.rs # ObservabilityConfig 纯数据（经 Tools config 槽注册；无全局单例）
│   ├── tools.rs         # ToolsBuilder/Tools：mysql/cache/token/http + 类型化 extension/config
│   ├── database/        # DatabaseInitializer（迁移治理 + additive schema 同步）
│   ├── definition/      # 定义内核：AppBuilder/BuiltApp/Catalog/Registry/Spec/字段与参数定义
│   ├── plugin/          # 旧代插件生命周期（新链路以 definition Addon 组织业务）
│   ├── action/          # child AGENTS.md: Action(业务 trait)/TypedHandler/TypedAction/DynAction
│   ├── table/           # child AGENTS.md: Table/Field/TableDefinition/Record/TableQuery
│   ├── router/          # 洋葱中间件（RequestId/authz 等）
│   ├── transport/       # axum.rs：Axum 0.8 HTTP 适配器，`transport-axum` feature
│   ├── http/            # reqwest wrapper，`http` feature（经 Tools http 槽获取）
│   ├── token/           # JWT TokenManager + revocation，`token` feature
│   └── error/           # BaseError with numeric codes
├── tests/               # integration tests, Docker/manual where ignored
├── docs/                # api/guides/reference/examples（部分 Global* 描述已过时，以代码为准）
└── examples/            # database + field config examples
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| 资源组装 | `src/tools.rs` | `ToolsBuilder` 仅启动期可变 → `Tools` 冻结只读；重复注册构建期报错；health_check/幂等 close |
| 定义与组装应用 | `src/definition/builder.rs` | `AppBuilder::build` 全量交叉校验 → `BuiltApp`（catalog/registry/tools/table_definitions/compiled_views） |
| Action 定义 | `src/action/typed.rs` + `#[derive(Action)]` | 业务实现 `Action::index`；宏生成 `TypedAction`；blanket 桥接 `DynAction` |
| 输入声明 | `yang_base::params!` / `src/definition/param.rs` | body/query/path/header 合并一次反序列化 |
| 文件/重定向响应 | `src/action/response.rs` | `ResponseBody::download/preview/redirect` → `ApiResponse::attachment`（JSON 线格式不变） |
| HTTP 服务 | `src/transport/axum.rs` | `router()`/`serve()` + `AxumTransportConfig`/`CorsConfig`；CORS 白名单、超时、压缩、`/health/live|ready`、x-request-id 透传 |
| DB/Redis bootstrap | `src/database/initializer.rs` | initialization/migration ordering |
| Plugin management | `src/plugin/mod.rs` | 旧代 `Plugin`/`PluginManager`/`PluginRegistry`（新链路不用于业务组织） |
| Table definitions | `src/table/definition.rs` | public schema-first `Table` / `Field` builders and immutable `TableDefinition` |
| HTTP client | `src/http/` | `ToolsBuilder::http(...)` 注册，`Tools::http()`/`ctx.http()?` 获取；无全局单例 |
| Token generation | `src/token/manager.rs` | feature-gated JWT generation/validation/refresh |
| Error handling | `src/error/mod.rs` | `BaseError` variants and codes |

## CODE MAP
| Symbol | Location | Role |
|--------|----------|------|
| `AppBuilder` / `BuiltApp` | `src/definition/builder.rs` | 构建期组装/校验 → 冻结运行时 |
| `Registry` / `ActionLink` | `src/definition/builder.rs`, `src/definition/plugins.rs` | slot 预解析 dispatch；强类型内部调用 |
| `ToolsBuilder` / `Tools` | `src/tools.rs` | 应用资源显式所有权（替代已删除的 GlobalDatabase/GlobalRedis） |
| `Action`（业务 trait） | `src/action/typed.rs` | 用户唯一手写接口：Input/Output + `index` |
| `TypedHandler` / `TypedAction` / `DynAction` | `src/action/typed.rs` | 三层桥接与擦除 dispatch |
| `ApiResponse` / `ResponseBody` | `src/action/response.rs` | 统一响应 + 文件/预览/重定向附件 |
| `ActionContext` | `src/action/context.rs` | request/user/tools/table context；`ctx.http()?` 等快捷入口 |
| `Table` / `Field` | `src/table/definition.rs` | application-facing schema builders |
| `TableDefinition` | `src/table/definition.rs` | immutable runtime schema, permissions and JSON Schema source |
| `TableQuery` | `src/table/table_query.rs` | table-aware SQL query builder/executor |
| `TokenManager` | `src/token/manager.rs` | JWT encode/decode/refresh |
| `HttpClient` | `src/http/client.rs` | reqwest client wrapper（Tools http 槽） |
| `router` / `serve` / `AxumTransportConfig` | `src/transport/axum.rs` | Axum 0.8 传输适配器 |
| `BaseError` | `src/error/mod.rs` | crate-wide structured errors |

## FEATURE GATES
| Feature | Enables |
|---------|---------|
| `token` | `src/token`, `TokenManager`（自动启用 `redis`） |
| `http` | `src/http` reqwest wrapper + Tools http 槽 |
| `mysql` | sqlx-backed table/action/database execution |
| `redis` | Redis client resource slot |
| `validator` | stricter regex-backed field validators |
| `plugin-schema` | JSON Schema plugin config validation |
| `metrics` | 运行期指标门面 |
| `openapi` | Catalog 投影 OpenAPI 3.1 |
| `admin-metadata` | 可选后台展示元数据 |
| `transport-axum` | Axum 0.8 HTTP 传输适配器（只拉 axum/tower-http，不拉 sqlx/reqwest） |

Default = `token`, `http`, `mysql`, `redis`, `validator`, `plugin-schema`（`transport-axum` 默认关闭）。

## CONVENTIONS
- `yang-base` inherits workspace lints and sets `#![warn(missing_docs)]`.
- Most public docs and user-facing errors are Chinese; keep that style.
- 资源一律经 `ToolsBuilder` 注册、`Tools` 获取；禁止新增进程级全局单例（`static OnceLock`/`lazy_static`）。
- `action` and `table` are large enough to have child AGENTS.md; read those before editing either module.
- Unit tests are colocated in `__tests__/`; integration tests live in `tests/` and often require Docker.

## ANTI-PATTERNS
- Builtin actions are non-generic typed handlers over runtime `TableDefinition`: add/get/select use `Record`, dynamic primary keys live inside typed DTOs. Custom actions implement the business `Action` trait（`index` + 强类型 Input/Output），由 `actions!`/`ModuleSpec::native_action` 原子注册；do not use a bare `serde_json::Value` as the whole input/output contract.
- 不要把 Action 定义与路由注册拆成两步；`#[derive(Action)]` 的 route/params/权限与 Handler 必须同源。
- Plugin code is a 1.4k-line single file with existing unwraps; avoid adding new panic paths.
- Do not bypass `TableQuery`/`FieldPermissions` when handling user-selected fields.
- Do not add hardcoded production credentials; test/default Docker credentials belong only in test docs/examples.
- Token system supports revocation via Redis-based blacklist (`TokenManager::revoke_token` / `is_revoked` / `verify_token_checked` in `token/revocation.rs`). For auth paths that need logout/revoke support, use `verify_token_checked` instead of bare `verify_token` (which skips the blacklist check).
- CORS 禁止通配来源 + credentials 组合；`transport-axum` 在构建期拒绝该配置。

## TESTING
```bash
cargo test --lib -p yang-base
cargo test --test transport_axum --features transport-axum
cargo test --test <name> -- --ignored --test-threads=1
```
Docker tests use MySQL 8.0 via testcontainers or standard containers. Prefer `.expect("具体上下文")` in tests over bare `.unwrap()`. Use `assert!(matches!(...))` for error-shape assertions.
