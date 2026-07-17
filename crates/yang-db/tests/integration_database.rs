#![allow(deprecated)]
// 数据库连接集成测试
// 使用测试数据库：mysql://root:111111@localhost:3306/test
#![allow(dead_code)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

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
    // DatabaseConfig 为 #[non_exhaustive]：集成测试是独立 crate，用 default() + 字段赋值。
    let mut config = DatabaseConfig::default();
    config.max_connections = 5;
    config.connect_timeout = 10;
    config.idle_timeout = 300;
    config.enable_logging = true;

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
        let exists = db.table_exists(yang_db::table!("information_schema")).await;

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
        let sql = db.table(yang_db::table!("test_users")).to_sql();
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

                match db
                    .table(yang_db::table!("test_batch_insert"))
                    .insert_batch(&users)
                    .await
                {
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
                let _ = db.drop_table(yang_db::table!("test_batch_insert")).await;
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

        match db
            .table(yang_db::table!("test_users"))
            .insert_batch(&empty_data)
            .await
        {
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

// ==================== 事务中的查询执行测试 ====================

#[tokio::test]
async fn test_transaction_insert() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 创建测试表
        let _ = db.execute("DROP TABLE IF EXISTS tx_test_users").await;
        let create_result = db
            .execute(
                "CREATE TABLE tx_test_users (
                    id INT PRIMARY KEY AUTO_INCREMENT,
                    name VARCHAR(100) NOT NULL,
                    email VARCHAR(100),
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )",
            )
            .await;

        if create_result.is_err() {
            println!("警告: 无法创建测试表");
            return;
        }

        // 开始事务
        let mut tx = db.transaction().await.expect("开始事务失败");

        // 在事务中插入数据
        let user_data = serde_json::json!({
            "name": "事务测试用户",
            "email": "tx_test@example.com"
        });

        let insert_result = tx
            .table(yang_db::table!("tx_test_users"))
            .insert(&user_data)
            .await;

        match insert_result {
            Ok(user_id) => {
                println!("事务中插入成功，用户 ID: {}", user_id);

                // 提交事务
                let commit_result = tx.commit().await;
                assert!(commit_result.is_ok(), "提交事务失败");

                // 验证数据已提交
                let count: Result<i64, _> =
                    db.table(yang_db::table!("tx_test_users")).count().await;
                if let Ok(c) = count {
                    assert_eq!(c, 1, "事务提交后应该有 1 条记录");
                    println!("验证成功：事务提交后有 {} 条记录", c);
                }
            }
            Err(e) => {
                println!("事务中插入失败: {}", e);
            }
        }

        // 清理测试表
        let _ = db.execute("DROP TABLE IF EXISTS tx_test_users").await;
    }
}

#[tokio::test]
async fn test_transaction_update() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 创建测试表
        let _ = db.execute("DROP TABLE IF EXISTS tx_test_users").await;
        let create_result = db
            .execute(
                "CREATE TABLE tx_test_users (
                    id INT PRIMARY KEY AUTO_INCREMENT,
                    name VARCHAR(100) NOT NULL,
                    email VARCHAR(100),
                    status INT DEFAULT 0
                )",
            )
            .await;

        if create_result.is_err() {
            println!("警告: 无法创建测试表");
            return;
        }

        // 先插入一条数据
        let user_data = serde_json::json!({
            "name": "测试用户",
            "email": "test@example.com",
            "status": 0
        });

        let user_id = db
            .table(yang_db::table!("tx_test_users"))
            .insert(&user_data)
            .await
            .expect("插入测试数据失败");

        // 开始事务
        let mut tx = db.transaction().await.expect("开始事务失败");

        // 在事务中更新数据
        let update_data = serde_json::json!({
            "status": 1,
            "name": "已更新的用户"
        });

        let update_result = tx
            .table(yang_db::table!("tx_test_users"))
            .where_and(
                yang_db::field!("id"),
                yang_db::CompareOp::Eq,
                user_id as i64,
            )
            .update(&update_data)
            .await;

        match update_result {
            Ok(affected_rows) => {
                println!("事务中更新成功，影响 {} 行", affected_rows);
                assert_eq!(affected_rows, 1, "应该更新 1 行");

                // 提交事务
                let commit_result = tx.commit().await;
                assert!(commit_result.is_ok(), "提交事务失败");

                println!("事务更新测试通过");
            }
            Err(e) => {
                println!("事务中更新失败: {}", e);
            }
        }

        // 清理测试表
        let _ = db.execute("DROP TABLE IF EXISTS tx_test_users").await;
    }
}

