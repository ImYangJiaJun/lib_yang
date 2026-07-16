# yang-base-derive

`yang-base` 的过程宏（proc-macro）crate，提供类型化 Action 系统的派生基础设施。

> 一般不需要直接依赖本 crate。`yang-base` 会重导出 `Action`，下游可直接写
> `yang_base::Action`。

## 提供的派生宏

### `#[derive(Action)]`

为 Action struct 生成 `TypedAction` impl 与静态 `ActionMeta` 聚合体：

- impl `TypedAction`（`name` / `display_name` / `description` / `is_public` / `permissions`）。
- 用 `schemars::schema_for!` 从 `TypedHandler::Input` / `Output` 生成静态 JSON Schema。
- 聚合出 `OnceLock<ActionMeta>`，供 `ModuleRouter::dispatch` 读取以做鉴权与派发。

依赖手写的 `TypedHandler`（声明 `Input` / `Output` 关联类型 + `handle` 业务逻辑）；本宏只补元数据与 trait 胶水。

支持泛型 Action，并透传原 struct 的泛型参数与 `where` 约束。

属性：

- `name = "..."`：唯一标识，必填。
- `display_name = "..."`：用户可见名称，默认同 `name`。
- `description = "..."`：简介，默认空字符串。
- `public`：公开 Action，默认关闭。
- `permissions("a", "b")`：权限列表。
- `permission_mode = "all" | "any"`：权限组合模式，默认 `all`。

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
