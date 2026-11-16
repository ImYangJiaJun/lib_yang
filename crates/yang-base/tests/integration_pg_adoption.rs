#![allow(clippy::expect_used, clippy::unwrap_used)]
#![cfg(feature = "postgres")]
//! PostgreSQL 原生枚举的建表、补值与幂等同步集成测试。
//!
//! 运行：
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-base --no-default-features --features postgres \
//!   --test integration_pg_adoption -- --ignored --test-threads=1
//! ```

use yang_base::database::DatabaseInitializer;
use yang_base::table::{Field, Table, TableDefinition};
use yang_db::postgres::{Database as PgDatabase, DatabaseConfig as PgConfig};

fn pg_url() -> String {
    std::env::var("PG_TEST_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432/test".to_string())
}

fn definition(values: &[&str]) -> TableDefinition {
    Table::new("native_enum_probe")
        .fields([
            Field::id("id"),
            Field::enumeration("status", values.iter().copied())
                .required()
                .native_enum("native_enum_probe_status"),
        ])
        .build()
        .expect("原生枚举表定义应有效")
}

async fn labels(pg: &PgDatabase) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT e.enumlabel FROM pg_enum e \
         JOIN pg_type t ON t.oid = e.enumtypid \
         WHERE t.typname = 'native_enum_probe_status' \
         ORDER BY e.enumsortorder",
    )
    .fetch_all(pg.pool())
    .await
    .expect("读取枚举标签失败")
}

#[tokio::test]
#[ignore = "需要本地 PostgreSQL 容器"]
async fn native_enum_roundtrip_extends_values_and_is_idempotent() {
    let pg = PgDatabase::connect_with_config(&pg_url(), PgConfig::default())
        .await
        .expect("连接 PG 失败");
    sqlx::query("DROP TABLE IF EXISTS native_enum_probe")
        .execute(pg.pool())
        .await
        .expect("清理表失败");
    sqlx::query("DROP TYPE IF EXISTS native_enum_probe_status")
        .execute(pg.pool())
        .await
        .expect("清理类型失败");

    let first = DatabaseInitializer::new_postgres(pg.clone())
        .sync_table_definitions(&[&definition(&["pending", "paid"])])
        .await
        .expect("首次同步应成功");
    assert!(!first.is_noop(), "首次同步应创建类型与表");
    assert_eq!(labels(&pg).await, vec!["pending", "paid"]);

    let udt: String = sqlx::query_scalar(
        "SELECT udt_name FROM information_schema.columns \
         WHERE table_name = 'native_enum_probe' AND column_name = 'status'",
    )
    .fetch_one(pg.pool())
    .await
    .expect("读取列类型失败");
    assert_eq!(udt, "native_enum_probe_status");

    // 建表阶段的默认/写入路径按类型约束生效。
    sqlx::query("INSERT INTO native_enum_probe (status) VALUES ('paid')")
        .execute(pg.pool())
        .await
        .expect("合法枚举值应可写入");
    assert!(
        sqlx::query("INSERT INTO native_enum_probe (status) VALUES ('unknown')")
            .execute(pg.pool())
            .await
            .is_err(),
        "原生枚举必须拒绝未声明取值"
    );

    let extended = DatabaseInitializer::new_postgres(pg.clone())
        .sync_table_definitions(&[&definition(&["pending", "paid", "refunded"])])
        .await
        .expect("追加枚举取值应成功");
    assert!(!extended.is_noop(), "追加枚举取值必须产生变更");
    assert_eq!(labels(&pg).await, vec!["pending", "paid", "refunded"]);

    let idempotent = DatabaseInitializer::new_postgres(pg.clone())
        .sync_table_definitions(&[&definition(&["pending", "paid", "refunded"])])
        .await
        .expect("重复同步应成功");
    assert!(idempotent.is_noop(), "重复同步不得产生任何 DDL");

    sqlx::query("DROP TABLE IF EXISTS native_enum_probe")
        .execute(pg.pool())
        .await
        .expect("清理表失败");
    sqlx::query("DROP TYPE IF EXISTS native_enum_probe_status")
        .execute(pg.pool())
        .await
        .expect("清理类型失败");
}

