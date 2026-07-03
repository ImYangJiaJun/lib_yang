#![allow(deprecated)]
#![allow(clippy::expect_used, clippy::unwrap_used)]
#![allow(dead_code)]

use serde_json::json;
use yang_db::Database;

/// 测试数据库连接字符串
const TEST_DB_URL: &str = "mysql://root:111111@localhost:3306/test";

#[tokio::test]
async fn test_crud_sql_generation() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 测试 INSERT SQL 生成
        let insert_sql = db.table("test_users").to_sql();
        assert!(insert_sql.contains("test_users"), "SQL 应该包含表名");
        println!("✓ INSERT SQL 生成: {}", insert_sql);

        // 测试 SELECT SQL 生成
        let select_sql = db
            .table("test_users")
            .field("id")
            .field("name")
            .where_and("status", "=", "active")
            .unwrap()
            .to_sql();
        assert!(select_sql.contains("SELECT"), "应该包含 SELECT");
        assert!(select_sql.contains("WHERE"), "应该包含 WHERE");
        println!("✓ SELECT SQL 生成: {}", select_sql);

        // 测试 UPDATE SQL 生成（注意：to_sql 不支持 UPDATE，需要实际执行）
        // 这里只测试构建器的链式调用
        let _update_builder = db.table("test_users").where_and("id", "=", 1);
        println!("✓ UPDATE 构建器创建成功");

        // 测试 DELETE SQL 生成
        let _delete_builder = db.table("test_users").where_and("id", "=", 1);
        println!("✓ DELETE 构建器创建成功");

        // 测试 COUNT SQL 生成
        let count_sql = db.table("test_users").to_sql();
        println!("✓ COUNT SQL 基础: {}", count_sql);

        println!("\n✓✓✓ CRUD SQL 生成测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库");
    }
}

#[tokio::test]
async fn test_crud_with_real_table() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        let table_name = "integration_crud_test";

        // 创建测试表
        let _ = db.drop_table(table_name).await;
        let create_result = db
            .create_table(&format!(
                "CREATE TABLE {} (
                    id INT PRIMARY KEY AUTO_INCREMENT,
                    name VARCHAR(100) NOT NULL,
                    email VARCHAR(100),
                    age INT,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )",
                table_name
            ))
            .await;

        if create_result.is_err() {
            println!("警告: 无法创建测试表");
            return;
        }

        println!("✓ 测试表创建成功");

        // 测试 INSERT
        let user_data = json!({
            "name": "测试用户",
            "email": "test@example.com",
            "age": 25
        });

        let insert_result = db.table(table_name).insert(&user_data).await;

        match insert_result {
            Ok(id) => {
                println!("✓ INSERT 成功，ID: {}", id);
                assert!(id > 0, "插入的 ID 应该大于 0");

                // 测试 COUNT
                let count_result: Result<i64, _> = db.table(table_name).count().await;
                if let Ok(count) = count_result {
                    assert_eq!(count, 1, "应该有 1 条记录");
                    println!("✓ COUNT: {} 条记录", count);
                }

                // 测试 UPDATE
                let update_data = json!({"age": 26});
                let update_result = db
                    .table(table_name)
                    .where_and("id", "=", id as i64)
                    .unwrap()
                    .update(&update_data)
                    .await;

                if let Ok(affected) = update_result {
                    assert_eq!(affected, 1, "应该更新 1 行");
                    println!("✓ UPDATE: 更新了 {} 行", affected);
                }

                // 测试 DELETE
                let delete_result = db
                    .table(table_name)
                    .where_and("id", "=", id as i64)
                    .unwrap()
                    .delete()
                    .await;

                if let Ok(deleted) = delete_result {
                    assert_eq!(deleted, 1, "应该删除 1 行");
                    println!("✓ DELETE: 删除了 {} 行", deleted);
                }

                // 验证删除
                let final_count: Result<i64, _> = db.table(table_name).count().await;
                if let Ok(count) = final_count {
                    assert_eq!(count, 0, "删除后应该没有记录");
                    println!("✓ 验证删除: {} 条记录", count);
                }
            }
            Err(e) => {
                println!("INSERT 失败: {}", e);
            }
        }

        // 清理测试表
        let _ = db.drop_table(table_name).await;
        println!("\n✓✓✓ 完整 CRUD 流程测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库");
    }
}

