#![allow(clippy::expect_used, clippy::unwrap_used)]
#![cfg(feature = "postgres")]
//! PostgreSQL `Record` 动态解码集成测试。
//!
//! 覆盖需求列出的全部类型族：布尔 / 整数 / 浮点 / NUMERIC / 字符串族 / BYTEA /
//! 日期时间族 / INTERVAL / UUID / JSON(JSONB) / 数组 / INET / CIDR / ENUM，
//! 以及 SQL NULL 与空字符串、JSONB 字面量 null、重复列名、未知/自定义类型错误路径。
//!
//! 所有测试均标记 `#[ignore]`，默认离线套件不触库。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-base --no-default-features --features postgres \
//!   --test integration_record_pg -- --ignored --test-threads=1
//! ```

use serde_json::{json, Value};
use sqlx::postgres::PgPool;
use yang_base::table::Record;

fn test_db_url() -> String {
    std::env::var("PG_TEST_URL")
        .or_else(|_| std::env::var("YANG_POSTGRES_URL"))
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432/test".to_string())
}

async fn connect() -> PgPool {
    PgPool::connect(&test_db_url())
        .await
        .expect("PG_TEST_URL / YANG_POSTGRES_URL 指向的 PostgreSQL 必须可连接")
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn record_decodes_all_supported_types() {
    let pool = connect().await;

    sqlx::query("CREATE SCHEMA IF NOT EXISTS record_pg_it")
        .execute(&pool)
        .await
        .expect("创建 schema 应成功");
    sqlx::query("DROP TABLE IF EXISTS record_pg_it.all_types")
        .execute(&pool)
        .await
        .expect("删除旧表应成功");
    sqlx::query("DROP TYPE IF EXISTS record_pg_it.mood")
        .execute(&pool)
        .await
        .expect("删除旧枚举应成功");
    sqlx::query("CREATE TYPE record_pg_it.mood AS ENUM ('happy', 'sad')")
        .execute(&pool)
        .await
        .expect("创建枚举类型应成功");
    sqlx::query(
        r#"CREATE TABLE record_pg_it.all_types (
            id BIGSERIAL PRIMARY KEY,
            b BOOLEAN NOT NULL,
            i2 SMALLINT NOT NULL,
            i4 INTEGER NOT NULL,
            i8 BIGINT NOT NULL,
            f4 REAL NOT NULL,
            f8 DOUBLE PRECISION NOT NULL,
            num NUMERIC(20,4) NOT NULL,
            txt TEXT NOT NULL,
            vc VARCHAR(32) NOT NULL,
            bp CHAR(4) NOT NULL,
            nm NAME NOT NULL,
            bin BYTEA NOT NULL,
            d DATE NOT NULL,
            t TIME NOT NULL,
            ttz TIMETZ NOT NULL,
            ts TIMESTAMP NOT NULL,
            tstz TIMESTAMPTZ NOT NULL,
            iv INTERVAL NOT NULL,
            uid UUID NOT NULL,
            js JSON NOT NULL,
            jsb JSONB NOT NULL,
            bools BOOLEAN[] NOT NULL,
            i4s INTEGER[] NOT NULL,
            txts TEXT[] NOT NULL,
            bins BYTEA[] NOT NULL,
            uids UUID[] NOT NULL,
            nums NUMERIC[] NOT NULL,
            inet_v INET NOT NULL,
            inet_vs INET[] NOT NULL,
            cidr_v CIDR NOT NULL,
            mood record_pg_it.mood NOT NULL,
            moods record_pg_it.mood[] NOT NULL,
            null_txt TEXT,
            empty_txt TEXT NOT NULL,
            json_null JSONB NOT NULL
        )"#,
    )
    .execute(&pool)
    .await
    .expect("创建测试表应成功");
    sqlx::query(
        r#"INSERT INTO record_pg_it.all_types (
            b, i2, i4, i8, f4, f8, num, txt, vc, bp, nm, bin,
            d, t, ttz, ts, tstz, iv, uid, js, jsb,
            bools, i4s, txts, bins, uids, nums, inet_v, inet_vs, cidr_v,
            mood, moods, null_txt, empty_txt, json_null
        ) VALUES (
            TRUE, -123, 456, 7890123456, 1.5, 2.25, 1234.5678, '世界', 'hello', 'abcd', 'alice',
            '\xdeadbeef',
            '2026-11-08', '12:34:56.789', '10:30:00+08', '2026-11-08 10:30:00.5',
            '2026-11-08 10:30:00+08', INTERVAL '1 year 2 mons 3 days 04:05:06.789',
            '00000000-0000-0000-0000-0000000000aa',
            '{"a": 1, "b": [true, null]}'::json,
            '{"k": "v"}'::jsonb,
            '{true,false,NULL}'::boolean[],
            '{1,2,NULL}'::integer[],
            '{"甲","乙"}'::text[],
            ARRAY['\x0102'::bytea, '\x03'::bytea],
            ARRAY['00000000-0000-0000-0000-0000000000aa'::uuid],
            '{1.5000,2.2500}'::numeric[],
            '192.168.1.1/24',
            ARRAY['192.168.1.0/24'::inet, '10.0.0.1'::inet],
            '10.0.0.0/8',
            'happy',
            '{happy,sad}'::record_pg_it.mood[],
            NULL,
            '',
            'null'::jsonb
        )"#,
    )
    .execute(&pool)
    .await
    .expect("插入测试数据应成功");

    let record = sqlx::query_as::<_, Record>("SELECT * FROM record_pg_it.all_types")
        .fetch_one(&pool)
        .await
        .expect("Record 应能解码全部受支持类型");

    let expected = json!({
        "id": 1,
        "b": true,
        "i2": -123,
        "i4": 456,
        "i8": 7_890_123_456_i64,
        "f4": 1.5,
        "f8": 2.25,
        "num": "1234.5678",
        "txt": "世界",
        "vc": "hello",
        "bp": "abcd",
        "nm": "alice",
        "bin": "3q2+7w==",
        "d": "2026-11-08",
        "t": "12:34:56.789",
        "ttz": "10:30:00+08:00",
        "ts": "2026-11-08T10:30:00.5",
        "tstz": "2026-11-08T02:30:00Z",
        "iv": {"months": 14, "days": 3, "microseconds": 14_706_789_000_i64},
        "uid": "00000000-0000-0000-0000-0000000000aa",
        "js": {"a": 1, "b": [true, null]},
        "jsb": {"k": "v"},
        "bools": [true, false, null],
        "i4s": [1, 2, null],
        "txts": ["甲", "乙"],
        "bins": ["AQI=", "Aw=="],
        "uids": ["00000000-0000-0000-0000-0000000000aa"],
        "nums": ["1.5000", "2.2500"],
        "inet_v": "192.168.1.1/24",
        "inet_vs": ["192.168.1.0/24", "10.0.0.1"],
        "cidr_v": "10.0.0.0/8",
        "mood": "happy",
        "moods": ["happy", "sad"],
        // SQL NULL 与空字符串必须区分。
        "null_txt": null,
        "empty_txt": "",
        // JSONB 字面量 null 与 SQL NULL 都映射为 JSON Null（本轮不引入 sidecar）。
        "json_null": null,
    });

    assert_eq!(
        serde_json::to_value(&record).expect("Record 应可序列化"),
        expected
    );

    sqlx::query("DROP SCHEMA record_pg_it CASCADE")
        .execute(&pool)
        .await
        .expect("清理 schema 应成功");
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn record_reports_unsupported_custom_type_with_column_context() {
    let pool = connect().await;

    sqlx::query("CREATE SCHEMA IF NOT EXISTS record_pg_unknown")
        .execute(&pool)
        .await
        .expect("创建 schema 应成功");
    sqlx::query("DROP TABLE IF EXISTS record_pg_unknown.items")
        .execute(&pool)
        .await
        .expect("删除旧表应成功");
    sqlx::query("DROP TYPE IF EXISTS record_pg_unknown.pair")
        .execute(&pool)
        .await
        .expect("删除旧类型应成功");
    sqlx::query("CREATE TYPE record_pg_unknown.pair AS (a integer, b text)")
        .execute(&pool)
        .await
        .expect("创建复合类型应成功");
    sqlx::query("CREATE TABLE record_pg_unknown.items (id INTEGER, p record_pg_unknown.pair)")
        .execute(&pool)
        .await
        .expect("创建测试表应成功");
    sqlx::query("INSERT INTO record_pg_unknown.items VALUES (1, ROW(1, 'x'))")
        .execute(&pool)
        .await
        .expect("插入测试数据应成功");

    let error = sqlx::query_as::<_, Record>("SELECT p FROM record_pg_unknown.items")
        .fetch_one(&pool)
        .await
        .expect_err("复合类型必须返回结构化错误，不得静默置 Null");

    match error {
        sqlx::Error::ColumnDecode { index, source } => {
            // 序号 + 列名由 index 携带。
            assert!(index.contains('p'), "index 应包含列名: {index}");
            assert!(index.contains('0'), "index 应包含序号: {index}");
            let message = source.to_string();
            assert!(message.contains("pair"), "错误应包含类型名: {message}");
            assert!(message.contains("OID"), "错误应包含 OID: {message}");
        }
        other => panic!("期望 ColumnDecode 错误，实得 {other:?}"),
    }

    sqlx::query("DROP SCHEMA record_pg_unknown CASCADE")
        .execute(&pool)
        .await
        .expect("清理 schema 应成功");
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn record_preserves_column_names_and_later_duplicates_win() {
    let pool = connect().await;

    let record = sqlx::query_as::<_, Record>(
        r#"SELECT 1::int4 AS "Weird Name", 2::int4 AS dup, 3::int4 AS dup, NULL::text AS n"#,
    )
    .fetch_one(&pool)
    .await
    .expect("简单查询应可解码");

    // 列名原样保留（含空格与大小写）。
    assert_eq!(record.get("Weird Name"), Some(&json!(1)));
    // 重复列名后者覆盖前者。
    assert_eq!(record.get("dup"), Some(&json!(3)));
    // SQL NULL → JSON Null。
    assert_eq!(record.get("n"), Some(&Value::Null));

    assert_eq!(record.as_map().len(), 3, "重复列名不应产生额外键");
}
