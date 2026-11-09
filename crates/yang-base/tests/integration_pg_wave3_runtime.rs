#![cfg(feature = "postgres")]
#![allow(clippy::expect_used, clippy::unwrap_used)]
//! PostgreSQL Wave 3 运行时契约集成测试。
//!
//! 覆盖：
//! - `AnyTransaction` 的嵌套保存点：回滚到外层保存点后，外层已做工作仍可提交；
//! - `EngineConfig.postgres_database` 映射的 `search_path` / `application_name` /
//!   `statement_timeout` / `lock_timeout` 在真实连接上生效。
//!
//! 所有用例标记 `#[ignore]`，默认离线套件不触库。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-base --no-default-features --features postgres \
//!   --test integration_pg_wave3_runtime -- --ignored --test-threads=1
//! ```

use sqlx::postgres::PgPool;
use yang_base::config::EngineConfig;
use yang_base::database::{AnyDatabase, AnyTransaction};
use yang_db::postgres::Database as PgDatabase;
use yang_db::DbError;

fn test_db_url() -> String {
    std::env::var("PG_TEST_URL")
        .or_else(|_| std::env::var("YANG_POSTGRES_URL"))
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432/test".to_string())
}

async fn connect_database() -> PgDatabase {
    PgDatabase::connect(&test_db_url())
        .await
        .expect("PG_TEST_URL / YANG_POSTGRES_URL 指向的 PostgreSQL 必须可连接")
}

async fn execute(pool: &PgPool, sql: &str) {
    sqlx::query(sql)
        .execute(pool)
        .await
        .unwrap_or_else(|error| panic!("执行 SQL 失败: {error}\n{sql}"));
}

/// 进程级唯一表名：多个进程并行跑同一真库时不会互相 DROP/CREATE。
fn test_table(base: &str) -> String {
    format!("{base}_{}", std::process::id())
}

/// 在统一事务句柄内执行一条无参数 SQL。
async fn execute_in_tx(transaction: &mut AnyTransaction, sql: &str) {
    let connection = transaction
        .as_postgres()
        .expect("应持有 PostgreSQL 事务")
        .executor()
        .expect("事务应处于活动状态");
    sqlx::query(sql)
        .execute(connection)
        .await
        .unwrap_or_else(|error| panic!("事务内执行 SQL 失败: {error}\n{sql}"));
}

/// 在真实连接上触发指定 SQLSTATE 的服务端错误（自动提交语句，连接保持可用）。
async fn forced_sqlstate_error(pool: &PgPool, code: &str) -> DbError {
    let sql =
        format!("DO $$ BEGIN RAISE EXCEPTION 'forced {code}' USING ERRCODE = '{code}'; END $$");
    sqlx::query(&sql)
        .execute(pool)
        .await
        .expect_err("DO 块必须抛错")
        .into()
}

#[ignore = "需要真实 PostgreSQL（PG_TEST_URL）"]
#[tokio::test]
async fn retry_transaction_retries_real_serialization_failure() {
    let database = AnyDatabase::postgres(connect_database().await);
    let pool = database
        .as_postgres()
        .expect("应持有 PostgreSQL 后端")
        .pool()
        .clone();
    let pool_ref = &pool;

    let mut attempts = 0;
    let result = database
        .retry_transaction(3, std::time::Duration::ZERO, || {
            attempts += 1;
            let attempt = attempts;
            async move {
                if attempt == 1 {
                    Err(forced_sqlstate_error(pool_ref, "40001").await)
                } else {
                    Ok(attempt)
                }
            }
        })
        .await;

    assert_eq!(result.expect("真实 40001 应被重试而后成功"), 2);
    assert_eq!(attempts, 2);
}

#[ignore = "需要真实 PostgreSQL（PG_TEST_URL）"]
#[tokio::test]
async fn retry_transaction_does_not_retry_real_unique_violation() {
    let database = AnyDatabase::postgres(connect_database().await);
    let pool = database
        .as_postgres()
        .expect("应持有 PostgreSQL 后端")
        .pool()
        .clone();
    let table = test_table("tq_pg_wave3_retry_unique");
    execute(&pool, &format!("DROP TABLE IF EXISTS {table}")).await;
    execute(&pool, &format!("CREATE TABLE {table} (id INT PRIMARY KEY)")).await;
    execute(&pool, &format!("INSERT INTO {table} (id) VALUES (1)")).await;
    let pool_ref = &pool;
    let table_ref = &table;

    let mut attempts = 0;
    let error = database
        .retry_transaction(5, std::time::Duration::ZERO, || {
            attempts += 1;
            async move {
                sqlx::query(&format!("INSERT INTO {table_ref} (id) VALUES (1)"))
                    .execute(pool_ref)
                    .await
                    .map(|_| ())
                    .map_err(DbError::from)
            }
        })
        .await
        .expect_err("唯一约束冲突不应重试");

    assert_eq!(error.sqlstate(), Some("23505"));
    assert_eq!(attempts, 1);
    execute(&pool, &format!("DROP TABLE IF EXISTS {table}")).await;
}

