# 新前端第一性原理验收与修复计划

**日期：** 2026-07-19  
**状态：** 已完成  
**工作方式：** 用户确认直接在当前工作区修改，不创建 worktree  
**事实源：** `docs/FRONTEND_FIRST_PRINCIPLES.md`、当前 `yang-base`、独立应用 `project/yang-system`、运行态 HTTP 与浏览器行为

## 1. 最终验收目标

验收对象不是“后端已经定义若干 UI DTO”，而是以下用户可观察结果：

1. 后端注册一个可访问 Action 后，不写对应前端页面，也能在默认界面发现它、填写参数、发起真实 HTTP 请求并查看结果。
2. 后端声明 Table/View 元数据后，同一前端自动升级为表格、表单、树、筛选、分页和关系选择器等通用业务体验。
3. 复杂场景可用稳定 `view_id` 映射到前端静态注册表；不存在或加载失败时安全降级，后端不能下发可执行代码或物理文件路径。
4. 身份、权限、租户、业务校验和 step-up 始终由服务端执行；前端隐藏或禁用只用于体验提示。
5. JSON、下载、预览、重定向与受限 multipart 均有明确且可测试的调用、展示和失败行为。

WebSocket 明确不在本轮范围内。

## 2. 第一性原理验收规则

- **可观察结果优先：** 类型、schema、提交和单元测试不能替代浏览器中的真实闭环。
- **安全事实源唯一：** UI catalog 可以提示能力，但每次真实 Action 调用仍必须经过同一份服务端授权、租户与校验边界。
- **未知输入安全降级：** 未知 schema 版本、枚举、控件和 `view_id` 不得导致任意代码加载或静默误调用。
- **默认路径永远可用：** 自定义页失败时回退通用页，通用页不可解释时回退 ActionDemo。
- **契约可演进：** schema 版本、运行时校验、缓存失效和错误诊断必须显式，不依赖刷新页面恢复状态。
- **每阶段单独验收：** 静态检查、单元/契约测试、构建、真实 HTTP、浏览器 E2E 五类证据按风险选择，缺一项必须记录原因。

## 3. 基线证据

- [x] 根仓库 `python scripts/run_ci.py quick` 通过。
- [x] `yang-base` `transport_axum` 38 个测试通过。
- [x] `project/yang-system` 11 个测试通过。
- [x] 排除 `br/` 与历史验收工件后，新前端 `package.json`、Vite 配置和 Vue/TSX 页面数量为 0。
- [x] Node 24、npm 11、pnpm 10 可用。
- [x] 根仓库现有未跟踪 `br/`、`.superpowers/`；独立 `yang-system` 现有 `Cargo.lock` 修改，均保留且不回退。

## 4. 验收问题账本

| ID | 优先级 | 问题 | 初始状态 | 完成条件 |
|---|---|---|---|---|
| FE-A1 | Critical | 没有目标前端工程，三层渲染均不可运行 | 已修复 | Vue 3 + TypeScript 应用可构建、可启动并有 E2E |
| FE-A2 | Critical | 没有默认 ActionDemo，阶段 1 产品验收失败 | 已修复 | 自动发现 Action、生成输入、真实调用并展示各响应类型 |
| FE-A3 | High | 没有统一 API client 与身份/租户/request-id 生命周期 | 已修复（阶段 1） | 单一调用边界覆盖参数来源、错误、附件和上下文切换 |
| FE-A4 | High | 没有 Table/Form/Tree 通用渲染器 | 已修复 | 标准 View 完成列表、表单、查询、树和操作验收 |
| FE-A5 | High | 关系 options 只有 DTO 和应用示例，没有前端组件与库级默认执行器 | 已修复 | `{value,label}` 搜索、回填、分页、过滤与租户边界均通过 |
| FE-A6 | High | 没有 `view_id` 静态注册表和三级 fallback | 已修复 | 自定义覆盖、未知 ID、加载失败三条路径可验证 |
| FE-A7 | High | 前端没有 schema 版本兼容与运行时验证 | 已修复 | 不支持版本和畸形 payload fail-closed 且可诊断 |
| FE-A8 | High | 没有 multipart 前端调用和上传生命周期验收 | 已修复 | 文件约束、错误、服务端边界与安全失败均验证 |
| BE-A1 | High | 内置关系 options 执行器仍在 BACKLOG | 已修复 | 公共安全实现替代应用层重复代码并有对抗性测试 |
| BE-A2 | High | 树节点上限是先读后拦 | 已修复 | 数据库读取使用 `max_nodes + 1` 边界并检测截断 |
| BE-A3 | High | 认证与租户中间件顺序仅靠文档 | 已修复 | 构建期拒绝内置中间件不安全反序，不擅自重排自定义链 |
| BE-A4 | Medium | step-up 缺内置 re-auth Action 且 proof 可在 TTL 内重放 | 已修复 | 内置 Action + 单实例/Redis 原子一次性消费形成默认安全闭环 |
| DOC-A1 | Medium | 原理文档第 30、179、181 行与已实现后端现状矛盾 | 已修复 | 文档成为当前态验收说明，不再混用历史缺口与现状 |

