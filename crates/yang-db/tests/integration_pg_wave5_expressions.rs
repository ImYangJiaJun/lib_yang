#![cfg(feature = "postgres")]
#![allow(clippy::expect_used, clippy::unwrap_used)]
//! PostgreSQL Wave 5 受控服务端表达式真库集成测试。
//!
//! 覆盖 `COALESCE` / `GREATEST` / `LEAST` / `NULLIF` / `strpos` /
//! `lpad` / `rpad` / `to_char` 在 `select_expr` 与 `where_expr` 入口上的
//! 真实执行结果。
//!
//! 所有用例标记 `#[ignore]`。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-db --no-default-features --features postgres \
//!   --test integration_pg_wave5_expressions -- --ignored --test-threads=1
//! ```

use yang_db::postgres::Database;
use yang_db::{field, CompareOp, ExprOperand, SortOrder, SqlExpr, TableRef};

fn test_db_url() -> String {
    std::env::var("PG_TEST_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432/test".to_string())
}

async fn connect() -> Database {
    Database::connect(&test_db_url())
        .await
        .expect("PG_TEST_URL 指向的 PostgreSQL 必须可连接")
}

fn schema_name() -> String {
    format!("wave5_expr_{}", std::process::id())
}

fn table_ref(schema: &str, name: &str) -> TableRef {
    TableRef::new(format!("{schema}.{name}")).expect("schema 限定表名应合法")
}

async fn prepare(db: &Database, schema: &str) {
    for sql in [
        format!("DROP SCHEMA IF EXISTS {schema} CASCADE"),
        format!("CREATE SCHEMA {schema}"),
        format!(
            "CREATE TABLE {schema}.expr_demo (
                id INT PRIMARY KEY,
                name TEXT NOT NULL,
                nick TEXT NULL,
                amount INT NOT NULL,
                bonus INT NULL,
                price NUMERIC(10, 2) NULL,
                date_text TEXT NULL,
                created_at TIMESTAMP NOT NULL
            )"
        ),
        format!(
            "INSERT INTO {schema}.expr_demo
                (id, name, nick, amount, bonus, price, date_text, created_at) VALUES
                (1, 'abc', NULL, 5, NULL, NULL, '2024-03-15', '2024-03-15 10:30:00'),
                (2, 'abcdef', 'x', 0, 7, 2.50, 'WRONG', '2023-01-02 00:00:00'),
                (3, 'hello', 'z', 15, 9, 0.00, 'WRONG', '2022-12-31 23:59:59')"
        ),
    ] {
        db.execute_with_params(&sql, vec![])
            .await
            .unwrap_or_else(|error| panic!("准备 SQL 失败: {sql}: {error}"));
    }
}

async fn cleanup(db: &Database, schema: &str) {
    let _ = db
        .execute_with_params(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), vec![])
        .await;
}

#[derive(Debug, sqlx::FromRow)]
struct ExprRow {
    id: i32,
    coalesced: String,
    greatest_amount: i64,
    least_amount: i64,
    position: i32,
    left_padded: String,
    right_padded: String,
    rendered: String,
    nulled: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct IdRow {
    id: i32,
}

/// `select_expr` 入口：所有表达式在真库上返回预期结果。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave5_select_expr_evaluates_whitelisted_expressions() {
    let schema = schema_name();
    let db = connect().await;
    prepare(&db, &schema).await;
    let demo = table_ref(&schema, "expr_demo");

