//! DatabaseInitializer 集成测试
//!
//! 测试数据库初始化器的功能

use yang_base::database::DatabaseInitializer;
use yang_base::plugin::{Plugin, PluginManager};
use yang_db::Database;

/// 测试插件 1
struct TestPlugin1;

#[async_trait::async_trait]
impl Plugin for TestPlugin1 {
    fn name(&self) -> &str {
        "test_plugin_1"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn init_sql(&self) -> Vec<String> {
        vec![r#"
            CREATE TABLE IF NOT EXISTS test_users (
                id INT AUTO_INCREMENT PRIMARY KEY,
                name VARCHAR(255) NOT NULL,
                email VARCHAR(255) NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            "#
        .to_string()]
    }

    fn migration_sql(&self) -> Vec<(String, String)> {
        vec![(
            "20240101000001".to_string(),
            "ALTER TABLE test_users ADD COLUMN status INT DEFAULT 1".to_string(),
        )]
    }
}

/// 测试插件 2（依赖插件 1）
struct TestPlugin2;

#[async_trait::async_trait]
impl Plugin for TestPlugin2 {
    fn name(&self) -> &str {
        "test_plugin_2"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> &[&str] {
        &["test_plugin_1"]
    }

    fn init_sql(&self) -> Vec<String> {
        vec![r#"
            CREATE TABLE IF NOT EXISTS test_orders (
                id INT AUTO_INCREMENT PRIMARY KEY,
                user_id INT NOT NULL,
                amount DECIMAL(10, 2) NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            "#
        .to_string()]
    }
}

/// 测试数据库初始化（非事务模式）
///
/// 注意：此测试需要真实的 MySQL 数据库连接
/// 如果没有可用的数据库，测试将被跳过
#[tokio::test]
#[ignore] // 默认忽略，需要手动运行
async fn test_database_initializer_without_transaction() {
    // 使用测试数据库连接字符串
    let db_url = "mysql://root:password@localhost:3306/test_db";

    // 连接数据库
    let db = match Database::connect(db_url).await {
        Ok(db) => db,
        Err(_) => {
            println!("跳过测试：无法连接到数据库");
            return;
        }
    };

    // 创建插件管理器
    let manager = PluginManager::new();
    manager.register(TestPlugin1).await.unwrap();
    manager.register(TestPlugin2).await.unwrap();

    // 创建数据库初始化器（非事务模式）
    let initializer = DatabaseInitializer::new(db, false);

    // 初始化所有插件的数据库
    let result = initializer.initialize_all(&manager).await;
    assert!(result.is_ok(), "数据库初始化失败: {:?}", result);

    // 重新连接数据库以验证表
    let db = Database::connect(db_url).await.unwrap();

    // 验证表已创建
    let table_exists = db
        .table_exists(yang_db::table!("test_users"))
        .await
        .unwrap();
    assert!(table_exists, "test_users 表未创建");

    let table_exists = db
        .table_exists(yang_db::table!("test_orders"))
        .await
        .unwrap();
    assert!(table_exists, "test_orders 表未创建");

    // 验证迁移记录表已创建
    let table_exists = db
        .table_exists(yang_db::table!("_migrations"))
        .await
        .unwrap();
    assert!(table_exists, "_migrations 表未创建");

    // 清理测试数据
    let _ = db.drop_table(yang_db::table!("test_users")).await;
    let _ = db.drop_table(yang_db::table!("test_orders")).await;
    let _ = db.drop_table(yang_db::table!("_migrations")).await;
}

/// 测试数据库初始化（事务模式）
///
/// 注意：此测试需要真实的 MySQL 数据库连接
/// 如果没有可用的数据库，测试将被跳过
#[tokio::test]
#[ignore] // 默认忽略，需要手动运行
async fn test_database_initializer_with_transaction() {
    // 使用测试数据库连接字符串
    let db_url = "mysql://root:password@localhost:3306/test_db";

    // 连接数据库
    let db = match Database::connect(db_url).await {
        Ok(db) => db,
        Err(_) => {
            println!("跳过测试：无法连接到数据库");
            return;
        }
    };

    // 创建插件管理器
    let manager = PluginManager::new();
    manager.register(TestPlugin1).await.unwrap();
    manager.register(TestPlugin2).await.unwrap();

    // 创建数据库初始化器（事务模式）
    let initializer = DatabaseInitializer::new(db, true);

    // 初始化所有插件的数据库
    let result = initializer.initialize_all(&manager).await;
    assert!(result.is_ok(), "数据库初始化失败: {:?}", result);

    // 重新连接数据库以验证表
    let db = Database::connect(db_url).await.unwrap();

    // 验证表已创建
    let table_exists = db
        .table_exists(yang_db::table!("test_users"))
        .await
        .unwrap();
    assert!(table_exists, "test_users 表未创建");

    let table_exists = db
        .table_exists(yang_db::table!("test_orders"))
        .await
        .unwrap();
    assert!(table_exists, "test_orders 表未创建");

    // 验证迁移记录表已创建
    let table_exists = db
        .table_exists(yang_db::table!("_migrations"))
        .await
        .unwrap();
    assert!(table_exists, "_migrations 表未创建");

    // 清理测试数据
    let _ = db.drop_table(yang_db::table!("test_users")).await;
    let _ = db.drop_table(yang_db::table!("test_orders")).await;
    let _ = db.drop_table(yang_db::table!("_migrations")).await;
}

/// 测试迁移幂等性
///
/// 验证相同的迁移不会被重复执行
#[tokio::test]
#[ignore] // 默认忽略，需要手动运行
async fn test_migration_idempotency() {
    // 使用测试数据库连接字符串
    let db_url = "mysql://root:password@localhost:3306/test_db";

    // 连接数据库
    let db = match Database::connect(db_url).await {
        Ok(db) => db,
        Err(_) => {
            println!("跳过测试：无法连接到数据库");
            return;
        }
    };

    // 创建插件管理器
    let manager = PluginManager::new();
    manager.register(TestPlugin1).await.unwrap();

    // 创建数据库初始化器
    let initializer = DatabaseInitializer::new(db, false);

    // 第一次初始化
    let result = initializer.initialize_all(&manager).await;
    assert!(result.is_ok(), "第一次初始化失败: {:?}", result);

    // 第二次初始化（应该跳过已执行的迁移）
    let result = initializer.initialize_all(&manager).await;
    assert!(result.is_ok(), "第二次初始化失败: {:?}", result);

    // 验证迁移记录
    let is_executed = initializer
        .is_migration_executed("test_plugin_1", "20240101000001")
        .await
        .unwrap();
    assert!(is_executed, "迁移记录未找到");

    // 重新连接数据库以清理
    let db = Database::connect(db_url).await.unwrap();
    let _ = db.drop_table(yang_db::table!("test_users")).await;
    let _ = db.drop_table(yang_db::table!("_migrations")).await;
}

/// 测试创建迁移记录表
#[tokio::test]
#[ignore] // 默认忽略，需要手动运行
async fn test_create_migration_table() {
    // 使用测试数据库连接字符串
    let db_url = "mysql://root:password@localhost:3306/test_db";

    // 连接数据库
    let db = match Database::connect(db_url).await {
        Ok(db) => db,
        Err(_) => {
            println!("跳过测试：无法连接到数据库");
            return;
        }
    };

    // 创建数据库初始化器
    let initializer = DatabaseInitializer::new(db, false);

    // 创建迁移记录表
    let result = initializer.create_migration_table().await;
    assert!(result.is_ok(), "创建迁移记录表失败: {:?}", result);

    // 重新连接数据库以验证表
    let db = Database::connect(db_url).await.unwrap();

    // 验证表已创建
    let table_exists = db
        .table_exists(yang_db::table!("_migrations"))
        .await
        .unwrap();
    assert!(table_exists, "_migrations 表未创建");

    // 清理测试数据
    let _ = db.drop_table(yang_db::table!("_migrations")).await;
}
