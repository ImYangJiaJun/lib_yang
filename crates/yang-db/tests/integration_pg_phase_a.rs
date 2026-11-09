#![allow(clippy::expect_used, clippy::unwrap_used)]
//! PostgreSQL Phase A 集成测试：RETURNING / UUID / NUMERIC / TIMESTAMPTZ /
//! ON CONFLICT / NOT / schema 限定与 search_path。
//!
//! 所有测试均标记 `#[ignore]`，默认离线套件不触库。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-db --no-default-features --features postgres \
//!   --test integration_pg_phase_a -- --ignored --test-threads=1
//! ```

use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde_json::json;
use yang_db::postgres::{Condition, Database, DatabaseConfig, SqlValue};
use yang_db::{field, table, CompareOp, SqlExpr};

fn test_db_url() -> String {
    std::env::var("PG_TEST_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432/test".to_string())
}

async fn connect(url: &str) -> Database {
    Database::connect(url)
        .await
        .expect("PG_TEST_URL 指向的 PostgreSQL 必须可连接")
}

#[derive(Debug, sqlx::FromRow)]
struct RecordRow {
    id: i64,
    external_id: uuid::Uuid,
    amount: BigDecimal,
    happened_at: DateTime<Utc>,
    happened_on: Option<NaiveDate>,
    happened_time: Option<NaiveTime>,
    name: Option<String>,
}

