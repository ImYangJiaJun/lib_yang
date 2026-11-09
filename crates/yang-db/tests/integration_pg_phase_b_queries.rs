#![allow(clippy::expect_used, clippy::unwrap_used)]
//! PostgreSQL Phase B 集成测试：CTE/递归、窗口、分组集、DISTINCT ON、JOIN 扩展、
//! 集合运算、HAVING 表达式、批量 UPSERT 与联表 DML（含 PG15+ MERGE）。
//!
//! 所有测试均标记 `#[ignore]`，默认离线套件不触库。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-db --no-default-features --features postgres \
//!   --test integration_pg_phase_b_queries -- --ignored --test-threads=1
//! ```

use serde_json::json;
use yang_db::postgres::query_builder::{FrameBound, WindowFrame, WindowFunction, WindowSpec};
use yang_db::postgres::{Database, FieldType, SqlValue};
use yang_db::{field, table, CompareOp, SelectExpr, SortOrder};

fn test_db_url() -> String {
    std::env::var("PG_TEST_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432/test".to_string())
}

async fn connect(url: &str) -> Database {
    Database::connect(url)
        .await
        .expect("PG_TEST_URL 指向的 PostgreSQL 必须可连接")
}

async fn prepare(db: &Database) {
    // 先整体清理，避免上次失败残留的表/类型阻塞后续运行。
    db.execute_with_params("DROP SCHEMA IF EXISTS phase_b_q CASCADE", vec![])
        .await
        .expect("清理旧 schema 应成功");
    db.execute_with_params("CREATE SCHEMA phase_b_q", vec![])
        .await
        .expect("创建 schema 应成功");
    for table_name in [
        "orders",
        "contacts",
        "staging",
        "archive",
        "dept_info",
        "docs",
        "employees",
    ] {
        db.execute_with_params(
            &format!("DROP TABLE IF EXISTS phase_b_q.{table_name} CASCADE"),
            vec![],
        )
        .await
        .expect("删除旧表应成功");
    }
    db.execute_with_params(
        "CREATE TABLE phase_b_q.employees (
            id BIGSERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            dept TEXT NOT NULL,
            salary INT NOT NULL,
            manager_id BIGINT
        )",
        vec![],
    )
    .await
    .expect("创建 employees 应成功");
    db.execute_with_params(
        "INSERT INTO phase_b_q.employees (name, dept, salary, manager_id) VALUES
            ('alice', 'eng', 100, NULL),
            ('bob',   'eng',  90, 1),
            ('carol', 'sales', 80, 1),
            ('dave',  'sales', 70, 3)",
        vec![],
    )
    .await
    .expect("插入 employees 应成功");
    db.execute_with_params(
        "CREATE TABLE phase_b_q.orders (
            id BIGSERIAL PRIMARY KEY,
            employee_id BIGINT NOT NULL,
            amount INT NOT NULL
        )",
        vec![],
    )
    .await
    .expect("创建 orders 应成功");
    db.execute_with_params(
        "INSERT INTO phase_b_q.orders (employee_id, amount) VALUES (1, 10), (1, 20), (3, 30)",
        vec![],
    )
    .await
    .expect("插入 orders 应成功");
    db.execute_with_params(
        "CREATE TABLE phase_b_q.staging (
            id BIGINT PRIMARY KEY,
            name TEXT NOT NULL,
            dept TEXT NOT NULL,
            salary INT NOT NULL
        )",
        vec![],
    )
    .await
    .expect("创建 staging 应成功");
    db.execute_with_params(
        "CREATE TABLE phase_b_q.archive (id BIGINT PRIMARY KEY, name TEXT NOT NULL)",
        vec![],
    )
    .await
    .expect("创建 archive 应成功");
    db.execute_with_params(
        "CREATE TABLE phase_b_q.dept_info (dept TEXT PRIMARY KEY, location TEXT NOT NULL)",
        vec![],
    )
    .await
    .expect("创建 dept_info 应成功");
    db.execute_with_params(
        "INSERT INTO phase_b_q.dept_info (dept, location) VALUES ('eng', 'floor-1'), ('sales', 'floor-2')",
        vec![],
    )
    .await
    .expect("插入 dept_info 应成功");
    db.execute_with_params(
        "CREATE TABLE phase_b_q.docs (
            id BIGSERIAL PRIMARY KEY,
            label TEXT NOT NULL,
            data JSONB NOT NULL,
            tags TEXT[] NOT NULL
        )",
        vec![],
    )
    .await
    .expect("创建 docs 应成功");
    db.execute_with_params(
        "INSERT INTO phase_b_q.docs (label, data, tags) VALUES
            ('r1', '{\"role\":\"admin\",\"nested\":{\"x\":1}}'::jsonb, ARRAY['a','b']),
            ('r2', '{\"role\":\"user\"}'::jsonb, ARRAY['b']),
            ('r3', '{\"role\":\"admin\"}'::jsonb, ARRAY['c'])",
        vec![],
    )
    .await
    .expect("插入 docs 应成功");
}

