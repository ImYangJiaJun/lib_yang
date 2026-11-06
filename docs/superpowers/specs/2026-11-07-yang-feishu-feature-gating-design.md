# yang-feishu 按域 feature 门控 + 端点全量核验设计（2026-11-07）

## 1. 意图

`yang-feishu` 目前把 66 个官方域、2367 个生成方法全部编译进默认产物。宿主只用到
bitable / approval / im 等少数域，却要承担全部生成代码的编译成本；同时，官方目录
与实现之间仍有少量未闭合的差异，需要一个可重复的核验基线。

目标：

1. 按**官方文档域**拆分 Cargo feature，默认不启用任何 API 域，做到真正的按需编译。
2. 保留 typed 核心链路的独立 feature（bitable / approval / contact / im / drive）。
3. 提供少量聚合 feature（docs / hr / ai / admin 等），便于整组开启。
4. 产出一份可复核的端点核验报告，并把确定的目录错误修掉。

## 2. 非目标

- 不改变任何请求/响应的运行时语义；feature 只控制编译期可见性。
- 不把宿主业务语义（审批状态机、字段映射、附件搬运编排）搬进 crate。
- 不为省依赖而把 `aes`/`sha2`/`base64` 等协议原语做成可选；它们仍是核心依赖。
- 不重新设计 `client.api()` 的方法签名；生成器只增加 `cfg` 门控。

## 3. 现状

- `src/api/mod.rs` 声明 66 个 `pub mod`；每个文件是 `impl ApiClient` 的薄封装。
- `src/lib.rs` 无条件导出 `api` / `approval` / `bitable` / `contact` / `drive` / `im`。
- `src/client.rs` 无条件提供 `api()` / `bitable()` / `approval()` / `contact()` /
  `im()` / `drive()` / `oauth()`。
- 生成器 `scripts/generate_openapi_endpoints.py` 只生成 `mod` 声明，不带 `cfg`。
- 覆盖率校验 `scripts/verify_openapi_coverage.py` 已接入 CI quick。
- feature 矩阵 `scripts/run_ci.py` / `.github/workflows/ci.yml` / `verify_ci_contract.py`
  目前只覆盖 `http` / `redis` / `metrics` / `all`。

## 4. 端点核验结论（确定性部分）

核验源：

- 官方 Go SDK `larksuite/oapi-sdk-go` 最新 `service/**/resource.go`（commit
  `db9f52d0`，2026-10-15），提取 1643 条端点。
- 官方 llms 文档（51 个 bundle、3164 个文档页），解析出 1153 条带
  `HTTP URL` / `HTTP Method` 的端点。
- 本地 `lark-cli 1.0.98` 的 `schema` / `--dry-run`。

结论：

- 修正前仓库目录 1709 条 = 最新 SDK 1643 条 + 手工/legacy 66 条；**SDK 侧 0 缺失**。
  修正后 1713 条 / 2372 个方法（见审计报告）。
- 文档侧 1153 条中，1147 条与仓库目录 path/method 一致；6 条缺失（见下）。
- 文档侧身份与仓库目录无“文档有、仓库完全没有”的冲突。
- SDK 与仓库有 2 条 token 差异，均为 legacy 补充 `User` 变体，文档确认可接受：
  - `GET /open-apis/hire/v2/interview_records`
  - `POST /open-apis/attendance/v1/archive_rule/upload_report`（文档明确
    `tenant_access_token` 或 `user_access_token`）

### 4.1 需要修的目录问题

| # | 端点 | 问题 | 修正 |
|---:|---|---|---|
| 1 | `GET /open-apis/im/v2/tags` | 文档有、目录缺失（SDK 只有 Create/Patch） | 新增，domain=im，tokens=[Tenant] |
| 2 | `POST /open-apis/interactive/v1/card/update` | 文档有、目录缺失（延时更新消息卡片） | 新增，domain=im，tokens=[Tenant] |
| 3 | `POST /open-apis/wiki/v2/nodes/search` | 文档有、目录缺失（搜索 Wiki） | 新增，domain=wiki，tokens=[User] |
| 4 | `PUT /open-apis/sheets/v2/spreadsheets/:token/dataValidation/:sheetId/:dataValidationId` | 文档路径只有 `:sheetId`，多了一段 | 改为 `.../dataValidation/:sheetId` |
| 5 | `PUT /open-apis/sheets/v3/spreadsheets/:token` | 官方只有 GET/POST/PATCH，没有 PUT | 从 manual 目录删除 |
| 6 | `POST /open-apis/authen/v2/oauth/token` | legacy 标了 `App`，实际无 Bearer，凭证在 body | tokens 改为 `[]`（生成 `_public`） |

### 4.2 明确排除的端点

| 端点 | 原因 |
|---|---|
| `GET /open-apis/authen/v1/authorize` | 浏览器授权跳转，不是服务端 REST 调用 |
| `POST /approval/openapi/v2/file/upload` | `www.feishu.cn` 域名 + multipart，已由 typed `drive::upload_approval_file` 覆盖；生成器固定用 open 域名，不能直接加目录 |

