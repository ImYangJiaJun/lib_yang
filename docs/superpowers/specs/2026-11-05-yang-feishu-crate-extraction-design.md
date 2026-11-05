# yang-feishu 抽取方案（方案 A）

> **状态**：设计定稿，待施工
> **基线**：`project/yang-system` @ `6957ce3`（工作区干净）；`lib_yang` @ `df91e6f`（工作区干净）
> **目标**：`lib_yang/crates/yang-feishu` 0.1.0
> **源清单**：[符号级清单（附录 A）](./2026-11-05-yang-feishu-extraction-symbol-map.md)
> **决策**：用户在 A / B / C 三方案中选择 **A（完整 crate）**

---

## 0. 决策摘要

### 0.1 一句话

把 `yang-system` 中约 **5k 行生产代码**的「飞书开放平台协议层」抽成 `crates/yang-feishu`；**不搬业务**。宿主通过薄 shim + `HostTransport` / `HostTokenCache` 适配器继续使用原有 `yang_base::http::HttpClient` 与 `yang_db::RedisClient`，行为与重试次数保持不变。

### 0.2 范围与成本

| 项 | 数值 |
|---|---|
| 迁入 crate 的生产代码 | 约 4.8–5.3k 行 |
| 迁入的测试 | 约 1.6–2.0k 行（其余宿主测试保留） |
| 新建文件 | 约 35 个 |
| 总工期 | **35–50 人日**（含测试迁移与真机回归） |
| Phase 1（transport + auth） | 8–12 人日，可独立交付 |
| 风险等级 | 中高（多应用缓存键、事件 401 语义、错误语义折叠是三个高危点） |

### 0.3 九组归属冲突的最终裁决

8 路盘点之间存在冲突。以下裁决覆盖附录 A 中的原始处置，**施工以本表为准**。

| # | 冲突 | 最终裁决 |
|---|---|---|
| 1 | `FailureKind` 是否删除 `UuidConflict` / `InstanceNotFound` | **保留 5 个变体**：`Retry`、`TokenExpired`、`Fatal`、`UuidConflict`、`InstanceNotFound`。二者是飞书审批 API 语义，不是产品状态机；删除会迫使宿主约 12 处 `match` 改成码值守卫，并有把 60012 写成终态的风险。 |
| 2 | `TenantTokenProvider` 进 crate 还是留宿主 | **Provider 进 crate，构造走 `OpenApiClient`**。`validate_deployment_name` 留宿主；`cache_namespace` 由宿主传入。crate 不提供 `TenantTokenProvider::new`。 |
| 3 | `RedisTenantTokenCache` 归属 | **crate 提供可选 `RedisTokenCache`（`redis` feature）供独立使用者；宿主不用它**，改用 `HostTokenCache` 适配 `yang_db::RedisClient`，避免第二套 Redis 连接池。 |
| 4 | `HttpClientTransport` / 审批创建限流 | **crate 提供可选 `ReqwestTransport`（`http` feature）；宿主用 `HostTransport` 包 `yang_base::http::HttpClient`**。审批创建限流整体留宿主，作为 `HostTransport` 内的 `CreateGate` 装饰器；crate 只暴露 `approval::is_create_request(&Request)` 谓词。 |
| 5 | `crypto` 的 `seal` / `issue_token` / `hash_token` 归属 | **crate 只保留通用原语**：`derive_key`、`encrypt_bytes`、`encrypt_json`、`decrypt_bytes`、`hash_token`、`verify_token`、`tokens_equal`。`seal` / `unseal` / `unseal_verified` / `issue_token` / `generate_token` / `IssuedToken` 是宿主凭据生命周期，留宿主。 |
| 6 | 重试常量归属 | **crate 拥有 `TOKEN_RETRY` / `PULL_RETRY` / `DEPARTMENT_RETRY` / `MESSAGE_RETRY` 与对应超时**，默认值与现网一致；宿主可按需覆盖。 |
| 7 | `Transport` 的错误类型 | **crate 自有 `Error { kind, message }`**，`Transport` 返回 crate `Result`。宿主 `HostTransport` 负责 `BaseError` ↔ `Error` 转换；`outbound_error` 不再无条件折成 `ParamInvalid`。 |
| 8 | `send_with_retry` 是否公开、是否注入 `OutboundDomain` | **不公开带 domain 的 5 参版本**。重试循环收进 `OpenApiClient::send` / `send_tenant`；宿主自定义请求用 `OpenApiClient::send_raw(request, policy)`。endpoint 别名与中文 hint 留宿主 `HostTransport`。 |
| 9 | `instance_applink` / `connection_codes` 归属 | **整体留宿主**。硬编码 `appId=cli_9cb844403dbb9108`、`source=bitable`、`applink.feishu.cn` 是产品资产；`approval_convert.rs:818-822` 的 `connection_codes` 同样留宿主。 |

另外三条补充裁决（来自对抗核验）：

| # | 问题 | 裁决 |
|---|---|---|
| 10 | `path::validate_path_segment` 两版语义不等价 | **保留两个函数**：`validate_path_segment`（严格白名单，bitable 用）与 `validate_opaque_path_segment`（宽松黑名单，approval/contact 用）。不要为了「共用一份」把 bitable 的路径穿越防护降级。 |
| 11 | `approval_match` / `approval_provision` 整体搬 | **只搬无状态协议原语**（`FormWidget` / `parse_form` / `WidgetMap` / `Converter` / `build_form`）。`Column`、`FieldMapping`、`MatchError`、`match_fields`、`columns_from_fields`、`resolve_field_coord`、`ProvisionError`、`check_*` 全部留宿主。 |
| 12 | `approval_attachment` 的 `preflight` / `source_identities` | **留宿主**。crate 只搬 `Attachment` / `UploadSpec` / `AttachmentError` / `parse_attachments` / `download_media` / `upload_approval_file`。`bitablePerm` extra、200ms pacing、`transfer` 编排留宿主。 |

### 0.4 明确不搬（否则 crate 会变成业务底座）

- `approval_dispatch.rs`（1997 行）、`approval_execution`、`approval_aggregation`、`approval_feedback`、`approval_notification` 的业务状态机、`approval_provision`、`approval_rate_limit`
- `pull.rs`、`derive.rs`、`field_binding.rs`、`source_key.rs`、`linkage.rs`、`option_write.rs`
- `portal/`、`datasource/actions/`、`option/actions/`、审批控制台 Action
- `feishu_user_token` 存储与刷新事务、`feishu_approval_event` 落库、任务表/组表
- 前端 `frontend/src/features/feishu/` 全量
- 中文运维文案、权限键、审计、路由、日志中间件

---

## 1. 目标与非目标

### 1.1 目标

