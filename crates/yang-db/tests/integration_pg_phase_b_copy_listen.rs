#![allow(clippy::expect_used, clippy::unwrap_used)]
//! PostgreSQL Phase B 集成测试：COPY 导入/导出与 LISTEN/NOTIFY。
//!
//! 所有测试均标记 `#[ignore]`，默认离线套件不触库。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-db --no-default-features --features postgres \
//!   --test integration_pg_phase_b_copy_listen -- --ignored --test-threads=1
//! ```

use serde_json::json;
use std::time::Duration;
use tokio::time::timeout;
use yang_db::postgres::{Database, PgNotificationListener};

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
    db.execute_with_params("DROP SCHEMA IF EXISTS phase_b_copy CASCADE", vec![])
        .await
        .expect("清理旧 schema 应成功");
    db.execute_with_params("CREATE SCHEMA phase_b_copy", vec![])
        .await
        .expect("创建 schema 应成功");
    db.execute_with_params(
        "CREATE TABLE phase_b_copy.items (
            id BIGINT NOT NULL,
            name TEXT NOT NULL,
            amount INT NOT NULL
        )",
        vec![],
    )
    .await
    .expect("创建 items 应成功");
}

/// COPY FROM STDIN：1000 行 CSV 导入后计数与抽样内容一致。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_b_copy_from_csv_imports_1000_rows() {
    let db = connect().await;
    prepare(&db).await;

    let rows: Vec<String> = (1..=1000)
        .map(|i| format!("{i},item-{i},{}", i * 3))
        .collect();
    let affected = db
        .copy_from_csv(
            "COPY phase_b_copy.items (id, name, amount) FROM STDIN WITH (FORMAT csv)",
            rows,
        )
        .await
        .expect("COPY 导入应成功");
    assert_eq!(affected, 1000, "COPY 应返回服务端确认的 1000 行");

    let counts: Vec<(i64,)> = db
        .query_with_params("SELECT COUNT(*) FROM phase_b_copy.items", vec![])
        .await
        .expect("计数查询应成功");
    assert_eq!(counts[0].0, 1000);

    // 抽样校验首行、末行与总量，避免「行都写进去了但内容错位」。
    let sample: Vec<(i64, String, i32)> = db
        .query_with_params(
            "SELECT id, name, amount FROM phase_b_copy.items WHERE id IN (1, 500, 1000) ORDER BY id",
            vec![],
        )
        .await
        .expect("抽样查询应成功");
    assert_eq!(
        sample,
        vec![
            (1, "item-1".to_string(), 3),
            (500, "item-500".to_string(), 1500),
            (1000, "item-1000".to_string(), 3000),
        ]
    );

    // 缺少行尾换行的记录也必须被接受。
    let tail = db
        .copy_from_csv(
            "COPY phase_b_copy.items (id, name, amount) FROM STDIN WITH (FORMAT csv)",
            ["1001,tail-no-newline,7".to_string()],
        )
        .await
        .expect("无行尾换行的 CSV 行应可导入");
    assert_eq!(tail, 1);
}

/// COPY TO STDOUT：输出为可解析的 CSV 文本（含表头）。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_b_copy_to_csv_output_is_parseable() {
    let db = connect().await;
    prepare(&db).await;
    db.execute_with_params(
        "INSERT INTO phase_b_copy.items (id, name, amount) VALUES
            (1, 'alice', 10),
            (2, 'bob', 20),
            (3, 'carol', 30)",
        vec![],
    )
    .await
    .expect("插入样本行应成功");

    let bytes = db
        .copy_to_csv(
            "COPY (SELECT id, name, amount FROM phase_b_copy.items ORDER BY id) \
             TO STDOUT WITH (FORMAT csv, HEADER)",
        )
        .await
        .expect("COPY 导出应成功");

    let text = String::from_utf8(bytes).expect("COPY OUT 输出应为 UTF-8");
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("id,name,amount"), "应包含 CSV 表头");

    let mut parsed: Vec<(i64, String, i32)> = Vec::new();
    for line in lines {
        let fields: Vec<&str> = line.split(',').collect();
        assert_eq!(fields.len(), 3, "每行应有 3 个 CSV 字段: {line:?}");
        parsed.push((
            fields[0].parse().expect("id 应为整数"),
            fields[1].to_string(),
            fields[2].parse().expect("amount 应为整数"),
        ));
    }
    assert_eq!(
        parsed,
        vec![
            (1, "alice".to_string(), 10),
            (2, "bob".to_string(), 20),
            (3, "carol".to_string(), 30),
        ]
    );
}

