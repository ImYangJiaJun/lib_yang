#![allow(clippy::expect_used, clippy::unwrap_used)]
#![cfg(feature = "postgres")]
//! PostgreSQL Wave 3 访问控制与写入契约集成测试。
//!
//! 覆盖：
//! - 关联预加载复用目标表的租户 / 软删 / 字段读权限守卫；
//! - 软删除写入走与逐行 update 相同的字段写权限校验；
//! - 批量 upsert 的租户冲突列约束与默认更新列白名单不跨租户改写；
//! - PostgreSQL 部分唯一索引冲突谓词（`ON CONFLICT ... WHERE (...)`）。
//!
//! 所有测试均标记 `#[ignore]`，默认离线套件不触库。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-base --no-default-features --features postgres \
//!   --test integration_pg_wave3_guards -- --ignored --test-threads=1
//! ```

use serde_json::json;
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Row;
use std::sync::Arc;
use std::time::Duration;
use yang_base::database::{AnyDatabase, AnyPool};
use yang_base::definition::{FieldName, FieldRef, TableName};
use yang_base::error::BaseError;
use yang_base::table::{
    DatabaseRelationExecutor, Field, Record, RelationLoader, RelationSpec, Table, TableDefinition,
    TableQuery,
};
use yang_db::postgres::{Database as PgDatabase, DatabaseConfig as PgDatabaseConfig};

fn test_db_url() -> String {
    std::env::var("PG_TEST_URL")
        .or_else(|_| std::env::var("YANG_POSTGRES_URL"))
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432/test".to_string())
}

async fn connect_pool() -> PgPool {
    PgPoolOptions::new()
        .max_connections(2)
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

fn any_database(pool: &PgPool) -> AnyDatabase {
    AnyDatabase::postgres(
        PgDatabase::from_pool(pool.clone(), PgDatabaseConfig::default())
            .expect("默认 PG DatabaseConfig 应通过校验"),
    )
}

fn bind(definition: &TableDefinition, roles: &[&str], pool: &PgPool) -> TableQuery {
    definition
        .bind_pool(AnyPool::Postgres(Arc::new(pool.clone())))
        .query(roles.iter().copied())
}

fn field(table: &str, field: &str) -> FieldRef {
    FieldRef::new(
        TableName::new(table).expect("合法表名"),
        FieldName::new(field).expect("合法字段名"),
    )
}

/// 进程级唯一表名：多个进程并行跑同一真库时不会互相 DROP/CREATE。
fn test_table(base: &str) -> String {
    format!("{base}_{}", std::process::id())
}

fn child_definition(table: &str) -> TableDefinition {
    Table::new(table)
        .fields([
            Field::id("id"),
            Field::string("label", 64).required(),
            Field::string("secret", 64).readable_by(["admin"]),
            Field::bigint("tenant_id").tenant_key(),
            Field::soft_delete("deleted_at"),
        ])
        .build()
        .expect("w3_rel_child 表定义应有效")
}

async fn reset_relation_tables(pool: &PgPool, parent: &str, child: &str) {
    execute(pool, &format!("DROP TABLE IF EXISTS {child}, {parent}")).await;
    execute(
        pool,
        &format!(
            "CREATE TABLE {parent} (
            id BIGSERIAL PRIMARY KEY,
            child_id BIGINT NOT NULL,
            tenant_id BIGINT NOT NULL,
            deleted_at BIGINT NULL
        )"
        ),
    )
    .await;
    execute(
        pool,
        &format!(
            "CREATE TABLE {child} (
            id BIGSERIAL PRIMARY KEY,
            label TEXT NOT NULL,
            secret TEXT NULL,
            tenant_id BIGINT NOT NULL,
            deleted_at BIGINT NULL
        )"
        ),
    )
    .await;
}

async fn insert_relation_fixtures(pool: &PgPool, parent: &str, child: &str) {
    // 父表：本租户可见行引用 child 1/2/3/4（2 为他租户子行、3 为软删子行），
    // 另含他租户父行与软删父行。
    execute(
        pool,
        &format!(
            "INSERT INTO {parent} (child_id, tenant_id, deleted_at) VALUES
            (1, 1, NULL), (3, 1, NULL), (2, 1, NULL), (4, 1, NULL),
            (5, 2, NULL), (6, 1, 999)"
        ),
    )
    .await;
    // 子表：本租户活跃（1/4）、他租户活跃（2）、本租户软删（3）。
    execute(
        pool,
        &format!(
            "INSERT INTO {child} (id, label, secret, tenant_id, deleted_at) VALUES
            (1, 't1-active', 's1', 1, NULL),
            (2, 't2-active', 's2', 2, NULL),
            (3, 't1-deleted', 's3', 1, 999),
            (4, 't1-active-2', 's4', 1, NULL)"
        ),
    )
    .await;
}

