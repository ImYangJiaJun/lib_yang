#![allow(deprecated)]
#![allow(clippy::expect_used, clippy::unwrap_used)]
// PostgreSQL 事务行为集成测试
// 验证 drop-without-commit 自动回滚与并发事务隔离（无脏读）。
// 所有触达数据库的测试都标记 #[ignore]，默认 `cargo test` 套件保持离线。

use serde_json::json;
use sqlx::FromRow;
use yang_db::postgres::Database;

/// 测试数据库连接字符串。
/// 可用环境变量 `PG_TEST_URL` 覆盖。
fn test_db_url() -> String {
    std::env::var("PG_TEST_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5433/test".to_string())
}

/// 辅助结构体：从测试表读取一行
#[derive(Debug, FromRow)]
#[allow(dead_code)]
struct TxRow {
    id: i64,
    value: i64,
}

/// TEST-7a: 开事务 INSERT → drop 不 commit → 断言行不存在（自动回滚）
///
/// sqlx::Transaction 默认在 Drop 时回滚未提交事务。
/// yang_db::postgres::Transaction 没有自定义 Drop，故 drop 即回滚。
#[tokio::test]
#[ignore = "需要本地 PostgreSQL 实例，默认离线套件跳过"]
async fn test_pg_transaction_rollback_on_drop() {
    let db = Database::connect(&test_db_url())
        .await
        .expect("连接数据库失败");
    let table = yang_db::table!("test_tx_rollback");

    // 准备测试表
    let _ = db.drop_table(table).await;
    db.create_table(&format!(
        "CREATE TABLE {} (
            id BIGSERIAL PRIMARY KEY,
            value BIGINT NOT NULL
        )",
        table
    ))
    .await
    .expect("创建测试表失败");

    // 开事务，插入一行，然后 drop（不 commit）
    {
        let mut tx = db.transaction().await.expect("开启事务失败");
        tx.table(table)
            .insert(&json!({"value": 42}))
            .await
            .expect("事务内 INSERT 失败");
        // tx 在此作用域结束时被 drop，sqlx 自动 ROLLBACK
    }

    // 在事务外查询：行应当不存在
    let rows: Vec<TxRow> = db
        .query_with_params(
            &format!("SELECT id, value FROM {} WHERE value = $1", table),
            vec![json!(42i64)],
        )
        .await
        .expect("查询失败");

    assert!(
        rows.is_empty(),
        "事务 drop 后行不应存在（应自动回滚），但找到 {} 行",
        rows.len()
    );

    // 清理
    let _ = db.drop_table(table).await;
    println!("✓ test_pg_transaction_rollback_on_drop 通过");
}

