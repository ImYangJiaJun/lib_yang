#![allow(clippy::expect_used, clippy::unwrap_used)]
#![cfg(feature = "postgres")]
//! PostgreSQL Wave 2 写入契约集成测试。
//!
//! 覆盖：
//! - `TableQuery::insert_returning`：`Pk` / `Columns` / `All` 三种目标；
//! - UUID 主键（数据库生成）经 `RETURNING` 原样回填；
//! - 批量 UPSERT 的显式更新列白名单只更新点名列；
//! - 默认白名单不覆盖主键 / 租户 / `created_at` / `source_file` / `company`。
//!
//! 所有测试均标记 `#[ignore]`，默认离线套件不触库。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-base --no-default-features --features postgres \
//!   --test integration_pg_wave2_write -- --ignored --test-threads=1
//! ```

use sqlx::postgres::{PgPool, PgPoolOptions};
use std::sync::Arc;
use std::time::Duration;
use yang_base::action::builtin::add::ReturningTarget;
use yang_base::database::{AnyDatabase, AnyPool};
use yang_base::table::{Field, Record, Table, TableDefinition, TableQuery};
use yang_db::postgres::{Database as PgDatabase, DatabaseConfig as PgDatabaseConfig};

fn test_db_url() -> String {
    std::env::var("PG_TEST_URL")
        .or_else(|_| std::env::var("YANG_POSTGRES_URL"))
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432/test".to_string())
}

async fn connect_pool() -> PgPool {
    PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&test_db_url())
        .await
        .expect("PG_TEST_URL / YANG_POSTGRES_URL 指向的 PostgreSQL 必须可连接")
}

async fn execute(pool: &PgPool, sql: &str) {
    sqlx::query(sql)
        .execute(pool)
        .await
        .unwrap_or_else(|error| panic!("执行 SQL 失败: {error}\n{sql}"));
}

fn bind(definition: &TableDefinition, roles: &[&str], pool: &PgPool) -> TableQuery {
    definition
        .bind_pool(AnyPool::Postgres(Arc::new(pool.clone())))
        .query(roles.iter().copied())
}

fn pg_database(pool: &PgPool) -> PgDatabase {
    PgDatabase::from_pool(pool.clone(), PgDatabaseConfig::default())
        .expect("默认 PG DatabaseConfig 应通过校验")
}

async fn reset_insert_table(pool: &PgPool) {
    execute(pool, "DROP TABLE IF EXISTS tq_pg_wave2_insert").await;
    execute(
        pool,
        "CREATE TABLE tq_pg_wave2_insert (
            id BIGSERIAL PRIMARY KEY,
            label TEXT NOT NULL,
            sort_order INTEGER NOT NULL,
            created_at BIGINT NOT NULL
        )",
    )
    .await;
}

fn insert_definition() -> TableDefinition {
    Table::new("tq_pg_wave2_insert")
        .fields([
            Field::id("id"),
            Field::string("label", 64).required(),
            Field::integer("sort_order").required(),
            Field::created_at("created_at"),
        ])
        .build()
        .expect("tq_pg_wave2_insert 表定义应有效")
}

fn insert_row(label: &str, sort_order: i32) -> Record {
    Record::new()
        .set("label", label)
        .set("sort_order", sort_order)
}

async fn reset_returning_json_table(pool: &PgPool) {
    execute(pool, "DROP TABLE IF EXISTS tq_pg_wave2_returning_json").await;
    execute(
        pool,
        "CREATE TABLE tq_pg_wave2_returning_json (
            id BIGSERIAL PRIMARY KEY,
            label TEXT NOT NULL,
            payload JSONB NULL
        )",
    )
    .await;
}

fn returning_json_definition() -> TableDefinition {
    Table::new("tq_pg_wave2_returning_json")
        .fields([
            Field::id("id"),
            Field::string("label", 64).required(),
            Field::json("payload"),
        ])
        .build()
        .expect("tq_pg_wave2_returning_json 表定义应有效")
}

async fn reset_uuid_table(pool: &PgPool) {
    execute(pool, "DROP TABLE IF EXISTS tq_pg_wave2_uuid").await;
    execute(
        pool,
        "CREATE TABLE tq_pg_wave2_uuid (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            label TEXT NOT NULL
        )",
    )
    .await;
}

