#![cfg(feature = "mysql")]
#![allow(clippy::expect_used)]

use yang_base::database::{DatabaseInitializer, Migration, MigrationManifest, MigrationPlanStatus};
use yang_base::error::BaseError;
use yang_base::plugin::Plugin;
use yang_base::table::{Field, SchemaIssueKind, Table};
use yang_db::Database;

struct MigrationPlugin {
    name: &'static str,
    version: &'static str,
    sql: String,
}

#[async_trait::async_trait]
impl Plugin for MigrationPlugin {
    fn name(&self) -> &str {
        self.name
    }

    fn migration_sql(&self) -> Vec<(String, String)> {
        vec![(self.version.to_string(), self.sql.clone())]
    }
}

fn mysql_url() -> String {
    std::env::var("MYSQL_TEST_URL")
        .unwrap_or_else(|_| "mysql://root:111111@localhost:3306/test".to_string())
}

#[tokio::test]
#[ignore = "需要真实 MySQL；通过 MYSQL_TEST_URL 配置"]
async fn dry_run_drift_and_concurrent_reservation_are_verifiable() {
    let verify_db = Database::connect(&mysql_url()).await.expect("连接验证库");
    #[allow(deprecated)]
    {
        verify_db
            .execute("DROP TABLE IF EXISTS p4_migration_audit")
            .await
            .expect("清理审计表");
        verify_db
            .execute("DROP TABLE IF EXISTS _migrations")
            .await
            .expect("清理迁移表");
        verify_db
            .execute("CREATE TABLE p4_migration_audit (marker VARCHAR(64) PRIMARY KEY)")
            .await
            .expect("创建审计表");
    }

    let dry_initializer = DatabaseInitializer::new(
        Database::connect(&mysql_url())
            .await
            .expect("连接 dry-run 库"),
        false,
    );
    let dry_plugin = MigrationPlugin {
        name: "dry_run",
        version: "202607150001",
        sql: "INSERT INTO p4_migration_audit(marker) VALUES ('dry')".to_string(),
    };
    let dry_plan = dry_initializer
        .plan_migrations(&dry_plugin)
        .await
        .expect("生成 dry-run");
    assert_eq!(dry_plan.entries[0].status, MigrationPlanStatus::Pending);
    assert!(
        !verify_db
            .table_exists(yang_db::table!("_migrations"))
            .await
            .expect("检查迁移表"),
        "dry-run 不得创建迁移表"
    );

    dry_initializer
        .create_migration_table()
        .await
        .expect("创建迁移表");
    dry_initializer
        .run_migrations(&dry_plugin)
        .await
        .expect("执行迁移");
    assert_eq!(
        dry_initializer
            .plan_migrations(&dry_plugin)
            .await
            .expect("应用后计划")
            .entries[0]
            .status,
        MigrationPlanStatus::Applied
    );

    let changed = MigrationPlugin {
        name: "dry_run",
        version: "202607150001",
        sql: "INSERT INTO p4_migration_audit(marker) VALUES ('changed')".to_string(),
    };
    assert_eq!(
        dry_initializer
            .plan_migrations(&changed)
            .await
            .expect("漂移计划")
            .entries[0]
            .status,
        MigrationPlanStatus::ChecksumMismatch
    );
    assert!(matches!(
        dry_initializer.run_migrations(&changed).await,
        Err(BaseError::MigrationChecksumMismatch { .. })
    ));

    let concurrent = MigrationPlugin {
        name: "concurrent",
        version: "202607150002",
        sql: "INSERT INTO p4_migration_audit(marker) VALUES ('once')".to_string(),
    };
    let first = DatabaseInitializer::new(
        Database::connect(&mysql_url()).await.expect("连接并发库 1"),
        false,
    );
    let second = DatabaseInitializer::new(
        Database::connect(&mysql_url()).await.expect("连接并发库 2"),
        false,
    );
    let (left, right) = tokio::join!(
        first.run_migrations(&concurrent),
        second.run_migrations(&concurrent)
    );
    assert!(left.is_ok() || right.is_ok(), "至少一个执行器必须成功");
    for result in [left, right] {
        assert!(
            result.is_ok() || matches!(result, Err(BaseError::MigrationInProgress { .. })),
            "并发失败必须是明确的 InProgress: {result:?}"
        );
    }

    #[derive(sqlx::FromRow)]
    struct CountRow {
        count: i64,
    }
    #[allow(deprecated)]
    let rows: Vec<CountRow> = verify_db
        .query("SELECT COUNT(*) AS count FROM p4_migration_audit WHERE marker = 'once'")
        .await
        .expect("统计并发执行");
    assert_eq!(rows[0].count, 1, "唯一预留必须阻止重复执行");
}

