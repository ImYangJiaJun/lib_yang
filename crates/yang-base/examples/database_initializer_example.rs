//! DatabaseInitializer 使用示例
//!
//! 演示如何使用 DatabaseInitializer 初始化插件数据库
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example database_initializer_example
//! ```
//!
//! # 前置条件
//!
//! 1. 安装并启动 MySQL 数据库
//! 2. 创建测试数据库：`CREATE DATABASE test_db;`
//! 3. 修改连接字符串中的用户名和密码

use yang_base::database::DatabaseInitializer;
use yang_base::plugin::{Plugin, PluginManager};
use yang_db::Database;

/// 示例插件：用户管理
struct UserPlugin;

#[async_trait::async_trait]
impl Plugin for UserPlugin {
    fn name(&self) -> &str {
        "user_plugin"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn init_sql(&self) -> Vec<String> {
        vec![r#"
            CREATE TABLE IF NOT EXISTS users (
                id INT AUTO_INCREMENT PRIMARY KEY,
                username VARCHAR(255) NOT NULL UNIQUE,
                email VARCHAR(255) NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_username (username)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='用户表'
            "#
        .to_string()]
    }

    fn migration_sql(&self) -> Vec<(String, String)> {
        vec![
            (
                "20240101000001".to_string(),
                "ALTER TABLE users ADD COLUMN status INT DEFAULT 1 COMMENT '用户状态'".to_string(),
            ),
            (
                "20240101000002".to_string(),
                "ALTER TABLE users ADD COLUMN last_login TIMESTAMP NULL COMMENT '最后登录时间'"
                    .to_string(),
            ),
        ]
    }

    async fn on_init(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("✓ 用户插件初始化完成");
        Ok(())
    }
}

/// 示例插件：订单管理（依赖用户插件）
struct OrderPlugin;

#[async_trait::async_trait]
impl Plugin for OrderPlugin {
    fn name(&self) -> &str {
        "order_plugin"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> &[&str] {
        &["user_plugin"]
    }

    fn init_sql(&self) -> Vec<String> {
        vec![r#"
            CREATE TABLE IF NOT EXISTS orders (
                id INT AUTO_INCREMENT PRIMARY KEY,
                user_id INT NOT NULL,
                order_no VARCHAR(255) NOT NULL UNIQUE,
                amount DECIMAL(10, 2) NOT NULL,
                status INT DEFAULT 0,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES users(id),
                INDEX idx_user_id (user_id),
                INDEX idx_order_no (order_no)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='订单表'
            "#
        .to_string()]
    }

    async fn on_init(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("✓ 订单插件初始化完成");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    env_logger::init();

    println!("========================================");
    println!("DatabaseInitializer 使用示例");
    println!("========================================\n");

    // 数据库连接字符串（请根据实际情况修改）
    let db_url = "mysql://root:password@localhost:3306/test_db";

    println!("1. 连接数据库...");
    let db = match Database::connect(db_url).await {
        Ok(db) => {
            println!("   ✓ 数据库连接成功\n");
            db
        }
        Err(e) => {
            eprintln!("   ✗ 数据库连接失败: {}", e);
            eprintln!("\n提示：");
            eprintln!("  1. 确保 MySQL 服务已启动");
            eprintln!("  2. 创建测试数据库：CREATE DATABASE test_db;");
            eprintln!("  3. 修改连接字符串中的用户名和密码");
            return Ok(());
        }
    };

    println!("2. 创建插件管理器并注册插件...");
    let manager = PluginManager::new();

    // 注册用户插件
    manager.register(UserPlugin).await?;
    println!("   ✓ 注册用户插件");

    // 注册订单插件
    manager.register(OrderPlugin).await?;
    println!("   ✓ 注册订单插件\n");

    println!("3. 创建数据库初始化器（事务模式）...");
    let initializer = DatabaseInitializer::new(db, true);
    println!("   ✓ 初始化器创建成功\n");

    println!("4. 初始化所有插件的数据库...");
    match initializer.initialize_all(&manager).await {
        Ok(_) => {
            println!("   ✓ 数据库初始化成功\n");
        }
        Err(e) => {
            eprintln!("   ✗ 数据库初始化失败: {}", e);
            return Err(e.into());
        }
    }

    println!("5. 验证表是否创建...");
    let db = Database::connect(db_url).await?;

    // 检查用户表
    if db.table_exists(yang_db::table!("users")).await? {
        println!("   ✓ users 表已创建");
    } else {
        println!("   ✗ users 表未创建");
    }

    // 检查订单表
    if db.table_exists(yang_db::table!("orders")).await? {
        println!("   ✓ orders 表已创建");
    } else {
        println!("   ✗ orders 表未创建");
    }

    // 检查迁移记录表
    if db.table_exists(yang_db::table!("_migrations")).await? {
        println!("   ✓ _migrations 表已创建");
    } else {
        println!("   ✗ _migrations 表未创建");
    }

    println!("\n========================================");
    println!("示例运行完成！");
    println!("========================================");

    println!("\n提示：");
    println!("  - 可以再次运行此示例，验证迁移的幂等性");
    println!("  - 使用 MySQL 客户端查看创建的表结构");
    println!("  - 查看 _migrations 表中的迁移记录");

    Ok(())
}
