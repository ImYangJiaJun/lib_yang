# 全仓审查报告 — 2026-09-20（第二轮，修复后复审）

> 承接 `docs/audit/2026-09-16-whole-repo-review.md`（上一轮 64 项候选 / 52 项已修）。
> 本轮是在那轮修复**之后**的再次全量审核。
>
> 范围：`lib_yang` 全 workspace（**明确排除 yang-pcg**）+ `project/yang-system`
> （后端 Rust、前端 React/TS、部署运维、脚本与门禁，重点）。
> 未经人工复核的原始发现、逐条 verdict 与证据在仓库外的工作流输出中，本文只收录
> 经对抗性核验后**存活**的条目。

## 一、方法与统计

两轮并行审计，每个单元由独立子代理先产出发现，再由另一个子代理**默认反驳**地逐条
核验（要求打开真实源码；不属实的行号/不可达的路径/文档明确要求的有意设计/纯风格问题
一律驳回）。

- 后端与框架：13 个单元（yang-system 8 个 + yang-base/yang-db/yang-runtime/yang-base-derive 5 个）
- 前端与基建：5 个单元（engine / features / shell+build / 部署运维 / 仓库脚本与 CI）
- 结论：原始 109 条 → 去重后**存活 103 条**（High 9 / Medium 33 / Low 61），其余驳回或
  跨单元重复；完整度批判轮额外检查了「无人认领的文件与跨单元盲区」。

本轮同时**实跑**了两个仓库的门禁（见第六节）。

## 二、已修复（9 项）

`project/yang-system` 仓库，`main` 分支，每项独立提交（commit message 带 `R2-*` 编号）：

| ID | commit | 缺陷 | 修复要点 |
|---|---|---|---|
| R2-H1 | `83d8eb2` | 启动校验漏掉 5 个占位密钥 | 占位判定改**前缀式**（`replace-with*` / `replace_with*` / 含 `placeholder`），取代易漏的手工精确清单；新增「从随仓库发布的示例配置逐个抽取占位值并断言被拒」的回归测试 |
| R2-H2 | `aedfa99` | 逐台撤销的 jti 黑名单 TTL 硬编码 7 天 < refresh 有效期 30 天 | 新增 `REVOCATION_BLACKLIST_TTL_SECONDS = MAX_REFRESH_TTL_SECONDS`（取校验上限而非当前配置值，不会漂移） |
| R2-H3 | `3f6d93d` | Step-up 资源指纹只哈希 body，三个管理动作的目标 ID 在路径上 | 指纹改为 `{body, path, query}` 规范化哈希 |
| R2-H4 | `0c2bdee` | 三个集成测试入口从未被任何门禁执行；自检用写死清单无法发现漏登记 | 补登三个入口 + 自检改为**反向发现** `tests/*.rs` |
| R2-H5 | `123be74` | `eslint.config.js` 的 plugins/rules 覆盖了 `react-hooks` 推荐集，16 条规则全失效 | 显式展开 plugins/rules；修 3 处 exhaustive-deps；其余 4 条规则逐条关闭并写明理由 |
| R2-H6 | `2901830` | 表数据查询键不含会话维度，登出 A 登录 B 会渲染 A 的整页行 | 新增 `shell/session-reset.ts` 在**会话边界**清空身份 store 与查询缓存（token 轮换不触发） |
| R2-M1 | `bd9886d` | `admin_enable_user` 可把已注销（匿名化）账号翻回 active | 守卫改为只接受 `UserStatus::Disabled` |
| R2-M2 | `3c5b486` | TOTP 防重放是 GET-then-SETEX 的 check-then-act，并发可双消费 | 改为 Redis 侧 Lua 原子「比较并写入」 |

每项都带**能判别真假的回归测试**：临时把修复改回旧行为后，对应测试确实失败（R2-H1/H3/H4/H5/H6 已实测复现）。

## 三、高严重度（9 项，已修 8）

