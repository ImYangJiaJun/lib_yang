# YANG 基础库、基础系统与前端统一完善路线图

> 适用仓库：`D:\code\lib_yang` 与独立 Git 仓库 `project/yang-system`
> 基线评估日期：2026-07-26
> 执行原则：一个可验证改进点对应一个 Git 提交；不跨仓库混合提交；不自动推送

## 一、为什么不能按三份文档从上到下直接实施

三份评估分别从基础库、基础系统和前端观察问题，但真实依赖方向是：

```text
基础库定义/认证/数据保护
        ↓
yang-system 领域装配与生产策略
        ↓
UI Catalog / HTTP 契约
        ↓
frontend 状态、页面与部署
```

如果按文档编号机械推进，会出现四类错误：

1. 前端先删除启发式，但后端 Catalog 尚未表达稳定语义；
2. 系统先接入 Token 撤销，但基础库水位线遇到损坏数据时仍会失败开放；
3. 系统直接加入 Redis 授权版本，却没有解决数据库提交与缓存传播之间的原子性；
4. 大型重构先行，而依赖漏洞、CI 覆盖和生产 Schema 等发布阻断项仍未关闭。

因此，实施顺序必须是：

`基线与门禁 → 安全原语 → P0 系统不变量 → 生产共同基线 → 显式前后端契约 → 可维护性重构 → 平台治理`

## 二、再次深析后的关键判断

### 2.1 基础库认证能力可以复用，但不能直接等同于授权新鲜度

基础库已有：

- JWT access/refresh token；
- `jti` 黑名单；
- subject 签发时间水位线；
- `verify_token_checked()` 将黑名单和水位线合并为一次 Redis pipeline；
- Redis 不可用时返回错误，而不是继续信任 Token。

但仍有两个边界：

- 水位线值无法解析时，当前实现记录 warning 后视为“没有水位线”，属于安全状态损坏后的失败开放；
- subject 水位线会同时撤销 access 和 refresh token，不能直接满足“旧 access 立即失效，但 refresh 仍可重新读取最新授权”的产品语义。

结论：

1. 先在基础库把损坏撤销状态改成失败关闭；
2. `yang-system` 的授权新鲜度采用持久化单调 `authz_version`，而不是把 subject watermark 当作完整方案；
3. 数据库授权事实和版本递增必须位于同一事务；
4. Redis 只做加速和传播，不做唯一事实源；
5. Token 中携带签发版本，请求期比较当前版本；
6. 敏感写在版本存储不可用时失败关闭。

### 2.2 租户隔离要从“主路径安全”升级为“旁路可枚举”

`TableQuery` 自动 tenant scope 是正确主路径，但系统还存在：

- 领域 raw sqlx；
- 显式事务；
- Join/锁查询；
- RelationLoader；
- 未来批量导入、后台任务和运维脚本。

因此不能只增加一个跨租户 E2E 就宣布完成。必须先建立路径清单和架构门禁，再为每种旁路增加双租户测试；发现旁路后，修复应单独提交，不能和测试清单混成一笔。

### 2.3 首个管理员初始化是独立 P0，可早于授权版本落地

当前 bootstrap 已经满足：

- 必须登录；
- 数据库约束保证只能成功一次；
- 并发重复初始化会失败。

缺失的是运维信任根。最小完整方案是：

- 运维生成高熵一次性 secret；
- 配置只保存不可逆摘要；
- 请求提交原始 secret，服务端常量时间比较；
- 初始化事实与唯一约束仍由数据库原子保证；
- 成功后重放永久失败；
- secret 不进入日志、Debug 或普通响应。

该项可以独立完成，不依赖授权版本；成功后用户重新登录取得新授权，避免把尚未完成的 refresh 语义强行耦合进本提交。

### 2.4 前端技术栈不变，先修发布阻断再改架构

当前 Quasar 2.21.4 命中已修复的中危公告。它与 Catalog 重构无关，应先独立升级并重新执行：

- frozen install；
- production dependency audit；
- lint/typecheck/unit/build；
- Playwright E2E。

随后把 dependency audit 固化为独立门禁。升级和新增门禁是两个不同改进点，使用两个提交。

### 2.5 UI Catalog 重构必须拆成三个提交层

消除 operation id/`id` 字段启发式不能由前端单方面完成。正确顺序是：

