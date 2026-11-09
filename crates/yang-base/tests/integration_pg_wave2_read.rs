#![allow(clippy::expect_used, clippy::unwrap_used)]
#![cfg(feature = "postgres")]
//! PostgreSQL Wave 2 读取语义与 NULL 契约集成测试。
//!
//! 覆盖：
//! - `Record` 区分 SQL NULL 与 JSONB 字面量 `null`；
//! - `order_nulls` 的 `NULLS FIRST/LAST`；
//! - 分页主键 tiebreaker 的跨页稳定性；
//! - 单查询窗口 `COUNT(*) OVER()` 返回的 `total` 与当前页一致；
//! - `exists()`；
//! - 超过 1000 个值的 IN 分块与 `= ANY($1)` 数组参数。
//!
//! 所有测试均标记 `#[ignore]`，默认离线套件不触库。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-base --no-default-features --features postgres \
//!   --test integration_pg_wave2_read -- --ignored --test-threads=1
//! ```

use serde_json::json;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::sync::Arc;
use std::time::Duration;
use yang_base::database::AnyPool;
use yang_base::table::{Field, SortOrder, Table, TableDefinition, TableQuery};

fn test_db_url() -> String {
    std::env::var("PG_TEST_URL")
        .or_else(|_| std::env::var("YANG_POSTGRES_URL"))
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432/test".to_string())
}

async fn connect_pool() -> PgPool {
    PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&test_db_url())
        .await
        .expect("PG_TEST_URL / YANG_POSTGRES_URL 指向的 PostgreSQL 必须可连接")
}

async fn reset_tables(pool: &PgPool) {
    sqlx::query("DROP TABLE IF EXISTS tq_pg_wave2_read")
        .execute(pool)
        .await
        .expect("删除旧测试表应成功");
    sqlx::query(
        "CREATE TABLE tq_pg_wave2_read (
            id BIGSERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            bucket INTEGER NULL,
            payload JSONB NULL
        )",
    )
    .execute(pool)
    .await
    .expect("创建 tq_pg_wave2_read 应成功");
}

fn definition() -> TableDefinition {
    Table::new("tq_pg_wave2_read")
        .fields([
            Field::id("id"),
            Field::string("name", 50).required(),
            Field::integer("bucket"),
            Field::json("payload"),
        ])
        .build()
        .expect("tq_pg_wave2_read 表定义应有效")
}

fn bind(definition: &TableDefinition, pool: &PgPool) -> TableQuery {
    definition
        .bind_pool(AnyPool::Postgres(Arc::new(pool.clone())))
        .query(["admin"])
}

async fn seed_rows(pool: &PgPool) {
    // id 依次为 1..=6：bucket 为 NULL / 有值交错，payload 覆盖 SQL NULL 与 JSON null。
    sqlx::query(
        r#"INSERT INTO tq_pg_wave2_read (name, bucket, payload) VALUES
            ('a', NULL, NULL),
            ('b', 1,    'null'::jsonb),
            ('c', 1,    '{"k":1}'::jsonb),
            ('d', 2,    NULL),
            ('e', 2,    'null'::jsonb),
            ('f', NULL, '{"k":2}'::jsonb)"#,
    )
    .execute(pool)
    .await
    .expect("插入测试数据应成功");
}

/// P0-05：SQL NULL 与 JSONB 字面量 null 必须可区分，且序列化值仍为 null。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn record_distinguishes_sql_null_from_json_null() {
    let pool = connect_pool().await;
    reset_tables(&pool).await;
    seed_rows(&pool).await;
    let definition = definition();

    let sql_null = bind(&definition, &pool)
        .select_fields(&["id", "payload"])
        .expect("投影字段应合法")
        .where_eq("id", json!(1))
        .expect("id 筛选应合法")
        .one()
        .await
        .expect("查询 SQL NULL 行应成功");
    assert!(sql_null.is_sql_null("payload"), "payload 应为 SQL NULL");
    assert!(!sql_null.is_json_null("payload"));
    assert_eq!(sql_null.get("payload"), Some(&json!(null)));

    let json_null = bind(&definition, &pool)
        .select_fields(&["id", "payload"])
        .expect("投影字段应合法")
        .where_eq("id", json!(2))
        .expect("id 筛选应合法")
        .one()
        .await
        .expect("查询 JSON null 行应成功");
    assert!(json_null.is_json_null("payload"), "payload 应为 JSON null");
    assert!(!json_null.is_sql_null("payload"));
    assert_eq!(json_null.get("payload"), Some(&json!(null)));

    // 非 JSON 类型的 SQL NULL 不会被误标为 JSON null。
    let bucket_null = bind(&definition, &pool)
        .select_fields(&["id", "bucket"])
        .expect("投影字段应合法")
        .where_eq("id", json!(1))
        .expect("id 筛选应合法")
        .one()
        .await
        .expect("查询 bucket 为 NULL 的行应成功");
    assert!(bucket_null.is_sql_null("bucket"));
    assert!(!bucket_null.is_json_null("bucket"));
}

