#![cfg(all(feature = "mysql", feature = "postgres"))]
#![allow(deprecated)]
#![allow(clippy::expect_used)]

use serde_json::json;
use std::time::Duration;
use yang_db::{mysql, postgres};

fn mysql_url() -> String {
    std::env::var("MYSQL_TEST_URL")
        .unwrap_or_else(|_| "mysql://root:111111@localhost:3306/test".to_string())
}

fn postgres_url() -> String {
    std::env::var("PG_TEST_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5433/test".to_string())
}

#[tokio::test]
#[ignore = "需要真实 MySQL；通过 MYSQL_TEST_URL 配置"]
async fn mysql_subquery_executes_with_bound_parameters() {
    let db = mysql::Database::connect(&mysql_url())
        .await
        .expect("连接 MySQL");
    db.execute("DROP TABLE IF EXISTS p3_orders")
        .await
        .expect("清理订单表");
    db.execute("DROP TABLE IF EXISTS p3_users")
        .await
        .expect("清理用户表");
    db.execute("CREATE TABLE p3_users (id BIGINT PRIMARY KEY, tenant_id BIGINT NOT NULL)")
        .await
        .expect("创建用户表");
    db.execute("CREATE TABLE p3_orders (id BIGINT PRIMARY KEY, user_id BIGINT NOT NULL, status VARCHAR(32) NOT NULL)")
        .await
        .expect("创建订单表");
    db.execute("INSERT INTO p3_users VALUES (1, 7), (2, 7), (3, 8)")
        .await
        .expect("写入用户");
    db.execute("INSERT INTO p3_orders VALUES (10, 1, 'paid'), (11, 2, 'pending'), (12, 3, 'paid')")
        .await
        .expect("写入订单");

    let paid_order = mysql::Subquery::new("p3_orders", "id")
        .expect("合法子查询")
        .where_column("p3_orders.user_id", "=", "p3_users.id")
        .expect("合法关联")
        .where_value("p3_orders.status", "=", "paid")
        .expect("合法参数");
    let rows: Vec<(i64,)> = db
        .table(yang_db::table!("p3_users"))
        .field(yang_db::field!("id"))
        .where_and(yang_db::field!("tenant_id"), yang_db::CompareOp::Eq, 7)
        .where_exists(paid_order)
        .select()
        .await
        .expect("执行 EXISTS");
    assert_eq!(rows, vec![(1,)]);
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL；通过 PG_TEST_URL 配置"]
async fn postgres_subquery_executes_with_numbered_bound_parameters() {
    let db = postgres::Database::connect(&postgres_url())
        .await
        .expect("连接 PostgreSQL");
    db.execute("DROP TABLE IF EXISTS p3_orders")
        .await
        .expect("清理订单表");
    db.execute("DROP TABLE IF EXISTS p3_users")
        .await
        .expect("清理用户表");
    db.execute("CREATE TABLE p3_users (id BIGINT PRIMARY KEY, tenant_id BIGINT NOT NULL)")
        .await
        .expect("创建用户表");
    db.execute("CREATE TABLE p3_orders (id BIGINT PRIMARY KEY, user_id BIGINT NOT NULL, status VARCHAR(32) NOT NULL)")
        .await
        .expect("创建订单表");
    db.execute("INSERT INTO p3_users VALUES (1, 7), (2, 7), (3, 8)")
        .await
        .expect("写入用户");
    db.execute("INSERT INTO p3_orders VALUES (10, 1, 'paid'), (11, 2, 'pending'), (12, 3, 'paid')")
        .await
        .expect("写入订单");

    let paid_order = postgres::Subquery::new("p3_orders", "id")
        .expect("合法子查询")
        .where_column("p3_orders.user_id", "=", "p3_users.id")
        .expect("合法关联")
        .where_value("p3_orders.status", "=", "paid")
        .expect("合法参数");
    let rows: Vec<(i64,)> = db
        .table(yang_db::table!("p3_users"))
        .field(yang_db::field!("id"))
        .where_and(yang_db::field!("tenant_id"), yang_db::CompareOp::Eq, 7)
        .where_exists(paid_order)
        .select()
        .await
        .expect("执行 EXISTS");
    assert_eq!(rows, vec![(1,)]);
}

#[tokio::test]
#[ignore = "需要真实 MySQL；通过 MYSQL_TEST_URL 配置"]
async fn mysql_union_all_preserves_branch_and_outer_scope() {
    let db = mysql::Database::connect(&mysql_url())
        .await
        .expect("连接 MySQL");
    db.execute("DROP TABLE IF EXISTS p3_archive")
        .await
        .expect("清理归档表");
    db.execute("DROP TABLE IF EXISTS p3_current")
        .await
        .expect("清理当前表");
    db.execute("CREATE TABLE p3_current (id BIGINT PRIMARY KEY, tenant_id BIGINT NOT NULL)")
        .await
        .expect("创建当前表");
    db.execute("CREATE TABLE p3_archive (id BIGINT PRIMARY KEY, tenant_id BIGINT NOT NULL)")
        .await
        .expect("创建归档表");
    db.execute("INSERT INTO p3_current VALUES (1, 7), (2, 8)")
        .await
        .expect("写入当前表");
    db.execute("INSERT INTO p3_archive VALUES (3, 7), (4, 7), (5, 8)")
        .await
        .expect("写入归档表");

    let archive = db
        .table(yang_db::table!("p3_archive"))
        .field(yang_db::field!("id"))
        .where_and(yang_db::field!("tenant_id"), yang_db::CompareOp::Eq, 7)
        .order(yang_db::field!("id"), yang_db::SortOrder::Desc)
        .limit(1);
    let rows: Vec<(i64,)> = db
        .table(yang_db::table!("p3_current"))
        .field(yang_db::field!("id"))
        .where_and(yang_db::field!("tenant_id"), yang_db::CompareOp::Eq, 7)
        .union_all(archive)
        .expect("输出一致")
        .order(yang_db::field!("id"), yang_db::SortOrder::Asc)
        .select()
        .await
        .expect("执行 UNION ALL");
    assert_eq!(rows, vec![(1,), (4,)]);
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL；通过 PG_TEST_URL 配置"]
async fn postgres_union_all_preserves_branch_and_outer_scope() {
    let db = postgres::Database::connect(&postgres_url())
        .await
        .expect("连接 PostgreSQL");
    db.execute("DROP TABLE IF EXISTS p3_archive")
        .await
        .expect("清理归档表");
    db.execute("DROP TABLE IF EXISTS p3_current")
        .await
        .expect("清理当前表");
    db.execute("CREATE TABLE p3_current (id BIGINT PRIMARY KEY, tenant_id BIGINT NOT NULL)")
        .await
        .expect("创建当前表");
    db.execute("CREATE TABLE p3_archive (id BIGINT PRIMARY KEY, tenant_id BIGINT NOT NULL)")
        .await
        .expect("创建归档表");
    db.execute("INSERT INTO p3_current VALUES (1, 7), (2, 8)")
        .await
        .expect("写入当前表");
    db.execute("INSERT INTO p3_archive VALUES (3, 7), (4, 7), (5, 8)")
        .await
        .expect("写入归档表");

    let archive = db
        .table(yang_db::table!("p3_archive"))
        .field(yang_db::field!("id"))
        .where_and(yang_db::field!("tenant_id"), yang_db::CompareOp::Eq, 7)
        .order(yang_db::field!("id"), yang_db::SortOrder::Desc)
        .limit(1);
    let rows: Vec<(i64,)> = db
        .table(yang_db::table!("p3_current"))
        .field(yang_db::field!("id"))
        .where_and(yang_db::field!("tenant_id"), yang_db::CompareOp::Eq, 7)
        .union_all(archive)
        .expect("输出一致")
        .order(yang_db::field!("id"), yang_db::SortOrder::Asc)
        .select()
        .await
        .expect("执行 UNION ALL");
    assert_eq!(rows, vec![(1,), (4,)]);
}

#[tokio::test]
#[ignore = "需要真实 MySQL；通过 MYSQL_TEST_URL 配置"]
async fn mysql_for_update_blocks_cancelled_wait_and_releases_on_rollback() {
    let db1 = mysql::Database::connect(&mysql_url())
        .await
        .expect("连接 MySQL 1");
    let db2 = mysql::Database::connect(&mysql_url())
        .await
        .expect("连接 MySQL 2");
    db1.execute("DROP TABLE IF EXISTS p3_lock_accounts")
        .await
        .expect("清理锁表");
    db1.execute("CREATE TABLE p3_lock_accounts (id BIGINT PRIMARY KEY, balance BIGINT NOT NULL) ENGINE=InnoDB")
        .await
        .expect("创建锁表");
    db1.execute("INSERT INTO p3_lock_accounts VALUES (1, 100)")
        .await
        .expect("写入锁表");

    let mut owner = db1.transaction().await.expect("开始持锁事务");
    let locked: Vec<(i64,)> = owner
        .select_for_update(
            db1.table(yang_db::table!("p3_lock_accounts"))
                .field(yang_db::field!("balance"))
                .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 1),
        )
        .await
        .expect("获取 FOR UPDATE");
    assert_eq!(locked, vec![(100,)]);

    let mut waiter = db2.transaction().await.expect("开始等待事务");
    let blocked = tokio::time::timeout(
        Duration::from_millis(250),
        waiter
            .table(yang_db::table!("p3_lock_accounts"))
            .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 1)
            .update(&json!({"balance": 101})),
    )
    .await;
    assert!(blocked.is_err(), "竞争更新未被行锁阻塞");
    owner.rollback().await.expect("回滚释放行锁");
    waiter.rollback().await.expect("取消后的等待事务可回滚");

    let mut retry = db2.transaction().await.expect("开始重试事务");
    assert_eq!(
        retry
            .table(yang_db::table!("p3_lock_accounts"))
            .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 1)
            .update(&json!({"balance": 102}))
            .await
            .expect("锁释放后更新"),
        1
    );
    retry.commit().await.expect("提交重试事务");
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL；通过 PG_TEST_URL 配置"]
async fn postgres_for_update_blocks_cancelled_wait_and_releases_on_rollback() {
    let db1 = postgres::Database::connect(&postgres_url())
        .await
        .expect("连接 PostgreSQL 1");
    let db2 = postgres::Database::connect(&postgres_url())
        .await
        .expect("连接 PostgreSQL 2");
    db1.execute("DROP TABLE IF EXISTS p3_lock_accounts")
        .await
        .expect("清理锁表");
    db1.execute("CREATE TABLE p3_lock_accounts (id BIGINT PRIMARY KEY, balance BIGINT NOT NULL)")
        .await
        .expect("创建锁表");
    db1.execute("INSERT INTO p3_lock_accounts VALUES (1, 100)")
        .await
        .expect("写入锁表");

    let mut owner = db1.transaction().await.expect("开始持锁事务");
    let locked: Vec<(i64,)> = owner
        .select_for_update(
            db1.table(yang_db::table!("p3_lock_accounts"))
                .field(yang_db::field!("balance"))
                .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 1),
        )
        .await
        .expect("获取 FOR UPDATE");
    assert_eq!(locked, vec![(100,)]);

    let mut waiter = db2.transaction().await.expect("开始等待事务");
    let blocked = tokio::time::timeout(
        Duration::from_millis(250),
        waiter
            .table(yang_db::table!("p3_lock_accounts"))
            .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 1)
            .update(&json!({"balance": 101})),
    )
    .await;
    assert!(blocked.is_err(), "竞争更新未被行锁阻塞");
    owner.rollback().await.expect("回滚释放行锁");
    waiter.rollback().await.expect("取消后的等待事务可回滚");

    let mut retry = db2.transaction().await.expect("开始重试事务");
    assert_eq!(
        retry
            .table(yang_db::table!("p3_lock_accounts"))
            .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 1)
            .update(&json!({"balance": 102}))
            .await
            .expect("锁释放后更新"),
        1
    );
    retry.commit().await.expect("提交重试事务");
}

