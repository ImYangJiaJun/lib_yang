# 侧边栏导航单一事实源实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 以 Catalog 为生产导航事实源，合并权限入口、退役便签、统一审批控制台。

**Architecture:** app_route 贯穿 Spec、编译期 RuntimeModule 和请求级 Schema，前端映射为 link。保留 Registry 剪枝和通用页面，identity 只表示导航功能域；审批子页面通过 tabs 编排。

**Tech Stack:** Rust、React 19、TypeScript、Zod、Vitest、React Router 7。

**Spec:** docs/superpowers/specs/2026-10-08-navigation-catalog-single-source-design.md

## Global Constraints

- 两仓库工作树 D:/code/lib_yang-navigation-catalog 与 project/yang-system，分支 codex/navigation-catalog；不操作主检出、不推送、不合并。
- 保留 AppLayout.tsx/App.test.tsx 用户修改；AccountSwitcher 内嵌在 AppLayout。以下路径相对对应仓库。
- UI_SCHEMA_VERSION = 2.4；前端同时接受 2.2/2.3/2.4。
- app_route 必须单 / 开头、非 //、最多 512 个 Unicode 码点，无 Unicode White_Space、Cc 控制字符、反斜杠；前端不使用语义不同的 \s，U+FEFF 两端均接受；后端不判断 SPA 路由存在性。
- 审批 list_configs/list_requests 共享 feishu.approval.read，两者必须显式 present_action；不使用 public dispatch 做导航门控。
- 普通用户无权限工作台菜单但保留组自服务 API；identity 不是授权角色。
- 删除生产 demo，不删除 example demo 静态 registry/view，不删除数据库或历史授权；不加依赖、不变更分层。
- 真实应用构建为 build_application(test_tools(), test_security()) 两参数，依赖 tokio 运行时。所有生产行为先 RED→GREEN，新增判断做摘除式变异。

## Review Focus

1. 非站内/歧义路由：Task 1/2 覆盖 //、反斜杠、空白、控制字符、Unicode 长度边界。
2. 显式字段复制漏掉：Task 1 覆盖请求级 app_route，并测试旧 Module 的 None。
3. presentation 意外暴露原始表：Task 3 覆盖控制台 Module 无声明 view，内部默认视图没有 data_action，因此不会进入请求级 TableView。
4. 导航消费者遗漏或 SPA 落点不存在：Task 4 覆盖 AppLayout/切换器/身份选择/首页，Task 6 测实际路由匹配。
5. demo 权限残留或普通用户越权：Task 3/6 覆盖目录、权限样本、孤儿与自服务不变。

---

### Task 1: yang-base 完整路由契约

**Files:** Modify crates/yang-base/src/definition/ui/module.rs、builder/compile.rs、builder/registry.rs、ui/mod.rs；Test ui/__tests__/module_test.rs、catalog_test.rs、form_test.rs（均在 definition 下）。

**Interfaces:** Produces Spec.app_route: Option<String>、fn app_route(self, impl Into<String>) -> Self、RuntimeModule.app_route、Schema.app_route 和版本 2.4。

- [x] Step 1: 复用 module_test 的 action/NoopAction，route fixture 的认证 Catalog 断言如下；非法路径构建返回 InvalidReference，合法 /、512 Unicode 字符通过，旧 Module 为 None。
```rust
assert_eq!(catalog.modules[0].app_route.as_deref(), Some("/console/page"));
assert!(build_route("//evil.example/x").is_err());
```
- [x] Step 2: cargo test -p yang-base --lib definition::ui --locked；Expected: 新字段/builder 缺失而 RED。
- [x] Step 3: Spec/new/schema/RuntimeModule 加字段；compile 与 projection 两处复制。路径校验独立于标题校验，错误用 BuildError::InvalidReference。
```rust
pub fn app_route(mut self, route: impl Into<String>) -> Self {
    self.app_route = Some(route.into()); self
}
let valid = route.starts_with('/') && !route.starts_with("//")
    && route.chars().count() <= 512
    && !route.chars().any(|c| c.is_whitespace() || c.is_control() || c == '\\');
```
- [x] Step 4: schema 字面量补 None，常量及版本测试改 2.4；cargo fmt --all；cargo test -p yang-base --lib definition::ui --locked；Expected: GREEN。逐一摘除路径守卫/复制确认对应测试 RED，再恢复 GREEN。
- [x] Step 5: 显式 git add 上述七文件；git commit -m "feat(ui): 增加站内 Module 路由并升级契约到 2.4"。