/// 存量 PG 表已有的部分索引/表达式索引必须被识别为已满足声明，重复同步不得漂移。
#[tokio::test]
#[ignore = "需要本地 PostgreSQL 容器"]
async fn partial_and_expression_indexes_are_adopted_without_drift() {
    let pg = PgDatabase::connect_with_config(&pg_url(), PgConfig::default())
        .await
        .expect("连接 PG 失败");
    sqlx::query("DROP TABLE IF EXISTS index_probe")
        .execute(pg.pool())
        .await
        .expect("清理表失败");
    sqlx::query("CREATE TABLE index_probe (id BIGINT PRIMARY KEY, email TEXT, deleted_at BIGINT)")
        .execute(pg.pool())
        .await
        .expect("创建存量表失败");
    sqlx::query("CREATE INDEX index_probe_lower ON index_probe (lower(email))")
        .execute(pg.pool())
        .await
        .expect("创建表达式索引失败");
    sqlx::query(
        "CREATE INDEX index_probe_active ON index_probe (deleted_at) WHERE deleted_at IS NULL",
    )
    .execute(pg.pool())
    .await
    .expect("创建部分索引失败");

    let definition = Table::new("index_probe")
        .fields([
            Field::bigint("id").required().primary_key(),
            Field::text("email"),
            Field::bigint("deleted_at"),
        ])
        .index_spec(
            Some("index_probe_lower".to_string()),
            [] as [&str; 0],
            ["lower(email)"],
            None,
            false,
            false,
        )
        .index_spec(
            Some("index_probe_active".to_string()),
            ["deleted_at"],
            [] as [&str; 0],
            Some("deleted_at IS NULL".to_string()),
            false,
            false,
        )
        .build()
        .expect("存量索引表定义应有效");

    let report = DatabaseInitializer::new_postgres(pg.clone())
        .sync_table_definitions(&[&definition])
        .await
        .expect("存量部分/表达式索引应可 adopt");
    assert!(
        report.is_noop(),
        "既有索引已满足声明时不得产生 DDL: {:?}",
        report.changes
    );

    sqlx::query("DROP TABLE IF EXISTS index_probe")
        .execute(pg.pool())
        .await
        .expect("清理表失败");
}

/// 既有表新增原生枚举列：类型创建必须先于 ADD COLUMN 执行。
#[tokio::test]
#[ignore = "需要本地 PostgreSQL 容器"]
async fn native_enum_type_is_created_before_added_column_on_existing_table() {
    let pg = PgDatabase::connect_with_config(&pg_url(), PgConfig::default())
        .await
        .expect("连接 PG 失败");
    sqlx::query("DROP TABLE IF EXISTS native_enum_add_probe")
        .execute(pg.pool())
        .await
        .expect("清理表失败");
    sqlx::query("DROP TYPE IF EXISTS native_enum_add_probe_status")
        .execute(pg.pool())
        .await
        .expect("清理类型失败");
    sqlx::query("CREATE TABLE native_enum_add_probe (id BIGINT PRIMARY KEY)")
        .execute(pg.pool())
        .await
        .expect("创建存量表失败");

    let definition = Table::new("native_enum_add_probe")
        .fields([
            Field::bigint("id").required().primary_key(),
            Field::enumeration("status", ["pending", "paid"])
                .native_enum("native_enum_add_probe_status"),
        ])
        .build()
        .expect("新增枚举列定义应有效");
    DatabaseInitializer::new_postgres(pg.clone())
        .sync_table_definitions(&[&definition])
        .await
        .expect("既有表新增原生枚举列应成功");

    let udt: String = sqlx::query_scalar(
        "SELECT udt_name FROM information_schema.columns \
         WHERE table_name = 'native_enum_add_probe' AND column_name = 'status'",
    )
    .fetch_one(pg.pool())
    .await
    .expect("读取列类型失败");
    assert_eq!(udt, "native_enum_add_probe_status");

    sqlx::query("DROP TABLE IF EXISTS native_enum_add_probe")
        .execute(pg.pool())
        .await
        .expect("清理表失败");
    sqlx::query("DROP TYPE IF EXISTS native_enum_add_probe_status")
        .execute(pg.pool())
        .await
        .expect("清理类型失败");
}

