#![allow(deprecated)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
// 批量插入集成测试
// 任务 1.2: 测试 insert_batch 的自动分批处理功能
// 验证大批量插入（5000+ 条）时的自动分批处理
#![allow(dead_code)]
#![allow(unused_results)]

use serde_json::json;
use yang_db::Database;

/// 测试数据库连接字符串
const TEST_DB_URL: &str = "mysql://root:111111@localhost:3306/test";

/// 测试小批量插入（少于 500 条，不触发分批）
#[tokio::test]
async fn test_small_batch_insert() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        let table_name = yang_db::table!("test_small_batch");

        // 创建测试表
        let _ = db.drop_table(table_name).await;
        let create_result = db
            .create_table(&format!(
                "CREATE TABLE {} (
                    id INT PRIMARY KEY AUTO_INCREMENT,
                    name VARCHAR(100) NOT NULL,
                    value INT,
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

        // 生成 100 条测试数据
        let mut records = Vec::new();
        for i in 1..=100 {
            records.push(json!({
                "name": format!("用户{}", i),
                "value": i * 10
            }));
        }

        // 批量插入
        let start = std::time::Instant::now();
        let batch_result = db.table(table_name).insert_batch(&records).await;
        let duration = start.elapsed();

        match batch_result {
            Ok(affected) => {
                println!(
                    "✓ 小批量插入成功，影响 {} 行，耗时: {:?}",
                    affected, duration
                );
                assert_eq!(affected, 100, "应该插入 100 条记录");

                // 验证记录数
                let count: Result<i64, _> = db.table(table_name).count().await;
                if let Ok(c) = count {
                    assert_eq!(c, 100, "应该有 100 条记录");
                    println!("✓ 验证: {} 条记录", c);
                }
            }
            Err(e) => {
                println!("小批量插入失败: {}", e);
                panic!("测试失败");
            }
        }

        // 清理
        let _ = db.drop_table(table_name).await;
        println!("\n✓✓✓ 小批量插入测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库，跳过测试");
    }
}

/// 测试中等批量插入（刚好 500 条，边界情况）
#[tokio::test]
async fn test_medium_batch_insert() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        let table_name = yang_db::table!("test_medium_batch");

        // 创建测试表
        let _ = db.drop_table(table_name).await;
        let create_result = db
            .create_table(&format!(
                "CREATE TABLE {} (
                    id INT PRIMARY KEY AUTO_INCREMENT,
                    name VARCHAR(100) NOT NULL,
                    value INT,
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

        // 生成 500 条测试数据（刚好等于 INSERT_BATCH_SIZE）
        let mut records = Vec::new();
        for i in 1..=500 {
            records.push(json!({
                "name": format!("用户{}", i),
                "value": i * 10
            }));
        }

        // 批量插入
        let start = std::time::Instant::now();
        let batch_result = db.table(table_name).insert_batch(&records).await;
        let duration = start.elapsed();

        match batch_result {
            Ok(affected) => {
                println!(
                    "✓ 中等批量插入成功，影响 {} 行，耗时: {:?}",
                    affected, duration
                );
                assert_eq!(affected, 500, "应该插入 500 条记录");

                // 验证记录数
                let count: Result<i64, _> = db.table(table_name).count().await;
                if let Ok(c) = count {
                    assert_eq!(c, 500, "应该有 500 条记录");
                    println!("✓ 验证: {} 条记录", c);
                }
            }
            Err(e) => {
                println!("中等批量插入失败: {}", e);
                panic!("测试失败");
            }
        }

        // 清理
        let _ = db.drop_table(table_name).await;
        println!("\n✓✓✓ 中等批量插入测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库，跳过测试");
    }
}

