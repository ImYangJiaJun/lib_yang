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
- 缺失表按字段、主键、索引、CHECK 和外键整体创建；已有表支持增加缺失结构、显式列改名和受控字段修改。
- 同一数据库使用 MySQL advisory lock 串行化多服务器并发启动；DDL 中断后下一次按 `information_schema` 重新规划，已完成步骤不会重复。
- 持锁后先读取并规划全部表；发现任意已知冲突时不会先修改排在前面的表。MySQL DDL 仍会隐式提交，执行阶段的数据库故障依靠下次启动幂等续作，而不是伪装成跨表事务。
- 所有可能被旧数据阻止的修改会先做全局只读预检；失败报告包含表、约束对象和确定性排序的主键，且不会执行任何 DDL。
- 同步器从不删除未知表、列、索引或约束；未声明 `renamed_from` 的字段不会被猜测改名。

```rust,ignore
let app_router = build_app_router()?;
let initializer = DatabaseInitializer::new(database);
let report = initializer.sync_app_schema(&app_router).await?;
// schema 就绪后才启动 HTTP listener
serve(app_router).await?;
```

## 明确不做

同步器不删除表/列/索引/约束，不猜测字段改名，不生成回滚，也不管理触发器、分区和在线 DDL 策略。旧数据不满足新约束时启动失败，由运维按报告中的表、对象和主键人工修复后重试。
