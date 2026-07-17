# 前端契约调研与 yang 架构第一性原理再审

**日期:** 2026-07-17  
**状态:** 已复核，产品目标已确认，作为新前端与后端 UI 投影设计输入  
**调研对象:** `br/scs-web/`（Quasar 2.16 + Vue 3.4 + Vite，JS，Options API）；对照 `br/scs-api/` 与 yang 生态（`crates/yang-base` + `project/yang-system`）

---

## 1. 产品目标与根本结论

目标不是复刻 scs-web，也不是只做一套 CRUD 后台，而是保住 br 生态最有价值的能力：

> 后端注册一个 Action 后，前端立即拥有一个可展示输入、发起真实调用、查看响应的默认演示页面；标准业务场景可以进一步投影成表格、表单、树等通用页面；复杂场景允许前端自定义页面覆盖。

新前端必须同时满足三层能力：

1. **默认 Action 演示层**：每个可访问 Action 都有零手写页面的默认入口，能查看接口说明、填写参数、调用接口并展示成功、错误、文件或重定向响应。
2. **通用业务渲染层**：当后端提供 Table/View/字段展示元数据时，自动升级为 TableView、FormDialog、TreeView、关联选择器等业务页面。
3. **自定义页面层**：可视化、多步骤流程、打印、地图、SVG 编辑等复杂场景由前端自定义实现；自定义页通过稳定 `view_id` 注册，不与 API 文件路径绑定。

这三层按“自定义页 → 通用业务页 → 默认 Action 演示页”依次解析；高层不存在或加载失败时可以安全降级到下一层。因此“写完接口就有页面”不再依赖所有接口都具备完整 UI schema。

第一性原理边界：

- 后端是**数据语义、Action 能力、权限、租户范围和业务前置条件**的事实源；
- 前端是**组件实现、布局、交互、可访问性和自定义业务体验**的事实源；
- OpenAPI/Action schema 描述“如何调用”，UI schema 描述“建议如何展示”；两者都只携带声明式数据，不下发可执行代码或任意组件文件路径；
- UI 中的隐藏、禁用和二次确认不能替代服务端鉴权、业务校验或 step-up authentication。

审计结论：**yang 的定义内核方向成立，不需要推倒重来，但缺口不只是一层序列化投影。** `ActionSpec`、字段定义、`ViewSpec`、`DefinitionCatalog`、OpenAPI 和 `CompiledTableView` 是正确地基；仍需补齐请求级 UI 投影、字段到控件的映射、关系选择器契约、租户注入、Action 展示语义、自定义页注册和非 JSON 输入模型。

当前产品明确**不需要 WebSocket 能力**。这是新系统的范围决策，不从 scs-web 的 WS 使用状态反推；本阶段不设计、不实现 WS 投影或实时传输。

---

## 2. scs-web 契约清单（字段级实证）

### 2.1 api.json：前端不消费

`br/scs-api/api.json` 是扁平字符串数组（1227 个 API 名），由 `Plugins::generate_api_list` 生成并灌库（`br/scs-api/src/main.rs:56-93`），用于后端自省、swagger 与权限系统。scs-web 全仓没有 `api.json` 引用。它不包含输入、输出或 UI 语义，不应升级为新前端契约载体。

### 2.2 HTTP 调用契约（`br/scs-web/src/boot/api.js`）

- URL：`POST {base}/api/{addon}/{module}/{action}/{addon}.{module}.{action}`（api.js:28，路径尾部重复三段名）；
- 自定义 header：`token`、`org_org`、`model`（api.js:13-23），`model ∈ user|org|admin`；
- 响应包：`{ success, code, message, data, timestamp }`；`$api` 解 `data.data`、`$api_new` 解整个 body，两套并存；
- 错误码流转：普通调用中 `1000` 清 token 并跳登录、`2000` 强制回到 user model；`100000` 的专门登出处理只存在于上传分支；
- 文件通道：`$apiupload`（multipart）、`$apidownload`（blob + content-disposition）、`$apipreview`。

### 2.3 Table schema（表页渲染契约）

