//! 数据库管理集成测试
//!
//! 使用 testcontainers 创建隔离的测试环境，测试：
//! - 全局数据库初始化
//! - 数据库初始化流程（事务和非事务模式）
//! - 迁移记录表创建
//! - 迁移执行和幂等性
//!
//! **验证需求**: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 6.1, 6.4, 9.2, 9.3, 9.4, 11.2, 11.3, 11.4
//!
//! **注意**: 这些测试需要 Docker 环境。如果没有 Docker，测试将被跳过。
//! 运行测试：`cargo test --test database_integration_test -- --test-threads=1`

use testcontainers::{clients::Cli, core::WaitFor, GenericImage};
use yang_base::database::{DatabaseInitializer, GlobalDatabase};
use yang_base::plugin::{Plugin, PluginManager};
use yang_db::{Database, DatabaseConfig};

/// 测试设置宏：创建 Docker 容器并等待 MySQL 启动
/// 返回 db_url 字符串
macro_rules! setup_test_db {
    ($docker:ident, $container:ident) => {{
        let $docker = Cli::default();
        let $container = match create_mysql_container(&$docker) {
            Some(c) => c,
            None => {
                println!("跳过测试：Docker 不可用");
                return;
            }
        };
        let db_url = get_db_url(&$container);

        // 等待 MySQL 完全启动
        if !wait_for_mysql(&db_url, 15).await {
            println!("跳过测试：MySQL 容器启动失败");
            return;
        }

        db_url
    }};
}

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

    fn dependencies(&self) -> Vec<&str> {
        vec!["test_plugin_1"]
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

/// 创建 MySQL 测试容器
///
/// 返回 None 如果 Docker 不可用
fn create_mysql_container(docker: &Cli) -> Option<testcontainers::Container<'_, GenericImage>> {
    let mysql_image = GenericImage::new("mysql", "8.0")
        .with_env_var("MYSQL_ROOT_PASSWORD", "test_password")
        .with_env_var("MYSQL_DATABASE", "test_db")
        .with_wait_for(WaitFor::message_on_stderr(
            "port: 3306  MySQL Community Server",
        ));

    // 尝试运行容器，如果失败则返回 None
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| docker.run(mysql_image))).ok()
}

/// 获取数据库连接字符串
fn get_db_url(container: &testcontainers::Container<'_, GenericImage>) -> String {
    let port = container.get_host_port_ipv4(3306);
    format!("mysql://root:test_password@127.0.0.1:{}/test_db", port)
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

/// 测试全局数据库初始化
///
/// **验证需求**: 6.1, 6.4
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_global_database_initialization() {
    let db_url = setup_test_db!(_docker, _container);

    // 初始化全局数据库
    let config = DatabaseConfig {
        max_connections: 5,
        connect_timeout: 10,
        idle_timeout: 300,
        enable_logging: false,
    };

    let result = GlobalDatabase::init(&db_url, config).await;
    assert!(result.is_ok(), "全局数据库初始化失败: {:?}", result);

    // 验证可以获取全局数据库实例
    let db = GlobalDatabase::get();
    assert!(db.is_ok(), "无法获取全局数据库实例");

    // 验证可以使用全局数据库执行查询
    let result = GlobalDatabase::execute("SELECT 1").await;
    assert!(result.is_ok(), "全局数据库查询失败: {:?}", result);
}

/// 测试数据库初始化流程（非事务模式）
///
/// **验证需求**: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 11.2
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_database_initialization_without_transaction() {
    let db_url = setup_test_db!(_docker, _container);

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
        db.table_exists("test_users").await.unwrap(),
        "test_users 表未创建"
    );
    assert!(
        db.table_exists("test_orders").await.unwrap(),
        "test_orders 表未创建"
    );
    assert!(
        db.table_exists("test_logs").await.unwrap(),
        "test_logs 表未创建"
    );
    assert!(
        db.table_exists("_migrations").await.unwrap(),
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
    let db_url = setup_test_db!(_docker, _container);

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
        db.table_exists("test_users").await.unwrap(),
        "test_users 表未创建"
    );
    assert!(
        db.table_exists("test_orders").await.unwrap(),
        "test_orders 表未创建"
    );
    assert!(
        db.table_exists("test_logs").await.unwrap(),
        "test_logs 表未创建"
    );
    assert!(
        db.table_exists("_migrations").await.unwrap(),
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
    let db_url = setup_test_db!(_docker, _container);

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
        db.table_exists("_migrations").await.unwrap(),
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
    let db_url = setup_test_db!(_docker, _container);

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
    let db_url = setup_test_db!(_docker, _container);

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
    let table_exists = db.table_exists("test_table1").await.unwrap();
    assert!(!table_exists, "事务回滚失败，test_table1 不应该存在");
}

/// 测试依赖顺序初始化
///
/// **验证需求**: 4.2, 4.3
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_dependency_order_initialization() {
    let db_url = setup_test_db!(_docker, _container);

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
        db.table_exists("test_users").await.unwrap(),
        "test_users 表未创建"
    );
    assert!(
        db.table_exists("test_orders").await.unwrap(),
        "test_orders 表未创建"
    );
}
