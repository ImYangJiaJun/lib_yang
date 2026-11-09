#![cfg(feature = "postgres")]
#![allow(clippy::expect_used, clippy::unwrap_used)]
//! PostgreSQL schema 同步第一波（P0/P1）真库集成测试。
//!
//! 覆盖 enum → varchar + CHECK 幂等、identity 反射、COMMENT/生成列、63 字节
//! 标识符规则与在线 DDL（CONCURRENTLY / NOT VALID + VALIDATE）。
//!
//! 所有测试标记 `#[ignore]`。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-base --no-default-features --features postgres \
//!   --test integration_schema_sync_pg_wave1 -- --ignored --test-threads=1
//! ```

use yang_base::database::{AnyDatabase, DatabaseInitializer, SchemaSyncChangeKind};
use yang_base::error::BaseError;
use yang_base::table::{Field, Table};
use yang_db::postgres::{Database, DatabaseConfig};

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

async fn query_scalar<T>(database: &Database, sql: &str, bind: &str) -> T
where
    T: for<'r> sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres> + Send + Unpin,
{
    sqlx::query_scalar::<_, T>(sql)
        .bind(bind)
        .fetch_one(database.pool())
        .await
        .expect("查询应成功")
}

// ---- P0-02：enum → varchar + CHECK ----

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave1_schema_sync_enum_varchar_check_is_idempotent() {
    let database = connect().await;
    reset_tables(&database, &["wave1_sync_enum"]).await;
    let initializer = initializer(connect().await);
    let definition = Table::new("wave1_sync_enum")
        .fields([
            Field::id("id"),
            Field::enumeration("status", ["active", "disabled"]).required(),
        ])
        .build()
        .expect("枚举表定义应有效");

    let created = initializer
        .sync_table_definitions(&[&definition])
        .await
        .expect("枚举表首次同步应成功");
    assert_eq!(created.changes.len(), 1);
    assert_eq!(created.changes[0].kind, SchemaSyncChangeKind::CreatedTable);

    // 列降级为 varchar(8)，不是原生 ENUM。
    let data_type: String = query_scalar(
        &database,
        "SELECT data_type FROM information_schema.columns \
         WHERE table_schema = current_schema() AND table_name = $1 AND column_name = 'status'",
        "wave1_sync_enum",
    )
    .await;
    assert_eq!(data_type, "character varying");
    let max_length: Option<i32> = sqlx::query_scalar(
        "SELECT character_maximum_length::int FROM information_schema.columns \
         WHERE table_schema = current_schema() AND table_name = $1 AND column_name = 'status'",
    )
    .bind("wave1_sync_enum")
    .fetch_one(database.pool())
    .await
    .expect("查询列长度应成功");
    assert_eq!(max_length, Some(8));

    // CHECK 约束实际生效。
    let check_definition: String = query_scalar(
        &database,
        "SELECT pg_get_constraintdef(oid) FROM pg_constraint \
         WHERE conrelid = 'wave1_sync_enum'::regclass AND conname = 'chk_wave1_sync_enum_status'",
        "wave1_sync_enum",
    )
    .await;
    assert!(check_definition.contains("active"));
    assert!(check_definition.contains("disabled"));
    let error = sqlx::query("INSERT INTO wave1_sync_enum (status) VALUES ('bogus')")
        .execute(database.pool())
        .await
        .expect_err("枚举 CHECK 必须拒绝非法取值");
    let code = error
        .as_database_error()
        .and_then(|error| error.code())
        .map(|code| code.into_owned());
    assert_eq!(code.as_deref(), Some("23514"));

    let second = initializer
        .sync_table_definitions(&[&definition])
        .await
        .expect("枚举表二次同步应成功");
    assert!(second.is_noop(), "枚举 CHECK 必须幂等: {second:?}");

    reset_tables(&database, &["wave1_sync_enum"]).await;
}

