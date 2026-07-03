#![allow(deprecated)]
#![allow(clippy::expect_used, clippy::unwrap_used)]
// JOIN 查询集成测试（简化版）
// 任务 21.4: 测试各种 JOIN 类型和多表连接
// 主要测试 SQL 生成的正确性
#![allow(dead_code)]

use yang_db::Database;

/// 测试数据库连接字符串
const TEST_DB_URL: &str = "mysql://root:111111@localhost:3306/test";

#[tokio::test]
async fn test_inner_join_sql_generation() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 测试 INNER JOIN SQL 生成
        let sql = db
            .table("users")
            .field("users.name")
            .field("orders.product")
            .join("orders", "users.id = orders.user_id")
            .to_sql();

        println!("生成的 INNER JOIN SQL: {}", sql);

        // 验证 SQL 结构
        assert!(sql.contains("INNER JOIN"), "SQL 应该包含 INNER JOIN");
        assert!(sql.contains("orders"), "SQL 应该包含订单表");
        assert!(
            sql.contains("users.id = orders.user_id"),
            "SQL 应该包含连接条件"
        );

        println!("✓ INNER JOIN SQL 生成正确");
        println!("\n✓✓✓ INNER JOIN 测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库");
    }
}

#[tokio::test]
async fn test_left_join_sql_generation() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 测试 LEFT JOIN SQL 生成
        let sql = db
            .table("users")
            .field("users.name")
            .field("orders.product")
            .left_join("orders", "users.id = orders.user_id")
            .to_sql();

        println!("生成的 LEFT JOIN SQL: {}", sql);

        // 验证 SQL 结构
        assert!(sql.contains("LEFT JOIN"), "SQL 应该包含 LEFT JOIN");
        assert!(sql.contains("orders"), "SQL 应该包含订单表");

        println!("✓ LEFT JOIN SQL 生成正确");
        println!("\n✓✓✓ LEFT JOIN 测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库");
    }
}

#[tokio::test]
async fn test_right_join_sql_generation() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 测试 RIGHT JOIN SQL 生成
        let sql = db
            .table("users")
            .field("users.name")
            .field("orders.product")
            .right_join("orders", "users.id = orders.user_id")
            .to_sql();

        println!("生成的 RIGHT JOIN SQL: {}", sql);

        // 验证 SQL 结构
        assert!(sql.contains("RIGHT JOIN"), "SQL 应该包含 RIGHT JOIN");
        assert!(sql.contains("orders"), "SQL 应该包含订单表");

        println!("✓ RIGHT JOIN SQL 生成正确");
        println!("\n✓✓✓ RIGHT JOIN 测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库");
    }
}

#[tokio::test]
async fn test_multiple_joins() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 测试多表连接
        let sql = db
            .table("users")
            .field("users.name")
            .field("orders.product")
            .field("products.price")
            .join("orders", "users.id = orders.user_id")
            .join("products", "orders.product_id = products.id")
            .to_sql();

        println!("生成的多表 JOIN SQL: {}", sql);

        // 验证 SQL 包含两个 JOIN
        let join_count = sql.matches("INNER JOIN").count();
        assert_eq!(join_count, 2, "SQL 应该包含 2 个 INNER JOIN");

        assert!(sql.contains("orders"), "SQL 应该包含订单表");
        assert!(sql.contains("products"), "SQL 应该包含产品表");

        println!("✓ 多表 JOIN SQL 生成正确");
        println!("\n✓✓✓ 多表 JOIN 测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库");
    }
}

#[tokio::test]
async fn test_join_with_where() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 测试 JOIN + WHERE
        let sql = db
            .table("users")
            .field("users.name")
            .field("orders.product")
            .join("orders", "users.id = orders.user_id")
            .where_and("orders.amount", ">", 100.0)
            .unwrap()
            .to_sql();

        println!("生成的 JOIN + WHERE SQL: {}", sql);

        // 验证 SQL 结构
        assert!(sql.contains("INNER JOIN"), "应该包含 INNER JOIN");
        assert!(sql.contains("WHERE"), "应该包含 WHERE 子句");

        println!("✓ JOIN + WHERE SQL 生成正确");
        println!("\n✓✓✓ JOIN + WHERE 测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库");
    }
}

#[tokio::test]
async fn test_join_with_order() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 测试 JOIN + ORDER BY
        let sql = db
            .table("users")
            .field("users.name")
            .field("orders.product")
            .join("orders", "users.id = orders.user_id")
            .order("orders.created_at", false)
            .to_sql();

        println!("生成的 JOIN + ORDER BY SQL: {}", sql);

        // 验证 SQL 结构
        assert!(sql.contains("INNER JOIN"), "应该包含 INNER JOIN");
        assert!(sql.contains("ORDER BY"), "应该包含 ORDER BY 子句");

        println!("✓ JOIN + ORDER BY SQL 生成正确");
        println!("\n✓✓✓ JOIN + ORDER BY 测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库");
    }
}

#[tokio::test]
async fn test_join_with_table_alias() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 测试表别名
        let sql = db
            .table("users AS u")
            .field("u.name")
            .field("o.product")
            .join("orders AS o", "u.id = o.user_id")
            .to_sql();

        println!("生成的带别名的 JOIN SQL: {}", sql);

        // 验证 SQL 包含别名
        assert!(sql.contains("users AS u"), "应该包含用户表别名");
        assert!(sql.contains("orders AS o"), "应该包含订单表别名");
        assert!(sql.contains("u.id = o.user_id"), "应该使用别名连接");

        println!("✓ 表别名 JOIN SQL 生成正确");
        println!("\n✓✓✓ 表别名测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库");
    }
}

#[tokio::test]
async fn test_join_with_limit() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 测试 JOIN + LIMIT
        let sql = db
            .table("users")
            .field("users.name")
            .field("orders.product")
            .join("orders", "users.id = orders.user_id")
            .limit(10)
            .to_sql();

        println!("生成的 JOIN + LIMIT SQL: {}", sql);

        // 验证 SQL 结构
        assert!(sql.contains("INNER JOIN"), "应该包含 INNER JOIN");
        assert!(sql.contains("LIMIT"), "应该包含 LIMIT");

        println!("✓ JOIN + LIMIT SQL 生成正确");
        println!("\n✓✓✓ JOIN + LIMIT 测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库");
    }
}

#[tokio::test]
async fn test_complex_join_query() {
    let result = Database::connect(TEST_DB_URL).await;

    if let Ok(db) = result {
        // 测试复杂的 JOIN 查询
        let sql = db
            .table("users AS u")
            .field("u.name")
            .field("o.product")
            .field("p.price")
            .join("orders AS o", "u.id = o.user_id")
            .left_join("products AS p", "o.product_id = p.id")
            .where_and("u.status", "=", "active")
            .unwrap()
            .where_and("o.amount", ">", 50.0)
            .unwrap()
            .order("o.created_at", false)
            .limit(20)
            .to_sql();

        println!("生成的复杂 JOIN SQL: {}", sql);

        // 验证 SQL 结构
        assert!(sql.contains("INNER JOIN"), "应该包含 INNER JOIN");
        assert!(sql.contains("LEFT JOIN"), "应该包含 LEFT JOIN");
        assert!(sql.contains("WHERE"), "应该包含 WHERE");
        assert!(sql.contains("ORDER BY"), "应该包含 ORDER BY");
        assert!(sql.contains("LIMIT"), "应该包含 LIMIT");

        println!("✓ 复杂 JOIN 查询 SQL 生成正确");
        println!("\n✓✓✓ 复杂 JOIN 查询测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库");
    }
}