async fn prepare_records_table(db: &Database) {
    db.execute_with_params("CREATE SCHEMA IF NOT EXISTS phase_a_it", vec![])
        .await
        .expect("创建 schema 应成功");
    db.execute_with_params("DROP TABLE IF EXISTS phase_a_it.records", vec![])
        .await
        .expect("删除旧表应成功");
    db.execute_with_params(
        "CREATE TABLE phase_a_it.records (
            id BIGSERIAL PRIMARY KEY,
            external_id UUID NOT NULL UNIQUE,
            amount NUMERIC(20,4) NOT NULL,
            happened_at TIMESTAMPTZ NOT NULL,
            happened_on DATE,
            happened_time TIME,
            duration INTERVAL,
            name TEXT,
            last_seen TIMESTAMPTZ
        )",
        vec![],
    )
    .await
    .expect("创建测试表应成功");
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_a_returning_uuid_numeric_timestamptz_and_conflict() {
    let db = connect(&test_db_url()).await;
    prepare_records_table(&db).await;

    let external_id = uuid::Uuid::new_v4();
    let row = db
        .table(table!("phase_a_it.records"))
        .uuid(field!("external_id"))
        .decimal(field!("amount"))
        .timestamp_tz(field!("happened_at"))
        .date(field!("happened_on"))
        .time(field!("happened_time"))
        .text(field!("name"))
        .returning_all()
        .insert_returning::<_, RecordRow>(&json!({
            "external_id": external_id.to_string(),
            "amount": "42.5000",
            "happened_at": "2026-11-08T10:00:00Z",
            "happened_on": "2026-11-08",
            "happened_time": "10:30:00",
            "name": "初始",
        }))
        .await
        .expect("insert_returning 应成功");

    assert_eq!(row.id, 1);
    assert_eq!(row.external_id, external_id);
    // NUMERIC 按真实类型解码为 BigDecimal，不经过 f64。
    assert_eq!(row.amount.to_string(), "42.5000");
    // TIMESTAMPTZ 按真实类型解码为 DateTime<Utc>。
    assert_eq!(row.happened_at.to_rfc3339(), "2026-11-08T10:00:00+00:00");
    assert_eq!(row.happened_on, NaiveDate::from_ymd_opt(2026, 11, 8));
    assert_eq!(row.happened_time, NaiveTime::from_hms_opt(10, 30, 0));
    assert_eq!(row.name.as_deref(), Some("初始"));

    // insert() 仍返回整型主键（BIGSERIAL → INT8）。
    let second_id = db
        .table(table!("phase_a_it.records"))
        .uuid(field!("external_id"))
        .decimal(field!("amount"))
        .timestamp_tz(field!("happened_at"))
        .text(field!("name"))
        .insert(&json!({
            "external_id": uuid::Uuid::new_v4().to_string(),
            "amount": 7,
            "happened_at": "2026-11-07T09:00:00Z",
            "name": "第二条",
        }))
        .await
        .expect("insert 应成功");
    assert_eq!(second_id, 2);

    // INTERVAL 以微秒绑定：DB 端 `INTERVAL '2 seconds'` 与 2_000_000 微秒等价。
    db.execute_with_params(
        "INSERT INTO phase_a_it.records (external_id, amount, happened_at, duration, name)
         VALUES ('00000000-0000-0000-0000-0000000000aa', 1, '2026-11-06T00:00:00Z',
                 INTERVAL '2 seconds', 'interval')",
        vec![],
    )
    .await
    .expect("INTERVAL 测试行插入应成功");
    let interval_count: i64 = db
        .table(table!("phase_a_it.records"))
        .where_and(
            field!("duration"),
            CompareOp::Eq,
            SqlValue::interval_micros(2_000_000),
        )
        .expect("固定受控条件应合法")
        .count()
        .await
        .expect("INTERVAL 绑定查询应成功");
    assert_eq!(interval_count, 1);

    // UPDATE ... RETURNING：按真实类型解码更新后的行。
    let updated = db
        .table(table!("phase_a_it.records"))
        .where_and(field!("id"), CompareOp::Eq, row.id)
        .expect("固定受控条件应合法")
        .returning_all()
        .update_returning::<_, RecordRow>(&json!({"name": "已更新"}))
        .await
        .expect("update_returning 应成功");
    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].name.as_deref(), Some("已更新"));

    // ON CONFLICT ON CONSTRAINT + 更新列白名单 + RETURNING。
    let upserted = db
        .table(table!("phase_a_it.records"))
        .uuid(field!("external_id"))
        .decimal(field!("amount"))
        .timestamp_tz(field!("happened_at"))
        .text(field!("name"))
        .on_conflict_constraint(field!("records_external_id_key"))
        .update_columns(&[field!("name")])
        .returning_all()
        .upsert_returning::<_, RecordRow>(&json!({
            "external_id": external_id.to_string(),
            "amount": "99.0000",
            "happened_at": "2026-11-08T10:00:00Z",
            "name": "冲突更新",
        }))
        .await
        .expect("upsert_returning 应成功");
    assert_eq!(upserted.id, row.id);
    assert_eq!(upserted.name.as_deref(), Some("冲突更新"));
    // 白名单只更新 name，amount 保持原值。
    assert_eq!(upserted.amount.to_string(), "42.5000");

    // 显式 DO NOTHING：冲突时不更新、不返回受影响行。
    let do_nothing = db
        .table(table!("phase_a_it.records"))
        .uuid(field!("external_id"))
        .decimal(field!("amount"))
        .timestamp_tz(field!("happened_at"))
        .text(field!("name"))
        .on_conflict(&[field!("external_id")])
        .do_nothing()
        .upsert(&json!({
            "external_id": external_id.to_string(),
            "amount": "1.0000",
            "happened_at": "2026-11-08T10:00:00Z",
            "name": "不应写入",
        }))
        .await
        .expect("DO NOTHING upsert 应成功");
    assert_eq!(do_nothing, 0);

    // ON CONFLICT DO UPDATE ... WHERE：条件不满足时受影响行数为 0，
    // 且 RETURNING 不返回被跳过的冲突行（PostgreSQL 语义），故这里用 upsert()。
    let affected = db
        .table(table!("phase_a_it.records"))
        .uuid(field!("external_id"))
        .decimal(field!("amount"))
        .timestamp_tz(field!("happened_at"))
        .text(field!("name"))
        .on_conflict(&[field!("external_id")])
        .update_columns(&[field!("name")])
        .upsert_where_and(field!("id"), CompareOp::Gt, i64::MAX)
        .expect("受控 upsert 条件应合法")
        .upsert(&json!({
            "external_id": external_id.to_string(),
            "amount": "1.0000",
            "happened_at": "2026-11-08T10:00:00Z",
            "name": "不应写入",
        }))
        .await
        .expect("upsert 应成功");
    assert_eq!(affected, 0);
    let unchanged: Vec<RecordRow> = db
        .table(table!("phase_a_it.records"))
        .field(field!("id"))
        .field(field!("external_id"))
        .field(field!("amount"))
        .field(field!("happened_at"))
        .field(field!("happened_on"))
        .field(field!("happened_time"))
        .field(field!("name"))
        .where_and(field!("id"), CompareOp::Eq, row.id)
        .expect("固定受控条件应合法")
        .select::<RecordRow>()
        .await
        .expect("复核查询应成功");
    assert_eq!(unchanged.len(), 1);
    assert_eq!(unchanged[0].name.as_deref(), Some("冲突更新"));

    // set_expr 走服务端 now()，占位符与 WHERE 连续。
    let touched = db
        .table(table!("phase_a_it.records"))
        .where_and(field!("id"), CompareOp::Eq, row.id)
        .expect("固定受控条件应合法")
        .set_expr(field!("last_seen"), SqlExpr::now())
        // 把初始行时间推到当前时刻，保证随后的 `< now()` 断言与运行时刻无关。
        .set_expr(field!("happened_at"), SqlExpr::now())
        .returning_all()
        .update_returning::<_, RecordRow>(&json!({"name": "表达式更新"}))
        .await
        .expect("set_expr 更新应成功");
    assert_eq!(touched.len(), 1);
    assert_eq!(touched[0].name.as_deref(), Some("表达式更新"));

    // NOT 条件与表达式入口组合：所有行的 happened_at 都早于新的 now()。
    let not_matched: Vec<RecordRow> = db
        .table(table!("phase_a_it.records"))
        .field(field!("id"))
        .field(field!("external_id"))
        .field(field!("amount"))
        .field(field!("happened_at"))
        .field(field!("happened_on"))
        .field(field!("happened_time"))
        .field(field!("name"))
        .where_not(Condition::Eq(
            "name".to_string(),
            yang_db::postgres::SqlValue::String("不存在".to_string()),
        ))
        .where_expr(field!("happened_at"), CompareOp::Lt, SqlExpr::now())
        .expect("受控表达式条件应合法")
        .select::<RecordRow>()
        .await
        .expect("NOT 查询应成功");
    assert_eq!(not_matched.len(), 3);

    // DELETE ... RETURNING。
    let deleted = db
        .table(table!("phase_a_it.records"))
        .where_and(field!("id"), CompareOp::Eq, row.id)
        .expect("固定受控条件应合法")
        .returning_all()
        .delete_returning::<RecordRow>()
        .await
        .expect("delete_returning 应成功");
    assert_eq!(deleted.len(), 1);
    assert_eq!(deleted[0].id, row.id);

    db.execute_with_params("DROP SCHEMA phase_a_it CASCADE", vec![])
        .await
        .expect("清理 schema 应成功");
}

