# 纯收益修复 A/B 实施与宿主影响审计（2026-11-16）

## 1. 范围与基线

- 基线提交：`5f99242`（master）。
- 工作树：`.worktrees/pure-ab-20261116`，分支 `fix/pure-benefit-ab-20261116`。
- A 类（strict_pure，39 条）与 B 类（validation_pure，18 条）共 **57 条**全部实施。
- C 类（edge_pure，19 条）与 D 类（behavior_change，58 条）共 **78 条**未改代码，只对
  `D:/code/lib_yang/project` 下三个宿主做只读使用形态审计。
- 实施按 22 个文件组并行完成，逐组交叉复核 conditions、越界改动与测试有效性。

## 2. A/B 实施结果

57 条发现全部落地，复核后无遗留 condition 违背。工作流结束后补齐的缺口：

| 发现 | 补齐内容 |
| --- | --- |
| F001/F022 | `validate_opaque_path_segment` 拒绝 C0/C1/DEL；新增尾部 `\t/\n/\r`、`.`+控制字符等 RED 回归 |
| F118 | 事件侧之外，补齐 `TenantTokenGrant`/`TenantTokenResponse`/`MemoryTokenCache`/`FeishuOptionsRequest` 的手写脱敏 Debug 与断言 |
| F096/F098 | 二进制响应改为 Content-Length 快速失败 + chunk 累计、先判后拷，峰值内存受 `binary_limit` 约束 |
| F121/F122 | token TTL 上界与 `checked_add`；取消路径 RAII 放锁（正常路径语义不变） |
| F019 | 仅新增 opt-in `HttpRedirectPolicy`，默认仍等价 reqwest `limited(10)`，未改默认行为 |
| F012 | 软删表零 WHERE 的 update/delete/事务写显式返回 `MissingWhereClause`；未污染 INSERT 与共享读路径 |
| F018 | 仅默认 `Everyone` 被收敛；显式 `writable_by` 的 Key/SoftDelete/Timestamp 保留，补角色分支测试 |
| F064 | 保留构建期跨方言 OR 规则；渲染时按**目标方言**再校验，拒绝 PG 词法下闭合外层括号的载荷 |
| F074 | 同表校验只在 `module.table.is_some()` 时施加；补无主表 Module 静默跳过回归 |
| F055 | 水位线 TTL 取 `max(access, refresh, 90 天)`，只放大不缩短 |
| F008/F049/F050 | 补 Join/特殊字段离线懒连接、真实触库 `#[ignore]` + 硬 expect；CI 接入 `integration_database`、`test_aggregate_combinations`、`integration_pg_wave5_expressions` |
| F021/F077/F078 | `schema_sync_integration_test` 改为 `REQUIRE_DOCKER_TESTS`/CI fail-closed，本地跳过可辨识 |
| F029/F136 | MySQL 软跳过扫描改为「门控变量控制的 panic/expect」并要求门控分支，新增负向 self-test；job 头容忍行尾注释 |
| F138 | `drive` 4 条 + `im` 2 条 multipart/二进制端点均标注 `generated:false` |
| F126 | 已执行 `cargo test -p yang-pcg test_spawn_respects_min_spacing`，通过 |

## 3. 验证结果

| 门禁 | 结果 |
| --- | --- |
| `cargo fmt --all` | 通过 |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | 通过 |
| `cargo test --workspace --all-targets --locked` | 92 个 suite 全绿，2079 passed / 0 failed / 164 ignored |
| `cargo test --workspace --all-targets --all-features --locked`（`CARGO_BUILD_JOBS=2`） | 98 个 suite 全绿，2364 passed / 0 failed |
| `python scripts/verify_ci_contract.py --self-test` | 通过 |
| `python scripts/verify_ci_contract.py .github/workflows/ci.yml` | 通过 |
| `python scripts/run_ci.py --self-test` | 通过 |
| `python scripts/verify_openapi_coverage.py` | generated 2372 / 67 files，mismatch 0 |

本机默认并行度运行 `cargo test --all-features --all-targets` 时曾出现
`os error 1455（页面文件太小）` 导致的 `E0786 invalid metadata`；清理
`yang-base`/`yang-db`/`yang-base-derive` 产物并用 `CARGO_BUILD_JOBS=2` 串行复验后
全绿（98 suites / 2364 passed）。这是 Windows 本机页面文件限制，不是代码失败；
Linux 上的 CI feature-matrix 仍是权威门禁。

## 4. C/D 宿主使用形态矩阵

方法：3 个宿主 × 7 个主题 = 21 次只读审计；检查 Rust 源码、Cargo 依赖、配置、
前端 TS/TSX、迁移与文档。共 234 个 finding×宿主检查：**yes 25 / no 209 /
uncertain 0**；去重后 **23 条**发现在至少一个宿主中真实使用。

