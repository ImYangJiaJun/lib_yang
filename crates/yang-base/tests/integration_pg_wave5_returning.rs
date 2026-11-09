#![allow(clippy::expect_used, clippy::unwrap_used)]
#![cfg(feature = "postgres")]
//! PostgreSQL Wave 5 RETURNING 契约集成测试。
//!
//! 覆盖：
//! - 多列 `ReturningTarget::Columns` 直接走 `RETURNING <列...>`（不再整行裁剪），
//!   结果列名/值/类型正确，并保留 JSON null 与 SQL NULL 的区分；
//! - 批量 `insert_batch_returning` 返回行数与输入一致且顺序稳定（含事务内变体）；
//! - 批量 `upsert_batch_returning` 冲突时返回更新后的行。
//!
//! 所有测试均标记 `#[ignore]`，默认离线套件不触库。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-base --no-default-features --features postgres \
//!   --test integration_pg_wave5_returning -- --ignored --test-threads=1
//! ```

use sqlx::postgres::{PgPool, PgPoolOptions};
use std::sync::Arc;
use std::time::Duration;
use yang_base::action::builtin::add::ReturningTarget;
use yang_base::database::{AnyDatabase, AnyPool};
use yang_base::table::{Field, Record, Table, TableDefinition, TableQuery};
use yang_db::postgres::{Database as PgDatabase, DatabaseConfig as PgDatabaseConfig};

fn test_db_url() -> String {
    std::env::var("PG_TEST_URL")
        .or_else(|_| std::env::var("YANG_POSTGRES_URL"))
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432/test".to_string())
}

async fn connect_pool() -> PgPool {
    PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&test_db_url())
        .await
        .expect("PG_TEST_URL / YANG_POSTGRES_URL 指向的 PostgreSQL 必须可连接")
}

async fn execute(pool: &PgPool, sql: &str) {
    sqlx::query(sql)
        .execute(pool)
        .await
        .unwrap_or_else(|error| panic!("执行 SQL 失败: {error}\n{sql}"));
}

fn bind(definition: &TableDefinition, roles: &[&str], pool: &PgPool) -> TableQuery {
    definition
        .bind_pool(AnyPool::Postgres(Arc::new(pool.clone())))
        .query(roles.iter().copied())
}

fn pg_database(pool: &PgPool) -> PgDatabase {
    PgDatabase::from_pool(pool.clone(), PgDatabaseConfig::default())
        .expect("默认 PG DatabaseConfig 应通过校验")
}

async fn reset_multi_column_table(pool: &PgPool) {
    execute(pool, "DROP TABLE IF EXISTS tq_pg_wave5_multi_column").await;
    execute(
        pool,
        "CREATE TABLE tq_pg_wave5_multi_column (
            id BIGSERIAL PRIMARY KEY,
            label TEXT NOT NULL,
            sort_order INTEGER NOT NULL,
            amount NUMERIC(12, 2) NOT NULL,
            payload JSONB NULL,
            note TEXT NULL,
            created_at BIGINT NOT NULL
        )",
    )
    .await;
}

fn multi_column_definition() -> TableDefinition {
    Table::new("tq_pg_wave5_multi_column")
        .fields([
            Field::id("id"),
            Field::string("label", 64).required(),
            Field::integer("sort_order").required(),
            Field::decimal("amount", 12, 2).required(),
            Field::json("payload"),
            Field::string("note", 64),
            Field::created_at("created_at"),
        ])
        .build()
        .expect("tq_pg_wave5_multi_column 表定义应有效")
}

async fn reset_batch_insert_table(pool: &PgPool) {
    execute(pool, "DROP TABLE IF EXISTS tq_pg_wave5_batch_insert").await;
    execute(
        pool,
        "CREATE TABLE tq_pg_wave5_batch_insert (
            id BIGSERIAL PRIMARY KEY,
            label TEXT NOT NULL,
            sort_order INTEGER NOT NULL,
            created_at BIGINT NOT NULL
        )",
    )
    .await;
}

fn batch_insert_definition() -> TableDefinition {
    Table::new("tq_pg_wave5_batch_insert")
        .fields([
            Field::id("id"),
            Field::string("label", 64).required(),
            Field::integer("sort_order").required(),
            Field::created_at("created_at"),
        ])
        .build()
        .expect("tq_pg_wave5_batch_insert 表定义应有效")
}

