#![cfg(feature = "postgres")]
#![allow(clippy::expect_used)]
//! PostgreSQL 声明式 schema 同步集成测试（真实库）。
//!
//! 所有测试标记 `#[ignore]`，默认离线套件不触库。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-base --no-default-features --features postgres \
//!   --test integration_schema_sync_pg -- --ignored --test-threads=1
//! ```

use yang_base::database::{AnyDatabase, DatabaseInitializer, SchemaSyncChangeKind};
use yang_base::table::{Field, Table, TableDefinition};
use yang_db::postgres::Database;

fn test_db_url() -> String {
    std::env::var("PG_TEST_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432/test".to_string())
}

async fn connect() -> Database {
    Database::connect(&test_db_url())
        .await
        .expect("PG_TEST_URL 指向的 PostgreSQL 必须可连接")
}

fn initializer(database: Database) -> DatabaseInitializer {
    DatabaseInitializer::from_any(AnyDatabase::postgres(database))
}

/// 表名均为测试常量，直接双引号包裹执行清理。
async fn reset_tables(database: &Database, tables: &[&str]) {
    for table in tables {
        database
            .execute_with_params(&format!("DROP TABLE IF EXISTS \"{table}\" CASCADE"), vec![])
            .await
            .expect("清理测试表应成功");
    }
}

async fn table_exists(database: &Database, table: &str) -> bool {
    sqlx::query_scalar::<_, bool>("SELECT to_regclass($1) IS NOT NULL")
        .bind(format!("\"{table}\""))
        .fetch_one(database.pool())
        .await
        .expect("查询表是否存在应成功")
}

