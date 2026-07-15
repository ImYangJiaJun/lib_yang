#![cfg(feature = "mysql")]
#![allow(clippy::expect_used)]

use yang_base::database::{DatabaseInitializer, MigrationPlanStatus};
use yang_base::error::BaseError;
use yang_base::plugin::Plugin;
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
            .table_exists("_migrations")
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