### Task 2: 前端解析与 link 模型

**Files:** Modify frontend/src/engine/contracts/ui-catalog.ts、frontend/src/engine/catalog/module-pages.ts；Test frontend/tests/engine/contracts/ui-catalog.test.ts、frontend/tests/engine/catalog/module-pages.test.ts。

**Interfaces:** Consumes Task 1；Produces ModulePageDefinition.link?: string，兼容 app_route null/省略。

- [x] Step 1: 现有 fixture 添加 2.4 route；断言解析保留 app_route、构建页面 link；旧版本/null/省略兼容；非法路径和 Unicode 长度与后端一致。
```ts
expect(parseUiCatalog(envelope).modules[0]?.app_route).toBe("/account");
expect(buildAccountModulePages(catalog)[0]?.link).toBe("/account");
```
- [x] Step 2: pnpm --dir frontend exec vitest run tests/engine/contracts/ui-catalog.test.ts tests/engine/catalog/module-pages.test.ts；Expected: 版本或 route 断言 RED。
- [x] Step 3: SUPPORTED 追加 2.4，Zod 可选/可空路径校验单斜线、字符规则、Array.from(route).length；link 映射。补 iconTokens 的 access/admin_panel_settings/database/send，未知回退 extension。
```ts
link: module.app_route ?? undefined,
```
- [x] Step 4: 重跑目标测试、pnpm --dir frontend typecheck；Expected: GREEN。摘除 Zod 守卫/link 映射分别 RED，再恢复。
- [x] Step 5: 显式 git add 上述四文件；git commit -m "feat(frontend): 解析目录路由并映射导航落点"。

### Task 3: 后端声明与生产便签退役

**Files:** Modify src/addon/mod.rs、account/user/mod.rs、access/grants/mod.rs、feishu/datasource/mod.rs、feishu/approval/mod.rs、src/app.rs；Delete src/addon/demo 已跟踪文件；Modify src/addon/access/domain/sensitive_permissions.rs、groups/resolution.rs、tests/permission_groups_integration.rs、README.md、AGENTS.md、docs/guides/ADDON_ONBOARDING.md；Regenerate frontend/contracts/openapi.json、frontend/src/engine/contracts/api-types.ts。

**Interfaces:** admin_identity(): admin/系统管理/admin_panel_settings/order20；四 route /account、/access/workspace、/feishu/datasources、/feishu/approval。不改变授权。

- [x] Step 1: tokio::test 构建真实应用，冻结四个声明落点及身份、审批显式读 Action、primary 门控和无 demo 权限；控制台无声明 view，内部默认视图均无 data_action。应用不能调用框架 pub(crate) 的 with_user；不增加测试后门，认证剪枝由框架现有测试与真实认证集成验证。
```rust
let app = build_application(test_tools(), test_security()).unwrap_or_else(|e| panic!("构建失败: {e:#}"));
assert!(!catalog.actions.iter().any(|a| a.operation_id.starts_with("demo.notes.")));
```
- [x] Step 2: cargo test --lib app::tests --locked；Expected: route/目录集合 RED。
- [x] Step 3: 添加 admin_identity；账号 /account，grants admin + /access/workspace，datasource admin + route + primary list_datasources；approval 无 primary、显式 present_action 两读 Action，用 ActionPlacement::Toolbar/ActionInteraction::Invoke。Result 注册函数用 ?。
- [x] Step 4: apply_patch 删除生产 demo/装配/专用日志/P7 测试；普通权限样本用 feishu.datasource.read，孤儿用 legacy.notes.read；保留 example demo/历史表。更新当前 README/AGENTS/接入说明，不重写历史评估。
- [x] Step 5: cargo fmt；python scripts/check_architecture.py；cargo test --lib --locked；Expected: GREEN。摘除 presentation 门控/审批展示分别 RED，再恢复。python scripts/dump_openapi.py 生成契约；不得手改。
- [x] Step 6: 显式 stage 上述修改、删除及生成物；git commit -m "feat(navigation): 声明控制台入口并退役生产便签"。