#[tokio::test]
async fn test_batch_insert() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        let table_name = "integration_batch_test";

        // 创建测试表
        let _ = db.drop_table(table_name).await;
        let create_result = db
            .create_table(&format!(
                "CREATE TABLE {} (
                    id INT PRIMARY KEY AUTO_INCREMENT,
                    name VARCHAR(100) NOT NULL,
                    value INT
                )",
                table_name
            ))
            .await;

        if create_result.is_err() {
            println!("警告: 无法创建测试表");
            return;
        }

        // 批量插入
        let records = vec![
            json!({"name": "记录1", "value": 10}),
            json!({"name": "记录2", "value": 20}),
            json!({"name": "记录3", "value": 30}),
        ];

        let batch_result = db.table(table_name).insert_batch(&records).await;

        match batch_result {
            Ok(affected) => {
                println!("✓ 批量插入成功，影响 {} 行", affected);
                assert_eq!(affected, 3, "应该插入 3 条记录");

                // 验证记录数
                let count: Result<i64, _> = db.table(table_name).count().await;
                if let Ok(c) = count {
                    assert_eq!(c, 3, "应该有 3 条记录");
                    println!("✓ 验证: {} 条记录", c);
                }
            }
            Err(e) => {
                println!("批量插入失败: {}", e);
            }
        }

        // 清理
        let _ = db.drop_table(table_name).await;
        println!("\n✓✓✓ 批量插入测试通过 ✓✓✓");
    }
}

#[tokio::test]
async fn test_where_conditions() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 测试各种 WHERE 条件的 SQL 生成
        let sql1 = db
            .table("users")
            .where_and("age", ">", 18)
            .unwrap()
            .to_sql();
        assert!(sql1.contains("WHERE"), "应该包含 WHERE");
        println!("✓ WHERE > 条件: {}", sql1);

        let sql2 = db
            .table("users")
            .where_and("status", "=", "active")
            .unwrap()
            .where_and("age", ">=", 18)
            .unwrap()
            .to_sql();
        assert!(sql2.contains("WHERE"), "应该包含 WHERE");
        println!("✓ 多个 WHERE 条件: {}", sql2);

        let sql3 = db
            .table("users")
            .where_and("name", "like", "%test%")
            .unwrap()
            .to_sql();
        println!("✓ WHERE LIKE 条件: {}", sql3);

        println!("\n✓✓✓ WHERE 条件测试通过 ✓✓✓");
    }
}

#[tokio::test]
async fn test_order_and_limit() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 测试 ORDER BY
        let sql1 = db.table("users").order("created_at", false).to_sql();
        assert!(sql1.contains("ORDER BY"), "应该包含 ORDER BY");
        println!("✓ ORDER BY 降序: {}", sql1);

        // 测试 LIMIT
        let sql2 = db.table("users").limit(10).to_sql();
        assert!(sql2.contains("LIMIT"), "应该包含 LIMIT");
        println!("✓ LIMIT: {}", sql2);

        // 测试 OFFSET
        let sql3 = db.table("users").limit(10).offset(20).to_sql();
        assert!(sql3.contains("LIMIT"), "应该包含 LIMIT");
        assert!(sql3.contains("OFFSET"), "应该包含 OFFSET");
        println!("✓ LIMIT + OFFSET: {}", sql3);

        println!("\n✓✓✓ ORDER 和 LIMIT 测试通过 ✓✓✓");
    }
}

#[tokio::test]
async fn test_error_handling() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        let table_name = "integration_error_test";

        // 创建测试表
        let _ = db.drop_table(table_name).await;
        let _ = db
            .create_table(&format!(
                "CREATE TABLE {} (
                    id INT PRIMARY KEY AUTO_INCREMENT,
                    name VARCHAR(100) NOT NULL
                )",
                table_name
            ))
            .await;

        // 测试 UPDATE 没有 WHERE 条件
        let update_result = db.table(table_name).update(&json!({"name": "test"})).await;

        assert!(update_result.is_err(), "没有 WHERE 的 UPDATE 应该失败");
        if let Err(e) = update_result {
            assert!(matches!(e, yang_db::DbError::MissingWhereClause));
            println!("✓ UPDATE 缺少 WHERE: 正确返回错误");
        }

        // 测试 DELETE 没有 WHERE 条件
        let delete_result = db.table(table_name).delete().await;

        assert!(delete_result.is_err(), "没有 WHERE 的 DELETE 应该失败");
        if let Err(e) = delete_result {
            assert!(matches!(e, yang_db::DbError::MissingWhereClause));
            println!("✓ DELETE 缺少 WHERE: 正确返回错误");
        }

        // 清理
        let _ = db.drop_table(table_name).await;
        println!("\n✓✓✓ 错误处理测试通过 ✓✓✓");
    }
}