| 状态 | 位置 | 缺陷 |
|---|---|---|
| 已修 R2-H1 | `project/yang-system/config.example.toml:36` | 启动校验漏掉 5 个占位密钥：可用示例配置烧出带公开 step-up 签名钥的生产实例 |
| 已修 R2-H5 | `project/yang-system/frontend/eslint.config.js:23` | eslint 配置的 plugins/rules 键覆盖了 react-hooks 推荐集，16 条 react-hooks 规则全部失效 |
| **未修** | `project/yang-system/frontend/src/engine/renderers/table/DataGrid.tsx:70` | 行选择按行下标 key 复用：数据集更换后界面勾选行与实际提交行不一致（批量危险操作命中已看不到的行） |
| 已修 R2-H6 | `project/yang-system/frontend/src/engine/renderers/table/use-table-query.ts:37` | 表数据查询键未纳入会话维度：换用户登录后仍渲染上一用户的整页行 |
| 已修 R2-H4 | `project/yang-system/scripts/run_ci.py:331` | 三个集成测试入口从未被任何门禁执行（含两个守护关键会话/删除 PII 回归的测试），且 self-test 把清单硬编码为 5 个名字因而无法发现遗漏 |
| 已修 R2-H2 | `project/yang-system/src/addon/account/domain/session/repository.rs:133` | 「踢出该设备」只写入 7 天 TTL 的 jti 黑名单，而 refresh token 有效期默认 30 天，且 refresh 路径从不检查 user_session.revoked_at；被踢设备 7 天后自动复活 |
| 已修 R2-H2 | `project/yang-system/src/addon/account/user/actions/revoke_session.rs:23` | 逐台撤销的 jti 黑名单 TTL 硬编码 7 天，短于 Refresh Token 的 30 天有效期，7 天后被踢设备的 refresh token 复活 |
| 已修 R2-H1 | `project/yang-system/src/config/mod.rs:1194` | step_up 的示例占位密钥不在校验黑名单里，被启动校验放行（与 AGENTS.md/示例配置的承诺相反），公开占位值可用于伪造 Step-up proof |
| 已修 R2-H3 | `project/yang-system/src/infrastructure/authorization/step_up.rs:344` | Step-up 资源指纹只哈希请求体，未纳入 path/query 参数：三个以路径参数为目标的 Step-up 保护动作，其证明可与任意目标互换（一次审批即可作用于任意账号） |

### 仍未修：DataGrid 行选择按行下标复用

`frontend/src/engine/renderers/table/DataGrid.tsx:70` 的 `rowSelection` 以 `row.key`
（`table-view-model.ts` 生成的路径下标 `root.0`…）为键，且行集变化时从不重置；
`TableView` 的 `selectedRows` 只在用户交互时更新，因此永远停留在旧行对象上。
后果：勾选若干行 → 改搜索词/翻页/数据刷新 → 新数据集在相同下标上的行仍显示为已勾选，
而批量（危险）操作提交的是**旧数据集**的行，用户看不见也无法取消。

**未在本轮修的原因**：可靠的修法（行 key 换成 view 声明的稳定业务主键，或在行集/查询键
变化时重置选择并把清除结果回传父组件）需要改 React 状态流，属于**行为变更**，而本环境
无法运行浏览器门禁（Playwright 需要浏览器与演示后端）来验证批量操作未被破坏。
建议单独一轮实施并在 `frontend/e2e` 补一条「换数据集后批量操作只提交可见行」的规格。

## 四、中严重度（33 项，已修 2）

