---
name: yang-feishu
description: 使用 lib_yang 的 yang-feishu Rust crate 调用飞书开放平台（tenant token、多维表格、审批、通讯录、IM、云盘、事件、外部选项）。当你要在 Rust 项目里接入飞书、排查飞书出站错误、或选择 typed/长尾接口时使用。
metadata:
  requires:
    bins: []
---

# yang-feishu 使用 Skill

本 skill 指导 agent 在 Rust 项目中装配和使用 `yang-feishu`。完整说明见 crate 根
`AGENTS.md`；这里给最短决策路径。

## 1. 判断项目形态

- **独立 Rust 项目**：`yang-feishu = { path/git = ..., features = ["http"] }`，
  builder 不传 transport，默认 `ReqwestTransport`；令牌缓存用 `MemoryTokenCache`
  或 `RedisTokenCache`（`redis` feature）。
- **已有 HTTP/Redis 的宿主**：`default-features = false, features = ["metrics"]`，
  注入自己的 `Transport` / `TokenCache`，不要启用 `http`/`redis`。

```rust
let client = OpenApiClient::builder(
    OpenApiConfig::static_credentials(app_id, app_secret)
        .cache_namespace("deployment-name"),
)
.transport(my_transport)       // 宿主
.token_cache(my_token_cache)   // 宿主
.build()?;
```

## 2. 选接口

1. **任意官方端点**：先查 `client.api()`。它覆盖 1709 个官方 REST 端点、2367 个
   方法；方法名带域前缀（如 `docx_documents_create`、`calendar_calendars_list`）。
   tenant 方法直接调用；user 端点用 `_user` 后缀并传 `bearer_token`；multipart
   传 `content_type` + `&[u8]`；二进制返回原始 `Response`。
2. 核心链路优先用 typed 方法（`AGENTS.md` 的模块导航）：多维表格、审批、表单、
   IM post、附件搬运都有强类型 DTO 与收敛断言。
3. 仍没有的端点：用 `client.send_tenant(Request{...}, &PULL_RETRY)` 裸调；不要猜
   路径，先查官方文档。

## 3. 三条硬规则

- **业务码优先**：`classify` 已处理；不要在调用方用 HTTP 状态码判成败。
- **60012/1390003 不是终态**：`UuidConflict` 按 uuid 反查，`InstanceNotFound`
  回到 create 或退避。
- **分页硬失败**：`list_all_records` 失败就是不完整快照，不能当部分成功写库。

## 4. 常用配方

- 拉表：`client.bitable().list_all_records(app_token, table_id, None, Some(&names))`
- 建单：`derive_idempotency_key` → `build_form` → `approval().create_instance`
- 通知：`im().send_post(recipient, &content, &uuid)`
- 附件：`drive().download_media` → `drive().upload_approval_file`
- 事件：`EventVerifier::decode_challenge` / `decode_callback`
- 外部选项：`options::build_result_body` + `crypto::encrypt_json`

## 5. 排查

| 现象 | 先看 |
|---|---|
| 所有请求 400/401 | token 缓存键、app_id/app_secret、`99991663` |
| 429/频控 | `x-ogw-ratelimit-reset`、`RetryPolicy` |
| 分页不收敛 | `page_token` 缺失/重复、`total` |
| 审批卡等待 | `record_id`→字段名解析、`rekey_cells_by_field_id` |
| 附件 403 | `bitablePerm` extra、`drive` 权限、200ms pacing |
| 事件 401 | Encrypt Key 配置、明文/密文策略、challenge token |

## 6. 验证

```bash
cargo test -p yang-feishu --all-features
cargo clippy -p yang-feishu --all-targets --all-features -- -D warnings
```

不要修改 crate 的硬失败语义、错误分类或产品 appId 约定；需要扩展时先读
`crates/yang-feishu/AGENTS.md`。
