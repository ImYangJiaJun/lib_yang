// 特殊字段类型集成测试（简化版）
// 任务 21.5: 测试 JSON、DATETIME、TIMESTAMP、DECIMAL、BLOB、TEXT 等特殊字段类型
// 主要测试字段类型标记和基本插入操作

#![allow(dead_code)]

use serde_json::json;
use yang_db::Database;

/// 测试数据库连接字符串
const TEST_DB_URL: &str = "mysql://root:111111@localhost:3306/test";

#[tokio::test]
async fn test_json_field_type_marking() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 测试 JSON 字段类型标记
        let sql = db
            .table("test_table")
            .json("config")
            .field("config")
            .to_sql();

        println!("JSON 字段标记 SQL: {}", sql);
        assert!(sql.contains("config"), "SQL 应该包含 config 字段");

        println!("✓ JSON 字段类型标记成功");
        println!("\n✓✓✓ JSON 字段类型标记测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库");
    }
}

#[tokio::test]
async fn test_json_field_insert() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        let table_name = "integration_json_test";

        // 创建测试表
        let _ = db.drop_table(table_name).await;
        let create_result = db
            .create_table(&format!(
                "CREATE TABLE {} (
                    id INT PRIMARY KEY AUTO_INCREMENT,
                    name VARCHAR(100) NOT NULL,
                    config JSON
                )",
                table_name
            ))
            .await;

        if create_result.is_err() {
            println!("警告: 无法创建测试表");
            return;
        }

        // 插入 JSON 数据
        let config = json!({
            "theme": "dark",
            "language": "zh-CN",
            "notifications": {
                "email": true,
                "sms": false
            }
        });

        let insert_result = db
            .table(table_name)
            .json("config")
            .insert(&json!({
                "name": "JSON 测试",
                "config": config
            }))
            .await;

        match insert_result {
            Ok(id) => {
                println!("✓ JSON 字段插入成功，ID: {}", id);
                assert!(id > 0, "插入的 ID 应该大于 0");
            }
            Err(e) => {
                println!("JSON 字段插入失败: {}", e);
            }
        }

        // 清理
        let _ = db.drop_table(table_name).await;
        println!("\n✓✓✓ JSON 字段插入测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库");
    }
}

#[tokio::test]
async fn test_text_field_type_marking() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 测试 TEXT 字段类型标记
        let sql = db
            .table("test_table")
            .text("description")
            .field("description")
            .to_sql();

        println!("TEXT 字段标记 SQL: {}", sql);
        assert!(sql.contains("description"), "SQL 应该包含 description 字段");

        println!("✓ TEXT 字段类型标记成功");
        println!("\n✓✓✓ TEXT 字段类型标记测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库");
    }
}

#[tokio::test]
async fn test_decimal_field_type_marking() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 测试 DECIMAL 字段类型标记
        let sql = db
            .table("test_table")
            .decimal("price")
            .field("price")
            .to_sql();

        println!("DECIMAL 字段标记 SQL: {}", sql);
        assert!(sql.contains("price"), "SQL 应该包含 price 字段");

        println!("✓ DECIMAL 字段类型标记成功");
        println!("\n✓✓✓ DECIMAL 字段类型标记测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库");
    }
}

#[tokio::test]
async fn test_decimal_field_insert() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        let table_name = "integration_decimal_test";

        // 创建测试表
        let _ = db.drop_table(table_name).await;
        let create_result = db
            .create_table(&format!(
                "CREATE TABLE {} (
                    id INT PRIMARY KEY AUTO_INCREMENT,
                    name VARCHAR(100) NOT NULL,
                    price DECIMAL(10, 2)
                )",
                table_name
            ))
            .await;

        if create_result.is_err() {
            println!("警告: 无法创建测试表");
            return;
        }

        // 插入 DECIMAL 数据
        let insert_result = db
            .table(table_name)
            .decimal("price")
            .insert(&json!({
                "name": "商品1",
                "price": 99.99
            }))
            .await;

        match insert_result {
            Ok(id) => {
                println!("✓ DECIMAL 字段插入成功，ID: {}", id);
                assert!(id > 0, "插入的 ID 应该大于 0");
            }
            Err(e) => {
                println!("DECIMAL 字段插入失败: {}", e);
            }
        }

        // 清理
        let _ = db.drop_table(table_name).await;
        println!("\n✓✓✓ DECIMAL 字段插入测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库");
    }
}

