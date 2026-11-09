#![cfg(feature = "postgres")]
#![allow(clippy::expect_used, clippy::unwrap_used)]
//! PostgreSQL Wave 3 查询能力真库集成测试。
//!
//! 覆盖 GROUPS 窗口帧、聚合窗口 `FILTER (WHERE ...)`、命名 `WINDOW` 子句、
//! 事务内完整行锁入口（`FOR ... OF` / `NOWAIT` / `SKIP LOCKED`）以及带条件的
//! MERGE 与 PostgreSQL 17 的 `WHEN NOT MATCHED BY SOURCE` 版本门控。
//!
//! 所有用例标记 `#[ignore]`。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-db --no-default-features --features postgres \
//!   --test integration_pg_wave3_query_features -- --ignored --test-threads=1
//! ```

use serde_json::json;
use yang_db::postgres::query_builder::{
    FrameBound, MergeNotMatchedBySource, MergeOptions, WindowFrame, WindowFunction, WindowSpec,
};
use yang_db::postgres::Condition;
use yang_db::postgres::{Database, SqlValue};
use yang_db::{field, CompareOp, DbError, RowLock, SortOrder, SqlErrorKind, TableRef};

fn test_db_url() -> String {
    std::env::var("PG_TEST_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432/test".to_string())
}

async fn connect() -> Database {
    Database::connect(&test_db_url())
        .await
        .expect("PG_TEST_URL 指向的 PostgreSQL 必须可连接")
}

/// 运行期唯一 schema 名：多个进程并行跑同一真库时不会互相覆盖。
fn schema_name() -> String {
    format!("wave3_query_{}", std::process::id())
}

fn table_ref(schema: &str, name: &str) -> TableRef {
    TableRef::new(format!("{schema}.{name}")).expect("schema 限定表名应合法")
}

async fn prepare(db: &Database, schema: &str) {
    let statements = [
        format!("DROP SCHEMA IF EXISTS {schema} CASCADE"),
        format!("CREATE SCHEMA {schema}"),
        format!(
            "CREATE TABLE {schema}.sales (
                id INT PRIMARY KEY,
                dept TEXT NOT NULL,
                amount INT NOT NULL
            )"
        ),
        format!(
            "INSERT INTO {schema}.sales (id, dept, amount) VALUES
                (1, 'a', 10), (2, 'a', 20), (3, 'a', 30), (4, 'a', 40), (5, 'b', 50)"
        ),
        format!("CREATE TABLE {schema}.lock_rows (id BIGINT PRIMARY KEY)"),
        format!("INSERT INTO {schema}.lock_rows (id) VALUES (1), (2)"),
        format!(
            "CREATE TABLE {schema}.merge_target (
                id BIGINT PRIMARY KEY,
                v INT NOT NULL,
                tag TEXT NOT NULL
            )"
        ),
        format!(
            "CREATE TABLE {schema}.merge_source (
                id BIGINT NOT NULL,
                v INT NOT NULL,
                tag TEXT NOT NULL
            )"
        ),
        format!(
            "INSERT INTO {schema}.merge_target (id, v, tag) VALUES (1, 10, 't1'), (2, 20, 't2')"
        ),
        format!(
            "INSERT INTO {schema}.merge_source (id, v, tag) VALUES (1, 100, 's1'), (3, 300, 's3')"
        ),
    ];
    for sql in statements {
        db.execute_with_params(&sql, vec![])
            .await
            .unwrap_or_else(|error| panic!("准备 SQL 失败: {sql}: {error}"));
    }
}

async fn cleanup(db: &Database, schema: &str) {
    db.execute_with_params(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), vec![])
        .await
        .expect("清理测试 schema 应成功");
}

