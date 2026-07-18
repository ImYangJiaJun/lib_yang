# FRONTEND_FIRST_PRINCIPLES 审查发现修复计划

**日期:** 2026-07-17
**基线:** a6165d6（HEAD，工作区未提交改动即修复内容）
**来源:** 对 `992a783..a6165d6` 的六领域并行代码审查，对照 `docs/FRONTEND_FIRST_PRINCIPLES.md`

## 背景

yang-base 按 FRONTEND_FIRST_PRINCIPLES.md 补全了 UI 目录、业务渲染投影、关系选项、租户中间件、step-up、multipart 上传六块能力。审查确认方向与主干达标，发现 2 个 Critical + 9 个 Important 问题。本计划按文件内聚性分为 8 个修复任务，串行执行，每个任务实现后接独立审查。

## Global Constraints（每个任务都必须遵守）

- 注释与公开文档用中文；`#![warn(missing_docs)]` 下新公开项必须有中文文档注释。
- 生产代码禁止新增 `unwrap()`/`expect()`（仅测试可用）。
- 禁止新增进程级全局单例（`static OnceLock`/`lazy_static`）；资源经 `ToolsBuilder`/`Tools`。
- UI 契约变更必须递增 `UI_SCHEMA_VERSION`（`crates/yang-base/src/definition/ui.rs` 顶部常量，当前 `"1.8"`）；新增/扩展枚举必须 `#[serde(other)]` 安全降级。
- 安全语义一律 fail-closed：解析/校验失败默认拒绝，不得降级放行。
- UI 投影不得携带 permissions/calls/tags 等内部信息；不得下发前端物理路径、动态 import 或脚本。
- 遵循 TDD：先写失败测试并确认失败原因正确，再写最小实现使其通过。
- **禁止任何 git 变更**（不 commit、不 stash、不动 index/HEAD）；改动保留在工作区。
- 只修改 `crates/yang-base/**`（含其 tests/trybuild）；不改 `docs/`、`AGENTS.md`、`project/`、`br/`。
- 测试运行：聚焦测试用 `cargo test -p yang-base --lib <名称>`；报告前跑一次 `cargo test --lib -p yang-base` 全量（transport 相关另跑 `cargo test -p yang-base --test transport_axum --features transport-axum` 视任务而定）。Docker `#[ignore]` 测试不跑。
- 最终报告写入指定 report 文件；返回消息 ≤15 行。

## Task 1: C-1 修复 UploadedFile 伪造路径（安全 Critical）

**问题:** 普通 JSON Action 在 Input 中声明 `UploadedFile`（`crates/yang-base/src/action/upload.rs:15-23`，公开 `Deserialize`）后，客户端可提交 `{"file":{...,"path":"/etc/passwd"}}`，`copy_to`（upload.rs:52-56）以服务进程权限读取任意本地文件。构建期没有 "input_schema 含二进制字段 ⇔ multipart" 的关联校验。

**修复:**
1. `UploadedFile` 增加非序列化的受信临时根字段（如 `#[serde(skip)]` 的 `Option<PathBuf>`），transport 构造实例时填入（`crates/yang-base/src/transport/axum.rs` 构造处）；反序列化得到的实例该字段为 `None`。
2. `copy_to` 执行前校验：临时根为 `None` → 拒绝；`canonicalize` 路径与临时根后必须 `starts_with`，否则拒绝。
3. 构建期校验（`crates/yang-base/src/definition/builder.rs` `validate_action_media` 附近）：先调查 `UploadedFile` 在 input_schema 中的实际表示（schemars 生成形态），然后双向强制——input_schema 含二进制文件字段则 `request_media_type` 必须为 Multipart（否则 BuildError）；Multipart Action 必须至少含一个文件字段（否则 BuildError）。
4. 测试（TDD）：JSON 通道声明文件字段 → 构建期拒绝；`copy_to` 拒绝无临时根实例与越界路径；既有 multipart 集成测试保持绿（`cargo test -p yang-base --test transport_axum --features transport-axum`）。