#[ignore = "需要真实 PostgreSQL（PG_TEST_URL）"]
#[tokio::test]
async fn nested_savepoints_keep_only_outer_committed_rows() {
    let pool = connect_database().await.pool().clone();
    let table = test_table("tq_pg_wave3_savepoint");
    execute(&pool, &format!("DROP TABLE IF EXISTS {table}")).await;
    execute(
        &pool,
        &format!(
            "CREATE TABLE {table} (
            id BIGSERIAL PRIMARY KEY,
            label TEXT NOT NULL
        )"
        ),
    )
    .await;

    let database = AnyDatabase::postgres(connect_database().await);
    let mut transaction = database.begin().await.expect("开启事务应成功");

    execute_in_tx(
        &mut transaction,
        &format!("INSERT INTO {table} (label) VALUES ('outer')"),
    )
    .await;
    transaction
        .savepoint("sp_outer")
        .await
        .expect("创建外层保存点");
    execute_in_tx(
        &mut transaction,
        &format!("INSERT INTO {table} (label) VALUES ('inner')"),
    )
    .await;
    transaction
        .savepoint("sp_inner")
        .await
        .expect("创建内层保存点");
    execute_in_tx(
        &mut transaction,
        &format!("INSERT INTO {table} (label) VALUES ('nested')"),
    )
    .await;

    // 回滚到外层保存点：撤销 inner 与 nested，但保留 outer。
    transaction
        .rollback_to("sp_outer")
        .await
        .expect("回滚到外层保存点应成功");
    transaction
        .release("sp_outer")
        .await
        .expect("释放外层保存点");
    execute_in_tx(
        &mut transaction,
        &format!("INSERT INTO {table} (label) VALUES ('after')"),
    )
    .await;
    transaction.commit().await.expect("外层事务提交应成功");

    let labels: Vec<String> = sqlx::query_scalar(&format!("SELECT label FROM {table} ORDER BY id"))
        .fetch_all(&pool)
        .await
        .expect("查询已提交行应成功");
    assert_eq!(labels, vec!["outer".to_string(), "after".to_string()]);
    execute(&pool, &format!("DROP TABLE IF EXISTS {table}")).await;
}

#[ignore = "需要真实 PostgreSQL（PG_TEST_URL）"]
#[tokio::test]
async fn postgres_session_settings_from_env_reach_real_connection() {
    std::env::set_var("YANG_PG_SEARCH_PATH", "pg_catalog,public");
    std::env::set_var("YANG_PG_APPLICATION_NAME", "yang_wave3_runtime");
    std::env::set_var("YANG_PG_STATEMENT_TIMEOUT_MS", "1234");
    std::env::set_var("YANG_PG_LOCK_TIMEOUT_MS", "4321");
    let config = EngineConfig::from_env().expect("PostgreSQL 会话配置应解析成功");
    std::env::remove_var("YANG_PG_SEARCH_PATH");
    std::env::remove_var("YANG_PG_APPLICATION_NAME");
    std::env::remove_var("YANG_PG_STATEMENT_TIMEOUT_MS");
    std::env::remove_var("YANG_PG_LOCK_TIMEOUT_MS");

    let database = PgDatabase::connect_with_config(&test_db_url(), config.postgres_database)
        .await
        .expect("用映射后的配置建立连接应成功");
    let database = AnyDatabase::postgres(database);
    let pool = database
        .as_postgres()
        .expect("应持有 PostgreSQL 后端")
        .pool();

    let search_path: String = sqlx::query_scalar("SHOW search_path")
        .fetch_one(pool)
        .await
        .expect("SHOW search_path 应成功");
    let application_name: String = sqlx::query_scalar("SHOW application_name")
        .fetch_one(pool)
        .await
        .expect("SHOW application_name 应成功");
    let statement_timeout: String = sqlx::query_scalar("SHOW statement_timeout")
        .fetch_one(pool)
        .await
        .expect("SHOW statement_timeout 应成功");
    let lock_timeout: String = sqlx::query_scalar("SHOW lock_timeout")
        .fetch_one(pool)
        .await
        .expect("SHOW lock_timeout 应成功");

    assert!(search_path.contains("pg_catalog"), "实得: {search_path}");
    assert!(search_path.contains("public"), "实得: {search_path}");
    assert_eq!(application_name, "yang_wave3_runtime");
    assert_eq!(statement_timeout, "1234ms");
    assert_eq!(lock_timeout, "4321ms");
}
