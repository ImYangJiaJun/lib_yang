#![allow(clippy::expect_used, clippy::unwrap_used)]
//! PostgreSQL Phase C 集成测试：范围/jsonpath/全文检索、EXPLAIN、内省、
//! SET LOCAL、版本探测、2PC 与表锁。
//!
//! 所有测试均标记 `#[ignore]`，默认离线套件不触库。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-db --no-default-features --features postgres \
//!   --test integration_pg_phase_c -- --ignored --test-threads=1
//! ```

use serde_json::json;
use std::time::Duration;
use yang_db::postgres::{Database, SqlValue, TableLockMode};
use yang_db::{field, table, DbError};

fn test_db_url() -> String {
    std::env::var("PG_TEST_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432/test".to_string())
}

async fn connect() -> Database {
    Database::connect(&test_db_url())
        .await
        .expect("PG_TEST_URL 指向的 PostgreSQL 必须可连接")
}

async fn prepare(db: &Database) {
    db.execute_with_params("DROP SCHEMA IF EXISTS phase_c CASCADE", vec![])
        .await
        .expect("清理旧 schema 应成功");
    db.execute_with_params("CREATE SCHEMA phase_c", vec![])
        .await
        .expect("创建 schema 应成功");
    db.execute_with_params(
        "CREATE TABLE phase_c.ranges (
            id BIGINT NOT NULL,
            span INT4RANGE NOT NULL,
            spans INT4MULTIRANGE NOT NULL
        )",
        vec![],
    )
    .await
    .expect("创建 ranges 应成功");
    db.execute_with_params(
        "INSERT INTO phase_c.ranges (id, span, spans) VALUES
            (1, '[1,10)',  '{[1,10)}'),
            (2, '[10,20)', '{[10,20)}'),
            (3, '[20,30)', '{[20,30)}')",
        vec![],
    )
    .await
    .expect("写入 ranges 样本应成功");
    db.execute_with_params(
        "CREATE TABLE phase_c.docs (
            id BIGINT NOT NULL,
            data JSONB NOT NULL,
            body TEXT NOT NULL
        )",
        vec![],
    )
    .await
    .expect("创建 docs 应成功");
    db.execute_with_params(
        "INSERT INTO phase_c.docs (id, data, body) VALUES
            (1, '{\"role\":\"admin\",\"age\":30}', 'hello world rust'),
            (2, '{\"role\":\"user\",\"age\":15}',  'goodbye world')",
        vec![],
    )
    .await
    .expect("写入 docs 样本应成功");
    db.execute_with_params("CREATE TABLE phase_c.locks (id BIGINT NOT NULL)", vec![])
        .await
        .expect("创建 locks 应成功");
}

/// 范围/多范围：`@>` 与 `&&` 谓词按显式 `::int4range` / `::int4multirange` 转换工作。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_c_range_contains_and_overlap() {
    let db = connect().await;
    prepare(&db).await;

    let contained: Vec<(i64,)> = db
        .table(table!("phase_c.ranges"))
        .field(field!("id"))
        .where_range_contains(field!("span"), SqlValue::Range("[5,6)".to_string()))
        .select()
        .await
        .expect("范围包含查询应成功");
    assert_eq!(contained, vec![(1,)]);

    let overlapping: Vec<(i64,)> = db
        .table(table!("phase_c.ranges"))
        .field(field!("id"))
        .where_range_overlaps(field!("span"), SqlValue::Range("[8,12)".to_string()))
        .select()
        .await
        .expect("范围重叠查询应成功");
    assert_eq!(overlapping, vec![(1,), (2,)]);

    let multirange: Vec<(i64,)> = db
        .table(table!("phase_c.ranges"))
        .field(field!("id"))
        .where_range_overlaps(
            field!("spans"),
            SqlValue::Multirange("{[15,25)}".to_string()),
        )
        .select()
        .await
        .expect("多范围重叠查询应成功");
    assert_eq!(multirange, vec![(2,), (3,)]);
}

