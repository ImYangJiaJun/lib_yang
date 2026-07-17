//! 数据库管理集成测试
//!
//! 使用 testcontainers 创建隔离的测试环境，测试：
//! - 显式数据库资源初始化
//! - 数据库初始化流程（事务和非事务模式）
//! - 迁移记录表创建
//! - 迁移执行和幂等性
//!
//! **验证需求**: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 6.1, 6.4, 9.2, 9.3, 9.4, 11.2, 11.3, 11.4
//!
//! **注意**: 这些测试需要 Docker 环境。如果没有 Docker，测试将被跳过。
//! 运行测试：`cargo test --test database_integration_test -- --test-threads=1`

#![allow(deprecated)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use testcontainers::{runners::AsyncRunner, GenericImage, ImageExt};
use yang_base::database::DatabaseInitializer;
use yang_base::plugin::{Plugin, PluginManager};
use yang_base::tools::ToolsBuilder;
use yang_db::{Database, DatabaseConfig};

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
                name VARCHAR(255) NOT NULL COMMENT '用户名',
                email VARCHAR(255) NOT NULL COMMENT '邮箱',
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间'
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='测试用户表'
            "#
        .to_string()]
    }

    fn migration_sql(&self) -> Vec<(String, String)> {
        vec![
            (
                "20240101000001".to_string(),
                "ALTER TABLE test_users ADD COLUMN status INT DEFAULT 1 COMMENT '状态'".to_string(),
            ),
            (
                "20240101000002".to_string(),
                "ALTER TABLE test_users ADD INDEX idx_email (email)".to_string(),
            ),
        ]
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
                user_id INT NOT NULL COMMENT '用户ID',
                amount DECIMAL(10, 2) NOT NULL COMMENT '金额',
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间'
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='测试订单表'
            "#
        .to_string()]
    }

    fn migration_sql(&self) -> Vec<(String, String)> {
        vec![(
            "20240101000001".to_string(),
            "ALTER TABLE test_orders ADD COLUMN order_no VARCHAR(50) COMMENT '订单号'".to_string(),
        )]
    }
}

/// 测试插件 3（无迁移）
struct TestPlugin3;