/// LISTEN/NOTIFY：监听连接与发送连接相互独立；自动提交的通知立即投递，
/// 事务内的通知在提交后才投递（回滚前不会到达）。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_b_listen_notify_delivers_after_commit() {
    let db = connect().await;
    let mut listener = PgNotificationListener::connect(test_db_url())
        .await
        .expect("监听器应能建立独立连接");
    listener
        .listen(["phase_b_copy_events"])
        .await
        .expect("LISTEN 应成功");
    assert_eq!(listener.channels(), ["phase_b_copy_events"]);

    // 另一条连接（连接池）发送通知。
    db.notify("phase_b_copy_events", "autocommit")
        .await
        .expect("pg_notify 应成功");
    let notification = timeout(Duration::from_secs(10), listener.recv())
        .await
        .expect("应在超时前收到通知")
        .expect("接收通知应成功");
    assert_eq!(notification.channel(), "phase_b_copy_events");
    assert_eq!(notification.payload(), "autocommit");
    assert!(notification.process_id() > 0, "应携带发送方后端进程 ID");

    // 事务内发送：提交前不得投递。
    let mut tx = db.transaction().await.expect("开启事务应成功");
    tx.execute_with_params(
        "SELECT pg_notify($1, $2)",
        vec![json!("phase_b_copy_events"), json!("after-commit")],
    )
    .await
    .expect("事务内 pg_notify 应成功");
    assert!(
        timeout(Duration::from_millis(300), listener.recv())
            .await
            .is_err(),
        "事务提交前不得投递通知（提交后投递语义）"
    );
    tx.commit().await.expect("提交事务应成功");

    let notification = timeout(Duration::from_secs(10), listener.recv())
        .await
        .expect("提交后应在超时前收到通知")
        .expect("接收通知应成功");
    assert_eq!(notification.channel(), "phase_b_copy_events");
    assert_eq!(notification.payload(), "after-commit");
}

/// 断线重订阅：终止监听连接的后端后，`reconnect` 建立新连接并恢复订阅。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_b_listen_reconnect_resubscribes_channels() {
    let db = connect().await;
    let mut listener = PgNotificationListener::connect(test_db_url())
        .await
        .expect("监听器应能建立独立连接");
    listener
        .listen(["phase_b_copy_reconnect", "phase_b_copy_extra"])
        .await
        .expect("LISTEN 应成功");
    let old_pid = listener.backend_pid().await.expect("读取后端 PID 应成功");

    // 从另一条连接终止监听后端，模拟断线。
    db.execute_with_params("SELECT pg_terminate_backend($1::int)", vec![json!(old_pid)])
        .await
        .expect("终止监听后端应成功");

    listener.reconnect().await.expect("重连并重订阅应成功");
    let new_pid = listener.backend_pid().await.expect("读取新后端 PID 应成功");
    assert_ne!(new_pid, old_pid, "重连后应使用新的服务端连接");
    assert_eq!(
        listener.channels(),
        ["phase_b_copy_reconnect", "phase_b_copy_extra"],
        "重连后订阅列表应保持不变"
    );

    db.notify("phase_b_copy_reconnect", "after-reconnect")
        .await
        .expect("重连后发送通知应成功");
    let notification = timeout(Duration::from_secs(10), listener.recv())
        .await
        .expect("重连后应在超时前收到通知")
        .expect("接收通知应成功");
    assert_eq!(notification.channel(), "phase_b_copy_reconnect");
    assert_eq!(notification.payload(), "after-reconnect");
}