/// jsonpath：路径与变量对象都走绑定参数。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_c_jsonpath_exists_and_match() {
    let db = connect().await;
    prepare(&db).await;

    let exists: Vec<(i64,)> = db
        .table(table!("phase_c.docs"))
        .field(field!("id"))
        .where_json_path_exists(field!("data"), "$.role ? (@ == \"admin\")")
        .select()
        .await
        .expect("jsonb_path_exists 查询应成功");
    assert_eq!(exists, vec![(1,)]);

    let matched: Vec<(i64,)> = db
        .table(table!("phase_c.docs"))
        .field(field!("id"))
        .where_json_path_match(field!("data"), "$.age > $min", json!({"min": 18}))
        .select()
        .await
        .expect("jsonb_path_match 查询应成功");
    assert_eq!(matched, vec![(1,)]);
}

/// 全文检索：`plainto_tsquery` 与 `to_tsquery` 两个变体。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_c_full_text_plain_and_query_variants() {
    let db = connect().await;
    prepare(&db).await;

    let plain: Vec<(i64,)> = db
        .table(table!("phase_c.docs"))
        .field(field!("id"))
        .where_full_text(field!("body"), "hello")
        .select()
        .await
        .expect("plainto_tsquery 查询应成功");
    assert_eq!(plain, vec![(1,)]);

    let boolean: Vec<(i64,)> = db
        .table(table!("phase_c.docs"))
        .field(field!("id"))
        .where_full_text_query(field!("body"), "hello & rust")
        .select()
        .await
        .expect("to_tsquery 查询应成功");
    assert_eq!(boolean, vec![(1,)]);

    let both: Vec<(i64,)> = db
        .table(table!("phase_c.docs"))
        .field(field!("id"))
        .where_full_text(field!("body"), "world")
        .select()
        .await
        .expect("全文查询应成功");
    assert_eq!(both, vec![(1,), (2,)]);
}

/// EXPLAIN：`FORMAT JSON` 结果可解析，且语句参数走绑定。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_c_explain_returns_parseable_json_plan() {
    let db = connect().await;

    let plans = db
        .explain("SELECT $1::int + 1", vec![json!(42)])
        .await
        .expect("EXPLAIN 应成功");
    assert!(!plans.is_empty(), "EXPLAIN 应返回至少一个计划对象");
    for plan in &plans {
        let parsed: serde_json::Value =
            serde_json::from_str(plan).expect("EXPLAIN 输出应为格式化 JSON");
        assert!(parsed.get("Plan").is_some(), "计划对象应包含 Plan 节点");
    }

    // 仅 EXPLAIN（无 ANALYZE）并非本接口语义；这里验证对表查询同样可用。
    prepare(&db).await;
    let plans = db
        .explain("SELECT * FROM phase_c.docs WHERE id = $1", vec![json!(1)])
        .await
        .expect("表查询 EXPLAIN 应成功");
    assert!(plans.iter().any(|plan| plan.contains("Plan")));
}

/// 内省：`pg_stat_activity` 返回结构化行并能看到固定连接自身。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_c_active_queries_includes_pinned_connection() {
    let db = connect().await;

    let mut conn = db.pool().acquire().await.expect("应能借出连接");
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *conn)
        .await
        .expect("读取后端 PID 应成功");

    let queries = db
        .active_queries()
        .await
        .expect("pg_stat_activity 查询应成功");
    assert!(!queries.is_empty(), "至少应包含当前连接/自身会话");
    let row = queries
        .iter()
        .find(|row| row.pid == pid)
        .expect("活动会话列表应包含固定连接的 PID");
    assert_eq!(row.pid, pid);
}

