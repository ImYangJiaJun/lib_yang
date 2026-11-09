#![cfg(feature = "postgres")]
#![allow(clippy::expect_used, clippy::unwrap_used)]
//! PostgreSQL Wave 3 契约真库集成测试。
//!
//! 覆盖：
//! - raw 参数的未类型化 NULL 绑定（`$1::text` / `$1::uuid` / `$1::timestamptz` /
//!   `$1::numeric` / `$1::jsonb` / `$1::int + 1` / `$1`）；
//! - PostgreSQL 标识符规则贯通到 `TableRef`/`FieldRef`（中文、`$`、内嵌引号、
//!   63 字节上限）；
//! - 连接池 `after_connect` 下发的会话 GUC（DateStyle 基线，以及配置生效的
//!   application_name / search_path / statement_timeout）。
//!
//! 所有用例标记 `#[ignore]`。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-db --no-default-features --features postgres \
//!   --test integration_pg_wave3_contract -- --ignored --test-threads=1
//! ```

use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use yang_db::postgres::{Database, DatabaseConfig};
use yang_db::{DbError, FieldRef, SortOrder, TableRef};

fn test_db_url() -> String {
    std::env::var("PG_TEST_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432/test".to_string())
}

async fn connect() -> Database {
    Database::connect(&test_db_url())
        .await
        .expect("PG_TEST_URL 指向的 PostgreSQL 必须可连接")
}

#[derive(Debug, sqlx::FromRow)]
struct TypedNullRow {
    text_value: Option<String>,
    uuid_value: Option<uuid::Uuid>,
    timestamptz_value: Option<DateTime<Utc>>,
    numeric_value: Option<BigDecimal>,
    jsonb_value: Option<Value>,
}

/// 残余 #8：raw 参数传入 `Value::Null` 时必须以未类型化 NULL 绑定，
/// 让服务端按 SQL 上下文推断参数类型，而不是固定成 int4。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn raw_null_parameter_is_untyped_for_every_cast() {
    let db = connect().await;

    let rows: Vec<TypedNullRow> = db
        .query_with_params(
            "SELECT $1::text AS text_value,
                    $2::uuid AS uuid_value,
                    $3::timestamptz AS timestamptz_value,
                    $4::numeric AS numeric_value,
                    $5::jsonb AS jsonb_value",
            vec![
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
            ],
        )
        .await
        .expect("显式 cast 的 NULL 参数应全部成功");

    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert!(row.text_value.is_none());
    assert!(row.uuid_value.is_none());
    assert!(row.timestamptz_value.is_none());
    assert!(row.numeric_value.is_none());
    assert!(row.jsonb_value.is_none());

    // NULL 参与运算仍应得到 NULL（类型由 `::int` 决定）。
    let sum: Vec<(Option<i32>,)> = db
        .query_with_params("SELECT $1::int + 1", vec![Value::Null])
        .await
        .expect("NULL 参与 int 运算应成功");
    assert_eq!(sum.len(), 1);
    assert!(sum[0].0.is_none());

    // 无 cast 的 `SELECT $1`：服务端按未类型化字面量推断为 text，返回 NULL 而不报错。
    let bare: Vec<(Option<String>,)> = db
        .query_with_params("SELECT $1", vec![Value::Null])
        .await
        .expect("无 cast 的 NULL 参数不应报错");
    assert_eq!(bare.len(), 1);
    assert!(bare[0].0.is_none());
}

/// 非 NULL 参数行为必须与改动前完全一致。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn non_null_raw_parameters_keep_their_binding_behavior() {
    let db = connect().await;

    let ints: Vec<(i32,)> = db
        .query_with_params("SELECT $1::int + 1", vec![json!(41)])
        .await
        .expect("非 NULL int 参数应成功");
    assert_eq!(ints[0].0, 42);

    let texts: Vec<(String,)> = db
        .query_with_params("SELECT $1::text", vec![json!("hello")])
        .await
        .expect("非 NULL text 参数应成功");
    assert_eq!(texts[0].0, "hello");

    let uuid_text = "8f14e45f-ea2f-4e1c-9b7e-2f0e5a1c3d4b";
    let uuids: Vec<(uuid::Uuid,)> = db
        .query_with_params("SELECT $1::uuid", vec![json!(uuid_text)])
        .await
        .expect("字符串参数经显式 uuid cast 应成功");
    assert_eq!(uuids[0].0.to_string(), uuid_text);

    let numerics: Vec<(BigDecimal,)> = db
        .query_with_params("SELECT $1::numeric", vec![json!(1.5)])
        .await
        .expect("浮点参数经显式 numeric cast 应成功");
    assert!(numerics[0].0.to_string().starts_with("1.5"));
}

