#![cfg(feature = "postgres")]
#![allow(clippy::expect_used, clippy::unwrap_used)]
//! PostgreSQL Wave 4 编解码与唯一索引空值语义真库集成测试。
//!
//! 覆盖：
//! - 内置 range（int4range / int8range / numrange / daterange / tstzrange）解码，
//!   含无界端点与空 range；
//! - 自定义 OID 解码器注册表（composite 文本协议解析、未注册类型的可操作错误）；
//! - PostgreSQL 15+ `NULLS NOT DISTINCT` 唯一索引：建表、二次 no-op、空值互不异。
//!
//! 所有测试标记 `#[ignore]`。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-base --no-default-features --features postgres \
//!   --test integration_pg_wave4_codec -- --ignored --test-threads=1
//! ```

use serde_json::json;
use sqlx::postgres::PgPool;
use sqlx::FromRow;
use yang_base::database::{AnyDatabase, DatabaseInitializer, SchemaSyncChangeKind};
use yang_base::error::BaseError;
use yang_base::table::{register_pg_type_decoder, Field, PgValue, Record, Table};
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

async fn connect_pool() -> PgPool {
    PgPool::connect(&test_db_url())
        .await
        .expect("PG_TEST_URL 指向的 PostgreSQL 必须可连接")
}

fn initializer(database: Database) -> DatabaseInitializer {
    DatabaseInitializer::from_any(AnyDatabase::postgres(database))
}

/// 进程级唯一名字：多个进程并行跑同一真库时不会互相 DROP/CREATE。
fn unique_name(base: &str) -> String {
    format!("{base}_{}", std::process::id())
}

async fn reset_tables(database: &Database, tables: &[&str]) {
    for table in tables {
        database
            .execute_with_params(&format!("DROP TABLE IF EXISTS \"{table}\" CASCADE"), vec![])
            .await
            .expect("清理测试表应成功");
    }
}

/// 内置 range 的 JSON 契约：端点/闭区间标志，空 range 显式 `{"empty":true}`。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave4_ranges_decode_to_documented_json() {
    let pool = connect_pool().await;

    let record = sqlx::query_as::<_, Record>(
        r#"SELECT
             '[1,5)'::int4range AS i4,
             '[10,20]'::int8range AS i8,
             '[,10.50]'::numrange AS num,
             '["2026-01-01","2026-02-01")'::daterange AS d,
             '["2026-01-01 00:00:00+00","2026-01-02 00:00:00+00")'::tstzrange AS tstz,
             'empty'::int4range AS empty_r,
             '[1,)'::int4range AS upper_unbounded"#,
    )
    .fetch_one(&pool)
    .await
    .expect("内置 range 必须可解码");

    assert_eq!(
        record.get("i4"),
        Some(&json!({
            "lower": 1,
            "upper": 5,
            "lower_inclusive": true,
            "upper_inclusive": false,
        }))
    );
    // int8range 是离散 range，服务端会把 `[10,20]` 规范化为 `[10,21)`。
    assert_eq!(
        record.get("i8"),
        Some(&json!({
            "lower": 10,
            "upper": 21,
            "lower_inclusive": true,
            "upper_inclusive": false,
        }))
    );
    // NUMERIC 端点按服务端二进制 scale 输出，测试只校验数值语义。
    let num = record.get("num").expect("numrange 应存在");
    assert_eq!(num["lower"], json!(null));
    assert_eq!(num["lower_inclusive"], json!(false));
    assert_eq!(num["upper_inclusive"], json!(true));
    assert_eq!(
        num["upper"]
            .as_str()
            .and_then(|upper| upper.parse::<f64>().ok()),
        Some(10.5),
        "numrange 上界应解码为十进制文本: {num}"
    );
    assert_eq!(
        record.get("d"),
        Some(&json!({
            "lower": "2026-01-01",
            "upper": "2026-02-01",
            "lower_inclusive": true,
            "upper_inclusive": false,
        }))
    );
    assert_eq!(
        record.get("tstz"),
        Some(&json!({
            "lower": "2026-01-01T00:00:00Z",
            "upper": "2026-01-02T00:00:00Z",
            "lower_inclusive": true,
            "upper_inclusive": false,
        }))
    );
    assert_eq!(record.get("empty_r"), Some(&json!({"empty": true})));
    assert_eq!(
        record.get("upper_unbounded"),
        Some(&json!({
            "lower": 1,
            "upper": null,
            "lower_inclusive": true,
            "upper_inclusive": false,
        }))
    );
}

