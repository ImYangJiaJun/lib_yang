# 数据库迁移治理

`DatabaseInitializer` 以显式 `MigrationManifest` 作为独立发布作业的主入口。清单按
`module + version` 标识每条不可变迁移；版本一旦应用，SQL 内容不得修改。旧
`Plugin::migration_sql()` 入口保留兼容，但新系统不应借用插件注册表表达数据库演进顺序。

```rust
use yang_base::database::{DatabaseInitializer, Migration, MigrationManifest};

let manifest = MigrationManifest::new(
    "account",
    [
        Migration::new("202607260001", "ALTER TABLE users ADD COLUMN locale VARCHAR(16) NULL"),
    ],
)?;
let plan = initializer.plan_manifest(&manifest).await?; // 只读 dry-run
initializer.apply_manifest(&manifest).await?; // 独立 migration job
```

`MigrationManifest::new` 在连接数据库前拒绝空 module/version、空 SQL、重复版本和
乱序版本。清单构建后字段私有且只读，避免运行期意外改写已发布迁移。

## 计划与 dry-run

- `plan_manifest(manifest)` 只读数据库，返回 `Pending`、`Applied`、`ChecksumMismatch`
  或 `InProgress`；`plan_migrations(plugin)` 和 `plan_all(plugin_manager)` 是旧兼容入口。
- dry-run 不创建 `_migrations`，不执行迁移 SQL，也不写迁移记录；迁移表不存在时所有声明均为 `Pending`。
- `_migrations.checksum` 保存 SQL 的稳定 FNV-1a 64 位校验和。相同 module/version 的 checksum 不一致会在启动时返回 `MigrationChecksumMismatch`。
- 从旧版本升级而来的无 checksum 记录不可验证，按 checksum mismatch 处理；必须人工核对并补录，不能静默信任。

## 执行与并发

- `apply_manifest(manifest)` 先取得数据库级 MySQL advisory lock，再确保迁移记录表
  存在，以 `(module_name, version)` 唯一键写入 `running` 预留、执行 SQL，成功后改为
  `applied`。并发显式清单作业在锁上串行等待，后到者会观察到最新 applied 状态。
- advisory lock 独占一个池连接，迁移执行使用另一个连接；调用方必须把 MySQL 连接池
  上限设置为至少 2，否则在执行 SQL 前明确失败。
- 执行错误会尽力删除本实例持有的 `running` 预留。进程中断会释放连接级 advisory
  lock；下一作业取得锁后，仅清理 checksum 一致的遗留 `running` 预留并重跑。显式
  清单 SQL 因此必须可重入；checksum 不一致仍 fail-closed，禁止自动恢复。
- 旧 `run_migrations(plugin)` 兼容入口不具备上述锁内恢复语义，看到 `running` 仍返回
  `MigrationInProgress`；生产部署必须使用显式清单入口。

## 事务边界

- `use_transaction = true` 时，DML 迁移与迁移记录在同一 `yang_db::Transaction` 上执行。
- MySQL DDL 会隐式提交，不能承诺 DDL 与迁移记录原子回滚；`running` 状态用于显式
  暴露这一恢复边界。每个 migration 应只包含一个可独立审计、可判定是否完成且可重入
  的前向语句。
- 一条 migration 不做分号切割。存储过程、触发器或包含内部分号的复杂脚本应交给专用脚本执行器。
- `yang_db::mysql::Database::init` 与 `yang_db::postgres::Database::init` 的分号切割入口已 deprecated，不作为迁移执行路径。