    let rows: Vec<ExprRow> = db
        .table(&demo)
        .field(field!("id"))
        .select_expr(
            SqlExpr::coalesce(field!("nick"), "fallback"),
            field!("coalesced"),
        )
        .select_expr(
            SqlExpr::greatest(vec![
                ExprOperand::column(field!("amount")),
                ExprOperand::int(10),
            ]),
            field!("greatest_amount"),
        )
        .select_expr(
            SqlExpr::least(vec![
                ExprOperand::column(field!("amount")),
                ExprOperand::int(10),
            ]),
            field!("least_amount"),
        )
        .select_expr(SqlExpr::strpos(field!("name"), "b"), field!("position"))
        .select_expr(SqlExpr::lpad(field!("name"), 8, "."), field!("left_padded"))
        .select_expr(
            SqlExpr::rpad(field!("name"), 8, "."),
            field!("right_padded"),
        )
        .select_expr(
            SqlExpr::to_char(field!("created_at"), "YYYY-MM-DD"),
            field!("rendered"),
        )
        .select_expr(SqlExpr::nullif(field!("name"), "abcdef"), field!("nulled"))
        .order(field!("id"), SortOrder::Asc)
        .select::<ExprRow>()
        .await
        .expect("select_expr 真库查询应成功");

    assert_eq!(rows.len(), 3, "应返回三行表达式投影");

    let first = &rows[0];
    assert_eq!(first.id, 1);
    assert_eq!(first.coalesced, "fallback");
    assert_eq!(first.greatest_amount, 10);
    assert_eq!(first.least_amount, 5);
    assert_eq!(first.position, 2);
    assert_eq!(first.left_padded, ".....abc");
    assert_eq!(first.right_padded, "abc.....");
    assert_eq!(first.rendered, "2024-03-15");
    assert_eq!(first.nulled.as_deref(), Some("abc"));

    let second = &rows[1];
    assert_eq!(second.id, 2);
    assert_eq!(second.coalesced, "x");
    assert_eq!(second.greatest_amount, 10);
    assert_eq!(second.least_amount, 0);
    assert_eq!(second.position, 2);
    assert_eq!(second.left_padded, "..abcdef");
    assert_eq!(second.right_padded, "abcdef..");
    assert_eq!(second.rendered, "2023-01-02");
    assert_eq!(second.nulled, None, "NULLIF 命中时应返回 NULL");

    let third = &rows[2];
    assert_eq!(third.id, 3);
    assert_eq!(third.coalesced, "z");
    assert_eq!(third.greatest_amount, 15);
    assert_eq!(third.least_amount, 10);
    assert_eq!(third.position, 0);
    assert_eq!(third.left_padded, "...hello");
    assert_eq!(third.right_padded, "hello...");
    assert_eq!(third.rendered, "2022-12-31");
    assert_eq!(third.nulled.as_deref(), Some("hello"));

    cleanup(&db, &schema).await;
}

/// `COALESCE` / `NULLIF` 字面量按值类型绑定（整数 / numeric / 文本），
/// 而不是固定按 TEXT 绑定——否则 `integer` / `numeric` 列会报 42804/42883。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave5_coalesce_nullif_bind_typed_literals() {
    let schema = schema_name();
    let db = connect().await;
    prepare(&db, &schema).await;
    let demo = table_ref(&schema, "expr_demo");

    #[derive(Debug, sqlx::FromRow)]
    struct TypedRow {
        coalesced_bonus: i64,
        nulled_bonus: Option<i32>,
        coalesced_price: bigdecimal::BigDecimal,
        coalesced_nick: String,
    }

    let rows: Vec<TypedRow> = db
        .table(&demo)
        .select_expr(
            SqlExpr::coalesce(field!("bonus"), "0"),
            field!("coalesced_bonus"),
        )
        .select_expr(
            SqlExpr::nullif(field!("bonus"), "7"),
            field!("nulled_bonus"),
        )
        .select_expr(
            SqlExpr::coalesce(field!("price"), "0.00"),
            field!("coalesced_price"),
        )
        .select_expr(
            SqlExpr::coalesce(field!("nick"), "x"),
            field!("coalesced_nick"),
        )
        .order(field!("id"), SortOrder::Asc)
        .select::<TypedRow>()
        .await
        .expect("类型化字面量真库查询应成功");

    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].coalesced_bonus, 0);
    assert_eq!(rows[0].nulled_bonus, None);
    assert_eq!(rows[0].coalesced_price.normalized().to_string(), "0");
    assert_eq!(rows[0].coalesced_nick, "x");

    assert_eq!(rows[1].coalesced_bonus, 7);
    assert_eq!(rows[1].nulled_bonus, None, "NULLIF 命中时应返回 NULL");
    assert_eq!(rows[1].coalesced_price.normalized().to_string(), "2.5");
    assert_eq!(rows[1].coalesced_nick, "x");

    assert_eq!(rows[2].coalesced_bonus, 9);
    assert_eq!(rows[2].nulled_bonus, Some(9));
    assert_eq!(rows[2].coalesced_price.normalized().to_string(), "0");
    assert_eq!(rows[2].coalesced_nick, "z");

    cleanup(&db, &schema).await;
}