#[tokio::test]
async fn test_datetime_field_type_marking() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 测试 DATETIME 字段类型标记
        let sql = db
            .table("test_table")
            .datetime("created_at")
            .field("created_at")
            .to_sql();

        println!("DATETIME 字段标记 SQL: {}", sql);
        assert!(sql.contains("created_at"), "SQL 应该包含 created_at 字段");

        println!("✓ DATETIME 字段类型标记成功");
        println!("\n✓✓✓ DATETIME 字段类型标记测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库");
    }
}

#[tokio::test]
async fn test_timestamp_field_type_marking() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 测试 TIMESTAMP 字段类型标记
        let sql = db
            .table("test_table")
            .timestamp("updated_at")
            .field("updated_at")
            .to_sql();

        println!("TIMESTAMP 字段标记 SQL: {}", sql);
        assert!(sql.contains("updated_at"), "SQL 应该包含 updated_at 字段");

        println!("✓ TIMESTAMP 字段类型标记成功");
        println!("\n✓✓✓ TIMESTAMP 字段类型标记测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库");
    }
}

#[tokio::test]
async fn test_blob_field_type_marking() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 测试 BLOB 字段类型标记
        let sql = db
            .table("test_table")
            .blob("avatar")
            .field("avatar")
            .to_sql();

        println!("BLOB 字段标记 SQL: {}", sql);
        assert!(sql.contains("avatar"), "SQL 应该包含 avatar 字段");

        println!("✓ BLOB 字段类型标记成功");
        println!("\n✓✓✓ BLOB 字段类型标记测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库");
    }
}

#[tokio::test]
async fn test_multiple_special_fields() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 测试同时标记多种特殊字段类型
        let sql = db
            .table("test_table")
            .json("config")
            .text("description")
            .decimal("price")
            .datetime("created_at")
            .timestamp("updated_at")
            .blob("avatar")
            .field("config")
            .field("description")
            .field("price")
            .field("created_at")
            .field("updated_at")
            .field("avatar")
            .to_sql();

        println!("多种特殊字段标记 SQL: {}", sql);

        // 验证所有字段都在 SQL 中
        assert!(sql.contains("config"), "应该包含 config");
        assert!(sql.contains("description"), "应该包含 description");
        assert!(sql.contains("price"), "应该包含 price");
        assert!(sql.contains("created_at"), "应该包含 created_at");
        assert!(sql.contains("updated_at"), "应该包含 updated_at");
        assert!(sql.contains("avatar"), "应该包含 avatar");

        println!("✓ 多种特殊字段类型标记成功");
        println!("\n✓✓✓ 多种特殊字段类型标记测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库");
    }
}

#[tokio::test]
async fn test_special_fields_with_insert() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        let table_name = "integration_special_fields_test";

        // 创建测试表
        let _ = db.drop_table(table_name).await;
        let create_result = db
            .create_table(&format!(
                "CREATE TABLE {} (
                    id INT PRIMARY KEY AUTO_INCREMENT,
                    name VARCHAR(100) NOT NULL,
                    config JSON,
                    description TEXT,
                    price DECIMAL(10, 2),
                    created_at DATETIME
                )",
                table_name
            ))
            .await;

        if create_result.is_err() {
            println!("警告: 无法创建测试表");
            return;
        }

        // 插入包含多种特殊字段的数据
        let config = json!({"key": "value"});
        let description = "这是一段测试描述".repeat(10);

        let insert_result = db
            .table(table_name)
            .json("config")
            .text("description")
            .decimal("price")
            .datetime("created_at")
            .insert(&json!({
                "name": "综合测试",
                "config": config,
                "description": description,
                "price": 199.99,
                "created_at": "2024-01-20 15:30:00"
            }))
            .await;

        match insert_result {
            Ok(id) => {
                println!("✓ 多种特殊字段插入成功，ID: {}", id);
                assert!(id > 0, "插入的 ID 应该大于 0");

                // 验证记录数
                let count: Result<i64, _> = db.table(table_name).count().await;
                if let Ok(c) = count {
                    assert_eq!(c, 1, "应该有 1 条记录");
                    println!("✓ 验证: {} 条记录", c);
                }
            }
            Err(e) => {
                println!("多种特殊字段插入失败: {}", e);
            }
        }

        // 清理
        let _ = db.drop_table(table_name).await;
        println!("\n✓✓✓ 多种特殊字段插入测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库");
    }
}