### Task 4: 全部导航消费者统一

**Files:** Modify frontend/src/shell/AppLayout.tsx（内嵌 AccountSwitcher）、frontend/src/features/auth/pages/SelectIdentityPage.tsx、frontend/src/shell/pages/DashboardPage.tsx；Test frontend/tests/shell/App.test.tsx、navigation.test.ts、frontend/tests/features/auth/identity-flow.test.tsx。

**Interfaces:** Consumes page.link，所有 Module 入口使用 page.link ?? `/m/${page.id}`；显式 view 深链接不改，未认领 view 仍可生成工作台。

- [x] Step 1: 真实 Provider/Router 渲染导航，断言无硬编码业务项/重复、user/admin 过滤；切换器、身份选择、首页卡片使用 link，缺省回退 /m/{id}。
```tsx
expect(screen.getByRole("link", { name: /权限管理/ })).toHaveAttribute("href", "/access/workspace");
```
- [x] Step 2: pnpm --dir frontend exec vitest run tests/shell/App.test.tsx tests/shell/navigation.test.ts tests/features/auth/identity-flow.test.tsx；Expected: 新 link/重复菜单 RED。
- [x] Step 3: 删除三组硬编码导航及失效 canRead/import，保留用户空间镜头修改；shell 图标补四 token，身份文案称功能域。
```tsx
to={page.link ?? `/m/${page.id}`}
```
- [x] Step 4: 重跑目标测试 GREEN；摘除一个 link 使用或加回硬编码菜单测试 RED 后恢复。
- [x] 评审修正：AppLayout 复用 resolveIdentityLanding 集中纠正已保存的失效域，删除 Dashboard 重复 effect；覆盖 Cookie 恢复及运行期撤销管理权限。摘除纠正 effect 后两个新测试均 RED，恢复后 GREEN。
- [x] Step 5: 显式 stage 上述六文件；git commit -m "refactor(frontend): 导航入口统一消费 Catalog"。

### Task 5: 审批 tabs 和旧 URL

**Files:** Create frontend/src/features/feishu/views/ApprovalConsolePage.tsx、frontend/tests/features/feishu/views/approval-console-page.test.tsx；Modify frontend/src/shell/routes.tsx、frontend/tests/shell/routes.test.tsx、frontend/src/features/feishu/api.ts、views/ApprovalRequestsPage.tsx，以及 tests/features/feishu/views/ 下的 approval-harness.ts 和既有页面测试。

**Interfaces:** /feishu/approval，?tab=requests；旧 configs/requests Navigate replace 到对应 tab。复用现有 api.ts/pages 和 @/engine 出口。

- [x] Step 1: 测默认配置/query记录/无权限/非法 tab；单 operation fixture 检验投影鲁棒性（不承诺实际读权限可拆）；键盘 Left/Right/Home/End、focus、aria-controls/tabpanel；MemoryRouter 实际旧 URL 重定向。
```tsx
expect(screen.getByRole("tab", { name: "配置" })).toHaveAttribute("aria-selected", "true");
```
- [x] Step 2: pnpm --dir frontend exec vitest run tests/features/feishu/views/approval-console-page.test.tsx tests/shell/routes.test.tsx；Expected: 缺新组件/路由 RED。
- [x] Step 3: useSearchParams 持久 tab，保留无关 query；hasOperation 控制 visible tab、无权限 aria-live；roving tabIndex/键盘 focus 与 aria 引用，复用两个既有页面；主 lazy route 用 RouteFallback，旧路由重定向。
```tsx
<Navigate to="/feishu/approval?tab=requests" replace />
```
- [x] Step 4: 目标测试/typecheck GREEN；摘除键盘/重定向分支 RED，再恢复。
- [x] Step 5: 显式 stage 新组件/测试、routes/test；git commit -m "feat(feishu): 合并审批配置与记录控制台"。