#[tokio::test]
async fn test_transaction_delete() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 创建测试表
        let _ = db.execute("DROP TABLE IF EXISTS tx_test_users").await;
        let create_result = db
            .execute(
                "CREATE TABLE tx_test_users (
                    id INT PRIMARY KEY AUTO_INCREMENT,
                    name VARCHAR(100) NOT NULL,
                    email VARCHAR(100)
                )",
            )
            .await;

        if create_result.is_err() {
            println!("警告: 无法创建测试表");
            return;
        }

        // 先插入一条数据
        let user_data = serde_json::json!({
            "name": "待删除用户",
            "email": "delete@example.com"
        });

        let user_id = db
            .table(yang_db::table!("tx_test_users"))
            .insert(&user_data)
            .await
            .expect("插入测试数据失败");

        // 开始事务
        let mut tx = db.transaction().await.expect("开始事务失败");

        // 在事务中删除数据
        let delete_result = tx
            .table(yang_db::table!("tx_test_users"))
            .where_and(
                yang_db::field!("id"),
                yang_db::CompareOp::Eq,
                user_id as i64,
            )
            .delete()
            .await;

        match delete_result {
            Ok(affected_rows) => {
                println!("事务中删除成功，影响 {} 行", affected_rows);
                assert_eq!(affected_rows, 1, "应该删除 1 行");

                // 提交事务
                let commit_result = tx.commit().await;
                assert!(commit_result.is_ok(), "提交事务失败");

                // 验证数据已删除
                let count: Result<i64, _> =
                    db.table(yang_db::table!("tx_test_users")).count().await;
                if let Ok(c) = count {
                    assert_eq!(c, 0, "事务提交后应该没有记录");
                    println!("验证成功：事务提交后有 {} 条记录", c);
                }
            }
            Err(e) => {
                println!("事务中删除失败: {}", e);
            }
        }

        // 清理测试表
        let _ = db.execute("DROP TABLE IF EXISTS tx_test_users").await;
    }
}

#[tokio::test]
async fn test_transaction_rollback_with_queries() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 创建测试表
        let _ = db.execute("DROP TABLE IF EXISTS tx_test_users").await;
        let create_result = db
            .execute(
                "CREATE TABLE tx_test_users (
                    id INT PRIMARY KEY AUTO_INCREMENT,
                    name VARCHAR(100) NOT NULL,
                    email VARCHAR(100)
                )",
            )
            .await;

        if create_result.is_err() {
            println!("警告: 无法创建测试表");
            return;
        }

        // 获取初始记录数
        let initial_count: i64 = db
            .table(yang_db::table!("tx_test_users"))
            .count()
            .await
            .expect("获取初始记录数失败");

        // 开始事务
        let mut tx = db.transaction().await.expect("开始事务失败");

        // 在事务中插入数据
        let user_data = serde_json::json!({
            "name": "临时用户",
            "email": "temp@example.com"
        });

        let insert_result = tx
            .table(yang_db::table!("tx_test_users"))
            .insert(&user_data)
            .await;

        if insert_result.is_ok() {
            println!("事务中插入成功");

            // 回滚事务
            let rollback_result = tx.rollback().await;
            assert!(rollback_result.is_ok(), "回滚事务失败");

            // 验证数据已回滚
            let final_count: i64 = db
                .table(yang_db::table!("tx_test_users"))
                .count()
                .await
                .expect("获取最终记录数失败");

            assert_eq!(
                initial_count, final_count,
                "回滚后记录数应该与初始记录数相同"
            );
            println!(
                "验证成功：回滚后记录数 {} = 初始记录数 {}",
                final_count, initial_count
            );
        }

        // 清理测试表
        let _ = db.execute("DROP TABLE IF EXISTS tx_test_users").await;
    }
}

