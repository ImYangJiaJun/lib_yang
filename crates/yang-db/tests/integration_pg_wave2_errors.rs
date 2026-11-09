#![cfg(feature = "postgres")]
#![allow(clippy::expect_used, clippy::unwrap_used)]
//! PostgreSQL Wave 2 结构化错误契约真库集成测试。
//!
//! 覆盖外键违例（SQLSTATE 23503）与非空违例（23502）经 `sqlx::Error` 映射到
//! [`yang_db::DbError::DatabaseError`] 后，SQLSTATE、约束名、表名、列名等诊断字段
//! 均由真实服务端报文保留，而不是空值。
//!
//! 所有用例标记 `#[ignore]`。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-db --no-default-features --features postgres \
//!   --test integration_pg_wave2_errors -- --ignored --test-threads=1
//! ```

use yang_db::postgres::Database;
use yang_db::DbError;

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
    for sql in [
        "DROP SCHEMA IF EXISTS wave2_errors CASCADE",
        "CREATE SCHEMA wave2_errors",
        "CREATE TABLE wave2_errors.parent (id INT PRIMARY KEY)",
        "CREATE TABLE wave2_errors.child (
            id INT PRIMARY KEY,
            parent_id INT NOT NULL,
            label TEXT NOT NULL,
            CONSTRAINT wave2_errors_child_parent_fk
                FOREIGN KEY (parent_id) REFERENCES wave2_errors.parent (id)
        )",
        "INSERT INTO wave2_errors.parent (id) VALUES (1)",
    ] {
        db.execute_with_params(sql, vec![])
            .await
            .unwrap_or_else(|error| panic!("准备 SQL 失败: {sql}: {error}"));
    }
}

async fn cleanup(db: &Database) {
    db.execute_with_params("DROP SCHEMA IF EXISTS wave2_errors CASCADE", vec![])
        .await
        .expect("清理 wave2_errors schema 应成功");
}

/// 外键违例：错误必须是数据库执行失败，并保留 SQLSTATE、约束名与子表名。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn foreign_key_violation_preserves_sqlstate_constraint_and_table() {
    let db = connect().await;
    prepare(&db).await;

    let error = db
        .execute_with_params(
            "INSERT INTO wave2_errors.child (id, parent_id, label) VALUES (2, 999, 'orphan')",
            vec![],
        )
        .await
        .expect_err("引用不存在的父行必须触发外键违例");

    assert!(
        matches!(error, DbError::DatabaseError(_)),
        "外键违例应映射为结构化 DatabaseError: {error:?}"
    );
    assert_eq!(error.sqlstate(), Some("23503"), "{error:?}");
    assert_eq!(
        error.constraint(),
        Some("wave2_errors_child_parent_fk"),
        "约束名必须来自真实服务端报文: {error:?}"
    );
    let info = error
        .sql_error()
        .expect("DatabaseError 必须携带 SqlErrorInfo");
    assert_eq!(
        info.table(),
        Some("child"),
        "表名应指向触发违例的子表: {error:?}"
    );

    cleanup(&db).await;
}

/// 非空违例：SQLSTATE 为 23502，且列名由服务端诊断字段保留。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn not_null_violation_preserves_sqlstate_and_column() {
    let db = connect().await;
    prepare(&db).await;

    let error = db
        .execute_with_params(
            "INSERT INTO wave2_errors.child (id, parent_id, label) VALUES (3, 1, NULL)",
            vec![],
        )
        .await
        .expect_err("NULL 写入 NOT NULL 列必须触发非空违例");

    assert!(
        matches!(error, DbError::DatabaseError(_)),
        "非空违例应映射为结构化 DatabaseError: {error:?}"
    );
    assert_eq!(error.sqlstate(), Some("23502"), "{error:?}");
    let info = error
        .sql_error()
        .expect("DatabaseError 必须携带 SqlErrorInfo");
    assert_eq!(
        info.column(),
        Some("label"),
        "列名必须来自真实服务端诊断字段: {error:?}"
    );

    cleanup(&db).await;
}
