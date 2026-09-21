# 版本与兼容策略

当前仓库基线：

- `yang-base` 0.3.0
- `yang-base-derive` 0.2.1
- `yang-db` 0.1.6

0.x 仍允许在次版本中发布 breaking change，但本仓库继续把 patch 版本用于兼容增强；任何有意删除、重命名或收紧公共契约的变化必须集中到明确的升级版本，并提供迁移说明和契约测试。

## yang-base 0.2.x 公共边界

0.2.0 已完成应用侧 schema-first 切换：

- 表结构通过 `Table` / `Field` 构建，并在 `build()` 后成为不可变 `TableDefinition`。
- 动态表行统一使用透明 JSON object `Record`。
- 标准表接口通过 `ModuleRouter::table(definition).crud()` 注册；附属启动期 schema 使用 `ModuleRouter::schema(definition)`。
- 自定义端点通过 `Api::{get,post,put,patch,delete}` 与 `ModuleRouter::api` / `apis` 原子注册。
- 应用模块通过 `AppRouter::module` / `modules` 聚合，`ApiCatalog` 是 transport、OpenAPI 与后台展示引用的确定性事实源。
- 自定义业务操作继续实现 `TypedHandler`，并由 `#[derive(Action)]` 生成 `TypedAction` 元数据。

从 0.1.x 升级到 0.2.0 是有意的公共 API 迁移，应用应按上述边界重写表声明和路由注册。0.2.x 后续 patch 不得再次恢复或扩展已删除的应用模型。

### 0.2.x 兼容规则

- 允许新增 builder 方法、只读元数据、feature、结构化错误和 `Result` 返回入口。
- `TableDefinition`、`Record`、`Api`、`ApiCatalog` 与现有 Router builder 的公开签名在 patch 版本内保持源码兼容。
- 新可选 feature 默认关闭，不得向关闭构建泄漏依赖。
- 默认 feature 的变化必须进入新的升级版本，并列出等价显式配置。
- 收紧字段校验或 schema 同步行为时，必须提供正反例测试并说明 fail-fast 条件。

## yang-db 0.1.x 规则

`yang-db` 仍处于 0.1.6：

- 允许新增类型、方言能力、checked identifier 入口和结构化错误；已有入口保持可编译。
- 不安全、含糊或无法验证的入口先 deprecated，并提供替代 API、迁移示例和兼容测试。
- MySQL/PostgreSQL 对称 API 不表示 SQL 语义完全相同；差异由 `BackendCapabilities` 明示。
- RAW/native SQL 是受控逃生舱，不接受不可信输入。

### 迁移记录 API

新代码应让初始化器执行迁移，或显式提供 checksum/status：

```rust,ignore
initializer.run_migrations(&plugin).await?;
initializer
    .record_migration_with_checksum(
        "accounts",
        "v2",
        "0123456789abcdef",
        "applied",
    )
    .await?;
```

无 checksum 的兼容入口无法验证迁移漂移，不应进入新代码。

### SQL 标识符与错误

可信固定表达式可以使用表达式入口；外部列名使用 checked API：

```rust,ignore
let query = db
    .table("users")
    .field_identifier(user_selected_column)?
    .order_identifier(user_selected_column, true)?;
let sql = query.try_to_sql()?;
```

unchecked operator、分号切割脚本和隐式 RAW 回退只用于兼容；新的公共路径必须返回结构化错误。

## 后续 breaking change 要求

未来若删除 deprecated 数据库入口、调整默认 feature、改变 schema 同步策略或收紧 identifier 语义，必须同时具备：

1. 固定升级版本与逐项迁移说明。
2. 新旧行为的编译或运行期契约测试。
3. feature 组合检查与目标数据库集成验证。
4. README、公共 API 文档、能力矩阵和 release docs contract 同步更新。

不得只修改源码而保留旧版本文档，也不得只更新文档而缺少可执行契约。

## yang-base 0.3.0 迁移说明

0.3.0 是一次**公共契约收紧**版本。逐项变更与迁移方式如下。

### 新增 `ResponseBody::Raw` 与 `ResponseAttachment::Raw`

外部系统的响应契约可能与框架的 `{code,message,data}` 包络不兼容（例如要求顶层键名为
`msg`）。为此新增逃生口：Action 以 `ResponseBody::Raw` 为输出时，传输层按变体自带的
`content_type` 直接返回 `body`，**不套 `ApiResponse` 包络**。

- 构造请用 `ResponseBody::raw(body, content_type) -> Result<Self, BaseError>`：它校验
  `content_type` 必须为 `application/json`（防止该通道被用来返回 `text/html`，从而绕过
  前端 JSON 契约的保护）。
- HTTP 状态固定 `200`；`max_attachment_bytes` 由该变体自行比对（`file_response` 的限额
  不覆盖它）。
- **迁移**：对 `ResponseBody` 或 `ResponseAttachment` 做穷尽 `match` 的下游代码必须补
  `Raw` 臂。不使用该能力的调用方无需任何改动。

### 两个枚举加 `#[non_exhaustive]`

`ResponseBody` 与 `ResponseAttachment` 现在标注 `#[non_exhaustive]`，此后新增变体不再
构成破坏性变更。

- **迁移**：下游穷尽 `match` 需补通配臂。注意该属性只对**其它 crate** 生效，yang-base
  内部仍是穷尽匹配。

### 未变更的部分

- `ActionResponseKind` **未**新增变体：`Raw` 的 body 就是 JSON，声明 `Json` 是诚实的，
  前端 zod 契约（`.enum([...]).catch("json")`）因此零影响。
- `ApiResponse` 的 JSON 线格式不变（`attachment` 字段带 `serde(skip)`）。
- 普通 Action、附件（下载/预览/重定向）的行为与状态码语义均未改变。

### 契约测试

- `crates/yang-base/src/action/response.rs` 的 `mod tests`：content-type 白名单、Raw →
  附件的映射、JSON 线格式不含 `attachment`。
- `crates/yang-base/tests/transport_axum.rs`：端到端裸 body 与响应头、`response_kind`
  投影保持 `json`。
- `crates/yang-base/tests/compatibility_contract.rs` 与 `release_docs_contract.rs`：
  版本号与文档一致性。

> 注意：`crates/yang-base/tests/*.rs` **不被 `run_ci.py` 执行**（quick/full 对 yang-base
> 只跑 `--lib`）。验证传输层行为需显式运行
> `cargo test --test transport_axum -p yang-base --features transport-axum --locked`。
