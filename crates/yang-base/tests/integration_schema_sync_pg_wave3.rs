#![cfg(feature = "postgres")]
#![allow(clippy::expect_used, clippy::unwrap_used)]
//! PostgreSQL schema 同步第三波（P0/P1 补缺）真库集成测试。
//!
//! 覆盖部分索引/表达式索引反射、dry-run DDL 文本、以及同一轮新增列上
//! CHECK/外键的只读预检顺序。
//!
//! 所有测试标记 `#[ignore]`。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-base --no-default-features --features postgres \
//!   --test integration_schema_sync_pg_wave3 -- --ignored --test-threads=1
//! ```

use yang_base::database::{AnyDatabase, DatabaseInitializer, SchemaSyncChangeKind};
use yang_base::error::BaseError;
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

/// 进程级唯一表名：多个进程并行跑同一真库时不会互相 DROP/CREATE。
fn test_table(base: &str) -> String {
    format!("{base}_{}", std::process::id())
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

async fn index_names(database: &Database, table: &str) -> Vec<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT indexname::text FROM pg_indexes \
         WHERE schemaname = current_schema() AND tablename = $1 ORDER BY indexname",
    )
    .bind(table)
    .fetch_all(database.pool())
    .await
    .expect("查询索引列表应成功")
}

/// 部分唯一/普通索引与表达式索引被完整反射后，未知索引不会被删除，同步保持幂等。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave3_partial_and_expression_indexes_are_preserved_and_idempotent() {
    let database = connect().await;
    let table = test_table("wave3_sync_partial_expr");
    reset_tables(&database, &[table.as_str()]).await;
    let initializer = initializer(connect().await);
    let definition = Table::new(&table)
        .fields([
            Field::id("id"),
            Field::string("email", 128).required().unique(),
        ])
        .build()
        .expect("部分/表达式索引测试表定义应有效");

    initializer
        .sync_table_definitions(&[&definition])
        .await
        .expect("首次建表应成功");

    // 外部声明的部分唯一索引与表达式索引（索引名同样是 schema 级的，需进程唯一下标）。
    let partial_index = format!("idx_wave3_partial_active_{}", std::process::id());
    let expression_index = format!("idx_wave3_lower_email_{}", std::process::id());
    database
        .execute_with_params(
            &format!(
                "CREATE UNIQUE INDEX \"{partial_index}\" \
                 ON \"{table}\" (\"email\") WHERE \"email\" IS NOT NULL"
            ),
            vec![],
        )
        .await
        .expect("创建部分唯一索引应成功");
    database
        .execute_with_params(
            &format!(
                "CREATE INDEX \"{expression_index}\" \
                 ON \"{table}\" (lower(\"email\"))"
            ),
            vec![],
        )
        .await
        .expect("创建表达式索引应成功");

    let second = initializer
        .sync_table_definitions(&[&definition])
        .await
        .expect("反射部分/表达式索引后同步应成功");
    assert!(second.is_noop(), "未知索引不得触发任何变更: {second:?}");
    assert!(
        second.statements.is_empty() && second.deferred_statements.is_empty(),
        "no-op 不应携带待执行 DDL: {second:?}"
    );

    let names = index_names(&database, &table).await;
    assert!(
        names.iter().any(|name| name == &partial_index),
        "部分索引必须被保留: {names:?}"
    );
    assert!(
        names.iter().any(|name| name == &expression_index),
        "表达式索引必须被保留: {names:?}"
    );
    reset_tables(&database, &[table.as_str()]).await;
}