fn uuid_definition() -> TableDefinition {
    Table::new("tq_pg_wave2_uuid")
        .fields([
            Field::string("id", 36)
                .primary_key()
                .required()
                .not_writable(),
            Field::string("label", 64).required(),
        ])
        .build()
        .expect("tq_pg_wave2_uuid 表定义应有效")
}

async fn reset_upsert_table(pool: &PgPool) {
    execute(pool, "DROP TABLE IF EXISTS tq_pg_wave2_upsert").await;
    execute(
        pool,
        "CREATE TABLE tq_pg_wave2_upsert (
            id BIGSERIAL PRIMARY KEY,
            tenant_id BIGINT NOT NULL,
            option_key TEXT NOT NULL,
            label TEXT NOT NULL,
            sort_order INTEGER NOT NULL,
            created_at BIGINT NOT NULL,
            source_file TEXT NOT NULL,
            company TEXT NOT NULL,
            UNIQUE (tenant_id, option_key)
        )",
    )
    .await;
}

fn upsert_definition() -> TableDefinition {
    Table::new("tq_pg_wave2_upsert")
        .fields([
            Field::id("id"),
            Field::bigint("tenant_id").required(),
            Field::string("option_key", 64).required(),
            Field::string("label", 64).required(),
            Field::integer("sort_order").required(),
            Field::created_at("created_at"),
            Field::string("source_file", 128).required(),
            Field::string("company", 64).required(),
        ])
        .build()
        .expect("tq_pg_wave2_upsert 表定义应有效")
}

fn upsert_row(label: &str, sort_order: i32, source_file: &str, company: &str) -> Record {
    Record::new()
        .set("tenant_id", 1i64)
        .set("option_key", "opt_a")
        .set("label", label)
        .set("sort_order", sort_order)
        .set("source_file", source_file)
        .set("company", company)
}

/// `insert_returning` 的三种目标都返回 `Record`；`Pk` 只含主键，`Columns` 只含点名
/// 列，`All` 返回整行。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn insert_returning_supports_pk_columns_and_all() {
    let pool = connect_pool().await;
    reset_insert_table(&pool).await;
    let definition = insert_definition();

    let pk_only = bind(&definition, &["admin"], &pool)
        .insert_returning(insert_row("pk", 1), ReturningTarget::Pk)
        .await
        .expect("insert_returning(Pk) 应成功");
    assert_eq!(
        pk_only.as_map().keys().cloned().collect::<Vec<_>>(),
        vec!["id".to_string()],
        "Pk 目标只能返回主键列: {pk_only:?}"
    );
    assert!(
        pk_only
            .get("id")
            .and_then(|value| value.as_u64())
            .unwrap_or(0)
            > 0,
        "整型主键应正确回填: {pk_only:?}"
    );

    let columns = bind(&definition, &["admin"], &pool)
        .insert_returning(
            insert_row("columns", 2),
            ReturningTarget::Columns(&["id", "label"]),
        )
        .await
        .expect("insert_returning(Columns) 应成功");
    let mut keys: Vec<&str> = columns.as_map().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["id", "label"], "Columns 目标应只返回点名列");
    assert_eq!(
        columns.get("label").and_then(|value| value.as_str()),
        Some("columns")
    );

    let all = bind(&definition, &["admin"], &pool)
        .insert_returning(insert_row("all", 3), ReturningTarget::All)
        .await
        .expect("insert_returning(All) 应成功");
    assert_eq!(
        all.get("label").and_then(|value| value.as_str()),
        Some("all")
    );
    assert_eq!(
        all.get("sort_order").and_then(|value| value.as_i64()),
        Some(3)
    );
    assert!(
        all.get("created_at")
            .and_then(|value| value.as_i64())
            .unwrap_or(0)
            > 0,
        "框架应补齐 created_at 并由 RETURNING * 返回: {all:?}"
    );
}

/// P0-05 + RETURNING：多列 `Columns` 投影必须保留 JSON null sidecar。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn insert_returning_columns_preserves_json_null_sidecar() {
    let pool = connect_pool().await;
    reset_returning_json_table(&pool).await;
    let definition = returning_json_definition();

    let returned = bind(&definition, &["admin"], &pool)
        .insert_returning(
            Record::new()
                .set("label", "json-null")
                .set("payload", serde_json::Value::Null),
            ReturningTarget::Columns(&["id", "label", "payload"]),
        )
        .await
        .expect("多列 RETURNING 应成功");

    let mut keys: Vec<&str> = returned.as_map().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["id", "label", "payload"], "只应返回点名列");
    assert!(
        returned.is_json_null("payload"),
        "RETURNING 的 jsonb 'null' 列必须仍被识别为 JSON null（sidecar 不得丢失）: {returned:?}"
    );
    assert!(!returned.is_sql_null("payload"));
    assert_eq!(returned.get("payload"), Some(&serde_json::Value::Null));
}

