#![cfg(feature = "postgres")]
#![allow(clippy::expect_used, clippy::unwrap_used)]
//! PostgreSQL Wave 2 查询能力真库集成测试。
//!
//! 覆盖 RETURNING 多列/别名/批量、ON CONFLICT 显式目标与部分索引谓词、
//! ILIKE/正则/IS DISTINCT FROM、行锁 OF/NOWAIT/SKIP LOCKED、宽表批量插入缩批
//! 与 MERGE 版本门控。
//!
//! 所有用例标记 `#[ignore]`。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-db --no-default-features --features postgres \
//!   --test integration_pg_wave2_query -- --ignored --test-threads=1
//! ```

use serde_json::{json, Value};
use yang_db::postgres::{Database, SqlValue};
use yang_db::{field, table, CompareOp, DbError, RowLock};

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
        "DROP SCHEMA IF EXISTS wave2_query CASCADE",
        "CREATE SCHEMA wave2_query",
        "CREATE TABLE wave2_query.users (
            id BIGSERIAL PRIMARY KEY,
            email TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            nick TEXT NULL,
            version INT NOT NULL DEFAULT 1
        )",
        "CREATE TABLE wave2_query.members (
            id BIGSERIAL PRIMARY KEY,
            email TEXT NOT NULL,
            tenant INT NOT NULL,
            active BOOLEAN NOT NULL
        )",
        "CREATE UNIQUE INDEX members_email_active
            ON wave2_query.members (email) WHERE active",
        "CREATE TABLE wave2_query.lock_rows (id BIGINT PRIMARY KEY)",
        "INSERT INTO wave2_query.lock_rows (id) VALUES (1), (2)",
        "CREATE TABLE wave2_query.merge_target (id BIGINT PRIMARY KEY, v INT NOT NULL)",
        "CREATE TABLE wave2_query.merge_source (id BIGINT NOT NULL, v INT NOT NULL)",
        "INSERT INTO wave2_query.merge_target (id, v) VALUES (1, 10)",
        "INSERT INTO wave2_query.merge_source (id, v) VALUES (1, 100), (2, 200)",
    ] {
        db.execute_with_params(sql, vec![])
            .await
            .unwrap_or_else(|error| panic!("准备 SQL 失败: {sql}: {error}"));
    }
}

#[allow(dead_code)]
#[derive(Debug, sqlx::FromRow)]
struct UserRow {
    id: i64,
    email: String,
    name: String,
}

#[allow(dead_code)]
#[derive(Debug, sqlx::FromRow)]
struct AliasedRow {
    user_id: i64,
    mail: String,
}

#[derive(Debug, sqlx::FromRow)]
struct LockId {
    id: i64,
}