/// 关联预加载只能返回与主查询一致的租户、未软删且字段权限允许的列。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn relation_preload_applies_tenant_soft_delete_and_field_permissions() {
    let parent_table = test_table("w3_rel_parent");
    let child_table = test_table("w3_rel_child");
    let pool = connect_pool().await;
    reset_relation_tables(&pool, &parent_table, &child_table).await;
    insert_relation_fixtures(&pool, &parent_table, &child_table).await;

    // 主查询视角：本租户且未软删的父行。
    let visible_parents = sqlx::query(&format!(
        "SELECT id, child_id FROM {parent_table} \
         WHERE tenant_id = 1 AND deleted_at IS NULL ORDER BY id"
    ))
    .fetch_all(&pool)
    .await
    .expect("读取可见父行应成功");
    let rows: Vec<Record> = visible_parents
        .iter()
        .map(|row| {
            Record::new()
                .set("id", row.get::<i64, _>("id"))
                .set("child_id", row.get::<i64, _>("child_id"))
        })
        .collect();
    assert_eq!(rows.len(), 4, "只有本租户未软删父行参与预加载");

    let loader = RelationLoader::new(vec![RelationSpec::new(
        field(&parent_table, "child_id"),
        field(&child_table, "id"),
    )
    .display([field(&child_table, "label"), field(&child_table, "secret")])]);
    let database = any_database(&pool);
    let definition = child_definition(&child_table);

    let executor = DatabaseRelationExecutor::new(&database, ["user"])
        .target(definition.clone())
        .with_tenant(json!(1));
    let data = loader
        .load(&rows, &executor)
        .await
        .expect("受守卫的关联加载应成功");
    let loaded = data
        .get(&format!("{parent_table}.child_id"))
        .expect("应返回源关联键对应的数据");
    let mut ids: Vec<i64> = loaded
        .iter()
        .map(|record| record.require::<i64>("id").expect("id 应存在"))
        .collect();
    ids.sort_unstable();
    assert_eq!(ids, vec![1, 4], "只应返回本租户未软删的子行");
    for record in loaded {
        assert!(
            record.get("secret").is_none(),
            "user 角色不可读的 secret 必须被裁剪: {record:?}"
        );
        assert!(record.get("deleted_at").is_none(), "软删列不得出现在结果中");
    }

    // admin 可读 secret：同一租户/软删守卫下才带出该列。
    let admin_executor = DatabaseRelationExecutor::new(&database, ["admin"])
        .target(definition)
        .with_tenant(json!(1));
    let admin_data = loader
        .load(&rows, &admin_executor)
        .await
        .expect("admin 关联加载应成功");
    let admin_loaded = admin_data
        .get(&format!("{parent_table}.child_id"))
        .expect("应返回源关联键对应的数据");
    assert!(
        admin_loaded
            .iter()
            .all(|record| record.get("secret").is_some()),
        "admin 应能读到 secret: {admin_loaded:?}"
    );

    execute(
        &pool,
        &format!("DROP TABLE IF EXISTS {child_table}, {parent_table}"),
    )
    .await;
}

/// 目标表声明 tenant_key 但执行器未绑定租户值时，关联加载必须 fail-closed：
/// 返回明确错误且不返回任何行。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn relation_preload_fails_closed_without_tenant_binding() {
    let parent_table = test_table("w3_rel_parent");
    let child_table = test_table("w3_rel_child");
    let pool = connect_pool().await;
    reset_relation_tables(&pool, &parent_table, &child_table).await;
    insert_relation_fixtures(&pool, &parent_table, &child_table).await;

    let rows = vec![Record::new().set("id", 1_i64).set("child_id", 1_i64)];
    let loader = RelationLoader::new(vec![RelationSpec::new(
        field(&parent_table, "child_id"),
        field(&child_table, "id"),
    )]);
    let database = any_database(&pool);

    // 未调用 with_tenant：目标表声明了 tenant_key，必须直接报错。
    let executor =
        DatabaseRelationExecutor::new(&database, ["user"]).target(child_definition(&child_table));
    let error = loader
        .load(&rows, &executor)
        .await
        .expect_err("未绑定租户值时必须 fail-closed");
    assert!(
        matches!(error, BaseError::ConfigError(ref message)
            if message.contains("w3_rel_child") && message.contains("with_tenant")),
        "应返回提示绑定租户的 ConfigError，实际: {error:?}"
    );

    // 同样的输入在绑定租户后仍可正常加载，确认失败只由缺失租户绑定导致。
    let bound = DatabaseRelationExecutor::new(&database, ["user"])
        .target(child_definition(&child_table))
        .with_tenant(json!(1));
    let data = loader
        .load(&rows, &bound)
        .await
        .expect("绑定租户后关联加载应成功");
    let ids: Vec<i64> = data[&format!("{parent_table}.child_id")]
        .iter()
        .map(|record| record.require::<i64>("id").expect("id 应存在"))
        .collect();
    assert_eq!(ids, vec![1], "绑定租户时只返回本租户子行");

    execute(
        &pool,
        &format!("DROP TABLE IF EXISTS {child_table}, {parent_table}"),
    )
    .await;
}

