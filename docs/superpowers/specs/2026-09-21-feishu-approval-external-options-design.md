# 飞书审批外部数据源 + 飞书能力基础库 — 设计

**日期**：2026-09-21
**状态**：待评审
**影响仓库**：`lib_yang`（`crates/yang-base`）、`project/yang-system`

**前置阅读**：根 `AGENTS.md`、`project/yang-system/docs/guides/ADDON_ONBOARDING.md`、
`project/yang-system/docs/contracts/SCHEMA.md`、`project/yang-system/docs/contracts/CONFIGURATION.md`。

---

## 1. 目标

为飞书审批的**单选/多选控件「关联外部选项」**自建一个数据源服务：

1. 在本系统存储「数据源」与其「选项」数据；
2. 对外暴露飞书要求的外部选项接口，供审批表单在发起/校验时动态拉取选项；
3. 提供写入 API，使**飞书多维表格自动化工作流**能推送数据变更，实现选项自动更新；
4. 把飞书相关的通用能力（协议 DTO、加解密、来源校验）沉淀为可复用资产。

最终形态是「多维表格负责编辑，本服务负责存储与对审批服务」——多维表格是编辑面，
本服务是服务面。

## 2. 非目标

- **不做飞书 OpenAPI 的出站调用。** 本设计的两个数据流向都是「飞书调我们」，我们不需要
  主动调飞书。`tenant_access_token` 缓存、IM 发消息、多维表格记录读写等出站能力不在本期。
  这与本仓库既有纪律一致：`src/addon/account/domain/oidc.rs:28` 的
  `ExternalIdentityProvider` 端口「等第一个真实 SSO 需求出现时再落实现」。
- 不做审批实例的创建 / 查询 / 处理（属三方审批，是另一条契约）。
- **不做联动参数（`linkage_params`）的过滤语义**，仅在数据模型上预留字段。
- 不做多租户（本仓库当前无租户域）。

## 3. 关键前置事实

本节记录设计所依赖的、**已经取证**的硬事实。实现时若与之冲突，以本节为准并回头修订本节。

### 3.1 飞书侧契约

取证方式：5 路独立检索（官方文档多版本 / 开源实现 / 社区排障 / 协议镜像 / 主动反证）
+ 14 条对抗式复核。结论分布：10 confirmed / 3 refuted / 1 uncertain。

**已被一手原文证实的事实：**

1. **官方文档的「返回参数」表只有三列**（`参数|类型|说明`），**没有「是否必须」列**；
   而同一篇文档的「请求参数」表是四列（`参数|类型|是否必须|描述`），且把 `token` 标为「是」。
   飞书在需要声明必填时会显式声明，对返回体则刻意不设该列。
2. **全文档唯一被明示「必须返回」的响应字段是 `i18nResources`**，且写明了后果：
   「i18nResources 必须返回，返回空会导致显示是空的，请至少返回一种语言数据」。
   `msg` 的说明只有一句「返回码的描述」，既未标必填，也未说会被展示或用作错误提示。
3. **`code` 是唯一的成败信号**：「错误码，非 0 表示失败。」
4. **文档没有独立的「错误码」章节**，不存在「返回体不合规会报某某错误码」的说明；也没有
   任何「字段名必须完全一致」「解析失败」之类的明文表述。
5. **`data.result` 是多态的**：未配置 Key 时是对象，配置了 Key 时是 base64 字符串。
   消费方不可能用单一固定结构体严格反序列化。
6. **存在一篇反证文档**：`historic-version/approval/v2/third-party-approval-integration/
   external-approval-instance-create.md` 的**响应体表带「必须」列，且把 `msg` 标为「是」**。
   这证明飞书在需要声明返回字段必填时确实会标——反过来使「关联外部选项」省略该列成为
   一个有信息量的差异，而非全局惯例。

**被证伪、不得引用的论据（避免后续误用）：**

- ❌ **「文档对返回体的字段名保持沉默」是错的。** 文档逐字列出了 `code`/`msg`/`data`，
  且在明文与加密两个示例中都写了顶层 `"msg"`。
- ❌ **「5 个文档版本逐字一致」是重复计数。** `uADM4QjLwADO04CMwgDN.md` 已 302 归并到
  当前版（字节完全相同）；Lark 国际版与 apifox 镜像是同一份旧版。**实际只有 2 份独立正文**：
  新版（飞书 CN + en-US）与旧版（Lark 国际版 + apifox 镜像）。

**版本差异（以当前中文版为准）：**

- 旧版（Lark 国际版）把 `user_id`/`employee_id` 标为「是」（必填），当前中文版标「否」。
  文档正文明确「如果不传 user_id 和 employee_id，表示期望返回所有的数据」。
  ⇒ **两个参数按可选处理**，不得依赖其存在。
- 旧版缺 `page_token`/`query`/`hasMore`/`nextPageToken`。

**⚠️ 已知的检索陷阱（必须记住）：**

多个搜索引擎与 AI 摘要层在转述这篇文档时会**自信地断言「msg 必填」「字段名写成 message
会解析失败」**。逐字核对官方原文后确认，**这些句子在原文中根本不存在**，是摘要模型的
推断幻觉。任何中文博客转述「官方要求 msg」都不可采信，必须回原文核验。

**仍未解决、无法从文档判定的：**

- 文档从未要求 `msg` 必填，但**也从未声明「`msg` 可省略」**。因此「省略 `msg` 一定安全」
  同样是无来源的断言。**唯一有文档背书的选择就是照文档示例原样返回 `code`/`msg`/`data`。**
  这是本设计选择「为飞书单独定义类型、走字面严格契约」的直接理由。
- 失败路径（`code != 0`）时飞书是否读取 `msg` 做提示，官方完全未定义。
- **文档层结论 ≠ 运行时行为。** 以上证明的是「契约未要求」，不是「运行时容忍缺失」。

### 3.2 框架侧约束

