//! 显式数据库资源集成测试。

use yang_base::tools::ToolsBuilder;
use yang_db::{Database, DatabaseConfig};

/// 真实数据库可用时，验证 Database 交给 Tools 后仍保持同一连接池能力。
#[tokio::test]
#[ignore = "需要本地 MySQL"]
async fn explicit_database_resource_is_available_from_tools() {
    let db_url = "mysql://root:password@localhost:3306/test_db";
    let database = match Database::connect_with_config(db_url, DatabaseConfig::default()).await {
        Ok(database) => database,
        Err(error) => {
            println!("跳过测试：无法连接到数据库: {error}");
            return;
        }
    };

    let tools = ToolsBuilder::new()
        .mysql(database)
        .build()
        .expect("Tools 应构建成功");

    assert!(tools
        .mysql()
        .expect("数据库应存在")
        .health_check()
        .await
        .is_ok());
    let table = yang_db::table!("test_table");
    assert!(tools
        .mysql()
        .expect("数据库应存在")
        .table(table)
        .try_to_sql()
        .is_ok());
}

#[test]
fn missing_database_returns_a_structured_error() {
    let tools = ToolsBuilder::new().build().expect("空 Tools 应可构建");
    assert!(matches!(
        tools.mysql(),
        Err(yang_base::BaseError::DatabaseNotInitialized)
    ));
}