/// 多张表共享同一原生枚举类型时，同批同步只能创建一次类型。
#[tokio::test]
#[ignore = "需要本地 PostgreSQL 容器"]
async fn shared_native_enum_is_created_once_for_multiple_tables() {
    let pg = PgDatabase::connect_with_config(&pg_url(), PgConfig::default())
        .await
        .expect("连接 PG 失败");
    for table in ["shared_enum_probe_a", "shared_enum_probe_b"] {
        sqlx::query(&format!("DROP TABLE IF EXISTS {table}"))
            .execute(pg.pool())
            .await
            .expect("清理表失败");
    }
    sqlx::query("DROP TYPE IF EXISTS shared_enum_probe_status")
        .execute(pg.pool())
        .await
        .expect("清理类型失败");

    let definitions = ["shared_enum_probe_a", "shared_enum_probe_b"]
        .into_iter()
        .map(|table| {
            Table::new(table)
                .fields([
                    Field::id("id"),
                    Field::enumeration("status", ["pending", "paid"])
                        .required()
                        .native_enum("shared_enum_probe_status"),
                ])
                .build()
                .expect("共享枚举表定义应有效")
        })
        .collect::<Vec<_>>();
    let references = definitions.iter().collect::<Vec<_>>();
    let report = DatabaseInitializer::new_postgres(pg.clone())
        .sync_table_definitions(&references)
        .await
        .expect("共享枚举多表同步应成功");
    assert!(
        report
            .statements
            .iter()
            .filter(|sql| sql.contains("CREATE TYPE \"shared_enum_probe_status\""))
            .count()
            == 1,
        "共享枚举类型只能创建一次: {:?}",
        report.statements
    );
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT table_name FROM information_schema.columns \
         WHERE column_name = 'status' AND udt_name = 'shared_enum_probe_status' \
         ORDER BY table_name",
    )
    .fetch_all(pg.pool())
    .await
    .expect("读取共享枚举列失败");
    assert_eq!(
        tables,
        vec![
            "shared_enum_probe_a".to_string(),
            "shared_enum_probe_b".to_string()
        ]
    );

    for table in ["shared_enum_probe_a", "shared_enum_probe_b"] {
        sqlx::query(&format!("DROP TABLE IF EXISTS {table}"))
            .execute(pg.pool())
            .await
            .expect("清理表失败");
    }
    sqlx::query("DROP TYPE IF EXISTS shared_enum_probe_status")
        .execute(pg.pool())
        .await
        .expect("清理类型失败");
}

/// DDL 在事务提交后失败时，所有权绑定仍必须先于 DDL 落库，避免下次误判 adopted。
#[tokio::test]
#[ignore = "需要本地 PostgreSQL 容器"]
async fn binding_is_persisted_even_when_a_later_ddl_fails() {
    let pg = PgDatabase::connect_with_config(&pg_url(), PgConfig::default())
        .await
        .expect("连接 PG 失败");
    sqlx::query("DROP TABLE IF EXISTS binding_failure_probe")
        .execute(pg.pool())
        .await
        .expect("清理表失败");
    sqlx::query("CREATE TABLE binding_failure_probe (id BIGINT PRIMARY KEY, name TEXT)")
        .execute(pg.pool())
        .await
        .expect("创建存量表失败");
    sqlx::query("INSERT INTO binding_failure_probe (id, name) VALUES (1, 'dup'), (2, 'dup')")
        .execute(pg.pool())
        .await
        .expect("创建重复数据失败");
    sqlx::query("DELETE FROM \"_yang_schema_binding\" WHERE table_name = 'binding_failure_probe'")
        .execute(pg.pool())
        .await
        .ok();

    let definition = Table::new("binding_failure_probe")
        .fields([
            Field::bigint("id").required().primary_key(),
            Field::text("name"),
        ])
        .unique(["name"])
        .build()
        .expect("失败场景定义应有效");
    assert!(
        DatabaseInitializer::new_postgres(pg.clone())
            .sync_table_definitions(&[&definition])
            .await
            .is_err(),
        "重复值上的唯一索引必须失败"
    );

    let mode: String = sqlx::query_scalar(
        "SELECT mode FROM \"_yang_schema_binding\" \
         WHERE datasource = 'default' AND table_name = 'binding_failure_probe'",
    )
    .fetch_one(pg.pool())
    .await
    .expect("绑定必须在 DDL 前落库");
    assert_eq!(mode, "managed");

    sqlx::query("DROP TABLE IF EXISTS binding_failure_probe")
        .execute(pg.pool())
        .await
        .expect("清理表失败");
    sqlx::query("DELETE FROM \"_yang_schema_binding\" WHERE table_name = 'binding_failure_probe'")
        .execute(pg.pool())
        .await
        .expect("清理绑定失败");
}

