# yang-feishu Agent Guide

面向在本仓库或其它 Rust 项目里**使用** `yang-feishu` 的 agent。实现细节见
`src/`，官方平台事实见 `README.md`。

## 先选身份与装配

```rust
use std::sync::Arc;
use yang_feishu::auth::MemoryTokenCache;
use yang_feishu::transport::TokioSleeper;
use yang_feishu::{Endpoints, OpenApiClient, OpenApiConfig};

let client = OpenApiClient::builder(
    OpenApiConfig::static_credentials(app_id, app_secret)
        .endpoints(Endpoints::feishu())      // 或 Endpoints::lark() / custom(...)
        .cache_namespace("my-deployment"),   // 令牌缓存键前缀
)
// 独立项目：启用 http feature 后可省略 transport，默认 ReqwestTransport。
// 宿主已有 HTTP/Redis：注入自己的 Transport / TokenCache，不要启用 http/redis feature。
.transport(Arc::new(my_transport))
.sleeper(Arc::new(TokioSleeper))
.token_cache(Arc::new(MemoryTokenCache::default()))
.build()?;
```

- `client.app_id()` / `client.endpoints()` / `client.tokens()`
- `client.send_raw(request, &policy)`：不带鉴权（OAuth、用户 token）
- `client.send_tenant(request, &policy)`：自动取 tenant token，失效强刷一次
- 子客户端：`bitable()`、`approval()`、`contact()`、`im()`、`drive()`、`oauth()`、
  `api()`（全量官方端点薄封装）

## Feature 门控（0.2.0 起）

`default = []`：默认只编译协议核心，不编译任何 typed 模块或生成域。

- typed：`bitable` / `approval` / `contact` / `im` / `drive`
- 单域：`api-<domain>`（如 `api-docx`、`api-sheets`）
- 分组：`docs` / `calendar` / `task` / `mail` / `attendance` / `hr` / `ai` /
  `admin` / `auth`
- 全量：`api-all`（66 个生成域）或 `full`（全部能力 + http/redis/metrics）

独立项目最小组合：`features = ["http", "bitable"]`。宿主已有 HTTP/Redis 时不要
开 `http` / `redis`。

## 模块导航

| 需求 | 入口 | 关键方法 |
|---|---|---|
| 任意官方 REST 端点 | `client.api()` | `docx_*` / `sheets_*` / `calendar_*` / `task_*` / `mail_*` / `corehr_*` 等 2372 个方法 |
| 多维表格读 | `client.bitable()` | `list_all_records` / `list_all_fields` / `list_all_tables` / `list_all_views` / `search_records` / `get_records_by_ids` / `record_row_numbers` |
| 多维表格写 | `client.bitable()` | `batch_update_records`；生成的长尾方法 `record_create` / `record_batch_create` / `record_delete` 等 |
| 审批建单/查单 | `client.approval()` | `create_instance` / `get_instance` / `get_approval_definition` / `subscribe_approval` |
| 审批任务/撤回/抄送 | `client.approval()` | 生成的长尾方法 `cancel_instance` / `cc_instance` / `approve_task` / `reject_task` / `transfer_task` 等 |
| 部门名/姓名→open_id | `client.contact()` | `resolve_department_name` / `resolve_applicant` |
| IM 发/改富文本 | `client.im()` | `send_post` / `edit_post`；长尾 `delete_message` / `reply_message` / `forward_message` 等 |
| 附件下载/上传 | `client.drive()` | `download_media` / `upload_approval_file` |
| multipart 构造 | `yang_feishu::multipart` | `Multipart::new` / `text` / `file` / `finish`（返回 `Result`，校验 CRLF） |
| 事件入站 | `yang_feishu::event` | `EventVerifier::decode_challenge` / `decode_callback` |
| 外部选项 | `yang_feishu::options` | `build_result_body` / `encode_cursor` / `decode_cursor` |
| AES/SHA-256 | `yang_feishu::crypto` | `derive_key` / `encrypt_bytes` / `decrypt_bytes` / `hash_token` / `verify_token` |

## 常用配方

### 1. 全量拉取多维表格（收敛失败即 Err）

```rust
let snapshot = client
    .bitable()
    .list_all_records(&app_token, &table_id, None, Some(&["申请人", "状态"]))
    .await?;
assert!(snapshot.total >= 0);
```

