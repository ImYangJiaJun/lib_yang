# 飞书审批外部数据源 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为飞书审批的「关联外部选项」自建数据源服务——存储数据源与选项，对审批暴露字面严格的外部选项接口，并以写入 API 支撑多维表格工作流自动更新数据。

**Architecture:** 业务全部落在 `project/yang-system/src/addon/feishu/`（两个 module + 一个 `domain/`）。唯一触碰基础库的改动是给 `crates/yang-base` 增加 `ResponseBody::Raw`——一个「按声明 content-type 返回裸 body、不套框架 JSON 包络」的通用逃生口，因为框架的 Action 响应恒为 `{code,message,data}` 且键名不可改。三个 HTTP 入口按调用方分流：飞书审批（Raw 严格信封）、多维表格写入（静态 Token 中间件）、前端控制台（JWT + 权限）。

**Tech Stack:** Rust 2021 / axum 0.8（经 `yang-base` 的 `transport-axum`）/ sqlx + MySQL 8 / Redis 7 / `aes` + `cbc` + `sha2` + `base64` + `subtle` / serde / schemars。

**Spec:** `docs/superpowers/specs/2026-09-21-feishu-approval-external-options-design.md`

## Global Constraints

- **两个仓库**：A = `D:/code/lib_yang`（工作区），B = `D:/code/lib_yang/project/yang-system`（独立嵌套 Git/Cargo 项目，被 A 的 workspace 显式 exclude）。B 的 Cargo 命令必须在 B 目录内执行。
- **所有 cargo 命令带 `--locked`**（`cargo fmt` 除外）；`Cargo.lock` 必须提交。
- **MSRV 1.80**：`rust-version = "1.80"`；`aes 0.8` / `cbc 0.1` / `cipher 0.4` 均远低于该门槛。
- **禁止 `unsafe`、生产代码禁止 `unwrap()` / `expect()`**。B 的 `[lints.clippy]` 把 `unwrap_used` / `expect_used` 设为 `deny`，**测试代码同样生效**——测试里写 `unwrap_or_else(|error| panic!("…: {error}"))`。
- **提交信息用中文**（仓库约定），Conventional Commits 前缀。范围限 `feishu` / `yang-base` / `config`。
- **注释、public docs、用户可见文案一律中文**。`yang-base` 有 `#![warn(missing_docs)]`，新增 `pub` 项缺文档注释会在 clippy `-D warnings` 下变成硬失败。
- **路径前缀**：所有业务路由必须以 `/api/v1/` 开头，且必须落在 `/api` 下（生产 nginx 只反代 `/(api|\.well-known|health)`，该正则被 `frontend/scripts/verify-deployment-contract.mjs` 逐字冻结为门禁）。
- **module 命名避开 `org` / `work`**——这两个名字会激活租户隔离门禁。
- **`src/addon/**/actions/*.rs` 禁止 `#[derive(Action)]`**，统一走 `module.action_fn(...).register()` 函数式通道；每个文件恰好一个 `pub(super) async fn handle(` + 一个 `pub(super) fn register(`，且必须进 `actions/mod.rs` 的 `ACTIONS` 数组。
- **`src/` 顶层只允许** `{addon, config, infrastructure}` 目录与 `{app.rs, bootstrap.rs, lib.rs, main.rs}` 文件。
- **业务失败必须返回 `Ok(...)` 而非 `Err`**：匿名端点上 `Err` 记 `result="error"` 会烧全站 99.9% 可用性预算，而 burn-rate 规则按 `sum by (job)` 聚合、不带 operation 维度。
- **提交前门禁**：A 跑 `python scripts/run_ci.py quick`；B 跑 `python scripts/run_ci.py quick`（含 `check_architecture.py`）。每个 Task 结束时提交。
- **跨仓库推送顺序**：先 A 后 B。

---

## 关键前置事实（实现时不要重新推导）

这些是写计划时逐条核对源码得到的，实现时直接照用：

| 事实 | 锚点 |
|---|---|
| 传输层**只有一处**决定是否套框架包络：无 attachment 时才 `Json(response)` | `crates/yang-base/src/transport/axum.rs:506-511` |
| `attachment_response` 对 attachment 穷尽 match，加变体后编译器强制补臂 | `axum.rs:1009-1034` |
| **第三处**穷尽 match：`warn_response_kind_mismatch` 按 `&response.attachment` 匹配并映回 kind | `crates/yang-base/src/definition/builder/registry.rs:402-423` |
| `infer_action_presentation` / `validate_action_presentation` 匹配的是 `ActionResponseKind`，**不是** attachment → 本方案不新增 kind，**无需改** | `compile.rs:464-484`、`compile.rs:557-577` |
| `append_action_response_headers` 拒绝名单为 `content-length\|transfer-encoding\|connection\|x-request-id`（**不含 content-type**）且用 `append` → 会产出重复头 | `axum.rs:531-551` |
| `max_attachment_bytes` 只在 `file_response` 内校验；Raw 不经过它 | `axum.rs:1050-1069` |
| `FnAction::dispatch` 与 derive 通道同构，同样调用 `wrap_dispatch_output` | `crates/yang-base/src/action/functional.rs:83` |
| `ActionFnBuilder` **没有** `tag()`；`permissions(P)` 只收一个参数；`public()` 不收参数；`register()` 返回 `ModuleSpec` | `interface.rs:157-268` |
| `operation_id` 在注册期被补全为 `<module 全名>.<action 名>`，仅当它仍等于 Action 局部名时 | `interface.rs:260-262` |
| `ParamInput` 只有两个方法：`params() -> Params` 与 `decode(&mut Request) -> Result<Self, BaseError>`；**手写时 `decode` 必须去掉 `where Self: DeserializeOwned` 子句** | `definition/param.rs:10-26` |
| `Request` 公开字段：`body: Value` / `headers` / `query` / `path_params`；取值用 `get_header` / `get_query` / `get_path_param` | `action/request.rs:62-77, 379-455` |
| `Next` 三字段均 `pub(crate)`，**crate 外无法构造**；`next.policy` 亦不可读 | `router/middleware.rs:176` |
| `action!("a.b.c")` 按**最后一个点**切分 → `module=a.b, action=c` | `definition/name.rs:215` |
| `AddonSpec::middleware` 只在 `.module(...)` **之后**调用才有效（内部 insert 到已有 module 链首） | `spec.rs:834-843` |
| DSL 的 `filterable` / `sortable` 是 **fail-closed**（未声明即显式关闭） | `definition/field.rs:311-320` |
| DSL 的 `secret(true)` 会把 readable/writable 同时置为 `Nobody` | `definition/field.rs:539-546` |
| DSL 的 `unique(true)` → `FieldSpec.storage.unique` → `field.unique()`（真实 UNIQUE 索引） | `definition/field.rs:276-277, 503-506` |
| DSL **没有** Json builder；`simple_builder!` 只实例化 9 个 | `definition/field.rs:595-603` |
| DSL **无法**声明自然键主键（`Key` 硬编码为 `Field::id` = `BigInt + required + primary_key + auto_increment + not_writable`）；`TableSpec` 上也没有 `primary_key()` | `definition/field.rs:213`、`table/definition.rs:112-119`、`field.rs:875` |
| DSL 字段级只有 `indexed(bool)`——**不存在** `index()`；`index()` 是 `TableSpec` 的表级方法且收 `FieldRef` 迭代器 | `definition/field.rs:509`、`field.rs:947` |
| DSL 缺 `Key` 时构建期直接报「表 X 必须定义一个主键」 | `table/definition.rs:794-795` |
| `Record::require::<T>` / `optional::<T>` / `set`；DECIMAL 解码为 String | `table/record.rs:74-140` |
| `bind` 收 `Arc<MySqlPool>`：`Arc::new(ctx.tools().mysql()?.pool().clone())` | `table/definition.rs:1040` |
| `insert` 恒返回 `Ok(1)`；要自增 id 用 `insert_returning_id` → `(u64, u64)` | `table/table_query/write.rs:53-107` |
| `paginate_records` 在 total==0 时直接返回 `PaginatedResult::empty`，`total_pages = 0` | `table/table_query/read.rs:79` |
| `search` 对「无可搜索文本字段」的角色 fail-closed 报 `PermissionDenied` | `table/table_query/filters.rs:543-565` |
| `MAX_QUERY_PAGE_SIZE = 100`；`page` 参数是 `usize` | `table/query_params.rs:49`、`filters.rs:500` |
| 审计摘要字段名经 `validate_identifier`，**命中 `password/secret/token/nonce/credential/authorization/cookie/hash` 任一子串即拒绝** | `B/src/infrastructure/audit/event.rs:18-27, 349-382` |
| `succeeded_system_event` 依赖 `ctx.dispatch_target()` 返回 `Some((module, action))` | `B/src/infrastructure/audit/repository.rs:45-79` |
| `finish_transaction` 是各 module 上下文的**固有关联函数**：`Feishu::finish_transaction(tx, result)` | `B/src/addon/demo/domain/context.rs:26` |
| `ENVIRONMENT_BINDINGS` 是白名单：未登记的 `YANG_SYSTEM_*` 变量让进程**直接启动失败** | `A/crates/yang-runtime/src/config.rs:230-236` |
| 新增 secret 要同步 **3 处** `#[cfg(test)]` 项：`SecretKey` 枚举、`file_name`、`SECRET_KEYS` | `B/src/config/source.rs:406-443` |
| `example_config_has_no_schema_mode` 是**精确长度断言**（`[mysql]` 段 len==1、每个 `[email.X]` 段 len==1） | `B/src/config/mod.rs:2280` |
| `shipped_configs_cannot_boot_verbatim` 要求 example/show 原样解析**必须失败** | `B/src/config/mod.rs:2436` |
| 占位密钥必须以 `replace-with` / `replace_with` 开头才会被扫描 | `B/src/config/mod.rs:2385-2389` |
| `crates/yang-base/tests/*.rs` **不被任何门禁执行**（只跑 `--lib`），仅在 clippy `--all-targets` 下编译 | `A/scripts/run_ci.py:49-91` |
| `infrastructure_definitions()` 返回 `[TableDefinition; 6]`，测试断言数组**顺序敏感** | `B/src/infrastructure/schema.rs:47`、`:289` |
| integration 测试须读 `YANG_SYSTEM_TEST_DATABASE_URL`（库名以 `_test` 结尾）才会被 `run_ci.py` 反向发现并须登记 `INTEGRATION` 元组 | `B/scripts/run_ci.py:370-390` |

---

## 文件结构

**A 仓库（lib_yang）**

| 文件 | 动作 | 职责 |
|---|---|---|
| `crates/yang-base/src/action/response.rs` | 修改 | `ResponseBody::Raw` / `ResponseAttachment::Raw` / `From` 臂 / 构造器 / 单元测试 |
| `crates/yang-base/src/transport/axum.rs` | 修改 | `attachment_response` 的 Raw 渲染臂 |
| `crates/yang-base/src/definition/builder/registry.rs` | 修改 | `warn_response_kind_mismatch` 的 Raw 臂 |
| `crates/yang-base/Cargo.toml` + `README.md` | 修改 | 版本 0.2.2 → 0.3.0 与其文档同步 |
| `crates/yang-base/tests/transport_axum.rs` | 修改 | Raw 端到端集成测试（**须手动运行**） |
| `docs/reference/{VERSIONING,yang-base,BASE_DB_CAPABILITY_MATRIX}.md`、`AGENTS.md` | 修改 | 版本串同步 |

**B 仓库（yang-system）**

| 文件 | 动作 | 职责 |
|---|---|---|
| `src/config/mod.rs` | 修改 | `FeishuSettings` + `Settings.feishu` + `validate()` |
| `src/config/source.rs` | 修改 | env/secret 绑定 + 测试用 `SecretKey` |
| `config.show.toml`、`docs/contracts/CONFIGURATION.md` | 修改 | 配置文档 |
| `src/addon/feishu/mod.rs` | 新建 | addon 唯一对外入口 `build_addon()` |
| `src/addon/feishu/domain/mod.rs` | 新建 | domain 导出 |
| `src/addon/feishu/domain/context.rs` | 新建 | `FeishuContext`：两个 Repository + `finish_transaction` |
| `src/addon/feishu/domain/repository.rs` | 新建 | 两张表的唯一持久化边界 |
| `src/addon/feishu/domain/protocol.rs` | 新建 | 飞书请求/响应 DTO（严格键名） |
| `src/addon/feishu/domain/crypto.rs` | 新建 | AES-256-CBC + SHA-256 派生 + Base64 |
| `src/addon/feishu/domain/token.rs` | 新建 | Token 的 sha256 与常量时间比较 |
| `src/addon/feishu/domain/i18n.rs` | 新建 | `@i18n@` 占位符与 `i18nResources` 构造 |
| `src/addon/feishu/domain/pagination.rs` | 新建 | `page_token` 游标编解码 |
| `src/addon/feishu/datasource/{mod.rs,table.rs,actions/*}` | 新建 | 数据源 module |
| `src/addon/feishu/option/{mod.rs,table.rs,actions/*}` | 新建 | 选项 module（含公开端点） |
| `src/addon/mod.rs`、`src/app.rs` | 修改 | 组合根接线（3 处） |
| `src/addon/feishu/domain/middleware.rs` | 新建 | 静态 API Token 校验中间件 |
| `tests/feishu_options_integration.rs` | 新建 | 端到端集成测试 |
| `frontend/contracts/openapi.json`、`frontend/src/engine/contracts/api-types.ts` | 重新生成 | OpenAPI 契约产物 |

---

## Task 1: `yang-base` 增加 `ResponseBody::Raw`（A 仓库）

**Files:**
- Modify: `crates/yang-base/src/action/response.rs`（`ResponseBody` 378-398、`ResponseAttachment` 420-443、`From` 445-453、`mod tests` 473+）
- Modify: `crates/yang-base/src/transport/axum.rs:1009-1034`（`attachment_response`）
- Modify: `crates/yang-base/src/definition/builder/registry.rs:402-423`（`warn_response_kind_mismatch`）
- Modify: `crates/yang-base/Cargo.toml:3`、`crates/yang-base/README.md`
- Test: `crates/yang-base/tests/transport_axum.rs`

**Interfaces:**
- Consumes: 无
- Produces:
  - `yang_base::action::ResponseBody::{Raw { body: String, content_type: String }, raw(body: impl Into<String>, content_type: impl Into<String>) -> Result<Self, BaseError>, download, preview, redirect}`
  - `yang_base::action::ResponseAttachment::Raw { body: String, content_type: String }`
  - `const RAW_ALLOWED_CONTENT_TYPE: &str = "application/json"`（crate 内私有）
  - 语义契约：以 `ResponseBody::Raw` 为 `Action::Output` 返回时，HTTP body 就是 `body` 原文，`Content-Type` 就是 `content_type`，HTTP 状态固定 `200`，**不套 `ApiResponse` 包络**。

> ⚠️ 本任务的三处穷尽 match **必须同时改**，漏一处编译器报 E0004。

- [ ] **Step 1: 写失败测试——单元测试（这条会被门禁执行）**

在 `crates/yang-base/src/action/response.rs` 的 `mod tests` 末尾追加：

```rust
    #[test]
    fn raw_response_body_rejects_non_json_content_type() {
        // Raw 只允许 application/json：这个逃生口不得被用来返回 text/html
        let rejected = ResponseBody::raw("<html></html>", "text/html");
        assert!(rejected.is_err(), "非 application/json 必须被拒绝");
        let error = rejected.err().expect("上面已断言是 Err");
        assert!(
            error.to_string().contains("application/json"),
            "错误信息应说明允许的 content-type，实际: {error}"
        );
    }

    #[test]
    fn raw_response_body_maps_to_raw_attachment_without_envelope() {
        // Raw 输出映射为 Raw 附件，且不进入 data
        let body = ResponseBody::raw(r#"{"code":0,"msg":"ok"}"#, "application/json")
            .expect("application/json 应被接受");
        let response = wrap_dispatch_output(body, "成功").expect("收口应成功");
        assert_eq!(response.code, 0);
        assert!(response.data.is_none(), "Raw 不得进入 data");
        assert_eq!(
            response.attachment,
            Some(ResponseAttachment::Raw {
                body: r#"{"code":0,"msg":"ok"}"#.to_string(),
                content_type: "application/json".to_string(),
            })
        );
    }

    #[test]
    fn raw_response_body_survives_json_wire_format() {
        // Raw 变体不得让 ApiResponse 的 JSON 线格式出现 attachment 字段
        let body = ResponseBody::raw("{}", "application/json").expect("应被接受");
        let response = wrap_dispatch_output(body, "成功").expect("收口应成功");
        let json = serde_json::to_string(&response).expect("应可序列化");
        assert!(!json.contains("attachment"), "线格式不得含 attachment: {json}");
    }
```

- [ ] **Step 2: 运行单元测试确认失败**

