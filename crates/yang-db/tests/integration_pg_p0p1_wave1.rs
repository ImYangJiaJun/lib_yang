#![cfg(feature = "postgres")]
#![allow(clippy::expect_used, clippy::unwrap_used)]
//! PostgreSQL P0/P1 第一波真库集成测试。
//!
//! 覆盖 shared advisory lock、CancelToken/57014、运行时配置（application_name /
//! statement cache / DISCARD ALL / 池等待）、TLS 失败路径、client_encoding 校验、
//! dollar-quote 脚本执行、标识符规则，以及无需真库的 SQLSTATE 重试码表与能力声明。
//!
//! 真库用例均标记 `#[ignore]`。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-db --no-default-features --features postgres \
//!   --test integration_pg_p0p1_wave1 -- --ignored --test-threads=1
//! ```

use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use yang_db::postgres::{quote_identifier, quote_table, Database, DatabaseConfig, SslMode};
use yang_db::{
    BackendCapability, DbError, DbErrorCategory, SafetyConstraint, SqlErrorInfo, SqlErrorKind,
};

fn test_db_url() -> String {
    std::env::var("PG_TEST_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432/test".to_string())
}

async fn connect() -> Database {
    Database::connect(&test_db_url())
        .await
        .expect("PG_TEST_URL 指向的 PostgreSQL 必须可连接")
}

/// 在连接串末尾追加查询参数（兼容已有 query）。
fn with_query_param(url: &str, param: &str) -> String {
    let separator = if url.contains('?') { '&' } else { '?' };
    format!("{url}{separator}{param}")
}

// ---- 纯逻辑：SQLSTATE 重试码表 ----

#[test]
fn wave1_is_retryable_covers_exactly_postgres_transient_classes() {
    // (SQLSTATE, is_retryable, 期望分类)
    let cases: &[(&str, bool, DbErrorCategory)] = &[
        ("40001", true, DbErrorCategory::Transient),
        ("40P01", true, DbErrorCategory::Transient),
        ("08000", true, DbErrorCategory::Transient),
        ("08006", true, DbErrorCategory::Transient),
        ("53300", true, DbErrorCategory::Transient),
        ("53100", true, DbErrorCategory::Transient),
        ("55P03", false, DbErrorCategory::Timeout),
        ("57014", false, DbErrorCategory::Timeout),
        ("25P02", false, DbErrorCategory::Client),
        ("23505", false, DbErrorCategory::Conflict),
        ("42P01", false, DbErrorCategory::NotFound),
        ("42601", false, DbErrorCategory::Client),
        ("XX000", false, DbErrorCategory::Server),
    ];
    for (code, expected_retryable, expected_category) in cases {
        let info = SqlErrorInfo::new(*code, "wave1");
        assert_eq!(
            info.is_retryable(),
            *expected_retryable,
            "SqlErrorInfo({code}) 的 is_retryable 不符"
        );
        let error = DbError::DatabaseError(Box::new(info));
        assert_eq!(
            error.is_retryable(),
            *expected_retryable,
            "DbError({code}) 的 is_retryable 不符"
        );
        assert_eq!(
            error.category(),
            *expected_category,
            "DbError({code}) 的分类不符"
        );
        assert_eq!(error.sqlstate(), Some(*code));
    }
    // 取消与锁超时都不是瞬时故障，也不是 Client。
    assert_eq!(
        SqlErrorInfo::new("57014", "cancel").kind(),
        SqlErrorKind::QueryCanceled
    );
    assert_eq!(
        SqlErrorInfo::new("55P03", "lock").kind(),
        SqlErrorKind::LockNotAvailable
    );
}

// ---- 纯逻辑：能力声明 ----

#[test]
fn wave1_capabilities_declare_postgres_wave1_features() {
    let capabilities = Database::capabilities();
    for capability in [
        BackendCapability::Copy,
        BackendCapability::ListenNotify,
        BackendCapability::Cte,
        BackendCapability::Window,
        BackendCapability::Arrays,
        BackendCapability::Range,
        BackendCapability::Multirange,
        BackendCapability::JsonPath,
        BackendCapability::FullText,
        BackendCapability::Diagnostics,
        BackendCapability::TwoPhaseCommit,
        BackendCapability::TableLock,
        BackendCapability::TransactionLocalConfig,
        BackendCapability::Savepoint,
        BackendCapability::Returning,
        BackendCapability::SkipLocked,
    ] {
        assert!(
            capabilities.supports(capability),
            "POSTGRES_CAPABILITIES 应声明 {capability:?}"
        );
    }
    assert!(capabilities.enforces(SafetyConstraint::ExplicitTrustedExpressions));
    assert!(capabilities.enforces(SafetyConstraint::ParameterizedValues));
}