| 事实 | 锚点 |
|---|---|
| Action 成功响应恒被收口为 `{code,message,data}`，结构体字段名无 `serde(rename)` | `crates/yang-base/src/action/response.rs:41-78` |
| `ResponseBody` 只有 `Download{path,filename}` / `Preview{path}` / `Redirect{url}`，**是文件路径不是字节**，无裸 body 通道 | `response.rs:380-398` |
| `DynAction::dispatch` 返回类型被钉死为 `Result<ApiResponse, BaseError>` | `action/typed.rs:196` |
| 传输层**只有一处**渲染附件；无附件时才套 `Json(ApiResponse)` | `transport/axum.rs:506-511`、`axum.rs:1010` |
| 输出收口 `wrap_dispatch_output`：`ApiResponse` 透传 / `ResponseBody` 转附件 / 其余进 `data` | `response.rs:459-471` |
| **函数式通道与 derive 通道同构**，`FnAction::dispatch` 同样调用 `wrap_dispatch_output` | `action/functional.rs:83` |
| `public` Action 跳过 `TokenAuthMiddleware` + `authorize()`，但日志/限流/tracing/租户中间件照跑 | `router/middleware.rs:206-209`、`:59-62` |
| `Middleware::handle` 取得 `ctx` 所有权；`ActionContext::actor()` / `user_roles_set()` **公开可读**，但 `with_user` / `ctx.user` 是 `pub(crate)`，**中间件无法注入身份** | `router/middleware.rs:167`、`action/context.rs:249/499` |
| 内置 `get`/`select` 在 handler 内硬性要求已登录用户，**匿名端点用不了** | `action/builtin/get.rs:47`、`select.rs:104` |
| `TableQuery` 分页硬上限 100；`prefetch_limit` 是 `pub(crate)` | `table/query_params.rs:49` |
| 字段级权限**默认 fail-open**（`filterable`/`sortable` 默认 true），而 `searchable` 默认 false | `table/field_config.rs`、`table_query/filters.rs:543` |
| `search` 对「无任何可搜索文本字段」的角色 fail-closed 报错 | `table_query/filters.rs:543` |
| `ensure_readable_projection` 是 crate 私有，自定义列表 Action **必须显式 `select_fields(...)`** | `ADDON_ONBOARDING.md` 摩擦点 ② |
| `Registry::dispatch` 只注入**所在 module 的主表** | `definition/builder/registry.rs:318` |
| `Module::table()` 只返回一张表 | `definition/interface.rs:298` |
| `params!` 只支持 `Str/Text/Decimal/Password/Key/Int/Table/Tree/Timestamp/Switch/Radio`，**拒绝 `serde(rename)`**，表达不了 Map | `yang-base-derive/src/params.rs:105-110, 320-330` |
| Schema 同步**纯增量、无回滚**；主键建了不可改；**不支持复合主键** | `database/schema_sync/plan.rs:284-289`、`render.rs:87-90` |
| 非自增自然键主键受完整支持 | `project/yang-system/src/infrastructure/schema.rs:213,270` |
| handler 返回 `Err` 记 `result="error"` 烧可用性预算；`Ok(ApiResponse{code≠0})` 记 `business_error` 不烧 | `yang-runtime/src/observability/logging.rs:170-215`、`docs/contracts/SLO.md` |
| burn-rate 规则按 `sum by (job)` 聚合，**不带 operation 维度** | `ops/prometheus/yang-system.rules.yml` |
| 生产边缘只反代 `/(api\|\.well-known\|health)`，且该正则被逐字冻结为部署门禁 | `frontend/deploy/nginx.conf:51`、`frontend/scripts/verify-deployment-contract.mjs:76-100` |
| `yang-system` 依赖 yang-base 时**未启用 `http` feature** | `project/yang-system/Cargo.toml:25` |
| `aes`/`cbc`/`cipher` 在 lib_yang 的 441 个锁定包中**零命中**；`sha2 0.10.9` / `base64 0.22.1` 已在 workspace | `Cargo.lock`、`Cargo.toml:29,32` |
| 新增 crate 会让 `run_ci.py --self-test` 断言失败（`crates/*` glob 与 `-p` 列表全等） | `scripts/run_ci.py:382-407` |
| 跨仓库推送必须 **lib_yang 先、yang-system 后**（CI 用 `--locked`） | `project/yang-system/AGENTS.md:128` |

### 3.3 门禁形状约束（`project/yang-system/scripts/check_architecture.py`）

- `src/` 顶层只允许 `{addon, config, infrastructure}` 目录与 `{app.rs, bootstrap.rs, lib.rs, main.rs}` 文件。
- module 目录只允许 `{mod.rs, table.rs, actions, domain}` 四个条目；**机制代码一律进 `domain/`**。
- `actions/*.rs` 每个文件**恰好一个** `pub(super) async fn handle(` + **恰好一个** `pub(super) fn register(`，
  且必须进 `actions/mod.rs` 的 `ACTIONS` 注册表数组。
- **禁止在 `src/addon/**/actions/*.rs` 使用 `#[derive(Action)]`**，统一走
  `action_fn(..).register()` 函数式通道。
- module 命名**必须避开 `org` 与 `work`**——这两个名字会激活租户隔离门禁，强制要求
  `docs/architecture/tenant-data-paths.md` + 隔离集成测试 + 三方对齐。
- 裸 SQL 只能落在 `src/addon/**/repository.rs` 等白名单路径，并同步
  `docs/architecture/raw-sql-boundaries.md`。**本设计优先用 `TableQuery`，不引入裸 SQL。**
- **已知门禁可信度缺陷**（`docs/audit/2026-09-20-whole-repo-review.md:68,236`）：
  `check_architecture.py:309` 的 `production_source()` 在文件内**第一个** `#[cfg(test)]` 处截断，
  4 个文件共 3265 行生产代码对源码扫描不可见。**不得把「门禁通过」当作合规证明**；
  即本设计的测试必须写在文件末尾（常规位置），不得依赖该缺陷。

## 4. 架构

### 4.1 落位

本方案分两个阶段（对应前期讨论的「方案 C」）：

- **Phase 1**：业务全部落在 `project/yang-system/src/addon/feishu/`；
  同时给 `crates/yang-base` 增加一个通用响应能力（§4.4）。