/// 同名部分索引与声明的完整唯一索引定义不同：必须 fail-closed，而不是重试
/// `CREATE INDEX` 撞上服务端 duplicate relation 错误。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave3_same_named_partial_index_is_detected_as_definition_drift() {
    let database = connect().await;
    let table = test_table("wave3_sync_partial_named");
    reset_tables(&database, &[table.as_str()]).await;
    let initializer = initializer(connect().await);
    let definition = Table::new(&table)
        .fields([
            Field::id("id"),
            Field::string("email", 128).required().unique(),
        ])
        .build()
        .expect("同名部分索引测试表定义应有效");
    let generated_name = format!("uk_{table}_email");

    initializer
        .sync_table_definitions(&[&definition])
        .await
        .expect("首次建表应成功");

    // 用同名部分唯一索引替换同步器创建的完整唯一索引。
    database
        .execute_with_params(&format!("DROP INDEX \"{generated_name}\""), vec![])
        .await
        .expect("删除完整唯一索引应成功");
    database
        .execute_with_params(
            &format!(
                "CREATE UNIQUE INDEX \"{generated_name}\" \
                 ON \"{table}\" (\"email\") WHERE \"email\" IS NOT NULL"
            ),
            vec![],
        )
        .await
        .expect("创建同名部分唯一索引应成功");

    let error = initializer
        .sync_table_definitions(&[&definition])
        .await
        .expect_err("同名部分索引必须被识别为定义漂移");
    assert!(
        matches!(&error, BaseError::DatabaseInitFailed(message) if message.contains("定义不同")),
        "应返回计划阶段的 fail-closed 错误，而不是服务端重名错误: {error}"
    );
    assert!(
        index_names(&database, &table)
            .await
            .iter()
            .any(|name| name == &generated_name),
        "失败的同步不得破坏既有部分索引"
    );
    reset_tables(&database, &[table.as_str()]).await;
}

/// dry-run 报告必须携带可审计的 DDL 文本，且不产生任何结构副作用。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave3_dry_run_reports_ddl_text_without_side_effects() {
    let database = connect().await;
    let table = test_table("wave3_sync_dry_run");
    reset_tables(&database, &[table.as_str()]).await;
    let initializer = initializer(connect().await);

    let created = Table::new(&table)
        .fields([
            Field::id("id"),
            Field::string("name", 32).required().unique(),
        ])
        .build()
        .expect("dry-run 测试表定义应有效");
    let report = initializer
        .plan_table_definitions(&[&created])
        .await
        .expect("只读计划应成功");
    assert!(
        report
            .statements
            .iter()
            .any(|statement| statement.starts_with(&format!("CREATE TABLE \"{table}\""))),
        "dry-run 应携带建表 DDL: {report:?}"
    );
    assert!(
        report
            .statements
            .iter()
            .any(|statement| statement.contains("CREATE UNIQUE INDEX")),
        "dry-run 应携带索引 DDL: {report:?}"
    );
    assert!(!table_exists(&database, &table).await, "dry-run 不得创建表");

    // 既有表上新增 CHECK：事务内 ADD ... NOT VALID，deferred VALIDATE。
    let base = Table::new(&table)
        .fields([Field::id("id"), Field::string("name", 32).required()])
        .build()
        .expect("基础表定义应有效");
    initializer
        .sync_table_definitions(&[&base])
        .await
        .expect("基础表建表应成功");

    let with_check = Table::new(&table)
        .fields([Field::id("id"), Field::string("name", 32).required()])
        .check_named("chk_wave3_name_not_empty", "name <> ''")
        .build()
        .expect("含 CHECK 的表定义应有效");
    let report = initializer
        .plan_table_definitions(&[&with_check])
        .await
        .expect("既有表只读计划应成功");
    assert!(
        report
            .statements
            .iter()
            .any(|statement| statement.ends_with("NOT VALID")),
        "dry-run 事务内语句应含 ADD ... NOT VALID: {report:?}"
    );
    assert!(
        report
            .deferred_statements
            .iter()
            .any(|statement| statement.contains("VALIDATE CONSTRAINT")),
        "dry-run 应单独携带 deferred VALIDATE 文本: {report:?}"
    );

    let constraint_count: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM pg_constraint \
         WHERE conrelid = '\"{table}\"'::regclass AND conname = 'chk_wave3_name_not_empty'"
    ))
    .fetch_one(database.pool())
    .await
    .expect("查询约束数量应成功");
    assert_eq!(constraint_count, 0, "dry-run 不得创建 CHECK 约束");
    reset_tables(&database, &[table.as_str()]).await;
}

