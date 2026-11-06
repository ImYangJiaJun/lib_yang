# yang-feishu 端点全量核验报告（2026-11-07）

## 1. 核验方法

| 来源 | 用途 | 规模 |
|---|---|---:|
| 官方 Go SDK `larksuite/oapi-sdk-go` @ `db9f52d0`（2026-10-15） | path/method/身份/multipart/binary 的机器目录（`resource.go` + `ext/api.go`） | 1644 条 |
| 官方 llms 文档（51 个 bundle、3164 个文档页） | 文档侧 HTTP URL/Method/凭证要求 | 1153 条 |
| 本地 `lark-cli 1.0.98` | `schema` / `--dry-run` 交叉核验 | 15 个 typed 域 |
| 仓库目录 `2026-11-06-yang-feishu-openapi-{catalog,manual,legacy}.json` | 现有生成输入 | 1713 条（修正后） |

文档侧快照由 `scripts/audit_official_docs.py` 生成：
`docs/superpowers/specs/2026-11-07-yang-feishu-doc-catalog.json`（1153 条）。
该脚本需要网络，不接入 CI。

核验维度：HTTP method、path、身份变体（tenant/user/app/public）、multipart、
binary、路径参数、分页与错误码语义。生成侧由
`scripts/verify_openapi_coverage.py` 做目录→源码的逐条对照。

## 2. 覆盖结论

- 最新 SDK 提取（`resource.go` + `ext/api.go`）1644 条，全部存在于仓库目录，0 缺失。
- 仓库目录修正后 1713 条（SDK 1644 条 + 手工/legacy 去重补充）。
- 文档侧 1153 条经修正后，除第 4 节两条排除项外 **0 缺失**。
- 文档侧身份解析：610 条 Tenant、463 条 Tenant+User、72 条 User、8 条无身份；
  **文档提到但目录缺失的身份变体为 0**；目录相对文档多出的变体 19 条（SDK 支持的
  更宽身份），属于“目录更宽松”而非缺能力。
- 仓库目录相对 SDK 多出的 70 条中，51 条能在文档侧找到同 path/method 记录；
  其余为 slides_ai / corehr / drive explorer 等 llms 索引未覆盖的条目，已用
  `lark-cli` 或官方 SDK 逐条核验，见第 5 节。

## 3. 确认需要修的目录问题

| # | 端点 | 证据 | 结论 | 修正 |
|---:|---|---|---|---|
| 1 | `GET /open-apis/im/v2/tags` | <https://open.feishu.cn/document/tenant-tag/list.md>：HTTP URL/Method 明确；请求头 `tenant_access_token` | SDK 只有 Create/Patch，GET 缺失 | 新增到 manual，domain=im，tokens=[Tenant]，方法 `im_tags_list` |
| 2 | `POST /open-apis/interactive/v1/card/update` | <https://open.feishu.cn/document/server-docs/im-v1/message-card/delay-update-message-card.md>：HTTP URL/Method；请求头 `tenant_access_token` | 目录完全缺失（延时更新消息卡片） | 新增到 manual，domain=im，tokens=[Tenant]，方法 `im_interactive_card_update` |
| 3 | `POST /open-apis/wiki/v2/nodes/search` | <https://open.feishu.cn/document/server-docs/docs/wiki-v2/search_wiki.md>：HTTP URL/Method；凭证要求 `user_access_token` | 目录只有 v1 `/wiki/v1/nodes/search`；文档已是 v2 | 新增 v2 到 manual，domain=wiki，tokens=[User]，方法 `wiki_nodes_search_v2_user`；保留 v1 |
| 4 | `PUT /open-apis/sheets/v2/spreadsheets/:token/dataValidation/:sheetId/:dataValidationId` | <https://open.feishu.cn/document/server-docs/docs/sheets-v3/datavalidation/update-datavalidation.md>：URL 只有 `:spreadsheetToken` 与 `:sheetId` | manual 多了一段 `:dataValidationId`，生成的路径错误 | 改为 `.../dataValidation/:sheetId`；方法名保持 `sheets_spreadsheets_data_validation_update` |
| 5 | `PUT /open-apis/sheets/v3/spreadsheets/:token` | 官方文档 v3 只有 GET/POST/PATCH（`spreadsheet/patch.md`） | manual 凭空多出 PUT，与 PATCH 重复且不存在 | 从 manual 删除 |
| 6 | `POST /open-apis/authen/v2/oauth/token` | <https://open.feishu.cn/document/authentication-management/access-token/get-user-access-token.md>：请求头只有 Content-Type，凭证在 body 的 `client_id`/`client_secret` | legacy 标 `App`，生成的 `authen_oauth_token_app` 会错误要求 `app_access_token` Bearer | tokens 改为 `[]`，生成 `authen_oauth_token_public` |
| 7 | `POST /open-apis/slides_ai/v1/slide_image/render` | `lark-cli slides xml_presentation.slide_image render --dry-run`：method POST、url `/open-apis/slides_ai/v1/slide_image/render`、query `xml_presentation_id` | manual 只有 list（`.../slide_images`），缺 render | 新增 manual，tokens=[Tenant,User]，方法 `slides_ai_slide_image_render`；原 list 摘要改为“获取幻灯片截图” |
| 8 | `POST /open-apis/drive/explorer/v2/file/:folderToken` | 官方 Go SDK `service/ext/api.go:106`，Tenant/User | 提取器只扫 `resource.go`，漏掉 `ext/api.go` | 新增 manual，方法 `drive_explorer_file_create`；提取器改为同时扫描 `api.go` |