- **Phase 2**：把飞书通用能力（协议 DTO、加解密、来源校验）抽成 `crates/yang-feishu`，
  按 `lib_yang` 的 CI 契约同步脚本并跨仓推送。

Phase 1 之所以仍然要动 `lib_yang`，是因为飞书的字面严格契约在框架内**无法实现**（§4.4）。

### 4.2 目录结构

```text
project/yang-system/src/addon/feishu/
├── mod.rs                  # build_addon()，addon 唯一对外入口
├── domain/                 # 机制代码的唯一合法居所（门禁强制）
│   ├── mod.rs
│   ├── context.rs          # FeishuContext：聚合两个 Repository + finish_transaction
│   ├── repository.rs       # 两张表的唯一持久化边界（受信 writer）
│   ├── protocol.rs         # 飞书请求/响应 DTO，键名逐字对齐
│   ├── crypto.rs           # AES-256-CBC + SHA-256(key) + Base64
│   ├── token.rs            # 数据源 Token 的 sha256 与常量时间比较
│   ├── i18n.rs             # @i18n@ 占位符与 i18nResources 构造
│   └── middleware.rs       # 静态 API Token 校验中间件（入口 2）
├── datasource/             # module 1：数据源注册表
│   ├── mod.rs
│   ├── table.rs            # feishu_datasource 表声明
│   └── actions/            # 前端管理用（受保护 Action）
└── option/                 # module 2：选项数据
    ├── mod.rs
    ├── table.rs            # feishu_option 表声明
    └── actions/            # 前端查询 + 两个机器入口
```

**为什么是两个 module。** `Module::table()` 只返回一张表（`interface.rs:298`），而
`Registry::dispatch` 只把**所在 module 的主表**注入 `ActionContext`（`registry.rs:318`）。
因此公开的取选项动作必须住在拥有 `feishu_option` 表的那个 module 里，否则
`ctx.table_query()` 直接返回 `TableDefinitionNotSet`。跨表访问（`option` module 读
`feishu_datasource`）由 `domain/context.rs` 在 `build_addon` 时同时持有两张
`TableDefinition` 解决——这是 `account` addon 已有的模式。

### 4.3 三个 HTTP 入口

| # | 路由 | 调用方 | 响应形态 | 鉴权 |
|---|---|---|---|---|
| 1 | `POST /api/v1/feishu/approval/options/{source_key}` | **飞书审批** | `Output = ResponseBody` → `ResponseBody::raw()`，**字面严格 `{code,msg,data}`** | 数据源 Token（库存 `sha256` + `subtle` 常量时间比较） |
| 2 | `POST /api/v1/feishu/inbound/options/upsert`<br>`POST /api/v1/feishu/inbound/options/delete` | **多维表格自动化工作流** | 标准框架包络（机器消费足够） | 静态 API Token（config secret），自定义 `Middleware` |
| 3 | 数据源 CRUD + 选项查询 | **前端控制台** | 标准框架包络 | 框架 JWT 会话 + `feishu.*` 权限 |

**为什么入口 1 必须独立**：框架的 Action 响应恒为 `{code,message,data}` 且键名不可改
（§3.2），而飞书文档的示例是 `{code,msg,data}`。虽然取证表明 `msg` 并非契约必需，但
「省略 `msg` 安全」同样无来源（§3.1），**照文档原样返回是唯一有背书的选择**。

**为什么入口 2 不能复用入口 3**：静态 Token 的调用方没有 JWT，过不了
`TokenAuthMiddleware`；而 `Middleware` 虽然能**读**身份（`actor()` / `user_roles_set()` 公开），
却**不能注入**身份（`with_user` / `ctx.user` 是 `pub(crate)`），所以无法让静态 Token 走
受保护 Action。

三个入口共用同一份 `domain/` 逻辑，只有 HTTP 外壳分叉。

### 4.4 `lib_yang` 侧改动：`ResponseBody::Raw`

**这是本设计唯一触碰基础库的地方**，也是一次通用能力增强而非飞书专用改装。

现状（§3.2）：传输层**只有一处**决定是否套框架包络——

```rust
// transport/axum.rs:506-511
let mut http_response = match response.attachment.clone() {
    Some(attachment) => attachment_response(attachment, state.max_attachment_bytes).await,
    None => (status, Json(response)).into_response(),   // ← 只有无附件时才套 ApiResponse
};
```

`attachment_response`（`axum.rs:1010`）对 `Redirect` / `Download` / `Preview` 逐个穷尽 match。
因此**只要给 `ResponseAttachment` 增加一个变体，HTTP body 就再也不会被套进框架包络**。

改动清单：

1. `action/response.rs`
   - `ResponseBody` 新增 `Raw { body: String, content_type: String }` 与构造器
   - `ResponseAttachment` 新增同构变体（两者按 `response.rs:420-423` 的注释是「结构一一对应」）
   - `From<ResponseBody> for ResponseAttachment` 补一个 match 臂
   - `wrap_dispatch_output`（`response.rs:459`）**无需改动**，它已经统一做转附件
   - **body 用 `String` 而不是 `Vec<u8>`**：`ResponseBody` 是 Action 的 Output 类型，必须满足
     `Serialize + JsonSchema`。`Vec<u8>` 会序列化成整数数组、schema 变成 `array of integer`；
     既然 content-type 已被限死为 `application/json`（UTF-8 文本），`String` 更诚实也更干净。
2. `transport/axum.rs:1009` 的 `attachment_response` 补一个臂

   ```rust
   ResponseAttachment::Raw { body, content_type } => match HeaderValue::from_str(&content_type) {
       Ok(value) => (
           StatusCode::OK,
           [(header::CONTENT_TYPE, value)],
           body,
       )
           .into_response(),
       Err(_) => error_response(
           StatusCode::INTERNAL_SERVER_ERROR,
           BaseError::ConfigError("Raw 附件响应 content-type 非法".to_string()),
       ),
   },
   ```

