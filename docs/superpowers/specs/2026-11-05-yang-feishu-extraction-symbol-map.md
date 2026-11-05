# yang-feishu 抽取：符号级清单（附录 A）

> 本文件由 2026-11-05 的 8 路符号盘点工作流生成，保留全部原始符号、行号与处置。原始清单之间存在归属冲突；**施工时以正文 §0「裁决表」为准**，本文件用于逐项勾选与定位。

## A.1 L0 transport（飞书出站传输 / 失败分类 / 重试引擎）

**目标**：把 src/addon/feishu/domain/outbound.rs 中的出站传输抽象、失败分类与重试引擎抽成 crates/yang-feishu 的 transport 模块：crate 只保留飞书协议通用语义（HTTP 方法/请求响应形状/重试/429 与 token 族分类/日志截断与去重），审批与多维表业务语义（创建闸门、60012/1390003 语义变体、错误码域表、运维文案、endpoint 别名中的 approval/bitable 条目）全部留在宿主，通过 OutboundDomain + CodeTable 注入。宿主侧用 outbound.rs 薄 shim 保住 39 处既有 import 路径，用 ApprovalCreateGate 装饰器取代 HttpClientTransport 内的审批 URL 嗅探，保证行为与重试次数逐字节等价。

### 源文件

| 文件 | 生产行区间 | 生产行数 | 测试行区间 | 测试行数 |
|---|---|---:|---|---:|
| src/addon/feishu/domain/outbound.rs | 1-781 | 781 | 783-1664 | 882 |
| src/addon/feishu/domain/outbound_setup.rs | 1-114 | 114 | 116-139 | 24 |
| src/addon/feishu/domain/approval_rate_limit.rs | 1-67 | 67 | 68-205 | 138 |
| lib_yang/crates/yang-base/src/http/client.rs | 1-502 | 502 | src/http/__tests__/（独立目录） | 0 |

### 符号（29）

| # | 符号 | 类型 | 位置 | 处置 | 备注 |
|---:|---|---|---|---|---|
| 1 | `OutboundMethod` | type | src/addon/feishu/domain/outbound.rs:124-130 | **move-as-is** | 通用 HTTP 方法；HttpClientTransport 直接映射到 yang_base::http::HttpClient 的 get/post/put。无任何飞书业务语义。 |
| 2 | `OutboundRequest` | type | src/addon/feishu/domain/outbound.rs:137-154 | **move-as-is** | 字段由 pub(crate) 改 pub（宿主 17 处结构体字面量直接构造）。不自带 operation 字段：日志别名改由 OutboundDomain.operation 注入，避免 17 处字面量被迫改签名。query 以键值对承载（防手拼注入）的契约原样保留。 |
| 3 | `OutboundResponse` | type | src/addon/feishu/domain/outbound.rs:156-164 | **move-as-is** | headers 键恒小写的契约保留；宿主 ~20 处测试直接构造。 |
| 4 | `OutboundTransport` | trait | src/addon/feishu/domain/outbound.rs:166-173 | **move-as-is** | 「实现不做重试、重试只在 send_with_retry」的分层契约必须随注释一起搬走。全仓 17 个实现者（生产只有 HttpClientTransport + 宿主装饰器，其余是测试假传输）都留宿主；crate 只要求 Send+Sync。 |
| 5 | `HttpClientTransport` | type | src/addon/feishu/domain/outbound.rs:175-294 | **move-with-refactor** | 重构三处：删除字段 approval_create_limit（:178）、方法 with_approval_create_limit（:189-197）、发请求前的审批 URL 嗅探块（:202-223）。删完后 crate 不再依赖 yang_db::RedisClient。其余语义逐字保留：json body 补 charset、binary_limit、timeout、headers 小写化、200+JSON 不落 bytes、content-disposition 例外。 |
| 6 | `Sleeper` | trait | src/addon/feishu/domain/outbound.rs:296-300 | **move-as-is** | 纯可测性接口，无业务语义。 |
| 7 | `TokioSleeper` | type | src/addon/feishu/domain/outbound.rs:302-310 | **move-as-is** | 生产实现；宿主多处 Arc<TokioSleeper> 直接使用，类型路径经 shim 不变。 |
| 8 | `FailureKind` | type | src/addon/feishu/domain/outbound.rs:316-335 | **move-with-refactor** | 删除 UuidConflict（:328）与 InstanceNotFound（:332）——这两个变体的文档与语义都是审批实例专有。删后发码仍完整保留在 Fatal{code}，宿主按 CODE_UUID_CONFLICT/CODE_INSTANCE_NOT_FOUND 匹配即可；disposition 对二者本来也返回 Give，行为不变。 |
| 9 | `classify` | fn | src/addon/feishu/domain/outbound.rs:337-401 | **move-with-refactor** | 判定顺序原样保留：429 → 业务码 → 状态码兜底；token 族先于 401/403 的注释与逻辑必须一起搬。删 380-385 的 uuid/not-found 分支，未知码统一 Fatal{code}。原来直接引用的 5 张常量切片改为读 codes.rate_limited / token_expired / token_malformed / retryable。 |
| 10 | `business_code` | fn | src/addon/feishu/domain/outbound.rs:415-419 | **move-as-is** | 非 JSON / 缺 code / 非整数一律 None。纯解析，无业务语义。 |
| 11 | `rate_limit_reset_seconds` | fn | src/addon/feishu/domain/outbound.rs:403-413 | **move-as-is** | 先 x-ogw-ratelimit-reset 再 retry-after 的优先级是飞书网关契约（全篇无 Retry-After），必须随注释搬走。 |
| 12 | `fatal_hint` | fn | src/addon/feishu/domain/outbound.rs:421-455 | **stay-in-host** | 整段中文运维文案是产品资产（含 1390001/1390013/1390015 审批域），不能进 crate。宿主以 fn(i32)->Option<&'static str> 形式作为 OutboundDomain.hint 注入 send_with_retry 的 warn 日志；approval_dispatch.rs:1854 与 approval_feedback.rs:9,216 继续直接调用。 |
| 13 | `summarize` | fn | src/addon/feishu/domain/outbound.rs:457-470 | **move-as-is** | 500 字符按 char 截断（中文不能按字节切）的不变式随注释搬。 |
| 14 | `dedup_key` | fn | src/addon/feishu/domain/outbound.rs:472-512 | **move-as-is** | 只抹 "log_id":"…" 值、容忍空白；无业务语义。宿主唯一调用方 infrastructure/feishu_approval_worker.rs:898。 |
| 15 | `OutboundFailure` | type | src/addon/feishu/domain/outbound.rs:514-534 | **move-with-refactor** | 重构点：Display（:521-532）去掉 fatal_hint 拼接，只输出 `飞书出站调用失败: {message}`。理由：hint 表留宿主，且 70 处宿主结构体字面量不能被新增字段打破。宿主新增 describe(&OutboundFailure)->String 在展示层补 hint。 |
| 16 | `RetryPolicy` | type | src/addon/feishu/domain/outbound.rs:540-550,595-614 | **move-as-is** | 类型与两个纯函数整体搬走；策略实例（TOKEN_RETRY/PULL_RETRY/DEPARTMENT_RETRY/MESSAGE_RETRY）留宿主，见对应常量条目。 |
| 17 | `TOKEN_RETRY / TOKEN_REQUEST_TIMEOUT_SECS` | const | src/addon/feishu/domain/outbound.rs:552-566 | **stay-in-host** | 与 tenant_token 的锁 TTL 不变式绑定（3×5s+2×2s=19s < 30s），属于认证域策略，不在 L0 范围；将来 tenant_token 整体迁移到 crate 的 auth 模块时再一并搬。 |
| 18 | `PULL_RETRY / PULL_REQUEST_TIMEOUT_SECS / DEPARTMENT_RETRY / DEPARTMENT_REQUEST_TIMEOUT_SECS` | const | src/addon/feishu/domain/outbound.rs:568-593 | **stay-in-host** | 取值依据是审批编号回填的单轮预算与多维表分页体积，是产品域策略；宿主常量留在 shim。另 approval_notification.rs:131 的 MESSAGE_RETRY 同样留宿主。 |
| 19 | `Disposition` | type | src/addon/feishu/domain/outbound.rs:617-624 | **move-as-is** | 纯决策结果。 |
| 20 | `disposition` | fn | src/addon/feishu/domain/outbound.rs:626-656 | **move-with-refactor** | 删掉 UuidConflict/InstanceNotFound 两个 Give 分支（:644-645）；剩余语义原样：非幂等必 Give、attempt>=max_attempts 必 Give、服务端建议秒数优先且被 max_retry_after_ms 截断。 |
| 21 | `is_idempotent_outcome` | fn | src/addon/feishu/domain/outbound.rs:658-664 | **move-with-refactor** | 唯一判断的 1390007 是审批订阅幂等码，不能写进 crate；改为 codes.idempotent.contains(&code)，默认空表即旧行为之外的新宿主表登记 1390007。 |
| 22 | `send_with_retry` | fn | src/addon/feishu/domain/outbound.rs:666-781 | **move-with-refactor** | 重构四处：① 677-691 endpoint 别名表改调 (domain.operation)(url)；② 703 classify 传 domain.codes；③ 751 幂等结果查 domain.codes.idempotent；④ 770 hint 改调 (domain.hint)(code)。其余逐字保留：单次 attempt 记 debug、失败 warn 字段、Give 三分支日志、metrics feishu_outbound_request_total、BaseError::RateLimit… |
| 23 | `通用错误码常量（CODE_RATE_LIMITED / CODES_TOKEN_EXPIRED / CODES_TOKEN_MALFORMED）` | const | src/addon/feishu/domain/outbound.rs:42-43,51-54,56-66 | **move-as-is** | 飞书网关/认证的通用错误码，与产品域无关。放进 crate 的 transport/codes.rs。 |
| 24 | `域错误码常量（CODE_TOO_MANY_REQUEST / CODE_MESSAGE_RATE_LIMITED / CODE_TOO_LARGE_RESPONSE / CODES_RETRYABLE / CODES_PERMISSION_…` | const | src/addon/feishu/domain/outbound.rs:44-49,68-118 | **stay-in-host** | 按域拆分：多维表域（1254290/1254030/CODES_RETRYABLE/CODES_PERMISSION_DENIED）、审批域（1395001/60012/1390003/1390007）、IM 域（230020）留在宿主（建议新文件 domain/feishu_codes.rs，或在 shim 内保留以便 39 处 import 不变）。宿主把它们组装成 SYSTEM_CODES: CodeTable 注入。 |
| 25 | `outbound_setup::Outbound` | type | src/addon/feishu/domain/outbound_setup.rs:22-39 | **stay-in-host** | 骨架留宿主，只有 transport 字段类型从 Arc<HttpClientTransport> 换成 Arc<ApprovalCreateGate>；两个访问器返回 &dyn 不变，调用方零改动。 |
| 26 | `outbound_setup::build` | fn | src/addon/feishu/domain/outbound_setup.rs:47-84 | **stay-in-host** | 全部留宿主：ActionContext::tools() 取 http/cache、AuthorizationVersionCache 推 deployment、FeishuSettings 读 app_id/app_secret/approval_create_rate_per_minute、can_pull 前置条件、TenantTokenProvider 组装——这些依赖产品配置/扩展/Redis 命名空间。变成 crate 构造函数的只有 HttpClientTransport::new(HttpClien… |
| 27 | `outbound_setup::require_settings` | fn | src/addon/feishu/domain/outbound_setup.rs:86-106 | **stay-in-host** | 40901/40902 是产品错误码与中文文案，留宿主。 |
| 28 | `outbound_setup::outbound_error` | fn | src/addon/feishu/domain/outbound_setup.rs:108-114 | **stay-in-host** | 改用 describe(&failure)（Display + hint）拼文案，否则外置 fatal_hint 后 ParamInvalid 消息会丢运维线索。映射到 ParamInvalid("feishu") 的取向保留。 |
| 29 | `approval_rate_limit::{bucket_key, min_interval_ms, acquire_create_slot}` | fn | src/addon/feishu/domain/approval_rate_limit.rs:12-67 | **stay-in-host** | Redis 键前缀 yang-system、审批创建配额、官方 100/分上限全是产品域。新增同文件装饰器 ApprovalCreateGate 承接 outbound.rs:202-223 的嗅探，实现 OutboundTransport 后先 acquire 再 inner.send；失败映射 BaseError::RateLimitExceeded{retry_after_seconds:1} 与原实现逐字一致，send_with_retry 仍按 Retry 处理。 |

### 需先重构

- **删除 HttpClientTransport.approval_create_limit 字段与方法 with_approval_create_limit，把 202-223 的 POST + "/open-apis/approval/v4/instances" 嗅探搬进宿主新装饰器 ApprovalCreateGate（approval_rate_limit.rs），由 outbound_set…** — crate 不应认识审批创建端点，也不该为它引入 yang_db::RedisClient 依赖；装饰器保持「每次真实出站（含每次重试）都领名额」的时序不变（`src/addon/feishu/domain/outbound.rs:175-223, src/addon/feishu/domain/outbound_setup.rs:62-68, src/infrastructure/feishu_approval_worker.rs:242-248`）
- **send_with_retry 的 endpoint 别名表（677-691）整表搬到宿主 fn operation(url)->&'static str，作为 OutboundDomain.operation 注入；crate 内不再出现 approval/bitable 路径字面量** — 别名只用于 tracing，控制流无关；搬表比给 OutboundRequest 新增 operation 字段更省（后者要改 17 处结构体字面量）（`src/addon/feishu/domain/outbound.rs:676-691, 696, 705, 736, 754, 760, 768`）
- **错误码常量按域拆成两处：crate 的 transport/codes.rs 只留 CODE_RATE_LIMITED、CODES_TOKEN_EXPIRED、CODES_TOKEN_MALFORMED；宿主保留多维表/审批/IM 域常量，并组装 SYSTEM_CODES: CodeTable** — 分类算法与域码表解耦，crate 不携带审批/多维表语义；宿主仍可一处登记新码（`src/addon/feishu/domain/outbound.rs:38-118`）
- **classify 增加 codes: &CodeTable 参数，五张常量切片改读表；删 380-385 的 uuid 冲突/实例不存在分支** — 码表外置后 classify 才能留在 crate；这两个分支是审批实例语义，删除后未知码统一落 Fatal{code}，码值不丢（`src/addon/feishu/domain/outbound.rs:345-401`）
- **FailureKind 删除 UuidConflict、InstanceNotFound；宿主 12 处匹配改为 FailureKind::Fatal { code } if code == CODE_UUID_CONFLICT / CODE_INSTANCE_NOT_FOUND（approval.rs:171,363；approval_dispatch.rs:966-967,1784-1787,…** — 两个变体的文档与名字都在描述审批实例；disposition 对它们本来就是 Give，等价于 Fatal（`src/addon/feishu/domain/outbound.rs:316-335`）
- **fatal_hint 整段留宿主，作为 OutboundDomain.hint 注入 send_with_retry 的 warn 字段；OutboundFailure::Display 去掉 hint 拼接，宿主新增 describe() 在 outbound_error / approval_feedback / dispatch 展示层补文案** — 文案是产品运维资产且含审批码；crate 的 Display 无上下文可取 hint，又不能给 OutboundFailure 加字段（70 处宿主字面量）（`src/addon/feishu/domain/outbound.rs:421-455, 521-532, 770`）
- **is_idempotent_outcome 改为 codes.idempotent.contains(&code)，CODE_SUBSCRIPTION_EXISTED(1390007) 由宿主表登记** — 订阅幂等是审批订阅接口专属语义（`src/addon/feishu/domain/outbound.rs:658-664, 751`）
- **send_with_retry 增加第 5 个参数 domain: &OutboundDomain；8 处调用点（approval.rs:654,778；approval_attachment.rs:158；approval_notification.rs:372；bitable.rs:875；tenant_token.rs:400；user_token.rs:370,455）统一传 &SYSTE…** — 一个注入点同时解决码表、运维文案、日志别名三件事；参数必填能防漏传导致日志静默退化（`src/addon/feishu/domain/outbound.rs:670-781`）
- **outbound_setup::build 拆成「crate 构造 + 宿主装配」：HttpClientTransport::new(HttpClient) 进 crate；deployment/ActionContext/FeishuSettings/RedisTenantTokenCache/TenantTokenProvider 全留宿主；transport 字段改为 Arc<Approva…** — crate 不依赖 ActionContext/配置扩展/部署命名空间；Outbound::transport() 返回 &dyn 使调用方零改动（`src/addon/feishu/domain/outbound_setup.rs:22-84`）
- **宿主 outbound.rs 保留为薄 shim：pub(crate) use yang_feishu::transport::{...} + 宿主常量与 SYSTEM_CODES/SYSTEM_DOMAIN/fatal_hint/operation，保住 39 处 `use super::outbound::…` 路径** — 一次性迁移成本最低；避免 55 个调用方/17 个实现者同时改 import（`src/addon/feishu/domain/outbound.rs:1-781（替换）`）
- **删除 outbound.rs:30 的 #![allow(dead_code)]，crate 内按需收紧 lint；宿主 shim 侧保留自己的 allow** — 原许可写的是「消费者在后续批次接入」，抽取后 crate 已有真实消费者（`src/addon/feishu/domain/outbound.rs:30`）
- **把 yang-feishu 纳入 CI 契约：scripts/run_ci.py:24-31 的 AUXILIARY_PACKAGE_FLAGS 增加 -p yang-feishu，补 doc-test 命令；.github/workflows/ci.yml feature-matrix 增加组合；scripts/verify_ci_contract.py 同步期望** — run_ci/CI 是显式 crate 列表，不是 workspace 自动发现；漏登记等于新 crate 不过门禁（`scripts/run_ci.py:21-31,88-90；.github/workflows/ci.yml；scripts/verify_ci_contract.py`）
- **crate 依赖收敛为 yang-base(default-features=false, features=["http"]) + async-trait + serde_json + tokio(time) + tracing + metrics；不引 yang-db** — HttpClient/BaseError 来自 yang-base；审批闸门搬走后不再需要 Redis（`新文件 crates/yang-feishu/Cargo.toml`）

### 目标公开 API

| 名称 | 签名 | 备注 |
|---|---|---|
| `transport::OutboundMethod` | `pub enum OutboundMethod { Get, Post, Put }` | transparent 映射到 HttpClient 的 get/post/put。 |
| `transport::OutboundRequest` | `pub struct OutboundRequest { pub method: OutboundMethod, pub url: String, pub query: Vec<(String, String)>, pub bearer_token: Option<String>, pub json_body: Option<serde_json::Value>, pub raw_body: Option<(String, Vec<u8>)>, pub binary_limit: Option<usize>, pub timeout_secs: Option<u64>, pub idempotent: bool }` | 字段全 pub；无 operation 字段（日志别名走 OutboundDomain）。 |
| `transport::OutboundResponse` | `pub struct OutboundResponse { pub status: u16, pub body: String, pub bytes: Option<Vec<u8>>, pub headers: BTreeMap<String, String> }` | headers 键恒小写。 |
| `transport::OutboundTransport` | `#[async_trait::async_trait] pub trait OutboundTransport: Send + Sync { async fn send(&self, request: OutboundRequest) -> Result<OutboundResponse, BaseError>; }` | 实现方不得内含重试；唯一重试入口是 send_with_retry。 |
| `transport::HttpClientTransport` | `pub struct HttpClientTransport { client: HttpClient } impl HttpClientTransport { pub fn new(client: HttpClient) -> Self } impl OutboundTransport for HttpClientTransport` | 不再有审批闸门构造；宿主用装饰器叠加。 |
| `transport::Sleeper / transport::TokioSleeper` | `#[async_trait] pub trait Sleeper: Send + Sync { async fn sleep(&self, duration: Duration); } pub struct TokioSleeper;` | 测试替身实现记录等待时长即可。 |
| `transport::FailureKind` | `pub enum FailureKind { Retry { retry_after_seconds: Option<u64> }, TokenExpired, Fatal { code: i32 } }` | 无审批语义；宿主用 Fatal{code} 匹配自己的域码。 |
| `transport::CodeTable` | `pub struct CodeTable { pub rate_limited: &'static [i32], pub token_expired: &'static [i32], pub token_malformed: &'static [i32], pub retryable: &'static [i32], pub idempotent: &'static [i32] } impl CodeTable { pub const EMPTY: Self; }` | 纯数据表，宿主 const 组合各域切片；EMPTY 供 crate 单测与 neutral 场景。 |
| `transport::classify` | `pub fn classify(status: u16, headers: &BTreeMap<String, String>, body: &str, codes: &CodeTable) -> Option<FailureKind>` | 业务码优先于 HTTP 状态；token 族先于 401/403；429 无条件 Retry。 |
| `transport::business_code / rate_limit_reset_seconds / summarize / dedup_key` | `pub fn business_code(body: &str) -> Option<i32>; pub fn rate_limit_reset_seconds(headers: &BTreeMap<String, String>) -> Option<u64>; pub fn summarize(status: u16, body: &str) -> String; pub fn dedup_key(message: &str) -> String` | 四个无状态工具函数，供宿主日志与去重直接复用。 |
| `transport::OutboundFailure` | `pub struct OutboundFailure { pub kind: FailureKind, pub message: String } impl Display for OutboundFailure（仅 message）; impl std::error::Error` | 展示层 hint 由宿主 describe() 补。 |
| `transport::RetryPolicy` | `pub struct RetryPolicy { pub max_attempts: u64, pub base_backoff_ms: u64, pub max_backoff_ms: u64, pub max_retry_after_ms: u64 } impl RetryPolicy { pub fn worst_case_seconds(&self, request_timeout_secs: u64) -> u64; pub fn backoff_ms(&self, attempt: u64, retry_after_seconds: Option<u64>) -> u64 }` | 策略实例全部宿主定义。 |
| `transport::Disposition / transport::disposition` | `pub enum Disposition { RetryAfter(u64), Give } pub fn disposition(kind: FailureKind, attempt: u64, idempotent: bool, policy: RetryPolicy) -> Disposition` | 非幂等或超预算必 Give；服务端建议等待优先并被上限截断。 |
| `transport::OutboundDomain` | `pub struct OutboundDomain { pub codes: &'static CodeTable, pub operation: fn(&str) -> &'static str, pub hint: fn(i32) -> Option<&'static str> } impl OutboundDomain { pub const fn neutral() -> Self }` | 宿主业务知识的唯一注入点（码表 + 日志别名 + 运维文案）。neutral() 用空表、"other"、无 hint。 |
| `transport::send_with_retry` | `pub async fn send_with_retry(transport: &dyn OutboundTransport, sleeper: &dyn Sleeper, request: &OutboundRequest, policy: RetryPolicy, domain: &OutboundDomain) -> Result<OutboundResponse, OutboundFailure>` | 唯一重试入口；成功返回原响应（含 bytes），失败返回最后一次 kind+message。 |
| `transport::codes（通用码）` | `pub const CODE_RATE_LIMITED: i32 = 99991400; pub const CODES_TOKEN_EXPIRED: &[i32]; pub const CODES_TOKEN_MALFORMED: &[i32]` | 飞书网关与认证通用；多维表/审批/IM 码不进 crate。 |

### 测试处置（38）

| 文件 | 区间 | 内容 | 处置 |
|---|---|---|---|
| src/addon/feishu/domain/outbound.rs | 804-916 | attachment_http_transport_preserves_bytes_multipart_and_json_errors：本地 TCP 服务验证 binary/raw_body/JSON 错误体保留 | **move** |
| src/addon/feishu/domain/outbound.rs | 918-960 | http_transport_sends_put_outside_the_approval_create_gate：PUT 真以 PUT 发出 + 不受创建闸门约束 | **rewrite** |
| src/addon/feishu/domain/outbound.rs | 964-982 | success_requires_zero_business_code / business_errors_on_http_200_are_not_success | **move** |
| src/addon/feishu/domain/outbound.rs | 984-989 | subscription_existed_is_an_expected_idempotent_outcome（1390007） | **rewrite** |
| src/addon/feishu/domain/outbound.rs | 991-1011 | dedup_key_ignores_volatile_log_id | **move** |
| src/addon/feishu/domain/outbound.rs | 1013-1026 | too_large_response_is_fatal_not_retryable（1254030 属多维表域） | **rewrite** |
| src/addon/feishu/domain/outbound.rs | 1028-1041 | bitable_native_rate_limit_is_retryable（1254290） | **stay** |
| src/addon/feishu/domain/outbound.rs | 1043-1056 | http_429_is_retryable_and_reads_the_reset_header | **move** |
| src/addon/feishu/domain/outbound.rs | 1058-1083 | rate_limit_reset_header_is_seconds_not_retry_after（官方头优先 + 非数字不 panic） | **move** |
| src/addon/feishu/domain/outbound.rs | 1085-1093 | legacy_rate_limit_is_http_400_with_business_code（99991400） | **move** |
| src/addon/feishu/domain/outbound.rs | 1095-1105 | vendor_advised_retry_codes_are_retryable_even_on_http_400（CODES_RETRYABLE 多维表域） | **stay** |
| src/addon/feishu/domain/outbound.rs | 1107-1118 | token_expiry_is_detected_regardless_of_http_status（token 族先于 401/403） | **move** |
| src/addon/feishu/domain/outbound.rs | 1120-1130 | malformed_token_codes_do_not_trigger_a_refresh | **move** |
| src/addon/feishu/domain/outbound.rs | 1132-1146 | no_business_code_falls_back_to_http_status | **move** |
| src/addon/feishu/domain/outbound.rs | 1148-1155 | truncated_json_is_treated_as_no_business_code | **move** |
| src/addon/feishu/domain/outbound.rs | 1157-1173 | fatal_hints_do_not_confuse_token_not_found_with_permission_denied | **stay** |
| src/addon/feishu/domain/outbound.rs | 1175-1183 | summarize_truncates_on_char_boundaries | **move** |
| src/addon/feishu/domain/outbound.rs | 1187-1216 | backoff_grows_exponentially_then_clamps / server_suggested_wait_wins_and_is_capped | **move** |
| src/addon/feishu/domain/outbound.rs | 1218-1232 | disposition_gives_up_on_terminal_failures（去掉已删变体引用即可） | **rewrite** |
| src/addon/feishu/domain/outbound.rs | 1234-1258 | disposition_respects_the_attempt_budget | **move** |
| src/addon/feishu/domain/outbound.rs | 1260-1266 | non_idempotent_requests_are_never_retried | **move** |
| src/addon/feishu/domain/outbound.rs | 1268-1280 | token_refresh_lock_ttl_outlives_the_worst_case_critical_section（引用 tenant_token::LOCK_TTL_SECONDS） | **stay** |
| src/addon/feishu/domain/outbound.rs | 1284-1372 | ScriptedTransport / RecordingSleeper / response() / get_request() 测试替身 | **move** |
| src/addon/feishu/domain/outbound.rs | 1374-1390 | unconfigured_http_transport_rejects_approval_create_before_connecting | **stay** |
| src/addon/feishu/domain/outbound.rs | 1392-1404 | local_create_gate_exhaustion_remains_retryable_with_delay（BaseError::RateLimitExceeded → Retry） | **move** |
| src/addon/feishu/domain/outbound.rs | 1406-1473 | real_redis_http_create_limit_covers_retries_and_independent_transports（#[ignore] 真 Redis） | **stay** |
| src/addon/feishu/domain/outbound.rs | 1475-1488 | retries_within_budget_then_succeeds | **move** |
| src/addon/feishu/domain/outbound.rs | 1490-1516 | exhausts_the_attempt_budget_and_reports_the_last_failure | **move** |
| src/addon/feishu/domain/outbound.rs | 1518-1534 | token_expiry_is_not_retried_in_the_backoff_loop | **move** |
| src/addon/feishu/domain/outbound.rs | 1536-1549 | framework_transport_errors_are_retried_when_they_are_transient | **move** |
| src/addon/feishu/domain/outbound.rs | 1551-1569 | non_retryable_transport_errors_fail_immediately | **move** |
| src/addon/feishu/domain/outbound.rs | 1571-1584 | server_suggested_wait_is_used_verbatim | **move** |
| src/addon/feishu/domain/outbound.rs | 1590-1634 | approval_domain_codes_are_classified_as_designed（1395001/60012/1390003/1390001/1390013/1390015） | **stay** |
| src/addon/feishu/domain/outbound.rs | 1641-1650 | idempotent_requests_do_retry_on_retryable_failure | **move** |
| src/addon/feishu/domain/outbound.rs | 1653-1663 | new_kinds_are_given_not_retried（UuidConflict/InstanceNotFound） | **rewrite** |
| src/addon/feishu/domain/outbound_setup.rs | 116-139 | outbound_failures_become_readable_param_errors（ParamInvalid 文案） | **stay** |
| src/addon/feishu/domain/approval_rate_limit.rs | 72-119 | bucket_key/interval 的纯函数单测（命名空间、向上取整、零速率、官方上限） | **stay** |
| src/addon/feishu/domain/approval_rate_limit.rs | 121-205 | 两个 #[ignore] 真 Redis 创建闸门测试（并发间隔、有界等待、不预订未来时段） | **stay** |

### 宿主替换点（15）

- `src/addon/feishu/domain/outbound.rs:1-781` → 整文件降为宿主 shim：`pub(crate) use yang_feishu::transport::{OutboundMethod, OutboundRequest, OutboundResponse, OutboundTransport, HttpClientTransport, Sleeper, TokioSleeper, FailureKind, OutboundFailure, RetryPolicy, Disposition, CodeTable, OutboundDomain, classify, disposition, send_with_retry, business_code, rate_limit_reset_seconds, summarize, dedup_key};` 再定义宿主常量（TOKEN_*/PULL_*/DEPARTMENT_*/MESSAGE_…
- `src/addon/feishu/domain/outbound.rs:202-223` → 移入 approval_rate_limit.rs 的 `impl OutboundTransport for ApprovalCreateGate { async fn send(&self, request) -> Result<OutboundResponse, BaseError> { if request.method == Post && request.url.split('?').next().unwrap_or_default().ends_with("/open-apis/approval/v4/instances") { acquire_create_slot(&self.redis, &self.app_id, self.rate).await.map_err(\|failure\| { tracing::warn!(...); BaseError::RateLim…
- `src/addon/feishu/domain/outbound.rs:677-691` → 原样搬到宿主 `pub(crate) fn operation(url: &str) -> &'static str { match url.split('?').next().unwrap_or_default() { url if url.contains("/im/v1/messages/") => "im.messages.edit", url if url.contains("/approval/v4/instances") => "approval.instances", … _ => "other" } }`，作为 `OutboundDomain.operation`。crate 里不再出现 approval/bitable 字面量。
- `src/addon/feishu/domain/outbound.rs:421-455` → 整段搬到宿主（建议 domain/failure_hints.rs，或先留在 shim），签名与文案一字不改；作为 `OutboundDomain.hint`。
- `src/addon/feishu/domain/outbound.rs:521-532` → crate 侧 Display 只写 `飞书出站调用失败: {message}`；宿主新增 `pub(crate) fn describe(failure: &OutboundFailure) -> String { match fatal_hint(code) { Some(h) => format!("{failure}；{h}"), None => failure.to_string() } }`，在 outbound_error/approval_feedback/dispatch 展示处调用。
- `src/addon/feishu/domain/outbound.rs:345-401` → classify 签名尾部加 `codes: &CodeTable`；五处 `CODES_*`/`CODE_*` 比较改读 `codes.rate_limited`/`codes.token_expired`/`codes.token_malformed`/`codes.retryable`；删除 380-385 两个分支，落到 `FailureKind::Fatal { code }`。
- `src/addon/feishu/domain/outbound.rs:670-675` → 签名加 `domain: &OutboundDomain`；677 用 `(domain.operation)(&request.url)`，703 用 `classify(..., domain.codes)`，751 用 `domain.codes.idempotent.contains(&code)`，770 用 `(domain.hint)(code)`。
- `src/addon/feishu/domain/outbound.rs:316-335` → 删掉 `UuidConflict`、`InstanceNotFound` 两个变体；宿主对应分支改为 `FailureKind::Fatal { code } if code == CODE_UUID_CONFLICT` / `== CODE_INSTANCE_NOT_FOUND`（approval.rs:171,363；approval_dispatch.rs:966-967,1784-1787,3786-3787,5074-5075；approval_feedback.rs:220-221,409-410）。
- `src/addon/feishu/domain/outbound_setup.rs:62-68` → let transport = Arc::new(ApprovalCreateGate::new(HttpClientTransport::new(ctx.tools().http()?.clone()), ctx.tools().cache()?.clone(), &app_id, settings.approval_create_rate_per_minute));  `Outbound` 的 transport 字段类型改为 Arc<ApprovalCreateGate>，两个访问器签名不变。
- `src/addon/feishu/domain/outbound_setup.rs:108-114` → `BaseError::ParamInvalid("feishu".to_string(), describe(&failure))`，避免外置 hint 后 ParamInvalid 文案丢排查线索。
- `src/infrastructure/feishu_approval_worker.rs:242-248` → 同样改成 `Arc::new(ApprovalCreateGate::new(HttpClientTransport::new(tools.http()?.clone()), tools.cache()?.clone(), settings.app_id.as_deref().unwrap_or_default(), settings.approval_create_rate_per_minute))`；RoundRunner.transport 仍是 Arc<dyn OutboundTransport>，无需改字段。
- `src/addon/feishu/domain/approval.rs:654,778; src/addon/feishu/domain/approval_attachment.rs:158; src/addon/feishu/domain/approval_notification.rs:372; src/addon/feishu/domain/bitable.rs:875; src/addon/feishu/domain/tenant_token.rs:400; src/addon/feishu/domain/user_token.rs:370,455` → 每个 send_with_retry(...) 调用尾部追加 `, &SYSTEM_DOMAIN`（宿主 outbound shim 导出）。漏传会编译失败，不会静默退化。
- `src/infrastructure/feishu_event_worker.rs:1518-1536` → 测试里直接调 classify 的两处补 `, &SYSTEM_CODES`；其余 event worker 构造（:63）只用 HttpClientTransport::new，无闸门，经 shim 路径不变。
- `scripts/run_ci.py:21-31,88-90 与 .github/workflows/ci.yml 与 scripts/verify_ci_contract.py` → AUXILIARY_PACKAGE_FLAGS 增加 `-p yang-feishu`（或单独一条 `cargo test --lib -p yang-feishu --locked`），clippy -p 列表加 yang-feishu，补 doctest 命令；feature-matrix 增加无 feature / --all-features 两个组合；verify_ci_contract 的期望命令表同步。
- `Cargo.toml（新 crates/yang-feishu/Cargo.toml）` → [package] name = "yang-feishu" version = "0.1.0" edition.workspace = true rust-version.workspace = true [dependencies] async-trait.workspace, serde_json.workspace, tokio.workspace, tracing.workspace, metrics.workspace, yang-base = { path = "../yang-base", version = "0.3.0", default-features = false, features = ["http"] }。workspace members=["crates/*"] 自动收编，根 Cargo.toml 无需改。

### 风险

- FailureKind 删两个变体会让 ~12 处 match 变得非穷尽而编译失败（approval.rs:171/363、approval_dispatch.rs:966-967/1784-1787/3786-3787/5074-5075、approval_feedback.rs:220-221/409-410）；必须同一提交内改成 Fatal{code} 守卫，且 approval_feedback 的中文文案映射不能丢。
- send_with_retry 增参会改 8 处调用（approval.rs:654/778、approval_attachment.rs:158、approval_notification.rs:372、bitable.rs:875、tenant_token.rs:400、user_token.rs:370/455）；tenant_token/user_token 属其他抽取批次，若并行推进需先约好 SYSTEM_DOMAIN 的归属。
- classify 增参会改 feishu_event_worker.rs:1518/1536 两处直接调用与十余条 classify 单测；单测里使用的多维表码表需迁到宿主表，否则 1254290/1254607 会从 Retry 退化为 Fatal，行为回归。
- ApprovalCreateGate 装饰器必须保持「每次真实出站（含每次重试）都领名额」与失败映射 BaseError::RateLimitExceeded{retry_after_seconds:1} 两点；若误把 acquire 放在 send_with_retry 外层，重试将不领名额，直接打爆 100/分 官方上限。
- OutboundFailure::Display 去掉 hint 后，所有未经 describe() 的展示路径（anyhow!("{failure}")、日志 %failure、event worker 的 outbound_failure_outcome）会丢掉运维文案；必须逐一核对 tenant_token.rs、approval_attachment.rs、approval_dispatch.rs、feishu_event_worker.rs 的失败呈现。
- 宿主 shim 保留 `pub(crate) mod outbound;` 是降低 39 处 import 改动的关键；若直接删文件改模块路径，会牵动 55 个 OutboundRequest 构造方与 17 个 OutboundTransport 实现者。
- 新 crate 进 workspace 后 run_ci.py / ci.yml / verify_ci_contract.py 是显式 crate 列表，不登记等于新代码不过 fmt/test/clippy/MSRV；feature isolation 脚本也可能因 yang-feishu 的 yang-base http feature 需要新规则。
- metrics 依赖取舍：yang-base 把 metrics 做成可选 feature；yang-feishu 若强制依赖 metrics 会使 feature 矩阵多一组，若做成 feature 则宿主必须显式开启，否则 feishu_outbound_request_total 静默消失。建议 crate 内 `metrics` 为默认开启的 feature，并在宿主显式 enable。
- crate 公开面扩大：OutboundRequest/OutboundResponse/OutboundFailure 字段从 pub(crate) 变 pub，宿主 70 处 OutboundFailure 字面量与 17 处 OutboundRequest 字面量继续可用，但也意味着这些类型从此是 semver 契约；建议 crate 标注 `#![warn(missing_docs)]` 并同步补文档。
- MSRV 1.80：所用 `is_some_and`(1.70)、`div_ceil`(1.73)、let-else(1.65) 均满足；但 `OutboundDomain::neutral()` 里闭包转 fn 指针需要改为具名 free fn（const 上下文更稳），否则可能触发 const-eval 限制。
- 端点别名搬到宿主后，crate 的 tracing 里 approval/bitable 调用会统一显示 `other` 直到宿主注入 SYSTEM_DOMAIN；若某个入口（如 feishu_pull.rs:338 直接 new transport）漏注入，排障粒度会退化——这也是 domain 必填参数的价值，但测试替身要显式选 neutral 还是 SYSTEM。

## A.2 L0 认证与令牌（tenant token + 用户 OAuth 协议 + 密码学原语）→ crates/yang-feishu 的 auth / crypto 模块

**目标**：把 tenant_access_token 换取/缓存、飞书 user OAuth 协议、SHA-256/常数时间比较、飞书 AES-256-CBC 与凭据封存抽到新 crate；用户 token 的单行表存储、OAuth 控制台 Action、提醒 worker、配置结构与产品缓存键留在宿主。目标 crate 不依赖数据库（Redis 实现留宿主），协议层不 import crate::config / crate::authorization / yang_base::table。

### 源文件

| 文件 | 生产行区间 | 生产行数 | 测试行区间 | 测试行数 |
|---|---|---:|---|---:|
| src/addon/feishu/domain/tenant_token.rs | 1-429 | 429 | 431-931 | 501 |
| src/addon/feishu/domain/user_token.rs | 1-761 | 761 | 763-1300 | 538 |
| src/addon/feishu/domain/crypto.rs | 1-234 | 234 | 236-509 | 274 |
| src/addon/feishu/domain/token.rs | 1-56 | 56 | 58-122 | 65 |
| src/addon/feishu/approval/domain/user_token_table.rs | 1-72 | 72 | 74-171 | 98 |
| src/addon/feishu/approval/actions/oauth_authorize_url.rs | 1-81 | 81 | 83-101 | 19 |
| src/addon/feishu/approval/actions/oauth_callback.rs | 1-240 | 240 | 242-280 | 39 |
| src/addon/feishu/approval/actions/oauth_status.rs | 1-80 | 80 | 82-99 | 18 |
| src/infrastructure/feishu_oauth_reminder.rs | 1-299 | 299 | 301-309 | 9 |
| src/config/mod.rs | 583-699, 750-786 | 154 |  |  |

### 符号（82）

| # | 符号 | 类型 | 位置 | 处置 | 备注 |
|---:|---|---|---|---|---|
| 1 | `FEISHU_OPEN_BASE` | const | src/addon/feishu/domain/tenant_token.rs:40 | **move-as-is** | 三处引用：tenant_token.rs:383、user_token.rs:44/446、approval.rs:37。放 crate 根 lib.rs；移动时删掉本文件第 25 行的 #![allow(dead_code)]。 |
| 2 | `TENANT_TOKEN_PATH` | const | src/addon/feishu/domain/tenant_token.rs:43 | **move-as-is** | 只被 token_request(:383) 用，保持私有。 |
| 3 | `LOCK_TTL_SECONDS` | const | src/addon/feishu/domain/tenant_token.rs:50 | **move-as-is** | 必须 pub：outbound.rs:1269-1280 的不变式测试断言 30 > worst_case_seconds(19)，该测试搬进 crate 后要改成 crate 内路径。 |
| 4 | `CACHE_TTL_MARGIN_SECONDS` | const | src/addon/feishu/domain/tenant_token.rs:53 | **move-as-is** | 官方「剩余 ≥30 分钟返回原 token」的安全侧取值。 |
| 5 | `LOCK_WAIT_ATTEMPTS / LOCK_WAIT_INTERVAL` | const | src/addon/feishu/domain/tenant_token.rs:56-57 | **move-as-is** | 3 轮 × 200ms，保持私有。 |
| 6 | `FeishuCredentials` | type | src/addon/feishu/domain/tenant_token.rs:67-81 | **move-as-is** | 手写 Debug（73-81）必须原样保留，secret 打码是安全不变量；字段转 pub 供宿主构造。这是「单应用假设」的载体：类型本身无害，问题是宿主只构造一份。 |
| 7 | `TenantTokenResponse` | type | src/addon/feishu/domain/tenant_token.rs:86-95 | **move-as-is** | 扁平信封，刻意不套 data。 |
| 8 | `TenantTokenGrant` | type | src/addon/feishu/domain/tenant_token.rs:98-102 | **move-as-is** | 字段 pub(crate) → pub。 |
| 9 | `parse_token_grant` | fn | src/addon/feishu/domain/tenant_token.rs:109-128 | **move-as-is** | 「业务码非 0」与「code=0 缺字段」分开报，是排查契约，勿合并。 |
| 10 | `cache_ttl_seconds` | fn | src/addon/feishu/domain/tenant_token.rs:135-141 | **move-as-is** | 下界自守，防止 SETEX 负数。 |
| 11 | `token_key` | fn | src/addon/feishu/domain/tenant_token.rs:144-146 | **stay-in-host** | 产品前缀 yang-system:{deployment}:feishu 不能进 crate。宿主保留并加 app_id 修单应用：format!("yang-system:{deployment}:feishu:app:{app_id}:tenant_token")。 |
| 12 | `lock_key` | fn | src/addon/feishu/domain/tenant_token.rs:149-151 | **move-with-refactor** | 并入宿主 RedisTenantTokenCache::new：lock_key = format!("{token_key}:lock")，不再是独立函数。 |
| 13 | `RELEASE_LOCK_SCRIPT` | const | src/addon/feishu/domain/tenant_token.rs:158-163 | **stay-in-host** | Redis compare-and-delete Lua，属于宿主缓存实现，随 RedisTenantTokenCache 一起留宿主。 |
| 14 | `TenantTokenCache` | trait | src/addon/feishu/domain/tenant_token.rs:173-181 | **move-as-is** | 5 个方法都不含产品语义，键的作用域由实现方的构造参数决定，crate 不需要知道 deployment/app_id。 |
| 15 | `RedisTenantTokenCache` | type | src/addon/feishu/domain/tenant_token.rs:184-198 | **stay-in-host** | 依赖 yang-db（sqlx 整棵树），搬进 crate 会把协议库拖成数据库库。宿主新文件 domain/tenant_token_cache.rs；构造签名改为 new(redis: RedisClient, token_key: impl Into<String>)，内部派生 lock_key。 |
| 16 | `impl TenantTokenCache for RedisTenantTokenCache` | type | src/addon/feishu/domain/tenant_token.rs:200-251 | **stay-in-host** | get:202 / put:209 / invalidate:218 / acquire_lock:227 / release_lock:238，逐行原样随实现搬。 |
| 17 | `TenantTokenProvider` | type | src/addon/feishu/domain/tenant_token.rs:258-264 | **move-with-refactor** | deployment 字段改名 namespace，只用于 3 处 tracing 日志（305/329/368），不再参与缓存键与校验。按 app_id 一实例，天然支持多应用（宿主建 N 个 provider 即可）。 |
| 18 | `TenantTokenProvider::new` | fn | src/addon/feishu/domain/tenant_token.rs:266-284 | **move-with-refactor** | 关键重构：删除 :276 的 validate_deployment_name（宿主 config/mod.rs:1572 的 AuthorizationSettings::validate 已在启动期校验同名值，version_cache.rs:67 再校验一次，此处纯冗余），失败分支消失 → 返回 Self 不再 Result。所有调用点的 .map_err/.unwrap_or_else 一并删。 |
| 19 | `TenantTokenProvider::tenant_access_token` | fn | src/addon/feishu/domain/tenant_token.rs:290-345 | **move-as-is** | 快路径读缓存→抢锁→二次读→换取→放锁；owner 每次新生成 (:297)。逻辑零改动。 |
| 20 | `TenantTokenProvider::invalidate_and_refresh` | fn | src/addon/feishu/domain/tenant_token.rs:351-358 | **move-as-is** | 调用点：approval.rs:661、approval_attachment.rs:162。 |
| 21 | `read_cached / token_request / issue_and_cache / release_lock` | fn | src/addon/feishu/domain/tenant_token.rs:360-428 | **move-as-is** | 360/380/399/423 四个私有方法整体随 provider 搬；token_request 的 bearer_token: None 与 idempotent: true 是协议契约。 |
| 22 | `AUTHORIZE_URL` | const | src/addon/feishu/domain/user_token.rs:48 | **move-as-is** | 与 TOKEN_URL 同为飞书端点常量。 |
| 23 | `TOKEN_URL` | const | src/addon/feishu/domain/user_token.rs:53 | **move-as-is** | 换 code 与刷新共用同一 URL，只有 grant_type 不同。 |
| 24 | `OAUTH_SCOPES` | const | src/addon/feishu/domain/user_token.rs:69-70 | **stay-in-host** | scope 是「本产品要什么权限」的决策（contact:user:search 是 2026-10-13 真机 99991679 点名补的），不是飞书协议常量。留在宿主，由调用方作为参数传。 |
| 25 | `STATE_TTL_SECONDS` | const | src/addon/feishu/domain/user_token.rs:73 | **move-as-is** | 随 InMemoryOAuthStateStore 的默认 TTL 进 crate。 |
| 26 | `ACCESS_TOKEN_REFRESH_MARGIN_SECONDS` | const | src/addon/feishu/domain/user_token.rs:76 | **move-as-is** | 被 access_token_is_fresh 使用。 |
| 27 | `CODE_REFRESH_TOKEN_EXPIRED/INVALID/REVOKED/USED` | const | src/addon/feishu/domain/user_token.rs:79-85 | **move-as-is** | 20037/20026/20064/20073，needs_reauth 的判据。 |
| 28 | `now_unix_secs` | fn | src/addon/feishu/domain/user_token.rs:88-93 | **move-as-is** | 宿主 4 处调用（oauth_authorize_url:74、oauth_callback:80、oauth_status:63、reminder:164）。 |
| 29 | `REAUTH_REMINDER_DAYS` | const | src/addon/feishu/domain/user_token.rs:100 | **move-as-is** | 飞书 365 天硬红线的提前量，属协议侧策略。 |
| 30 | `reauth_reminder_due` | fn | src/addon/feishu/domain/user_token.rs:103-105 | **move-as-is** | 状态页与提醒 worker 共用同一判据，必须在 crate 里只留一份。 |
| 31 | `OAuthConfig` | type | src/addon/feishu/domain/user_token.rs:108-111 | **move-as-is** | 字段 pub(crate) → pub（oauth_authorize_url.rs:76 从外部模块读）。 |
| 32 | `oauth_config` | fn | src/addon/feishu/domain/user_token.rs:117-139 | **stay-in-host** | 唯一读 [feishu] 配置段、给出中文可行动文案的函数；crate 不 import crate::config。 |
| 33 | `percent_encode` | fn | src/addon/feishu/domain/user_token.rs:150-161 | **move-as-is** | RFC 3986 unreserved 集合，空格编 %20 不用 +，与飞书文档示例一致。 |
| 34 | `build_authorize_url` | fn | src/addon/feishu/domain/user_token.rs:164-172 | **move-with-refactor** | 新增第 3 参 scope（去掉对 OAUTH_SCOPES 的依赖）。proto 仍是 client_id/response_type/redirect_uri/scope/state/prompt=consent 六参数。 |
| 35 | `OAUTH_STATES` | const | src/addon/feishu/domain/user_token.rs:175-176 | **move-with-refactor** | 进程级 static LazyLock<Mutex<HashMap>> → 变成 InMemoryOAuthStateStore 的私有字段；crate 不提供 lib 级全局单例（会跨测试污染）。 |
| 36 | `with_states` | fn | src/addon/feishu/domain/user_token.rs:180-185 | **move-with-refactor** | 锁中毒取回内部值的写法保留，变成 InMemoryOAuthStateStore 的私有 helper。 |
| 37 | `mint_state` | fn | src/addon/feishu/domain/user_token.rs:188-197 | **move-with-refactor** | 改成 trait 方法。trait 取同步（唯一实现是进程内存；设计文档明确「state 不落库、不引 Redis」），宿主薄壳保持 sync 调用点不变。 |
| 38 | `consume_state` | fn | src/addon/feishu/domain/user_token.rs:203-208 | **move-with-refactor** | 命中即删（一次性）；过期/未签发一律 false。CSRF 防护的落点，语义不得弱化。 |
| 39 | `TokenResponse` | type | src/addon/feishu/domain/user_token.rs:216-233 | **move-as-is** | 成功/失败共用结构，error/error_description 只在失败时出现。 |
| 40 | `TokenBundle` | type | src/addon/feishu/domain/user_token.rs:236-243 | **move-as-is** | 字段 pub：宿主 store_authorization(:541) 与 oauth_callback 都要读。 |
| 41 | `TokenEndpointError` | type | src/addon/feishu/domain/user_token.rs:246-251 | **move-as-is** | code=0 表示拿不到飞书业务码（网关错误页/传输失败）。字段 pub 供宿主 oauth_callback.rs:273 结构体字面量构造。 |
| 42 | `TokenEndpointError::new` | fn | src/addon/feishu/domain/user_token.rs:253-259 | **move-as-is** | 继续私有；外部用字面量或经 From<&OutboundFailure>。 |
| 43 | `TokenEndpointError::needs_reauth` | fn | src/addon/feishu/domain/user_token.rs:263-271 | **move-as-is** | 唯一补救是重新授权的四码族。 |
| 44 | `TokenEndpointError::hint` | fn | src/addon/feishu/domain/user_token.rs:275-294 | **move-as-is** | 10 个已登记业务码的中文排查提示（20074 后台开关、20071 redirect_uri 等），属飞书错误码契约。 |
| 45 | `impl From<&OutboundFailure> for TokenEndpointError` | type | src/addon/feishu/domain/user_token.rs:297-305 | **move-as-is** | 依赖 crate 的 transport 模块，需先落地 OutboundFailure/FailureKind。 |
| 46 | `parse_token_response / to_bundle / request_tokens` | fn | src/addon/feishu/domain/user_token.rs:308-374 | **move-as-is** | request_tokens 的 idempotent:false(:368) 是硬契约：code/refresh_token 一次性，重发只会拿到「已使用」。 |
| 47 | `exchange_code` | fn | src/addon/feishu/domain/user_token.rs:377-399 | **move-as-is** | grant_type=authorization_code；redirect_uri 必须与授权时逐字一致。 |
| 48 | `refresh_tokens` | fn | src/addon/feishu/domain/user_token.rs:402-421 | **move-as-is** | 被宿主 user_access_token(:749) 与提醒 worker 经其调用；旧的 refresh_token 立即作废。 |
| 49 | `fetch_user_info` | fn | src/addon/feishu/domain/user_token.rs:424-470 | **move-as-is** | GET /open-apis/authen/v1/user_info，bearer=user token；返回 (open_id, name)。 |
| 50 | `UserTokenRow` | type | src/addon/feishu/domain/user_token.rs:477-487 | **stay-in-host** | 映射宿主表 feishu_user_token，含 id/authorized_at 等库列，不属协议。 |
| 51 | `SINGLE_SLOT` | const | src/addon/feishu/domain/user_token.rs:491 | **stay-in-host** | 单行槽位 1，配合表声明的唯一索引。 |
| 52 | `load_single` | fn | src/addon/feishu/domain/user_token.rs:498-532 | **stay-in-host** | 必须显式 select_fields 读 secret 列（默认投影剔除 hidden），这条坑留在宿主。 |
| 53 | `store_authorization` | fn | src/addon/feishu/domain/user_token.rs:541-591 | **stay-in-host** | 单行替换 + 1062 竞态回读；authorized_at 只在这里写（365 天时钟判据）。TokenBundle 来自 crate 即可。 |
| 54 | `is_duplicate_key_error` | fn | src/addon/feishu/domain/user_token.rs:598-601 | **stay-in-host** | MySQL 1062 识别，宿主库细节。 |
| 55 | `update_tokens` | fn | src/addon/feishu/domain/user_token.rs:604-628 | **stay-in-host** | 刷新写回，不碰 authorized_at。 |
| 56 | `UserTokenError / message / From<TokenEndpointError>` | type | src/addon/feishu/domain/user_token.rs:636-678 | **stay-in-host** | 错误面混了宿主库错误（Failed）与控制台文案（NotAuthorized 的「去授权」），是产品 UX，不进协议库。 |
| 57 | `access_token_is_fresh` | fn | src/addon/feishu/domain/user_token.rs:681-683 | **move-as-is** | 提前 5 分钟刷新的纯函数。 |
| 58 | `user_access_token` | fn | src/addon/feishu/domain/user_token.rs:689-761 | **stay-in-host** | SELECT ... FOR UPDATE 串行刷新 + 5s 超时 + 事务提交，全是库操作；协议部分只剩对 crate refresh_tokens 的一行调用。 |
| 59 | `BLOCK_SIZE / BASE64 / Aes256CbcEnc / Aes256CbcDec` | const | src/addon/feishu/domain/crypto.rs:24-32 | **move-as-is** | 拆到 crypto/aes_cbc.rs 后仍为私有。 |
| 60 | `derive_key` | fn | src/addon/feishu/domain/crypto.rs:37-42 | **move-as-is** | sha256(配置原文)，宿主 context.rs:213 与 event/receive_approval_event 都调。 |
| 61 | `encrypt_bytes` | fn | src/addon/feishu/domain/crypto.rs:52-69 | **move-as-is** | PKCS#7 对齐时仍追加整块；输出 base64(IV‖密文)。与飞书 Go 参考实现一致，行为不能改。 |
| 62 | `encrypt_json` | fn | src/addon/feishu/domain/crypto.rs:78-82 | **move-as-is** | 外部选项响应加密（option/actions/approval_options.rs:638）。 |
| 63 | `decrypt_bytes` | fn | src/addon/feishu/domain/crypto.rs:88-109 | **move-as-is** | 事件订阅 challenge/事件解密（event_challenge.rs:92、receive_approval_event.rs:108）。CBC 无完整性保护，是飞书规范缺口。 |
| 64 | `seal` | fn | src/addon/feishu/domain/crypto.rs:118-123 | **move-as-is** | 返回 (sha256 摘要, 可逆密文)；拆到 crypto/seal.rs。 |
| 65 | `generate_token` | fn | src/addon/feishu/domain/crypto.rs:130-138 | **move-as-is** | 32 字节 OsRng 的 64 字符 hex。 |
| 66 | `IssuedToken` | type | src/addon/feishu/domain/crypto.rs:143-150 | **move-as-is** | 宿主 rotate_config_token.rs:66 与 rotate_token.rs 都读字段 → 必须 pub。 |
| 67 | `issue_token` | fn | src/addon/feishu/domain/crypto.rs:156-164 | **move-as-is** | 建表 / 轮换的唯一签发路径（5 处宿主调用），漏封存会让「复制」按钮静默失效。 |
| 68 | `unseal` | fn | src/addon/feishu/domain/crypto.rs:174-178 | **move-as-is** | 低层、不做完整性校验；生产路径应走 unseal_verified。 |
| 69 | `unseal_verified` | fn | src/addon/feishu/domain/crypto.rs:189-201 | **move-as-is** | 用同记录摘要当校验和，把「篡改/换钥匙/贴错行」变成确定性失败。 |
| 70 | `decrypt_cbc` | fn | src/addon/feishu/domain/crypto.rs:208-234 | **move-as-is** | 仅测试用（验证 PKCS#7 填充字节），随 crypto/aes_cbc.rs 的测试模块搬。 |
| 71 | `HASH_HEX_LEN` | const | src/addon/feishu/domain/token.rs:12 | **move-as-is** | 文件整体改名 crypto/digest.rs。 |
| 72 | `hash_token` | fn | src/addon/feishu/domain/token.rs:18-28 | **move-as-is** | 手写 hex 编码（不引 hex crate）是仓库惯例；宿主 6 处调用（dispatch、derive、portal_query、option 等）。 |
| 73 | `verify_token` | fn | src/addon/feishu/domain/token.rs:35-44 | **move-as-is** | fail-closed：库存摘要非 64 位 hex 一律 false 且不 panic；大小写/空白归一化保留。 |
| 74 | `tokens_equal` | fn | src/addon/feishu/domain/token.rs:49-56 | **move-as-is** | 明文 verification_token 常数时间比较（event_challenge.rs:168、receive_approval_event.rs:158）。 |
| 75 | `FeishuSettings` | type | src/config/mod.rs:583-699 | **stay-in-host** | 配置结构留在宿主；crate 只接 app_id/app_secret/redirect_uri 三个值。app_id/app_secret 是 Option<String>，宿主负责 unwrap_or_default。 |
| 76 | `FeishuSettings::can_pull` | fn | src/config/mod.rs:764-786 | **stay-in-host** | 占位凭证闸门（CHANGE_ME_*），启动期语义，不进库。 |
| 77 | `FeishuContext::encryption_key` | fn | src/addon/feishu/domain/context.rs:211-215 | **stay-in-host** | 替换点：super::crypto::derive_key → yang_feishu::crypto::derive_key。 |
| 78 | `user_token_table::table_spec` | fn | src/addon/feishu/approval/domain/user_token_table.rs:23-72 | **stay-in-host** | 表声明与字段权限（access_token/refresh_token 标 secret 且仅 system 可读写）是产品数据模型。 |
| 79 | `oauth_authorize_url::handle` | fn | src/addon/feishu/approval/actions/oauth_authorize_url.rs:58-81 | **stay-in-host** | 权限门 feishu.approval.write + 产品文案；只换 crate import 与 build_authorize_url 的 scope 参数。 |
| 80 | `oauth_callback::handle` | fn | src/addon/feishu/approval/actions/oauth_callback.rs:75-175 | **stay-in-host** | public 路由 + 审计 + 落库，Action 层整体不动。 |
| 81 | `oauth_status::handle` | fn | src/addon/feishu/approval/actions/oauth_status.rs:58-80 | **stay-in-host** | 读宿主表 + 调 crate 判据。 |
| 82 | `FeishuOauthReminderWorker` | type | src/infrastructure/feishu_oauth_reminder.rs:58-128 | **stay-in-host** | worker 生命周期、邮件与审计全留宿主；只把 :46 的 FeishuCredentials import 指向 crate。 |

### 需先重构

- **TenantTokenProvider::new 去掉 deployment 参数与 :276 的 crate::authorization::validate_deployment_name 调用；字段改名 namespace，仅用于 tracing；返回类型 Result<Self> 改 Self** — 这是 tenant_token.rs 唯一的反向依赖（crate 不能 import 宿主的 authorization 模块）。该值在宿主启动期已被 config/mod.rs:1572 与 version_cache.rs:67 校验两次，重复校验没有必要；去掉后 new 无失败分支。（`src/addon/feishu/domain/tenant_token.rs:258-284`）
- **token_key(144-146)/lock_key(149-151) 拆成两处：宿主保留 token_key(deployment, app_id) 生成 `yang-system:{deployment}:feishu:app:{app_id}:tenant_token`；crate 的 RedisTenantTokenCache::new(redis, token_key) 内部派生 `{…** — 去掉 crate 里的产品前缀；同时把 app_id 纳入键，修掉「同一部署两个自建应用共用一个缓存槽、互相拿到对方 token」的单应用假设。键变更会让存量缓存一次性失效，最长 2 小时后自愈。（`src/addon/feishu/domain/tenant_token.rs:144-151, 184-198`）
- **RedisTenantTokenCache + TenantTokenCache impl + RELEASE_LOCK_SCRIPT 不迁入 crate，移到宿主新文件 domain/tenant_token_cache.rs** — 它依赖 yang_db::RedisClient（sqlx 整棵树）。协议库应只依赖注入的 trait；crate 里保留 TenantTokenCache 定义即可，宿主/其他项目可各写各的实现。（`src/addon/feishu/domain/tenant_token.rs:158-251`）
- **user_token.rs 按 470 行切成两半：1-470（协议 + state）进 crate 的 auth/user_oauth.rs 与 auth/state.rs；472-761（UserTokenRow/load_single/store_authorization/is_duplicate_key_error/update_tokens/UserTokenError/user_ac…** — 下半段全部依赖 Repository/Transaction/sqlx 与宿主表结构；上半段只依赖 transport trait、serde 与 uuid，可独立编译。切分后协议文件不得出现 use crate::config / crate::addon / yang_base::table 的 import。（`src/addon/feishu/domain/user_token.rs:472-761`）
- **state 存储抽成 trait：pub trait OAuthStateStore { fn mint(&self, now: i64) -> String; fn consume(&self, state: &str, now: i64) -> bool; } + #[derive(Default)] pub struct InMemoryOAuthStateStore** — 把 175-208 的进程级 static 从协议函数里剥离，宿主可注入自己的实现（多实例共享、测试隔离）。trait 取同步：唯一实现是内存表，设计文档明确 state 不落库；改异步只会在 3 个调用点加 .await 而无收益。crate 不提供 lib 级全局单例，避免跨测试污染。（`src/addon/feishu/domain/user_token.rs:175-208`）
- **build_authorize_url 增加 scope 参数；OAUTH_SCOPES 留在宿主** — scope 是「本产品要哪些权限」的配置决策（每加一个 scope 都要在开发者后台开通），不是飞书协议常量。crate 只负责按 RFC 3986 编码并拼六参数。（`src/addon/feishu/domain/user_token.rs:69-70, 164-172`）
- **oauth_config(settings) 留在宿主，作为 FeishuSettings → OAuthConfig 的唯一适配器** — 它读 [feishu].app_id/app_secret/oauth_redirect_uri 并调用 can_pull()，是 crate 绝不能知道的配置形状；同时它承担中文可行动文案（缺 redirect_uri 的 20071 提示）。（`src/addon/feishu/domain/user_token.rs:117-139`）
- **crypto.rs 拆四文件：crypto/mod.rs（derive_key + re-export）、crypto/aes_cbc.rs（BLOCK_SIZE/BASE64/Aes256CbcEnc/Dec/encrypt_bytes/encrypt_json/decrypt_bytes/decrypt_cbc）、crypto/digest.rs（原 token.rs 全文）、crypto/s…** — AES 与摘要/封存是三层不同责任：aes_cbc 无状态纯函数，digest 被 derive/portal 复用，seal 组合两者且是凭据生命周期语义。拆开后 expiry/凭据相关测试各归其位，也避免 seal 的调用方被迫链接 AES。（`src/addon/feishu/domain/crypto.rs:24-234`）
- **token.rs 整体改名为 crypto/digest.rs，内容零改动** — 文件内容就是 SHA-256 与常数时间比较，放进 crypto 命名空间与 derive_key 同层；宿主 import 路径由薄壳保持兼容。（`src/addon/feishu/domain/token.rs:1-56`）
- **删除 crypto.rs:484-508 的真实 challenge 测试向量，不迁入 crate；宿主如需保留，改为从环境变量读 key/ciphertext 的 #[ignore] 测试** — 该测试内联了一把真实的飞书 event_encrypt_key 与对应密文（生产日志截取）。搬进公共 crate 等于把租户密钥随库分发。crate 侧用 key_derivation_is_sha256_of_raw_text(:402-406) 与 round_trip(:449-471) 已足够钉住算法形态。（`src/addon/feishu/domain/crypto.rs:484-508`）
- **Cargo.toml：yang-feishu 依赖 yang-base（BaseError、http）、async-trait、serde、serde_json、anyhow、sha2、subtle、aes、cbc、base64、rand_core、uuid、metrics；不依赖 yang-db** — crypto 的公开签名返回 BaseError，transport trait 也用 BaseError，故 yang-base 必需；Redis 实现留宿主后 yang-db 不再是依赖，协议库不会拖入 sqlx/mysql/postgres。（`crates/yang-feishu/Cargo.toml（新建）`）
- **宿主四个 domain 模块改成薄壳 re-export（crypto.rs / token.rs / tenant_token.rs / user_token.rs 头部），调用点不动** — 宿主现有 ~40 处 crate::addon::feishu::domain::{crypto,token,tenant_token}::… 的 import；保留同名模块做 pub(crate) use 能把 diff 压到 4 个文件，符合最小改动。（`src/addon/feishu/domain/mod.rs:8-12, 34-36`）
- **outbound.rs:1269-1280 的锁 TTL 不变式测试改为引用 yang_feishu::auth::tenant_token::LOCK_TTL_SECONDS；若 outbound 整体也进 crate，则改为 crate::auth::…** — 该测试是 LOCK_TTL_SECONDS=30 的唯一依据（19s < 30s），跨模块引用路径必须随符号搬家同步改，否则编译失败或测试失去约束。（`src/addon/feishu/domain/outbound.rs:1269-1280`）

### 目标公开 API

| 名称 | 签名 | 备注 |
|---|---|---|
| `yang_feishu::FEISHU_OPEN_BASE` | `pub const FEISHU_OPEN_BASE: &str = "https://open.feishu.cn";` | crate 根；tenant/user/approval 三处共用。 |
| `yang_feishu::auth::tenant_token::FeishuCredentials` | `pub struct FeishuCredentials { pub app_id: String, pub app_secret: String }  // 手写 Debug，secret 打码` | 一实例 = 一个自建应用，多应用由宿主建多个 provider。 |
| `TenantTokenGrant / parse_token_grant` | `pub struct TenantTokenGrant { pub tenant_access_token: String, pub expire: i64 } pub fn parse_token_grant(body: &str) -> anyhow::Result<TenantTokenGrant>` | 扁平信封；业务码失败与缺字段分开报。 |
| `cache_ttl_seconds` | `pub fn cache_ttl_seconds(expire: i64) -> anyhow::Result<i64>` | expire-300，且要求 expire>300。 |
| `TenantTokenCache` | `#[async_trait] pub trait TenantTokenCache: Send + Sync {   async fn get(&self) -> anyhow::Result<Option<String>>;   async fn put(&self, token: &str, ttl_seconds: i64) -> anyhow::Result<()>;   async fn invalidate(&self) -> anyhow::Result<()>;   async fn acquire_lock(&self, owner: &str, ttl_seconds: i64) -> anyhow::Result<bool>;   async fn release_lock(&self, owner: &str) -> anyhow::Result<()>; }` | 键作用域由实现方构造参数决定，crate 不含 deployment/app_id。 |
| `TenantTokenProvider` | `pub struct TenantTokenProvider; impl TenantTokenProvider {   pub fn new(cache: Arc<dyn TenantTokenCache>, transport: Arc<dyn OutboundTransport>, sleeper: Arc<dyn Sleeper>, credentials: FeishuCredentials, namespace: &str) -> Self;   pub async fn tenant_access_token(&self) -> anyhow::Result<String>;   pub async fn invalidate_and_refresh(&self) -> anyhow::Result<String>; }` | new 不再返回 Result（校验已删）。 |
| `LOCK_TTL_SECONDS` | `pub const LOCK_TTL_SECONDS: i64 = 30;` | 必须 > TOKEN_RETRY.worst_case_seconds(TOKEN_REQUEST_TIMEOUT_SECS)=19。 |
| `OAuthConfig` | `pub struct OAuthConfig { pub credentials: FeishuCredentials, pub redirect_uri: String }` | host 的 oauth_config() 产出它。 |
| `build_authorize_url` | `pub fn build_authorize_url(app_id: &str, redirect_uri: &str, scope: &str, state: &str) -> String` | 自动加 response_type=code 与 prompt=consent。 |
| `OAuthStateStore / InMemoryOAuthStateStore` | `pub trait OAuthStateStore: Send + Sync {   fn mint(&self, now: i64) -> String;   fn consume(&self, state: &str, now: i64) -> bool; } #[derive(Default)] pub struct InMemoryOAuthStateStore { /* Mutex<HashMap<String, i64>> */ } pub const STATE_TTL_SECONDS: i64 = 600;` | 宿主持有单个实例（static 或 context 字段）；crate 不导出全局。 |
| `TokenBundle` | `pub struct TokenBundle { pub access_token: String, pub access_token_expires_at: i64, pub refresh_token: String, pub refresh_token_expires_at: i64, pub scope: Option<String> }` | 绝对过期时刻由 now + expires_in 算好。 |
| `TokenEndpointError` | `pub struct TokenEndpointError { pub code: i32, pub message: String } impl TokenEndpointError { pub fn needs_reauth(&self) -> bool; pub fn hint(&self) -> Option<&'static str>; }` | code=0 表示没拿到飞书业务码。 |
| `exchange_code` | `pub async fn exchange_code(transport: &dyn OutboundTransport, sleeper: &dyn Sleeper, credentials: &FeishuCredentials, code: &str, redirect_uri: &str, now: i64) -> Result<TokenBundle, TokenEndpointError>` | grant_type=authorization_code；不重试。 |
| `refresh_tokens` | `pub async fn refresh_tokens(transport: &dyn OutboundTransport, sleeper: &dyn Sleeper, credentials: &FeishuCredentials, refresh_token: &str, now: i64) -> Result<TokenBundle, TokenEndpointError>` | 一次性凭证，idempotent=false。 |
| `fetch_user_info` | `pub async fn fetch_user_info(transport: &dyn OutboundTransport, sleeper: &dyn Sleeper, access_token: &str) -> Result<(String, Option<String>), TokenEndpointError>` | 返回 (open_id, name)。 |
| `access_token_is_fresh / reauth_reminder_due / now_unix_secs` | `pub fn access_token_is_fresh(expires_at: i64, now: i64) -> bool; pub fn reauth_reminder_due(authorized_at: i64, now: i64) -> bool; pub fn now_unix_secs() -> i64;` | 三条时钟的纯函数判据，宿主状态页与 worker 共用。 |
| `yang_feishu::crypto::{derive_key, encrypt_bytes, encrypt_json, decrypt_bytes}` | `pub fn derive_key(raw: &str) -> [u8; 32]; pub fn encrypt_bytes(plaintext: &[u8], key: &[u8; 32]) -> Result<String, BaseError>; pub fn encrypt_json(value: &serde_json::Value, key: &[u8; 32]) -> Result<String, BaseError>; pub fn decrypt_bytes(encoded: &str, key: &[u8; 32]) -> Result<Vec<u8>, BaseError>;` | AES-256-CBC + PKCS#7 + base64(IV‖密文)，与飞书 Go 参考实现逐字节一致。 |
| `yang_feishu::crypto::{hash_token, verify_token, tokens_equal}` | `pub fn hash_token(token: &str) -> String; pub fn verify_token(presented: &str, stored_hash: &str) -> bool; pub fn tokens_equal(left: &str, right: &str) -> bool;` | SHA-256 hex + subtle 常数时间比较；verify_token fail-closed。 |
| `yang_feishu::crypto::{seal, unseal, unseal_verified, generate_token, issue_token, IssuedToken}` | `pub fn seal(plaintext: &str, key: &[u8; 32]) -> Result<(String, String), BaseError>; pub fn unseal(cipher: &str, key: &[u8; 32]) -> Result<String, BaseError>; pub fn unseal_verified(cipher: &str, expected_hash: &str, key: &[u8; 32]) -> Result<String, BaseError>; pub fn generate_token() -> String; pub struct IssuedToken { pub plaintext: String, pub hash: String, pub cipher: String } pub fn issue_to…` | 凭据封存三件套；生产回显路径必须用 unseal_verified。 |

### 测试处置（28）

| 文件 | 区间 | 内容 | 处置 |
|---|---|---|---|
| src/addon/feishu/domain/tenant_token.rs | 441-504 | parse_token_grant 五例：扁平信封、非 0 码、缺字段分开报、空白 token、非正 expire | **move** |
| src/addon/feishu/domain/tenant_token.rs | 506-519 | cache_ttl_keeps_the_official_thirty_minute_margin：7200→6900、300/299 拒绝、301→1 | **move** |
| src/addon/feishu/domain/tenant_token.rs | 521-532 | cache_keys_are_namespaced_by_deployment：断言 yang-system:{deployment}:feishu:tenant_token | **stay** |
| src/addon/feishu/domain/tenant_token.rs | 534-549 | credentials_debug_never_leaks_the_secret | **move** |
| src/addon/feishu/domain/tenant_token.rs | 551-560 | deployment_name_shape_is_validated：测的是宿主 authorization::validate_deployment_name | **stay** |
| src/addon/feishu/domain/tenant_token.rs | 562-761 | 测试假件 ScriptedTransport/CountingSleeper/InMemoryCache/grant_body/provider() | **move** |
| src/addon/feishu/domain/tenant_token.rs | 765-809 | cache_hit_never_touches_the_network_or_the_lock + fresh_fetch_caches_with_expire_minus_300_and_releases_the_lock | **move** |
| src/addon/feishu/domain/tenant_token.rs | 811-857 | waiting_for_another_holder_then_adopting_its_value + lock_wait_timeout_degrades_without_writing_the_cache | **move** |
| src/addon/feishu/domain/tenant_token.rs | 859-930 | invalidate_and_refresh 清缓存强刷、空白缓存值按未命中、业务失败冒泡且仍放锁 | **move** |
| src/addon/feishu/domain/user_token.rs | 763-858 | 测试假件 ScriptedTransport/NoSleep/PausedRefresh/credentials/success_body | **rewrite** |
| src/addon/feishu/domain/user_token.rs | 860-893 | authorize_url_carries_offline_access_and_encodes_the_redirect | **rewrite** |
| src/addon/feishu/domain/user_token.rs | 895-911 | state_is_single_use_and_expires：一次性 + TTL 边界 | **rewrite** |
| src/addon/feishu/domain/user_token.rs | 913-919 | access_token_freshness_keeps_a_margin | **move** |
| src/addon/feishu/domain/user_token.rs | 921-1034 | exchange_code 请求体与过期时刻、缺 refresh_token 点名 offline_access、刷新错误分类、刷新不重试、user_info 返回 open_id/name | **move** |
| src/addon/feishu/domain/user_token.rs | 1036-1059 | only_unique_key_conflicts_are_treated_as_an_insert_race | **stay** |
| src/addon/feishu/domain/user_token.rs | 1061-1082 | reminder_line_starts_at_350_days | **move** |
| src/addon/feishu/domain/user_token.rs | 1084-1102 | error_messages_are_actionable（UserTokenError 文案） | **stay** |
| src/addon/feishu/domain/user_token.rs | 1104-1132 | bundle() 辅助（随 TokenBundle 迁移）与 expect_row/row_count（留宿主） | **rewrite** |
| src/addon/feishu/domain/user_token.rs | 1142-1299 | real_mysql_round_trips_secret_token_columns（#[ignore]，真实 MySQL：secret 列读写、重授权替换、刷新不重置 authorized_at、行锁串行） | **stay** |
| src/addon/feishu/domain/crypto.rs | 241-400 | seal/unseal 辅助与 10 例封存测试（自洽、换钥匙、篡改 IV、跨记录贴密文、轮换、非法密文、多字节）+ expected_key/decode | **move** |
| src/addon/feishu/domain/crypto.rs | 402-482 | derive_key 已知摘要、整块填充、部分填充、IV 前置与块对齐、随机 IV、PKCS#7 往返、encrypt_json 形状 | **move** |
| src/addon/feishu/domain/crypto.rs | 484-508 | real_feishu_encrypted_challenge_decrypts_correctly：内联真实 event_encrypt_key 与生产密文 | **rewrite** |
| src/addon/feishu/domain/token.rs | 58-121 | hash_token 已知向量/稳定性、verify_token 匹配/不匹配/畸形摘要/大小写/空白、tokens_equal | **move** |
| src/addon/feishu/approval/actions/oauth_authorize_url.rs | 83-101 | the_url_carries_the_configured_redirect_and_a_fresh_state | **stay** |
| src/addon/feishu/approval/actions/oauth_callback.rs | 242-280 | query 解码（含多余参数容忍）与 describe 20071 文案 | **stay** |
| src/addon/feishu/approval/actions/oauth_status.rs | 82-99 | status_projection_serializes_the_console_fields | **stay** |
| src/infrastructure/feishu_oauth_reminder.rs | 301-309 | the_interval_is_one_day | **stay** |
| src/addon/feishu/approval/domain/user_token_table.rs | 74-171 | 表名/主键、slot 唯一索引、必填列、secret 字段、中文展示名 | **stay** |

### 宿主替换点（12）

- `src/addon/feishu/domain/crypto.rs:1-509` → 整文件替换为薄壳：`pub(crate) use yang_feishu::crypto::*;`（crypto 无任何宿主依赖，可直接删除原实现）
- `src/addon/feishu/domain/token.rs:1-122` → 整文件替换为：`pub(crate) use yang_feishu::crypto::{hash_token, tokens_equal, verify_token};`
- `src/addon/feishu/domain/tenant_token.rs:1-429` → 替换为： ```rust pub(crate) use yang_feishu::auth::tenant_token::{     cache_ttl_seconds, parse_token_grant, FeishuCredentials, TenantTokenCache,     TenantTokenGrant, TenantTokenProvider, LOCK_TTL_SECONDS, FEISHU_OPEN_BASE, };  /// 产品缓存键：加 app_id 修单应用假设。 pub(crate) fn token_key(deployment: &str, app_id: &str) -> String {     format!("yang-system:{deployment}:feishu:app:{app_id}:tenant_token") } ```
- `src/addon/feishu/domain/tenant_token.rs:158-251` → 新建 `src/addon/feishu/domain/tenant_token_cache.rs`：原样搬 RELEASE_LOCK_SCRIPT 与 impl TenantTokenCache for RedisTenantTokenCache，构造函数改为 ```rust pub(crate) fn new(redis: RedisClient, token_key: impl Into<String>) -> Self {     let key = token_key.into();     Self { lock_key: format!("{key}:lock"), redis, key } } ```
- `src/addon/feishu/domain/outbound_setup.rs:58-84` → ```rust let cache = Arc::new(RedisTenantTokenCache::new(     ctx.tools().cache()?.clone(),     token_key(&deployment, &app_id), )); ... let tokens = TenantTokenProvider::new(     cache, transport.clone(), sleeper.clone(),     FeishuCredentials { app_id, app_secret },     &deployment,          // 仅日志 );                        // 不再 .map_err(...)? ```
- `src/addon/feishu/domain/user_token.rs:1-470` → 替换为： ```rust pub(crate) use yang_feishu::auth::user_oauth::{     access_token_is_fresh, build_authorize_url, exchange_code, fetch_user_info,     now_unix_secs, reauth_reminder_due, refresh_tokens, OAuthConfig,     OAuthStateStore, TokenBundle, TokenEndpointError, REAUTH_REMINDER_DAYS, }; pub(crate) use yang_feishu::auth::state::InMemoryOAuthStateStore;  /// 产品权限策略，不进协议库。 pub(crate) const OAUTH_SCO…
- `src/addon/feishu/domain/context.rs:211-215` → `.map(yang_feishu::crypto::derive_key)`（若 crypto.rs 保留薄壳则无需改）
- `src/addon/feishu/domain/outbound.rs:1269-1280` → `worst < yang_feishu::auth::tenant_token::LOCK_TTL_SECONDS as u64`（outbound 若也进 crate 则用 `crate::auth::tenant_token::LOCK_TTL_SECONDS`）
- `src/addon/feishu/approval/actions/oauth_authorize_url.rs:74-76` → ```rust let state = user_token::mint_state(user_token::now_unix_secs()); let authorize_url = user_token::build_authorize_url(     &config.credentials.app_id,     &config.redirect_uri,     user_token::OAUTH_SCOPES,     &state, ); ```
- `src/addon/feishu/domain/mod.rs:8-12` → 保留 `pub(crate) mod crypto;` / `pub(crate) mod token;` / `pub(crate) mod tenant_token;` 作为薄壳文件名，新增 `pub(crate) mod tenant_token_cache;`；`user_token` 继续存在（只剩存储）
- `src/infrastructure/feishu_oauth_reminder.rs:44-47` → `FeishuCredentials` 改从薄壳（crate::addon::feishu::domain::tenant_token）导入即可；user_token::* 的调用点不变
- `src/addon/feishu/domain/crypto.rs:484-508` → 删除该测试。若必须保留真机回归，宿主另建 `#[ignore = "需要真实密钥"]` 测试，从环境变量读 `FEISHU_EVENT_ENCRYPT_KEY_SAMPLE` / `FEISHU_ENCRYPTED_CHALLENGE_SAMPLE`，不得内联任何真实密钥或密文

### 风险

- crypto.rs:485-508 内联了真实的生产 event_encrypt_key 与对应密文（生产日志截取）。这是凭据泄漏：绝不能迁入公共 crate，也不应留在源码里；建议按已泄露处置——在飞书开发者后台轮换 event_encrypt_key，并把该测试改为环境变量驱动的 #[ignore] 测试或直接删除。
- TenantTokenProvider::new 从 Result 改 Self 会波及约 12 处调用点（outbound_setup.rs:70、feishu_approval_worker.rs:251、refresh_names.rs:416、dispatch.rs:2957、approval.rs:1417、approval_dispatch.rs:2199/2977、approval_notification.rs:843/1254、approval_row_edit.rs:274、bitable.rs:2306、tenant_token.rs:750），全部需要删掉 .map_err/.unwrap_or_else；漏改会编译失败，不会静默。
- 缓存键加入 app_id 会使存量 tenant token 缓存一次性失效（旧键 yang-system:{deployment}:feishu:tenant_token 不再命中）。影响仅一次换取请求，最长 2 小时后自然消失，但部署当天要预期一轮额外的 token 换取。
- crate 的 auth 模块依赖 transport 层的 OutboundTransport/OutboundRequest/OutboundResponse/send_with_retry/RetryPolicy/TOKEN_RETRY/TOKEN_REQUEST_TIMEOUT_SECS/OutboundFailure/FailureKind。若 L1 尚未把 outbound 抽进 crate，auth 无法编译；建议先定义这些 trait/类型（它们本身无宿主依赖），host 继续提供 HttpClientTransport 实现。
- RedisTenantTokenCache 留宿主意味着 crate 不提供生产可用的缓存；若其他消费方直接用 crate，必须自己实现 5 个方法的 TenantTokenCache。若要内置，应加 feature（redis = dep:yang-db）而不是无条件依赖——yang-db 会拖入 sqlx/mysql/postgres 整棵树。
- crate 的公开 API 目前泄漏 anyhow::Result（TenantTokenCache、parse_token_grant）与 yang_base::BaseError（crypto）。这会让 yang-feishu 与 yang-base 的版本强绑定；若要独立发版，需要后续引入自有错误类型，本轮不做。
- user_token.rs 的测试假件 PausedRefresh 与 real_mysql 测试共用；按 763-858 整体搬会把 MySQL 依赖带进 crate。实际切分要逐符号：PausedRefresh(:816) 留宿主，其余 ScriptedTransport/NoSleep/credentials/success_body 进 crate。
- state 改成 trait 后，宿主若继续用进程级 static，多实例部署下 state 仍是各进程独立——与当前行为一致（设计文档明确不落库），但计划做多实例灰度时要重新评估；crate 不应偷偷引入全局单例。
- user_token.rs 协议层当前 import 了 yang_base::table::Record 与 super::repository::Repository（第 36、43 行）。切分时若只删函数不删 import，crate 会因宿主类型无法解析而编译失败；必须保证协议文件对 crate::addon / crate::config / yang_base::table 零引用。
- 所有行号基于当前 HEAD e5f294c 与工作区实际内容；抽取分支若先合入其他改动（尤其 crypto.rs / user_token.rs），需按 git diff 重新核对。
- oauth_callback.rs:273 用结构体字面量构造 TokenEndpointError，要求字段 pub；若 crate 改成私有字段会直接编译失败，签名设计时已按 pub 给出。

## A.3 L1 多维表格客户端 → crates/yang-feishu::bitable

**目标**：把 yang-system 的 bitable 能力（HTTP 端点、DTO、分页收敛、字段类型判定、单元格归一化、链接解析、读写客户端）原样抽成 D:/code/lib_yang/crates/yang-feishu 的 bitable 模块；拉取编排（pull.rs）、选项派生（derive.rs）、绑定校验（field_binding.rs）、source_key、选项游标（pagination.rs）全部留在宿主 D:/code/lib_yang/project/yang-system。结论：bitable.rs 生产代码 1763 行几乎 100% 可搬，只有 3 组符号必须先拆到 crate 公共层（path / api），另有 6 处命名与 1 处宿主耦合需先重构。

### 源文件

| 文件 | 生产行区间 | 生产行数 | 测试行区间 | 测试行数 |
|---|---|---:|---|---:|
| src/addon/feishu/domain/bitable.rs | 1-1036,1073-1799 | 1763 | 1037-1072,1800-2863 | 1100 |
| src/addon/feishu/domain/pagination.rs | 1-47 | 47 | 48-128 | 81 |
| src/addon/feishu/domain/pull.rs | 1-1363 | 1363 | 1364-2059 | 696 |
| src/addon/feishu/domain/derive.rs | 1-295 | 295 | 296-755 | 460 |
| src/addon/feishu/domain/field_binding.rs | 1-145 | 145 | 146-258 | 113 |
| src/addon/feishu/domain/source_key.rs | 1-23 | 23 | 24-57 | 34 |

### 符号（99）

| # | 符号 | 类型 | 位置 | 处置 | 备注 |
|---:|---|---|---|---|---|
| 1 | `bitable` | module | src/addon/feishu/domain/mod.rs:11 | **move-with-refactor** | 整个 bitable.rs 搬成 crates/yang-feishu/src/bitable.rs（单文件，先不拆子模块：1900 行生产代码是现状，拆模块不产生任何正确性收益）。先把 path::validate_path_segment 与 api::{FeishuApiEnvelope,decode_envelope,send_json} 抽到 crate 公共层，再搬剩余部分。宿主 domain/mod.rs 用 `pub(crate) use yang_feishu::bitable;` 保留 `cra… |
| 2 | `PAGE_SIZE` | const | src/addon/feishu/domain/bitable.rs:43 | **move-as-is** | 《列出记录》每页 500（官方上限 500、默认 20）。文件头 :21-27 明确记录：设计文档写的「100 上限」是审批后台选项数限制，不是分页上限，照抄会把页数放大 5 倍。不可与 PAGE_SIZE_FIELDS / SEARCH_PAGE_SIZE 合并——三者出处不同（:1376-1379 已写明）。 |
| 3 | `PAGE_SIZE_FIELDS` | const | src/addon/feishu/domain/bitable.rs:46 | **move-as-is** | 字段/数据表/视图三个列表端点共用（官方默认 20、上限 100）。测试 :2177 间接钉住这个值。 |
| 4 | `MAX_PAGES` | const | src/addon/feishu/domain/bitable.rs:49 | **move-as-is** | 单轮最多翻页数，防 page_token 不推进的死循环。宿主 src/infrastructure/feishu_pull.rs:52-54 又声明了一份 MAX_PAGES_PER_ROUND=40（注释自认「与 bitable::MAX_PAGES 同源」）；搬走后宿主应改为引用 crate 常量，避免两处漂移。记录类型另有一份局部上限 20（:1179，见 record_row_numbers）。 |
| 5 | `MAX_PATH_SEGMENT_LEN` | const | src/addon/feishu/domain/bitable.rs:52 | **split** | 必须与 validate_path_segment 一起上提到 yang_feishu::path，不要留在 bitable：approval.rs:43 已复制了一份同名常量（注释「与 bitable 同源」），是第二个消费者。 |
| 6 | `validate_path_segment` | fn | src/addon/feishu/domain/bitable.rs:66 | **split** | 必须拆分：它是 URL 路径段白名单（ASCII 字母数字 + - _），不是 bitable 语义。宿主调用点至少 6 处：approval.rs:63（instance_code）、datasource/actions/list_bitable_fields.rs:15、list_bitable_tables.rs:19、list_bitable_views.rs:15、create_datasource_table.rs:23、update_datasource_table.rs:22。page_token … |
| 7 | `records_url` | fn | src/addon/feishu/domain/bitable.rs:84 | **move-as-is** | 在函数内部校验两个路径段再拼（:81-83 写明理由：把校验放调用方是这类代码最常见的漏洞）。get_records_by_ids(:1635) 与 get_record_shared_url(:799) 都靠它拼子路径。 |
| 8 | `fields_url` | fn | src/addon/feishu/domain/bitable.rs:93 | **move-as-is** | 测试 :2084 钉住「不发送 view_id」——列出字段的 view_id 参数实测不生效，别顺手加。 |
| 9 | `views_url` | fn | src/addon/feishu/domain/bitable.rs:104 | **move-as-is** | 官方《列出视图》路径。测试 :2065 与 :2078 钉住形状与两段路径穿越拒绝。 |
| 10 | `tables_url` | fn | src/addon/feishu/domain/bitable.rs:116 | **move-as-is** | 只有 app_token 一个路径段。测试 :1938 钉住 /open-apis 这一段必须在（漏掉会打到一个不存在的路径上）。 |
| 11 | `list_records_query` | fn | src/addon/feishu/domain/bitable.rs:131 | **move-as-is** | 三条必须原样保留的契约：① field_names 是单个 JSON 数组字符串，不是重复参数（写成重复参数稳定吃 1254024）；② 不发 offset（官方该接口没有这个参数）；③ text_field_as_array 固定 false（多行文本保持字符串）。测试 :2158/:2177/:2189/:2200/:2210。 |
| 12 | `list_fields_query` | fn | src/addon/feishu/domain/bitable.rs:166 | **move-with-refactor** | 重构：改名为 list_page_query。它同时服务列出字段（:1343）、列出数据表（:1295）、列出视图（:1247）——「fields」这个名字会让后来的人以为表/视图另有实现。 |
| 13 | `FeishuApiEnvelope` | type | src/addon/feishu/domain/bitable.rs:186 | **split** | 必须拆到 yang_feishu::api。同形状信封在宿主被重复实现：approval.rs:681 的局部 `Envelope`、user_token.rs:458 的局部 `Envelope`，两处各写一遍 decode。data 刻意是 Option<Value> 而不是 Option<T>（:178-184：泛型会让 #[serde(default)] 引入 T: Default 约束）。 |
| 14 | `RecordItem` | type | src/addon/feishu/domain/bitable.rs:199 | **move-as-is** | fields 必须是 map<string,union> 原样 Value，不能 typed（:202-206：数字/进度/货币在《列出记录》里是字符串、在《查询记录》里是 number，typed 会反序列化失败）。created_by 在 record 顶层、不在 fields 里（官方 batch_get 示例）。只到 PartialEq，不实现 Eq（f64）。 |
| 15 | `RecordPerson` | type | src/addon/feishu/domain/bitable.rs:219 | **move-as-is** | id 的类型跟随请求体的 user_id_type（本模块固定 open_id）；name 缺 contact 字段权限时可能不返回，只消费 id。 |
| 16 | `ListRecordsData` | type | src/addon/feishu/domain/bitable.rs:229 | **move-as-is** | page_token 必须是 Option：has_more=false 时该字段根本不出现，写成 String 会反序列化失败（测试 :2826）。total 是 Option，用于收敛断言。 |
| 17 | `BitableTablesData` | type | src/addon/feishu/domain/bitable.rs:247 | **move-as-is** | 必须带分页字段：一张 Base 的表数可超过默认 page_size，少了它会把第 2 页静默吞掉。 |
| 18 | `BitableTableItem` | type | src/addon/feishu/domain/bitable.rs:260 | **move-as-is** | 表 id 与展示名。配置向导第一步用它让运维在界面上选表，宿主侧不持久化除 title 外的字段。 |
| 19 | `BitableViewsData` | type | src/addon/feishu/domain/bitable.rs:269 | **move-as-is** | 与 ListFieldsData 同理必须带分页字段；视图数超默认 page_size 时少了它同样静默丢页。 |
| 20 | `BitableViewItem` | type | src/addon/feishu/domain/bitable.rs:282 | **move-as-is** | view_type: grid/kanban/gallery/gantt/form。视图只决定拉取哪些行、不决定能勾哪些字段——这句产品语义写在 :1228-1229，搬走后宿主 UI 文案要自行保留。 |
| 21 | `ListFieldsData` | type | src/addon/feishu/domain/bitable.rs:298 | **move-as-is** | 没有分页字段时字段接口默认 page_size=20，第 2 页起看不见，resolve_field_name 会把「存在但没翻到」报成「找不到」——别删分页字段。 |
| 22 | `FieldItem` | type | src/addon/feishu/domain/bitable.rs:311 | **move-as-is** | field_type 是消除「单选取值形态」歧义的唯一可靠依据（:323-324）；property 里才有选项。宿主 8 个文件把它当字段元数据载体。 |
| 23 | `FieldProperty` | type | src/addon/feishu/domain/bitable.rs:341 | **move-as-is** | 选项嵌在 property 下、不在字段顶层；没有这一位将派生不出任何选项。 |
| 24 | `PropertyOption` | type | src/addon/feishu/domain/bitable.rs:349 | **move-as-is** | 只取 name；宿主 approval_match.rs:1035 的测试直接构造它。 |
| 25 | `FieldItem::option_names` | fn | src/addon/feishu/domain/bitable.rs:356 | **move-as-is** | trim + 去空；非选项类字段返回空。 |
| 26 | `is_single_value_field_type` | fn | src/addon/feishu/domain/bitable.rs:375 | **move-as-is** | 1/2/3/5/13/15/20/1005。只用于级联父列（父键按一格一个文案折叠）。宿主 field_maintenance.rs:20 也在用。 |
| 27 | `is_option_source_field_type` | fn | src/addon/feishu/domain/bitable.rs:388 | **move-as-is** | 比单值版多接受多选 4；人员 11/附件 17/关联 18,21/群组 23/复选框 7/地理位置 22 一律拒绝。宿主 pull.rs:964、health_check.rs:32、field_maintenance.rs:20 在用。 |
| 28 | `check_coordinate_field` | fn | src/addon/feishu/domain/bitable.rs:397 | **move-as-is** | field_type 缺失时放行（:402-405：宁可放行也不因元数据缺失拒绝一个本来可用的配置）。错误文案含产品词「选项来源」，是宿主 UI 直接展示的字符串，逐字保留。 |
| 29 | `resolve_field_name` | fn | src/addon/feishu/domain/bitable.rs:426 | **move-as-is** | 三种失败都必须报：找不到 field_id、名字为空、同名不唯一。直接送 field_id 给 field_names 会稳定吃 1254024。 |
| 30 | `resolve_field_names` | fn | src/addon/feishu/domain/bitable.rs:460 | **move-as-is** | 批量、点名全部缺失项、保持输入顺序、按 id 稳定（改名不断链）、重名拦截。宿主 dispatch.rs:1170/3754 与 worker 都在用。 |
| 31 | `resolve_current_field_names` | fn | src/addon/feishu/domain/bitable.rs:510 | **move-as-is** | 全仓库零调用点（grep 只命中定义自身），被 :29 的 #![allow(dead_code)] 掩盖。搬走时二选一：删掉，或加函数级 #[allow(dead_code)] 保留；不要带文件级 allow 进 crate。 |
| 32 | `CellValue` | type | src/addon/feishu/domain/bitable.rs:535 | **move-as-is** | 三态是刻意的：把「合法空」与「不支持的列类型」分开，否则取数列选到人员/群组/地理位置会静默产出空标签，再叠加补集停用会把整列选项停掉。 |
| 33 | `cell_label` | fn | src/addon/feishu/domain/bitable.rs:556 | **move-as-is** | 单选同时接受裸字符串与长度 1 的数组（两种形态都有实证，:546-551）。多选逐元素出标签；数组里混入不可识别元素整格判 Unsupported、不截断。宿主 pull.rs:1154/1181、pull_probe.rs:42 在用。 |
| 34 | `union_text` | fn | src/addon/feishu/domain/bitable.rs:587 | **move-as-is** | 字符串直取、数字字符串化、{text,type} 取 text；其余 None。私有辅助，随 cell_label 一起搬。 |
| 35 | `classify_text` | fn | src/addon/feishu/domain/bitable.rs:599 | **move-as-is** | 空白 → Empty，否则 Text([text])。私有辅助。 |
| 36 | `PaginationState` | type | src/addon/feishu/domain/bitable.rs:617 | **move-as-is** | 与载荷解耦（只吃计数与游标），所以记录/字段/表/视图四种列表共用同一套收敛判据。成功判据不是 fetched != 0。 |
| 37 | `PaginationState::new` | fn | src/addon/feishu/domain/bitable.rs:627 | **move-as-is** | max_pages 由调用方给：拉取传 MAX_PAGES(40)，宿主 worker 按轮次预算传自己的值。 |
| 38 | `PaginationState::accumulated` | fn | src/addon/feishu/domain/bitable.rs:639 | **move-as-is** | 累计行数。宿主用它判断「读成功但为空」这种歧义形态（可疑空快照）。 |
| 39 | `PaginationState::first_total` | fn | src/addon/feishu/domain/bitable.rs:643 | **move-as-is** | 首屏 total；list_all_records 用它填 RecordsSnapshot.total。 |
| 40 | `PaginationState::accept` | fn | src/addon/feishu/domain/bitable.rs:650 | **move-as-is** | 三条硬失败必须原样保留：① 页数超过 max_pages（:658-662）；② has_more=true 但 page_token 缺失或空串（:671-674，继续翻只会重复请求同一页）；③ 重复游标（:677-681，seen_tokens 去重，否则死循环到被频控拦下）。返回 None 表示耗尽。 |
| 41 | `PaginationState::assert_converged` | fn | src/addon/feishu/domain/bitable.rs:693 | **move-as-is** | 四条判据：has_more 必须已耗尽；首屏必须返回 total；total 不得为负；累计行数 ≥ total（用 ≥ 不用 ==，官方不承诺 total 等于分页行数之和，并发编辑会让 == 恒假把守卫变成噪声源 :688-692）。 |
| 42 | `RecordsSnapshot` | type | src/addon/feishu/domain/bitable.rs:714 | **move-as-is** | total 单独暴露是给调用方一个「拒绝空集」的机会：高级权限下 code=0/total=0/items=[] 也能通过收敛断言。宿主 pull.rs:30/1367 在用。 |
| 43 | `fetch_page` | fn | src/addon/feishu/domain/bitable.rs:727 | **move-as-is** | GET 形态的唯一重试入口（带 body 的写请求走 send_json）。可以留在 bitable 私有，不必上升为 api 层。 |
| 44 | `get_base_name` | fn | src/addon/feishu/domain/bitable.rs:748 | **move-as-is** | 只读展示名，不读记录内容。宿主 refresh_names.rs:7、approval_provision.rs:315、dispatch.rs:1186 在用。 |
| 45 | `is_document_link` | fn | src/addon/feishu/domain/bitable.rs:769 | **move-as-is** | 目前唯一消费者是 get_record_shared_url(:827)，所以留在 bitable 而不单开 url 模块。规则：https、≤4096 字节、无空白/控制字符/反斜杠、host 只允许 [A-Za-z0-9.-] 且以 .feishu.cn 或 .larksuite.com 结尾——这也挡住 userinfo 形式（feishu.cn@evil）。 |
| 46 | `get_record_shared_url` | fn | src/addon/feishu/domain/bitable.rs:788 | **move-as-is** | 走单条记录接口而不是批量接口：批量接口的 record_url 文档自相矛盾（:786-787）。只做读、不改分享设置。timeout 硬编码 2 秒（:818），与其他路径的 PULL_REQUEST_TIMEOUT_SECS 不同——搬走时原样保留，改它会改变线上超时行为。 |
| 47 | `send_json` | fn | src/addon/feishu/domain/bitable.rs:837 | **split** | 必须拆到 yang_feishu::api：它是「取 tenant token + 幂等标记 + 重试 + 信封解码 + token 失效清缓存强制刷新一次」这套通用飞书调用骨架，不属于 bitable。:847-849 明确不做递归：每页最多刷新一次 token。idempotent 恒 true（:868-872）。 |
| 48 | `decode_envelope` | fn | src/addon/feishu/domain/bitable.rs:898 | **split** | 拆到 yang_feishu::api。code=0 但 data 缺席必须报错，不能当空集（:909-916）。解析失败时带 outbound::summarize 原文摘要。 |
| 49 | `BitableCoordinates` | type | src/addon/feishu/domain/bitable.rs:929 | **move-as-is** | 必须是结构体而不是三个散参：三个值总是一起来自同一行数据源，拆开容易拼出「A 源的 app_token + B 源的 table_id」这种不报错只读错表的组合。宿主 4 个 domain 文件 + 2 个 worker + 8 个 action 用它做入参。 |
| 50 | `ParsedBitableLink` | type | src/addon/feishu/domain/bitable.rs:938 | **move-as-is** | 配置期链接解析产物：app_token 必有，table_id/view_id 可缺（只贴 Base 链接的旧输入仍合法）。宿主 create_config/create_datasource_table 直接消费。 |
| 51 | `parse_bitable_link` | fn | src/addon/feishu/domain/bitable.rs:945 | **move-as-is** | host 不参与判断（不同租户/私有化入口只要保留 /base/<token>?table=&view= 形状就能解析）。兼容旧的纯 Base Token 输入。http 与 https 都接受，但 /wiki/ 路径与 ../ 路径段被拒。link_tests(:1037-1072) 随迁。 |
| 52 | `resolve_bitable_link` | fn | src/addon/feishu/domain/bitable.rs:1013 | **move-as-is** | 合并完整链接与分字段输入；两边都给了但值不同必须报错，不能静默取一边。宿主 create_config.rs:30、create_datasource_table.rs:23、update_datasource_table.rs:22 在用。 |
| 53 | `first_records_page` | fn | src/addon/feishu/domain/bitable.rs:1080 | **move-as-is** | 诊断专用，故意不套 PaginationState、不按 MAX_PAGES 翻页——多页表上走收敛判据会把「探测成功」误报成「拉取失败」。宿主 pull_probe.rs:42 在用（page_size 传 PROBE_PAGE_SIZE=200）。 |
| 54 | `list_all_records` | fn | src/addon/feishu/domain/bitable.rs:1106 | **move-as-is** | 成功即返回已收敛快照；任何一页失败或未收敛都返回 Err，绝不返回「部分行 + 看起来成功」。每页固定 PAGE_SIZE=500。宿主 pull.rs 主路径。 |
| 55 | `record_row_numbers` | fn | src/addon/feishu/domain/bitable.rs:1172 | **move-with-refactor** | 重构：函数内 :1179 的 `const MAX_PAGES: u32 = 20;` 与模块级 MAX_PAGES(:49) 同名遮蔽，搬进 crate 后极易误读——改名为 ROW_SCAN_MAX_PAGES。行为语义原样保留：最佳努力，翻页结束/超上限/缺 page_token 都返回已找到的部分而不报错，全部命中即提前返回；行号只用于展示，稳定标识仍是 record_id。宿主 approval_row_edit.rs:96 在用。 |
| 56 | `list_all_views` | fn | src/addon/feishu/domain/bitable.rs:1230 | **move-as-is** | 注意参数形状与 list_all_tables/list_all_fields 不同：这里收两个裸路径段而不是 BitableCoordinates（视图配置期还没有完整的坐标行）。宿主 list_bitable_views.rs:15、health_check.rs 在用。 |
| 57 | `list_all_tables` | fn | src/addon/feishu/domain/bitable.rs:1279 | **move-as-is** | 配置向导第一步用：运维只填 app_token，表列表由这里取回。宿主 list_bitable_tables.rs:19、refresh_names.rs:7、approval_provision.rs:319 在用。 |
| 58 | `list_all_fields` | fn | src/addon/feishu/domain/bitable.rs:1325 | **move-as-is** | 宿主使用最广：dispatch.rs、update_config.rs、refresh_names.rs、health_check.rs、pull_probe.rs、approval_provision.rs、field_maintenance.rs、worker。 |
| 59 | `SEARCH_PAGE_SIZE` | const | src/addon/feishu/domain/bitable.rs:1380 | **move-as-is** | 《查询记录》的单页上限，与《列出记录》的 PAGE_SIZE 同值但出处不同（:1376-1379 明写不复用同一常量），保持独立。 |
| 60 | `BACKFILL_CHUNK` | const | src/addon/feishu/domain/bitable.rs:1392 | **move-with-refactor** | 重构：改名 BATCH_UPDATE_CHUNK。值 100 是两条约束的交集：batch_update 全有全无语义（批越小毒记录牵连越少）+ 官方对 1254607 的建议是降低批量大小；官方单次上限是 1000。它同时是宿主「折半拆批」的起点（拆分逻辑留在宿主 dispatch.rs:1953 与 :4498-4515 的测试）。 |
| 61 | `search_records_url` | fn | src/addon/feishu/domain/bitable.rs:1395 | **move-as-is** | 《查询记录》端点。宿主 dispatch.rs 的测试 :1871 与 e2e 断言它必须以 /records/search 结尾。 |
| 62 | `batch_update_records_url` | fn | src/addon/feishu/domain/bitable.rs:1404 | **move-as-is** | 宿主 dispatch.rs 的 e2e 测试 :1920-1922/3502-3530 直接调它拼 URL，所以要保持 pub。 |
| 63 | `BackfillRow` | type | src/addon/feishu/domain/bitable.rs:1413 | **move-with-refactor** | 重构：改名 UpdateRow。当前名字把审批派发的「回填」业务词带进库 API，而类型本身只是「记录 id + 要写的单元格」。 |
| 64 | `batch_update_records` | fn | src/addon/feishu/domain/bitable.rs:1421 | **move-as-is** | 全有全无：任一记录触发字段转换错误，整批非 0 code 且没有 per-record 失败列表。函数本身不做拆批（拆分在编排层，因为只有那里知道哪些记录同批）。宿主 approval_dispatch.rs:418/450 在用。 |
| 65 | `SearchRecordsData` | type | src/addon/feishu/domain/bitable.rs:1465 | **move-as-is** | 与 ListRecordsData 同形但字段顺序不同；page_token 同样必须是 Option（has_more=false 时不出现）。 |
| 66 | `search_records` | fn | src/addon/feishu/domain/bitable.rs:1482 | **move-as-is** | page_token/page_size 是查询参数而不是请求体字段（:1497-1498）；请求体只该有 filter/sort/field_names/automatic_fields。只发一页、不做收敛断言。 |
| 67 | `search_all_records` | fn | src/addon/feishu/domain/bitable.rs:1523 | **move-as-is** | 与 list_all_records 的差别要原样保留：不套 PaginationState、不校验 total、不看重复游标；只做「缺 page_token 即失败」(:1547-1557) + 「到 max_pages 仍未收敛即失败」(:1559-1562)。max_pages 是防失控（筛选条件写错会一页页拉全表拖死单线程 worker），不是分页参数。 |
| 68 | `pending_records_filter` | fn | src/addon/feishu/domain/bitable.rs:1573 | **move-with-refactor** | 重构：改名 pending_backfill_filter，其余逐字搬。它是 bitable filter DSL 的组装（isEmpty 的 value 必须是空数组否则吃 1254018；复选框 value 是字符串布尔 "true"、operator 只能 is；两条件用 and），没有宿主表/配置依赖。宿主 worker feishu_approval_worker.rs 在使用。若将来出现非「回填」语义的 pending 定义，应由宿主自己拼 filter，不要在这里加开关。 |
| 69 | `describe_unfetched` | fn | src/addon/feishu/domain/bitable.rs:1604 | **move-as-is** | forbidden（找管理员开高级权限）与 absent（记录已删）文案必须分开，合并会把用户引向错误方向。 |
| 70 | `get_records_by_ids` | fn | src/addon/feishu/domain/bitable.rs:1626 | **move-as-is** | 走 records/batch_get，不用 search+filter（record_id 是响应系统字段、不是可过滤字段）。请求体只发 record_ids/user_id_type=open_id/automatic_fields=true，不投影 field_names（该接口参数表没有它）。forbidden/absent 必须显式报错而不是返回空集。宿主 dispatch.rs、approval_dispatch.rs、worker 都在用。 |
| 71 | `SYSTEM_FIELD_KEYS` | const | src/addon/feishu/domain/bitable.rs:1693 | **move-as-is** | 这些系统字段没有 field_id；真机形态以实测为准，record 顶层与 fields 内两条路都收。rekey 的唯一豁免名单。 |
| 72 | `rekey_cells_by_field_id` | fn | src/addon/feishu/domain/bitable.rs:1722 | **move-as-is** | 官方 fields 按列名作键，本域按 field_id 取值；不做这一步的症状是「每条记录都缺少申请人」——不报错、永远停在等待态。未知键一律点名报错，不静默保留；唯一例外是 SYSTEM_FIELD_KEYS。宿主 dispatch.rs:1205、worker 在用。 |
| 73 | `ensure_ids` | fn | src/addon/feishu/domain/bitable.rs:1763 | **move-as-is** | 1~100 条、不得含空串。刻意不用 ensure!（它产出 anyhow::Error，失败分类必须在出站层保持 Fatal）。 |
| 74 | `BatchGetRecordsData` | type | src/addon/feishu/domain/bitable.rs:1789 | **move-as-is** | forbidden_record_ids 与 absent_record_ids 是两个独立数组，不能合并去重——describe_unfetched 靠它们分流文案。 |
| 75 | `encode_cursor` | fn | src/addon/feishu/domain/pagination.rs:22 | **stay-in-host** | 容易误判：这个 pagination.rs 不是 bitable 分页，而是飞书选项列表的 keyset 游标（TableQuery 是 page/page_size 语义）。游标内容绑定 option_id/sort_order 这两个宿主业务列，且返回 BaseError——留在宿主 domain/pagination.rs，不随 L1 迁移。 |
| 76 | `decode_cursor` | fn | src/addon/feishu/domain/pagination.rs:34 | **stay-in-host** | 留在宿主。与 bitable 的 PaginationState 无调用关系；两者都叫「分页」但一个编业务游标、一个收飞书 page_token。fail-closed 语义（非法游标报错而非当第一页）必须留在宿主。 |
| 77 | `DERIVE_RULE_VERSION` | const | src/addon/feishu/domain/derive.rs:71 | **stay-in-host** | 派生口径变更时必须手动 bump 并并入摘要输入；这是宿主持久化数据的兼容约定，crate 不感知。 |
| 78 | `truncate_label` | fn | src/addon/feishu/domain/derive.rs:99 | **stay-in-host** | 阈值 255 必须与宿主 option/table.rs 的列定义同步，属于产品表契约，不进飞书客户端 crate。xlsx 导入与飞书拉取共用同一口径。 |
| 79 | `RawValue` | type | src/addon/feishu/domain/derive.rs:115 | **stay-in-host** | 借用型祖先链 + 文案，是选项派生的输入形状；与 crate 的 CellValue 之间由宿主 pull.rs 的 extract_values_owned 桥接。 |
| 80 | `DerivedOption` | type | src/addon/feishu/domain/derive.rs:142 | **stay-in-host** | 派生结果行（option_id/label/parent_key/sort_order）；option_id 是宿主表级唯一索引的一部分。 |
| 81 | `label_option_id` | fn | src/addon/feishu/domain/derive.rs:169 | **stay-in-host** | 根级公式，只供绑定级 merge_same_label_options 开关使用；读端与派发的同文案映射靠它，属宿主语义。 |
| 82 | `ancestor_key` | fn | src/addon/feishu/domain/derive.rs:189 | **stay-in-host** | 沿祖先链逐级折叠父键；不变量是「子行 parent_key == 父行自己的 option_id」。与飞书 API 无关，纯宿主数据模型。 |
| 83 | `derive_options` | fn | src/addon/feishu/domain/derive.rs:224 | **stay-in-host** | 选项派生整块留宿主：option_id 是宿主表级唯一索引的一部分，哈希输入、截断口径、父键折叠规则都与产品数据模型绑定，不是飞书契约。 |
| 84 | `snapshot_digest` | fn | src/addon/feishu/domain/derive.rs:279 | **stay-in-host** | 内容摘要写回绑定行，决定「源没变就跳过落库」；绑定行的列，不是飞书概念。 |
| 85 | `FieldBindingInput` | type | src/addon/feishu/domain/field_binding.rs:17 | **stay-in-host** | 带 JsonSchema 派生，直接生成前端契约（frontend/contracts/openapi.json / api-types.ts）。crate 没有 schemars 依赖，也不该有。 |
| 86 | `has_parent_cycle` | fn | src/addon/feishu/domain/field_binding.rs:43 | **stay-in-host** | 防环检测，步数上限取节点数、刻意不递归（深链会爆栈）。输入是宿主绑定行输入，不是飞书字段元数据。 |
| 87 | `validate_fields` | fn | src/addon/feishu/domain/field_binding.rs:76 | **stay-in-host** | 创建与更新共用的唯一事实源，产出 BaseError::ParamInvalid（框架错误码），属于宿主配置面。 |
| 88 | `valid_source_key` | fn | src/addon/feishu/domain/source_key.rs:14 | **stay-in-host** | 进的是宿主 HTTP 路由路径 /api/v1/feishu/approval/options/{source_key}，是产品标识规则而非飞书契约。 |
| 89 | `PullDeps` | type | src/addon/feishu/domain/pull.rs:44 | **stay-in-host** | 拉取编排的入口依赖。搬走后它持有的是 crate 的 transport/sleeper/tokens，但 context/database 是宿主的。 |
| 90 | `BoundField` | type | src/addon/feishu/domain/pull.rs:59 | **stay-in-host** | 「已解析过名字的绑定」：field_id + 当前 field_name + 归一化的 parent_field_id。是宿主两步法（先解析名字再拼 field_names）的载体。 |
| 91 | `collect_field_names` | fn | src/addon/feishu/domain/pull.rs:78 | **stay-in-host** | 为 list_records_query 的 field_names 组参数（绑定列 + 父列并集、去重保序）。它知道「父列也要取」，这是宿主派生规则，不是客户端能力。 |
| 92 | `TableBinding` | type | src/addon/feishu/domain/pull.rs:105 | **stay-in-host** | 宿主绑定行在拉取视角的投影（id/source_key 是库列）。 |
| 93 | `PullTable` | type | src/addon/feishu/domain/pull.rs:122 | **stay-in-host** | 一张待拉取的表级数据源：宿主表级行主键 + 标题 + crate 的 BitableCoordinates + 启用中的绑定。 |
| 94 | `load_pull_tables` | fn | src/addon/feishu/domain/pull.rs:262 | **stay-in-host** | 读宿主两张表（数据源 + 绑定），不属客户端。 |
| 95 | `pull_table` | fn | src/addon/feishu/domain/pull.rs:441 | **stay-in-host** | 整轮表级拉取编排：解析列名 → 一次取回（调 crate 的 list_all_records）→ 逐字段派生 → 落库。这是「拉取编排留在宿主」的主体。 |
| 96 | `partition_bindings` | fn | src/addon/feishu/domain/pull.rs:936 | **stay-in-host** | 典型跨界点：入参字段元数据来自 crate 的 FieldItem、类型判定用 crate 的 is_option_source_field_type，但失败粒度（MissingField/DuplicateName/UnsupportedType/MultiValueParent/ParentUnavailable/ParentSkipped/ParentCycle）是宿主绑定语义。搬走 crate 后此函数只改 import，不搬实现。 |
| 97 | `parent_linkage` | fn | src/addon/feishu/domain/pull.rs:1074 | **stay-in-host** | 防环守卫（手工改库能造出 a→b→a，上溯不设 visited 会卡死 worker）。xlsx 导入复用同一份，搬走会把这个守卫复制两遍。 |
| 98 | `extract_values_owned` | fn | src/addon/feishu/domain/pull.rs:1142 | **stay-in-host** | 第二个典型跨界点：单元格归一化用 crate 的 cell_label/CellValue，但「非多选列返回多值即跳过并告警」的取值口径、祖先链折叠与 RawValue 构造都是宿主派生规则。留宿主，只改 import。 |
| 99 | `internal_error` | fn | src/addon/feishu/domain/pull.rs:1133 | **stay-in-host** | crate 的 OutboundFailure → 宿主 BaseError 的映射必须在宿主侧：Fatal 归 ConfigError、其余归 UpstreamUnavailable。crate 不该知道 BaseError 的变体语义。 |

### 需先重构

- **把 validate_path_segment(:66) 与 MAX_PATH_SEGMENT_LEN(:52) 上提到 crates/yang-feishu/src/path.rs，并在 bitable 内 `pub use crate::path::validate_path_segment;` 保留旧路径** — 它不是 bitable 语义：approval.rs:63 用它校验 instance_code，4 个 datasource action 用它校验用户输入的 base/table/view id。留在 bitable 会让「审批模块依赖多维表格模块」成为编译期事实。（`src/addon/feishu/domain/bitable.rs:52,66`）
- **把 FeishuApiEnvelope(:186)、decode_envelope(:898)、send_json(:837) 上提到 crates/yang-feishu/src/api.rs** — 这是「取 tenant token → 幂等标记 → 重试 → 信封解码 → token 失效清缓存重试一次」的通用飞书调用骨架。宿主已有两份重复的信封实现（approval.rs:681 的局部 Envelope、user_token.rs:458 的局部 Envelope），留在 bitable 会让第三份继续分裂。（`src/addon/feishu/domain/bitable.rs:186,837,898`）
- **搬走后删除文件级 #![allow(dead_code)]（:29）** — 整文件豁免会把 resolve_current_field_names(:510) 这种全仓库零调用点的死代码永久藏起来。crate 里应改为函数级 #[allow(dead_code)] 或直接删该函数，让新出现的死代码能被 clippy -D warnings 抓住。（`src/addon/feishu/domain/bitable.rs:29,510`）
- **record_row_numbers 内 :1179 的局部 `const MAX_PAGES: u32 = 20;` 改名 ROW_SCAN_MAX_PAGES** — 它与模块级 MAX_PAGES(:49)=40 同名遮蔽，搬进同一 crate 模块后阅读者会把两个上限混为一谈；两者语义不同（一个是整轮拉取上限，一个是行号扫描的尽力上限）。（`src/addon/feishu/domain/bitable.rs:1179`）
- **list_fields_query(:166) 改名 list_page_query** — 它同时服务列出字段(:1343)、列出数据表(:1295)、列出视图(:1247)。名字带 fields 会让人以为表和视图另有分页实现，改错时只改一处。（`src/addon/feishu/domain/bitable.rs:166`）
- **BACKFILL_CHUNK(:1392) → BATCH_UPDATE_CHUNK，BackfillRow(:1413) → UpdateRow，pending_records_filter(:1573) → pending_backfill_filter** — 「回填」是审批派发的产品词（宿主 approval_dispatch.rs），不应出现在通用飞书客户端 crate 的公开 API 上。三个都是纯重命名，零行为变化。（`src/addon/feishu/domain/bitable.rs:1392,1413,1573`）
- **删除 tenant_token.rs:271 的 `crate::authorization::validate_deployment_name(deployment)` 宿主耦合，改为由调用方传入/校验或在 crate 内自实现键形状校验** — TenantTokenProvider 是 bitable 每个出站函数的必需依赖；它构造时调宿主模块 = crate 无法脱离 yang-system 编译。这是 L0（auth/transport）的阻塞项，L1 依赖它先完成。（`src/addon/feishu/domain/tenant_token.rs:266-272`）
- **outbound::OutboundTransport::send 返回 yang_base::BaseError（outbound.rs:172），crate 版要换成 crate 自己的错误类型（thiserror 枚举），宿主在实现该 trait 时做映射** — 否则 yang-feishu 必须依赖 yang-base，与「平台能力 crate」的定位矛盾；且 BaseError 的变体语义（RateLimitExceeded/UpstreamUnavailable/ConfigError）是宿主框架概念。（`src/addon/feishu/domain/outbound.rs:170-173`）
- **HttpClientTransport(:176-294) 留在宿主（或 feature-gate），不搬进 crate** — 它依赖 yang_base::http::HttpClient、yang_db::RedisClient，并在 :211-223 硬编码「审批创建共享限流」（调 approval_rate_limit::acquire_create_slot）——这是审批产品语义，不是传输层。crate 只搬 trait + send_with_retry + 失败分类。（`src/addon/feishu/domain/outbound.rs:176-294`）
- **PULL_RETRY(:569) / PULL_REQUEST_TIMEOUT_SECS(:577) 在 crate 里改名 DEFAULT_RETRY / DEFAULT_REQUEST_TIMEOUT_SECS（或保留并补文档）** — 这两个常量现在同时被读路径（list_all_records）和写路径（batch_update_records:1457、search_records:1511）使用，名字里的 PULL 已经不准。（`src/addon/feishu/domain/outbound.rs:569,577`）
- **（可选，非搬迁前置）四个高度重复的分页循环合并成一个泛型 paginate_all：list_all_records:1126-1153、list_all_views:1246-1267、list_all_tables:1294-1315、list_all_fields:1342-1363** — 四段循环体逐字相同（query→fetch_page→accept→extend→break），只有 URL/查询/DTO 不同。合并后新增列表端点不会漏掉收敛断言。不做也不影响搬迁，属搬家后的清理。（`src/addon/feishu/domain/bitable.rs:1126-1153,1246-1267,1294-1315,1342-1363`）
- **（可选）approval.rs:86-98 的局部 validate_path_segment 与 crate 版合并时，注意两版判据不同：crate 版是白名单（ASCII 字母数字 + - _），宿主版是黑名单（只拒 / ? # 空格 \）** — 直接合并成严格版会收紧 approval 的入参校验（例如含 . : @ 的 approval_code 会从「放行」变「拒绝」）。合并前要先确认存量 instance_code/approval_code 全部落在白名单内，否则保持两版并存。这是 behavior change，不是纯重构。（`src/addon/feishu/domain/approval.rs:86-98`）

### 目标公开 API

| 名称 | 签名 | 备注 |
|---|---|---|
| `yang_feishu::bitable (module)` | `pub mod bitable;` | 单文件模块，承载 crates/yang-feishu/src/bitable.rs。所有原 pub(crate) 项升为 pub；内部实现细节（union_text/classify_text/describe_unfetched/ensure_ids/fetch_page）保持私有。 |
| `BitableCoordinates` | `pub struct BitableCoordinates { pub app_token: String, pub table_id: String, pub view_id: Option<String> }  // Debug + Clone + PartialEq + Eq` | 所有读写函数的坐标入参。宿主 14 个文件引用它。 |
| `ParsedBitableLink` | `pub struct ParsedBitableLink { pub app_token: String, pub table_id: Option<String>, pub view_id: Option<String> }  // Debug + Clone + PartialEq + Eq` | 配置期链接解析产物。 |
| `parse_bitable_link / resolve_bitable_link` | `pub fn parse_bitable_link(raw: &str) -> anyhow::Result<ParsedBitableLink> pub fn resolve_bitable_link(base_token: &str, table_id: Option<&str>, view_id: Option<&str>) -> anyhow::Result<ParsedBitableLink>` | 错误信息是运维直接看的文案（「多维表格链接路径必须是 /base/<token>」「完整链接中的 bitable_table_id 与单独字段不一致」），逐字保留。 |
| `URL builders` | `pub fn records_url(app_token: &str, table_id: &str) -> anyhow::Result<String> pub fn fields_url(app_token: &str, table_id: &str) -> anyhow::Result<String> pub fn views_url(app_token: &str, table_id: &str) -> anyhow::Result<String> pub fn tables_url(app_token: &str) -> anyhow::Result<String> pub fn search_records_url(app_token: &str, table_id: &str) -> anyhow::Result<String> pub fn batch_update_rec…` | 每个都先内部校验路径段。base 前缀来自 crate 常量 FEISHU_OPEN_BASE（现放 tenant_token.rs:40，搬进 api 模块）。 |
| `query builders` | `pub fn list_records_query(field_names: &[String], view_id: Option<&str>, page_token: Option<&str>, page_size: u32) -> anyhow::Result<Vec<(String, String)>> pub fn list_page_query(page_token: Option<&str>) -> Vec<(String, String)>` | list_page_query 即原来的 list_fields_query（每页 100）。返回键值对而不是拼好的查询串：百分号编码交给传输层。 |
| `DTOs` | `pub struct RecordItem { pub record_id: String, pub fields: BTreeMap<String, serde_json::Value>, pub created_by: Option<RecordPerson> } pub struct RecordPerson { pub id: String, pub name: Option<String> } pub struct ListRecordsData / BitableTablesData / BitableTableItem / BitableViewsData / BitableViewItem / ListFieldsData / FieldItem / FieldProperty / PropertyOption / SearchRecordsData / BatchGetR…` | 全部 #[derive(Debug, Clone, Deserialize)]（RecordItem 额外 PartialEq）。不要给它们实现 Default——「默认空快照」正是要拒绝的形态。 |
| `PaginationState` | `pub struct PaginationState; impl PaginationState {   pub fn new(max_pages: u32) -> Self;   pub fn accept(&mut self, has_more: bool, page_token: Option<String>, total: Option<i64>, item_count: usize) -> anyhow::Result<Option<String>>;   pub fn assert_converged(&self) -> anyhow::Result<()>;   pub fn accumulated(&self) -> usize;   pub fn first_total(&self) -> Option<i64>; }` | 契约守卫的单一实现点：500/100 每页上限、MAX_PAGES、重复游标、has_more 缺 page_token 的语义全部在这里和它的调用点保留。 |
| `CellValue / cell_label` | `pub enum CellValue { Text(Vec<String>), Empty, Unsupported } pub fn cell_label(value: &serde_json::Value) -> CellValue` | 三态不可合并成 Option<Vec<String>>：Unsupported 是「列类型不支持」，必须能被上层看见并拒绝。 |
| `field type predicates` | `pub fn is_single_value_field_type(field_type: i32) -> bool pub fn is_option_source_field_type(field_type: i32) -> bool pub fn check_coordinate_field(field: &FieldItem) -> anyhow::Result<String>` | 前两个是纯函数，宿主 pull/field_maintenance/health_check 都在吃第三个的错误文案。 |
| `field name resolution` | `pub fn resolve_field_name(fields: &[FieldItem], field_id: &str) -> anyhow::Result<String> pub fn resolve_field_names(fields: &[FieldItem], field_ids: &[String]) -> anyhow::Result<Vec<(String, String)>> pub fn rekey_cells_by_field_id(fields: &[FieldItem], cells: &BTreeMap<String, serde_json::Value>) -> anyhow::Result<serde_json::Map<String, serde_json::Value>>` | 「存 field_id、按当前名字取数」纪律的运行期实现；rekey 的未知键 fail-loud 必须保留。 |
| `SYSTEM_FIELD_KEYS` | `pub const SYSTEM_FIELD_KEYS: &[&str]` | rekey 的唯一豁免名单。 |
| `read clients` | `pub async fn first_records_page(transport: &dyn OutboundTransport, sleeper: &dyn Sleeper, tokens: &TenantTokenProvider, coordinates: &BitableCoordinates, field_names: &[String], page_size: u32) -> Result<ListRecordsData, OutboundFailure> pub async fn list_all_records(transport: &dyn OutboundTransport, sleeper: &dyn Sleeper, tokens: &TenantTokenProvider, coordinates: &BitableCoordinates, field_name…` | 统一形状：&dyn OutboundTransport + &dyn Sleeper + &TenantTokenProvider 前三个参数。刻意不包成 BitableClient 结构体——只会在每个宿主调用点多一层解引用，不减少任何参数。 |
| `search & batch get` | `pub async fn search_records(...body: serde_json::Value, page_token: Option<&str>) -> Result<SearchRecordsData, OutboundFailure> pub async fn search_all_records(...body: serde_json::Value, max_pages: u32) -> Result<Vec<RecordItem>, OutboundFailure> pub async fn get_records_by_ids(...record_ids: &[String]) -> Result<Vec<RecordItem>, OutboundFailure>` | search_all_records 不套 PaginationState（行为差异是刻意的）；get_records_by_ids 单次 1~100 条。 |
| `write client` | `pub type UpdateRow = (String, BTreeMap<String, serde_json::Value>); pub const BATCH_UPDATE_CHUNK: usize = 100; pub async fn batch_update_records(transport: &dyn OutboundTransport, sleeper: &dyn Sleeper, tokens: &TenantTokenProvider, coordinates: &BitableCoordinates, rows: &[UpdateRow]) -> Result<(), OutboundFailure>` | 全有全无；拆批与折半重试留在宿主。 |
| `filter builder` | `pub fn pending_backfill_filter(backfill_field_name: &str, batch_scope_field_name: Option<&str>) -> serde_json::Value` | 返回完整请求体的 filter 部分（含 automatic_fields:false）。宿主 worker 直接当 search_all_records 的 body 传。 |
| `constants` | `pub const PAGE_SIZE: u32 = 500; pub const SEARCH_PAGE_SIZE: u32 = 500; pub const MAX_PAGES: u32 = 40; pub const BATCH_UPDATE_CHUNK: usize = 100;` | 四个上限各自独立，注释里写明出处。PAGE_SIZE_FIELDS=100 保持私有（只经 list_page_query 体现）。 |
| `dependency layer (pre-existing, must exist first)` | `yang_feishu::{outbound::{OutboundTransport, OutboundRequest, OutboundResponse, OutboundMethod, Sleeper, OutboundFailure, FailureKind, RetryPolicy, send_with_retry, PULL_RETRY, PULL_REQUEST_TIMEOUT_SECS}, tenant_token::TenantTokenProvider, api::{FEISHU_OPEN_BASE, FeishuApiEnvelope, decode_envelope, send_json}, path::validate_path_segment}` | bitable 模块的编译前置：这些必须先由 L0 节点（transport/auth/公共 api）落到同一 crate。L1 只负责把 bitable.rs 内私有的 Envelope/send_json/validate_path_segment 提上去并改引用。 |

### 测试处置（20）

| 文件 | 区间 | 内容 | 处置 |
|---|---|---|---|
| src/addon/feishu/domain/bitable.rs | 1037-1072 | link_tests：完整链接解析（host 不参与）、缺 table/view 时保留最近坐标、非 /base/ 路径与 ../ 拒绝 | **move** |
| src/addon/feishu/domain/bitable.rs | 1804-1828 | unfetched_records_explain_absent_and_forbidden_separately：forbidden/absent 文案必须分开 | **move** |
| src/addon/feishu/domain/bitable.rs | 1830-1858 | feedback_document_links_reject_untrusted_urls：is_document_link 的正反例（含 feishu.cn@evil、协议、长度上限） | **rewrite** |
| src/addon/feishu/domain/bitable.rs | 1863-1933 | 写路径 URL/filter/批量上限：batch_update_url_targets_the_batch_update_endpoint、search_url_targets_the_search_endpoint、pending_records_filter 两例、batch_update_rejects_unsafe_path_segments、backfill_chunk_supports_halving_to_a_single_record | **move** |
| src/addon/feishu/domain/bitable.rs | 1937-1970 | 列出数据表：URL 形状、路径穿越拒绝、page_token 缺失可解析 | **move** |
| src/addon/feishu/domain/bitable.rs | 1972-2058 | field_id → 当前字段名：批量点名全部缺失、改名不断链、保序、重名拒绝、空输入、空名 | **move** |
| src/addon/feishu/domain/bitable.rs | 2065-2212 | URL/查询参数形状：views/fields/records_url 的路径穿越、field_names 单 JSON 数组、page_size=500、page_token 原样透传、view_id 校验、page_size=0 拒绝 | **move** |
| src/addon/feishu/domain/bitable.rs | 2214-2413 | record_row_numbers 三个 async 用例 + ScanTransport/ScanCache/NoSleep 辅助设施（:2216-2325） | **move** |
| src/addon/feishu/domain/bitable.rs | 2417-2482 | created_by 顶层解析、缺 created_by 仍可解析、fields 里的系统字段被跳过而不是报未知键 | **move** |
| src/addon/feishu/domain/bitable.rs | 2483-2620 | 字段解析与类型判定：精确 id、缺失报错、重名拒绝、空名拒绝；单值类型集合、多选可作来源但不可作父、无文本/对象类型拒绝、缺 field_type 放行、错误信息带可读类型名 | **move** |
| src/addon/feishu/domain/bitable.rs | 2622-2735 | cell_label 全谱：字符串/数字字符串化/长度 1 数组单选/多选逐元素/混合数组 Unsupported/公式 lookup 取 text/空值三态/人员与地理位置 Unsupported/布尔 Unsupported | **move** |
| src/addon/feishu/domain/bitable.rs | 2733-2825 | PaginationState 六个用例：两页收敛、缺游标必须失败、重复游标必须失败、页数上限、缺/负/截断 total 拒绝、并发新增容忍（≥ 判据） | **move** |
| src/addon/feishu/domain/bitable.rs | 2826-2853 | DTO 形状：ListRecordsData 无 page_token 可解析、ListFieldsData 分页字段 | **move** |
| src/addon/feishu/domain/bitable.rs | 2855-2862 | envelope_parses_with_data：FeishuApiEnvelope 解析 | **rewrite** |
| src/addon/feishu/domain/pull.rs | 1364-2059 | 拉取编排测试：partition_bindings 跳过原因、父链传播/成环、collect_field_names 去重保序、extract_values_owned 取值口径、补集停用与可疑空快照 | **stay** |
| src/addon/feishu/domain/derive.rs | 296-755 | 选项派生：option_id 形状、父键沿祖先链折叠、截断与摘要、按 id 去重 | **stay** |
| src/addon/feishu/domain/field_binding.rs | 146-258 | 绑定校验：父链成环检测、字段 id 唯一、source_key 形态 | **stay** |
| src/addon/feishu/domain/source_key.rs | 24-57 | source_key 合法形态（大小写、下划线、长度、非 ASCII） | **stay** |
| src/addon/feishu/domain/pagination.rs | 48-128 | 选项游标编解码往返、非法游标 fail-closed、游标不泄露原文 | **stay** |
| tests/feishu_approval_dispatch_integration.rs | 507-961 | 审批派发集成测试（含 bitable_field_name 列断言） | **stay** |

### 宿主替换点（10）

- `src/addon/feishu/domain/mod.rs:11` → 删除 `pub(crate) mod bitable;`，改为 `pub(crate) use yang_feishu::bitable;`。这样 src 下 22 处 `use crate::addon::feishu::domain::bitable::{...}` 与 `use super::bitable::{...}` 全部继续编译，宿主 diff 只有这一行。
- `project/yang-system/Cargo.toml:8` → 在 yang-runtime 之后追加：`yang-feishu = { version = "0.1.0", path = "../../crates/yang-feishu", default-features = false }`。与现有 yang-base/yang-db/yang-runtime 的 path 依赖写法一致。
- `src/addon/feishu/domain/bitable.rs:1-2863` → 整文件删除。内容去向：生产代码 → crates/yang-feishu/src/bitable.rs（约 1763 行）、公共价格层 → crates/yang-feishu/src/{api.rs,path.rs}、测试 → 同 crate 对应模块的 #[cfg(test)]。
- `src/addon/feishu/domain/approval.rs:63` → `super::bitable::validate_path_segment("instance_code", instance_code)?` → `yang_feishu::path::validate_path_segment(...)`。若保留 bitable 内的兼容 re-export，则此行可不动。
- `src/addon/feishu/domain/approval.rs:86-98` → 宿主侧重复实现的 validate_path_segment（黑名单版）可删除并改用 crate 白名单版——但这是收紧校验的行为变更，需先确认存量 instance_code/approval_code 全在 `[A-Za-z0-9_-]` 内；不确定就保留原函数，只把两处注释里的「与 bitable 同源」改成指向 yang_feishu::path。
- `src/infrastructure/feishu_pull.rs:52-54` → `const MAX_PAGES_PER_ROUND: u32 = 40;` → `const MAX_PAGES_PER_ROUND: u32 = yang_feishu::bitable::MAX_PAGES;`，消除两份手抄常量。
- `src/addon/feishu/domain/pull.rs:29-32` → `use super::bitable::{cell_label, is_option_source_field_type, list_all_fields, list_all_records, BitableCoordinates, CellValue, FieldItem, RecordsSnapshot};` → 因 mod.rs 的 re-export 可原样保留；若不做 re-export，则整段替换为 `use yang_feishu::bitable::{...};`（同名项）。
- `src/addon/feishu/domain/outbound.rs：全文` → L0 完成后替换为 `pub(crate) use yang_feishu::outbound::*;`（trait/失败分类/重试策略），HttpClientTransport + TokioSleeper + 审批创建限流留在宿主作 trait 实现。
- `src/addon/feishu/domain/tenant_token.rs：全文` → L0 完成后替换为 `pub(crate) use yang_feishu::tenant_token::*;`；RedisTenantTokenCache 若依赖 yang_db::RedisClient，则作为 feature 或留在宿主实现 TenantTokenCache trait。
- `src/addon/feishu/domain/approval_attachment.rs:3 / approval_dispatch.rs:41 / approval_feedback.rs:8 / approval_match.rs:49,1035 / approval_provision.rs:39 / approval_row_edit.rs:11 / field_maintenance.rs:19 / pull.rs:1367 / approval/actions/*.rs / datasource/actions/*.rs / infrastructure/feishu_approval_worker.rs:76 / infrastructure/feishu_event_worker.rs:27` → 零改动（依赖 domain/mod.rs 的 pub(crate) use re-export）。后续可逐个改为 yang_feishu::bitable::*，但不必与搬迁同批做。

### 风险

- 阻塞项（L0 必须先落地）：TenantTokenProvider::new 在 src/addon/feishu/domain/tenant_token.rs:271 调用 crate::authorization::validate_deployment_name。bitable 的每个出站函数都要求 &TenantTokenProvider，这个宿主耦合不清掉，crate 无法独立编译。
- 阻塞项：OutboundTransport::send 的返回类型是 yang_base::BaseError（outbound.rs:172）。若接受 yang-feishu 依赖 yang-base，可原样搬；若要保持 crate 独立，需要先定义 crate 错误枚举并在宿主 trait 实现里映射——两种都会改变宿主代码，需在 L0 定调。
- HttpClientTransport（outbound.rs:176-294）混着传输实现、yang_db::RedisClient 与审批创建限流（:211-223）。不要顺手搬进 crate：它把审批产品语义塞进传输层，且会让 yang-feishu 依赖 yang-db。
- validate_path_segment 有两版不等价实现（bitable.rs:66 白名单 vs approval.rs:86 黑名单）。合并会收紧 approval 的入参校验，可能把线上含 `.` `:` `@` 的 approval_code 判为非法——合并前必须先查存量数据，否则保留两版。
- PaginationState 的三条硬失败是生产事故的防线，搬动时不可「优化」：重复游标（:677-681）与 has_more 缺 page_token（:671-674）如果退化为 break，会让拉取在半份快照上「成功」，随后补集停用把有效选项整列停掉。测试 2733-2825 必须逐条随迁并通过。
- search_all_records（:1523）刻意没有套 PaginationState：它不校验 total、不查重复游标，只靠 max_pages 兜底。别在搬迁时「统一」成 PaginationState——那会改变审批派发 pending 扫描的失败语义。
- record_row_numbers（:1172）与其它 list 函数的失败语义相反：缺 page_token / 到上限 / 翻页结束都返回已找到的部分而不报错。统一成「硬失败」会让消息渲染在长表上直接失败。
- 四个上限常量不可合并：PAGE_SIZE=500（列出记录）、PAGE_SIZE_FIELDS=100（字段/表/视图）、SEARCH_PAGE_SIZE=500（查询记录）、BATCH_UPDATE_CHUNK=100（写）。同值不等于同源，文件头 :21-27 与 :1376-1379 已写明两个 500 出处不同。
- 后台允许的 page_size 上限与服务端实际表现可能漂移：官方对 1254030 TooLargeResponse 的建议是降低 page_size，本模块刻意不做自适应回退，而是把 1254030 归为不可重试。搬迁时不要给 send_with_retry 加自动降页逻辑，否则失败分类会变。
- 全部错误/告警文案是中文且是宿主 UI 与运维的直接输出（例如 check_coordinate_field 的类型不可用说明、resolve_field_names 的「字段名在表内不唯一」）。搬家必须逐字复制，任何翻译或重排都会让宿主断言（如 tests 中 contains("记录不存在")）与线上排障线索同时失效。
- bitable.rs:29 的 #![allow(dead_code)] 会随文件搬过去；若照搬，crate 内的新死代码将永远不报警。搬之前必须处理（删 allow + 删 resolve_current_field_names，或函数级 allow）。
- FEISHU_OPEN_BASE 硬编码为 https://open.feishu.cn（tenant_token.rs:40），而 is_document_link 同时接受 *.larksuite.com。搬到通用 crate 后这个不对称更显眼：私有化/Lark 租户无法切换域名。建议本次不做（保持行为），但在 crate 文档里标注为已知限制。
- get_record_shared_url 的 timeout 硬编码 2 秒（:818），与其它路径的 PULL_REQUEST_TIMEOUT_SECS=20 不同。它看起来像笔误，但改它等于改线上超时行为——搬迁时原样保留，另开 issue 再评估。
- 宿主的 domain/mod.rs re-export（pub(crate) use yang_feishu::bitable;）只在宿主侧有效；若将来有人在 yang-system 里写 `pub use`，会把 crate 类型泄漏进宿主公开 API。保持 pub(crate)。
- MSRV 1.80 / edition 2021 约束（workspace 根 + CI 的 MSRV job）：bitable 现有代码没有用到更高版本特性，但新增 crate 的 Cargo.toml 必须沿用 workspace 的 rust-version 与依赖版本，锁死 serde_json 1.0.140 等，否则 full CI 的 MSRV job 会红。
- 测试规模：约 1100 行测试整体随迁后，crate 的单测数量会从 0 跳到 ~60。搬迁 PR 要同步把这些测试纳入 run_ci.py 的分 crate test 名单（crates/* 自动包含，但 integration 那组仍只跑 yang-system）。

## A.4 yang-feishu::approval（L1：飞书审批 v4 客户端 + 幂等 uuid + 创建限速）

**目标**：把 approval v4 的 URL 构造、建单/查单/审批定义/事件订阅客户端、幂等 uuid 派生、创建限速闸门抽到 crates/yang-feishu 的 approval 模块；派发编排、状态机、回填、执行权、配置、审计与权限键全部留在宿主 yang-system。当前工作区为只读调研，行号以 D:/code/lib_yang/project/yang-system 现有内容为准。

### 源文件

| 文件 | 生产行区间 | 生产行数 | 测试行区间 | 测试行数 |
|---|---|---:|---|---:|
| src/addon/feishu/domain/approval.rs | 1-843 | 843 | 845-1550 | 706 |
| src/addon/feishu/domain/approval_uuid.rs | 1-72 | 72 | 74-149 | 76 |
| src/addon/feishu/domain/approval_rate_limit.rs | 1-67 | 67 | 68-205 | 138 |
| src/addon/feishu/domain/outbound.rs | 1-782 | 782 | 783-1664 | 882 |
| src/addon/feishu/domain/tenant_token.rs | 1-430 | 430 | 431-931 | 501 |
| src/addon/feishu/domain/outbound_setup.rs | 1-115 | 115 | 116-139 | 24 |
| src/addon/feishu/domain/approval_dispatch.rs | 1-1997 | 1997 | 1998-5088 | 3091 |
| src/addon/feishu/domain/approval_execution/transport.rs | 1-237 | 237 |  |  |
| src/addon/feishu/domain/approval_execution/mod.rs | 1-82 | 82 | 83-1098 | 1016 |
| src/addon/feishu/domain/approval_aggregation/mod.rs | 1-128 | 128 | 129-221 | 93 |
| src/addon/feishu/domain/approval_provision.rs | 1-1501 | 1501 | 1502-2731 | 1230 |
| src/addon/feishu/domain/approval_feedback.rs | 1-256 | 256 | 257-535 | 279 |

### 符号（79）

| # | 符号 | 类型 | 位置 | 处置 | 备注 |
|---:|---|---|---|---|---|
| 1 | `approval` | module | src/addon/feishu/domain/approval.rs:1-27 | **split** | 文件头 27 行文档 + #![allow(dead_code)]。内容按职责拆成 crate 的 approval（URL/建单/查单/定义/订阅/共用发送）与 contact（部门、姓名搜索）；#![allow(dead_code)] 在公开化后删除。 |
| 2 | `APPROVAL_REQUEST_TIMEOUT_SECS` | const | src/addon/feishu/domain/approval.rs:40 | **move-as-is** | 20 秒，审批主链路（创建/详情/定义/订阅）共用；contact 有自己的 DEPARTMENT_REQUEST_TIMEOUT_SECS。 |
| 3 | `MAX_PATH_SEGMENT_LEN` | const | src/addon/feishu/domain/approval.rs:43 | **move-with-refactor** | 与 bitable.rs:52 同名同值（128），抽到 crate::transport::path 后只保留一份。 |
| 4 | `create_instance_url` | fn | src/addon/feishu/domain/approval.rs:46-48 | **move-as-is** | 纯客户端；被宿主测试用作请求定位（actions/dispatch.rs:3256,3584）。 |
| 5 | `instance_detail_url` | fn | src/addon/feishu/domain/approval.rs:54-59 | **move-as-is** | instance_id 既可是 instance_code 也可是 uuid（60012 反查路径），不要在 crate 内改名或收紧。 |
| 6 | `instance_applink` | fn | src/addon/feishu/domain/approval.rs:62-67 | **move-with-refactor** | 第 63 行调用的是 bitable 的校验器；crate 内不能依赖宿主 bitable，改为 crate::transport::validate_path_segment。宿主 notification/convert/dispatch 多处使用，必须保留函数。 |
| 7 | `approval_definition_url` | fn | src/addon/feishu/domain/approval.rs:70-75 | **move-as-is** | 纯客户端。 |
| 8 | `subscribe_url` | fn | src/addon/feishu/domain/approval.rs:78-83 | **move-as-is** | 当前私有、仅内部与测试用；随测试一起进 crate，无需宿主调用。 |
| 9 | `validate_path_segment` | fn | src/addon/feishu/domain/approval.rs:86-98 | **move-with-refactor** | 与 bitable.rs:66-78 是两份实现（审批版拒绝 / ? # 空格 \，bitable 版只允许 ASCII 字母数字-_）。抽到 crate::transport 时保留审批版语义，避免 instance_applink 行为回退；两版统一属后续清理。 |
| 10 | `CreateOutcome` | type | src/addon/feishu/domain/approval.rs:109-115 | **move-as-is** | 60012 的建模：UuidConflict 是「实例已建、响应丢失」的可恢复结果，不是 Err；crate 必须保留这个枚举，客户端不能把它折成 FailureKind::Fatal。 |
| 11 | `create_instance` | fn | src/addon/feishu/domain/approval.rs:121-176 | **move-with-refactor** | 依赖 TenantTokenProvider 具体类型（第 124 行）→ 改 &dyn TokenSource 或 crate::token::TenantTokenProvider。第 155 行 idempotent:true 是不变式（uuid 服务端幂等），测试钉住，不能改。 |
| 12 | `CreateInstanceData` | type | src/addon/feishu/domain/approval.rs:178-181 | **move-as-is** | 仅 {instance_code} 的反序列化载体；创建响应不含 serial_number，这点由后续 get_instance 补。 |
| 13 | `InstanceDetail` | type | src/addon/feishu/domain/approval.rs:205-249 | **move-with-refactor** | 字段全 pub(crate)，公开化时逐字段改 pub。第 231-239 行 department_name 不是 API 字段，是宿主的 contact 富化结果：crate 内保留字段但注明 host-owned；建议同时 derive Default，宿主 10+ 处结构体字面量（approval_dispatch.rs:4063-4405）可少改。 |
| 14 | `TaskNode` | type | src/addon/feishu/domain/approval.rs:255-276 | **move-as-is** | bot 形态 task_list[]；全字段 Option + serde(default)。宿主 pending_handlers（approval_dispatch.rs:1703）与测试直接消费，公开即可。 |
| 15 | `TimelineRecord` | type | src/addon/feishu/domain/approval.rs:279-293 | **move-as-is** | bot 形态 timeline[]。 |
| 16 | `GetOutcome` | type | src/addon/feishu/domain/approval.rs:296-306 | **move-as-is** | 1390003 的建模：NotFound 是结果不是 Err，宿主据此区分并发窗口与真实失败；Found 装箱是为 large_enum_variant，保持 Box。 |
| 17 | `get_instance` | fn | src/addon/feishu/domain/approval.rs:311-366 | **move-with-refactor** | 依赖 TenantTokenProvider（第 314 行）。instance_id 传 uuid 即 60012 反查；NotFound 映射在 363 行。 |
| 18 | `InstanceDetailData` | type | src/addon/feishu/domain/approval.rs:368-398 | **move-as-is** | 两形态（bot/user）并存的宽松反序列化；全部 #[serde(default)]，不要收严。 |
| 19 | `department_url` | fn | src/addon/feishu/domain/approval.rs:405-410 | **move-as-is** | 属于 contact/v3/departments，不是 approval v4；切到 crate::contact。 |
| 20 | `department_id_type` | fn | src/addon/feishu/domain/approval.rs:417-423 | **move-as-is** | 按 od- 前缀分流；纯函数，随 contact 一起。 |
| 21 | `resolve_department_name` | fn | src/addon/feishu/domain/approval.rs:437-501 | **move-with-refactor** | 依赖 TenantTokenProvider（第 440 行）；属 contact 模块。失败一律 None + warn，不冒泡——这是宿主回填退化写 ID 的前提。 |
| 22 | `ApprovalDefinition` | type | src/addon/feishu/domain/approval.rs:508-515 | **move-as-is** | approval_name/form/is_external；三方定义判据 is_external 由宿主 provisioning 使用。 |
| 23 | `get_approval_definition` | fn | src/addon/feishu/domain/approval.rs:518-552 | **move-with-refactor** | 依赖 TenantTokenProvider（第 521 行）。宿主调用点：approval_provision.rs:257、actions/list_widgets.rs:75、actions/refresh_names.rs:155、approval_dispatch.rs:621。 |
| 24 | `subscribe_approval` | fn | src/addon/feishu/domain/approval.rs:564-608 | **move-with-refactor** | 依赖 TenantTokenProvider（第 568 行）。1390007 当前映射 Fatal{code} 再在 588-607 转 Ok；建议改为返回 SubscribeOutcome::{Subscribed,AlreadySubscribed}，宿主只判 Err 的两个调用点（create_config.rs:286、feishu_event_worker.rs:207）无需改动。 |
| 25 | `ApprovalDefinitionData` | type | src/addon/feishu/domain/approval.rs:610-617 | **move-as-is** | is_external 缺省 Option<bool>，默认 false 在 get_approval_definition 第 550 行施加。 |
| 26 | `send_approval` | fn | src/addon/feishu/domain/approval.rs:635-673 | **move-with-refactor** | 依赖 TenantTokenProvider（第 638 行）。与 bitable::send_json（bitable.rs:837）是刻意重复的两份 token 失效补救；crate 内若两者都在，可收敛为一个 crate::transport::send_with_token_refresh，但非本步必需。 |
| 27 | `decode_data` | fn | src/addon/feishu/domain/approval.rs:676-693 | **move-as-is** | 纯函数，取信封 data 再序列化成文本；应放到 crate::transport 供 approval/contact 共用。 |
| 28 | `detail_query` | fn | src/addon/feishu/domain/approval.rs:700-702 | **move-as-is** | 显式 user_id_type=open_id，避免多要通讯录字段权限；随 get_instance 测试一起。 |
| 29 | `applicant_search_url` | fn | src/addon/feishu/domain/approval.rs:722-724 | **move-as-is** | 寄生在 approval.rs 的 contact/search 能力。宿主 actions/dispatch.rs:2703 直接断言该 URL，切走后必须保留同名导出。 |
| 30 | `ApplicantResolution` | type | src/addon/feishu/domain/approval.rs:727-735 | **move-as-is** | Resolved/NotFound/Ambiguous；宿主 actions/dispatch.rs:899 消费。 |
| 31 | `resolve_applicant` | fn | src/addon/feishu/domain/approval.rs:749-786 | **move-as-is** | 唯一不依赖 TenantTokenProvider 的客户端：bearer 由调用方给（user_access_token，105 行注释说明 tenant token 会 99991663）。切到 crate::contact。 |
| 32 | `ApplicantSearchData` | type | src/addon/feishu/domain/approval.rs:797-801 | **move-as-is** | items/users 双键 alias；元素字段名仍是漂移风险（注释 788-796）。 |
| 33 | `SearchUser` | type | src/addon/feishu/domain/approval.rs:807-813 | **move-as-is** | open_id + name/localized_name。 |
| 34 | `classify_applicants` | fn | src/addon/feishu/domain/approval.rs:821-843 | **move-as-is** | 纯函数，精确命中>1 判 Ambiguous、无精确但唯一模糊命中判 Resolved。属于 contact 搜索策略，不属 approval 域。 |
| 35 | `approval_uuid` | module | src/addon/feishu/domain/approval_uuid.rs:1-23 | **move-as-is** | 只有 sha2 依赖，无宿主耦合；文件头记录「不用 UUIDv5（避免 sha1 feature 与 MSRV）」「必须带 approval_code」。整体进 crate::approval::idempotency。 |
| 36 | `derive_uuid` | fn | src/addon/feishu/domain/approval_uuid.rs:36-72 | **move-with-refactor** | 第 44 行 format！("{base}\|{table}\|{code}\|{record}") 是持久化身份：feishu_approval_task.uuid / 组 uuid 都落库。泛化为 derive_idempotency_key(parts) 时必须保持四元组字节完全一致（仍用 \| join），否则蓝绿/存量任务无法按 uuid 回捞。 |
| 37 | `derive_idempotency_key` | fn | src/addon/feishu/domain/approval_uuid.rs:36-72（新增，替代 derive_uuid 的固定元组） | **move-with-refactor** | 实现 = parts.join("\|") 后取 SHA-256 前 16 字节，b[6]=0x80、b[8]=0x80，再格式化为 36 字符 UUID。derive_uuid(a,b,c,d) 退化为 derive_idempotency_key(&[a,b,c,d])。不要改成长度前缀（会改所有存量 uuid）；可在 debug 断言 parts 不含 \|。 |
| 38 | `approval_rate_limit` | module | src/addon/feishu/domain/approval_rate_limit.rs:1-67 | **move-as-is** | Redis TIME 匀速闸门，按 app 跨部署共享；进 crate::approval::rate_limit。 |
| 39 | `bucket_key` | fn | src/addon/feishu/domain/approval_rate_limit.rs:12-14 | **move-with-refactor** | 当前硬编码 yang-system:feishu:app:{app_id}:approval_create_gate；参数化为 {key_prefix}:app:{app_id}:approval_create_gate。宿主必须传 "yang-system:feishu" 才能保持既有键与跨部署共享语义。 |
| 40 | `min_interval_ms` | fn | src/addon/feishu/domain/approval_rate_limit.rs:17-19 | **move-as-is** | 向上取整；0 按最慢（60000ms）而非不限速。纯函数。 |
| 41 | `acquire_create_slot` | fn | src/addon/feishu/domain/approval_rate_limit.rs:22-67 | **move-with-refactor** | 改为 pre-send hook 的实现体：错误类型从 OutboundFailure 换成 BaseError::RateLimitExceeded{retry_after_seconds:1}，因为 send_with_retry（outbound.rs:716-722）已把该 BaseError 折成 Retry。每 attempt 领取一次，不能提到 send_with_retry 之外。 |
| 42 | `FailureKind` | type | src/addon/feishu/domain/outbound.rs:318-335 | **move-as-is** | 随 outbound 一起进 crate::transport。保留 UuidConflict（60012）与 InstanceNotFound（1390003）两个专用变体：disposition（630-646）靠它们直接 Give 且不重试，宿主 feedback（approval_feedback.rs:212-223）与 dispatch（approval_dispatch.rs:964-969、1776-1806）也穷举匹配。不要降级成 Fatal{code}。 |
| 43 | `CODE_UUID_CONFLICT` | const | src/addon/feishu/domain/outbound.rs:106 | **move-as-is** | 60012。放在 crate::transport::codes，由 approval 再导出；宿主 approval_execution/transport.rs:64,183 依赖它。 |
| 44 | `CODE_INSTANCE_NOT_FOUND` | const | src/addon/feishu/domain/outbound.rs:113 | **move-as-is** | 1390003。只用于 classify 与日志降噪（outbound.rs:757-766）。 |
| 45 | `CODE_SUBSCRIPTION_EXISTED` | const | src/addon/feishu/domain/outbound.rs:118 | **move-as-is** | 1390007。订阅与退订共用；subscribe_approval 按幂等成功处理，send_with_retry 把它降为 DEBUG（751-756），两处必须一起搬，否则每次启动约 49 条 WARN 回归。 |
| 46 | `classify` | fn | src/addon/feishu/domain/outbound.rs:345-401 | **move-as-is** | 随 transport 层移动。第 380-385 行是 60012/1390003 的判据，必须在 Fatal 兜底之前。 |
| 47 | `disposition` | fn | src/addon/feishu/domain/outbound.rs:630-656 | **move-as-is** | UuidConflict/InstanceNotFound/TokenExpired/Fatal → Give，审批的恢复决策全在上层；随 transport 移动。 |
| 48 | `send_with_retry` | fn | src/addon/feishu/domain/outbound.rs:670-781 | **move-with-refactor** | 第 681-682 行的 endpoint 标签把 approval 路径硬编码进通用发送层；第 751-766 行对 1390007/1390003 做日志与 failed 计数特判。抽 crate 时保留这些分支（crate 是飞书专用，不是通用 HTTP 库），否则日志噪声/告警口径回退。 |
| 49 | `HttpClientTransport` | type | src/addon/feishu/domain/outbound.rs:176-198 | **move-with-refactor** | 限速内嵌在 send 里。改为 pre_send: Option<Arc<dyn PreSendHook>> + with_pre_send_hook；with_approval_create_limit 删除。 |
| 50 | `HttpClientTransport::send` | fn | src/addon/feishu/domain/outbound.rs:201-294 | **move-with-refactor** | 第 203-223 行按 URL 后缀 /open-apis/approval/v4/instances + POST 匹配并调用闸门，缺配置时返回 ConfigError（fail-closed）。这段整块删除，改由 hook 在每次真实出站前执行；fail-closed 改由 crate 的 approval_transport(inner, gate) 构造器保证。 |
| 51 | `PULL_RETRY` | const | src/addon/feishu/domain/outbound.rs:569-574 | **move-as-is** | 3 次、500ms 起退避、Retry-After 上限 30s；创建与详情都用它。进 crate 后可改名 APPROVAL_RETRY，但非必需。 |
| 52 | `DEPARTMENT_RETRY` | const | src/addon/feishu/domain/outbound.rs:585-590 | **move-as-is** | max_attempts=1 的 best-effort 策略，唯一使用者是 resolve_department_name；应随 contact 模块走，不要留在通用 transport 常量里。 |
| 53 | `DEPARTMENT_REQUEST_TIMEOUT_SECS` | const | src/addon/feishu/domain/outbound.rs:593 | **move-as-is** | 5 秒；随 contact 模块。 |
| 54 | `OutboundFailure` | type | src/addon/feishu/domain/outbound.rs:516-519 | **move-as-is** | 宿主 dispatch 与 feedback 大量消费；公开化必需。 |
| 55 | `TenantTokenProvider` | type | src/addon/feishu/domain/tenant_token.rs:258-358 | **stay-in-host** | 依赖宿主授权命名空间（tenant_token.rs:276 crate::authorization::validate_deployment_name）与 Redis 缓存。L1 不搬它：approval 模块只通过 TokenSource 两方法（tenant_access_token / invalidate_and_refresh）使用；若后续 L0/L2 把 token 整体抽进 crate，再改为 crate::token::TenantTokenProvider。依赖它的 only 函数：creat… |
| 56 | `DispatchResult` | type | src/addon/feishu/domain/approval_dispatch.rs:47-71 | **stay-in-host** | 派发状态机（Backfilled/Waiting/Terminal/CreatedPendingBackfill/Retryable）绑定产品表与任务状态，不能进 crate。 |
| 57 | `DispatchInput` | type | src/addon/feishu/domain/approval_dispatch.rs:202-252 | **stay-in-host** | 依赖产品配置/表格字段/执行权/时区；只把其中 approval_code、form_snapshot 传给 crate 客户端。 |
| 58 | `Backfill` | trait | src/addon/feishu/domain/approval_dispatch.rs:255-287 | **stay-in-host** | 多维表格回写抽象，属产品侧。 |
| 59 | `dispatch_one` | fn | src/addon/feishu/domain/approval_dispatch.rs:476-888 | **stay-in-host** | 核心编排：落库时序、60012 回捞、serial_number 轮询、回填。宿主侧调用 crate 客户端的最大消费点。 |
| 60 | `dispatch_group` | fn | src/addon/feishu/domain/approval_dispatch.rs:998-1248 | **stay-in-host** | 聚合组：一次 create + 组 uuid 回捞 + 批量回填；状态机留宿主。 |
| 61 | `GroupOutcome` | type | src/addon/feishu/domain/approval_dispatch.rs:949-957 | **stay-in-host** | 组暂停/重试语义。 |
| 62 | `classify_group_failure` | fn | src/addon/feishu/domain/approval_dispatch.rs:960-978 | **stay-in-host** | 消费 FailureKind 的审批专用变体；仅导入路径随 crate 变化。 |
| 63 | `fill_department_name` | fn | src/addon/feishu/domain/approval_dispatch.rs:1482-1503 | **stay-in-host** | 宿主决定何时富化（配了部门列 + 已有编号才出站）；实现改为调 crate::contact::resolve_department_name，结果写回 InstanceDetail.department_name。 |
| 64 | `finish_backfill` | fn | src/addon/feishu/domain/approval_dispatch.rs:1505-1578 | **stay-in-host** | 沿用 crate::approval::instance_applink；其余是产品回填。 |
| 65 | `write_extended` | fn | src/addon/feishu/domain/approval_dispatch.rs:1581-1668 | **stay-in-host** | 把 InstanceDetail 映射成多维表格列；产品列语义不能进 crate。第 1653 行消费 department_name。 |
| 66 | `millis_of` | fn | src/addon/feishu/domain/approval_dispatch.rs:1671-1674 | **stay-in-host** | 字符串毫秒 → i64，"0" 视为空；产品日期列语义。 |
| 67 | `status_text` | fn | src/addon/feishu/domain/approval_dispatch.rs:1683-1700 | **stay-in-host** | PENDING/APPROVED/… → 中文 + (已撤销)；未知枚举不猜并 warn。展示文案属宿主。 |
| 68 | `pending_handlers` | fn | src/addon/feishu/domain/approval_dispatch.rs:1703-1725 | **stay-in-host** | task_list 中 PENDING 节点 open_id 去重 → 人员列 JSON；产品列语义。 |
| 69 | `flow_text` | fn | src/addon/feishu/domain/approval_dispatch.rs:1730-1752 | **stay-in-host** | 节点名 ✓/⏳/✗ 拼接；展示层。 |
| 70 | `serial_number_of` | fn | src/addon/feishu/domain/approval_dispatch.rs:1755-1762 | **stay-in-host** | 空串与缺失同义；宿主用它判 CreatedPendingBackfill。 |
| 71 | `classify_failure` | fn | src/addon/feishu/domain/approval_dispatch.rs:1768-1807 | **stay-in-host** | 失败 → 重试/终态；写回填列只发生在 Fatal。依赖 crate 的 FailureKind 五变体，crate 不能删变体。 |
| 72 | `sanitize_terminal_message` | fn | src/addon/feishu/domain/approval_dispatch.rs:1853-1870 | **stay-in-host** | 终态文案裁剪（读者范围远大于运维）。 |
| 73 | `ExecutionTransport` | type | src/addon/feishu/domain/approval_execution/transport.rs:10-129 | **stay-in-host** | 执行权/副作用确认，必须留宿主；但第 19-22 行用 URL 后缀识别 create，应改为 crate::approval::is_create_request，避免 crate 改 URL 后静默失效。 |
| 74 | `GroupExecutionTransport` | type | src/addon/feishu/domain/approval_execution/transport.rs:140-237 | **stay-in-host** | 同上；第 150-151 行同样改为 is_create_request。 |
| 75 | `group_key` | fn | src/addon/feishu/domain/approval_aggregation/mod.rs:63-71 | **stay-in-host** | 产品分组键（config_id + signature + 成员），不进 crate。 |
| 76 | `group_uuid` | fn | src/addon/feishu/domain/approval_aggregation/mod.rs:77-84 | **stay-in-host** | 保留薄包装，仅把 derive_uuid 的 import 改到 crate。 |
| 77 | `build_plan` | fn | src/addon/feishu/domain/approval_provision.rs:212-308 | **stay-in-host** | 配置建单/字段映射/三方定义校验：产品侧；第 257 行调用 crate 的定义客户端。 |
| 78 | `outbound_reason` | fn | src/addon/feishu/domain/approval_feedback.rs:212-223 | **stay-in-host** | 用户可读失败文案，穷举 FailureKind 五变体；crate 的 FailureKind 必须保持这些变体名。 |
| 79 | `outbound_setup::build` | fn | src/addon/feishu/domain/outbound_setup.rs:47-91 | **stay-in-host** | 第 61-68 行是生产限速接线点：with_approval_create_limit → with_pre_send_hook(ApprovalCreateGateHook)。另一处接线在 feishu_approval_worker.rs:244-250。 |

### 需先重构

- **Token 抽象解耦：approval 客户端不再依赖具体 TenantTokenProvider，改为 &dyn TokenSource（两个方法：tenant_access_token / invalidate_and_refresh）** — tenant_token.rs:276 依赖宿主的 crate::authorization::validate_deployment_name 与 Redis 缓存，属于宿主装配；crate 若直接持有具体类型就无法独立编译，也无法在测试里只伪造两方法（`src/addon/feishu/domain/approval.rs:121-176,311-366,437-501,518-552,564-608,635-673; src/addon/feishu/domain/tenant_token.rs:258-358`）
- **把 contact/v3/departments 与 search/v1/user 从 approval.rs 切到 crate::contact：department_url + department_id_type + resolve_department_name + applicant_search_url + ApplicantResolution + resolve_applicant…** — 这两组是通讯录域，不是审批 v4 契约；混在 approval.rs 会让 approval 模块被迫携带 user_access_token 语义与 DEPARTMENT_RETRY 常量，也无法被其他飞书能力复用（`src/addon/feishu/domain/approval.rs:404-501,704-843; 常量 src/addon/feishu/domain/outbound.rs:585-593`）
- **derive_uuid 泛化为 derive_idempotency_key(parts: &[&str])，derive_uuid 变成四元组薄包装；parts 用 '\|' join，保持四元组输出与现在逐字节一致** — 组派发（approval_aggregation/mod.rs:83）已经复用四元组，但后续若出现三元组/五元组调用不必再写新函数；同时 uuid 已持久化在 feishu_approval_task.uuid 与组行，换分隔规则会导致存量任务无法按 uuid 回捞（`src/addon/feishu/domain/approval_uuid.rs:36-72`）
- **限速键前缀参数化 + 把 outbound.rs:203-223 的内联闸门改成可选 pre-send hook** — key 里硬编码 yang-system:feishu，crate 化后不能假定宿主命名空间；闸门内嵌在 HttpClientTransport 里使 transport 与审批域耦合，且 with_approval_create_limit 未配置时对审批创建 fail-closed（211-214），改为 hook 后可显式选择是否安装，但需要用 crate 侧构造器保住 fail-closed（`src/addon/feishu/domain/approval_rate_limit.rs:12-67; src/addon/feishu/domain/outbound.rs:176-198,203-223; src/addon/feishu/domain/outbound_setup.rs:61-68; src/infrastructure/feishu_approval_worker.rs:244-250`）
- **路径段校验去重：approval.rs 自带一份、bitable.rs:66-78 另一份，instance_applink 还跨模块调 bitable 版** — crate 不能依赖宿主 bitable 模块；两份实现的字符集规则不同（审批版禁 /?#空格\，bitable 版只允许 ASCII 字母数字-_），统一时必须以审批版为准，否则 instance_applink 的入参规则收紧/放宽（`src/addon/feishu/domain/approval.rs:43,62-67,86-98; src/addon/feishu/domain/bitable.rs:52,66-78`）
- **宿主执行传输不再用 URL 后缀识别 create，改用 crate::approval::is_create_request(&request)** — URL 构造搬进 crate 后，宿主字符串匹配 /approval/v4/instances 会在 crate 调整路径（或 query 形态）时静默失效，直接后果是执行权阶段不落 creating，崩溃后无法回捞（`src/addon/feishu/domain/approval_execution/transport.rs:19-22,150-151; 日志端点标签 src/addon/feishu/domain/outbound.rs:681-682`）
- **InstanceDetail.department_name 明确为 host-owned 富化字段，并给 InstanceDetail 加 Default** — 该字段不在 API 响应里（355 行恒 None），由宿主 fill_department_name 填；公开后若不注明会被误当成飞书契约。加 Default 可让宿主 10+ 处结构体字面量（approval_dispatch.rs:4063-4405）少改（`src/addon/feishu/domain/approval.rs:231-239,342-360; src/addon/feishu/domain/approval_dispatch.rs:1482-1503,1653,4063-4405`）
- **subscribe_approval 把 1390007 从 Fatal{code} 特判升级为 SubscribeOutcome::AlreadySubscribed** — 当前 588-607 行在客户端里把 Fatal 变体重新解释成成功，语义藏在错误路径；显式枚举让宿主能区分「本次新订阅」与「已订阅/已取消」，同时 send_with_retry 的日志降级（outbound.rs:751-756）仍需保留以免启动噪声（`src/addon/feishu/domain/approval.rs:564-608; src/addon/feishu/domain/outbound.rs:658-664,751-756`）
- **send_approval 与 bitable::send_json 的 token 失效补救逻辑二选一：要么收敛为 crate::transport::send_with_token_refresh，要么保持私有重复但共享 TokenSource** — 两处各写一遍「失效→清缓存→强刷一次」，当前注释明确说故意不复用（错误文案不同）；crate 化后同处一个 crate，重复的成本开始大于收益，但合并会改宿主 bitable 的错误文案，可作为后续独立提交（`src/addon/feishu/domain/approval.rs:623-673; src/addon/feishu/domain/bitable.rs:837`）
- **宿主保留 shim 模块（approval.rs / approval_uuid.rs / approval_rate_limit.rs 只做 pub(crate) use 重导出），第一阶段不改 13 个消费文件的 import** — 宿主消费点横跨 dispatch/provision/actions/worker/execution/aggregation/notification，一次性改 import 会把抽取与业务改动混在同一 diff；shim 让 crate 落地可独立验证（`src/addon/feishu/domain/mod.rs:2,12,14; src/addon/feishu/domain/approval.rs:1-1550; src/addon/feishu/domain/approval_uuid.rs:1-149; src/addon/feishu/domain/approval_rate_limit.rs:1-205`）
- **删除 approval.rs 的 #![allow(dead_code)] 与 approval_uuid.rs 的同类属性** — 抽取后这些符号由宿主真实消费（目前注释说消费者后续接入，实际上 dispatch/worker 已在用）；公开化后不应再整体压制 dead_code（`src/addon/feishu/domain/approval.rs:27; src/addon/feishu/domain/approval_uuid.rs:23`）

### 目标公开 API

| 名称 | 签名 | 备注 |
|---|---|---|
| `approval::create_instance_url` | `pub fn create_instance_url() -> String` | POST /open-apis/approval/v4/instances。 |
| `approval::instance_detail_url` | `pub fn instance_detail_url(instance_id: &str) -> anyhow::Result<String>` | instance_id 可为 instance_code 或 uuid；先做路径段校验。 |
| `approval::instance_applink` | `pub fn instance_applink(instance_code: &str) -> anyhow::Result<String>` | 官方审批 AppLink（cli_9cb844403dbb9108），宿主通知与回填链接使用。 |
| `approval::approval_definition_url` | `pub fn approval_definition_url(approval_code: &str) -> anyhow::Result<String>` | GET /approval/v4/approvals/:code。 |
| `approval::subscribe_url` | `pub fn subscribe_url(approval_code: &str) -> anyhow::Result<String>` | POST /approval/v4/approvals/:code/subscribe。 |
| `approval::is_create_request` | `pub fn is_create_request(request: &OutboundRequest) -> bool` | 新增：method==POST 且去 query 后路径以 /open-apis/approval/v4/instances 结尾；宿主 ExecutionTransport 与限速 hook 共用，替代字符串散落匹配。 |
| `approval::CreateOutcome` | `pub enum CreateOutcome { Created { instance_code: String }, UuidConflict }` | 60012 不作为 Err：UuidConflict 表示实例已存在、需按 uuid 反查。 |
| `approval::GetOutcome` | `pub enum GetOutcome { Found(Box<InstanceDetail>), NotFound }` | 1390003 不作为 Err：NotFound 用于区分并发窗口与真实查询失败。 |
| `approval::SubscribeOutcome` | `pub enum SubscribeOutcome { Subscribed, AlreadySubscribed }` | 1390007 建模为幂等成功；宿主现有调用点只判 Err，可直接兼容。 |
| `approval::InstanceDetail` | `pub struct InstanceDetail { pub instance_code: String, pub approval_code: Option<String>, pub serial_number: Option<String>, pub status: Option<String>, pub start_time: Option<String>, pub end_time: Option<String>, pub open_id: Option<String>, pub user_id: Option<String>, pub department_id: Option<String>, pub department_name: Option<String>, pub uuid: Option<String>, pub reverted: Option<bool>, p…` | 以 bot 形态为准、user 形态缺省 None；department_name 是宿主 contact 富化值，不属于 API 响应。建议 derive Default。 |
| `approval::TaskNode` | `pub struct TaskNode { pub id, node_id, node_name, user_id, open_id, status, r#type, start_time, end_time: Option<String> }` | 全字段 Option + serde(default)，两形态可解析不炸。 |
| `approval::TimelineRecord` | `pub struct TimelineRecord { pub r#type, create_time, user_id, open_id, ext: Option<String> }` | 流程动态。 |
| `approval::ApprovalDefinition` | `pub struct ApprovalDefinition { pub approval_name: String, pub form: Option<serde_json::Value>, pub is_external: bool }` | 三方定义 is_external=true 时不可用 instances create。 |
| `approval::create_instance` | `pub async fn create_instance(transport: &dyn OutboundTransport, sleeper: &dyn Sleeper, tokens: &dyn TokenSource, approval_code: &str, form: &str, open_id: &str, uuid: &str) -> Result<CreateOutcome, OutboundFailure>` | form 必须是压缩后的 JSON 数组字符串；请求固定 idempotent=true。 |
| `approval::get_instance` | `pub async fn get_instance(transport: &dyn OutboundTransport, sleeper: &dyn Sleeper, tokens: &dyn TokenSource, instance_id: &str) -> Result<GetOutcome, OutboundFailure>` | 传 uuid 即 60012 反查路径；查询参数显式 user_id_type=open_id。 |
| `approval::get_approval_definition` | `pub async fn get_approval_definition(transport: &dyn OutboundTransport, sleeper: &dyn Sleeper, tokens: &dyn TokenSource, approval_code: &str) -> Result<ApprovalDefinition, OutboundFailure>` | 宿主 provisioning 与派发补快照共用。 |
| `approval::subscribe_approval` | `pub async fn subscribe_approval(transport: &dyn OutboundTransport, sleeper: &dyn Sleeper, tokens: &dyn TokenSource, approval_code: &str) -> Result<SubscribeOutcome, OutboundFailure>` | POST 无 body、idempotent=true；1390007 返回 AlreadySubscribed。 |
| `approval::TokenSource` | `#[async_trait] pub trait TokenSource: Send + Sync { async fn tenant_access_token(&self) -> anyhow::Result<String>; async fn invalidate_and_refresh(&self) -> anyhow::Result<String>; }` | 新增边界：宿主 TenantTokenProvider 实现之；若 token 模块也迁入 crate，可省掉 trait 直接用具体类型。 |
| `contact::applicant_search_url` | `pub fn applicant_search_url() -> String` | GET /open-apis/search/v1/user；保留名字，宿主测试已断言。 |
| `contact::ApplicantResolution` | `pub enum ApplicantResolution { Resolved { open_id: String }, NotFound, Ambiguous { candidates: Vec<String> } }` | 姓名→open_id 收敛规则的结果。 |
| `contact::resolve_applicant` | `pub async fn resolve_applicant(transport: &dyn OutboundTransport, sleeper: &dyn Sleeper, bearer_token: &str, name: &str) -> Result<ApplicantResolution, OutboundFailure>` | 只接受 user_access_token；空姓名零出站。 |
| `contact::resolve_department_name` | `pub async fn resolve_department_name(transport: &dyn OutboundTransport, sleeper: &dyn Sleeper, tokens: &dyn TokenSource, department_id: &str) -> Option<String>` | best-effort，失败返回 None 不冒泡；内部用 DEPARTMENT_RETRY 一次即止。 |
| `contact::department_id_type` | `pub fn department_id_type(department_id: &str) -> &'static str` | od- 前缀→open_department_id，其余→department_id。 |
| `approval::idempotency::derive_idempotency_key` | `pub fn derive_idempotency_key(parts: &[&str]) -> String` | parts.join("\|") → SHA-256 前 16 字节 → v8/RFC 变体位 → 36 字符 UUID；parts 不得含 '\|'。 |
| `approval::idempotency::derive_uuid` | `pub fn derive_uuid(base_token: &str, table_id: &str, approval_code: &str, record_id: &str) -> String` | = derive_idempotency_key(&[base, table, code, record])；保持与现有持久化 uuid 完全一致。 |
| `approval::rate_limit::min_interval_ms` | `pub fn min_interval_ms(rate_per_minute: u32) -> u64` | 向上取整；0 按 1 次/分钟。 |
| `approval::rate_limit::bucket_key` | `pub fn bucket_key(key_prefix: &str, app_id: &str) -> String` | = {key_prefix}:app:{app_id}:approval_create_gate；宿主传 yang-system:feishu 保持既有键。 |
| `approval::rate_limit::CreateGate` | `pub struct CreateGate { redis: RedisClient, key_prefix: String, app_id: String, rate_per_minute: u32, max_wait: Duration } impl CreateGate { pub fn new(...) -> Self; pub async fn acquire(&self) -> Result<(), BaseError> }` | 匀速闸门；最多等 5 秒，超时返回 BaseError::RateLimitExceeded{retry_after_seconds:1}。 |
| `approval::rate_limit::ApprovalCreateGateHook` | `pub struct ApprovalCreateGateHook { gate: CreateGate } #[async_trait] impl PreSendHook for ApprovalCreateGateHook { async fn before_send(&self, request: &OutboundRequest) -> Result<(), BaseError> }` | 仅对 approval::is_create_request 取名额；每次真实出站（含重试）都会经过。 |
| `transport::PreSendHook` | `#[async_trait] pub trait PreSendHook: Send + Sync { async fn before_send(&self, request: &OutboundRequest) -> Result<(), BaseError>; }` | 新增：HttpClientTransport::with_pre_send_hook(Arc<dyn PreSendHook>)；None = 不闸门。 |
| `transport::validate_path_segment` | `pub fn validate_path_segment(name: &str, value: &str) -> anyhow::Result<()>` | 审批版语义：非空、<=128 字节、不含 / ? # 空格 \；approval/contact/instance_applink 共用。 |
| `approval::APPROVAL_REQUEST_TIMEOUT_SECS` | `pub const APPROVAL_REQUEST_TIMEOUT_SECS: u64 = 20;` | 审批主链路请求超时。 |
| `contact::DEPARTMENT_RETRY / DEPARTMENT_REQUEST_TIMEOUT_SECS` | `pub const DEPARTMENT_RETRY: RetryPolicy = RetryPolicy { max_attempts: 1, base_backoff_ms: 0, max_backoff_ms: 0, max_retry_after_ms: 0 }; pub const DEPARTMENT_REQUEST_TIMEOUT_SECS: u64 = 5;` | 从通用 outbound 常量里移到 contact，语义归属更清楚。 |

### 测试处置（22）

| 文件 | 区间 | 内容 | 处置 |
|---|---|---|---|
| src/addon/feishu/domain/approval.rs | 857-860 | create_url_is_the_official_path：创建 URL 路径 | **move** |
| src/addon/feishu/domain/approval.rs | 862-869 | subscribe_url_is_the_official_path：订阅 URL 路径 | **move** |
| src/addon/feishu/domain/approval.rs | 875-900 | subscribe_approval_treats_already_subscribed_as_success：1390007 幂等成功 + 请求形状（POST/无 body） | **move** |
| src/addon/feishu/domain/approval.rs | 902-912 | detail_url_accepts_a_uuid：uuid 可作 instance_id | **move** |
| src/addon/feishu/domain/approval.rs | 914-939 | path_segments_are_validated / definition_url_is_the_official_path / detail_request_asks_for_open_id_shape | **move** |
| src/addon/feishu/domain/approval.rs | 941-1128 | 实例详情解析 5 条：serial/status、缺 serial、bot 形态 task_list+timeline、user 形态、空 serial 归一 | **move** |
| src/addon/feishu/domain/approval.rs | 1130-1175 | 定义解析 form/is_external、创建响应 instance_code、decode_data 成功/缺 data | **move** |
| src/addon/feishu/domain/approval.rs | 1179-1239 | ScriptedTransport / NoopSleeper / envelope 测试夹具 | **move** |
| src/addon/feishu/domain/approval.rs | 1241-1389 | resolve_applicant 6 条：唯一精确命中、请求形状、无命中、重名、单模糊命中、缺 scope Fatal、空姓名零出站 | **move** |
| src/addon/feishu/domain/approval.rs | 1393-1428 | FakeTokenCache + fake_tokens（依赖 TenantTokenProvider::new/FeishuCredentials） | **rewrite** |
| src/addon/feishu/domain/approval.rs | 1446-1549 | 部门名解析 5 条：id_type 分流、40004/99991672 退化 None、缺 name、非法 ID 零出站 | **move** |
| src/addon/feishu/domain/approval_uuid.rs | 78-148 | uuid 规范形态、稳定性、跨表/定义/坐标分隔、版本位与变体位 | **move** |
| src/addon/feishu/domain/approval_uuid.rs | 36-72（新增黄金向量用例） | 新增：钉住四元组 (appbcbWCzen6,tblsRc9GRRX,4202AD96-9EC1,recqwIwhc6) 的完整 uuid 字面量，防止泛化时派生漂移破坏存量任务回捞 | **rewrite** |
| src/addon/feishu/domain/approval_rate_limit.rs | 71-119 | bucket_key 命名空间、不同 app 不同键、间隔取整、非零、0 速率、官方上限 | **move** |
| src/addon/feishu/domain/approval_rate_limit.rs | 121-204 | 两条 #[ignore] 真实 Redis 并发闸门测试（DB15） | **move** |
| src/addon/feishu/domain/approval_dispatch.rs | 3287-3400 | dispatch_one 部门名解析接线（创建路径与恢复路径），重点验证顺序与零额外出站 | **stay** |
| src/addon/feishu/domain/approval_dispatch.rs | 3440 | derive_uuid 字面量断言（appbcbWCzen6/tblsRc9GRRX/4202AD96-9EC1/rec001） | **stay** |
| src/addon/feishu/domain/approval_dispatch.rs | 3779-3790,5066-5075 | outbound_reason / sanitize 对 FailureKind 五变体的映射与裁剪 | **stay** |
| src/addon/feishu/domain/approval_dispatch.rs | 4063-4405 | InstanceDetail 结构体字面量与 write_extended/status/flow/handler 断言；字段公开化或加 Default 后需同步 | **rewrite** |
| src/addon/feishu/domain/approval_execution/mod.rs | 83-1098 | ExecutionTransport/GroupExecutionTransport 的单测：create URL 识别、60012 转 lookup、非零业务码恢复安全阶段 | **stay** |
| src/addon/feishu/approval/actions/dispatch.rs | 2042,2703,3256,3584 | 端到端派发测试：instance_applink、applicant_search_url、create_instance_url、derive_uuid 请求体断言 | **stay** |
| tests/feishu_approval_dispatch_integration.rs | 1-1577 | 真实 MySQL 的派发集成测试（含 rate limit 配置 90、Action 派发、request_log 断言） | **stay** |

### 宿主替换点（22）

- `src/addon/feishu/domain/approval.rs:1-1550` → 第一阶段删除实现，留兼容 shim：`pub(crate) use yang_feishu::approval::*;` + `pub(crate) use yang_feishu::contact::{applicant_search_url, resolve_applicant, resolve_department_name, ApplicantResolution};`；随后按文件逐个改 import 并删除 shim。
- `src/addon/feishu/domain/approval_uuid.rs:1-149` → `pub(crate) use yang_feishu::approval::idempotency::{derive_uuid, derive_idempotency_key};`（保留 derive_uuid 名字，7 个宿主调用点零改动）。
- `src/addon/feishu/domain/approval_rate_limit.rs:1-205` → 删除文件；唯一生产调用方 outbound.rs:215 改为 pre-send hook 后不再需要宿主侧模块。
- `src/addon/feishu/domain/mod.rs:2,12,14` → 删除 `pub(crate) mod approval;` / `approval_rate_limit` / `approval_uuid`（若用 shim 则保留 approval 与 approval_uuid 两个 shim 文件）。
- `src/addon/feishu/domain/outbound.rs:176-198` → `pub struct HttpClientTransport { client: HttpClient, pre_send: Option<Arc<dyn PreSendHook>> }`，新增 `pub fn with_pre_send_hook(mut self, hook: Arc<dyn PreSendHook>) -> Self`；删除 with_approval_create_limit。
- `src/addon/feishu/domain/outbound.rs:203-223` → 删除整段审批创建限速匹配；send 只做纯 HTTP 发送。`approval_rate_limit` 的 warn 文案（审批创建未取得名额）移到 hook 内。
- `src/addon/feishu/domain/outbound_setup.rs:61-68` → `HttpClientTransport::new(ctx.tools().http()?.clone()).with_pre_send_hook(Arc::new(ApprovalCreateGateHook::new(ctx.tools().cache()?.clone(), "yang-system:feishu", &app_id, settings.approval_create_rate_per_minute)))`。
- `src/infrastructure/feishu_approval_worker.rs:244-250` → 与 outbound_setup 同一接线（worker 自己构造 transport，不走 outbound_setup）：安装 ApprovalCreateGateHook，前缀仍传 yang-system:feishu。
- `src/addon/feishu/domain/approval_execution/transport.rs:19-22` → `let create = crate::addon::feishu::approval::is_create_request(&request);`，删除 URL 字符串匹配。
- `src/addon/feishu/domain/approval_execution/transport.rs:150-151` → `let create = crate::addon::feishu::approval::is_create_request(&request);`。
- `src/addon/feishu/domain/approval_execution/transport.rs:64,183` → `CODE_UUID_CONFLICT` 改为 `yang_feishu::transport::codes::CODE_UUID_CONFLICT`（或 approval 再导出的同名常量）；其余 CODES_TOKEN_EXPIRED/CODE_RATE_LIMITED 仍来自宿主 outbound（若 L0 已迁则一并改）。
- `src/addon/feishu/domain/approval_dispatch.rs:31-40` → 改为 `use yang_feishu::approval::{create_instance, get_approval_definition, get_instance, CreateOutcome, GetOutcome, InstanceDetail, TaskNode};` + `use yang_feishu::contact::resolve_department_name;` + `use yang_feishu::approval::idempotency::derive_uuid;`。
- `src/addon/feishu/domain/approval_dispatch.rs:1521` → `super::approval::instance_applink` → `yang_feishu::approval::instance_applink`（其余 1263、2215、2388、2675、4213、515 等处同）。
- `src/addon/feishu/domain/approval_aggregation/mod.rs:77-84` → group_uuid 内部改调 `yang_feishu::approval::idempotency::derive_uuid`；group_key 保留。
- `src/addon/feishu/domain/approval_execution/repository.rs:21` → `super::super::approval_uuid::derive_uuid` → crate 路径。
- `src/infrastructure/feishu_approval_worker.rs:1461,1579,1949` → derive_uuid 改为 crate 路径；若保留 shim 则无需改。
- `src/addon/feishu/approval/actions/dispatch.rs:47,899,2042,2703,2812,3597` → resolve_applicant/ApplicantResolution/applicant_search_url/instance_applink/create_instance_url/derive_uuid 全部改为 crate 路径；行为不变。
- `src/addon/feishu/domain/approval_provision.rs:33,257` → get_approval_definition 改 crate 路径；build_plan 逻辑不动。
- `src/addon/feishu/approval/actions/list_widgets.rs:14,75; src/addon/feishu/approval/actions/refresh_names.rs:5-6,155` → get_approval_definition 改 crate 路径。
- `src/infrastructure/feishu_event_worker.rs:207,479,707` → subscribe_approval / get_instance / GetOutcome 改 crate 路径；subscribe_enabled_configs（189-226）的调度逻辑不动。
- `src/addon/feishu/domain/approval_notification.rs:155,515,568,1176; src/addon/feishu/domain/approval_convert.rs:1415,1461` → instance_applink 改 crate 路径（后两处仅测试）。
- `Cargo.toml:4-10` → 新增 `yang-feishu = { version = "0.1.0", path = "../../crates/yang-feishu", default-features = false }`；crates/yang-feishu/Cargo.toml 复用 workspace 的 serde/serde_json/sha2/tokio/async-trait/tracing/anyhow/yang-db(redis)，不新增依赖，保持 MSRV 1.80。

### 风险

- 幂等 uuid 漂移是最危险项：uuid 已持久化在 feishu_approval_task.uuid 与 feishu_approval_group.uuid，derive_idempotency_key 必须保持四元组 `\|` 拼接的逐字节输出；若改成长度前缀/换分隔符，蓝绿切换后存量任务按 uuid 回捞全部 1390003，审批单会被重复创建或永久卡住。落地时必须先加黄金向量测试。
- crate 依赖尚未抽取的 outbound / tenant_token：approval 的 6 个函数依赖 OutboundTransport/Sleeper/RetryPolicy/FailureKind 与 TenantTokenProvider。若 L0/L2 未先落地，crates/yang-feishu 无法独立编译；本步必须同时交付 TokenSource trait 或把 tenant_token 一起迁入。
- 限速键前缀参数化后若宿主误传带 deployment 的前缀，同一 app 的多部署将各自持有一份配额，蓝绿/多实例会叠加超过飞书 100/分钟上限。必须固定传 yang-system:feishu，并在 crate 文档里写明「按 app 共享、不按部署隔离」。
- 把限速从 HttpClientTransport 的 fail-closed 检查（outbound.rs:211-214，未配置直接 ConfigError）改成可选 hook 后，宿主若漏装 hook，创建请求会静默无闸门。必须在 outbound_setup.rs 与 feishu_approval_worker.rs 两处都接线，并提供 crate 侧 approval_transport(inner, gate) 构造器作为唯一入口。
- 请求识别依赖 URL 字符串：outbound.rs:203-223 与 approval_execution/transport.rs:19-22,150-151 都用 /open-apis/approval/v4/instances 后缀。抽取后 URL 构造在 crate，宿主字符串匹配会随路径调整静默失效（执行权不落 creating、限速不打）。必须换 is_create_request。
- 1390007 的日志降级（outbound.rs:751-756）与客户端按成功处理（approval.rs:588-607）是一对：只搬客户端不搬日志分支，会把每次启动约 49 条重复订阅日志从 DEBUG 抬回 WARN；只搬日志不搬客户端则订阅失败。两处必须同批移动。
- 1390003 的日志降级同理（outbound.rs:757-766）：派发前 uuid 预检是高频正常路径，丢失降级会让 failed 指标与 WARN 噪声暴涨。
- FailureKind 的五个变体是宿主公开契约：approval_feedback.rs:212-223 与 approval_dispatch.rs:964-969、1776-1806 穷举匹配，crate 若合并变体或把 UuidConflict/InstanceNotFound 降级为 Fatal{code}，宿主编译失败且 60012 会被写成终态错误（不可逆回填）。
- InstanceDetail 字段公开化会让 10+ 处宿主结构体字面量（approval_dispatch.rs:4063-4405）在字段变更时全部破碎；department_name 是宿主富化值、不是 API 字段，若 crate 未来给 InstanceDetail 加 Serialize 会把宿主状态泄漏到协议层。建议公开时 derive Default 并明确标注 host-owned。
- 路径段校验统一可能改变 instance_applink 的入参规则：审批版允许任意字符但禁 /?#空格\，bitable 版只允许 ASCII 字母数字-_。若错误地采用 bitable 版，含非 ASCII 的 instance_code 会被拒；若采用审批版，原本被拒的字符会放行。两者都拒绝路径分隔符，安全影响可控，但行为差异需在测试里钉住。
- 中文错误文案随 crate 公开：宿主最终用户看到的审批失败文案（approval_feedback.rs:212-223 的 outbound_reason、approval.rs 的解析失败文案）会成为 crate 的稳定契约；后续 crate 版本升级改文案会直接影响生产用户提示。
- MSRV 与 workspace 依赖：crates/yang-feishu 会被 lib_yang workspace 的 crates/* 收录，必须遵守 edition 2021 / rust-version 1.80；不要为 derive_idempotency_key 引入 uuid v5（会拉 sha1 feature 并改 Cargo.lock），继续用 workspace 的 sha2。
- 宿主与 crate 的测试夹具分叉：approval.rs 的 ScriptedTransport/NoopSleeper/fake_tokens 被搬走后，宿主 approval_dispatch.rs 自己另有一套同名夹具；两套若语义漂移，crate 客户端测试通过但宿主编排测试可能喂出与真实响应不同的形状。建议 crate 暴露 test-support feature 或在宿主保留统一夹具。

## A.5 L1 审批表单契约与值编码

**目标**：把审批定义 form 解析、控件类型规则、值编码/组装与附件搬运抽成新 crate crates/yang-feishu 的 approval 契约模块（附件部分落在 crate 内的 drive 子模块）；多维表格字段映射、派发编排、回写与配置持久化全部留在宿主 yang-system。本清单为只读调研产物，含逐符号处置、精确行号、必须前置的重构点、目标公开 API 与宿主替换点。

### 源文件

| 文件 | 生产行区间 | 生产行数 | 测试行区间 | 测试行数 |
|---|---|---:|---|---:|
| src/addon/feishu/domain/widget_rules.rs | 1-165 | 165 | 167-245 | 79 |
| src/addon/feishu/domain/approval_match.rs | 1-1030 | 1030 | 1032-2477 | 1446 |
| src/addon/feishu/domain/approval_convert.rs | 1-1288 | 1288 | 1290-2769 | 1480 |
| src/addon/feishu/domain/approval_attachment.rs | 1-260 | 260 | 262-447 | 186 |
| src/addon/feishu/domain/approval.rs | 1-844 | 844 | 846-1550 | 705 |
| src/addon/feishu/domain/approval_provision.rs | 1-1501 | 1501 | 1502-2731 | 1230 |
| src/addon/feishu/domain/approval_feedback.rs | 1-256 | 256 | 257-535 | 279 |
| src/addon/feishu/domain/approval_dispatch.rs | 1-1997 | 1997 | 1998-5088 | 3091 |

### 符号（109）

| # | 符号 | 类型 | 位置 | 处置 | 备注 |
|---:|---|---|---|---|---|
| 1 | `UNSUPPORTED_TYPES` | const | src/addon/feishu/domain/widget_rules.rs:19-34 | **move-as-is** | 14 项官方不支持 + 本系统未实现控件清单；纯数据无依赖。 |
| 2 | `CONFIG_REJECT_TYPES` | const | src/addon/feishu/domain/widget_rules.rs:41-48 | **move-as-is** | 必填时配置期必须拦截的控件类型。 |
| 3 | `must_reject_at_config` | fn | src/addon/feishu/domain/widget_rules.rs:51-53 | **move-as-is** | 配置期判定入口，被 approval_match::match_fields:657 使用。 |
| 4 | `MANUAL_VALUE_TYPES` | const | src/addon/feishu/domain/widget_rules.rs:58 | **move-as-is** | address/attachment/document 三类需人工准备值。 |
| 5 | `CONTAINER_TYPES` | const | src/addon/feishu/domain/widget_rules.rs:64 | **move-as-is** | fieldList + 三个控件组父级。 |
| 6 | `is_unsupported` | fn | src/addon/feishu/domain/widget_rules.rs:67-69 | **move-as-is** | 配置期与派发期共用同一判据。 |
| 7 | `needs_manual_value` | fn | src/addon/feishu/domain/widget_rules.rs:72-74 | **move-as-is** | 被 approval_convert:1286-1288 二次包装。 |
| 8 | `is_container` | fn | src/addon/feishu/domain/widget_rules.rs:77-79 | **move-as-is** | approval_convert::assemble_definition 与 prepare_definition 都靠它分支。 |
| 9 | `is_control_group` | fn | src/addon/feishu/domain/widget_rules.rs:83-85 | **move-as-is** | 只认 leaveGroupV2/workGroup/outGroup，不含 fieldList；旧配置父级映射据此忽略。 |
| 10 | `is_configurable` | fn | src/addon/feishu/domain/widget_rules.rs:88-90 | **move-as-is** | 配置向导可见性。 |
| 11 | `is_mappable` | fn | src/addon/feishu/domain/widget_rules.rs:93-95 | **move-as-is** | 宿主 approval_match::is_eligible_for_mapping:591-593 只是它的别名；crate 内应直接用它，避免反向依赖宿主。 |
| 12 | `is_media` | fn | src/addon/feishu/domain/widget_rules.rs:98-100 | **move-as-is** | attachmentV2/image/imageV2 走搬运链路。 |
| 13 | `upload_type` | fn | src/addon/feishu/domain/widget_rules.rs:103-109 | **move-as-is** | 图片控件只能传 image，其它传 attachment；是 drive 上传 spec 的输入之一。 |
| 14 | `max_upload_bytes` | fn | src/addon/feishu/domain/widget_rules.rs:112-118 | **move-as-is** | 图片 10 MiB、其它附件 50 MiB。 |
| 15 | `group_child_ids` | fn | src/addon/feishu/domain/widget_rules.rs:124-150 | **move-as-is** | 三个控件组的子控件白名单与官方顺序；approval_match::group_children 依赖它。 |
| 16 | `is_raw_option_child` | fn | src/addon/feishu/domain/widget_rules.rs:154-156 | **move-as-is** | widgetLeaveGroupType / widgetWorkGroupType 可直取类型 ID；approval_convert::check_converter_matches:729 依赖。 |
| 17 | `unsupported_reason` | fn | src/addon/feishu/domain/widget_rules.rs:159-165 | **move-as-is** | 中文可读原因，会写进多维表格回填列，属于产品文案。 |
| 18 | `WidgetOptions` | type | src/addon/feishu/domain/approval_match.rs:60-67 | **move-as-is** | untagged 枚举表达 option 的四种实测形态（缺失/null -> None；数组 -> Fixed；对象 -> Config）。移动时绝不能加单元变体，否则会抢在数组/对象之前匹配（注释 54-59）。 |
| 19 | `OptionItem` | type | src/addon/feishu/domain/approval_match.rs:70-76 | **move-as-is** | text=可读文案，value=提交值。 |
| 20 | `FormWidget` | type | src/addon/feishu/domain/approval_match.rs:87-109 | **move-as-is** | 审批定义 form 节点的契约类型；external_data 在 102-106，children 在 107-108。只声明要用的字段，未知键由 serde 忽略。 |
| 21 | `FormWidgetRaw` | type | src/addon/feishu/domain/approval_match.rs:115-136 | **move-as-is** | 反序列化中间形态；externalData 的 rename 在 129-133，删掉会静默把链接态当非链接（症状指向完全相反方向）。 |
| 22 | `From<FormWidgetRaw> for FormWidget` | fn | src/addon/feishu/domain/approval_match.rs:138-157 | **move-as-is** | 控件组从 value 数组解析 children，其它类型沿用 children。 |
| 23 | `group_children` | fn | src/addon/feishu/domain/approval_match.rs:161-206 | **move-as-is** | 丢弃 visible=false 与白名单外项、按白名单排序；192-204 按 isSetRule 校正 workGroup 两个条件必填项。 |
| 24 | `option_flag_is_one` | fn | src/addon/feishu/domain/approval_match.rs:209-215 | **move-as-is** | isSetRule 实测可能是数字 1 或布尔 true。 |
| 25 | `FormWidget::parent_widget_ids` | fn | src/addon/feishu/domain/approval_match.rs:222-237 | **move-as-is** | 取 linkageConfigs[].linkageWidgetID 最后一段（可带点号前缀）。 |
| 26 | `FormWidget::connect_definition_codes` | fn | src/addon/feishu/domain/approval_match.rs:240-288 | **move-as-is** | 关联审批允许的定义 Code；notLimitScope 返回 None。派发前 validate_connections 依赖它。 |
| 27 | `FormWidget::walk` | fn | src/addon/feishu/domain/approval_match.rs:294-300 | **move-as-is** | 递归收集自身与后代；crate 内外都用。 |
| 28 | `FormWidget::links_to_our_options` | fn | src/addon/feishu/domain/approval_match.rs:306-338 | **split** | 前半段（URL 形状识别）是契约，但 324 行硬编码宿主路径 /api/v1/feishu/approval/options/。目标 crate 暴露 links_to_external_options(options_path_prefix: &str)；宿主保留一个薄包装传入常量。 |
| 29 | `Column` | type | src/addon/feishu/domain/approval_match.rs:342-351 | **stay-in-host** | 多维表格列定义（field_id/列名/选项名/type 码）——项目字段映射的输入，不进 crate。 |
| 30 | `columns_from_fields` | fn | src/addon/feishu/domain/approval_match.rs:360-370 | **stay-in-host** | 直接吃 bitable::FieldItem（bitable.rs:311）。若将来 Column 要进 crate，先把这里改成中立的 ColumnSpec 输入。 |
| 31 | `resolve_field_coord` | fn | src/addon/feishu/domain/approval_match.rs:385-394 | **stay-in-host** | field_id 优先、列名兜底的坐标解析。 |
| 32 | `ResolvedCoords + resolve_key_coords` | type | src/addon/feishu/domain/approval_match.rs:401-423 | **stay-in-host** | 申请/回填两坐标；同列复用直接拒绝。宿主 provision 使用。 |
| 33 | `MatchError` | type | src/addon/feishu/domain/approval_match.rs:429-544 | **stay-in-host** | 10 个变体全是配置期产品话术（会写回填列），依赖列/配置概念。 |
| 34 | `MatchOutcome` | type | src/addon/feishu/domain/approval_match.rs:547-554 | **stay-in-host** | 全量错误一起返回。 |
| 35 | `match_by_name` | fn | src/addon/feishu/domain/approval_match.rs:560-562 | **stay-in-host** | 默认按名列匹配链路。 |
| 36 | `FieldMapping` | type | src/addon/feishu/domain/approval_match.rs:565-577 | **stay-in-host** | API 请求体契约（deny_unknown_fields + JsonSchema）；含 interval_end_field / option_source_key。 |
| 37 | `is_eligible_for_mapping` | fn | src/addon/feishu/domain/approval_match.rs:591-593 | **stay-in-host** | 只是 widget_rules::is_mappable 的别名；crate 内不得反向 import 它（见 refactor）。 |
| 38 | `mapping_widgets` | fn | src/addon/feishu/domain/approval_match.rs:595-600 | **stay-in-host** | 进映射表的控件集合，list_configs Action 使用。 |
| 39 | `preview_name` | fn | src/addon/feishu/domain/approval_match.rs:604-616 | **stay-in-host** | 控件组空名兜底（请假控件组等），是配置向导展示文案。 |
| 40 | `preview_widgets` | fn | src/addon/feishu/domain/approval_match.rs:619-642 | **stay-in-host** | 明细/控件组父级保留、限定名 parent_child；list_widgets Action 使用。 |
| 41 | `match_fields` | fn | src/addon/feishu/domain/approval_match.rs:644-899 | **stay-in-host** | 项目字段映射的唯一实现（显式 maps + 按名匹配 + 全量校验），整段留宿主。 |
| 42 | `find_column` | fn | src/addon/feishu/domain/approval_match.rs:902-926 | **stay-in-host** | 同名列唯一性判定。 |
| 43 | `check_type_compatibility` | fn | src/addon/feishu/domain/approval_match.rs:938-961 | **stay-in-host** | 只拦「多选列->单选控件」「数字控件->选项列」两类。 |
| 44 | `derive_option_map` | fn | src/addon/feishu/domain/approval_match.rs:977-999 | **stay-in-host** | 控件 option[].text ↔ 列选项名 -> 取 value；对不上静默跳过。 |
| 45 | `has_fixed_options` | fn | src/addon/feishu/domain/approval_match.rs:1002-1007 | **stay-in-host** | raw option 例外判据之一。 |
| 46 | `converter_for` | fn | src/addon/feishu/domain/approval_match.rs:1017-1029 | **stay-in-host** | 配置期定好转换器并落库；产物类型 Converter 来自 crate。 |
| 47 | `WidgetMap` | type | src/addon/feishu/domain/approval_convert.rs:35-59 | **move-with-refactor** | 值组装输入快照。bitable_field(44) 是唯一的多维表格耦合，移入 crate 前改名 source_field；DB 行构造在 approval_dispatch.rs:1871-1909 手工完成（无 serde），改名只波及构造点。 |
| 48 | `amount_currency` | fn | src/addon/feishu/domain/approval_convert.rs:66-93 | **move-as-is** | 从定义快照的 option.currencyRange 取首值；生产路径 WidgetMap.currency 恒为 None，靠它补。 |
| 49 | `Converter` | type | src/addon/feishu/domain/approval_convert.rs:96-117 | **move-as-is** | 数据库持久化为 direct/date/option 三个字符串（109-116 fail-closed），字符串值是不可破坏的存量契约。 |
| 50 | `ConvertError` | type | src/addon/feishu/domain/approval_convert.rs:123-188 | **move-as-is** | 7 个变体；Display 文案由宿主 approval_feedback::conversion_reason 拼装后写回多维表格，属于产品文案。 |
| 51 | `FormInput` | type | src/addon/feishu/domain/approval_convert.rs:191-204 | **move-as-is** | cells 以来源字段 id 为键的原始 JSON，crate 只读不解析多维表格类型。 |
| 52 | `FixedOffset (from_seconds/from_iana/render)` | type | src/addon/feishu/domain/approval_convert.rs:210-250 | **move-as-is** | ±18h 校验、只认 8 个常用 IANA 名；宿主 dispatch 用 from_iana 把 base_timezone 变成偏移。 |
| 53 | `prepare_definition` | fn | src/addon/feishu/domain/approval_convert.rs:257-306 | **move-with-refactor** | 解析 + 结构校验 + 映射校验三合一（build_form 与 build_signature 共用）。移入 crate 后拆出 parse_form_snapshot 供宿主 provision 复用解析。 |
| 54 | `GroupFormRow` | type | src/addon/feishu/domain/approval_convert.rs:311-315 | **move-as-is** | record_id 只用于错误定位；cells/attachment_codes 是该成员自己的。 |
| 55 | `GroupFormInput` | type | src/addon/feishu/domain/approval_convert.rs:318-323 | **move-as-is** | 聚合表单输入。 |
| 56 | `build_form` | fn | src/addon/feishu/domain/approval_convert.rs:328-341 | **move-as-is** | 产物是压缩 JSON 数组字符串（不是对象）。 |
| 57 | `build_group_form` | fn | src/addon/feishu/domain/approval_convert.rs:349-357 | **move-as-is** | 顶层 fieldList 每条成员一行；任一成员明细为空即 BadValue。 |
| 58 | `assemble` | fn | src/addon/feishu/domain/approval_convert.rs:363-390 | **move-as-is** | build_form/build_group_form 公共段。 |
| 59 | `build_signature` | fn | src/addon/feishu/domain/approval_convert.rs:392-412 | **move-as-is** | 聚合一致性签名；签名阶段禁止网络副作用，附件用 source_identities 填 attachment_codes。 |
| 60 | `collect_signature` | fn | src/addon/feishu/domain/approval_convert.rs:415-454 | **move-as-is** | fieldList 任意深度整棵跳过；必填缺失编码 null 不报错。 |
| 61 | `canonical_signature_value + canonical_number` | fn | src/addon/feishu/domain/approval_convert.rs:458-485 | **move-as-is** | 数值统一成规范十进制字符串，保证 100 与 100.0 同签名。 |
| 62 | `assemble_definition` | fn | src/addon/feishu/domain/approval_convert.rs:494-623 | **move-with-refactor** | fieldList 值编码在 507-567（多行 518-543、单行 545-567）；控件组在 568-595；必填判定 613 直接调宿主 approval_match::is_eligible_for_mapping，必须改成 widget_rules::is_mappable。 |
| 63 | `assemble_widget` | fn | src/addon/feishu/domain/approval_convert.rs:626-717 | **move-as-is** | 单控件组装；媒体控件只消费 attachment_codes(678-691)；contact 走 open_ids 兄弟字段(706-711)；amount 的 currency 是 value 的兄弟(712-715)。 |
| 64 | `check_converter_matches` | fn | src/addon/feishu/domain/approval_convert.rs:723-753 | **move-as-is** | raw option 子控件例外 727-734；其余按 type 推导期望转换器。 |
| 65 | `assemble_direct` | fn | src/addon/feishu/domain/approval_convert.rs:756-780 | **move-as-is** | number/amount/formula(760)、telephone(762)、connect(763)、contact(770)、department(771-776)、其余文本(778)。 |
| 66 | `connection_codes` | fn | src/addon/feishu/domain/approval_convert.rs:783-888 | **move-with-refactor** | 解析 AppLink/实例 Code，不访问 URL；822 行硬编码 appId cli_9cb844403dbb9108（生产应用 ID），crate 必须参数化或由调用方注入。 |
| 67 | `assemble_date` | fn | src/addon/feishu/domain/approval_convert.rs:896-933 | **move-as-is** | DateInterval 两列模式 902-916、{start,end} 模式 917-925、单点日期 928-932。 |
| 68 | `date_interval_value` | fn | src/addon/feishu/domain/approval_convert.rs:936-943 | **move-as-is** | 输出 {start,end,interval}，interval 按天（毫秒差/86400000）。 |
| 69 | `assemble_option` | fn | src/addon/feishu/domain/approval_convert.rs:946-973 | **move-as-is** | radio/radioV2 单字符串（961-969），checkbox 数组（970-972）；未映射文案报 UnmappedOption。 |
| 70 | `assemble_telephone` | fn | src/addon/feishu/domain/approval_convert.rs:979-1009 | **move-as-is** | 拆 {countryCode,nationalNumber}；无 + 前缀默认 +86；区号 1~3 位。 |
| 71 | `number_from_cell` | fn | src/addon/feishu/domain/approval_convert.rs:1011-1037 | **move-as-is** | 兼容 [1] 与 {type:2,value:[1]}；多值不静默截断；字符串数字转 f64。 |
| 72 | `open_ids_from_cell` | fn | src/addon/feishu/domain/approval_convert.rs:1043-1071 | **move-as-is** | 人员字段三种形态：对象数组/裸字符串数组/逗号文本；contact 与 department 共用。 |
| 73 | `unwrap_typed_cell` | fn | src/addon/feishu/domain/approval_convert.rs:1078-1086 | **move-as-is** | 剥 batch_get 的 {type,value} 包装；宿主 dispatch 的 applicant 读取也复用它（approval_dispatch.rs:151）。 |
| 74 | `labels_from_cell` | fn | src/addon/feishu/domain/approval_convert.rs:1093-1114 | **move-as-is** | 取选项文案；数字也算文案（汇率 0.0435）；与 pull 侧同口径 trim。provision 外部选项解析依赖。 |
| 75 | `normalize_decimal_label` | fn | src/addon/feishu/domain/approval_convert.rs:1121-1150 | **move-as-is** | 纯十进制归一 9.1330 -> 9.133；带单位/指数一律 None。 |
| 76 | `labels_equivalent` | fn | src/addon/feishu/domain/approval_convert.rs:1157-1162 | **move-as-is** | 字面相等或数值等价；外部选项匹配用。 |
| 77 | `text_from_cell` | fn | src/addon/feishu/domain/approval_convert.rs:1173-1186 | **move-as-is** | 数组是富文本分段而非多值，必须整段拼接（URL 切分历史 bug）。 |
| 78 | `is_empty` | fn | src/addon/feishu/domain/approval_convert.rs:1193-1218 | **move-as-is** | 空值判定口径（null/空串/空数组/{text:""}）；宿主点击路径也用它，必须保持 pub。 |
| 79 | `format_rfc3339` | fn | src/addon/feishu/domain/approval_convert.rs:1225-1240 | **move-as-is** | 毫秒 + 固定偏移 -> 2019-10-01T08:12:01+08:00；手写换算不引 chrono。 |
| 80 | `format_local` | fn | src/addon/feishu/domain/approval_convert.rs:1246-1255 | **move-as-is** | Unix 秒 -> 'YYYY-MM-DD HH:MM:SS'；通知正文与任务展示用（worker:4766、action dispatch:1781）。 |
| 81 | `civil_from_days` | fn | src/addon/feishu/domain/approval_convert.rs:1261-1273 | **move-as-is** | Howard Hinnant 算法，两处日期格式化的共同底座。 |
| 82 | `is_unsupported_widget_type + needs_manual_value` | fn | src/addon/feishu/domain/approval_convert.rs:1279-1288 | **move-as-is** | 两个薄包装；移入 crate 后可直接删掉、调用点改用 widget_rules，避免同名双入口。 |
| 83 | `UPLOAD_URL` | const | src/addon/feishu/domain/approval_attachment.rs:15 | **move-as-is** | https://www.feishu.cn/approval/openapi/v2/file/upload —— 审批附件上传端点，属于飞书契约。 |
| 84 | `Attachment` | type | src/addon/feishu/domain/approval_attachment.rs:17-22 | **move-with-refactor** | 三个字段目前是私有（18-20），drive 模块外要构造/断言必须改 pub；是 drive 层的中立文件元数据。 |
| 85 | `attachments` | fn | src/addon/feishu/domain/approval_attachment.rs:24-68 | **split** | 混合了两件事：控件规则（28-40 取上限与标签）与文件校验（47-66 file_token 字符/长度、name 长度与控制字符/扩展名、size 范围）。拆成 drive::parse_attachments(cell, spec) + approval::media 的控件适配。 |
| 86 | `preflight` | fn | src/addon/feishu/domain/approval_attachment.rs:71-86 | **move-with-refactor** | 生成占位 code，保证 build_form 校验通过后才产生外部副作用；属于 approval 侧的适配层。 |
| 87 | `source_identities` | fn | src/addon/feishu/domain/approval_attachment.rs:95-114 | **move-with-refactor** | 签名用的 <token>\|<name>\|<size> 三元组，不调网络；worker 播种与聚合签名都依赖，移入 crate 时保留 allow(dead_code) 仅到消费者接上为止。 |
| 88 | `multipart` | fn | src/addon/feishu/domain/approval_attachment.rs:116-127 | **move-as-is** | 手写 multipart（name/type/content 三个 part、随机 boundary）；Content-Type 决定在 117-121。 |
| 89 | `image_content_type` | fn | src/addon/feishu/domain/approval_attachment.rs:130-143 | **move-as-is** | 按扩展名猜图片 Content-Type（png/jpg/jpeg/gif/webp/bmp/heic），否则 octet-stream。 |
| 90 | `send` | fn | src/addon/feishu/domain/approval_attachment.rs:145-171 | **move-with-refactor** | token 失效重取一次；TenantTokenProvider 是 Redis 支撑的宿主实现，crate 需先抽象成 BearerToken trait（异步 token/refresh）。 |
| 91 | `retry` | fn | src/addon/feishu/domain/approval_attachment.rs:173-180 | **move-as-is** | 构造 Retry 失败。 |
| 92 | `transfer` | fn | src/addon/feishu/domain/approval_attachment.rs:182-260 | **move-with-refactor** | 先按 widget 过滤媒体控件，再逐文件下载(drive/v1/medias/{token}/download, 200-220)并上传(225-242)，最后解析 data.code(243-253)。200 行的 extra 查询参数直接引用 BitableCoordinates.table_id 与 widget.bitable_field —— 这是必须解耦的签名。 |
| 93 | `ApprovalDefinition` | type | src/addon/feishu/domain/approval.rs:509-515 | **move-with-refactor** | form 保持原始 JSON Value（512），因为快照要存原文而非 FormWidget 回序列化；is_external 是三方的判据。 |
| 94 | `get_approval_definition` | fn | src/addon/feishu/domain/approval.rs:518-552 | **move-with-refactor** | 定义详情 GET；依赖 outbound + tenant token 两个尚未抽取的层，需 trait 化后随 approval client 一起移。 |
| 95 | `ApprovalDefinitionData` | type | src/addon/feishu/domain/approval.rs:611-617 | **move-with-refactor** | 响应信封：approval_name/form/is_external（is_external 缺省 None）。 |
| 96 | `create_instance_url + approval_definition_url` | fn | src/addon/feishu/domain/approval.rs:46-75 | **move-as-is** | 两个官方路径构造；FEISHU_OPEN_BASE 目前来自 tenant_token，移入 crate 时应变成 crate 常量或参数。 |
| 97 | `validate_path_segment` | fn | src/addon/feishu/domain/approval.rs:86-98 | **move-as-is** | 与 bitable.rs 同源的一份私有实现；crate 内应只保留一份。 |
| 98 | `CreateOutcome` | type | src/addon/feishu/domain/approval.rs:110-115 | **move-as-is** | UuidConflict 不是失败，是回捞信号。 |
| 99 | `create_instance` | fn | src/addon/feishu/domain/approval.rs:121-176 | **move-with-refactor** | form 参数是字符串（126、130-137）；idempotent 必须为 true（148-155 的注释是硬约束）。随 approval client 移。 |
| 100 | `instance_applink` | fn | src/addon/feishu/domain/approval.rs:62-67 | **stay-in-host** | 硬编码生产应用 appId cli_9cb844403dbb9108 与 bitable 来源参数——产品/租户配置，不能进通用 crate。 |
| 101 | `parse_form` | fn | src/addon/feishu/domain/approval_provision.rs:159-185 | **move-with-refactor** | 实测 form 可能是 JSON 字符串也可能是数组，两种都认；返回宿主 ProvisionError::Invalid(Vec<String>)，移入 crate 时改返回 crate 的 FormError，宿主包一层。 |
| 102 | `form_snapshot_text` | fn | src/addon/feishu/domain/approval_provision.rs:192-198 | **move-as-is** | 快照统一存数组形态；与 prepare_definition 共用同一解析入口。 |
| 103 | `widget_name + field_name + container_columns` | fn | src/addon/feishu/domain/approval_feedback.rs:96-144 | **stay-in-host** | 把控件 id 翻成「控件名/字段名」并处理容器父级；读多维表格列元数据，纯宿主能力。 |
| 104 | `conversion_reason` | fn | src/addon/feishu/domain/approval_feedback.rs:146-201 | **stay-in-host** | 把 ConvertError 拼成写回多维表格的中文文案（控件定位/字段定位）；产品文案 + 表结构耦合，必须留宿主。 |
| 105 | `DispatchInput` | type | src/addon/feishu/domain/approval_dispatch.rs:202-248 | **stay-in-host** | 派发输入（申请人、回填列、任务、坐标、定义快照）。没有任何 crate 函数以它为参数——耦合是单向的「宿主持有 crate 类型」，无需解耦签名。 |
| 106 | `GroupDispatchInput` | type | src/addon/feishu/domain/approval_dispatch.rs:900-940 | **stay-in-host** | 聚合组派发输入；members 的 attachment_codes 是 drive 产物。 |
| 107 | `widget_maps_from_rows` | fn | src/addon/feishu/domain/approval_dispatch.rs:1871-1909 | **stay-in-host** | DB 行 -> WidgetMap 的宿主适配器；WidgetMap 改字段名后这里是主要修改点（1879/1895）。 |
| 108 | `log_conversion_error + is_data_problem` | fn | src/addon/feishu/domain/approval_dispatch.rs:1810-1847 | **stay-in-host** | 把 ConvertError 分成「等待态/终态」并降噪日志；含宿主决策语义，留宿主。 |
| 109 | `requires_media_transfer` | fn | src/addon/feishu/domain/approval_dispatch.rs:462-466 | **stay-in-host** | 编排层判据：要先查实例还是先搬附件；调用 crate 的 is_media 即可。 |

### 需先重构

- **把 approval_convert 对宿主匹配层的反向依赖换成 widget_rules::is_mappable** — approval_convert 第 31 行 import 了 approval_match::is_eligible_for_mapping，第 613 行使用；match 模块留宿主后这会让 crate 依赖宿主，形成环。is_eligible_for_mapping(591-593) 本身就只是 widget_rules::is_mappable 的别名。（`src/addon/feishu/domain/approval_convert.rs:31, 613`）
- **WidgetMap.bitable_field 改名 source_field，并同步所有读写点** — 这是 crate 类型里唯一的多维表格语义；不改名等于把 bitable 概念带进契约层。WidgetMap 无 serde derive（手工构造），改名不破坏持久化。（`src/addon/feishu/domain/approval_convert.rs:44, 632, 659; approval_attachment.rs:41, 200; approval_provision.rs:1161-1165, 1186, 1196; approval_dispatch.rs:1879, 1895, 1830`）
- **approval_attachment::transfer 去掉 &BitableCoordinates，改为宿主注入的 extra 查询构造器** — 第 200 行用 coordinates.table_id + widget.bitable_field 拼 download 的 bitablePerm extra 参数；drive 能力不应认识多维表格。建议 transfer 新增参数 extra_of: impl Fn(&str /*source_field*/, &str /*file_token*/) -> Option<String>，宿主传 BitableCoordinates 的闭包。（`src/addon/feishu/domain/approval_attachment.rs:182-190, 200`）
- **拆分 attachments()：drive 侧只收 (cell_value, UploadSpec) 做文件校验，approval 侧保留控件适配** — 24-68 混合了控件规则（28-40：上限/标签）与文件契约（47-66：file_token 字符集与长度、name 控制字符与扩展名、size 范围）。拆开后 drive 不 import WidgetMap。（`src/addon/feishu/domain/approval_attachment.rs:24-68`）
- **approval_attachment::send 的 TenantTokenProvider 换成 trait BearerToken** — TenantTokenProvider 是 Redis 分布式锁 + 缓存的宿主实现（tenant_token.rs:258-430）。crate 只应要求 token()/invalidate_and_refresh() 两个异步方法。（`src/addon/feishu/domain/approval_attachment.rs:145-171`）
- **links_to_our_options 的外部选项 URL 前缀参数化** — 第 324 行硬编码 /api/v1/feishu/approval/options/，这是宿主 HTTP 路由；crate 里应接受 options_url_path: &str。（`src/addon/feishu/domain/approval_match.rs:306-338 (尤其 324)`）
- **connection_codes 的 appId 常量化/参数化** — 第 822 行硬编码 cli_9cb844403dbb9108（生产审批应用 ID）；换租户或自建应用时整条关联审批解析都会失败且报错指向「链接不是飞书审批应用」。（`src/addon/feishu/domain/approval_convert.rs:822`）
- **抽出 parse_form_snapshot 供 prepare_definition 与宿主 parse_form 共用** — 两处各自 serde_json::from_str::<Vec<FormWidget>>（approval_convert.rs:261、approval_provision.rs:166/169），错误文案与校验纪律会漂移；移动后 crate 暴露一个解析入口，宿主错误类型在外层映射。（`src/addon/feishu/domain/approval_convert.rs:257-306; src/addon/feishu/domain/approval_provision.rs:159-185`）
- **approval_attachment::Attachment 三个字段改 pub** — 当前 18-20 行为私有字段，模块外无法构造或断言；拆到 crate 的 drive 层后 approval 侧与测试都需要读。（`src/addon/feishu/domain/approval_attachment.rs:17-22`）
- **approval_dispatch::validate_connections 直接 serde 解析 FormWidget 的点改用 crate 解析器** — 第 1386 行绕过统一入口直接 serde_json::from_str；FormWidget 移入 crate 后应走 crate 的 parse 函数并统一错误文案。（`src/addon/feishu/domain/approval_dispatch.rs:1386-1389; src/addon/feishu/domain/approval_feedback.rs:183`）
- **文件级 #![allow(dead_code)] 改为 crate 内模块级/按项标注** — widget_rules.rs:9、approval_match.rs:42、approval_convert.rs:25 是内部属性，整文件搬进 crate 后要么变成模块内属性、要么在新 crate 里重新评估（多数消费者已在，可以删）。（`src/addon/feishu/domain/widget_rules.rs:9; approval_match.rs:42; approval_convert.rs:25`）
- **crate 内不再保留 is_unsupported_widget_type / needs_manual_value 两个薄包装** — approval_convert.rs:1279-1288 只是 widget_rules 的同义转发，会同名双入口；迁移时调用点直接指向 widget_rules。（`src/addon/feishu/domain/approval_convert.rs:1279-1288`）

### 目标公开 API

| 名称 | 签名 | 备注 |
|---|---|---|
| `yang_feishu::approval::form::parse_form` | `pub fn parse_form(form: &serde_json::Value) -> Result<Vec<FormWidget>, FormError>` | 接受 JSON 字符串与数组两种实测形态；空表单报错。宿主 provision 用 ? 映射成 ProvisionError::Invalid。 |
| `yang_feishu::approval::form::FormWidget` | `pub struct FormWidget { pub id: String, pub name: String, pub r#type: String, pub required: bool, pub value: serde_json::Value, pub option: Option<WidgetOptions>, pub external_data: Option<serde_json::Value>, pub children: Vec<FormWidget> }` | 从 FormWidgetRaw 反序列化，控件组自动从 value 数组解析 children；保留 serde(rename="externalData")。 |
| `yang_feishu::approval::form::FormWidget::links_to_external_options` | `pub fn links_to_external_options(&self, options_url_path: &str) -> bool` | 原 links_to_our_options，路径前缀由宿主传入（见 refactor）。 |
| `yang_feishu::approval::widget_rules` | `pub fn is_media(&str) -> bool; pub fn upload_type(&str) -> &'static str; pub fn max_upload_bytes(&str) -> usize; pub fn group_child_ids(&str) -> Option<&'static [&'static str]>; pub fn is_mappable(&str) -> bool; pub fn is_container(&str) -> bool; pub fn is_control_group(&str) -> bool; pub fn must_reject_at_config(&str) -> bool; pub fn is_raw_option_child(&str) -> bool` | 整表原样公开，配置期与派发期共用同一判据。 |
| `yang_feishu::approval::value::WidgetMap` | `pub struct WidgetMap { pub widget_id: String, pub widget_type: String, pub required: bool, pub source_field: String, pub interval_end_field: Option<String>, pub converter: Converter, pub option_map: BTreeMap<String, String>, pub option_source_key: Option<String>, pub currency: Option<String> }` | 宿主映射层与 crate 组装层之间的唯一数据结构；字段改名 bitable_field -> source_field。 |
| `yang_feishu::approval::value::FormInput` | `pub struct FormInput<'a> { pub widgets: &'a [WidgetMap], pub form_snapshot: &'a str, pub cells: &'a serde_json::Map<String, Value>, pub attachment_codes: &'a BTreeMap<String, Vec<String>>, pub timezone_offset: FixedOffset }` | cells 键是 source_field；attachment_codes 是 drive 上传产物，approval 只消费不生产。 |
| `yang_feishu::approval::value::build_form` | `pub fn build_form(input: &FormInput<'_>) -> Result<String, ConvertError>` | 返回压缩后的 JSON 数组字符串，直接喂 create_instance 的 form 字段。 |
| `yang_feishu::approval::value::build_group_form` | `pub fn build_group_form(input: &GroupFormInput<'_>) -> Result<String, ConvertError>` | 聚合：顶层 fieldList 每条成员一行，空明细整组失败。 |
| `yang_feishu::approval::value::build_signature` | `pub fn build_signature(input: &FormInput<'_>) -> Result<String, ConvertError>` | 确定性签名，无网络副作用；附件按 source_identities 参与。 |
| `yang_feishu::approval::value::amount_currency` | `pub fn amount_currency(form_snapshot: &str, widget_id: &str) -> Option<String>` | 从定义 option.currencyRange 取币种；宿主在构造 WidgetMap 时回填 currency。 |
| `yang_feishu::approval::value::FixedOffset` | `pub struct FixedOffset; impl FixedOffset { pub fn from_seconds(i32) -> Result<Self, ConvertError>; pub fn from_iana(&str) -> Result<Self, ConvertError> }` | 宿主仍负责把 base_timezone 字符串转成它；失败是配置错误。 |
| `yang_feishu::approval::value::{labels_from_cell, labels_equivalent, normalize_decimal_label, is_empty, unwrap_typed_cell…` | `pub fn labels_from_cell(&Value) -> Vec<String>; pub fn labels_equivalent(&str, &str) -> bool; pub fn normalize_decimal_label(&str) -> Option<String>; pub fn is_empty(&Value) -> bool; pub fn unwrap_typed_cell(&Value) -> &Value; pub fn format_local(i64, FixedOffset) -> String` | 宿主 provision 的外部选项解析、dispatch 的申请人空判定都复用；共享同一套单元格口径。 |
| `yang_feishu::drive::media::parse_attachments` | `pub fn parse_attachments(cell: &Value, spec: UploadSpec<'_>) -> Result<Vec<Attachment>, MediaError>` | UploadSpec { label: &str, file_type: &'static str, max_bytes: usize }；只做文件契约校验，不认识 WidgetMap。 |
| `yang_feishu::drive::media::transfer` | `pub async fn transfer(transport: &dyn OutboundTransport, sleeper: &dyn Sleeper, tokens: &dyn BearerToken, extra_of: impl Fn(&str, &str) -> Option<String>, items: &[TransferItem<'_>]) -> Result<BTreeMap<String, Vec<String>>, OutboundFailure>` | TransferItem { widget_id, source_field, files: Vec<Attachment>, file_type, max_bytes }；下载 drive medias、上传审批 file/upload，返回 widget_id -> codes。extra_of 是 bitable 权限参数的宿主注入点。 |
| `yang_feishu::approval::media::preflight` | `pub fn preflight(widgets: &[WidgetMap], cells: &Map<String, Value>) -> Result<BTreeMap<String, Vec<String>>, ConvertError>` | approval 侧适配：按 widget 取 spec 调 drive::parse_attachments，返回占位 code。 |
| `yang_feishu::approval::media::source_identities` | `pub fn source_identities(widgets: &[WidgetMap], cells: &Map<String, Value>) -> Result<BTreeMap<String, Vec<String>>, ConvertError>` | token\|name\|size 三元组，供签名播种使用，零网络。 |
| `yang_feishu::approval::client::get_approval_definition` | `pub async fn get_approval_definition(transport, sleeper, tokens, approval_code: &str) -> Result<ApprovalDefinition, OutboundFailure>` | 依赖 crate 的 transport/token trait（outbound 与 tenant_token 属其他抽取层）；ApprovalDefinition.form 保持原始 Value。 |
| `yang_feishu::approval::client::create_instance` | `pub async fn create_instance(transport, sleeper, tokens, approval_code: &str, form: &str, open_id: &str, uuid: &str) -> Result<CreateOutcome, OutboundFailure>` | form 为 build_form 产物；idempotent=true 是不可改的硬约束（60012 语义）。 |

### 测试处置（33）

| 文件 | 区间 | 内容 | 处置 |
|---|---|---|---|
| src/addon/feishu/domain/widget_rules.rs | 167-245 | 规则表全量断言：容器/媒体/上传类型/上限/控件组白名单/不支持清单/raw option。 | **move** |
| src/addon/feishu/domain/approval_match.rs | 1032-1160 | 测试夹具：fixed_radio/input/linked_radio/detail/column/multi_column/mapping。 | **rewrite** |
| src/addon/feishu/domain/approval_match.rs | 1163-1263 | parent_widget_ids 点号前缀取末段；connect 的 notLimitScope/definitionIds 两种官方形态。 | **move** |
| src/addon/feishu/domain/approval_match.rs | 1264-1585 | 显式 maps 覆盖/未知/重复/复用、预览分组、明细限定名、多选与选项派生等映射行为。 | **stay** |
| src/addon/feishu/domain/approval_match.rs | 1586-1627 | real_form_shape_parses_with_all_four_option_kinds：option 四种形态的真实响应复刻。 | **move** |
| src/addon/feishu/domain/approval_match.rs | 1628-1715 | matched 列的 option 值派生、columns_from_fields 从 property 取选项、坐标按 id/名解析。 | **stay** |
| src/addon/feishu/domain/approval_match.rs | 1716-2124 | 全部匹配错误收集、类型兼容、多选列拦截、必填缺失与选项目录不可派生。 | **stay** |
| src/addon/feishu/domain/approval_match.rs | 2125-2170, 2384-2438, 2467-2477 | 控件组 children 从 value 解析/过滤/排序、workGroup 条件必填校正、value 为 JSON 字符串形态。 | **move** |
| src/addon/feishu/domain/approval_match.rs | 2171-2204, 2439-2466 | 控件组父级只预览不映射、空名兜底、必填 image 可映射。 | **stay** |
| src/addon/feishu/domain/approval_match.rs | 2205-2242, 2243-2383 | raw option 子控件 direct 转换器、account/旧控件组不支持、config reject 必填拦截。 | **stay** |
| src/addon/feishu/domain/approval_convert.rs | 1290-1411 | 夹具与渲染助手：shanghai/widget/cells/render/render_definition/flat_snapshot。 | **rewrite** |
| src/addon/feishu/domain/approval_convert.rs | 1362-1488 | 附件原始 token 不能提交、connect 单元格变实例 Code 数组、非法链接与序列号拒绝。 | **move** |
| src/addon/feishu/domain/approval_convert.rs | 1489-1711 | 明细子控件嵌套顺序、请假/加班/外出控件组组装、旧配置父级映射忽略。 | **move** |
| src/addon/feishu/domain/approval_convert.rs | 1712-1884 | image 用上传 code、空可选明细省略/必填等待、formula 取值、明细子控件必填只在有行时校验。 | **move** |
| src/addon/feishu/domain/approval_convert.rs | 1885-1970 | 压缩 JSON 数组字符串、可选空值整体省略、必填缺失指名控件、number 是 JSON number、amount 币种兄弟字段。 | **move** |
| src/addon/feishu/domain/approval_convert.rs | 1972-2073 | date RFC3339 与偏移、负偏移、DateInterval 单列与双列、结束缺失按缺值。 | **move** |
| src/addon/feishu/domain/approval_convert.rs | 2075-2151 | 转换器不匹配、纪元/闰日、radio 单串 vs checkbox 数组、选项值非文案。 | **move** |
| src/addon/feishu/domain/approval_convert.rs | 2152-2310 | 文案 trim、富文本多段拼接、batch_get 包装剥壳、数字 label 与十进制等价。 | **move** |
| src/addon/feishu/domain/approval_convert.rs | 2311-2356 | contact open_ids、department {open_id} 数组、telephone 区号拆分与默认 +86。 | **move** |
| src/addon/feishu/domain/approval_convert.rs | 2357-2452 | 不支持控件即使有值也拒绝、官方清单全覆盖、未知转换器 fail-closed、时区越界、空/Null 判定。 | **move** |
| src/addon/feishu/domain/approval_convert.rs | 2453-2560 | 签名排除嵌套 fieldList、数值归一、缺失编码 null。 | **move** |
| src/addon/feishu/domain/approval_convert.rs | 2561-2742 | 聚合组表单：每成员一行、空明细拒绝、嵌套明细按行、顶层取 leader。 | **move** |
| src/addon/feishu/domain/approval_convert.rs | 2743-2769 | 签名区分附件源身份（依赖 source_identities）。 | **rewrite** |
| src/addon/feishu/domain/approval_attachment.rs | 262-347 | 附件元数据非法 token/name/size 与空单元格；preflight 空值。 | **rewrite** |
| src/addon/feishu/domain/approval_attachment.rs | 349-409 | image 10 MiB 上限与 multipart 的 type/Content-Type。 | **rewrite** |
| src/addon/feishu/domain/approval_attachment.rs | 411-446 | source_identities 顺序与三元组、非法附件在签名阶段即拒绝。 | **rewrite** |
| src/addon/feishu/domain/approval.rs | 1131-1160 | ApprovalDefinitionData 解析 form 字符串与非三方默认。 | **move** |
| src/addon/feishu/domain/approval.rs | 846-1130, 1162-1550 | 实例详情两形态解析、URL 构造、申请人解析、部门名解析。 | **stay** |
| src/addon/feishu/domain/approval_provision.rs | 1835-1870, 1929-1970 | parse_form 的数组/字符串形态与缺表单报错。 | **move** |
| src/addon/feishu/domain/approval_provision.rs | 1502-1834, 1874-2731 | 建配置计划、显式映射校验、外部选项解析与回填列校验。 | **stay** |
| src/addon/feishu/domain/approval_feedback.rs | 257-535 | 控件名/字段名/容器列文案与已发起反馈。 | **stay** |
| src/addon/feishu/domain/approval_dispatch.rs | 1998-5088 | 派发编排、回填、聚合组、失效分类。 | **stay** |
| tests/feishu_approval_dispatch_integration.rs | 全文 | 端到端派发集成测试（未直接 import 被抽取的 domain 符号）。 | **stay** |

### 宿主替换点（24）

- `src/addon/feishu/domain/approval_dispatch.rs:35-39` → 改成 use yang_feishu::approval::value::{build_form, build_group_form, Converter, FixedOffset, FormInput, GroupFormInput, GroupFormRow, WidgetMap}; 与 use yang_feishu::approval::media as approval_attachment;（或直接 use yang_feishu::approval::media::{preflight, transfer}）
- `src/addon/feishu/domain/approval_dispatch.rs:44, 462-466` → use yang_feishu::approval::widget_rules; requires_media_transfer 保持不变（调用 widget_rules::is_media）
- `src/addon/feishu/domain/approval_dispatch.rs:672-680, 731-779` → preflight 用 approval::media::preflight；transfer 用 drive::media::transfer 并由宿主传 extra_of 闭包与 TransferItem 列表；build_form 的 FormInput.attachment_codes 仍由 drive 返回的 widget_id->codes 直接喂入
- `src/addon/feishu/domain/approval_dispatch.rs:1103-1108` → build_group_form / GroupFormInput 改从 yang_feishu::approval::value 导入；GroupFormRow 的 attachment_codes 仍来自 crate 的 drive 结果
- `src/addon/feishu/domain/approval_dispatch.rs:1810-1847` → ConvertError 改 from yang_feishu::approval::value::ConvertError；log_conversion_error / is_data_problem 逻辑不变（仅换路径）
- `src/addon/feishu/domain/approval_dispatch.rs:1871-1909` → widget_maps_from_rows 保留在宿主；WidgetMap 构造处把键 bitable_field 改为 source_field（1879 读行键、1895 赋值），DB 列名与配置字段名不变
- `src/addon/feishu/domain/approval_dispatch.rs:1386-1395` → use yang_feishu::approval::form::{FormWidget, parse_snapshot}; 直接调用 crate 解析器替代 serde_json::from_str::<Vec<FormWidget>>
- `src/addon/feishu/domain/approval_feedback.rs:7, 105-144` → use yang_feishu::approval::form::FormWidget; use yang_feishu::approval::value::{ConvertError, WidgetMap}; 其余函数体不变（field_name/widget_name 仍读宿主 FieldItem）
- `src/addon/feishu/domain/approval_feedback.rs:183` → serde_json::from_str::<Vec<FormWidget>>(snapshot) 换成 crate 的 parse_snapshot（错误时同样 unwrap_or_default）
- `src/addon/feishu/domain/approval_provision.rs:34-38` → use yang_feishu::approval::value::{Converter, FixedOffset, WidgetMap, labels_from_cell, labels_equivalent, normalize_decimal_label, amount_currency}; use yang_feishu::approval::form::{FormWidget, WidgetOptions, parse_form}（parse_form 的错误映射回 ProvisionError::Invalid）
- `src/addon/feishu/domain/approval_provision.rs:36-37, 274-276, 394-400` → columns_from_fields/match_fields/MatchOutcome/Column/FieldMapping 继续从宿主 approval_match 导入（这些不迁）；只有 FormWidget/WidgetOptions 改从 crate 导入
- `src/addon/feishu/domain/approval_provision.rs:159-185` → 删除本地 parse_form 实现，改为对 yang_feishu::approval::form::parse_form 的薄包装（保持 ProvisionError 文案）
- `src/addon/feishu/domain/approval_match.rs:48-50` → 改为 use yang_feishu::approval::form::{FormWidget, FormWidgetRaw?, OptionItem, WidgetOptions}。若 FormWidgetRaw/group_children 不公开，则把 FormWidget 的 Deserialize 直接暴露（推荐 pub 构造或 pub(crate) re-export）
- `src/addon/feishu/domain/approval_match.rs:591-593, 977-1029` → is_eligible_for_mapping 保留为宿主别名（内部改调 widget_rules::is_mappable）；derive_option_map/converter_for 继续产出 crate 的 WidgetMap/Converter
- `src/addon/feishu/approval/actions/dispatch.rs:48-50` → use yang_feishu::approval::value::{amount_currency, format_local, is_empty, FixedOffset};
- `src/addon/feishu/approval/actions/dispatch.rs:1071-1090` → amount_currency(&form_snapshot, widget_id) 与 FixedOffset::from_iana 的调用保持不变，仅换 import 路径；构造 WidgetMap 处字段名改 source_field
- `src/addon/feishu/approval/actions/dispatch.rs:1330, 1780-1781` → is_empty / format_local 改从 yang_feishu::approval::value 导入
- `src/infrastructure/feishu_approval_worker.rs:59-61` → use yang_feishu::drive::media::{preflight, source_identities, transfer}; use yang_feishu::approval::value::{amount_currency, build_signature, format_local, FixedOffset, FormInput, WidgetMap};
- `src/infrastructure/feishu_approval_worker.rs:1910, 2001, 3415, 4766, 8011` → 签名/币种/时间格式化调用保持不变，仅换路径；transfer 的实参改为 drive 的 TransferItem + extra_of 闭包
- `src/addon/feishu/approval/actions/list_widgets.rs:96-101` → parse_form 改从 yang_feishu::approval::form 导入；preview_widgets 仍来自宿主 approval_match
- `src/addon/feishu/approval/actions/list_configs.rs:14-15, 314-317` → parse_form 改从 crate；mapping_widgets 仍从宿主 approval_match
- `src/addon/feishu/domain/approval.rs:509-552, 611-617` → ApprovalDefinition/ApprovalDefinitionData/get_approval_definition 改从 yang_feishu::approval::client 导入；ApprovalDefinition.form 仍是 Option<Value>，快照落库逻辑（approval_provision.rs:192-198）不变
- `src/addon/feishu/domain/approval.rs:117-176` → create_instance/CreateOutcome 改从 yang_feishu::approval::client 导入；调用点（approval_dispatch.rs:788、1133）只换路径，idempotent=true 的语义随实现迁移
- `src/addon/feishu/domain/approval.rs:62-67` → instance_applink 留在宿主（硬编码生产 appId）；若 crate 也需生成链接，改由调用方传 app_id 参数

### 风险

- WidgetMap.bitable_field 改名会波及 DB 行适配（approval_dispatch.rs:1879/1895）、provision 的外部选项解析（1161-1196）与三处测试字面量；漏改的表现是编译错，风险可控但面广。
- Converter 的持久化字符串 direct/date/option（approval_convert.rs:109-116）是存量配置契约，移动时不能顺手改名或加 serde rename，否则旧配置全部 UnknownConverter 而终态失败。
- 审批定义快照必须继续存原始 JSON 文本，不能改成 FormWidget 序列化结果（approval_provision.rs:143-148）：后者会丢 printable/display_condition 等字段，定义变更比对就失效。
- FormWidgetRaw 的 #[serde(rename = "externalData")]（approval_match.rs:129-133）一旦丢字，链接态会被静默判成非链接，错误方向完全相反且没有任何报错。
- WidgetOptions 的 untagged 表达（60-67）不能加单元变体或调整变体顺序：单元变体会抢在数组/对象前匹配，把 Fixed/Config 全部挡死（注释 54-59 已记录该坑）。
- approval_match.rs:324 的 /api/v1/feishu/approval/options/ 与 approval_convert.rs:822 的 appId cli_9cb844403dbb9108 都是宿主/租户常量；原样搬进 crate 会让通用库绑定本产品，换租户即静默失效。
- approval_attachment.rs:200 的 bitablePerm extra 参数解耦是行为敏感点：拼错不会报配置错，只会在下载阶段持续 403/重试，把派发卡成可重试死循环。
- 附件上传无幂等键（approval_attachment.rs:238-239），重试会重复上传；拆分 drive 时不能顺手改成可重试幂等。200ms 串行节流（199 行）对应官方 5 QPS 配额，也不能删。
- ConvertError/MatchError 的 Display 是写进多维表格的中文产品文案；移入 crate 后文案与产品耦合在库内，宿主无法按产品覆写。若需多产品复用，应改为宿主渲染错误码。
- Attachment 三字段目前私有（17-22），拆模块前不改 pub 会直接编不过；同时 preflight 的占位 code 与 transfer 的真实 code 语义不同，合并两者会破坏「校验通过才产生外部副作用」的时序（approval_attachment.rs:70-73）。
- crate 仍需 outbound 传输与 tenant token 两个宿主能力；若这两个层不在同一批次抽取，crate 会反向依赖 yang-system 或被迫引入 yang_base —— 建议本批只抽纯逻辑（widget_rules/form/value），drive 与 client 等 trait 化后再迁。
- approval_convert.rs:1279-1288 的两个薄包装与 widget_rules 同义，迁移后若两处都保留会出现同名双入口，后续改动只改一处造成配置期/派发期判据漂移。
- 测试面较大：approval_match 的解析测试与匹配测试共用夹具（1032-1160），一次性整块搬走会让宿主测试缺夹具；需要拆夹具或各留一份。

## A.6 crates/yang-feishu — L1 通讯录 / IM / 云盘（contact / im / drive）

**目标**：把散落在 approval.rs（通讯录 contact/search）、approval_notification.rs（IM 消息）、approval_attachment.rs（drive 媒体搬运）里的飞书平台契约切成 yang-feishu 的 contact / im / drive 三个独立模块。工作区没有 get_department_name / search_user 这两个符号：等价物是 approval.rs:437 的 resolve_department_name（部门）与 approval.rs:749 + 722 的 resolve_applicant / applicant_search_url（用户搜索）。通用 HTTP 形状（URL、query、body、2s 超时、uuid、multipart、二进制通道）随迁；产品编排（WidgetMap、BitableCoordinates、ConvertError、FeedbackStatus、NotificationResult/NotificationStatus 及全部中文文案）留在宿主做薄适配层。前置依赖：outbound/tenant_token/auth 三个 L0 模块必须先落进 crate，且要先把 approval.rs 的私有 send_approval 抽成共享 send_auth…

### 源文件

| 文件 | 生产行区间 | 生产行数 | 测试行区间 | 测试行数 |
|---|---|---:|---|---:|
| src/addon/feishu/domain/approval.rs | 86-98, 400-501, 635-693, 704-843 (L1 相关；… | 314 | 1180-1550 (L1 相关；tests 模块自 845 起) | 370 |
| src/addon/feishu/domain/approval_notification.rs | 1-447 | 447 | 449-1388 | 940 |
| src/addon/feishu/domain/approval_attachment.rs | 1-261 | 261 | 262-447 | 186 |

### 符号（51）

| # | 符号 | 类型 | 位置 | 处置 | 备注 |
|---:|---|---|---|---|---|
| 1 | `validate_path_segment` | fn | src/addon/feishu/domain/approval.rs:86 | **move-with-refactor** | 路径段校验；bitable.rs:66 已有同名副本。抽成 yang-feishu::path::validate_segment 供 approval/contact 共用，否则 contact 会复制第三份。 |
| 2 | `department_url` | fn | src/addon/feishu/domain/approval.rs:405 | **move-as-is** | GET /open-apis/contact/v3/departments/{id}。crate 中改 pub；无产品类型。 |
| 3 | `department_id_type` | fn | src/addon/feishu/domain/approval.rs:417 | **move-as-is** | od- 前缀 → open_department_id，其余 → department_id。2026-10-15 真机验证的判据，勿改。 |
| 4 | `resolve_department_name` | fn | src/addon/feishu/domain/approval.rs:437 | **move-with-refactor** | 纯通讯录逻辑，签名不绑任何产品类型。阻塞点：调用 approval.rs 的私有 send_approval（635-673），且依赖 outbound.rs 的部门专用常量。失败一律 None+warn，不阻塞派发。crate 中改 pub。 |
| 5 | `applicant_search_url` | fn | src/addon/feishu/domain/approval.rs:722 | **move-as-is** | GET /open-apis/search/v1/user（query 参数）。工作区无 search_user；本函数 + resolve_applicant 即其等价物。禁用 contact/v3/users/search——它把姓名埋进 display_info HTML，会静默解析成空串。 |
| 6 | `ApplicantResolution` | type | src/addon/feishu/domain/approval.rs:727 | **move-as-is** | 纯结果类型，无产品绑定。crate 中改 pub。 |
| 7 | `resolve_applicant` | fn | src/addon/feishu/domain/approval.rs:749 | **move-with-refactor** | 只接受 user_access_token（tenant token 返回 99991663）；空姓名直接 NotFound 不出站；不做 tenant token 刷新循环。decode_data 需先抽到 crate envelope 模块。crate 中改 pub。 |
| 8 | `ApplicantSearchData` | type | src/addon/feishu/domain/approval.rs:798 | **move-as-is** | serde(default, alias = "users") 同时收 users/items 两种列表键。 |
| 9 | `SearchUser` | type | src/addon/feishu/domain/approval.rs:808 | **move-as-is** | name 带 alias = localized_name。字段名漂移会被 serde(default) 静默成空串，必须保留请求形状测试。 |
| 10 | `classify_applicants` | fn | src/addon/feishu/domain/approval.rs:821 | **move-as-is** | 纯函数：精确唯一命中 / 重名 Ambiguous / 整页单模糊命中 Resolved / 其余 NotFound。 |
| 11 | `decode_data` | fn | src/addon/feishu/domain/approval.rs:676 | **move-with-refactor** | 通用信封 data 提取，approval 与 contact 共用；抽到 crate envelope 模块，两份调用点统一。 |
| 12 | `send_approval` | fn | src/addon/feishu/domain/approval.rs:635 | **move-with-refactor** | tenant token 获取 + 失效刷新一次 + send_with_retry + 解信封。resolve_department_name 直接依赖它，必须先抽成 crate 级 send_authorized，否则 contact 迁不走。 |
| 13 | `DEPARTMENT_RETRY` | const | src/addon/feishu/domain/outbound.rs:585 | **move-as-is** | max_attempts=1（best-effort 富化，不能拖住主产物）。仅部门名解析使用，随 contact 迁移。 |
| 14 | `DEPARTMENT_REQUEST_TIMEOUT_SECS` | const | src/addon/feishu/domain/outbound.rs:593 | **move-as-is** | 5s 超时；仅部门名解析使用，随 contact 迁移。 |
| 15 | `fill_department_name` | fn | src/addon/feishu/domain/approval_dispatch.rs:1482 | **stay-in-host** | 产品编排：只在配置了部门回填列且已有编号时才出站；绑定 InstanceDetail（approval.rs:205，department_id :230 / department_name :239）。 |
| 16 | `resolve_initiator` | fn | src/addon/feishu/approval/actions/dispatch.rs:871 | **stay-in-host** | 用户身份授权（user_access_token）是宿主关注点，不能进通讯录模块。 |
| 17 | `resolve_applicant_param` | fn | src/addon/feishu/approval/actions/dispatch.rs:893 | **stay-in-host** | 把 ApplicantResolution 翻成可行动中文文案的唯一落点（:901-908）。 |
| 18 | `applicant_failure_message` | fn | src/addon/feishu/approval/actions/dispatch.rs:916 | **stay-in-host** | 权限码 → 产品文案（通讯录 scope 未开通 vs 用户重新授权 vs 凭证无效）。 |
| 19 | `ReceiveIdType` | type | src/addon/feishu/domain/approval_notification.rs:17 | **move-as-is** | serde rename_all = snake_case。随 im 迁移并改 pub；宿主 re-export 别名保持调用点不变。 |
| 20 | `ReceiveIdType::as_str` | fn | src/addon/feishu/domain/approval_notification.rs:25 | **move-as-is** | 产出 receive_id_type 查询参数值（open_id/user_id/union_id/email）。 |
| 21 | `NotificationRecipient` | type | src/addon/feishu/domain/approval_notification.rs:37 | **move-with-refactor** | 改名 im::MessageRecipient。保留 deny_unknown_fields 与 JsonSchema（Action 入参），并加 #[schemars(rename = "NotificationRecipient")] 防 OpenAPI 组件名漂移；宿主 approval_notification.rs 用 pub(crate) use 别名。 |
| 22 | `NotificationRecipient::validate` | fn | src/addon/feishu/domain/approval_notification.rs:42 | **move-as-is** | open_id/union_id 前缀、user_id ASCII、email 形态、≤320、禁空白/控制字符。crate 因此依赖 yang-base。 |
| 23 | `NotificationStatus` | type | src/addon/feishu/domain/approval_notification.rs:69 | **stay-in-host** | Action 响应词汇，不是 IM 契约。 |
| 24 | `NotificationResult` | type | src/addon/feishu/domain/approval_notification.rs:77 | **stay-in-host** | 带中文文案与 JSON Schema，属于宿主 Action 响应；uuid 用于本轮去重（飞书只保证 1 小时）。 |
| 25 | `NotificationResult::not_requested` | fn | src/addon/feishu/domain/approval_notification.rs:88 | **stay-in-host** | 产品文案：未指定接收人，完整反馈已通过 HTTP 返回。 |
| 26 | `NotificationResult::deferred_to_worker` | fn | src/addon/feishu/domain/approval_notification.rs:103 | **stay-in-host** | 产品文案：中间态交给 worker 轮询发送。 |
| 27 | `NotificationResult::skipped` | fn | src/addon/feishu/domain/approval_notification.rs:113 | **stay-in-host** | 产品文案：参数/配置未通过校验。 |
| 28 | `NotificationResult::unavailable` | fn | src/addon/feishu/domain/approval_notification.rs:121 | **stay-in-host** | 产品文案：出站配置不可用且审批结果不受影响。 |
| 29 | `MESSAGE_RETRY` | const | src/addon/feishu/domain/approval_notification.rs:131 | **move-as-is** | max_attempts=3 / 250ms 起 / 1000ms 封顶。IM 专用，随 im 迁移。 |
| 30 | `FeedbackReference` | type | src/addon/feishu/domain/approval_notification.rs:140 | **stay-in-host** | 标签（多维表/原记录/审批）是产品语义，只被 post_content 消费；不随 IM 迁移。 |
| 31 | `approval_reference` | fn | src/addon/feishu/domain/approval_notification.rs:150 | **stay-in-host** | 审批编号/编号待生成文案 + applink 生成，绑定审批产品语义。 |
| 32 | `send_feedback` | fn | src/addon/feishu/domain/approval_notification.rs:175 | **split** | 宿主保留外层 10s 超时（:185-191）、失败分类文案（:192-211）与 NotificationResult。uuid 在 :183 生成、失败路径 :216 仍要回传，所以必须由宿主生成后传入 crate，crate 不得内部生成。 |
| 33 | `post_content` | fn | src/addon/feishu/domain/approval_notification.rs:221 | **stay-in-host** | 解析 approval_feedback::render 的中文正文（记录/数据表/点击时间/原因/下一步/技术定位），并按标题挑「结果/原因」（:282-284）。绑定反馈状态机与产品文案，整段留下；只把最终 {"zh_cn":{...}} 信封的序列化交给 im。 |
| 34 | `send_post` | fn | src/addon/feishu/domain/approval_notification.rs:320 | **split** | URL :334；receive_id_type query :335-338；post body :345-347（receive_id/msg_type=post/content/uuid）；2s :350；idempotent :351。通用 HTTP 形状抽到 im::send_post(recipient, content: &Value, uuid)；content 由宿主 post_content 构造。 |
| 35 | `send_message_request` | fn | src/addon/feishu/domain/approval_notification.rs:364 | **move-as-is** | code==0 确认（:376-378）+ token 失效刷新一次（:381-389）+ 原始响应体绝不外抛（:375/:377）。与 send_approval / attachment::send 属同一模式，抽取时合并为 send_authorized。 |
| 36 | `edit_feedback` | fn | src/addon/feishu/domain/approval_notification.rs:399 | **split** | PUT /open-apis/im/v1/messages/:id :414；2s :425；idempotent :426。通用 PUT 抽到 im::edit_post(message_id, content: &Value)；宿主保留 content 构造与 20 次编辑限制说明。 |
| 37 | `valid_message_id` | fn | src/addon/feishu/domain/approval_notification.rs:434 | **move-as-is** | ASCII 字母数字 + _ -，≤128；防止 message_id 注入 URL 路径。 |
| 38 | `invalid_response` | fn | src/addon/feishu/domain/approval_notification.rs:442 | **move-as-is** | 固定文案「消息调用未获得有效确认」，Fatal{code:0}，不泄露原始响应。 |
| 39 | `UPLOAD_URL` | const | src/addon/feishu/domain/approval_attachment.rs:15 | **move-as-is** | 注意主机是 www.feishu.cn（审批开放接口），不是 open.feishu.cn；字符串逐字保留。 |
| 40 | `Attachment` | type | src/addon/feishu/domain/approval_attachment.rs:18 | **move-as-is** | crate 中字段改 pub；纯文件元数据，无产品绑定。 |
| 41 | `attachments` | fn | src/addon/feishu/domain/approval_attachment.rs:24 | **split** | 通用解析/校验（:47-67）抽成 drive::parse_attachments(value: &Value, limit: u64) -> Result<Vec<Attachment>, AttachmentIssue>；宿主 wrapper 继续负责 widget_id、图片/附件中文 label 与 ConvertError::BadValue。 |
| 42 | `preflight` | fn | src/addon/feishu/domain/approval_attachment.rs:71 | **stay-in-host** | 只产生 "preflight" 占位 code 让 build_form 先校验，禁止网络副作用。绑定产品映射。 |
| 43 | `source_identities` | fn | src/addon/feishu/domain/approval_attachment.rs:95 | **stay-in-host** | 聚合签名契约 `<file_token>\|<name>\|<size>`（:108），属于宿主审批聚合，不可抽走。 |
| 44 | `multipart` | fn | src/addon/feishu/domain/approval_attachment.rs:116 | **move-with-refactor** | boundary feishu-<uuid>，:123 拼 name/type/content 三个 part。当前 name 只在 attachments(:48-66) 校验；抽成 pub 原语后必须在 multipart 内复用同一校验/转义，否则 CRLF 可注入 part 头。 |
| 45 | `image_content_type` | fn | src/addon/feishu/domain/approval_attachment.rs:130 | **move-as-is** | png/jpg/jpeg/gif/webp/bmp/heic → image/*；猜不到回退 application/octet-stream。 |
| 46 | `send` | fn | src/addon/feishu/domain/approval_attachment.rs:145 | **move-with-refactor** | 第三份「取 token + 失效刷新一次」逻辑；用 crate 级 send_authorized 替代，不再保留 drive 私有副本。 |
| 47 | `retry` | fn | src/addon/feishu/domain/approval_attachment.rs:173 | **move-as-is** | 固定 Retry 失败构造；随 drive 迁移。 |
| 48 | `transfer` | fn | src/addon/feishu/domain/approval_attachment.rs:182 | **split** | 签名强绑产品类型：coordinates: &BitableCoordinates(:186)、widgets: &[WidgetMap](:188)，并构造 bitablePerm extra(:200)。拆分：通用下载 → drive::download_media(:197-224)，通用审批上传 → drive::upload_approval_file(:225-253)；宿主保留 widget 循环、200ms 配额 pacing 位置与 widget_id→codes 映射(:191-259)。 |
| 49 | `download_media (to extract)` | fn | src/addon/feishu/domain/approval_attachment.rs:197-224 | **split** | GET /open-apis/drive/v1/medias/{token}/download(:207-210)；extra 走 query 对(:211)；binary_limit=limit(:215)；120s(:216)；200ms 下载配额 pacing(:199)；要求 status==200 且 bytes.len()==file.size(:221-224)。 |
| 50 | `upload_approval_file (to extract)` | fn | src/addon/feishu/domain/approval_attachment.rs:225-253 | **split** | POST UPLOAD_URL(:231)；raw multipart body(:235)；120s(:237)；idempotent=false(:239，上传无幂等键，重试交给任务下一轮)；解析 data.code 且顶层 code==0(:245-252)。 |
| 51 | `InstanceDetail` | type | src/addon/feishu/domain/approval.rs:205 | **stay-in-host** | department_id(:230)/department_name(:239) 只由宿主 fill_department_name 填充；contact crate 不应感知这个审批类型。 |

### 需先重构

- **把三处「tenant token + 失效刷新一次 + send_with_retry」合并为 crate 级 send_authorized(transport, sleeper, tokens, request, retry) -> Result<OutboundResponse, OutboundFailure>** — resolve_department_name 目前直接调用 approval.rs 的私有 send_approval（contact 无法独立迁出）；notification 的 send_message_request 与 attachment 的 send 是同一逻辑的另外两份复制，且各自错误文案不同（`approval.rs:635-673; approval_notification.rs:364-393; approval_attachment.rs:145-171`）
- **把路径段校验抽成 yang-feishu::path::validate_segment，approval 与 contact 共用** — department_url 依赖 approval.rs:86 的私有副本，bitable.rs:66 已有第二份；不抽则 contact 出现第三份（`approval.rs:86-98; bitable.rs:66`）
- **把信封 data 提取抽成 crate 级 envelope::decode_data** — resolve_applicant 与 resolve_department_name 都要解飞书信封；现在前者用 approval.rs 私有函数、后者经 send_approval 间接使用（`approval.rs:676-693`）
- **把 DEPARTMENT_RETRY / DEPARTMENT_REQUEST_TIMEOUT_SECS 从 outbound.rs 迁到 contact 模块** — 两者只服务部门名解析，放在通用出站层会让 L0 模块背 L1 语义（`outbound.rs:585-593`）
- **NotificationRecipient 改名 MessageRecipient，FeedbackReference 留在宿主；宿主用 pub(crate) use 别名保住全部调用点** — crate API 不应出现审批词汇；但 Action 入参与 worker 有 20+ 处按旧名引用（dispatch.rs:162、approval_execution/mod.rs:16、feishu_approval_worker.rs:71），别名可零改动（`approval_notification.rs:35-40, 138-144`）
- **拆 send_feedback：宿主保留 10s 外层超时、失败文案映射与 NotificationResult，内部改调 im::send_post(recipient, content, uuid)** — send_feedback 名称/返回类型和中文失败原因都是产品语义；只有 HTTP 形状是飞书契约（`approval_notification.rs:175-218, 320-359`）
- **拆 edit_feedback：通用 PUT 抽成 im::edit_post(message_id, content)，宿主保留 post_content 与 references 参数** — edit_feedback 的签名含 FeedbackReference 且 content 由产品正文解析产生；HTTP 部分（:414、:425）与产品无关（`approval_notification.rs:399-431`）
- **把 attachments() 的通用解析/校验抽成 drive::parse_attachments(value, limit)，宿主 wrapper 负责 WidgetMap→ConvertError** — WidgetMap/ConvertError 是审批映射产品类型；Attachment 三元组与 file_token/name/size 校验是飞书附件契约（`approval_attachment.rs:24-68`）
- **把 transfer 拆成 drive::download_media(...) 与 drive::upload_approval_file(...)，宿主保留 bitablePerm extra 与 widget_id→codes 编排** — transfer 签名绑定 BitableCoordinates/WidgetMap；下载 URL、二进制通道、multipart、上传 URL 均与产品无关（`approval_attachment.rs:182-260`）
- **multipart 原语内部复用 attachments 的 name 校验（控制字符、引号、反斜杠、斜杠、扩展名）** — 现在 name 只在校验器里被拦；一旦 multipart/upload_approval_file 变成 pub，未校验 name 会直接拼进 Content-Disposition，可 CRLF 注入 part 头（`approval_attachment.rs:48-66, 116-127`）
- **download_media 原语内部校验 file_token（1-128 ASCII alphanumeric）** — 现在 file_token 只在校验器里被拦；公开原语若直接拼进 URL 路径，可注入 /../ 或查询串（`approval_attachment.rs:49-51, 207-210`）
- **把测试替身 ScriptedTransport/NoopSleeper/Replay/StaleCache/provider 抽成 crate 的 test_support（或复制一份到 crate tests）** — contact/IM 的通用测试要迁进 crate，但宿主 approval/product 测试仍需要这些替身；共享模块不存在会重复三份（`approval.rs:1180-1230; approval_notification.rs:763-861`）

### 目标公开 API

| 名称 | 签名 | 备注 |
|---|---|---|
| `contact::resolve_department_name` | `pub async fn resolve_department_name(transport: &dyn OutboundTransport, sleeper: &dyn Sleeper, tokens: &TenantTokenProvider, department_id: &str) -> Option<String>` | get_department_name 的等价物。返回 None 表示无 scope/不可见/响应缺 name，调用方退化写原始 ID；crate 内部 warn。 |
| `contact::resolve_applicant` | `pub async fn resolve_applicant(transport: &dyn OutboundTransport, sleeper: &dyn Sleeper, bearer_token: &str, name: &str) -> Result<ApplicantResolution, OutboundFailure>` | search_user 的等价物。只接受 user_access_token；空姓名直接 NotFound 不出站；第一页 page_size=20。 |
| `contact::ApplicantResolution` | `pub enum ApplicantResolution { Resolved { open_id: String }, NotFound, Ambiguous { candidates: Vec<String> } }` | 宿主 dispatch.rs:899 直接 match；candidates 文案格式「姓名(open_id)」在 crate 内生成。 |
| `contact::applicant_search_url` | `pub fn applicant_search_url() -> String` | GET /open-apis/search/v1/user；测试用它钉住接口路径。 |
| `contact::department_url` | `pub fn department_url(department_id: &str) -> Result<String, PathError>` | 拼 GET /open-apis/contact/v3/departments/{id}；非法 ID 返回错误，由 resolve_department_name 转 None。 |
| `im::send_post` | `pub async fn send_post(transport: &dyn OutboundTransport, sleeper: &dyn Sleeper, tokens: &TenantTokenProvider, recipient: &MessageRecipient, content: &serde_json::Value, uuid: &str) -> Result<String, OutboundFailure>` | POST /open-apis/im/v1/messages?receive_id_type=...；body {receive_id,msg_type:post,content:<JSON 字符串>,uuid}；请求超时 2s；3 次重试沿用同一 uuid。返回 message_id；uuid 由宿主传入以保留失败路径回传。 |
| `im::edit_post` | `pub async fn edit_post(transport: &dyn OutboundTransport, sleeper: &dyn Sleeper, tokens: &TenantTokenProvider, message_id: &str, content: &serde_json::Value) -> Result<(), OutboundFailure>` | PUT /open-apis/im/v1/messages/{message_id}；2s；idempotent；先校验 message_id 形态。 |
| `im::MessageRecipient` | `pub struct MessageRecipient { pub receive_id_type: ReceiveIdType, pub receive_id: String }` | 从 NotificationRecipient 迁移并改名；保留 deny_unknown_fields + JsonSchema，建议 #[schemars(rename = "NotificationRecipient")] 保住 Action schema 名。 |
| `im::ReceiveIdType` | `pub enum ReceiveIdType { OpenId, UserId, UnionId, Email }` | serde snake_case；validate 校验 open_id/union_id 前缀、user_id ASCII、email 形态、≤320。 |
| `drive::download_media` | `pub async fn download_media(transport: &dyn OutboundTransport, sleeper: &dyn Sleeper, tokens: &TenantTokenProvider, file_token: &str, extra: Option<&str>, expected_size: u64, max_bytes: usize) -> Result<Vec<u8>, OutboundFailure>` | GET /open-apis/drive/v1/medias/{token}/download；extra 走 query 对而非拼 URL；binary_limit=max_bytes；120s；200ms pacing；要求 200 且长度等于 expected_size。 |
| `drive::upload_approval_file` | `pub async fn upload_approval_file(transport: &dyn OutboundTransport, sleeper: &dyn Sleeper, tokens: &TenantTokenProvider, name: &str, bytes: &[u8], file_type: &str) -> Result<String, OutboundFailure>` | POST https://www.feishu.cn/approval/openapi/v2/file/upload；multipart；120s；idempotent=false；返回 data.code（file code）。 |
| `drive::parse_attachments` | `pub fn parse_attachments(value: &serde_json::Value, limit: u64) -> Result<Vec<Attachment>, AttachmentIssue>` | AttachmentIssue = NotArray \| InvalidToken \| InvalidName \| InvalidSize；宿主 wrapper 据此构造既有中文 ConvertError::BadValue。 |
| `drive::multipart` | `pub fn multipart(name: &str, bytes: &[u8], file_type: &str) -> (String, Vec<u8>)` | 返回 (content_type, body)；boundary feishu-<uuid>；必须在内部校验/转义 name，不能只依赖上游 parse_attachments。 |
| `drive::image_content_type` | `pub fn image_content_type(name: &str) -> &'static str` | 扩展名猜测；非图片或未知扩展返回 application/octet-stream。 |
| `drive::Attachment` | `pub struct Attachment { pub file_token: String, pub name: String, pub size: u64 }` | 宿主 source_identities 仍按 `<token>\|<name>\|<size>` 拼签名。 |
| `outbound::send_authorized (前置重构)` | `pub async fn send_authorized(transport: &dyn OutboundTransport, sleeper: &dyn Sleeper, tokens: &TenantTokenProvider, request: OutboundRequest, retry: RetryPolicy) -> Result<OutboundResponse, OutboundFailure>` | 取 tenant token、TokenExpired 时 invalidate_and_refresh 后重试一次。contact/drive/im 共用；不带 envelope 解码（contact/approval 自行 decode_data）。 |
| `envelope::decode_data (前置重构)` | `pub fn decode_data(response: &str) -> Result<String, OutboundFailure>` | 取出飞书响应信封的 data 并序列化为文本；approval 与 contact 共用。 |
| `path::validate_segment (前置重构)` | `pub fn validate_segment(name: &str, value: &str) -> Result<(), PathError>` | 非空、≤128、不含 / ? # 空格 \；approval 与 contact 共用，替代 bitable.rs:66 与 approval.rs:86 两份副本。 |

### 测试处置（47）

| 文件 | 区间 | 内容 | 处置 |
|---|---|---|---|
| src/addon/feishu/domain/approval.rs | 1242-1266 | resolve_applicant_matches_a_unique_exact_name：唯一精确命中 → Resolved，恰一次请求 | **move** |
| src/addon/feishu/domain/approval.rs | 1269-1315 | resolve_applicant_calls_the_documented_search_endpoint：钉住 GET /open-apis/search/v1/user、query=姓名、page_size=20、无 body、带调用方 bearer | **move** |
| src/addon/feishu/domain/approval.rs | 1317-1325 | resolve_applicant_returns_not_found_when_nothing_matches | **move** |
| src/addon/feishu/domain/approval.rs | 1327-1342 | resolve_applicant_reports_ambiguous_on_duplicate_names：候选串格式「姓名(open_id)」 | **move** |
| src/addon/feishu/domain/approval.rs | 1344-1360 | resolve_applicant_accepts_a_single_fuzzy_hit：无精确命中但整页单候选 | **move** |
| src/addon/feishu/domain/approval.rs | 1362-1378 | resolve_applicant_maps_missing_scope_to_fatal：99991672 → Fatal 且不重试 | **move** |
| src/addon/feishu/domain/approval.rs | 1380-1389 | resolve_applicant_skips_the_request_for_a_blank_name：空姓名零出站 | **move** |
| src/addon/feishu/domain/approval.rs | 1392-1413 | FakeTokenCache：TenantTokenCache 假实现（预置 t-fake） | **rewrite** |
| src/addon/feishu/domain/approval.rs | 1415-1429 | fake_tokens：脚本传输 + NoopSleeper + 假凭据构造 TenantTokenProvider | **move** |
| src/addon/feishu/domain/approval.rs | 1431-1444 | department_envelope：data.department.name 信封 | **move** |
| src/addon/feishu/domain/approval.rs | 1447-1474 | department_name_resolves_and_declares_the_id_type：GET contact/v3/departments/{id} + department_id_type=department_id | **move** |
| src/addon/feishu/domain/approval.rs | 1476-1499 | department_name_uses_open_department_id_for_od_prefix | **move** |
| src/addon/feishu/domain/approval.rs | 1501-1523 | department_name_degrades_to_none_without_authority：40004/99991672 都返回 None 且各只一次请求 | **move** |
| src/addon/feishu/domain/approval.rs | 1525-1539 | department_name_is_none_when_response_has_no_name | **move** |
| src/addon/feishu/domain/approval.rs | 1541-1550 | department_name_skips_the_request_for_blank_or_illegal_ids | **move** |
| src/addon/feishu/domain/approval.rs | 1180-1230 | ScriptedTransport + NoopSleeper 共享替身，同时服务审批 URL 测试与 contact 测试 | **rewrite** |
| src/addon/feishu/domain/approval.rs | 1232-1240 | envelope() 搜索响应构造器（items 形态） | **move** |
| src/addon/feishu/domain/approval_dispatch.rs | 3339-3379 | dispatch_one_skips_department_lookup_without_the_field：未配部门列时零 contact 出站 | **stay** |
| src/addon/feishu/domain/approval_dispatch.rs | 3381-3410 | recover_path_also_resolves_department_name：补回填链路也解析部门名 | **stay** |
| src/addon/feishu/approval/actions/dispatch.rs | 2693-2752 | applicant_param_* 四个用例：命中/未找到/重名/缺 scope 的产品文案 | **stay** |
| src/addon/feishu/approval/actions/dispatch.rs | 2753-2770 | initiator_prefers_applicant_and_falls_back_to_the_compat_alias | **stay** |
| src/addon/feishu/domain/approval_notification.rs | 460-510 | post_reference_footer_uses_native_links_after_next_step | **stay** |
| src/addon/feishu/domain/approval_notification.rs | 514-542 | approval_reference_links_the_serial_and_falls_back_to_text | **stay** |
| src/addon/feishu/domain/approval_notification.rs | 546-573 | post_content_links_only_the_trusted_serial_reference | **stay** |
| src/addon/feishu/domain/approval_notification.rs | 576-591 | post_content_is_compact_and_keeps_named_error_context | **stay** |
| src/addon/feishu/domain/approval_notification.rs | 597-613 | post_content_shows_the_technical_record_id | **stay** |
| src/addon/feishu/domain/approval_notification.rs | 615-629 | post_content_puts_the_click_time_under_the_record | **stay** |
| src/addon/feishu/domain/approval_notification.rs | 631-661 | post_content_keeps_plain_headings_and_dynamic_markup_literal | **stay** |
| src/addon/feishu/domain/approval_notification.rs | 663-679 | post_content_keeps_distinct_values_and_general_next_step | **stay** |
| src/addon/feishu/domain/approval_notification.rs | 681-719 | post_content_uses_each_business_conclusion_as_title | **stay** |
| src/addon/feishu/domain/approval_notification.rs | 722-761 | recipient_requires_explicit_type_and_valid_identifier：四种 receive_id_type 形态 + 超长 + 未知字段 | **move** |
| src/addon/feishu/domain/approval_notification.rs | 763-861 | Replay/NoSleep/StaleCache/provider/recipient 测试替身 | **rewrite** |
| src/addon/feishu/domain/approval_notification.rs | 864-919 | message_rate_limit_retries_same_uuid_and_sends_serialized_post：230020 重试、同一 uuid、receive_id_type、body、2s、idempotent | **rewrite** |
| src/addon/feishu/domain/approval_notification.rs | 924-1000 | edit_feedback_puts_post_to_the_same_message：PUT 到同一 message_id、msg_type=post、正文含行号/record_id | **rewrite** |
| src/addon/feishu/domain/approval_notification.rs | 1002-1047 | expired_token_refreshes_once_and_preserves_message_uuid | **move** |
| src/addon/feishu/domain/approval_notification.rs | 1049-1090 | message_requires_confirmation_and_bounds_retries_without_leaking_raw_body | **move** |
| src/addon/feishu/domain/approval_notification.rs | 1092-1143 | all_recipient_types_reach_im_without_search_and_malformed_confirmation_fails | **move** |
| src/addon/feishu/domain/approval_notification.rs | 1145-1173 | notification_timeout_is_bounded_and_does_not_claim_delivery：10s 总预算 + NotificationResult 文案「超时」 | **stay** |
| src/addon/feishu/domain/approval_notification.rs | 1175-1322 | real_app_sends_rich_feedback_and_confirms_sender_and_content（#[ignore] 真机） | **stay** |
| src/addon/feishu/domain/approval_notification.rs | 1324-1358 | assert_post_receipt：历史回执与请求逐字段比对 | **stay** |
| src/addon/feishu/domain/approval_notification.rs | 1360-1388 | saved_rich_feedback_receipt_preserves_title_content_styles_and_sender | **stay** |
| src/addon/feishu/domain/approval_attachment.rs | 267-280 | widget_map 测试构造器 | **stay** |
| src/addon/feishu/domain/approval_attachment.rs | 282-348 | attachment_metadata_rejects_invalid_tokens_names_sizes_and_shapes：file_token/name/size/形状四类拒绝 + 边界 | **rewrite** |
| src/addon/feishu/domain/approval_attachment.rs | 350-385 | image_preflight_accepts_images_and_enforces_ten_mib | **stay** |
| src/addon/feishu/domain/approval_attachment.rs | 387-410 | multipart_carries_upload_type_and_guessed_image_content_type | **move** |
| src/addon/feishu/domain/approval_attachment.rs | 412-445 | source_identities_keep_source_order_and_include_token_name_size | **stay** |
| src/addon/feishu/domain/approval_dispatch.rs | 2839-2880 | attachment_transfer_reaches_plain_and_detail_create_requests：钉住 /medias/{token}/download 与 approval upload URL | **stay** |

### 宿主替换点（21）

- `src/addon/feishu/domain/approval_dispatch.rs:32` → use yang_feishu::contact::resolve_department_name;
- `src/addon/feishu/domain/approval_dispatch.rs:1502` → detail.department_name = yang_feishu::contact::resolve_department_name(transport, sleeper, tokens, department_id).await;
- `src/addon/feishu/approval/actions/dispatch.rs:47` → use yang_feishu::contact::{resolve_applicant, ApplicantResolution};
- `src/addon/feishu/approval/actions/dispatch.rs:899` → match yang_feishu::contact::resolve_applicant(transport, sleeper, bearer_token, name).await { Ok(ApplicantResolution::Resolved { open_id }) => ..., Ok(NotFound) => ..., Ok(Ambiguous { candidates }) => ..., Err(failure) => ... }（错误文案 :901-935 原样保留）
- `src/addon/feishu/domain/approval_notification.rs:17-65` → pub(crate) use yang_feishu::im::{ReceiveIdType, MessageRecipient as NotificationRecipient};（NotificationStatus/NotificationResult 仍留本文件）
- `src/addon/feishu/domain/approval_notification.rs:131-136` → use yang_feishu::im::MESSAGE_RETRY;
- `src/addon/feishu/domain/approval_notification.rs:175-218` → 保留 fn send_feedback；uuid 仍在 :183 生成，外层仍包 tokio::time::timeout(10s)；内部改为 let content = post_content(text, references); match yang_feishu::im::send_post(transport, sleeper, tokens, recipient, &content, &uuid).await { Ok(message_id) => NotificationResult::Sent..., Err(failure) => 既有中文 reason 映射 }
- `src/addon/feishu/domain/approval_notification.rs:320-359` → 删除；由 yang_feishu::im::send_post(recipient, &post_content(text, references), uuid) 取代
- `src/addon/feishu/domain/approval_notification.rs:364-393` → 删除；由 crate 级 outbound::send_authorized（+ code==0 确认）取代
- `src/addon/feishu/domain/approval_notification.rs:399-431` → 保留 fn edit_feedback；内部改为 let content = post_content(text, references); yang_feishu::im::edit_post(transport, sleeper, tokens, message_id, &content).await
- `src/addon/feishu/domain/approval_notification.rs:221-318` → post_content 整段保留（FeedbackStatus::heading 与产品标签解析）；仅最终信封 json! 可换成 im 的构造，若 im::send_post 接收 &Value 则无需任何改动
- `src/addon/feishu/domain/approval_notification.rs:150-173` → approval_reference 原样保留；返回宿主 FeedbackReference，不随 im 迁移
- `src/addon/feishu/domain/approval_attachment.rs:15` → 删除本地常量；upload 逻辑内部使用 yang_feishu::drive::APPROVAL_FILE_UPLOAD_URL（宿主不再直接持有）
- `src/addon/feishu/domain/approval_attachment.rs:24-68` → 保留 fn attachments 作为宿主 wrapper：let files = yang_feishu::drive::parse_attachments(value, limit as u64).map_err(\|issue\| ConvertError::BadValue { widget_id, reason: format!(...按 issue 生成现有中文...) })?;
- `src/addon/feishu/domain/approval_attachment.rs:116-143` → 删除；使用 yang_feishu::drive::{multipart, image_content_type}
- `src/addon/feishu/domain/approval_attachment.rs:145-171` → 删除；使用 yang_feishu::outbound::send_authorized(transport, sleeper, tokens, request, PULL_RETRY)
- `src/addon/feishu/domain/approval_attachment.rs:182-260` → transfer 保留 widget 循环与 widget_id→codes 映射；每个 file：let extra = json!({"bitablePerm":{...}}).to_string(); let bytes = drive::download_media(transport, sleeper, tokens, &file.file_token, Some(&extra), file.size, limit).await?; let code = drive::upload_approval_file(transport, sleeper, tokens, &file.name, &bytes, upload_type(&widget.widget_type)).await?;
- `src/addon/feishu/domain/outbound.rs:585-593` → 删除 DEPARTMENT_RETRY / DEPARTMENT_REQUEST_TIMEOUT_SECS；改由 yang_feishu::contact 导出
- `src/addon/feishu/domain/approval.rs:400-501, 704-843` → 删除通讯录段；本文件只保留审批契约（create/detail/definition/subscribe）与审批专用的 send_approval（或改调 crate send_authorized）
- `src/infrastructure/feishu_approval_worker.rs:70-72` → 无需改动（宿主 approval_notification.rs 的 pub(crate) use 别名继续导出 FeedbackReference/NotificationRecipient/ReceiveIdType）；若去掉别名则改为 use yang_feishu::im::{MessageRecipient as NotificationRecipient, ReceiveIdType};
- `src/addon/feishu/domain/approval_row_edit.rs:151` → 无需改动；edit_feedback 宿主 wrapper 保留原签名

### 风险

- 前置阻塞：resolve_department_name 调用 approval.rs:635 的私有 send_approval。若不先抽 send_authorized + decode_data，contact 模块会把整个 approval.rs 拖进 crate。
- 前置阻塞：OutboundTransport / Sleeper / TenantTokenProvider / OutboundFailure / RetryPolicy 必须先存在于 yang-feishu（L0 层）；否则 L1 三个模块没有可依赖的底座。
- uuid 语义：send_feedback 在失败路径（approval_notification.rs:216）仍回传本轮 uuid 供去重。若让 crate 内部生成 uuid，失败时宿主拿不到值，worker 去重与 NotificationResult 会退化；必须由宿主传入。
- receive_id_type / NotificationRecipient 是 Action 入参并派生 JsonSchema；改名 MessageRecipient 会改变 OpenAPI 组件名。用 #[schemars(rename = "NotificationRecipient")] 或宿主别名保住外部契约。
- multipart 的 name 当前仅在 attachments()（:48-66）校验；一旦 multipart/upload_approval_file 成为 crate 公开原语，未校验 name 可经 Content-Disposition 注入 CRLF。校验必须随原语一起下沉。
- drive 下载 URL 直接拼 file_token（:207-210）；现在只靠 attachments() 的 ASCII 校验兜底。公开 download_media 后必须在函数内校验，否则 URL 路径注入。
- 二进制通道依赖 OutboundResponse.bytes 与 binary_limit：outbound.rs:251 设置 max_response_bytes，:281 仅在 binary_limit 且非 JSON 时返回 bytes。若 crate 的出站接口改成只回 String，transfer 的“长度等于 file.size”校验会静默失效。
- extra=bitablePerm 含引号、中文与 record_id，必须继续走 query 键值对（outbound.rs:230-236 的 queries 编码）；若在 crate 里手工拼 URL 会破坏编码并可能注入 & 分隔符。
- 上传 URL 主机是 www.feishu.cn/approval/openapi/v2/file/upload，不是 open.feishu.cn；与 drive 下载（open.feishu.cn）是两套主机，常量不可混用。
- resolve_applicant 只接受 user_access_token（tenant token 返回 99991663），且必须用 GET /open-apis/search/v1/user；改成 contact/v3/users/search 不会报错，只会把每条命中解析成空串，最终伪装成“查无此人”。
- 部门 ID 类型靠 od- 前缀启发式（approval.rs:417）：bot 形态非 od-、user 形态 od-。迁移时不要按接口来源推断，否则部门名解析会停在 99992357。
- post_content 绑定 FeedbackStatus::heading（approval_feedback.rs:39）与 render 的中文行格式（记录/数据表/点击时间/原因/下一步/技术定位）。若把正文解析一起搬进 crate，产品文案与状态机将反向依赖宿主类型。
- 10s 外层超时与 2s 单次超时是两个不同预算：MESSAGE_RETRY 3 次 × 2s + 退避 ≤1s 仍在 10s 内。迁移时不要合并或去掉任一层，否则“通知等待超时”的语义会变。
- 宿主的 ScriptedTransport/NoopSleeper/Replay 替身同时服务联系人与审批/产品测试；直接搬走会导致 approval.rs 余下测试编译失败，需在 crate test_support 复制一份。
- crate 需要对 yang-base（BaseError）与 schemars 的依赖；若新 crate 打算零 yang-base 依赖，则 MessageRecipient::validate 的返回类型要先改成 crate 自有错误类型，这会让宿主调用点增加一层 map_err。
- approval_attachment::transfer 的 `preflight`（:71）必须在真正上传前跑完，才能让 build_form 在无网络副作用的前提下先报数据错误；拆分时不能把 preflight 并进 transfer 的上传循环，否则会先上传再发现表单非法。

## A.7 L1 — yang-feishu::event（事件入站）与 yang-feishu::options（外部选项协议）

**目标**：把飞书事件的 challenge 应答/解密/来源校验，以及外部选项的请求响应 DTO、@i18n@ 装配、keyset 游标、AES-256-CBC 原语抽成 lib_yang 新 crate crates/yang-feishu 的 event 与 options 模块；事件落库、审批字段提取与全部 DB/Action/配置逻辑留在 yang-system 宿主。

### 源文件

| 文件 | 生产行区间 | 生产行数 | 测试行区间 | 测试行数 |
|---|---|---:|---|---:|
| src/addon/feishu/event/mod.rs | 1-33 | 33 |  | 0 |
| src/addon/feishu/event/actions/mod.rs | 1-24 | 24 |  | 0 |
| src/addon/feishu/event/actions/event_challenge.rs | 1-183 | 183 | 185-350 | 166 |
| src/addon/feishu/event/actions/receive_approval_event.rs | 1-299 | 299 | 301-353 | 53 |
| src/addon/feishu/domain/protocol.rs | 1-190 | 190 | 192-372 | 181 |
| src/addon/feishu/domain/i18n.rs | 1-125 | 125 | 127-322 | 196 |
| src/addon/feishu/domain/pagination.rs | 1-46 | 46 | 48-128 | 81 |
| src/addon/feishu/domain/crypto.rs | 1-234 | 234 | 236-508 | 273 |
| src/addon/feishu/domain/token.rs | 1-56 | 56 | 58-122 | 65 |
| src/addon/feishu/domain/option_write.rs | 1-213 | 213 | 215-248 | 34 |
| src/addon/feishu/option/actions/approval_options.rs | 1-779 | 779 | 781-1379 | 599 |
| src/addon/feishu/approval/domain/event_table.rs | 1-83 | 83 | 85-204 | 120 |
| src/config/mod.rs | 591-699 | 109 |  | 0 |

### 符号（59）

| # | 符号 | 类型 | 位置 | 处置 | 备注 |
|---:|---|---|---|---|---|
| 1 | `MODULE` | const | src/addon/feishu/event/mod.rs:15 | **stay-in-host** | 模块限定名 "feishu.event"，属于 yang-base ModuleSpec 装配，不进 crate。 |
| 2 | `build_module` | fn | src/addon/feishu/event/mod.rs:20-29 | **stay-in-host** | settings 为 None 时返回空 module（25-27）。这是宿主配置/装配策略，crate 不感知 FeishuSettings。 |
| 3 | `module_name` | fn | src/addon/feishu/event/mod.rs:31-33 | **stay-in-host** | 模块名构造，纯宿主装配。 |
| 4 | `register_all` | fn | src/addon/feishu/event/actions/mod.rs:13-24 | **stay-in-host** | 18-23 的 Arc::try_unwrap/unwrap_or_else 克隆是为绕过 register 的按值 FeishuContext；改为两个 register 都接 Arc<FeishuContext>，让 sink 只构造一次。 |
| 5 | `ChallengeRequest` | type | src/addon/feishu/event/actions/event_challenge.rs:24-26 | **stay-in-host** | 框架输入适配（裸 Value，避免反序列化失败），不是飞书契约。 |
| 6 | `impl ParamInput for ChallengeRequest` | type | src/addon/feishu/event/actions/event_challenge.rs:28-32 | **stay-in-host** | 框架 param 元数据为空集合，属宿主 Action 层。 |
| 7 | `event_challenge::register` | fn | src/addon/feishu/event/actions/event_challenge.rs:35-63 | **stay-in-host** | 读 settings.event_verification_token（40-42 无 token 不注册）与 event_encrypt_key（44），声明 POST /api/v1/feishu/event/challenge 与 public()（46-62）。改造成构造 Arc<dyn EventSink> 并捕获，不再把 context 逐请求 clone。 |
| 8 | `event_challenge::handle` | fn | src/addon/feishu/event/actions/event_challenge.rs:75-176 | **split** | 逐行：AES 解密 85-125（取 encrypt 85、derive_key 91、decrypt_bytes 92、解析 JSON 93、失败 102-113）；明文拒绝 118-122；type/challenge/token 提取 128-130；event_callback 委托 133-147（传 encrypt_key=None 防二次误判，141-144）；challenge 缺失 150-156；type != url_verification 159-165；verification_toke… |
| 9 | `event_challenge::raw_json` | fn | src/addon/feishu/event/actions/event_challenge.rs:179-183 | **stay-in-host** | 飞书只认裸 {"challenge":"..."}；ResponseBody::raw 是框架传输能力，不进 crate。与 receive 的同名函数重复，属既有冗余，不影响抽取。 |
| 10 | `EventRequest` | type | src/addon/feishu/event/actions/receive_approval_event.rs:23-28 | **stay-in-host** | 框架输入适配，crate 的 decode_* 直接收 &Value，不需要这个 wrapper。 |
| 11 | `impl ParamInput for EventRequest` | type | src/addon/feishu/event/actions/receive_approval_event.rs:30-34 | **stay-in-host** | 同 ChallengeRequest，属框架输入契约。 |
| 12 | `receive_approval_event::register` | fn | src/addon/feishu/event/actions/receive_approval_event.rs:37-65 | **stay-in-host** | 43-45 无 verification_token 就不注册（公开端点无从校验）；声明 POST /api/v1/feishu/approval/event 与 public()。改造同 challenge：构造一次 EventSink。 |
| 13 | `receive_approval_event::handle` | fn | src/addon/feishu/event/actions/receive_approval_event.rs:79-292 | **split** | 逐行：解密或明文 98-145（加密 98-134、明文拒绝 137-141）；verification_token fail-closed 148-162（158 比对，165-167 从 body 删 token）；事件字段提取 169-208：event 对象 175、事件 id=顶层 uuid 176、event_type 177-179、instance_code 180-182、类型白名单+身份校验 184-195（184 顶层 type==event_callback、185 matches appr… |
| 14 | `receive_approval_event::raw_json` | fn | src/addon/feishu/event/actions/receive_approval_event.rs:295-299 | **stay-in-host** | 成功 ACK 是宿主自定义形状（{"success":true[, "duplicate":true]}）；飞书只读 HTTP 状态。 |
| 15 | `FeishuOptionsRequest` | type | src/addon/feishu/domain/protocol.rs:23-57 | **move-with-refactor** | 字段：user_id 31、employee_id 37、token 39、linkage_params 43、page_token 46、query 49、locale 56；刻意不 deny_unknown_fields（15-17）。反向依赖仅两处：10 行的 use 与 59-64 的 impl ParamInput。移动时字段 pub(crate)→pub，impl 留宿主新类型（孤儿规则不允许宿主为外部类型实现外部 trait）。 |
| 16 | `impl ParamInput for FeishuOptionsRequest` | type | src/addon/feishu/domain/protocol.rs:59-64 | **stay-in-host** | 唯一反向依赖点之一。crate 不得保留此 impl：ParamInput 与 FeishuOptionsRequest 在宿主侧都是外部类型，违反孤儿规则。宿主用 #[serde(transparent)] newtype ApprovalOptionsInput 实现 ParamInput，handler 改收 newtype。 |
| 17 | `FeishuResultBody` | type | src/addon/feishu/domain/protocol.rs:67-80 | **move-as-is** | 键 i18nResources 73、hasMore 76、nextPageToken 79（None 时省略）。纯 serde，无 yang-base 依赖。 |
| 18 | `FeishuResultBody::empty` | fn | src/addon/feishu/domain/protocol.rs:82-96 | **move-as-is** | #[cfg(test)] 构造器（87），仅测试用；随 DTO 一起进 crate 的 cfg(test)。 |
| 19 | `FeishuOption` | type | src/addon/feishu/domain/protocol.rs:98-108 | **move-as-is** | id/value/isDefault（camelCase，106）。纯 serde。 |
| 20 | `FeishuI18nResource` | type | src/addon/feishu/domain/protocol.rs:110-120 | **move-as-is** | locale/isDefault/texts；texts 键必须是 @i18n@<option_id>。 |
| 21 | `FeishuResult` | type | src/addon/feishu/domain/protocol.rs:122-130 | **move-as-is** | #[serde(untagged)]：未配 Key 是对象，配 Key 是 base64 字符串（124-129）。 |
| 22 | `FeishuData` | type | src/addon/feishu/domain/protocol.rs:132-137 | **move-as-is** | data.result 包装。 |
| 23 | `FeishuEnvelope` | type | src/addon/feishu/domain/protocol.rs:139-151 | **move-as-is** | 顶层 {code,msg,data}，msg 不是框架的 message（139-142）。 |
| 24 | `FeishuEnvelope::ok` | fn | src/addon/feishu/domain/protocol.rs:153-163 | **move-as-is** | 成功明文，msg 固定 "success!"。 |
| 25 | `FeishuEnvelope::encrypted` | fn | src/addon/feishu/domain/protocol.rs:166-174 | **move-as-is** | 成功加密（base64 IV‖密文）。 |
| 26 | `FeishuEnvelope::fail` | fn | src/addon/feishu/domain/protocol.rs:177-183 | **move-as-is** | 业务失败；code 由宿主定义（见 approval_options::codes），crate 只承载。 |
| 27 | `FeishuEnvelope::to_json` | fn | src/addon/feishu/domain/protocol.rs:186-189 | **move-with-refactor** | 第二个 yang-base 反向依赖点：返回 BaseError::JsonSerializeFailed。改为 crate 的 Error::Config 或直接 Result<String, serde_json::Error>；宿主映射回 BaseError。 |
| 28 | `DEFAULT_LOCALE` | const | src/addon/feishu/domain/i18n.rs:13 | **move-as-is** | "zh_cn"；读端 default_locale 缺失时的兜底（approval_options.rs:500-502）。 |
| 29 | `i18n_key` | fn | src/addon/feishu/domain/i18n.rs:18-20 | **move-as-is** | 由 option_id 生成 @i18n@<id>；options[].value 与 i18nResources.texts 的键必须同源（两处调用在 build_result_body 的 77 与 108）。 |
| 30 | `I18N_PREFIX` | const | src/addon/feishu/domain/i18n.rs:24 | **move-as-is** | "@i18n@"，拼/剥两侧共用。 |
| 31 | `normalize_linkage_value` | fn | src/addon/feishu/domain/i18n.rs:39-44 | **move-as-is** | 有前缀才剥、没前缀原样用、两端 trim（41-43）。真机回传文案（如「成都」）也走这条；解析成父键仍是宿主 approval_options::resolve_parent_key 的事。 |
| 32 | `OptionRow` | type | src/addon/feishu/domain/i18n.rs:47-59 | **move-as-is** | 协议装配的中间行；sort_order 不被 i18n 消费，供宿主游标编码。 |
| 33 | `build_result_body` | fn | src/addon/feishu/domain/i18n.rs:67-125 | **move-as-is** | @i18n@ 装配核心：options 73-80、语言集合=默认∪额外并字典序 82-91（默认恒存在）、每语言同键集且缺翻译回退默认文案 93-117、输出 119-124。零 yang-base 依赖。 |
| 34 | `SEPARATOR` | const | src/addon/feishu/domain/pagination.rs:15 | **move-as-is** | ASCII US 0x1F，避免 option_id 中的可打印字符冲突。 |
| 35 | `pagination::BASE64` | const | src/addon/feishu/domain/pagination.rs:18-19 | **move-as-is** | URL-safe 无填充。 |
| 36 | `encode_cursor` | fn | src/addon/feishu/domain/pagination.rs:22-25 | **move-as-is** | keyset 游标：base64(sort_order ‖ US ‖ option_id)。宿主 approval_options.rs:702-709 与 755-758 是消费者。 |
| 37 | `decode_cursor` | fn | src/addon/feishu/domain/pagination.rs:34-46 | **move-with-refactor** | 空/空白→None 37-38；非法 base64/非 UTF-8/缺分隔符/排序键非整数一律 fail-closed 41-45，现返回 BaseError::ParamInvalid("page_token")。改为 crate Error::InvalidCursor；宿主 approval_options.rs:516-522 仍映射成信封码 40002，不冒泡成 BaseError。 |
| 38 | `BLOCK_SIZE` | const | src/addon/feishu/domain/crypto.rs:24 | **move-as-is** | AES 块 16 字节，IV 长度与 PKCS#7 块大小共用。 |
| 39 | `crypto::BASE64` | const | src/addon/feishu/domain/crypto.rs:27-28 | **move-as-is** | 飞书 AES 输出用 STANDARD（不是游标的 URL_SAFE）。 |
| 40 | `Aes256CbcEnc / Aes256CbcDec` | type | src/addon/feishu/domain/crypto.rs:30-32 | **move-as-is** | 类型别名随 AES 原语移动；解密方向只给事件入站/凭据回显。 |
| 41 | `derive_key` | fn | src/addon/feishu/domain/crypto.rs:37-42 | **move-as-is** | 密钥=sha256(配置原文)，不对 hex/base64/UTF-8 形态做猜测。四处消费者：events 91/107、options 626、context.encryption_key 214、seal/unseal。 |
| 42 | `encrypt_bytes` | fn | src/addon/feishu/domain/crypto.rs:52-69 | **move-with-refactor** | 随机 IV 53-54、预留整块 57-58、PKCS#7 对齐时仍追加整块 60-63（飞书 Go standardizeDataEn）、IV 前置 65-68。错误类型 BaseError→crate Error；字节形态必须逐字节不变，迁移测试 409-471 钉住。 |
| 43 | `encrypt_json` | fn | src/addon/feishu/domain/crypto.rs:78-82 | **move-with-refactor** | 外部选项加密返回的唯一入口（approval_options.rs:635-639）；只序列化 data.result 内容。错误类型改为 crate Error。 |
| 44 | `decrypt_bytes` | fn | src/addon/feishu/domain/crypto.rs:88-109 | **move-with-refactor** | 事件解密唯一入口（event_challenge 92、receive 108）；长度校验 93-95、IV 切分 96-99、PKCS#7 解填充 105-107。错误类型改为 crate Error::Decrypt；宿主仍映射 ConfigError（5xx，触发飞书重推）。 |
| 45 | `seal` | fn | src/addon/feishu/domain/crypto.rs:118-123 | **stay-in-host** | 返回 (sha256 摘要, 可逆密文)，是宿主凭据存储语义（token_hash 列），不是飞书协议。改调 yang_feishu::crypto::encrypt_bytes。 |
| 46 | `generate_token` | fn | src/addon/feishu/domain/crypto.rs:130-138 | **stay-in-host** | 32 字节随机 hex；签发宿主凭据，不是飞书契约。 |
| 47 | `IssuedToken` | type | src/addon/feishu/domain/crypto.rs:143-150 | **stay-in-host** | 明文/摘要/密文三元组，宿主凭据签发结果。 |
| 48 | `issue_token` | fn | src/addon/feishu/domain/crypto.rs:156-164 | **stay-in-host** | 建数据源/建审批配置/轮换必须走同一条签发路径（注释 152-155）。 |
| 49 | `unseal` | fn | src/addon/feishu/domain/crypto.rs:174-178 | **stay-in-host** | 低层解封、不做完整性校验；宿主独有。 |
| 50 | `unseal_verified` | fn | src/addon/feishu/domain/crypto.rs:189-201 | **stay-in-host** | 摘要校验把 CBC 缺完整性保护变成确定性失败（180-188 的注释）；依赖宿主 token 摘要列。 |
| 51 | `decrypt_cbc` | fn | src/addon/feishu/domain/crypto.rs:208-234 | **move-as-is** | #[cfg(test)] 专用（207）：验证产出确实是 PKCS#7。AES 测试随之迁移，所以函数也进 crate 的 cfg(test)，不暴露生产 API。 |
| 52 | `hash_token` | fn | src/addon/feishu/domain/token.rs:18-28 | **stay-in-host** | L1 不抽：它服务宿主 token_hash 列与 derive.rs 的选项 id 哈希，属数据源凭据/业务派生，不是事件或外部选项协议。后续若抽 token.rs，可整体迁入 crate::token。 |
| 53 | `verify_token` | fn | src/addon/feishu/domain/token.rs:35-44 | **stay-in-host** | 比对库存摘要，属数据源/管理 Token 鉴权；宿主 approval_options.rs:364、middleware.rs:71、dispatch.rs:287 调用。 |
| 54 | `tokens_equal` | fn | src/addon/feishu/domain/token.rs:49-56 | **move-as-is** | 事件 verification_token 的常数时间比较（event_challenge 168、receive 158）。移入 crate::token，保持长度不同直接 false。 |
| 55 | `MAX_BATCH` | const | src/addon/feishu/domain/option_write.rs:57 | **stay-in-host** | option_write.rs 无协议段：整文件是 feishu_option 表的写机制（ODKU、跨源归属、补集停用），依赖 yang-db Transaction 与业务表，全部留宿主。 |
| 56 | `find_foreign_option_owner` | fn | src/addon/feishu/domain/option_write.rs:68-99 | **stay-in-host** | 跨源夺取预检，依赖 feishu_option 表与 utf8mb4_unicode_ci 排序规则，纯宿主业务。 |
| 57 | `apply_option_rows` | fn | src/addon/feishu/domain/option_write.rs:123-165 | **stay-in-host** | 多行 ODKU + 写后归属断言；属宿主持久化，不是飞书协议。 |
| 58 | `disable_option_rows` | fn | src/addon/feishu/domain/option_write.rs:174-196 | **stay-in-host** | 补集停用落点，依赖业务表与启用语义。 |
| 59 | `count_option_rows` | fn | src/addon/feishu/domain/option_write.rs:203-213 | **stay-in-host** | 空快照歧义守卫，依赖业务表。 |

### 需先重构

- **把 FeishuOptionsRequest 的 ParamInput 实现移出 crate，宿主新增透明 newtype ApprovalOptionsInput 并改 handler 输入类型** — ParamInput 与 FeishuOptionsRequest 分属 yang-base / yang-feishu 两个外部 crate，宿主无法为外部类型实现外部 trait（孤儿规则）；透明 newtype 同时保留 serde 与 schemars 形状（`src/addon/feishu/domain/protocol.rs:59-64 → src/addon/feishu/option/actions/approval_options.rs:411-416`）
- **crate 定义统一 Error，替换 protocol/pagination/crypto 里的 yang_base::BaseError** — 让 yang-feishu 不反向依赖 yang-base（除 ParamInput 外的另一处依赖）；宿主集中映射回 BaseError，保住 400/401/403/5xx 语义（`src/addon/feishu/domain/protocol.rs:186-189; src/addon/feishu/domain/pagination.rs:9,34-46; src/addon/feishu/domain/crypto.rs:19,52-109`）
- **crypto.rs 按职责切两半：AES 原语（14-109）进 crate，凭据封存/签发/解封（111-201）留宿主** — seal/unseal/issue_token 依赖宿主 token_hash 列与摘要校验，不是飞书协议；AES 原语同时服务事件解密与选项加密（`src/addon/feishu/domain/crypto.rs:14-109 vs 111-201`）
- **event_challenge::handle 拆成 crate 的 decode_challenge 与宿主 Action 适配** — handle 目前把『解密/来源校验/类型分派/challenge 回显』与 HTTP 注册、FeishuContext、encrypt_key 传递混在一个 100 行函数里；拆后 crate 可纯函数测试，宿主只保留响应构造与 sink 调用（`src/addon/feishu/event/actions/event_challenge.rs:75-176`）
- **receive_approval_event::handle 拆成 crate 的 decode_callback + 宿主 event/domain/sink.rs 的 ApprovalEventSink** — 来源校验与可信信封是通用飞书能力；审批字段白名单/instance_code/status/operate_time 与 feishu_approval_event 表、事务、1062 去重是宿主业务（`src/addon/feishu/event/actions/receive_approval_event.rs:79-292`）
- **为 X-Lark-Signature 在 yang-base 保留原始请求体字节** — 签名是 sha256(timestamp+nonce+encrypt_key+原始 body)，官方明确『不要在反序列化后再计算』；当前 transport 在 axum.rs 反序列化后丢弃字节，Request 只有 Value（`crates/yang-base/src/action/request.rs:62-77,114-122; crates/yang-base/src/transport/axum.rs:164-167,584-590,664-704`）
- **两个 event register 改收 Arc<FeishuContext> 并在注册期构造一次 Arc<dyn EventSink>** — 当前 actions/mod.rs:18-23 用 Arc::try_unwrap().unwrap_or_else(clone) 绕过按值参数；sink 每请求克隆上下文会放大开销，也妨碍单次构造（`src/addon/feishu/event/actions/mod.rs:18-23; event_challenge.rs:35-63; receive_approval_event.rs:37-65`）
- **宿主 domain 模块降级为转发门面，减少调用点改动** — protocol/i18n/pagination/crypto 被多处 use；保留同名模块 pub(crate) use crate 的符号，可把抽取 diff 收敛到 4 个文件，而不是改动所有消费者（`src/addon/feishu/domain/protocol.rs:1-190; i18n.rs:1-125; pagination.rs:1-46; crypto.rs:14-109`）
- **crate 内所有抽取符号的可见性 pub(crate)→pub，并补 crate 级文档与 #[non_exhaustive] Error** — 跨 crate 后 pub(crate) 不可见；pub(crate) 字段（如 FeishuOptionsRequest 的 7 个字段、OptionRow 的 5 个字段）必须公开（`src/addon/feishu/domain/protocol.rs:24-57; i18n.rs:47-59; pagination.rs:22,34; crypto.rs:37-109`）
- **protocol 测试 helper decode_request 改用 serde_json::from_value** — ParamInput impl 不再随 DTO 进 crate；测试只关心 serde 形状，不应依赖框架 decode（`src/addon/feishu/domain/protocol.rs:215-218; 同形 helper 在 approval_options.rs:970-982,1253-1264`）
- **新 crate 接入 lib_yang CI 门禁与 workspace 依赖** — verify_ci_contract.py 的 REQUIRED_FRAGMENTS 是 ci.yml 的字面片段，改一处必须同步改另一处；run_ci.py 的 AUXILIARY_PACKAGE_FLAGS 未列新 crate 时 --self-test 不覆盖它（`lib_yang/scripts/run_ci.py:25-30; lib_yang/.github/workflows/ci.yml:57,59,65; lib_yang/scripts/verify_ci_contract.py:23-27; lib_yang/Cargo.toml:16-33`）

### 目标公开 API

| 名称 | 签名 | 备注 |
|---|---|---|
| `yang_feishu::Error` | `#[derive(Debug, thiserror::Error)] #[non_exhaustive] pub enum Error { Config(String), Decrypt(String), InvalidBody { field: &'static str, message: String }, Unauthorized(String), Forbidden(String), InvalidCursor, InvalidSignature, Sink(#[source] Box<dyn std::error::Error + Send + Sync>) }` | 宿主映射：Config/Decrypt→BaseError::ConfigError(5xx)；InvalidBody→ValidationFailed(400)；Unauthorized→Unauthorized(401)；Forbidden→PermissionDenied(403)；InvalidCursor→宿主的信封码 40002（不冒泡 BaseError）；Sink 内 Box 用 downcast::<BaseError>() 取回原始错误，失败则退化为 5xx。 |
| `yang_feishu::event::EventConfig` | `pub struct EventConfig { pub verification_token: String, pub encrypt_key: Option<String>, pub require_signature: bool }` | 宿主由 FeishuSettings.event_verification_token / event_encrypt_key 构造（config/mod.rs:691,698）。verification_token 为 None 时宿主不注册端点，crate 不处理『未配置』。require_signature 先默认 false，生产观察 X-Lark-Signature 稳定出现后再翻 true。 |
| `yang_feishu::event::CallbackEnvelope` | `#[derive(Debug, Clone, PartialEq)] pub struct CallbackEnvelope { pub event_id: String, pub event_type: String, pub event: serde_json::Value, pub payload: serde_json::Value }` | event_id 取顶层 uuid（不是 event.uuid，二者同名不同义）；event_type 取 event.type；event 是 event 嵌套层原始 JSON；payload 是剥掉 token 后的完整体，供宿主落 raw_payload。crate 只校验顶层 type==event_callback 与 event_id 非空且≤128，不校验审批专属字段。 |
| `yang_feishu::event::Inbound` | `pub enum Inbound { Challenge(String), Callback(CallbackEnvelope) }` | Challenge 内是回显文案。 |
| `yang_feishu::event::decode_challenge` | `pub fn decode_challenge(body: &serde_json::Value, config: &EventConfig) -> Result<Inbound, Error>` | 复刻 event_challenge 的精确判定顺序：①有 encrypt 就解密（缺 key→Config、解密/JSON 失败→Decrypt），无 encrypt 且配了 key→Forbidden；②type=event_callback→校验 token(401)、构信封；③challenge 空→InvalidBody；④type!=url_verification→InvalidBody；⑤token 缺失或不匹配→Forbidden（403）；⑥返回 Challenge。共用订阅 URL 的加密事件… |
| `yang_feishu::event::decode_callback` | `pub fn decode_callback(body: &serde_json::Value, config: &EventConfig) -> Result<CallbackEnvelope, Error>` | 专用事件端点入口，判定顺序与现 receive_approval_event 一致：先解密/明文策略，再 token fail-closed(401)，再要求顶层 type==event_callback 与合法 event_id(400)。不处理 challenge。审批白名单/instance_code/status/operate_time 由宿主 sink 做。 |
| `yang_feishu::event::SignatureHeaders` | `pub struct SignatureHeaders<'a> { pub timestamp: Option<&'a str>, pub nonce: Option<&'a str>, pub signature: Option<&'a str> }` | 对应 X-Lark-Request-Timestamp / X-Lark-Request-Nonce / X-Lark-Signature；宿主经 ctx.request.get_header(...) 取值（header 名已被 yang-base 归一为小写）。 |
| `yang_feishu::event::verify_signature` | `pub fn verify_signature(headers: SignatureHeaders<'_>, raw_body: &[u8], encrypt_key: &str, now_unix: i64, max_skew_secs: Option<u64>) -> Result<(), Error>` | 按官方算法 sha256(timestamp+nonce+encrypt_key+原始 body) 取小写 hex，用 subtle 常数时间比较。encrypt_key 用配置原文，不是 derive_key 的结果。timestamp 缺失或超窗→InvalidSignature。请求体必须是原始字节，重序列化不算。 |
| `yang_feishu::event::EventSink` | `#[async_trait::async_trait] pub trait EventSink: Send + Sync { async fn deliver(&self, event: CallbackEnvelope) -> Result<Ack, Error>; }` | 宿主注册点：register() 时构造 Arc<dyn EventSink> 注入两个 handler。事件类型白名单与审批字段校验放宿主实现里（accepts 语义不需要单列方法），crate 保持平台通用；将来第二类事件源由宿主组合一个 router sink，crate 不建注册表（YAGNI）。 |
| `yang_feishu::event::Ack` | `#[derive(Debug, Clone, Copy, Default)] pub struct Ack { pub duplicate: bool }` | 宿主 sink 把唯一索引冲突（Duplicate/1062）折成 duplicate=true；Action 据此返回 {"success":true,"duplicate":true}。 |
| `yang_feishu::options::protocol::FeishuOptionsRequest` | `pub struct FeishuOptionsRequest { pub user_id: Option<String>, pub employee_id: Option<String>, pub token: String, pub linkage_params: Option<BTreeMap<String, String>>, pub page_token: Option<String>, pub query: Option<String>, pub locale: Option<String> }` | derive(Debug, Clone, Deserialize, JsonSchema)；刻意不 deny_unknown_fields。宿主侧新增 #[serde(transparent)] newtype ApprovalOptionsInput 实现 ParamInput。 |
| `yang_feishu::options::protocol::FeishuEnvelope` | `pub struct FeishuEnvelope { pub code: i32, pub msg: String, pub data: Option<FeishuData> } impl FeishuEnvelope { pub fn ok(body: FeishuResultBody) -> Self; pub fn encrypted(cipher_base64: String) -> Self; pub fn fail(code: i32, msg: impl Into<String>) -> Self; pub fn to_json(&self) -> Result<String, Error>; }` | 顶层键必须是 msg（框架 ApiResponse 是 message，改不了）；to_json 供 ResponseBody::raw 使用。子类型 FeishuResultBody/FeishuOption/FeishuI18nResource/FeishuResult/FeishuData 一并 pub 导出，键名与 camelCase 保持不变。 |
| `yang_feishu::options::i18n` | `pub const DEFAULT_LOCALE: &str = "zh_cn"; pub const I18N_PREFIX: &str = "@i18n@"; pub fn i18n_key(option_id: &str) -> String; pub fn normalize_linkage_value(raw: &str) -> &str; pub struct OptionRow { pub option_id: String, pub label: String, pub i18n: BTreeMap<String, String>, pub sort_order: i64, pub is_default: bool } pub fn build_result_body(rows: &[OptionRow], default_locale: &str, has_more: b…` | build_result_body 保证 i18nResources 非空、所有语言同键集、缺翻译回退默认文案、语言顺序=默认语言+字典序，输出字节稳定。宿主仍负责把数据库行读成 OptionRow（approval_options.rs:765-779）。 |
| `yang_feishu::options::pagination` | `pub fn encode_cursor(sort_order: i64, option_id: &str) -> String; pub fn decode_cursor(raw: &str) -> Result<Option<(i64, String)>, Error>;` | base64(URL_SAFE_NO_PAD)(sort_order ‖ U+001F ‖ option_id)。空/空白→Ok(None)；其余非法一律 Error::InvalidCursor。宿主 approval_options.rs:516-522 把 Err 转成 code=40002 的信封，不冒泡。 |
| `yang_feishu::crypto` | `pub fn derive_key(raw: &str) -> [u8; 32]; pub fn encrypt_bytes(plaintext: &[u8], key: &[u8; 32]) -> Result<String, Error>; pub fn encrypt_json(value: &serde_json::Value, key: &[u8; 32]) -> Result<String, Error>; pub fn decrypt_bytes(encoded: &str, key: &[u8; 32]) -> Result<Vec<u8>, Error>;` | AES-256-CBC + sha256(key) + 随机 IV 前置 + PKCS#7（对齐时追加整块）+ base64(IV‖密文)。只搬原语；seal/unseal/issue_token/generate_token 与 token_hash 留宿主。crate 的 Cargo.toml 需要 aes=0.8、cbc=0.1(alloc,block-padding)、rand_core=0.6(getrandom)、subtle=2.6、base64、sha2（其中 aes/cbc/rand_core/s… |
| `yang_feishu::token::tokens_equal` | `pub fn tokens_equal(left: &str, right: &str) -> bool` | 常数时间比较明文 Token；长度不同直接 false（长度本身不是秘密）。hash_token/verify_token 不随 L1 迁移。 |

### 测试处置（17）

| 文件 | 区间 | 内容 | 处置 |
|---|---|---|---|
| src/addon/feishu/domain/protocol.rs | 192-372 | DTO 形状：msg 而非 message、data.result 嵌套、加密 result 是字符串、isDefault camelCase、nextPageToken 省略、7 字段解码、只带 token、未知字段容忍、缺 token 报错 | **move** |
| src/addon/feishu/domain/i18n.rs | 127-322 | @i18n@ 占位符、默认语言恒存在、额外语言同键集、缺翻译回退、isDefault 传播、字节稳定、normalize_linkage_value 四类边界 | **move** |
| src/addon/feishu/domain/pagination.rs | 48-128 | 游标往返、负数/i64::MAX、含分隔符字符的 option_id、空游标、非法游标 fail-closed、不泄露明文 id | **move** |
| src/addon/feishu/domain/crypto.rs | 236-376 | 凭据封存/签发/轮换：seal/unseal、摘要稳定、错钥匙/篡改/跨行密文确定性失败、issue_token 自洽 | **stay** |
| src/addon/feishu/domain/crypto.rs | 378-508 | AES 原语：sha256 派生已知值、对齐时多整块、部分填充、IV 前置与块对齐、随机 IV、PKCS#7 往返、encrypt_json 形状、真实飞书加密 challenge 解密 | **move** |
| src/addon/feishu/domain/token.rs | 58-116 | hash_token/verify_token：摘要形态、大小写与空白容忍、非法库存值 fail-closed | **stay** |
| src/addon/feishu/domain/token.rs | 117-121 | tokens_equal：等值、差一字符、长度不同 | **move** |
| src/addon/feishu/event/actions/event_challenge.rs | 185-253 | malformed_callbacks_never_return_success_ack：无效 encrypt/解密后非 JSON/缺 challenge/未知 type 必须区分服务端故障与客户端格式错误，且不得返回成功 ACK | **rewrite** |
| src/addon/feishu/event/actions/event_challenge.rs | 255-349 | event_diagnostics_never_log_verification_tokens_or_decrypted_payloads：诊断日志不得泄露 token/明文 | **rewrite** |
| src/addon/feishu/event/actions/receive_approval_event.rs | 301-353 | missing_event_identity_is_rejected_before_database_access：缺 uuid/类型/instance_code 在进数据库前拒绝 | **stay** |
| src/addon/feishu/option/actions/approval_options.rs | 781-968 | verify_source 的 token 优先级/错误码、read_row 的 i18n 解析、next_page_token 的 hasMore 判据 | **stay** |
| src/addon/feishu/option/actions/approval_options.rs | 970-1250 | 级联参数归一化与父键解析的全部决策测试；helper request_with 需从 ParamInput::decode 改成 serde_json::from_value | **rewrite** |
| src/addon/feishu/option/actions/approval_options.rs | 1253-1379 | wants_every_option、同名折叠与规范 id 内存 keyset 分页；helper request_with_paging 需改成 serde_json::from_value | **rewrite** |
| tests/feishu_approval_options_integration.rs | 1-end | 端点级：裸 {code,msg,data}、i18nResources 非空、@i18n@ 占位符、非法游标 fail-closed、加密返回、keyset 翻页不漏不重 | **stay** |
| tests/feishu_approval_dispatch_integration.rs | 1490-1560 | 共用 challenge URL 的加密 event_callback 必须解密、委托 sink 并落库；重复投递按 uuid 去重；raw_payload 不含 token | **stay** |
| crates/yang-feishu/src/options/protocol.rs (新增) | new | 承接 protocol.rs 的 DTO 形状测试；decode helper 改用 serde_json::from_value | **move** |
| crates/yang-feishu/src/event/mod.rs (新增) | new | 重写为纯函数：decode_challenge 的顺序与错误分类、decode_callback 的信封校验、签名算法与时间窗、AES 解密失败分类 | **rewrite** |

### 宿主替换点（18）

- `src/addon/feishu/domain/protocol.rs:1-190` → 改为门面：pub(crate) use yang_feishu::options::protocol::*; 保留同名 re-export 以减少消费者改动；删除 ParamInput impl 与 BaseError 依赖。
- `src/addon/feishu/option/actions/approval_options.rs:411-416` → 新增 #[derive(Debug, serde::Deserialize, schemars::JsonSchema)] #[serde(transparent)] pub(crate) struct ApprovalOptionsInput(pub(crate) FeishuOptionsRequest); impl ParamInput for ApprovalOptionsInput { fn params() -> Params { Params::new() } }；handle 改收 ApprovalOptionsInput，resolve 传 &input.0。
- `src/addon/feishu/domain/i18n.rs:1-125` → 改为门面：pub(crate) use yang_feishu::options::i18n::{build_result_body, i18n_key, normalize_linkage_value, OptionRow, DEFAULT_LOCALE, I18N_PREFIX};
- `src/addon/feishu/domain/pagination.rs:1-46` → 改为门面：pub(crate) use yang_feishu::options::pagination::{decode_cursor, encode_cursor}; 消费者 approval_options.rs:29 的 use 不变。
- `src/addon/feishu/domain/crypto.rs:14-109` → 改为门面：pub(crate) use yang_feishu::crypto::{decrypt_bytes, derive_key, encrypt_bytes, encrypt_json}; seal/unseal/issue_token/generate_token/IssuedToken 保留在本文件，内部改调 crate 原语。
- `src/addon/feishu/domain/token.rs:46-56` → 改为 pub(crate) use yang_feishu::token::tokens_equal; hash_token/verify_token 原样保留。
- `src/addon/feishu/event/actions/event_challenge.rs:75-176` → handle 改调 yang_feishu::event::decode_challenge(&input.0, &config)；Inbound::Challenge(c) → raw_json(json!({"challenge": c}))；Inbound::Callback(ev) → sink.deliver(ev).await 后 raw_json({"success":true[,"duplicate":true]})。
- `src/addon/feishu/event/actions/receive_approval_event.rs:79-292` → handle 改调 yang_feishu::event::decode_callback(&input.0, &config) 得 CallbackEnvelope，再 sink.deliver(event).await；第 175-291 的审批字段提取与落库整体迁到新增 src/addon/feishu/event/domain/sink.rs 的 ApprovalEventSink（持 Arc<FeishuContext>）。
- `src/addon/feishu/event/domain/sink.rs (新增)` → pub(crate) struct ApprovalEventSink { context: Arc<FeishuContext> }；impl yang_feishu::event::EventSink：先做 event_type∈{approval_instance,approval_task}、instance_code 非空≤128、status/operate_time 提取，再复用当前 receive 224-291 的 Record/事务/1062 去重逻辑；Error::Sink(Box::new(base_error)) 保留原始错误供宿主 downcast。
- `src/addon/feishu/event/actions/mod.rs:18-23` → 删除 Arc::try_unwrap().unwrap_or_else(clone)：event_challenge::register 与 receive_approval_event::register 均接收 Arc<FeishuContext>，各自在注册期构造一次 Arc<dyn EventSink>。
- `src/addon/feishu/option/actions/approval_options.rs:26-31,516-522,621-639` → 26-31 的 use 改指向 yang_feishu::options::{i18n, pagination, protocol}；516-522 的 decode Err 分支继续返回 FeishuEnvelope::fail(40002)；621-639 的 build_result_body/encrypt_json 改调 crate，错误经宿主映射函数转 BaseError。
- `src/addon/feishu/option/actions/approval_options.rs:970-982,1253-1264 (tests)` → request_with / request_with_paging 里的 <FeishuOptionsRequest as ParamInput>::decode(&mut request) 改成 serde_json::from_value(body).unwrap_or_else(...)，不再依赖框架 decode。
- `crates/yang-base/src/action/request.rs:62-77,114-122` → Request 增加 raw_body: Option<Arc<[u8]>> 字段与 raw_body() getter；Request::new 置 None；Clone 保留（Arc 克隆）；新增 builder raw_body(bytes)。
- `crates/yang-base/src/transport/axum.rs:164-167,584-590,664-704` → DecodedBody 增加 raw: Option<Vec<u8>>；JSON 分支在 serde_json::from_slice 前把 body 字节存入；构造 Request 时 .raw_body(bytes) 挂上；multipart 分支置 None。
- `crates/yang-feishu/Cargo.toml (新增)` → [package] name="yang-feishu", edition/rust-version/license 继承 workspace, publish=false；[dependencies] serde/serde_json/base64/sha2/thiserror/async-trait/schemars 用 workspace，aes/cbc/rand_core/subtle 待加进根 [workspace.dependencies] 后引用；[lints] workspace=true。
- `lib_yang/Cargo.toml:16-33` → 在 [workspace.dependencies] 增加 aes = "0.8"、cbc = { version = "0.1", features = ["alloc","block-padding"] }、rand_core = { version = "0.6", features = ["getrandom"] }、subtle = "2.6"（sha2/base64/schemars/serde/serde_json/thiserror/async-trait 已有）。
- `lib_yang/scripts/run_ci.py:25-30` → AUXILIARY_PACKAGE_FLAGS 增加 "-p", "yang-feishu"，否则新 crate 不进 all-targets 测试与 clippy。
- `lib_yang/.github/workflows/ci.yml:57,59,65 + lib_yang/scripts/verify_ci_contract.py:23-27` → 三处 -p 列表同步加 yang-feishu，并同步更新 REQUIRED_FRAGMENTS 中对应的字面片段（两文件必须逐字一致）；doc 测试行按需增加。

### 风险

- 孤儿规则：宿主不能为 yang_feishu::options::FeishuOptionsRequest 实现 yang_base::definition::ParamInput。必须用透明 newtype；若 schemars 0.8 对 tuple newtype 生成包装 schema 而非内联 schema，需加 #[schemars(transparent)] 并用 schema_for! 单测钉住 OpenAPI 形状。
- X-Lark-Signature 必须对原始请求体计算（官方明确『不要在反序列化后再计算』）。当前 yang-base 在 transport/axum.rs:664-704 反序列化后丢弃字节，必须先在 yang-base 加 raw_body 字段；这是跨仓前置改动，未随 yang-feishu 发布顺序处理会让签名功能无法落地。
- 签名强制策略不能一步到位：先 verify-if-present + WARN，确认生产所有加密推送都带三个头后再把 require_signature 置 true。过早 fail-closed 会让事件 401，飞书按 15s/5min/1h/6h 重推后永久丢弃。
- 事件错误映射必须保住 HTTP 语义：challenge token 不匹配现在是 403 PermissionDenied，事件 token 不匹配是 401 Unauthorized，解密/配置故障是 5xx（触发飞书重推）。映射错一个分支会改变飞书的去留行为，且日志上不易发现。
- 共用订阅 URL 的 event_callback 委托必须保留：challenge 端点解密后把 encrypt_key=None 继续下传（event_challenge.rs:141-144），否则『共用 URL + Encrypt Key』会让所有事件被误判为明文注入并 401。decode_challenge 的 Callback 分支要复刻这一点。
- 落库语义不能随抽取漂移：必须显式 commit（receive_approval_event.rs:241-254），只 insert 不 commit 会重演 2026-10-14 的『已落地日志与 raw_row_count:0 并存』事故；唯一索引 Duplicate/1062 必须仍返回 200。
- operate_time 单位：现在字符串/数字两种形态都收，插入时 /1000 毫秒→秒（199-208、231-235）。crate 只暴露原始 Value 或毫秒 i64，换算留在宿主；若 crate 擅自换成秒，列值会整体差 1000 倍。
- 认证材料零外泄：verification_token 必须在校验后从 body 移除再交宿主落 raw_payload（165-167），日志/DB/备份都不得含 token。迁移 event_challenge.rs:255-349 的日志测试，防止只剩『代码看起来删了』。
- EventSink 的 Error::Sink 采用 Box<dyn Error> 让宿主塞回 BaseError；若宿主在中间包装一层字符串，downcast 失败会退化为 500 —— 状态仍可重试，但错误归因链丢失。sink 实现必须直接 Box::new(BaseError)。
- 事件类型白名单留宿主后，未识别类型必须仍返回 400（当前 184-195）。若 sink 选择静默 200，飞书会停止重推，未来新增事件类型会被无声吞掉；白名单判定应放在 sink.deliver 的最前面并返回 Error::InvalidBody。
- crate 的 AES 必须逐字节兼容飞书 Go 实现：IV 前置、PKCS#7 在明文已对齐时仍追加整块。迁移测试 crypto.rs:409-471 与真实 challenge 解密用例 485-508 是唯一机器守卫，不能删或只保留一个。
- decode_cursor 的错误类型改变后，宿主必须继续把它折叠成信封 code=40002，而不是 BaseError；否则非法游标会变成 HTTP 非 200/内部错误，飞书按接口报错处理。
- option_write.rs 没有任何协议段（整文件是 feishu_option 的 ODKU/归属/停用机制），不要因为任务描述含该文件而把它拆进 crate；它依赖 yang-db::Transaction、Repository 与 utf8mb4_unicode_ci 归属比较。
- 新 crate 会引入 aes/cbc/rand_core/subtle：lib_yang 的 cargo-deny 许可证白名单（deny.toml）与 feature-isolation 自检需跑通；aes/cbc 的 MSRV 需确认 ≤1.80（aes 0.8/cbc 0.1 满足，rand_core 0.6 满足）。
- CI 契约是字面片段校验：ci.yml 与 verify_ci_contract.py 的 REQUIRED_FRAGMENTS 是两份重复的字符串，只改一处会导致 --self-test / CI contract 门禁失败；run_ci.py --self-test 也要覆盖新包。
- 新增宿主文件 event/domain/sink.rs 必须遵守 MODULE_CONTAINER_ENTRIES 门禁（module 目录只允许 mod.rs/table.rs/actions/domain）；直接放 event/sink.rs 会被 check_architecture.py 判违规。
- 测试迁移后宿主 approval_options.rs 的 request_with/request_with_paging 不能再调 ParamInput::decode；若漏改，编译期就会暴露（好），但若改成重复的 serde 结构体会让 DTO 契约出现第二份真相（坏）——必须直接 from_value 到 crate 类型。

## A.8 宿主改造与 workspace/CI 接线

**目标**：把 yang-system 的飞书平台能力抽到 crates/yang-feishu：给出宿主所有替换点、outbound_setup 的构造器化改造、4 处 token 刷新循环收敛方案、新 crate 的 manifest/feature 设计，以及 run_ci.py / ci.yml / verify_* 的具体接线。结论：纯契约模块（outbound、tenant_token、bitable、approval 客户端、convert/match/attachment/uuid/rate_limit/widget_rules、crypto/token、option 协议族）整体迁出；产品表/配置/Action/审计耦合（context、repository、pull、option_write、dispatch/execution、feedback/notification/row_edit、alert、user_token 存储半、provision 落库半）留在宿主。迁移用 domain/mod.rs 的 pub(crate) use 桥接，可把宿主动刀面压到 1 个聚合文件 + 4 个拆分文件 + outbound_setup。

### 源文件

| 文件 | 生产行区间 | 生产行数 | 测试行区间 | 测试行数 |
|---|---|---:|---|---:|
| src/addon/feishu/domain/outbound.rs | 1-782 | 782 | 783-1664 | 882 |
| src/addon/feishu/domain/tenant_token.rs | 1-430 | 430 | 431-931 | 501 |
| src/addon/feishu/domain/bitable.rs | 1-1036 | 1036 | 1037-2863 | 1827 |
| src/addon/feishu/domain/approval.rs | 1-844 | 844 | 845-1550 | 706 |
| src/addon/feishu/domain/approval_convert.rs | 1-1289 | 1289 | 1290-2769 | 1480 |
| src/addon/feishu/domain/approval_match.rs | 1-1030 | 1030 | 1031-2477 | 1447 |
| src/addon/feishu/domain/approval_attachment.rs | 1-261 | 261 | 262-447 | 186 |
| src/addon/feishu/domain/approval_uuid.rs | 1-73 | 73 | 74-149 | 76 |
| src/addon/feishu/domain/approval_rate_limit.rs | 1-67 | 67 | 68-205 | 138 |
| src/addon/feishu/domain/widget_rules.rs | 1-166 | 166 | 167-245 | 79 |
| src/addon/feishu/domain/crypto.rs | 1-206 | 206 | 207-509 | 303 |
| src/addon/feishu/domain/token.rs | 1-57 | 57 | 58-122 | 65 |
| src/addon/feishu/domain/pagination.rs | 1-47 | 47 | 48-128 | 81 |
| src/addon/feishu/domain/i18n.rs | 1-126 | 126 | 127-322 | 196 |
| src/addon/feishu/domain/derive.rs | 1-295 | 295 | 296-755 | 460 |
| src/addon/feishu/domain/linkage.rs | 1-63 | 63 |  | 0 |
| src/addon/feishu/domain/source_key.rs | 1-23 | 23 | 24-57 | 34 |
| src/addon/feishu/domain/protocol.rs | 1-86 | 86 | 87-372 | 286 |
| src/addon/feishu/domain/user_token.rs | 1-762 | 762 | 763-1300 | 538 |
| src/addon/feishu/domain/approval_provision.rs | 1-1501 | 1501 | 1502-2731 | 1230 |
| src/addon/feishu/domain/approval_notification.rs | 1-448 | 448 | 449-1388 | 940 |
| src/addon/feishu/domain/approval_feedback.rs | 1-256 | 256 | 257-535 | 279 |
| src/addon/feishu/domain/approval_row_edit.rs | 1-179 | 179 | 180-504 | 325 |
| src/addon/feishu/domain/context.rs | 1-240 | 240 | 241-303 | 63 |
| src/addon/feishu/domain/repository.rs | 1-94 | 94 |  | 0 |
| src/addon/feishu/domain/outbound_setup.rs | 1-115 | 115 | 116-139 | 24 |
| src/addon/feishu/domain/pull.rs | 1-1363 | 1363 | 1364-2059 | 696 |
| src/addon/feishu/domain/approval_dispatch.rs | 1-1997 | 1997 | 1998-5088 | 3091 |
| src/addon/feishu/domain/mod.rs | 1-45 | 45 |  | 0 |
| src/addon/feishu/mod.rs | 1-155 | 155 |  | 0 |
| src/infrastructure/feishu_approval_worker.rs | 1-4905 | 4905 | 4906-9081 | 4176 |
| src/infrastructure/feishu_event_worker.rs | 1-1329 | 1329 | 1330-1975 | 646 |
| src/infrastructure/feishu_pull.rs | 1-491 | 491 | 492-528 | 37 |
| src/infrastructure/feishu_oauth_reminder.rs | 1-300 | 300 | 301-309 | 9 |
| Cargo.toml | 1-56 | 56 |  | 0 |
| lib_yang/Cargo.toml | 1-151 | 151 |  | 0 |
| lib_yang/scripts/run_ci.py | 1-455 | 455 |  | 0 |
| lib_yang/.github/workflows/ci.yml | 1-252 | 252 |  | 0 |
| lib_yang/scripts/verify_feature_isolation.py | 1-115 | 115 |  | 0 |
| lib_yang/scripts/verify_ci_contract.py | 1-105 | 105 |  | 0 |

### 符号（57）

| # | 符号 | 类型 | 位置 | 处置 | 备注 |
|---:|---|---|---|---|---|
| 1 | `outbound` | module | src/addon/feishu/domain/outbound.rs:1 | **move-as-is** | 纯飞书出站契约：错误码常量、分类、重试、传输抽象。无宿主耦合。全文件 36 个单测随迁。 |
| 2 | `OutboundTransport` | trait | src/addon/feishu/domain/outbound.rs:171 | **move-as-is** | trait 只依赖原语类型与 BaseError；宿主所有测试替身（ScriptedTransport）在改路径后可继续实现它。 |
| 3 | `OutboundRequest / OutboundResponse` | type | src/addon/feishu/domain/outbound.rs:138 | **move-as-is** | OutboundRequest:138-157；OutboundResponse:158-166。字段需 pub 以支撑宿主所有调用点与测试替身。 |
| 4 | `HttpClientTransport` | type | src/addon/feishu/domain/outbound.rs:176 | **move-with-refactor** | approval_create_limit 字段与 with_approval_create_limit(:189) 必须随 redis feature 一起 cfg 化；否则 crate 无法在不依赖 Redis 时编译。构造器 new(:183) 保持。 |
| 5 | `send_with_retry` | fn | src/addon/feishu/domain/outbound.rs:670 | **move-as-is** | 唯一重试入口；内部按业务码 classify、按 idempotent 决定 disposition，迁出后调用点签名不变，宿主测试可直接复用。 |
| 6 | `FailureKind / OutboundFailure / classify / disposition` | type | src/addon/feishu/domain/outbound.rs:318 | **move-as-is** | FailureKind:318；classify:345；OutboundFailure:516；disposition:630。宿主 dispatch/worker 大量引用，迁出后仍经 domain 再导出保持类型同一。 |
| 7 | `RetryPolicy / TOKEN_RETRY / PULL_RETRY` | const | src/addon/feishu/domain/outbound.rs:543 | **move-as-is** | RetryPolicy:543-550；TOKEN_RETRY:558、PULL_RETRY:569、DEPARTMENT_RETRY:585。注释里引用的 LOCK_TTL 不变式需在 crate 内重写文档链接。 |
| 8 | `send_with_tenant_token（新增）` | fn | src/addon/feishu/domain/outbound.rs（新增，建议紧邻 send_with_retry） | **move-with-refactor** | 4 处刷新循环的唯一收敛点：取 token→填 bearer→send_with_retry→TokenExpired 且未刷新过则 invalidate_and_refresh 后重试一次；刷新失败直接冒泡 Fatal。 |
| 9 | `tenant_token` | module | src/addon/feishu/domain/tenant_token.rs:1 | **move-with-refactor** | 唯一宿主耦合是 :276 crate::authorization::validate_deployment_name。改为 crate 内 validate_deployment（规则一致：1..=64、小写字母/数字/连字符）或删除该校验。 |
| 10 | `FeishuCredentials` | type | src/addon/feishu/domain/tenant_token.rs:68 | **move-as-is** | 手写 Debug 隐藏 secret；迁出后成为宿主 OAuth 与客户端构造的公共入参。 |
| 11 | `TenantTokenCache / RedisTenantTokenCache` | trait | src/addon/feishu/domain/tenant_token.rs:174 | **move-with-refactor** | trait:174 无条件保留；RedisTenantTokenCache:184 必须 cfg(feature="redis")，否则 none 组合无法编译。 |
| 12 | `TenantTokenProvider` | type | src/addon/feishu/domain/tenant_token.rs:258 | **move-with-refactor** | tenant_access_token:290、invalidate_and_refresh:351。这两方法是 4 处循环收敛的基石。 |
| 13 | `bitable` | module | src/addon/feishu/domain/bitable.rs:1 | **move-as-is** | 2863 行全部是飞书 OpenAPI 契约，无 crate:: 引用。64 个单测随迁。 |
| 14 | `list_all_records / list_all_fields / list_all_tables / list_all_views` | fn | src/addon/feishu/domain/bitable.rs:1106 | **move-as-is** | 1106/1325/1279/1230；均以 (transport, sleeper, tokens, ...) 为参数，天然与宿主解耦。 |
| 15 | `batch_update_records / get_records_by_ids / search_records` | fn | src/addon/feishu/domain/bitable.rs:1421 | **move-as-is** | 1421/1626/1482；BackfillRow:1413 类型别名随迁。 |
| 16 | `parse_bitable_link / resolve_bitable_link / BitableCoordinates` | fn | src/addon/feishu/domain/bitable.rs:945 | **move-as-is** | 945/1013/929；host create_config/update_config/pull 使用。 |
| 17 | `approval（客户端）` | module | src/addon/feishu/domain/approval.rs:1 | **move-as-is** | create_instance:121、get_instance:311、get_approval_definition:518、subscribe_approval:564、resolve_department_name:437、resolve_applicant:749。纯 API 客户端，无宿主表。 |
| 18 | `CreateOutcome / GetOutcome / ApplicantResolution / InstanceDetail` | type | src/addon/feishu/domain/approval.rs:110 | **move-as-is** | 110/297/728/206；宿主 dispatch/worker 直接匹配这些枚举，迁出后经 domain 再导出保持同一类型。 |
| 19 | `approval_convert` | module | src/addon/feishu/domain/approval_convert.rs:1 | **move-as-is** | build_form:328、build_group_form:349、build_signature:404、WidgetMap:36、FormInput:191、FixedOffset:211、Converter:97、ConvertError:124。纯表单组装。 |
| 20 | `approval_match` | module | src/addon/feishu/domain/approval_match.rs:1 | **move-as-is** | 建议 crate 内命名 approval::matching。FormWidget:89、Column:343、FieldMapping:567、match_by_name:560、match_fields:644、mapping_widgets:595、preview_widgets:619。 |
| 21 | `approval_attachment` | module | src/addon/feishu/domain/approval_attachment.rs:1 | **move-as-is** | transfer:182、preflight:71、attachments:24、source_identities:95。附件下载/上传是平台能力，随迁。 |
| 22 | `approval_uuid::derive_uuid` | fn | src/addon/feishu/domain/approval_uuid.rs:36 | **move-as-is** | 纯确定性派生；宿主 dispatch 与 approval_aggregation 使用。 |
| 23 | `approval_rate_limit` | module | src/addon/feishu/domain/approval_rate_limit.rs:1 | **move-with-refactor** | bucket_key:12、min_interval_ms:17、acquire_create_slot:21。整模块 cfg(feature="redis")；关闭 redis 时审批创建共享限流器不存在，宿主必须保持默认 feature。 |
| 24 | `widget_rules` | module | src/addon/feishu/domain/widget_rules.rs:1 | **move-as-is** | 纯规则表（UNSUPPORTED_TYPES:22 与 13 个判据函数）。只被 approval_convert/feedback 引用。 |
| 25 | `crypto` | module | src/addon/feishu/domain/crypto.rs:1 | **move-as-is** | derive_key:37、encrypt_bytes:52、encrypt_json:78、decrypt_bytes:88、seal:118、unseal:174、unseal_verified:189、issue_token:156、generate_token:130。外部选项 AES-CBC 契约。 |
| 26 | `token` | module | src/addon/feishu/domain/token.rs:1 | **move-as-is** | hash_token:18、verify_token:35、tokens_equal:49。宿主 event/portal/approval 的入站校验共用。 |
| 27 | `pagination` | module | src/addon/feishu/domain/pagination.rs:1 | **move-as-is** | encode_cursor:22、decode_cursor:34。外部选项 page_token 契约；建议归入 crate 的 option 子模块。 |
| 28 | `i18n` | module | src/addon/feishu/domain/i18n.rs:1 | **move-as-is** | normalize_linkage_value:39、build_result_body:67、DEFAULT_LOCALE、I18N_PREFIX。飞书回传值形态的唯一适配层。 |
| 29 | `derive` | module | src/addon/feishu/domain/derive.rs:1 | **move-as-is** | label_option_id:169、ancestor_key:189、derive_options:224、snapshot_digest:279、truncate_label:99。option_id 派生规则属于飞书契约。 |
| 30 | `linkage` | module | src/addon/feishu/domain/linkage.rs:1 | **move-as-is** | 63 行纯类型，无测试；宿主 import_xlsx 使用。 |
| 31 | `source_key` | module | src/addon/feishu/domain/source_key.rs:1 | **move-as-is** | valid_source_key:14。字段绑定/导入/重指向共用校验。 |
| 32 | `protocol` | module | src/addon/feishu/domain/protocol.rs:1 | **move-as-is** | FeishuOptionsRequest:24、FeishuResultBody:68、FeishuEnvelope:144。依赖 yang-base 的 definition 核心（无需 mysql feature），随迁成立。 |
| 33 | `user_token（纯 OAuth 半）` | module | src/addon/feishu/domain/user_token.rs:88 | **split** | 迁出 88-470：now_unix_secs:88、REAUTH_REMINDER_DAYS:100、reauth_reminder_due:103、OAuthConfig:108、oauth_config:117（需改签名）、build_authorize_url:164、mint_state:188、consume_state:203、TokenBundle:237、TokenEndpointError:247、exchange_code:377、refresh_tokens:402、fetch_user_i… |
| 34 | `user_token（存储半）` | module | src/addon/feishu/domain/user_token.rs:478 | **stay-in-host** | UserTokenRow:478、SINGLE_SLOT、load_single:498、store_authorization:541、UserTokenError:636、user_access_token:689。读写产品表 feishu_user_token 并做跨进程单行锁，必须留宿主。 |
| 35 | `approval_provision（纯校验半）` | module | src/addon/feishu/domain/approval_provision.rs:73 | **split** | 迁出 ProvisionError:73、parse_form:159、type_name、resolve_optional_id:505、check_backfill_targets:518、check_batch_scope_field:578、check_aggregate_batch:620。建议 crate 路径 approval::provision。 |
| 36 | `approval_provision（落库半）` | module | src/addon/feishu/domain/approval_provision.rs:212 | **stay-in-host** | build_plan:212、fill_table_names:309、insert_plan:689、insert_maps:778、list_option_sources:940、resolve_external_options:1061。直接读写 config/field_map/option 表。 |
| 37 | `approval_feedback` | module | src/addon/feishu/domain/approval_feedback.rs:1 | **stay-in-host** | FeedbackStatus:14 的中文 heading/suggestion 与 render:226 是产品文案；宿主 Action 响应与通知正文共用。留宿主，路径改指 crate 的 convert/match。 |
| 38 | `approval_notification` | module | src/addon/feishu/domain/approval_notification.rs:1 | **stay-in-host** | 生产代码 post_content 用 FeedbackStatus::heading 判定标题，与产品文案强耦合；send_feedback:175/edit_feedback:399 虽为平台能力，但拆开收益小于耦合成本。可复用 crate 的 send_with_tenant_token 收敛其刷新循环。 |
| 39 | `approval_row_edit` | module | src/addon/feishu/domain/approval_row_edit.rs:1 | **stay-in-host** | 依赖 feedback 的 FeedbackStatus 与通知正文；随 feedback 留宿主。 |
| 40 | `approval_dispatch` | module | src/addon/feishu/domain/approval_dispatch.rs:1 | **stay-in-host** | 1997 行产品编排：状态机、回填、组聚合、配置行读取。飞书契约部分（convert/match/uuid）迁出后这里只保留编排。 |
| 41 | `approval_execution` | module | src/addon/feishu/domain/approval_execution/mod.rs:1 | **stay-in-host** | 任务租约/认领/持久执行，全部依赖产品表与事务；不迁。 |
| 42 | `approval_aggregation` | module | src/addon/feishu/domain/approval_aggregation/mod.rs:1 | **stay-in-host** | signature_hash:55、group_key:63、group_uuid:77、plan_groups:91；虽纯但是产品批量派发语义，且 repository.rs:1 留在同目录。迁出需为 repository 造壳，收益低。 |
| 43 | `context (FeishuContext)` | module | src/addon/feishu/domain/context.rs:1 | **stay-in-host** | 13 张产品表的聚合器 + 事务收尾 + xlsx 信号量。crate 绝不能引用它。 |
| 44 | `repository (Repository / all_pages)` | module | src/addon/feishu/domain/repository.rs:1 | **stay-in-host** | SYSTEM_ROLE:17、Repository:21、PAGE_SIZE:54、all_pages:74。框架表查询绑定，留宿主。 |
| 45 | `pull` | module | src/addon/feishu/domain/pull.rs:1 | **stay-in-host** | 选表→快照→逐字段事务落库→补集停用；依赖产品表、审计与事务。迁出后改成调用 crate 的 bitable/derive。 |
| 46 | `option_write` | module | src/addon/feishu/domain/option_write.rs:1 | **stay-in-host** | feishu_option 的写机制；Repository 由调用方传入。 |
| 47 | `field_maintenance / field_binding` | module | src/addon/feishu/domain/field_maintenance.rs:1 | **stay-in-host** | field_maintenance.rs:1（加载/远端校验/门户同步）与 field_binding.rs:1（schemars 输入形状）都绑定产品表与 OpenAPI 产物，留宿主。 |
| 48 | `alert` | module | src/addon/feishu/domain/alert.rs:1 | **stay-in-host** | validate_recipients:37、FeishuAlertSender。依赖宿主 SMTP 与配置校验；配置加载期由 config/mod.rs:1095 调用。 |
| 49 | `request_log / middleware / list_input / rate_limit / uploaded_files` | module | src/addon/feishu/domain/request_log.rs:1 | **stay-in-host** | 宿主 HTTP 面/框架中间件：request_log.rs:1、middleware.rs:1、list_input.rs:1、rate_limit.rs:1、uploaded_files.rs:1。不是飞书平台契约。 |
| 50 | `import_progress / xlsx / xlsx_job` | module | src/addon/feishu/domain/xlsx.rs:1 | **stay-in-host** | xlsx 是通用文件解析而非飞书平台契约；且 import_progress/xlsx_job 是宿主任务状态。建议整体留宿主。若后续确要抽，作为独立 xlsx feature（calamine 0.30.1 已锁 MSRV）。 |
| 51 | `schema_anchor / projection_contract` | module | src/addon/feishu/domain/schema_anchor.rs:1 | **stay-in-host** | 两者 #![cfg(test)] 扫宿主源码与产品表声明。迁出文件会改变弱档清单，需在 :946-1012 的 expected 重基。 |
| 52 | `outbound_setup::build` | fn | src/addon/feishu/domain/outbound_setup.rs:47 | **stay-in-host** | 改写成 crate 构造器 + 宿主注入；Outbound 结构体改为 pub(crate) type Outbound = yang_feishu::FeishuClient，保留 tokens 公共字段以零改 25 处 outbound.tokens 调用点。 |
| 53 | `require_settings / outbound_error` | fn | src/addon/feishu/domain/outbound_setup.rs:93 | **stay-in-host** | require_settings:93 返回 (码, 文案)，outbound_error:112 把 OutboundFailure 映射成 BaseError::ParamInvalid；都是宿主错误语义。 |
| 54 | `build_addon / build_context` | fn | src/addon/feishu/mod.rs:31 | **stay-in-host** | build_addon:31、build_context:93。组合根与产品表装配，不迁；bootstrap.rs:360/384/406/422 四处调用不变。 |
| 55 | `FeishuSettings` | type | src/config/mod.rs:583 | **stay-in-host** | is_usable:764、can_pull:777。宿主配置形状（enabled/pull interval/alert 等）；crate 只接收 app_id/app_secret/deployment/rate 四个字段。 |
| 56 | `FeishuClient` | type | lib_yang/crates/yang-feishu/src/client.rs（新增） | **move-with-refactor** | 新增构造器，取代 outbound_setup::build 与 3 个 worker 的内联组装；方法 transport(): &dyn OutboundTransport、sleeper(): &dyn Sleeper。 |
| 57 | `FeishuClientConfig` | type | lib_yang/crates/yang-feishu/src/client.rs（新增） | **move-with-refactor** | 宿主持有 FeishuSettings，只把四个字段投影进来；配置结构本身不迁。 |

### 需先重构

- **把 tenant_token 的宿主部署名校验换成本地校验** — tenant_token.rs:276 调 crate::authorization::validate_deployment_name，crate 无法引用宿主模块；规则仅 1..=64 且 [a-z0-9-]，复制为 crate 内 validate_deployment 即可，同时修正 :553/:556 两处测试（`src/addon/feishu/domain/tenant_token.rs:276`）
- **把 outbound_setup::build 改成 crate 构造器 + 宿主注入** — build 目前自己 new RedisTenantTokenCache/HttpClientTransport/TenantTokenProvider 并读 AuthorizationVersionCache。改为宿主只取 http/cache/deployment，交给 FeishuClient::with_redis(FeishuClientConfig, http, redis)；Outbound 改为 pub(crate) type Outbound = yang_feishu::FeishuClient，25 处 outbound.tokens 字段访问零改动；transport(…（`src/addon/feishu/domain/outbound_setup.rs:23-84`）
- **4 处 tenant token 失效刷新循环收敛为一个 helper** — bitable send_json 与 approval send_approval 几乎是逐字重复，attachment/notification 各写一遍 for/loop；统一成 crate 的 send_with_tenant_token(transport, sleeper, tokens, &mut request, policy)：取 token→send_with_retry→TokenExpired 且未刷新→invalidate_and_refresh 后重试一次。四个调用点只保留各自的 decode/校验尾巴（`src/addon/feishu/domain/bitable.rs:849-893; approval.rs:642-673; approval_attachment.rs:145-172; approval_notification.rs:364-395`）
- **user_token 拆成 oauth 纯半与宿主存储半** — 88-470 只依赖 outbound/FeishuCredentials/uuid/serde_json，可整体迁出；478-762 读写 feishu_user_token 单行表并做 select_for_update 刷新锁，必须留宿主。宿主 user_token.rs 用 pub(crate) use yang_feishu::oauth::{...} 桥接，外部调用点（oauth_authorize_url/callback/status、feishu_oauth_reminder）路径不变（`src/addon/feishu/domain/user_token.rs:88-470 与 478-762`）
- **oauth_config(&FeishuSettings) 拆成宿主适配 + crate OAuthConfig::new** — crate 不能依赖 host::FeishuSettings。宿主保留 15 行 oauth_config(settings) 读 can_pull/oauth_redirect_uri，构造 crate 的 OAuthConfig{credentials, redirect_uri}；crate 只做 URL/state/token 端点（`src/addon/feishu/domain/user_token.rs:108-152`）
- **approval_provision 拆成纯校验半与落库半** — parse_form/ProvisionError/check_* 只碰 FormWidget/Column/Value；build_plan/insert_plan/insert_maps/list_option_sources 读写 config/field_map/option 表。迁出 :73-119、:159-211、:505-688；宿主 approval_provision.rs 保留并用再导出桥接 parse_form/ProvisionError，外部 Action 路径不变（`src/addon/feishu/domain/approval_provision.rs:73-119,159-211,505-688`）
- **修掉 outbound 对宿主路径的文档/测试链接** — outbound.rs:555 的 LOCK_TTL 文档链接与 :1275/:1278 的单测断言都写成 crate::addon::feishu::domain::tenant_token::LOCK_TTL_SECONDS；迁入 crate 后改成 crate::tenant_token::LOCK_TTL_SECONDS（`src/addon/feishu/domain/outbound.rs:555,1275,1278`）
- **domain/mod.rs 用 pub(crate) use 做迁移桥** — 把 18 个已迁模块的 mod 声明换成 pub(crate) use yang_feishu::{...}；approval 子模块用 as 别名维持 approval_convert/approval_match/approval_uuid/approval_rate_limit 等旧名；option 族从 yang_feishu::option::{derive,i18n,linkage,pagination,protocol,source_key} 再导出。这样宿主 30+ 文件的 use 行全部零改动，只需保证 crate 侧 item 是 pub（`src/addon/feishu/domain/mod.rs:1-45`）
- **worker 的三处内联出站组装改用 FeishuClient** — feishu_approval_worker.rs:238-263、feishu_event_worker.rs:59-75、feishu_pull.rs:180-195 各自 new 一遍 RedisTenantTokenCache/HttpClientTransport/TenantTokenProvider，并各自复制 deployment_namespace。三处改为 FeishuClient::with_redis(...) 后与 outbound_setup 同一构造路径，少 ~40 行重复（`src/infrastructure/feishu_approval_worker.rs:238-263; feishu_event_worker.rs:59-75; feishu_pull.rs:180-195`）
- **redis feature 化：HttpClientTransport 的审批创建限流字段/方法/分支** — outbound.rs:178 approval_create_limit 字段、:189 with_approval_create_limit、:210-220 分支直接使用 yang_db::RedisClient 与 approval_rate_limit。加 cfg(feature="redis")，关闭时 crate 可不拉 yang-db；宿主保持 default features（默认开 redis），审批创建限流行为不变（`src/addon/feishu/domain/outbound.rs:176-222`）
- **新 crate 的 Cargo.toml 与 feature 矩阵** — 见 public_api/notes 的完整 manifest：yang-base(http) 必选，yang-db(redis) 可选，aes/cbc/subtle/rand_core 走 workspace；default=["redis"]，xlsx 暂不迁。edition/rust-version/license/authors/repository 全部 .workspace=true，publish=false（`lib_yang/crates/yang-feishu/Cargo.toml（新增）`）
- **把 aes/cbc/subtle/rand_core 提升为 workspace 依赖** — 在根 Cargo.toml 的 [workspace.dependencies] 增加 aes="0.8"、cbc={version="0.1",features=["alloc","block-padding"]}、subtle="2.6"、rand_core={version="0.6",features=["getrandom"]}；宿主与 yang-feishu 都用 .workspace=true。root lock 已有 rand_core 0.6.4 与 subtle 2.6.1，实际只新增 aes/cbc 及其 cipher/cpufeatures 链（`lib_yang/Cargo.toml:16-139（建议插在 :32 sha2 之后）`）
- **宿主持久化清单更新** — yang-system Cargo.toml 增加 yang-feishu = { version="0.1.0", path="../../crates/yang-feishu" }；删除只被迁出模块使用的 aes(:32)/cbc(:33)/subtle(:34)；rand_core(:19) 因账号 MFA 保留，sha2/base64/calamine 保留（xlsx 不迁）（`Cargo.toml:8-40`）
- **run_ci.py 接入新成员** — AUXILIARY_PACKAGE_FLAGS(:25-32) 追加 "-p","yang-feishu"（同时覆盖 Auxiliary all-target tests 与 Clippy，满足 :406-407 的 tested/clippy == workspace_packages 断言）；增加 yang-feishu 的 doctest 命令；FEATURE_CASES(:127-153) 增加 yang-feishu-none/yang-feishu-redis/yang-feishu-all；self_test 的 expected_features(:349-367) 同步加入这…（`lib_yang/scripts/run_ci.py:25-32,34-91,127-153,349-367`）
- **ci.yml -p 列表与 feature 矩阵** — stable job 的 test(:57) 与 clippy(:59) 各加 -p yang-feishu；新增 cargo test --doc -p yang-feishu --locked；feature-matrix(:107-179) 增加 3 个 include 条目。MSRV job(:67-79) 的 --workspace 已自动覆盖新成员，无需改（`lib_yang/.github/workflows/ci.yml:56-65,107-179`）
- **verify_feature_isolation.py 增加 yang-feishu 隔离用例** — 新增 Case(yang-feishu-none)：required {reqwest,rustls}（yang-base http），forbidden {sqlx,sqlx-mysql,sqlx-postgres,redis,deadpool-redis,calamine}；Case(yang-feishu-redis)：required {redis,deadpool-redis,reqwest,rustls}，forbidden {sqlx,calamine}；Case(yang-feishu-all)：同上再加 calamine（若将来迁 xlsx）。自检会逐 required 删改…（`lib_yang/scripts/verify_feature_isolation.py:22-32`）
- **verify_ci_contract.py 的 REQUIRED_FRAGMENTS 同步** — CI 契约按整串匹配：必须把 :15-63 里的 test/clippy 两整行替换成带 -p yang-feishu 的新串，并追加 name: yang-feishu-none/redis/all 与 cargo test --doc -p yang-feishu --locked；否则 stable job 的 verify_ci_contract 立即红（`lib_yang/scripts/verify_ci_contract.py:10-63`）
- **schema_anchor 弱档清单重基** — schema_anchor 扫 src/addon/feishu 且 :946-1012 硬编码 weak 文件清单。user_token/approval_provision 的部分行迁出后，原来的弱档站点可能整体消失，the_weakly_checked_sites_are_exactly_the_recorded_ones 的 gone 断言会失败；要按实际扫描结果删/留条目（`src/addon/feishu/domain/schema_anchor.rs:946-1012`）
- **AGENTS.md 结构/命令/CI 计数同步** — AGENTS.md 顶部写“五个基础库 crate”与 17 组 feature 矩阵；新增成员后必须更新 STRUCTURE/COMMANDS/CI 段落，否则下一位代理按文档跑漏新 crate（`lib_yang/AGENTS.md:1-60,100-130`）

### 目标公开 API

| 名称 | 签名 | 备注 |
|---|---|---|
| `FeishuClient` | `pub struct FeishuClient { pub tokens: TenantTokenProvider, transport: Arc<HttpClientTransport>, sleeper: Arc<TokioSleeper> }` | 取代宿主 outbound_setup::Outbound。tokens 用 pub 字段是为了让宿主 25 处 outbound.tokens 零改动；transport()/sleeper() 返回 &dyn。 |
| `FeishuClient::new / with_redis` | `pub fn new(config: FeishuClientConfig, http: HttpClient, cache: Arc<dyn TenantTokenCache>) -> Result<Self, BaseError>; #[cfg(feature="redis")] pub fn with_redis(config: FeishuClientConfig, http: HttpClient, redis: RedisClient) -> Result<Self, BaseError>` | new 供测试注入 FakeCache；with_redis 供宿主生产路径。错误沿用 yang_base::BaseError::ConfigError，避免引第二套错误类型；publish=false 期间可接受，发布前再评估 FeishuError。 |
| `FeishuClientConfig` | `pub struct FeishuClientConfig { pub app_id: String, pub app_secret: String, pub deployment: String, pub approval_create_rate_per_minute: u32 }` | 宿主 FeishuSettings 只投影这四项；deployment 即 AuthorizationVersionCache 的命名空间，复用同一键空间。 |
| `send_with_retry` | `pub async fn send_with_retry(transport: &dyn OutboundTransport, sleeper: &dyn Sleeper, request: &OutboundRequest, policy: RetryPolicy) -> Result<OutboundResponse, OutboundFailure>` | 原样迁出；调用点签名不变。 |
| `send_with_tenant_token` | `pub async fn send_with_tenant_token(transport: &dyn OutboundTransport, sleeper: &dyn Sleeper, tokens: &TenantTokenProvider, request: &mut OutboundRequest, policy: RetryPolicy) -> Result<OutboundResponse, OutboundFailure>` | 新增，4 处刷新循环的唯一入口；调用方只保留各自的 decode/业务码校验。 |
| `OutboundTransport / Sleeper / TokioSleeper` | `pub trait OutboundTransport: Send + Sync { async fn send(&self, request: OutboundRequest) -> Result<OutboundResponse, BaseError>; } pub trait Sleeper: Send + Sync { async fn sleep(&self, duration: Duration); } pub struct TokioSleeper;` | trait 保持 BaseError 返回，宿主测试替身（RecordingSleeper/ScriptedTransport）可原样改路径复用。 |
| `TenantTokenProvider` | `pub fn new(cache: Arc<dyn TenantTokenCache>, transport: Arc<dyn OutboundTransport>, sleeper: Arc<dyn Sleeper>, credentials: FeishuCredentials, deployment: &str) -> anyhow::Result<Self>` | tenant_access_token():290、invalidate_and_refresh():351。crate 内 LOCK_TTL_SECONDS=30 的不变式（TOKEN_RETRY 最坏 19s）继续由单测钉住。 |
| `TenantTokenCache / RedisTenantTokenCache` | `pub trait TenantTokenCache: Send + Sync { async fn get(&self)->anyhow::Result<Option<String>>; async fn put(&self,&str,i64)->anyhow::Result<()>; async fn invalidate(&self)->anyhow::Result<()>; async fn acquire_lock(&self,&str,i64)->anyhow::Result<bool>; async fn release_lock(&self,&str)->anyhow::Result<()>; } #[cfg(feature="redis")] impl RedisTenantTokenCache { pub fn new(redis: RedisClient, deplo…` | trait 无条件；Redis 实现随 feature。宿主的 FakeTokenCache 测试可继续实现该 trait。 |
| `bitable 客户端` | `pub async fn list_all_records(transport, sleeper, tokens, coordinates: &BitableCoordinates, field_names: &[String], max_pages: u32) -> Result<RecordsSnapshot, OutboundFailure>; pub async fn list_all_fields(...); list_all_tables(...); list_all_views(...); batch_update_records(...); get_records_by_ids(...); search_records(...); get_base_name(...); record_row_numbers(...)` | 行号见 symbols；BitableCoordinates:929、RecordItem:199、FieldItem:311、BackfillRow:1413 一起公开。 |
| `approval 客户端` | `pub async fn create_instance(transport, sleeper, tokens, approval_code: &str, form: &str, open_id: &str, uuid: &str) -> Result<CreateOutcome, OutboundFailure>; get_instance(...); get_approval_definition(...); subscribe_approval(...); resolve_department_name(...); resolve_applicant(...)` | resolve_applicant 只收 bearer_token（user token），不碰宿主用户表。 |
| `approval::convert` | `pub fn build_form(input: &FormInput<'_>) -> Result<String, ConvertError>; pub fn build_group_form(...); pub fn build_signature(...); pub struct WidgetMap { pub widget_id: String, pub widget_type: String, pub value: Value }` | 表单 JSON 组装；WidgetMap 是宿主 dispatch/attachment 的跨边界类型。 |
| `approval::matching` | `pub fn match_by_name(form: &[FormWidget], columns: &[Column]) -> MatchOutcome; pub fn match_fields(...) -> Result<Vec<FieldMapping>, MatchError>; pub fn mapping_widgets(...); pub fn preview_widgets(...); pub struct FieldMapping` | crate 内模块名 matching（对应原 approval_match）；FieldMapping 必须保持 Serialize/Deserialize 以兼容宿主 config 行的 JSON。 |
| `approval::attachment` | `pub async fn transfer(transport, sleeper, tokens, coordinates: &BitableCoordinates, record_id: &str, widgets: &[WidgetMap], cells: &Map<String, Value>) -> Result<BTreeMap<String, Vec<String>>, OutboundFailure>` | 附件/图片搬运；preflight/source_identities/attachments 一并公开。 |
| `approval::uuid::derive_uuid` | `pub fn derive_uuid(base_token: &str, table_id: &str, approval_code: &str, record_id: &str) -> String` | v8 形态确定性 UUID；宿主 dispatch/aggregation 共用。 |
| `crypto` | `pub fn derive_key(raw: &str) -> [u8;32]; pub fn encrypt_bytes(&[u8], &[u8;32]) -> Result<String, BaseError>; pub fn encrypt_json(&Value, &[u8;32]) -> Result<String, BaseError>; pub fn decrypt_bytes(&str, &[u8;32]) -> Result<Vec<u8>, BaseError>; pub fn seal(&str,&[u8;32]) -> Result<(String,String),BaseError>; pub fn unseal(&str,&[u8;32]) -> Result<String,BaseError>; pub fn unseal_verified(&str,&str…` | AES-256-CBC + SHA-256 摘要校验；宿主 Action 只做端点编排。 |
| `token` | `pub fn hash_token(&str) -> String; pub fn verify_token(&str,&str) -> bool; pub fn tokens_equal(&str,&str) -> bool` | 入站校验；event/portal/approval 三处共用。 |
| `option 协议族` | `pub fn encode_cursor(i64,&str)->String; pub fn decode_cursor(&str)->Result<Option<(i64,String)>,BaseError>; pub fn build_result_body(...); pub fn normalize_linkage_value(&str)->&str; pub fn derive_options(...); pub fn label_option_id(&str,&str)->String; pub fn ancestor_key(&[&str],&[&str])->String` | crate 内嵌套为 yang_feishu::option::{protocol,i18n,pagination,derive,linkage,source_key}。 |
| `oauth` | `pub struct OAuthConfig { pub credentials: FeishuCredentials, pub redirect_uri: String }; pub fn build_authorize_url(app_id:&str, redirect_uri:&str, state:&str)->String; pub fn mint_state(now:i64)->String; pub fn consume_state(state:&str, now:i64)->bool; pub async fn exchange_code(...); pub async fn refresh_tokens(...); pub async fn fetch_user_info(...); pub fn access_token_is_fresh(i64,i64)->bool;…` | 原 user_token 纯半。宿主 user_token.rs 用 pub(crate) use 再导出这些名字，外部路径不变。 |
| `approval::provision（纯半）` | `pub fn parse_form(form: &Value) -> Result<Vec<FormWidget>, ProvisionError>; pub enum ProvisionError; pub fn resolve_optional_id(...); pub fn check_backfill_targets(...); pub fn check_batch_scope_field(...); pub fn check_aggregate_batch(...)` | 宿主 approval_provision.rs 用再导出桥接，落库函数留在宿主。 |
| `错误与重试策略常量` | `pub enum FailureKind { Retry{..}, TokenExpired, TokenMalformed, PermissionDenied, Fatal{code}, GiveUp } ; pub struct OutboundFailure { pub kind: FailureKind, pub message: String } ; pub struct RetryPolicy { pub max_attempts: u64, pub base_backoff_ms: u64, pub max_backoff_ms: u64, pub max_retry_after_ms: u64 }` | 宿主 dispatch/worker 有大量 match，迁出后类型身份经再导出保持。 |

### 测试处置（30）

| 文件 | 区间 | 内容 | 处置 |
|---|---|---|---|
| src/addon/feishu/domain/outbound.rs | 783-1664 | 36 个分类/退避/幂等/限流单测；其中 :1405-1460 的 real_redis_http_create_limit... 标 #[ignore] 且需 Redis DB15 | **move** |
| src/addon/feishu/domain/tenant_token.rs | 431-931 | 16 个换取/缓存/短锁/降级单测（FakeCache 为主）；:553-556 的部署名校验断言随本地校验改写 | **move** |
| src/addon/feishu/domain/bitable.rs | 1037-2863 | 64 个 URL/分页收敛/单元格形态/路径段校验单测，全部自包含 | **move** |
| src/addon/feishu/domain/approval.rs | 845-1550 | 29 个 create/get/definition/applicant 单测 | **move** |
| src/addon/feishu/domain/approval_convert.rs | 1290-2769 | 64 个表单组装/时间/金额/多选/附件单测 | **move** |
| src/addon/feishu/domain/approval_match.rs | 1031-2477 | 44 个控件匹配/列解析单测 | **move** |
| src/addon/feishu/domain/approval_attachment.rs | 262-447 | 4 个附件元数据/源标识单测 | **move** |
| src/addon/feishu/domain/approval_uuid.rs | 74-149 | 7 个 uuid 稳定性/版本位单测 | **move** |
| src/addon/feishu/domain/approval_rate_limit.rs | 68-205 | 8 个限速间隔单测；两个 real_redis_* 标 #[ignore] 需 Redis DB15 | **move** |
| src/addon/feishu/domain/crypto.rs | 207-509 | 20 个 AES-CBC/PKCS#7/摘要校验/真实 challenge 单测 | **move** |
| src/addon/feishu/domain/token.rs | 58-122 | 8 个 SHA-256/常数时间比较单测 | **move** |
| src/addon/feishu/domain/pagination.rs | 48-128 | 6 个游标编解码单测 | **move** |
| src/addon/feishu/domain/i18n.rs | 127-322 | 13 个 @i18n@ 键/资源构造/联动值归一单测 | **move** |
| src/addon/feishu/domain/derive.rs | 296-755 | 28 个 option_id 派生/祖先链折叠/摘要单测 | **move** |
| src/addon/feishu/domain/source_key.rs | 24-57 | 4 个 key 形态单测 | **move** |
| src/addon/feishu/domain/protocol.rs | 87-372 | 12 个外部选项请求/响应契约单测 | **move** |
| src/addon/feishu/domain/widget_rules.rs | 167-245 | 1 个控件规则表一致性单测 | **move** |
| src/addon/feishu/domain/user_token.rs | 861-1030,1062 | OAuth URL/state 一次性/换码/刷新分类/用户信息/提醒线单测 | **move** |
| src/addon/feishu/domain/user_token.rs | 1037,1085,1105-1300 | 单行插入竞态、UserTokenError 文案、真实 MySQL 往返（#[ignore]，需 YANG_SYSTEM_TEST_MYSQL_URL）；依赖宿主 Repository/配置 | **stay** |
| src/addon/feishu/domain/approval_provision.rs | 159-211,505-688 | parse_form 与 check_* 纯校验单测随纯半迁出；其余 build_plan/insert_* 测试留宿主 | **rewrite** |
| src/addon/feishu/domain/approval_provision.rs | 1502-2731 | 大量落库 plan 测试依赖 FeishuContext/真实表，留在宿主 | **stay** |
| src/addon/feishu/domain/approval_notification.rs | 449-1388 | 18 个消息发送/超时/编辑单测；:1172 的 real_app_sends_rich_feedback... 是 #[ignore] 烟测，读 FEEDBACK_SMOKE_CONFIG 与 crate::config::Settings，不能随 crate | **stay** |
| src/addon/feishu/domain/approval_feedback.rs | 257-535 | 文案渲染/截断/状态 heading 单测 | **stay** |
| src/addon/feishu/domain/approval_row_edit.rs | 180-504 | 补行号/降级单测，依赖 feedback 文案 | **stay** |
| src/addon/feishu/domain/schema_anchor.rs | 930-1095 | 扫描宿源码的弱档清单断言；迁出文件后必须重基 expected，不随 crate | **stay** |
| src/addon/feishu/domain/context.rs | 241-303 | 13 张产品表 Repository 可访问性断言 | **stay** |
| src/addon/feishu/domain/outbound_setup.rs | 116-139 | OutboundFailure→ParamInvalid 映射断言 | **stay** |
| src/infrastructure/feishu_approval_worker.rs | 4906-9081 | 审批 worker 的完整集成/回放测试（含真实 MySQL/Redis 的 #[ignore]），依赖宿主表 | **stay** |
| src/infrastructure/feishu_event_worker.rs | 1330-1975 | 事件 worker 测试，依赖 FeishuContext | **stay** |
| src/infrastructure/feishu_oauth_reminder.rs | 301-309 | 提醒窗口测试，随宿主 user_token 存储半保留 | **stay** |

### 宿主替换点（40）

- `src/addon/feishu/domain/mod.rs:1-45` → 推荐的迁移桥：删除已迁模块的 pub(crate) mod 声明，改为 pub(crate) use yang_feishu::{bitable, crypto, outbound, tenant_token, token}; pub(crate) use yang_feishu::approval; pub(crate) use yang_feishu::approval::{attachment as approval_attachment, convert as approval_convert, matching as approval_match, rate_limit as approval_rate_limit, uuid as approval_uuid, widget_rules}; pub(crate) use yang_feishu::option::{derive,…
- `src/addon/feishu/domain/outbound_setup.rs:23-84` → pub(crate) type Outbound = yang_feishu::FeishuClient; build() 改为：deployment = ctx.tools().extension::<AuthorizationVersionCache>()?.deployment().to_string(); FeishuClient::with_redis(FeishuClientConfig{app_id,app_secret,deployment,approval_create_rate_per_minute}, ctx.tools().http()?.clone(), ctx.tools().cache()?.clone()). require_settings/outbound_error 原样保留。
- `src/addon/feishu/approval/actions/create_config.rs:25-33` → crate::addon::feishu::domain::approval → yang_feishu::approval；approval_match → yang_feishu::approval::matching；approval_provision → yang_feishu::approval::provision 或经宿主 approval_provision 再导出；bitable → yang_feishu::bitable；crypto → yang_feishu::crypto；outbound_setup 留在原路径。
- `src/addon/feishu/approval/actions/dispatch.rs:47-81` → approval/approval_convert/approval_match/approval_feedback/approval_notification/approval_uuid/approval_row_edit/approval_provision/bitable/outbound/outbound_setup/tenant_token/token/user_token 中：convert/match/uuid/provision(纯)/bitable/outbound/tenant_token/token 改 yang_feishu；feedback/notification/row_edit/outbound_setup/repository/context/approval_dispatch/approval_execution 留宿主；user_token 纯 OAu…
- `src/addon/feishu/approval/actions/list_configs.rs:14-18` → approval_match → yang_feishu::approval::matching；approval_provision → yang_feishu::approval::provision；context/repository/list_input 不变。
- `src/addon/feishu/approval/actions/list_widgets.rs:14-17,101` → approval → yang_feishu::approval；approval_match → yang_feishu::approval::matching（:101 preview_widgets）；approval_provision 的 parse_form/list_option_sources 中 parse_form 改 crate、list_option_sources 留宿主；outbound_setup/context 不变。
- `src/addon/feishu/approval/actions/oauth_authorize_url.rs:19` → user_token 变为宿主适配模块：build_authorize_url/mint_state 等纯符号由宿主 user_token.rs 再导出 yang_feishu::oauth::*，此文件路径可不变；若直接改路径则 use yang_feishu::oauth。
- `src/addon/feishu/approval/actions/oauth_callback.rs:26-27` → outbound_setup 不变；user_token 的 exchange_code/fetch_user_info/store_authorization：前两者来自 yang_feishu::oauth（或宿主再导出），store_authorization 留宿主 user_token.rs。
- `src/addon/feishu/approval/actions/oauth_status.rs:18` → user_token::load_single/reauth_reminder_due/now_unix_secs：load_single 留宿主，后两者来自 yang_feishu::oauth（宿主 user_token 再导出即可零改路径）。
- `src/addon/feishu/approval/actions/reveal_config_token.rs:37,221` → crypto::unseal_verified → yang_feishu::crypto::unseal_verified；测试里的 derive_key/seal 同样改 crate。
- `src/addon/feishu/approval/actions/rotate_config_token.rs:30,158-159` → crypto::{issue_token,IssuedToken,derive_key,seal,unseal_verified} → yang_feishu::crypto::*；token::verify_token → yang_feishu::token::verify_token。
- `src/addon/feishu/approval/actions/update_config.rs:19-26` → approval_match → yang_feishu::approval::matching；approval_provision → yang_feishu::approval::provision（纯 parse/check）；bitable → yang_feishu::bitable；outbound_setup 不变。
- `src/addon/feishu/datasource/actions/add_datasource_field.rs:24,31` → crypto::issue_token → yang_feishu::crypto::issue_token；outbound_setup 不变；context/field_maintenance 不变。
- `src/addon/feishu/datasource/actions/create_datasource_table.rs:23-27` → bitable → yang_feishu::bitable；crypto::issue_token → yang_feishu::crypto；field_binding/context/field_maintenance 不变。
- `src/addon/feishu/datasource/actions/health_check.rs:31-38` → bitable → yang_feishu::bitable；outbound → yang_feishu::outbound；outbound_setup/context 不变。
- `src/addon/feishu/datasource/actions/list_bitable_fields.rs:14,18` → bitable → yang_feishu::bitable；outbound_setup 不变。
- `src/addon/feishu/datasource/actions/list_bitable_tables.rs:19,21` → bitable → yang_feishu::bitable；outbound_setup 不变。
- `src/addon/feishu/datasource/actions/list_bitable_views.rs:15,17` → bitable → yang_feishu::bitable；outbound_setup 不变。
- `src/addon/feishu/datasource/actions/pull_probe.rs:41,46` → bitable → yang_feishu::bitable；outbound_setup 不变。
- `src/addon/feishu/datasource/actions/repoint_datasource_field.rs:30-31` → source_key → yang_feishu::option::source_key（或经宿主再导出）；outbound_setup 不变。
- `src/addon/feishu/datasource/actions/reveal_token.rs:27,197` → crypto::{unseal_verified,derive_key,seal} → yang_feishu::crypto::*。
- `src/addon/feishu/datasource/actions/rotate_token.rs:31,169-170` → crypto::{issue_token,IssuedToken,derive_key,seal,unseal_verified} → yang_feishu::crypto；token::verify_token → yang_feishu::token。
- `src/addon/feishu/datasource/actions/set_datasource_field_enabled.rs:33-34` → source_key → yang_feishu::option::source_key；outbound_setup 不变。
- `src/addon/feishu/datasource/actions/set_datasource_field_option_merge.rs:27` → source_key → yang_feishu::option::source_key。
- `src/addon/feishu/datasource/actions/update_datasource_table.rs:22-25` → bitable → yang_feishu::bitable；crypto::{generate_token,seal} → yang_feishu::crypto；field_binding/context 不变。
- `src/addon/feishu/datasource/actions/import_xlsx.rs:44,49` → derive → yang_feishu::option::derive；linkage → yang_feishu::option::linkage；xlsx/pull/option_write/uploaded_files/import_progress/xlsx_job 留宿主。
- `src/addon/feishu/option/actions/approval_options.rs:25-31,638,786` → derive → yang_feishu::option::derive；i18n → yang_feishu::option::i18n；pagination → yang_feishu::option::pagination；protocol → yang_feishu::option::protocol；token → yang_feishu::token；crypto::encrypt_json → yang_feishu::crypto；context 不变。
- `src/addon/feishu/event/actions/event_challenge.rs:20-21` → crypto → yang_feishu::crypto；token::tokens_equal → yang_feishu::token；context 不变。
- `src/addon/feishu/event/actions/receive_approval_event.rs:19-20` → crypto → yang_feishu::crypto；token::tokens_equal → yang_feishu::token；context 不变。
- `src/addon/feishu/portal/actions/portal_query.rs:33` → domain::token::hash_token → yang_feishu::token::hash_token；context/rate_limit 不变。
- `src/infrastructure/feishu_approval_worker.rs:52-82,238-263` → bitable/approval/approval_convert/approval_attachment/approval_uuid/approval_rate_limit/approval_provision(纯)/outbound/tenant_token → yang_feishu::*；approval_feedback/approval_notification/approval_row_edit/repository/context/approval_dispatch/approval_execution/approval_aggregation 留宿主。:238-263 的 cache+transport+sleeper+provider 组装替换为 FeishuClient::with_redis(...)，RoundRunner 改持 Arc<FeishuClient>…
- `src/infrastructure/feishu_event_worker.rs:18-32,59-75` → approval/approval_convert/bitable/outbound/tenant_token → yang_feishu::*；approval_aggregation/context/approval_dispatch 留宿主；:59-75 组装替换为 FeishuClient::with_redis(...)。
- `src/infrastructure/feishu_oauth_reminder.rs:44-47` → outbound/tenant_token → yang_feishu::*；user_token 走宿主 user_token 模块（其纯符号已从 yang_feishu::oauth 再导出）；repository/context/alert 不变。
- `src/infrastructure/feishu_pull.rs:42-48,180-195` → outbound/tenant_token → yang_feishu::*；:180-195 的组装替换为 FeishuClient::with_redis(...)；pull/context/alert 不变。
- `src/addon/feishu/domain/user_token.rs:88-470` → 删除这些定义，文件顶部加 pub(crate) use yang_feishu::oauth::{now_unix_secs, REAUTH_REMINDER_DAYS, reauth_reminder_due, OAuthConfig, build_authorize_url, mint_state, consume_state, TokenBundle, TokenEndpointError, exchange_code, refresh_tokens, fetch_user_info, access_token_is_fresh}; oauth_config(settings) 保留为宿主适配器（读 can_pull/oauth_redirect_uri 后构造 OAuthConfig）。
- `src/addon/feishu/domain/approval_provision.rs:73-119,159-211,505-688` → 删除这些定义，文件顶部加 pub(crate) use yang_feishu::approval::provision::{ProvisionError, parse_form, resolve_optional_id, check_backfill_targets, check_batch_scope_field, check_aggregate_batch}; build_plan/insert_*/list_option_sources/resolve_external_options 保留。
- `Cargo.toml:8-40` → 新增 yang-feishu = { version = "0.1.0", path = "../../crates/yang-feishu" }；删除 aes(:32)、cbc(:33)、subtle(:34)（仅迁出模块使用）；保留 rand_core(:19，账号 MFA)、sha2(:23)、base64(:30)、calamine(:40，xlsx 不迁)。
- `src/addon/feishu/domain/schema_anchor.rs:946-1012` → 按迁移后实际扫描结果重基 expected 集合：user_token.rs / approval_provision.rs 的弱档条目可能因代码迁出而消失（gone 断言会红），逐项删除或保留并写理由；不要新增条目。
- `src/addon/feishu/mod.rs:31-155` → 不变：build_addon/build_context 继续装配 13 张产品表与 FeishuSettings；bootstrap.rs:360/384/406/422 与 workers 的 build_context 调用不变。
- `src/bootstrap.rs:8,360-430` → 不变：alert 符号留宿主；四处 build_context 仍返回 Arc<FeishuContext>。仅当 build_context 内部改用 crate 的 FeishuClient 时才需要动。

### 风险

- lib_yang CI 不编译 project/yang-system（根 Cargo.toml exclude），所以 crate 公开面改名/收窄不会在 lib_yang CI 里暴露；抽取后必须在 yang-system 目录单独跑 cargo check --all-targets --locked（含 clippy）才能证明宿主替换点齐。
- 兼容再导出要求 crate 侧 item 全部 pub，会把 decode_cursor/bucket_key/hash_token 等内部 helper 抬成永久 semver 面；publish=false 期间可接受，首次发布前应改为直接路径（约 32 个文件、90 行 use）或收窄到 curated prelude。
- aes/cbc 是 root lock 新增项（rand_core 0.6.4、subtle 2.6.1 已在 lock）。需重生成并提交根 Cargo.lock，否则所有 --locked 命令红；MSRV job 用 --all-features 会实测 aes/cbc 的 MSRV 1.80 兼容性。
- cargo-deny 会对新增依赖（aes/cbc 的 cipher/cpufeatures 链，若将来迁 xlsx 还有 calamine/zip/zopfli/atoi_simd）做 advisory/license/source 检查；advisories 的 unused-ignored-advisory=deny 与 verify_dependency_policy 的 reason 格式（scope/review-by/exit、180 天窗口）必须同步，否则 supply-chain job 红。
- redis 关闭时 HttpClientTransport 不再装共享审批创建限流器（官方 100 次/分钟），这是一个静默的行为降级；宿主 yang-system 必须保持 yang-feishu default features（default=["redis"]），不能 --no-default-features。
- feature-matrix 与 clippy 都带 -D warnings；迁入代码原本在 yang-system 的 [lints.clippy] unwrap_used/expect_used=deny 下写成，但 crate 只继承 workspace lints，可能触发新的 clippy 告警（例如 large_enum_variant、needless_pass_by_value）；需要按 crate 实际 lint 结果补 allow 或改签名。
- user_token 拆分后，UserTokenError（宿主）需要 impl From<yang_feishu::oauth::TokenEndpointError>；由于 UserTokenError 是宿主本地类型，orphan 规则允许，但必须放在宿主 user_token.rs 而不是 crate。
- approval_provision 的 ProvisionError 跨边界后，宿主 build_plan/insert_* 与 Action 响应都依赖它；若 crate 将其改为 #[non_exhaustive] 或改变体，宿主 match 会破。建议首版原样迁出、不做重构。
- schema_anchor 的 gone 断言在文件搬走后会失败（弱档站点消失被当成异常）；这不是编译错误而是测试红，容易在抽取时被忽略。必须在同一提交重基 :946-1012。
- approval_notification 的 real_app_sends_rich_feedback 烟测读 FEEDBACK_SMOKE_CONFIG 与 crate::config::Settings；它不能进 crate，若误迁会让 crate 反向依赖宿主配置。同理 approval_feedback 的中文文案不应下沉到通用 crate。
- xlsx/calamine 若被判定要一起抽，会显著扩大依赖面（zip 4.2.0、zopfli、quick-xml、atoi_simd 等）并新增 cargo-deny 风险；本方案按“非飞书平台契约”留宿主，xlsx 相关代码 1541 行继续留在 yang-system。
- root workspace 的 [workspace.package].rust-version=1.80 与 run_ci.py self_test 的 member manifest 断言要求新 crate 用 rust-version.workspace=true；漏写会让 python scripts/run_ci.py --self-test 直接失败（不是 cargo 失败）。
- AGENTS.md 写的“五个基础库 crate / 17 组 feature 矩阵”会与新增成员漂移；verify_ci_contract 只校验 ci.yml 文本，不校验人数/组数，文档漂移不会被门禁抓住。