**文件:** `crates/yang-base/src/action/upload.rs`、`crates/yang-base/src/definition/builder.rs`、`crates/yang-base/src/transport/axum.rs`、相关测试

## Task 2: I-9 multipart 文本 part 上限 + anyOf 标量解码 + CORS 租户头

**问题:** 文本（非文件）part 无独立上限且整字段在内存累积（`crates/yang-base/src/transport/axum.rs:674-694`）；`decode_text_value`（axum.rs:749-785）只识别平铺 `properties.<name>.type`，`Option<i64>` 的 `anyOf` 与嵌套 `$ref` 静默退化为 string；CORS 默认 allowlist（axum.rs:76-81）缺 `x-tenant-id`。

**修复:**
1. `MultipartSpec`（`crates/yang-base/src/definition/media.rs`）增加文本字段上限（默认 64 KiB），axum.rs 文本 part 累积处超限即 413；构建期校验其合理性（不得超过 max_total_bytes）。
2. `decode_text_value` 支持 schemars `anyOf`（含 null 分支的 `Option<T>` → 取非 null 分支类型）；本地 `$ref` 暂不解析，在函数/模块文档注明该限制。
3. CORS 默认 allowlist 加入 `x-tenant-id`。
4. 测试（TDD）：文本 part 超限 413；`Option<i64>`/`Option<String>` 等字段经 multipart 反序列化成功；既有测试绿。

**文件:** `crates/yang-base/src/definition/media.rs`、`crates/yang-base/src/transport/axum.rs`、`crates/yang-base/tests/transport_axum.rs`

## Task 3: I-2 response_kind 声明与运行时一致性

**问题:** `response_kind` 声明与运行时行为无校验；`crates/yang-base/tests/transport_axum.rs:180-254` 的 `DownloadAction`/`PreviewAction`/`RedirectAction` 三个 fixture 返回附件却未声明 `response_kind`，UI 目录正在误描述它们。

**修复:**
1. 三个 fixture 补上 `response_kind = "download"/"preview"/"redirect"` 声明（derive 属性语法参照既有用法）。
2. 在 dispatch 返回附件响应的单点（transport/axum.rs:421-426 或 builder.rs dispatch 路径，由实现者选择最合适的唯一位置）与 Action 声明的 `response_kind` 比对，不匹配时 `tracing::warn!`（不阻断、不改变响应）。
3. 测试（TDD）：fixture 修正后 UI 目录/传输层对三个 Action 投影的 `response_kind` 正确；不一致路径只 warn 不 panic。

**文件:** `crates/yang-base/tests/transport_axum.rs`、`crates/yang-base/src/transport/axum.rs` 或 `crates/yang-base/src/definition/builder.rs`

## Task 4: C-2 FormSchema 校验提示 + I-1 OpenAPI response_kind + I-3 租户维度文档决策

**问题:** `FormFieldSchema`（`crates/yang-base/src/definition/ui.rs:407-433`）缺 §5.3 明列的"校验提示"（数据在 `FieldSpec.validation`，`crates/yang-base/src/definition/field.rs:30-41`）；OpenAPI `operation_json`（`crates/yang-base/src/definition/openapi.rs:164-177`）无条件声明 JSON 信封，与 download/preview/redirect 实际响应脱节，且无 `x-yang-response-kind` 扩展；OpenAPI multipart 扩展硬编码 `"lifecycle": "request_scoped"` 未序列化真实值；`ActionConfirmation`（ui.rs:117-132）无构建期内容校验；UI 投影不含租户维度这一决策未记录。