3. `definition/builder/registry.rs:402` 的 `warn_response_kind_mismatch` 必须补臂。
   它按 `&response.attachment` 穷尽匹配并把 attachment 映回 `ActionResponseKind`——
   这是**第三处**必须同步的穷尽 match（前两处是上面两个）。`Raw` 映射为
   `ActionResponseKind::Json`（body 确实是 JSON），与 Action 声明的 `Json` 一致，不触发告警。

   注意 `infer_action_presentation`（`compile.rs:464`）与 `validate_action_presentation`
   （`compile.rs:557`）匹配的是 **`ActionResponseKind` 而不是 attachment**，因为本方案不新增
   kind 变体，**这两处不需要改**。同理 `ActionResponseKind` 有 `#[non_exhaustive]`，
   前端 zod 契约（`ui-catalog.ts:49` 的 `.enum([...]).catch("json")`）对未知值静默降级，
   **前端零影响**。

4. **两个 enum 同时加 `#[non_exhaustive]`**：既然本次已是破坏性变更（两者当前都不是
   `#[non_exhaustive]`），一并加上，使未来新增变体不再构成破坏性变更。注意
   `#[non_exhaustive]` 只对**其它 crate** 生效，crate 内仍是穷尽匹配——所以上面三处照旧必须改。

5. **两个真实的陷阱，必须处理**：
   - `append_action_response_headers`（`axum.rs:531`）的拒绝名单逐字为
     `content-length | transfer-encoding | connection | x-request-id`，**不含 `content-type`**，
     且用 `target.append(...)` 而非 `insert`。⇒ 若 Action 再用 `with_header("Content-Type", …)`
     会产出**重复的 Content-Type 头**。本方案的 Raw 动作不得声明 Content-Type 头。
   - `max_attachment_bytes` 目前只在 `file_response` 内校验（`axum.rs:1050-1069`），
     Raw 变体不经过它，**必须自己比对 `body.len()`**，否则完全绕过上限（默认 64 MiB）。
3. **两个 enum 同时加 `#[non_exhaustive]`**：既然本次已是破坏性变更（两者当前都不是
   `#[non_exhaustive]`），一并加上，使未来新增变体不再构成破坏性变更。
4. **安全硬化**：`Raw` 能让任意 Action 返回任意字节与任意 Content-Type。若放任，未来可用它
   返回 `text/html` 从而绕过前端 JSON 契约的保护。**`Raw` 构造期只允许 `application/json`
   （或一个显式白名单），非法 content-type 返回 `BaseError`**，并在 crate 文档中写明它是给
   「外部系统契约与框架包络不兼容」用的逃生口，不是通用响应通道。
5. **版本边界**：`yang-base` `0.2.2 → 0.3.0`。连锁同步：
   `crates/yang-base/tests/compatibility_contract.rs`、`release_docs_contract.rs` 读取的
   `docs/RELEASE_CANDIDATE_REPORT.md` 与 `docs/YANG_BASE_DB_COMPLETENESS_PLAN.md`、
   `README.md`、`docs/reference/yang-base.md`，以及 yang-system 的版本钉与 `Cargo.lock`。
   参考先例：`f3a70bd fix(test): 兼容性契约测试版本断言更新到 0.2.2/0.1.6`。

**不做的**：不给 `ActionResponseKind` 加 `Raw` 变体。它被投影进 UI Catalog 并由前端 zod
白名单消费，加变体会牵动前端契约；且静态 kind 保持 `Json` 是诚实的（body 确实是 JSON）。
代价是该端点在 OpenAPI 里被描述成框架包络形状，文档不精确——留待 Phase 2 评估。

## 5. 数据模型

**表声明走 `TableSpec` + `fields!` DSL。** 这不是风格选择——它是「进 Catalog / UI / 权限目录」
与「拿不到 DSL 能力」之间的取舍，见 §5.3。

### 5.1 `feishu_datasource`（module `feishu.datasource`）

| 字段 | DSL 声明 | 说明 |
|---|---|---|
| `id` | `Key::new()` | → `Field::id`：自增大整数主键、`not_writable` |
| `source_key` | `Str::new().require(true).unique(true).max_length(64).searchable(true).filterable(true).sortable(true)` | 进 URL；规则 `^[a-z][a-z0-9_]{0,63}$` |
| `title` | `Str::new().require(true).max_length(100).searchable(true)` | 展示名 |
| `token_hash` | `Str::new().require(true).max_length(64).secret(true).readable_by([SYSTEM_ROLE]).writable_by([SYSTEM_ROLE])` | 只存 sha256 十六进制，**永不存明文** |
| `encrypt_enabled` | `Switch::new().require(true).default(false)` | |
| `default_locale` | `Str::new().require(true).max_length(16).default("zh_cn")` | |
| `status` | `Radio::<String>::new().require(true).varchar(16).options([("active","启用"),("disabled","停用")]).default("active")` | 另加 `check_named` 兜底 |
| `linkage_mapping` | `Text::new()` | JSON 文本（DSL 无 Json builder）；预留，v1 不消费 |
| `created_at` / `updated_at` | `Timestamp::new().created_at()` / `.updated_at()` | 框架自动写入 |

### 5.2 `feishu_option`（module `feishu.option`）

| 字段 | DSL 声明 | 说明 |
|---|---|---|
| `id` | `Key::new()` | 自增大整数主键 |
| `option_id` | `Str::new().require(true).unique(true).max_length(128).searchable(true).filterable(true).sortable(true)` | **飞书契约 id**；唯一性由唯一索引强制 |
| `source_key` | `Str::new().require(true).max_length(64).indexed(true).filterable(true).sortable(true)` | 所属数据源 |
| `label` | `Str::new().require(true).max_length(255).searchable(true)` | 飞书 `query` 关键词检索依赖它 |
| `i18n` | `Text::new()` | JSON 文本 `{"en_us":"…","ja_jp":"…"}` |
| `sort_order` | `Int::new().require(true).default(0).sortable(true)` | 稳定排序键 |
| `is_default` | `Switch::new().require(true).default(false)` | |
| `enabled` | `Switch::new().require(true).default(true).filterable(true)` | 禁用而非删除，避免历史审批单失联 |
| `extra` | `Text::new()` | JSON 文本；预留联动筛选键值 |
| `created_at` / `updated_at` | `Timestamp::new().created_at()` / `.updated_at()` | |

