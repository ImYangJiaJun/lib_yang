# TableConfig 与数据库 Schema

`TableConfig` 是运行期访问、字段校验、权限、筛选/排序的契约，也是启动期 additive schema 同步的声明来源。它不是任意 DDL、触发器、分区或数据库运维配置的完整真相。

## 可选兼容验证

- `TableConfig::validate_schema(&[SchemaColumn])` 对任意已获取的列快照做纯内存验证。
- 启用 `mysql` 时，`DatabaseInitializer::validate_table_config` 从 `information_schema.columns` 只读列快照后调用同一验证器。
- 报告包括声明字段缺失、存储类型不足、NULL 约束和 `AUTO_INCREMENT` 不一致。字符串列必须能容纳声明的 `max_length`。
- 数据库额外列被忽略，因为迁移、触发器或其他消费者可以合法拥有这些列。
- `ForeignKey` 只验证本地列存在与 NULL 约束；它没有本地物理类型信息，因此启动期同步拒绝为它生成 DDL。

## 启动期 additive 同步

- `AppRouter::table_configs()` 汇总每个模块的主表与附属表；附属表通过 `ModuleRouter::with_schema_table` 注册，不进入主表 CRUD 上下文。
- `DatabaseInitializer::sync_app_schema(&app_router)` 必须在监听 HTTP 端口前调用。系统项目不需要 `.sql` 文件。
- 缺失表按字段、主键和索引整体创建；已有表只增加缺失列、主键和索引。
- 同一数据库使用 MySQL advisory lock 串行化多服务器并发启动；DDL 中断后下一次按 `information_schema` 重新规划，已完成步骤不会重复。
- 持锁后先读取并规划全部表；发现任意已知冲突时不会先修改排在前面的表。MySQL DDL 仍会隐式提交，执行阶段的数据库故障依靠下次启动幂等续作，而不是伪装成跨表事务。
- 已有表的类型、NULL、自增、主键或同名索引冲突都会 fail-fast；同步器从不执行 `DROP`，也不自动改写现存列。
- 已有数据的表不能增加无默认值的必填字段；已有表缺失自增主键列时要求人工处理。

```rust,ignore
let app_router = build_app_router()?;
let initializer = DatabaseInitializer::new(database, false);
let report = initializer.sync_app_schema(&app_router).await?;
// schema 就绪后才启动 HTTP listener
serve(app_router).await?;
```

## 明确不做

同步器不删除表/列/索引，不修改已有字段，不生成回滚，不管理外键、触发器、分区和在线 DDL 策略。需要数据回填、类型变更或零停机大表变更时，必须走单独、可审计的运维流程。
