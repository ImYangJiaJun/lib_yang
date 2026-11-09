#![cfg(feature = "postgres")]
#![allow(clippy::expect_used)]
//! PostgreSQL Wave 4 类型层集成测试（真实库）。
//!
//! 覆盖：
//! - `FieldType::Uuid` / `Bytes` / 标量数组的建表与二次同步 no-op；
//! - 插入与 `Record` 读回：UUID → 字符串、BYTEA → Base64、数组 → JSON 数组；
//! - 数组内 NULL 元素与整行 NULL；
//! - VARCHAR(100000)（超过 MySQL 上界）在 PostgreSQL 建表成功。
//!
//! 所有测试均标记 `#[ignore]`，默认离线套件不触库。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-base --no-default-features --features postgres \
//!   --test integration_pg_wave4_types -- --ignored --test-threads=1
//! ```

use serde_json::json;
use std::sync::Arc;
use yang_base::database::{AnyDatabase, AnyPool, DatabaseInitializer, SchemaSyncChangeKind};
use yang_base::table::{Field, FieldType, Record, Table, TableDefinition};
use yang_db::postgres::Database;

fn test_db_url() -> String {
    std::env::var("PG_TEST_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432/test".to_string())
}

async fn connect() -> Database {
    Database::connect(&test_db_url())
        .await
        .expect("PG_TEST_URL 指向的 PostgreSQL 必须可连接")
}

async fn reset_tables(database: &Database, tables: &[&str]) {
    for table in tables {
        database
            .execute_with_params(&format!("DROP TABLE IF EXISTS \"{table}\" CASCADE"), vec![])
            .await
            .expect("清理测试表应成功");
    }
}