**关于 `option_id` 的 `searchable(true)`**：`TableQuery::search` 会对「所有 `searchable` 且为
文本类型」的字段构造一个 OR-LIKE 组（`table_query/filters.rs:543`）。把 `option_id` 纳入搜索面，
是为了让审批人能直接按选项编码检索（多数外部系统的主键本身就是有意义的编码）。代价是关键词
会同时匹配 id 与 label。若不希望如此，去掉该 `searchable(true)` 即可——但**必须至少保留
`label` 的 `searchable(true)`**，否则当前角色没有可搜索文本字段时 `search` 会 fail-closed 报错。

### 5.3 关于 DSL 能力的三个硬约束（决定了上面的写法）

`fields!` + `TableSpec` 是**进 Catalog / 前端零代码 TableView / 权限目录的唯一通道**
（`ModuleSpec::table` 只接受 `TableSpec`），但它有三个能力缺口，实测确认：

1. **没有 Json builder。** `simple_builder!` 只实例化 9 个（`Key` / `Str` / `Text` / `Int` /
   `Decimal` / `Switch` / `Table` / `Tree` / `Timestamp`，`field.rs:595-603`），
   `definition/mod.rs:32` 的导出列表里也没有 `Json`；`Field::json` 只存在于 schema-first 侧
   （`table/definition.rs:172`）。⇒ `i18n` / `extra` / `linkage_mapping` **只能落成 `Text`
   列存放 JSON 文本**，由 `domain/` 自己 serde 序列化。这三个列从不被 SQL 查询进内部，
   代价可接受。
2. **无法声明自然键主键。** DSL 的 `Key` 硬编码映射到 `Field::id(name)`，即
   `required().primary_key().auto_increment().not_writable()`（`table/definition.rs:113`），
   builder 上也没有 `primary_key()`。⇒ `option_id` **不能做单列主键**，改为
   `Str::new().unique(true)`——`FieldSpec.storage.unique` 在 `field.rs:276-277` 映射为
   schema-first 的 `Field::unique()`，落成真实 UNIQUE 索引。**约束力等价**：
   飞书要求的「id 全局唯一且固定」由唯一索引强制，与由主键强制没有区别。
3. **`filterable` / `sortable` 在 DSL 侧是 fail-closed 的。** `FieldSpec::into_schema_field`
   对未声明的字段会显式调 `not_filterable()` / `not_sortable()` 关闭表层默认
   （`field.rs:311-320`）。⇒ 每个需要筛选或排序的字段都必须显式打开，漏一个就是运行时
   `FieldPermissionDenied`。

另外两条会让实现写错的差异：

- **`secret(true)` 会把 readable/writable 同时置为 `Nobody`**（`field.rs:539-546`），
  所以 `token_hash` 后面**必须**跟 `.readable_by([SYSTEM_ROLE]).writable_by([SYSTEM_ROLE])`，
  否则连受信 writer 都读写不了。
- **DSL 与 schema-first 的参数形状相反**：DSL 的 `require(bool)` / `unique(bool)` /
  `searchable(bool)` / `readable(bool)` 都收 `bool`；schema-first 的 `Field::required()` /
  `unique()` / `searchable()` 都不收参数。混用即编译错误。

### 5.4 必须现在就说清的两个取舍

1. **`option_id` 全局唯一 ⇒ 一个选项只能属于一个数据源。** 这正是飞书契约的强制实现；
   若将来要把同一选项复用到多个数据源，需要引入连接表。
2. **Schema 纯增量、无回滚**，主键一旦建立不可改（`plan.rs:284-289`），且不能事后补自增主键。
   ⇒ 这两张表的结构必须**一次设计到位**。注意唯一索引同样不可事后改定义
   （`plan.rs:308-317` 对同名不同定义的索引直接拒绝启动）。

## 6. 接口契约

### 6.1 入口 1：飞书审批外部选项

**路由**：`POST /api/v1/feishu/approval/options/{source_key}`
（路径必须落在 `/api` 下，否则生产 nginx 不反代——`nginx.conf:51`）

**请求**（`Content-Type: application/json`，飞书超时 3 秒）：

```json
{
  "user_id": "123",
  "employee_id": "abc",
  "token": "1e8e999f...",
  "linkage_params": { "key1": "value1" },
  "page_token": "xxxxx",
  "query": "北京",
  "locale": "zh_cn"
}
```

**请求类型单独定义，手写 `ParamInput`**（`params!` 表达不了 `linkage_params` 这个 Map，
且禁止 `serde(rename)`）。字段名逐字对齐。

- **不设 `deny_unknown_fields`**：飞书将来新增字段不应打挂我们，且该 DTO 没有内部字段
  可供注入，拒绝未知字段在这里只有可用性风险、没有安全收益。
- `user_id` / `employee_id` **按可选处理**，两者都空表示返回全部（§3.1 版本差异）。
- `token` 是必填（文档唯一标为「是」的请求参数）。

**响应（未配置 Key）**：

```json
{
  "code": 0,
  "msg": "success!",
  "data": {
    "result": {
      "options": [ { "id": "options_1_id_1", "value": "@i18n@options_1_name_1", "isDefault": true } ],
      "i18nResources": [
        { "locale": "zh_cn", "isDefault": true,  "texts": { "@i18n@options_1_name_1": "值1" } },
        { "locale": "en_us", "isDefault": false, "texts": { "@i18n@options_1_name_1": "value1" } }
      ],
      "hasMore": true,
      "nextPageToken": "xxxx"
    }
  }
}
```

**响应（配置了 Key）**：`data.result` 为 base64 字符串。

```json
{ "code": 0, "msg": "success!", "data": { "result": "tKqgkBNFEzakJAeS..." } }
```

**实现要点：**