/// 同一轮同步新增列 + 引用该列的 CHECK/外键：只读预检与首次同步都必须成功，
/// 二次同步 no-op。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave3_preflight_and_sync_allow_new_columns_with_constraints() {
    let database = connect().await;
    let parent_table = test_table("wave3_sync_parent");
    let child_table = test_table("wave3_sync_child");
    reset_tables(&database, &[child_table.as_str(), parent_table.as_str()]).await;
    let initializer = initializer(connect().await);

    let parent = Table::new(&parent_table)
        .fields([Field::id("id")])
        .build()
        .expect("父表定义应有效");
    let child = Table::new(&child_table)
        .fields([Field::id("id")])
        .build()
        .expect("子表定义应有效");
    initializer
        .sync_table_definitions(&[&parent, &child])
        .await
        .expect("基础表建表应成功");

    let extended: TableDefinition = Table::new(&child_table)
        .fields([
            Field::id("id"),
            Field::bigint("parent_id").required(),
            Field::integer("score").required(),
        ])
        .foreign_key_named(
            "fk_wave3_child_parent",
            ["parent_id"],
            parent_table.as_str(),
            ["id"],
        )
        .check_named("chk_wave3_score_positive", "score >= 0")
        .build()
        .expect("扩展表定义应有效");

    // 只读预检不得引用本轮尚未创建的列。
    let report = initializer
        .preflight_table_definitions(&[&parent, &extended])
        .await
        .expect("只读预检不应引用尚不存在的列");
    assert!(report.is_safe(), "无旧数据时预检应安全: {report:?}");
    assert!(
        report
            .plan
            .changes
            .iter()
            .any(|change| change.kind == SchemaSyncChangeKind::AddedColumn),
        "预检计划应包含新增列: {report:?}"
    );
    let column_exists: Option<String> = sqlx::query_scalar(
        "SELECT column_name::text FROM information_schema.columns \
         WHERE table_schema = current_schema() AND table_name = $1 \
           AND column_name = 'score'",
    )
    .bind(&child_table)
    .fetch_optional(database.pool())
    .await
    .expect("查询列是否存在应成功");
    assert!(column_exists.is_none(), "只读预检不得创建列");

    let first = initializer
        .sync_table_definitions(&[&parent, &extended])
        .await
        .expect("新增列 + CHECK/外键首次同步应成功");
    assert!(first
        .changes
        .iter()
        .any(|change| change.kind == SchemaSyncChangeKind::AddedCheck));
    assert!(first
        .changes
        .iter()
        .any(|change| change.kind == SchemaSyncChangeKind::AddedForeignKey));

    let second = initializer
        .sync_table_definitions(&[&parent, &extended])
        .await
        .expect("二次同步应成功");
    assert!(second.is_noop(), "新增列 + CHECK/外键必须幂等: {second:?}");
    reset_tables(&database, &[child_table.as_str(), parent_table.as_str()]).await;
}

/// 只读预检必须跳过引用任一新增列的 CHECK/外键/唯一索引，且不落库。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave3_mixed_column_constraint_preflight_does_not_reference_new_columns() {
    let database = connect().await;
    let parent_table = test_table("wave3_mixed_parent");
    let child_table = test_table("wave3_mixed_child");
    reset_tables(&database, &[child_table.as_str(), parent_table.as_str()]).await;
    let initializer = initializer(connect().await);

    let parent = Table::new(&parent_table)
        .fields([Field::id("id"), Field::integer("code").required()])
        .unique_named(
            format!("uk_wave3_mixed_parent_id_code_{}", std::process::id()),
            ["id", "code"],
        )
        .build()
        .expect("父表定义应有效");
    let child = Table::new(&child_table)
        .fields([
            Field::id("id"),
            Field::string("status", 32).required(),
            Field::integer("parent_id").required(),
        ])
        .build()
        .expect("子表定义应有效");
    initializer
        .sync_table_definitions(&[&parent, &child])
        .await
        .expect("基础表建表应成功");

    // score/code 为本轮新增列，CHECK 同时引用既有 status、外键同时引用既有
    // parent_id；只读预检若生成整条 SQL 会因引用尚不存在的列而 42703。
    let extended: TableDefinition = Table::new(&child_table)
        .fields([
            Field::id("id"),
            Field::string("status", 32).required(),
            Field::integer("score").required(),
            Field::integer("parent_id").required(),
            Field::integer("code").required(),
        ])
        .unique_named(
            format!("uk_wave3_mixed_child_status_score_{}", std::process::id()),
            ["status", "score"],
        )
        .check_named("chk_wave3_mixed_and", "score >= 0 AND status <> 'banned'")
        .foreign_key_named(
            "fk_wave3_mixed_parent",
            ["parent_id", "code"],
            parent_table.as_str(),
            ["id", "code"],
        )
        .build()
        .expect("扩展表定义应有效");

    let report = initializer
        .preflight_table_definitions(&[&parent, &extended])
        .await
        .expect("只读预检不得引用本轮尚未创建的列");
    assert!(report.is_safe(), "无旧数据时预检应安全: {report:?}");
    let persisted: Option<String> = sqlx::query_scalar(
        "SELECT column_name::text FROM information_schema.columns \
         WHERE table_schema = current_schema() AND table_name = $1 AND column_name = 'score'",
    )
    .bind(&child_table)
    .fetch_optional(database.pool())
    .await
    .expect("查询列是否存在应成功");
    assert!(persisted.is_none(), "只读预检不得创建新增列");
    reset_tables(&database, &[child_table.as_str(), parent_table.as_str()]).await;
}