## 5. 执行顺序

### 阶段 1：默认 ActionDemo

- [x] 在 `project/yang-system/frontend` 建立 Vue 3 + TypeScript 单组件库工程。
- [x] 定义并运行时校验 `UiCatalog`、`ActionDemoSchema` 和响应 envelope。
- [x] 实现统一 API client：path/query/header/body、认证、租户、request-id、JSON 与附件响应。
- [x] 实现 Action 目录、schema 表单、原始 JSON fallback、请求/响应/错误/耗时展示。
- [x] 以无数据库真实 YANG 服务完成匿名 Action、JSON/download/preview/redirect 浏览器验收；受保护 Action 留在阶段 2 身份验收。

### 阶段 2：通用业务页

- [x] 实现 TableView、Form、Tree、分页、排序、搜索、结构化筛选。
- [x] 实现 Action placement/interaction/confirmation/availability。
- [x] 实现关系选择器并修复所需后端默认执行能力。
- [x] 用真实业务 fixture 完成字段和操作浏览器验收；org 投影与身份/租户边界由 Rust 契约测试覆盖。

### 阶段 3：自定义页面

- [x] 建立静态 `view_id` 注册表。
- [x] 实现 custom → generic → ActionDemo 三级解析与诊断。
- [x] 添加一个可视化自定义页作为边界样例。
- [x] 对抗性验证路径注入、未知 ID 和动态加载失败。

### 阶段 4：非 JSON 与工程化

- [x] 实现 multipart 表单、文件约束提示和附件响应处理。
- [x] 实现 schema 版本拒绝策略、缓存/失效和运行时错误边界。
- [x] 建立单元、契约和浏览器 E2E 门禁。
- [x] 修复验收中发现的后端结构性缺口并同步文档。

## 6. 最终门禁

- [x] Rust quick/full 与 8 组数据库集成命令通过。
- [x] 前端 format/lint/typecheck/unit/build/E2E 通过。
- [x] 新增普通 JSON Action 后不改前端即可完成默认演示。
- [x] 标准 View 自动渲染且租户/权限由服务端 fail-closed。
- [x] 自定义页面覆盖与两级安全降级通过。
- [x] multipart 与附件响应通过真实浏览器验证。
- [x] `docs/FRONTEND_FIRST_PRINCIPLES.md` 与实现、BACKLOG、验收证据一致。

## 7. 执行日志

- 2026-07-19：开始正式验收。确认后端基线为绿，但目标前端产物为 0；判定整体未完成，进入阶段 1 修复。
- 2026-07-19：阶段 1 闭环。新增 Vue 3 + TypeScript + Element Plus 前端、真实 Rust 验收服务、8 个单测与 2 个 Chromium E2E；修复 Axum 未回传 `x-request-id`（新增 3 个红绿回归测试，完整 transport 39/39 通过）、参数展示元数据未合并、Fetch `opaqueredirect` 误报 HTTP 0、Vitest/E2E 边界污染和 Element Plus 全量打包。注意：RTK 的 `pnpm typecheck` 快捷过滤曾给出假阴性，后续门禁固定使用 `rtk proxy pnpm typecheck` 或 production build。
- 2026-07-19：阶段 2-4 实现闭环。schema 升级为 2.2 并显式声明 `data_action`；完成通用 Table/Form/Tree、关系 options、静态 custom view、multipart、身份/租户隔离 revision cache；同时修复 select count/search、树读前上限、module view 权限组合、bulk 误标、关系排序、内置中间件反序和 step-up proof 重放。
- 2026-07-19：最终验收通过。`pnpm check`：5 个文件 15 个单测 + production build；Chromium E2E 7/7；`yang-system` 11/11；`yang-base --lib` 536 通过、4 ignored；Axum transport 39/39；all-targets/all-features Clippy 零错误；根 `scripts/run_ci.py quick` 与 `full` 通过。数据库 integration 聚合脚本因每个 MySQL case 独立启动容器而超过 10 分钟工具上限，随后按脚本原始 8 条命令逐项执行：MySQL 31、PostgreSQL 6、Redis 22，共 59/59；PostgreSQL 使用临时 `postgres:16` 容器真实验证后已删除。E2E 过程中额外修复目录加载覆盖用户导航选择的竞态；full 过程中修复 `prefetch_limit` 在 no-default-features 下的 dead code；并用 `.prettierignore` 固化生成声明文件的格式门禁边界。