| 状态 | 位置 | 缺陷 |
|---|---|---|
| 未修 | `clippy.toml:2` | clippy.toml 的 disallowed-types 从未被启用（clippy::disallowed_types 属 restriction 组、默认 allow），RUSTSEC-2026-0009 豁免所依赖的「不使用 Rfc2822」没有任何机械约束 |
| 未修 | `crates/yang-base/src/definition/builder/registry.rs:175` | 请求级 UI 目录直接复用「全角色并集」的表 JSON Schema，向低权用户泄露受限字段的名字/类型/校验约束 |
| 未修 | `crates/yang-base/src/http/request.rs:915` | 出站请求失败日志用 `error = %e` 绕过 URL 脱敏，把带 query/userinfo 的完整上游 URL 写进日志 |
| 未修 | `crates/yang-base/src/table/table_query/sql_render.rs:596` | 慢查询日志（C4）在生产构建中整体不存在：唯一计时入口 timed 被 cfg(test) 剔除，slow_threshold/request_id 只写不读 |
| 未修 | `crates/yang-db/src/mysql/query_builder/generator.rs:933` | MySQL 写路径把 u64 顶半区数字降级为 f64 绑定，静默丢精度改写 BIGINT UNSIGNED / 雪花 ID |
| 未修 | `crates/yang-db/src/postgres/query_builder.rs:971` | PG 写路径同样把 u64 顶半区降级为 f64 绑定（同 M29/NEW-11 遗漏点） |
| 未修 | `crates/yang-db/src/postgres/query_builder.rs:929` | PG 的 FieldType::Decimal 分支把数值降级为字符串绑定，而 PG 不允许 text→数值列的赋值转换，写 NUMERIC 列必然报类型错误 |
| 未修 | `crates/yang-db/src/redis/client.rs:109` | enable_logging 打开时把含密码的完整 Redis 连接串写入 info 日志 |
| 未修 | `project/yang-system/frontend/src/features/account/api.ts:54` | 账号中心写接口与 listSessions/fetchAvatar 绕过共享刷新客户端，access token 过期后永久硬失败 401 |
| 未修 | `project/yang-system/frontend/src/shell/App.tsx:32` | 会话结束不清空 TanStack Query 缓存，且 table-data 查询键不含会话身份 → 同标签页换账号会复用上一账号的数据 |
| 未修 | `project/yang-system/frontend/src/shell/App.tsx:41` | 会话重置只清空身份 store，TanStack Query 缓存在同标签页换用户后仍被复用（table-data / relation-options 键不含会话） |
| 未修 | `project/yang-system/frontend/tests/engine/contracts/openapi-contract.test.ts:72` | OpenAPI/api-types 快照无任何再生成门禁，后端契约漂移不会被任何门禁发现（且漂移的唯一后果是表单静默卡死） |
| 未修 | `project/yang-system/ops/prometheus/yang-system.rules.yml:71` | readiness critical 告警依赖探针流量，而仓库唯一发布的探针从不访问 /health/ready |
| 未修 | `project/yang-system/scripts/check_architecture.py:309` | production_source() 在文件内首个 #[cfg(test)] 处截断，导致 4 个文件共 3265 行生产代码对全部五项源码扫描检查不可见（含 Step-up proof store 与配置/密钥合成） |
| 未修 | `project/yang-system/src/addon/access/grants/actions/grant_permission.rs:65` | grant_permission 对「目标用户已停用」返回 401 Unauthorized，前端会把它当成调用者会话失效而清空操作者自己的会话 |
| 未修 | `project/yang-system/src/addon/access/grants/actions/revoke_permission.rs:93` | revoke_permission 用 revoke_by_subject 做「即时收敛」，连带撤销目标用户的 refresh token，使「刷新后透明生效」的契约与接口文案失效（修复 4c3912c 引入） |
| 未修 | `project/yang-system/src/addon/account/domain/claims.rs:78` | Refresh 凭据版本校验把「字段缺失」当作 0，而 4 条不受 issue_refresh_credential_version 约束的路径仍递增 credential_version，导致开关关闭（存量迁移模式）下这些用户的 refresh 全部 401 |
| 已修 R2-M2 | `project/yang-system/src/addon/account/domain/context.rs:218` | TOTP 第二因子防重放记录的是「当前时间步」而非「命中窗口」，同一码可在相邻窗口再消费一次（违反框架契约「防止相邻窗口重放」） |
| 未修 | `project/yang-system/src/addon/account/domain/login_event.rs:54` | login_event（及 user_session）没有任何保留期/清理机制，而登录失败路径任何人都可写；schema 注释与路线图声称「自带保留策略」但代码中不存在 |
| 已修 R2-M1 | `project/yang-system/src/addon/account/user/actions/admin_enable_user.rs:39` | admin_enable_user 只判断 is_active()，可把匿名化注销（status=deleted）的账号复活为 active，注销终态与撤销授权事实被破坏 |
| 未修 | `project/yang-system/src/addon/account/user/actions/change_password.rs:105` | 改密/重置/换绑/改用户名只收敛 Redis 水位线，不把 user_session 行标记为已撤销，导致「登录设备」列表继续把已失效设备显示为活跃 |
| 未修 | `project/yang-system/src/addon/account/user/actions/list_sessions.rs:39` | 会话行只在 revoke_session/delete_account 时才被标记撤销或删除：logout/停用/改密后设备列表与导出的 sessions 仍把已失效设备当作活跃，且无排序与清理 |
| 未修 | `project/yang-system/src/addon/account/user/actions/request_login_email_code.rs:60` | 免密登录验证码请求按「邮箱是否存在且启用」决定是否真实投递，投递在请求路径内同步 await，响应耗时泄露邮箱注册状态 |
| 未修 | `project/yang-system/src/addon/account/user/actions/request_mfa_email_code.rs:48` | request_mfa_email_code 是完整口令校验入口，但限流键仍按归一化标识（username/email），未跟随 login.rs 的 id:{user_id} 收敛，单账号口令猜测额度被拆成 3 个桶 |
| 未修 | `project/yang-system/src/addon/account/user/actions/request_mfa_email_code.rs:54` | MFA 邮箱验证码端点的登录限流身份键未绑定账号（仍用「提交的标识」），使同一账号获得多倍在线密码猜测预算，与文件头声明的「与登录同一限流预算」不符 |
| 未修 | `project/yang-system/src/addon/account/user/actions/request_password_reset.rs:48` | 密码找回端点仅对「存在且启用的邮箱」同步等待 SMTP 投递，未注册邮箱立即返回，响应体一致但耗时相差两个数量级，构成账号存在性时序枚举 |
| 未修 | `project/yang-system/src/addon/account/user/actions/revoke_session.rs:35` | 待拉黑的 jti 在事务外读取，且 refresh 路径从不检查 user_session.revoked_at：并发轮换产生的 refresh jti 永不被拉黑，撤销可被永久绕过 |
| 已修 R2-H1 | `project/yang-system/src/config/mod.rs:1249` | security.totp.aead_key 与 email.{change,mfa,login}.secret 的示例占位值同样被启动校验放行 |
| 未修 | `project/yang-system/src/config/source.rs:352` | ENVIRONMENT_BINDINGS 缺失 security.{issue_refresh_credential_version,password_reset_ttl_seconds,totp.*} 与 email.{change,mfa,login}.* 共 13 个字段，纯环境变量部署无法配置，且设置即启动失败 |
| 未修 | `project/yang-system/src/infrastructure/authorization/outbox.rs:254` | Outbox 清理是无 LIMIT 的单条无限定 DELETE，且内联在每 250ms 的派发循环里：升级后首次运行会一次性删除全部累积已发布行，阻塞授权版本传播 |
| 未修 | `project/yang-system/src/infrastructure/schema.rs:79` | authorization_outbox 保留清理谓词无索引支撑，且每 250ms（每轮轮询）扫描并删除一次已发布行 |
| 未修 | `project/yang-system/src/infrastructure/schema.rs:220` | user_agent 列宽 512 与未截断的 User-Agent 头不匹配：长 UA 可静默吞掉会话行与登录事件 |
| 未修 | `project/yang-system/src/infrastructure/schema.rs:235` | login_event 只有追加与按用户删除，没有保留期清理，公开登录端点可无界放大表体积 |

