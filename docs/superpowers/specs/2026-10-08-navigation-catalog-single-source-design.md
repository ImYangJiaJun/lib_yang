# 侧边栏导航单一事实源设计（Module 级入口）

**日期**：2026-10-08
**状态**：设计已确认，待实现
**影响面**：`crates/yang-base`（UI 契约）、`project/yang-system`（后端声明 + 前端）
**契约版本**：UI schema `2.3` → `2.4`

> **行号时效**：文中 `file:line` 于 2026-10-08 现场核对。该仓库存在多会话并行编辑，行号会漂移（例如 `access/grants/mod.rs` 的 presentation 在核对期间即从 74 行移至 64 行）。**以符号名与模块名为准，行号仅作定位辅助。**

## 1. 问题陈述

两个可复现的症状：

1. 侧边栏出现**两个「权限管理」**条目。
2. 侧边栏出现**测试用的「便签」模块**。

## 2. 根因：侧边栏由两个事实源拼成

侧边栏实际是两套独立机制生成的条目，拼在同一个 `<nav>` 里：

| 来源 | 生成方式 | 产出 |
|---|---|---|
| **A. 硬编码 JSX** | `frontend/src/shell/AppLayout.tsx:269-377` 手写三组 | 个人（账号设置）/ 飞书集成（数据源、审批派发、派发记录）/ 权限管理（权限工作台） |
| **B. Catalog 动态投影** | `buildNavigationPages()` → `buildAccountModulePages()` → `frontend/src/engine/catalog/module-pages.ts:46` | 每个注册了 presentation 的 Module 一个条目，按 `identity` 分组 |

**症状 1 的构成**：A 里硬编码的分组标题「权限管理」（内含条目「权限工作台」→ `/access/workspace`）+ B 里 `access.grants` Module 自己投影出的条目「权限管理」（`src/addon/access/grants/mod.rs:64`，落地 `/m/access.grants`）。同一功能两个入口、两条代码路径在造菜单。

**症状 2 的构成**：演示 addon `demo.notes` 无条件装配（`src/app.rs:112`），注册了真实 Module（`src/addon/demo/notes/mod.rs:45`），于是被 B 自动投影成条目。

补充事实：前端另有一条兜底规则——**未被任何 Module 认领的 TableView 会合成成模块页，挂进「工作台」组**（`frontend/src/shell/navigation.ts:16-38` + `module-pages.ts:163`）。因此「把 demo 的 presentation 摘掉」不足以让它消失，只会把它从「个人账户」挪到「工作台」。

## 3. 第一性原理

侧边栏只有一个合法语义：

> **当前身份下、对所有已授权功能的、导航入口清单。有功能（Catalog）→ 有权限（投影授权）→ 成为入口（导航）。**

推论出的铁律：**「有什么 nav 项」应完全由后端 Catalog 决定，前端只负责渲染。** 门的开闭由身份投影与权限剪枝控制；后端不给这个 Module/Action，前端就不该长出这个入口。

现状在反向做：Catalog 投影方向正确，但遇到「专用控制台页」（飞书数据源、权限工作台、账号设置）后无法表达——`ModulePresentationSpec` 只会把 Module 投影成 `/m/{id}` 的通用表格页。于是 AppLayout 被迫手写 `NavLink` + 手写门控布尔来补，导航从此有两个事实源。

## 4. 关键发现：服务端已有 module 级权限剪枝

`crates/yang-base/src/definition/builder/registry.rs:241-249`：

```rust
let primary_action = module.primary_action
    .and_then(|handle| self.handlers.get(handle.slot()))
    .filter(|runtime| runtime.policy.allows(context))
    .map(|runtime| runtime.ui_schema.operation_id.clone());
if module.primary_action.is_some() && primary_action.is_none() {
    return None;                      // 主 Action 无权 → 整个 module 出局
}
// ...
if primary_action.is_none() && allowed_actions.is_empty() && views.is_empty() {
    return None;                      // 全空 → 剪掉
}
```

**结论**：AppLayout 里那四个 `canReadXxx` 布尔是在重造服务端已有的机制。方案落地后连同硬编码区块一起删除，不是搬家。