后端 `Tables::get_table()`（[br-addon tables.rs:646-658](https://docs.rs/br-addon/0.1.66/src/br_addon/tables.rs.html)），通用 table 组件消费的契约族如下：

```jsonc
{
  "pages": 3,
  "total": 57,
  "data": [/* 行；table/tree 字段可能是 {value,label} */],
  "columns": { "<field>": {/* 字段语义 + 表格展示属性 */} },
  "filter_fields": ["status", "created_at"],
  "search_name": "名称/排序",
  "total_fields": [],
  "btn_all": [],
  "btn_api": [],
  "btn_ids": [],
  "label_field": "name"
}
```

`table.vue:418-429` 直接消费 `columns/data/search_name/btn_*/total/filter_fields`；`total_fields` 和 `label_field` 由其它 table/tree 变体消费，不能把上面的联合契约误认为单个组件全部读取。

column 项混合了字段层（`require/field/mode/title/length/def/show/describe/example`）与渲染层（`name/label/align/sortable/version/dataIndex/ellipsis/tooltip`）。`table.vue:165-253` 的单元格分派覆盖 `table/url/tree/timestamp/switch/file/text/pass/float/object/color/location/polygon/editor/int` 等模式。

服务端组装实例：`br/scs-api/src/addon/admin/auth/table.rs:37-47`。

### 2.4 字段 JSON（br_fields）

通用键为 `require/field/mode/title/length/def/show/describe/example`，另有类型专属键，例如 `Code.dec`、`select.option`、`table.api/where/fields`、`tree.pid_field`、`dict.api/fields`。证据：[br-fields str.rs](https://docs.rs/br-fields/2.2.4/src/br_fields/str.rs.html) 94-108 行。

br_fields 还把部分元数据写入 MySQL 列注释（`mode|require|title|length|def`）。新系统应以代码定义为事实源，不再从数据库注释反解 UI 契约。

### 2.5 按钮 schema（私有 UI 行为语言）

[br-addon action.rs `Btn::json()` 610-632](https://docs.rs/br-addon/0.1.66/src/br_addon/action.rs.html)：

```jsonc
{
  "addon": "admin",
  "api": "admin.auth.edit",
  "title": "编辑",
  "icon": "...",
  "color": "primary|negative|warning|positive",
  "auth": true,
  "public": true,
  "pass": false,
  "btn_type": "form|form_download|form_custom|form_data|api|download|url|path|dialog_custom|form_api_dialog_custom|preview",
  "cnd": [["status", "=", "已启用"]],
  "url": "",
  "path": "admin/auth/xxx",
  "fields": {/* Action params */},
  "tags": ["admin"]
}
```

- `btn_all` 是工具栏操作，`btn_ids` 是多选批量操作，`btn_api` 是行操作；
- `cnd` 支持 `= <> < <= > >= in`，只在浏览器端决定按钮是否显示；
- `btn_type` 把表单、确认调用、下载、预览、导航和自定义弹窗混在同一个字符串枚举中；
- `pass` 会先调用独立密码校验接口，再调用目标 Action，两次请求没有绑定证明，不能作为新系统的安全设计沿用。

OpenAPI 标准词汇不原生表达这些 UI 行为；虽然可以使用 `x-*` 扩展携带任意声明式数据，仍应让 UI schema 独立版本化，避免 API 调用契约和具体渲染器生命周期耦合。

### 2.6 菜单/应用 schema（驱动动态路由）

- `api.api.addon` 返回 addon 信息；
- `api.api.model` 返回 module 与 action 菜单树；
- 前端将 API 三段名同时用作路由名、URL 片段和页面文件路径，并从 `import.meta.glob('pages/**/*.vue')` 中加载组件（`stores/base.js:122-150`、`router/index.js:30-54`）。

这证明“服务端下发能力、前端动态组成导航”有效，也暴露了 API 标识与组件物理路径耦合的问题。

### 2.7 请求小契约

- 表格参数：`{page, limit, order:[[f,desc]], search, where_and, where_or, params}`；
- 关联下拉：带搜索、已选值回填、过滤、分页和主键信息，返回 `[{value,label}]`；
- Dashboard：`{title,data,model,class,icon,desc,api,options}`，由多个 addon 首页解释执行。

---

## 3. 从 br 保留什么、舍弃什么

### 3.1 应保留

- 后端声明能力，前端解释元数据；
- 标准页面无需逐个手写；
- 表格、表单、筛选、关系选择器、按钮和菜单形成统一协议；
- 复杂页面可以脱离通用渲染器单独实现；
- 菜单和操作只展示当前用户可访问的能力。

### 3.2 不应照搬

- API 名、路由名、URL 和组件文件路径四位一体；
- 数据类型、数据库字段和 UI 控件使用同一个 `mode` 枚举；
- `btn_type` 同时承担调用方式、布局位置、导航和组件加载；
- 客户端 `cnd` 被误认为业务前置条件；
- 独立密码校验后直接执行敏感操作；
- 前端依赖错误码 reload 完成身份/租户切换；
- 多套 table 组件和多处按钮派发复制。

### 3.3 页面数量只能说明趋势

当前仓库共有 416 个 `src/pages/**/*.vue` 文件。按可复现的静态搜索：约 245 个文件包含某种 table 组件标签，其中约 186 个不超过 30 行。不同统计口径会把测试页、addon 首页、带少量定制逻辑的 table 页划到不同类别，因此不再使用“244 schema 页 vs 163 手工页”或 60/40 作为验收线。

正确的验收维度是能力覆盖：注册 Action 是否自动出现、标准 Table/View 是否自动升级、关系选择器是否可用、自定义页是否可覆盖、权限与租户是否始终生效。

### 3.4 WebSocket 范围

scs-web 的 `ws.js` 已注册 `$wss/$wss_subscribe`，但连接初始化被注释，通知组件仍保留订阅调用，属于未完成或休眠能力，不能称为“整文件死代码”。新系统本阶段不需要 WS，因此明确排除；未来若业务出现实时通知、协作或长任务进度，再作为独立传输能力评估。

---

## 4. yang 架构审计（目标 → 现状 → 判定）

| 目标能力 | yang 现状 | 判定 |
|---|---|---|
| 每个 Action 自动生成默认演示 | `ActionSpec` 已有 method/path/operation_id/params/input_schema/output_schema/权限，Catalog 可生成 OpenAPI | ⚠️ 原料具备，缺请求级目录端点与前端 ActionDemo |
| 菜单/应用 schema | `DefinitionCatalog` 有 Addon/Module/Action 元数据 | ⚠️ 缺用户/租户过滤后的菜单投影 |
| 基础数据 schema | CRUD 已注册 `/schema`，`TableAction` 返回按角色过滤的 input/output JSON Schema | ✅ 已有，不等于完整 UI schema |
| Table/Form/Tree UI schema | 字段展示元数据 + `CompiledTableView` 提供表、字段、Action 引用 | ⚠️ 缺列、筛选、布局和控件映射投影 |
| Action 展示行为 | `ViewSpec::action(ActionRef)` 只有引用 | ❌ 缺展示位置、交互方式、确认和自定义 view 引用 |
| 字段到控件映射 | yang `FieldKind` 约 10 类，br_fields `FieldMode` 约 36 类 | ⚠️ 不能一一映射，需要独立 WidgetHint 和降级规则 |
| 关联 `{value,label}` 选择器 | `RelationLoader`、`table_select()`、字段 `display/select` 提供查询基础 | ⚠️ 缺统一 options 请求/响应 DTO |
| 下载/预览/重定向 | `ResponseBody::download/preview/redirect` | ✅ 已对齐 |
| multipart 上传 | transport-axum 把 body 强制解析为 JSON，`Request.body` 是 `serde_json::Value` | ❌ 需设计 Action media type 与文件生命周期，不只是加 extractor |
| 认证与权限 | `Authorization`、`TokenAuthMiddleware`、Action/Module permission 已有 | ✅ 认证授权基础成立 |
| 请求租户上下文 | `TenantContext` 和 table fail-closed 已有，但 transport/auth 不负责注入 | ⚠️ 缺可信 tenant resolver middleware |
| 错误处理 | 真实 HTTP 状态码 + 结构化 code | ✅ 已升级 |
| step-up authentication | 无绑定目标 Action/资源的短期证明 | ❌ 敏感操作真实缺口 |
| 自定义页面 | 无稳定 view registry 契约 | ❌ 需前端白名单注册与安全降级 |
| WS 实时通道 | 无 | ➖ 当前范围外 |
| 字段事实源 | `fields!` → TableDefinition/JSON Schema/additive schema sync | ✅ 方向成立，但 UI 控件语义仍需补充 |

`admin-metadata` 当前是独立的轻量展示注册表，没有接入 `AppBuilder` 或 Catalog。它可以作为 UI 投影的输入之一，但不应直接承担请求级投影端点、权限过滤和完整 Table/Form schema。建议在应用层新增 UI projector，组合 Catalog、CompiledView、TableDefinition 与可选 AdminMetadata。

---

## 5. 目标前端架构

### 5.1 契约层

只保留两个职责清晰的契约来源：

- **API contract**：由 `ActionSpec`/OpenAPI 提供 method、path、参数来源、输入输出 JSON Schema、响应类型、权限说明；
- **UI contract**：由请求级 UI projector 提供菜单、View、字段控件提示、Action 展示方式、自定义 `view_id` 和安全要求。

UI projector 必须按当前用户、权限、租户和应用配置生成结果。前端过滤只改善体验，服务端 Action 派发仍独立执行同样的授权与业务校验。

所有契约都带稳定 `schema_version`，未知枚举值必须有安全降级；可使用 ETag/版本号缓存，但切换身份或租户后必须重新获取。

### 5.2 默认 Action 演示层

每个有权访问的 Action 自动投影为 `ActionDemoSchema`：

```jsonc
{
  "operation_id": "org.user.add",
  "title": "新增用户",
  "description": "...",
  "method": "POST",
  "path": "/api/org/user",
  "params": [/* body/query/path/header 参数 */],
  "input_schema": {/* JSON Schema */},
  "output_schema": {/* JSON Schema */},
  "response_kind": "json|download|preview|redirect",
  "requires_auth": true
}
```

前端 `ActionDemo` 根据 schema 生成最小可用表单，调用统一 API client，并展示请求、响应、错误、耗时和 request-id。它是开发演示和未知 Action 的最终 fallback，不承诺等同于业务成品页面。

### 5.3 通用业务渲染层

当 Action 属于 Table/View 时，UI projector 追加：

- `TableViewSchema`：列、默认排序、搜索字段、过滤字段、分页、关系展示；
- `FormSchema`：字段顺序、WidgetHint、校验提示、只读/隐藏规则；
- `ActionPresentation`：工具栏/批量/行操作位置，`form/invoke/download/preview/navigate/custom` 展示方式；
- `RelationOptionsSchema`：稳定的 `search/selected/filter/page/limit` 输入与 `{value,label}` 输出；
- `AvailabilityHint`：用于隐藏/禁用和原因展示，但不替代服务端前置条件。

WidgetHint 与存储类型分离。例如字符串字段可以投影为 text/password/email/url/color/editor；无法识别时降级为 JSON/text 输入，而不是拒绝渲染整个页面。

### 5.4 自定义页面层

自定义页面只通过稳定、白名单化的 `view_id` 解析：

```ts
const customViews = {
  'dms.task.flow': () => import('./views/dms/TaskFlow.vue'),
  'goods.map.editor': () => import('./views/goods/MapEditor.vue'),
}
```

后端只能返回 `view_id` 和声明式 props schema，不能返回前端物理路径、动态 import 表达式或脚本。未注册 `view_id` 时回退到通用业务页或 ActionDemo，并给出可诊断提示。

### 5.5 安全不变量

- 菜单、字段和 Action 投影按当前用户与租户过滤；
- 未显示的 Action 仍必须在服务端拒绝未授权调用；
- `AvailabilityHint` 只控制界面，不是业务校验；
- 敏感操作使用服务端 step-up challenge/proof，并绑定用户、Action、资源和短过期时间；
- 自定义 view 只能从前端静态注册表加载；
- 文件上传必须限制大小、数量、类型、临时文件生命周期和流式处理策略。

---

## 6. 技术与工程建议

- **技术栈**：Vue 3 + TypeScript + 单组件库；不带入 Quasar/Arco 双轨和未实际使用的 Vuetify 依赖；
- **统一运行时**：一个 API client、一个 ActionDemo、一个 TableView、一个 FormDialog、一个 TreeView；
- **类型生成**：OpenAPI 生成调用层类型；UI schema 自身维护版本化 TS 类型和运行时校验；
- **错误路由**：以 HTTP 401/403/409/422/5xx 为主，结构化 code 处理细分业务；不靠整页 reload 切换身份；
- **租户切换**：由可信服务端上下文解析，切换后清理请求缓存并重新获取目录/UI schema；
- **可观测性**：默认演示页展示 request-id，便于前后端串联诊断；
- **测试**：对 schema 版本、未知枚举降级、权限过滤、租户切换、step-up、自定义 view fallback 建契约测试。

可迁移的是交互模式和业务知识，不默认“整模块搬迁”。i18n 词条、图表配置、SVG/地图/统计组件需要逐项核对依赖、数据契约、许可和可测试性后再复用。

---

## 7. 行动顺序与验收标准

### 阶段 1：默认 Action 演示闭环

1. 定义并版本化请求级 `ActionDemoSchema/UiCatalog`；
2. 从 Catalog/OpenAPI 投影当前用户可访问的 Action；
3. 前端实现 Action 列表、自动参数表单、统一调用和响应查看；
4. JSON、download、preview、redirect 均有安全降级展示。

验收：新增一个普通 JSON Action 后，不写前端页面即可在目录中发现、填写参数、真实调用并查看结果。

### 阶段 2：org 模块通用业务页

1. 实现可信 tenant resolver middleware；
2. 投影 TableView/FormSchema/ActionPresentation；
3. 固化关系 options DTO；
4. 用 org 模块验证列表、新增、编辑、删除、筛选、关联选择和权限过滤。

验收：标准 Table/View 自动升级为业务页面；切换用户或租户后菜单、字段、数据和操作一致变化，直接越权调用仍被服务端拒绝。

### 阶段 3：自定义页面覆盖

1. 建立前端 `view_id` 白名单注册表；
2. 支持自定义页覆盖与通用页/ActionDemo fallback；
3. 选择一个可视化或多步骤页面验证边界。

验收：自定义页面不要求 API 名与文件路径一致；删除注册项后仍能安全降级，不出现任意动态代码加载。

### 阶段 4：非 JSON 输入与工程完善

1. 以真实上传 Action 为输入设计 multipart/file contract；
2. 补 schema 版本兼容、缓存、运行时校验和契约测试；
3. 按业务优先级迁移复杂页面，不以历史页面数量比例作为目标。

本阶段明确不包含 WebSocket。

---

## 附录 A：scs-web 与 br 后端耦合 Top 5

1. API 三段名 = 路由名 = URL = 文件路径；
2. 多个 table 变体和多处按钮派发重复；
3. `btn_type + cnd + pass` 混合展示、导航与安全语义；
4. 三 header 鉴权、model 三态和错误码 reload；
5. br_fields 字段 JSON、`{value,label}`、where 三元组与数据库注释反解 schema。

## 附录 B：证据文件索引

- 前端契约实现：`br/scs-web/src/boot/api.js`、`src/boot/ws.js`、`src/components/table/table.vue`、`src/components/form/form.vue`、`src/stores/base.js`、`src/router/index.js`、`src/layouts/AddonLayout.vue`；
- 后端 schema 组装：`br/scs-api/src/addon/admin/auth/table.rs`、`br/scs-api/src/main.rs:56-93`；
- br 库公开源码：[br-addon tables.rs](https://docs.rs/br-addon/0.1.66/src/br_addon/tables.rs.html)、[br-addon action.rs](https://docs.rs/br-addon/0.1.66/src/br_addon/action.rs.html)、[br-fields str.rs](https://docs.rs/br-fields/2.2.4/src/br_fields/str.rs.html)；
- yang 侧判定依据：`crates/yang-base/src/definition/`、`crates/yang-base/src/action/builtin/table.rs`、`crates/yang-base/src/action/auth.rs`、`crates/yang-base/src/table/relation_loader.rs`、`crates/yang-base/src/transport/axum.rs`、`crates/yang-base/src/action/response.rs`。