- `field_names` 必须是**精确字段名**，不是 field_id；先用 `list_all_fields` +
  `resolve_field_names` 解析。
- `list_all_records` 任何一页失败或未收敛都返回 Err，绝不返回部分快照。
- `first_records_page` 只取一页、不套收敛断言（诊断用）。
- `record_row_numbers` 是尽力而为，可能返回部分结果。

### 2. 建审批单（60012 不是失败）

```rust
use yang_feishu::approval::{CreateOutcome, GetOutcome};

let uuid = yang_feishu::approval::derive_idempotency_key(&[
    &app_token, &table_id, &approval_code, &record_id,
]);
let form = yang_feishu::approval::build_form(&FormInput { /* ... */ })?;
match client.approval().create_instance(&approval_code, &form, &open_id, &uuid).await? {
    CreateOutcome::Created { instance_code } => { /* 落库 */ }
    CreateOutcome::UuidConflict => {
        // 响应丢了，实例其实已建：按 uuid 反查
        match client.approval().get_instance(&uuid).await? {
            GetOutcome::Found(detail) => { /* detail.serial_number */ }
            GetOutcome::NotFound => { /* 并发窗口：下一轮重试 */ }
        }
    }
}
```

- `FormInput` 的 `approval_app_id` 传审批应用 appId 时，`connect` 控件的 AppLink
  会被解析成实例 Code；传 `None` 时只接受 36 位实例 Code。
- 金额币种用 `amount_currency(form_snapshot, widget_id)`。

### 3. 发 IM post / 编辑

```rust
use yang_feishu::im::{MessageRecipient, ReceiveIdType};

let recipient = MessageRecipient {
    receive_id_type: ReceiveIdType::OpenId,
    receive_id: "ou_xxx".into(),
};
recipient.validate()?;
let message_id = client.im().send_post(&recipient, &content, &uuid).await?;
client.im().edit_post(&message_id, &content).await?;
```

`content` 是飞书 post 的 `{"zh_cn":{"title":..., "content":[...]}}` 结构；
`uuid` 由调用方生成，用于失败路径回传去重。

### 4. 事件入站

```rust
use yang_feishu::event::{EventConfig, EventVerifier, Inbound};

let verifier = EventVerifier::new(EventConfig {
    verification_token: token,
    encrypt_key: Some(key),       // 配置 Encrypt Key 后必须加密
    require_signature: false,     // X-Lark-Signature 尚未实现
})?;
match verifier.decode_challenge(&raw_body)? {
    Inbound::Challenge(text) => { /* 原样返回 {"challenge": text} */ }
    Inbound::Callback(event) => { /* 落库/投递；token 已从 payload 移除 */ }
}
```

错误映射：明文但配置了 Encrypt Key → `Unauthorized`(401)；challenge token 错 →
`Forbidden`(403)；解密/配置故障 → 5xx（让飞书重推）。

### 5. 外部选项响应

```rust
use yang_feishu::options::{build_result_body, OptionRow, FeishuEnvelope};

let body = build_result_body(&rows, "zh_cn", has_more, next_page_token);
let response = if encryption_key {
    FeishuEnvelope::encrypted(crypto::encrypt_json(&body, &key)?)
} else {
    FeishuEnvelope::ok(body)
};
let text = response.to_json()?;
```

## 错误模型

```rust
pub struct Error { pub kind: FailureKind, pub message: String }
pub enum FailureKind {
    Retry { retry_after_seconds: Option<u64> },
    TokenExpired,
    UuidConflict,
    InstanceNotFound,
    Fatal { code: i32 },
}
```

| kind | 处置 |
|---|---|
| `Retry` | 退避重试；优先读 `x-ogw-ratelimit-reset` |
| `TokenExpired` | 客户端内部已清缓存强刷一次；仍失败按 Fatal/原样上抛 |
| `UuidConflict` | 60012：按 uuid 反查，不要写终态 |
| `InstanceNotFound` | 1390003：并发窗口，回到 create 或退避 |
| `Fatal { code }` | 配置/权限错误，不重试；按 code 补运维文案 |

`Error::Display` 固定 `飞书出站调用失败: {message}`；中文 hint 由宿主展示层补。

## 端点覆盖（四层）

1. **typed 核心链路**：记录读取/分页、审批建单/查单/表单、IM post、附件搬运。
2. **全量官方端点**（`client.api()`，`src/api/*.rs`）：官方 SDK、llms 文档与历史
   版本中的 1713 个 REST 端点，按 66 个域生成 2372 个方法。方法名带域前缀，例如
   `docx_documents_create`、`sheets_spreadsheets_values_get`、
   `calendar_calendars_events_create`。