1. 其它 Rust 项目能 `yang-feishu = { path/git = ... }` 后，用十余行装配拿到：
   - `tenant_access_token` 换取、Redis/内存缓存、短锁单飞、失效强刷
   - 多维表格读（表/视图/字段/记录/搜索/批量取）与写（`batch_update`）
   - 审批 v4 建单、查单、取定义、订阅事件
   - 通讯录解析、IM 发消息/编辑、云盘下载 + 审批附件上传
   - 事件 challenge / 解密 / verification_token 校验
   - 外部选项协议 DTO、i18n、keyset 游标、AES-256-CBC
2. 宿主 `yang-system` 行为不变：重试次数、错误分类、限流时序、日志口径保持。
3. 公开面在 0.x 尽量小，DTO 用 `#[non_exhaustive]`，后续可演进。

### 1.2 非目标

- **不是**「飞书业务底座」：不提供审批派发、字段映射、选项派生、查询门户、控制台。
- **不**在 0.1.0 实现 `X-Lark-Signature`：它需要 `yang-base` 的 `Request` 新增 `raw_body` 并在 axum transport 保留原始字节，是跨 crate 的新功能，单独立项。
- **不**把用户 token 存储做成 crate 内置 MySQL 实现：存储策略（单行 slot、明文/密文、提醒线）由宿主决定。
- **不**把 Lark 国际版 `upload` 域宣称为已验证：`Endpoints::lark()` 提供预设，但上线前必须用真实 Lark 租户验证，未验证前用 `Endpoints::custom` 覆盖。

---

## 2. 目标架构

### 2.1 位置与依赖方向

```text
lib_yang/
├── Cargo.toml                 # members = ["crates/*"]，自动收编新 crate
├── crates/
│   ├── yang-base/             # 宿主框架；yang-feishu 不依赖
│   ├── yang-db/               # 宿主存储；yang-feishu 不依赖
│   ├── yang-runtime/
│   ├── yang-pcg/
│   └── yang-feishu/           # 新 crate
└── project/yang-system/       # 宿主；依赖 yang-feishu
```

依赖规则：

- `yang-feishu` **不得**依赖 `yang-base`、`yang-db`、`yang-runtime`、`schemars`、`anyhow`。
- `yang-feishu` 默认只依赖：`async-trait`、`serde`、`serde_json`、`thiserror`、`tracing`、`sha2`、`base64`、`subtle`、`rand_core`、`aes`、`cbc`、`uuid`、`tokio(time)`。
- 可选：`http` → `reqwest` + `rustls`；`redis` → `redis`；`metrics` → `metrics`。
- 宿主 `yang-system` 使用 `default-features = false, features = ["metrics"]`，用自己的 HTTP/Redis 适配器。

### 2.2 模块树

```text
crates/yang-feishu/
├── Cargo.toml
├── README.md
├── CHANGELOG.md
├── src/
│   ├── lib.rs
│   ├── client.rs              # OpenApiClient / OpenApiClientBuilder / OpenApiConfig / ClientCore
│   ├── endpoints.rs           # Endpoints / BaseUrl（open / accounts / upload）
│   ├── error.rs               # Error / FailureKind / Result
│   ├── path.rs                # 严格与宽松两个路径段校验
│   ├── transport/
│   │   ├── mod.rs             # Transport / Request / Response / Sleeper / RetryPolicy / disposition / classify
│   │   ├── codes.rs           # 飞书平台错误码常量
│   │   └── http.rs            # #[cfg(feature="http")] ReqwestTransport
│   ├── auth/
│   │   ├── mod.rs             # SecretString / Credentials / CredentialsProvider / StaticCredentials
│   │   ├── token.rs           # TokenProvider / TokenCache / MemoryTokenCache / parse_token_grant / cache_ttl_seconds
│   │   ├── redis.rs           # #[cfg(feature="redis")] RedisTokenCache
│   │   ├── oauth.rs           # OAuthClient / TokenBundle / TokenEndpointError
│   │   └── state.rs           # StateStore / InMemoryStateStore
│   ├── bitable/
│   │   ├── mod.rs
│   │   ├── client.rs          # BitableClient
│   │   ├── dto.rs             # RecordItem / FieldItem / ...（#[non_exhaustive]）
│   │   ├── pagination.rs      # PaginationState / RecordsSnapshot
│   │   ├── fields.rs          # 字段类型判定 / 名称解析 / rekey
│   │   ├── cells.rs           # CellValue / cell_label
│   │   ├── link.rs            # BitableCoordinates / parse_bitable_link
│   │   └── filters.rs         # pending_backfill_filter（原 pending_records_filter）
│   ├── approval/
│   │   ├── mod.rs             # ApprovalClient re-export / is_create_request
│   │   ├── client.rs          # create/get/definition/subscribe
│   │   ├── form.rs            # FormWidget / WidgetOptions / parse_form / form_snapshot_text
│   │   ├── value.rs           # WidgetMap / Converter / FormInput / build_form / amount_currency
│   │   ├── widget_rules.rs    # 控件类型规则表
│   │   └── idempotency.rs     # derive_idempotency_key / derive_uuid
│   ├── contact.rs             # ContactClient
│   ├── im.rs                  # ImClient
│   ├── drive.rs               # DriveClient
│   ├── event.rs               # EventVerifier / EventConfig / decode_challenge / decode_callback
│   ├── options/
│   │   ├── mod.rs
│   │   ├── protocol.rs        # 外部选项 DTO 与 {code,msg,data}
│   │   ├── i18n.rs            # @i18n@ 资源装配
│   │   └── pagination.rs      # encode_cursor / decode_cursor
│   └── crypto.rs              # AES/SHA-256 原语
└── tests/
    ├── public_api.rs
    ├── transport_retry.rs
    ├── bitable_pagination.rs
    ├── approval_contract.rs
    └── event_verify.rs
```

### 2.3 feature flags

| feature | 内容 | 默认 | 宿主是否开 |
|---|---|---|---|
| （无） | 平台契约核心 + 加密原语 | ✅ | ✅ |
| `http` | `ReqwestTransport` + `HttpConfig` | ❌ | ❌（用 `HostTransport`） |
| `redis` | `RedisTokenCache` | ❌ | ❌（用 `HostTokenCache`） |
| `metrics` | `feishu_outbound_request_total` / `feishu_outbound_token_total` | ❌ | ✅ |
| `full` | `http + redis + metrics` | ❌ | ❌ |

`default = []`，避免独立使用者被迫拖入 reqwest/redis。`crypto` 依赖（aes/cbc/sha2/base64/subtle/rand_core）不是可选，因为外部选项协议与事件解密都需要。

### 2.4 错误模型

```rust
pub enum FailureKind {
    Retry { retry_after_seconds: Option<u64> },
    TokenExpired,
    Fatal { code: i32 },
    UuidConflict,
    InstanceNotFound,
}

pub struct Error {
    pub kind: FailureKind,
    pub message: String,
}
```

- `Display` 固定 `飞书出站调用失败: {message}`；中文运维 hint 留宿主。
- `Debug` 对 `Request.bearer_token`、`json_body`、`raw_body` 脱敏；`SecretString` 恒打码。
- 宿主错误映射：