/// 运行期唯一 schema 名：多个进程并行跑同一真库时不会互相覆盖。
fn contract_schema() -> String {
    format!("wave3_contract_{}", std::process::id())
}

async fn prepare_unicode_table(db: &Database, schema: &str) {
    for sql in [
        format!("DROP SCHEMA IF EXISTS {schema} CASCADE"),
        format!("CREATE SCHEMA {schema}"),
        format!("CREATE TABLE {schema}.\"订单\" (id INT PRIMARY KEY, \"金额\" INT NOT NULL)"),
        format!("INSERT INTO {schema}.\"订单\" (id, \"金额\") VALUES (1, 10), (2, 0)"),
    ] {
        db.execute_with_params(&sql, vec![])
            .await
            .unwrap_or_else(|error| panic!("准备 SQL 失败: {sql}: {error}"));
    }
}

async fn cleanup_unicode_table(db: &Database, schema: &str) {
    db.execute_with_params(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), vec![])
        .await
        .expect("清理测试 schema 应成功");
}

/// B42/B43：中文表名/字段名经 `TableRef`/`FieldRef` 构造并真实执行。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn unicode_table_and_field_refs_execute_against_real_database() {
    let schema = contract_schema();
    let db = connect().await;
    prepare_unicode_table(&db, &schema).await;

    let table = TableRef::new(format!("{schema}.订单")).expect("中文限定表名应合法");
    let id = FieldRef::new("id").expect("id 字段应合法");
    let amount = FieldRef::new("订单.金额").expect("中文限定字段应合法");

    let sql = db
        .table(&table)
        .field(&id)
        .field(&amount)
        .order(&amount, SortOrder::Desc)
        .try_to_sql()
        .expect("中文标识符查询应生成 SQL");
    assert!(sql.contains(&format!("\"{schema}\".\"订单\"")), "{sql}");
    assert!(sql.contains("\"订单\".\"金额\""), "{sql}");

    let rows: Vec<(i32, i32)> = db
        .query_with_params(&sql, vec![])
        .await
        .expect("中文标识符查询应执行成功");
    assert_eq!(rows, vec![(1, 10), (2, 0)]);

    // 内嵌双引号与 `$` 也按 PostgreSQL 规则转义。
    let quoted = FieldRef::new("a\"b").expect("内嵌双引号应合法");
    assert_eq!(quoted.postgres_quoted(), "\"a\"\"b\"");
    let dollar = TableRef::new("a$b").expect("非首字符 `$` 应合法");
    assert_eq!(dollar.as_str(), "a$b");

    // 超过 63 字节的标识符必须返回明确错误，而不是交给服务端静默截断。
    let oversized = "用".repeat(22);
    let error = TableRef::new(oversized.clone()).expect_err("超长表名必须拒绝");
    assert!(matches!(error, DbError::InvalidArgument(_)), "{error:?}");
    assert!(FieldRef::new(oversized).is_err(), "超长字段名必须拒绝");

    cleanup_unicode_table(&db, &schema).await;
}

/// GUC 回归：`after_connect` 会话基线（DateStyle）与配置项必须实际生效。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn session_gucs_from_after_connect_are_effective() {
    let config = DatabaseConfig::default()
        .with_application_name("yang-db-wave3")
        .with_search_path("public")
        .with_statement_timeout_ms(5_000u64);
    let db = Database::connect_with_config(&test_db_url(), config)
        .await
        .expect("带 GUC 配置的连接应成功");

    let (date_style,): (String,) = db
        .query_with_params("SHOW DateStyle", vec![])
        .await
        .expect("读取 DateStyle 应成功")
        .into_iter()
        .next()
        .expect("SHOW DateStyle 应返回一行");
    assert_eq!(date_style, "ISO, YMD");

    let (application_name,): (String,) = db
        .query_with_params("SELECT current_setting('application_name')", vec![])
        .await
        .expect("读取 application_name 应成功")
        .into_iter()
        .next()
        .expect("current_setting 应返回一行");
    assert_eq!(application_name, "yang-db-wave3");

    let (search_path,): (String,) = db
        .query_with_params("SHOW search_path", vec![])
        .await
        .expect("读取 search_path 应成功")
        .into_iter()
        .next()
        .expect("SHOW search_path 应返回一行");
    assert_eq!(search_path, "public");

    let (statement_timeout,): (String,) = db
        .query_with_params("SHOW statement_timeout", vec![])
        .await
        .expect("读取 statement_timeout 应成功")
        .into_iter()
        .next()
        .expect("SHOW statement_timeout 应返回一行");
    assert_eq!(statement_timeout, "5s");
}
