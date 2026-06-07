---
name: maturity-implementation-effort
description: Large ongoing effort implementing all gaps from the yang-base maturity assessment
metadata:
  type: project
---

正在按 `docs/yang-base-engine-maturity-assessment.md` 第十二节方案，落地该评估发现的全部问题：yang-base C1–C6/I1–I12、yang-db DB-1..DB-21、Tier-Nice N1–N12、深度复核 NG-1..NG-4。

**实施顺序**（第七节）：事务(C1)→OR(C2a)→可观测(C4)→错误分类(C5)→弹性→写路径→多后端(C3)→JOIN(C2b)。yang-db 三个 High（DB-1/2/3）可先行。

**硬约束**：中文注释；checked API 优先（禁新增生产 panic/unwrap/expect）；保持 feature gate（token/http/mysql/validator/plugin-schema）；鉴权热路径零分配；确定性契约不破坏；向后兼容优先，破坏性变更标 SemVer；不顺手拆 query_builder.rs；集成测试 `#[ignore]`+环境变量；每步 `cargo build/test --lib` 绿。

**进度回填**：每条完成后回填评估文档 11.1/11.2/11.3/11.4/11.5 追踪表的状态/commit/日期（2026-06-07 起）。注意文档 file:line 可能因 H-1 重构而陈旧，编辑前先核实当前代码。

启动日期：2026-06-07。