/// UUID 主键由数据库默认值生成，`insert_returning(Pk)` / `insert_returning_pk_value`
/// 都必须原样回填字符串主键。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn insert_returning_returns_uuid_primary_key() {
    let pool = connect_pool().await;
    reset_uuid_table(&pool).await;
    let definition = uuid_definition();

    let returned = bind(&definition, &["admin"], &pool)
        .insert_returning(Record::new().set("label", "uuid-row"), ReturningTarget::Pk)
        .await
        .expect("UUID 主键 INSERT ... RETURNING 应成功");
    let uuid = returned
        .get("id")
        .and_then(|value| value.as_str())
        .expect("RETURNING 应返回 UUID 字符串");
    assert_eq!(uuid.len(), 36, "UUID 应为标准 36 字符形式: {uuid}");

    let (affected, value) = bind(&definition, &["admin"], &pool)
        .insert_returning_pk_value(Record::new().set("label", "uuid-row-2"))
        .await
        .expect("insert_returning_pk_value 对 UUID 主键应成功");
    assert_eq!(affected, 1);
    assert!(
        value.as_str().is_some_and(|text| text.len() == 36),
        "主键值应按真实 UUID 类型回填: {value:?}"
    );

    let missing = bind(&definition, &["admin"], &pool)
        .insert_returning_id(Record::new().set("label", "uuid-row-3"))
        .await
        .expect_err("insert_returning_id 对非整型主键必须明确报错");
    assert!(
        matches!(missing, yang_base::error::BaseError::ParamInvalid(_, _)),
        "应返回 ParamInvalid 而非伪造 0: {missing:?}"
    );
}

/// 默认白名单：冲突时只更新非保护列，`created_at` / `source_file` / `company` /
/// 主键 / 租户保持原值。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn batch_upsert_default_whitelist_keeps_audit_columns() {
    let pool = connect_pool().await;
    reset_upsert_table(&pool).await;
    let definition = upsert_definition();
    let conflicts = ["tenant_id", "option_key"];

    bind(&definition, &["admin"], &pool)
        .upsert_batch(vec![upsert_row("甲", 1, "s1.csv", "c1")], &conflicts)
        .await
        .expect("首轮 upsert 插入应成功");

    let (id_before, created_before): (i64, i64) = sqlx::query_as(
        "SELECT id, created_at FROM tq_pg_wave2_upsert WHERE tenant_id = 1 AND option_key = 'opt_a'",
    )
    .fetch_one(&pool)
    .await
    .expect("读取首轮数据应成功");

    bind(&definition, &["admin"], &pool)
        .upsert_batch(vec![upsert_row("甲改", 9, "s2.csv", "c2")], &conflicts)
        .await
        .expect("冲突轮 upsert 应成功");

    let (id_after, created_after, tenant_after, label, sort_order, source_file, company): (
        i64,
        i64,
        i64,
        String,
        i32,
        String,
        String,
    ) = sqlx::query_as(
        "SELECT id, created_at, tenant_id, label, sort_order, source_file, company \
         FROM tq_pg_wave2_upsert WHERE tenant_id = 1 AND option_key = 'opt_a'",
    )
    .fetch_one(&pool)
    .await
    .expect("读取冲突轮数据应成功");

    assert_eq!(id_after, id_before, "主键不得被 upsert 覆盖");
    assert_eq!(
        created_after, created_before,
        "created_at 不得被 upsert 刷新"
    );
    assert_eq!(tenant_after, 1, "租户列不得被 upsert 覆盖");
    assert_eq!(label, "甲改", "普通业务列应走 DO UPDATE");
    assert_eq!(sort_order, 9, "普通业务列应走 DO UPDATE");
    assert_eq!(source_file, "s1.csv", "source_file 是审计列，默认不覆盖");
    assert_eq!(company, "c1", "company 是归属列，默认不覆盖");
}

