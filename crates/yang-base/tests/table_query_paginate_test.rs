//! TableQuery 分页查询集成测试
//!
//! 测试 paginate 方法的完整功能，包括：
//! - 执行 COUNT 查询获取总记录数
//! - 计算 LIMIT 和 OFFSET
//! - 执行数据查询
//! - 构建 PaginatedResult 返回结果
//!
//! **验证需求**: 5.7
//!
//! **注意**: 这些测试需要 Docker 环境。如果没有 Docker，测试将被跳过。
//! 运行测试：`cargo test --test table_query_paginate_test -- --test-threads=1 --ignored`

use serde::{Deserialize, Serialize};
use sqlx::mysql::MySqlPoolOptions;
use std::sync::Arc;
use std::time::Duration;
use testcontainers::{runners::AsyncRunner, GenericImage, ImageExt};
use yang_base::table::{
    FieldConfig, FieldType, PaginatedResult, SortOrder, TableConfig, TableQuery,
};
use yang_db::Database;

/// 测试用户结构
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, PartialEq)]
struct TestUser {
    id: i64,
    name: String,
    email: String,
    age: i32,
}

/// 创建 MySQL 测试容器并返回数据库 URL
async fn setup_mysql() -> Option<(testcontainers::ContainerAsync<GenericImage>, String)> {
    let mysql_image = GenericImage::new("mysql", "8.0")
        .with_env_var("MYSQL_ROOT_PASSWORD", "test_password")
        .with_env_var("MYSQL_DATABASE", "test_db");

    let container = match mysql_image.start().await {
        Ok(c) => c,
        Err(e) => {
            println!("跳过测试：无法启动 Docker 容器: {}", e);
            return None;
        }
    };

    let port = container.get_host_port_ipv4(3306).await.ok()?;
    let db_url = format!("mysql://root:test_password@127.0.0.1:{}/test_db", port);

    if !wait_for_mysql(&db_url, 15).await {
        println!("跳过测试：MySQL 容器启动超时");
        return None;
    }

    Some((container, db_url))
}

/// 等待 MySQL 完全启动
async fn wait_for_mysql(db_url: &str, max_retries: u32) -> bool {
    for i in 0..max_retries {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        if let Ok(db) = Database::connect(db_url).await {
            if db.execute("SELECT 1").await.is_ok() {
                println!("MySQL 已就绪");
                return true;
            }
        }

        if i < max_retries - 1 {
            println!("等待 MySQL 启动... (尝试 {}/{})", i + 1, max_retries);
        }
    }
    false
}

/// 创建测试表并插入测试数据
async fn setup_test_data(db: &Database) -> Result<(), Box<dyn std::error::Error>> {
    // 创建测试表
    db.execute(
        r#"
        CREATE TABLE IF NOT EXISTS test_users (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(50) NOT NULL,
            email VARCHAR(100) NOT NULL,
            age INT NOT NULL
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4
        "#,
    )
    .await?;

    // 插入测试数据（50 条记录）
    for i in 1..=50 {
        db.execute(&format!(
            "INSERT INTO test_users (name, email, age) VALUES ('User{}', 'user{}@example.com', {})",
            i,
            i,
            20 + (i % 30)
        ))
        .await?;
    }

    Ok(())
}

/// 创建测试用的表配置
fn create_test_table_config() -> Arc<TableConfig> {
    Arc::new(
        TableConfig::new("test_users")
            .field(FieldConfig::new("id", FieldType::BigInt))
            .field(FieldConfig::new(
                "name",
                FieldType::String { max_length: 50 },
            ))
            .field(FieldConfig::new(
                "email",
                FieldType::String { max_length: 100 },
            ))
            .field(FieldConfig::new("age", FieldType::Integer)),
    )
}

/// 测试基本分页查询
///
/// **验证需求**: 5.7
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_paginate_basic() {
    let (_container, db_url) = match setup_mysql().await {
        Some(setup) => setup,
        None => return,
    };

    // 创建数据库连接池
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&db_url)
        .await
        .unwrap();

    // 创建测试数据
    let db = Database::connect(&db_url).await.unwrap();
    setup_test_data(&db).await.unwrap();

    // 创建表配置和查询
    let table_config = create_test_table_config();

    let query = TableQuery::new(table_config, vec!["user".to_string()].into(), Some(Arc::new(pool)));

    // 执行分页查询：第 1 页，每页 10 条
    let result: PaginatedResult<TestUser> = query.page(1, 10).unwrap().paginate().await.unwrap();

    // 验证结果
    assert_eq!(result.total, 50, "总记录数应该是 50");
    assert_eq!(result.page, 1, "当前页应该是 1");
    assert_eq!(result.page_size, 10, "每页大小应该是 10");
    assert_eq!(result.total_pages, 5, "总页数应该是 5");
    assert_eq!(result.data.len(), 10, "当前页数据条数应该是 10");
    assert!(result.has_next(), "应该有下一页");
    assert!(!result.has_prev(), "不应该有上一页");
}