#[derive(Debug, sqlx::FromRow)]
struct LockId {
    id: i64,
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave3_groups_frame_accumulates_peer_groups() {
    let schema = schema_name();
    let db = connect().await;
    prepare(&db, &schema).await;
    let sales = table_ref(&schema, "sales");

    let rows: Vec<(i32, i64)> = db
        .table(&sales)
        .field(field!("id"))
        .window_expr(
            WindowFunction::sum(field!("amount")),
            WindowSpec::new()
                .order_by(field!("id"), SortOrder::Asc)
                .frame(WindowFrame::groups_between(
                    FrameBound::Preceding(1),
                    FrameBound::CurrentRow,
                )),
            field!("running"),
        )
        .order(field!("id"), SortOrder::Asc)
        .select::<(i32, i64)>()
        .await
        .expect("GROUPS 帧查询应成功");

    // id 各不相同，因此每个 peer group 只有一行：跨度为「前 1 组 + 当前组」。
    assert_eq!(rows, vec![(1, 10), (2, 30), (3, 50), (4, 70), (5, 90)]);
    cleanup(&db, &schema).await;
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave3_aggregate_filter_counts_only_matching_rows() {
    let schema = schema_name();
    let db = connect().await;
    prepare(&db, &schema).await;
    let sales = table_ref(&schema, "sales");

    let rows: Vec<(String, i64)> = db
        .table(&sales)
        .field(field!("dept"))
        .window_expr(
            WindowFunction::count_all()
                .filter(Condition::Gt("amount".to_string(), SqlValue::Int(15))),
            WindowSpec::new().partition_by(&[field!("dept")]),
            field!("cnt"),
        )
        .order(field!("dept"), SortOrder::Asc)
        .select::<(String, i64)>()
        .await
        .expect("FILTER 窗口查询应成功");

    // dept a：10/20/30/40 中只有 3 行 > 15；dept b：50 命中 1 行。
    assert_eq!(
        rows,
        vec![
            ("a".to_string(), 3),
            ("a".to_string(), 3),
            ("a".to_string(), 3),
            ("a".to_string(), 3),
            ("b".to_string(), 1),
        ]
    );
    cleanup(&db, &schema).await;
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave3_named_window_is_reused_by_reference() {
    let schema = schema_name();
    let db = connect().await;
    prepare(&db, &schema).await;
    let sales = table_ref(&schema, "sales");

    let rows: Vec<(i32, i64)> = db
        .table(&sales)
        .field(field!("id"))
        .window(
            "w",
            WindowSpec::new().order_by(field!("id"), SortOrder::Asc),
        )
        .window_expr(
            WindowFunction::row_number(),
            WindowSpec::reference("w"),
            field!("rn"),
        )
        .order(field!("id"), SortOrder::Asc)
        .select::<(i32, i64)>()
        .await
        .expect("命名窗口查询应成功");

    assert_eq!(rows, vec![(1, 1), (2, 2), (3, 3), (4, 4), (5, 5)]);
    cleanup(&db, &schema).await;
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave3_select_locked_with_nowait_maps_lock_not_available() {
    let schema = schema_name();
    let db = connect().await;
    prepare(&db, &schema).await;
    let lock_rows = table_ref(&schema, "lock_rows");

    let mut holder = db.transaction().await.expect("事务应开始");
    holder
        .execute_with_params(
            &format!("SELECT id FROM {schema}.lock_rows WHERE id = $1 FOR UPDATE"),
            vec![json!(1)],
        )
        .await
        .expect("持有行锁应成功");

    let mut waiter = db.transaction().await.expect("事务应开始");
    let error = waiter
        .select_locked_with::<LockId>(
            db.table(&lock_rows)
                .where_and(field!("id"), CompareOp::Eq, 1)
                .expect("受控条件应合法"),
            RowLock::ForUpdate,
            &[],
            true,
            false,
        )
        .await
        .expect_err("NOWAIT 抢锁应失败");
    assert_eq!(error.sqlstate(), Some("55P03"), "{error:?}");
    assert_eq!(
        error.sql_error().map(|info| info.kind()),
        Some(SqlErrorKind::LockNotAvailable),
        "{error:?}"
    );
    waiter.rollback().await.expect("回滚应成功");
    holder.rollback().await.expect("回滚应成功");
    cleanup(&db, &schema).await;
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave3_select_locked_with_of_and_skip_locked() {
    let schema = schema_name();
    let db = connect().await;
    prepare(&db, &schema).await;
    let lock_rows = table_ref(&schema, "lock_rows");

    // FOR ... OF 指定表：在事务内正常执行并返回全部匹配行。
    let mut tx = db.transaction().await.expect("事务应开始");
    let rows: Vec<LockId> = tx
        .select_locked_with(
            db.table(&lock_rows)
                .where_and(field!("id"), CompareOp::Gte, 1)
                .expect("受控条件应合法"),
            RowLock::ForUpdate,
            &[field!("lock_rows")],
            false,
            false,
        )
        .await
        .expect("FOR UPDATE OF 应成功");
    assert_eq!(rows.len(), 2);
    tx.rollback().await.expect("回滚应成功");

    // SKIP LOCKED 只返回未被其他事务锁定的行。
    let mut holder = db.transaction().await.expect("事务应开始");
    holder
        .execute_with_params(
            &format!("SELECT id FROM {schema}.lock_rows WHERE id = $1 FOR UPDATE"),
            vec![json!(1)],
        )
        .await
        .expect("持有行锁应成功");

    let mut reader = db.transaction().await.expect("事务应开始");
    let unlocked: Vec<LockId> = reader
        .select_locked_with(
            db.table(&lock_rows)
                .where_and(field!("id"), CompareOp::Gt, 0)
                .expect("受控条件应合法"),
            RowLock::ForUpdate,
            &[],
            false,
            true,
        )
        .await
        .expect("SKIP LOCKED 查询应成功");
    assert_eq!(unlocked.len(), 1);
    assert_eq!(unlocked[0].id, 2);
    reader.rollback().await.expect("回滚应成功");
    holder.rollback().await.expect("回滚应成功");
    cleanup(&db, &schema).await;
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave3_merge_matched_condition_and_by_source_version_gate() {
    let schema = schema_name();
    let db = connect().await;
    prepare(&db, &schema).await;
    let merge_target = table_ref(&schema, "merge_target");
    let merge_source = table_ref(&schema, "merge_source");

    // 带条件的命中更新：只有目标行 v = 10 时更新；未命中的 id=3 插入。
    let affected = db
        .table(&merge_target)
        .merge_with_options(
            &merge_source,
            field!("id"),
            field!("id"),
            &[field!("id"), field!("v"), field!("tag")],
            MergeOptions::default().matched_condition(Condition::Eq(
                "merge_target.v".to_string(),
                SqlValue::Int(10),
            )),
        )
        .await
        .expect("带条件的 MERGE 应成功");
    assert_eq!(affected, 2);

    let rows: Vec<(i64, i32, String)> = db
        .table(&merge_target)
        .field(field!("id"))
        .field(field!("v"))
        .field(field!("tag"))
        .order(field!("id"), SortOrder::Asc)
        .select::<(i64, i32, String)>()
        .await
        .expect("MERGE 后查询应成功");
    assert_eq!(
        rows,
        vec![
            (1, 100, "s1".to_string()),
            (2, 20, "t2".to_string()),
            (3, 300, "s3".to_string()),
        ]
    );

    // PG16 不支持 WHEN NOT MATCHED BY SOURCE：必须在客户端门控，不下发语法。
    let error = db
        .table(&merge_target)
        .merge_with_options(
            &merge_source,
            field!("id"),
            field!("id"),
            &[field!("v")],
            MergeOptions::default().not_matched_by_source(MergeNotMatchedBySource::Delete),
        )
        .await
        .expect_err("PG16 上 BY SOURCE 分支必须被版本门控拒绝");
    assert!(
        matches!(error, DbError::UnsupportedOperator(_)),
        "{error:?}"
    );
    cleanup(&db, &schema).await;
}