**副产物 bug**：`canReadAccessGroups`（`AppLayout.tsx:233-237`）查的是 `access.groups.list_groups`，而按 `src/addon/access/groups/mod.rs:83-87` 的既有语义，9 个组 Action 一律 authenticated-only、不声明权限键——该 operation 对**任何已登录用户都存在**，这个布尔恒为 `true`，从未起到门控作用。删除即修复。

## 5. 决策

| # | 决策 | 主要取舍 |
|---|---|---|
| 1 | `ModulePresentationSpec.app_route` —— 入口落点成为 Module 的声明属性 | 一个 Module 一个入口；换来零新概念 |
| 2 | `identity` 升格为功能域（`user` / `admin`） | 复用现成机制，账号切换器从死代码变真功能；管理员登录多一步选择 |
| 3 | 删除 demo addon | 仓库最干净；丢掉 P7 活样板与那层守护 |
| 4 | 审批两页合并为 tabs 控制台 | `Module == 入口` 恒成立；前端多约 40 行，侧边栏少一个入口 |

### 决策 2 的背景（为什么不能只加新 identity 了事）

`identity` 现在**身兼两职**：侧边栏分组标题（`groupNavigationPages` 用 `identity.title`）+ 角色切换单元（`AccountSwitcher`、`SelectIdentityPage`、`resolveIdentityLanding`）。今天它只有**一个值**（`user`/"个人账户"，三个 Module 全用它，见 `src/addon/mod.rs:13-15`），矛盾未暴露。

若照旧直接新增 `feishu_identity()` / `access_identity()`，它们会立刻变成「可切换的角色」，`SelectIdentityPage` 将询问「选择本次使用的角色：个人账户 / 飞书集成 / 权限管理」——语义上错误。因此本设计把 identity 明确**升格为功能域**：它既是分组，也是角色镜头，两职统一。

### 决策 4 的背景（`feishu.approval` 是一对多）

`feishu.approval` 是**单个** Module（`src/addon/feishu/approval/mod.rs:93`），却对应两个前端页面（`/feishu/approval/configs`、`/feishu/approval/requests`），且两者可见性**独立**（`AppLayout.tsx:239-251` 的注释明确说明一个身份可能只有 record read 而无 config read）。这与决策 1 的「一个 Module 一个入口」冲突。

拆 Module 这条路已堵死：`operation_id = format!("{module.name}.{action.name}")`（`src/addon/access/domain/permission_catalog.rs:81`），拆分会改掉 `feishu.approval.list_configs` 等权限键，属破坏性变更。

采用：**合并为一个带 tab 的控制台**。后端该 Module **不设 `primary_action`**，依赖 `registry.rs:269` 的既有兜底（任一 action 可见即入目录）；前端在 tab 内按权限各自判权。

## 6. 目标形态

**个人账户**（identity `user`，order 10）

| 条目 | 落点 | 门控（`primary_action`） |
|---|---|---|
| 账号设置 | `/account` | `account.user.me` |

**系统管理**（identity `admin`，order 20，新增）

| 条目 | 落点 | 门控 |
|---|---|---|
| 权限管理 | `/access/workspace` | `access.grants.list_user_grants` |
| 飞书数据源 | `/feishu/datasources` | `feishu.datasource.list_datasources` |
| 审批派发 | `/feishu/approval` | **不设** → 任一 approval action 可见即入目录 |

对照今天：删 3 个硬编码区块 + 4 个 `canReadXxx` 布尔；「权限管理」两处合一；账号入口两处合一（硬编码「账号设置」`/account` 与动态「用户中心」`/m/account.user`，两者功能高度重叠，`AccountSettingsPage` 严格更丰富）；便签消失。

## 7. 设计

### 7.1 契约层（`crates/yang-base`）

`crates/yang-base/src/definition/ui/module.rs`：

```rust
pub struct ModulePresentationSpec {
    // ...现有字段
    /// 入口落点的前端路由；缺省时前端落到 `/m/{module_id}` 通用页。
    pub app_route: Option<String>,
}

impl ModulePresentationSpec {
    #[must_use]
    pub fn app_route(mut self, route: impl Into<String>) -> Self {
        self.app_route = Some(route.into());
        self
    }
}
```