/// P1-12：`NULLS FIRST/LAST` 在 PostgreSQL 上按请求生效。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn order_nulls_places_nulls_first_and_last() {
    let pool = connect_pool().await;
    reset_tables(&pool).await;
    seed_rows(&pool).await;
    let definition = definition();

    let first = bind(&definition, &pool)
        .select_fields(&["id", "bucket"])
        .expect("投影字段应合法")
        .order_nulls("bucket", SortOrder::Asc, true)
        .expect("order_nulls 应合法")
        .all()
        .await
        .expect("NULLS FIRST 查询应成功");
    let buckets: Vec<Option<i64>> = first
        .iter()
        .map(|row| row.get("bucket").and_then(serde_json::Value::as_i64))
        .collect();
    assert_eq!(
        buckets.first(),
        Some(&None),
        "NULLS FIRST 应把 NULL 排在最前"
    );
    assert_eq!(buckets.last(), Some(&Some(2)), "非 NULL 应按升序收尾");

    let last = bind(&definition, &pool)
        .select_fields(&["id", "bucket"])
        .expect("投影字段应合法")
        .order_nulls("bucket", SortOrder::Asc, false)
        .expect("order_nulls 应合法")
        .all()
        .await
        .expect("NULLS LAST 查询应成功");
    let buckets: Vec<Option<i64>> = last
        .iter()
        .map(|row| row.get("bucket").and_then(serde_json::Value::as_i64))
        .collect();
    assert_eq!(buckets.last(), Some(&None), "NULLS LAST 应把 NULL 排在最后");
}

/// P1-12：分页排序追加主键 tiebreaker，跨页无重复、无遗漏。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn pagination_is_stable_across_pages() {
    let pool = connect_pool().await;
    reset_tables(&pool).await;
    // 追加更多同 bucket 行，制造仅按 bucket 排序时的并列。
    seed_rows(&pool).await;
    sqlx::query(
        "INSERT INTO tq_pg_wave2_read (name, bucket, payload)
         SELECT 'extra', 2, NULL FROM generate_series(1, 8)",
    )
    .execute(&pool)
    .await
    .expect("插入并列行应成功");
    let definition = definition();

    let mut seen = Vec::new();
    for page in 1..=7 {
        let result = bind(&definition, &pool)
            .select_fields(&["id", "bucket"])
            .expect("投影字段应合法")
            .order_by("bucket", SortOrder::Asc)
            .expect("排序字段应合法")
            .page(page, 2)
            .expect("分页参数应合法")
            .paginate_records()
            .await
            .expect("分页查询应成功");
        seen.extend(
            result
                .data
                .iter()
                .map(|row| row.require::<i64>("id").expect("id 应为整数")),
        );
    }

    let mut sorted = seen.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), seen.len(), "跨页结果不应有重复 id");
    assert_eq!(seen.len(), 14, "7 页 × 2 行应覆盖全部 14 行");
}

/// P1-12：窗口 COUNT(*) OVER() 的 total 与全量行数一致（含空页回退）。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn pagination_total_matches_full_count() {
    let pool = connect_pool().await;
    reset_tables(&pool).await;
    seed_rows(&pool).await;
    let definition = definition();

    let result = bind(&definition, &pool)
        .select_fields(&["id"])
        .expect("投影字段应合法")
        .page(2, 2)
        .expect("分页参数应合法")
        .paginate_records()
        .await
        .expect("分页查询应成功");
    assert_eq!(result.total, 6, "total 应等于全表行数");
    assert_eq!(result.data.len(), 2, "第二页应返回 2 行");
    assert!(
        !result
            .data
            .iter()
            .any(|row| row.get("__yang_total").is_some()),
        "窗口计数别名不应泄漏到结果行"
    );

    let count = bind(&definition, &pool)
        .count()
        .await
        .expect("COUNT 查询应成功");
    assert_eq!(result.total as u64, count, "total 应与独立 COUNT 一致");

    // OFFSET 越界：窗口计数结果为空，回退到 COUNT 仍给出真实总数。
    let beyond = bind(&definition, &pool)
        .select_fields(&["id"])
        .expect("投影字段应合法")
        .page(4, 2)
        .expect("分页参数应合法")
        .paginate_records()
        .await
        .expect("越界分页查询应成功");
    assert!(beyond.data.is_empty(), "越界页应无数据");
    assert_eq!(beyond.total, 6, "越界页也应报告真实总数");
}