1. 根仓库：基础库定义稳定 module/action presentation 语义并构建期校验；
2. 嵌套仓库 Rust：yang-system 为现有模块声明语义；
3. 嵌套仓库前端：消费显式语义并删除启发式。

任何过渡兼容只能存在于明确的相邻 schema version 窗口，不能保留双 Catalog 或双解释运行链。

## 三、一点一提交协议

### 3.1 改进点定义

一个“点”必须同时满足：

- 只有一个主不变量或一个发布风险；
- 能用一组明确命令独立验证；
- 可以单独回滚；
- 提交信息能完整描述其行为；
- 不依赖同一提交中的第二个无关改动。

以下情况必须拆分：

- 基础库 API 与系统消费方；
- 依赖升级与新增 CI 门禁；
- 契约生产方与前端消费方；
- 回归测试清单与测试发现后的行为修复；
- 性能基准框架与具体性能优化；
- 数据库 Schema/迁移与后续 UI 重构。

### 3.2 每个点的固定流程

1. **边界检查**
   - 记录两个仓库 HEAD 和 status；
   - 保留用户原有修改；
   - 确定唯一目标仓库。
2. **结构取证**
   - 结构问题先用 CodeGraph 查看定义、调用链和影响面；
   - 配置、文档和字符串再使用原生文本工具。
3. **失败证据**
   - 行为变更先增加最小失败测试；
   - 纯文档、机械版本升级和无行为重命名可跳过红灯，但必须说明。
4. **最小实现**
   - 不顺手清理相邻问题；
   - 不引入兼容运行链；
   - 不扩大公共 API。
5. **风险匹配验证**
   - 运行聚焦测试；
   - 再运行该类改动的质量门禁。
6. **提交前复核**
   - `git diff --check`；
   - 核对 staged 文件白名单；
   - 确认用户文件未被暂存。
7. **独立提交**
   - 使用 Conventional Commit；
   - 提交后读取 `git show --stat -1`；
   - 不 push。

行为测试与实现属于同一个可回滚语义单元，不提交刻意保持红灯的中间状态。只有“路径清单、契约矩阵、基准框架”本身就是独立产物时，才使用纯测试或基准提交。

### 3.3 验证矩阵

| 改动类型 | 聚焦验证 | 提交前门禁 |
|---|---|---|
| 文档/ADR | 链接、章节、占位、尾随空格 | `git diff --check` |
| 前端依赖 | frozen install、audit | `python scripts/run_ci.py full` + `pnpm --dir frontend e2e` |
| 前端逻辑 | Vitest/组件测试 | `pnpm --dir frontend check`；高风险旅程加 E2E |
| Rust 内部逻辑 | 对应 package/test | `python scripts/run_ci.py quick` |
| Rust 公共 API/feature | 单元、doc、feature 测试 | 根仓库 `python scripts/run_ci.py full` |
| 数据库/Redis 行为 | 真实依赖集成测试 | 嵌套仓库 `python scripts/run_ci.py full` + `integration` |
| Schema/迁移 | plan/apply/validate、中断重跑 | full + integration + 独立测试库 |
| 性能 | 固定工作负载相对基线 | 重复采样；稳定回归大于 3% 阻断 |

### 3.4 两个 Git 边界

| 仓库 | 负责内容 | 禁止事项 |
|---|---|---|
| `D:\code\lib_yang` | 基础库、根 CI、性能基线、跨项目 ADR | 不把 `project/` 加入根提交 |
| `project/yang-system` | 系统 Rust、前端、嵌套 CI、系统文档 | 不暂存用户现有 `Cargo.lock`，除非某个改进点明确需要且先证明差异来源 |

跨仓库能力按“基础库生产者提交 → 根门禁通过 → 系统消费者提交 → 嵌套门禁通过”的顺序执行。消费者提交应增加 `Foundation-Commit: <sha>` trailer，记录所依赖的基础库提交；删除旧 API 必须等消费者迁移完成。该短迁移窗口不得演化为双 Registry、双 Catalog 或双运行时。

## 四、依赖有序实施队列

### 阶段 A：基线、发布阻断与质量护栏

