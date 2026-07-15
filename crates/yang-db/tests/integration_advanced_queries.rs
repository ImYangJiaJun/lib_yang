#![cfg(all(feature = "mysql", feature = "postgres"))]
#![allow(deprecated)]
#![allow(clippy::expect_used)]

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
        .table("p3_users")
        .field_identifier("id")
        .expect("合法投影")
        .where_and("tenant_id", "=", 7)
        .expect("合法条件")
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
        .table("p3_users")
        .field_identifier("id")
        .expect("合法投影")
        .where_and("tenant_id", "=", 7)
        .expect("合法条件")
        .where_exists(paid_order)
        .select()
        .await
        .expect("执行 EXISTS");
    assert_eq!(rows, vec![(1,)]);
}