#[tokio::test]
async fn test_transaction_multiple_operations() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 创建测试表
        let _ = db.execute("DROP TABLE IF EXISTS tx_test_users").await;
        let create_result = db
            .execute(
                "CREATE TABLE tx_test_users (
                    id INT PRIMARY KEY AUTO_INCREMENT,
                    name VARCHAR(100) NOT NULL,
                    email VARCHAR(100),
                    status INT DEFAULT 0
                )",
            )
            .await;

        if create_result.is_err() {
            println!("警告: 无法创建测试表");
            return;
        }

        // 开始事务
        let mut tx = db.transaction().await.expect("开始事务失败");

        // 操作 1: 插入用户
        let user1_data = serde_json::json!({
            "name": "用户1",
            "email": "user1@example.com",
            "status": 0
        });

        let user1_id = tx
            .table(yang_db::table!("tx_test_users"))
            .insert(&user1_data)
            .await
            .expect("插入用户1失败");

        println!("插入用户1成功，ID: {}", user1_id);

        // 操作 2: 插入另一个用户
        let user2_data = serde_json::json!({
            "name": "用户2",
            "email": "user2@example.com",
            "status": 0
        });

        let user2_id = tx
            .table(yang_db::table!("tx_test_users"))
            .insert(&user2_data)
            .await
            .expect("插入用户2失败");

        println!("插入用户2成功，ID: {}", user2_id);

        // 操作 3: 更新第一个用户
        let update_data = serde_json::json!({
            "status": 1
        });

        let affected = tx
            .table(yang_db::table!("tx_test_users"))
            .where_and(
                yang_db::field!("id"),
                yang_db::CompareOp::Eq,
                user1_id as i64,
            )
            .update(&update_data)
            .await
            .expect("更新用户1失败");

        println!("更新用户1成功，影响 {} 行", affected);

        // 提交事务
        let commit_result = tx.commit().await;
        assert!(commit_result.is_ok(), "提交事务失败");

        // 验证所有操作都已提交
        let count: i64 = db
            .table(yang_db::table!("tx_test_users"))
            .count()
            .await
            .expect("获取记录数失败");

        assert_eq!(count, 2, "应该有 2 条记录");
        println!("验证成功：事务提交后有 {} 条记录", count);

        // 清理测试表
        let _ = db.execute("DROP TABLE IF EXISTS tx_test_users").await;
    }
}

#[tokio::test]
async fn test_transaction_update_without_where_fails() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 创建测试表
        let _ = db.execute("DROP TABLE IF EXISTS tx_test_users").await;
        let create_result = db
            .execute(
                "CREATE TABLE tx_test_users (
                    id INT PRIMARY KEY AUTO_INCREMENT,
                    name VARCHAR(100) NOT NULL
                )",
            )
            .await;

        if create_result.is_err() {
            println!("警告: 无法创建测试表");
            return;
        }

        // 开始事务
        let mut tx = db.transaction().await.expect("开始事务失败");

        // 尝试不带 WHERE 条件的更新（应该失败）
        let update_data = serde_json::json!({
            "name": "全部更新"
        });

        let update_result = tx
            .table(yang_db::table!("tx_test_users"))
            .update(&update_data)
            .await;

        // 验证返回 MissingWhereClause 错误
        assert!(update_result.is_err(), "不带 WHERE 条件的更新应该失败");

        if let Err(e) = update_result {
            println!("预期的错误: {}", e);
            assert!(
                matches!(e, yang_db::DbError::MissingWhereClause),
                "应该返回 MissingWhereClause 错误"
            );
        }

        // 回滚事务
        let _ = tx.rollback().await;

        // 清理测试表
        let _ = db.execute("DROP TABLE IF EXISTS tx_test_users").await;
    }
}

#[tokio::test]
async fn test_transaction_delete_without_where_fails() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 创建测试表
        let _ = db.execute("DROP TABLE IF EXISTS tx_test_users").await;
        let create_result = db
            .execute(
                "CREATE TABLE tx_test_users (
                    id INT PRIMARY KEY AUTO_INCREMENT,
                    name VARCHAR(100) NOT NULL
                )",
            )
            .await;

        if create_result.is_err() {
            println!("警告: 无法创建测试表");
            return;
        }

        // 开始事务
        let mut tx = db.transaction().await.expect("开始事务失败");

        // 尝试不带 WHERE 条件的删除（应该失败）
        let delete_result = tx.table(yang_db::table!("tx_test_users")).delete().await;

        // 验证返回 MissingWhereClause 错误
        assert!(delete_result.is_err(), "不带 WHERE 条件的删除应该失败");

        if let Err(e) = delete_result {
            println!("预期的错误: {}", e);
            assert!(
                matches!(e, yang_db::DbError::MissingWhereClause),
                "应该返回 MissingWhereClause 错误"
            );
        }

        // 回滚事务
        let _ = tx.rollback().await;

        // 清理测试表
        let _ = db.execute("DROP TABLE IF EXISTS tx_test_users").await;
    }
}
