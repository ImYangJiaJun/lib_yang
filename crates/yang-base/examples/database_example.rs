//! 显式 Database 使用示例
//!
//! 演示如何把 Database 作为应用拥有的资源进行数据库操作
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example database_example
//! ```
//!
//! # 注意
//!
//! 此示例需要可用的 MySQL 数据库连接

#![allow(deprecated)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use yang_db::{Database, DatabaseConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Database 使用示例 ===\n");

    // 1. 创建当前应用拥有的数据库
    println!("1. 连接数据库...");
    let db_url = "mysql://root:password@localhost:3306/test_db";
    // DatabaseConfig 为 #[non_exhaustive]：跨 crate 不能用结构体字面量，
    // 改用 default() + 字段赋值 / 链式 setter 构造。
    let mut config = DatabaseConfig::default();
    config.max_connections = 10;
    config.connect_timeout = 30;
    config.idle_timeout = 600;
    config.enable_logging = true;

    let db = match Database::connect_with_config(db_url, config).await {
        Ok(database) => {
            println!("   ✓ 数据库初始化成功\n");
            database
        }
        Err(e) => {
            eprintln!("   ✗ 数据库初始化失败: {}", e);
            eprintln!("   提示：请确保 MySQL 服务正在运行，并且连接信息正确");
            return Ok(());
        }
    };

    // 2. 创建测试表
    println!("2. 创建测试表...");
    let create_table_sql = r#"
        CREATE TABLE IF NOT EXISTS users (
            id INT AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(100) NOT NULL,
            email VARCHAR(100) NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
    "#;

    match db.execute(create_table_sql).await {
        Ok(_) => println!("   ✓ 表创建成功\n"),
        Err(e) => {
            eprintln!("   ✗ 表创建失败: {}", e);
            return Ok(());
        }
    }

    // 3. 插入数据
    println!("3. 插入测试数据...");
    let insert_sql = r#"
        INSERT INTO users (name, email) VALUES 
        ('Alice', 'alice@example.com'),
        ('Bob', 'bob@example.com'),
        ('Charlie', 'charlie@example.com')
    "#;

    match db.execute(insert_sql).await {
        Ok(affected) => println!("   ✓ 插入 {} 条记录\n", affected),
        Err(e) => {
            eprintln!("   ✗ 插入失败: {}", e);
            return Ok(());
        }
    }

    // 4. 查询数据
    println!("4. 查询数据...");
    #[derive(Debug, sqlx::FromRow)]
    struct User {
        id: i32,
        name: String,
        email: String,
    }

    match db.query::<User>("SELECT id, name, email FROM users").await {
        Ok(users) => {
            println!("   ✓ 查询到 {} 条记录:", users.len());
            for user in users {
                println!(
                    "     - ID: {}, 姓名: {}, 邮箱: {}",
                    user.id, user.name, user.email
                );
            }
            println!();
        }
        Err(e) => {
            eprintln!("   ✗ 查询失败: {}", e);
            return Ok(());
        }
    }

    // 5. 使用事务
    println!("5. 使用事务更新数据...");
    match db.transaction().await {
        Ok(mut tx) => {
            // 在事务中执行多个操作
            if let Err(e) = tx
                .execute("UPDATE users SET email = 'alice.new@example.com' WHERE name = 'Alice'")
                .await
            {
                eprintln!("   ✗ 更新失败: {}", e);
                return Ok(());
            }

            if let Err(e) = tx
                .execute("INSERT INTO users (name, email) VALUES ('David', 'david@example.com')")
                .await
            {
                eprintln!("   ✗ 插入失败: {}", e);
                return Ok(());
            }

            // 提交事务
            if let Err(e) = tx.commit().await {
                eprintln!("   ✗ 事务提交失败: {}", e);
                return Ok(());
            }

            println!("   ✓ 事务执行成功\n");
        }
        Err(e) => {
            eprintln!("   ✗ 开始事务失败: {}", e);
            return Ok(());
        }
    }

    // 6. 验证事务结果
    println!("6. 验证事务结果...");
    match db
        .query::<User>("SELECT id, name, email FROM users ORDER BY id")
        .await
    {
        Ok(users) => {
            println!("   ✓ 当前数据库中的用户:");
            for user in users {
                println!(
                    "     - ID: {}, 姓名: {}, 邮箱: {}",
                    user.id, user.name, user.email
                );
            }
            println!();
        }
        Err(e) => {
            eprintln!("   ✗ 查询失败: {}", e);
            return Ok(());
        }
    }

    // 7. 清理测试数据
    println!("7. 清理测试数据...");
    match db.execute("DROP TABLE IF EXISTS users").await {
        Ok(_) => println!("   ✓ 测试表已删除\n"),
        Err(e) => {
            eprintln!("   ✗ 删除表失败: {}", e);
            return Ok(());
        }
    }

    println!("=== 示例执行完成 ===");

    Ok(())
}
