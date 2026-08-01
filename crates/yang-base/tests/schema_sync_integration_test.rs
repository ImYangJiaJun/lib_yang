#![allow(clippy::expect_used)]

use std::sync::Arc;
use testcontainers::{runners::AsyncRunner, GenericImage, ImageExt};
use yang_base::database::{DatabaseInitializer, SchemaSyncChangeKind};
use yang_base::table::{Field, Table, TableDefinition};
use yang_db::Database;

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
    Table::new("schema_sync_accounts")
        .fields(fields)
        .build()
        .expect("schema_sync_accounts 表定义应有效")
}

async fn setup_mysql() -> Option<(testcontainers::ContainerAsync<GenericImage>, String)> {
    let image = GenericImage::new("mysql", "8.0")
        .with_env_var("MYSQL_ROOT_PASSWORD", "test_password")
        .with_env_var("MYSQL_DATABASE", "test_db");
    let container = match image.start().await {
        Ok(container) => container,
        Err(error) => {
            println!("跳过测试：无法启动 Docker 容器: {error}");
            return None;
        }
    };
    let port = container.get_host_port_ipv4(3306).await.ok()?;
    let url = format!("mysql://root:test_password@127.0.0.1:{port}/test_db");
    for _ in 0..30 {
        if Database::connect(&url).await.is_ok() {
            return Some((container, url));
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    println!("跳过测试：MySQL 容器启动超时");
    None
}

#[tokio::test]
#[ignore = "需要 Docker 环境"]
async fn schema_sync_is_concurrent_idempotent_additive_and_fail_closed() {
    let (_container, url) = match setup_mysql().await {
        Some(setup) => setup,
        None => return,
    };
    let first = DatabaseInitializer::new(
        Database::connect(&url)
            .await
            .expect("第一个数据库连接应成功"),
        false,
    );
    let second = DatabaseInitializer::new(
        Database::connect(&url)
            .await
            .expect("第二个数据库连接应成功"),
        false,
    );
    let initial = Arc::new(account_table(32, false));
    let first_definitions = [initial.as_ref()];
    let second_definitions = [initial.as_ref()];

    let (left, right) = tokio::join!(
        first.sync_table_definitions(&first_definitions),
        second.sync_table_definitions(&second_definitions)
    );
    let left = left.expect("第一个并发初始化应成功");
    let right = right.expect("第二个并发初始化应成功");
    let created = left
        .changes
        .iter()
        .chain(&right.changes)
        .filter(|change| change.kind == SchemaSyncChangeKind::CreatedTable)
        .count();
    assert_eq!(created, 1, "跨实例锁应保证只创建一次表");

    let expanded_table = account_table(32, true);
    let report = first
        .sync_table_definitions(&[&expanded_table])
        .await
        .expect("新增可空字段应安全同步");
    assert_eq!(report.changes.len(), 1);
    assert_eq!(report.changes[0].kind, SchemaSyncChangeKind::AddedColumn);
    assert_eq!(report.changes[0].object, "display_name");
    assert!(first
        .validate_table_definition(&expanded_table)
        .await
        .expect("同步后 schema 应可读取")
        .is_compatible());
    assert!(first
        .sync_table_definitions(&[&expanded_table])
        .await
        .expect("重复同步应成功")
        .is_noop());

    let widened = account_table(255, true);
    let widened_report = first
        .sync_table_definitions(&[&widened])
        .await
        .expect("VARCHAR 扩容应在只读预检后自动更新");
    assert!(widened_report
        .changes
        .iter()
        .any(|change| change.kind == SchemaSyncChangeKind::ModifiedColumn));

    let pending_table = Table::new("aaa_schema_sync_pending")
        .fields([Field::bigint("id").required().primary_key()])
        .build()
        .expect("pending 表定义应有效");
    let incompatible = Table::new("schema_sync_accounts")
        .fields([
            Field::id("id"),
            Field::bigint("username").required(),
            Field::string("display_name", 64),
        ])
        .build()
        .expect("不兼容测试表定义应有效");
    let error = first
        .sync_table_definitions(&[&pending_table, &incompatible])
        .await
        .expect_err("文本转数值语义不明确，启动应失败");
    assert!(error.to_string().contains("不可自动修改"));
    let verification_db = Database::connect(&url).await.expect("验证数据库连接应成功");
    let pending_table_ref = yang_db::table!("aaa_schema_sync_pending");
    assert!(
        !verification_db
            .table_exists(pending_table_ref)
            .await
            .expect("应能查询待建表是否存在"),
        "全表预规划应保证已知冲突发生时不先创建前序表"
    );
}

#[tokio::test]
#[ignore = "需要 Docker 环境"]
async fn schema_evolution_reports_dirty_primary_keys_before_any_ddl_and_retries_cleanly() {
    let (_container, url) = match setup_mysql().await {
        Some(setup) => setup,
        None => return,
    };
    let database = Database::connect(&url).await.expect("数据库连接应成功");
    sqlx::query(
        "CREATE TABLE schema_evolution_parent (id BIGINT NOT NULL PRIMARY KEY) ENGINE=InnoDB",
    )
    .execute(database.pool())
    .await
    .expect("父表应创建成功");
    sqlx::query(
        "CREATE TABLE schema_evolution_child (\
         id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY, \
         legacy_parent BIGINT NOT NULL, status VARCHAR(16) NULL, \
         schema_version SMALLINT UNSIGNED NOT NULL DEFAULT 1\
         ) ENGINE=InnoDB",
    )
    .execute(database.pool())
    .await
    .expect("旧子表应创建成功");
    sqlx::query("INSERT INTO schema_evolution_parent (id) VALUES (1), (2), (3)")
        .execute(database.pool())
        .await
        .expect("父数据应写入成功");
    sqlx::query(
        "INSERT INTO schema_evolution_child (id, legacy_parent, status, schema_version) VALUES \
         (7, 999, 'broken', 2), (8, 1, 'active', 2), (9, 1, 'active', 2), (10, 1, NULL, 2)",
    )
    .execute(database.pool())
    .await
    .expect("脏数据应写入成功");

    let desired = Table::new("schema_evolution_child")
        .fields([
            Field::id("id"),
            Field::bigint("parent_id")
                .required()
                .renamed_from("legacy_parent"),
            Field::enumeration("status", ["active", "disabled"]).required(),
            Field::integer("schema_version").required().default(1),
        ])
        .unique_named("uk_schema_evolution_parent", ["parent_id"])
        .check_named(
            "chk_schema_evolution_status",
            "`status` IN ('active', 'disabled')",
        )
        .foreign_key_named(
            "fk_schema_evolution_parent",
            ["parent_id"],
            "schema_evolution_parent",
            ["id"],
        )
        .build()
        .expect("目标表定义应有效");
    let pool = database.pool().clone();
    let initializer = DatabaseInitializer::new(database, false);

    let preflight = initializer
        .preflight_table_definitions(&[&desired])
        .await
        .expect("只读预检本身应成功");
    assert!(!preflight.is_safe());
    assert!(preflight.violations.iter().all(|violation| {
        violation.table == "schema_evolution_child" && !violation.primary_keys.is_empty()
    }));
    assert!(preflight
        .violations
        .iter()
        .any(|violation| violation.primary_keys.iter().any(|key| key == "7")));

    let error = initializer
        .sync_table_definitions(&[&desired])
        .await
        .expect_err("脏数据必须在任何 DDL 前阻止更新");
    let message = error.to_string();
    assert!(message.contains("schema_evolution_child"));
    assert!(message.contains("primary_keys"));
    assert!(message.contains('7'));
    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT CAST(COLUMN_NAME AS CHAR) FROM information_schema.columns \
         WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'schema_evolution_child' \
         ORDER BY ORDINAL_POSITION",
    )
    .fetch_all(&pool)
    .await
    .expect("应能核对失败后的列");
    assert!(columns.iter().any(|column| column == "legacy_parent"));
    assert!(!columns.iter().any(|column| column == "parent_id"));

    sqlx::query("DELETE FROM schema_evolution_child WHERE id = 9")
        .execute(&pool)
        .await
        .expect("重复数据应可人工删除");
    sqlx::query(
        "UPDATE schema_evolution_child SET legacy_parent = 2, status = 'active' WHERE id = 7",
    )
    .execute(&pool)
    .await
    .expect("脏数据应可人工修复");
    sqlx::query(
        "UPDATE schema_evolution_child SET legacy_parent = 3, status = 'active' WHERE id = 10",
    )
    .execute(&pool)
    .await
    .expect("NULL 与唯一键冲突应可人工修复");

    let report = initializer
        .sync_table_definitions(&[&desired])
        .await
        .expect("人工修复后应原位更新结构");
    assert!(report
        .changes
        .iter()
        .any(|change| change.kind == SchemaSyncChangeKind::RenamedColumn));
    assert!(report
        .changes
        .iter()
        .any(|change| change.kind == SchemaSyncChangeKind::ModifiedColumn));
    assert!(initializer
        .sync_table_definitions(&[&desired])
        .await
        .expect("更新完成后重试应幂等")
        .is_noop());
    let versions: Vec<i64> = sqlx::query_scalar(
        "SELECT CAST(schema_version AS SIGNED) FROM schema_evolution_child ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .expect("smallint 扩为 int 后应能读取旧数据");
    assert_eq!(versions, [2, 2, 2]);
}