**修复:**
1. `FormFieldSchema` 增加可选 `validation` 子结构（min_length/max_length/minimum/maximum/pattern 五项，来源 `FieldSpec.validation`，全部可选序列化），按字段权限投影；`UI_SCHEMA_VERSION` 升 `"2.0"`（"1.9" 已被 Task 2 的 multipart 契约变更占用，以代码现状为准）；补序列化契约测试与字段权限过滤测试。
2. OpenAPI 成功响应按 `action.response_kind` 分支：Json 保持现状；Download/Preview → `application/octet-stream`（或按附件 content-type）；Redirect → 3xx 无 content；统一输出 `x-yang-response-kind` 扩展；`x-yang-multipart` 的 `lifecycle` 改为序列化 `spec.lifecycle` 真实值。
3. ui.rs 模块文档显式记录"目录投影当前不含租户维度"为有意决策，并注明后续接入点。
4. `ActionConfirmation` 构建期内容校验：title/message 非空白且长度有界（对齐 availability reason 的 1..=500 风格，`crates/yang-base/src/definition/builder.rs:871-876`）。
5. 测试（TDD）：validation 投影与权限过滤、OpenAPI 四分支、lifecycle 真实值、confirmation 构建期负例。

**文件:** `crates/yang-base/src/definition/ui.rs`、`crates/yang-base/src/definition/openapi.rs`、`crates/yang-base/src/definition/builder.rs`、`crates/yang-base/src/definition/field.rs`（如需）

## Task 5: I-4 关系选项契约默认强制

**问题:** `RelationOptionsRequest::validate()` 不在 decode 默认路径（`crates/yang-base/src/table/relation_options.rs:47-51`，`ParamInput::decode` 默认实现见 `crates/yang-base/src/definition/param.rs:18-24`），handler 忘调即边界失守；构建期不校验 select Action 的输入/输出类型（`crates/yang-base/src/definition/builder.rs:913-940`，`resolve_typed` 的 TypeId 机制见 builder.rs:153-179）；无 dispatch 级越权测试。

**修复:**
1. `RelationOptionsRequest` 的 `ParamInput` impl 覆盖 `decode()`：先默认反序列化再 `validate()?`，超限 body 经 decode 直接报错；补"decode 拒绝超限"测试。
2. 构建期校验 select Action 的 `input_type_id() == TypeId::of::<RelationOptionsRequest>()` 且 `output_type_id() == TypeId::of::<RelationOptionsResponse>()`，不匹配报 BuildError；补构建期负例测试。
3. dispatch 级越权测试：无权限用户直接调用 select Action 返回 PermissionDenied。
4. 测试（TDD）如上。

**文件:** `crates/yang-base/src/table/relation_options.rs`、`crates/yang-base/src/definition/builder.rs`

## Task 6: I-5 searchable 独立位 + I-7 树节点上限

**问题:** `searchable`/`filterable` 双开关在运行时折叠为单一 filterable 位（`crates/yang-base/src/definition/field.rs:257-259`），UI 契约与服务端强制两个方向错位；非文本字段可声明 searchable 但服务端静默跳过（`crates/yang-base/src/table/table_query.rs:1005-1012`）；`table_tree_view`（`crates/yang-base/src/table/tables.rs:127-136`）全表无界读入内存；`table_select`（tables.rs:113）文档自称"默认受 TableQuery 最大分页保护"与实际 `all()` 行为不符。

**修复:**
1. table 层 `Field`/`FieldConfig` 增加独立 `searchable` 位；`into_schema_field` 从 `AccessSpec.searchable` 写入；`TableQuery::search` 改用 searchable 判定（`filterable` 继续服务结构化 where 校验 `validate_filter_field`）；UI 投影 `search_fields`/`filter_fields` 与服务端强制点对点对齐。
2. 构建期对非文本字段声明 `searchable=true` 报 BuildError（校验入口参照既有 AccessSpec/字段校验）。
3. 树查询节点上限：常量默认值（如 10_000）+ `TreeViewSpec` 可配置，超出报错；修正 `table_select` 文档表述。
4. 测试（TDD）：searchable/filterable 四组合行为、非文本 searchable 构建负例、树超限报错、现有测试绿。

**文件:** `crates/yang-base/src/definition/field.rs`、`crates/yang-base/src/table/table_query.rs`、`crates/yang-base/src/table/tables.rs`、table 层 Field/FieldConfig 定义文件、`crates/yang-base/src/definition/spec.rs`（TreeViewSpec 配置）

