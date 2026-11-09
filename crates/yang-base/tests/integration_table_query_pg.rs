#![allow(clippy::expect_used, clippy::unwrap_used)]
#![cfg(feature = "postgres")]
//! 受保护 `TableQuery` / 内置 CRUD 的 PostgreSQL 执行层集成测试。
//!
//! 覆盖：SELECT、排序、COUNT、分页、软删除、租户过滤、INSERT / UPDATE / DELETE、
//! 事务提交与回滚、字段读/写/筛选/排序权限，以及 JSONB / DATE / TIMESTAMPTZ /
//! NUMERIC / BOOLEAN 的类型化绑定往返。
//!
//! 所有测试均标记 `#[ignore]`，默认离线套件不触库。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-base --no-default-features --features postgres \
//!   --test integration_table_query_pg -- --ignored --test-threads=1
//! ```

use serde_json::json;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::sync::Arc;
use std::time::Duration;
use yang_base::action::{ActionContext, Request, TenantContext, TenantId};
use yang_base::database::{AnyDatabase, AnyPool};
use yang_base::error::BaseError;
use yang_base::table::{Field, Record, SortOrder, Table, TableDefinition, TableQuery};
use yang_base::tools::ToolsBuilder;
use yang_db::postgres::{Database as PgDatabase, DatabaseConfig as PgDatabaseConfig};

fn test_db_url() -> String {
    std::env::var("PG_TEST_URL")
        .or_else(|_| std::env::var("YANG_POSTGRES_URL"))
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432/test".to_string())
}

async fn connect_pool() -> PgPool {
    PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&test_db_url())
        .await
        .expect("PG_TEST_URL / YANG_POSTGRES_URL 指向的 PostgreSQL 必须可连接")
}

async fn reset_tables(pool: &PgPool) {
    sqlx::query("DROP TABLE IF EXISTS tq_pg_rows, tq_pg_plain, tq_pg_typed")
        .execute(pool)
        .await
        .expect("删除旧测试表应成功");
    sqlx::query(
        "CREATE TABLE tq_pg_rows (
            id BIGSERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            age INTEGER NOT NULL,
            status TEXT NOT NULL,
            tenant_id BIGINT NOT NULL,
            secret TEXT NULL,
            deleted_at BIGINT NULL
        )",
    )
    .execute(pool)
    .await
    .expect("创建 tq_pg_rows 应成功");
    sqlx::query(
        "CREATE TABLE tq_pg_plain (
            id BIGSERIAL PRIMARY KEY,
            name TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await
    .expect("创建 tq_pg_plain 应成功");
    sqlx::query(
        "CREATE TABLE tq_pg_typed (
            id BIGSERIAL PRIMARY KEY,
            payload JSONB NOT NULL,
            day DATE NOT NULL,
            at TIMESTAMPTZ NOT NULL,
            amount NUMERIC(10,2) NOT NULL,
            flag BOOLEAN NOT NULL,
            note TEXT NULL
        )",
    )
    .execute(pool)
    .await
    .expect("创建 tq_pg_typed 应成功");
}

/// 带软删除、租户键与字段权限的主测试表定义。
fn rows_definition() -> TableDefinition {
    Table::new("tq_pg_rows")
        .fields([
            Field::id("id"),
            Field::string("name", 50).required(),
            Field::string("email", 100).required(),
            Field::integer("age").required(),
            Field::enumeration("status", ["active", "inactive"]).required(),
            Field::bigint("tenant_id").tenant_key(),
            Field::string("secret", 100)
                .readable_by(["admin"])
                .writable_by(["admin"])
                .filterable_by(["admin"])
                .sortable_by(["admin"]),
            Field::soft_delete("deleted_at"),
        ])
        .build()
        .expect("tq_pg_rows 表定义应有效")
}

/// 无软删除的简单表定义（覆盖物理删除）。
fn plain_definition() -> TableDefinition {
    Table::new("tq_pg_plain")
        .fields([Field::id("id"), Field::string("name", 50).required()])
        .build()
        .expect("tq_pg_plain 表定义应有效")
}

/// 类型化绑定的测试表定义。
fn typed_definition() -> TableDefinition {
    Table::new("tq_pg_typed")
        .fields([
            Field::id("id"),
            Field::json("payload").required(),
            Field::date("day").required(),
            Field::datetime("at").required(),
            Field::decimal("amount", 10, 2).required(),
            Field::boolean("flag").required(),
            Field::text("note"),
        ])
        .build()
        .expect("tq_pg_typed 表定义应有效")
}