| ID | 仓库 | 独立改进点 | 验收与提交边界 |
|---|---|---|---|
| A-00 | 两个 | 三份评估文档基线 | 三个文档各自提交，不混入实现 |
| A-01 | 根 | 本统一路线图 | 文档检查后独立提交 |
| A-02 | 嵌套 | Quasar 升级到已修复版本 | audit 不再命中 GHSA；full/E2E 通过 |
| A-03 | 嵌套 | production dependency audit 进入 CI | 人工引入已知漏洞 fixture/脚本自检；full 通过 |
| A-04 | 根 | full 门禁覆盖 derive/migrate/pcg | 额外 crate test + Clippy 纳入脚本与 workflow 契约 |
| A-05 | 根 | Rust advisory/license/source 门禁 | `cargo deny` 或等价方案独立配置并通过 |
| A-06 | 根 | 声明并验证统一 MSRV | workspace manifest、文档与 CI 使用同一 Rust 下限 |

阶段退出条件：

- 当前已知前端漏洞关闭；
- 前端和 Rust 供应链检查成为可执行门禁；
- 每个 workspace 成员都有明确 full gate；
- workspace 的最低 Rust 版本不会只靠开发机最新 toolchain 偶然通过；
- 后续大改动不会建立在缺失质量护栏的基础上。

### 阶段 B：安全原语与生产共同前置

| ID | 仓库 | 独立改进点 | 验收与提交边界 |
|---|---|---|---|
| B-01 | 根 | 固定运行时性能 shadow 基线 | 固定工作负载、指标格式和重复采样；只采集，不立即阻断 |
| B-02 | 根 | Token 水位线损坏时失败关闭 | 先有失败测试；损坏值返回结构化错误，不放行 Token |
| B-03 | 嵌套 | Tools 所有退出路径统一关闭 | build/schema/bind/serve 失败与正常退出均恰好 close |
| B-04 | 嵌套 | 生产 Schema 默认 validate | apply/off 必须显式配置；配置和测试独立提交 |
| B-05 | 嵌套 | Schema apply 非生产保护 | 生产标识下拒绝 apply；多实例/失败重跑测试 |
| B-06 | 嵌套 | 版本化迁移作业 | version/checksum/dry-run/执行记录/幂等重跑；发布顺序为 migrate→validate→ready |
| B-07 | 根 | 校准后启用性能回归门禁 | runner 方差达标后再以稳定热路径 3% 回归阻断 |

阶段退出条件：

- 安全状态损坏不会静默放行；
- 启动失败不泄漏生命周期资源；
- 生产实例不会自行执行未受控 DDL；
- 数据库变更可审计、可重跑，并与应用实例启动解耦；
- 后续热路径改动具备比较基线。

### 阶段 C：yang-system 三个 P0

#### C1：初始化信任根

| ID | 仓库 | 独立改进点 | 验收与提交边界 |
|---|---|---|---|
| C1-01 | 嵌套 | bootstrap secret 配置值对象 | 只保存摘要；弱值/非法摘要启动失败；单元测试 |
| C1-02 | 嵌套 | bootstrap Action 常量时间校验 | 缺失/错误 secret 失败，正确 secret 可继续到数据库 |
| C1-03 | 嵌套 | bootstrap 真实集成矩阵 | 错误、成功、并发、重放、日志脱敏 |

#### C2：租户隔离全路径证明

| ID | 仓库 | 独立改进点 | 验收与提交边界 |
|---|---|---|---|
| C2-01 | 嵌套 | 租户数据路径清单和架构门禁 | 枚举表、Action、raw SQL、事务、relation、后台任务 |
| C2-02 | 嵌套 | 双租户读写/对象 ID 负例 | 只增加契约测试；当前行为若失败，不在同提交修复 |
| C2-03 | 嵌套 | Join/批量/relation/事务负例 | 每类旁路有真实数据库证据 |
| C2-04+ | 所属仓库 | 修复测试发现的具体旁路 | 每个旁路一个提交，再重跑完整租户矩阵 |
| C2-final | 嵌套 | repository 强制租户 capability | 禁止 `Option<TenantContext>`、布尔 bypass；系统访问使用显式受控 capability |

#### C3：授权新鲜度