#[tokio::test]
#[ignore = "需要真实 MySQL；通过 MYSQL_TEST_URL 配置"]
async fn explicit_manifest_supports_dry_run_apply_drift_and_idempotent_retry() {
    let verify_db = Database::connect(&mysql_url()).await.expect("连接验证库");
    #[allow(deprecated)]
    {
        verify_db
            .execute("DROP TABLE IF EXISTS p4_manifest_audit")
            .await
            .expect("清理显式清单审计表");
        verify_db
            .execute("DROP TABLE IF EXISTS _migrations")
            .await
            .expect("清理迁移表");
        verify_db
            .execute("CREATE TABLE p4_manifest_audit (marker VARCHAR(64) PRIMARY KEY)")
            .await
            .expect("创建显式清单审计表");
    }
    let initializer = DatabaseInitializer::new(
        Database::connect(&mysql_url())
            .await
            .expect("连接显式迁移库"),
        false,
    );
    let manifest = MigrationManifest::new(
        "explicit-manifest",
        [
            Migration::new(
                "202607260001",
                "INSERT INTO p4_manifest_audit(marker) VALUES ('first')",
            ),
            Migration::new(
                "202607260002",
                "INSERT INTO p4_manifest_audit(marker) VALUES ('second')",
            ),
        ],
    )
    .expect("显式清单应有效");

    let dry_run = initializer
        .plan_manifest(&manifest)
        .await
        .expect("生成显式清单 dry-run");
    assert!(dry_run
        .entries
        .iter()
        .all(|entry| entry.status == MigrationPlanStatus::Pending));
    assert!(
        !verify_db
            .table_exists(yang_db::table!("_migrations"))
            .await
            .expect("检查迁移表"),
        "dry-run 不得创建迁移表"
    );

    initializer
        .apply_manifest(&manifest)
        .await
        .expect("首次执行显式清单");
    initializer
        .apply_manifest(&manifest)
        .await
        .expect("相同清单重跑必须幂等");
    let applied = initializer
        .plan_manifest(&manifest)
        .await
        .expect("应用后计划");
    assert!(applied
        .entries
        .iter()
        .all(|entry| entry.status == MigrationPlanStatus::Applied));

    let changed = MigrationManifest::new(
        "explicit-manifest",
        [
            Migration::new(
                "202607260001",
                "INSERT INTO p4_manifest_audit(marker) VALUES ('changed')",
            ),
            Migration::new(
                "202607260002",
                "INSERT INTO p4_manifest_audit(marker) VALUES ('second')",
            ),
        ],
    )
    .expect("漂移夹具清单本身应有效");
    assert_eq!(
        initializer
            .plan_manifest(&changed)
            .await
            .expect("生成漂移计划")
            .entries[0]
            .status,
        MigrationPlanStatus::ChecksumMismatch
    );
    assert!(matches!(
        initializer.apply_manifest(&changed).await,
        Err(BaseError::MigrationChecksumMismatch { .. })
    ));

    #[derive(sqlx::FromRow)]
    struct CountRow {
        count: i64,
    }
    #[allow(deprecated)]
    let rows: Vec<CountRow> = verify_db
        .query("SELECT COUNT(*) AS count FROM p4_manifest_audit")
        .await
        .expect("统计显式迁移执行次数");
    assert_eq!(rows[0].count, 2, "幂等重跑不得重复执行 SQL");
}