fn user(email: &str, name: &str) -> Value {
    json!({ "email": email, "name": name })
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave2_returning_multi_columns_alias_and_dml_paths() {
    let db = connect().await;
    prepare(&db).await;

    // INSERT：多列 RETURNING。
    let inserted: UserRow = db
        .table(table!("wave2_query.users"))
        .returning_columns(&[field!("id"), field!("email"), field!("name")])
        .insert_returning(&user("a@x", "Alice"))
        .await
        .expect("INSERT ... RETURNING 多列应成功");
    assert_eq!(inserted.email, "a@x");
    assert_eq!(inserted.name, "Alice");

    // INSERT：别名 RETURNING。
    let aliased: AliasedRow = db
        .table(table!("wave2_query.users"))
        .returning_column_as(field!("id"), field!("user_id"))
        .returning_column_as(field!("email"), field!("mail"))
        .insert_returning(&user("b@x", "Bob"))
        .await
        .expect("INSERT ... RETURNING 别名应成功");
    assert_eq!(aliased.mail, "b@x");

    // UPDATE：多列 RETURNING。
    let updated: Vec<UserRow> = db
        .table(table!("wave2_query.users"))
        .returning_columns(&[field!("id"), field!("email"), field!("name")])
        .where_and(field!("email"), CompareOp::Eq, "a@x")
        .expect("受控条件应合法")
        .update_returning(&json!({ "name": "Alice2" }))
        .await
        .expect("UPDATE ... RETURNING 应成功");
    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].name, "Alice2");

    // UPSERT：显式冲突目标 + RETURNING。
    let upserted: UserRow = db
        .table(table!("wave2_query.users"))
        .on_conflict(&[field!("email")])
        .returning_columns(&[field!("id"), field!("email"), field!("name")])
        .upsert_returning(&user("a@x", "Alice3"))
        .await
        .expect("UPSERT ... RETURNING 应成功");
    assert_eq!(upserted.name, "Alice3");

    // DELETE：整行 RETURNING。
    let deleted: Vec<UserRow> = db
        .table(table!("wave2_query.users"))
        .returning_all()
        .where_and(field!("email"), CompareOp::Eq, "b@x")
        .expect("受控条件应合法")
        .delete_returning()
        .await
        .expect("DELETE ... RETURNING 应成功");
    assert_eq!(deleted.len(), 1);
    assert_eq!(deleted[0].name, "Bob");
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave2_insert_and_upsert_batch_returning() {
    let db = connect().await;
    prepare(&db).await;

    let inserted: Vec<AliasedRow> = db
        .table(table!("wave2_query.users"))
        .returning_column_as(field!("id"), field!("user_id"))
        .returning_column_as(field!("email"), field!("mail"))
        .insert_batch_returning(&[user("c@x", "C"), user("d@x", "D"), user("e@x", "E")])
        .await
        .expect("批量 INSERT ... RETURNING 应成功");
    assert_eq!(inserted.len(), 3);
    let mut mails: Vec<&str> = inserted.iter().map(|row| row.mail.as_str()).collect();
    mails.sort_unstable();
    assert_eq!(mails, vec!["c@x", "d@x", "e@x"]);

    let upserted: Vec<UserRow> = db
        .table(table!("wave2_query.users"))
        .returning_columns(&[field!("id"), field!("email"), field!("name")])
        .upsert_batch_returning(&[user("c@x", "C2"), user("f@x", "F")], &[field!("email")])
        .await
        .expect("批量 UPSERT ... RETURNING 应成功");
    assert_eq!(upserted.len(), 2);

    let names: Vec<String> = db
        .table(table!("wave2_query.users"))
        .field(field!("name"))
        .where_and(field!("email"), CompareOp::Eq, "c@x")
        .expect("受控条件应合法")
        .select::<(String,)>()
        .await
        .expect("查询应成功")
        .into_iter()
        .map(|(name,)| name)
        .collect();
    assert_eq!(names, vec!["C2".to_string()]);
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave2_upsert_requires_explicit_conflict_target() {
    let db = connect().await;
    prepare(&db).await;

    let error = db
        .table(table!("wave2_query.users"))
        .upsert(&user("a@x", "Alice"))
        .await
        .expect_err("未指定冲突目标的 upsert 必须在客户端报错");
    assert!(matches!(error, DbError::InvalidArgument(_)), "{error:?}");

    // do_nothing() 允许省略冲突目标。
    let affected = db
        .table(table!("wave2_query.users"))
        .do_nothing()
        .upsert(&user("a@x", "Alice"))
        .await
        .expect("DO NOTHING 应可省略冲突目标");
    assert_eq!(affected, 1);
    let again = db
        .table(table!("wave2_query.users"))
        .do_nothing()
        .upsert(&user("a@x", "Alice"))
        .await
        .expect("重复插入应被 DO NOTHING 忽略");
    assert_eq!(again, 0);
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave2_partial_unique_index_conflict_predicate() {
    let db = connect().await;
    prepare(&db).await;

    let member =
        |tenant: i64, active: bool| json!({ "email": "m@x", "tenant": tenant, "active": active });
    db.table(table!("wave2_query.members"))
        .insert(&member(1, true))
        .await
        .expect("首次插入应成功");

    let affected = db
        .table(table!("wave2_query.members"))
        .on_conflict_with_predicate(&[field!("email")], "active")
        .update_columns(&[field!("tenant")])
        .upsert(&member(2, true))
        .await
        .expect("部分唯一索引谓词 UPSERT 应命中并更新");
    assert_eq!(affected, 1);

    // 谓词为 false 的行不参与冲突：插入新行而不是更新。
    db.table(table!("wave2_query.members"))
        .on_conflict_with_predicate(&[field!("email")], "active")
        .update_columns(&[field!("tenant")])
        .upsert(&member(3, false))
        .await
        .expect("非活跃行应插入新行");

    let tenants: Vec<i32> = db
        .table(table!("wave2_query.members"))
        .field(field!("tenant"))
        .order(field!("tenant"), yang_db::SortOrder::Asc)
        .select::<(i32,)>()
        .await
        .expect("查询应成功")
        .into_iter()
        .map(|(tenant,)| tenant)
        .collect();
    assert_eq!(tenants, vec![2, 3]);

    // 注入型谓词必须在客户端被拒绝。
    let error = db
        .table(table!("wave2_query.members"))
        .on_conflict_with_predicate(&[field!("email")], "active; DROP TABLE x")
        .upsert(&member(4, true))
        .await
        .expect_err("注入型谓词必须被拒绝");
    assert!(matches!(error, DbError::InvalidArgument(_)), "{error:?}");
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave2_compare_operators_against_real_postgres() {
    let db = connect().await;
    prepare(&db).await;
    for (email, name, nick) in [
        ("1@x", "Alice", Some("Al")),
        ("2@x", "alice", None),
        ("3@x", "bob", Some("Bobby")),
        ("4@x", "carol", None),
    ] {
        db.table(table!("wave2_query.users"))
            .insert(&json!({ "email": email, "name": name, "nick": nick }))
            .await
            .expect("插入样本应成功");
    }

    let names = |op: CompareOp, value: &str| {
        let db = &db;
        let value = value.to_string();
        async move {
            db.table(table!("wave2_query.users"))
                .field(field!("name"))
                .where_and(field!("name"), op, value.as_str())
                .expect("受控条件应合法")
                .order(field!("name"), yang_db::SortOrder::Asc)
                .select::<(String,)>()
                .await
                .expect("查询应成功")
                .into_iter()
                .map(|(name,)| name)
                .collect::<Vec<_>>()
        }
    };

    assert_eq!(names(CompareOp::ILike, "AL%").await, vec!["Alice", "alice"]);
    assert_eq!(
        names(CompareOp::NotILike, "AL%").await,
        vec!["bob", "carol"]
    );
    // `~` 区分大小写：只有小写 alice 命中。
    assert_eq!(names(CompareOp::RegexMatch, "^a.*e$").await, vec!["alice"]);
    assert_eq!(
        names(CompareOp::RegexIMatch, "^a.*e$").await,
        vec!["Alice", "alice"]
    );
    // `~` 区分大小写，所以 "Alice" 不匹配 "^a.*e$"，会被 NOT 正则保留。
    assert_eq!(
        names(CompareOp::NotRegexMatch, "^a.*e$").await,
        vec!["Alice", "bob", "carol"]
    );
    assert_eq!(
        names(CompareOp::NotRegexIMatch, "^A.*E$").await,
        vec!["bob", "carol"]
    );

    let distinct: Vec<String> = db
        .table(table!("wave2_query.users"))
        .field(field!("name"))
        .where_and(field!("nick"), CompareOp::IsDistinctFrom, SqlValue::Null)
        .expect("NULL-safe 条件应合法")
        .order(field!("name"), yang_db::SortOrder::Asc)
        .select::<(String,)>()
        .await
        .expect("IS DISTINCT FROM NULL 查询应成功")
        .into_iter()
        .map(|(name,)| name)
        .collect();
    assert_eq!(distinct, vec!["Alice", "bob"]);

    let not_distinct: Vec<String> = db
        .table(table!("wave2_query.users"))
        .field(field!("name"))
        .where_and(field!("nick"), CompareOp::IsNotDistinctFrom, SqlValue::Null)
        .expect("NULL-safe 条件应合法")
        .order(field!("name"), yang_db::SortOrder::Asc)
        .select::<(String,)>()
        .await
        .expect("IS NOT DISTINCT FROM NULL 查询应成功")
        .into_iter()
        .map(|(name,)| name)
        .collect();
    assert_eq!(not_distinct, vec!["alice", "carol"]);
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave2_row_lock_variants_of_nowait_and_skip_locked() {
    let db = connect().await;
    prepare(&db).await;

    // NoKeyUpdate / KeyShare 可通过事务 select_locked 正常执行。
    for lock in [RowLock::NoKeyUpdate, RowLock::KeyShare] {
        let mut tx = db.transaction().await.expect("事务应开始");
        let rows: Vec<LockId> = tx
            .select_locked(
                db.table(table!("wave2_query.lock_rows"))
                    .where_and(field!("id"), CompareOp::Gte, 1)
                    .expect("受控条件应合法"),
                lock,
            )
            .await
            .expect("行锁 SELECT 应成功");
        assert_eq!(rows.len(), 2);
        tx.rollback().await.expect("回滚应成功");
    }

    // 持有 FOR UPDATE 后再用 NOWAIT 抢锁 → 55P03。
    let mut holder = db.transaction().await.expect("事务应开始");
    holder
        .execute_with_params(
            "SELECT id FROM wave2_query.lock_rows WHERE id = $1 FOR UPDATE",
            vec![json!(1)],
        )
        .await
        .expect("持有行锁应成功");

    let nowait_sql = db
        .table(table!("wave2_query.lock_rows"))
        .where_and(field!("id"), CompareOp::Eq, 1)
        .expect("受控条件应合法")
        .try_to_sql_with_lock(RowLock::ForUpdate, &[], true, false)
        .expect("NOWAIT SQL 应可渲染");
    assert!(nowait_sql.ends_with("FOR UPDATE NOWAIT"), "{nowait_sql}");

    let mut waiter = db.transaction().await.expect("事务应开始");
    let error = waiter
        .execute_with_params(&nowait_sql, vec![json!(1)])
        .await
        .expect_err("NOWAIT 抢锁应失败");
    assert_eq!(error.sqlstate(), Some("55P03"), "{error:?}");
    waiter.rollback().await.expect("回滚应成功");

    // 同一把锁下 SKIP LOCKED 只返回未被锁定的行。
    let skip_sql = db
        .table(table!("wave2_query.lock_rows"))
        .where_and(field!("id"), CompareOp::Gt, 0)
        .expect("受控条件应合法")
        .try_to_sql_with_lock(RowLock::ForUpdate, &[], false, true)
        .expect("SKIP LOCKED SQL 应可渲染");
    assert!(skip_sql.ends_with("FOR UPDATE SKIP LOCKED"), "{skip_sql}");
    let unlocked: Vec<LockId> = db
        .query_with_params(&skip_sql, vec![json!(0)])
        .await
        .expect("SKIP LOCKED 查询应成功");
    assert_eq!(unlocked.len(), 1);
    assert_eq!(unlocked[0].id, 2);

    holder.rollback().await.expect("回滚应成功");
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave2_insert_batch_shrinks_wide_tables_to_parameter_limit() {
    let db = connect().await;
    prepare(&db).await;

    // 140 列 × 500 行 = 70000 个参数，超过 65535 上限；缩批后应成功。
    const WIDE_COLUMNS: usize = 140;
    const WIDE_ROWS: usize = 500;
    let columns: Vec<String> = (0..WIDE_COLUMNS)
        .map(|index| format!("c{index} INT"))
        .collect();
    db.execute_with_params(
        &format!(
            "CREATE TABLE wave2_query.wide (id BIGSERIAL PRIMARY KEY, {})",
            columns.join(", ")
        ),
        vec![],
    )
    .await
    .expect("创建宽表应成功");

    let rows: Vec<Value> = (0..WIDE_ROWS)
        .map(|row| {
            let mut object = serde_json::Map::new();
            for column in 0..WIDE_COLUMNS {
                object.insert(format!("c{column}"), json!(row));
            }
            Value::Object(object)
        })
        .collect();
    let affected = db
        .table(table!("wave2_query.wide"))
        .insert_batch(&rows)
        .await
        .expect("宽表批量插入缩批后应成功");
    assert_eq!(affected, WIDE_ROWS as u64);

    let count: i64 = db
        .table(table!("wave2_query.wide"))
        .count()
        .await
        .expect("计数应成功");
    assert_eq!(count, WIDE_ROWS as i64);
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave2_merge_runs_on_supported_server_version() {
    let db = connect().await;
    prepare(&db).await;

    let affected = db
        .table(table!("wave2_query.merge_target"))
        .merge(
            table!("wave2_query.merge_source"),
            field!("id"),
            field!("id"),
            &[field!("id"), field!("v")],
        )
        .await
        .expect("PG16 上 MERGE 应成功");

    assert_eq!(affected, 2);
    let values: Vec<i32> = db
        .table(table!("wave2_query.merge_target"))
        .field(field!("v"))
        .order(field!("id"), yang_db::SortOrder::Asc)
        .select::<(i32,)>()
        .await
        .expect("MERGE 后查询应成功")
        .into_iter()
        .map(|(value,)| value)
        .collect();
    assert_eq!(values, vec![100, 200]);
}