- 类型定义：
  ```rust
  #[derive(Serialize)]
  struct FeishuEnvelope { code: i32, msg: String, data: FeishuData }
  #[derive(Serialize)]
  struct FeishuData { result: FeishuResult }
  #[derive(Serialize)]
  #[serde(untagged)]
  enum FeishuResult { Plain(FeishuOptions), Encrypted(String) }
  ```
- `value` = `@i18n@<option_id>`（`option_id` 全局唯一且固定，天然满足占位符要求）。
- **`i18nResources` 永远至少回一种语言**——这是文档唯一明示的必传项，返回空会导致
  控件「显示是空的」。至少回 `default_locale` 一条并标 `isDefault: true`。
- **分页**：`page_token` 由本服务自行编码（keyset：`(sort_order, option_id)` 的 base64），
  内部按 ≤100 翻页（框架硬上限）。`hasMore` 为 true 时才返回 `nextPageToken`。
- **HTTP 状态恒 200**，成败由 `code` 承载。非 200 会让飞书按「接口报错」处理，
  而 FAQ 对失败现象的归因正是「外部数据源返回接口报错，所以获取选项失败」。
- **3 秒预算**：内部约 2.5s 主动收口，返回一个合法的 `{code≠0, msg}` 错误包络，
  而不是被飞书掐断留下无响应的黑洞。
- `linkage_params` 收到时**忽略**（v1）。

### 6.2 入口 2：多维表格写入

- `POST /api/v1/feishu/inbound/options/upsert` —— 批量幂等 upsert，body 带
  `source_key` + `options[]`（`id` / `label` / `i18n` / `sort_order` / `is_default` / `enabled`），
  单次条数设显式上限。
- `POST /api/v1/feishu/inbound/options/delete` —— 按 `id` 批量禁用或删除。

**鉴权**：配置中的静态 API Token，由 `domain/middleware.rs` 的
`Middleware::handle` 校验（`target_action()` 限定到这两个 Action，`scope()` 用
`MiddlewareScope::AllActions`）。比较同样走常量时间。

**风险与缓解**：这两个 Action 是 `public`，若中间件漏挂就会裸奔。缓解：
① 构建期/测试期断言这两个 Action 的 `target_action` 中间件存在；
② 在 `app.rs` 的冒烟测试中断言「无 Token 调用返回失败」。

### 6.3 入口 3：前端控制台

- 数据源 CRUD + 列表；选项列表查询。
- **本入口对选项只读**：选项的增改由入口 2（多维表格工作流）承担，紧急订正则走运维 SQL。
  这是刻意的——本项目的定位是「多维表格负责编辑，本服务负责存储与对审批服务」，
  两处并存的可写入口会让审计语义与数据来源分叉。若后续需要控制台可编辑选项，
  应新增受保护 Action 并复用同一份 `domain/` 写入逻辑，而不是放宽入口 2 的鉴权。
- 声明 `.permissions(["feishu.datasource.read"|".write", "feishu.option.read"])`
  （权限字符串须匹配 `^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+$`）。
- 自定义列表 Action **必须显式 `select_fields(...)`**（`ensure_readable_projection` 是
  crate 私有），并遵循标准分页契约
  （`page/page_size/search/where/order_by/count_total` → `items/page/page_size/total`）,
  以便直接作为前端通用 `TableView` 的 `data_action`。
- **首个授权由运维 SQL 完成**（决策 D2 无自提权路径），模板见
  `docs/contracts/AUTHZ_GRANTS.md` 的「初始授权（运维）」——须同事务写事实行 +
  递增 `users.authz_version` + 追加 `authorization_outbox`。

## 7. 加解密与安全

### 7.1 AES-256-CBC

- **密钥派生**：`key = SHA-256(配置原文)` → 32 字节。沿用 `account/domain/mfa.rs:39-40` 的既有形态
  （注释明确「密钥取配置原文的 SHA-256 摘要，避免对 hex/base64/原文的形态猜测」）。
- **IV**：`rand_core::OsRng` 生成 16 字节随机值，**前置拼接到密文**，整体
  `base64(STANDARD)` 输出。这与飞书文档的 Go 参考实现一致（解密端取前 16 字节作 IV）。
- **填充**：PKCS#7。**关键**：飞书 Go 参考实现的 `standardizeDataEn` 在明文长度已对齐时
  **仍追加整块 16 字节**（`appendingLen = 16 - len % 16`，对齐时为 16）。RustCrypto 的
  `Pkcs7` 语义一致，但**必须用固定向量把该行为钉死**——这是最容易静默偏离的一处。
- **只加密、不解密**：飞书来文是明文，我们从不解密外部输入 ⇒ **不存在 padding oracle
  暴露面**。CBC 无 MAC 是飞书规范本身的缺口，在文档中注明；**不自行加 HMAC**，
  否则飞书无法解密。
- **依赖**：`aes` + `cbc`（首次进入 lib_yang lock）。`sha2 0.10.9` / `base64 0.22.1` 已在
  workspace。许可均为 MIT OR Apache-2.0，通过 `deny.toml` 白名单。

### 7.2 来源校验

- 数据源 Token **只存 `sha256` 十六进制**，字段声明 `.secret()` + `.not_readable()`。
- 比较用 `subtle::ConstantTimeEq`（`subtle 2.6.1` 已由传递依赖锁在两侧 lock，需显式声明）。
- 校验失败、数据源不存在、数据源被禁用等**业务失败一律返回 `Ok(ApiResponse::fail(...))`**，
  **不得返回 `Err`**：匿名端点的 `Err` 记 `result="error"` 会烧全站 99.9% 可用性预算，
  而 burn-rate 规则按 `sum by (job)` 聚合、不带 operation 维度，会把飞书的探活流量算进
  全站告警。

### 7.3 密钥域隔离

新增的密钥必须与既有 Token / Step-up / email 各验证码 / TOTP 的密钥互不复用，并在
`Settings::validate()`（`config/mod.rs:691-751`）中显式拒绝复用。

## 8. 配置

新增 `[feishu]` 段，采用既有可选段范式（照 `settings.email.change` /
`security.totp` 的 `Option` + `#[serde(default)]` 形态，见 `bootstrap.rs:120-142`）：