/// 用 PG 连接池绑定表定义并创建受保护查询。
fn bind(definition: &TableDefinition, roles: &[&str], pool: &PgPool) -> TableQuery {
    definition
        .bind_pool(AnyPool::Postgres(Arc::new(pool.clone())))
        .query(roles.iter().copied())
}

fn row(name: &str, age: i64, status: &str, tenant_id: i64) -> Record {
    Record::new()
        .set("name", name)
        .set("email", format!("{name}@example.com"))
        .set("age", age)
        .set("status", status)
        .set("tenant_id", tenant_id)
}

fn pg_database(pool: &PgPool) -> PgDatabase {
    PgDatabase::from_pool(pool.clone(), PgDatabaseConfig::default())
        .expect("默认 PG DatabaseConfig 应通过校验")
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn select_count_pagination_and_soft_delete() {
    let pool = connect_pool().await;
    reset_tables(&pool).await;
    let definition = rows_definition();

    let (affected, first_id) = bind(&definition, &["admin"], &pool)
        .insert_returning_id(row("alice", 30, "active", 1))
        .await
        .expect("PG 整型主键 INSERT ... RETURNING 应成功");
    assert_eq!(affected, 1, "单行插入应影响 1 行");
    assert!(first_id > 0, "RETURNING 应返回正整数主键");

    bind(&definition, &["admin"], &pool)
        .insert(row("bob", 25, "inactive", 1))
        .await
        .expect("第二行插入应成功");
    bind(&definition, &["admin"], &pool)
        .insert(row("carol", 40, "active", 2))
        .await
        .expect("第三行插入应成功");

    // COUNT
    assert_eq!(
        bind(&definition, &["admin"], &pool)
            .count()
            .await
            .expect("COUNT 应成功"),
        3
    );

    // WHERE + ORDER BY + SELECT
    let rows = bind(&definition, &["admin"], &pool)
        .where_eq("status", json!("active"))
        .expect("status 可筛选")
        .order_by("id", SortOrder::Asc)
        .expect("id 可排序")
        .all()
        .await
        .expect("SELECT 应成功");
    assert_eq!(rows.len(), 2, "status=active 应有 2 行");
    assert_eq!(
        rows[0].require::<String>("name").expect("name 应存在"),
        "alice"
    );
    assert_eq!(rows[0].require::<i64>("age").expect("age 应存在"), 30);
    assert_eq!(
        rows[0].require::<String>("status").expect("status 应存在"),
        "active"
    );

    // 分页：总数不受 LIMIT/OFFSET 影响
    let page = bind(&definition, &["admin"], &pool)
        .order_by("id", SortOrder::Asc)
        .expect("id 可排序")
        .page(2, 2)
        .expect("分页参数应有效")
        .paginate_records()
        .await
        .expect("分页查询应成功");
    assert_eq!(page.total, 3, "总记录数应为 3");
    assert_eq!(page.page, 2, "当前页应为 2");
    assert_eq!(page.page_size, 2);
    assert_eq!(page.total_pages, 2);
    assert_eq!(page.data.len(), 1, "第 2 页应只剩 1 行");

    // 软删除：UPDATE 标记 + 默认读取隐藏 + with_trashed 可见
    let soft_deleted = bind(&definition, &["admin"], &pool)
        .where_eq("id", json!(first_id))
        .expect("id 可筛选")
        .delete()
        .await
        .expect("软删除应成功");
    assert_eq!(soft_deleted, 1);

    assert_eq!(
        bind(&definition, &["admin"], &pool)
            .count()
            .await
            .expect("COUNT 应成功"),
        2,
        "默认读取必须隐藏软删除行"
    );

    let trashed = bind(&definition, &["admin"], &pool)
        .with_trashed()
        .where_eq("id", json!(first_id))
        .expect("id 可筛选")
        .all()
        .await
        .expect("with_trashed 读取应成功");
    assert_eq!(trashed.len(), 1, "with_trashed 应看到软删除行");
    assert!(
        trashed[0]
            .require::<i64>("deleted_at")
            .expect("deleted_at 应存在")
            > 0,
        "deleted_at 应写入 Unix 时间戳"
    );
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn field_permissions_gate_read_filter_sort_and_write() {
    let pool = connect_pool().await;
    reset_tables(&pool).await;
    let definition = rows_definition();

    let (_, id) = bind(&definition, &["admin"], &pool)
        .insert_returning_id(row("alice", 30, "active", 1).set("secret", "s3cr3t"))
        .await
        .expect("admin 应可写入 secret 并取回主键");

    // 读取权限：user 角色不能选择 secret，默认投影也不包含它。
    let error = bind(&definition, &["user"], &pool)
        .select_fields(&["id", "secret"])
        .expect_err("user 角色读取 secret 必须失败");
    assert!(
        matches!(error, BaseError::FieldPermissionDenied(_, _, _)),
        "应返回字段权限错误: {error:?}"
    );
    let rows = bind(&definition, &["user"], &pool)
        .all()
        .await
        .expect("user 角色应能读取可读字段");
    assert!(
        rows[0].get("secret").is_none(),
        "默认投影不得包含不可读字段"
    );

    // 筛选权限
    let error = bind(&definition, &["user"], &pool)
        .where_eq("secret", json!("s3cr3t"))
        .expect_err("user 角色筛选 secret 必须失败");
    assert!(matches!(error, BaseError::FieldPermissionDenied(_, _, _)));

    // 排序权限
    let error = bind(&definition, &["user"], &pool)
        .order_by("secret", SortOrder::Asc)
        .expect_err("user 角色按 secret 排序必须失败");
    assert!(matches!(error, BaseError::FieldPermissionDenied(_, _, _)));

    // 写入权限：user 角色 UPDATE secret 必须失败
    let error = bind(&definition, &["user"], &pool)
        .where_eq("id", json!(id))
        .expect("id 可筛选")
        .update(Record::new().set("secret", "leaked"))
        .await
        .expect_err("user 角色写入 secret 必须失败");
    assert!(matches!(error, BaseError::FieldPermissionDenied(_, _, _)));

    // admin 可以读取 secret。
    let rows = bind(&definition, &["admin"], &pool)
        .all()
        .await
        .expect("admin 读取应成功");
    assert_eq!(
        rows[0].require::<String>("secret").expect("secret 应存在"),
        "s3cr3t"
    );
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn tenant_scope_filters_rows_through_action_context() {
    let pool = connect_pool().await;
    reset_tables(&pool).await;
    let definition = rows_definition();

    bind(&definition, &["admin"], &pool)
        .insert(row("tenant-one", 20, "active", 1))
        .await
        .expect("租户 1 数据插入应成功");
    bind(&definition, &["admin"], &pool)
        .insert(row("tenant-two", 30, "active", 2))
        .await
        .expect("租户 2 数据插入应成功");

    let tools = Arc::new(
        ToolsBuilder::new()
            .database(AnyDatabase::postgres(pg_database(&pool)))
            .build()
            .expect("Tools 构建应成功"),
    );
    let context = ActionContext::new(Request::new(json!({})), tools)
        .with_table_definition(definition)
        .with_tenant(TenantContext::new(TenantId::new(1)));

    let rows = context
        .table_query()
        .expect("带租户上下文的查询应可构建")
        .order_by("id", SortOrder::Asc)
        .expect("id 可排序")
        .all()
        .await
        .expect("租户过滤查询应成功");
    assert_eq!(rows.len(), 1, "只应看到当前租户的行");
    assert_eq!(
        rows[0]
            .require::<i64>("tenant_id")
            .expect("tenant_id 应存在"),
        1
    );

    // 显式筛选其他租户不会逃逸租户范围（AND 组合后为空）。
    let escaped = context
        .table_query()
        .expect("带租户上下文的查询应可构建")
        .where_eq("tenant_id", json!(2))
        .expect("tenant_id 可筛选")
        .all()
        .await
        .expect("查询应成功");
    assert!(escaped.is_empty(), "显式条件不得绕过租户范围");
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn insert_update_physical_delete_roundtrip() {
    let pool = connect_pool().await;
    reset_tables(&pool).await;
    let definition = plain_definition();

    let (affected, id) = bind(&definition, &["user"], &pool)
        .insert_returning_id(Record::new().set("name", "one"))
        .await
        .expect("物理删除表的 INSERT ... RETURNING 应成功");
    assert_eq!(affected, 1);
    assert!(id > 0);

    let affected = bind(&definition, &["user"], &pool)
        .where_eq("id", json!(id))
        .expect("id 可筛选")
        .update(Record::new().set("name", "two"))
        .await
        .expect("UPDATE 应成功");
    assert_eq!(affected, 1, "UPDATE 应影响 1 行");

    let record = bind(&definition, &["user"], &pool)
        .where_eq("id", json!(id))
        .expect("id 可筛选")
        .one()
        .await
        .expect("按主键读取应成功");
    assert_eq!(
        record.require::<String>("name").expect("name 应存在"),
        "two"
    );

    let affected = bind(&definition, &["user"], &pool)
        .where_eq("id", json!(id))
        .expect("id 可筛选")
        .delete()
        .await
        .expect("物理 DELETE 应成功");
    assert_eq!(affected, 1, "物理删除应影响 1 行");
    assert_eq!(
        bind(&definition, &["user"], &pool)
            .count()
            .await
            .expect("COUNT 应成功"),
        0,
        "物理删除后不应有记录"
    );
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn transaction_rollback_discards_protected_writes() {
    let pool = connect_pool().await;
    reset_tables(&pool).await;
    let definition = rows_definition();
    let any_database = AnyDatabase::postgres(pg_database(&pool));

    // 回滚：事务内可见，回滚后全部撤销。
    let mut tx = any_database.begin().await.expect("开启 PG 事务应成功");
    bind(&definition, &["admin"], &pool)
        .insert_in_tx(&mut tx, row("rolled-back", 20, "active", 1))
        .await
        .expect("事务内 INSERT 应成功");
    let in_tx = bind(&definition, &["admin"], &pool)
        .all_in_tx(&mut tx)
        .await
        .expect("事务内 SELECT 应成功");
    assert_eq!(in_tx.len(), 1, "事务内应可见未提交写入");
    tx.rollback().await.expect("回滚应成功");
    assert_eq!(
        bind(&definition, &["admin"], &pool)
            .count()
            .await
            .expect("COUNT 应成功"),
        0,
        "回滚后不应留下记录"
    );

    // 提交：事务内写入持久化。
    let mut tx = any_database.begin().await.expect("开启 PG 事务应成功");
    let (_, committed_id) = bind(&definition, &["admin"], &pool)
        .insert_returning_id_in_tx(&mut tx, row("committed", 30, "active", 1))
        .await
        .expect("事务内 INSERT ... RETURNING 应成功");
    assert!(committed_id > 0);
    tx.commit().await.expect("提交应成功");
    assert_eq!(
        bind(&definition, &["admin"], &pool)
            .count()
            .await
            .expect("COUNT 应成功"),
        1,
        "提交后应保留记录"
    );
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn typed_values_roundtrip_through_bindings() {
    let pool = connect_pool().await;
    reset_tables(&pool).await;
    let definition = typed_definition();

    bind(&definition, &["admin"], &pool)
        .insert(
            Record::new()
                .set("payload", json!({"k": "v", "n": 7}))
                .set("day", "2024-01-02")
                .set("at", "2024-01-02T03:04:05Z")
                .set("amount", "12.34")
                .set("flag", true)
                .set("note", "hello"),
        )
        .await
        .expect("类型化 INSERT 应成功");

    let rows = bind(&definition, &["admin"], &pool)
        .all()
        .await
        .expect("类型化 SELECT 应成功");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].get("payload"),
        Some(&json!({"k": "v", "n": 7})),
        "JSONB 应往返为 JSON 值"
    );
    assert_eq!(
        rows[0].require::<String>("day").expect("day 应存在"),
        "2024-01-02",
        "DATE 应按 YYYY-MM-DD 解码"
    );
    let at = rows[0].require::<String>("at").expect("at 应存在");
    assert!(
        at.starts_with("2024-01-02T03:04:05"),
        "TIMESTAMPTZ 应按 RFC3339 解码: {at}"
    );
    // NUMERIC 以十进制字符串返回；sqlx 的 BigDecimal 可能带尾随零
    // （NUMERIC(10,2) 实测为 "12.3400"），因此按数值断言而非字面量。
    let amount = rows[0].require::<String>("amount").expect("amount 应存在");
    let amount: f64 = amount.parse().expect("NUMERIC 应可解析为数值");
    assert!(
        (amount - 12.34).abs() < 1e-9,
        "NUMERIC 应保留十进制值: {amount}"
    );
    assert!(
        rows[0].require::<bool>("flag").expect("flag 应存在"),
        "BOOLEAN 应往返为 true"
    );

    // DATE / BOOLEAN 过滤在类型登记后应可用。
    assert_eq!(
        bind(&definition, &["admin"], &pool)
            .where_eq("day", json!("2024-01-02"))
            .expect("day 可筛选")
            .count()
            .await
            .expect("DATE 过滤应成功"),
        1
    );
    assert_eq!(
        bind(&definition, &["admin"], &pool)
            .where_eq("flag", json!(true))
            .expect("flag 可筛选")
            .count()
            .await
            .expect("BOOLEAN 过滤应成功"),
        1
    );
}