/// `where_expr` 入口：表达式作为比较右值时参数编号与条件渲染正确。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave5_where_expr_filters_with_whitelisted_expressions() {
    let schema = schema_name();
    let db = connect().await;
    prepare(&db, &schema).await;
    let demo = table_ref(&schema, "expr_demo");
    let amount = field!("amount");

    // amount = GREATEST(amount, 10) 只有 amount > 10 的行（id=3）成立。
    let rows: Vec<IdRow> = db
        .table(&demo)
        .field(field!("id"))
        .where_expr(
            amount,
            CompareOp::Eq,
            SqlExpr::greatest(vec![ExprOperand::column(amount), ExprOperand::int(10)]),
        )
        .expect("受控表达式比较应合法")
        .order(field!("id"), SortOrder::Asc)
        .select::<IdRow>()
        .await
        .expect("where_expr 真库查询应成功");
    assert_eq!(rows.iter().map(|row| row.id).collect::<Vec<_>>(), vec![3]);

    // nick = COALESCE(nick, 'x')：NULL 行比较结果为 UNKNOWN（id=1 被过滤），
    // 其余非 NULL 行恒等成立（id=2、3）。
    let rows: Vec<IdRow> = db
        .table(&demo)
        .field(field!("id"))
        .where_expr(
            field!("nick"),
            CompareOp::Eq,
            SqlExpr::coalesce(field!("nick"), "x"),
        )
        .expect("受控表达式比较应合法")
        .select::<IdRow>()
        .await
        .expect("where_expr 真库查询应成功");
    assert_eq!(
        rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![2, 3]
    );

    // amount = LEAST(amount, 10)：amount <= 10 的行（id=1、2）成立。
    let rows: Vec<IdRow> = db
        .table(&demo)
        .field(field!("id"))
        .where_expr(
            amount,
            CompareOp::Eq,
            SqlExpr::least(vec![ExprOperand::column(amount), ExprOperand::int(10)]),
        )
        .expect("受控表达式比较应合法")
        .order(field!("id"), SortOrder::Asc)
        .select::<IdRow>()
        .await
        .expect("where_expr 真库查询应成功");
    assert_eq!(
        rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![1, 2]
    );

    // name = NULLIF(name, 'abcdef')：id=2 的右值为 NULL（UNKNOWN），其余恒等成立。
    let rows: Vec<IdRow> = db
        .table(&demo)
        .field(field!("id"))
        .where_expr(
            field!("name"),
            CompareOp::Eq,
            SqlExpr::nullif(field!("name"), "abcdef"),
        )
        .expect("受控表达式比较应合法")
        .order(field!("id"), SortOrder::Asc)
        .select::<IdRow>()
        .await
        .expect("where_expr 真库查询应成功");
    assert_eq!(
        rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![1, 3]
    );

    // id = strpos(name, 'b')：只有 id=2 的 'abcdef' 中 'b' 位于第 2 位。
    let rows: Vec<IdRow> = db
        .table(&demo)
        .field(field!("id"))
        .where_expr(
            field!("id"),
            CompareOp::Eq,
            SqlExpr::strpos(field!("name"), "b"),
        )
        .expect("受控表达式比较应合法")
        .order(field!("id"), SortOrder::Asc)
        .select::<IdRow>()
        .await
        .expect("where_expr 真库查询应成功");
    assert_eq!(rows.iter().map(|row| row.id).collect::<Vec<_>>(), vec![2]);

    // name = lpad(name, 3, '.')：id=1 的 'abc' 长度已是 3，填充为恒等。
    let rows: Vec<IdRow> = db
        .table(&demo)
        .field(field!("id"))
        .where_expr(
            field!("name"),
            CompareOp::Eq,
            SqlExpr::lpad(field!("name"), 3, "."),
        )
        .expect("受控表达式比较应合法")
        .order(field!("id"), SortOrder::Asc)
        .select::<IdRow>()
        .await
        .expect("where_expr 真库查询应成功");
    assert_eq!(rows.iter().map(|row| row.id).collect::<Vec<_>>(), vec![1]);

    // name = rpad(name, 5, '.')：id=3 的 'hello' 长度已是 5，填充为恒等。
    let rows: Vec<IdRow> = db
        .table(&demo)
        .field(field!("id"))
        .where_expr(
            field!("name"),
            CompareOp::Eq,
            SqlExpr::rpad(field!("name"), 5, "."),
        )
        .expect("受控表达式比较应合法")
        .order(field!("id"), SortOrder::Asc)
        .select::<IdRow>()
        .await
        .expect("where_expr 真库查询应成功");
    assert_eq!(rows.iter().map(|row| row.id).collect::<Vec<_>>(), vec![3]);

    // date_text = to_char(created_at, 'YYYY-MM-DD')：只有 id=1 的文本匹配。
    let rows: Vec<IdRow> = db
        .table(&demo)
        .field(field!("id"))
        .where_expr(
            field!("date_text"),
            CompareOp::Eq,
            SqlExpr::to_char(field!("created_at"), "YYYY-MM-DD"),
        )
        .expect("受控表达式比较应合法")
        .order(field!("id"), SortOrder::Asc)
        .select::<IdRow>()
        .await
        .expect("where_expr 真库查询应成功");
    assert_eq!(rows.iter().map(|row| row.id).collect::<Vec<_>>(), vec![1]);

    cleanup(&db, &schema).await;
}