/// 内省：`pg_locks` 阻塞链能观察到表锁造成的等待。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_c_lock_waits_reports_blocking_chain() {
    let db = connect().await;
    prepare(&db).await;

    let mut tx = db.transaction().await.expect("开启事务应成功");
    let rows: Vec<(i32,)> = tx
        .query_with_params("SELECT pg_backend_pid()", vec![])
        .await
        .expect("读取阻塞方 PID 应成功");
    let blocker_pid = rows[0].0;
    tx.lock_table(table!("phase_c.locks"), TableLockMode::AccessExclusive)
        .await
        .expect("LOCK TABLE 应成功");

    // 另一条连接上的普通 SELECT 会被 ACCESS EXCLUSIVE 锁阻塞。
    let blocked_db = connect().await;
    let handle = tokio::spawn(async move {
        let _ = blocked_db
            .execute_with_params("SELECT * FROM phase_c.locks", vec![])
            .await;
    });

    let mut found = false;
    for _ in 0..50 {
        let waits = db.lock_waits().await.expect("pg_locks 查询应成功");
        if waits.iter().any(|wait| wait.blocking_pid == blocker_pid) {
            found = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    tx.commit().await.expect("释放表锁应成功");
    handle.await.expect("阻塞查询任务应正常结束");
    assert!(found, "应观察到 LOCK TABLE 造成的阻塞链");
}

/// SET LOCAL：值绑定参数、事务内可见、提交后不泄漏；非法键名被拒绝。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_c_set_local_is_transaction_scoped_and_rejects_bad_keys() {
    let db = connect().await;

    let mut tx = db.transaction().await.expect("开启事务应成功");
    tx.set_local("app.tenant_id", "tenant-42")
        .await
        .expect("set_local 应成功");
    let rows: Vec<(Option<String>,)> = tx
        .query_with_params("SELECT current_setting('app.tenant_id', true)", vec![])
        .await
        .expect("读取事务内 GUC 应成功");
    let inside = rows[0].0.clone();
    assert_eq!(inside.as_deref(), Some("tenant-42"));
    tx.commit().await.expect("提交事务应成功");

    // SET LOCAL 在事务结束后恢复：自定义 GUC 变回空串（或未定义）。
    let rows: Vec<(Option<String>,)> = db
        .query_with_params("SELECT current_setting('app.tenant_id', true)", vec![])
        .await
        .expect("读取事务外 GUC 应成功");
    let after = rows[0].0.clone();
    assert!(
        after.is_none() || after.as_deref() == Some(""),
        "SET LOCAL 不得泄漏到事务外，实得 {after:?}"
    );

    let mut tx = db.transaction().await.expect("开启事务应成功");
    for invalid in ["app.tenant_id; DROP TABLE t", "app..tenant", "app tenant"] {
        assert!(
            matches!(
                tx.set_local(invalid, "x").await,
                Err(DbError::InvalidArgument(_))
            ),
            "非法 GUC 键名应被拒绝: {invalid:?}"
        );
    }
    tx.rollback().await.expect("回滚事务应成功");
}

/// 版本探测：`server_version_num` 缓存到实例，`supports` 按阈值比较。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_c_server_version_is_cached_and_drives_supports() {
    let db = connect().await;

    let version = db.server_version_num().await.expect("版本探测应成功");
    assert!(
        version >= 140_000,
        "Phase C 需要 PostgreSQL 14+，实得 {version}"
    );
    assert_eq!(
        version,
        db.server_version_num().await.expect("缓存读取应成功"),
        "二次读取应命中实例缓存"
    );
    assert!(db.supports(130_000).await.expect("supports 应成功"));
    assert!(!db.supports(999_999).await.expect("supports 应成功"));
}

/// 2PC：`max_prepared_transactions=0` 时返回明确错误；启用时可 prepare/commit；
/// GID 白名单校验在触库前拦截注入。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_c_two_phase_commit_reports_disabled_or_round_trips() {
    let db = connect().await;

    assert!(matches!(
        db.commit_prepared("bad'gid").await,
        Err(DbError::InvalidArgument(_))
    ));
    assert!(matches!(
        db.rollback_prepared("bad gid").await,
        Err(DbError::InvalidArgument(_))
    ));

    let rows: Vec<(String,)> = db
        .query_with_params("SHOW max_prepared_transactions", vec![])
        .await
        .expect("读取 max_prepared_transactions 应成功");
    let max_prepared = rows[0].0.clone();

    let mut tx = db.transaction().await.expect("开启事务应成功");
    tx.execute_with_params("SELECT 1", vec![])
        .await
        .expect("事务内语句应成功");

    if max_prepared.trim() == "0" {
        let error = db
            .prepare_transaction(tx, "phase_c_disabled_gid")
            .await
            .expect_err("max_prepared_transactions=0 时 PREPARE 必须失败");
        assert!(
            matches!(error, DbError::TransactionError(_)),
            "应返回明确的两阶段提交错误，实得 {error:?}"
        );
        assert!(
            error.to_string().contains("max_prepared_transactions"),
            "错误信息应指出配置项，实得 {error}"
        );
    } else {
        db.prepare_transaction(tx, "phase_c_enabled_gid")
            .await
            .expect("启用 2PC 时 prepare 应成功");
        db.commit_prepared("phase_c_enabled_gid")
            .await
            .expect("commit_prepared 应成功");
    }
}