#[derive(Debug, sqlx::FromRow)]
struct UuidRow {
    id: uuid::Uuid,
    name: Option<String>,
}

/// 非整型主键：insert() 必须给出清晰错误，insert_returning 必须按真实类型取回。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_a_non_integer_primary_key() {
    let db = connect(&test_db_url()).await;
    db.execute_with_params("CREATE SCHEMA IF NOT EXISTS phase_a_it", vec![])
        .await
        .expect("创建 schema 应成功");
    db.execute_with_params("DROP TABLE IF EXISTS phase_a_it.uuid_pk", vec![])
        .await
        .expect("删除旧表应成功");
    db.execute_with_params(
        "CREATE TABLE phase_a_it.uuid_pk (id UUID PRIMARY KEY, name TEXT)",
        vec![],
    )
    .await
    .expect("创建 UUID 主键表应成功");

    let id = uuid::Uuid::new_v4();
    let error = db
        .table(table!("phase_a_it.uuid_pk"))
        .uuid(field!("id"))
        .text(field!("name"))
        .insert(&json!({"id": id.to_string(), "name": "插入"}))
        .await
        .expect_err("非整型主键的 insert() 必须失败");
    match error {
        yang_db::DbError::InvalidArgument(message) => {
            assert!(
                message.contains("insert_returning"),
                "错误信息应指引 insert_returning: {message}"
            );
        }
        other => panic!("期望 InvalidArgument，实得 {other:?}"),
    }

    // insert() 的 RETURNING 解码失败必须回滚 INSERT，不能留下半提交行。
    let remaining: Vec<(i64,)> = db
        .query_with_params(
            "SELECT COUNT(*)::BIGINT FROM phase_a_it.uuid_pk WHERE id::text = $1",
            vec![json!(id.to_string())],
        )
        .await
        .expect("复核计数应成功");
    assert_eq!(remaining[0].0, 0, "insert() 解码失败必须回滚已写入行");
    let returning_id = uuid::Uuid::new_v4();
    let row = db
        .table(table!("phase_a_it.uuid_pk"))
        .uuid(field!("id"))
        .text(field!("name"))
        .returning_all()
        .insert_returning::<_, UuidRow>(&json!({"id": returning_id.to_string(), "name": "插入"}))
        .await
        .expect("insert_returning 应成功");
    assert_eq!(row.id, returning_id);
    assert_eq!(row.name.as_deref(), Some("插入"));

    db.execute_with_params("DROP SCHEMA phase_a_it CASCADE", vec![])
        .await
        .expect("清理 schema 应成功");
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_a_search_path_and_table_exists() {
    let url = test_db_url();
    let admin = connect(&url).await;
    admin
        .execute_with_params("CREATE SCHEMA IF NOT EXISTS phase_a_search", vec![])
        .await
        .expect("创建 schema 应成功");
    admin
        .execute_with_params(
            "CREATE TABLE IF NOT EXISTS phase_a_search.items (id SERIAL PRIMARY KEY, name TEXT)",
            vec![],
        )
        .await
        .expect("创建表应成功");

    let scoped = Database::connect_with_config(
        &url,
        DatabaseConfig::default().with_search_path("phase_a_search, public"),
    )
    .await
    .expect("带 search_path 的连接应成功");

    // 限定名与 search_path 下的非限定名都要能识别。
    assert!(scoped
        .table_exists(table!("phase_a_search.items"))
        .await
        .expect("限定名 table_exists 应成功"));
    assert!(scoped
        .table_exists(table!("items"))
        .await
        .expect("search_path 下 table_exists 应成功"));
    assert!(!scoped
        .table_exists(table!("phase_a_search.missing"))
        .await
        .expect("不存在表的检查应成功"));

    // 非限定表名走 search_path 完成写入与读取。
    let id = scoped
        .table(table!("items"))
        .insert(&json!({"name": "search-path"}))
        .await
        .expect("search_path 下插入应成功");
    assert!(id > 0);
    let count: i64 = scoped
        .table(table!("items"))
        .where_and(field!("name"), CompareOp::Eq, "search-path")
        .expect("固定受控条件应合法")
        .count()
        .await
        .expect("search_path 下查询应成功");
    assert_eq!(count, 1);

    admin
        .execute_with_params("DROP SCHEMA phase_a_search CASCADE", vec![])
        .await
        .expect("清理 schema 应成功");
}