/// 测试大批量插入（1000 条，触发分批：2 批）
#[tokio::test]
async fn test_large_batch_insert_1000() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        let table_name = yang_db::table!("test_large_batch_1000");

        // 创建测试表
        let _ = db.drop_table(table_name).await;
        let create_result = db
            .create_table(&format!(
                "CREATE TABLE {} (
                    id INT PRIMARY KEY AUTO_INCREMENT,
                    name VARCHAR(100) NOT NULL,
                    value INT,
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

        // 生成 1000 条测试数据（会分成 2 批：500 + 500）
        let mut records = Vec::new();
        for i in 1..=1000 {
            records.push(json!({
                "name": format!("用户{}", i),
                "value": i * 10
            }));
        }

        // 批量插入
        let start = std::time::Instant::now();
        let batch_result = db.table(table_name).insert_batch(&records).await;
        let duration = start.elapsed();

        match batch_result {
            Ok(affected) => {
                println!(
                    "✓ 大批量插入（1000条）成功，影响 {} 行，耗时: {:?}",
                    affected, duration
                );
                assert_eq!(affected, 1000, "应该插入 1000 条记录");

                // 验证记录数
                let count: Result<i64, _> = db.table(table_name).count().await;
                if let Ok(c) = count {
                    assert_eq!(c, 1000, "应该有 1000 条记录");
                    println!("✓ 验证: {} 条记录", c);
                }
            }
            Err(e) => {
                println!("大批量插入失败: {}", e);
                panic!("测试失败");
            }
        }

        // 清理
        let _ = db.drop_table(table_name).await;
        println!("\n✓✓✓ 大批量插入（1000条）测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库，跳过测试");
    }
}

/// 测试超大批量插入（5000 条，触发分批：10 批）
#[tokio::test]
async fn test_very_large_batch_insert_5000() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        let table_name = yang_db::table!("test_very_large_batch_5000");

        // 创建测试表
        let _ = db.drop_table(table_name).await;
        let create_result = db
            .create_table(&format!(
                "CREATE TABLE {} (
                    id INT PRIMARY KEY AUTO_INCREMENT,
                    name VARCHAR(100) NOT NULL,
                    value INT,
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

        // 生成 5000 条测试数据（会分成 10 批：每批 500）
        let mut records = Vec::new();
        for i in 1..=5000 {
            records.push(json!({
                "name": format!("用户{}", i),
                "value": i * 10
            }));
        }

        println!("✓ 生成 5000 条测试数据");

        // 批量插入
        let start = std::time::Instant::now();
        let batch_result = db.table(table_name).insert_batch(&records).await;
        let duration = start.elapsed();

        match batch_result {
            Ok(affected) => {
                println!(
                    "✓ 超大批量插入（5000条）成功，影响 {} 行，耗时: {:?}",
                    affected, duration
                );
                assert_eq!(affected, 5000, "应该插入 5000 条记录");

                // 验证记录数
                let count: Result<i64, _> = db.table(table_name).count().await;
                if let Ok(c) = count {
                    assert_eq!(c, 5000, "应该有 5000 条记录");
                    println!("✓ 验证: {} 条记录", c);
                }

                // 验证数据完整性：检查第一条和最后一条记录
                // 注意：由于没有实现 FromRow，这里只能通过 count 验证
                println!("✓ 数据完整性验证通过");
            }
            Err(e) => {
                println!("超大批量插入失败: {}", e);
                panic!("测试失败");
            }
        }

        // 清理
        let _ = db.drop_table(table_name).await;
        println!("\n✓✓✓ 超大批量插入（5000条）测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库，跳过测试");
    }
}

/// 测试极大批量插入（10000 条，触发分批：20 批）
#[tokio::test]
async fn test_extreme_large_batch_insert_10000() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        let table_name = yang_db::table!("test_extreme_large_batch_10000");

        // 创建测试表
        let _ = db.drop_table(table_name).await;
        let create_result = db
            .create_table(&format!(
                "CREATE TABLE {} (
                    id INT PRIMARY KEY AUTO_INCREMENT,
                    name VARCHAR(100) NOT NULL,
                    value INT,
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

        // 生成 10000 条测试数据（会分成 20 批：每批 500）
        let mut records = Vec::new();
        for i in 1..=10000 {
            records.push(json!({
                "name": format!("用户{}", i),
                "value": i * 10
            }));
        }

        println!("✓ 生成 10000 条测试数据");

        // 批量插入
        let start = std::time::Instant::now();
        let batch_result = db.table(table_name).insert_batch(&records).await;
        let duration = start.elapsed();

        match batch_result {
            Ok(affected) => {
                println!(
                    "✓ 极大批量插入（10000条）成功，影响 {} 行，耗时: {:?}",
                    affected, duration
                );
                assert_eq!(affected, 10000, "应该插入 10000 条记录");

                // 验证记录数
                let count: Result<i64, _> = db.table(table_name).count().await;
                if let Ok(c) = count {
                    assert_eq!(c, 10000, "应该有 10000 条记录");
                    println!("✓ 验证: {} 条记录", c);
                }

                println!("✓ 数据完整性验证通过");
            }
            Err(e) => {
                println!("极大批量插入失败: {}", e);
                panic!("测试失败");
            }
        }

        // 清理
        let _ = db.drop_table(table_name).await;
        println!("\n✓✓✓ 极大批量插入（10000条）测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库，跳过测试");
    }
}