/// 存量自定义类型列（inet）声明为 opaque 后应被完整保留且不参与通用 CRUD。
#[tokio::test]
#[ignore = "需要本地 PostgreSQL 容器"]
async fn opaque_columns_are_adopted_without_being_modified() {
    let pg = PgDatabase::connect_with_config(&pg_url(), PgConfig::default())
        .await
        .expect("连接 PG 失败");
    sqlx::query("DROP TABLE IF EXISTS opaque_probe")
        .execute(pg.pool())
        .await
        .expect("清理表失败");
    sqlx::query("CREATE TABLE opaque_probe (id BIGINT PRIMARY KEY, addr INET)")
        .execute(pg.pool())
        .await
        .expect("创建存量表失败");

    let definition = Table::new("opaque_probe")
        .fields([Field::bigint("id").required().primary_key()])
        .opaque_column("addr", "inet")
        .build()
        .expect("opaque 定义应有效");
    let report = DatabaseInitializer::new_postgres(pg.clone())
        .sync_table_definitions(&[&definition])
        .await
        .expect("opaque 列应可 adopt");
    assert!(
        report.is_noop(),
        "opaque 列不得产生 DDL: {:?}",
        report.changes
    );

    let udt: String = sqlx::query_scalar(
        "SELECT udt_name FROM information_schema.columns \
         WHERE table_name = 'opaque_probe' AND column_name = 'addr'",
    )
    .fetch_one(pg.pool())
    .await
    .expect("读取列类型失败");
    assert_eq!(udt, "inet");

    sqlx::query("DROP TABLE IF EXISTS opaque_probe")
        .execute(pg.pool())
        .await
        .expect("清理表失败");
}

