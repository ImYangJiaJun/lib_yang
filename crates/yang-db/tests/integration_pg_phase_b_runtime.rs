#![allow(clippy::expect_used, clippy::unwrap_used)]
//! PostgreSQL Phase B 运行时集成测试：保存点、事务选项、序列化冲突重试、
//! advisory lock、语句/锁超时与流式游标。
//!
//! 所有测试均标记 `#[ignore]`，默认离线套件不触库。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-db --no-default-features --features postgres \
//!   --test integration_pg_phase_b_runtime -- --ignored --test-threads=1
//! ```

use serde_json::json;
use sqlx::Row as _;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use yang_db::postgres::transaction::TransactionOptions;
use yang_db::postgres::{Database, DatabaseConfig};
use yang_db::{DbError, DbErrorCategory, IsolationLevel, SqlErrorKind};

fn test_db_url() -> String {
    std::env::var("PG_TEST_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432/test".to_string())
}

async fn connect() -> Database {
    Database::connect(&test_db_url())
        .await
        .expect("PG_TEST_URL 指向的 PostgreSQL 必须可连接")
}

/// 创建测试 schema 并重建指定表。
async fn prepare_table(db: &Database, table: &str, columns: &str) {
    db.execute_with_params("CREATE SCHEMA IF NOT EXISTS phase_b_it", vec![])
        .await
        .expect("创建 schema 应成功");
    db.execute_with_params(&format!("DROP TABLE IF EXISTS phase_b_it.{table}"), vec![])
        .await
        .expect("删除旧表应成功");
    db.execute_with_params(
        &format!("CREATE TABLE phase_b_it.{table} ({columns})"),
        vec![],
    )
    .await
    .expect("创建测试表应成功");
}

/// 保存点局部回滚：内层回滚不得影响外层，外层回滚再撤销内层之前的工作。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_b_savepoint_partial_rollback() {
    let db = connect().await;
    prepare_table(
        &db,
        "savepoint_items",
        "id BIGSERIAL PRIMARY KEY, tag TEXT NOT NULL",
    )
    .await;

    let mut tx = db.transaction().await.expect("开启事务应成功");
    tx.execute_with_params(
        "INSERT INTO phase_b_it.savepoint_items (tag) VALUES ($1)",
        vec![json!("outer")],
    )
    .await
    .expect("外层插入应成功");
    tx.savepoint("sp_outer")
        .await
        .expect("创建 sp_outer 应成功");

    tx.execute_with_params(
        "INSERT INTO phase_b_it.savepoint_items (tag) VALUES ($1)",
        vec![json!("inner")],
    )
    .await
    .expect("内层插入应成功");
    tx.savepoint("sp_inner")
        .await
        .expect("创建 sp_inner 应成功");
    tx.execute_with_params(
        "INSERT INTO phase_b_it.savepoint_items (tag) VALUES ($1)",
        vec![json!("deep")],
    )
    .await
    .expect("更深层插入应成功");

    // 回滚到 sp_inner：只撤销 deep，inner 与 outer 保留（外层不受影响）。
    tx.rollback_to("sp_inner")
        .await
        .expect("回滚到 sp_inner 应成功");
    let tags: Vec<(String,)> = tx
        .query_with_params(
            "SELECT tag FROM phase_b_it.savepoint_items ORDER BY id",
            vec![],
        )
        .await
        .expect("保存点内查询应成功");
    assert_eq!(
        tags,
        vec![("outer".to_string(),), ("inner".to_string(),)],
        "回滚到 sp_inner 只应撤销 deep"
    );

    // ROLLBACK TO 之后保存点仍存在，可以释放。
    tx.release_savepoint("sp_inner")
        .await
        .expect("释放 sp_inner 应成功");

    // 再回滚到外层保存点：inner 被撤销，outer 仍在。
    tx.rollback_to("sp_outer")
        .await
        .expect("回滚到 sp_outer 应成功");
    let tags: Vec<(String,)> = tx
        .query_with_params(
            "SELECT tag FROM phase_b_it.savepoint_items ORDER BY id",
            vec![],
        )
        .await
        .expect("保存点内查询应成功");
    assert_eq!(
        tags,
        vec![("outer".to_string(),)],
        "回滚到 sp_outer 只应撤销 inner"
    );

    // 非法保存点名必须被本地拒绝，不得拼进 SQL。
    assert!(matches!(
        tx.savepoint("sp; DROP TABLE phase_b_it.savepoint_items")
            .await,
        Err(DbError::InvalidArgument(_))
    ));

    tx.release_savepoint("sp_outer")
        .await
        .expect("释放 sp_outer 应成功");
    tx.commit().await.expect("提交事务应成功");

    let rows: Vec<(String,)> = db
        .query_with_params(
            "SELECT tag FROM phase_b_it.savepoint_items ORDER BY id",
            vec![],
        )
        .await
        .expect("提交后查询应成功");
    assert_eq!(
        rows,
        vec![("outer".to_string(),)],
        "提交后只应保留 outer 行"
    );
}

