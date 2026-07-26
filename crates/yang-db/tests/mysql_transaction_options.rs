#![cfg(feature = "mysql")]
#![allow(clippy::expect_used)]

use yang_db::{Database, IsolationLevel};

fn mysql_url() -> String {
    std::env::var("MYSQL_TEST_URL")
        .unwrap_or_else(|_| "mysql://root:111111@localhost:3306/test".to_string())
}

async fn checked_database() -> Database {
    let database = Database::connect(&mysql_url())
        .await
        .expect("连接 MySQL 测试库");
    let name: Option<String> = sqlx::query_scalar("SELECT DATABASE()")
        .fetch_one(database.pool())
        .await
        .expect("读取数据库名");
    assert!(
        name.is_some_and(|name| name.ends_with("_test")),
        "拒绝在非 _test 数据库执行事务选项测试"
    );
    database
}

#[tokio::test]
#[ignore = "需要 MYSQL_TEST_URL 指向 _test MySQL 数据库"]
async fn isolation_is_set_before_begin_and_read_only_snapshot_rejects_writes() {
    let database = checked_database().await;
    let observer = checked_database().await;
    #[allow(deprecated)]
    {
        database
            .execute("DROP TABLE IF EXISTS yang_db_tx_options")
            .await
            .expect("清理事务选项测试表");
        database
            .execute(
                "CREATE TABLE yang_db_tx_options (id BIGINT NOT NULL PRIMARY KEY) ENGINE=InnoDB",
            )
            .await
            .expect("创建事务选项测试表");
    }

    let mut isolated = database
        .transaction_with_isolation(IsolationLevel::Serializable)
        .await
        .expect("以 SERIALIZABLE 开启事务");
    let _: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM yang_db_tx_options")
        .fetch_one(
            isolated
                .executor()
                .unwrap_or_else(|| panic!("事务连接应可用")),
        )
        .await
        .expect("激活 InnoDB 事务");
    let isolation: String = sqlx::query_scalar(
        "SELECT trx_isolation_level FROM information_schema.innodb_trx WHERE trx_mysql_thread_id = CONNECTION_ID()",
    )
    .fetch_one(
        isolated
            .executor()
            .unwrap_or_else(|| panic!("事务连接应可用")),
    )
    .await
    .expect("读取 InnoDB 当前事务隔离级别");
    assert_eq!(isolation, "SERIALIZABLE");
    isolated.rollback().await.expect("回滚隔离级别测试事务");

    let mut snapshot = database
        .read_only_transaction()
        .await
        .expect("开启一致性只读快照");
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM yang_db_tx_options")
        .fetch_one(
            snapshot
                .executor()
                .unwrap_or_else(|| panic!("只读事务连接应可用")),
        )
        .await
        .expect("读取快照初值");
    #[allow(deprecated)]
    observer
        .execute("INSERT INTO yang_db_tx_options (id) VALUES (1)")
        .await
        .expect("并发连接写入测试行");
    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM yang_db_tx_options")
        .fetch_one(
            snapshot
                .executor()
                .unwrap_or_else(|| panic!("只读事务连接应可用")),
        )
        .await
        .expect("再次读取同一快照");
    assert_eq!((before, after), (0, 0), "同一快照不得观察到并发提交");

    let write = snapshot
        .execute_with_params(
            "INSERT INTO yang_db_tx_options (id) VALUES (?)",
            vec![serde_json::json!(2_i64)],
        )
        .await;
    assert!(write.is_err(), "只读事务必须由 MySQL 拒绝写入");
    snapshot.rollback().await.expect("回滚只读事务");

    #[allow(deprecated)]
    database
        .execute("DROP TABLE IF EXISTS yang_db_tx_options")
        .await
        .expect("清理事务选项测试表");
}