### 中严重度处置建议（按主题）

**授权失效与传播**
- `revoke_permission` 无即时收敛（`revoke_permission.rs:93`）：被撤销的权限在 Outbox 传播
  窗口内仍生效。窗口有界（`outbox_poll_interval_ms` 校验上限 250ms）且 ADR 有记录，但同类
  的 `converge_revocation` 已被另外 8 个动作使用，此处可对齐。
- `list_sessions` 只显示 `revoked_at IS NULL` 的行（`list_sessions.rs:39`）：`logout`/停用/改密
  走的是水位线或版本递增，**不**标记会话行，于是设备列表把已失效设备显示为活跃。
- `change_password.rs:105` 同类问题（只写水位线不改会话行）。

**限流与防枚举**
- `request_mfa_email_code.rs:48/54` 与 `request_login_email_code.rs:60` 的账号桶仍按归一化标识
  键控，未跟上 `login.rs` 的 `id:{user_id}` 命名空间修复：同一账号在该端点获得独立预算，
  且注释「与登录同一套标识分派」已与实现不符。
- 三个公开发码端点的「投递失败」与「未注册」分支存在可测的耗时差异（时序枚举），
  `request_password_reset.rs:48`、`request_login_email_code.rs:60`、`request_registration_email.rs:30`。

**数据层**
- `yang-db` 写路径的 u64 绑定精度：`mysql/query_builder/generator.rs:933`、
  `postgres/query_builder.rs:971` 把 u64 顶半区转 f64 绑定（上一轮 M29 只修了一处）。
- `postgres/query_builder.rs:929` 把 `FieldType::Decimal` 的值当字符串绑定，PG 无 text→numeric
  隐式转换，写 NUMERIC 列会类型报错。
- `definition/builder/registry.rs:175` 实例 UI 目录复用全角色 schema，未按调用者裁剪
  JSON Schema（泄露字段名/类型/约束）。
- `addon/account/domain/login_event.rs:54`、`infrastructure/schema.rs:235`、
  `password_reset/repository.rs`：`login_event` / `password_reset_token` 只有追加与按用户删除，
  没有保留期清理 worker（同仓的 `authorization_outbox` 已有）。
- `infrastructure/authorization/outbox.rs:254` 的已发布行清理是无 `LIMIT` 的单次 DELETE；
  `infrastructure/schema.rs:79` 缺支撑该 DELETE 的保留期索引，每 250ms 全表扫描。

**门禁自身的可信度（本轮新增的一类）**
- `scripts/check_architecture.py:309` 的 `production_source()` 在文件里**第一个** `#[cfg(test)]`
  处截断，其后的生产代码完全不被扫描（实测约 4 个文件、3265 行）：在 `step_up.rs` 的生产
  段写入裸 `sqlx::query(...)` 或 `ctx.table_query().update(...)` 仍报 passed。