/// 接入报告应识别原生枚举、opaque 列、部分/表达式索引与 identity。
#[tokio::test]
#[ignore = "需要本地 PostgreSQL 容器"]
async fn adoption_report_describes_existing_schema() {
    let pg = PgDatabase::connect_with_config(&pg_url(), PgConfig::default())
        .await
        .expect("连接 PG 失败");
    sqlx::query("DROP TABLE IF EXISTS report_probe")
        .execute(pg.pool())
        .await
        .expect("清理表失败");
    sqlx::query("DROP TYPE IF EXISTS report_probe_status")
        .execute(pg.pool())
        .await
        .expect("清理类型失败");
    sqlx::query("CREATE TYPE report_probe_status AS ENUM ('pending', 'paid')")
        .execute(pg.pool())
        .await
        .expect("创建枚举失败");
    sqlx::query(
        "CREATE TABLE report_probe ( \
         id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY, \
         status report_probe_status NOT NULL, addr INET, email TEXT, deleted_at BIGINT)",
    )
    .execute(pg.pool())
    .await
    .expect("创建报告表失败");
    sqlx::query("CREATE INDEX report_probe_lower ON report_probe (lower(email))")
        .execute(pg.pool())
        .await
        .expect("创建表达式索引失败");
    sqlx::query(
        "CREATE INDEX report_probe_active ON report_probe (deleted_at) WHERE deleted_at IS NULL",
    )
    .execute(pg.pool())
    .await
    .expect("创建部分索引失败");

    let report = yang_base::database::describe_postgres_schema(&pg, Some("public"))
        .await
        .expect("读取接入报告失败");
    let table = report
        .tables
        .iter()
        .find(|table| table.name == "report_probe")
        .expect("报告应包含 report_probe");
    let status = table
        .columns
        .iter()
        .find(|column| column.name == "status")
        .expect("报告应包含 status 列");
    assert_eq!(status.enum_labels, vec!["pending", "paid"]);
    assert!(
        status
            .suggested_field()
            .is_some_and(|field| field.contains("native_enum(\"report_probe_status\")")),
        "枚举列应建议 native_enum"
    );
    let addr = table
        .columns
        .iter()
        .find(|column| column.name == "addr")
        .expect("报告应包含 addr 列");
    assert!(
        addr.suggested_field().is_none(),
        "inet 列应建议 opaque 而非普通字段"
    );
    let id = table
        .columns
        .iter()
        .find(|column| column.name == "id")
        .expect("报告应包含 id 列");
    assert!(id.auto_increment, "identity 列必须被识别");
    assert!(
        table
            .indexes
            .iter()
            .any(|index| index.name == "report_probe_lower" && !index.expressions.is_empty()),
        "表达式索引必须出现在报告中"
    );
    assert!(
        table
            .indexes
            .iter()
            .any(|index| index.name == "report_probe_active"
                && index
                    .predicate
                    .as_deref()
                    .is_some_and(|predicate| predicate.contains("IS NULL"))),
        "部分索引谓词必须出现在报告中"
    );
    let markdown = report.to_markdown();
    assert!(markdown.contains("report_probe_lower"));
    assert!(markdown.contains("opaque_column"));

    sqlx::query("DROP TABLE IF EXISTS report_probe")
        .execute(pg.pool())
        .await
        .expect("清理表失败");
    sqlx::query("DROP TYPE IF EXISTS report_probe_status")
        .execute(pg.pool())
        .await
        .expect("清理类型失败");
}

/// 存量表首次绑定为 adopted；框架实际演进后升级为 managed。
#[tokio::test]
#[ignore = "需要本地 PostgreSQL 容器"]
async fn schema_binding_tracks_adopted_then_managed() {
    let pg = PgDatabase::connect_with_config(&pg_url(), PgConfig::default())
        .await
        .expect("连接 PG 失败");
    sqlx::query("DROP TABLE IF EXISTS binding_probe")
        .execute(pg.pool())
        .await
        .expect("清理表失败");
    sqlx::query("DROP TABLE IF EXISTS \"_yang_schema_binding\"")
        .execute(pg.pool())
        .await
        .expect("清理绑定表失败");
    sqlx::query("CREATE TABLE binding_probe (id BIGINT PRIMARY KEY)")
        .execute(pg.pool())
        .await
        .expect("创建存量表失败");

    let v1 = Table::new("binding_probe")
        .fields([Field::bigint("id").required().primary_key()])
        .build()
        .expect("v1 定义应有效");
    let first = DatabaseInitializer::new_postgres(pg.clone())
        .sync_table_definitions(&[&v1])
        .await
        .expect("首次接管应成功");
    assert_eq!(first.bindings.len(), 1);
    assert!(first.bindings[0].newly_bound);
    assert_eq!(
        first.bindings[0].mode,
        yang_base::database::SchemaBindingMode::Adopted
    );

    let v2 = Table::new("binding_probe")
        .fields([
            Field::bigint("id").required().primary_key(),
            Field::string("note", 32),
        ])
        .build()
        .expect("v2 定义应有效");
    let second = DatabaseInitializer::new_postgres(pg.clone())
        .sync_table_definitions(&[&v2])
        .await
        .expect("演进接管表应成功");
    assert_eq!(
        second.bindings[0].mode,
        yang_base::database::SchemaBindingMode::Managed
    );

    let mode: String = sqlx::query_scalar(
        "SELECT mode FROM \"_yang_schema_binding\" \
         WHERE datasource = 'default' AND table_name = 'binding_probe'",
    )
    .fetch_one(pg.pool())
    .await
    .expect("读取绑定记录失败");
    assert_eq!(mode, "managed");

    sqlx::query("DROP TABLE IF EXISTS binding_probe")
        .execute(pg.pool())
        .await
        .expect("清理表失败");
}