| ID | 仓库 | 独立改进点 | 验收与提交边界 |
|---|---|---|---|
| C3-00 | 嵌套 | 授权版本与失效传播 ADR | 明确事实源、事务、缓存、故障策略和最大延迟 |
| C3-01 | 嵌套 | 用户持久化 `authz_version` | Schema/读写/迁移独立完成 |
| C3-02 | 嵌套 | Token 写入授权版本 | 登录/refresh 使用同一授权快照版本 |
| C3-03 | 嵌套 | 管理员状态/角色事务递增版本 | 业务事实与版本同事务 |
| C3-04 | 嵌套 | 组织成员/角色事务递增版本 | 业务事实与版本同事务 |
| C3-05 | 嵌套 | outbox/Redis 传播加速 | 数据库仍是事实源；积压和重放可观测 |
| C3-06 | 嵌套 | 请求期版本比较 | 仅在 writer 全覆盖后启用；access 旧版本失败，故障策略失败关闭 |
| C3-07 | 嵌套 | 授权失效集成矩阵 | 停用、降级、移除、角色变化、并发、Redis 重启 |

阶段退出条件：

- bootstrap 不再是“第一个登录用户抢占”；
- 所有租户旁路都有可枚举证据；
- 高权限撤销在定义的窗口内生效；
- Redis 故障不会把过期权限重新放行。

## 五、生产共同基线队列

| ID | 来源 | 独立改进点 |
|---|---|---|
| P-01 | S-04 | 受信代理 CIDR 与标准 client IP 解析 |
| P-02 | S-07 | 配置文件 < 环境变量 < secret provider 的明确优先级 |
| P-03 | S-07 | JWT `kid` 与 active/retiring key ring |
| P-04 | S-08 | 高权限业务 audit 表 |
| P-05 | S-08 | 事务内 audit/outbox 原子写 |
| P-06 | S-09 | JSON 结构化日志与统一字段 |
| P-07 | S-09 | metrics/trace exporter 与低基数标签 |
| P-08 | S-09 | readiness 总预算与 SLO/告警 |
| P-09 | S-10 | raw SQL 边界与 sqlx offline 检查 |

这些点不能合成一个“production hardening”大提交。每个点独立设计、验证和回滚。

## 六、显式 UI 契约队列

| ID | 仓库 | 独立改进点 |
|---|---|---|
| U-01 | 根 | Module 稳定 ID、audience、navigation 语义 |
| U-02 | 根 | Action `query/command/row/bulk` 与 placement/selection 语义 |
| U-03 | 根 | Catalog schema version 兼容窗口和未知语义失败方式 |
| U-04 | 嵌套 Rust | 为 account/admin/org 声明 module/action presentation |
| U-05 | 嵌套前端 | Zod 消费新契约并增加 fixture |
| U-06 | 嵌套前端 | 删除 `.list/.me/.select` 和 `id` 启发式 |
| U-07 | 前后端分提交 | Catalog revision/ETag 协议 |

约束：

- 后端只输出稳定业务语义，不输出任意 Quasar 组件名或动态 import；
- 前端使用静态白名单映射图标/widget；
- 不长期维护旧新双 Catalog；
- 根 API 与前端消费绝不放在同一仓库提交中伪装成原子变更。

## 七、前端可维护性与生产完整性队列

### 7.1 生命周期和状态

| ID | 独立改进点 |
|---|---|
| FE-01 | `store.start()` 移到唯一 Quasar boot/app root |
| FE-02 | start 幂等与显式 dispose |
| FE-03 | session store 从 Catalog store 提取 |
| FE-04 | tenant/identity 状态边界提取 |
| FE-05 | Catalog store 只负责给定上下文的目录 |

### 7.2 TableView 按行为拆分

| ID | 独立改进点 |
|---|---|
| FE-T01 | 锁定 TableView 当前行为的组件测试 |
| FE-T02 | 提取 `useTableQuery` |
| FE-T03 | 提取 `useRelationOptions` |
| FE-T04 | 提取 `useTableSelection` |
| FE-T05 | 提取 `useTableActions` |
| FE-T06 | 提取 `useColumnPreferences` |
| FE-T07+ | 分别提取纯呈现子组件 |

每个 composable 单独提交；禁止一次性重写 1,000 行组件。

### 7.3 浏览器安全、部署与可用性