// ---- 真库：shared advisory lock ----

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave1_shared_advisory_lock_allows_shared_and_blocks_exclusive() {
    let db = connect().await;
    let key = ((std::process::id() as i64) << 32) | 0x5741_5645;

    // 两个共享锁可以同时持有。
    let first = db
        .advisory_lock_shared(key)
        .await
        .expect("获取共享锁应成功");
    assert!(first.is_shared());
    let second = db
        .try_advisory_lock_shared(key)
        .await
        .expect("尝试共享锁应成功")
        .expect("共享锁彼此不互斥");
    assert!(second.is_shared());

    // 同 key 的排他锁必须被共享锁阻塞。
    assert!(
        db.try_advisory_lock(key)
            .await
            .expect("尝试排他锁应成功")
            .is_none(),
        "共享锁持有时排他锁不得获取"
    );

    assert!(first.unlock().await.expect("释放共享锁应成功"));
    // 仍有一把共享锁：排他锁依旧不可用。
    assert!(
        db.try_advisory_lock(key)
            .await
            .expect("尝试排他锁应成功")
            .is_none(),
        "还有共享锁持有时排他锁不得获取"
    );
    assert!(second.unlock().await.expect("释放第二把共享锁应成功"));

    // 全部释放后排他锁可获取。
    let exclusive = db
        .try_advisory_lock(key)
        .await
        .expect("解锁后尝试排他锁应成功")
        .expect("共享锁全部释放后应能获取排他锁");
    assert!(!exclusive.is_shared());
    assert!(exclusive.unlock().await.expect("释放排他锁应成功"));
}