### 4.3 语义审计

typed 模块（approval / bitable / im / drive / contact / event / options / crypto /
auth / transport / client）与 66 条 manual/legacy 端点的语义审计由并行只读工作流
产出，结论写入 `docs/audit/2026-11-07-yang-feishu-endpoint-audit.md` 第 6 节。
发现按“确认 / 存疑 / 驳回”分级；确认项作为本任务的修复输入（可与 feature
门控同批提交），存疑项记录为后续任务。语义审计不改写官方契约，只修实现与目录
的不一致。

### 4.4 文档侧核验的可复现性

仓库新增 `scripts/audit_official_docs.py`：从 51 个 llms bundle 拉取 3164 个文档页，
解析 `HTTP URL` / `HTTP Method` / `请求头 Authorization`，输出
`docs/superpowers/specs/2026-11-07-yang-feishu-doc-catalog.json` 快照。
该脚本需要网络，不接入 CI；报告引用快照与逐条文档 URL 作为证据。

### 4.5 新增条目的稳定方法名

目录新增条目带可选 `name` 字段覆盖生成器基名，避免版本冲突产生 `_1` 后缀：

| 端点 | 生成方法 |
|---|---|
| `GET /open-apis/im/v2/tags` | `im_tags_list` |
| `POST /open-apis/interactive/v1/card/update` | `im_interactive_card_update` |
| `POST /open-apis/wiki/v2/nodes/search` | `wiki_nodes_search_v2_user` |
| `PUT .../dataValidation/:sheetId` | `sheets_spreadsheets_data_validation_update`（原名不变，只修路径） |
| `POST /open-apis/authen/v2/oauth/token` | `authen_oauth_token_public`（原 `_app` 改为无鉴权） |

## 5. Feature 设计

### 5.1 命名

- 每个官方域一个 `api-<domain>`，与 `src/api/<domain>.rs` 一一对应。
  domain 保留原样下划线，例如 `api-document_ai`、`api-human_authentication`。
- typed 模块各一个 feature：`bitable` / `approval` / `contact` / `im` / `drive`。
  其中 `bitable` 同时启用 `api-bitable` + `api-base`；`approval` 同时启用
  `api-approval`；`contact` 启用 `api-contact` + `api-user` + `api-directory` +
  `api-trust_party`；`im` 启用 `api-im` + `api-message` + `api-bot` + `api-cardkit`
  + `api-event` + `api-ephemeral`；`drive` 启用 `api-drive`。
- 核心协议层（`transport`、`client`、`endpoints`、`error`、`path`、`auth`、
  `crypto`、`event`、`options`、`multipart`、`ApiClient` 空壳）**始终编译**。

### 5.2 聚合 feature

| feature | 包含 |
|---|---|
| `docs` | `drive` + `api-docx` `api-docs` `api-wiki` `api-sheets` `api-slides_ai` `api-block` `api-board` `api-lingo` `api-suite` |
| `im` | `im` + `api-im` `api-message` `api-bot` `api-cardkit` `api-event` `api-ephemeral` |
| `contact` | `contact` + `api-contact` `api-user` `api-directory` `api-trust_party` |
| `calendar` | `api-calendar` `api-vc` `api-meeting_room` `api-minutes` |
| `task` | `api-task` |
| `mail` | `api-mail` |
| `approval` | `approval` + `api-approval` |
| `attendance` | `api-attendance` |
| `bitable` | `bitable` + `api-bitable` `api-base` |
| `hr` | `api-corehr` `api-ehr` `api-hire` `api-payroll` `api-compensation` `api-performance` `api-okr` |
| `ai` | `api-aily` `api-spark` `api-document_ai` `api-optical_char_recognition` `api-speech_to_text` `api-translation` `api-baike` |
| `admin` | `api-admin` `api-tenant` `api-application` `api-security_and_compliance` `api-unified_kms` `api-verification` `api-human_authentication` `api-face_verify` `api-personal_settings` `api-workplace` `api-moments` `api-report` `api-search` `api-helpdesk` `api-acs` `api-passport` `api-mdm` `api-jssdk` `api-pay` `api-apaas` |
| `auth` | `api-auth` `api-authen`（只门控生成薄封装；核心 `auth` 模块始终可用） |
| `api-all` | 全部 66 个 `api-*` |
| `full` | `api-all` + 5 个 typed + `http` + `redis` + `metrics` |

### 5.3 默认值

```toml
[features]
default = []
```

`full` 语义从“http+redis+metrics”升级为“全部能力”，是旧 `full` 的超集；
依赖隔离仍由 `http` / `redis` / `metrics` 单独控制。

### 5.4 代码改动

1. `scripts/generate_openapi_endpoints.py`：在 `src/api/mod.rs` 的每个
   `pub mod <domain>;` 前生成 `#[cfg(feature = "api-<domain>")]`；增加可选
   `name` 字段覆盖方法基名，用于 `wiki/v1` 与 `wiki/v2` 这类同基名不同版本的
   冲突（v2 命名为 `wiki_nodes_search_v2`）。
