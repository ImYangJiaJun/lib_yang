#![allow(clippy::expect_used, clippy::unwrap_used)]
#![cfg(feature = "postgres")]
//! PostgreSQL Phase C 应用层能力集成测试。
//!
//! 覆盖：
//! - `TableQuery::upsert_batch` / `upsert_batch_in_tx`：显式冲突列下的批量插入与
//!   冲突更新；
//! - `upsert_batch_do_nothing` / `upsert_batch_do_nothing_in_tx`：`ON CONFLICT
//!   DO NOTHING` 保持原值；
//! - `ActionContext::set_local`：同一事务内可见、回滚后不泄漏；
//! - `ActionContext::set_tenant_local`：租户 ID 写入 `app.tenant_id`。
//!
//! 所有测试均标记 `#[ignore]`，默认离线套件不触库。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-base --no-default-features --features postgres \
//!   --test integration_table_query_pg_phase_c -- --ignored --test-threads=1
//! ```

use serde_json::json;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::sync::Arc;
use std::time::Duration;
use yang_base::action::{ActionContext, Request, TenantContext, TenantId};
use yang_base::database::{AnyDatabase, AnyPool};
use yang_base::table::{Field, Record, Table, TableDefinition, TableQuery};
use yang_base::tools::ToolsBuilder;
use yang_db::postgres::{Database as PgDatabase, DatabaseConfig as PgDatabaseConfig};

fn test_db_url() -> String {
    std::env::var("PG_TEST_URL")
        .or_else(|_| std::env::var("YANG_POSTGRES_URL"))
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432/test".to_string())
}

/// 单连接池：事务结束后下一次 `begin` 必然复用同一物理连接，用于证明 SET LOCAL
/// 不会随连接归还而泄漏。
async fn connect_pool() -> PgPool {
    PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&test_db_url())
        .await
        .expect("PG_TEST_URL / YANG_POSTGRES_URL 指向的 PostgreSQL 必须可连接")
}

async fn reset_tables(pool: &PgPool) {
    sqlx::query("DROP TABLE IF EXISTS tq_pg_phase_c_rows")
        .execute(pool)
        .await
        .expect("删除旧测试表应成功");
    sqlx::query(
        "CREATE TABLE tq_pg_phase_c_rows (
            id BIGSERIAL PRIMARY KEY,
            tenant_id BIGINT NOT NULL,
            option_key TEXT NOT NULL,
            label TEXT NOT NULL,
            sort_order INTEGER NOT NULL,
            UNIQUE (tenant_id, option_key)
        )",
    )
    .execute(pool)
    .await
    .expect("创建 tq_pg_phase_c_rows 应成功");
}

fn phase_c_definition() -> TableDefinition {
    Table::new("tq_pg_phase_c_rows")
        .fields([
            Field::id("id"),
            Field::bigint("tenant_id").required(),
            Field::string("option_key", 64).required(),
            Field::string("label", 64).required(),
            Field::integer("sort_order").required(),
        ])
        .build()
        .expect("tq_pg_phase_c_rows 表定义应有效")
}

fn bind(definition: &TableDefinition, roles: &[&str], pool: &PgPool) -> TableQuery {
    definition
        .bind_pool(AnyPool::Postgres(Arc::new(pool.clone())))
        .query(roles.iter().copied())
}

fn row(option_key: &str, label: &str, sort_order: i64) -> Record {
    Record::new()
        .set("tenant_id", 1i64)
        .set("option_key", option_key)
        .set("label", label)
        .set("sort_order", sort_order)
}

fn pg_database(pool: &PgPool) -> PgDatabase {
    PgDatabase::from_pool(pool.clone(), PgDatabaseConfig::default())
        .expect("默认 PG DatabaseConfig 应通过校验")
}

