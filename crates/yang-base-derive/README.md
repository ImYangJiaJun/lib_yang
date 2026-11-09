# yang-base-derive

`yang-base` 的过程宏（proc-macro）crate，提供类型化 Action 系统的派生基础设施。

> 一般不需要直接依赖本 crate。`yang-base` 会重导出 `Action`，下游可直接写
> `yang_base::Action`。

## 提供的派生宏

### `#[derive(Action)]`

为 Action struct 生成 `TypedAction` impl 与静态 `ActionMeta` 聚合体：

- impl `TypedAction`（`name` / `display_name` / `description` / `is_public` / `permissions` / `permission_mode` / `http_method` / `path` / `success_status` / `response_kind` / `multipart_spec`）。
- 用 `schemars::schema_for!` 从 `TypedHandler::Input` / `Output` 生成静态 JSON Schema。
- `ActionMeta` 由 `TypedAction::meta_static()` 以 `OnceLock` 构造，经 `DynAction::meta()` 供运行时读取；鉴权策略在 `AppBuilder` 构建期由 `permissions` / `is_public` / `permission_mode` 冻结为 `AuthorizationPolicy`。

依赖手写的 `TypedHandler`（声明 `Input` / `Output` 关联类型 + `handle` 业务逻辑）；本宏只补元数据与 trait 胶水。

支持泛型 Action，并透传原 struct 的泛型参数与 `where` 约束。

属性：

- `name = "..."`：唯一标识，必填。
- `display_name = "..."`：用户可见名称，默认同 `name`。
- `description = "..."`：简介，默认空字符串。
- `public`：公开 Action，默认关闭。
- `permissions("a", "b")`：权限列表。
- `permission_mode = "all" | "any"`：权限组合模式，默认 `all`；其它取值编译期报错。
- `method = "..."`：HTTP method，默认 `POST`；仅接受 `GET` / `POST` / `PUT` / `PATCH` / `DELETE` / `OPTIONS` / `HEAD`，其它取值编译期报错。
- `path = "..."`：HTTP path，须以 `/` 开头；未提供时由 Action 名生成。
- `success_status = ...`：成功状态码，默认 `200`。
- `response_kind = "..."`：成功响应类别，默认 `json`；可选 `json` / `download` / `preview` / `redirect`。
- `request_media = "..."`：请求媒体类型，默认 `json`；可选 `json` / `multipart`。
- `content_types("image/png", ...)`：multipart 必须声明的精确 MIME 类型，须为小写。
- `max_fields = ...` / `max_files = ...`：multipart 非文件字段数 / 文件数上限。
- `max_file_bytes = ...` / `max_total_bytes = ...`：multipart 单文件 / 整请求字节上限，须大于 0，且 `max_file_bytes` 不得大于 `max_total_bytes`。

multipart 相关选项只能与 `request_media = "multipart"` 同用，且此时 `method` 限 `POST` / `PUT` / `PATCH`。

### `params!`

一次声明强类型 Input 结构与唯一的原生 `Params` 定义：生成实现 `ParamInput` 的 struct（含 `Deserialize` / `JsonSchema`），可直接作为 `Action::Input` / `TypedHandler::Input`。字段默认取自 body，可用 `#[param(source = body|query|path|header)]` 指定来源。

```rust
yang_base::params! {
    #[deny_unknown_fields]
    LoginInput {
        username: Str::new().require(true),
        #[param(source = header)]
        x_trace_id: Str::new().require(true),
    }
}
```

## 使用示例

```rust
use yang_base::Action;

#[derive(Action)]
#[action(
    name = "ping",
    display_name = "心跳",
    description = "检查服务连通性",
    public
)]
struct PingAction;
```

`PingAction` 还需实现 `yang_base::action::TypedHandler`，声明 `Input`、`Output`
并实现 `handle`。派生宏负责补齐元信息、JSON Schema 缓存和 `TypedAction` 实现。

表结构不再由本 crate 从 Rust 实体推导；请使用 `yang-base` 的 schema-first
`Table::new(...).fields([Field::...])` API 显式声明。

## 许可证

MIT OR Apache-2.0