// ---- P0-09：identity 来自 pg_catalog ----

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave1_schema_sync_identity_is_reflected_from_catalog() {
    let database = connect().await;
    reset_tables(&database, &["wave1_sync_identity"]).await;
    let initializer = initializer(connect().await);
    let definition = Table::new("wave1_sync_identity")
        .fields([Field::id("id"), Field::string("name", 32)])
        .build()
        .expect("identity 表定义应有效");

    initializer
        .sync_table_definitions(&[&definition])
        .await
        .expect("identity 表同步应成功");

    let identity: String = query_scalar(
        &database,
        "SELECT attidentity::text FROM pg_attribute \
         WHERE attrelid = 'wave1_sync_identity'::regclass AND attname = 'id'",
        "wave1_sync_identity",
    )
    .await;
    assert_eq!(identity, "d", "id 应为 GENERATED BY DEFAULT AS IDENTITY");

    let second = initializer
        .sync_table_definitions(&[&definition])
        .await
        .expect("identity 表二次同步应成功");
    assert!(
        second.is_noop(),
        "identity 必须从 catalog 读取，不应产生伪漂移: {second:?}"
    );

    reset_tables(&database, &["wave1_sync_identity"]).await;
}

// ---- P1-06：COMMENT ON COLUMN + 生成列 ----

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave1_schema_sync_comments_and_generated_columns() {
    let database = connect().await;
    reset_tables(&database, &["wave1_sync_generated"]).await;
    let initializer = initializer(connect().await);

    let base = Table::new("wave1_sync_generated")
        .fields([Field::id("id"), Field::integer("price").required()])
        .build()
        .expect("基础表定义应有效");
    initializer
        .sync_table_definitions(&[&base])
        .await
        .expect("基础表同步应成功");

    let extended = Table::new("wave1_sync_generated")
        .fields([
            Field::id("id"),
            Field::integer("price").required(),
            Field::bigint("total").generated("price * 2"),
            Field::string("nickname", 32).comment("昵称"),
        ])
        .build()
        .expect("扩展表定义应有效");
    let report = initializer
        .sync_table_definitions(&[&extended])
        .await
        .expect("加生成列与注释应成功");
    let kinds = report
        .changes
        .iter()
        .map(|change| change.kind)
        .collect::<Vec<_>>();
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SchemaSyncChangeKind::AddedColumn)
            .count(),
        2
    );
    assert!(kinds.contains(&SchemaSyncChangeKind::CommentedColumn));

    // 生成列在 catalog 中为 STORED，表达式可回读。
    let generated: String = query_scalar(
        &database,
        "SELECT attgenerated::text FROM pg_attribute \
         WHERE attrelid = 'wave1_sync_generated'::regclass AND attname = 'total'",
        "wave1_sync_generated",
    )
    .await;
    assert_eq!(generated, "s");
    let expression: String = query_scalar(
        &database,
        "SELECT pg_get_expr(d.adbin, d.adrelid) FROM pg_attrdef d \
         JOIN pg_attribute a ON a.attrelid = d.adrelid AND a.attnum = d.adnum \
         WHERE a.attrelid = 'wave1_sync_generated'::regclass AND a.attname = 'total'",
        "wave1_sync_generated",
    )
    .await;
    assert!(expression.contains("price * 2"), "实得表达式: {expression}");
    let comment: Option<String> = sqlx::query_scalar(
        "SELECT col_description(a.attrelid, a.attnum) FROM pg_attribute a \
         WHERE a.attrelid = 'wave1_sync_generated'::regclass AND a.attname = 'nickname'",
    )
    .fetch_one(database.pool())
    .await
    .expect("查询列注释应成功");
    assert_eq!(comment.as_deref(), Some("昵称"));

    // 注释变更只在差异时下发 COMMENT ON COLUMN。
    let renamed = Table::new("wave1_sync_generated")
        .fields([
            Field::id("id"),
            Field::integer("price").required(),
            Field::bigint("total").generated("price * 2"),
            Field::string("nickname", 32).comment("新昵称"),
        ])
        .build()
        .expect("注释变更表定义应有效");
    let report = initializer
        .sync_table_definitions(&[&renamed])
        .await
        .expect("更新注释应成功");
    assert_eq!(
        report
            .changes
            .iter()
            .filter(|change| change.kind == SchemaSyncChangeKind::CommentedColumn)
            .count(),
        1
    );
    let comment: Option<String> = sqlx::query_scalar(
        "SELECT col_description(a.attrelid, a.attnum) FROM pg_attribute a \
         WHERE a.attrelid = 'wave1_sync_generated'::regclass AND a.attname = 'nickname'",
    )
    .fetch_one(database.pool())
    .await
    .expect("查询列注释应成功");
    assert_eq!(comment.as_deref(), Some("新昵称"));

    let second = initializer
        .sync_table_definitions(&[&renamed])
        .await
        .expect("注释更新后二次同步应成功");
    assert!(second.is_noop(), "生成列与注释必须幂等: {second:?}");

    reset_tables(&database, &["wave1_sync_generated"]).await;
}