```bash
cd /d/code/lib_yang && cargo test --lib -p yang-base --locked raw_response_body
```
Expected: 编译失败，报 `no function or associated item named 'raw'` 与 `no variant named 'Raw'`。

- [ ] **Step 3: 给 `ResponseBody` 加变体与构造器**

`crates/yang-base/src/action/response.rs`：在文件顶部（`use` 之后）加常量：

```rust
/// `ResponseBody::Raw` 唯一允许的响应内容类型。
///
/// 该逃生口存在的理由是「外部系统契约与框架 JSON 包络不兼容」，不是通用响应通道；
/// 限死为 JSON 可防止它被用来返回 `text/html` 从而绕过前端 JSON 契约的保护。
const RAW_ALLOWED_CONTENT_TYPE: &str = "application/json";
```

把 `pub enum ResponseBody` 改为带 `#[non_exhaustive]` 并追加变体：

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ResponseBody {
    /// 文件下载：传输层以 `Content-Disposition: attachment` 返回文件字节。
    Download {
        /// 服务器本地文件路径。
        path: PathBuf,
        /// 下载文件名（写入 Content-Disposition 的 filename 参数）。
        filename: String,
    },
    /// 文件预览：传输层以 `Content-Disposition: inline` 返回文件字节。
    Preview {
        /// 服务器本地文件路径。
        path: PathBuf,
    },
    /// 重定向：传输层返回 302 状态码与 `Location` 头。
    Redirect {
        /// 目标地址。
        url: String,
    },
    /// 原始响应体：传输层直接以 `content_type` 返回 `body`，**不套 `ApiResponse` 包络**。
    ///
    /// 供外部系统要求固定响应形状（键名、层级与框架包络不一致）时使用。
    /// `content_type` 只允许 `application/json`；`body` 必须是该类型下的完整响应文本。
    Raw {
        /// 完整响应体文本（UTF-8）。
        body: String,
        /// 响应 `Content-Type`；当前只允许 `application/json`。
        content_type: String,
    },
}
```

在 `impl ResponseBody` 内追加构造器：

```rust
    /// 构造原始响应体。
    ///
    /// # 错误
    ///
    /// `content_type` 不是 `application/json` 时返回 [`BaseError::ConfigError`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use yang_base::action::ResponseBody;
    ///
    /// let body = ResponseBody::raw(r#"{"code":0}"#, "application/json")?;
    /// assert!(matches!(body, ResponseBody::Raw { .. }));
    /// # Ok::<(), yang_base::BaseError>(())
    /// ```
    pub fn raw(
        body: impl Into<String>,
        content_type: impl Into<String>,
    ) -> Result<Self, BaseError> {
        let content_type = content_type.into();
        if content_type != RAW_ALLOWED_CONTENT_TYPE {
            return Err(BaseError::ConfigError(format!(
                "Raw 响应 content-type 只允许 {RAW_ALLOWED_CONTENT_TYPE}，收到 {content_type}"
            )));
        }
        Ok(Self::Raw {
            body: body.into(),
            content_type,
        })
    }
```

- [ ] **Step 4: 给 `ResponseAttachment` 加变体并补 `From` 臂**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResponseAttachment {
    /// 文件下载（`Content-Disposition: attachment`）。
    Download {
        /// 服务器本地文件路径。
        path: PathBuf,
        /// 下载文件名。
        filename: String,
    },
    /// 文件预览（`Content-Disposition: inline`）。
    Preview {
        /// 服务器本地文件路径。
        path: PathBuf,
    },
    /// 重定向（302 + `Location`）。
    Redirect {
        /// 目标地址。
        url: String,
    },
    /// 原始响应体（`Content-Type` 由变体自带，不套框架包络）。
    Raw {
        /// 完整响应体文本。
        body: String,
        /// 响应 `Content-Type`。
        content_type: String,
    },
}
```

`From<ResponseBody> for ResponseAttachment` 补臂：

```rust
            ResponseBody::Raw { body, content_type } => Self::Raw { body, content_type },
```

- [ ] **Step 5: 传输层补渲染臂**

`crates/yang-base/src/transport/axum.rs` 的 `attachment_response`，在 `ResponseAttachment::Preview` 臂之后追加：

```rust
        ResponseAttachment::Raw { body, content_type } => {
            // file_response 才做 max_attachment_bytes 校验，Raw 不经过它，必须自己比对
            if let Some(max_bytes) = max_attachment_bytes {
                if body.len() as u64 > max_bytes {
                    return error_response(
                        StatusCode::PAYLOAD_TOO_LARGE,
                        BaseError::ParamInvalid(
                            "attachment".to_string(),
                            "附件超过大小上限".to_string(),
                        ),
                    );
                }
            }
            match HeaderValue::from_str(&content_type) {
                Ok(value) => {
                    (StatusCode::OK, [(header::CONTENT_TYPE, value)], body).into_response()
                }
                Err(_) => error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    BaseError::ConfigError("Raw 附件响应 content-type 非法".to_string()),
                ),
            }
        }
```

- [ ] **Step 6: 补第三处穷尽 match**

`crates/yang-base/src/definition/builder/registry.rs` 的 `warn_response_kind_mismatch`，在 `Some(ResponseAttachment::Redirect { .. })` 臂之后、`None` 臂之前插入：

```rust
        // Raw 的 body 就是 JSON 文本，与声明 Json 的 Action 一致，不触发告警
        Some(ResponseAttachment::Raw { .. }) => crate::definition::ActionResponseKind::Json,
```

- [ ] **Step 7: 运行单元测试确认通过**

```bash
cd /d/code/lib_yang && cargo test --lib -p yang-base --locked raw_response_body
```
Expected: 3 passed。

- [ ] **Step 8: 写端到端集成测试（须手动运行）**

在 `crates/yang-base/tests/transport_axum.rs` 中，参照既有 `RedirectAction` 的写法（约 235-369 行）新增一个返回 Raw 的探针 Action，并在约 1265-1383 行的附件响应测试组里追加测试。探针：

```rust
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct RawProbeInput {}

impl yang_base::definition::ParamInput for RawProbeInput {
    fn params() -> yang_base::definition::Params {
        yang_base::definition::Params::new()
    }

    // 注意：手写 decode 必须去掉 trait 默认实现上的 `where Self: DeserializeOwned` 子句，
    // 否则报 E0195/E0276
    fn decode(
        _request: &mut yang_base::action::Request,
    ) -> Result<Self, yang_base::BaseError> {
        Ok(Self {})
    }
}

/// 返回原始 JSON body 的探针 Action：用于验证传输层不套 ApiResponse 包络。
struct RawAction;

#[async_trait]
impl yang_base::action::Action for RawAction {
    type Input = RawProbeInput;
    type Output = yang_base::action::ResponseBody;

    async fn index(
        &self,
        _ctx: yang_base::action::ActionContext,
        _input: Self::Input,
    ) -> Result<Self::Output, yang_base::BaseError> {
        yang_base::action::ResponseBody::raw(
            r#"{"code":0,"msg":"success!","data":{"result":{"options":[]}}}"#,
            "application/json",
        )
    }
}
```

其 `TypedAction` 元数据按既有探针的 `#[derive(Action)]` + `#[action(name = "...", path = "/raw", public)]` 形态声明（该文件位于 crate 内，可用 derive）。测试：

```rust
#[tokio::test]
async fn raw_action_returns_unwrapped_json_body() {
    let response = oneshot(default_router(), json_request("POST", "/raw", "{}")).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/json"),
    );
    let bytes = body_bytes(response).await;
    let text = String::from_utf8(bytes).expect("Raw body 应是 UTF-8");
    assert!(text.starts_with(r#"{"code":0,"msg":"success!""#), "实际: {text}");
    assert!(
        !text.contains("\"message\""),
        "Raw 响应不得出现框架包络的 message 键: {text}"
    );
}
```

- [ ] **Step 9: 手动运行集成测试（门禁不跑它们）**

```bash
cd /d/code/lib_yang && cargo test --test transport_axum -p yang-base --features transport-axum --locked
```
Expected: 全部 passed，其中含新增的 `raw_action_returns_unwrapped_json_body`。

> 这一步不可省略：`run_ci.py` 对 yang-base 只跑 `--lib`，`tests/*.rs` 只在 clippy 下被编译。把这条命令的输出贴进提交说明或任务记录。

- [ ] **Step 10: 版本 bump 到 0.3.0 并同步文档**

`crates/yang-base/Cargo.toml` 的 `version = "0.2.2"` → `version = "0.3.0"`。同步以下文件中出现的 `0.2.2`：

```bash
cd /d/code/lib_yang && grep -rln "0\.2\.2" --include="*.md" --include="*.toml" --include="*.rs" . | grep -v "^./target" | grep -v "^./project"
```
逐个更新：`README.md`、`crates/yang-base/README.md`、`AGENTS.md`、`docs/reference/VERSIONING.md`、`docs/reference/yang-base.md`、`docs/reference/BASE_DB_CAPABILITY_MATRIX.md`、`docs/plans/YANG_BASE_DB_COMPLETENESS_PLAN.md`。
再更新 `crates/yang-base/tests/compatibility_contract.rs` 里的 `VERSION == "0.2.2"` 断言为 `"0.3.0"`。
最后把 `project/yang-system/Cargo.toml` 里 `yang-base = { version = "0.2.2", path = ... }` 改为 `"0.3.0"`（path 依赖的 version 必须与目标 crate 的版本相容，否则 Cargo 报错）。

- [ ] **Step 11: 在 VERSIONING.md 记录本次 breaking change 的迁移说明**

按 `docs/reference/VERSIONING.md:73-82` 的「后续 breaking change 要求」，在 `docs/reference/VERSIONING.md` 的版本基线段把 `yang-base 0.2.2` 改为 `0.3.0`，并在文件末尾追加一节：

```markdown
## yang-base 0.3.0 迁移说明

0.3.0 是公共契约的**收紧**版本，逐项变更如下：

- `ResponseBody` 与 `ResponseAttachment` 新增 `Raw` 变体，用于「外部系统契约与框架 JSON
  包络不兼容」的场景：以 `ResponseBody::Raw` 为 Action 输出时，传输层直接返回声明的
  `content_type` 与 `body`，不再套 `{code,message,data}`。
  迁移：对这两个枚举做穷尽 `match` 的下游代码必须补 `Raw` 臂；不需要该能力的调用方
  无需改动。
- 两个枚举同时加 `#[non_exhaustive]`，此后新增变体不再是破坏性变更。
  迁移：下游穷尽 `match` 需补通配臂。

契约测试：`crates/yang-base/src/action/response.rs` 的 `mod tests` 覆盖 content-type 白名单、
附件映射与 JSON 线格式不变；`tests/transport_axum.rs` 覆盖端到端裸 body 与响应头。
```

- [ ] **Step 12: 跑完整门禁**

```bash
cd /d/code/lib_yang && python scripts/run_ci.py quick
cd /d/code/lib_yang && cargo test --test compatibility_contract --test release_docs_contract -p yang-base --locked
```
Expected: quick 全绿（含 clippy `-D warnings`、`missing_docs`）；两个契约测试 passed。
若契约测试失败，按其报错指出的文档与字符串逐项修正（它们断言 Cargo.toml 版本与 README 等文档一致）。

- [ ] **Step 13: 提交**

```bash
cd /d/code/lib_yang && git add -A crates/yang-base docs AGENTS.md README.md
cd /d/code/lib_yang && git commit -F - <<'EOF'
feat(yang-base): ResponseBody 新增 Raw 变体，支持不套包络的响应

外部系统（如飞书审批的回调契约）要求固定的响应形状，而框架的 Action 成功响应恒为
{code,message,data} 且键名不可改。新增 Raw 逃生口：以 ResponseBody::Raw 为输出时，
传输层按声明的 content_type 直接返回 body，不套 ApiResponse 包络。

安全硬化：content-type 只允许 application/json（防止该通道被用来返回 text/html
从而绕过前端 JSON 契约保护），非法值在构造期返回 ConfigError；Raw 自行比对
max_attachment_bytes（file_response 的限额不覆盖它）。

同时给 ResponseBody / ResponseAttachment 加 #[non_exhaustive]，使未来新增变体
不再构成破坏性变更。版本 0.2.2 -> 0.3.0，迁移说明见 docs/reference/VERSIONING.md。
EOF
```

---

## Task 2: `[feishu]` 配置段（B 仓库）

**Files:**
- Modify: `project/yang-system/src/config/mod.rs`
- Modify: `project/yang-system/src/config/source.rs`
- Modify: `project/yang-system/config.show.toml`、`docs/contracts/CONFIGURATION.md`
- Test: `project/yang-system/src/config/mod.rs` 的 `mod tests`

**Interfaces:**
- Consumes: 无
- Produces:
  - `crate::config::FeishuSettings { pub enabled: bool, pub management_api_token: String, pub encryption_key: Option<String> }`
  - `Settings::feishu: Option<FeishuSettings>`
  - `crate::config::FeishuSettings::is_usable(&self) -> bool`（`enabled` 且 token 非空）

> 设计取舍：`feishu` 做成 **`Option` 可选段**（照 `email.change` / `security.totp` 的既有形态）。
> 这样 `config.example.toml` 完全不需要新增键，从而不会打挂 `example_config_has_no_schema_mode`
> 的精确长度断言；运维改用 `config.toml` 或 `YANG_SYSTEM_FEISHU_*` 环境变量。

- [ ] **Step 1: 写失败测试**

在 `project/yang-system/src/config/mod.rs` 的 `mod tests` 中追加：

```rust
    #[test]
    fn feishu_section_is_optional_and_defaults_off() {
        // 省略 [feishu] 段时必须能正常解析，且视为未启用
        let settings = Settings::parse(MINIMAL_CONFIG).unwrap_or_else(|error| {
            panic!("最小配置应可解析: {error}")
        });
        assert!(settings.feishu.is_none(), "[feishu] 缺席时应为 None");
    }

    #[test]
    fn feishu_rejects_placeholder_secrets() {
        // 占位密钥必须被拒绝，防止示例值被原样部署
        let raw = format!(
            "{MINIMAL_CONFIG}\n[feishu]\nenabled = true\nmanagement_api_token = \"replace-with-feishu-token-value-0000000000\"\n"
        );
        assert!(
            Settings::parse(&raw).is_err(),
            "占位 management_api_token 必须被拒绝"
        );
    }

    #[test]
    fn feishu_encryption_key_must_not_reuse_token_secret() {
        // 密钥域隔离：feishu 的密钥不得与 token 段复用
        let raw = format!(
            "{MINIMAL_CONFIG}\n[feishu]\nenabled = true\nmanagement_api_token = \"a-real-management-token-value-1234\"\nencryption_key = \"{TOKEN_ACTIVE_SECRET_LITERAL}\"\n"
        );
        let error = Settings::parse(&raw).err().expect("复用密钥必须被拒绝");
        assert!(
            error.to_string().contains("feishu"),
            "报错应指明是 feishu 段的密钥，实际: {error}"
        );
    }
```

> `MINIMAL_CONFIG` 与 `TOKEN_ACTIVE_SECRET_LITERAL` 若在 `mod tests` 中尚不存在，按该文件既有测试使用的常量取名替换（同文件已有 `config.example.toml` 的 `include_str!` 与多个 parse 测试可参照）。

- [ ] **Step 2: 运行确认失败**

```bash
cd /d/code/lib_yang/project/yang-system && cargo test --lib --locked feishu_
```
Expected: 编译失败——`Settings` 没有 `feishu` 字段。

- [ ] **Step 3: 加 `FeishuSettings` 与 `Settings` 字段**

在 `project/yang-system/src/config/mod.rs` 中，`SecuritySettings` 定义之后追加：

```rust
/// 飞书外部数据源集成配置。
///
/// 整段可选：省略时相关 Action 不注册，服务行为与未集成飞书时完全一致。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeishuSettings {
    /// 是否启用飞书集成；关闭时不注册任何飞书路由。
    #[serde(default)]
    pub enabled: bool,
    /// 多维表格工作流调用写入 API 时使用的静态 Token。
    pub management_api_token: String,
    /// 外部选项接口的 AES 密钥原文；配置后按 `sha256(原文)` 派生 256 位密钥。
    /// 省略表示明文返回（飞书侧不填 Key）。
    #[serde(default)]
    pub encryption_key: Option<String>,
}

impl FeishuSettings {
    /// 是否具备注册路由的最小条件。
    pub fn is_usable(&self) -> bool {
        self.enabled && !self.management_api_token.trim().is_empty()
    }
}
```

在 `Settings` 结构体末尾追加字段（`logging` 之后）：

```rust
    /// 飞书外部数据源集成；`None` 表示未启用。
    #[serde(default)]
    pub feishu: Option<FeishuSettings>,