| crate `FailureKind` | 宿主 `BaseError` |
|---|---|
| `Retry { retry_after_seconds }` | `BaseError::UpstreamUnavailable(message)`；若已知是频控码，可改 `RateLimitExceeded { retry_after_seconds: retry_after_seconds.unwrap_or(1) }` |
| `TokenExpired` | `BaseError::Unauthorized(message)` |
| `Fatal { code }` | `BaseError::ParamInvalid("feishu", message)`（保持现网 4xx 语义） |
| `UuidConflict` / `InstanceNotFound` | 在 `ApprovalClient` 内转成 `CreateOutcome::UuidConflict` / `GetOutcome::NotFound`，不冒泡成 `BaseError` |

**禁止**再出现 `outbound_error` 把一切折成 `ParamInvalid("feishu")` 的写法；那是当前排障税。

### 2.5 核心公开 API（签名级）

```rust
// client.rs
pub struct OpenApiClient { /* Arc<ClientCore> */ }
impl OpenApiClient {
    pub fn builder(config: OpenApiConfig) -> OpenApiClientBuilder;
    pub fn app_id(&self) -> &str;
    pub fn endpoints(&self) -> &Endpoints;
    pub fn bitable(&self) -> BitableClient;
    pub fn approval(&self) -> ApprovalClient;
    pub fn contact(&self) -> ContactClient;
    pub fn im(&self) -> ImClient;
    pub fn drive(&self) -> DriveClient;
    pub fn oauth(&self) -> OAuthClient;
    pub async fn send_raw(&self, request: Request, policy: &RetryPolicy) -> Result<Response>;
    pub async fn send_tenant(&self, request: Request, policy: &RetryPolicy) -> Result<Response>;
}

// transport
#[async_trait]
pub trait Transport: Send + Sync {
    async fn send(&self, request: Request) -> Result<Response>;
}
#[async_trait]
pub trait Sleeper: Send + Sync { async fn sleep(&self, duration: Duration); }

pub fn classify(status: u16, headers: &BTreeMap<String, String>, body: &str) -> Option<FailureKind>;
pub fn disposition(kind: &FailureKind, attempt: u64, idempotent: bool, policy: &RetryPolicy) -> Disposition;

// auth
#[async_trait]
pub trait TokenCache: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<String>>;
    async fn put(&self, key: &str, value: &str, ttl_seconds: i64) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<()>;
    async fn acquire_lock(&self, key: &str, owner: &str, ttl_seconds: i64) -> Result<bool>;
    async fn release_lock(&self, key: &str, owner: &str) -> Result<()>;
}
pub struct TokenProvider;
impl TokenProvider {
    pub async fn tenant_access_token(&self) -> Result<String>;
    pub async fn invalidate_and_refresh(&self) -> Result<String>;
}

// bitable
impl BitableClient {
    pub async fn list_all_records(&self, app_token: &str, table_id: &str, view_id: Option<&str>, field_names: Option<&[&str]>) -> Result<RecordsSnapshot>;
    pub async fn first_records_page(&self, ...) -> Result<ListRecordsData>;
    pub async fn list_all_fields(&self, app_token: &str, table_id: &str) -> Result<Vec<FieldItem>>;
    pub async fn list_all_tables(&self, app_token: &str) -> Result<Vec<BitableTableItem>>;
    pub async fn list_all_views(&self, app_token: &str, table_id: &str) -> Result<Vec<BitableViewItem>>;
    pub async fn get_records_by_ids(&self, ...) -> Result<BatchGetRecordsData>;
    pub async fn search_records(&self, ...) -> Result<SearchRecordsData>;
    pub async fn search_all_records(&self, ...) -> Result<Vec<RecordItem>>;
    pub async fn batch_update_records(&self, ...) -> Result<()>;
    pub async fn record_row_numbers(&self, ..., max_pages: u32) -> Result<Vec<(String, u64)>>;
}

// approval
impl ApprovalClient {
    pub async fn create_instance(&self, approval_code: &str, form: &str, open_id: &str, uuid: &str) -> Result<CreateOutcome>;
    pub async fn get_instance(&self, instance_id: &str) -> Result<GetOutcome>;
    pub async fn get_approval_definition(&self, approval_code: &str) -> Result<ApprovalDefinition>;
    pub async fn subscribe_approval(&self, approval_code: &str) -> Result<SubscribeOutcome>;
}

// event
impl EventVerifier {
    pub fn new(config: EventConfig) -> Result<Self>;
    pub fn decode_challenge(&self, body: &[u8]) -> Result<Inbound>;
    pub fn decode_callback(&self, body: &[u8]) -> Result<Inbound>;
}
```

更完整的类型与方法见附录 A 各模块的「目标公开 API」。

---

## 3. 符号级归属

### 3.1 移动总表