// ---- P0-03：63 字节标识符规则 ----

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave1_schema_sync_enforces_63_byte_identifiers() {
    let database = connect().await;
    let initializer = initializer(connect().await);

    // 显式 64 字节索引名 fail-closed。
    let long_index = Table::new("wave1_sync_long_idx")
        .fields([Field::id("id"), Field::string("name", 32)])
        .index_named("i".repeat(64), ["name"])
        .build()
        .expect("构建期允许 64 字节名称");
    let error = initializer
        .sync_table_definitions(&[&long_index])
        .await
        .expect_err("64 字节索引名必须拒绝");
    assert!(
        matches!(error, BaseError::ConfigError(ref message) if message.contains("63")),
        "实得错误: {error:?}"
    );

    // 自动生成的索引名超长时按稳定 hash 截断到 63 字节内。
    let table_name = "t".repeat(30);
    let field_name = "c".repeat(29);
    reset_tables(&database, &[table_name.as_str()]).await;
    let truncated = Table::new(table_name.as_str())
        .fields([Field::id("id"), Field::string(field_name.as_str(), 32)])
        .index([field_name.as_str()])
        .build()
        .expect("长名字表定义应有效");
    initializer
        .sync_table_definitions(&[&truncated])
        .await
        .expect("自动索引名应可截断");
    let index_names: Vec<String> = sqlx::query_scalar(
        "SELECT i.relname::text FROM pg_index ix \
         JOIN pg_class i ON i.oid = ix.indexrelid \
         JOIN pg_class t ON t.oid = ix.indrelid \
         WHERE t.relname = $1 AND NOT ix.indisprimary",
    )
    .bind(&table_name)
    .fetch_all(database.pool())
    .await
    .expect("查询索引名应成功");
    assert_eq!(index_names.len(), 1, "应创建一个自动索引");
    assert!(
        index_names[0].len() <= 63,
        "截断后索引名必须 <= 63 字节: {}",
        index_names[0]
    );
    let second = initializer
        .sync_table_definitions(&[&truncated])
        .await
        .expect("截断索引二次同步应成功");
    assert!(second.is_noop(), "截断索引名必须稳定: {second:?}");
    reset_tables(&database, &[table_name.as_str()]).await;
}