fn batch_insert_rows() -> Vec<Record> {
    (1..=3)
        .map(|index| {
            Record::new()
                .set("label", format!("row-{index}"))
                .set("sort_order", index)
        })
        .collect()
}

async fn reset_batch_upsert_table(pool: &PgPool) {
    execute(pool, "DROP TABLE IF EXISTS tq_pg_wave5_batch_upsert").await;
    execute(
        pool,
        "CREATE TABLE tq_pg_wave5_batch_upsert (
            id BIGSERIAL PRIMARY KEY,
            option_key TEXT NOT NULL UNIQUE,
            label TEXT NOT NULL,
            sort_order INTEGER NOT NULL,
            created_at BIGINT NOT NULL
        )",
    )
    .await;
}

fn batch_upsert_definition() -> TableDefinition {
    Table::new("tq_pg_wave5_batch_upsert")
        .fields([
            Field::id("id"),
            Field::string("option_key", 64).required(),
            Field::string("label", 64).required(),
            Field::integer("sort_order").required(),
            Field::created_at("created_at"),
        ])
        .build()
        .expect("tq_pg_wave5_batch_upsert 表定义应有效")
}

fn batch_upsert_rows(suffix: &str, sort_order: i32) -> Vec<Record> {
    ["a", "b"]
        .into_iter()
        .map(|key| {
            Record::new()
                .set("option_key", key)
                .set("label", format!("{key}-{suffix}"))
                .set("sort_order", sort_order)
        })
        .collect()
}

/// 多列 `Columns` 直接走 `RETURNING <列...>`：结果只含点名列，列名/值/顺序对应的
/// 类型正确；jsonb 字面量 null 与 SQL NULL 仍可区分。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn multi_column_returning_keeps_names_types_and_null_kinds() {
    let pool = connect_pool().await;
    reset_multi_column_table(&pool).await;
    let definition = multi_column_definition();

    let returned = bind(&definition, &["admin"], &pool)
        .insert_returning(
            Record::new()
                .set("label", "multi")
                .set("sort_order", 7)
                .set("amount", "12.50")
                .set("payload", serde_json::Value::Null),
            ReturningTarget::Columns(&["id", "label", "sort_order", "amount", "payload", "note"]),
        )
        .await
        .expect("多列 RETURNING 应成功");

    let mut keys: Vec<&str> = returned.as_map().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["amount", "id", "label", "note", "payload", "sort_order"],
        "只应返回点名列，列名不得被改写: {returned:?}"
    );

    assert!(
        returned
            .get("id")
            .and_then(|value| value.as_u64())
            .unwrap_or(0)
            > 0,
        "id 应按整型返回: {returned:?}"
    );
    assert_eq!(
        returned.get("label").and_then(|value| value.as_str()),
        Some("multi")
    );
    assert_eq!(
        returned.get("sort_order").and_then(|value| value.as_i64()),
        Some(7),
        "integer 列应按数值返回"
    );
    let amount = returned
        .get("amount")
        .and_then(|value| value.as_str())
        .expect("NUMERIC 应保留十进制精度并以字符串返回");
    assert!(
        amount.starts_with("12.5"),
        "NUMERIC 值应按真实精度返回，实际: {amount}"
    );

    assert!(
        returned.is_json_null("payload"),
        "jsonb 字面量 null 必须登记为 JSON null: {returned:?}"
    );
    assert!(!returned.is_sql_null("payload"));
    assert!(
        returned.is_sql_null("note"),
        "未提供的 nullable 列应为 SQL NULL: {returned:?}"
    );
    assert!(!returned.is_json_null("note"));
}