```toml
[feishu]
enabled = false                            # 可选段；省略时相关 Action 不注册
management_api_token = "replace-with-…"   # secret
encryption_key = "replace-with-…"          # secret；配置后才启用加密返回
```

同步清单（缺一即门禁失败）：

1. `src/config/mod.rs`：`FeishuSettings` struct + `Settings` 字段 + `validate()`
2. `src/config/source.rs`：`ENVIRONMENT_BINDINGS`（`:34`）+ `SECRET_BINDINGS`（`:427`）
   + 测试用 `SecretKey` 枚举（`:385-419`）
3. `config.example.toml` 与 `config.show.toml`——由 4 个同步测试机械保证
   （`config/mod.rs:1529/2281/2366/2436`）
4. `docs/contracts/CONFIGURATION.md`

**注意**：未在 `ENVIRONMENT_BINDINGS` 登记的 `YANG_SYSTEM_*` 环境变量会让进程**直接启动
失败**（`yang-runtime/src/config.rs:235`）。占位密钥会被启动校验拒绝。

## 9. 门禁与 CI

| 门禁 | 触发条件 | 动作 |
|---|---|---|
| `scripts/run_ci.py --self-test`（A） | 新增 `crates/*` | 同步 `AUXILIARY_PACKAGE_FLAGS` 与 `-p` 列表 |
| `verify_ci_contract.py`（A） | 改 `ci.yml` 的 clippy/aux-test 行 | 同步 `REQUIRED_FRAGMENTS` |
| `deny.toml`（A） | 新增依赖 | `aes`/`cbc` 许可已在白名单，可直接通过 |
| MSRV 1.80（A+B） | 任何依赖变更 | `aes 0.8` / `cbc 0.1` MSRV 远低于 1.80，无风险 |
| `check_architecture.py`（B） | 任何 addon 改动 | 按 §3.3 的形状约束 |
| `run_ci.py --self-test`（B） | 新增集成测试 | `INTEGRATION` 元组须与「含 `YANG_SYSTEM_TEST_` 的测试文件集」完全相等 |
| `pnpm verify:deployment-contract`（B） | 新增非 `/api` 顶层路径 | 本设计全部路由在 `/api/v1/` 下，无需改 nginx |
| OpenAPI 契约产物（B） | 新增 Action | 重跑 `scripts/dump_openapi.py` 与 `pnpm gen:contracts`。**注意：该产物无 CI 新鲜度门禁**（`run_ci.py` 中无 openapi 步骤），只有 `AGENTS.md` 的约定，容易静默过期 |
| **`crates/yang-base/tests/*.rs`（A）** | —— | **任何门禁都不执行它们**：`run_ci.py` 对 yang-base 只跑 `--lib`（单元测试），而 `--all-targets` 只覆盖 `yang-base-derive`/`yang-pcg`/`yang-runtime`（`AUXILIARY_PACKAGE_FLAGS`）。`transport_axum.rs`（2052 行）、`compatibility_contract.rs`、`release_docs_contract.rs` 等都**只在 clippy 的 `--all-targets --all-features` 下被编译**。⇒ P0 必须**显式手动运行**它们并保留输出，不能把「`run_ci.py quick` 绿」当作验收 |

**跨仓推送顺序**：先 `lib_yang`（Phase 1 的 yang-base 变更）→ 确认推送完成 → 再推
`yang-system`。两侧都用 `--locked`，间隔过近会因锁文件与旧清单不匹配而失败。

## 10. 测试策略

**`crates/yang-base`（P0）**

- `ResponseBody::Raw` / `ResponseAttachment::Raw` 的映射与 `From` 单测
- `transport_axum.rs` 集成测试：断言响应 body 是裸 JSON（**不含 `code`/`message` 包络**）、
  `Content-Type: application/json`、且非 JSON 的 `content_type` 在构造期被拒
- 既有 `test_attachment_response_json_wire_format_unchanged` 等必须继续通过

**`yang-system` 单元测试（colocated `#[cfg(test)]`）**

- **加密对照向量**：用飞书 Go 参考实现的行为钉死——特别是「明文长度已对齐时仍追加整块」
- 加解密 round-trip；Token 哈希与常量时间比较
- `page_token` 编解码（含非法 token fail-closed）
- **协议序列化断言**：逐字断言顶层键为 `msg`（不是 `message`）、`isDefault`、
  `i18nResources`、`hasMore`、`nextPageToken`
- `i18nResources` 构造：至少一种语言、`isDefault` 唯一性、`@i18n@` 键一致性
- 3 秒预算超时路径返回合法错误包络而非 `Err`

**`yang-system` 集成测试**（`tests/feishu_*_integration.rs`，`#[ignore]`，真实 MySQL/Redis）

- **文件须读 `YANG_SYSTEM_TEST_DATABASE_URL` 以被 `run_ci.py` 反向发现**，并同步登记
  `INTEGRATION` 元组。

分两个入口，职责不同：

| 入口 | 覆盖 |
|---|---|
| `feishu_options_integration.rs` | **Schema 级**：两张表被创建、`option_id` / `source_key` 的唯一索引真实存在且真的拒绝重复、`token_hash` 列宽、二次同步为无操作 |
| `feishu_approval_options_integration.rs` | **端点级**：装配完整应用经 Registry 派发，断言响应体字面形状（裸 `{code,msg,data}`、`Content-Type`、`@i18n@` 占位符、`i18nResources` 非空）、分页推进不漏不重（含**排序键并列**时靠 `option_id` 推进的那一支）、`query` 关键词命中与未命中、非法游标 fail-closed、数据源不存在（`40401`）、错误 Token / 停用数据源被拒、禁用选项不出现、加密路径（用文档 Go 参考实现独立解密核对）、开了加密但服务端缺密钥（`50002`）、写入入口的管理 Token 鉴权与跨数据源归属保护（含比较用的排序规则必须与写入一致）、public 端点不得被无关 `Authorization` 头打成 401 |

