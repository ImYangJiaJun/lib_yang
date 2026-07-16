#![allow(clippy::expect_used)]

use std::sync::Arc;
use testcontainers::{runners::AsyncRunner, GenericImage, ImageExt};
use yang_base::database::{DatabaseInitializer, SchemaSyncChangeKind};
use yang_base::router::{AppRouter, ModuleRouter};
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

fn account_app(table: TableDefinition) -> AppRouter {
    AppRouter::new()
        .module(ModuleRouter::new("account", "账号").table(table))
        .expect("账号模块应注册成功")
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
    let initial = Arc::new(account_app(account_table(32, false)));

    let (left, right) = tokio::join!(
        first.sync_app_schema(initial.as_ref()),
        second.sync_app_schema(initial.as_ref())
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
    let expanded = account_app(expanded_table.clone());
    let report = first
        .sync_app_schema(&expanded)
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
        .sync_app_schema(&expanded)
        .await
        .expect("重复同步应成功")
        .is_noop());

    let pending_table = Table::new("aaa_schema_sync_pending")
        .fields([Field::bigint("id").required().primary_key()])
        .build()
        .expect("pending 表定义应有效");
    let incompatible = AppRouter::new()
        .module(
            ModuleRouter::new("account", "账号")
                .table(account_table(255, true))
                .schema(pending_table),
        )
        .expect("不兼容测试模块应注册成功");
    let error = first
        .sync_app_schema(&incompatible)
        .await
        .expect_err("扩大字段容量需要人工迁移，启动应失败");
    assert!(error.to_string().contains("不可自动修改"));
    let verification_db = Database::connect(&url).await.expect("验证数据库连接应成功");
    assert!(
        !verification_db
            .table_exists("aaa_schema_sync_pending")
            .await
            .expect("应能查询待建表是否存在"),
        "全表预规划应保证已知冲突发生时不先创建前序表"
    );
}
