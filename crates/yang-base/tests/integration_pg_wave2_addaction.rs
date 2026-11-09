#![cfg(feature = "postgres")]
#![allow(clippy::expect_used, clippy::unwrap_used)]
//! PostgreSQL Wave 2 `AddAction` 端到端插入契约集成测试。
//!
//! 覆盖 `AddAction -> TableQuery::insert_returning_pk_value` 在数据库生成 UUID
//! 主键（`gen_random_uuid()` 默认值）的表上的完整链路：返回值中的主键必须是
//! 36 字符 UUID，且该行真实落库。
//!
//! 所有用例标记 `#[ignore]`。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-base --no-default-features --features postgres \
//!   --test integration_pg_wave2_addaction -- --ignored --test-threads=1
//! ```

use serde_json::json;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::sync::Arc;
use std::time::Duration;
use yang_base::action::builtin::AddAction;
use yang_base::action::{ActionContext, Request, TypedHandler};
use yang_base::database::AnyDatabase;
use yang_base::table::{Field, Record, Table, TableDefinition};
use yang_base::tools::ToolsBuilder;
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

async fn reset_table(pool: &PgPool) {
    sqlx::query("DROP TABLE IF EXISTS tq_pg_wave2_addaction")
        .execute(pool)
        .await
        .expect("删除旧测试表应成功");
    sqlx::query(
        "CREATE TABLE tq_pg_wave2_addaction (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            label TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await
    .expect("创建 tq_pg_wave2_addaction 应成功");
}

fn add_definition() -> TableDefinition {
    Table::new("tq_pg_wave2_addaction")
        .fields([
            Field::string("id", 36)
                .primary_key()
                .required()
                .not_writable(),
            Field::string("label", 64).required(),
        ])
        .build()
        .expect("tq_pg_wave2_addaction 表定义应有效")
}

fn pg_database(pool: &PgPool) -> PgDatabase {
    PgDatabase::from_pool(pool.clone(), PgDatabaseConfig::default())
        .expect("默认 PG DatabaseConfig 应通过校验")
}

async fn cleanup(pool: &PgPool) {
    sqlx::query("DROP TABLE IF EXISTS tq_pg_wave2_addaction")
        .execute(pool)
        .await
        .expect("清理 tq_pg_wave2_addaction 应成功");
}

/// `AddAction` 对 UUID 默认主键表的端到端插入：返回 36 字符 UUID，且行真实落库。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn add_action_returns_generated_uuid_and_persists_row() {
    let pool = connect_pool().await;
    reset_table(&pool).await;

    let tools = Arc::new(
        ToolsBuilder::new()
            .database(AnyDatabase::postgres(pg_database(&pool)))
            .build()
            .expect("Tools 构建应成功"),
    );
    let context =
        ActionContext::new(Request::new(json!({})), tools).with_table_definition(add_definition());

    let result = AddAction::new()
        .handle(context, Record::new().set("label", "addaction-row"))
        .await
        .expect("AddAction 插入应成功");

    assert_eq!(result.affected, 1, "应插入 1 行");
    let uuid = result
        .as_str()
        .expect("UUID 主键必须按字符串回填到 InsertResult.id");
    assert_eq!(
        uuid.len(),
        36,
        "InsertResult.as_str() 应是 36 字符 UUID: {uuid}"
    );

    let (label,): (String,) =
        sqlx::query_as("SELECT label FROM tq_pg_wave2_addaction WHERE id::text = $1")
            .bind(uuid)
            .fetch_one(&pool)
            .await
            .expect("按返回的 UUID 应能查回插入行");
    assert_eq!(label, "addaction-row", "落库内容应与输入一致");

    cleanup(&pool).await;
}