async fn cleanup(db: &Database) {
    db.execute_with_params("DROP SCHEMA phase_b_q CASCADE", vec![])
        .await
        .expect("清理 schema 应成功");
}

#[derive(Debug, sqlx::FromRow)]
struct IdNameRow {
    id: i64,
    name: String,
}

#[derive(Debug, sqlx::FromRow)]
struct IdRow {
    id: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct RankedRow {
    id: i64,
    rn: i64,
    running: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct DeptTotalRow {
    dept: Option<String>,
    total: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct DeptCountRow {
    dept: String,
    count: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct DistinctRow {
    dept: String,
    name: String,
    salary: i32,
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_b_cte_recursive_window_grouping_distinct_and_having() {
    let db = connect(&test_db_url()).await;
    prepare(&db).await;

    // --- CTE：预过滤后作为主查询的数据源 ---
    let high = db
        .table(table!("phase_b_q.employees"))
        .field(field!("id"))
        .field(field!("name"))
        .where_and(field!("salary"), CompareOp::Gte, 90)
        .expect("受控条件应合法");
    let rows = db
        .table(table!("high"))
        .field(field!("id"))
        .field(field!("name"))
        .with("high", high)
        .order(field!("id"), SortOrder::Asc)
        .select::<IdNameRow>()
        .await
        .expect("CTE 查询应成功");
    assert_eq!(
        rows.iter()
            .map(|row| (row.id, row.name.as_str()))
            .collect::<Vec<_>>(),
        vec![(1, "alice"), (2, "bob")]
    );

    // --- 递归 CTE：从根节点向下遍历汇报链 ---
    let base = db
        .table(table!("phase_b_q.employees"))
        .field(field!("id"))
        .field(field!("name"))
        .field(field!("manager_id"))
        .where_null(field!("manager_id"));
    let recursive = db
        .table(table!("phase_b_q.employees"))
        .field(field!("employees.id"))
        .field(field!("employees.name"))
        .field(field!("employees.manager_id"))
        .join(
            table!("chain"),
            field!("employees.manager_id"),
            field!("chain.id"),
        );
    let chain = base.union_all(recursive).expect("递归分支列数应一致");
    let rows = db
        .table(table!("chain"))
        .field(field!("id"))
        .field(field!("name"))
        .with_recursive("chain", chain)
        .order(field!("id"), SortOrder::Asc)
        .select::<IdNameRow>()
        .await
        .expect("递归 CTE 查询应成功");
    assert_eq!(
        rows.iter().map(|row| row.name.as_str()).collect::<Vec<_>>(),
        vec!["alice", "bob", "carol", "dave"]
    );

    // --- 窗口：分组排名 + 累计和 ---
    let ranked = db
        .table(table!("phase_b_q.employees"))
        .field(field!("id"))
        .window_expr(
            WindowFunction::row_number(),
            WindowSpec::new()
                .partition_by(&[field!("dept")])
                .order_by(field!("salary"), SortOrder::Desc),
            field!("rn"),
        )
        .window_expr(
            WindowFunction::sum(field!("salary")),
            WindowSpec::new()
                .partition_by(&[field!("dept")])
                .order_by(field!("salary"), SortOrder::Desc)
                .frame(WindowFrame::rows_between(
                    FrameBound::UnboundedPreceding,
                    FrameBound::CurrentRow,
                )),
            field!("running"),
        )
        .order(field!("id"), SortOrder::Asc)
        .select::<RankedRow>()
        .await
        .expect("窗口函数查询应成功");
    // 部门内按薪资降序：alice=1/100，bob=2/190；carol=1/80，dave=2/150。
    assert_eq!(
        ranked
            .iter()
            .map(|row| (row.id, row.rn, row.running))
            .collect::<Vec<_>>(),
        vec![(1, 1, 100), (2, 2, 190), (3, 1, 80), (4, 2, 150)]
    );

    // --- GROUPING SETS：按部门 + 总计 ---
    let totals = db
        .table(table!("phase_b_q.employees"))
        .field(field!("dept"))
        .expr(SelectExpr::sum(field!("salary")).alias(field!("total")))
        .grouping_sets(vec![vec![field!("dept").clone()], vec![]])
        .order(field!("dept"), SortOrder::Asc)
        .select::<DeptTotalRow>()
        .await
        .expect("GROUPING SETS 查询应成功");
    assert_eq!(
        totals
            .iter()
            .map(|row| (row.dept.clone(), row.total))
            .collect::<Vec<_>>(),
        vec![
            (Some("eng".to_string()), 190),
            (Some("sales".to_string()), 150),
            (None, 340),
        ]
    );

    // --- HAVING COUNT(*) > 1 ---
    let groups = db
        .table(table!("phase_b_q.employees"))
        .field(field!("dept"))
        .expr(SelectExpr::count_all())
        .group(field!("dept"))
        .having_expr(SelectExpr::count_all(), CompareOp::Gt, 1)
        .expect("受控 HAVING 表达式应合法")
        .order(field!("dept"), SortOrder::Asc)
        .select::<DeptCountRow>()
        .await
        .expect("HAVING 聚合查询应成功");
    assert_eq!(
        groups
            .iter()
            .map(|row| (row.dept.as_str(), row.count))
            .collect::<Vec<_>>(),
        vec![("eng", 2), ("sales", 2)]
    );

    // 无 GROUP BY 的聚合 HAVING：谓词必须真实生效（4 > 1000 不成立 → 0 行）。
    let total = db
        .table(table!("phase_b_q.employees"))
        .having_expr(SelectExpr::count_all(), CompareOp::Gt, 1000)
        .expect("受控 HAVING 表达式应合法")
        .count()
        .await
        .expect("无 GROUP BY 的 HAVING 查询应成功");
    assert_eq!(total, 0, "HAVING COUNT(*) > 1000 不应返回任何行");

    let total = db
        .table(table!("phase_b_q.employees"))
        .having_expr(SelectExpr::count_all(), CompareOp::Gte, 3)
        .expect("受控 HAVING 表达式应合法")
        .count()
        .await
        .expect("无 GROUP BY 的 HAVING 查询应成功");
    assert_eq!(total, 4, "HAVING COUNT(*) >= 3 应保留整表计数行");

    // --- DISTINCT ON：每个部门取薪资最高者 ---
    let first_per_dept = db
        .table(table!("phase_b_q.employees"))
        .field(field!("dept"))
        .field(field!("name"))
        .field(field!("salary"))
        .distinct_on(&[field!("dept")])
        .order(field!("dept"), SortOrder::Asc)
        .order(field!("salary"), SortOrder::Desc)
        .select::<DistinctRow>()
        .await
        .expect("DISTINCT ON 查询应成功");
    assert_eq!(
        first_per_dept
            .iter()
            .map(|row| (row.dept.as_str(), row.name.as_str(), row.salary))
            .collect::<Vec<_>>(),
        vec![("eng", "alice", 100), ("sales", "carol", 80)]
    );

    cleanup(&db).await;
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_b_set_operations_execute() {
    let db = connect(&test_db_url()).await;
    prepare(&db).await;

    let employee_ids = || db.table(table!("phase_b_q.employees")).field(field!("id"));
    let order_employee_ids = || {
        db.table(table!("phase_b_q.orders"))
            .field(field!("employee_id"))
    };

    let intersect = employee_ids()
        .intersect(order_employee_ids())
        .expect("列数一致")
        .order(field!("id"), SortOrder::Asc)
        .select::<IdRow>()
        .await
        .expect("INTERSECT 应成功");
    assert_eq!(
        intersect.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![1, 3]
    );

    let except = employee_ids()
        .except(order_employee_ids())
        .expect("列数一致")
        .order(field!("id"), SortOrder::Asc)
        .select::<IdRow>()
        .await
        .expect("EXCEPT 应成功");
    assert_eq!(
        except.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![2, 4]
    );

    let intersect_all = employee_ids()
        .intersect_all(order_employee_ids())
        .expect("列数一致")
        .order(field!("id"), SortOrder::Asc)
        .select::<IdRow>()
        .await
        .expect("INTERSECT ALL 应成功");
    assert_eq!(
        intersect_all.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![1, 3]
    );

    let except_all = employee_ids()
        .except_all(order_employee_ids())
        .expect("列数一致")
        .order(field!("id"), SortOrder::Asc)
        .select::<IdRow>()
        .await
        .expect("EXCEPT ALL 应成功");
    assert_eq!(
        except_all.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![2, 4]
    );

    let union_all = employee_ids()
        .where_and(field!("dept"), CompareOp::Eq, "eng")
        .expect("受控条件应合法")
        .union_all(order_employee_ids())
        .expect("列数一致")
        .order(field!("id"), SortOrder::Asc)
        .select::<IdRow>()
        .await
        .expect("UNION ALL 应成功");
    assert_eq!(
        union_all.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![1, 1, 1, 2, 3]
    );

    cleanup(&db).await;
}

#[derive(Debug, sqlx::FromRow)]
struct FullJoinRow {
    id: Option<i64>,
    name: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct DeptLocationRow {
    dept: String,
    location: String,
}

#[derive(Debug, sqlx::FromRow)]
struct LateralRow {
    id: i64,
    amount: Option<i32>,
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_b_join_extensions_execute() {
    let db = connect(&test_db_url()).await;
    prepare(&db).await;
    db.execute_with_params(
        "INSERT INTO phase_b_q.staging (id, name, dept, salary) VALUES
            (1, 'alice', 'eng', 100), (9, 'zoe', 'hr', 60)",
        vec![],
    )
    .await
    .expect("插入 staging 应成功");

    // FULL JOIN：左表全保留，右表未匹配行的 id 为 NULL。
    let full = db
        .table(table!("phase_b_q.employees"))
        .field(field!("employees.id"))
        .field(field!("staging.name"))
        .full_join(
            table!("phase_b_q.staging"),
            field!("employees.id"),
            field!("staging.id"),
        )
        .order(field!("employees.id"), SortOrder::Asc)
        .select::<FullJoinRow>()
        .await
        .expect("FULL JOIN 应成功");
    assert_eq!(full.len(), 5, "4 个员工 + 1 个未匹配 staging 行");
    assert_eq!(full[0].name.as_deref(), Some("alice"));
    let unmatched = full.last().expect("应有未匹配行");
    assert_eq!(unmatched.id, None);
    assert_eq!(unmatched.name.as_deref(), Some("zoe"));

    // CROSS JOIN：4 × 2 = 8 行。
    let cross_count = db
        .table(table!("phase_b_q.employees"))
        .cross_join(table!("phase_b_q.staging"))
        .count()
        .await
        .expect("CROSS JOIN 应成功");
    assert_eq!(cross_count, 8);

    // NATURAL JOIN：按同名 dept 列连接，输出单份 dept。
    let natural = db
        .table(table!("phase_b_q.employees"))
        .field(field!("dept"))
        .field(field!("location"))
        .natural_join(table!("phase_b_q.dept_info"))
        .order(field!("dept"), SortOrder::Asc)
        .select::<DeptLocationRow>()
        .await
        .expect("NATURAL JOIN 应成功");
    assert_eq!(
        natural
            .iter()
            .map(|row| (row.dept.as_str(), row.location.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("eng", "floor-1"),
            ("eng", "floor-1"),
            ("sales", "floor-2"),
            ("sales", "floor-2"),
        ]
    );

    // JOIN ... USING：与 NATURAL 结果一致。
    let using = db
        .table(table!("phase_b_q.employees"))
        .field(field!("dept"))
        .field(field!("location"))
        .join_using(table!("phase_b_q.dept_info"), &[field!("dept")])
        .order(field!("dept"), SortOrder::Asc)
        .select::<DeptLocationRow>()
        .await
        .expect("JOIN USING 应成功");
    assert_eq!(
        using
            .iter()
            .map(|row| (row.dept.as_str(), row.location.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("eng", "floor-1"),
            ("eng", "floor-1"),
            ("sales", "floor-2"),
            ("sales", "floor-2"),
        ]
    );

    // LEFT JOIN LATERAL：每个员工取金额最大的一笔订单。
    let lateral = db
        .table(table!("phase_b_q.orders"))
        .field(field!("employee_id"))
        .field(field!("amount"))
        .where_and(field!("amount"), CompareOp::Gt, 0)
        .expect("受控条件应合法")
        .where_column(
            field!("orders.employee_id"),
            CompareOp::Eq,
            field!("employees.id"),
        )
        .expect("受控列比较应合法")
        .order(field!("amount"), SortOrder::Desc)
        .limit(1);
    let rows = db
        .table(table!("phase_b_q.employees"))
        .field(field!("employees.id"))
        .field(field!("o.amount"))
        .left_join_lateral(
            lateral,
            field!("o"),
            field!("o.employee_id"),
            field!("employees.id"),
        )
        .order(field!("employees.id"), SortOrder::Asc)
        .select::<LateralRow>()
        .await
        .expect("LEFT JOIN LATERAL 应成功");
    assert_eq!(
        rows.iter()
            .map(|row| (row.id, row.amount))
            .collect::<Vec<_>>(),
        vec![(1, Some(20)), (2, None), (3, Some(30)), (4, None)]
    );

    cleanup(&db).await;
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_b_batch_upsert_and_join_dml() {
    let db = connect(&test_db_url()).await;
    prepare(&db).await;

    // --- 批量 UPSERT：插入 ---
    let inserted = db
        .table(table!("phase_b_q.staging"))
        .upsert_batch(
            &[
                json!({"id": 1, "name": "n1", "dept": "eng", "salary": 10}),
                json!({"id": 2, "name": "n2", "dept": "eng", "salary": 20}),
            ],
            &[field!("id")],
        )
        .await
        .expect("批量 UPSERT 插入应成功");
    assert_eq!(inserted, 2);

    // --- 批量 UPSERT：冲突更新 + 新行 ---
    let affected = db
        .table(table!("phase_b_q.staging"))
        .upsert_batch(
            &[
                json!({"id": 1, "name": "n1-updated", "dept": "eng", "salary": 11}),
                json!({"id": 3, "name": "n3", "dept": "sales", "salary": 30}),
            ],
            &[field!("id")],
        )
        .await
        .expect("批量 UPSERT 冲突更新应成功");
    assert_eq!(affected, 2);
    let rows: Vec<(i64, String, i32)> = db
        .query_with_params(
            "SELECT id, name, salary FROM phase_b_q.staging ORDER BY id",
            vec![],
        )
        .await
        .expect("复核 staging 应成功");
    assert_eq!(
        rows,
        vec![
            (1, "n1-updated".to_string(), 11),
            (2, "n2".to_string(), 20),
            (3, "n3".to_string(), 30),
        ]
    );

    // --- 批量 UPSERT：DO NOTHING ---
    let skipped = db
        .table(table!("phase_b_q.staging"))
        .do_nothing()
        .upsert_batch(
            &[json!({"id": 1, "name": "ignored", "dept": "x", "salary": 0})],
            &[field!("id")],
        )
        .await
        .expect("DO NOTHING 批量 UPSERT 应成功");
    assert_eq!(skipped, 0);

    // --- INSERT ... SELECT ---
    let source = db
        .table(table!("phase_b_q.employees"))
        .field(field!("id"))
        .field(field!("name"))
        .where_and(field!("dept"), CompareOp::Eq, "eng")
        .expect("受控条件应合法");
    let copied = db
        .table(table!("phase_b_q.archive"))
        .insert_from_select(&[field!("id"), field!("name")], source)
        .await
        .expect("INSERT ... SELECT 应成功");
    assert_eq!(copied, 2);
    let archived: Vec<(i64, String)> = db
        .query_with_params("SELECT id, name FROM phase_b_q.archive ORDER BY id", vec![])
        .await
        .expect("复核 archive 应成功");
    assert_eq!(
        archived,
        vec![(1, "alice".to_string()), (2, "bob".to_string())]
    );

    // --- UPDATE ... FROM ---
    let updated = db
        .table(table!("phase_b_q.staging"))
        .where_and(field!("employees.dept"), CompareOp::Eq, "eng")
        .expect("受控条件应合法")
        .update_from(
            &json!({"name": "from-join"}),
            table!("phase_b_q.employees"),
            field!("staging.id"),
            field!("employees.id"),
        )
        .await
        .expect("UPDATE ... FROM 应成功");
    assert_eq!(updated, 2, "staging 中 id=1,2 对应 eng 员工");
    let names: Vec<(i64, String)> = db
        .query_with_params("SELECT id, name FROM phase_b_q.staging ORDER BY id", vec![])
        .await
        .expect("复核 staging 应成功");
    assert_eq!(
        names,
        vec![
            (1, "from-join".to_string()),
            (2, "from-join".to_string()),
            (3, "n3".to_string()),
        ]
    );

    // --- DELETE ... USING ---
    let deleted = db
        .table(table!("phase_b_q.staging"))
        .where_and(field!("employees.dept"), CompareOp::Eq, "sales")
        .expect("受控条件应合法")
        .delete_using(
            table!("phase_b_q.employees"),
            field!("staging.id"),
            field!("employees.id"),
        )
        .await
        .expect("DELETE ... USING 应成功");
    assert_eq!(deleted, 1, "staging id=3 对应 sales 员工");
    let remaining: Vec<(i64,)> = db
        .query_with_params("SELECT id FROM phase_b_q.staging ORDER BY id", vec![])
        .await
        .expect("复核 staging 应成功");
    assert_eq!(remaining, vec![(1,), (2,)]);

    // --- MERGE（PostgreSQL 15+）---
    let merged = db
        .table(table!("phase_b_q.staging"))
        .merge(
            table!("phase_b_q.employees"),
            field!("id"),
            field!("id"),
            &[
                field!("id"),
                field!("name"),
                field!("dept"),
                field!("salary"),
            ],
        )
        .await
        .expect("MERGE 应成功");
    assert_eq!(merged, 4, "2 行匹配更新 + 2 行未匹配插入");
    let merged_rows: Vec<(i64, String, String, i32)> = db
        .query_with_params(
            "SELECT id, name, dept, salary FROM phase_b_q.staging ORDER BY id",
            vec![],
        )
        .await
        .expect("复核 staging 应成功");
    assert_eq!(
        merged_rows,
        vec![
            (1, "alice".to_string(), "eng".to_string(), 100),
            (2, "bob".to_string(), "eng".to_string(), 90),
            (3, "carol".to_string(), "sales".to_string(), 80),
            (4, "dave".to_string(), "sales".to_string(), 70),
        ]
    );

    cleanup(&db).await;
}

#[derive(Debug, sqlx::FromRow)]
struct LabelRow {
    label: String,
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_b_json_conditions_and_array_binding() {
    let db = connect(&test_db_url()).await;
    prepare(&db).await;

    let labels = db
        .table(table!("phase_b_q.docs"))
        .where_json_contains(field!("data"), SqlValue::Json(json!({"role": "admin"})))
        .order(field!("id"), SortOrder::Asc)
        .select::<LabelRow>()
        .await
        .expect("JSON 包含查询应成功");
    assert_eq!(
        labels
            .iter()
            .map(|row| row.label.as_str())
            .collect::<Vec<_>>(),
        vec!["r1", "r3"]
    );

    let labels = db
        .table(table!("phase_b_q.docs"))
        .where_json_has_key(field!("data"), "nested")
        .select::<LabelRow>()
        .await
        .expect("JSON 键存在查询应成功");
    assert_eq!(
        labels
            .iter()
            .map(|row| row.label.as_str())
            .collect::<Vec<_>>(),
        vec!["r1"]
    );

    // BIGINT[] 数组绑定：`id = ANY($1)`。
    let labels = db
        .table(table!("phase_b_q.docs"))
        .where_any(field!("id"), vec![1_i64, 3])
        .order(field!("id"), SortOrder::Asc)
        .select::<LabelRow>()
        .await
        .expect("ANY 数组查询应成功");
    assert_eq!(
        labels
            .iter()
            .map(|row| row.label.as_str())
            .collect::<Vec<_>>(),
        vec!["r1", "r3"]
    );

    // TEXT[] 数组相等：整个数组作为单个绑定参数。
    let labels = db
        .table(table!("phase_b_q.docs"))
        .where_and(
            field!("tags"),
            CompareOp::Eq,
            SqlValue::from(vec!["a".to_string(), "b".to_string()]),
        )
        .expect("受控数组相等条件应合法")
        .select::<LabelRow>()
        .await
        .expect("数组相等查询应成功");
    assert_eq!(
        labels
            .iter()
            .map(|row| row.label.as_str())
            .collect::<Vec<_>>(),
        vec!["r1"]
    );

    // FieldType::Array 插入路径：JSON 数组递归转换为单个 TEXT[] 绑定参数。
    let inserted_id = db
        .table(table!("phase_b_q.docs"))
        .field_type(field!("data"), FieldType::Jsonb)
        .field_type(field!("tags"), FieldType::Array(Box::new(FieldType::Text)))
        .insert(&json!({
            "label": "r4",
            "data": {"role": "admin", "new": true},
            "tags": ["x", "y"],
        }))
        .await
        .expect("数组字段插入应成功");
    assert!(inserted_id > 0);
    let roundtrip: Vec<(String,)> = db
        .query_with_params(
            "SELECT array_to_string(tags, ',') FROM phase_b_q.docs WHERE id = $1",
            vec![json!(inserted_id as i64)],
        )
        .await
        .expect("复核数组字段应成功");
    assert_eq!(roundtrip[0].0, "x,y");

    let labels = db
        .table(table!("phase_b_q.docs"))
        .where_json_contains(field!("data"), SqlValue::Json(json!({"new": true})))
        .select::<LabelRow>()
        .await
        .expect("插入行的 JSON 包含查询应成功");
    assert_eq!(
        labels
            .iter()
            .map(|row| row.label.as_str())
            .collect::<Vec<_>>(),
        vec!["r4"]
    );

    cleanup(&db).await;
}

/// FieldType 的数组/网络类型映射直接用于建表，验证类型名与真实 PostgreSQL 一致。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_b_array_and_network_type_names_work_in_ddl() {
    let db = connect(&test_db_url()).await;
    prepare(&db).await;

    let ddl = format!(
        "CREATE TABLE phase_b_q.type_matrix (
            id BIGSERIAL PRIMARY KEY,
            tags {},
            ips {},
            mac {}
        )",
        FieldType::Array(Box::new(FieldType::Text))
            .postgres_type_name()
            .expect("TEXT[] 应有类型名"),
        FieldType::Array(Box::new(FieldType::Inet))
            .postgres_type_name()
            .expect("INET[] 应有类型名"),
        FieldType::MacAddr
            .postgres_type_name()
            .expect("MACADDR 应有类型名"),
    );
    db.execute_with_params(&ddl, vec![])
        .await
        .expect("按 FieldType 映射建表应成功");

    // 插入路径：数组/网络类型经 field_type 标记后绑定。
    let inserted = db
        .table(table!("phase_b_q.type_matrix"))
        .field_type(field!("tags"), FieldType::Array(Box::new(FieldType::Text)))
        .field_type(field!("ips"), FieldType::Array(Box::new(FieldType::Inet)))
        .field_type(field!("mac"), FieldType::MacAddr)
        .insert(&json!({
            "tags": ["a", "b"],
            "ips": ["10.0.0.1", "2001:db8::1"],
            "mac": "08:00:2b:01:02:03",
        }))
        .await;
    match inserted {
        Ok(id) => assert!(id > 0),
        Err(error) => panic!("数组/网络类型插入应成功，实得 {error:?}"),
    }

    cleanup(&db).await;
}
