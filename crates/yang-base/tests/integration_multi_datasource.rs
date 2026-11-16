#![allow(clippy::expect_used, clippy::unwrap_used)]
#![cfg(all(feature = "mysql", feature = "postgres"))]
//! 多数据源路由与事务同源守卫的 PostgreSQL + MySQL 集成测试。
//!
//! 运行：
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//! MYSQL_TEST_URL=mysql://root:yang-local@127.0.0.1:3306/yang_system_test \
//!   cargo test -p yang-base --no-default-features --features mysql,postgres \
//!   --test integration_multi_datasource -- --ignored --test-threads=1
//! ```

use serde_json::json;
use std::sync::Arc;
use yang_base::action::{ActionContext, Request};
use yang_base::database::{AnyTransaction, DatabaseInitializer};
use yang_base::datasource::DatasourceId;
use yang_base::definition::{
    AddonName, AddonSpec, AppBuilder, ModuleName, ModuleSpec, Str, TableName, TableSpec,
};
use yang_base::error::BaseError;
use yang_base::table::{Field, Record, Table, TableDefinition};
use yang_base::tools::ToolsBuilder;
use yang_db::mysql::{Database as MyDatabase, DatabaseConfig as MyConfig};
use yang_db::postgres::{Database as PgDatabase, DatabaseConfig as PgConfig};
use yang_db::BackendKind;

fn pg_url() -> String {
    std::env::var("PG_TEST_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432/test".to_string())
}

fn mysql_url() -> String {
    std::env::var("MYSQL_TEST_URL")
        .unwrap_or_else(|_| "mysql://root:yang-local@127.0.0.1:3306/yang_system_test".to_string())
}

fn mysql_definition() -> TableDefinition {
    Table::new("multi_ds_mysql_rows")
        .fields([Field::id("id"), Field::string("name", 64).required()])
        .build()
        .expect("MySQL 表定义应有效")
}

fn pg_definition(source: DatasourceId) -> TableDefinition {
    Table::new("multi_ds_pg_rows")
        .datasource(source)
        .fields([Field::id("id"), Field::string("name", 64).required()])
        .build()
        .expect("PG 表定义应有效")
}