/// 批量 insert RETURNING：行数与输入一致，且 `RETURNING` 顺序与输入顺序一致。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn batch_insert_returning_returns_rows_in_input_order() {
    let pool = connect_pool().await;
    reset_batch_insert_table(&pool).await;
    let definition = batch_insert_definition();
    let rows = batch_insert_rows();

    let empty = bind(&definition, &["admin"], &pool)
        .insert_batch_returning(&[], ReturningTarget::Pk)
        .await
        .expect("空批应返回空向量");
    assert!(empty.is_empty(), "空批不应生成语句: {empty:?}");

    let columns = bind(&definition, &["admin"], &pool)
        .insert_batch_returning(&rows, ReturningTarget::Columns(&["label", "sort_order"]))
        .await
        .expect("批量 insert RETURNING 应成功");
    assert_eq!(columns.len(), rows.len(), "返回行数必须与输入一致");
    let labels: Vec<&str> = columns
        .iter()
        .map(|record| {
            record
                .get("label")
                .and_then(|value| value.as_str())
                .unwrap_or("")
        })
        .collect();
    assert_eq!(labels, vec!["row-1", "row-2", "row-3"], "必须保持输入顺序");
    let sort_orders: Vec<i64> = columns
        .iter()
        .map(|record| {
            record
                .get("sort_order")
                .and_then(|value| value.as_i64())
                .unwrap_or(-1)
        })
        .collect();
    assert_eq!(sort_orders, vec![1, 2, 3], "列值与输入一一对应");

    let primary_keys = bind(&definition, &["admin"], &pool)
        .insert_batch_returning(&batch_insert_rows(), ReturningTarget::Pk)
        .await
        .expect("批量 insert RETURNING(Pk) 应成功");
    assert_eq!(primary_keys.len(), 3);
    let ids: Vec<u64> = primary_keys
        .iter()
        .map(|record| {
            record
                .get("id")
                .and_then(|value| value.as_u64())
                .unwrap_or(0)
        })
        .collect();
    assert!(
        ids.windows(2).all(|pair| pair[0] < pair[1]),
        "RETURNING 主键应按输入顺序递增: {ids:?}"
    );
}

/// 事务内批量 insert RETURNING：提交后行数与顺序同样稳定。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn batch_insert_returning_in_tx_commits_in_input_order() {
    let pool = connect_pool().await;
    reset_batch_insert_table(&pool).await;
    let definition = batch_insert_definition();
    let database = AnyDatabase::postgres(pg_database(&pool));

    let mut tx = database.begin().await.expect("开启 PG 事务应成功");
    let returned = bind(&definition, &["admin"], &pool)
        .insert_batch_returning_in_tx(
            &mut tx,
            &batch_insert_rows(),
            ReturningTarget::Columns(&["label"]),
        )
        .await
        .expect("事务内批量 insert RETURNING 应成功");
    tx.commit().await.expect("提交应成功");

    let labels: Vec<&str> = returned
        .iter()
        .map(|record| {
            record
                .get("label")
                .and_then(|value| value.as_str())
                .unwrap_or("")
        })
        .collect();
    assert_eq!(labels, vec!["row-1", "row-2", "row-3"], "必须保持输入顺序");

    let persisted: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tq_pg_wave5_batch_insert")
        .fetch_one(&pool)
        .await
        .expect("读取行数应成功");
    assert_eq!(persisted, 3);
}

/// 批量 upsert RETURNING：冲突时返回的是更新后的行，顺序稳定。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn batch_upsert_returning_returns_updated_rows() {
    let pool = connect_pool().await;
    reset_batch_upsert_table(&pool).await;
    let definition = batch_upsert_definition();
    let conflicts = ["option_key"];

    bind(&definition, &["admin"], &pool)
        .upsert_batch(batch_upsert_rows("v1", 1), &conflicts)
        .await
        .expect("首轮 upsert 应成功");

    let returned = bind(&definition, &["admin"], &pool)
        .upsert_batch_returning(
            batch_upsert_rows("v2", 2),
            &conflicts,
            ReturningTarget::Columns(&["option_key", "label", "sort_order"]),
        )
        .await
        .expect("批量 upsert RETURNING 应成功");

    assert_eq!(returned.len(), 2, "两个冲突键都应返回更新后的行");
    let pairs: Vec<(&str, &str, i64)> = returned
        .iter()
        .map(|record| {
            (
                record
                    .get("option_key")
                    .and_then(|value| value.as_str())
                    .unwrap_or(""),
                record
                    .get("label")
                    .and_then(|value| value.as_str())
                    .unwrap_or(""),
                record
                    .get("sort_order")
                    .and_then(|value| value.as_i64())
                    .unwrap_or(-1),
            )
        })
        .collect();
    assert_eq!(
        pairs,
        vec![("a", "a-v2", 2), ("b", "b-v2", 2)],
        "应返回冲突更新后的行并保持输入顺序: {returned:?}"
    );

    let (label, sort_order): (String, i32) = sqlx::query_as(
        "SELECT label, sort_order FROM tq_pg_wave5_batch_upsert WHERE option_key = 'a'",
    )
    .fetch_one(&pool)
    .await
    .expect("读取更新后的行应成功");
    assert_eq!(label, "a-v2");
    assert_eq!(sort_order, 2);
}