/// 服务端表达式的文本参数确实走绑定，而不是拼接进 SQL 文本。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave5_expression_parameters_are_bound_not_interpolated() {
    let schema = schema_name();
    let db = connect().await;
    prepare(&db, &schema).await;
    let demo = table_ref(&schema, "expr_demo");

    let sql = db
        .table(&demo)
        .field(field!("id"))
        .select_expr(
            SqlExpr::coalesce(field!("nick"), "'; DROP TABLE expr_demo; --"),
            field!("coalesced"),
        )
        .try_to_sql()
        .expect("受控表达式应生成 SQL");

    assert!(sql.contains("COALESCE(\"nick\", $1)"), "{sql}");
    assert!(!sql.contains("DROP TABLE"), "文本参数绝不能内联: {sql}");

    // 真库执行同一查询，注入载荷只作为普通字符串返回。
    let rows: Vec<InjectionRow> = db
        .table(&demo)
        .field(field!("id"))
        .select_expr(
            SqlExpr::coalesce(field!("nick"), "'; DROP TABLE expr_demo; --"),
            field!("coalesced"),
        )
        .order(field!("id"), SortOrder::Asc)
        .select::<InjectionRow>()
        .await
        .expect("参数化表达式查询应成功");
    assert_eq!(rows[0].id, 1);
    assert_eq!(rows[0].coalesced, "'; DROP TABLE expr_demo; --");
    assert_eq!(rows[1].coalesced, "x");

    cleanup(&db, &schema).await;
}

#[derive(Debug, sqlx::FromRow)]
struct InjectionRow {
    id: i32,
    coalesced: String,
}