// ---- P1-14：CONCURRENTLY + NOT VALID/VALIDATE ----

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave1_schema_sync_online_ddl_concurrently_and_validated() {
    let database = connect().await;
    reset_tables(&database, &["wave1_sync_child", "wave1_sync_parent"]).await;
    let initializer = initializer(connect().await);

    let parent = Table::new("wave1_sync_parent")
        .fields([
            Field::id("id"),
            Field::string("code", 32).required().unique(),
        ])
        .build()
        .expect("父表定义应有效");
    let child_initial = Table::new("wave1_sync_child")
        .fields([
            Field::id("id"),
            Field::bigint("parent_id").required(),
            Field::string("status", 16).required().default("active"),
        ])
        .build()
        .expect("子表初始定义应有效");
    initializer
        .sync_table_definitions(&[&parent, &child_initial])
        .await
        .expect("初始结构应创建成功");

    // 控制表规模：3 个父行 + 200 个子行，CONCURRENTLY 也足够快。
    database
        .execute_with_params(
            "INSERT INTO wave1_sync_parent (id, code) VALUES (1, 'a'), (2, 'b'), (3, 'c')",
            vec![],
        )
        .await
        .expect("插入父行应成功");
    database
        .execute_with_params(
            "INSERT INTO wave1_sync_child (parent_id, status)
             SELECT (g % 3) + 1, 'active' FROM generate_series(1, 200) AS g",
            vec![],
        )
        .await
        .expect("插入子行应成功");

    let child_extended = Table::new("wave1_sync_child")
        .fields([
            Field::id("id"),
            Field::bigint("parent_id").required(),
            Field::string("display_name", 64),
            Field::string("status", 16).required().default("active"),
        ])
        .index_named("idx_wave1_sync_child_display", ["display_name"])
        .check_named(
            "chk_wave1_sync_child_status",
            "\"status\" IN ('active', 'disabled')",
        )
        .foreign_key_named(
            "fk_wave1_sync_child_parent",
            ["parent_id"],
            "wave1_sync_parent",
            ["id"],
        )
        .build()
        .expect("子表扩展定义应有效");
    let report = initializer
        .sync_table_definitions(&[&parent, &child_extended])
        .await
        .expect("在线加列/索引/CHECK/外键应成功");
    let kinds = report
        .changes
        .iter()
        .map(|change| change.kind)
        .collect::<Vec<_>>();
    assert!(kinds.contains(&SchemaSyncChangeKind::AddedColumn));
    assert!(kinds.contains(&SchemaSyncChangeKind::AddedIndex));
    assert!(kinds.contains(&SchemaSyncChangeKind::AddedCheck));
    assert!(kinds.contains(&SchemaSyncChangeKind::AddedForeignKey));

    // CONCURRENTLY 建出的索引必须有效（不是 INVALID 残留）。
    let index_valid: bool = sqlx::query_scalar(
        "SELECT ix.indisvalid FROM pg_index ix \
         JOIN pg_class i ON i.oid = ix.indexrelid \
         WHERE i.relname = 'idx_wave1_sync_child_display'",
    )
    .fetch_one(database.pool())
    .await
    .expect("查询索引有效性应成功");
    assert!(index_valid, "CONCURRENTLY 索引必须有效");

    // NOT VALID 后必须 VALIDATE，约束最终 convalidated = true。
    let constraints: Vec<(String, bool)> = sqlx::query_as(
        "SELECT conname, convalidated FROM pg_constraint \
         WHERE conrelid = 'wave1_sync_child'::regclass \
           AND conname IN ('chk_wave1_sync_child_status', 'fk_wave1_sync_child_parent') \
         ORDER BY conname",
    )
    .fetch_all(database.pool())
    .await
    .expect("查询约束状态应成功");
    assert_eq!(constraints.len(), 2, "CHECK 与外键都应存在");
    for (name, validated) in constraints {
        assert!(validated, "约束 {name} 必须已完成 VALIDATE");
    }

    let second = initializer
        .sync_table_definitions(&[&parent, &child_extended])
        .await
        .expect("在线 DDL 后二次同步应成功");
    assert!(second.is_noop(), "在线 DDL 必须幂等: {second:?}");

    reset_tables(&database, &["wave1_sync_child", "wave1_sync_parent"]).await;
}