/// 测试分页查询第 2 页
///
/// **验证需求**: 5.7
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_paginate_second_page() {
    let (_container, db_url) = match setup_mysql().await {
        Some(setup) => setup,
        None => return,
    };

    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&db_url)
        .await
        .unwrap();

    let db = Database::connect(&db_url).await.unwrap();
    setup_test_data(&db).await.unwrap();

    let table_config = create_test_table_config();

    let query = TableQuery::new(table_config, vec!["user".to_string()].into(), Some(Arc::new(pool)));

    // 执行分页查询：第 2 页，每页 10 条
    let result: PaginatedResult<TestUser> = query.page(2, 10).unwrap().paginate().await.unwrap();

    // 验证结果
    assert_eq!(result.total, 50);
    assert_eq!(result.page, 2);
    assert_eq!(result.page_size, 10);
    assert_eq!(result.total_pages, 5);
    assert_eq!(result.data.len(), 10);
    assert!(result.has_next(), "应该有下一页");
    assert!(result.has_prev(), "应该有上一页");
}

/// 测试分页查询最后一页
///
/// **验证需求**: 5.7
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_paginate_last_page() {
    let (_container, db_url) = match setup_mysql().await {
        Some(setup) => setup,
        None => return,
    };

    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&db_url)
        .await
        .unwrap();

    let db = Database::connect(&db_url).await.unwrap();
    setup_test_data(&db).await.unwrap();

    let table_config = create_test_table_config();

    let query = TableQuery::new(table_config, vec!["user".to_string()].into(), Some(Arc::new(pool)));

    // 执行分页查询：第 5 页（最后一页），每页 10 条
    let result: PaginatedResult<TestUser> = query.page(5, 10).unwrap().paginate().await.unwrap();

    // 验证结果
    assert_eq!(result.total, 50);
    assert_eq!(result.page, 5);
    assert_eq!(result.page_size, 10);
    assert_eq!(result.total_pages, 5);
    assert_eq!(result.data.len(), 10);
    assert!(!result.has_next(), "不应该有下一页");
    assert!(result.has_prev(), "应该有上一页");
}

/// 测试分页查询空结果
///
/// **验证需求**: 5.7
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_paginate_empty_result() {
    let (_container, db_url) = match setup_mysql().await {
        Some(setup) => setup,
        None => return,
    };

    // 创建数据库连接池
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&db_url)
        .await
        .unwrap();

    let db = Database::connect(&db_url).await.unwrap();

    // 创建空表
    db.execute(
        r#"
        CREATE TABLE IF NOT EXISTS test_users (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(50) NOT NULL,
            email VARCHAR(100) NOT NULL,
            age INT NOT NULL
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4
        "#,
    )
    .await
    .unwrap();

    let table_config = create_test_table_config();

    let query = TableQuery::new(table_config, vec!["user".to_string()].into(), Some(Arc::new(pool)));

    // 执行分页查询
    let result: PaginatedResult<TestUser> = query.page(1, 10).unwrap().paginate().await.unwrap();

    // 验证结果
    assert_eq!(result.total, 0, "总记录数应该是 0");
    assert_eq!(result.page, 1);
    assert_eq!(result.page_size, 10);
    assert_eq!(result.total_pages, 0, "总页数应该是 0");
    assert_eq!(result.data.len(), 0, "数据列表应该为空");
    assert!(!result.has_next());
    assert!(!result.has_prev());
}