边界：三个宿主仓库的 **tracked 文件零改动**；`yang-system` 与 `receivables-system`
各有 2 个与本审计无关的未跟踪文件（设计文档/探针脚本/数据重置 SQL），本次未创建、
未修改、未纳入任何提交。

| 发现 | 宿主 | 使用形态与修改后影响 |
| --- | --- | --- |
| F009 | yang-system | 浏览器 refresh 走框架 `RefreshAction`，logout/改密等经 `revoke_by_subject` 写水位线；并发撤销时合法 refresh 会由成功变 `TokenRevoked` |
| F052 | yang-system | 公开免密登录按邮箱存在性选择 `deliver`；改异步会改变 SMTP 故障时的 503/202 与前端重试语义 |
| F053 | yang-system | refresh 的 verify 与 rotate 是两次 Redis 交互；并发 logout 下合法 refresh 可能被拒，且每次刷新多 1 次 Redis GET |
| F014 | yang-system | 存量库有 INT/SMALLINT UNSIGNED 列；读取走 `CAST(... AS SIGNED)`，按修复建议支持 unsigned 解码无影响；若改为 schema 校验拒绝则启动失败 |
| F039 | inventory-system | `execute_with_params` 传 `Value::Null` 写可空 TEXT；NULL 类型推断语义不变 |
| F005 | receivables-system | `stream_query` 做全量导出，无 timeout/abort；修复只影响取消清理，正常导出不变 |
| F007 | yang-system | 直接使用 `eval_script`/`set_nx_ex`（授权缓存、飞书锁、限流）；关闭重试会改变 Redis 抖动时的降级行为 |
| F111 | yang-system | 关闭阶段调用 `shutdown_management()`；引入内部超时会改变慢 /metrics 连接的停机路径 |
| F023 | yang-system | 自实现审批创建，超时已是 60s；宿主不依赖 yang-feishu，crate 改动无影响 |
| F027/F081/F082 | yang-system | 使用同类 binary-limit/出站机制，但缺陷形态不存在（crate 改动不进入宿主） |
| F083 | yang-system | 使用同一套 tenant token 刷新锁且无 RAII；若同步修复，取消时会提前放锁 |
| F084 | yang-system | 用 yang-db RedisClient 承载 token 缓存，命令级超时未配置；给宿主加固定超时会改变慢 Redis 语义 |
| F085 | yang-system | 已使用修复后的签名口径；crate 改动与宿主一致，但宿主仍依赖 legacy 复核 |
| F087 | yang-system | batch_get 的 typed wrapper 只剥部分类型；同步修复会让 date/contact 等由 Waiting 变可提交 |
| F090 | yang-system | `rekey_cells_by_field_id` 对重名列取第一个；同步修复会让整条记录变 Err，需按行为变更发布 |
| F119 | yang-system | 多条 TokenExpired 强刷路径把失败折成 Fatal；同步修复会改为 Retry |
| F011 | inventory-system | multipart Action 依赖 transport 注入的 `temp_root`/`original_filename`；该条仍是 uncertain，**未修改** |
| F066 | yang-system | 既有表升级新增可空唯一列 `custom_slug`，真实命中「新增列唯一索引跳过预检」；修复必须保留「新列对旧行全 NULL」例外 |
| F072 | yang-system | 入库 `to_openapi` 快照并用前端 AJV 按子 schema 编译；改写 `$ref` 会重写快照并可能打挂 AJV 子树测试 |
| F075 | yang-system | 单次后置 `middleware`；修复交错/重复语义不影响 |
| F076 | yang-system / receivables-system / inventory-system | 三者都依赖默认 `serve`/graceful shutdown 无 drain 截止；若加默认有限截止会截断 1 GiB 上传、导出、Doris 同步等长请求，必须默认 `None`/新增 opt-in |

### 4.1 对 A/B 的宿主约束

- **F018**：yang-system 的 `occurred_at`/`claimed_at` 使用显式 `writable_by([SYSTEM_ROLE])`；
  实现已只收敛默认 `Everyone`，这些列仍可写。
- **F074**：yang-system 的 View 字段来自本模块主表；同表校验不改变其构建结果。
- **F055**：水位线只放大 TTL；yang-system 的 `converge_revocation` 自动受益，无配置改动。
- **F037**：只拒绝严格负值；receivables/inventory 的原始 SQL 路径不经过该入口。
- **F061**：PG upsert 拒绝 `where_any`，只拒绝本会被静默丢弃的行过滤条件。
- **F012**：零 WHERE 的软删写本就违反框架 `MissingWhereClause` 契约；合法租户范围/主键写不受影响。

## 5. 未实施项与残余风险

- **F011（uncertain）**：`UploadedFile.temp_root` 的信任边界需要产品决策。inventory-system
  明确依赖 transport 注入的 `temp_root`/`original_filename` 键；直接收紧会使其上传整体失败。