## Task 7: I-6 租户中间件加固（文档 + 组合测试）

**问题:** 中间件顺序只有注释约束（`crates/yang-base/src/action/tenant.rs:36-38`），`ModuleSpec::middleware`（`crates/yang-base/src/definition/spec.rs:604-612`）文档未提洋葱序；租户中间件测试用 `with_user` 直接注入（tenant.rs:229-237），未覆盖真实认证链组合；"公开 Action + 可选认证 + 租户中间件"三方组合无测试；"有效 token 注入公开 Action 身份"仅 `#[ignore]` Docker 测试覆盖（`crates/yang-base/src/action/auth.rs:1145-1204`）。

**修复:**
1. `ModuleSpec::middleware` 文档写明：按注册顺序构成洋葱链，认证类中间件必须先于 `TenantResolverMiddleware` 注册。
2. 组合测试：真实 `TokenAuthMiddleware` → `TenantResolverMiddleware` 链路端到端测试。
3. 三方组合集成测试：公开 Action + 可选认证 + 租户中间件。
4. 调查 `verify_token_checked` 在无 cache（无黑名单存储）的 Tools 下是否降级跳过撤销检查（`crates/yang-base/src/token/revocation.rs:233-245`）；若是，将"有效 token → 注入用户 → 目录按身份投影"改写为非 Docker 单测（`test_tools()` 即无 cache 配置）；若否，报告阻塞原因，不强行改。
5. 不做机制性顺序校验（phase/构建期检测），该事项由控制器记入 BACKLOG。
6. 测试（TDD）如上。

**文件:** `crates/yang-base/src/definition/spec.rs`、`crates/yang-base/src/action/tenant.rs`、`crates/yang-base/src/action/auth.rs`、`crates/yang-base/src/token/revocation.rs`（只读调查）

## Task 8: I-8 step-up 加固（文档 + 锁定测试 + 审计事件）

**问题:** 内部 `Registry::call`/`api_run` 不跑中间件链，敏感 Action 可被内部调用静默跳过 step-up（`crates/yang-base/src/definition/builder.rs:295-321`），语义未文档化也无测试锁定；proof 在 TTL 内可重放，`StepUpResourceResolver` 文档未指引绑定操作参数指纹；`complete_challenge`/`CredentialVerifier` 未要求实现方限流；敏感操作全程无审计事件（`crates/yang-base/src/action/step_up.rs`）。

**修复:**
1. 文档：`StepUpMiddleware` 与 `Registry::call` 显式声明"step-up 仅约束 dispatch 路径，内部 call/api_run 不经过中间件"；锁定测试：内部 call 调用敏感 Action 无需 proof 即可执行（固化该语义，防止日后无意改变）。
2. `StepUpResourceResolver` 文档：资源标识应包含操作参数指纹（如金额、目标账户的 hash），以收窄 proof 重放窗口。
3. `complete_challenge`/`CredentialVerifier` 文档：实现方必须做速率限制与失败计数，rustdoc 给出接线要点。
4. 审计：challenge 签发、challenge 完成、proof 验证成功、proof 验证失败四处发 `tracing::info!`/`tracing::warn!`，含 proof_id、subject、action、resource_hash，不含敏感原文；先确认 tracing 依赖可用。
5. 测试（TDD）：锁定测试 + 现有 step-up 测试绿（`cargo test -p yang-base --lib step_up`）。

**文件:** `crates/yang-base/src/action/step_up.rs`、`crates/yang-base/src/definition/builder.rs`（仅文档）、`crates/yang-base/src/router/middleware.rs`（如需）

## 不做项（由控制器记入 BACKLOG，不在本计划实施）

- 内置关系 options 执行器（中期）。
- 中间件 phase/构建期顺序校验机制。
- 内置 step-up re-auth Action。
- 投影结果按身份缓存、HTTP ETag/304 协商。
- `UiCatalog` 字段私有化、revision 单次计算等性能优化。
- 其余审查 Minor 项（final review 统一分诊）。