2. `src/lib.rs`：`approval` / `bitable` / `contact` / `drive` / `im` 及对应
   `*_endpoints` 模块加 `#[cfg(feature = "...")]`。
3. `src/client.rs`：对应 `bitable()` / `approval()` / `contact()` / `im()` /
   `drive()` 访问器加 `#[cfg]`；`api()` / `oauth()` 始终存在。
4. `Cargo.toml`：新增 66 个 `api-*`、5 个 typed、13 个聚合 feature；
   `default = []`；`full` 改为全量。
5. `tests/`：
   - `api_surface` → `required-features = ["api-docx", "api-contact", "api-im", "api-auth"]`
   - `approval_contract` → `["approval"]`
   - `bitable_pagination` → `["bitable"]`
   - `endpoints_contract` → `["im"]`
   - `public_api` → `["drive", "im"]`
   - `event_verify` / `transport_retry` 保持默认。
6. `examples/bitable_pull.rs`：`required-features = ["http", "bitable"]`。
7. CI：
   - **必须改 stable 的 auxiliary all-targets 命令**：现有
     `cargo test --all-targets -p yang-base-derive -p yang-pcg -p yang-runtime -p yang-feishu`
     在空 feature 下会静默跳过带 `required-features` 的测试。拆成
     `... -p yang-base-derive -p yang-pcg -p yang-runtime` 与
     `cargo test --all-targets -p yang-feishu --all-features` 两条；
     `verify_ci_contract.py` 的对应片段同步替换（不是只新增）。
   - `run_ci.py` STABLE_COMMANDS 同步拆分；FEATURE_CASES 增加
     `yang-feishu-default`、`yang-feishu-bitable`、`yang-feishu-docs`、
     `yang-feishu-api-all`。
   - `.github/workflows/ci.yml` feature-matrix 同步增加。
   - `verify_ci_contract.py` REQUIRED_FRAGMENTS 同步增加四个矩阵名与新命令。
   - `verify_feature_isolation.py` 增加 `yang-feishu-bitable` 隔离用例。
8. 文档：README / AGENTS.md / skills/yang-feishu/SKILL.md 增加 feature 表与
   `default-features = false` 示例；CHANGELOG 记录 0.2.0 破坏性变更。

### 5.5 `-Dwarnings` 约束

CI 的 feature-matrix 对每个组合设置 `RUSTFLAGS=-Dwarnings` 与
`RUSTDOCFLAGS=-Dwarnings`，因此门控后必须做到：

- 把 `src/lib.rs` 里指向 `OpenApiClient::bitable` 的 intra-doc link 改成纯文本
  （模块被 `cfg` 掉后该链接会解析失败）。
- 检查并消除 `--no-default-features` 下的 `dead_code`：
  - `src/api/mod.rs` 的 `join_url` / `query_vec` / `MAX_BINARY_BYTES` 只在生成域
    启用时使用；
  - `ClientCore` 的 `call_json_public` / `call_raw_json*` 等方法只在 API/typed
    feature 启用时使用；
  - `src/multipart.rs` 只在 `im` / `drive` 启用时需要。
  优先用 `#[cfg(any(feature = "...", ...))]` 精确门控；无法简洁表达时用
  `#[cfg_attr(..., allow(dead_code))]` 并注明原因。
- `src/endpoints.rs` 中仅 `drive` 使用的 `upload` 域名等私有字段同样按需 `cfg`。

### 5.6 防漂移

`scripts/verify_openapi_coverage.py` 增加三项校验，避免新增域后忘记加 feature：

1. 每个 `src/api/<domain>.rs` 在 `mod.rs` 中都有
   `#[cfg(feature = "api-<domain>")]`；
2. `Cargo.toml` 存在对应的 `api-<domain>` feature；
3. `api-all` 的 feature 列表覆盖全部 `api-*`。

### 5.7 兼容性

- `default = []` 是破坏性变更：旧宿主必须显式开启所需 feature，或直接用
  `features = ["full"]` 保持旧行为。
- `full` 变为超集，不会让旧 `features = ["full"]` 的宿主少功能。
- 版本从 0.1.0 升到 0.2.0（0.x 的破坏性变更走 minor）。

## 6. 验收标准

1. `cargo check -p yang-feishu --no-default-features` 通过；`ApiClient` 存在但无 API 方法。
2. `cargo check -p yang-feishu --no-default-features --features bitable` 只编译
   typed bitable + bitable/base 两个生成域。
3. `cargo check -p yang-feishu --no-default-features --features api-docx` 只编译
   docx 生成域。
4. `cargo check -p yang-feishu --all-features` 通过；生成方法数仍为 2372（修正后
   按新增/删除条目调整，覆盖率脚本输出 0 missing / 0 extra）。
5. `python -I scripts/verify_openapi_coverage.py` 通过。
6. `cargo test -p yang-feishu --all-features`、`cargo clippy ... -D warnings`、
   `cargo test --doc` 全部通过。
7. 端点核验报告列出 6 条目录修正与语义审计结论；修复后文档侧 0 缺失（排除 4.2）。
