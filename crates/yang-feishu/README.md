# yang-feishu

飞书开放平台协议客户端（Rust）：出站传输、tenant_access_token、多维表格、审批 v4、
通讯录、IM、云盘/附件、事件入站、外部选项协议与 AES/SHA-256 原语。

**只包含平台契约，不包含业务**：审批派发状态机、字段映射、数据源拉取编排、控制台
Action、产品表与中文运维文案都留在宿主。

## 安装（0.1.0 仅 path 依赖）

```toml
[dependencies]
yang-feishu = { path = "../../crates/yang-feishu", default-features = false, features = ["metrics"] }
```

独立使用者：

| feature | 内容 |
|---|---|
| `http` | `transport::http::ReqwestTransport`（自带 rustls） |
| `redis` | `auth::redis::RedisTokenCache` |
| `metrics` | `feishu_outbound_request_total` / `feishu_outbound_token_total` |
| `full` | 以上三者 |

宿主已有 HTTP/Redis 时**不要**开 `http` / `redis`，实现 `Transport` / `TokenCache`
注入即可。

## 30 行样例

```rust
use std::sync::Arc;
use yang_feishu::{Endpoints, OpenApiClient, OpenApiConfig};
use yang_feishu::auth::MemoryTokenCache;
use yang_feishu::transport::TokioSleeper;

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

（需要 `--features http`，见 `examples/bitable_pull.rs`。）

## 平台事实（mock 永远绿、真机才红）

- 成败判据是**响应体业务码**，不是 HTTP 状态码；`1254302`、`1254003`、`1254040` 等
  大量错误是 HTTP 200。
- `TokenExpired` 必须先于 401/403 判定；`99991663` 常以 HTTP 400 返回。
- 频控等待在 `x-ogw-ratelimit-reset`（秒），没有 `Retry-After`；`1254290` 是 HTTP 200。
- `tenant_access_token` 响应是扁平 `{code,msg,tenant_access_token,expire}`，不套 `data`；
  缓存 TTL = `expire - 300`；锁 TTL 30s > `TOKEN_RETRY` 最坏 19s。
- `has_more=true` 但 `page_token` 缺失/空串必须硬失败；`code=0` 但缺 `data` 必须报错。
- 记录页 `page_size` 上限 500；字段/表/视图页上限 100；重复游标硬失败。
- 60012 = uuid 已创建（响应丢了）；1390003 = 实例不存在；1390007 = 已订阅（幂等成功）。
- 金额控件 `currency` 只能从审批定义 `option.currencyRange` 取。
- `DateInterval` 是真实字段；外部选项 `linkage_params` 的真机值是文案，不是 `@i18n@id`。
- 文本列含 URL 会被切成富文本分段，取第一段会丢正文。

## 端点覆盖

- **typed 核心**：记录全量/分页、字段/表/视图解析、审批建单/查单/定义/订阅、表单
  值编码、IM post、附件搬运、事件与外部选项。
- **全量官方端点**：`client.api()` 覆盖官方 SDK、llms 文档与历史版本中的
  **1709 个 REST 端点**，按 66 个域生成 **2367 个方法**（tenant/user/app 身份变体
  分别计数）。生成源码在 `src/api/`，目录在
  `docs/superpowers/specs/2026-11-06-yang-feishu-openapi-catalog.json`、
  `...-openapi-manual.json` 与 `...-openapi-legacy.json`。
- **官方端点薄封装**：bitable v1（42）、approval v4（13）、contact（5）、
  IM（46）、Drive（25）；路径来自官方 markdown 文档或 `lark-cli` dry-run。
- **Base v3 薄封装**（107）：对齐 `lark-cli base` 的 `/open-apis/base/v3/...`；
  官方 llms 索引未完整收录的部分以 CLI 为准。
- **二进制/多部分**：IM 文件/图片上传、消息资源下载；Drive 文件/素材上传与下载。

长尾方法的路径参数是显式 `&str`，query 用 `&[(&str, &str)]`，body/response 为
`serde_json::Value`；multipart 端点接收 `content_type` 与 `&[u8]`，二进制端点返回
原始 `Response`。使用指南见 `AGENTS.md` 与 `skills/yang-feishu/SKILL.md`。

全量端点由脚本维护：

```bash
python -I scripts/generate_openapi_endpoints.py \
  --catalog docs/superpowers/specs/2026-11-06-yang-feishu-openapi-catalog.json \
  --catalog docs/superpowers/specs/2026-11-06-yang-feishu-openapi-manual.json \
  --catalog docs/superpowers/specs/2026-11-06-yang-feishu-openapi-legacy.json \
  --out crates/yang-feishu/src/api
python -I scripts/verify_openapi_coverage.py
```

## 已知缺口

- `X-Lark-Signature` 未实现；`EventConfig::require_signature=true` 会显式失败。
- Lark 国际版 `upload` 域未用真实租户验证。
- 关联审批的 AppLink 解析需要调用方提供 `FormInput.approval_app_id`；传 `None` 时
  只接受 36 位实例 Code。附件搬运编排、`bitablePerm` extra 与 200ms pacing
  属于宿主产品语义，不在本 crate。
