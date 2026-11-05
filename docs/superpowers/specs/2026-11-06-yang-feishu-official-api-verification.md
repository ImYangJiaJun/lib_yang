# yang-feishu 官方接口核验与覆盖（2026-11-06）

> 工具：本地 `lark-cli 1.0.98`（`lark-cli <domain> --help` / `--dry-run` / `schema`）
> + 飞书官方 markdown 文档（`https://open.feishu.cn/llms-docs/zh-CN/...`）。
> 目的：核验已实现接口，并补全基础库缺失的官方端点。

## 1. 已实现 typed 接口核验

| 模块 | 方法 | 官方路径 | 结论 |
|---|---|---|---|
| approval | `create_instance` | `POST /open-apis/approval/v4/instances` | ✅ |
| approval | `get_instance` | `GET /open-apis/approval/v4/instances/{instance_id}` | ✅ |
| approval | `get_approval_definition` | `GET /open-apis/approval/v4/approvals/{approval_code}` | ✅ |
| approval | `subscribe_approval` | `POST /open-apis/approval/v4/approvals/{approval_code}/subscribe` | ✅ |
| bitable | `list_all_records` / `first_records_page` | `GET /open-apis/bitable/v1/apps/{app_token}/tables/{table_id}/records` | ✅ |
| bitable | `list_all_fields` | `GET .../fields` | ✅ |
| bitable | `list_all_tables` | `GET /open-apis/bitable/v1/apps/{app_token}/tables` | ✅ |
| bitable | `list_all_views` | `GET .../views` | ✅ |
| bitable | `search_records` / `search_all_records` | `POST .../records/search` | ✅ |
| bitable | `batch_update_records` | `POST .../records/batch_update` | ✅ |
| bitable | `get_records_by_ids` | `POST .../records/batch_get` | ✅ |
| im | `send_post` / `edit_post` | `POST /open-apis/im/v1/messages` / `PUT .../{message_id}` | ✅ |
| drive | `download_media` / `upload_approval_file` | `GET /open-apis/drive/v1/medias/{token}/download` / `POST www.feishu.cn/approval/openapi/v2/file/upload` | ✅ |
| contact | `resolve_department_name` | `GET /open-apis/contact/v3/departments/{department_id}` | ✅ |
| contact | `resolve_applicant` | `GET /open-apis/search/v1/user` | ✅ |

结论：抽取时的 typed 接口路径与官方文档一致；无需改路径。

## 2. 新增官方端点薄封装

目录 JSON（可直接用于重新生成）：

- `2026-11-06-yang-feishu-api-catalog-bitable.json`（44）
- `2026-11-06-yang-feishu-api-catalog-approval.json`（17）
- `2026-11-06-yang-feishu-api-catalog-contact.json`（5）
- `2026-11-06-yang-feishu-api-catalog-im.json`（48）
- `2026-11-06-yang-feishu-api-catalog-drive.json`（29）

生成结果（`src/*_endpoints.rs` / `src/bitable/endpoints.rs` / `src/approval/endpoints.rs`）：

| 模块 | 生成方法数 | 说明 |
|---|---:|---|
| bitable v1 | 42 | App/表/视图/字段/记录/仪表盘/表单/工作流/角色 |
| approval v4 | 13 | 撤回/抄送/查询/任务同意拒绝加签退回转交/催办 |
| contact | 5 | 用户/搜索用户/搜索机器人/个人资料/部门 |
| im | 46 | 消息/群聊/成员/管理员/禁言/昵称/Pin/表情/文件/图片/feed 分组 |
| drive | 25 | 文件/元数据/权限/评论/回复/统计/订阅/配额 |
| Base v3（`bitable/v3.rs`） | 107 | 对齐 `lark-cli base` 的 `/open-apis/base/v3/...` 表面 |

另手工实现 6 个二进制/多部分端点：IM 文件/图片上传、消息资源下载；Drive
文件/素材上传、文件下载。

> Base v3 说明：官方 llms 索引只覆盖 bitable v1；`/open-apis/base/v3` 来自本地
> lark-cli dry-run。升级 lark-cli 后应重新生成 `bitable/v3.rs`。

## 3. 未生成的 CLI 命令

以下命令属于 CLI 组合命令、仅 BaseApp/工作区/模板/区块等非官方 llms 表面，
或需要真实文件/输出路径，未生成薄封装：

- `lark-cli base`：`app-block-create`、`app-block-get`（保留为字段解析辅助）、
  `button-rule-bind/unbind/get`、`dashboard-block-create`、`record-upload-attachment`、
  `record-download-attachment`、`title-resolve`、`url-resolve`。
- 其中 record/table/workflow 的基本 CRUD 已由 bitable v1 薄封装覆盖；其余为
  CLI 组合命令、需要复杂 `data_config`、二进制上传下载或 BaseApp 专属能力。
- `lark-cli drive` 的 `export/import/version/search/secure-label/cover/preview/sync/status`
  属于高层组合命令，未生成；核心文件/权限/评论/上传下载已覆盖。
- `lark-cli im` 的 `feed shortcuts/flag` 属于用户个性化快捷命令，未生成。

## 4. 重新生成

目录生成脚本与端点生成脚本目前是本次施工的临时工具；如需长期维护，建议把
`lark-cli <domain> --help/--dry-run` 的采集与 `src/*_endpoints.rs` 的生成固化到
`scripts/`（本次未提交临时脚本，避免把 CLI 版本耦合进 CI）。