`ModulePresentationSchema` 增同名字段 `pub app_route: Option<String>`，随投影带出（`registry.rs:272` 处加一行 `app_route: module.app_route.clone()`）。

**构建期校验**（与现有展示文本校验同处：`crates/yang-base/src/definition/builder/compile.rs:443` 的 `validate_presentation_text`，返回 `BuildError::InvalidReference`）：

- 非空
- 以 `/` 开头
- 不以 `//` 开头（挡掉协议相对 URL）
- 长度上限（防止异常长串进目录）

实现上建议新增同族的 `validate_app_route`，而非扩展现有 `validate_presentation_text`——后者被 title / description 复用，签名（`kind, value, max_chars`）不含路径语义。

**只做格式校验，不校验路由存在性**——SPA 路由表是前端的事实，后端不该知道。

**剪枝逻辑零改动**：`registry.rs:241-249` 的规则原样生效，这正是控制台 Module 要的门控。`revision` 哈希整个 modules 数组（`ui/catalog.rs:69-86`），会自动变化，前端缓存失效正确。

**版本**：`UI_SCHEMA_VERSION`（`ui/mod.rs:42`）`2.3` → `2.4`；前端 `SUPPORTED_UI_SCHEMA_VERSIONS`（`frontend/src/engine/contracts/ui-catalog.ts:3`）加 `"2.4"`。两仓库（`crates/` 与 `project/yang-system/frontend`）必须同批落地，否则 `parseUiCatalog` 直接抛 `ContractError`。

### 7.2 声明层（`project/yang-system` 后端）

`src/addon/mod.rs` 新增：

```rust
pub(crate) fn admin_identity() -> AccountIdentitySpec {
    AccountIdentitySpec::new("admin", "系统管理", "admin_panel_settings").order(20)
}
```

各 Module 调整：

| 文件 | 改动 |
|---|---|
| `src/addon/account/user/mod.rs:70` | identity 不变；加 `.app_route("/account")`；标题定为「账号设置」（与落地页一致，取代动态投影出的「用户中心」） |
| `src/addon/access/grants/mod.rs:64` | identity 换 `admin_identity()`；加 `.app_route("/access/workspace")` |
| `src/addon/feishu/datasource/mod.rs` | **新增** `.presentation(...)`：admin identity、`/feishu/datasources`、`primary_action(feishu.datasource.list_datasources)` |
| `src/addon/feishu/approval/mod.rs` | **新增** `.presentation(...)`：admin identity、`/feishu/approval`、**不设 `primary_action`** |
| `src/addon/feishu/option/mod.rs` | 不动（无 presentation，本就无导航入口） |
| `src/addon/access/groups/mod.rs` | 不动（无 presentation；其 Action 仍是工作台的能力来源） |

### 7.3 前端

| 文件 | 改动 |
|---|---|
| `engine/contracts/ui-catalog.ts` | `modulePresentationSchema` 加 `app_route: z.string().startsWith("/").nullable().optional()`；SUPPORTED 加 `"2.4"` |
| `engine/catalog/module-pages.ts` | `ModulePageDefinition` 加 `link?: string`；`buildAccountModulePages` 映射 `link: module.app_route ?? undefined` |
| `shell/AppLayout.tsx` | **净删除**：4 个 `canReadXxx` 布尔、4 个硬编码区块、随之失效的 imports。动态区唯一改动：`to={page.link ?? \`/m/${page.id}\`}` |
| `shell/routes.tsx` | `/feishu/approval/configs` 与 `/requests` 合并为 `/feishu/approval`，旧两条保留为**重定向**（沿用 `access/groups` → 工作台的兼容先例） |
| 新增 `features/feishu/views/ApprovalConsolePage.tsx` | tab 容器：读 `?tab=`，按权限决定可见 tab 与默认 tab；两个既有页面组件原样复用为 tab 内容 |
| `features/registry.ts` | 移除 `DemoItemInsight` 的 custom view 注册 |

身份过滤仍由 `AppLayout.tsx:215-220` 完成，语义不变。

### 7.4 删除项

- `src/addon/demo/**` 整个目录
- `src/addon/mod.rs` 的 `pub(crate) mod demo;`
- `src/app.rs`：`demo::build_addon` 装配、`ActionLogMiddleware` 挂载、P7 冒烟测试
- `frontend/src/features/demo/views/DemoItemInsight.tsx`