| 模块 | 源文件（生产行） | 迁入 crate | 留宿主 | 目标模块 |
|---|---|---|---|---|
| L0 transport | `domain/outbound.rs` 1-781；`outbound_setup.rs` 1-114 | `OutboundMethod`/`Request`/`Response`/`Transport`/`Sleeper`/`TokioSleeper`/`FailureKind`/`classify`/`business_code`/`rate_limit_reset_seconds`/`summarize`/`dedup_key`/`RetryPolicy`/`disposition`/`send_with_retry`（内部）/错误码常量/重试常量 | `fatal_hint`、`describe`、endpoint 别名、`HostTransport`、`CreateGate`、`outbound_setup::build` | `transport/`、`error.rs` |
| L0 auth | `domain/tenant_token.rs` 1-430；`user_token.rs` OAuth 协议约 1-470；`token.rs` 1-57；`crypto.rs` 1-234 | `Credentials`/`SecretString`/`CredentialsProvider`/`TokenCache`/`TokenProvider`/`parse_token_grant`/`cache_ttl_seconds`/`LOCK_TTL_SECONDS`/`OAuthClient`/`TokenBundle`/`TokenEndpointError`/`StateStore`/`hash_token`/`verify_token`/`tokens_equal` | 用户 token 存储（`load_single`/`store_authorization`/`user_access_token`/`UserTokenRow`）、350 天提醒、OAuth 三个 Action、`seal`/`issue_token`/`unseal` | `auth/`、`crypto.rs` |
| L1 bitable | `domain/bitable.rs` 1-1799 生产（两个测试段：1037-1072、1800-2863） | URL 构造/信封/DTO/`PaginationState`/`RecordItem`/`FieldItem`/`CellValue`/`BitableCoordinates`/`list_all_*`/`search_*`/`get_records_by_ids`/`batch_update_records`/`rekey_cells_by_field_id`/`record_row_numbers`/`pending_backfill_filter` | `pull.rs`/`derive.rs`/`field_binding.rs`/`source_key.rs`/`option_write.rs`/`health_check`/`pull_probe` | `bitable/` |
| L1 approval | `domain/approval.rs` 1-844；`approval_uuid.rs`；`approval_rate_limit.rs`；`approval_match.rs`；`approval_convert.rs`；`approval_attachment.rs`；`widget_rules.rs` | `ApprovalClient`/`CreateOutcome`/`GetOutcome`/`SubscribeOutcome`/`InstanceDetail`/`ApprovalDefinition`/`FormWidget`/`WidgetOptions`/`parse_form`/`WidgetMap`/`Converter`/`build_form`/`build_group_form`/`build_signature`/`amount_currency`/控件规则表/`derive_idempotency_key` | `instance_applink`/`connection_codes`/`Column`/`FieldMapping`/`MatchError`/`match_fields`/`columns_from_fields`/`resolve_field_coord`/`ProvisionError`/`check_*`/`preflight`/`source_identities`/`transfer`/`approval_rate_limit`/状态机/通知文案 | `approval/` |
| L1 contact/im/drive | `approval.rs` contact 段：404-410、708-763；`approval_notification.rs` 320-431；`approval_attachment.rs` 15-260 | `ContactClient`/`ImClient`/`DriveClient`/`Attachment`/`UploadSpec`/`AttachmentError`/`parse_attachments` | `approval_notification` 的 `post_content`/`send_feedback`/`edit_feedback`/去重状态机；`bitablePerm` extra；200ms pacing | `contact.rs`/`im.rs`/`drive.rs` |
| L1 event/options | `event/actions/event_challenge.rs`；`receive_approval_event.rs` 98-236；`domain/protocol.rs`；`domain/i18n.rs`；`domain/pagination.rs`；`crypto.rs` AES 段 | `EventVerifier`/`EventConfig`/`Inbound`/`CallbackEnvelope`/`decode_challenge`/`decode_callback`；`FeishuOptionsRequest`/`FeishuEnvelope`/`FeishuResult`/`FeishuData`/`build_result_body`/`i18n_key`/`normalize_linkage_value`/`encode_cursor`/`decode_cursor`/AES 原语 | 事件路由、审批事件白名单、`instance_code`/`status`/`operate_time` 解析、落库、1062 去重；`FeishuOptionsRequest` 的 `ParamInput` 实现（宿主 newtype） | `event.rs`、`options/`、`crypto.rs` |

### 3.2 必须留宿主的产品语义（不可谈判）

- `derive.rs:169` `label_option_id`、`:189` `ancestor_key`、`:224` `derive_options`、`:279` `snapshot_digest`
- `source_key.rs:14` `valid_source_key`（URL 路径键规则）
- `linkage.rs:59` `Linkage`（祖先链产品类型）
- `approval_match.rs:342` `Column`、`:565` `FieldMapping`（`deny_unknown_fields` + `JsonSchema` 的 Action 请求体）、`:429` `MatchError`（10 条中文产品文案）、`:644` `match_fields`、`:360` `columns_from_fields`、`:385` `resolve_field_coord`
- `approval_provision.rs:73` `ProvisionError`、`:518` `check_backfill_targets`、`:578` `check_batch_scope_field`、`:620` `check_aggregate_batch`（依赖 `&[Column]` 与产品列名）
- `approval_attachment.rs:71` `preflight`、`:95` `source_identities`、`:182` `transfer`、`:199-200` `bitablePerm` extra 与 200ms pacing
- `approval_rate_limit.rs:12-67` `bucket_key`（`yang-system:feishu:app:{app_id}:approval_create_gate`）
- `approval_notification.rs:221-318` `post_content`（飞书 IM 富文本 schema 是产品文案契约）、`:175-218` `send_feedback`
- `instance_applink`（`approval.rs:62-67`）与 `connection_codes`（`approval_convert.rs:818-822`）里的 `appId=cli_9cb844403dbb9108`
- `crypto.rs:118-201` `seal`/`issue_token`/`unseal`/`unseal_verified`
- `user_token.rs:477-761` 单行 slot、`SELECT ... FOR UPDATE`、350 天提醒

### 3.3 必须保留的飞书平台事实

这些是「mock 永远绿、真机才红」的资产，随代码与注释一起搬：

- 成败判据是**响应体业务码**，不是 HTTP 状态码；`1254302`、`1254003`、`1254040`、`1390001` 等大量错误是 HTTP 200。
- `TokenExpired` 必须先于 401/403 判定；`99991663` 常以 HTTP 400 返回。
- 频控等待在 `x-ogw-ratelimit-reset`，没有 `Retry-After`；`1254290` 是 HTTP 200。
- `tenant_access_token` 响应是扁平 `{code,msg,tenant_access_token,expire}`，不套 `data`；`expire - 300` 缓存；锁 TTL 30s > `TOKEN_RETRY` 最坏 19s。
- `has_more=true` 但 `page_token` 缺失/空串必须硬失败，不能当最后一页。
- `code=0` 但缺 `data` 必须报契约错误，不能当空集。
- 记录页 `page_size` 上限 500；字段页上限 100；`MAX_PAGES` 40；重复游标硬失败；`assert_converged` 用 `accumulated >= total`。
- 60012 = uuid 已创建（响应丢了），不是失败；1390003 = 实例不存在，不是终态；1390007 = 已订阅/已取消，幂等成功。
- 金额控件 `currency` 只能从审批定义 `option.currencyRange` 取。
- `DateInterval` 是真实字段，单值 datetime 填不出 `{start,end}`。
- 外部选项 `linkage_params` 的真机值是文案（如「成都」），不是 `@i18n@<option_id>`。
- 外部选项非搜索型必须首响应给全量；`@i18n@` 资源至少一项。
- 文本列含 URL 会被飞书切成富文本分段，取第一段会丢正文。
- `record_id` 跨副本不唯一；`field_id` 同 Base 多表共享；任务唯一键必须带 `config_id`。

---

## 4. 宿主改造

### 4.1 shim 策略

为了把 44 处 `use ...outbound`、23 处 `OutboundRequest` 字面量、74 处 `OutboundFailure` 字面量、21 个 `OutboundTransport` 实现的改动压到最小，宿主保留同名薄 shim：

```rust
// src/addon/feishu/domain/outbound.rs
pub(crate) use yang_feishu::transport::{
    classify, disposition, send_with_retry, OutboundMethod as Method,
    OutboundRequest as Request, OutboundResponse as Response,
    OutboundTransport as Transport, RetryPolicy, Sleeper, TokioSleeper,
};
pub(crate) use yang_feishu::error::{Error as OutboundFailure, FailureKind};
// 以下留宿主：
pub(crate) fn fatal_hint(code: i32) -> Option<&'static str> { ... }
pub(crate) fn describe(...) -> ... { ... }
pub(crate) const SYSTEM_ENDPOINTS: ... = ...;
pub(crate) struct HostTransport { ... }        // 包 yang_base::http::HttpClient + CreateGate
pub(crate) struct Outbound { pub tokens: Arc<TokenProvider>, ... }
```

