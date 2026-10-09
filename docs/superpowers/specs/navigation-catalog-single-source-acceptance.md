# 导航单一事实源实施验收

日期：2026-10-09。两个仓库均在 `codex/navigation-catalog` 隔离分支；工作树为 `D:/code/lib_yang-navigation-catalog` 及其 `project/yang-system`。未合并、推送或部署，主检出用户修改未动。

## 已实施行为

- UI schema 2.4 增加站内 `app_route`，贯穿声明、编译和请求级 Catalog；前端兼容 2.2/2.3/2.4，缺省仍使用 `/m/{id}`。
- 四个生产入口由 Catalog 投影：账号设置、权限管理、飞书数据源、审批派发。`identity` 只表示功能域；授权仍由后端决定。
- 权限工作台只从管理员功能域导航进入；权限组原有自服务 API 保留。
- 审批配置与派发记录合并为可键盘操作的 tabs，旧路径重定向；两个读 Action 共享原有权限并显式参与投影。
- 退役生产 demo Addon 和 API；保留独立演示视图、历史数据表和授权事实，不做删库迁移。
- Catalog 更新与 Cookie 恢复时纠正已保存但失效的功能域；前后端路径校验统一采用 Unicode White_Space/Cc，U+FEFF 两端均接受。

## 验证证据

- 应用 Rust lib：901 passed、4 ignored；架构门禁、fmt、clippy all-targets/all-features/locked 均 exit0。
- 评审修正后前端 Vitest 限制两个 worker：70 文件、729 项通过。format/lint/typecheck、locale、build、生产产物/预算/部署契约重新验证均 exit0；首屏 271.8 kB gzip。框架 UI 契约目标测试22项通过。
- 真实专用 MySQL `_test` / Redis DB15：集成前半段已通过；权限组首次失败来自两个遗漏的 `demo.notes.read` 夹具，换为普通权限 `feishu.datasource.read` 后全57项通过；后续9组集成测试全部通过（含审批派发控制台8项）。完整脚本首次失败与分段重跑均保留，不将分段结果写成完整脚本一次通过。
- MFA 首次有一次 TOTP 重放测试失败，随后该3项测试及完整集成重试中的3项均通过；原因未确定，本分支未改动 MFA。
- 浏览器拦截 Catalog/API 的 smoke：11项通过，覆盖两功能域、四落点、去重、审批权限、键盘及旧路径；不等同于真实后端浏览器端到端授权验证。
- 父仓库默认 quick/doctest 曾39项失败，出现 E0462/E0460；独立新 target 尝试还遇到页面文件不足及编译栈溢出。单独以 `CARGO_BUILD_JOBS=1`、`RUST_MIN_STACK=33554432`、doctest `--test-threads=1` 重跑后39项全部通过、113 ignored。但低并发 quick 再跑仍 exit1：38 passed、1 failed，FieldType::String doctest 链接报 LNK1102 内存不足。综合失败的根因未确定，不能宣称父仓库 quick 通过。
- 应用 quick / 前端默认 check 曾发生 Vitest spawn UNKNOWN (-4094)；最后一次默认 check 进入 Vitest 后退出，PowerShell 的 pnpm.ps1 报 NullReference。各子检查通过与限制并发全量测试通过，不等于标准 quick/check 全绿；本次不改动门禁以规避失败。
- 新路由校验/字段复制/link、审批展示及真实路由落点均有摘除式变异 RED→恢复 GREEN；详见计划 ledger。
- 一次独立整分支评审完成，无 Critical、两项 Important。两项均已修复：FEFF 解析回归先 RED 后 GREEN；摘除 AppLayout 的域纠正 effect 后 Cookie 恢复、运行期权限撤销两项新测试及既有单域测试均 RED，恢复后全量 GREEN。早期身份夹具未正确初始化 store 的失败不作为有效回归证据。

## 证据位置与限制

完整日志与逐任务 ruling 保留在工作树 `.superpowers/sdd/navigation-catalog-single-source/`，不提交浏览器临时产物。生成 OpenAPI/TypeScript 契约由脚本重生成，生产源码/生成物/集成夹具已核对无 `demo.notes`。两仓库 diff 检查通过；主检出中其他会话的修改保留，未纳入本次提交。

实现与评审修正已完成，标准聚合门禁验收仍未通过；计划 Task7 Step1 保持未完成。两个分支保留在隔离工作树，不执行推送或合并。

尚未运行 `run_ci.py full`、两套完整 Playwright、冷缓存 MSRV；现有 smoke 使用模拟 API。这些限制不改变已实现行为，但不满足完整推送前验收。