## 8. 连带影响

### 8.1 权限目录

`demo.notes.read` / `demo.notes.write` 从目录消失 → 已授出的直授与组成员条目变为孤儿。现有 `src/addon/access/domain/groups/resolution.rs` 的孤儿语义会将其标出，不新增机制。

### 8.2 测试

需更新：

- `src/addon/access/domain/sensitive_permissions.rs:106` —— `"demo.notes.read"` 是 `#[cfg(test)]` 内的样本串，替换
- `src/addon/access/domain/groups/resolution.rs:107/136/140` —— 三处样本串，替换
- `src/app.rs` —— P7 冒烟测试删除
- `frontend/tests/shell/navigation.test.ts`、`tests/engine/catalog/module-pages.test.ts`
- `frontend/tests/shell/routes.test.tsx` —— 逐条钉住 lazy 路由约定，合并 approval 路由必须同步
- `frontend/tests/features/feishu/*` —— approval 页面路径变化

建议新增守护测试（本次最该留下的东西）：

- **后端**：每个注册了 presentation 的 Module 必须能落到某处（`app_route` 或 views），否则用户点进去是空页
- **前端**：`侧边栏条目数 == catalog modules 数` —— 钉死「第二事实源不得复活」
- **后端**：`feishu.approval` 等控制台 Module 的 `app_route` 稳定（防止静默改路由）

### 8.3 文档

`docs/guides/ADDON_ONBOARDING.md:100` 附近示范 `user_identity()`，需补 `admin_identity()` 与 `app_route` 的选用规则。

## 9. 风险与代价

1. **管理员登录多一步**：同时拥有两域 Module 的用户会被 `resolveIdentityLanding` 送到 `SelectIdentityPage`（已存 identity 则直进）。这是决策 2 接受的成本。
2. **`identity` 是纯前端镜头，不授予任何权限**。过滤发生在 `AppLayout.tsx:215-220` 的 `.filter(page => page.identity === identity)`；真门控**始终**是服务端剪枝。此条必须写死，否则后人会误以为切身份能解锁。
3. **`/m/account.user` 与 `/m/access.grants` 通用页仍在**，只是不再有导航入口，深链接可达。有意保留——它们是 Module 的降级视图。
4. **格式校验挡不住「路由写错」**：`app_route` 合规但前端无此路由时，用户点进去是空白。可选补一条前端契约测试（拿后端 app_route 清单比对 `routes.tsx`）。
5. **丢掉 P7 守护**：删除 demo 后，「新 addon 全流程接入」这条架构守护消失。与本次改动无直接关系；若在意可另开更轻的 spec 级测试。

## 10. 验收标准

1. `AppLayout` 里不再手写任何导航条目——侧边栏条目集合完全由 catalog 决定
2. 全权限账号：4 个条目分两组（个人账户 1 / 系统管理 3），无重复、无便签
3. 只有 `feishu.approval.list_requests` 的账号：看不到「飞书数据源」，看得到「审批派发」，进去只有「记录」tab
4. 无 grants 权限的账号：系统管理组只剩飞书相关条目；个人账户组只剩账号设置
5. `python scripts/run_ci.py quick` 绿；前端 `pnpm test` 与 `pnpm run check` 绿

## 11. 非目标

- 不改 `identity` 的后端授权语义（它从未参与授权判定，本次也不引入）
- 不引入「一个 Module 多入口」概念（决策 4 通过合并页面规避）
- 不重构通用表格页 `/m/{id}` 的渲染
- 不处理 demo 之外的其它 Module 归位争议

## 12. 实施顺序建议

跨仓库契约变更是硬依赖，顺序不能反：

1. `crates/yang-base`：加 `app_route` + 校验 + schema 版本 → 跑 `run_ci.py quick`
2. `frontend`：契约解析 + `link` 字段 + SUPPORTED 加 `"2.4"`
3. `project/yang-system` 后端：`admin_identity()` + 四个 Module 的 presentation + 删除 demo
4. `frontend`：AppLayout 净删除 + approval 控制台合并 + 路由重定向
5. 测试与文档补齐