fn account_table(username_length: usize, with_display_name: bool) -> TableDefinition {
    let mut fields = vec![
        Field::id("id"),
        Field::string("username", username_length)
            .required()
            .unique(),
    ];
    if with_display_name {
        fields.push(Field::string("display_name", 64));
    }
    Table::new("schema_sync_pg_accounts")
        .fields(fields)
        .build()
        .expect("schema_sync_pg_accounts 表定义应有效")
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn schema_sync_pg_creates_table_and_second_sync_is_noop() {
    let database = connect().await;
    reset_tables(&database, &["schema_sync_pg_accounts"]).await;
    let initializer = initializer(database);
    let definition = account_table(32, false);

    let created = initializer
        .sync_table_definitions(&[&definition])
        .await
        .expect("首次建表应成功");
    assert_eq!(created.changes.len(), 1);
    assert_eq!(created.changes[0].kind, SchemaSyncChangeKind::CreatedTable);
    assert!(initializer
        .validate_table_definition(&definition)
        .await
        .expect("同步后应能读取列")
        .is_compatible());

    let second = initializer
        .sync_table_definitions(&[&definition])
        .await
        .expect("二次同步应成功");
    assert!(second.is_noop(), "二次同步必须幂等: {second:?}");
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn schema_sync_pg_adds_column_index_check_and_foreign_key() {
    let database = connect().await;
    reset_tables(
        &database,
        &["schema_sync_pg_child", "schema_sync_pg_parent"],
    )
    .await;
    let initializer = initializer(database);

    let parent = Table::new("schema_sync_pg_parent")
        .fields([
            Field::id("id"),
            Field::string("code", 32).required().unique(),
        ])
        .build()
        .expect("父表定义应有效");

    let child_initial = Table::new("schema_sync_pg_child")
        .fields([
            Field::id("id"),
            Field::bigint("parent_id").required(),
            // status 先存在：CHECK 预检引用的列必须在 DDL 前可读。
            Field::string("status", 16).required().default("active"),
        ])
        .build()
        .expect("子表初始定义应有效");
    initializer
        .sync_table_definitions(&[&parent, &child_initial])
        .await
        .expect("初始结构应创建成功");

    let child_extended = Table::new("schema_sync_pg_child")
        .fields([
            Field::id("id"),
            Field::bigint("parent_id").required(),
            Field::string("display_name", 64),
            Field::string("status", 16).required().default("active"),
        ])
        .index_named("idx_schema_sync_pg_child_display", ["display_name"])
        .check_named(
            "chk_schema_sync_pg_child_status",
            "\"status\" IN ('active', 'disabled')",
        )
        .foreign_key_named(
            "fk_schema_sync_pg_child_parent",
            ["parent_id"],
            "schema_sync_pg_parent",
            ["id"],
        )
        .build()
        .expect("子表扩展定义应有效");

    let report = initializer
        .sync_table_definitions(&[&parent, &child_extended])
        .await
        .expect("加列/索引/CHECK/外键应成功");
    let kinds = report
        .changes
        .iter()
        .map(|change| change.kind)
        .collect::<Vec<_>>();
    assert!(kinds.contains(&SchemaSyncChangeKind::AddedColumn));
    assert!(kinds.contains(&SchemaSyncChangeKind::AddedIndex));
    assert!(kinds.contains(&SchemaSyncChangeKind::AddedCheck));
    assert!(kinds.contains(&SchemaSyncChangeKind::AddedForeignKey));

    let second = initializer
        .sync_table_definitions(&[&parent, &child_extended])
        .await
        .expect("扩展后二次同步应成功");
    assert!(second.is_noop(), "扩展结构必须幂等: {second:?}");
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn schema_sync_pg_rejects_dangerous_type_drift() {
    let database = connect().await;
    reset_tables(&database, &["schema_sync_pg_drift"]).await;
    let initializer = initializer(connect().await);

    let definition = Table::new("schema_sync_pg_drift")
        .fields([Field::id("id"), Field::bigint("value").required()])
        .build()
        .expect("漂移测试表定义应有效");
    initializer
        .sync_table_definitions(&[&definition])
        .await
        .expect("初始建表应成功");

    // 人为制造不可自动转换的类型漂移：BIGINT -> TEXT。
    database
        .execute_with_params(
            "ALTER TABLE \"schema_sync_pg_drift\" \
             ALTER COLUMN \"value\" TYPE TEXT USING \"value\"::text",
            vec![],
        )
        .await
        .expect("制造类型漂移应成功");

    let error = initializer
        .sync_table_definitions(&[&definition])
        .await
        .expect_err("危险漂移必须拒绝自动修改");
    assert!(
        error.to_string().contains("不可自动修改"),
        "应给出拒绝原因: {error}"
    );
    assert!(table_exists(&database, "schema_sync_pg_drift").await);
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn schema_sync_pg_dry_run_has_no_side_effects() {
    let database = connect().await;
    reset_tables(&database, &["schema_sync_pg_dry_run"]).await;
    let initializer = initializer(connect().await);
    let definition = Table::new("schema_sync_pg_dry_run")
        .fields([Field::id("id"), Field::string("username", 32).required()])
        .build()
        .expect("dry-run 测试表定义应有效");

    let plan = initializer
        .plan_table_definitions(&[&definition])
        .await
        .expect("只读计划应成功");
    assert!(plan
        .changes
        .iter()
        .any(|change| change.kind == SchemaSyncChangeKind::CreatedTable));
    assert!(
        !table_exists(&database, "schema_sync_pg_dry_run").await,
        "dry-run 不得产生任何 DDL 副作用"
    );
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn schema_sync_pg_concurrent_startups_are_serialized() {
    let database = connect().await;
    reset_tables(&database, &["schema_sync_pg_concurrent"]).await;
    let definition = Table::new("schema_sync_pg_concurrent")
        .fields([
            Field::id("id"),
            Field::string("username", 32).required().unique(),
        ])
        .build()
        .expect("并发测试表定义应有效");

    let first = initializer(connect().await);
    let second = initializer(connect().await);
    let definitions = [&definition];
    let (left, right) = tokio::join!(
        first.sync_table_definitions(&definitions),
        second.sync_table_definitions(&definitions)
    );
    let left = left.expect("第一个并发初始化应成功");
    let right = right.expect("第二个并发初始化应成功");
    let created = left
        .changes
        .iter()
        .chain(&right.changes)
        .filter(|change| change.kind == SchemaSyncChangeKind::CreatedTable)
        .count();
    assert_eq!(created, 1, "advisory lock 应保证只创建一次表");
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn schema_sync_pg_failed_ddl_rolls_back_whole_transaction() {
    let database = connect().await;
    reset_tables(
        &database,
        &["schema_sync_pg_rollback_a", "schema_sync_pg_rollback_b"],
    )
    .await;
    let initializer = initializer(connect().await);

    let first = Table::new("schema_sync_pg_rollback_a")
        .fields([Field::id("id"), Field::string("name", 32)])
        .index_named("idx_schema_sync_pg_shared", ["name"])
        .build()
        .expect("第一张表定义应有效");
    initializer
        .sync_table_definitions(&[&first])
        .await
        .expect("第一张表应创建成功");

    // PostgreSQL 索引名在 schema 内全局唯一：第二张表的同名索引会让
    // “CREATE TABLE 已成功、CREATE INDEX 失败”触发整体回滚。
    let second = Table::new("schema_sync_pg_rollback_b")
        .fields([Field::id("id"), Field::string("name", 32)])
        .index_named("idx_schema_sync_pg_shared", ["name"])
        .build()
        .expect("第二张表定义应有效");
    initializer
        .sync_table_definitions(&[&second])
        .await
        .expect_err("同名索引应导致 DDL 失败");

    assert!(
        !table_exists(&database, "schema_sync_pg_rollback_b").await,
        "DDL 失败必须整体回滚，不能残留半成品表"
    );
}