/// 同一批中父表新增被引用列、子表声明复合外键：只读预检不得引用尚未创建的
/// 被引用列（42703），首次同步成功，二/三次同步 no-op。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave3_cross_table_fk_preflight_skips_referenced_new_column() {
    let database = connect().await;
    let parent_table = test_table("wave3_cross_fk_parent");
    let child_table = test_table("wave3_cross_fk_child");
    reset_tables(&database, &[child_table.as_str(), parent_table.as_str()]).await;
    let initializer = initializer(connect().await);

    let parent = Table::new(&parent_table)
        .fields([Field::id("id")])
        .build()
        .expect("父表定义应有效");
    let child = Table::new(&child_table)
        .fields([
            Field::id("id"),
            Field::bigint("parent_id").required(),
            Field::integer("parent_code").required(),
        ])
        .build()
        .expect("子表定义应有效");
    initializer
        .sync_table_definitions(&[&parent, &child])
        .await
        .expect("基础表建表应成功");

    let parent_extended = Table::new(&parent_table)
        .fields([Field::id("id"), Field::integer("code").required()])
        .unique_named(
            format!("uk_wave3_cross_fk_parent_id_code_{}", std::process::id()),
            ["id", "code"],
        )
        .build()
        .expect("扩展父表定义应有效");
    let child_extended = Table::new(&child_table)
        .fields([
            Field::id("id"),
            Field::bigint("parent_id").required(),
            Field::integer("parent_code").required(),
        ])
        .foreign_key_named(
            "fk_wave3_cross_fk",
            ["parent_id", "parent_code"],
            parent_table.as_str(),
            ["id", "code"],
        )
        .build()
        .expect("扩展子表定义应有效");

    // 本地列都既有，但被引用的 code 是本批新增列：只读预检必须跳过外键项。
    let preflight = initializer
        .preflight_table_definitions(&[&parent_extended, &child_extended])
        .await
        .expect("跨表被引用列新增时只读预检不得报 42703");
    assert!(preflight.is_safe(), "无旧数据时预检应安全: {preflight:?}");

    let first = initializer
        .sync_table_definitions(&[&parent_extended, &child_extended])
        .await
        .expect("跨表复合外键首次同步应成功");
    assert!(
        first
            .changes
            .iter()
            .any(|change| change.kind == SchemaSyncChangeKind::AddedForeignKey),
        "首次同步应新增跨表外键: {first:?}"
    );
    for attempt in 2..=3 {
        let report = initializer
            .sync_table_definitions(&[&parent_extended, &child_extended])
            .await
            .unwrap_or_else(|error| panic!("第 {attempt} 次同步应成功: {error}"));
        assert!(
            report.is_noop(),
            "第 {attempt} 次同步必须 no-op: {report:?}"
        );
    }
    reset_tables(&database, &[child_table.as_str(), parent_table.as_str()]).await;
}

/// 未加引号的大写列名按 PostgreSQL 折叠规则命中新增列：只读预检不报 42703，
/// 首次同步成功，二次同步 no-op。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave3_unquoted_uppercase_check_column_is_case_folded() {
    let database = connect().await;
    let table = test_table("wave3_upper_check");
    reset_tables(&database, &[table.as_str()]).await;
    let initializer = initializer(connect().await);

    let base = Table::new(&table)
        .fields([Field::id("id"), Field::string("status", 32).required()])
        .build()
        .expect("基础表定义应有效");
    initializer
        .sync_table_definitions(&[&base])
        .await
        .expect("基础表建表应成功");

    let extended = Table::new(&table)
        .fields([
            Field::id("id"),
            Field::string("status", 32).required(),
            Field::integer("score").required(),
        ])
        .check_named("chk_wave3_upper_score", "SCORE >= 0 AND status <> 'banned'")
        .build()
        .expect("大写列名 CHECK 表定义应有效");

    let preflight = initializer
        .preflight_table_definitions(&[&extended])
        .await
        .expect("未加引号大写列名不应触发 42703");
    assert!(preflight.is_safe(), "无旧数据时预检应安全: {preflight:?}");

    let first = initializer
        .sync_table_definitions(&[&extended])
        .await
        .expect("未加引号大写列名 CHECK 首次同步应成功");
    assert!(
        first
            .changes
            .iter()
            .any(|change| change.kind == SchemaSyncChangeKind::AddedCheck),
        "首次同步应新增 CHECK: {first:?}"
    );

    let second = initializer
        .sync_table_definitions(&[&extended])
        .await
        .expect("二次同步应成功");
    assert!(second.is_noop(), "大写列名 CHECK 必须幂等: {second:?}");
    reset_tables(&database, &[table.as_str()]).await;
}