fn types_definition() -> TableDefinition {
    Table::new("pg_wave4_types")
        .fields([
            Field::id("id"),
            Field::uuid("uid"),
            Field::bytes("payload"),
            Field::array("numbers", FieldType::Integer),
            Field::array("tags", FieldType::Text),
        ])
        .build()
        .expect("pg_wave4_types 表定义应有效")
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave4_types_create_sync_noop_and_readback() {
    let database = connect().await;
    reset_tables(&database, &["pg_wave4_types"]).await;
    let pool = database.pool().clone();
    let initializer = DatabaseInitializer::from_any(AnyDatabase::postgres(database));
    let definition = types_definition();

    let created = initializer
        .sync_table_definitions(&[&definition])
        .await
        .expect("首次建表应成功");
    assert_eq!(created.changes.len(), 1);
    assert_eq!(created.changes[0].kind, SchemaSyncChangeKind::CreatedTable);
    assert!(initializer
        .validate_table_definition(&definition)
        .await
        .expect("同步后应能读取列")
        .is_compatible());

    let second = initializer
        .sync_table_definitions(&[&definition])
        .await
        .expect("二次同步应成功");
    assert!(second.is_noop(), "二次同步必须幂等: {second:?}");

    // 一行覆盖全部类型与数组内 NULL，另一行整行 NULL。
    sqlx::query(
        "INSERT INTO pg_wave4_types (uid, payload, numbers, tags) VALUES \
         ('11111111-1111-1111-1111-111111111111', \
          '\\xdeadbeef'::bytea, \
          ARRAY[1, 2, NULL]::int4[], \
          ARRAY['a', NULL, 'c']::text[]), \
         (NULL, NULL, NULL, NULL)",
    )
    .execute(&pool)
    .await
    .expect("插入测试行应成功");

    let rows: Vec<Record> =
        sqlx::query_as("SELECT id, uid, payload, numbers, tags FROM pg_wave4_types ORDER BY id")
            .fetch_all(&pool)
            .await
            .expect("读取 Record 应成功");
    assert_eq!(rows.len(), 2);

    let row = &rows[0];
    assert_eq!(
        row.get("uid"),
        Some(&json!("11111111-1111-1111-1111-111111111111")),
        "UUID 必须解码为连字符小写字符串"
    );
    assert_eq!(
        row.get("payload"),
        Some(&json!("3q2+7w==")),
        "BYTEA 必须解码为 Base64 字符串"
    );
    assert_eq!(
        row.get("numbers"),
        Some(&json!([1, 2, null])),
        "int4[] 必须解码为 JSON 数组且保留 NULL 元素"
    );
    assert_eq!(
        row.get("tags"),
        Some(&json!(["a", null, "c"])),
        "text[] 必须解码为 JSON 数组且保留 NULL 元素"
    );

    let null_row = &rows[1];
    assert!(null_row.is_sql_null("uid"));
    assert!(null_row.is_sql_null("payload"));
    assert!(null_row.is_sql_null("numbers"));
    assert!(null_row.is_sql_null("tags"));
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave4_varchar_above_mysql_limit_creates_table() {
    let database = connect().await;
    reset_tables(&database, &["pg_wave4_varchar"]).await;
    let pool = database.pool().clone();
    let initializer = DatabaseInitializer::from_any(AnyDatabase::postgres(database));
    let definition = Table::new("pg_wave4_varchar")
        .fields([Field::id("id"), Field::string("body", 100_000)])
        .build()
        .expect("PostgreSQL 构建应接受 VARCHAR(100000)");

    let created = initializer
        .sync_table_definitions(&[&definition])
        .await
        .expect("PostgreSQL 应能创建 VARCHAR(100000)");
    assert_eq!(created.changes.len(), 1);
    assert_eq!(created.changes[0].kind, SchemaSyncChangeKind::CreatedTable);

    let second = initializer
        .sync_table_definitions(&[&definition])
        .await
        .expect("二次同步应成功");
    assert!(second.is_noop(), "二次同步必须幂等: {second:?}");

    let max_length: Option<i64> = sqlx::query_scalar(
        "SELECT character_maximum_length::bigint FROM information_schema.columns \
         WHERE table_schema = current_schema() AND table_name = 'pg_wave4_varchar' \
           AND column_name = 'body'",
    )
    .fetch_one(&pool)
    .await
    .expect("读取列长度应成功");
    assert_eq!(max_length, Some(100_000));
}

/// 数组写入（空数组 / 全 NULL / bytea[] / uuid[] / text[] / int4[]）与数组等值谓词。
///
/// 缺陷回归：空数组与全 NULL 数组曾退化为 `TEXT[]` 绑定（42804）；整数数组谓词
/// 曾以 `int8[]` 绑定 `int4[]` 列（42883）。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave4_array_writes_and_equality_predicates() {
    let database = connect().await;
    let table = format!("pg_wave4_array_q_{}", std::process::id());
    reset_tables(&database, &[table.as_str()]).await;
    let pool = database.pool().clone();
    let initializer = DatabaseInitializer::from_any(AnyDatabase::postgres(database));
    let definition = Table::new(&table)
        .fields([
            Field::id("id"),
            Field::string("label", 32).required(),
            Field::array("numbers", FieldType::Integer),
            Field::array("tags", FieldType::Text),
            Field::array("payloads", FieldType::Bytes),
            Field::array("uids", FieldType::Uuid),
            Field::array("flags", FieldType::Boolean),
        ])
        .build()
        .expect("数组表定义应有效");
    initializer
        .sync_table_definitions(&[&definition])
        .await
        .expect("数组表建表应成功");

    let query = |roles: &[&str]| {
        definition
            .bind_pool(AnyPool::Postgres(Arc::new(pool.clone())))
            .query(roles.iter().copied())
    };

    // 空数组：所有数组列都走无元素类型线索的绑定路径。
    query(&["admin"])
        .insert(
            Record::new()
                .set("label", "empty")
                .set("numbers", json!([]))
                .set("tags", json!([]))
                .set("payloads", json!([]))
                .set("uids", json!([]))
                .set("flags", json!([])),
        )
        .await
        .expect("空数组写入应成功");

    // 全 NULL 数组：保留元素个数且不能退化为 TEXT[]。
    query(&["admin"])
        .insert(
            Record::new()
                .set("label", "nulls")
                .set("numbers", json!([null, null]))
                .set("tags", json!([null]))
                .set("payloads", json!([null, null]))
                .set("uids", json!([null]))
                .set("flags", json!([null])),
        )
        .await
        .expect("全 NULL 数组写入应成功");

    // 含真实元素的数组：bytea[] 必须绑定真实 byte[] 而不是 Base64 文本。
    query(&["admin"])
        .insert(
            Record::new()
                .set("label", "typed")
                .set("numbers", json!([1, 2, null]))
                .set("tags", json!(["a", null]))
                .set("payloads", json!(["3q2+7w==", null]))
                .set(
                    "uids",
                    json!(["22222222-2222-2222-2222-222222222222", null]),
                )
                .set("flags", json!([true, false, null])),
        )
        .await
        .expect("带元素数组写入应成功");

    // int4[] 等值谓词（曾因 int8[] 绑定报 42883）。
    let found = query(&["admin"])
        .where_eq("numbers", json!([1, 2, null]))
        .expect("int4[] 等值条件应通过校验")
        .all()
        .await
        .expect("int4[] 等值查询应成功");
    assert_eq!(found.len(), 1, "int4[] 等值应命中唯一行");
    assert_eq!(found[0].get("label"), Some(&json!("typed")));

    // 空数组与全 NULL 数组的等值谓词。
    for (label, value) in [("empty", json!([])), ("nulls", json!([null, null]))] {
        let found = query(&["admin"])
            .where_eq("numbers", value)
            .expect("int4[] 等值条件应通过校验")
            .all()
            .await
            .expect("int4[] 等值查询应成功");
        assert_eq!(found.len(), 1, "numbers = {label} 应命中唯一行");
        assert_eq!(found[0].get("label"), Some(&json!(label)));
    }

    // bytea[] / uuid[] / text[] 等值谓词。
    let found = query(&["admin"])
        .where_eq("payloads", json!(["3q2+7w==", null]))
        .expect("bytea[] 等值条件应通过校验")
        .all()
        .await
        .expect("bytea[] 等值查询应成功");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].get("label"), Some(&json!("typed")));

    let found = query(&["admin"])
        .where_eq(
            "uids",
            json!(["22222222-2222-2222-2222-222222222222", null]),
        )
        .expect("uuid[] 等值条件应通过校验")
        .all()
        .await
        .expect("uuid[] 等值查询应成功");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].get("label"), Some(&json!("typed")));

    let found = query(&["admin"])
        .where_eq("tags", json!(["a", null]))
        .expect("text[] 等值条件应通过校验")
        .all()
        .await
        .expect("text[] 等值查询应成功");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].get("label"), Some(&json!("typed")));

    // boolean[]：空数组 / 全 NULL / 含真实元素都必须按 bool[] 绑定并原样读回。
    for (label, expected) in [
        ("empty", json!([])),
        ("nulls", json!([null])),
        ("typed", json!([true, false, null])),
    ] {
        let rows = query(&["admin"])
            .where_eq("label", json!(label))
            .expect("label 等值条件应通过校验")
            .all()
            .await
            .expect("读取 boolean[] 行应成功");
        assert_eq!(rows.len(), 1, "label = {label} 应命中唯一行");
        assert_eq!(
            rows[0].get("flags"),
            Some(&expected),
            "boolean[] 读回应与写入一致"
        );
    }

    // boolean[] 等值谓词：含真实元素时命中，含 NULL 但不存在的组合不报错且不命中。
    let found = query(&["admin"])
        .where_eq("flags", json!([true, false, null]))
        .expect("boolean[] 等值条件应通过校验")
        .all()
        .await
        .expect("boolean[] 等值查询应成功");
    assert_eq!(found.len(), 1, "boolean[] 等值应命中唯一行");
    assert_eq!(found[0].get("label"), Some(&json!("typed")));

    let found = query(&["admin"])
        .where_eq("flags", json!([true, null]))
        .expect("boolean[] 含 NULL 等值条件应通过校验")
        .all()
        .await
        .expect("boolean[] 含 NULL 等值查询应成功");
    assert!(found.is_empty(), "boolean[] = [true, NULL] 不应命中任何行");

    reset_tables(&connect().await, &[table.as_str()]).await;
}