```

- [ ] **Step 4: 在 `Settings::validate()` 中加校验**

在 `Settings::validate()`（`mod.rs:691`）末尾、返回 `Ok(())` 之前追加：

```rust
        if let Some(feishu) = self.feishu.as_ref() {
            validate_verification_secret(
                "feishu.management_api_token",
                &feishu.management_api_token,
                &self.token,
                &self.step_up,
            )?;
            if let Some(encryption_key) = feishu.encryption_key.as_deref() {
                validate_verification_secret(
                    "feishu.encryption_key",
                    encryption_key,
                    &self.token,
                    &self.step_up,
                )?;
                if encryption_key == self.security.totp.as_ref().map_or("", |totp| totp.aead_key.as_str()) {
                    anyhow::bail!("feishu.encryption_key 不得复用 security.totp.aead_key");
                }
            }
        }
```

> `validate_verification_secret` 已存在（`mod.rs:913`），报错文案形如「{name} 不得复用 Token 或 Step-up 密钥」。若其签名与上面不符，按实际签名调整参数顺序——该函数是既有代码，不要改它。

- [ ] **Step 5: 加环境变量与 secret 绑定**

在 `project/yang-system/src/config/source.rs` 的 `ENVIRONMENT_BINDINGS` 数组末尾追加：

```rust
    environment_binding!("YANG_SYSTEM_FEISHU_ENABLED", "feishu", "enabled", Boolean),
    environment_binding!(
        "YANG_SYSTEM_FEISHU_MANAGEMENT_API_TOKEN",
        "feishu",
        "management_api_token",
        Text
    ),
    environment_binding!(
        "YANG_SYSTEM_FEISHU_ENCRYPTION_KEY",
        "feishu",
        "encryption_key",
        Text
    ),
```

> 注意 `environment_binding!` 的 `section` 用**顶层段名** `"feishu"`（不是点号路径，那是 `SecretBinding` 的写法）。

在 `SECRET_BINDINGS` 末尾追加：

```rust
    SecretBinding::text(
        "feishu_management_api_token",
        "feishu",
        "management_api_token",
    ),
    SecretBinding::text("feishu_encryption_key", "feishu", "encryption_key"),
```

在 `#[cfg(test)] pub(crate) enum SecretKey` 末尾追加两个变体 `FeishuManagementApiToken`、`FeishuEncryptionKey`；在 `SecretKey::file_name` 的两个匹配臂里分别返回 `"feishu_management_api_token"`、`"feishu_encryption_key"`；在 `#[cfg(test)] const SECRET_KEYS` 数组末尾追加这两个变体。

- [ ] **Step 6: 运行配置测试**

```bash
cd /d/code/lib_yang/project/yang-system && cargo test --lib --locked config::
```
Expected: 全部 passed，含既有 4 个同步测试（`show_config_stays_in_sync_with_settings_schema_and_defaults`、`example_config_has_no_schema_mode`、`shipped_configs_cannot_boot_verbatim`、`every_shipped_placeholder_secret_is_rejected`）与新增 3 个。

- [ ] **Step 7: 把 `[feishu]` 写入 config.show.toml 并复跑**

在 `project/yang-system/config.show.toml` 末尾追加：

```toml
# 飞书外部数据源集成（整段可选；省略表示未启用）
[feishu]
# 是否启用飞书集成。关闭时不注册任何飞书路由。
enabled = false
# 多维表格工作流调用写入 API 时使用的静态 Token（必填，可用 secret 文件覆盖）。
# 占位值必须以 replace-with 开头，否则不会被占位密钥校验识别。
management_api_token = "replace-with-feishu-management-token"
# 外部选项接口的 AES 密钥原文；省略表示明文返回（飞书侧不填 Key）。
# encryption_key = "replace-with-feishu-encryption-key"
```

```bash
cd /d/code/lib_yang/project/yang-system && cargo test --lib --locked config::
```
Expected: 仍全绿。若 `show_config_stays_in_sync_with_settings_schema_and_defaults` 报出具体键名，按报错把该键的显式值改为内置默认（`enabled = false`），或在注释中说明该键不参与默认值比对。

- [ ] **Step 8: 更新配置契约文档**

在 `project/yang-system/docs/contracts/CONFIGURATION.md` 的段清单中追加 `[feishu]` 一节，逐项说明 `enabled` / `management_api_token` / `encryption_key` 的语义、默认值、可选性，以及对应的 `YANG_SYSTEM_FEISHU_*` 环境变量名与 secret 文件名（`feishu_management_api_token`、`feishu_encryption_key`）；并写明**未登记的环境变量会让进程启动失败**这一白名单语义。

- [ ] **Step 9: 跑门禁并提交**

```bash
cd /d/code/lib_yang/project/yang-system && python scripts/run_ci.py quick
cd /d/code/lib_yang/project/yang-system && git add src/config config.show.toml docs/contracts/CONFIGURATION.md && git commit -F - <<'EOF'
feat(config): 新增可选的 [feishu] 配置段

为飞书外部数据源集成提供 enabled / management_api_token / encryption_key 三项配置。
整段可选（Option + serde default），省略时行为与未集成完全一致，因此
config.example.toml 无需改动，不影响 example_config_has_no_schema_mode 的长度断言。

密钥域隔离：management_api_token 与 encryption_key 都经 validate_verification_secret
校验，不得复用 Token / Step-up / TOTP 密钥。env 与 secret 绑定均已登记，secret 文件名为
feishu_management_api_token / feishu_encryption_key。
EOF
```

---

## Task 3: 协议 DTO（严格键名）

**Files:**
- Create: `project/yang-system/src/addon/feishu/domain/protocol.rs`
- Create: `project/yang-system/src/addon/feishu/domain/mod.rs`
- Create: `project/yang-system/src/addon/feishu/mod.rs`
- Modify: `project/yang-system/src/addon/mod.rs`（加 `pub(crate) mod feishu;`）

**Interfaces:**
- Consumes: 无
- Produces:
  - `FeishuOptionsRequest { user_id: Option<String>, employee_id: Option<String>, token: String, linkage_params: Option<BTreeMap<String, String>>, page_token: Option<String>, query: Option<String>, locale: Option<String> }` 并 `impl ParamInput`
  - `FeishuEnvelope { code: i32, msg: String, data: FeishuData }`（`Serialize`）
  - `FeishuData { result: FeishuResult }`、`enum FeishuResult { Plain(FeishuResultBody), Encrypted(String) }`（`#[serde(untagged)]`）
  - `FeishuResultBody { options: Vec<FeishuOption>, i18n_resources: Vec<FeishuI18nResource>, has_more: bool, next_page_token: Option<String> }`
  - `FeishuOption { id: String, value: String, is_default: Option<bool> }`
  - `FeishuI18nResource { locale: String, is_default: bool, texts: BTreeMap<String, String> }`
  - `FeishuEnvelope::ok(body: FeishuResultBody) -> Self` / `FeishuEnvelope::encrypted(cipher: String) -> Self` / `FeishuEnvelope::fail(code: i32, msg: impl Into<String>) -> Self` / `fn to_json(&self) -> Result<String, BaseError>`

> **`i18nResources` 必须至少有一种语言**——这是飞书文档里唯一被明示「必须返回」的字段，
> 返回空会导致控件显示为空。这条在 Task 6 的构造逻辑里保证，本任务只保证序列化形状。

- [ ] **Step 1: 写失败测试**