/// 文本协议下的空 range：原始值是字面量 `empty`，必须直接解出 `{"empty": true}`。
///
/// 缺陷回归：此前仍调用 `PgRange<T>` 解码，对 `empty` 会报 ParseIntError。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave4_empty_ranges_decode_in_text_protocol() {
    let pool = connect_pool().await;
    // `raw_sql` 走文本协议，覆盖 `empty` 字面量路径（二进制路径由上一个用例覆盖）。
    let row = sqlx::raw_sql(
        "SELECT 'empty'::int4range AS i4, 'empty'::int8range AS i8, \
                'empty'::numrange AS num, 'empty'::daterange AS d, \
                'empty'::tstzrange AS tstz",
    )
    .fetch_one(&pool)
    .await
    .expect("文本协议查询应成功");
    let record = Record::from_row(&row).expect("文本协议空 range 必须可解码");
    for column in ["i4", "i8", "num", "d", "tstz"] {
        assert_eq!(
            record.get(column),
            Some(&json!({"empty": true})),
            "空 range 列 {column} 必须解码为 {{\"empty\": true}}"
        );
    }
}

/// 注册 composite 解码器后，文本协议结果（`sqlx::raw_sql`）可解码成功。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave4_registered_decoder_decodes_composite_text_value() {
    let pool = connect_pool().await;
    sqlx::query("CREATE SCHEMA IF NOT EXISTS wave4_codec_it")
        .execute(&pool)
        .await
        .expect("创建 schema 应成功");
    sqlx::query("DROP TYPE IF EXISTS wave4_codec_it.pair CASCADE")
        .execute(&pool)
        .await
        .expect("清理旧复合类型应成功");
    sqlx::query("CREATE TYPE wave4_codec_it.pair AS (a integer, b text)")
        .execute(&pool)
        .await
        .expect("创建复合类型应成功");

    let oid: i64 = sqlx::query_scalar("SELECT 'wave4_codec_it.pair'::regtype::oid::bigint")
        .fetch_one(&pool)
        .await
        .expect("读取复合类型 OID 应成功");
    let oid = u32::try_from(oid).expect("OID 应落在 u32 范围");

    /// 文本协议下 composite 形如 `(1,x)`；这里做最小的逗号切分。
    fn parse_pair(value: &PgValue<'_>) -> Result<serde_json::Value, BaseError> {
        let text = value
            .as_str()
            .ok_or_else(|| BaseError::ConfigError("pair 解码器仅支持文本协议结果".to_string()))?;
        let inner = text
            .strip_prefix('(')
            .and_then(|text| text.strip_suffix(')'))
            .ok_or_else(|| BaseError::ConfigError(format!("无法解析 composite 文本: {text}")))?;
        let (a, b) = inner
            .split_once(',')
            .ok_or_else(|| BaseError::ConfigError(format!("无法解析 composite 文本: {text}")))?;
        let a = a
            .trim()
            .parse::<i64>()
            .map_err(|error| BaseError::ConfigError(format!("pair.a 不是整数: {error}")))?;
        Ok(json!({ "a": a, "b": b }))
    }

    register_pg_type_decoder(oid, parse_pair);

    // `raw_sql` 走文本协议，自定义 OID 以 DeclareWithOid 暴露；注册表据此分派。
    let row = sqlx::raw_sql("SELECT ROW(1, 'x')::wave4_codec_it.pair AS p")
        .fetch_one(&pool)
        .await
        .expect("文本协议查询应成功");
    let record = Record::from_row(&row).expect("注册解码器后 composite 应解码成功");
    assert_eq!(record.get("p"), Some(&json!({"a": 1, "b": "x"})));

    // 未注册的未知 OID 仍必须明确报错，且错误信息可操作。
    sqlx::query("DROP TYPE IF EXISTS wave4_codec_it.unknown_pair CASCADE")
        .execute(&pool)
        .await
        .expect("清理旧复合类型应成功");
    sqlx::query("CREATE TYPE wave4_codec_it.unknown_pair AS (a integer, b text)")
        .execute(&pool)
        .await
        .expect("创建未注册复合类型应成功");
    let error = sqlx::query_as::<_, Record>("SELECT ROW(2, 'y')::wave4_codec_it.unknown_pair AS p")
        .fetch_one(&pool)
        .await
        .expect_err("未注册的 composite 必须返回结构化错误");
    let message = error.to_string();
    assert!(
        message.contains("unknown_pair"),
        "错误应包含类型名: {message}"
    );
    assert!(
        message.contains("register_pg_type_decoder"),
        "错误应给出注册解码器的可操作提示: {message}"
    );

    sqlx::query("DROP SCHEMA wave4_codec_it CASCADE")
        .execute(&pool)
        .await
        .expect("清理 schema 应成功");
}