/// 冲突列由唯一索引 `(tenant_id, option_key)` 提供；插入与冲突更新都不得改变行数。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn batch_upsert_inserts_updates_and_do_nothing() {
    let pool = connect_pool().await;
    reset_tables(&pool).await;
    let definition = phase_c_definition();
    let conflicts = ["tenant_id", "option_key"];

    // 首轮：两行全新 → 全插入。
    let affected = bind(&definition, &["admin"], &pool)
        .upsert_batch(
            vec![row("opt_a", "甲", 1), row("opt_b", "乙", 2)],
            &conflicts,
        )
        .await
        .expect("PG 批量 upsert 插入应成功");
    assert_eq!(affected, 2, "两行全插入应影响 2 行");

    // 第二轮：冲突键命中 → DO UPDATE，标签与排序更新。
    let affected = bind(&definition, &["admin"], &pool)
        .upsert_batch(
            vec![row("opt_a", "甲改", 10), row("opt_b", "乙改", 20)],
            &conflicts,
        )
        .await
        .expect("PG 批量 upsert 冲突更新应成功");
    assert_eq!(affected, 2, "两行命中冲突应影响 2 行");

    let rows: Vec<(String, String, i32)> = sqlx::query_as(
        "SELECT option_key, label, sort_order FROM tq_pg_phase_c_rows ORDER BY option_key",
    )
    .fetch_all(&pool)
    .await
    .expect("读取 upsert 结果应成功");
    assert_eq!(
        rows,
        vec![
            ("opt_a".to_string(), "甲改".to_string(), 10),
            ("opt_b".to_string(), "乙改".to_string(), 20),
        ],
        "冲突更新应写入新值且不长出新行"
    );

    // 第三轮：DO NOTHING 跳过冲突行，原值保持不变。
    let affected = bind(&definition, &["admin"], &pool)
        .upsert_batch_do_nothing(vec![row("opt_a", "不应写入", 99)], &conflicts)
        .await
        .expect("PG 批量 upsert DO NOTHING 应成功");
    assert_eq!(affected, 0, "DO NOTHING 跳过的行不计数");

    let label: (String,) = sqlx::query_as(
        "SELECT label FROM tq_pg_phase_c_rows WHERE tenant_id = 1 AND option_key = 'opt_a'",
    )
    .fetch_one(&pool)
    .await
    .expect("读取 DO NOTHING 后的值应成功");
    assert_eq!(label.0, "甲改", "DO NOTHING 不得覆盖已存在行");
}

/// 事务内变体：提交后持久化，回滚后整体撤销。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn batch_upsert_in_tx_commits_and_rolls_back() {
    let pool = connect_pool().await;
    reset_tables(&pool).await;
    let definition = phase_c_definition();
    let conflicts = ["tenant_id", "option_key"];
    let database = AnyDatabase::postgres(pg_database(&pool));

    // 提交：事务内插入两行。
    let mut tx = database.begin().await.expect("开启 PG 事务应成功");
    let affected = bind(&definition, &["admin"], &pool)
        .upsert_batch_in_tx(
            &mut tx,
            vec![row("opt_a", "甲", 1), row("opt_b", "乙", 2)],
            &conflicts,
        )
        .await
        .expect("事务内批量 upsert 应成功");
    assert_eq!(affected, 2);
    tx.commit().await.expect("提交应成功");

    // 提交：冲突行改为新标签。
    let mut tx = database.begin().await.expect("开启 PG 事务应成功");
    let affected = bind(&definition, &["admin"], &pool)
        .upsert_batch_in_tx(&mut tx, vec![row("opt_a", "甲改", 10)], &conflicts)
        .await
        .expect("事务内冲突更新应成功");
    assert_eq!(affected, 1);
    tx.commit().await.expect("提交应成功");

    // 事务内 DO NOTHING 不覆盖已提交值。
    let mut tx = database.begin().await.expect("开启 PG 事务应成功");
    let affected = bind(&definition, &["admin"], &pool)
        .upsert_batch_do_nothing_in_tx(&mut tx, vec![row("opt_a", "不应写入", 99)], &conflicts)
        .await
        .expect("事务内 DO NOTHING 应成功");
    assert_eq!(affected, 0);
    tx.commit().await.expect("提交应成功");

    let label: (String,) = sqlx::query_as(
        "SELECT label FROM tq_pg_phase_c_rows WHERE tenant_id = 1 AND option_key = 'opt_a'",
    )
    .fetch_one(&pool)
    .await
    .expect("读取事务结果应成功");
    assert_eq!(label.0, "甲改");

    // 回滚：新键的批量插入整体撤销。
    let mut tx = database.begin().await.expect("开启 PG 事务应成功");
    bind(&definition, &["admin"], &pool)
        .upsert_batch_in_tx(&mut tx, vec![row("opt_c", "丙", 3)], &conflicts)
        .await
        .expect("事务内批量 upsert 应成功");
    tx.rollback().await.expect("回滚应成功");

    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM tq_pg_phase_c_rows WHERE option_key = 'opt_c'")
            .fetch_one(&pool)
            .await
            .expect("统计应成功");
    assert_eq!(count.0, 0, "回滚后不得留下批量 upsert 写入");
}