### Task 6: 夹具与落点守护

**Files:** Modify frontend/tests/features/access/workspace-api.test.ts、permission-groups-page.test.tsx、permission-meta.test.ts、module-page-access-grants.test.tsx；frontend/tests/engine/contracts/ajv.test.ts；Create frontend/tests/shell/nav-single-source.test.ts；Modify src/app.rs tests、docs/guides/ADDON_ONBOARDING.md。

**Interfaces:** 声明 fixture 用真实 access.grants.*，孤儿用 legacy.notes.read；route 守护使用 matchRoutes 的末节点，不仅源码 grep。

- [x] Step 1: 对四个业务路径实际 matchRoutes，断言末节点 route.path；后端冻结 Catalog route 精确集合与 landing 完整性。
```ts
expect(matchRoutes(appRoutes, "/feishu/approval")?.at(-1)?.route.path).toBe("feishu/approval");
```
- [x] Step 2: 暂时摘除一个路径确认守护 RED 后恢复；已有正确行为的守护初次 GREEN 如实记录，不伪称功能 RED。
- [x] Step 3: 替换 demo 权限 fixture/注释，保持孤儿语义；接入指南记录 primary/present_action/route/identity 规则。
- [x] Step 4: pnpm --dir frontend exec vitest run tests/features/access tests/shell/nav-single-source.test.ts；cargo test --lib app::tests --locked；Expected: GREEN；rg 核对当前生产源码/生成物无 demo.notes（独立 example/历史文档保留）。
- [x] Step 5: 显式 stage 上述文件；git commit -m "test(nav): 固定控制台落点并清理便签夹具"。

### Task 7: 验收与最终复核

**Files:** 本计划专用 ledger，记录 RED/GREEN、提交、ruling、未验证项；重要验收问题先失败回归测试再修复。

**Interfaces:** 最终交付两个未推送分支、准确验证证据，不把未执行写成通过。

- [ ] Step 1: 父仓库 python scripts/run_ci.py quick；应用 python scripts/run_ci.py quick、pnpm --dir frontend check、cargo clippy --all-targets --all-features --locked -- -D warnings；Expected: exit0。长日志保存在计划目录，核对退出码。
  实际：clippy 与各前端子检查通过，标准聚合命令未全绿；父仓库 quick 的末次 doctest 报 LNK1102，默认前端测试启动遇到 spawn UNKNOWN/PowerShell NullReference。限两 worker 的729项前端测试通过；独立串行39项 doctest通过。详细证据见验收报告，不能勾选本步。
- [x] Step 2: 使用专用 *_test MySQL/RedisDB15 运行 integration。首轮与重试结果分开记录；遗漏夹具修复后权限组及后续全部套件通过，不称完整脚本一次通过。
- [x] Step 3: 受影响浏览器 smoke 11 项通过（拦截 API）；全权限两个功能域四入口但不要求同时显示；审批读权限双 tab/无数据源；普通用户无工作台菜单。
- [x] Step 4: 一次 fresh-context、只读整分支评审完成，无 Critical；两项 Important 已以回归及摘除式变异修复。git diff --check、两仓库 status、生成物和用户改动复核。
- [x] Step 5: 最终报告列完成项、提交、证据、ruling、未验证/风险；不 push/merge；不足完成契约的任务明示未完成。记录见 specs/navigation-catalog-single-source-acceptance.md。

## 自审

Spec 完整传播/兼容对应 Task1/2，四入口/删除/授权 Task3，导航与功能域文案 Task4，审批可访问性/URL Task5，守护/夹具/文档 Task6，验收 Task7。AccountSwitcher 内嵌，build_application 两参数；审批真实权限共享，单 Action fixture 仅验证鲁棒性。不存在执行期间再补 spec 的步骤或未经实现的接口。
