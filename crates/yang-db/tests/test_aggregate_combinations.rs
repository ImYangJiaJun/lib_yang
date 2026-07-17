#![allow(deprecated)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
#![allow(dead_code)]
#![allow(unused_results)]
/// 测试聚合函数与其他子句的组合使用
///
/// 本测试文件验证 AVG、MIN、MAX 聚合函数与 WHERE、GROUP BY 子句的组合使用
#[allow(dead_code)]
use serde::{Deserialize, Serialize};
use serde_json::json;
use testcontainers::{runners::AsyncRunner, GenericImage, ImageExt};
use yang_db::Database;

/// 测试用户结构
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
#[allow(dead_code)]
struct User {
    id: i64,
    name: String,
    age: i32,
    status: i32,
}

/// 订单结构
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
#[allow(dead_code)]
struct Order {
    id: i64,
    user_id: i64,
    amount: f64,
    status: String,
}

/// 用户订单统计结构（用于 GROUP BY 查询）
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
struct UserOrderStats {
    user_id: i64,
    total_amount: f64,
    avg_amount: f64,
    min_amount: f64,
    max_amount: f64,
    order_count: i64,
}

/// 启动 MySQL 容器并返回连接 URL
async fn setup_mysql_container() -> Option<(testcontainers::ContainerAsync<GenericImage>, String)> {
    let mysql_image = GenericImage::new("mysql", "8.0")
        .with_env_var("MYSQL_ROOT_PASSWORD", "test_password")
        .with_env_var("MYSQL_DATABASE", "test_db");

    match mysql_image.start().await {
        Ok(container) => {
            let port = container.get_host_port_ipv4(3306).await.ok()?;
            let url = format!("mysql://root:test_password@127.0.0.1:{}/test_db", port);

            // 等待 MySQL 完全启动并可以接受连接
            if !wait_for_mysql(&url, 15).await {
                eprintln!("MySQL 容器启动超时");
                return None;
            }

            Some((container, url))
        }
        Err(e) => {
            eprintln!("无法启动 MySQL 容器: {}. 跳过集成测试。", e);
            None
        }
    }
}

/// 等待 MySQL 完全启动并可以接受连接
async fn wait_for_mysql(db_url: &str, max_retries: u32) -> bool {
    for i in 0..max_retries {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        match Database::connect(db_url).await {
            Ok(_) => {
                println!("MySQL 容器已就绪（尝试 {}/{}）", i + 1, max_retries);
                return true;
            }
            Err(e) => {
                if i < max_retries - 1 {
                    println!(
                        "等待 MySQL 启动...（尝试 {}/{}）: {}",
                        i + 1,
                        max_retries,
                        e
                    );
                }
            }
        }
    }
    false
}

/// 创建测试数据库和表
async fn setup_test_db(db: &Database) -> Result<(), yang_db::DbError> {
    // 创建 users 表
    db.execute(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id BIGINT PRIMARY KEY AUTO_INCREMENT,
            name VARCHAR(100) NOT NULL,
            age INT NOT NULL,
            status INT NOT NULL DEFAULT 1
        )
        "#,
    )
    .await?;

    // 创建 orders 表
    db.execute(
        r#"
        CREATE TABLE IF NOT EXISTS orders (
            id BIGINT PRIMARY KEY AUTO_INCREMENT,
            user_id BIGINT NOT NULL,
            amount DECIMAL(10, 2) NOT NULL,
            status VARCHAR(20) NOT NULL
        )
        "#,
    )
    .await?;

    // 插入测试用户数据
    let users = vec![
        ("张三", 25, 1),
        ("李四", 30, 1),
        ("王五", 28, 0),
        ("赵六", 35, 1),
        ("钱七", 22, 0),
    ];

    for (name, age, status) in users {
        db.table(yang_db::table!("users"))
            .insert(&json!({"name": name, "age": age, "status": status}))
            .await?;
    }

    // 插入测试订单数据
    let orders = vec![
        (1, 100.50, "completed"),
        (1, 200.00, "completed"),
        (1, 150.75, "pending"),
        (2, 300.00, "completed"),
        (2, 250.50, "completed"),
        (3, 180.00, "completed"),
        (3, 220.00, "cancelled"),
        (4, 500.00, "completed"),
    ];

    for (user_id, amount, status) in orders {
        db.table(yang_db::table!("orders"))
            .insert(&json!({"user_id": user_id, "amount": amount, "status": status}))
            .await?;
    }

    Ok(())
}