其余文件同理：

- `domain/tenant_token.rs` → `pub(crate) use yang_feishu::auth::{...}`，保留 `HostTokenCache`（Redis 适配）与 `RELEASE_LOCK_SCRIPT`。
- `domain/user_token.rs` → OAuth 协议半部 re-export；`UserTokenRow`/`load_single`/`store_authorization`/`update_tokens`/`user_access_token` 留宿主。
- `domain/{bitable,approval,approval_uuid,approval_convert,approval_match,widget_rules,crypto,token,i18n,pagination,protocol}.rs` → 只 re-export 迁走符号；产品类型与函数留原地。
- `domain/approval_rate_limit.rs` 整体留宿主。

shim 保留到 0.1.0 发布后一个版本；之后可选择性删除。

### 4.2 `outbound_setup` → `build_client`

```rust
pub(crate) fn build_client(
    ctx: &ActionContext,
    settings: &FeishuSettings,
) -> Result<Outbound, BaseError> {
    let deployment = ctx.extension::<AuthorizationVersionCache>().deployment();
    let namespace = format!("yang-system:{deployment}:feishu");
    let client = OpenApiClient::builder(
        OpenApiConfig::static_credentials(&settings.app_id, &settings.app_secret)
            .endpoints(Endpoints::feishu())
            .cache_namespace(namespace),
    )
    .transport(Arc::new(HostTransport::new(
        ctx.tools().http(),
        CreateGate::new(ctx.tools().cache()?, &settings.app_id)?,
    )))
    .sleeper(Arc::new(TokioSleeper))
    .token_cache(Arc::new(HostTokenCache::new(ctx.tools().cache()?)))
    .build()?;

    Ok(Outbound { client, tokens: client.tokens().clone(), ... })
}
```

`Outbound` 保留 `pub tokens` 字段，25 处 `outbound.tokens` 不改。

### 4.3 调用点清单（已补漏）

| 符号/改动 | 调用点 |
|---|---|
| `TenantTokenProvider::new` | **19 处**：`domain/outbound_setup.rs`；`infrastructure/feishu_approval_worker.rs:5172/6578/6952`；`feishu_event_worker.rs:66/1430/1799`；`feishu_pull.rs:187`；其余见附录 A.2。改走 `build_client` 后全部消失。 |
| `impl OutboundTransport` | **21 处**：生产 `HttpClientTransport`、`ExecutionTransport`、`GroupExecutionTransport`；其余 18 个测试替身。改为 `impl Transport`。 |
| `OutboundRequest {` 字面量 | **23 处**；字段名/顺序与 crate `Request` 一致，类型别名后原样编译。 |
| `WidgetMap.bitable_field` → `source_field` | `approval_match.rs:879`；`approval/actions/dispatch.rs:3110/3200`；`approval_feedback.rs:424/476/508`；`approval_provision.rs:1513/1687`；`feishu_approval_worker.rs:5495`；`approval_convert.rs`/`approval_attachment.rs`/`approval_dispatch.rs` 其余站点。**只改内部类型，不改前端/API 的 `bitable_field` 字段名。** |
| `validate_path_segment` | `approval.rs:63`；`datasource/actions/` 4 处；`feishu_approval_worker.rs:2119/2120/2454/2455/4784/4785`；`approval/actions/dispatch.rs:1920-1921`。严格版与宽松版按调用方分别替换。 |
| `FEISHU_OPEN_BASE` | `tenant_token.rs:383`；`user_token.rs:44/446`；`approval.rs:37`；`bitable.rs:40` 及 `:88/:97/:108/:119/:758/:1399/:1408`。迁到 `Endpoints`。 |
| `instance_applink` | `approval.rs:62-67`（定义）；`approval_convert.rs:818-822`（`connection_codes` 重复 appId）；`feishu_approval_worker.rs:4923/5800/5845`（测试）。 |
| `check_configs.rs:25-28` | 使用 `approval_match::FieldMapping` 与 `approval_provision::{build_plan, check_aggregate_batch, ProvisionError, ProvisionInput}`；`approval_match` 拆分时必须保留这些符号的宿主路径。 |
| `approval_execution/transport.rs:22-24` | 生产路径仍用 `/records/batch_update` 后缀识别回填；URL 构造迁走后必须改为基于 `Request` 的谓词或显式标记，不能留字符串嗅探。 |

### 4.4 事件入口

- `event_challenge.rs` 调 `EventVerifier::decode_challenge`；`receive_approval_event.rs` 调 `decode_callback`。
- 宿主保留：路由、审批事件白名单、`instance_code` 非空/长度校验、`operate_time` 字符串/数字兼容、`token` 删除、`feishu_approval_event` 落库、1062 去重。
- **错误语义**：配置了 `encrypt_key` 却收到明文 → 保持 `BaseError::Unauthorized`（HTTP 401），不要映射成 403。`event_challenge.rs:135-145` 的 `encrypt_key=None` 委托语义必须保留，否则共享 URL 的加密事件会 401。
- `X-Lark-Signature` **不在本期**：当前 `yang-base` 的 `Request` 没有 `raw_body`（`crates/yang-base/src/action/request.rs:62-77`），axum transport 也未保留原始字节。先单独立项；在完成前 `EventConfig.require_signature=false`，只做 `verification_token` 校验。

### 4.5 安全清理（Phase 0，先于一切迁移）

`src/addon/feishu/domain/crypto.rs:484-508` 的 `real_feishu_encrypted_challenge_decrypts_correctly` 内联了真实生产 `event_encrypt_key` 与从生产日志截取的密文。必须：

1. 在飞书开发者后台**轮换**该 `event_encrypt_key`；
2. 删除该测试，或改成环境变量驱动的 `#[ignore]` 测试；
3. 确认该 key 是否与外部选项 `encryption_key` 复用；若复用，旋转会牵动已封存的 `token_cipher`，需先做密钥轮换方案（`encryption_key` 目前无版本列）。
4. 不要把该测试迁入 crate。

---

## 5. Workspace / CI / 发布

### 5.1 根 `Cargo.toml`

`members = ["crates/*"]` 会自动收编 `crates/yang-feishu`，但 `[workspace.dependencies]` 需要补：

```toml
aes = "0.8"
cbc = { version = "0.1", features = ["alloc", "block-padding"] }
subtle = "2.6"
rand_core = { version = "0.6", features = ["getrandom"] }
```

`reqwest`、`rustls`、`redis`、`metrics`、`serde`、`serde_json`、`thiserror`、`tracing`、`sha2`、`base64`、`async-trait`、`uuid`、`tokio` 已存在，直接复用。

> **tokio 特例**：根 workspace 的 `tokio` 是 `features = ["full"]`。crate 为保持发布依赖最小，**不要**用 `tokio.workspace = true`；显式写 `tokio = { version = "1.52.1", default-features = false, features = ["time"] }`，dev-dependencies 再补 `macros`/`rt-multi-thread`。