- **F076（C/D）**：drain 截止只能以默认 `None` 的新增 opt-in 形式落地；否则三个宿主都会
  在停机时截断合法长请求。
- **F066/F072（C/D）**：分别涉及 yang-system 的存量表升级与 OpenAPI/AJV 契约快照，
  需要宿主侧迁移与快照重生成，不能作为库侧静默变更。
- **F009/F053（C/D）**：并发 logout/refresh 的语义变化会让少量合法 refresh 返回
  `TokenRevoked`；需要宿主前端按终态处理并在发布说明标注。
- **F007（C/D）**：Redis 重试语义影响 yang-system 的授权缓存、飞书锁与限流降级路径，
  需要按命令幂等性与可区分 unknown-outcome 错误单独设计。
- **F012 覆盖边界**：零 WHERE 守卫只作用于 `update`/`update_in_tx`/`delete`/
  `delete_in_tx`；upsert 由冲突键定位且已拒绝 `where`/`where_any`，不存在同一绕过面。
  软删表 upsert 是否应排除已删除行属于既有语义，本次未改变。

### 5.1 非零代价（不改变业务逻辑，但需发布说明）

- `crates/yang-feishu/Cargo.toml` 为 F122 在正常依赖的 tokio 上启用 `rt` feature
  （`Handle::try_current`），会扩大所有下游的依赖编译面；dev-dependency 另加
  `net`/`io-util` 仅供 loopback 测试。
- F118 让公开类型 `FeishuOptionsRequest`/`TenantTokenGrant` 的 `{:?}` 文本变为
  `<已隐藏>`。Debug 不是稳定 API，但宿主若解析 Debug 文本会观察到变化。
- F137 的 `.dockerignore` 新增排除项会改变 Docker 构建上下文；已核对保留
  `Cargo.toml`/`Cargo.lock`/`crates/`/`project/yang-system` 构建输入。
- F114 的 action 固定 SHA 需要人工跟随上游升级（本仓库未配置 Renovate）。

### 5.2 需宿主侧拍板的 7 条

| 发现 | 宿主 | 需要决定的事 |
| --- | --- | --- |
| F011 | inventory-system | 上传信任边界：是否允许保留 transport 注入的 `temp_root`/`original_filename` 契约 |
| F076 | 三个宿主 | drain 截止是否只做默认 `None` 的新增 opt-in，避免截断长请求 |
| F066 | yang-system | 存量表新增可空唯一列：保留「旧行全 NULL」例外，还是接受启动 fail-closed |
| F072 | yang-system | OpenAPI `$ref` 改写需要同步重生成快照并调整前端 AJV 子 schema 编译 |
| F009 | yang-system | 并发 logout/refresh 时合法 refresh 变 `TokenRevoked`，前端需按终态处理 |
| F053 | yang-system | refresh 的 verify/rotate 原子性会改变并发语义与 Redis RTT 基线 |
| F007 | yang-system | Redis 非幂等命令重试的降级语义，需要可区分 unknown-outcome 错误的设计 |

## 6. 合并与宿主编译验证（2026-11-16）

已快进合并到 `master`，未推送远端；基线 `5f99242` 起共 11 个 A/B 代码与报告提交，
最新代码提交 `d79dd3c`。四个宿主的 `cargo check --all-targets --locked`
（`CARGO_BUILD_JOBS=2`、`CARGO_INCREMENTAL=0`）：

| 项目 | 结果 | 耗时 | 诊断 |
| --- | --- | --- | --- |
| yang-system | 通过 | 32s | 0 error / 0 warning |
| inventory-system | 通过 | 6m15s | 0 error / 0 warning |
| yang-system-feishu-form-sync | 通过 | 33s | 0 error / 0 warning |
| receivables-system | 无法验证 | — | 依赖解析失败：外部 path 依赖 `D:\code\fin_receivables_rs\crates\{receivables-core,app,io}` 在本机不存在，与本次改动无关 |

首次 yang-system 检查发现：仅启用 `mysql` 的 feature 组合下，`table/mod.rs` 的
PG CHECK 辅助函数 re-export 触发 `unused_imports`，宿主 CI 以 `-D warnings` 跑
clippy 会失败。已在 `85eb87a` 按 feature 拆分 re-export（`d79dd3c` 为 rustfmt
排序），并用宿主同款 feature 组合复验 `cargo check -p yang-base` 通过
（3m52s，无 warning）。`--locked` 对三个通过的宿主均未报锁文件不一致。

`yang-system-feishu-form-sync` 是 yang-system 的同源副本，同样通过；其独有的
飞书 addon 不依赖 `yang-feishu`。receivables-system 的 yang-base/yang-db 调用面
仍只有 §4 的静态审计结论，真实编译验证需在具备 `fin_receivables_rs` 签出的环境
补跑。
