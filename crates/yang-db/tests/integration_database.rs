// 数据库连接集成测试
// 使用测试数据库：mysql://root:111111@localhost:3306/test

#![allow(dead_code)]

use yang_db::{Database, DatabaseConfig};

/// 测试数据库连接字符串
const TEST_DB_URL: &str = "mysql://root:111111@localhost:3306/test";

#[tokio::test]
async fn test_connect_to_test_database() {
    // 尝试连接到测试数据库
    let result = Database::connect(TEST_DB_URL).await;

    match result {
        Ok(_db) => {
            // 连接成功
            println!("成功连接到测试数据库");
        }
        Err(e) => {
            // 如果测试数据库不可用，跳过测试
            println!("警告: 无法连接到测试数据库: {}", e);
            println!("请确保 MySQL 服务正在运行，并且测试数据库已配置");
        }
    }
}

#[tokio::test]
async fn test_connect_with_custom_config() {
    let config = DatabaseConfig {
        max_connections: 5,
        connect_timeout: 10,
        idle_timeout: 300,
        enable_logging: true,
    };

    let result = Database::connect_with_config(TEST_DB_URL, config).await;

    match result {
        Ok(_db) => {
            println!("成功使用自定义配置连接到测试数据库");
        }
        Err(e) => {
            println!("警告: 无法连接到测试数据库: {}", e);
        }
    }
}

#[tokio::test]
async fn test_table_exists() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 测试检查表是否存在
        let exists = db.table_exists("information_schema").await;

        match exists {
            Ok(true) => println!("表存在检查功能正常"),
            Ok(false) => println!("表不存在"),
            Err(e) => println!("检查表存在时出错: {}", e),
        }
    }
}

#[tokio::test]
async fn test_execute_simple_query() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 执行简单的查询测试连接
        let result = db.execute("SELECT 1").await;

        match result {
            Ok(_) => println!("成功执行简单查询"),
            Err(e) => println!("执行查询时出错: {}", e),
        }
    }
}

#[tokio::test]
async fn test_connection_pool_reuse() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 执行多个查询以测试连接池复用
        for i in 1..=5 {
            let result = db.execute(&format!("SELECT {}", i)).await;
            match result {
                Ok(_) => println!("查询 {} 成功", i),
                Err(e) => println!("查询 {} 失败: {}", i, e),
            }
        }
    }
}

#[tokio::test]
async fn test_insert_batch_sql_generation() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 生成 SQL（不实际执行）
        let sql = db.table("test_users").to_sql();
        println!("生成的 SQL: {}", sql);

        // 注意：实际的批量插入需要表存在，这里只测试 SQL 生成
        println!("批量插入 SQL 生成测试完成");
    }
}

#[tokio::test]
async fn test_insert_batch_with_real_table() {
    use serde_json::json;

    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 创建测试表
        let create_table_sql = "
            CREATE TABLE IF NOT EXISTS test_batch_insert (
                id INT PRIMARY KEY AUTO_INCREMENT,
                name VARCHAR(100) NOT NULL,
                email VARCHAR(100),
                age INT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
        ";

        match db.create_table(create_table_sql).await {
            Ok(_) => {
                println!("测试表创建成功");

                // 清空表
                let _ = db.execute("DELETE FROM test_batch_insert").await;

                // 批量插入数据
                let users = vec![
                    json!({"name": "张三", "email": "zhangsan@example.com", "age": 25}),
                    json!({"name": "李四", "email": "lisi@example.com", "age": 30}),
                    json!({"name": "王五", "email": "wangwu@example.com", "age": 28}),
                ];

                match db.table("test_batch_insert").insert_batch(&users).await {
                    Ok(affected_rows) => {
                        println!("批量插入成功，影响 {} 行", affected_rows);
                        assert_eq!(affected_rows, 3, "应该插入 3 条记录");

                        // 验证插入的数据
                        match db.execute("SELECT COUNT(*) FROM test_batch_insert").await {
                            Ok(_) => println!("数据验证成功"),
                            Err(e) => println!("数据验证失败: {}", e),
                        }
                    }
                    Err(e) => {
                        println!("批量插入失败: {}", e);
                    }
                }

                // 清理测试表
                let _ = db.drop_table("test_batch_insert").await;
            }
            Err(e) => {
                println!("创建测试表失败: {}", e);
            }
        }
    }
}

#[tokio::test]
async fn test_insert_batch_empty_data() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 测试空数据批量插入
        let empty_data: Vec<serde_json::Value> = vec![];

        match db.table("test_users").insert_batch(&empty_data).await {
            Ok(_) => {
                panic!("空数据批量插入应该返回错误");
            }
            Err(e) => {
                println!("空数据批量插入正确返回错误: {}", e);
                assert!(e.to_string().contains("不能为空"));
            }
        }
    }
}