#[tokio::test]
#[ignore = "需要真实 MySQL；通过 MYSQL_TEST_URL 配置"]
async fn mysql_atomic_updates_cover_negative_transaction_and_overflow() {
    let db = mysql::Database::connect(&mysql_url())
        .await
        .expect("连接 MySQL");
    db.execute("DROP TABLE IF EXISTS p3_counters")
        .await
        .expect("清理计数表");
    db.execute("CREATE TABLE p3_counters (id BIGINT PRIMARY KEY, value BIGINT NOT NULL)")
        .await
        .expect("创建计数表");
    db.execute("INSERT INTO p3_counters VALUES (1, 10), (2, 9223372036854775807)")
        .await
        .expect("写入计数");

    assert_eq!(
        db.table(yang_db::table!("p3_counters"))
            .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 1)
            .increment(yang_db::field!("value"), -3)
            .await
            .expect("负增量"),
        1
    );
    assert_eq!(
        db.table(yang_db::table!("p3_counters"))
            .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 1)
            .decrement(yang_db::field!("value"), 2)
            .await
            .expect("原子递减"),
        1
    );
    let mut tx = db.transaction().await.expect("开始事务");
    assert_eq!(
        tx.table(yang_db::table!("p3_counters"))
            .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 1)
            .increment(yang_db::field!("value"), 5)
            .await
            .expect("事务内原子增加"),
        1
    );
    tx.commit().await.expect("提交事务");
    let rows: Vec<(i64,)> = db
        .table(yang_db::table!("p3_counters"))
        .field(yang_db::field!("value"))
        .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 1)
        .select()
        .await
        .expect("读取结果");
    assert_eq!(rows, vec![(10,)]);
    assert!(db
        .table(yang_db::table!("p3_counters"))
        .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 2)
        .increment(yang_db::field!("value"), 1)
        .await
        .is_err());
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL；通过 PG_TEST_URL 配置"]
async fn postgres_atomic_updates_cover_negative_transaction_and_overflow() {
    let db = postgres::Database::connect(&postgres_url())
        .await
        .expect("连接 PostgreSQL");
    db.execute("DROP TABLE IF EXISTS p3_counters")
        .await
        .expect("清理计数表");
    db.execute("CREATE TABLE p3_counters (id BIGINT PRIMARY KEY, value BIGINT NOT NULL)")
        .await
        .expect("创建计数表");
    db.execute("INSERT INTO p3_counters VALUES (1, 10), (2, 9223372036854775807)")
        .await
        .expect("写入计数");

    assert_eq!(
        db.table(yang_db::table!("p3_counters"))
            .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 1)
            .increment(yang_db::field!("value"), -3)
            .await
            .expect("负增量"),
        1
    );
    assert_eq!(
        db.table(yang_db::table!("p3_counters"))
            .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 1)
            .decrement(yang_db::field!("value"), 2)
            .await
            .expect("原子递减"),
        1
    );
    let mut tx = db.transaction().await.expect("开始事务");
    assert_eq!(
        tx.table(yang_db::table!("p3_counters"))
            .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 1)
            .increment(yang_db::field!("value"), 5)
            .await
            .expect("事务内原子增加"),
        1
    );
    tx.commit().await.expect("提交事务");
    let rows: Vec<(i64,)> = db
        .table(yang_db::table!("p3_counters"))
        .field(yang_db::field!("value"))
        .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 1)
        .select()
        .await
        .expect("读取结果");
    assert_eq!(rows, vec![(10,)]);
    assert!(db
        .table(yang_db::table!("p3_counters"))
        .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 2)
        .increment(yang_db::field!("value"), 1)
        .await
        .is_err());
}