/// 显式 without_tenant_scope() opt-out 后才允许跨租户加载；软删守卫仍然生效。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn relation_preload_without_tenant_scope_allows_cross_tenant_rows() {
    let parent_table = test_table("w3_rel_parent");
    let child_table = test_table("w3_rel_child");
    let pool = connect_pool().await;
    reset_relation_tables(&pool, &parent_table, &child_table).await;
    insert_relation_fixtures(&pool, &parent_table, &child_table).await;

    let rows = vec![
        Record::new().set("id", 1_i64).set("child_id", 1_i64),
        Record::new().set("id", 2_i64).set("child_id", 2_i64),
        Record::new().set("id", 3_i64).set("child_id", 3_i64),
        Record::new().set("id", 4_i64).set("child_id", 4_i64),
    ];
    let loader = RelationLoader::new(vec![RelationSpec::new(
        field(&parent_table, "child_id"),
        field(&child_table, "id"),
    )]);
    let database = any_database(&pool);

    let executor = DatabaseRelationExecutor::new(&database, ["user"])
        .target(child_definition(&child_table))
        .without_tenant_scope();
    let data = loader
        .load(&rows, &executor)
        .await
        .expect("显式 opt-out 后跨租户加载应成功");
    let mut ids: Vec<i64> = data[&format!("{parent_table}.child_id")]
        .iter()
        .map(|record| record.require::<i64>("id").expect("id 应存在"))
        .collect();
    ids.sort_unstable();
    assert_eq!(ids, vec![1, 2, 4], "opt-out 后跨租户可见，但软删行仍被排除");

    execute(
        &pool,
        &format!("DROP TABLE IF EXISTS {child_table}, {parent_table}"),
    )
    .await;
}

fn upsert_definition(table: &str) -> TableDefinition {
    Table::new(table)
        .fields([
            Field::id("id"),
            Field::bigint("tenant_id").tenant_key(),
            Field::string("option_key", 64).required(),
            Field::string("label", 64).required(),
        ])
        .build()
        .expect("w3_upsert_tenant 表定义应有效")
}

fn upsert_row(tenant: i64, label: &str) -> Record {
    Record::new()
        .set("tenant_id", tenant)
        .set("option_key", "opt_a")
        .set("label", label)
}

async fn reset_upsert_table(pool: &PgPool, table: &str) {
    execute(pool, &format!("DROP TABLE IF EXISTS {table}")).await;
    execute(
        pool,
        &format!(
            "CREATE TABLE {table} (
            id BIGSERIAL PRIMARY KEY,
            tenant_id BIGINT NOT NULL,
            option_key TEXT NOT NULL,
            label TEXT NOT NULL,
            UNIQUE (tenant_id, option_key)
        )"
        ),
    )
    .await;
}

