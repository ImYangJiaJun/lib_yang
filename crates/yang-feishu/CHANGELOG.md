# Changelog

## 0.3.0

- 新增 `approval::relation_record_ids(cell)`：从关联列单元格取 record_id，接受
  `{"link_record_ids":[...]}`、`[{"id":"rec…"}]` 与 `["rec…"]`；其它形态返回
  `None`，不会把普通超链接/文本误当成关联列。
- `build_signature` 对 `connect` 控件 + 关联列单元格改用**排序去重**后的
  record_id 作签名值（与宿主 2026-11-06 关联列功能对齐）：顺序与重复不影响签名，
  换一批行必换签名。
- `approval::connection_codes(cell, approval_app_id)` 由私有改为公开：单格
  AppLink / 36 位实例 Code → Code 数组；传 `None` 时只接受实例 Code。
- `bitable::dto::FieldProperty` 新增 `table_id: Option<String>`：单向/双向关联列
  （type 18/21）指向的数据表 id。
- 新增 `bitable::encode_cell_value(field_type, value)`：写方向单元格编码（单选裸串、
  日期毫秒、人员数组、链接对象、文本取 `text`）；`None` 表示「该值不该写」，
  与 `Err(CellEncodeError)` 刻意区分。人员数组的元素必须带非空 `id`（空数组仍
  合法，用于清空），`["ou_1"]` / `[{"text":"张三"}]` 直接 `Err` 而不是等平台拒绝。
- `connection_codes` 的 `None` 判定下移到「确是 AppLink」之后：普通非法文本
  （只填了申请编号等）仍返回可行动原文案，不再被内部措辞覆盖。
- **破坏性变更**：`options::build_result_body` 去掉 `has_more` 参数，改由
  `next_page_token.is_some()` 派生，避免两个参数互相矛盾；新增
  `FeishuResultBody::empty(default_locale)`。
- 文档修正：`bitable/dto.rs` 不再声称全部 `#[non_exhaustive]`（实际不标，保证结构体
  字面量 drop-in）；补用户 OAuth 配方；`AGENTS.md` 的方法名与 `has_more`/加密示例
  改为可直接编译的写法。

## 0.2.0

- **破坏性变更**：`default = []`。typed 模块与 66 个官方生成域全部改为 feature
  门控：`bitable` / `approval` / `contact` / `im` / `drive`，单域 `api-<domain>`，
  分组 `docs` / `calendar` / `task` / `mail` / `attendance` / `hr` / `ai` /
  `admin` / `auth`，以及 `api-all` / `full`。旧用法需显式开启所需 feature，
  或使用 `features = ["full"]` 保持全量。
- `full` 从 `http+redis+metrics` 升级为“全部能力”的超集；依赖隔离仍由
  `http` / `redis` / `metrics` 单独控制。
- 端点目录修正（对照官方 Go SDK 与 llms 文档）：
  - 新增 `GET /open-apis/im/v2/tags`（`im_tags_list`）；
  - 新增 `POST /open-apis/interactive/v1/card/update`
    （`im_interactive_card_update`）；
  - 新增 `POST /open-apis/wiki/v2/nodes/search`（`wiki_nodes_search_v2_user`）；
  - 新增 `POST /open-apis/slides_ai/v1/slide_image/render`
    （`slides_ai_slide_image_render`）；
  - 新增 `POST /open-apis/drive/explorer/v2/file/:folderToken`
    （`drive_explorer_file_create`，提取器现同时扫描 `ext/api.go`）；
  - 修正 `PUT /open-apis/sheets/v2/.../dataValidation/:sheetId` 的多余路径参数；
  - 删除不存在的 `PUT /open-apis/sheets/v3/spreadsheets/:token`；
  - `POST /open-apis/authen/v2/oauth/token` 改为无鉴权
    （`authen_oauth_token_public`）。
- 语义修复：
  - IM `ReceiveIdType` 增加 `chat_id`；
  - 事件入站支持 v2.0 `schema/header` 信封；
  - 宽松路径段拒绝 `.`/`..`/`%2e`/`%2f`/`%5c` 回退片段；
  - 5xx + 未知业务码改为可重试，补充多维表格/云盘/IM 瞬态错误码；
  - Base v3 `v3_record_list` 改为显式 query，表单接口补充 body；
  - `Multipart` 改为公开并返回 `Result`，校验字段值/Content-Type 的 CRLF；
  - spark `GET .../storage` 按二进制下载生成，binary 优先于 multipart。
- 生成器支持目录条目可选 `name` 覆盖方法基名，并给每个生成域加
  `#[cfg(feature = "api-<domain>")]`；覆盖率脚本增加 feature 与
  binary/multipart 标志校验。
- CI 拆分 `yang-feishu` 的 all-target 测试为 `--all-features`，避免
  `required-features` 测试被静默跳过；feature matrix 增加 default / bitable /
  docs / api-all 组合。

## 0.1.0

- 从 `project/yang-system` 抽取飞书开放平台协议层：
  - `transport`：`Transport` / `Sleeper` / `Request` / `Response` / `FailureKind` /
    `classify` / `disposition` / 重试常量；`http` feature 提供 `ReqwestTransport`。
  - `auth`：tenant token 换取/缓存/短锁、`TokenCache`、`MemoryTokenCache`、
    `redis` feature 的 `RedisTokenCache`、用户 OAuth 协议与 `StateStore`。
  - `bitable`：记录/字段/表/视图读取、搜索、批量回写、分页收敛断言、链接解析。
  - `approval`：建单/查单/定义/订阅、幂等 uuid、表单解析与值编码、控件规则。
  - `contact` / `im` / `drive`：部门名、姓名→open_id、post 发送/编辑、媒体下载与
    审批附件上传。
  - `event`：challenge/回调解密与 verification_token 校验。
  - `options`：外部选项 DTO、`@i18n@` 装配、keyset 游标。
  - `crypto`：AES-256-CBC 与 SHA-256/常数时间比较。
- 公开 DTO/枚举暂不标 `#[non_exhaustive]`：宿主的既有测试与结构体字面量需要 drop-in
  兼容；0.x 的破坏性变更走 minor。
- `FormInput` / `GroupFormInput` 增加 `approval_app_id: Option<&str>`：关联审批的
  AppLink 解析不再硬编码产品 appId；传 `None` 时只接受 36 位实例 Code。
- 按官方文档 + `lark-cli 1.0.98` 目录补全长尾端点薄封装：bitable v1 42、approval v4
  13、contact 5、IM 46、Drive 25、Base v3 107；另加 IM/Drive 二进制上传下载 6 个。
- 新增 `client.api()` 全量官方端点：覆盖官方 SDK、llms 文档与历史版本中的 1709 个
  REST 端点，按 66 个域生成 2367 个方法（tenant/user/app/public 身份变体）；包含
  multipart 上传（`content_type` + `&[u8]`）与二进制下载（原始 `Response`）。
  目录与生成/校验脚本见 `docs/superpowers/specs/2026-11-06-yang-feishu-openapi-*.json`
  与 `scripts/generate_openapi_endpoints.py`、`scripts/verify_openapi_coverage.py`。
- 新增 crate 级 `AGENTS.md` 与 `skills/yang-feishu/SKILL.md` 使用指南。
- 不依赖 `yang-base` / `yang-db` / `schemars` / `anyhow`。
- 已知缺口：`X-Lark-Signature`、Lark 国际版 `upload` 域真机验证。
