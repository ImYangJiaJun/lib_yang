# Changelog

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