/// ON CONFLICT 的 EXCLUDED 伪表引用、ON CONSTRAINT 默认更新列，以及 NULL 三值逻辑。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_a_upsert_excluded_constraint_and_null_semantics() {
    let db = connect(&test_db_url()).await;
    db.execute_with_params("CREATE SCHEMA IF NOT EXISTS phase_a_it", vec![])
        .await
        .expect("创建 schema 应成功");

    // --- ON CONFLICT ON CONSTRAINT：默认不得覆盖主键 ---
    db.execute_with_params("DROP TABLE IF EXISTS phase_a_it.pk_guard", vec![])
        .await
        .expect("删除旧表应成功");
    db.execute_with_params(
        "CREATE TABLE phase_a_it.pk_guard (
            id BIGSERIAL PRIMARY KEY,
            email TEXT NOT NULL UNIQUE,
            name TEXT
        )",
        vec![],
    )
    .await
    .expect("创建测试表应成功");

    let original_id = db
        .table(table!("phase_a_it.pk_guard"))
        .text(field!("email"))
        .text(field!("name"))
        .insert(&json!({"email": "guard@example.com", "name": "first"}))
        .await
        .expect("首次插入应成功");

    let affected = db
        .table(table!("phase_a_it.pk_guard"))
        .text(field!("email"))
        .text(field!("name"))
        .on_conflict_constraint(field!("pk_guard_email_key"))
        .upsert(&json!({"id": 999_i64, "email": "guard@example.com", "name": "second"}))
        .await
        .expect("ON CONSTRAINT upsert 应成功");
    assert_eq!(affected, 1);
    let rows: Vec<(i64, String)> = db
        .query_with_params(
            "SELECT id, name FROM phase_a_it.pk_guard WHERE email = $1",
            vec![json!("guard@example.com")],
        )
        .await
        .expect("复核查询应成功");
    assert_eq!(rows[0].0, original_id as i64, "默认更新列不得覆盖主键");
    assert_eq!(rows[0].1, "second");

    // --- EXCLUDED 伪表：显式引用必须渲染为未加引号的 excluded ---
    let affected = db
        .table(table!("phase_a_it.pk_guard"))
        .text(field!("email"))
        .text(field!("name"))
        .on_conflict(&[field!("email")])
        .update_columns(&[field!("name")])
        .upsert_where_and(field!("EXCLUDED.name"), CompareOp::Gt, "second")
        .expect("EXCLUDED 条件应合法")
        .upsert(&json!({"email": "guard@example.com", "name": "zzz"}))
        .await
        .expect("EXCLUDED 条件 upsert 应成功");
    assert_eq!(affected, 1);
    let rows: Vec<(String,)> = db
        .query_with_params(
            "SELECT name FROM phase_a_it.pk_guard WHERE email = $1",
            vec![json!("guard@example.com")],
        )
        .await
        .expect("复核查询应成功");
    assert_eq!(rows[0].0, "zzz");

    // --- NULL 三值逻辑：NOT (v > NULL) 仍为 UNKNOWN，不得匹配任何行 ---
    db.execute_with_params("DROP TABLE IF EXISTS phase_a_it.not_null", vec![])
        .await
        .expect("删除旧表应成功");
    db.execute_with_params("CREATE TABLE phase_a_it.not_null (v INT)", vec![])
        .await
        .expect("创建测试表应成功");
    db.execute_with_params(
        "INSERT INTO phase_a_it.not_null (v) VALUES (NULL), (1), (2)",
        vec![],
    )
    .await
    .expect("插入测试数据应成功");

    let not_null_count = db
        .table(table!("phase_a_it.not_null"))
        .where_not(Condition::Gt("v".to_string(), SqlValue::Null))
        .count()
        .await
        .expect("NOT + NULL 查询应成功");
    assert_eq!(not_null_count, 0, "NOT (v > NULL) 不得匹配任何行");

    let in_count = db
        .table(table!("phase_a_it.not_null"))
        .where_in(field!("v"), vec![SqlValue::Int(1), SqlValue::Null])
        .count()
        .await
        .expect("IN + NULL 查询应成功");
    assert_eq!(in_count, 1, "IN 含 NULL 时只按非 NULL 值精确匹配");

    let not_in_count = db
        .table(table!("phase_a_it.not_null"))
        .where_predicate(&yang_db::Predicate::NotIn(
            field!("v").clone(),
            vec![json!(1), serde_json::Value::Null],
        ))
        .expect("固定受控谓词应合法")
        .count()
        .await
        .expect("NOT IN + NULL 查询应成功");
    assert_eq!(not_in_count, 0, "NOT IN 含 NULL 时恒不为 TRUE");

    db.execute_with_params("DROP SCHEMA phase_a_it CASCADE", vec![])
        .await
        .expect("清理 schema 应成功");
}