## 4. 明确排除的端点

| 端点 | 原因 |
|---|---|
| `GET /open-apis/authen/v1/authorize` | 浏览器授权跳转（OAuth authorize URL），不是服务端 REST 调用；由 `auth::oauth` 的 URL 构造覆盖 |
| `POST /approval/openapi/v2/file/upload` | 域名是 `www.feishu.cn`，且为 multipart；已由 typed `drive::upload_approval_file` 覆盖。生成器固定使用 `open` 域，不能直接加入 `src/api` 目录 |

## 5. manual/legacy 15 条不在 llms 索引的条目

| 条目 | 核验结果 |
|---|---|
| `GET /open-apis/corehr/v2/onboarding_flows` | ✅ 官方文档 `corehr-v1/pre_hire/onboarding-flow.md` 明确 HTTP URL |
| `POST /open-apis/corehr/v2/onboarding_qr_codes` | ✅ `create-onboarding-qr-code.md` 明确 |
| `POST /open-apis/corehr/v2/onboarding_qr_codes/dimension` | ✅ `onboarding-qr-code-dimension.md` 明确 |
| slides_ai 10 条 `xml_presentations` | ✅ `lark-cli slides` 原始资源与 `--dry-run` 对齐（spot-check：`GET /open-apis/slides_ai/v1/xml_presentations/:id`） |
| `PUT /open-apis/sheets/v3/spreadsheets/:token` | ❌ 不存在，见第 3 节 #5 |
| `PUT /open-apis/sheets/v2/.../dataValidation/:sheetId/:dataValidationId` | ❌ 多参数，见第 3 节 #4 |

另：legacy 给 `attendance/v1/archive_rule/upload_report` 补充 `User` 与
`hire/v2/interview_records` 补充 `User`，与官方文档一致（SDK 只标 Tenant 属于
SDK 侧收窄），保留。

### 5.1 未纳入的文档页

`llms-app-information` 列出的 3 条 application 服务端接口（获取应用管理员管理范围、
查询用户是否在应用开通范围、查询租户购买的付费方案）的 `.md` 链接均返回 404，
最新 Go SDK 也没有对应端点；未加入目录，待 API Explorer 确认真实 path/method 后
再补。

## 6. typed 模块语义审计

由独立只读工作流按模块审计（approval / bitable / im / drive / contact /
event+options+crypto / auth+transport+client / manual+legacy 目录），每条发现再经
两个独立视角对抗核验。最终：8 个模块、53 条原始发现，44 条存活、9 条驳回；
完整性批评补充的盲区已处理身份解析、binary/multipart、ext 扫描、multipart 公开
与覆盖率标志校验，剩余见 5.1 与 6.2。

### 6.1 本批已修复