| ID | 独立改进点 |
|---|---|
| FE-S01 | 浏览器认证威胁模型与协议 ADR |
| FE-S02 | refresh token HttpOnly Cookie/BFF 后端协议 |
| FE-S03 | 前端 access token 内存化与多标签页语义 |
| FE-S04 | CSRF、rotation、reuse detection 集成测试 |
| FE-S05 | Workbench build/permission gate |
| FE-S06 | CSP report-only |
| FE-S07 | CSP enforce 与违规报告 |
| FE-O01 | 全局错误处理和 request id 关联 |
| FE-O02 | history fallback 与深链接 smoke test |
| FE-O03 | HTML/hash 资源缓存策略 |
| FE-A01 | accessibility lint |
| FE-A02 | axe + keyboard E2E |
| FE-C01 | Chromium/Firefox/WebKit 支持矩阵 |

## 八、基础库长期收敛队列

| ID | 来源 | 独立改进点 |
|---|---|---|
| CORE-01 | F-01 | 将内部 `Plugins` 重命名为 Action 调用语义 |
| CORE-02 | F-01 | 盘点并收窄 `PluginManager` 为基础设施扩展 |
| CORE-03 | F-01 | DatabaseInitializer 脱离旧插件列表 |
| CORE-04 | F-02 | 发布端到端数据库能力矩阵 |
| CORE-05 | F-03 | 建立领域错误到稳定传输错误投影 |
| CORE-06+ | F-03 | 每组同义 BaseError 独立废弃/迁移 |
| CORE-07 | F-04 | 修正 `cached_roles` 源码/注释漂移 |
| CORE-08 | F-07 | 强类型/动态 Record 使用边界检查 |
| CORE-09 | F-08 | tracing/metrics/health 接入示例 |

其中：

- `CORE-01` 先重命名调用概念，不同时删除旧插件系统；
- `CORE-02` 先完成使用者清单和 ADR，再决定 deprecated；
- `CORE-07` 必须先测量，不能把未测量的角色复制当成性能瓶颈；
- 所有热路径变更必须遵守 3% 相对基线门槛。

## 九、提交命名建议

| 类型 | 示例 |
|---|---|
| 基础库安全 | `fix(token): fail closed on invalid revocation watermark` |
| 根质量门禁 | `ci: cover every workspace crate in full gate` |
| 系统安全 | `fix(admin): require operator bootstrap secret` |
| 数据库行为 | `feat(auth): persist authorization version` |
| 前端依赖 | `fix(frontend): upgrade Quasar security patch` |
| 前端架构 | `refactor(frontend): centralize catalog startup` |
| 契约 | `feat(catalog): declare action placement semantics` |
| 测试 | `test(org): cover cross-tenant relation access` |
| 文档/ADR | `docs: define authorization freshness model` |

测试提交只有在“建立独立契约或证明矩阵”时单独存在；与某个小行为修复直接对应的回归测试应和该修复同提交。

## 十、立即执行顺序

从当前基线开始，前十个执行点固定为：

1. A-01：提交本路线图；
2. A-02：升级 Quasar 修复当前公告；
3. A-03：把 production dependency audit 加入门禁；
4. A-04：根 full gate 覆盖所有 workspace crate；
5. A-05：增加 Rust 供应链门禁；
6. A-06：声明并验证统一 MSRV；
7. B-01：建立性能 shadow 基线；
8. B-02：Token 水位线损坏失败关闭；
9. B-03：所有 bootstrap 退出路径统一关闭 Tools；
10. B-04：Schema 默认 validate。

之后按 B-05、B-06、C1、C2、C3 继续；C3 的请求期版本比较必须晚于所有写路径，期间不插入大型前端重构。B-07 只有在 shadow 数据证明 runner 方差可控后才启用。

若任何一点发现新的 P0：

- 先停止后续依赖点；
- 为新风险建立独立编号和验收条件；
- 不在当前提交顺手修复；
- 完成并提交后再恢复队列。

## 十一、完成定义

单个点只有同时满足以下条件才算完成：

- 需求不变量已写清；
- 影响面已核对；
- 自动化证据通过；
- 风险匹配门禁通过；
- staged 范围只包含该点；
- Git 提交成功且提交后复核通过；
- 未推送；
- 没有把失败、跳过或未执行的验证写成通过。

整个完善流程只有在三个项目各自的 P0、生产共同基线和显式契约完成后，才能重新评估是否达到 L4 生产候选。代码量增长、测试数量增长或所有现有门禁绿色，都不能单独替代这一判断。