/// 测试 AVG 与 WHERE 子句组合
#[tokio::test]
async fn test_avg_with_where() {
    let Some((_container, url)) = setup_mysql_container().await else {
        eprintln!("跳过测试：无法启动 MySQL 容器");
        return;
    };

    let db = Database::connect(&url).await.unwrap();
    setup_test_db(&db).await.unwrap();

    // 测试：计算 status=1 的用户平均年龄
    let avg_age = db
        .table(yang_db::table!("users"))
        .where_and(yang_db::field!("status"), yang_db::CompareOp::Eq, 1)
        .avg(yang_db::field!("age"))
        .await
        .unwrap();

    assert!(avg_age.is_some());
    let avg = avg_age.unwrap();
    // status=1 的用户年龄：25, 30, 35，平均值 = 30
    assert!(
        (avg - 30.0).abs() < 0.01,
        "平均年龄应该是 30，实际: {}",
        avg
    );

    println!("✓ AVG 与 WHERE 组合测试通过");
}

/// 测试 MIN 与 WHERE 子句组合
#[tokio::test]
async fn test_min_with_where() {
    let Some((_container, url)) = setup_mysql_container().await else {
        eprintln!("跳过测试：无法启动 MySQL 容器");
        return;
    };

    let db = Database::connect(&url).await.unwrap();
    setup_test_db(&db).await.unwrap();

    // 测试：查询已完成订单的最小金额
    #[derive(Debug, sqlx::FromRow)]
    struct MinResult {
        min_amount: f64,
    }

    let result = db
        .table(yang_db::table!("orders"))
        .where_and(
            yang_db::field!("status"),
            yang_db::CompareOp::Eq,
            "completed",
        )
        .expr(
            yang_db::SelectExpr::min(yang_db::field!("amount"))
                .cast_double()
                .alias(yang_db::field!("min_amount")),
        )
        .select::<MinResult>()
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    let min = result[0].min_amount;
    // 已完成订单金额：100.50, 200.00, 300.00, 250.50, 180.00, 500.00
    // 最小值 = 100.50
    assert!(
        (min - 100.50).abs() < 0.01,
        "最小金额应该是 100.50，实际: {}",
        min
    );

    println!("✓ MIN 与 WHERE 组合测试通过");
}

/// 测试 MAX 与 WHERE 子句组合
#[tokio::test]
async fn test_max_with_where() {
    let Some((_container, url)) = setup_mysql_container().await else {
        eprintln!("跳过测试：无法启动 MySQL 容器");
        return;
    };

    let db = Database::connect(&url).await.unwrap();
    setup_test_db(&db).await.unwrap();

    // 测试：查询已完成订单的最大金额
    #[derive(Debug, sqlx::FromRow)]
    struct MaxResult {
        max_amount: f64,
    }

    let result = db
        .table(yang_db::table!("orders"))
        .where_and(
            yang_db::field!("status"),
            yang_db::CompareOp::Eq,
            "completed",
        )
        .expr(
            yang_db::SelectExpr::max(yang_db::field!("amount"))
                .cast_double()
                .alias(yang_db::field!("max_amount")),
        )
        .select::<MaxResult>()
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    let max = result[0].max_amount;
    // 已完成订单金额：100.50, 200.00, 300.00, 250.50, 180.00, 500.00
    // 最大值 = 500.00
    assert!(
        (max - 500.00).abs() < 0.01,
        "最大金额应该是 500.00，实际: {}",
        max
    );

    println!("✓ MAX 与 WHERE 组合测试通过");
}