#[tokio::test]
#[ignore = "需要本地 MySQL 与 PostgreSQL 容器"]
async fn routes_tables_to_their_own_datasource_and_rejects_cross_source_tx() {
    let mysql = MyDatabase::connect_with_config(&mysql_url(), MyConfig::default())
        .await
        .expect("连接 MySQL 失败");
    let pg = PgDatabase::connect_with_config(&pg_url(), PgConfig::default())
        .await
        .expect("连接 PG 失败");

    sqlx::query("DROP TABLE IF EXISTS multi_ds_mysql_rows")
        .execute(mysql.pool())
        .await
        .expect("清理 MySQL 表失败");
    sqlx::query("DROP TABLE IF EXISTS multi_ds_pg_rows")
        .execute(pg.pool())
        .await
        .expect("清理 PG 表失败");

    let pg_source = DatasourceId::new("pg_main").expect("数据源名应有效");
    let mysql_def = mysql_definition();
    let pg_def = pg_definition(pg_source.clone());

    DatabaseInitializer::new(mysql.clone())
        .sync_table_definitions(&[&mysql_def])
        .await
        .expect("MySQL schema 同步应成功");
    DatabaseInitializer::new_postgres(pg.clone())
        .sync_table_definitions(&[&pg_def])
        .await
        .expect("PG schema 同步应成功");

    let tools = Arc::new(
        ToolsBuilder::new()
            .mysql_named(DatasourceId::default_source(), mysql.clone())
            .postgres_named(pg_source.clone(), pg.clone())
            .default_datasource(DatasourceId::default_source())
            .build()
            .expect("双数据源 Tools 应构建成功"),
    );

    let reports =
        yang_base::database::sync_table_definitions_grouped(&tools, &[&mysql_def, &pg_def])
            .await
            .expect("按数据源分组同步应成功");
    assert_eq!(reports.len(), 2);
    assert!(reports.contains_key(&DatasourceId::default_source()));
    assert!(reports.contains_key(&pg_source));

    assert_eq!(tools.pool_statuses().len(), 2);
    assert_eq!(
        tools.database().expect("默认源应存在").backend(),
        BackendKind::MySql
    );
    assert_eq!(
        tools
            .database_named(&pg_source)
            .expect("PG 源应存在")
            .backend(),
        BackendKind::PostgreSql
    );

    let mysql_ctx = ActionContext::new(Request::new(json!({})), Arc::clone(&tools))
        .with_table_definition(mysql_def);
    let pg_ctx = ActionContext::new(Request::new(json!({})), Arc::clone(&tools))
        .with_table_definition(pg_def);

    mysql_ctx
        .table_query()
        .expect("MySQL 查询应构建")
        .insert(Record::new().set("name", "mysql-row"))
        .await
        .expect("MySQL 插入应成功");
    pg_ctx
        .table_query()
        .expect("PG 查询应构建")
        .insert(Record::new().set("name", "pg-row"))
        .await
        .expect("PG 插入应成功");

    let mysql_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM multi_ds_mysql_rows")
        .fetch_one(mysql.pool())
        .await
        .expect("统计 MySQL 行数失败");
    let pg_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM multi_ds_pg_rows")
        .fetch_one(pg.pool())
        .await
        .expect("统计 PG 行数失败");
    assert_eq!(mysql_count, 1);
    assert_eq!(pg_count, 1);

    // 事务必须与表同源：PG 表 + MySQL 事务必须被拒绝。
    let mut mysql_tx =
        AnyTransaction::MySql(mysql.transaction().await.expect("开启 MySQL 事务失败"));
    let error = pg_ctx
        .table_query()
        .expect("PG 查询应构建")
        .insert_in_tx(&mut mysql_tx, Record::new().set("name", "wrong-source"))
        .await
        .expect_err("跨源事务必须被拒绝");
    assert!(
        matches!(error, BaseError::DatasourceMismatch { .. }),
        "应返回 DatasourceMismatch，实得: {error:?}"
    );
    mysql_tx.rollback().await.expect("回滚 MySQL 事务失败");

    // PG 事务自动携带 PG 数据源身份。
    let pg_tx = pg_ctx.begin_transaction().await.expect("PG 事务应开启");
    assert_eq!(pg_tx.source(), Some(&pg_source));
    pg_tx.rollback().await.expect("回滚 PG 事务失败");

    // 模块级数据源声明：ModuleSpec 绑定 PG 源后，编译出的表定义必须带同一源。
    let fields = yang_base::fields! {
        id => yang_base::definition::Key::new(),
        name => Str::new().require(true),
    };
    let table = TableSpec::new(TableName::new("multi_ds_pg_module_rows").expect("表名应有效"))
        .fields(fields);
    let module = ModuleSpec::new(ModuleName::new("pgdemo.items").expect("模块名应有效"))
        .table(table)
        .datasource(pg_source.clone());
    let addon = AddonSpec::new(AddonName::new("pgdemo").expect("Addon 名应有效")).module(module);
    let app = AppBuilder::new()
        .addon(addon)
        .build(Arc::clone(&tools))
        .expect("模块级数据源声明应通过构建");
    let definition = app
        .table_definitions()
        .iter()
        .find(|definition| definition.name() == "multi_ds_pg_module_rows")
        .expect("模块表定义应存在");
    assert_eq!(definition.datasource(), &pg_source);

    // 不同数据源允许同名表：两个模块都叫 shared_rows，各自绑定 MySQL/PG。
    let mysql_shared =
        ModuleSpec::new(ModuleName::new("pgdemo.mysql_shared").expect("模块名应有效")).table(
            TableSpec::new(TableName::new("shared_rows").expect("表名应有效")).fields(
                yang_base::fields! {
                    id => yang_base::definition::Key::new(),
                    name => Str::new().require(true),
                },
            ),
        );
    let pg_shared = ModuleSpec::new(ModuleName::new("pgdemo.pg_shared").expect("模块名应有效"))
        .table(
            TableSpec::new(TableName::new("shared_rows").expect("表名应有效")).fields(
                yang_base::fields! {
                    id => yang_base::definition::Key::new(),
                    name => Str::new().require(true),
                },
            ),
        )
        .datasource(pg_source.clone());
    let shared_addon = AddonSpec::new(AddonName::new("pgdemo").expect("Addon 名应有效"))
        .module(mysql_shared)
        .module(pg_shared);
    let shared_app = AppBuilder::new()
        .addon(shared_addon)
        .build(Arc::clone(&tools))
        .expect("跨数据源同名表应通过构建");
    let shared: Vec<_> = shared_app
        .table_definitions()
        .iter()
        .filter(|definition| definition.name() == "shared_rows")
        .collect();
    assert_eq!(shared.len(), 2, "两个数据源应各保留一份同名表定义");
    assert!(shared
        .iter()
        .any(|definition| definition.datasource().is_default()));
    assert!(shared
        .iter()
        .any(|definition| definition.datasource() == &pg_source));

    // 未注册的数据源必须在构建期 fail-closed。
    let missing = DatasourceId::new("missing_source").expect("数据源名应有效");
    let invalid_module = ModuleSpec::new(ModuleName::new("pgdemo.invalid").expect("模块名应有效"))
        .table(
            TableSpec::new(TableName::new("multi_ds_missing_rows").expect("表名应有效"))
                .fields(yang_base::fields! { id => yang_base::definition::Key::new() }),
        )
        .datasource(missing);
    let invalid_addon =
        AddonSpec::new(AddonName::new("pgdemo").expect("Addon 名应有效")).module(invalid_module);
    let error = AppBuilder::new()
        .addon(invalid_addon)
        .build(Arc::clone(&tools))
        .expect_err("未注册数据源必须被拒绝");
    assert!(
        error.to_string().contains("missing_source"),
        "错误应指出缺失数据源，实得: {error}"
    );

    sqlx::query("DROP TABLE IF EXISTS multi_ds_mysql_rows")
        .execute(mysql.pool())
        .await
        .expect("清理 MySQL 表失败");
    sqlx::query("DROP TABLE IF EXISTS multi_ds_pg_rows")
        .execute(pg.pool())
        .await
        .expect("清理 PG 表失败");
}