/// TEST-7b: 两 task 并发操作同一行，验证无脏读
///
/// 流程：
///   1. 插入一行 (value=1)
///   2. Task1 开事务：UPDATE value=2，**不 commit**
///   3. Task2 开事务：SELECT → 应仍看到 value=1（无脏读）
///   4. Task1 commit
///   5. Task2 在同一事务内再 SELECT → 应看到 value=2（READ COMMITTED 语义）
///   6. Task2 UPDATE value=3 并 commit
///   7. 最终验证 value=3
#[tokio::test]
#[ignore = "需要本地 PostgreSQL 实例，默认离线套件跳过"]
async fn test_pg_transaction_concurrent_isolation() {
    let db = Database::connect(&test_db_url())
        .await
        .expect("连接数据库失败");
    let table = yang_db::table!("test_tx_isolation");

    // 准备测试表 + 种子行
    let _ = db.drop_table(table).await;
    db.create_table(&format!(
        "CREATE TABLE {} (
            id BIGSERIAL PRIMARY KEY,
            value BIGINT NOT NULL
        )",
        table
    ))
    .await
    .expect("创建测试表失败");

    db.table(table)
        .insert(&json!({"value": 1}))
        .await
        .expect("种子 INSERT 失败");

    // 用 channel 协调两个 task 的执行顺序
    let (tx1_ready, rx1_ready) = tokio::sync::oneshot::channel::<()>();
    let (tx2_read, rx2_read) = tokio::sync::oneshot::channel::<()>();

    let db_url = test_db_url();

    // Task1: 开事务 → UPDATE → 等 Task2 读完 → commit
    let handle1 = tokio::spawn({
        let url = db_url.clone();
        let tbl = table.clone();
        async move {
            let db = Database::connect(&url).await.expect("task1: 连接失败");
            let mut tx = db.transaction().await.expect("task1: 开启事务失败");

            // UPDATE value=2（在事务内）
            tx.table(&tbl)
                .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 1i64)
                .update(&json!({"value": 2}))
                .await
                .expect("task1: UPDATE 失败");

            // 通知 Task2："我已 UPDATE 但还没 commit"
            let _ = tx1_ready.send(());

            // 等 Task2 完成脏读检测
            let _ = rx2_read.await;

            // commit
            tx.commit().await.expect("task1: commit 失败");
        }
    });

    // Task2: 等 Task1 UPDATE 完成 → SELECT（应看到旧值）→ 通知 Task1 → Task1 commit → 再 SELECT
    let handle2 = tokio::spawn({
        let url = db_url.clone();
        let tbl = table.clone();
        async move {
            let db = Database::connect(&url).await.expect("task2: 连接失败");

            // 等 Task1 完成 UPDATE（未 commit）
            let _ = rx1_ready.await;

            // 开事务，在事务内 SELECT
            let mut tx = db.transaction().await.expect("task2: 开启事务失败");

            let rows: Vec<TxRow> = tx
                .query_with_params(
                    &format!("SELECT id, value FROM {} WHERE id = $1", tbl),
                    vec![json!(1i64)],
                )
                .await
                .expect("task2: 第一次 SELECT 失败");

            assert_eq!(rows.len(), 1, "task2: 应找到 1 行");
            assert_eq!(
                rows[0].value, 1,
                "task2: 第一次 SELECT 应看到 value=1（无脏读），实际看到 {}",
                rows[0].value
            );

            // 通知 Task1 可以 commit 了
            let _ = tx2_read.send(());

            // 给 Task1 一点时间完成 commit
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;

            // 同一事务内再 SELECT → READ COMMITTED 下应看到 Task1 已提交的 value=2
            let rows2: Vec<TxRow> = tx
                .query_with_params(
                    &format!("SELECT id, value FROM {} WHERE id = $1", tbl),
                    vec![json!(1i64)],
                )
                .await
                .expect("task2: 第二次 SELECT 失败");

            assert_eq!(rows2.len(), 1, "task2: 第二次 SELECT 应找到 1 行");
            assert_eq!(
                rows2[0].value, 2,
                "task2: 第二次 SELECT 应看到 value=2（Task1 已提交），实际看到 {}",
                rows2[0].value
            );

            // UPDATE value=3 并 commit
            tx.table(&tbl)
                .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 1i64)
                .update(&json!({"value": 3}))
                .await
                .expect("task2: UPDATE 失败");
            tx.commit().await.expect("task2: commit 失败");
        }
    });

    // 等两个 task 都完成
    let (r1, r2) = tokio::join!(handle1, handle2);
    r1.expect("task1 panic");
    r2.expect("task2 panic");

    // 最终验证
    let final_rows: Vec<TxRow> = db
        .query_with_params(
            &format!("SELECT id, value FROM {} WHERE id = $1", table),
            vec![json!(1i64)],
        )
        .await
        .expect("最终查询失败");

    assert_eq!(final_rows.len(), 1);
    assert_eq!(
        final_rows[0].value, 3,
        "最终 value 应为 3，实际为 {}",
        final_rows[0].value
    );

    // 清理
    let _ = db.drop_table(table).await;
    println!("✓ test_pg_transaction_concurrent_isolation 通过");
}
