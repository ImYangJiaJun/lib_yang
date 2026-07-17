# BR 到原生 YANG 迁移指南

BR 业务模块应一次性迁移到唯一的 YANG 原生链路，不保留 compat crate、compat feature 或运行时 Adapter。迁移完成后，定义、Catalog、Registry、查询和资源所有权分别只有一个事实来源。

## 高频映射

| BR 写法 | 原生 YANG 写法 |
|---|---|
| Addon / Module 配置 | `Addon`、`Module`、`addon!`、`module!`、`modules!` |
| 动态 `br_fields` object | `fields!` / `params!` + 类型化 Builder |
| `request.body["name"]` | `params!` 生成的强类型 `Input` |
| `self.tools()` | `ActionContext::tools()` / `ctx.tools()` |
| `db.table("users")` | `yang_db::table!("users")` 或 `ctx.tables()` |
| 字符串字段和操作符 | `field!`、`CompareOp`、`Predicate` |
| `Tables::params_table` | `TableQueryPlan` / `Tables::params_table()` |
| `select` / `table_list` | `ctx.tables()?.search(...).page(...).table_select/table_list().await` |
| `Plugins::api_run("a.b.c")` | 构建期绑定的 `ActionLink<I, O>` + `ctx.plugins()?.api_run(...)` |
| 全局请求数据 | `RequestContext`、`ContextKey<T>`、`TenantContext`、`ActorContext` |
| 全局数据库/Redis | `ToolsBuilder -> Tools`，由每个 `BuiltApp` 显式持有 |

## 机械迁移

先以只读模式检查：

```powershell
cargo run -p yang-migrate -- br-to-yang --check path/to/addon
```

确认 diff 后写入：

```powershell
cargo run -p yang-migrate -- br-to-yang --apply path/to/addon
```

工具会转换可证明安全的表、字段、比较符、排序和 Join 字面量。输出中的 `manual-migration <file>:<line>` 是必须处理的字段级诊断，主要包括动态表名/字段名、`JsonValue` 参数读取、BR 字段 Builder、字符串内部 Action 调用和隐式 Tools 所有权。工具不会猜测动态业务含义。

## 单个 Module 的迁移顺序

1. 用 `fields!` 固定表字段、关系、权限和租户键，并确保 `AppBuilder::build` 能解析全部引用。
2. 用 `params!` 声明 body/query/path/header 来源，删除 `request.body[...]` 读取。
3. 为每个接口实现强类型 `Action::index`，通过 `actions!` 原子注册定义和 Handler。
4. 将查询迁移到 `TableQueryPlan` / `Tables` 或受控 `yang-db::QueryBuilder`，移除字符串操作符解析。
5. 将跨 Action 调用迁移为 `ActionLink`，将请求状态迁移为类型化 `RequestContext`。
6. 同步验证 Catalog、Registry、Schema、OpenAPI、后台 View 和租户 fail-closed 行为。

`project/yang-system/src/modules/org.rs` 是关系、租户、列表和选择器的可运行参考；`project/yang-system/src/modules/user/register_via_plugin.rs` 是零 JSON 内部调用参考。该项目通过 `../../crates/yang-base` 与 `../../crates/yang-db` 相对路径直接联合调试。