### 5.2 `crates/yang-feishu/Cargo.toml`

```toml
[package]
name = "yang-feishu"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
description = "飞书开放平台协议客户端：传输、令牌、多维表格、审批、事件、外部选项"
keywords = ["feishu", "lark", "openapi", "bitable", "approval"]
categories = ["api-bindings", "asynchronous"]
publish = false

[features]
default = []
http = ["dep:reqwest", "dep:rustls"]
redis = ["dep:redis"]
metrics = ["dep:metrics"]
full = ["http", "redis", "metrics"]

[dependencies]
async-trait.workspace = true
serde = { workspace = true, features = ["derive"] }
serde_json.workspace = true
thiserror.workspace = true
tracing.workspace = true
sha2.workspace = true
base64.workspace = true
subtle.workspace = true
rand_core.workspace = true
aes.workspace = true
cbc.workspace = true
uuid = { workspace = true, features = ["v4"] }
tokio = { version = "1.52.1", default-features = false, features = ["time"] }
reqwest = { workspace = true, optional = true }
rustls = { workspace = true, optional = true }
redis = { workspace = true, optional = true }
metrics = { workspace = true, optional = true }

[dev-dependencies]
tokio = { version = "1.52.1", features = ["macros", "rt-multi-thread", "time"] }

[lints]
workspace = true
```

`#![forbid(unsafe_code)]`、`#![warn(missing_docs)]`、`publish = false`（0.1.0 先 path 依赖，验证后再决定是否发布）。

### 5.3 `scripts/run_ci.py`

- `AUXILIARY_PACKAGE_FLAGS`（:25-32）增加 `-p yang-feishu`。
- 增加 `cargo test --doc -p yang-feishu --locked`。
- `FEATURE_CASES`（:127-153）增加 5 组：
  - `yang-feishu-none` → `--no-default-features`
  - `yang-feishu-http` → `--no-default-features --features http`
  - `yang-feishu-redis` → `--no-default-features --features redis`
  - `yang-feishu-metrics` → `--no-default-features --features metrics`
  - `yang-feishu-all` → `--all-features`
- `expected_features`（:349-369）同步加这 5 个名字。
- `run_ci.py:382-407` 断言 `tested_packages == workspace_packages` 与 `clippy_packages == workspace_packages`；`-p yang-feishu` 同时满足两处。新 manifest 必须 `rust-version.workspace = true`（:388-389 会校验）。

### 5.4 `.github/workflows/ci.yml`

- stable job 的 test 行（:57）与 clippy 行（:59）各加 `-p yang-feishu`。
- 增加 `cargo test --doc -p yang-feishu --locked`。
- feature-matrix（:107-167）增加与 `FEATURE_CASES` 完全一致的 5 组。
- MSRV job（:68-79）的 `cargo check --workspace --all-targets --all-features --locked` 自动覆盖新成员。

### 5.5 契约脚本

- `scripts/verify_ci_contract.py` 的 `REQUIRED_FRAGMENTS`（:21-27）是**逐字匹配**：必须把 ci.yml 的两整行 `-p` 命令串、doc 行、5 个 feature 名同步写进去，否则 stable job 自检立刻红。
- `scripts/verify_feature_isolation.py`（:23-33）增加 5 条 Case：
  - `none`：要求 `aes/cbc/sha2/base64/subtle`，禁止 `reqwest/rustls/redis/sqlx/yang-base/yang-db`
  - `http`：要求 `reqwest/rustls`
  - `redis`：要求 `redis`
  - `metrics`：要求 `metrics`
  - `all`：要求 `reqwest/rustls/redis/metrics`
  - 全部禁止 `sqlx`、`yang-base`、`yang-db`

### 5.6 文档

- `README.md:53` 当前写「根 workspace 包含 yang-base、yang-base-derive、yang-db、yang-runtime 和 yang-pcg」，补 `yang-feishu`。
- `AGENTS.md:36,62` 的 crate 数量、feature 矩阵组数（当前 17 组）同步更新。
- `crates/yang-feishu/README.md`：给 30 行接入样例（见 §7.3）。
- `crates/yang-feishu/CHANGELOG.md`：从 0.1.0 起追加式维护。

### 5.7 版本与发布

- `0.1.0` 先 `publish = false`，只走 path 依赖；宿主先接。
- 等第二个真实 Rust 消费者出现、且公开面稳定一个迭代后，再考虑发布 crates.io。
- 公开面用 `#[non_exhaustive]`；破坏性变更走 minor（0.x）。
- `CHANGELOG.md` 记录每次公开面变化；CI 可加 `cargo public-api` 快照（Phase 5 可选）。

---

## 6. 施工计划

每个 Phase 结束都必须满足：`cargo test` + `cargo clippy -D warnings` + `cargo fmt --check` 全绿，宿主可运行。

### Phase 0 — 安全与基线（1 人日）

| 任务 | 验收 |
|---|---|
| P0-1 轮换事件 EncryptKey，删除/隔离 `crypto.rs:484-508` 的真实 key 测试 | 仓库中不再出现真实 key；`grep` 无命中 |
| P0-2 记录基线：`yang-system` 跑 `cargo test --locked`；`lib_yang` 跑 `python scripts/run_ci.py quick` | 基线红绿清单入库（见附录 A 备注） |
| P0-3 建分支：`yang-system` 与 `lib_yang` 各一条；确认 `lib_yang` 工作区干净 | `git status` 干净；分支名记录 |

### Phase 1 — crate 脚手架 + transport + auth（8–12 人日）

| 任务 | 文件 |
|---|---|
| P1-1 建 crate、manifest、lib.rs、error/path/endpoints | `crates/yang-feishu/` |
| P1-2 transport：Request/Response/Transport/Sleeper/classify/disposition/RetryPolicy/错误码/重试常量 | 从 `domain/outbound.rs:124-781` 迁；删除审批 URL 嗅探与 endpoint 别名 |
| P1-3 auth：Credentials/Provider/TokenCache/TokenProvider/parse_token_grant/cache_ttl/LOCK_TTL/OAuthClient/StateStore | 从 `tenant_token.rs`、`user_token.rs` 协议半部迁 |
| P1-4 crypto 原语：derive_key/encrypt/decrypt/hash/verify/tokens_equal | 从 `crypto.rs:1-234`、`token.rs:1-57` 迁 |
| P1-5 宿主 shim：`outbound.rs`、`tenant_token.rs`、`user_token.rs`、`crypto.rs`、`outbound_setup.rs` | 见 §4.1 |
| P1-6 测试：迁 outbound 882 行、tenant 约 400 行、crypto 约 200 行；新增 fake Transport | `tests/transport_retry.rs` |

**验收**：`cargo test -p yang-feishu`；`cargo test` in `yang-system`；`cargo tree -p yang-feishu` 中无 `yang-base`/`yang-db`/`sqlx`；`grep -ri "yang-system\|portal\|approval_dispatch" crates/yang-feishu/src` 无命中。