// ---- 地基步骤先于预检：已有表新增列引用的约束 ----

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave1_schema_sync_adds_enum_column_to_existing_table() {
    let database = connect().await;
    reset_tables(&database, &["wave1_sync_add_enum"]).await;
    let initializer = initializer(connect().await);

    let base = Table::new("wave1_sync_add_enum")
        .fields([Field::id("id")])
        .build()
        .expect("基础表定义应有效");
    initializer
        .sync_table_definitions(&[&base])
        .await
        .expect("基础表应创建成功");

    // 新增 enum 列会生成引用该新列的 CHECK；预检必须在加列之后执行。
    let extended = Table::new("wave1_sync_add_enum")
        .fields([
            Field::id("id"),
            Field::enumeration("status", ["active", "disabled"]).required(),
        ])
        .build()
        .expect("扩展表定义应有效");
    let report = initializer
        .sync_table_definitions(&[&extended])
        .await
        .expect("已有表新增 enum 列应成功");
    assert_eq!(
        report
            .changes
            .iter()
            .filter(|change| change.kind == SchemaSyncChangeKind::AddedColumn)
            .count(),
        1
    );
    assert_eq!(
        report
            .changes
            .iter()
            .filter(|change| change.kind == SchemaSyncChangeKind::AddedCheck)
            .count(),
        1
    );

    // CHECK 已 VALIDATE 并实际拒绝非法取值。
    sqlx::query("INSERT INTO wave1_sync_add_enum (id, status) VALUES (1, 'bogus')")
        .execute(database.pool())
        .await
        .expect_err("enum CHECK 必须拒绝非法取值");
    sqlx::query("INSERT INTO wave1_sync_add_enum (id, status) VALUES (2, 'active')")
        .execute(database.pool())
        .await
        .expect("合法取值应可插入");
    let validated: bool = sqlx::query_scalar(
        "SELECT convalidated FROM pg_constraint \
         WHERE conrelid = 'wave1_sync_add_enum'::regclass \
           AND conname = 'chk_wave1_sync_add_enum_status'",
    )
    .fetch_one(database.pool())
    .await
    .expect("查询 CHECK 状态应成功");
    assert!(validated, "enum CHECK 必须已完成 VALIDATE");

    let second = initializer
        .sync_table_definitions(&[&extended])
        .await
        .expect("enum 列二次同步应成功");
    assert!(second.is_noop(), "enum 列必须幂等: {second:?}");

    reset_tables(&database, &["wave1_sync_add_enum"]).await;
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave1_schema_sync_adds_column_and_foreign_key_to_existing_table() {
    let database = connect().await;
    reset_tables(
        &database,
        &["wave1_sync_add_fk_child", "wave1_sync_add_fk_parent"],
    )
    .await;
    let initializer = initializer(connect().await);

    let parent = Table::new("wave1_sync_add_fk_parent")
        .fields([Field::id("id")])
        .build()
        .expect("父表定义应有效");
    let child_base = Table::new("wave1_sync_add_fk_child")
        .fields([Field::id("id")])
        .build()
        .expect("子表基础定义应有效");
    initializer
        .sync_table_definitions(&[&parent, &child_base])
        .await
        .expect("基础结构应创建成功");

    // 外键引用的是本次新增的列；预检必须在加列之后执行，否则报 42703。
    let child_extended = Table::new("wave1_sync_add_fk_child")
        .fields([Field::id("id"), Field::bigint("parent_id").required()])
        .foreign_key_named(
            "fk_wave1_sync_add_fk_parent",
            ["parent_id"],
            "wave1_sync_add_fk_parent",
            ["id"],
        )
        .build()
        .expect("扩展子表定义应有效");
    let report = initializer
        .sync_table_definitions(&[&parent, &child_extended])
        .await
        .expect("已有表新增列+外键应成功");
    assert!(report
        .changes
        .iter()
        .any(|change| change.kind == SchemaSyncChangeKind::AddedColumn));
    assert!(report
        .changes
        .iter()
        .any(|change| change.kind == SchemaSyncChangeKind::AddedForeignKey));

    // 外键实际生效。
    sqlx::query("INSERT INTO wave1_sync_add_fk_child (id, parent_id) VALUES (1, 999)")
        .execute(database.pool())
        .await
        .expect_err("外键必须拒绝不存在的父行");
    sqlx::query("INSERT INTO wave1_sync_add_fk_parent (id) VALUES (1)")
        .execute(database.pool())
        .await
        .expect("插入父行应成功");
    sqlx::query("INSERT INTO wave1_sync_add_fk_child (id, parent_id) VALUES (1, 1)")
        .execute(database.pool())
        .await
        .expect("合法外键应可插入");
    let validated: bool = sqlx::query_scalar(
        "SELECT convalidated FROM pg_constraint \
         WHERE conrelid = 'wave1_sync_add_fk_child'::regclass \
           AND conname = 'fk_wave1_sync_add_fk_parent'",
    )
    .fetch_one(database.pool())
    .await
    .expect("查询外键状态应成功");
    assert!(validated, "外键必须已完成 VALIDATE");

    let second = initializer
        .sync_table_definitions(&[&parent, &child_extended])
        .await
        .expect("外键同步后二次同步应成功");
    assert!(second.is_noop(), "已验证外键必须幂等: {second:?}");

    reset_tables(
        &database,
        &["wave1_sync_add_fk_child", "wave1_sync_add_fk_parent"],
    )
    .await;
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave1_schema_sync_preflight_violation_rolls_back_foundation_ddl() {
    let database = connect().await;
    reset_tables(&database, &["wave1_sync_rollback"]).await;
    let initializer = initializer(connect().await);

    let base = Table::new("wave1_sync_rollback")
        .fields([Field::id("id"), Field::integer("score").required()])
        .build()
        .expect("基础表定义应有效");
    initializer
        .sync_table_definitions(&[&base])
        .await
        .expect("基础表应创建成功");
    sqlx::query("INSERT INTO wave1_sync_rollback (id, score) VALUES (1, -5)")
        .execute(database.pool())
        .await
        .expect("旧数据应写入成功");

    // residue 列属于地基步骤，必须先于预检执行；预检违规时整体回滚。
    let extended = Table::new("wave1_sync_rollback")
        .fields([
            Field::id("id"),
            Field::integer("score").required(),
            Field::string("residue", 16),
        ])
        .check_named("chk_wave1_sync_rollback_score", "\"score\" >= 0")
        .build()
        .expect("扩展表定义应有效");
    let error = initializer
        .sync_table_definitions(&[&extended])
        .await
        .expect_err("旧数据违反新 CHECK 时必须整体回滚");
    assert!(
        error.to_string().contains("人工处理"),
        "应返回旧数据违规错误: {error}"
    );

    let residue_columns: i64 = query_scalar(
        &database,
        "SELECT COUNT(*) FROM information_schema.columns \
         WHERE table_schema = current_schema() AND table_name = $1 AND column_name = 'residue'",
        "wave1_sync_rollback",
    )
    .await;
    assert_eq!(residue_columns, 0, "回滚后不得留下新列");

    reset_tables(&database, &["wave1_sync_rollback"]).await;
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave1_schema_sync_resets_lock_timeout_after_sync() {
    // 单连接池保证同步借用与前后断言查询命中同一物理连接，才能真正验证会话参数复位。
    let mut config = DatabaseConfig::default();
    config.max_connections = 1;
    let database = Database::connect_with_config(&test_db_url(), config)
        .await
        .expect("PG_TEST_URL 指向的 PostgreSQL 必须可连接");
    reset_tables(&database, &["wave1_sync_lock_timeout"]).await;
    let pool = database.pool().clone();
    let initializer = initializer(database);

    let timeout_before: String = sqlx::query_scalar("SELECT current_setting('lock_timeout')")
        .fetch_one(&pool)
        .await
        .expect("查询 lock_timeout 应成功");
    assert_eq!(timeout_before, "0", "同步前默认 lock_timeout 应为 0");

    let definition = Table::new("wave1_sync_lock_timeout")
        .fields([Field::id("id"), Field::string("name", 32)])
        .build()
        .expect("表定义应有效");
    initializer
        .sync_table_definitions(&[&definition])
        .await
        .expect("同步应成功");

    // sync 借用过的连接归还池后，业务查询不得继承 30s lock_timeout。
    let timeout_after: String = sqlx::query_scalar("SELECT current_setting('lock_timeout')")
        .fetch_one(&pool)
        .await
        .expect("查询 lock_timeout 应成功");
    assert_eq!(timeout_after, "0", "sync 后必须复位 lock_timeout");

    sqlx::query("DROP TABLE IF EXISTS \"wave1_sync_lock_timeout\" CASCADE")
        .execute(&pool)
        .await
        .expect("清理测试表应成功");
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave1_schema_sync_supports_dotted_collation_names() {
    let database = connect().await;
    reset_tables(&database, &["wave1_sync_collation"]).await;
    let initializer = initializer(connect().await);

    let dotted: Option<String> = sqlx::query_scalar(
        "SELECT collname::text FROM pg_collation \
         WHERE collname LIKE '%.%' ORDER BY collname LIMIT 1",
    )
    .fetch_optional(database.pool())
    .await
    .expect("查询 pg_collation 应成功");

    let collation = dotted.as_deref().unwrap_or("en_US.utf8");
    let definition = Table::new("wave1_sync_collation")
        .fields([
            Field::id("id"),
            Field::string("name", 32).required().collation(collation),
        ])
        .build()
        .expect("collation 表定义应有效");

    match dotted {
        Some(name) => {
            initializer
                .sync_table_definitions(&[&definition])
                .await
                .expect("带点号的 libc collation 应可建表");
            let actual: String = query_scalar(
                &database,
                "SELECT co.collname::text FROM pg_attribute a \
                 JOIN pg_class c ON c.oid = a.attrelid \
                 JOIN pg_collation co ON co.oid = a.attcollation \
                 WHERE c.relname = $1 AND a.attname = 'name'",
                "wave1_sync_collation",
            )
            .await;
            assert_eq!(actual, name, "整段 collation 名称必须作为单个标识符");
        }
        None => {
            // 实例无该名称时至少验证渲染：整体引用会报 “en_US.utf8 不存在”，
            // 若被按点号拆段则会报 schema "en_US" 不存在（3F000）。
            let error = initializer
                .sync_table_definitions(&[&definition])
                .await
                .expect_err("实例缺少该 collation 时应失败");
            let message = error.to_string();
            assert!(
                message.contains("en_US.utf8"),
                "collation 必须作为单个标识符渲染: {message}"
            );
        }
    }
    reset_tables(&database, &["wave1_sync_collation"]).await;
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave1_schema_sync_self_created_collation_is_idempotent() {
    let database = connect().await;
    reset_tables(&database, &["wave1_sync_custom_collation"]).await;
    database
        .execute_with_params("DROP COLLATION IF EXISTS xx_custom_coll CASCADE", vec![])
        .await
        .expect("清理测试 collation 应成功");

    // ICU 优先；实例不支持时退回必然存在的 libc locale。
    if sqlx::query("CREATE COLLATION xx_custom_coll (provider = icu, locale = 'en-US')")
        .execute(database.pool())
        .await
        .is_err()
    {
        sqlx::query("CREATE COLLATION xx_custom_coll (provider = libc, locale = 'C')")
            .execute(database.pool())
            .await
            .expect("libc collation 应可创建");
    }

    let initializer = initializer(connect().await);
    let definition = Table::new("wave1_sync_custom_collation")
        .fields([
            Field::id("id"),
            Field::string("name", 32)
                .required()
                .collation("xx_custom_coll"),
        ])
        .build()
        .expect("自建 collation 表定义应有效");

    let created = initializer
        .sync_table_definitions(&[&definition])
        .await
        .expect("首次同步应创建表");
    assert_eq!(created.changes.len(), 1);

    // introspection 必须回读裸名，否则 public.xx_custom_coll 与声明侧永不相等。
    let actual: String = query_scalar(
        &database,
        "SELECT co.collname::text FROM pg_attribute a \
         JOIN pg_class c ON c.oid = a.attrelid \
         JOIN pg_collation co ON co.oid = a.attcollation \
         WHERE c.relname = $1 AND a.attname = 'name'",
        "wave1_sync_custom_collation",
    )
    .await;
    assert_eq!(actual, "xx_custom_coll");

    let second = initializer
        .sync_table_definitions(&[&definition])
        .await
        .expect("二次同步应成功");
    assert!(
        second.is_noop(),
        "自建 collation 不得在二次同步被判为漂移: {second:?}"
    );

    reset_tables(&database, &["wave1_sync_custom_collation"]).await;
    database
        .execute_with_params("DROP COLLATION xx_custom_coll", vec![])
        .await
        .expect("清理测试 collation 应成功");
}

// ---- collation 收敛：default 归一与大小写敏感 ----

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave1_schema_sync_default_collation_is_noop_after_first_sync() {
    let database = connect().await;
    reset_tables(&database, &["wave1_sync_default_collation"]).await;
    let initializer = initializer(connect().await);

    let definition = Table::new("wave1_sync_default_collation")
        .fields([
            Field::id("id"),
            Field::string("name", 32).required().collation("default"),
        ])
        .build()
        .expect("default collation 表定义应有效");

    for attempt in 1..=3 {
        let report = initializer
            .sync_table_definitions(&[&definition])
            .await
            .expect("default collation 同步应成功");
        if attempt == 1 {
            assert_eq!(report.changes.len(), 1);
        } else {
            assert!(
                report.is_noop(),
                "第 {attempt} 次同步必须 no-op: {report:?}"
            );
        }
    }

    reset_tables(&database, &["wave1_sync_default_collation"]).await;
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn wave1_schema_sync_collation_names_are_case_sensitive() {
    let database = connect().await;
    reset_tables(&database, &["wave1_sync_collation_case"]).await;
    database
        .execute_with_params("DROP COLLATION IF EXISTS public.\"c\" CASCADE", vec![])
        .await
        .expect("清理测试 collation 应成功");
    database
        .execute_with_params(
            "CREATE COLLATION public.\"c\" (provider = libc, locale = 'C')",
            vec![],
        )
        .await
        .expect("libc collation 应可创建");
    database
        .execute_with_params(
            "CREATE TABLE wave1_sync_collation_case (\
             \"id\" BIGINT NOT NULL GENERATED BY DEFAULT AS IDENTITY, \
             \"name\" VARCHAR(32) COLLATE public.\"c\" NOT NULL, \
             PRIMARY KEY (\"id\"))",
            vec![],
        )
        .await
        .expect("初始表应可创建");

    let initializer = initializer(connect().await);
    let definition = Table::new("wave1_sync_collation_case")
        .fields([
            Field::id("id"),
            Field::string("name", 32).required().collation("C"),
        ])
        .build()
        .expect("大小写 collation 表定义应有效");

    let report = initializer
        .sync_table_definitions(&[&definition])
        .await
        .expect("大小写不同应产生修改计划");
    assert!(
        report
            .changes
            .iter()
            .any(|change| change.kind == SchemaSyncChangeKind::ModifiedColumn),
        "c 与 C 是不同 collation，必须产生 ModifiedColumn: {report:?}"
    );

    let actual: String = query_scalar(
        &database,
        "SELECT co.collname::text FROM pg_attribute a \
         JOIN pg_class c ON c.oid = a.attrelid \
         JOIN pg_collation co ON co.oid = a.attcollation \
         WHERE c.relname = $1 AND a.attname = 'name'",
        "wave1_sync_collation_case",
    )
    .await;
    assert_eq!(actual, "C", "读回 collation 必须为 C");

    let second = initializer
        .sync_table_definitions(&[&definition])
        .await
        .expect("切换到 C 后二次同步应成功");
    assert!(second.is_noop(), "切换后必须幂等: {second:?}");

    reset_tables(&database, &["wave1_sync_collation_case"]).await;
    database
        .execute_with_params("DROP COLLATION public.\"c\"", vec![])
        .await
        .expect("清理测试 collation 应成功");
}