/// 只读事务内写入必须被服务端拒绝，SQLSTATE 25006。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_b_read_only_transaction_rejects_writes() {
    let db = connect().await;
    prepare_table(&db, "read_only_items", "id BIGSERIAL PRIMARY KEY, tag TEXT").await;

    let mut tx = db
        .read_only_transaction()
        .await
        .expect("开启只读事务应成功");
    let error = tx
        .execute_with_params(
            "INSERT INTO phase_b_it.read_only_items (tag) VALUES ($1)",
            vec![json!("should-fail")],
        )
        .await
        .expect_err("只读事务内写入必须失败");
    assert_eq!(
        error.sqlstate(),
        Some("25006"),
        "只读事务写入应返回 SQLSTATE 25006，实得 {error:?}"
    );
    tx.rollback().await.expect("回滚只读事务应成功");

    // 通过 transaction_with_options 显式声明只读，行为一致。
    let mut tx = db
        .transaction_with_options(TransactionOptions::default().with_read_only(true))
        .await
        .expect("按选项开启只读事务应成功");
    let error = tx
        .execute_with_params(
            "UPDATE phase_b_it.read_only_items SET tag = $1",
            vec![json!("x")],
        )
        .await
        .expect_err("只读事务内更新必须失败");
    assert_eq!(error.sqlstate(), Some("25006"));
    tx.rollback().await.expect("回滚只读事务应成功");
}

/// 序列化冲突（40001）触发重试，且只重试可重试错误。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_b_serialization_conflict_is_retried() {
    let db = connect().await;
    prepare_table(&db, "retry_rows", "id INT PRIMARY KEY, v INT NOT NULL").await;
    db.execute_with_params(
        "INSERT INTO phase_b_it.retry_rows (id, v) VALUES (1, 0)",
        vec![],
    )
    .await
    .expect("种子行插入应成功");

    let pool = db.pool().clone();
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_for_closure = Arc::clone(&attempts);

    db.retry_transaction_with_options(
        TransactionOptions::default().with_isolation(IsolationLevel::Serializable),
        move |mut tx: yang_db::postgres::Transaction| {
            let pool = pool.clone();
            let attempts = Arc::clone(&attempts_for_closure);
            async move {
                let attempt = attempts.fetch_add(1, Ordering::SeqCst);
                let rows: Vec<(i32,)> = tx
                    .query_with_params("SELECT v FROM phase_b_it.retry_rows WHERE id = 1", vec![])
                    .await?;
                assert_eq!(rows.len(), 1, "种子行必须存在");

                if attempt == 0 {
                    // 在本次事务读取之后、写入之前，由另一个连接提交一次更新，
                    // 使本次事务的 UPDATE 命中「行已被并发更新」→ 40001。
                    let mut other = Database::from_pool(pool, DatabaseConfig::default())?
                        .transaction_with_options(
                            TransactionOptions::default()
                                .with_isolation(IsolationLevel::Serializable),
                        )
                        .await?;
                    other
                        .execute_with_params(
                            "UPDATE phase_b_it.retry_rows SET v = v + 100 WHERE id = 1",
                            vec![],
                        )
                        .await?;
                    other.commit().await?;
                }

                tx.execute_with_params(
                    "UPDATE phase_b_it.retry_rows SET v = v + 1 WHERE id = 1",
                    vec![],
                )
                .await?;
                tx.commit().await?;
                Ok(())
            }
        },
        3,
    )
    .await
    .expect("序列化冲突应被重试并最终成功");

    assert_eq!(
        attempts.load(Ordering::SeqCst),
        2,
        "应恰好重试一次（首次 40001，第二次成功）"
    );
    let rows: Vec<(i32,)> = db
        .query_with_params("SELECT v FROM phase_b_it.retry_rows WHERE id = 1", vec![])
        .await
        .expect("最终查询应成功");
    assert_eq!(rows[0].0, 101, "首次读到 0，外部 +100，重试后 +1");
}