// ---- 真库：CancelToken 取消 -> 57014 / Timeout ----

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave1_cancel_token_cancels_query_with_57014_timeout() {
    let db = connect().await;
    let (mut conn, token) = db
        .acquire_with_cancel_token()
        .await
        .expect("借出带 cancel token 的连接应成功");
    assert!(token.backend_pid() > 0);

    let pid = token.backend_pid();
    let handle =
        tokio::spawn(async move { sqlx::query("SELECT pg_sleep(10)").execute(&mut *conn).await });

    // 等待目标后端真正开始执行 pg_sleep，避免取消落在空闲会话上。
    let mut active = false;
    for _ in 0..100 {
        let rows: Vec<(bool,)> = db
            .query_with_params(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid = $1 AND state = 'active')",
                vec![json!(pid)],
            )
            .await
            .expect("查询 pg_stat_activity 应成功");
        if rows[0].0 {
            active = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(active, "取消前目标后端应处于 active 状态");

    assert!(token.cancel().await.expect("发送取消应成功"));
    let error = handle
        .await
        .expect("取消任务不应 panic")
        .expect_err("pg_sleep 应被取消");
    let error = DbError::from(error);
    assert_eq!(
        error.sqlstate(),
        Some("57014"),
        "取消应产生 SQLSTATE 57014，实得 {error:?}"
    );
    assert_eq!(
        error.sql_error().expect("应为结构化错误").kind(),
        SqlErrorKind::QueryCanceled
    );
    assert_eq!(error.category(), DbErrorCategory::Timeout);
    assert!(!error.is_retryable(), "查询取消不应被视为可重试");
    // 取消只终止语句，连接仍可继续使用。
    assert!(db.health_check().await.expect("取消后健康检查应成功"));
}

// ---- 真库：运行时配置 + 池等待 ----

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave1_runtime_config_application_name_reset_and_waiting() {
    let mut config = DatabaseConfig::default()
        .with_application_name("yang-db-wave1")
        .with_statement_cache_size(0)
        .with_reset_on_release(true);
    // cancel token 需要另借一条连接，池上限必须 >= 2。
    config.max_connections = 2;
    let db = Arc::new(
        Database::connect_with_config(&test_db_url(), config)
            .await
            .expect("带运行时配置的连接应成功"),
    );

    let name: Vec<(String,)> = db
        .query_with_params("SELECT current_setting('application_name')", vec![])
        .await
        .expect("读取 application_name 应成功");
    assert_eq!(name[0].0, "yang-db-wave1");

    // statement_cache_size = 0：同一参数化查询重复执行仍应成功（仅关闭缓存）。
    for _ in 0..2 {
        let values: Vec<(i32,)> = db
            .query_with_params("SELECT $1::int AS value", vec![json!(7)])
            .await
            .expect("关闭语句缓存后查询应成功");
        assert_eq!(values[0].0, 7);
    }

    // DISCARD ALL：会话 GUC 不得跨借出泄漏；基线 GUC 由 before_acquire 重新应用。
    db.execute_with_params("SET search_path = 'pg_catalog'", vec![])
        .await
        .expect("设置会话 search_path 应成功");
    let search_path: Vec<(String,)> = db
        .query_with_params("SELECT current_setting('search_path')", vec![])
        .await
        .expect("读取 search_path 应成功");
    assert_ne!(
        search_path[0].0, "pg_catalog",
        "归还连接前 DISCARD ALL 应清理 search_path"
    );
    let timezone: Vec<(String,)> = db
        .query_with_params("SELECT current_setting('TimeZone')", vec![])
        .await
        .expect("读取 TimeZone 应成功");
    assert_eq!(timezone[0].0, "UTC", "before_acquire 应重新应用会话基线");

    // 池被全部连接占满时，等待计数应为 1；归还后恢复 0。
    let (held, _token) = db
        .acquire_with_cancel_token()
        .await
        .expect("借出连接应成功");
    let (_held_second, _second_token) = db
        .acquire_with_cancel_token()
        .await
        .expect("借出第二条连接应成功");
    let db_for_task = Arc::clone(&db);
    let task = tokio::spawn(async move {
        db_for_task
            .query_with_params::<(i32,)>("SELECT 1", vec![])
            .await
    });
    let mut observed = false;
    for _ in 0..100 {
        if db.pool_status().waiting == 1 {
            observed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(
        observed,
        "池被占满时 waiting 应为 1，实得 {}",
        db.pool_status().waiting
    );
    drop(held);
    let rows = task
        .await
        .expect("查询任务不应 panic")
        .expect("等待后查询应成功");
    assert_eq!(rows[0].0, 1);
    assert_eq!(db.pool_status().waiting, 0, "池归还后等待计数应归零");
}

// ---- 真库：TLS 配置失败路径 ----

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave1_tls_failure_paths_fail_closed() {
    let url = test_db_url();

    // verify-full 缺根证书：在配置校验阶段 fail-closed，不落到降级连接。
    let missing_ca = Database::connect_with_config(
        &url,
        DatabaseConfig::default().with_ssl_mode(Some(SslMode::VerifyFull)),
    )
    .await;
    assert!(
        matches!(missing_ca, Err(DbError::InvalidArgument(_))),
        "verify-full 缺 CA 必须返回 InvalidArgument"
    );

    // 非 PEM 根证书：建连必须失败，而不是静默回退明文。
    let certificate =
        std::env::temp_dir().join(format!("yang-db-wave1-bogus-{}.pem", std::process::id()));
    std::fs::write(&certificate, b"not a certificate").expect("写入临时文件应成功");
    let bad_ca = Database::connect_with_config(
        &url,
        DatabaseConfig::default()
            .with_ssl_mode(Some(SslMode::VerifyFull))
            .with_ssl_root_cert(&certificate),
    )
    .await;
    let _ = std::fs::remove_file(&certificate);
    assert!(bad_ca.is_err(), "非 PEM 根证书必须导致建连失败");

    // 本地服务未启用 TLS 时，Require 必须失败（禁止降级）。
    let db = connect().await;
    let ssl: Vec<(String,)> = db
        .query_with_params("SHOW ssl", vec![])
        .await
        .expect("读取 SHOW ssl 应成功");
    if ssl[0].0.eq_ignore_ascii_case("off") {
        let required = Database::connect_with_config(
            &url,
            DatabaseConfig::default().with_ssl_mode(Some(SslMode::Require)),
        )
        .await;
        assert!(required.is_err(), "服务端未启用 TLS 时 Require 必须失败");
    }
}

// ---- 真库：client_encoding 校验 ----
//
// sqlx 在 StartupMessage 中硬编码 `client_encoding=UTF8`（先于连接串的
// `options` 生效），因此无法从外部把新建连接伪装成 LATIN1；「非 UTF8 拒绝」
// 的失败路径由 `database.rs` 的 `client_encoding_must_be_utf8` 单测固定。
// 这里验证真实连接上该校验确实被接线，且服务端编码为 UTF8。

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave1_client_encoding_is_utf8_on_new_connections() {
    assert!(
        DatabaseConfig::default().require_utf8_client_encoding,
        "client_encoding 校验默认必须开启（fail-closed）"
    );
    let db = connect().await;
    let encoding: Vec<(String,)> = db
        .query_with_params("SELECT current_setting('client_encoding')", vec![])
        .await
        .expect("读取 client_encoding 应成功");
    assert_eq!(encoding[0].0, "UTF8");

    // 连接串请求 LATIN1 也会被驱动的 UTF8 启动参数覆盖，非 UTF8 不会悄悄生效。
    let url = with_query_param(&test_db_url(), "options=-c%20client_encoding%3DLATIN1");
    let db = Database::connect_with_config(&url, DatabaseConfig::default())
        .await
        .expect("显式 UTF8 时应能连接");
    let encoding: Vec<(String,)> = db
        .query_with_params("SELECT current_setting('client_encoding')", vec![])
        .await
        .expect("读取 client_encoding 应成功");
    assert_eq!(encoding[0].0, "UTF8");
}

// ---- 真库：dollar-quote 脚本执行 + 整体回滚 ----

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave1_execute_script_handles_dollar_quotes_and_rolls_back() {
    let db = connect().await;
    let function = format!("wave1_dollar_{}", std::process::id());

    // 函数体中的分号不能被脚本切分器拆坏。
    let script = format!(
        "CREATE OR REPLACE FUNCTION {function}(x INT) RETURNS INT AS $body$\n\
         BEGIN\n    IF x > 0 THEN\n        RETURN x + 1;\n    END IF;\n    RETURN 0;\nEND;\n\
         $body$ LANGUAGE plpgsql;"
    );
    assert_eq!(
        db.execute_script(&script)
            .await
            .expect("执行函数脚本应成功"),
        1,
        "dollar-quote 函数体应作为一个语句执行"
    );
    let value: Vec<(i32,)> = db
        .query_with_params(&format!("SELECT {function}(41)"), vec![])
        .await
        .expect("调用函数应成功");
    assert_eq!(value[0].0, 42);
    db.execute_script(&format!("DROP FUNCTION {function}(INT)"))
        .await
        .expect("清理函数应成功");

    // 脚本中任一条失败：整脚本回滚，不残留半成品表。
    db.execute_script("DROP TABLE IF EXISTS wave1_script_rollback")
        .await
        .expect("清理旧表应成功");
    let failing = "CREATE TABLE wave1_script_rollback (id INT); \
                   SELECT * FROM wave1_script_missing;";
    assert!(
        db.execute_script(failing).await.is_err(),
        "脚本包含失败语句时必须返回错误"
    );
    let exists: Vec<(bool,)> = db
        .query_with_params(
            "SELECT to_regclass('wave1_script_rollback') IS NOT NULL",
            vec![],
        )
        .await
        .expect("查询表是否存在应成功");
    assert!(!exists[0].0, "失败脚本必须整体回滚，不能残留建表结果");
}

// ---- 真库：标识符规则（Unicode / $ / 内嵌引号 / 63 字节） ----

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave1_postgres_identifier_rules_round_trip() {
    assert_eq!(
        quote_identifier("用户").expect("中文标识符应合法"),
        "\"用户\""
    );
    assert_eq!(
        quote_identifier("a$b").expect("非首字符 $ 应合法"),
        "\"a$b\""
    );
    assert_eq!(
        quote_identifier("a\"b").expect("内嵌引号应双写"),
        "\"a\"\"b\""
    );
    assert_eq!(
        quote_table("审计.事件").expect("schema 限定中文表名应合法"),
        "\"审计\".\"事件\""
    );
    assert!(quote_identifier("$leading").is_err(), "首字符 $ 必须拒绝");
    assert!(
        quote_identifier(&"a".repeat(64)).is_err(),
        "超过 63 字节必须拒绝而不是截断"
    );

    let db = connect().await;
    db.execute_script("DROP TABLE IF EXISTS \"wave1_中文$表\"")
        .await
        .expect("清理旧表应成功");
    db.execute_script("CREATE TABLE \"wave1_中文$表\" (\"列名\" INT)")
        .await
        .expect("PostgreSQL 应接受 Unicode + $ 的标识符");
    let exists: Vec<(bool,)> = db
        .query_with_params(
            "SELECT to_regclass($1) IS NOT NULL",
            vec![json!("\"wave1_中文$表\"")],
        )
        .await
        .expect("查询表是否存在应成功");
    assert!(exists[0].0);
    db.execute_script("DROP TABLE \"wave1_中文$表\"")
        .await
        .expect("清理表应成功");
}