- `clippy.toml:2` 的 `disallowed-types` 未启用（`clippy::disallowed_types` 属 restriction 组、
  默认 allow），RUSTSEC-2026-0009 的豁免因此在机制上没有任何强制力。

## 五、低严重度（61 项）

低严重度条目数量多、单条影响小，本报告不逐条展开；下列为位置索引，供后续按需处理。
其中相当一部分是**注释与实现不符**（例如 `session/repository.rs:133` 的「降级为只更新 jti
的幂等写入」实际什么都没写）与**契约描述滞后**，建议并入一次「注释与契约对账」清理。

| 位置 | 缺陷 |
|---|---|
| `.github/workflows/ci.yml:21` | 两个 workflow 的第三方 action 只按可变 tag/branch 引用，仅 cargo-deny-action 固定到摘要（而契约恰恰只校验了它） |
| `crates/yang-base-derive/src/params.rs:184` | params! 容器属性（含白名单放行的 #[cfg]）只作用于生成的结构体，不作用于生成的 impl，cfg 为假时产生难以理解的编译错误 |
| `crates/yang-base/src/action/auth/email_verification.rs:623` | keyed_digest 中的 unreachable!() 是生产代码里的 panic 宏，位于邮箱验证码生成与校验的必经路径上，违反项目『生产代码禁止 panic!』硬规则 |
| `crates/yang-base/src/action/auth/mfa.rs:104` | TotpLiteVerifier 是无状态纯函数，从不消费 TOTP 窗口，与模块文档承诺的『一次校验只成功消费一个窗口』不符，同一枚 TOTP 码在 ±1 容忍窗口内可无限次重复通过 |
| `crates/yang-base/src/action/auth/rate_limit.rs:219` | clear_failures 连带删除来源 IP 维度的失败计数，与『清除该身份的失败计数』的文档语义不符，使持有任一可用账号者能把共享的 IP 维度失败预算反复清零 |
| `crates/yang-base/src/database/schema_sync/sync.rs:153` | schema 同步的 MySQL advisory lock 在 future 被取消时不释放，连接归还池后锁仍被本进程占用 |
| `crates/yang-base/src/http/request.rs:783` | 熔断器分键只用 host_str()（不含端口），同一主机不同端口的健康上游会被连带熔断 |
| `crates/yang-base/src/http/response.rs:124` | M12 后 `text()` 自解析 charset，带引号或大小写不一致的非 UTF-8 charset 被静默按 UTF-8 有损解码 |
| `crates/yang-base/src/plugin/manager.rs:102` | PluginManager::register 的 on_register 在两次校验之间 await，并发同名注册会重复回调且失败者无 on_shutdown |
| `crates/yang-base/src/table/record.rs:328` | DATETIME 字段读回的是无时区 'T' 串，回写时被 validate_datetime 按 RFC 3339 拒绝，读-改-写往返断裂 |
| `crates/yang-base/src/table/table_query/plan.rs:138` | 读计划回落 default_order 时不校验 sortable/可读，ORDER BY 可落在受限字段上（测试只在测试专用渲染器上断言，生产路径无此校验） |
| `crates/yang-base/src/table/table_query/write.rs:481` | delete 的软删分支绕过字段写权限，与 del.rs 注释声称的「软删走更新路径上的字段写权限」相反 |
| `crates/yang-base/src/token/manager.rs:535` | 签发 Token 时 exp 用未检查的 now + 有效期，与同仓库 StepUpManager 的 checked_add 写法不一致 |
| `crates/yang-base/src/token/revocation.rs:154` | 撤销路径把 u64 剩余有效期直接 as i64 传给 Redis，与同文件 revoke_by_jti_with_ttl 的 i64::try_from 检查不一致 |
| `crates/yang-base/src/transport/axum.rs:401` | 框架默认下 /health/ready 为未认证端点，每次调用同步占用 MySQL 与 Redis 连接且无超时/并发上限 |
| `crates/yang-db/src/mysql/query_builder/write.rs:14` | insert_batch/insert_batch_with_size 未做占位符上限推导，宽表下超 65535 占位符（update_batch 的 M32 修复未同步） |
| `crates/yang-db/src/mysql/query_builder/write.rs:134` | enable_logging 打开时按 Debug 打印绑定参数值（含 PII/口令/令牌），与 crate 自述的日志边界冲突 |
| `crates/yang-db/src/redis/transaction.rs:370` | 有 WATCH 键时一律把 EXECABORT 判为乐观锁冲突，队列期错误被无意义重试 100 次；且 EXEC 内单命令失败时「部分已生效」被报成整体失败 |
| `crates/yang-db/src/redis/value.rs:205` | 未固定 RESP 协议：RESP3 的 Map/Set/Boolean 回复被降级为 Debug 字符串或空/false，读接口静默给错答案 |
| `crates/yang-db/src/reference.rs:95` | H5 只堵了 JOIN 表名；列标识符的同类 sink 仍无渲染期校验，且 __from_validated_owned 是 pub 且不校验 |
| `crates/yang-runtime/src/observability/mod.rs:72` | metrics_bind 与 http.bind 的冲突检查用地址相等，通配地址组合可绕过并让 HTTP 绑定先失败 |
| `crates/yang-runtime/src/observability/telemetry.rs:394` | 管理面优雅关闭无独立超时，一个不读响应的 /metrics 客户端即可挂住关闭阶段并连带跳过 OTel flush |
| `project/yang-system/frontend/src/features/account/AccountSettingsPage.tsx:149` | revokeSession 返回 void，「done === undefined」恒真：踢出设备成功后无提示、设备列表不刷新 |
| `project/yang-system/frontend/src/features/account/AccountSettingsPage.tsx:199` | 账号中心凭据变更只清本地会话、不广播结束信号，其它标签页不即时收敛 |
| `project/yang-system/frontend/src/features/account/api.ts:344` | listSessions/fetchSecurityEvents 把畸形响应静默降级成空列表，UI 显示“暂无...” |
| `project/yang-system/.github/workflows/ci.yml:12` | CI 用可变分支 ref 签出依赖仓库 lib_yang（LIB_YANG_REF: master），且该 job 同时持有只读 Deploy Key |
| `project/yang-system/.github/workflows/ci.yml:29` | 自仓库 checkout 未禁用 persist-credentials，GITHUB_TOKEN 留在工作区供后续第三方代码读取（契约只覆盖跨仓库签出） |
| `project/yang-system/compose.yaml:7` | 本地 compose 硬编码 root 口令且 Redis 完全无认证 |
| `project/yang-system/docker/app/Dockerfile:13` | 后端镜像基座只用可漂移 tag，未按摘要固定（与前端/CI 的双重固定标准不一致） |
| `project/yang-system/frontend/deploy/Dockerfile:18` | 前端构建上下文没有 .dockerignore，COPY . . 会把宿主 node_modules 与本地 .env* 带进镜像构建层 |
| `project/yang-system/frontend/deploy/Dockerfile:22` | 前端生产镜像以 root 运行，且自定义 nginx.conf 连 worker 都不降权 |
| `project/yang-system/frontend/scripts/verify-production-build.mjs:40` | 生产 CSP 校验用子串匹配，放宽 script-src 后门禁仍然通过 |
| `project/yang-system/frontend/src/engine/contracts/ui-catalog.ts:34` | Action path 只校验以 / 开头，协议相对路径会把 Bearer token 发往任意外域 |
| `project/yang-system/frontend/src/engine/renderers/action/use-presented-actions.ts:178` | redirect Action 在真实浏览器永不跳转，却提示“操作成功” |
| `project/yang-system/frontend/src/engine/renderers/table/use-relation-options.ts:47` | 关系选项缓存同样未按会话隔离，标签会被上一用户会话复用 |
| `project/yang-system/frontend/src/engine/session/session-controller.ts:110` | 启动 Cookie 恢复把瞬时网络错误也判为 anonymous，且此后不再重试 |
| `project/yang-system/frontend/src/shell/density.ts:21` | loadDensity 未包裹 localStorage 读取，站点数据被禁用时整个外壳渲染抛错白屏 |
| `project/yang-system/frontend/src/shell/pages/ModulePage.tsx:41` | ModulePage 身份守卫在未选择身份时不生效（新标签页/清空 sessionStorage 后可直接进入任意身份的模块） |
| `project/yang-system/frontend/vite.config.ts:89` | advancedChunks 的 router 分组永不生效，react-router 被打进 react chunk |
| `project/yang-system/scripts/run_ci.py:104` | OpenAPI 契约快照无漂移门禁：openapi.json / api-types.ts 与后端不一致时所有门禁仍全绿 |
| `project/yang-system/src/addon/access/domain/repository.rs:108` | 账号匿名化删除后 authz_grant 直授权限事实残留：授权 writer 只有按 (user_id, permission) 的删除端口，没有按用户清理的端口（2026-09-14 评审已提出，仍未修复） |
| `project/yang-system/src/addon/account/domain/authz_version.rs:269` | 账号启用原语只做「乐观相等」状态校验、不校验合法跃迁，已匿名化（status=deleted）账号可被管理员改回 active，且审计把前态记成 disabled |
| `project/yang-system/src/addon/account/domain/password_reset/repository.rs:94` | password_reset_token 行没有任何清理路径，过期/已消费/已作废记录无界增长（同批修复只给 authorization_outbox 加了清理） |
| `project/yang-system/src/addon/account/domain/session/repository.rs:226` | active_count_for_user / list_active 不带 LIMIT，登录热路径与 GET /users/sessions 全量加载该用户所有会话行（O(累计登录次数)） |
| `project/yang-system/src/addon/account/domain/session/repository.rs:77` | user_agent/ip 直接写入 VARCHAR(512)/VARCHAR(64) 且无长度护栏，超长 User-Agent 使会话行与登录事件写入静默失败（调用方只记 warn），该设备既不出现在设备列表也无法单独踢出 |
| `project/yang-system/src/addon/account/user/actions/change_email.rs:108` | change_email 返回 relogin_required:true 但未清除 yang_refresh Cookie（也未做同源校验），与三个同类凭据变更动作不一致 |
| `project/yang-system/src/addon/account/user/actions/change_email.rs:133` | 换绑端点的注册门控用了 credential_mutations_enabled 而不是 email.change 配置段是否存在：缺 [email.change] 时端点仍注册并对外返回 500 |
| `project/yang-system/src/addon/account/user/actions/delete_account.rs:55` | 注销事务未清理 authz_grant 直授行，注销账号的权限事实残留（与 admin_enable_user 复活路径叠加后重新生效） |
| `project/yang-system/src/addon/account/user/actions/login.rs:319` | record_login_session 静默吞错：refresh_jti 用 .ok() 降级写入 NULL，成功登录事件用 let _ = 丢弃，活跃会话数用 unwrap_or(0) 兜底 |
| `project/yang-system/src/addon/account/user/actions/logout.rs:44` | 退出全部会话只写 subject 水位线并把版本递增，从不标记 user_session.revoked_at，导致设备列表仍显示已退出设备且会话行随每次登录无界增长 |
| `project/yang-system/src/addon/account/user/actions/request_registration_email.rs:30` | 注册发码端点以邮箱是否已注册决定是否真实投递，SMTP 同步等待同样泄露账号存在性（与密码找回端点同根因、方向相反） |
| `project/yang-system/src/addon/account/user/actions/security_events.rs:55` | 安全事件查询无 ORDER BY，与“按时间倒序、分页”契约不符，且 LIMIT/OFFSET 在无序结果集上会重复或漏行 |
| `project/yang-system/src/addon/account/user/actions/totp_activate.rs:51` | TOTP 激活的“已激活”判断在行锁之外，且 activate_totp_in_tx 无 WHERE 守卫，并发两次激活会覆盖已写入的密钥与刚签发的恢复码 |
| `project/yang-system/src/config/mod.rs:721` | TOTP AEAD 密钥域的复用校验只覆盖 email.mfa / email.login，未覆盖 email.verification、email.change 以及 token / step-up 密钥 |
| `project/yang-system/src/config/mod.rs:1093` | email.password_reset.link_base_url 允许空主机（如 "https://:8443"），通过的配置会生成无法打开的重置链接 |
| `project/yang-system/src/config/mod.rs:568` | TotpSettings（含 aead_key）派生了 Debug，未像同文件其它密钥域那样做 REDACTED，一旦被打印即泄露密钥 |
| `project/yang-system/src/infrastructure/authorization/outbox.rs:236` | Outbox 积压指标每 250ms 执行一次无法走索引的全表 COUNT/MAX，随表规模与保留期线性增长 |
| `project/yang-system/src/infrastructure/authorization/step_up.rs:139` | 每次未携带 proof 的 Step-up 保护请求都会签发 challenge 并写入一行 audit_event，无任何节流：已登录用户可无界放大 365 天保留的审计表 |
| `project/yang-system/src/bootstrap.rs:225` | readiness 在业务监听器绑定之前就被置为就绪 |
| `project/yang-system/src/bootstrap.rs:197` | 启动路径未执行 SCHEMA.md 第 5 步的「再次规划必须为空」收敛校验 |
| `project/yang-system/src/infrastructure/schema.rs:43` | sync_with_database 会关闭调用方共享的 MySQL 连接池（形参按值传递并不能隔离 sqlx Arc 池） |