/// `set_local` 在同一事务内可见；提交/回滚后由 PostgreSQL 自动还原。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn set_local_is_visible_in_tx_and_does_not_leak() {
    let pool = connect_pool().await;
    let tools = Arc::new(
        ToolsBuilder::new()
            .database(AnyDatabase::postgres(pg_database(&pool)))
            .build()
            .expect("Tools 构建应成功"),
    );
    let context = ActionContext::new(Request::new(json!({})), tools);

    let mut tx = context.begin_transaction().await.expect("开启事务应成功");
    context
        .set_local(&mut tx, "app.tenant_id", "42")
        .await
        .expect("set_local 应成功");

    let visible: Vec<(String,)> = tx
        .as_postgres()
        .expect("事务应为 PostgreSQL")
        .query_with_params(
            "SELECT COALESCE(current_setting('app.tenant_id', true), '')",
            vec![],
        )
        .await
        .expect("读取事务局部配置应成功");
    assert_eq!(visible[0].0, "42", "同一事务内应看到 set_local 写入的值");
    tx.rollback().await.expect("回滚应成功");

    // 单连接池保证下一次事务复用同一物理连接：回滚必须把局部配置还原。
    let mut tx = context.begin_transaction().await.expect("开启事务应成功");
    let after: Vec<(String,)> = tx
        .as_postgres()
        .expect("事务应为 PostgreSQL")
        .query_with_params(
            "SELECT COALESCE(current_setting('app.tenant_id', true), '')",
            vec![],
        )
        .await
        .expect("读取连接状态应成功");
    assert_eq!(after[0].0, "", "回滚后 set_local 不得泄漏到后续事务");
    tx.commit().await.expect("提交应成功");
}

/// `set_tenant_local` 把当前 `TenantContext` 写入 `app.tenant_id`。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn set_tenant_local_writes_current_tenant() {
    let pool = connect_pool().await;
    let tools = Arc::new(
        ToolsBuilder::new()
            .database(AnyDatabase::postgres(pg_database(&pool)))
            .build()
            .expect("Tools 构建应成功"),
    );
    let context = ActionContext::new(Request::new(json!({})), tools)
        .with_tenant(TenantContext::new(TenantId::new(7)));

    let mut tx = context.begin_transaction().await.expect("开启事务应成功");
    context
        .set_tenant_local(&mut tx)
        .await
        .expect("set_tenant_local 应成功");

    let tenant: Vec<(String,)> = tx
        .as_postgres()
        .expect("事务应为 PostgreSQL")
        .query_with_params(
            "SELECT COALESCE(current_setting($1, true), '')",
            vec![json!(ActionContext::TENANT_LOCAL_KEY)],
        )
        .await
        .expect("读取租户局部配置应成功");
    assert_eq!(tenant[0].0, "7", "租户 ID 应写入 app.tenant_id");
    tx.rollback().await.expect("回滚应成功");

    // 缺少普通租户 capability 时必须 fail-closed。
    let no_tenant_tools = Arc::new(
        ToolsBuilder::new()
            .database(AnyDatabase::postgres(pg_database(&pool)))
            .build()
            .expect("Tools 构建应成功"),
    );
    let no_tenant = ActionContext::new(Request::new(json!({})), no_tenant_tools);
    let mut tx = no_tenant.begin_transaction().await.expect("开启事务应成功");
    let error = no_tenant
        .set_tenant_local(&mut tx)
        .await
        .expect_err("缺少租户 capability 必须失败");
    assert!(
        matches!(error, yang_base::error::BaseError::Unauthorized(_)),
        "应返回 Unauthorized，实际: {error:?}"
    );
    tx.rollback().await.expect("回滚应成功");
}