| 模块 | 问题 | 修复 |
|---|---|---|
| im | `ReceiveIdType` 缺 `chat_id`，无法向群发 post | 新增 `ChatId` + `oc_` 校验与测试 |
| event | 只解析 v1.0 信封，v2.0 `schema/header` 事件被拒 | 新增 v2.0 解析（`header.event_id/event_type/token`）+ 测试 |
| path | 宽松路径段允许 `.`/`..`/`%2e`，URL 规范化可回退到其他端点 | 拒绝路径回退片段 + 测试 |
| transport | 5xx + 未知业务码被判 `Fatal`，网关/后端超时不重试 | 5xx 一律 `Retry`；补 1254291/1255040/1061045/1066002/1069601/230049 |
| bitable v3 | `v3_record_list` 把 `field_id=TEST&view_id=TEST&offset=1&limit=1` 写死进 URL | 改为显式 `query` 参数 |
| bitable v3 | `v3_form_create` / `v3_form_questions_update` / `v3_form_update` 不发送 body | 增加 `body: &Value` 并透传 |
| generator | spark `GET .../storage` 同时带 `WithFileUpload()` 与 `resp.File`，被生成为 multipart JSON | `binary` 优先于 `multipart`；提取器不再把 GET 标成 multipart |
| coverage | 覆盖率脚本只比 path/method/identity，multipart/binary 误标可通过 | 增加生成代码的 binary/multipart 标志对照（当前 0 mismatch） |
| multipart | `Multipart` 是 `pub(crate)`，外部无法构造 43 个 multipart 端点的请求体；`text`/`file` 用 `assert!` 会 panic，且不校验字段值/Content-Type | 改为 `pub mod multipart` + 公开 `Multipart`；`text`/`file` 返回 `Result`，校验 CRLF/控制字符 |
| doc audit | `auth_tokens()` 只认行首 `|`，1153 条文档身份全部解析为空 | 修正解析并重跑：0 条“文档有、目录缺”的身份变体 |
| extractor | 只扫 `resource.go`，漏掉 `service/ext/api.go` | 同时扫描 `api.go`；补入 `drive/explorer/v2/file/:folderToken` |
| catalog | 见第 3 节 8 条目录修正 | 已重新生成并过覆盖率校验 |

### 6.2 待决策/后续（未改）

以下为核验后保留的语义风险；涉及公开 API 形态或宿主策略，未在本批强行修改。

| 模块 | 问题 | 建议 |
|---|---|---|
| drive | `upload_media` 未发送官方 `extra`（docx_image / bitable_file 等场景会定位失败） | 增加 `extra` 参数或新方法 |
| drive | `upload_file` 大小上限未前置校验 | 加大小上限 |
| drive | `permissions/{token}/public` 使用已标记历史版本的 v1 | 迁移 v2 字段形状 |
| contact | 单个模糊命中直接 `Resolved`；`resolve_applicant` 只取第一页 | 模糊命中改 `Ambiguous`；补分页 |
| contact | `department_id_type` 仅凭 `od-` 前缀推断 | 改为显式参数或双查 |
| approval | `is_external` 字段、document/address/v1 attachment 编码、金额币种取 `currencyRange[0]` | 按官方文档补编码规则 |
| approval | `create_instance` 缺 `node_approver_open_id_list` / `node_cc_open_id_list`；写接口 `idempotent=false` | 扩展参数；评估重试策略 |
| approval | 1390007 同时表示“已订阅”与“已取消订阅” | 按官方语义区分 |
| bitable v3 | `v3_record_upsert` 必填 `record_id` 且只 PATCH，无法创建 | 改名或增加 create 分支 |
| bitable | `search_all_records` 未检测重复 `page_token` | 与 list_* 路径一致硬失败 |
| im | `upload_image` 硬编码 PNG；`upload_file` 未发送 `duration` | 按实际类型推断；补可选参数 |
| transport | `MESSAGE_RETRY.max_retry_after_ms=1s` 截断服务端 reset；token 锁等待与最坏 19s 不匹配 | 调整重试上限与锁等待 |
| transport | `CredentialsProvider` 只在 build 时取一次；token 刷新失败统一 `Fatal{code:0}` | 按调用取凭证；区分可重试刷新失败 |
| transport | 生成二进制端点固定 64 MiB；typed 侧 200 MiB / 调用方上限不一致 | 做成参数或按端点配置 |
| event | `X-Lark-Signature` 未实现，`EventVerifier` 不接收 headers | 增加 `decode_with_headers`，按官方 sha256(timestamp+nonce+key+body) 校验 |
| options | `build_result_body` 在 `hasMore=true` 时仍可能没有 `nextPageToken` | 按官方外部选项契约校验 |

## 7. 修复后的验收

1. `python -I scripts/verify_openapi_coverage.py`：0 missing / 0 extra，
   feature gate checks 0 issues。
2. 用 `scripts/audit_official_docs.py` 重新生成文档快照后，除第 4 节排除项外
   **0 缺失**（当前 diff 仅剩 `GET /open-apis/authen/v1/authorize` 与
   `POST /approval/openapi/v2/file/upload` 两条排除项）。
3. 新增/修正条目生成的方法可被 `cargo test -p yang-feishu --all-features` 编译。