/// 测试分页查询带 WHERE 条件
///
/// **验证需求**: 5.7
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_paginate_with_where_condition() {
    let (_container, db_url) = match setup_mysql().await {
        Some(setup) => setup,
        None => return,
    };

    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&db_url)
        .await
        .unwrap();

    let db = Database::connect(&db_url).await.unwrap();
    setup_test_data(&db).await.unwrap();

    let table_config = create_test_table_config();

    let query = TableQuery::new(table_config, vec!["user".to_string()].into(), Some(Arc::new(pool)));

    // 执行分页查询：age = 25
    let result: PaginatedResult<TestUser> = query
        .where_eq("age", serde_json::json!(25))
        .unwrap()
        .page(1, 10)
        .unwrap()
        .paginate()
        .await
        .unwrap();

    // 验证结果：应该有一些记录（age = 25 的记录）
    assert!(result.total > 0, "应该有符合条件的记录");
    assert_eq!(result.page, 1);
    assert_eq!(result.page_size, 10);
    assert!(result.data.len() <= 10);

    // 验证所有返回的记录都满足条件
    for user in &result.data {
        assert_eq!(user.age, 25, "所有记录的 age 应该等于 25");
    }
}

/// 测试分页查询带排序
///
/// **验证需求**: 5.7
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_paginate_with_order_by() {
    let (_container, db_url) = match setup_mysql().await {
        Some(setup) => setup,
        None => return,
    };

    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&db_url)
        .await
        .unwrap();

    let db = Database::connect(&db_url).await.unwrap();
    setup_test_data(&db).await.unwrap();

    let table_config = create_test_table_config();

    let query = TableQuery::new(table_config, vec!["user".to_string()].into(), Some(Arc::new(pool)));

    // 执行分页查询：按 id 降序排列
    let result: PaginatedResult<TestUser> = query
        .order_by("id", SortOrder::Desc)
        .unwrap()
        .page(1, 10)
        .unwrap()
        .paginate()
        .await
        .unwrap();

    // 验证结果
    assert_eq!(result.total, 50);
    assert_eq!(result.data.len(), 10);

    // 验证排序：第一条记录的 id 应该是最大的
    assert_eq!(result.data[0].id, 50, "第一条记录的 id 应该是 50");
    assert_eq!(result.data[9].id, 41, "第十条记录的 id 应该是 41");

    // 验证降序排列
    for i in 0..result.data.len() - 1 {
        assert!(
            result.data[i].id > result.data[i + 1].id,
            "记录应该按 id 降序排列"
        );
    }
}

/// 测试分页查询带字段选择
///
/// **验证需求**: 5.7
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_paginate_with_field_selection() {
    let (_container, db_url) = match setup_mysql().await {
        Some(setup) => setup,
        None => return,
    };

    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&db_url)
        .await
        .unwrap();

    let db = Database::connect(&db_url).await.unwrap();
    setup_test_data(&db).await.unwrap();

    let table_config = create_test_table_config();

    let query = TableQuery::new(table_config, vec!["user".to_string()].into(), Some(Arc::new(pool)));

    // 执行分页查询：选择所有字段
    let result: PaginatedResult<TestUser> = query
        .select_fields(&["id", "name", "email", "age"])
        .unwrap()
        .page(1, 10)
        .unwrap()
        .paginate()
        .await
        .unwrap();

    // 验证结果
    assert_eq!(result.total, 50);
    assert_eq!(result.data.len(), 10);

    // 验证字段值存在
    for user in &result.data {
        assert!(user.id > 0, "id 应该大于 0");
        assert!(!user.name.is_empty(), "name 不应该为空");
    }
}

/// 测试分页查询使用默认分页参数
///
/// **验证需求**: 5.7
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_paginate_with_default_params() {
    let (_container, db_url) = match setup_mysql().await {
        Some(setup) => setup,
        None => return,
    };

    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&db_url)
        .await
        .unwrap();

    let db = Database::connect(&db_url).await.unwrap();
    setup_test_data(&db).await.unwrap();

    let table_config = create_test_table_config();

    let query = TableQuery::new(table_config, vec!["user".to_string()].into(), Some(Arc::new(pool)));

    // 执行分页查询：不设置分页参数，使用默认值
    let result: PaginatedResult<TestUser> = query.paginate().await.unwrap();

    // 验证结果：默认应该是第 1 页，每页 20 条
    assert_eq!(result.total, 50);
    assert_eq!(result.page, 1, "默认页码应该是 1");
    assert_eq!(result.page_size, 20, "默认每页大小应该是 20");
    assert_eq!(result.total_pages, 3, "总页数应该是 3");
    assert_eq!(result.data.len(), 20, "当前页数据条数应该是 20");
}