3. **官方 v1/v4 薄封装**（`src/bitable/endpoints.rs`、`src/approval/endpoints.rs`、
   `src/contact_endpoints.rs`、`src/im_endpoints.rs`、`src/drive_endpoints.rs`）：
   路径逐条来自官方 markdown 文档或 lark-cli dry-run，覆盖 App/表/视图/字段/记录/
   仪表盘/表单/工作流/角色、审批实例/定义/任务、通讯录、IM 消息/群聊/Pin/表情、
   云文档文件/权限/评论等。
4. **Base v3 薄封装**（`src/bitable/v3.rs`，107 个方法）：对齐本地 `lark-cli base`
   使用的 `/open-apis/base/v3/...` 路径；这部分官方 llms 索引未完整收录，升级
   lark-cli 后应重新生成。方法名统一 `v3_` 前缀，避免与 bitable v1 冲突。

全量端点的方法签名约定：

```rust
// tenant 身份（客户端自动取 token）
pub async fn docx_documents_create(&self, query: &[(&str, &str)],
    body: &serde_json::Value) -> Result<serde_json::Value>

// user 身份：第一个参数是调用方 token，方法名带 _user
pub async fn contact_departments_list_user(&self, bearer_token: &str,
    query: &[(&str, &str)]) -> Result<serde_json::Value>

// app 身份：方法名带 _app；无鉴权端点带 _public
pub async fn authen_access_token_create_app(&self, app_access_token: &str, ...)

// multipart 上传：传 Content-Type 与完整请求体
pub async fn im_files_create(&self, query: &[(&str, &str)],
    content_type: &str, body: &[u8]) -> Result<serde_json::Value>

// 二进制下载：返回原始 Response，由调用方读取 bytes
pub async fn im_files_get(&self, file_key: &str,
    query: &[(&str, &str)]) -> Result<Response>
```

- 路径参数是显式 `&str` 并做注入校验；query 用 `&[(&str, &str)]`。
- 二元身份端点同时生成基础方法（tenant）与 `_user`；只有 user 的端点只生成 `_user`。
- 文档注释带官方 API Explorer URL；`Value` 字段形状以官方文档为准。
- 写接口不会自动确认业务意图；调用方负责幂等键与重试策略。
- 重新生成与校验：
  `python -I scripts/generate_openapi_endpoints.py --catalog ... --out crates/yang-feishu/src/api`
  与 `python -I scripts/verify_openapi_coverage.py`。

端点目录与覆盖说明见
`docs/superpowers/specs/2026-11-06-yang-feishu-official-api-verification.md`。

## 平台事实（必读）

- 成败判据是**响应体业务码**，不是 HTTP 状态码；大量错误是 HTTP 200 + 非 0 码。
- `99991663` 常以 HTTP 400 返回，必须先于 401/403 判 token 失效。
- 频控等待在 `x-ogw-ratelimit-reset`（秒），没有 `Retry-After`。
- `has_more=true` 缺 `page_token` 必须硬失败；`code=0` 缺 `data` 必须报错。
- 60012 / 1390003 / 1390007 的语义见上表。
- 多维表格字段 `field_names` 用精确字段名；`record_id` 跨副本不唯一。
- 外部选项 `linkage_params` 真机值是文案，不是 `@i18n@<id>`。
- 文本列含 URL 会被切成富文本分段，取第一段会丢正文。

## 验证命令

```bash
cargo test -p yang-feishu --all-features
cargo clippy -p yang-feishu --all-targets --all-features -- -D warnings
cargo test -p yang-feishu --doc
```

## 不要做

- 不要为了「统一」把 `PaginationState` 的三条硬失败改成 break。
- 不要手工编辑 `src/api/*.rs`；改端点目录后运行生成脚本并跑覆盖率校验。
- 不要把 `FailureKind::UuidConflict` / `InstanceNotFound` 折成 `Fatal`。
- 不要在 crate 里硬编码产品 appId、宿主表名、中文运维 hint。
- 不要给 `connect` 的 AppLink 解析硬编码 appId；用 `FormInput::approval_app_id`。
- 不要把 `X-Lark-Signature` 宣称已实现；`require_signature=true` 会显式失败。