## 六、验证状态（本轮实跑）

| 命令 | 结果 |
|---|---|
| `lib_yang`: `python scripts/run_ci.py quick`（fmt + 分 crate test + clippy `-D warnings` + doctest） | ✅ exit 0 |
| `yang-system`: `python scripts/check_architecture.py` | ✅ passed |
| `yang-system`: `python scripts/run_ci.py --self-test` | ✅ passed |
| `yang-system`: `cargo fmt --all -- --check` | ✅ |
| `yang-system`: `cargo clippy --all-targets --all-features --locked -- -D warnings` | ✅ |
| `yang-system`: `cargo test --lib --locked` | ✅ 168 passed / 4 ignored |
| `yang-system` 前端：`eslint . --max-warnings 0` | ✅ exit 0（且探针确认 `rules-of-hooks` 真的会报错） |
| `yang-system` 前端：`tsc --noEmit` / `prettier --check` | ✅ |
| `yang-system` 前端：`vitest run` | ✅ 276 passed / 39 files |

**未执行**：`run_ci.py integration`（需要 MySQL 8.0 / PostgreSQL 16 / Redis 7 容器，本机
6379/3306 均未监听）。因此本轮涉及 Redis/MySQL 行为的修复（R2-M2 的 Lua 脚本、R2-H2 的
黑名单 TTL、以及 R2-H4 新补登的三个集成测试入口）**只做了编译与单元级验证**，其容器行为
由 `#[ignore]` 测试描述，推送前必须按 AGENTS.md 跑 `python scripts/run_ci.py integration`。