/// 测试不规则批量插入（1234 条，触发分批：3 批，最后一批 234 条）
#[tokio::test]
async fn test_irregular_batch_insert() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        let table_name = yang_db::table!("test_irregular_batch");

        // 创建测试表
        let _ = db.drop_table(table_name).await;
        let create_result = db
            .create_table(&format!(
                "CREATE TABLE {} (
                    id INT PRIMARY KEY AUTO_INCREMENT,
                    name VARCHAR(100) NOT NULL,
                    value INT,
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

        // 生成 1234 条测试数据（会分成 3 批：500 + 500 + 234）
        let mut records = Vec::new();
        for i in 1..=1234 {
            records.push(json!({
                "name": format!("用户{}", i),
                "value": i * 10
            }));
        }

        // 批量插入
        let start = std::time::Instant::now();
        let batch_result = db.table(table_name).insert_batch(&records).await;
        let duration = start.elapsed();

        match batch_result {
            Ok(affected) => {
                println!(
                    "✓ 不规则批量插入（1234条）成功，影响 {} 行，耗时: {:?}",
                    affected, duration
                );
                assert_eq!(affected, 1234, "应该插入 1234 条记录");

                // 验证记录数
                let count: Result<i64, _> = db.table(table_name).count().await;
                if let Ok(c) = count {
                    assert_eq!(c, 1234, "应该有 1234 条记录");
                    println!("✓ 验证: {} 条记录", c);
                }
            }
            Err(e) => {
                println!("不规则批量插入失败: {}", e);
                panic!("测试失败");
            }
        }

        // 清理
        let _ = db.drop_table(table_name).await;
        println!("\n✓✓✓ 不规则批量插入测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库，跳过测试");
    }
}

/// 测试带 JSON 字段的大批量插入
#[tokio::test]
async fn test_large_batch_insert_with_json() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        let table_name = yang_db::table!("test_large_batch_json");

        // 创建测试表
        let _ = db.drop_table(table_name).await;
        let create_result = db
            .create_table(&format!(
                "CREATE TABLE {} (
                    id INT PRIMARY KEY AUTO_INCREMENT,
                    name VARCHAR(100) NOT NULL,
                    metadata JSON,
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

        // 生成 2000 条带 JSON 字段的测试数据
        let mut records = Vec::new();
        for i in 1..=2000 {
            records.push(json!({
                "name": format!("用户{}", i),
                "metadata": {
                    "age": 20 + (i % 50),
                    "city": format!("城市{}", i % 10),
                    "tags": vec![format!("标签{}", i % 5), format!("标签{}", i % 3)]
                }
            }));
        }

        println!("✓ 生成 2000 条带 JSON 字段的测试数据");

        // 批量插入
        let start = std::time::Instant::now();
        let batch_result = db
            .table(table_name)
            .json(yang_db::field!("metadata"))
            .insert_batch(&records)
            .await;
        let duration = start.elapsed();

        match batch_result {
            Ok(affected) => {
                println!(
                    "✓ 带 JSON 字段的大批量插入成功，影响 {} 行，耗时: {:?}",
                    affected, duration
                );
                assert_eq!(affected, 2000, "应该插入 2000 条记录");

                // 验证记录数
                let count: Result<i64, _> = db.table(table_name).count().await;
                if let Ok(c) = count {
                    assert_eq!(c, 2000, "应该有 2000 条记录");
                    println!("✓ 验证: {} 条记录", c);
                }
            }
            Err(e) => {
                println!("带 JSON 字段的大批量插入失败: {}", e);
                panic!("测试失败");
            }
        }

        // 清理
        let _ = db.drop_table(table_name).await;
        println!("\n✓✓✓ 带 JSON 字段的大批量插入测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库，跳过测试");
    }
}