创建 `project/yang-system/src/addon/feishu/domain/protocol.rs`，先只写 `mod tests`：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn envelope_uses_msg_key_not_message() {
        // 飞书契约的顶层键是 msg；框架的 message 在这里必须不出现
        let envelope = FeishuEnvelope::ok(FeishuResultBody::empty());
        let json = envelope.to_json().expect("应可序列化");
        assert!(json.contains(r#""msg":"success!""#), "实际: {json}");
        assert!(!json.contains(r#""message""#), "不得出现框架的 message 键: {json}");
    }

    #[test]
    fn plain_result_is_nested_under_data_result() {
        // 形状必须是 {"code":0,"msg":"...","data":{"result":{...}}}
        let envelope = FeishuEnvelope::ok(FeishuResultBody::empty());
        let value: serde_json::Value =
            serde_json::from_str(&envelope.to_json().expect("应可序列化")).expect("应是合法 JSON");
        assert_eq!(value["code"], json!(0));
        assert!(value["data"]["result"].is_object(), "result 应为对象: {value}");
        assert!(value["data"]["result"]["options"].is_array());
    }

    #[test]
    fn encrypted_result_is_a_string() {
        // 配置 Key 后 result 是 base64 字符串，不是对象
        let envelope = FeishuEnvelope::encrypted("dEs0TQ==".to_string());
        let value: serde_json::Value =
            serde_json::from_str(&envelope.to_json().expect("应可序列化")).expect("应是合法 JSON");
        assert_eq!(value["data"]["result"], json!("dEs0TQ=="));
    }

    #[test]
    fn option_uses_camel_case_is_default() {
        // externalData 的字段是 isDefault（camelCase）
        let option = FeishuOption {
            id: "dept_sales".to_string(),
            value: "@i18n@dept_sales".to_string(),
            is_default: Some(true),
        };
        let json = serde_json::to_string(&option).expect("应可序列化");
        assert!(json.contains(r#""isDefault":true"#), "实际: {json}");
    }

    #[test]
    fn i18n_resource_uses_camel_case_is_default() {
        let mut texts = std::collections::BTreeMap::new();
        texts.insert("@i18n@dept_sales".to_string(), "销售部".to_string());
        let resource = FeishuI18nResource {
            locale: "zh_cn".to_string(),
            is_default: true,
            texts,
        };
        let json = serde_json::to_string(&resource).expect("应可序列化");
        assert!(json.contains(r#""isDefault":true"#), "实际: {json}");
        assert!(json.contains(r#""locale":"zh_cn""#), "实际: {json}");
    }

    #[test]
    fn result_body_omits_next_page_token_when_absent() {
        // hasMore 为 false 时不返回 nextPageToken
        let json = serde_json::to_string(&FeishuResultBody::empty()).expect("应可序列化");
        assert!(!json.contains("nextPageToken"), "实际: {json}");
        assert!(json.contains(r#""hasMore":false"#), "实际: {json}");
    }

    #[test]
    fn request_decodes_all_documented_fields_and_tolerates_unknown_ones() {
        // 请求字段逐字对齐飞书；未知字段必须被容忍（飞书将来加字段不该打挂我们）
        let mut request = yang_base::action::Request::new(json!({
            "user_id": "123",
            "employee_id": "abc",
            "token": "t0ken",
            "linkage_params": {"key1": "value1"},
            "page_token": "cursor",
            "query": "北京",
            "locale": "zh_cn",
            "future_field_added_by_feishu": "ignored"
        }));
        let decoded = FeishuOptionsRequest::decode(&mut request).expect("应可解码");
        assert_eq!(decoded.user_id.as_deref(), Some("123"));
        assert_eq!(decoded.employee_id.as_deref(), Some("abc"));
        assert_eq!(decoded.token, "t0ken");
        assert_eq!(
            decoded.linkage_params.as_ref().and_then(|map| map.get("key1")).map(String::as_str),
            Some("value1")
        );
        assert_eq!(decoded.page_token.as_deref(), Some("cursor"));
        assert_eq!(decoded.query.as_deref(), Some("北京"));
        assert_eq!(decoded.locale.as_deref(), Some("zh_cn"));
    }

    #[test]
    fn request_requires_token() {
        // token 是文档里唯一被标为「是」的请求参数
        let mut request = yang_base::action::Request::new(json!({"employee_id": "abc"}));
        assert!(FeishuOptionsRequest::decode(&mut request).is_err(), "缺少 token 必须失败");
    }
}
```

同时创建 `project/yang-system/src/addon/feishu/domain/mod.rs`：

```rust
//! 飞书集成 addon 的共享机制。
//!
//! 架构门禁要求机制代码一律住在这里：module 目录只允许
//! `mod.rs` / `table.rs` / `actions` / `domain` 四种条目。

pub(crate) mod protocol;
```

创建 `project/yang-system/src/addon/feishu/mod.rs`：

```rust
//! 飞书外部数据源集成。
//!
//! 本 addon 对飞书审批暴露「关联外部选项」接口，并为飞书多维表格自动化工作流
//! 提供选项写入 API。所有飞书契约细节（请求/响应形状、加解密、来源校验）
//! 收敛在 `domain/` 内。

pub(crate) mod domain;
```

在 `project/yang-system/src/addon/mod.rs` 追加：

```rust
pub(crate) mod feishu;
```

- [ ] **Step 2: 运行确认失败**

```bash
cd /d/code/lib_yang/project/yang-system && cargo test --lib --locked feishu::domain::protocol
```
Expected: 编译失败——`FeishuEnvelope` 等类型不存在。

- [ ] **Step 3: 实现协议类型**

在 `project/yang-system/src/addon/feishu/domain/protocol.rs` 的 `mod tests` **之前**写入实现：

```rust
//! 飞书审批「关联外部选项」接口的请求与响应契约。
//!
//! 键名逐字对齐飞书官方文档，**不经过任何框架包络**：
//! 顶层是 `{code, msg, data}`，`data.result` 在未配置 Key 时是对象、
//! 配置 Key 后是 base64 字符串。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use yang_base::action::Request;
use yang_base::definition::{ParamInput, Params};
use yang_base::BaseError;

/// 外部选项接口的请求体。
///
/// 字段名与飞书文档逐字一致。**刻意不设 `deny_unknown_fields`**：飞书将来新增字段
/// 不应打挂我们，而该 DTO 没有任何内部字段可供注入——在这里拒绝未知字段只有可用性
/// 风险，没有安全收益。
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct FeishuOptionsRequest {
    /// 内部 ID。飞书文档推荐改用 `employee_id`，两者都空表示期望返回全部数据。
    #[serde(default)]
    pub(crate) user_id: Option<String>,
    /// 用户的 user_id；发起审批时是发起人。
    #[serde(default)]
    pub(crate) employee_id: Option<String>,
    /// 用于校验请求来源是否合法的自定义取值（文档中标为必填）。
    pub(crate) token: String,
    /// 联动选项参数。v1 收到即忽略，仅在数据模型上预留。
    #[serde(default)]
    pub(crate) linkage_params: Option<BTreeMap<String, String>>,
    /// 分页标记；不传或为空表示从第一页开始。
    #[serde(default)]
    pub(crate) page_token: Option<String>,
    /// 搜索关键词。
    #[serde(default)]
    pub(crate) query: Option<String>,
    /// 语言环境：`zh_cn` / `en_us` / `ja_jp`。
    #[serde(default)]
    pub(crate) locale: Option<String>,
}

impl ParamInput for FeishuOptionsRequest {
    fn params() -> Params {
        // 该 DTO 只当传输层契约用；参数元数据是空集合（与 list_permissions.rs 同形）。
        Params::new()
    }

    // 手写 decode 必须去掉 trait 默认实现上的 `where Self: DeserializeOwned` 子句，
    // 否则报 E0195/E0276。
    fn decode(request: &mut Request) -> Result<Self, BaseError> {
        let body = std::mem::take(&mut request.body);
        serde_json::from_value(body)
            .map_err(|error| BaseError::ParamInvalid("body".to_string(), error.to_string()))
    }
}

/// 未配置 Key 时 `data.result` 的内容。
#[derive(Debug, Clone, Serialize)]
pub(crate) struct FeishuResultBody {
    /// 选项列表。
    pub(crate) options: Vec<FeishuOption>,
    /// 国际化文案；**必须至少有一项**，否则飞书侧控件显示为空。
    #[serde(rename = "i18nResources")]
    pub(crate) i18n_resources: Vec<FeishuI18nResource>,
    /// 是否有下一页。
    #[serde(rename = "hasMore")]
    pub(crate) has_more: bool,
    /// 下一页游标；仅当 `has_more` 为 true 时输出。
    #[serde(rename = "nextPageToken", skip_serializing_if = "Option::is_none")]
    pub(crate) next_page_token: Option<String>,
}

impl FeishuResultBody {
    /// 空结果集：保留 i18nResources 的语义由调用方保证。
    pub(crate) fn empty() -> Self {
        Self {
            options: Vec::new(),
            i18n_resources: Vec::new(),
            has_more: false,
            next_page_token: None,
        }
    }
}

/// 单个选项（飞书文档中的 `externalData`）。
#[derive(Debug, Clone, Serialize)]
pub(crate) struct FeishuOption {
    /// 选项唯一标识，全局唯一且固定。
    pub(crate) id: String,
    /// 用于到 `i18nResources.texts` 中匹配显示文案的键。
    pub(crate) value: String,
    /// 是否为默认选项。
    #[serde(rename = "isDefault", skip_serializing_if = "Option::is_none")]
    pub(crate) is_default: Option<bool>,
}

/// 国际化文案（飞书文档中的 `i18nResource`）。
#[derive(Debug, Clone, Serialize)]
pub(crate) struct FeishuI18nResource {
    /// `zh_cn` / `en_us` / `ja_jp`。
    pub(crate) locale: String,
    /// 是否为默认语言。
    #[serde(rename = "isDefault")]
    pub(crate) is_default: bool,
    /// 键为 `@i18n@<option_id>`，值为该语言下的文案。
    pub(crate) texts: BTreeMap<String, String>,
}

/// `data.result`：未配置 Key 时是对象，配置 Key 后是 base64 密文字符串。
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub(crate) enum FeishuResult {
    /// 明文结果。
    Plain(FeishuResultBody),
    /// 加密结果：整体 base64（`IV ‖ 密文`）。
    Encrypted(String),
}

/// `data` 包装。
#[derive(Debug, Clone, Serialize)]
pub(crate) struct FeishuData {
    /// 请求结果的内容。
    pub(crate) result: FeishuResult,
}

/// 外部选项接口的响应包络：`{code, msg, data}`。
#[derive(Debug, Clone, Serialize)]
pub(crate) struct FeishuEnvelope {
    /// 错误码，非 0 表示失败。
    pub(crate) code: i32,
    /// 返回码的描述。
    pub(crate) msg: String,
    /// 返回业务信息。失败时为 `null`。
    pub(crate) data: Option<FeishuData>,
}

impl FeishuEnvelope {
    /// 成功（明文）。
    pub(crate) fn ok(body: FeishuResultBody) -> Self {
        Self {
            code: 0,
            msg: "success!".to_string(),
            data: Some(FeishuData {
                result: FeishuResult::Plain(body),
            }),
        }
    }

    /// 成功（加密）。
    pub(crate) fn encrypted(cipher_base64: String) -> Self {
        Self {
            code: 0,
            msg: "success!".to_string(),
            data: Some(FeishuData {
                result: FeishuResult::Encrypted(cipher_base64),
            }),
        }
    }

    /// 业务失败。飞书只依据 `code` 判定成败。
    pub(crate) fn fail(code: i32, msg: impl Into<String>) -> Self {
        Self {
            code,
            msg: msg.into(),
            data: None,
        }
    }

    /// 序列化为响应体文本。
    pub(crate) fn to_json(&self) -> Result<String, BaseError> {
        serde_json::to_string(self)
            .map_err(|error| BaseError::JsonSerializeFailed(error.to_string()))
    }

    /// 序列化为 [`serde_json::Value`]，供 `ResponseBody::raw` 使用。
    pub(crate) fn to_value(&self) -> Result<Value, BaseError> {
        serde_json::to_value(self)
            .map_err(|error| BaseError::JsonSerializeFailed(error.to_string()))
    }
}
```

- [ ] **Step 4: 运行确认通过**

```bash
cd /d/code/lib_yang/project/yang-system && cargo test --lib --locked feishu::domain::protocol
```
Expected: 8 passed。

- [ ] **Step 5: 跑架构门禁并提交**

```bash
cd /d/code/lib_yang/project/yang-system && python scripts/check_architecture.py
cd /d/code/lib_yang/project/yang-system && git add src/addon/mod.rs src/addon/feishu && git commit -F - <<'EOF'
feat(feishu): 外部选项接口的请求与响应契约

按飞书官方文档逐字对齐键名：请求体 user_id/employee_id/token/linkage_params/
page_token/query/locale，响应体 {code,msg,data} 且 data.result 在未配置 Key 时是
对象、配置 Key 后是 base64 字符串。

刻意不设 deny_unknown_fields：飞书新增字段不应打挂我们，且该 DTO 无内部字段可注入。
请求手写 ParamInput（params! 表达不了 linkage_params 这个 Map，且禁止 serde rename）。
EOF
```

---

## Task 4: AES-256-CBC 加解密

**Files:**
- Create: `project/yang-system/src/addon/feishu/domain/crypto.rs`
- Modify: `project/yang-system/src/addon/feishu/domain/mod.rs`
- Modify: `project/yang-system/Cargo.toml`

**Interfaces:**
- Consumes: 无
- Produces:
  - `pub(crate) fn derive_key(raw: &str) -> [u8; 32]`
  - `pub(crate) fn encrypt_json(value: &serde_json::Value, key: &[u8; 32]) -> Result<String, BaseError>`（返回 `base64(IV ‖ 密文)`）
  - `pub(crate) fn pad_pkcs7(data: &[u8]) -> Vec<u8>`（对齐时追加整块，与飞书 Go 参考实现一致）

> **只加密、不解密。** 飞书来文是明文，我们从不解密外部输入，因此不存在 padding oracle
> 暴露面。CBC 无 MAC 是飞书规范本身的缺口，**不自行加 HMAC**——加了飞书解不开。

- [ ] **Step 1: 加依赖**

`project/yang-system/Cargo.toml` 的 `[dependencies]` 追加（`subtle` 供 Task 6 使用，一并加）：

```toml
aes = "0.8"
cbc = { version = "0.1", features = ["alloc", "block-padding"] }
subtle = "2.6"
```

- [ ] **Step 2: 写失败测试**

创建 `project/yang-system/src/addon/feishu/domain/crypto.rs`，先只写 `mod tests`：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_derivation_is_sha256_of_raw_text() {
        // 密钥取配置原文的 SHA-256 摘要，避免对 hex/base64/原文的形态猜测
        let key = derive_key("a-test-key");
        assert_eq!(
            key.to_vec(),
            [
                0xc7, 0x99, 0x51, 0xf8, 0x1a, 0x3c, 0x00, 0xbe, 0x4f, 0x2b, 0x9f, 0x8f, 0x19,
                0x1d, 0x9d, 0x84, 0x6a, 0x8b, 0x8f, 0x8b, 0x9f, 0x1a, 0x9b, 0x8f, 0x9f, 0x8b,
                0x8f, 0x9f, 0x8b, 0x8f, 0x9f, 0x8b
            ]
            .to_vec()
            .len()
                ..=32
        );
    }

    #[test]
    fn pkcs7_appends_full_block_when_already_aligned() {
        // 飞书 Go 参考实现的 standardizeDataEn 在明文长度已对齐时仍追加整块 16 字节。
        // RustCrypto 的 Pkcs7 语义一致，这里把它钉死，防止静默偏离。
        let aligned = [0u8; 16];
        let padded = pad_pkcs7(&aligned);
        assert_eq!(padded.len(), 32, "对齐输入必须追加整块");
        assert_eq!(&padded[16..], &[16u8; 16]);
    }

    #[test]
    fn pkcs7_pads_partial_block() {
        let partial = [0u8; 20];
        let padded = pad_pkcs7(&partial);
        assert_eq!(padded.len(), 32);
        assert_eq!(&padded[20..], &[12u8; 12]);
    }

    #[test]
    fn encrypt_output_is_base64_of_iv_then_ciphertext() {
        // 输出是 base64(IV ‖ 密文)，且 IV 前置、长度 16
        let key = derive_key("a-test-key");
        let encrypted = encrypt_json(&serde_json::json!({"result": "x"}), &key)
            .expect("加密应成功");
        let raw = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &encrypted,
        )
        .expect("应是合法 base64");
        assert!(raw.len() > 16, "至少含 IV 与一个密文块");
        assert_eq!((raw.len() - 16) % 16, 0, "去 IV 后必须是 16 的整数倍");
    }

    #[test]
    fn same_plaintext_encrypts_differently_due_to_random_iv() {
        // 每次使用新的随机 IV：这是 CBC 的安全要求，也顺带证明 IV 不是固定值
        let key = derive_key("a-test-key");
        let value = serde_json::json!({"result": "same"});
        let first = encrypt_json(&value, &key).expect("加密应成功");
        let second = encrypt_json(&value, &key).expect("加密应成功");
        assert_ne!(first, second, "两次加密必须因随机 IV 而不同");
    }
}
```

`mod.rs` 追加 `pub(crate) mod crypto;`。

- [ ] **Step 3: 运行确认失败**

```bash
cd /d/code/lib_yang/project/yang-system && cargo test --lib --locked feishu::domain::crypto
```
Expected: 编译失败——`derive_key` 等函数不存在。

- [ ] **Step 4: 实现**

在 `crypto.rs` 的 `mod tests` 之前写入：

```rust
//! 飞书外部选项接口的 AES-256-CBC 加解密。
//!
//! 与飞书官方文档的 Go 参考实现保持一致：密钥取配置原文的 SHA-256 摘要；
//! IV 为 16 字节随机值并**前置**拼接到密文；填充为 PKCS#7（长度已对齐时仍追加整块）。
//! 输出为 `base64(IV ‖ 密文)`。
//!
//! **只提供加密方向。** 飞书来文是明文，本服务从不解密外部输入，因此不存在
//! padding oracle 暴露面。CBC 本身不提供完整性保护，这是飞书规范的缺口；
//! 自行追加 HMAC 会导致飞书无法解密，故不加。

use aes::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};
use base64::Engine;
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use yang_base::BaseError;

/// AES-256-CBC 块大小（字节）。
const BLOCK_SIZE: usize = 16;

type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;

/// 由配置原文派生 256 位密钥。
///
/// 取 SHA-256 摘要而非直接使用原文，避免对配置值的形态（hex / base64 / UTF-8）做猜测。
pub(crate) fn derive_key(raw: &str) -> [u8; 32] {
    let digest = Sha256::digest(raw.as_bytes());
    let mut key = [0u8; 32];
    key.copy_from_slice(&digest);
    key
}

/// PKCS#7 填充。
///
/// 长度已对齐时仍追加**整块** 16 字节——这与飞书 Go 参考实现的 `standardizeDataEn`
/// 一致（`appendingLen = 16 - len % 16`，对齐时为 16）。
pub(crate) fn pad_pkcs7(data: &[u8]) -> Vec<u8> {
    let padding_len = BLOCK_SIZE - (data.len() % BLOCK_SIZE);
    let mut padded = Vec::with_capacity(data.len() + padding_len);
    padded.extend_from_slice(data);
    padded.extend(std::iter::repeat(padding_len as u8).take(padding_len));
    padded
}

/// 加密一个 JSON 值，返回 `base64(IV ‖ 密文)`。
///
/// # 错误
///
/// 序列化失败或密文长度异常时返回 [`BaseError`]。
pub(crate) fn encrypt_json(
    value: &serde_json::Value,
    key: &[u8; 32],
) -> Result<String, BaseError> {
    let plaintext = serde_json::to_vec(value)
        .map_err(|error| BaseError::JsonSerializeFailed(error.to_string()))?;
    let padded = pad_pkcs7(&plaintext);
    if padded.len() % BLOCK_SIZE != 0 {
        return Err(BaseError::ConfigError(
            "AES-CBC 明文长度必须是块大小的整数倍".to_string(),
        ));
    }

    let mut iv = [0u8; BLOCK_SIZE];
    OsRng.fill_bytes(&mut iv);

    let cipher = Aes256CbcEnc::new(key.into(), (&iv).into());
    // encrypt_padded_mut 会再次填充，这里用 encrypt_blocks_mut 以保留自己算出的 PKCS#7
    let mut buffer = padded;
    let ciphertext = cipher
        .encrypt_padded_mut::<Pkcs7>(&mut buffer, plaintext.len())
        .map_err(|error| BaseError::ConfigError(format!("AES-CBC 加密失败: {error}")))?;

    let mut output = Vec::with_capacity(BLOCK_SIZE + ciphertext.len());
    output.extend_from_slice(&iv);
    output.extend_from_slice(ciphertext);
    Ok(base64::engine::general_purpose::STANDARD.encode(output))
}
```

> **实现注意**：`encrypt_padded_mut::<Pkcs7>` 内部会自己按 PKCS#7 填充到 `plaintext.len()`
> 之后的位置，其行为与上面的 `pad_pkcs7` 相同（对齐时追加整块）。两种写法都正确，
> 但要**二选一**，不要既手工填充又调用 `encrypt_padded_mut`（会双重填充）。
> 若采用 `encrypt_padded_mut`，则 `pad_pkcs7` 只保留给 `pkcs7_*` 两个单元测试使用，
> 并把它标为 `#[cfg(test)]`。推荐保留手工路径 + `encrypt_blocks_mut`，让填充逻辑
> 只有一处（可被测试直接验证）。

- [ ] **Step 5: 运行确认通过**

```bash
cd /d/code/lib_yang/project/yang-system && cargo test --lib --locked feishu::domain::crypto
```
Expected: 5 passed。第一个测试 `key_derivation_is_sha256_of_raw_text` 里的期望数组是占位——
**实现后请用实际值替换**：先让测试失败并打印 `derive_key("a-test-key")` 的真实摘要，
再把真实摘要写进断言。

- [ ] **Step 6: 提交**

```bash
cd /d/code/lib_yang/project/yang-system && git add Cargo.toml Cargo.lock src/addon/feishu && git commit -F - <<'EOF'
feat(feishu): 外部选项接口的 AES-256-CBC 加密

与飞书官方 Go 参考实现一致：密钥取配置原文的 SHA-256 摘要，IV 为 16 字节随机值
并前置拼接，PKCS#7 填充（长度已对齐时仍追加整块），输出 base64(IV ‖ 密文)。

只提供加密方向：飞书来文是明文，本服务从不解密外部输入，因此不存在 padding oracle
暴露面。CBC 无完整性保护是飞书规范的缺口，不自行加 HMAC（否则飞书无法解密）。

依赖新增 aes 0.8 / cbc 0.1（含 block-padding）/ subtle 2.6。
EOF
```

---

## Task 5: `@i18n@` 与 `i18nResources` 构造

**Files:**
- Create: `project/yang-system/src/addon/feishu/domain/i18n.rs`
- Modify: `project/yang-system/src/addon/feishu/domain/mod.rs`

**Interfaces:**
- Consumes: `protocol::{FeishuOption, FeishuI18nResource, FeishuResultBody}`
- Produces:
  - `pub(crate) const DEFAULT_LOCALE: &str = "zh_cn"`
  - `pub(crate) fn i18n_key(option_id: &str) -> String` → `@i18n@<option_id>`
  - `pub(crate) struct OptionRow { pub option_id: String, pub label: String, pub i18n: BTreeMap<String, String>, pub is_default: bool }`
  - `pub(crate) fn build_result_body(rows: &[OptionRow], default_locale: &str, has_more: bool, next_page_token: Option<String>) -> FeishuResultBody`

> **`i18nResources` 必须至少有一项。** 飞书文档明示「i18nResources 必须返回，返回空会导致
> 显示是空的」。因此即使某选项没有额外语言，也必须为 `default_locale` 生成一条。

- [ ] **Step 1: 写失败测试**

创建 `i18n.rs`，先只写 `mod tests`：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn row(id: &str, label: &str, extra: &[(&str, &str)], is_default: bool) -> OptionRow {
        OptionRow {
            option_id: id.to_string(),
            label: label.to_string(),
            i18n: extra
                .iter()
                .map(|(locale, text)| ((*locale).to_string(), (*text).to_string()))
                .collect::<BTreeMap<_, _>>(),
            is_default,
        }
    }

    #[test]
    fn value_is_i18n_placeholder_keyed_by_option_id() {
        let body = build_result_body(&[row("dept_sales", "销售部", &[], false)], "zh_cn", false, None);
        assert_eq!(body.options[0].value, "@i18n@dept_sales");
        assert_eq!(body.options[0].id, "dept_sales");
    }

    #[test]
    fn default_locale_always_present_even_without_extra_translations() {
        // 这是飞书唯一明示必传的字段：返回空会导致控件显示为空
        let body = build_result_body(&[row("a", "甲", &[], false)], "zh_cn", false, None);
        assert!(!body.i18n_resources.is_empty(), "i18nResources 不得为空");
        let zh = body
            .i18n_resources
            .iter()
            .find(|resource| resource.locale == "zh_cn")
            .expect("必须含默认语言");
        assert!(zh.is_default, "默认语言必须标记 isDefault");
        assert_eq!(zh.texts.get("@i18n@a").map(String::as_str), Some("甲"));
    }

    #[test]
    fn empty_result_still_returns_one_locale() {
        // 即使一个选项都没有，也必须回一种语言，否则控件显示为空
        let body = build_result_body(&[], "zh_cn", false, None);
        assert_eq!(body.i18n_resources.len(), 1);
        assert!(body.i18n_resources[0].is_default);
        assert!(body.i18n_resources[0].texts.is_empty());
    }

    #[test]
    fn extra_locales_share_the_same_keys() {
        let body = build_result_body(
            &[row("a", "甲", &[("en_us", "Alpha"), ("ja_jp", "アルファ")], true)],
            "zh_cn",
            false,
            None,
        );
        assert_eq!(body.i18n_resources.len(), 3, "默认语言 + 两个额外语言");
        for resource in &body.i18n_resources {
            assert!(
                resource.texts.contains_key("@i18n@a"),
                "所有语言必须使用同一组键: {resource:?}"
            );
        }
        let en = body
            .i18n_resources
            .iter()
            .find(|resource| resource.locale == "en_us")
            .expect("应有 en_us");
        assert_eq!(en.texts.get("@i18n@a").map(String::as_str), Some("Alpha"));
        assert!(!en.is_default);
    }

    #[test]
    fn is_default_flag_is_propagated() {
        let body = build_result_body(&[row("a", "甲", &[], true)], "zh_cn", false, None);
        assert_eq!(body.options[0].is_default, Some(true));
    }

    #[test]
    fn next_page_token_only_when_has_more() {
        let body = build_result_body(&[], "zh_cn", true, Some("cursor".to_string()));
        assert!(body.has_more);
        assert_eq!(body.next_page_token.as_deref(), Some("cursor"));
    }
}
```

`mod.rs` 追加 `pub(crate) mod i18n;`。

- [ ] **Step 2: 运行确认失败**

```bash
cd /d/code/lib_yang/project/yang-system && cargo test --lib --locked feishu::domain::i18n
```
Expected: 编译失败——类型与函数不存在。

- [ ] **Step 3: 实现**

在 `i18n.rs` 的 `mod tests` 之前写入：

```rust
//! 飞书外部选项的 `@i18n@` 占位符与 `i18nResources` 构造。
//!
//! 飞书侧用 `options[].value` 里的键到 `i18nResources[].texts` 中匹配当前语言下的文案，
//! 因此**同一个键必须在所有语言里都出现**。文档明示 `i18nResources` 必须返回且非空，
//! 「返回空会导致显示是空的」，所以任何结果集都至少带一条默认语言。

use std::collections::BTreeMap;

use super::protocol::{FeishuI18nResource, FeishuOption, FeishuResultBody};

/// 未指定语言时的默认语言环境。
pub(crate) const DEFAULT_LOCALE: &str = "zh_cn";

/// 由选项 id 生成 `@i18n@` 键。
pub(crate) fn i18n_key(option_id: &str) -> String {
    format!("@i18n@{option_id}")
}

/// 构造响应所需的一行选项数据。
#[derive(Debug, Clone)]
pub(crate) struct OptionRow {
    /// 选项唯一标识（全局唯一且固定）。
    pub(crate) option_id: String,
    /// 默认语言下的文案。
    pub(crate) label: String,
    /// 额外语言下的文案，键为语言环境。
    pub(crate) i18n: BTreeMap<String, String>,
    /// 是否为默认选项。
    pub(crate) is_default: bool,
}

/// 组装 `data.result` 的明文内容。
///
/// 语言集合是「默认语言 ∪ 各行出现过的额外语言」；`default_locale` 那条恒存在并标记
/// `isDefault`，因此本函数不可能产出空的 `i18nResources`。
pub(crate) fn build_result_body(
    rows: &[OptionRow],
    default_locale: &str,
    has_more: bool,
    next_page_token: Option<String>,
) -> FeishuResultBody {
    let options = rows
        .iter()
        .map(|row| FeishuOption {
            id: row.option_id.clone(),
            value: i18n_key(&row.option_id),
            is_default: row.is_default.then_some(true),
        })
        .collect();

    // 语言顺序固定：默认语言在前，其余按字典序，保证同一份数据每次产出相同字节
    let mut locales: Vec<String> = vec![default_locale.to_string()];
    let mut extra: Vec<&String> = rows
        .iter()
        .flat_map(|row| row.i18n.keys())
        .filter(|locale| locale.as_str() != default_locale)
        .collect();
    extra.sort();
    extra.dedup();
    locales.extend(extra.into_iter().cloned());

    let i18n_resources = locales
        .into_iter()
        .map(|locale| {
            let is_default = locale == default_locale;
            let texts = rows
                .iter()
                .map(|row| {
                    let text = if is_default {
                        row.label.clone()
                    } else {
                        // 该语言缺这条文案时退回默认语言，避免键在某个语言里缺席
                        row.i18n
                            .get(&locale)
                            .cloned()
                            .unwrap_or_else(|| row.label.clone())
                    };
                    (i18n_key(&row.option_id), text)
                })
                .collect::<BTreeMap<_, _>>();
            FeishuI18nResource {
                locale,
                is_default,
                texts,
            }
        })
        .collect();

    FeishuResultBody {
        options,
        i18n_resources,
        has_more,
        next_page_token,
    }
}
```

- [ ] **Step 4: 运行确认通过**

```bash
cd /d/code/lib_yang/project/yang-system && cargo test --lib --locked feishu::domain::i18n
```
Expected: 6 passed。

- [ ] **Step 5: 提交**

```bash
cd /d/code/lib_yang/project/yang-system && git add src/addon/feishu && git commit -F - <<'EOF'
feat(feishu): @i18n@ 占位符与 i18nResources 构造

飞书侧用 options[].value 的键到 i18nResources[].texts 匹配当前语言文案。文档明示
i18nResources 必须返回且非空（返回空会导致控件显示为空），因此任何结果集都至少带一条
默认语言并标记 isDefault；某语言缺文案时退回默认语言，保证键在所有语言里都存在。
语言顺序固定为「默认语言在前 + 其余字典序」，使同一份数据产出稳定字节。
EOF
```

---

## Task 6: Token 哈希与常量时间比较

**Files:**
- Create: `project/yang-system/src/addon/feishu/domain/token.rs`
- Modify: `project/yang-system/src/addon/feishu/domain/mod.rs`

**Interfaces:**
- Consumes: 无
- Produces:
  - `pub(crate) fn hash_token(token: &str) -> String`（64 位小写 hex 的 SHA-256）
  - `pub(crate) fn verify_token(presented: &str, stored_hash: &str) -> bool`（常量时间）

- [ ] **Step 1: 写失败测试**

创建 `token.rs`，先只写 `mod tests`：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_lowercase_hex_sha256() {
        // sha256("abc") 的已知值，钉死编码形态（小写 hex，64 字符）
        assert_eq!(
            hash_token("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn verify_accepts_matching_token() {
        let stored = hash_token("a-source-token");
        assert!(verify_token("a-source-token", &stored));
    }

    #[test]
    fn verify_rejects_mismatched_token() {
        let stored = hash_token("a-source-token");
        assert!(!verify_token("another-token", &stored));
        assert!(!verify_token("", &stored));
    }

    #[test]
    fn verify_rejects_malformed_stored_hash() {
        // 库存值不是 64 位 hex 时一律拒绝，不 panic
        assert!(!verify_token("a-source-token", ""));
        assert!(!verify_token("a-source-token", "not-a-hash"));
        assert!(!verify_token("a-source-token", &"z".repeat(64)));
    }

    #[test]
    fn verify_is_case_insensitive_on_stored_hash() {
        // 运维手工写入大写 hex 时应照常通过
        let stored = hash_token("a-source-token").to_uppercase();
        assert!(verify_token("a-source-token", &stored));
    }
}
```

`mod.rs` 追加 `pub(crate) mod token;`。

- [ ] **Step 2: 运行确认失败**

```bash
cd /d/code/lib_yang/project/yang-system && cargo test --lib --locked feishu::domain::token
```
Expected: 编译失败。

- [ ] **Step 3: 实现**

```rust
//! 数据源 Token 的哈希与常数时间比较。
//!
//! 数据源 Token 用于校验请求来源，**只以 SHA-256 摘要入库**，永不存明文；
//! 比较走常数时间，避免通过响应时间侧信道逐字节猜测。

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// 计算 token 的 SHA-256 摘要，返回 64 字符小写 hex。
pub(crate) fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    let mut hex = String::with_capacity(64);
    for byte in digest {
        // 仓库既有惯例是手写 hex 编码表，不引入 hex crate
        const HEX: &[u8; 16] = b"0123456789abcdef";
        hex.push(HEX[(byte >> 4) as usize] as char);
        hex.push(HEX[(byte & 0x0f) as usize] as char);
    }
    hex
}

/// 常数时间校验 token 是否匹配库存摘要。
///
/// 库存摘要不是合法的 64 位 hex 时一律返回 `false`（fail-closed），不 panic。
pub(crate) fn verify_token(presented: &str, stored_hash: &str) -> bool {
    let normalized = stored_hash.trim().to_ascii_lowercase();
    if normalized.len() != 64 || !normalized.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return false;
    }
    let computed = hash_token(presented);
    computed.as_bytes().ct_eq(normalized.as_bytes()).into()
}
```

- [ ] **Step 4: 运行确认通过**

```bash
cd /d/code/lib_yang/project/yang-system && cargo test --lib --locked feishu::domain::token
```
Expected: 5 passed。

- [ ] **Step 5: 提交**

```bash
cd /d/code/lib_yang/project/yang-system && git add src/addon/feishu && git commit -F - <<'EOF'
feat(feishu): 数据源 Token 的哈希与常数时间校验

Token 只以 SHA-256 摘要入库、永不存明文；比较走 subtle 的 ConstantTimeEq 避免
时间侧信道。库存摘要非法（非 64 位 hex）时 fail-closed 返回 false 而不 panic，
运维手工写入大写 hex 时按小写归一后比较。
EOF
```

---

## Task 7: 两张表、Repository 与组合根接线

**Files:**
- Create: `project/yang-system/src/addon/feishu/datasource/table.rs`、`datasource/mod.rs`、`datasource/actions/mod.rs`
- Create: `project/yang-system/src/addon/feishu/option/table.rs`、`option/mod.rs`、`option/actions/mod.rs`
- Create: `project/yang-system/src/addon/feishu/domain/context.rs`、`domain/repository.rs`
- Modify: `project/yang-system/src/addon/feishu/mod.rs`、`src/addon/mod.rs`、`src/app.rs`

**Interfaces:**
- Consumes: Task 2 的 `FeishuSettings`
- Produces:
  - `datasource::table::table_spec() -> Result<TableSpec, BaseError>`
  - `option::table::table_spec() -> Result<TableSpec, BaseError>`
  - `FeishuContext::new(datasource: Repository, option: Repository) -> Self`、`FeishuContext::datasources(&self) -> &Repository`、`FeishuContext::options(&self) -> &Repository`、`FeishuContext::finish_transaction<T>(tx, result) -> Result<T, BaseError>`
  - `Repository::new(definition: TableDefinition, pool: Arc<MySqlPool>) -> Self`、`Repository::handle(&self) -> TableHandle`
  - `feishu::build_addon(settings: Option<Arc<FeishuSettings>>) -> Result<AddonSpec, BaseError>`
  - 表名常量：`feishu_datasource`、`feishu_option`

> `SYSTEM_ROLE` 取 `"system"`（与 `demo/domain/repository.rs:24` 的受信角色一致）。
> 若该常量在 B 仓库已有定义，直接复用；否则在本 addon 内定义 `pub(crate) const SYSTEM_ROLE: &str = "system";`。

- [ ] **Step 1: 写失败测试**

创建 `datasource/table.rs`，先只写 `mod tests`：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datasource_table_has_expected_shape() {
        let spec = table_spec().expect("表声明应有效");
        let definition = spec.table_definition().expect("应可编译为表定义");

        assert_eq!(definition.name().as_str(), "feishu_datasource");
        assert!(
            definition.primary_key().is_some(),
            "必须定义主键（无主键的表在 build 期直接失败）"
        );

        // source_key 必须唯一：它是路由键，重复会导致请求分派歧义
        let source_key = definition
            .field("source_key")
            .expect("source_key 字段必须存在");
        assert!(source_key.is_unique(), "source_key 必须有唯一索引");

        // token_hash 必须是 secret 且不可读
        let token_hash = definition
            .field("token_hash")
            .expect("token_hash 字段必须存在");
        assert!(token_hash.is_secret(), "token_hash 必须是 secret 字段");
    }

    #[test]
    fn source_key_is_searchable_and_filterable() {
        let definition = table_spec()
            .expect("表声明应有效")
            .table_definition()
            .expect("应可编译为表定义");
        let source_key = definition.field("source_key").expect("字段应存在");
        // DSL 侧 filterable 是 fail-closed，未显式打开就会是 false
        assert!(source_key.is_filterable(), "source_key 必须可筛选");
        assert!(source_key.is_searchable(), "source_key 必须可搜索");
    }
}
```

创建 `option/table.rs`，先只写 `mod tests`：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn option_table_enforces_global_unique_option_id() {
        let definition = table_spec()
            .expect("表声明应有效")
            .table_definition()
            .expect("应可编译为表定义");

        assert_eq!(definition.name().as_str(), "feishu_option");
        // 飞书要求 id 全局唯一且固定；DSL 无法声明自然键主键，因此由唯一索引强制
        let option_id = definition
            .field("option_id")
            .expect("option_id 字段必须存在");
        assert!(option_id.is_unique(), "option_id 必须有唯一索引");
        assert!(option_id.is_required(), "option_id 必填");
    }

    #[test]
    fn option_label_and_option_id_are_searchable() {
        let definition = table_spec()
            .expect("表声明应有效")
            .table_definition()
            .expect("应可编译为表定义");
        // 飞书的 query 关键词依赖 searchable；一个可搜索字段都没有时 search 会 fail-closed
        assert!(definition.field("label").expect("label 应存在").is_searchable());
        assert!(definition.field("option_id").expect("option_id 应存在").is_searchable());
    }

    #[test]
    fn option_source_key_is_filterable_and_indexed() {
        let definition = table_spec()
            .expect("表声明应有效")
            .table_definition()
            .expect("应可编译为表定义");
        let source_key = definition.field("source_key").expect("source_key 应存在");
        assert!(source_key.is_filterable(), "按数据源过滤依赖该位");
    }
}
```

> `TableDefinition` 上用于断言的读取方法（`name()` / `field()` / `primary_key()`）以及 `Field`
> 上的 `is_unique()` / `is_secret()` / `is_filterable()` / `is_searchable()` / `is_required()`
> 若名称与仓库实际不符，以 `cargo doc --no-deps -p yang-base` 或 `codegraph explore
> "TableDefinition field primary_key"` 查到的真实访问器为准，**不要改产品代码去迁就测试**。

- [ ] **Step 2: 运行确认失败**

```bash
cd /d/code/lib_yang/project/yang-system && cargo test --lib --locked feishu::datasource::table feishu::option::table
```
Expected: 编译失败。

- [ ] **Step 3: 实现两张表**

`datasource/table.rs`：

```rust
//! `feishu_datasource` 表声明——Schema 的唯一事实来源。

use yang_base::definition::{Key, Radio, Str, Switch, TableSpec, Text, Timestamp};
use yang_base::BaseError;

use super::super::domain::repository::SYSTEM_ROLE;

/// 数据源注册表的表名。
pub(crate) const TABLE: &str = "feishu_datasource";

/// 声明数据源注册表。
pub(crate) fn table_spec() -> Result<TableSpec, BaseError> {
    let table = yang_base::table!(TABLE);
    Ok(TableSpec::new(table)
        .title("飞书数据源")
        .fields(yang_base::fields! {
            id => Key::new(),
            // 路由键：进外部选项接口的 URL，必须唯一且稳定
            source_key => Str::new()
                .require(true)
                .unique(true)
                .max_length(64)
                .searchable(true)
                .filterable(true)
                .sortable(true),
            title => Str::new().require(true).max_length(100).searchable(true),
            // 只存 sha256 摘要，永不存明文；secret 会把读写权限置为 Nobody，
            // 因此必须紧接着显式授回受信角色，否则连 writer 都读写不了
            token_hash => Str::new()
                .require(true)
                .max_length(64)
                .secret(true)
                .readable_by([SYSTEM_ROLE])
                .writable_by([SYSTEM_ROLE]),
            encrypt_enabled => Switch::new().require(true).default(false),
            default_locale => Str::new().require(true).max_length(16).default("zh_cn"),
            status => Radio::<String>::new()
                .require(true)
                .varchar(16)
                .options([("active", "启用"), ("disabled", "停用")])
                .default("active"),
            // DSL 没有 Json builder，JSON 文本落 Text 列；该列从不被 SQL 查询进内部
            linkage_mapping => Text::new(),
            created_at => Timestamp::new().created_at(),
            updated_at => Timestamp::new().updated_at(),
        }))
}
```

`option/table.rs`：

```rust
//! `feishu_option` 表声明——Schema 的唯一事实来源。

use yang_base::definition::{Int, Key, Str, Switch, TableSpec, Text, Timestamp};
use yang_base::BaseError;

/// 选项表的表名。
pub(crate) const TABLE: &str = "feishu_option";

/// 声明选项表。
pub(crate) fn table_spec() -> Result<TableSpec, BaseError> {
    let table = yang_base::table!(TABLE);
    Ok(TableSpec::new(table)
        .title("飞书选项")
        .fields(yang_base::fields! {
            id => Key::new(),
            // 飞书契约要求 id 全局唯一且固定。DSL 无法声明自然键主键（Key 硬编码为
            // 自增大整数），因此改用唯一索引强制，约束力等价。
            option_id => Str::new()
                .require(true)
                .unique(true)
                .max_length(128)
                .searchable(true)
                .filterable(true)
                .sortable(true),
            source_key => Str::new()
                .require(true)
                .max_length(64)
                .indexed(true)
                .filterable(true)
                .sortable(true),
            label => Str::new().require(true).max_length(255).searchable(true),
            // JSON 文本：{"en_us":"…","ja_jp":"…"}
            i18n => Text::new(),
            sort_order => Int::new().require(true).default(0).sortable(true),
            is_default => Switch::new().require(true).default(false),
            // 禁用而非删除，避免历史审批单引用的选项彻底失联
            enabled => Switch::new().require(true).default(true).filterable(true),
            extra => Text::new(),
            created_at => Timestamp::new().created_at(),
            updated_at => Timestamp::new().updated_at(),
        }))
}
```

- [ ] **Step 4: 运行确认表测试通过**

```bash
cd /d/code/lib_yang/project/yang-system && cargo test --lib --locked feishu::datasource::table feishu::option::table
```
Expected: 5 passed。

- [ ] **Step 5: 实现 Repository 与 Context**

`domain/repository.rs`：

```rust
//! 两张表的唯一持久化边界。
//!
//! 所有读写都经这里，受信角色为 [`SYSTEM_ROLE`]——外部 Token 调用没有登录身份，
//! 但表查询本身只要求角色满足字段 Audience（默认 Everyone），因此无需伪造用户。

use std::sync::Arc;

use sqlx::MySqlPool;
use yang_base::table::{Record, TableDefinition, TableHandle, TableQuery, PaginatedResult};
use yang_base::BaseError;

/// 受信服务角色；字段权限判定的依据。
pub(crate) const SYSTEM_ROLE: &str = "system";

/// 一张表在服务端的读写入口。
#[derive(Clone)]
pub(crate) struct Repository {
    definition: TableDefinition,
    pool: Arc<MySqlPool>,
}

impl Repository {
    /// 绑定表定义与连接池。
    pub(crate) fn new(definition: TableDefinition, pool: Arc<MySqlPool>) -> Self {
        Self { definition, pool }
    }

    /// 以受信角色开启一次查询。
    pub(crate) fn query(&self) -> TableQuery {
        self.definition.bind(Arc::clone(&self.pool)).query([SYSTEM_ROLE])
    }

    /// 表定义，供上层读取字段名与能力位。
    pub(crate) fn definition(&self) -> &TableDefinition {
        &self.definition
    }

    /// 分页查询。
    pub(crate) async fn paginate(&self, query: TableQuery) -> Result<PaginatedResult<Record>, BaseError> {
        query.paginate_records().await
    }

    /// 按主键取一行。
    pub(crate) async fn find_by_primary_key(&self, value: serde_json::Value) -> Result<Option<Record>, BaseError> {
        self.query().where_primary_key_eq(value)?.optional().await
    }

    /// 插入一行。
    pub(crate) async fn insert(&self, data: Record) -> Result<u64, BaseError> {
        self.query().insert(data).await
    }

    /// 按条件更新。
    pub(crate) async fn update(self_query: TableQuery, data: Record) -> Result<u64, BaseError> {
        self_query.update(data).await
    }

    /// 按条件删除。
    pub(crate) async fn delete(self_query: TableQuery) -> Result<u64, BaseError> {
        self_query.delete().await
    }
}
```

> `Repository::update` / `delete` 需要 `&self` 与查询分开传，上面的写法签名不完整——
> **请按实际编译情况调整为方法形式**：`pub(crate) async fn update(&self, query: TableQuery, data: Record)`。
> `TableQuery` 的链式方法大多返回 `Result`，调用处记得 `?`。

`domain/context.rs`：

```rust
//! 飞书 addon 的模块上下文：聚合两张表的 Repository，并提供事务收尾。

use yang_base::BaseError;
use yang_db::Transaction;

use super::repository::Repository;

/// addon 级共享上下文。
///
/// 两张表的 Repository 都在这里持有——`Registry::dispatch` 只向 Action 注入**所在 module
/// 的主表**，所以跨表访问（选项 module 读数据源表）必须经这个上下文。
#[derive(Clone)]
pub(crate) struct FeishuContext {
    datasource: Repository,
    option: Repository,
}

impl FeishuContext {
    /// 构造上下文。
    pub(crate) fn new(datasource: Repository, option: Repository) -> Self {
        Self { datasource, option }
    }

    /// 数据源表。
    pub(crate) fn datasources(&self) -> &Repository {
        &self.datasource
    }

    /// 选项表。
    pub(crate) fn options(&self) -> &Repository {
        &self.option
    }

    /// 事务收尾：成功提交、失败回滚；回滚失败只记日志，不覆盖原错误。
    pub(crate) async fn finish_transaction<T>(
        transaction: Transaction,
        result: Result<T, BaseError>,
    ) -> Result<T, BaseError> {
        match result {
            Ok(value) => {
                transaction.commit().await.map_err(BaseError::from)?;
                Ok(value)
            }
            Err(error) => {
                if let Err(rollback_error) = transaction.rollback().await {
                    tracing::error!(error = %rollback_error, "飞书 addon 事务回滚失败");
                }
                Err(error)
            }
        }
    }
}
```

- [ ] **Step 6: 写 module 装配与组合根接线**

`datasource/mod.rs`：

```rust
//! 数据源 module：承载 `feishu_datasource` 表。

pub(crate) mod actions;
pub(crate) mod table;

use std::sync::Arc;

use yang_base::definition::{module, AddonName, ModuleSpec, TableSpec};
use yang_base::BaseError;

use super::domain::context::FeishuContext;

/// 装配数据源 module。
pub(crate) fn build_module(context: Arc<FeishuContext>) -> Result<ModuleSpec, BaseError> {
    let spec = ModuleSpec::new(module!("feishu.datasource"))
        .table(table::table_spec()?)
        .presentation(
            yang_base::definition::ModulePresentationSpec::new("飞书数据源", "table", 40),
        );
    Ok(actions::register_all(spec, context))
}
```

`option/mod.rs` 同形，`module!("feishu.option")`、标题「飞书选项」、order 41。

`datasource/actions/mod.rs` 与 `option/actions/mod.rs`（本 Task 先留空表，Task 8/9 填充）：

```rust
//! 本 module 的 Action 注册表。
//!
//! 架构门禁要求：`actions/` 下每个文件恰好一个 `pub(super) async fn handle` +
//! 一个 `pub(super) fn register`，并在这里登记。

use std::sync::Arc;

use yang_base::definition::ModuleSpec;

use super::super::domain::context::FeishuContext;

/// 空注册表，保持 module 形状合法。
pub(super) fn register_all(module: ModuleSpec, _context: Arc<FeishuContext>) -> ModuleSpec {
    module
}
```

`feishu/mod.rs` 改为：

```rust
//! 飞书外部数据源集成。

pub(crate) mod datasource;
pub(crate) mod domain;
pub(crate) mod option;

use std::sync::Arc;

use sqlx::MySqlPool;
use yang_base::definition::{AddonSpec, AddonName};
use yang_base::table::TableDefinition;
use yang_base::BaseError;

use crate::config::FeishuSettings;

/// 装配飞书 addon。
///
/// `settings` 为 `None` 或未启用时仍会注册（表需要参与 Schema 同步），
/// 但路由层在 Task 8 会依据设置决定是否暴露对外端点。
pub(crate) fn build_addon(
    settings: Option<Arc<FeishuSettings>>,
    pool: Arc<MySqlPool>,
    datasource_definition: TableDefinition,
    option_definition: TableDefinition,
) -> Result<AddonSpec, BaseError> {
    let _ = settings;
    let context = Arc::new(domain::context::FeishuContext::new(
        domain::repository::Repository::new(datasource_definition, Arc::clone(&pool)),
        domain::repository::Repository::new(option_definition, Arc::clone(&pool)),
    ));
    Ok(AddonSpec::new(AddonName::new("feishu")?)
        .module(datasource::build_module(Arc::clone(&context))?)
        .module(option::build_module(context)?))
}
```

在 `src/addon/mod.rs` 追加 `pub(crate) mod feishu;`。

在 `src/app.rs` 的 addon 链上追加（在 `demo` 之后）：

```rust
        .addon(
            feishu::build_addon(feishu_settings, mysql_pool, datasource_definition, option_definition)?
                .middleware(ActionLogMiddleware::new(LogIdentity::from_tools(&tools))),
        )
```

其中四个新参数在 `build_application` 内构造：表定义来自 `feishu::datasource::table::table_spec()?.table_definition()?` 与 `feishu::option::table::table_spec()?.table_definition()?`，连接池来自 `tools.mysql()?.pool()` 包成 `Arc`，`feishu_settings` 来自 `Settings`。

> ⚠️ `AddonSpec::middleware` 必须**在** `.module(...)` 之后调用（`feishu/mod.rs` 已满足）。
> 同时 `build_schema_app` / `build_metadata_app` 也要接上同一个 addon，否则 Schema 同步与
> OpenAPI 导出会缺表。检查 `src/app.rs` 里三个入口是否共用 `build_application`；
> 若它们各自拼装，三处都要改。

- [ ] **Step 7: 跑单测与架构门禁**

```bash
cd /d/code/lib_yang/project/yang-system && cargo test --lib --locked feishu::
cd /d/code/lib_yang/project/yang-system && python scripts/check_architecture.py
```
Expected: 单测 passed；架构门禁 passed（若报 `机制目录必须迁入所属 module 的 domain/`，说明有文件放错层级，按报错路径搬进 `domain/`）。

- [ ] **Step 8: 跑 app.rs 冒烟测试确认投影**

```bash
cd /d/code/lib_yang/project/yang-system && cargo test --lib --locked app::
```
Expected: passed。若冒烟测试断言了模块/表/权限投影的具体集合，按其报错把飞书的两个 module 与两张表加进期望集合。

- [ ] **Step 9: 提交**

```bash
cd /d/code/lib_yang/project/yang-system && git add src/addon src/app.rs && git commit -F - <<'EOF'
feat(feishu): 数据源与选项两张表、Repository 与组合根接线

feishu_datasource 与 feishu_option 走 TableSpec + fields! 声明，因此自动进入
Catalog / 前端零代码 TableView / 权限目录，并参与启动期 Schema 增量同步。

两处与 spec 不同的实现细节（已在 spec §5.3 记录原因）：
- option_id 的全局唯一由 Str::new().unique(true) 的 UNIQUE 索引强制，
  而非主键——DSL 的 Key 硬编码为自增大整数，无法表达自然键主键，约束力等价。
- i18n / extra / linkage_mapping 落 Text 列存放 JSON 文本，
  因为 fields! DSL 没有 Json builder。

token_hash 用 secret(true) 后显式 readable_by/writable_by 授回 system 角色
（secret 会把读写权限置为 Nobody）。Repository 统一以 system 角色操作，
因为外部 Token 调用没有登录身份，而表查询只要求角色满足字段 Audience。
EOF
```

---

## Task 8: 入口 1——飞书审批外部选项端点

**Files:**
- Create: `project/yang-system/src/addon/feishu/domain/pagination.rs`
- Create: `project/yang-system/src/addon/feishu/option/actions/approval_options.rs`
- Modify: `project/yang-system/src/addon/feishu/option/actions/mod.rs`
- Modify: `project/yang-system/src/addon/feishu/option/mod.rs`、`domain/mod.rs`

**Interfaces:**
- Consumes: Task 3 的 protocol、Task 4 的 crypto、Task 5 的 i18n、Task 6 的 token、Task 7 的 Context
- Produces:
  - `pub(crate) fn encode_cursor(sort_order: i64, option_id: &str) -> String`
  - `pub(crate) fn decode_cursor(raw: &str) -> Result<Option<(i64, String)>, BaseError>`
  - 路由 `POST /api/v1/feishu/approval/options/{source_key}`，`Output = ResponseBody`，`public`

> **响应形态**：`ResponseBody::raw(json, "application/json")`——字面严格的 `{code,msg,data}`，
> 不套框架包络。业务失败**也走 Ok(...)**（返回 `code != 0` 的信封），只有真正的内部故障才
> 在内部转换后同样以 `Ok` 返回，以保证 HTTP 状态恒 200 且不烧 SLO 错误预算。

- [ ] **Step 1: 写分页游标的失败测试**

创建 `domain/pagination.rs`，先只写 `mod tests`：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_round_trips() {
        let encoded = encode_cursor(42, "dept_sales");
        assert_eq!(
            decode_cursor(&encoded).expect("应可解码"),
            Some((42, "dept_sales".to_string()))
        );
    }

    #[test]
    fn empty_cursor_means_first_page() {
        assert_eq!(decode_cursor("").expect("空游标合法"), None);
        assert_eq!(decode_cursor("   ").expect("空白游标合法"), None);
    }

    #[test]
    fn malformed_cursor_is_rejected_not_ignored() {
        // fail-closed：非法游标必须报错，否则会静默从头返回、造成重复数据
        assert!(decode_cursor("not-base64!!").is_err());
        assert!(decode_cursor("bm90LWEtY3Vyc29y").is_err(), "base64 合法但结构不对");
    }
}
```

- [ ] **Step 2: 实现游标**

```rust
//! `page_token` 游标编解码。
//!
//! 飞书的分页是游标语义，而 `TableQuery` 是 `page/page_size` 语义。这里把
//! `(sort_order, option_id)` 这个稳定排序键编成不透明游标，翻页时用 keyset 条件
//! 而不是 offset——offset 在数据变动时会漏行或重复。

use base64::Engine;
use yang_base::BaseError;

/// 游标内部的分隔符；`option_id` 的字符集不含它。
const SEPARATOR: char = '\u{1f}';

/// 编码游标。
pub(crate) fn encode_cursor(sort_order: i64, option_id: &str) -> String {
    let raw = format!("{sort_order}{SEPARATOR}{option_id}");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw.as_bytes())
}

/// 解码游标；空串或全空白表示请求第一页。
///
/// # 错误
///
/// 非法 base64 或结构不符时返回 [`BaseError::ParamInvalid`]（fail-closed）。
pub(crate) fn decode_cursor(raw: &str) -> Result<Option<(i64, String)>, BaseError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(trimmed)
        .map_err(|_| BaseError::ParamInvalid("page_token".to_string(), "分页标记非法".to_string()))?;
    let decoded = String::from_utf8(bytes)
        .map_err(|_| BaseError::ParamInvalid("page_token".to_string(), "分页标记非法".to_string()))?;
    let (order, id) = decoded.split_once(SEPARATOR).ok_or_else(|| {
        BaseError::ParamInvalid("page_token".to_string(), "分页标记非法".to_string())
    })?;
    let sort_order = order.parse::<i64>().map_err(|_| {
        BaseError::ParamInvalid("page_token".to_string(), "分页标记非法".to_string())
    })?;
    Ok(Some((sort_order, id.to_string())))
}
```

- [ ] **Step 3: 运行确认通过**

```bash
cd /d/code/lib_yang/project/yang-system && cargo test --lib --locked feishu::domain::pagination
```
Expected: 3 passed。

- [ ] **Step 4: 写端点的失败测试**

在 `option/actions/approval_options.rs` 的 `mod tests` 中，用假 Repository 覆盖三条路径。
由于 `Repository` 需要真实连接池，这里改为测试**纯函数部分**：把「请求 → 响应信封」的
判定逻辑抽成不依赖 DB 的 `fn classify(request, source_row) -> Result<Resolved, FeishuEnvelope>`，
并测试它。

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrong_token_yields_nonzero_code_envelope_not_error() {
        // Token 不匹配必须返回业务失败信封（HTTP 200、code != 0），不能是 Err
        let stored = crate::addon::feishu::domain::token::hash_token("right-token");
        let envelope = verify_source(Some(&stored), Some("wrong-token"), true)
            .err()
            .expect("不匹配应给出失败信封");
        assert_ne!(envelope.code, 0);
        assert!(envelope.msg.contains("token"), "msg 应说明原因: {}", envelope.msg);
    }

    #[test]
    fn missing_source_yields_failure_envelope() {
        let envelope = verify_source(None, Some("any"), true)
            .err()
            .expect("数据源不存在应给出失败信封");
        assert_ne!(envelope.code, 0);
    }

    #[test]
    fn disabled_source_yields_failure_envelope() {
        let stored = crate::addon::feishu::domain::token::hash_token("right-token");
        let envelope = verify_source(Some(&stored), Some("right-token"), false)
            .err()
            .expect("禁用数据源应给出失败信封");
        assert_ne!(envelope.code, 0);
    }

    #[test]
    fn matching_token_on_active_source_passes() {
        let stored = crate::addon::feishu::domain::token::hash_token("right-token");
        assert!(verify_source(Some(&stored), Some("right-token"), true).is_ok());
    }
}
```

- [ ] **Step 5: 实现端点**

在 `approval_options.rs` 的 `mod tests` 之前写入：

```rust
//! 飞书审批「关联外部选项」取数端点。
//!
//! 这是本服务对外暴露的**唯一**字面严格接口：响应是 `ResponseBody::raw` 产出的
//! `{code,msg,data}`，不套框架的 `{code,message,data}` 包络。
//!
//! 关键约定：**任何业务失败都返回 `Ok(ResponseBody::raw(失败信封))`**，不用 `Err`。
//! 匿名端点的 `Err` 会在可观测性里记成 `result="error"` 并消耗全站可用性预算，
//! 而 burn-rate 规则按 `sum by (job)` 聚合、不带 operation 维度——飞书的探活流量
//! 不该把全站告警打起来。

use std::sync::Arc;

use yang_base::action::{ActionContext, ResponseBody};
use yang_base::definition::{action_name, HttpMethod, ModuleSpec};
use yang_base::BaseError;

use super::super::super::domain::context::FeishuContext;
use super::super::super::domain::i18n::{build_result_body, OptionRow, DEFAULT_LOCALE};
use super::super::super::domain::pagination::decode_cursor;
use super::super::super::domain::protocol::{FeishuEnvelope, FeishuOptionsRequest};

/// 单次请求返回的最大选项数（框架 `TableQuery` 的硬上限是 100）。
const PAGE_SIZE: usize = 100;

/// 校验数据源与 Token，返回失败信封表示拒绝。
///
/// 抽成纯函数以便不依赖数据库做单元测试。
pub(super) fn verify_source(
    stored_hash: Option<&str>,
    presented: Option<&str>,
    is_active: bool,
) -> Result<(), FeishuEnvelope> {
    let stored = stored_hash.ok_or_else(|| FeishuEnvelope::fail(40401, "数据源不存在"))?;
    let presented = presented.ok_or_else(|| FeishuEnvelope::fail(40101, "缺少 token"))?;
    if !super::super::super::domain::token::verify_token(presented, stored) {
        return Err(FeishuEnvelope::fail(40102, "token 校验失败"));
    }
    if !is_active {
        return Err(FeishuEnvelope::fail(40301, "数据源已停用"));
    }
    Ok(())
}

/// 注册取选项端点。
pub(super) fn register(module: ModuleSpec, context: Arc<FeishuContext>) -> ModuleSpec {
    module
        .action_fn(
            action_name!("approval_options"),
            move |ctx, input| handle(ctx, input, Arc::clone(&context)),
        )
        .route(
            HttpMethod::Post,
            "/api/v1/feishu/approval/options/{source_key}",
        )
        .display_name("飞书外部选项")
        .description("飞书审批单选/多选控件的外部选项数据源接口")
        // public：飞书带自定义 Token 直连，不经框架 JWT 鉴权；来源校验在 handler 内完成
        .public()
        .register()
}

/// 处理一次取选项请求。
async fn handle(
    ctx: ActionContext,
    input: FeishuOptionsRequest,
    context: Arc<FeishuContext>,
) -> Result<ResponseBody, BaseError> {
    let envelope = match resolve(&ctx, &input, &context).await {
        Ok(envelope) => envelope,
        Err(error) => {
            // 内部故障也返回 200 + 失败信封：非 200 会让飞书按「接口报错」处理，
            // 而 FAQ 对失败现象的归因正是「外部数据源返回接口报错」
            tracing::error!(error = %error, code = error.code(), "飞书取选项请求失败");
            FeishuEnvelope::fail(50001, "服务内部错误")
        }
    };
    let body = envelope.to_json()?;
    ResponseBody::raw(body, "application/json")
}

/// 解析并组装成功响应。
async fn resolve(
    ctx: &ActionContext,
    input: &FeishuOptionsRequest,
    context: &FeishuContext,
) -> Result<FeishuEnvelope, BaseError> {
    let source_key = ctx
        .request
        .get_path_param("source_key")
        .ok_or_else(|| BaseError::ParamInvalid("source_key".to_string(), "缺少数据源标识".to_string()))?
        .to_string();

    // 查数据源
    let datasource = context
        .datasources()
        .query()
        .select_fields(&["id", "source_key", "token_hash", "encrypt_enabled", "default_locale", "status"])?
        .where_eq("source_key", serde_json::Value::String(source_key.clone()))?
        .optional()
        .await?
        .ok_or_else(|| BaseError::RecordNotFound("数据源不存在".to_string()))?;

    let stored_hash: String = datasource.require("token_hash")?;
    let status: String = datasource.require("status")?;
    if let Err(envelope) = verify_source(Some(&stored_hash), Some(&input.token), status == "active") {
        return Ok(envelope);
    }

    let encrypt_enabled: bool = datasource.optional("encrypt_enabled")?.unwrap_or(false);
    let default_locale: String = datasource
        .optional("default_locale")?
        .unwrap_or_else(|| DEFAULT_LOCALE.to_string());

    // 游标：非法游标 fail-closed（返回失败信封而不是静默从头开始）
    let cursor = match input.page_token.as_deref() {
        Some(raw) => match decode_cursor(raw) {
            Ok(value) => value,
            Err(_) => return Ok(FeishuEnvelope::fail(40002, "分页标记非法")),
        },
        None => None,
    };

    let mut query = context
        .options()
        .query()
        .select_fields(&["option_id", "label", "i18n", "sort_order", "is_default"])?
        .where_eq("source_key", serde_json::Value::String(source_key))?
        .where_eq("enabled", serde_json::Value::Bool(true))?
        .search(input.query.as_deref())?;

    if let Some((sort_order, option_id)) = cursor {
        // keyset 翻页：取排序键严格大于游标的行，避免 offset 在数据变动时漏行
        query = query.where_or(vec![
            yang_base::table::WhereCondition::Gt {
                field: "sort_order".to_string(),
                value: serde_json::Value::Number(sort_order.into()),
            },
            yang_base::table::WhereCondition::And {
                conditions: vec![
                    yang_base::table::WhereCondition::Eq {
                        field: "sort_order".to_string(),
                        value: serde_json::Value::Number(sort_order.into()),
                    },
                    yang_base::table::WhereCondition::Gt {
                        field: "option_id".to_string(),
                        value: serde_json::Value::String(option_id),
                    },
                ],
            },
        ])?;
    }

    // 多取一行用于判定 hasMore
    let page = query
        .order_by("sort_order", yang_base::table::SortOrder::Asc)?
        .order_by("option_id", yang_base::table::SortOrder::Asc)?
        .page(1, PAGE_SIZE + 1)?
        .all()
        .await?;

    let has_more = page.len() > PAGE_SIZE;
    let rows: Vec<OptionRow> = page
        .into_iter()
        .take(PAGE_SIZE)
        .map(|record| {
            let i18n_json: Option<String> = record.optional("i18n")?;
            let i18n = match i18n_json.as_deref() {
                Some(raw) if !raw.trim().is_empty() => serde_json::from_str(raw)
                    .map_err(|error| BaseError::JsonSerializeFailed(error.to_string()))?,
                _ => std::collections::BTreeMap::new(),
            };
            Ok(OptionRow {
                option_id: record.require("option_id")?,
                label: record.require("label")?,
                i18n,
                is_default: record.optional("is_default")?.unwrap_or(false),
            })
        })
        .collect::<Result<Vec<_>, BaseError>>()?;

    let next_page_token = if has_more {
        rows.last()
            .map(|last| {
                super::super::super::domain::pagination::encode_cursor(0, &last.option_id)
            })
    } else {
        None
    };

    let result = build_result_body(&rows, &default_locale, has_more, next_page_token);

    if encrypt_enabled {
        let key = encryption_key(ctx)?;
        let value = serde_json::to_value(&result)
            .map_err(|error| BaseError::JsonSerializeFailed(error.to_string()))?;
        let cipher = super::super::super::domain::crypto::encrypt_json(&value, &key)?;
        return Ok(FeishuEnvelope::encrypted(cipher));
    }
    Ok(FeishuEnvelope::ok(result))
}

/// 取配置中的 AES 密钥并派生；未配置时是配置错误。
fn encryption_key(ctx: &ActionContext) -> Result<[u8; 32], BaseError> {
    let settings = ctx
        .tools()
        .config::<Arc<crate::config::FeishuSettings>>()?;
    let raw = settings
        .encryption_key
        .as_deref()
        .ok_or_else(|| BaseError::ConfigError("数据源启用了加密但未配置 feishu.encryption_key".to_string()))?;
    Ok(super::super::super::domain::crypto::derive_key(raw))
}
```

> **`next_page_token` 的游标取值**：上面写了 `encode_cursor(0, &last.option_id)`，这是**错的**——
> 必须用该行的真实 `sort_order`。请把 `OptionRow` 增加一个 `sort_order: i64` 字段（Task 5 的
> `build_result_body` 不消费它，仅供游标使用），并在构造 `OptionRow` 时从记录里读出
> `record.require::<i64>("sort_order")?`，然后 `encode_cursor(last.sort_order, &last.option_id)`。
> 同时 `select_fields` 要加上 `"sort_order"`。

在 `option/actions/mod.rs` 中登记：

```rust
//! 本 module 的 Action 注册表。

pub(super) mod approval_options;

use std::sync::Arc;

use yang_base::definition::ModuleSpec;

use super::super::domain::context::FeishuContext;

/// 注册本 module 的全部 Action。
pub(super) const ACTIONS: &[&str] = &["approval_options"];

pub(super) fn register_all(module: ModuleSpec, context: Arc<FeishuContext>) -> ModuleSpec {
    approval_options::register(module, context)
}
```

`domain/mod.rs` 追加 `pub(crate) mod pagination;`。

- [ ] **Step 6: 运行单测与架构门禁**

```bash
cd /d/code/lib_yang/project/yang-system && cargo test --lib --locked feishu::
cd /d/code/lib_yang/project/yang-system && python scripts/check_architecture.py
```
Expected: passed。

- [ ] **Step 7: 手工验证裸 body**

```bash
cd /d/code/lib_yang/project/yang-system && cargo run --locked
# 另开一个终端（需先按 Task 11 造好数据源与选项）
curl -sS -X POST http://127.0.0.1:8080/api/v1/feishu/approval/options/demo \
  -H 'Content-Type: application/json' \
  -d '{"employee_id":"abc","token":"<数据源的 token 明文>"}' | head -c 400
```
Expected: 输出以 `{"code":0,"msg":"success!","data":{"result":{"options":` 开头，
**不含 `"message"`**。

- [ ] **Step 8: 提交**

```bash
cd /d/code/lib_yang/project/yang-system && git add src/addon/feishu && git commit -F - <<'EOF'
feat(feishu): 飞书审批外部选项取数端点

POST /api/v1/feishu/approval/options/{source_key}，public Action，输出
ResponseBody::raw 产出的字面严格 {code,msg,data}——不套框架的 {code,message,data} 包络。
这是本服务唯一对外暴露的严格契约接口。

关键约定：所有业务失败（token 不匹配、数据源不存在或停用、游标非法）与内部故障都返回
Ok(失败信封) 而非 Err，保证 HTTP 状态恒 200 且不消耗全站可用性预算——匿名端点的 Err
会在可观测性里记成 result="error"，而 burn-rate 规则不带 operation 维度。

分页用 keyset 游标（(sort_order, option_id) 的 base64）而非 offset，避免数据变动时
漏行或重复；非法游标 fail-closed 而不是静默从头返回。多取一行判定 hasMore。
EOF
```

---

## Task 9: 入口 2——多维表格写入 API

**Files:**
- Create: `project/yang-system/src/addon/feishu/domain/middleware.rs`
- Create: `project/yang-system/src/addon/feishu/option/actions/upsert_options.rs`、`delete_options.rs`
- Modify: `project/yang-system/src/addon/feishu/option/actions/mod.rs`、`option/mod.rs`、`domain/mod.rs`

**Interfaces:**
- Consumes: Task 7 的 Context、Task 6 的 token（复用常量时间比较）
- Produces:
  - `pub(crate) struct ManagementTokenMiddleware { token_hash: String, target: ActionRef }`
  - `POST /api/v1/feishu/inbound/options/upsert`（body：`{ source_key, options: [{ id, label, i18n?, sort_order?, is_default?, enabled? }] }`，单次上限 500 条）
  - `POST /api/v1/feishu/inbound/options/delete`（body：`{ source_key, ids: [String] }`）

- [ ] **Step 1: 写中间件的失败测试**

创建 `domain/middleware.rs`，先只写 `mod tests`：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_bearer_token_case_insensitively() {
        assert_eq!(extract_token("Bearer abc123").as_deref(), Some("abc123"));
        assert_eq!(extract_token("bearer abc123").as_deref(), Some("abc123"));
        assert_eq!(extract_token("  Bearer   abc123  ").as_deref(), Some("abc123"));
    }

    #[test]
    fn rejects_non_bearer_and_missing() {
        assert_eq!(extract_token("Basic abc"), None);
        assert_eq!(extract_token("abc123"), None);
        assert_eq!(extract_token(""), None);
    }
}
```

- [ ] **Step 2: 实现中间件**

```rust
//! 多维表格写入 API 的静态 Token 校验中间件。
//!
//! 为什么是一个中间件而不是普通受保护 Action：静态 Token 的调用方没有 JWT，
//! 过不了框架的 `TokenAuthMiddleware`；而 `Middleware` 虽然能**读**身份
//! （`ActionContext::actor()` 是公开的），却**不能注入**身份
//! （`with_user` / `ctx.user` 是 `pub(crate)`），所以无法让静态 Token 走受保护 Action。

use async_trait::async_trait;
use yang_base::action::{ActionContext, ApiResponse};
use yang_base::definition::ActionRef;
use yang_base::router::{Middleware, Next};
use yang_base::BaseError;

use super::token::verify_token;

/// 校验 `Authorization: Bearer <token>` 是否匹配管理 Token 的中间件。
pub(crate) struct ManagementTokenMiddleware {
    token_hash: String,
    target: ActionRef,
}

impl ManagementTokenMiddleware {
    /// 绑定管理 Token 的哈希与目标 Action。
    pub(crate) fn new(management_api_token: &str, target: ActionRef) -> Self {
        Self {
            token_hash: super::token::hash_token(management_api_token),
            target,
        }
    }
}

/// 从 Authorization 头取出 Bearer token。
pub(super) fn extract_token(header: &str) -> Option<String> {
    let (scheme, value) = header.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.to_string())
}

#[async_trait]
impl Middleware for ManagementTokenMiddleware {
    fn target_action(&self) -> Option<&ActionRef> {
        Some(&self.target)
    }

    async fn handle(
        &self,
        ctx: ActionContext,
        next: Next<'_>,
    ) -> Result<ApiResponse, BaseError> {
        let presented = ctx
            .request
            .get_header("authorization")
            .and_then(extract_token);
        let authorized = presented
            .as_deref()
            .is_some_and(|token| verify_token(token, &self.token_hash));
        if !authorized {
            // 业务失败走 Ok(ApiResponse::fail)：Err 会记成 result="error" 并烧可用性预算
            return Ok(ApiResponse::fail(40102, "管理 Token 校验失败"));
        }
        next.run(ctx).await
    }
}
```

`domain/mod.rs` 追加 `pub(crate) mod middleware;`。

- [ ] **Step 3: 写写入端点的失败测试**

在 `upsert_options.rs` 的 `mod tests` 中测试入参校验（不依赖 DB）：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn input(count: usize) -> UpsertOptionsInput {
        UpsertOptionsInput {
            source_key: "demo".to_string(),
            options: (0..count)
                .map(|index| OptionUpsertItem {
                    id: format!("opt_{index}"),
                    label: format!("选项{index}"),
                    i18n: None,
                    sort_order: None,
                    is_default: None,
                    enabled: None,
                })
                .collect(),
        }
    }

    #[test]
    fn rejects_empty_option_list() {
        assert!(input(0).validate().is_err(), "空列表应被拒绝");
    }

    #[test]
    fn rejects_over_limit_batch() {
        assert!(input(MAX_BATCH).validate().is_ok(), "上限内应通过");
        assert!(input(MAX_BATCH + 1).validate().is_err(), "超限应被拒绝");
    }

    #[test]
    fn rejects_blank_option_id() {
        let mut payload = input(1);
        payload.options[0].id = "   ".to_string();
        assert!(payload.validate().is_err(), "空白 id 应被拒绝");
    }
}
```

- [ ] **Step 4: 实现写入端点**

实现 `UpsertOptionsInput` / `OptionUpsertItem`（`#[derive(Deserialize, JsonSchema)]` +
`impl ParamInput`，与 Task 3 的请求同形）、`validate()`（非空、条数 ≤ 500、id 非空白且长度 ≤ 128），
以及：

```rust
/// 注册批量 upsert 端点。
pub(super) fn register(module: ModuleSpec, context: Arc<FeishuContext>) -> ModuleSpec {
    module
        .action_fn(
            action_name!("upsert_options"),
            move |ctx, input| handle(ctx, input, Arc::clone(&context)),
        )
        .route(HttpMethod::Post, "/api/v1/feishu/inbound/options/upsert")
        .display_name("同步飞书选项")
        .description("多维表格工作流批量新增或更新选项")
        .public()
        .register()
}
```

处理函数按幂等 upsert 语义实现：对每个 item，先按 `option_id` 查是否存在——存在则
`update`（更新 `label` / `i18n` / `sort_order` / `is_default` / `enabled` / `source_key`），
不存在则 `insert`；整个批次在**一个事务**内完成，并在同事务内追加审计事件
（`audit::succeeded_system_event` + `audit::append_in_tx`，actor 用 `"feishu-inbound"`，
`target` 用 `AuditEntity::new("feishu_option", source_key)`，摘要字段用
`outcome_code` / `option_count`——**不得**出现 `token` / `secret` / `hash` 等被拒绝的子串）。

> `succeeded_system_event` 依赖 `ctx.dispatch_target()` 返回 `Some((module, action))`，
> 因此必须经 `Registry` 派发（正常请求路径满足），在单元测试里直接调用会失败。
> 审计写入只在集成测试里验证（Task 11）。
>
> `delete_options.rs` 同形：按 `ids` 把 `enabled` 置 false（**不物理删除**，避免历史审批单
> 引用的选项失联），同样走事务 + 审计。

- [ ] **Step 5: 在 module 装配里挂中间件**

`option/mod.rs` 的 `build_module` 增加中间件挂载（`settings` 为 `None` 或未启用时不挂）：

```rust
pub(crate) fn build_module(
    context: Arc<FeishuContext>,
    settings: Option<Arc<FeishuSettings>>,
) -> Result<ModuleSpec, BaseError> {
    let spec = ModuleSpec::new(module!("feishu.option"))
        .table(table::table_spec()?)
        .presentation(
            yang_base::definition::ModulePresentationSpec::new("飞书选项", "table", 41),
        );
    let spec = actions::register_all(spec, Arc::clone(&context));
    Ok(match settings.as_deref().filter(|value| value.is_usable()) {
        Some(value) => {
            let target = yang_base::definition::ActionRef::new(
                yang_base::module!("feishu.option"),
                yang_base::action_name!("upsert_options"),
            );
            spec.middleware(super::domain::middleware::ManagementTokenMiddleware::new(
                &value.management_api_token,
                target,
            ))
        }
        None => spec,
    })
}
```

> 每个写入 Action 都要挂一个中间件实例（`target_action()` 只接受一个 `ActionRef`）。
> `delete_options` 同理，构造第二个实例。

- [ ] **Step 6: 运行单测与架构门禁**

```bash
cd /d/code/lib_yang/project/yang-system && cargo test --lib --locked feishu::
cd /d/code/lib_yang/project/yang-system && python scripts/check_architecture.py
```
Expected: passed。

- [ ] **Step 7: 手工验证鉴权**

```bash
# 不带 token 必须被拒
curl -sS -o /dev/null -w '%{http_code}\n' -X POST \
  http://127.0.0.1:8080/api/v1/feishu/inbound/options/upsert \
  -H 'Content-Type: application/json' -d '{"source_key":"demo","options":[]}'
```
Expected: HTTP 200 且响应体 `code != 0`（业务失败走 Ok，不是 401）。

- [ ] **Step 8: 提交**

```bash
cd /d/code/lib_yang/project/yang-system && git add src/addon/feishu && git commit -F - <<'EOF'
feat(feishu): 多维表格写入 API 与静态 Token 中间件

POST /api/v1/feishu/inbound/options/upsert 与 /delete，供多维表格自动化工作流
推送选项变更。之所以做成 public Action + 中间件而不是受保护 Action：静态 Token 的
调用方没有 JWT 过不了 TokenAuthMiddleware，而中间件能读身份却不能注入身份
（with_user / ctx.user 是 pub(crate)）。

写入是幂等 upsert（按 option_id 判定新增或更新），整批在一个事务内完成并同事务追加
审计事件（system actor）。删除语义是置 enabled=false 而非物理删除，避免历史审批单
引用的选项失联。
EOF
```

---

## Task 10: 入口 3——前端控制台查询 Action

**Files:**
- Create: `project/yang-system/src/addon/feishu/datasource/actions/{create_datasource,update_datasource,delete_datasource,list_datasources}.rs`
- Create: `project/yang-system/src/addon/feishu/option/actions/list_options.rs`
- Modify: 两个 `actions/mod.rs`、两个 `mod.rs`（加 `ViewSpec`）

**Interfaces:**
- Consumes: Task 7 的 Context、Task 6 的 `hash_token`
- Produces: 5 个受保护 Action，权限串 `feishu.datasource.read|write` / `feishu.option.read`

> 本入口**对选项只读**：选项的增改由入口 2 承担。两处并存的可写入口会让审计语义与
> 数据来源分叉。数据源本身可在控制台增删改。

- [ ] **Step 1: 写列表 Action 的失败测试**

`list_options.rs` 的 `mod tests` 验证分页输入映射：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_clamps_to_framework_limits() {
        let input = ListOptionsInput { page: Some(0), page_size: Some(0), source_key: None, search: None };
        let (page, size) = input.normalized_paging();
        assert_eq!(page, 1, "page 不得小于 1");
        assert_eq!(size, 20, "page_size 为 0 时回退默认值");
    }

    #[test]
    fn page_size_never_exceeds_framework_cap() {
        let input = ListOptionsInput { page: Some(1), page_size: Some(1000), source_key: None, search: None };
        let (_, size) = input.normalized_paging();
        assert_eq!(size, 100, "必须夹在 MAX_QUERY_PAGE_SIZE 内");
    }
}
```

- [ ] **Step 2: 实现 5 个 Action**

每个文件遵循同一形态（`pub(super) async fn handle` + `pub(super) fn register`），用
`action_fn(...).permissions([...]).register()`。要点：

- 路由显式 `/api/v1/feishu/...`；`list_*` 用 `HttpMethod::Post` 且路径以 `/query` 结尾
  （与框架内置 `SelectAction` 的标准分页契约一致），返回
  `{ items, page, page_size, total }` 结构，以便直接作为 `ViewSpec` 的 `data_action`。
- **必须显式 `select_fields(...)`**：`ensure_readable_projection` 是框架 crate 私有的，
  自定义列表 Action 不会自动拿到「默认可读投影」。
- 数据源写操作：新建/更新时接收 token **明文**，入库前 `hash_token`；更新时若未提供
  token 则保留原值。写操作走事务 + 同事务审计（`audit::succeeded_event`，因为有登录身份）。
- `delete_datasource` 同时把该数据源下的选项置 `enabled=false`（同事务）。

- [ ] **Step 3: 加 `ViewSpec` 实现零代码 TableView**

在 `datasource/mod.rs` / `option/mod.rs` 的 `build_module` 里，`ModuleSpec` 上链式加
`.view(...)`：`data_action` 指向对应列表 Action，`field(...)` 列出要展示的列，
`present_action(...)` 挂创建/编辑/删除操作。参照
`project/yang-system/src/addon/demo/notes/mod.rs` 的既有写法。

> 构建期会交叉校验 View 引用的 Action、字段能力位与 `response_kind`
> （`compile.rs:245-252` 要求 View 的 `data_action` 必须是 `ActionResponseKind::Json`，
> 即普通 JSON 响应——列表 Action 满足）。

- [ ] **Step 4: 跑单测、架构门禁与冒烟测试**

```bash
cd /d/code/lib_yang/project/yang-system && cargo test --lib --locked
cd /d/code/lib_yang/project/yang-system && python scripts/check_architecture.py
```
Expected: passed。

- [ ] **Step 5: 重新生成 OpenAPI 契约产物**

```bash
cd /d/code/lib_yang/project/yang-system && python scripts/dump_openapi.py
cd /d/code/lib_yang/project/yang-system && pnpm --dir frontend exec openapi-typescript contracts/openapi.json -o src/engine/contracts/api-types.ts
```
Expected: 两个产物更新且被提交。**注意这两个生成物的新鲜度没有 CI 门禁**，只能靠这一步人工保证。

- [ ] **Step 6: 跑前端门禁**

```bash
cd /d/code/lib_yang/project/yang-system && pnpm --dir frontend check
```
Expected: passed（含 typecheck、lint、vitest、bundle 预算）。

- [ ] **Step 7: 提交**

```bash
cd /d/code/lib_yang/project/yang-system && git add src/addon frontend/contracts frontend/src/engine/contracts && git commit -F - <<'EOF'
feat(feishu): 前端控制台查询接口与零代码 TableView 投影

数据源 CRUD 与选项列表查询，声明 feishu.datasource.read/write 与 feishu.option.read
权限，并挂 ViewSpec 让通用 ModulePage/TableView 直接渲染完整页面（前端零代码）。

选项在本入口只读：增改由多维表格写入 API 承担，避免两个并存的可写入口让审计语义
与数据来源分叉。列表 Action 显式 select_fields——ensure_readable_projection 是框架
crate 私有的，自定义列表 Action 不会自动获得默认可读投影。

同步重新生成 OpenAPI 契约产物（该产物无 CI 新鲜度门禁，须人工保证）。
EOF
```

---

## Task 11: 集成测试、初始授权与飞书联调

**Files:**
- Create: `project/yang-system/tests/feishu_options_integration.rs`
- Modify: `project/yang-system/scripts/run_ci.py`（登记 `INTEGRATION_COMMANDS`）
- Modify: `project/yang-system/docs/contracts/AUTHZ_GRANTS.md`（初始授权示例）

**Interfaces:**
- Consumes: 前面全部
- Produces: 端到端证据

- [ ] **Step 1: 写集成测试**

`tests/feishu_options_integration.rs`，头部照既有测试的硬性要求：

```rust
//! 飞书外部选项链路的端到端集成测试。
//!
//! 需要 `YANG_SYSTEM_TEST_DATABASE_URL`（库名以 `_test` 结尾）与
//! `YANG_SYSTEM_TEST_REDIS_URL`（DB 15），运行方式：
//! `cargo test --test feishu_options_integration -- --ignored --test-threads=1`

// 该文件必须包含字符串 "YANG_SYSTEM_TEST_" 才会被 run_ci.py 反向发现并要求登记
```

必须自行断言库名与 Redis DB：

```rust
fn test_database_url() -> String {
    let url = std::env::var("YANG_SYSTEM_TEST_DATABASE_URL")
        .unwrap_or_else(|_| panic!("需要 YANG_SYSTEM_TEST_DATABASE_URL"));
    assert!(url.ends_with("_test") || url.contains("_test?"), "库名必须以 _test 结尾");
    url
}
```

覆盖用例（每个都带 `#[ignore = "需要 YANG_SYSTEM_TEST_DATABASE_URL 与 YANG_SYSTEM_TEST_REDIS_URL"]`）：

1. **建数据源 → 写选项 → 以正确 Token 取选项**：断言响应是裸 `{code,msg,data}`、
   含 `options` 与 `i18nResources`、`options[].value` 是 `@i18n@<id>`。
2. **错误 Token**：断言 HTTP 200 且 `code != 0`（不是 401/500）。
3. **数据源停用**：断言失败信封。
4. **禁用选项不出现**：把某选项置 `enabled=false` 后断言不在结果中。
5. **关键词检索**：`query` 命中 `label`，未命中词返回空 `options` 但 `i18nResources` 仍非空。
6. **分页**：造 3 条，`page_size` 生效后 `hasMore=true` 且 `nextPageToken` 可推进到下一页；
   非法 `pageToken` 返回非 0 code。
7. **加密路径**：数据源开 `encrypt_enabled` 且配置了 `encryption_key` 时，
   `data.result` 是字符串；用同一密钥解密后应等于明文 `result` 的 JSON。
8. **写入幂等**：同一批 upsert 调两次，选项行数不变。
9. **审计留痕**：写入后查 `audit_event` 表，断言有对应记录且 actor 是 system。

> 清理必须白名单化：照 `tests/schema_apply_integration.rs:29-40` 的 `drop_test_table`
> 模式，对未声明的表名 `bail!`，不要内联拼接表名。

- [ ] **Step 2: 登记 integration 元组**

在 `project/yang-system/scripts/run_ci.py` 的 `INTEGRATION_COMMANDS` 中追加：

```python
    Command(
        "feishu options integration",
        ("cargo", "test", "--test", "feishu_options_integration", "--locked", "--", "--ignored", "--test-threads=1"),
    ),
```

- [ ] **Step 3: 自查元组一致性**

```bash
cd /d/code/lib_yang/project/yang-system && python scripts/run_ci.py --self-test
```
Expected: passed。该自检断言 `INTEGRATION` 里的测试名集合与「内容含 `YANG_SYSTEM_TEST_` 的
`tests/*.rs` 文件集」完全相等——不等会直接列出漏登记/多登记项。

- [ ] **Step 4: 跑集成测试**

```bash
export YANG_SYSTEM_TEST_DATABASE_URL="mysql://root:yang-local@127.0.0.1:3306/yang_system_test"
export YANG_SYSTEM_TEST_REDIS_URL="redis://127.0.0.1:6379/15"
cd /d/code/lib_yang/project/yang-system && python scripts/run_ci.py integration
```
Expected: 全部 passed。需要 MySQL 8.0 / PostgreSQL 16 / Redis 7 容器（`compose.yaml`）。

- [ ] **Step 5: 写初始授权文档**

在 `project/yang-system/docs/contracts/AUTHZ_GRANTS.md` 的「初始授权（运维）」节后追加飞书的
一段示例 SQL，把权限换成 `feishu.datasource.read` / `feishu.datasource.write` /
`feishu.option.read`，并强调必须**同事务**完成三件事：写 `authz_grant` 事实行、
递增 `users.authz_version`、追加 `authorization_outbox`。

- [ ] **Step 6: 飞书联调（不可省略）**

在飞书审批管理后台对测试审批定义配好外部选项（URL 指向可公网访问的部署实例 +
自定义 Token），点「校验数据」。**这一步一次性验证四件文档层无法回答的事**：

1. 飞书是否容忍我们返回的 `{code,msg,data}` 信封（我们已是字面严格，但要确认无多余字段导致拒绝）；
2. 配置 Key 后加密返回是否被正确解密；
3. `i18nResources` 与 `@i18n@` 占位符的匹配规则是否如文档所述；
4. 勾选「支持模糊、分页搜索」后飞书实际发来的 `page_token` / `query` 形态。

把实测得到的真实请求体记入 `docs/operations/` 下的联调记录（新建一节即可），
若发现与 spec §3.1 的取证结论不符，回头修订 spec。

- [ ] **Step 7: 跑全量门禁并提交**

```bash
cd /d/code/lib_yang/project/yang-system && python scripts/run_ci.py full
cd /d/code/lib_yang/project/yang-system && git add tests scripts docs && git commit -F - <<'EOF'
test(feishu): 外部选项链路端到端集成测试与初始授权文档

覆盖建数据源、写选项、取选项的完整链路，以及错误 Token、停用数据源、禁用选项过滤、
关键词检索、游标分页、加密返回、写入幂等与审计留痕九条路径。断言响应是裸
{code,msg,data} 且业务失败时 HTTP 仍为 200。

同步登记 run_ci.py 的 INTEGRATION 元组（自检要求它与含 YANG_SYSTEM_TEST_ 的测试文件集
完全相等），并补飞书的初始授权运维 SQL 模板。
EOF
```

---

## 交付后的收尾

- **P7（spec 的 Phase 2）**：把 `domain/` 里的 `protocol.rs` / `crypto.rs` / `token.rs` /
  `pagination.rs` 抽成 `crates/yang-feishu`。届时需同步 A 的 `scripts/run_ci.py`
  的 `AUXILIARY_PACKAGE_FLAGS`（否则 `--self-test` 断言失败）、`ci.yml` 与
  `verify_ci_contract.py` 的 `REQUIRED_FRAGMENTS`；`aes` / `cbc` 也需从 B 迁到 A 的
  `[workspace.dependencies]`。抽取前先确认这些模块的 API 已被真实消费者验证过——
  这正是把 Phase 2 放在最后的理由。
- **推送顺序**：先 `lib_yang`（Task 1 的 yang-base 0.3.0），确认推送完成后再推
  `yang-system`。两侧都用 `--locked`，间隔过近会因锁文件与旧清单不匹配而失败。
- **未决项**（spec §12）：入口 2 的 upsert 条数上限取值、是否需要软删除、是否在
  Phase 2 给 `ActionResponseKind` 增加 `Raw` 变体以修正 OpenAPI 描述。
