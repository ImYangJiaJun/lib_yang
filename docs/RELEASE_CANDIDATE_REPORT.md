# yang-base / yang-db 发布候选验证报告

**验证日期**：2026-07-15  
**验证基线**：`7775541`（P5-02 提交后 clean checkout）  
**候选版本**：yang-base 0.1.2 / yang-db 0.1.4 / yang-base-derive 0.1.0

## 结论

`docs/YANG_BASE_DB_COMPLETENESS_PLAN.md` 的 23 个工作包均已完成逐点 RED/GREEN、adversarial 反例验证和独立 Git 提交。stable、MSRV 1.80、feature matrix、doc tests、真实数据库、dependency audit 与 cargo package 门禁均已执行；未发现未记录的发布阻塞。

发布仍需遵守内部依赖顺序：先发布 `yang-base-derive`，再发布 `yang-db`，最后发布 `yang-base`。Cargo workspace 多包打包已用临时 registry 按该顺序完成三包 verify。

## 工具链与静态门禁

| 门禁 | 结果 |
|---|---|
| stable | rustc 1.94.1；fmt check 通过 |
| MSRV 1.80 | 两库 all-features、yang-db none、yang-base openapi-only 在 `-Dwarnings` 下通过 |
| feature matrix | 17 组组合的 check/lib/doc 共 51 个子门禁通过；覆盖 none、每个单 feature、default、all-features |
| feature isolation | 脚本 self-test 与真实依赖树通过；CI 契约新增 openapi 单 feature，恶意删除该行会失败 |
| 单元测试 | yang-db 397 passed/1 ignored；yang-base 481 passed/8 ignored |
| doc tests | yang-db 65 passed；yang-base 74 passed/148 ignored |
| Clippy | 两库 all-targets/all-features `-D warnings` 通过 |

## 真实后端集成

固定镜像为 MySQL 8.0、PostgreSQL 16-alpine、Redis 7-alpine，共 69 项通过：

| 路径 | 结果 |
|---|---:|
| MySQL/PostgreSQL 子查询、UNION、行锁、原子更新 | 8 passed |
| MySQL migration dry-run/checksum/concurrency 与只读 schema 验证 | 2 passed |
| MySQL/Redis Typed Action 纵向 CRUD | 1 passed |
| MySQL TableQuery CRUD / paginate / transaction | 12 / 8 / 10 passed |
| PostgreSQL CRUD / transaction isolation | 4 / 2 passed |
| Redis Pipeline / Lua | 9 / 13 passed |

发布候选验证实际捕获并修复了四类被默认离线套件隐藏的 fixture 漂移：容器 guard 提前 drop、默认 page size 20→10、DECIMAL/f64 类型不匹配、PostgreSQL SERIAL/i64 类型不匹配。软删除用例同时验证默认隐藏与 `with_trashed` 可见。

## dependency audit

`rustls-webpki` 已从 0.103.12 升级到修复版 0.103.13。以下两项使用显式 `cargo audit --ignore`，不存在静默忽略：

- `RUSTSEC-2023-0071`（rsa 0.9.10）：上游无修复版；依赖来自 sqlx-mysql，当前项目不持有或执行 RSA 私钥解密，Marvin 私钥计时路径不可达。
- `RUSTSEC-2026-0009`（time 0.3.36）：修复版 0.3.47 要求 Rust 1.88，与 MSRV 1.80 冲突；漏洞只影响 RFC 2822 恶意输入解析，仓库无该调用，time 仅由 jsonwebtoken/simple_asn1 引入。

带上述两项显式豁免后，cargo audit 为 0 个未豁免漏洞。另记录 5 个上游 warning：`proc-macro-error`/`rustls-pemfile` unmaintained、`anyhow`/`rand` advisory warning、`spin 0.9.8` yanked；均为传递依赖，后续依赖升级时复核。

## cargo package

| crate | 内容 | 压缩包 | verify |
|---|---:|---:|---|
| yang-base-derive 0.1.0 | 9 files | 8.7 KiB | 通过 |
| yang-db 0.1.4 | 84 files | 221.7 KiB | 通过 |
| yang-base 0.1.2 | 144 files | 305.3 KiB | 通过 |

独立打包 `yang-base` 会在 crates.io 尚无内部依赖时按预期失败；`cargo package -p yang-base-derive -p yang-db -p yang-base --locked` 使用临时 registry 完成全部三包生成和 verify，证明内容与发布顺序可执行。

## adversarial 发布契约

`release_candidate_contract` 要求报告必须记录版本、clean checkout、MSRV/stable、feature matrix、doc tests、MySQL 8/PostgreSQL 16/Redis 7、dependency audit、cargo package 与 adversarial 证据，并拒绝计划中的任何 `PENDING` 工作包。报告缺失且 P5-03 未完成时两项均 RED；完成本报告与计划状态后必须转 GREEN。