/// P0-05 + 分页：`strip_total_count` 剥离窗口计数列时不得丢失 JSON null sidecar。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn pagination_preserves_json_null_sidecar() {
    let pool = connect_pool().await;
    reset_tables(&pool).await;
    seed_rows(&pool).await;
    let definition = definition();

    let result = bind(&definition, &pool)
        .select_fields(&["id", "payload"])
        .expect("投影字段应合法")
        .order_by("id", SortOrder::Asc)
        .expect("排序字段应合法")
        .page(1, 10)
        .expect("分页参数应合法")
        .paginate_records()
        .await
        .expect("分页查询应成功");
    assert_eq!(result.data.len(), 6, "第一页应返回全部 6 行");

    let find = |id: i64| {
        result
            .data
            .iter()
            .find(|row| row.get("id").and_then(serde_json::Value::as_i64) == Some(id))
            .unwrap_or_else(|| panic!("分页结果应包含 id={id}"))
    };

    // id=1: payload IS NULL
    let sql_null = find(1);
    assert!(
        sql_null.is_sql_null("payload"),
        "SQL NULL 行经分页后仍应为 SQL NULL: {sql_null:?}"
    );
    assert!(!sql_null.is_json_null("payload"));

    // id=2: payload = 'null'::jsonb
    let json_null = find(2);
    assert!(
        json_null.is_json_null("payload"),
        "JSON null 行经分页后必须仍是 JSON null（sidecar 不得丢失）: {json_null:?}"
    );
    assert!(!json_null.is_sql_null("payload"));

    // id=3: payload = '{"k":1}'::jsonb
    let object = find(3);
    assert!(!object.is_sql_null("payload"));
    assert!(!object.is_json_null("payload"));
    assert_eq!(object.get("payload"), Some(&json!({"k": 1})));
}

/// P1-12：exists() 走 LIMIT 1，不依赖全量计数。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn exists_reports_match_without_full_count() {
    let pool = connect_pool().await;
    reset_tables(&pool).await;
    seed_rows(&pool).await;
    let definition = definition();

    assert!(bind(&definition, &pool)
        .where_eq("name", json!("c"))
        .expect("筛选应合法")
        .exists()
        .await
        .expect("exists 查询应成功"));
    assert!(!bind(&definition, &pool)
        .where_eq("name", json!("missing"))
        .expect("筛选应合法")
        .exists()
        .await
        .expect("exists 查询应成功"));
}

/// P1-12：IN 列表超过 1000 条时分块执行，不触发参数上限。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn in_list_over_one_thousand_values_is_chunked() {
    let pool = connect_pool().await;
    reset_tables(&pool).await;
    seed_rows(&pool).await;
    let definition = definition();

    let values: Vec<serde_json::Value> = (1..=1500).map(|id| json!(id)).collect();
    let rows = bind(&definition, &pool)
        .select_fields(&["id"])
        .expect("投影字段应合法")
        .where_in_chunked("id", values)
        .expect("分块 IN 应合法")
        .all()
        .await
        .expect("分块 IN 查询应成功");
    assert_eq!(rows.len(), 6, "1..=1500 应命中全部 6 行");
}

/// P1-12：`where_any` 在 PostgreSQL 上使用 `= ANY($1)` 单个数组参数。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn where_any_binds_single_array_parameter() {
    let pool = connect_pool().await;
    reset_tables(&pool).await;
    seed_rows(&pool).await;
    let definition = definition();

    let rows = bind(&definition, &pool)
        .select_fields(&["id"])
        .expect("投影字段应合法")
        .where_any("id", vec![json!(2), json!(4)])
        .expect("ANY 条件应合法")
        .all()
        .await
        .expect("ANY 查询应成功");
    let mut ids: Vec<i64> = rows
        .iter()
        .map(|row| row.require::<i64>("id").expect("id 应为整数"))
        .collect();
    ids.sort_unstable();
    assert_eq!(ids, vec![2, 4]);

    // 超过 1000 个元素仍然只有一个数组参数（不会触碰参数上限）。
    let many: Vec<serde_json::Value> = (1..=1500).map(|id| json!(id)).collect();
    let rows = bind(&definition, &pool)
        .select_fields(&["id"])
        .expect("投影字段应合法")
        .where_any("id", many)
        .expect("ANY 条件应合法")
        .all()
        .await
        .expect("ANY 大列表查询应成功");
    assert_eq!(rows.len(), 6, "ANY 大列表应命中全部 6 行");
}