同样未执行：前端 Playwright（dev-server 与 production-build 两套），因此前端修复
（R2-H5 / R2-H6）只到 Vitest + typecheck + lint 级别。

## 七、遗留与建议（按优先级）

1. **推送前**：跑 `run_ci.py integration` 与 `run_ci.py full`。本轮新增/修改的容器相关行为
   与前端门禁都需要在完整环境复验；`full` 还包含 cargo-deny 与 MSRV 1.80 矩阵。
2. **DataGrid 行选择**（唯一未修的高严重度）：按第三节的修法单独一轮，并补 E2E 规格。
3. **门禁可信度**：`check_architecture.py` 的 `#[cfg(test)]` 截断与 `clippy.toml` 的
   `disallowed_types` 未启用，两者都属于「门禁看起来在管、实际没管」。建议与 R2-H5 同类处理：
   先修机制，再用一条**能判别真假**的测试锁住它。
4. **注释与实现对账**：低严重度里「注释宣称的行为与代码不符」反复出现（`session/repository.rs`
   的降级写入、`revoke_session` 的「保守覆盖」、`request_mfa_email_code` 的「与登录同一套键」）。
   本轮的教训是：这类失真注释会掩盖真实缺陷——`revoke_session` 的 TTL 缺陷正是被那句
   「保守覆盖 refresh token 完整有效期」掩盖的。