#[async_trait::async_trait]
impl Plugin for TestPlugin3 {
    fn name(&self) -> &str {
        "test_plugin_3"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn init_sql(&self) -> Vec<String> {
        vec![r#"
            CREATE TABLE IF NOT EXISTS test_logs (
                id INT AUTO_INCREMENT PRIMARY KEY,
                message TEXT NOT NULL COMMENT '日志消息',
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间'
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='测试日志表'
            "#
        .to_string()]
    }
}

/// 创建 MySQL 测试容器并返回数据库 URL
async fn setup_mysql() -> Option<(testcontainers::ContainerAsync<GenericImage>, String)> {
    let mysql_image = GenericImage::new("mysql", "8.0")
        .with_env_var("MYSQL_ROOT_PASSWORD", "test_password")
        .with_env_var("MYSQL_DATABASE", "test_db");

    // 尝试启动容器
    let container = match mysql_image.start().await {
        Ok(c) => c,
        Err(e) => {
            println!("跳过测试：无法启动 Docker 容器: {}", e);
            return None;
        }
    };

    let port = container.get_host_port_ipv4(3306).await.ok()?;
    let db_url = format!("mysql://root:test_password@127.0.0.1:{}/test_db", port);

    // 等待 MySQL 完全启动
    if !wait_for_mysql(&db_url, 15).await {
        println!("跳过测试：MySQL 容器启动超时");
        return None;
    }

    Some((container, db_url))
}

/// 等待 MySQL 完全启动并可以接受连接
async fn wait_for_mysql(db_url: &str, max_retries: u32) -> bool {
    for i in 0..max_retries {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        if let Ok(db) = Database::connect(db_url).await {
            // 尝试执行一个简单的查询
            if db.execute("SELECT 1").await.is_ok() {
                println!("MySQL 已就绪");
                return true;
            }
        }

        if i < max_retries - 1 {
            println!("等待 MySQL 启动... (尝试 {}/{})", i + 1, max_retries);
        }
    }
    false
}

/// 测试显式数据库资源初始化
///
/// **验证需求**: 6.1, 6.4
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_explicit_database_initialization() {
    let (_container, db_url) = match setup_mysql().await {
        Some(setup) => setup,
        None => return,
    };

    // DatabaseConfig 为 #[non_exhaustive]：跨 crate 用 default() + 字段赋值构造。
    let mut config = DatabaseConfig::default();
    config.max_connections = 5;
    config.connect_timeout = 10;
    config.idle_timeout = 300;
    config.enable_logging = false;

    let database = Database::connect_with_config(&db_url, config)
        .await
        .expect("数据库应连接成功");
    let tools = ToolsBuilder::new()
        .database(database)
        .build()
        .expect("Tools 应构建成功");

    assert!(tools
        .db()
        .expect("数据库应存在")
        .health_check()
        .await
        .is_ok());
    assert!(tools
        .db()
        .expect("数据库应存在")
        .execute("SELECT 1")
        .await
        .is_ok());
}

/// 测试数据库初始化流程（非事务模式）
///
/// **验证需求**: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 11.2
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_database_initialization_without_transaction() {
    let (_container, db_url) = match setup_mysql().await {
        Some(setup) => setup,
        None => return,
    };

    // 连接数据库
    let db = Database::connect(&db_url).await.unwrap();

    // 创建插件管理器并注册插件
    let manager = PluginManager::new();
    manager.register(TestPlugin1).await.unwrap();
    manager.register(TestPlugin2).await.unwrap();
    manager.register(TestPlugin3).await.unwrap();

    // 创建数据库初始化器（非事务模式）
    let initializer = DatabaseInitializer::new(db, false);

    // 初始化所有插件的数据库
    let result = initializer.initialize_all(&manager).await;
    assert!(result.is_ok(), "数据库初始化失败: {:?}", result);

    // 重新连接数据库以进行验证
    let db = Database::connect(&db_url).await.unwrap();

    // 验证表已创建
    assert!(
        db.table_exists(yang_db::table!("test_users"))
            .await
            .unwrap(),
        "test_users 表未创建"
    );
    assert!(
        db.table_exists(yang_db::table!("test_orders"))
            .await
            .unwrap(),
        "test_orders 表未创建"
    );
    assert!(
        db.table_exists(yang_db::table!("test_logs")).await.unwrap(),
        "test_logs 表未创建"
    );
    assert!(
        db.table_exists(yang_db::table!("_migrations"))
            .await
            .unwrap(),
        "_migrations 表未创建"
    );

    // 验证迁移已执行
    let initializer = DatabaseInitializer::new(db, false);
    assert!(
        initializer
            .is_migration_executed("test_plugin_1", "20240101000001")
            .await
            .unwrap(),
        "test_plugin_1 的迁移 20240101000001 未执行"
    );
    assert!(
        initializer
            .is_migration_executed("test_plugin_1", "20240101000002")
            .await
            .unwrap(),
        "test_plugin_1 的迁移 20240101000002 未执行"
    );
    assert!(
        initializer
            .is_migration_executed("test_plugin_2", "20240101000001")
            .await
            .unwrap(),
        "test_plugin_2 的迁移 20240101000001 未执行"
    );
}

/// 测试数据库初始化流程（事务模式）
///
/// **验证需求**: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 11.3, 11.4
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_database_initialization_with_transaction() {
    let (_container, db_url) = match setup_mysql().await {
        Some(setup) => setup,
        None => return,
    };

    // 连接数据库
    let db = Database::connect(&db_url).await.unwrap();

    // 创建插件管理器并注册插件
    let manager = PluginManager::new();
    manager.register(TestPlugin1).await.unwrap();
    manager.register(TestPlugin2).await.unwrap();
    manager.register(TestPlugin3).await.unwrap();

    // 创建数据库初始化器（事务模式）
    let initializer = DatabaseInitializer::new(db, true);

    // 初始化所有插件的数据库
    let result = initializer.initialize_all(&manager).await;
    assert!(result.is_ok(), "数据库初始化失败: {:?}", result);

    // 重新连接数据库以进行验证
    let db = Database::connect(&db_url).await.unwrap();

    // 验证表已创建
    assert!(
        db.table_exists(yang_db::table!("test_users"))
            .await
            .unwrap(),
        "test_users 表未创建"
    );
    assert!(
        db.table_exists(yang_db::table!("test_orders"))
            .await
            .unwrap(),
        "test_orders 表未创建"
    );
    assert!(
        db.table_exists(yang_db::table!("test_logs")).await.unwrap(),
        "test_logs 表未创建"
    );
    assert!(
        db.table_exists(yang_db::table!("_migrations"))
            .await
            .unwrap(),
        "_migrations 表未创建"
    );

    // 验证迁移已执行
    let initializer = DatabaseInitializer::new(db, false);
    assert!(
        initializer
            .is_migration_executed("test_plugin_1", "20240101000001")
            .await
            .unwrap(),
        "test_plugin_1 的迁移 20240101000001 未执行"
    );
    assert!(
        initializer
            .is_migration_executed("test_plugin_2", "20240101000001")
            .await
            .unwrap(),
        "test_plugin_2 的迁移 20240101000001 未执行"
    );
}

/// 测试迁移记录表创建
///
/// **验证需求**: 9.2
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_migration_table_creation() {
    let (_container, db_url) = match setup_mysql().await {
        Some(setup) => setup,
        None => return,
    };

    // 连接数据库
    let db = Database::connect(&db_url).await.unwrap();

    // 创建数据库初始化器
    let initializer = DatabaseInitializer::new(db, false);

    // 创建迁移记录表
    let result = initializer.create_migration_table().await;
    assert!(result.is_ok(), "创建迁移记录表失败: {:?}", result);

    // 重新连接数据库以进行验证
    let db = Database::connect(&db_url).await.unwrap();

    // 验证表已创建
    assert!(
        db.table_exists(yang_db::table!("_migrations"))
            .await
            .unwrap(),
        "_migrations 表未创建"
    );

    // 验证表结构
    #[derive(sqlx::FromRow)]
    struct ColumnInfo {
        #[sqlx(rename = "Field")]
        field: String,
    }

    let columns: Vec<ColumnInfo> = db.query("DESCRIBE _migrations").await.unwrap();
    assert!(columns.len() >= 4, "迁移记录表字段数量不正确");

    // 验证必要字段存在
    let field_names: Vec<String> = columns.iter().map(|c| c.field.clone()).collect();
    assert!(field_names.contains(&"id".to_string()), "缺少 id 字段");
    assert!(
        field_names.contains(&"module_name".to_string()),
        "缺少 module_name 字段"
    );
    assert!(
        field_names.contains(&"version".to_string()),
        "缺少 version 字段"
    );
    assert!(
        field_names.contains(&"executed_at".to_string()),
        "缺少 executed_at 字段"
    );

    // 测试重复创建（幂等性）
    let initializer = DatabaseInitializer::new(db, false);
    let result = initializer.create_migration_table().await;
    assert!(result.is_ok(), "重复创建迁移记录表失败: {:?}", result);
}

/// 测试迁移执行和幂等性
///
/// **验证需求**: 9.3, 9.4
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_migration_execution_and_idempotency() {
    let (_container, db_url) = match setup_mysql().await {
        Some(setup) => setup,
        None => return,
    };

    // 连接数据库
    let db = Database::connect(&db_url).await.unwrap();

    // 创建插件管理器并注册插件
    let manager = PluginManager::new();
    manager.register(TestPlugin1).await.unwrap();

    // 创建数据库初始化器
    let initializer = DatabaseInitializer::new(db, false);

    // 第一次初始化
    let result = initializer.initialize_all(&manager).await;
    assert!(result.is_ok(), "第一次初始化失败: {:?}", result);

    // 验证迁移已执行
    assert!(
        initializer
            .is_migration_executed("test_plugin_1", "20240101000001")
            .await
            .unwrap(),
        "迁移未执行"
    );

    // 查询迁移记录数量
    #[derive(sqlx::FromRow)]
    struct CountResult {
        count: i64,
    }

    let db_verify = Database::connect(&db_url).await.unwrap();
    let sql = "SELECT COUNT(*) as count FROM _migrations WHERE module_name = 'test_plugin_1'";
    let results: Vec<CountResult> = db_verify.query(sql).await.unwrap();
    let first_count = results[0].count;
    assert_eq!(first_count, 2, "迁移记录数量不正确");

    // 第二次初始化（测试幂等性）
    let result = initializer.initialize_all(&manager).await;
    assert!(result.is_ok(), "第二次初始化失败: {:?}", result);

    // 验证迁移记录数量没有增加
    let results: Vec<CountResult> = db_verify.query(sql).await.unwrap();
    let second_count = results[0].count;
    assert_eq!(second_count, first_count, "迁移被重复执行，幂等性测试失败");

    // 第三次初始化（再次验证幂等性）
    let result = initializer.initialize_all(&manager).await;
    assert!(result.is_ok(), "第三次初始化失败: {:?}", result);

    let results: Vec<CountResult> = db_verify.query(sql).await.unwrap();
    let third_count = results[0].count;
    assert_eq!(third_count, first_count, "迁移被重复执行，幂等性测试失败");
}

/// 测试事务回滚（当初始化失败时）
///
/// **验证需求**: 11.3, 11.4
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_transaction_rollback_on_failure() {
    let (_container, db_url) = match setup_mysql().await {
        Some(setup) => setup,
        None => return,
    };

    // 连接数据库
    let db = Database::connect(&db_url).await.unwrap();

    // 定义一个会失败的插件
    struct FailingPlugin;

    #[async_trait::async_trait]
    impl Plugin for FailingPlugin {
        fn name(&self) -> &str {
            "failing_plugin"
        }

        fn init_sql(&self) -> Vec<String> {
            vec![
                // 第一个 SQL 正常
                "CREATE TABLE IF NOT EXISTS test_table1 (id INT PRIMARY KEY)".to_string(),
                // 第二个 SQL 故意错误（语法错误）
                "CREATE TABLE INVALID SQL SYNTAX".to_string(),
            ]
        }
    }

    // 创建插件管理器并注册插件
    let manager = PluginManager::new();
    manager.register(FailingPlugin).await.unwrap();

    // 创建数据库初始化器（事务模式）
    let initializer = DatabaseInitializer::new(db, true);

    // 初始化应该失败
    let result = initializer.initialize_all(&manager).await;
    assert!(result.is_err(), "初始化应该失败但成功了");

    // 重新连接数据库以进行验证
    let db = Database::connect(&db_url).await.unwrap();

    // 验证事务回滚：test_table1 不应该被创建
    let table_exists = db
        .table_exists(yang_db::table!("test_table1"))
        .await
        .unwrap();
    assert!(!table_exists, "事务回滚失败，test_table1 不应该存在");
}

/// 测试依赖顺序初始化
///
/// **验证需求**: 4.2, 4.3
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_dependency_order_initialization() {
    let (_container, db_url) = match setup_mysql().await {
        Some(setup) => setup,
        None => return,
    };

    // 连接数据库
    let db = Database::connect(&db_url).await.unwrap();

    // 创建插件管理器并注册插件（故意逆序注册）
    let manager = PluginManager::new();
    manager.register(TestPlugin2).await.unwrap(); // 依赖 TestPlugin1
    manager.register(TestPlugin1).await.unwrap();

    // 创建数据库初始化器
    let initializer = DatabaseInitializer::new(db, false);

    // 初始化应该成功（因为 PluginManager 会自动排序）
    let result = initializer.initialize_all(&manager).await;
    assert!(result.is_ok(), "依赖顺序初始化失败: {:?}", result);

    // 重新连接数据库以进行验证
    let db = Database::connect(&db_url).await.unwrap();

    // 验证两个表都已创建
    assert!(
        db.table_exists(yang_db::table!("test_users"))
            .await
            .unwrap(),
        "test_users 表未创建"
    );
    assert!(
        db.table_exists(yang_db::table!("test_orders"))
            .await
            .unwrap(),
        "test_orders 表未创建"
    );
}