/// 显式白名单：只更新点名的列，其余列（即使行里带了新值）保持原值；冲突列与
/// 框架保护列在触库前被拒绝。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn batch_upsert_explicit_whitelist_updates_only_named_columns() {
    let pool = connect_pool().await;
    reset_upsert_table(&pool).await;
    let definition = upsert_definition();
    let conflicts = ["tenant_id", "option_key"];

    bind(&definition, &["admin"], &pool)
        .upsert_batch(vec![upsert_row("甲", 1, "s1.csv", "c1")], &conflicts)
        .await
        .expect("首轮 upsert 插入应成功");

    let (id_before, created_before): (i64, i64) = sqlx::query_as(
        "SELECT id, created_at FROM tq_pg_wave2_upsert WHERE tenant_id = 1 AND option_key = 'opt_a'",
    )
    .fetch_one(&pool)
    .await
    .expect("读取首轮数据应成功");

    bind(&definition, &["admin"], &pool)
        .upsert_batch_with_update_columns(
            vec![upsert_row("甲改", 99, "s2.csv", "c2")],
            &conflicts,
            &["label"],
        )
        .await
        .expect("显式白名单 upsert 应成功");

    let (id_after, created_after, tenant_id, label, sort_order, source_file): (
        i64,
        i64,
        i64,
        String,
        i32,
        String,
    ) = sqlx::query_as(
        "SELECT id, created_at, tenant_id, label, sort_order, source_file \
         FROM tq_pg_wave2_upsert WHERE tenant_id = 1 AND option_key = 'opt_a'",
    )
    .fetch_one(&pool)
    .await
    .expect("读取显式白名单结果应成功");
    assert_eq!(id_after, id_before, "主键不得被 upsert 覆盖");
    assert_eq!(
        created_after, created_before,
        "created_at 不得被 upsert 刷新"
    );
    assert_eq!(tenant_id, 1, "租户列不得被 upsert 覆盖");
    assert_eq!(label, "甲改", "白名单列照常更新");
    assert_eq!(sort_order, 1, "未点名的列即使带了新值也必须保持原值");
    assert_eq!(source_file, "s1.csv", "未点名的审计列必须保持原值");

    let conflict_error = bind(&definition, &["admin"], &pool)
        .upsert_batch_with_update_columns(
            vec![upsert_row("甲2", 2, "s1.csv", "c1")],
            &conflicts,
            &["tenant_id"],
        )
        .await
        .expect_err("冲突列不能出现在更新列白名单里");
    assert!(
        matches!(
            conflict_error,
            yang_base::error::BaseError::ParamInvalid(_, _)
        ),
        "应返回 ParamInvalid: {conflict_error:?}"
    );

    let protected_error = bind(&definition, &["admin"], &pool)
        .upsert_batch_with_update_columns(
            vec![upsert_row("甲2", 2, "s1.csv", "c1")],
            &conflicts,
            &["created_at"],
        )
        .await
        .expect_err("created_at 是框架保护列");
    assert!(
        matches!(
            protected_error,
            yang_base::error::BaseError::ParamInvalid(_, _)
        ),
        "应返回 ParamInvalid: {protected_error:?}"
    );
}

/// 事务内显式白名单入口：提交后只写入点名列。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn batch_upsert_in_tx_with_update_columns_commits() {
    let pool = connect_pool().await;
    reset_upsert_table(&pool).await;
    let definition = upsert_definition();
    let conflicts = ["tenant_id", "option_key"];
    let database = AnyDatabase::postgres(pg_database(&pool));

    bind(&definition, &["admin"], &pool)
        .upsert_batch(vec![upsert_row("甲", 1, "s1.csv", "c1")], &conflicts)
        .await
        .expect("首轮 upsert 插入应成功");

    let mut tx = database.begin().await.expect("开启 PG 事务应成功");
    let affected = bind(&definition, &["admin"], &pool)
        .upsert_batch_in_tx_with_update_columns(
            &mut tx,
            vec![upsert_row("甲改", 77, "s2.csv", "c2")],
            &conflicts,
            &["label"],
        )
        .await
        .expect("事务内显式白名单 upsert 应成功");
    assert_eq!(affected, 1);
    tx.commit().await.expect("提交应成功");

    let (label, sort_order): (String, i32) = sqlx::query_as(
        "SELECT label, sort_order FROM tq_pg_wave2_upsert \
         WHERE tenant_id = 1 AND option_key = 'opt_a'",
    )
    .fetch_one(&pool)
    .await
    .expect("读取事务结果应成功");
    assert_eq!(label, "甲改");
    assert_eq!(sort_order, 1, "事务内白名单同样只写点名列");
}