/// 会话级 advisory lock 在两条连接间互斥；事务级锁随事务结束释放。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_b_advisory_lock_mutual_exclusion() {
    let db = connect().await;
    prepare_table(&db, "advisory_items", "id INT PRIMARY KEY").await;

    let key = ((std::process::id() as i64) << 32) | 0x4c4f_434b;
    let guard = db.advisory_lock(key).await.expect("获取会话级锁应成功");
    assert_eq!(guard.key(), key);
    assert!(
        db.try_advisory_lock(key)
            .await
            .expect("尝试获取锁应成功")
            .is_none(),
        "另一连接不得重复获取同一会话级锁"
    );
    assert!(guard.unlock().await.expect("解锁应成功"));

    let guard = db
        .try_advisory_lock(key)
        .await
        .expect("解锁后尝试获取应成功")
        .expect("解锁后应能重新获取");
    assert!(guard.advisory_unlock().await.expect("再次解锁应成功"));

    // 事务级锁：未提交前其他事务无法获取，回滚后自动释放。
    let xact_key = key + 1;
    let mut holder = db.transaction().await.expect("开启事务应成功");
    holder
        .advisory_xact_lock(xact_key)
        .await
        .expect("获取事务级锁应成功");

    let mut probe = db.transaction().await.expect("开启探测事务应成功");
    assert!(
        !probe
            .try_advisory_xact_lock(xact_key)
            .await
            .expect("尝试事务级锁应成功"),
        "事务级锁被占用时 try 应返回 false"
    );
    probe.rollback().await.expect("回滚探测事务应成功");

    holder.rollback().await.expect("回滚持锁事务应成功");

    let mut after = db.transaction().await.expect("开启事务应成功");
    assert!(
        after
            .try_advisory_xact_lock(xact_key)
            .await
            .expect("回滚后尝试事务级锁应成功"),
        "持有锁的事务回滚后锁应释放"
    );
    after.rollback().await.expect("回滚事务应成功");
}

/// statement_timeout 触发 57014，且映射到 QueryCanceled / Timeout。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_b_statement_timeout_maps_to_57014() {
    let db = Database::connect_with_config(
        &test_db_url(),
        DatabaseConfig::default().with_statement_timeout_ms(150u64),
    )
    .await
    .expect("带 statement_timeout 的连接应成功");

    let error = db
        .execute_with_params("SELECT pg_sleep(2)", vec![])
        .await
        .expect_err("pg_sleep(2) 应被 statement_timeout 取消");
    assert_eq!(
        error.sqlstate(),
        Some("57014"),
        "语句超时应返回 SQLSTATE 57014，实得 {error:?}"
    );
    let info = error.sql_error().expect("应为结构化数据库错误");
    assert_eq!(info.kind(), SqlErrorKind::QueryCanceled);
    assert_eq!(error.category(), DbErrorCategory::Timeout);

    // 取消只终止语句，连接仍可继续使用。
    assert!(db.health_check().await.expect("超时后健康检查应成功"));
}

/// lock_timeout 触发 55P03，且映射到 LockNotAvailable。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_b_lock_timeout_maps_to_55p03() {
    let db = Database::connect_with_config(
        &test_db_url(),
        DatabaseConfig::default().with_lock_timeout_ms(150u64),
    )
    .await
    .expect("带 lock_timeout 的连接应成功");
    prepare_table(&db, "lock_rows", "id INT PRIMARY KEY, v INT").await;
    db.execute_with_params(
        "INSERT INTO phase_b_it.lock_rows (id, v) VALUES (1, 0)",
        vec![],
    )
    .await
    .expect("种子行插入应成功");

    let mut holder = db.transaction().await.expect("开启持锁事务应成功");
    holder
        .execute_with_params(
            "SELECT id FROM phase_b_it.lock_rows WHERE id = 1 FOR UPDATE",
            vec![],
        )
        .await
        .expect("持锁事务应成功锁定行");

    let error = db
        .execute_with_params(
            "SELECT id FROM phase_b_it.lock_rows WHERE id = 1 FOR UPDATE",
            vec![],
        )
        .await
        .expect_err("行已锁定，第二连接应等待超时");
    assert_eq!(
        error.sqlstate(),
        Some("55P03"),
        "锁等待超时应返回 SQLSTATE 55P03，实得 {error:?}"
    );
    assert_eq!(
        error.sql_error().expect("应为结构化错误").kind(),
        SqlErrorKind::LockNotAvailable
    );

    holder.rollback().await.expect("回滚持锁事务应成功");
}