5. **`touch_on_refresh` 的语义**（`session/repository.rs:133`）：它对「行缺失」与「行已撤销」
   同样静默 `Ok(())`，即 refresh 路径始终不以 `user_session.revoked_at` 为准。本轮的 TTL 修复
   把撤销窗口拉长到覆盖任何合法 refresh token，但**没有**改变这个依赖关系；若要做到「撤销
   是 MySQL 事实、黑名单只是加速」，需要让 refresh 在轮换**之前**校验会话行（注意会动到
   受 `refresh_load_benchmark.rs` 守护的热路径）。

## 八、误报与驳回

对抗性核验驳回了 12 条（跨单元去重后计入 103 条之外），典型类型：

- **文档明确要求的有意设计**：Outbox 传播窗口（`docs/architecture/authorization-freshness-adr.md`
  记录了 10..=250ms 的有界窗口）被多次误判为「撤销延迟」；`user_user` 这一非常规列名是
  `infrastructure/schema.rs` 的**一致**声明（全仓 6 处引用一致），不是拼写错误。
- **需要前提不可达**：`yang-base` 的 `unreachable!()`（`email_verification.rs:623` 的 HMAC 任意
  长度密钥、`yang-base-derive/src/params.rs:151` 被 `if !matches!(...)` 守卫）——与上一轮
  M51 的裁决一致。
- **纯风格**：类型命名、注释密度、测试组织。

## 九、与上一轮的对比

上一轮（2026-09-16）的 52 项修复经本轮复核**未发现回归**：`where_and/where_or/having_cond`
的 `Result` 化（M33）、`RuntimeMetricNames` 扩展（M54）、写路径不再跑读投影门禁（H6，本轮
复核确认写字段权限校验仍在 `table_query/write.rs` 生效）、撤销水位线 TTL 取
`max(access, refresh)`（M2）等均正确。

本轮的 9 项高严重度里有 6 项属于**上一轮同类问题的遗漏点**（同一修复模式在别处没跟上）：
M2 修了水位线 TTL 但没修逐台撤销的黑名单 TTL；M29 修了一处 u64 绑定但没修写路径；
M18/门禁类修复没有覆盖 `check_architecture.py` 的截断与 `clippy.toml`。**建议下一轮把
「同类遗漏点」作为一个显式的检查维度**，而不是逐点修。
