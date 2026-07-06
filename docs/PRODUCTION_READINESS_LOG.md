# 生产级基础库修复记录

本文档记录基础库生产级审计中已经完成的修复点。每个完成点对应一次本地 git 提交；未完成的 RED 测试、探索结论或临时状态不作为完成点记录。

## 2026-07-06 - yang-db QueryBuilder SQL 调试接口错误暴露

- 范围：`crates/yang-db/src/mysql/query_builder.rs`、`crates/yang-db/src/postgres/query_builder.rs`
- 风险：公开 `to_sql()` 在 SQL 生成失败时吞掉真实错误，并拼出简化 SQL。非法表名或缺少 `GROUP BY` 的查询会被伪装成成功生成的 SQL，既影响调试判断，也可能在日志/上层拼接中泄漏未校验输入。
- 修复：新增 `try_to_sql() -> Result<String, DbError>`，让调用方可以拿到 `InvalidArgument`、`MissingGroupByClause` 等真实错误。
- 兼容：保留 `to_sql() -> String` 签名；失败时返回固定不可执行哨兵 `/* SQL generation failed */`，不再包含未校验表名或不完整查询结构。
- 对抗性验证：新增 MySQL/PostgreSQL 各 3 个单元测试，覆盖非法表名错误暴露、缺少 `GROUP BY` 错误暴露、旧降级路径不再泄漏 `DROP TABLE` 载荷。
- 已运行验证：`cargo test -p yang-db --lib try_to_sql`
- 已运行验证：`cargo test -p yang-db --lib to_sql_does_not_fallback_to_raw_untrusted_table`
