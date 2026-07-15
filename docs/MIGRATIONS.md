# 数据库迁移治理

`DatabaseInitializer` 把插件 `migration_sql()` 返回的每个 `(module, version, SQL)` 视为一条不可变迁移。版本一旦应用，SQL 内容不得修改。

## 计划与 dry-run

- `plan_migrations(plugin)` 和 `plan_all(plugin_manager)` 只读数据库，返回 `Pending`、`Applied`、`ChecksumMismatch` 或 `InProgress`。
- dry-run 不创建 `_migrations`，不执行迁移 SQL，也不写迁移记录；迁移表不存在时所有声明均为 `Pending`。
- `_migrations.checksum` 保存 SQL 的稳定 FNV-1a 64 位校验和。相同 module/version 的 checksum 不一致会在启动时返回 `MigrationChecksumMismatch`。
- 从旧版本升级而来的无 checksum 记录不可验证，按 checksum mismatch 处理；必须人工核对并补录，不能静默信任。

## 执行与并发

- 执行器先以 `(module_name, version)` 唯一键写入 `running` 预留，再执行 SQL，成功后改为 `applied`。并发启动只能有一个执行器获得预留，因此同一迁移不会重复执行。
- 看到 `running` 的其他启动实例返回 `MigrationInProgress`，由部署编排重试；不绕过、不重复执行。
- 执行失败会尽力删除本实例持有的 `running` 预留。进程崩溃留下的 `running` 必须由操作者核对数据库实际状态后处理。

## 事务边界

- `use_transaction = true` 时，DML 迁移与迁移记录在同一 `yang_db::Transaction` 上执行。
- MySQL DDL 会隐式提交，不能承诺 DDL 与迁移记录原子回滚；`running` 状态用于显式暴露这一恢复边界。插件应把每个 migration 声明为一个可独立审计、可判定是否完成的语句。
- 一条 migration 不做分号切割。存储过程、触发器或包含内部分号的复杂脚本应交给专用脚本执行器。
- `yang_db::mysql::Database::init` 与 `yang_db::postgres::Database::init` 的分号切割入口已 deprecated，不作为迁移执行路径。
