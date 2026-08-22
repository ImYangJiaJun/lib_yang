#![cfg(feature = "mysql")]
#![allow(clippy::expect_used)]

//! 受控服务端时间表达式（`SqlExpr`）集成测试：验证 `UNIX_TIMESTAMP()` 系列表达式
//! 在真实 MySQL 上的 INSERT / UPDATE / 行锁 SELECT 行为，以及 `insert_returning_id`
//! 返回自增主键。需要 `MYSQL_TEST_URL` 指向名称以 `_test` 结尾的 MySQL 数据库。
//!
//! 运行方式：
//!   MYSQL_TEST_URL=mysql://root:111111@localhost:3306/yang_db_test \
//!   cargo test --test integration_mysql_server_expr -- --ignored --test-threads=1

use serde_json::json;
use yang_db::{CompareOp, Database, QueryBuilder, SqlExpr};

fn mysql_url() -> String {
    std::env::var("MYSQL_TEST_URL")
        .unwrap_or_else(|_| "mysql://root:111111@localhost:3306/test".to_string())
}

/// 连接测试库并校验库名以 `_test` 结尾，防止误操作真实数据库。
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
        "拒绝在非 _test 数据库执行服务端表达式测试"
    );
    database
}

#[tokio::test]
#[ignore = "需要 MYSQL_TEST_URL 指向 _test MySQL 数据库"]
async fn server_time_expressions_drive_insert_update_and_locked_select() {
    let database = checked_database().await;

    #[allow(deprecated)]
    {
        database
            .execute("DROP TABLE IF EXISTS yang_db_server_expr")
            .await
            .expect("清理测试表");
        database
            .execute(
                "CREATE TABLE yang_db_server_expr (\
                    id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY, \
                    label VARCHAR(64) NOT NULL, \
                    used_at BIGINT NULL, \
                    expires_at BIGINT NOT NULL\
                ) ENGINE=InnoDB",
            )
            .await
            .expect("创建测试表");
    }

    // INSERT：表达式列（expires_at = UNIX_TIMESTAMP() + ?）+ 显式自增 ID 返回
    let id = database
        .table(yang_db::table!("yang_db_server_expr"))
        .set_expr(
            yang_db::field!("expires_at"),
            SqlExpr::unix_timestamp_add(600),
        )
        .insert_returning_id(&json!({"label": "otp"}))
        .await
        .expect("表达式插入");
    assert!(id > 0, "自增 ID 应大于 0");

    let expires_at: i64 =
        sqlx::query_scalar("SELECT expires_at FROM yang_db_server_expr WHERE id = ?")
            .bind(id)
            .fetch_one(database.pool())
            .await
            .expect("读取 expires_at");
    let server_now: i64 = sqlx::query_scalar("SELECT UNIX_TIMESTAMP()")
        .fetch_one(database.pool())
        .await
        .expect("读取服务端时间");
    assert!(
        (expires_at - server_now - 600).abs() <= 5,
        "expires_at 应约为服务端当前时间 + 600 秒，实际差 {}",
        expires_at - server_now - 600
    );

    // UPDATE：SET 表达式赋值 + WHERE 列↔表达式比较（仅更新未过期行）
    let affected = database
        .table(yang_db::table!("yang_db_server_expr"))
        .set_expr(yang_db::field!("used_at"), SqlExpr::unix_timestamp())
        .where_expr(
            yang_db::field!("expires_at"),
            CompareOp::Gt,
            SqlExpr::unix_timestamp(),
        )
        .expect("合法的列↔表达式比较")
        .where_and(yang_db::field!("id"), CompareOp::Eq, id)
        .update(&json!({}))
        .await
        .expect("表达式更新");
    assert_eq!(affected, 1, "未过期行应被精确命中");

    let used_at: Option<i64> =
        sqlx::query_scalar("SELECT used_at FROM yang_db_server_expr WHERE id = ?")
            .bind(id)
            .fetch_one(database.pool())
            .await
            .expect("读取 used_at");
    let used_at = used_at.expect("used_at 应已被服务端时间填充");
    assert!(
        (used_at - server_now).abs() <= 5,
        "used_at 应约为服务端当前时间，实际差 {}",
        used_at - server_now
    );

    // 事务内行锁 SELECT：投影服务端时间表达式（构建器从 pool 创建，不借用事务）
    let mut tx = database.transaction().await.expect("开启事务");
    let rows: Vec<(i64, i64)> = tx
        .select_for_update(
            QueryBuilder::from_pool(database.pool(), yang_db::table!("yang_db_server_expr"))
                .field(yang_db::field!("id"))
                .select_expr(SqlExpr::unix_timestamp(), yang_db::field!("server_now"))
                .where_and(yang_db::field!("id"), CompareOp::Eq, id),
        )
        .await
        .expect("行锁查询");
    tx.rollback().await.expect("回滚测试事务");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0 as u64, id);
    assert!(
        (rows[0].1 - server_now).abs() <= 600,
        "投影的 server_now 应在测试时间窗口内"
    );

    #[allow(deprecated)]
    database
        .execute("DROP TABLE IF EXISTS yang_db_server_expr")
        .await
        .expect("删除测试表");
}