> **端点级测试是后补的，原因值得记住。** 本设计把上述用例列为「必做」，但 P6 只落地了
> Schema 级入口，端点级从未实现——于是「页大小越硬上限」与「keyset 引用未声明
> `filterable` 的字段」两个缺陷同时逃逸，取选项端点在生产形态下对**每一个**合法请求
> 返回 `code=50001`，而 110 条单测全绿。

**仍未覆盖**：入口 1 的 2.5 秒处理预算（`50401`）没有自动化用例——需要注入一个会阻塞的
查询，本期的 harness 不具备该注入点。该路径目前只有代码审查与手工联调保障。

## 11. 实施阶段

| 阶段 | 内容 | 出口条件 |
|---|---|---|
| **P0** | yang-base：`Raw` 变体 + `#[non_exhaustive]` + content-type 白名单 + 版本 bump + 测试 | `run_ci.py full` 绿 → **先推 lib_yang** |
| **P1** | yang-system：`[feishu]` 配置段 + 两张表 + `domain/repository.rs` | `check_architecture.py` 绿、配置同步测试绿 |
| **P2** | `protocol.rs` / `crypto.rs` / `token.rs` / `i18n.rs` + 单元测试 | `cargo test --lib` 绿 |
| **P3** | 入口 3（受保护 Action）+ 入口 2（静态 Token 中间件） | 单测 + 架构门禁绿 |
| **P4** | 入口 1（Raw 严格信封 + 加密 + 分页 + i18n + 3 秒预算） | 单测绿 |
| **P5** | 前端 `ModulePresentationSpec` + `ViewSpec`；重跑 OpenAPI 契约产物 | `pnpm check` 绿 |
| **P6** | 集成测试 + 初始授权 SQL + **飞书审批后台「校验数据」联调** | `run_ci.py integration` 绿 |
| **P7** | 抽 `crates/yang-feishu`，同步 lib_yang CI 契约 | 新 crate 过 CI 全套；跨仓推送 |

**P6 的联调是不可省略的。** 它一次性验证四件文档层无法回答的事：
① `msg` 键名是否真被容忍（虽然我们已按字面严格实现，需确认飞书不因**多余字段**报错）；
② 配置 Key 后加密返回是否被正确解密；
③ `i18nResources` 与 `@i18n@` 占位符的匹配规则；
④ 勾选「支持模糊、分页搜索」后飞书发来的 `page_token` / `query` 实际形态。
联调需要一个**公网可达**的地址（飞书不能访问内网）。

## 12. 风险与未决项

| 风险 | 等级 | 处置 |
|---|---|---|
| 飞书运行时是否容忍**多余字段**（我们返回严格信封时不会有多余字段；但入口 2/3 会） | 中 | 入口 1 已按字面严格实现，无多余字段。P6 联调确认 |
| 失败路径（`code≠0`）时 `msg` 是否被飞书展示 | 低 | 我们照文档返回 `msg`，最坏情况是提示文案为空 |
| 3 秒超时 vs 数据库抖动 | 中 | 2.5s 内部收口；`TableQuery` 加索引；否决项：任何外部调用 |
| `Raw` 逃生口被滥用（返回 `text/html`） | 中 | 构造期 content-type 白名单（§4.4） |
| Schema 无回滚，主键设计错就必须重建表 | 中 | 主键语义已在 §5.3 固定；表结构需在 P1 一次评审到位 |
| 静态 Token 中间件漏挂导致入口 2 裸奔 | 中 | 构建期/冒烟测试断言（§6.2） |
| 门禁可信度缺陷（`check_architecture.py:309` 截断） | 低 | 测试写在文件末尾；不依赖门禁作为合规证明 |
| OpenAPI 契约产物无 CI 新鲜度门禁 | 低 | P5 手工重跑并提交，不依赖 CI 提醒 |
| Phase 2 抽 crate 时 API 返工 | 中 | Phase 1 只把「已被真实消费者验证过」的部分抽出去 |

**明确未决、留待联调或评审决定：**

- 入口 2 的单次 upsert 条数上限取值。
- 是否为 `feishu_option` 引入软删除（当前用 `enabled=false` 表达禁用）。
- `ActionResponseKind` 是否在 Phase 2 增加 `Raw` 变体以修正 OpenAPI 描述。

## 13. 证据索引

**飞书文档（正文经 curl 直取原始 `.md` 逐字核对）**

- 关联外部选项（中文）：`https://open.feishu.cn/document/server-docs/approval-v4/approval/associate-external-options.md`
- 英文版：同 URL `?lang=en-US`
- Lark 国际版：`https://open.larksuite.com/document/server-docs/approval-v4/approval/associate-external-options.md`
- 反证文档（响应表把 `msg` 标为「是」）：`historic-version/approval/v2/third-party-approval-integration/external-approval-instance-create.md`
- 多维表格自动化发送 HTTP 请求：`https://www.feishu.cn/hc/zh-CN/articles/410063847664`
- 唯一公开真实实现：`https://github.com/rawchen/feishu-approval-option`（Java + 金蝶云星空）

**本仓库**

- `crates/yang-base/src/action/response.rs`、`src/action/functional.rs`、
  `src/transport/axum.rs`、`src/router/middleware.rs`、`src/action/context.rs`
- `project/yang-system/docs/guides/ADDON_ONBOARDING.md`、`docs/contracts/{SCHEMA,CONFIGURATION,AUTHZ_GRANTS,SLO}.md`
- `project/yang-system/src/addon/account/domain/mfa.rs`（AES-GCM 骨架，密钥派生与编码层的直接参照）
- `project/yang-system/src/addon/account/user/table.rs`（TableSpec + `fields!` 范式）
- `project/yang-system/src/infrastructure/schema.rs:213`（自然键主键先例）
- `project/yang-system/docs/architecture/raw-sql-boundaries.md`
- `docs/audit/2026-09-20-whole-repo-review.md`（门禁可信度缺陷）

**取证方法学备注**：本设计的飞书契约结论来自 5 路独立检索 + 14 条对抗式复核
（10 confirmed / 3 refuted / 1 uncertain）。**被证伪的论据已在本文件 §3.1 显式标注，
后续不得引用。**