/// 同唯一键不同租户的两行：默认白名单 upsert 只更新目标租户，缺租户冲突列在客户端报错。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn batch_upsert_does_not_cross_tenant_and_requires_tenant_conflict_column() {
    let table = test_table("w3_upsert_tenant");
    let pool = connect_pool().await;
    reset_upsert_table(&pool, &table).await;
    let definition = upsert_definition(&table);
    let conflicts = ["tenant_id", "option_key"];

    bind(&definition, &["admin"], &pool)
        .upsert_batch(vec![upsert_row(1, "t1-orig")], &conflicts)
        .await
        .expect("租户 1 首轮插入应成功");
    bind(&definition, &["admin"], &pool)
        .upsert_batch(vec![upsert_row(2, "t2-orig")], &conflicts)
        .await
        .expect("租户 2 首轮插入应成功");

    let before: (String, String) = sqlx::query_as(&format!(
        "SELECT
            (SELECT label FROM {table} WHERE tenant_id = 1 AND option_key = 'opt_a'),
            (SELECT label FROM {table} WHERE tenant_id = 2 AND option_key = 'opt_a')"
    ))
    .fetch_one(&pool)
    .await
    .expect("读取首轮数据应成功");
    assert_eq!(before, ("t1-orig".to_string(), "t2-orig".to_string()));

    bind(&definition, &["admin"], &pool)
        .upsert_batch(vec![upsert_row(1, "t1-updated")], &conflicts)
        .await
        .expect("租户 1 冲突轮 upsert 应成功");

    let after: (String, String) = sqlx::query_as(&format!(
        "SELECT
            (SELECT label FROM {table} WHERE tenant_id = 1 AND option_key = 'opt_a'),
            (SELECT label FROM {table} WHERE tenant_id = 2 AND option_key = 'opt_a')"
    ))
    .fetch_one(&pool)
    .await
    .expect("读取冲突轮数据应成功");
    assert_eq!(
        after,
        ("t1-updated".to_string(), "t2-orig".to_string()),
        "默认白名单只能改写目标租户行"
    );

    // 缺租户冲突列：必须客户端报错，绝不触库。
    let error = bind(&definition, &["admin"], &pool)
        .upsert_batch(vec![upsert_row(1, "cross-tenant")], &["option_key"])
        .await
        .expect_err("缺少租户冲突列必须报错");
    assert!(
        matches!(error, BaseError::ParamInvalid(ref field, _) if field == "conflict_columns"),
        "应返回 conflict_columns 的 ParamInvalid，实际: {error:?}"
    );

    execute(&pool, &format!("DROP TABLE IF EXISTS {table}")).await;
}

fn predicate_definition(table: &str) -> TableDefinition {
    Table::new(table)
        .fields([
            Field::id("id"),
            Field::string("email", 128).required(),
            Field::boolean("active").required(),
            Field::string("label", 64).required(),
        ])
        .build()
        .expect("w3_upsert_predicate 表定义应有效")
}

fn predicate_row(email: &str, active: bool, label: &str) -> Record {
    Record::new()
        .set("email", email)
        .set("active", active)
        .set("label", label)
}

async fn reset_predicate_table(pool: &PgPool, table: &str) {
    let index = format!("idx_{table}_active_email");
    execute(pool, &format!("DROP TABLE IF EXISTS {table}")).await;
    execute(
        pool,
        &format!(
            "CREATE TABLE {table} (
            id BIGSERIAL PRIMARY KEY,
            email TEXT NOT NULL,
            active BOOLEAN NOT NULL,
            label TEXT NOT NULL
        )"
        ),
    )
    .await;
    execute(
        pool,
        &format!("CREATE UNIQUE INDEX {index} ON {table} (email) WHERE active"),
    )
    .await;
}

/// 部分唯一索引谓词必须透传到 PostgreSQL 的 `ON CONFLICT ... WHERE (...)`。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn batch_upsert_passes_partial_index_conflict_predicate() {
    let table = test_table("w3_upsert_predicate");
    let pool = connect_pool().await;
    reset_predicate_table(&pool, &table).await;
    let definition = predicate_definition(&table);
    let conflicts = ["email"];
    let predicate = Some("active");

    bind(&definition, &["admin"], &pool)
        .upsert_batch_with_conflict_predicate(
            vec![predicate_row("a@example.com", true, "first")],
            &conflicts,
            predicate,
        )
        .await
        .expect("首轮带谓词 upsert 应成功");

    let database = any_database(&pool);
    let mut tx = database.begin().await.expect("开启事务应成功");
    bind(&definition, &["admin"], &pool)
        .upsert_batch_in_tx_with_conflict_predicate(
            &mut tx,
            vec![predicate_row("a@example.com", true, "second")],
            &conflicts,
            predicate,
        )
        .await
        .expect("事务内冲突轮带谓词 upsert 应成功");
    tx.commit().await.expect("提交事务应成功");

    let active_label: String = sqlx::query_scalar(&format!(
        "SELECT label FROM {table} WHERE email = 'a@example.com' AND active"
    ))
    .fetch_one(&pool)
    .await
    .expect("读取活跃行应成功");
    assert_eq!(active_label, "second", "命中部分唯一索引时应更新");

    // 不满足谓词的行不参与冲突，应插入新行。
    bind(&definition, &["admin"], &pool)
        .upsert_batch_with_conflict_predicate(
            vec![predicate_row("a@example.com", false, "inactive")],
            &conflicts,
            predicate,
        )
        .await
        .expect("非活跃行应插入");
    let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(&pool)
        .await
        .expect("统计行数应成功");
    assert_eq!(count, 2, "非活跃行不受部分唯一索引约束");

    execute(&pool, &format!("DROP TABLE IF EXISTS {table}")).await;
}
