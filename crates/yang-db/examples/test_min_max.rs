// MIN 和 MAX 聚合函数手动测试
// 任务 8.2: 实现 MIN 和 MAX 聚合函数

use serde_json::json;
use yang_db::Database;

const TEST_DB_URL: &str = "mysql://root:111111@localhost:3306/test";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== MIN 和 MAX 聚合函数测试 ===\n");

    // 连接数据库
    let db = Database::connect(TEST_DB_URL).await?;
    println!("✓ 数据库连接成功\n");

    let table_name = "test_min_max";

    // 删除旧表（如果存在）
    let _ = db.drop_table(table_name).await;

    // 创建测试表
    db.create_table(&format!(
        "CREATE TABLE {} (
            id INT PRIMARY KEY AUTO_INCREMENT,
            name VARCHAR(100) NOT NULL,
            price DOUBLE,
            stock INT,
            rating FLOAT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
        table_name
    ))
    .await?;
    println!("✓ 测试表创建成功\n");

    // 插入测试数据
    let products = vec![
        json!({"name": "产品A", "price": 99.99, "stock": 10, "rating": 4.5}),
        json!({"name": "产品B", "price": 149.50, "stock": 5, "rating": 4.8}),
        json!({"name": "产品C", "price": 79.00, "stock": 20, "rating": 4.2}),
        json!({"name": "产品D", "price": 199.99, "stock": 3, "rating": 4.9}),
        json!({"name": "产品E", "price": 59.99, "stock": 15, "rating": 4.0}),
    ];

    for product in &products {
        db.table(table_name).insert(product).await?;
    }
    println!("✓ 插入 {} 条测试数据\n", products.len());

    // 测试 MIN - 浮点数类型
    println!("--- 测试 MIN(price) - 浮点数类型 ---");
    let min_price: Option<f64> = db.table(table_name).min("price").await?;
    println!("最低价格: {:?}", min_price);
    assert_eq!(min_price, Some(59.99), "最低价格应该是 59.99");
    println!("✓ MIN(price) 测试通过\n");

    // 测试 MAX - 浮点数类型
    println!("--- 测试 MAX(price) - 浮点数类型 ---");
    let max_price: Option<f64> = db.table(table_name).max("price").await?;
    println!("最高价格: {:?}", max_price);
    assert_eq!(max_price, Some(199.99), "最高价格应该是 199.99");
    println!("✓ MAX(price) 测试通过\n");

    // 测试 MIN - 整数类型
    println!("--- 测试 MIN(stock) - 整数类型 ---");
    let min_stock: Option<i32> = db.table(table_name).min("stock").await?;
    println!("最小库存: {:?}", min_stock);
    assert_eq!(min_stock, Some(3), "最小库存应该是 3");
    println!("✓ MIN(stock) 测试通过\n");

    // 测试 MAX - 整数类型
    println!("--- 测试 MAX(stock) - 整数类型 ---");
    let max_stock: Option<i32> = db.table(table_name).max("stock").await?;
    println!("最大库存: {:?}", max_stock);
    assert_eq!(max_stock, Some(20), "最大库存应该是 20");
    println!("✓ MAX(stock) 测试通过\n");

    // 测试 MIN - 字符串类型（字典序）
    println!("--- 测试 MIN(name) - 字符串类型 ---");
    let min_name: Option<String> = db.table(table_name).min("name").await?;
    println!("字典序最小名称: {:?}", min_name);
    assert_eq!(min_name, Some("产品A".to_string()), "字典序最小应该是产品A");
    println!("✓ MIN(name) 测试通过\n");

    // 测试 MAX - 字符串类型（字典序）
    println!("--- 测试 MAX(name) - 字符串类型 ---");
    let max_name: Option<String> = db.table(table_name).max("name").await?;
    println!("字典序最大名称: {:?}", max_name);
    assert_eq!(max_name, Some("产品E".to_string()), "字典序最大应该是产品E");
    println!("✓ MAX(name) 测试通过\n");

    // 测试 MIN 与 WHERE 条件组合
    println!("--- 测试 MIN 与 WHERE 条件组合 ---");
    let min_price_filtered: Option<f64> = db
        .table(table_name)
        .where_and("stock", ">", 5)?
        .min("price")
        .await?;
    println!("库存>5的最低价格: {:?}", min_price_filtered);
    assert_eq!(
        min_price_filtered,
        Some(59.99),
        "库存>5的最低价格应该是 59.99"
    );
    println!("✓ MIN 与 WHERE 组合测试通过\n");

    // 测试 MAX 与 WHERE 条件组合
    println!("--- 测试 MAX 与 WHERE 条件组合 ---");
    let max_rating_filtered: Option<f32> = db
        .table(table_name)
        .where_and("price", "<", 150.0)?
        .max("rating")
        .await?;
    println!("价格<150的最高评分: {:?}", max_rating_filtered);
    assert_eq!(
        max_rating_filtered,
        Some(4.8),
        "价格<150的最高评分应该是 4.8"
    );
    println!("✓ MAX 与 WHERE 组合测试通过\n");

    // 测试空结果集
    println!("--- 测试空结果集 ---");
    let min_empty: Option<f64> = db
        .table(table_name)
        .where_and("price", ">", 1000.0)?
        .min("price")
        .await?;
    println!("空结果集的 MIN: {:?}", min_empty);
    assert_eq!(min_empty, None, "空结果集应该返回 None");
    println!("✓ 空结果集测试通过\n");

    // 清理测试表
    db.drop_table(table_name).await?;
    println!("✓ 测试表清理完成\n");

    println!("=== 所有测试通过！ ===");

    Ok(())
}