/// 连接池流式游标：分批读取全部行、支持参数绑定、读尽后归还连接。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_b_stream_query_iterates_in_batches() {
    let db = connect().await;
    prepare_table(&db, "stream_rows", "id INT PRIMARY KEY, v INT NOT NULL").await;
    db.execute_with_params(
        "INSERT INTO phase_b_it.stream_rows (id, v)
         SELECT g, g * 2 FROM generate_series(1, 250) AS g",
        vec![],
    )
    .await
    .expect("批量插入应成功");

    let mut cursor = db
        .stream_query_with_batch_size(
            "SELECT id, v FROM phase_b_it.stream_rows WHERE id > $1 ORDER BY id;",
            vec![json!(0)],
            40,
        )
        .await
        .expect("创建流式游标应成功");
    assert_eq!(cursor.batch_size(), 40);

    let mut seen = Vec::new();
    while let Some(row) = cursor.next().await.expect("游标读取应成功") {
        let id: i32 = row.try_get("id").expect("id 应解码");
        let v: i32 = row.try_get("v").expect("v 应解码");
        seen.push((id, v));
    }
    assert_eq!(seen.len(), 250, "应逐行读完全部行");
    assert_eq!(seen.first(), Some(&(1, 2)));
    assert_eq!(seen.last(), Some(&(250, 500)));

    // 游标读尽后内部事务已回滚、连接已归还，后续查询正常。
    let count: i64 = db
        .table(yang_db::table!("phase_b_it.stream_rows"))
        .count()
        .await
        .expect("读尽后查询应成功");
    assert_eq!(count, 250);

    // 提前 drop 未读尽的游标：连接被关闭而非带事务回池，池仍可用。
    {
        let mut cursor = db
            .stream_query("SELECT id FROM phase_b_it.stream_rows ORDER BY id", vec![])
            .await
            .expect("创建游标应成功");
        let first = cursor.next().await.expect("读取首行应成功");
        assert!(first.is_some());
        // cursor 在此作用域结束时 drop
    }
    assert!(db
        .health_check()
        .await
        .expect("提前丢弃游标后健康检查应成功"));
}

/// 事务内流式游标：游标读尽或被丢弃后，事务仍可继续使用。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_b_transaction_stream_query() {
    let db = connect().await;
    prepare_table(&db, "tx_stream_rows", "id INT PRIMARY KEY, v INT NOT NULL").await;
    db.execute_with_params(
        "INSERT INTO phase_b_it.tx_stream_rows (id, v)
         SELECT g, g FROM generate_series(1, 120) AS g",
        vec![],
    )
    .await
    .expect("批量插入应成功");

    let mut tx = db.transaction().await.expect("开启事务应成功");

    {
        let mut cursor = tx
            .stream_query_with_batch_size(
                "SELECT id FROM phase_b_it.tx_stream_rows WHERE id >= $1 ORDER BY id",
                vec![json!(1)],
                25,
            )
            .await
            .expect("事务内创建游标应成功");
        let mut count = 0;
        while let Some(row) = cursor.next().await.expect("事务内游标读取应成功") {
            let id: i32 = row.try_get("id").expect("id 应解码");
            count += 1;
            assert_eq!(id, count, "行应按 id 顺序返回");
        }
        assert_eq!(count, 120);
        cursor.close().await.expect("关闭游标应成功");
    }

    // 未读尽即丢弃游标，事务依然可继续执行查询。
    {
        let mut cursor = tx
            .stream_query(
                "SELECT id FROM phase_b_it.tx_stream_rows ORDER BY id",
                vec![],
            )
            .await
            .expect("事务内创建游标应成功");
        assert!(cursor.next().await.expect("读取首行应成功").is_some());
    }
    let rows: Vec<(i32,)> = tx
        .query_with_params("SELECT 1::INT AS v", vec![])
        .await
        .expect("游标丢弃后事务内查询应成功");
    assert_eq!(rows[0].0, 1);

    tx.rollback().await.expect("回滚事务应成功");
}