### Phase 2 — bitable（5–7 人日）

| 任务 | 文件 |
|---|---|
| P2-1 拆 `bitable.rs`：客户端子集、DTO、`PaginationState`、`CellValue`、`BitableCoordinates` | `bitable/` |
| P2-2 产品语义留宿主：`pending_records_filter` 改名 `pending_backfill_filter` 进 crate？**是**（它只是 filter DSL 组装）；`SYSTEM_FIELD_KEYS` 留宿主；`resolve_current_field_names` 删除（零调用点） | `bitable/filters.rs`、宿主 `bitable.rs` |
| P2-3 `record_row_numbers` 增加显式 `max_pages` 参数；保留「部分结果」语义并在文档标注 | `bitable/client.rs` |
| P2-4 宿主 shim 与 4 个 datasource Action / `pull.rs` 改 import | `domain/bitable.rs` |
| P2-5 测试：迁分页收敛、重复游标、`has_more` 缺 token、字段类型、单元格归一化 | `tests/bitable_pagination.rs` |

**验收**：`list_all_records` 对真实表读数与旧实现逐行一致；`first_records_page` 仍不套收敛断言；`search_all_records` 行为不变。

### Phase 3 — approval + form/value + contact/im/drive（10–14 人日）

| 任务 | 文件 |
|---|---|
| P3-1 `ApprovalClient`：create/get/definition/subscribe；保留 60012/1390003/1390007 语义 | `approval/client.rs` |
| P3-2 `WidgetMap`/`Converter`/`build_form`/`build_group_form`/`build_signature`/`amount_currency` | `approval/value.rs` |
| P3-3 `FormWidget`/`parse_form`/`WidgetOptions`/控件规则表 | `approval/form.rs`、`widget_rules.rs` |
| P3-4 `derive_idempotency_key`（与存量 uuid 逐字节一致） | `approval/idempotency.rs` |
| P3-5 `ContactClient`/`ImClient`/`DriveClient` | `contact.rs`、`im.rs`、`drive.rs` |
| P3-6 宿主保留 `Column`/`FieldMapping`/`MatchError`/`match_fields`/`ProvisionError`/`check_*`/`preflight`/`source_identities`/`instance_applink` | 见 §3.2 |
| P3-7 `WidgetMap.bitable_field` → `source_field` 全站点改名 | 见 §4.3 |
| P3-8 测试：迁审批契约、表单解析、值编码、附件契约 | `tests/approval_contract.rs` |

**验收**：`derive_uuid` 对同一组 `(base, table, approval_code, record_id)` 输出与存量逐字节一致；`build_form` 对现有配置生成压缩 JSON 字符串与旧实现一致；`instance_applink` 仍由宿主生成，硬编码 appId 不进 crate。

### Phase 4 — event + options + crypto 清理（4–6 人日）

| 任务 | 文件 |
|---|---|
| P4-1 `EventVerifier::decode_challenge`/`decode_callback`；保留 401 语义与 `encrypt_key=None` 委托 | `event.rs` |
| P4-2 外部选项 DTO/信封/i18n/游标 | `options/` |
| P4-3 宿主 `FeishuOptionsRequest` 改成 `#[serde(transparent)]` newtype 并实现 `ParamInput` | 宿主 `protocol.rs` |
| P4-4 `crypto.rs` 只留 `seal`/`issue_token` 等宿主族，内部调 crate 原语 | 宿主 `crypto.rs` |
| P4-5 测试：challenge 解密、verification_token、明文拒绝、游标 fail-closed、i18n 形状 | `tests/event_verify.rs` |

**验收**：事件端点对明文/密文/错 token 的 HTTP 状态与现网一致；crate 内无中文业务文案；`X-Lark-Signature` 仍未实现（明确记录为 known gap）。

### Phase 5 — 宿主切换 + CI + 真机回归（7–10 人日）

| 任务 | 验收 |
|---|---|
| P5-1 全仓 `use` 清理与 shim 验证；`cargo test --locked` | 零编译警告 |
| P5-2 CI 接线（run_ci / ci.yml / verify_ci_contract / verify_feature_isolation） | `python scripts/run_ci.py quick` 全绿 |
| P5-3 真机回归清单（§7.2） | 全部通过并记录 serial/实例码 |
| P5-4 文档：README/AGENTS/CHANGELOG/`examples/bitable_pull.rs` | 30 行样例可独立跑 |
| P5-5 0.1.0 打 tag（`publish=false`） | 宿主 path 依赖锁定 |

**总计：35–50 人日。**

---

## 7. 测试与真机回归

### 7.1 随迁 / 重写 / 留下

| 测试 | 处置 |
|---|---|
| outbound 882 行（分类、重试、429、业务码、非幂等） | 随迁到 `tests/transport_retry.rs` |
| tenant token 约 400 行（锁 TTL、缓存 TTL、扁平信封、降级） | 随迁到 `tests/transport_retry.rs` 或 `auth` 单测 |
| crypto 约 200 行（AES 向量、常数时间、畸形 fail-closed） | 随迁；**删除真实生产 key 测试** |
| bitable 约 1100 行（分页、字段、单元格、链接） | 随迁；`pending_records_filter` 相关留宿主集成测试 |
| approval 契约（create/get/definition/subscribe） | 随迁；60012/1390003/1390007 用假 Transport 覆盖 |
| form/value（控件规则、日期、金额、DateInterval） | 随迁到 `tests/approval_contract.rs` |
| event（challenge、解密、token 比对、日志不含明文） | 随迁到 `tests/event_verify.rs` |
| 宿主 `approval_dispatch`/`worker`/`portal`/`pull` 集成测试 | **留下**，只改 import |
| 真实 MySQL/Redis 集成测试 | 留下；crate 不引入 sqlx |

### 7.2 真机回归清单（必须逐项记录结果）

1. `tenant_access_token` 冷启动换取、缓存命中、过期强刷；观察 Redis 键与 TTL。
2. 多维表格：`list_all_records` 全量收敛（含 `has_more`/`page_token`）、`list_all_fields` 第二页、`search_records` 筛选、`batch_update_records` 回写。
3. 审批：创建实例（记 `instance_code`）、取详情（`serial_number`）、订阅（重订幂等 `1390007`）。
4. 60012：人为制造 uuid 冲突，确认按 uuid 反查并继续，不写终态错误。
5. 审批表单：金额控件 `currencyRange`、`DateInterval`、附件上传、控件组 `fieldList`。
6. IM：发送 + 编辑；确认 `uuid` 去重。
7. 云盘：下载多维表格附件 → 上传为审批附件；确认长度与 `Content-Type`。
8. 事件：`url_verification` challenge、加密事件解密、错误 `verification_token` 拒绝；确认 HTTP 401 而非 403。
9. 外部选项：非搜索型首响应全量、`@i18n@` 资源、`linkage_params` 文案值、`page_token` 非法 fail-closed。
10. Lark 国际版（若可用）：用 `Endpoints::lark()` 与 `Endpoints::custom` 各跑一次；`upload` 域未验证前不得宣称支持。