/// `NULLS NOT DISTINCT`：首次建表、二次 no-op、两行 NULL 被唯一约束拒绝。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave4_nulls_not_distinct_unique_index_is_enforced() {
    let database = connect().await;
    let table = unique_name("wave4_codec_nnd");
    reset_tables(&database, &[table.as_str()]).await;
    let initializer = initializer(connect().await);

    let definition = Table::new(&table)
        .fields([Field::id("id"), Field::string("code", 32)])
        .unique_nulls_not_distinct(["code"])
        .build()
        .expect("NULLS NOT DISTINCT 表定义应有效");

    let created = initializer
        .sync_table_definitions(&[&definition])
        .await
        .expect("首次建表应成功");
    assert_eq!(created.changes.len(), 1);
    assert_eq!(created.changes[0].kind, SchemaSyncChangeKind::CreatedTable);
    assert!(
        created
            .statements
            .iter()
            .any(|statement| statement.contains("NULLS NOT DISTINCT")),
        "建表 DDL 必须携带 NULLS NOT DISTINCT: {:?}",
        created.statements
    );

    let second = initializer
        .sync_table_definitions(&[&definition])
        .await
        .expect("二次同步应成功");
    assert!(second.is_noop(), "二次同步必须幂等: {second:?}");

    let index_name = format!("uk_{table}_code");
    let reflected: Option<bool> = sqlx::query_scalar(
        "SELECT indnullsnotdistinct FROM pg_index WHERE indexrelid = to_regclass($1)",
    )
    .bind(&index_name)
    .fetch_optional(database.pool())
    .await
    .expect("读取 pg_index.indnullsnotdistinct 应成功");
    assert_eq!(reflected, Some(true), "反射必须回读 NULLS NOT DISTINCT");

    // 第一行 NULL 可插入，第二行 NULL 必须触发唯一冲突。
    database
        .execute_with_params(
            &format!("INSERT INTO \"{table}\" (\"code\") VALUES (NULL)"),
            vec![],
        )
        .await
        .expect("第一行 NULL 应可插入");
    let duplicate = database
        .execute_with_params(
            &format!("INSERT INTO \"{table}\" (\"code\") VALUES (NULL)"),
            vec![],
        )
        .await;
    assert!(
        duplicate.is_err(),
        "NULLS NOT DISTINCT 下第二个 NULL 必须被唯一约束拒绝"
    );

    reset_tables(&database, &[table.as_str()]).await;
}