/// 比较运算符右侧的 AND 分组括号具有语义：`flag = (x AND y)` 与
/// `(flag = x) AND y` 必须被识别为不同表达式，而不是静默 no-op。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave3_check_parentheses_right_of_operator_are_not_silently_folded() {
    let database = connect().await;
    let table = test_table("wave3_bool_group_check");
    reset_tables(&database, &[table.as_str()]).await;
    let initializer = initializer(connect().await);

    let fields = || {
        [
            Field::id("id"),
            Field::boolean("flag").required(),
            Field::boolean("x").required(),
            Field::boolean("y").required(),
        ]
    };
    let base = Table::new(&table)
        .fields(fields())
        .build()
        .expect("基础表定义应有效");
    initializer
        .sync_table_definitions(&[&base])
        .await
        .expect("基础表建表应成功");

    let grouped_right = Table::new(&table)
        .fields(fields())
        .check_named("chk_wave3_bool_group", "flag = (x AND y)")
        .build()
        .expect("布尔分组 CHECK 定义应有效");
    initializer
        .sync_table_definitions(&[&grouped_right])
        .await
        .expect("首次同步布尔分组 CHECK 应成功");
    let repeat = initializer
        .sync_table_definitions(&[&grouped_right])
        .await
        .expect("相同定义重复同步应成功");
    assert!(repeat.is_noop(), "相同布尔分组 CHECK 必须幂等: {repeat:?}");

    let grouped_left = Table::new(&table)
        .fields(fields())
        .check_named("chk_wave3_bool_group", "(flag = x) AND y")
        .build()
        .expect("语义不同的布尔分组 CHECK 定义应有效");
    let error = initializer
        .plan_table_definitions(&[&grouped_left])
        .await
        .expect_err("语义不同的表达式必须报定义漂移，而不是静默 no-op");
    assert!(
        matches!(&error, BaseError::DatabaseInitFailed(message) if message.contains("表达式不同")),
        "应返回表达式不同错误: {error}"
    );
    reset_tables(&database, &[table.as_str()]).await;
}

/// 混合既有/新增列的多子句 AND、OR CHECK：首次同步成功，后续同步 no-op。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave3_multi_clause_checks_are_idempotent() {
    let database = connect().await;
    let table = test_table("wave3_multi_check");
    reset_tables(&database, &[table.as_str()]).await;
    let initializer = initializer(connect().await);

    let base = Table::new(&table)
        .fields([Field::id("id"), Field::string("status", 32).required()])
        .build()
        .expect("基础表定义应有效");
    initializer
        .sync_table_definitions(&[&base])
        .await
        .expect("基础表建表应成功");

    let extended = Table::new(&table)
        .fields([
            Field::id("id"),
            Field::string("status", 32).required(),
            Field::integer("score").required(),
        ])
        .check_named("chk_wave3_multi_and", "score >= 0 AND status <> 'banned'")
        .check_named("chk_wave3_multi_or", "score >= 0 OR status <> 'banned'")
        .build()
        .expect("多子句 CHECK 表定义应有效");

    for attempt in 1..=3 {
        let report = initializer
            .sync_table_definitions(&[&extended])
            .await
            .unwrap_or_else(|error| panic!("第 {attempt} 次同步应成功: {error}"));
        if attempt == 1 {
            assert!(
                report
                    .changes
                    .iter()
                    .any(|change| change.kind == SchemaSyncChangeKind::AddedCheck),
                "首次同步应新增 CHECK: {report:?}"
            );
        } else {
            assert!(
                report.is_noop(),
                "第 {attempt} 次同步必须 no-op（多子句 CHECK 归一化必须幂等）: {report:?}"
            );
        }
    }
    reset_tables(&database, &[table.as_str()]).await;
}
