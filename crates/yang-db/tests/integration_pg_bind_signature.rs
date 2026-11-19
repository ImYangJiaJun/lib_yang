#![allow(clippy::expect_used, clippy::unwrap_used)]
//! PostgreSQL 参数绑定缓存回归测试。
//!
//! sqlx 0.8 的预编译语句缓存只按 SQL 文本索引，而同一 SQL 在不同调用间可能
//! 使用不同的参数类型（NULL / TEXT / INT8 / FLOAT8）。首次 PARSE 定型后，
//! 后续 Bind 按错误类型发送二进制值，会报 22P03 / 08P01，或静默写错值。
//!
//! 本文件在单连接池上复现三条路径：`Database`、`Transaction`、`QueryBuilder`。
//! 运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55433/test \
//!   cargo test -p yang-db --no-default-features --features postgres \
//!   --test integration_pg_bind_signature -- --ignored --test-threads=1
//! ```

use serde_json::json;
use yang_db::postgres::{Database, DatabaseConfig};
use yang_db::{field, table, CompareOp};

fn test_db_url() -> String {
    std::env::var("PG_TEST_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432/test".to_string())
}

async fn connect_single() -> Database {
    let mut config = DatabaseConfig::default();
    config.max_connections = 1;
    config.min_connections = 1;
    Database::connect_with_config(&test_db_url(), config)
        .await
        .expect("PG_TEST_URL 指向的 PostgreSQL 必须可连接")
}

async fn prepare(db: &Database) {
    db.execute_with_params("CREATE SCHEMA IF NOT EXISTS bind_sig_it", vec![])
        .await
        .expect("创建 schema 应成功");
    db.execute_with_params("DROP TABLE IF EXISTS bind_sig_it.records", vec![])
        .await
        .expect("删除旧表应成功");
    db.execute_with_params(
        "CREATE TABLE bind_sig_it.records (
            id BIGSERIAL PRIMARY KEY,
            amount NUMERIC(20,2),
            ratio DOUBLE PRECISION,
            note TEXT
        )",
        vec![],
    )
    .await
    .expect("创建测试表应成功");
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn db_null_then_text_on_same_sql_does_not_reuse_typed_statement() {
    let db = connect_single().await;
    prepare(&db).await;

    let sql = "INSERT INTO bind_sig_it.records (amount, note) VALUES ($1::numeric, $2)";
    db.execute_with_params(sql, vec![json!(null), json!("first")])
        .await
        .expect("NULL 首绑应成功");
    db.execute_with_params(sql, vec![json!("12.50"), json!("second")])
        .await
        .expect("同一 SQL 再绑字符串应成功，而不是复用 numeric 定型的语句");

    let rows: Vec<(Option<String>, String)> = db
        .query_with_params(
            "SELECT amount::text, note FROM bind_sig_it.records ORDER BY id",
            vec![],
        )
        .await
        .expect("查询结果应成功");
    assert_eq!(
        rows,
        vec![
            (None, "first".to_string()),
            (Some("12.50".to_string()), "second".to_string()),
        ]
    );
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn tx_null_then_int_on_same_sql_does_not_reuse_typed_statement() {
    let db = connect_single().await;
    prepare(&db).await;

    let mut tx = db.transaction().await.expect("开启事务应成功");
    let sql = "INSERT INTO bind_sig_it.records (amount, note) VALUES ($1::numeric, $2)";
    tx.execute_with_params(sql, vec![json!(null), json!("first")])
        .await
        .expect("事务内 NULL 首绑应成功");
    tx.execute_with_params(sql, vec![json!(7i64), json!("second")])
        .await
        .expect("事务内同一 SQL 再绑整数应成功");
    tx.commit().await.expect("提交事务应成功");

    let rows: Vec<(Option<String>, String)> = db
        .query_with_params(
            "SELECT amount::text, note FROM bind_sig_it.records ORDER BY id",
            vec![],
        )
        .await
        .expect("查询结果应成功");
    assert_eq!(
        rows,
        vec![
            (None, "first".to_string()),
            (Some("7.00".to_string()), "second".to_string()),
        ]
    );
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn query_builder_int_then_float_on_same_sql_keeps_parameter_type() {
    let db = connect_single().await;
    prepare(&db).await;

    db.execute_with_params(
        "INSERT INTO bind_sig_it.records (amount, ratio, note) VALUES ($1::numeric, $2, $3)",
        vec![json!("1.50"), json!(1.5f64), json!("ratio")],
    )
    .await
    .expect("预置浮点行应成功");

    let by_int = db
        .table(table!("bind_sig_it.records"))
        .where_and(field!("ratio"), CompareOp::Eq, 1i64)
        .expect("整数条件应合法")
        .count()
        .await
        .expect("整数条件查询应成功");
    assert_eq!(by_int, 0);

    let by_float = db
        .table(table!("bind_sig_it.records"))
        .where_and(field!("ratio"), CompareOp::Eq, 1.5f64)
        .expect("浮点条件应合法")
        .count()
        .await
        .expect("浮点条件查询应成功，而不是复用 INT8 定型的语句");
    assert_eq!(by_float, 1);
}