/// 测试聚合函数与 GROUP BY 组合（使用原始 SQL）
#[tokio::test]
async fn test_aggregates_with_group_by() {
    let Some((_container, url)) = setup_mysql_container().await else {
        eprintln!("跳过测试：无法启动 MySQL 容器");
        return;
    };

    let db = Database::connect(&url).await.unwrap();
    setup_test_db(&db).await.unwrap();

    // 测试：按用户分组统计订单
    // 注意：由于 QueryBuilder 的聚合函数会清空 fields，我们需要使用原始 SQL 查询
    let stats = db
        .query::<UserOrderStats>(
            r#"
        SELECT 
            user_id,
            CAST(SUM(amount) AS DOUBLE) as total_amount,
            CAST(AVG(amount) AS DOUBLE) as avg_amount,
            CAST(MIN(amount) AS DOUBLE) as min_amount,
            CAST(MAX(amount) AS DOUBLE) as max_amount,
            COUNT(*) as order_count
        FROM orders
        WHERE status = 'completed'
        GROUP BY user_id
        ORDER BY user_id
        "#,
        )
        .await
        .unwrap();

    assert_eq!(stats.len(), 4, "应该有 4 个用户有已完成订单");

    // 验证用户 1 的统计数据
    let user1_stats = stats.iter().find(|s| s.user_id == 1).unwrap();
    assert_eq!(user1_stats.order_count, 2);
    assert!((user1_stats.total_amount - 300.50).abs() < 0.01);
    assert!((user1_stats.avg_amount - 150.25).abs() < 0.01);
    assert!((user1_stats.min_amount - 100.50).abs() < 0.01);
    assert!((user1_stats.max_amount - 200.00).abs() < 0.01);

    // 验证用户 2 的统计数据
    let user2_stats = stats.iter().find(|s| s.user_id == 2).unwrap();
    assert_eq!(user2_stats.order_count, 2);
    assert!((user2_stats.total_amount - 550.50).abs() < 0.01);

    println!("✓ 聚合函数与 GROUP BY 组合测试通过");
}

/// 测试多个聚合函数组合（使用 field() 方法）
#[tokio::test]
async fn test_multiple_aggregates() {
    let Some((_container, url)) = setup_mysql_container().await else {
        eprintln!("跳过测试：无法启动 MySQL 容器");
        return;
    };

    let db = Database::connect(&url).await.unwrap();
    setup_test_db(&db).await.unwrap();

    // 测试：使用 field() 方法添加多个聚合函数
    #[derive(Debug, sqlx::FromRow)]
    struct AggregateResult {
        avg_amount: f64,
        min_amount: f64,
        max_amount: f64,
    }

    let result = db
        .table(yang_db::table!("orders"))
        .where_and(
            yang_db::field!("status"),
            yang_db::CompareOp::Eq,
            "completed",
        )
        .expr(
            yang_db::SelectExpr::avg(yang_db::field!("amount"))
                .cast_double()
                .alias(yang_db::field!("avg_amount")),
        )
        .expr(
            yang_db::SelectExpr::min(yang_db::field!("amount"))
                .cast_double()
                .alias(yang_db::field!("min_amount")),
        )
        .expr(
            yang_db::SelectExpr::max(yang_db::field!("amount"))
                .cast_double()
                .alias(yang_db::field!("max_amount")),
        )
        .select::<AggregateResult>()
        .await
        .unwrap();

    assert_eq!(result.len(), 1, "应该返回一行聚合结果");

    let stats = &result[0];
    // 已完成订单金额：100.50, 200.00, 300.00, 250.50, 180.00, 500.00
    // 平均值 = (100.50 + 200.00 + 300.00 + 250.50 + 180.00 + 500.00) / 6 = 255.17
    assert!((stats.avg_amount - 255.17).abs() < 0.01, "平均金额计算错误");
    assert!((stats.min_amount - 100.50).abs() < 0.01, "最小金额计算错误");
    assert!((stats.max_amount - 500.00).abs() < 0.01, "最大金额计算错误");

    println!("✓ 多个聚合函数组合测试通过");
}

/// 测试 SQL 生成顺序正确性
#[tokio::test]
async fn test_sql_order_with_aggregates() {
    let Some((_container, url)) = setup_mysql_container().await else {
        eprintln!("跳过测试：无法启动 MySQL 容器");
        return;
    };

    let db = Database::connect(&url).await.unwrap();
    setup_test_db(&db).await.unwrap();

    // 测试：验证 WHERE 在聚合函数之前
    let avg_age = db
        .table(yang_db::table!("users"))
        .where_and(yang_db::field!("status"), yang_db::CompareOp::Eq, 1)
        .where_and(yang_db::field!("age"), yang_db::CompareOp::Gt, 25)
        .avg(yang_db::field!("age"))
        .await
        .unwrap();

    assert!(avg_age.is_some());
    let avg = avg_age.unwrap();
    // status=1 且 age>25 的用户：30, 35，平均值 = 32.5
    assert!(
        (avg - 32.5).abs() < 0.01,
        "平均年龄应该是 32.5，实际: {}",
        avg
    );

    println!("✓ SQL 顺序正确性测试通过");
}