#[tokio::test]
#[ignore = "需要真实 MySQL；通过 MYSQL_TEST_URL 配置"]
async fn explicit_manifest_serializes_concurrency_and_recovers_interrupted_reservation() {
    let verify_db = Database::connect(&mysql_url()).await.expect("连接验证库");
    #[allow(deprecated)]
    {
        verify_db
            .execute("DROP TABLE IF EXISTS p4_manifest_recovery")
            .await
            .expect("清理恢复测试表");
        verify_db
            .execute("DROP TABLE IF EXISTS _migrations")
            .await
            .expect("清理迁移表");
        verify_db
            .execute("CREATE TABLE p4_manifest_recovery (marker VARCHAR(64) PRIMARY KEY)")
            .await
            .expect("创建恢复测试表");
    }
    let manifest = MigrationManifest::new(
        "manifest-recovery",
        [Migration::new(
            "202607260010",
            "INSERT IGNORE INTO p4_manifest_recovery(marker) VALUES ('once')",
        )],
    )
    .expect("恢复清单应有效");
    let first = DatabaseInitializer::new(
        Database::connect(&mysql_url()).await.expect("连接并发库 1"),
        false,
    );
    let second = DatabaseInitializer::new(
        Database::connect(&mysql_url()).await.expect("连接并发库 2"),
        false,
    );

    let (left, right) = tokio::join!(
        first.apply_manifest(&manifest),
        second.apply_manifest(&manifest)
    );
    assert!(left.is_ok(), "第一个显式清单作业应成功: {left:?}");
    assert!(
        right.is_ok(),
        "并发显式清单作业应在数据库锁后观察到 applied: {right:?}"
    );

    #[allow(deprecated)]
    verify_db
        .execute("UPDATE _migrations SET status = 'running' WHERE module_name = 'manifest-recovery' AND version = '202607260010'")
        .await
        .expect("模拟 DDL 已完成但记录未标记 applied 的进程中断");
    first
        .apply_manifest(&manifest)
        .await
        .expect("持有数据库迁移锁后应恢复遗留 running 预留并安全重跑");
    assert_eq!(
        first
            .plan_manifest(&manifest)
            .await
            .expect("恢复后计划")
            .entries[0]
            .status,
        MigrationPlanStatus::Applied
    );

    #[derive(sqlx::FromRow)]
    struct CountRow {
        count: i64,
    }
    #[allow(deprecated)]
    let rows: Vec<CountRow> = verify_db
        .query("SELECT COUNT(*) AS count FROM p4_manifest_recovery")
        .await
        .expect("统计中断重跑结果");
    assert_eq!(rows[0].count, 1, "幂等 SQL 在中断重跑后仍只能产生一份结果");
}

#[tokio::test]
#[ignore = "需要真实 MySQL；通过 MYSQL_TEST_URL 配置"]
async fn table_definition_validation_reads_schema_without_altering_it() {
    let db = Database::connect(&mysql_url()).await.expect("连接 MySQL");
    #[allow(deprecated)]
    {
        db.execute("DROP TABLE IF EXISTS p4_schema_users")
            .await
            .expect("清理表");
        db.execute("CREATE TABLE p4_schema_users (id BIGINT NOT NULL PRIMARY KEY, name VARCHAR(32) NULL, database_only JSON NULL)")
            .await
            .expect("创建表");
    }
    let initializer = DatabaseInitializer::new(
        Database::connect(&mysql_url()).await.expect("连接验证器"),
        false,
    );
    let definition = Table::new("p4_schema_users")
        .fields([
            Field::bigint("id").required().primary_key(),
            Field::string("name", 64).required(),
            Field::integer("age"),
        ])
        .build()
        .expect("表定义应合法");

    let report = initializer
        .validate_table_definition(&definition)
        .await
        .expect("验证 schema");
    assert_eq!(report.issues.len(), 3);
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.kind == SchemaIssueKind::MissingColumn && issue.field == "age"));
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.kind == SchemaIssueKind::IncompatibleType && issue.field == "name"));
    assert!(report.issues.iter().any(|issue| {
        issue.kind == SchemaIssueKind::NullabilityMismatch && issue.field == "name"
    }));

    #[derive(sqlx::FromRow)]
    struct CountRow {
        count: i64,
    }
    let rows: Vec<CountRow> = db
        .query_with_params(
            "SELECT COUNT(*) AS count FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name = ?",
            vec![serde_json::Value::String("p4_schema_users".to_string())],
        )
        .await
        .expect("统计列");
    assert_eq!(rows[0].count, 3, "验证接口不得自动 ALTER");
}