### 7.3 独立接入样例（`examples/bitable_pull.rs`）

```rust
use std::sync::Arc;
use yang_feishu::{Endpoints, OpenApiClient, OpenApiConfig, auth::MemoryTokenCache, transport::TokioSleeper};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = OpenApiClient::builder(
        OpenApiConfig::static_credentials(
            std::env::var("FEISHU_APP_ID")?,
            std::env::var("FEISHU_APP_SECRET")?,
        )
        .endpoints(Endpoints::feishu())
        .cache_namespace("demo"),
    )
    .sleeper(Arc::new(TokioSleeper))
    .token_cache(Arc::new(MemoryTokenCache::default()))
    .build()?;

    let snapshot = client
        .bitable()
        .list_all_records(&std::env::var("BITABLE_APP_TOKEN")?, &std::env::var("BITABLE_TABLE_ID")?, None, None)
        .await?;
    println!("rows = {}", snapshot.items.len());
    Ok(())
}
```

---

## 8. 风险与回滚

### 8.1 高危点

| 风险 | 影响 | 缓解 |
|---|---|---|
| Redis 缓存键变更 | 全实例冷启动同时打飞书签发接口 | 键固定 `{namespace}:app:{app_id}:tenant_token`；Phase 1 用单测钉死旧键；部署 canary 观察 token 刷新次数与 429；低峰发布 |
| `FailureKind` 语义折叠 | 60012/1390003 被写成终态，丢失回捞 | 保留 5 变体；宿主 12 处 `match` 不改写；单测覆盖 60012/1390003/1390007 |
| 事件 401 → 403 | 飞书事件重推/告警口径变化 | `decode_*` 只返回 crate Error；宿主映射保持 `Unauthorized` |
| 审批创建限流时序 | 每次真实出站（含重试）必须领名额 | `HostTransport` 内保留 `CreateGate`；`is_create_request` 谓词替代 URL 嗅探；单测钉住「每次 send 都领」 |
| `validate_path_segment` 合并 | bitable 的路径穿越防护被降级 | 保留严格/宽松两个函数（§0.3 #10） |
| `WidgetMap.bitable_field` 改名 | 10+ 构造点编译失败 | 只改内部类型；前端/API 字段名不变；按 §4.3 清单一次改齐 |
| `record_row_numbers` 部分结果 | 上层把截断当完整 | 显式 `max_pages` 参数 + 文档标注；宿主继续按 20 页调用 |
| tokio `full` 传染发布依赖 | 独立使用者拖入完整运行时 | crate 显式 `default-features=false, features=["time"]`，不走 workspace 继承 |
| `crypto.rs` 生产 key | 公共库分发即泄密 | Phase 0 轮换 + 删除测试；不迁入 crate |

### 8.2 已知缺口（本次不修，单独立项）

- `X-Lark-Signature` / 时间戳重放窗口：需要 `yang-base` `Request.raw_body` + axum 原始字节保留。
- 用户 token 明文入库：crate 只搬 OAuth 协议；存储加密/多用户建模留宿主后续。
- Lark 国际版 `upload` 域未真机验证。
- IM 富文本 schema（`zh_cn.title/content`、`tag:text/a`）留宿主，crate 只发送调用方给的 `content`。
- `bitablePerm` extra 与 200ms pacing 留宿主。

### 8.3 回滚

- 每个 Phase 是独立 PR；shim 保证宿主始终可编译。
- 回滚一个 Phase = revert 该 PR；因为 crate 与宿主通过 shim 解耦，不需要同时回滚两边。
- Phase 1 的 Redis 键变更单独一个 PR，可 canary 回退。
- 0.1.0 打 tag 前不发布 crates.io，回滚无外部消费者影响。

---

## 9. Definition of Done

- [ ] `crates/yang-feishu` 在 `cargo check --workspace --all-targets --all-features --locked` 下通过（MSRV 1.80）。
- [ ] `cargo tree -p yang-feishu` 无 `yang-base` / `yang-db` / `sqlx` / `schemars` / `anyhow`。
- [ ] `grep -ri "yang-system\|portal\|approval_dispatch\|source_key" crates/yang-feishu/src` 无命中（产品词不进公共面）。
- [ ] `run_ci.py` 的 stable / MSRV / feature 矩阵 / 契约自检全绿。
- [ ] `yang-system` 全量测试绿；真机回归清单 §7.2 全部通过。
- [ ] 仓库中不再有真实 `event_encrypt_key`；该 key 已在飞书后台轮换。
- [ ] `examples/bitable_pull.rs` 30 行可独立跑通（只用 crate 公开面）。
- [ ] `README.md` / `AGENTS.md` / `CHANGELOG.md` / crate `README.md` 已更新。
- [ ] 附录 A 的每个 `move*` 符号都有对应目标文件；每个 `stay-in-host` 符号仍能在宿主找到。

---

## 附录

### 附录 A：符号级清单

[`2026-11-05-yang-feishu-extraction-symbol-map.md`](./2026-11-05-yang-feishu-extraction-symbol-map.md)

包含 8 个模块、565 个符号的 `file:line`、类型、处置、依赖、备注，以及每个模块的：
- 源文件与生产/测试行区间
- 需先重构项
- 目标公开 API
- 测试处置（随迁/重写/留下）
- 宿主替换点
- 风险

### 附录 B：飞书平台事实清单

见 §3.3。建议在 crate 的 `README.md` 中保留一份精简版，作为独立使用者最容易踩坑的入口。

### 附录 C：关键错误码

| 码 | 含义 | 处置 |
|---|---|---|
| `99991400` / `1254290` / `230020` | 频控（HTTP 429 或 200/400） | `Retry`，读 `x-ogw-ratelimit-reset` |
| `99991663` / `20013` | token 过期 | `TokenExpired`，清缓存强刷一次 |
| `99991661/64/65/68/71` | token 缺失/形态错 | `Fatal`，不要刷新 token |
| `1254036` / `1254607` / `1255001` / `1255002` | 瞬态，可重试 | `Retry` |
| `1254030` | 响应过大 | `Fatal`，提示降低 `page_size` |
| `1254302` / `1254303` | 权限不足（HTTP 200） | `Fatal` |
| `60012` | uuid 已创建（响应丢了） | `UuidConflict`，按 uuid 反查 |
| `1390003` | 审批实例不存在 | `InstanceNotFound`，回到 create 或退避 |
| `1390007` | 已订阅/已取消 | 幂等成功 |
| `1390001` | 审批控件值非法 | `Fatal`，检查金额币种/控件类型 |
| `1254003` / `1254040` / `1254041` | Base/表不存在 | `Fatal` |
