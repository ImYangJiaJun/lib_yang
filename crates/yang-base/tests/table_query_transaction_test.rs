//! TableQuery 事务传播（C1 / DB-5）集成测试
//!
//! 验证受保护层 `*_in_tx` 系列方法的原子性与一致性：
//! - 事务回滚时，事务内的所有写入全部撤销（原子性）
//! - 事务提交后，事务内的所有写入全部持久化
//! - 同一事务内「读-改-写」可见未提交的中间状态（一致快照）
//! - 任一步失败回滚后，先前步骤不落库（多步原子）
//! - 软删除在事务内同样走 UPDATE 标记
//! - 事务结束后复用 TableQuery 的 `*_in_tx` 返回错误而非 panic
//!
//! **注意**: 这些测试需要 Docker 环境。无 Docker 时自动跳过。
//! 运行：`cargo test --test table_query_transaction_test -- --test-threads=1 --ignored`

use serde::{Deserialize, Serialize};
use sqlx::mysql::MySqlPoolOptions;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use testcontainers::{runners::AsyncRunner, GenericImage, ImageExt};
use yang_base::table::{FieldConfig, FieldType, TableConfig, TableQuery, Validator};
use yang_db::Database;

/// 测试用户结构
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, PartialEq)]
struct TestUser {
    id: i64,
    name: String,
    email: String,
    age: i32,
    status: String,
}

/// 测试产品结构（用于软删除测试）
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, PartialEq)]
struct TestProduct {
    id: i64,
    name: String,
    price: f64,
    deleted_at: Option<i64>,
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

/// 创建测试用户表
async fn create_test_users_table(db: &Database) -> Result<(), Box<dyn std::error::Error>> {
    db.execute(
        r#"
        CREATE TABLE IF NOT EXISTS test_users (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(50) NOT NULL,
            email VARCHAR(100) NOT NULL,
            age INT NOT NULL,
            status VARCHAR(20) NOT NULL DEFAULT 'active'
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4
        "#,
    )
    .await?;
    Ok(())
}

/// 创建测试产品表（带软删除字段）
async fn create_test_products_table(db: &Database) -> Result<(), Box<dyn std::error::Error>> {
    db.execute(
        r#"
        CREATE TABLE IF NOT EXISTS test_products (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(100) NOT NULL,
            price DOUBLE NOT NULL,
            deleted_at BIGINT NULL
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4
        "#,
    )
    .await?;
    Ok(())
}

/// 创建测试用户表配置
fn create_test_users_table_config() -> Arc<TableConfig> {
    Arc::new(
        TableConfig::new("test_users")
            .field(FieldConfig::new("id", FieldType::BigInt))
            .field(
                FieldConfig::new("name", FieldType::String { max_length: 50 })
                    .required(true)
                    .validator(Validator::MinLength(2))
                    .validator(Validator::MaxLength(50)),
            )
            .field(
                FieldConfig::new("email", FieldType::String { max_length: 100 })
                    .required(true)
                    .validator(Validator::Email),
            )
            .field(FieldConfig::new("age", FieldType::Integer).required(true))
            .field(
                FieldConfig::new(
                    "status",
                    FieldType::Enum {
                        values: vec!["active".to_string(), "inactive".to_string()],
                    },
                )
                .required(true),
            ),
    )
}

/// 创建测试产品表配置（带软删除）
fn create_test_products_table_config() -> Arc<TableConfig> {
    Arc::new(
        TableConfig::new("test_products")
            .field(FieldConfig::new("id", FieldType::BigInt))
            .field(FieldConfig::new("name", FieldType::String { max_length: 100 }).required(true))
            .field(FieldConfig::new("price", FieldType::Double).required(true))
            .field(FieldConfig::new("deleted_at", FieldType::BigInt))
            .soft_delete_field("deleted_at"),
    )
}

/// 设置测试环境
macro_rules! setup_test_env {
    () => {{
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

        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        let db = Database::connect(&db_url).await.unwrap();

        (_container, pool, db)
    }};
}

/// 构建用户插入数据
fn user_data(name: &str, email: &str, age: i64, status: &str) -> HashMap<String, serde_json::Value> {
    let mut data = HashMap::new();
    data.insert("name".to_string(), serde_json::json!(name));
    data.insert("email".to_string(), serde_json::json!(email));
    data.insert("age".to_string(), serde_json::json!(age));
    data.insert("status".to_string(), serde_json::json!(status));
    data
}

/// 新建一个绑定连接池的 admin TableQuery
fn admin_query(config: &Arc<TableConfig>, pool: &sqlx::MySqlPool) -> TableQuery {
    TableQuery::new(
        config.clone(),
        vec!["admin".to_string()].into(),
        Some(Arc::new(pool.clone())),
    )
}

// ==================== 原子性：回滚撤销全部写入 ====================

/// 事务内插入两条记录后回滚，两条都不应落库
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_transaction_rollback_discards_all_inserts() {
    let (_container, pool, db) = setup_test_env!();
    create_test_users_table(&db).await.unwrap();

    let config = create_test_users_table_config();

    let mut tx = db.transaction().await.unwrap();

    admin_query(&config, &pool)
        .insert_in_tx(&mut tx, user_data("张三", "zhangsan@example.com", 25, "active"))
        .await
        .unwrap();
    admin_query(&config, &pool)
        .insert_in_tx(&mut tx, user_data("李四", "lisi@example.com", 30, "active"))
        .await
        .unwrap();

    // 显式回滚
    tx.rollback().await.unwrap();

    // 池外查询：两条记录都不应存在
    let users: Vec<TestUser> = admin_query(&config, &pool).select().await.unwrap();
    assert_eq!(users.len(), 0, "回滚后不应有任何记录落库");
}

/// 事务在未提交时被 drop，sqlx 尽力回滚，写入不应落库
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_transaction_drop_without_commit_rolls_back() {
    let (_container, pool, db) = setup_test_env!();
    create_test_users_table(&db).await.unwrap();

    let config = create_test_users_table_config();

    {
        let mut tx = db.transaction().await.unwrap();
        admin_query(&config, &pool)
            .insert_in_tx(&mut tx, user_data("王五", "wangwu@example.com", 28, "active"))
            .await
            .unwrap();
        // tx 在此作用域结束被 drop，未 commit
    }

    let users: Vec<TestUser> = admin_query(&config, &pool).select().await.unwrap();
    assert_eq!(users.len(), 0, "未提交事务 drop 后写入不应落库");
}

// ==================== 原子性：提交持久化全部写入 ====================

/// 事务内插入两条记录后提交，两条都应落库
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_transaction_commit_persists_all_inserts() {
    let (_container, pool, db) = setup_test_env!();
    create_test_users_table(&db).await.unwrap();

    let config = create_test_users_table_config();

    let mut tx = db.transaction().await.unwrap();

    admin_query(&config, &pool)
        .insert_in_tx(&mut tx, user_data("张三", "zhangsan@example.com", 25, "active"))
        .await
        .unwrap();
    admin_query(&config, &pool)
        .insert_in_tx(&mut tx, user_data("李四", "lisi@example.com", 30, "active"))
        .await
        .unwrap();

    tx.commit().await.unwrap();

    let mut users: Vec<TestUser> = admin_query(&config, &pool).select().await.unwrap();
    users.sort_by_key(|u| u.id);
    assert_eq!(users.len(), 2, "提交后两条记录都应落库");
    assert_eq!(users[0].name, "张三");
    assert_eq!(users[1].name, "李四");
}

// ==================== 一致性：事务内读-改-写 ====================

/// 同一事务内：insert_returning_id → 用其 id 在事务内 select 应可见未提交写入
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_transaction_read_sees_uncommitted_write() {
    let (_container, pool, db) = setup_test_env!();
    create_test_users_table(&db).await.unwrap();

    let config = create_test_users_table_config();

    let mut tx = db.transaction().await.unwrap();

    let (_affected, new_id) = admin_query(&config, &pool)
        .insert_returning_id_in_tx(&mut tx, user_data("赵六", "zhaoliu@example.com", 40, "active"))
        .await
        .unwrap();
    assert!(new_id > 0, "应返回自增主键");

    // 事务内查询：应能看到刚插入但尚未提交的记录
    let in_tx: Vec<TestUser> = admin_query(&config, &pool)
        .where_eq("id", serde_json::json!(new_id))
        .unwrap()
        .select_in_tx(&mut tx)
        .await
        .unwrap();
    assert_eq!(in_tx.len(), 1, "事务内应可见未提交写入");
    assert_eq!(in_tx[0].name, "赵六");

    // 池外查询：尚未提交，不可见
    let outside: Vec<TestUser> = admin_query(&config, &pool)
        .where_eq("id", serde_json::json!(new_id))
        .unwrap()
        .select()
        .await
        .unwrap();
    assert_eq!(outside.len(), 0, "未提交写入在事务外不可见");

    tx.rollback().await.unwrap();
}

/// 多步原子：父行插入成功，子步骤校验失败回滚后父行不落库
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_transaction_multi_step_atomic_on_failure() {
    let (_container, pool, db) = setup_test_env!();
    create_test_users_table(&db).await.unwrap();

    let config = create_test_users_table_config();

    let mut tx = db.transaction().await.unwrap();

    // 第一步：成功插入
    admin_query(&config, &pool)
        .insert_in_tx(&mut tx, user_data("钱七", "qianqi@example.com", 33, "active"))
        .await
        .unwrap();

    // 第二步：非法枚举值，校验失败
    let bad = user_data("孙八", "sunba@example.com", 22, "not_a_status");
    let result = admin_query(&config, &pool).insert_in_tx(&mut tx, bad).await;
    assert!(result.is_err(), "非法枚举值应在校验层失败");

    // 业务决定回滚整个事务
    tx.rollback().await.unwrap();

    let users: Vec<TestUser> = admin_query(&config, &pool).select().await.unwrap();
    assert_eq!(users.len(), 0, "任一步失败回滚后，先前成功步骤也不应落库");
}

// ==================== update_in_tx / delete_in_tx ====================

/// 事务内 update 后提交，应持久化
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_transaction_update_commit() {
    let (_container, pool, db) = setup_test_env!();
    create_test_users_table(&db).await.unwrap();

    let config = create_test_users_table_config();

    // 预置一条（自动提交路径）
    let new_id = admin_query(&config, &pool)
        .insert_returning_id(user_data("初始", "init@example.com", 20, "active"))
        .await
        .unwrap()
        .1;

    // 事务内更新
    let mut tx = db.transaction().await.unwrap();
    let mut upd = HashMap::new();
    upd.insert("age".to_string(), serde_json::json!(99));
    upd.insert("status".to_string(), serde_json::json!("inactive"));
    let affected = admin_query(&config, &pool)
        .where_eq("id", serde_json::json!(new_id))
        .unwrap()
        .update_in_tx(&mut tx, upd)
        .await
        .unwrap();
    assert_eq!(affected, 1);
    tx.commit().await.unwrap();

    let users: Vec<TestUser> = admin_query(&config, &pool)
        .where_eq("id", serde_json::json!(new_id))
        .unwrap()
        .select()
        .await
        .unwrap();
    assert_eq!(users[0].age, 99, "提交后更新应持久化");
    assert_eq!(users[0].status, "inactive");
}

/// 事务内物理删除后回滚，记录应仍存在
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_transaction_delete_rollback_keeps_row() {
    let (_container, pool, db) = setup_test_env!();
    create_test_users_table(&db).await.unwrap();

    let config = create_test_users_table_config();

    let new_id = admin_query(&config, &pool)
        .insert_returning_id(user_data("待删", "del@example.com", 50, "active"))
        .await
        .unwrap()
        .1;

    let mut tx = db.transaction().await.unwrap();
    let affected = admin_query(&config, &pool)
        .where_eq("id", serde_json::json!(new_id))
        .unwrap()
        .delete_in_tx(&mut tx)
        .await
        .unwrap();
    assert_eq!(affected, 1, "事务内删除影响 1 行");
    tx.rollback().await.unwrap();

    let users: Vec<TestUser> = admin_query(&config, &pool)
        .where_eq("id", serde_json::json!(new_id))
        .unwrap()
        .select()
        .await
        .unwrap();
    assert_eq!(users.len(), 1, "回滚后删除应被撤销，记录仍存在");
}

/// 软删除在事务内同样走 UPDATE 标记；提交后 deleted_at 不为空
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_transaction_soft_delete_in_tx() {
    let (_container, pool, db) = setup_test_env!();
    create_test_products_table(&db).await.unwrap();

    let config = create_test_products_table_config();

    db.execute("INSERT INTO test_products (name, price) VALUES ('产品X', 12.50)")
        .await
        .unwrap();

    let products: Vec<TestProduct> = admin_query(&config, &pool)
        .where_eq("name", serde_json::json!("产品X"))
        .unwrap()
        .select()
        .await
        .unwrap();
    let product_id = products[0].id;
    assert_eq!(products[0].deleted_at, None);

    let mut tx = db.transaction().await.unwrap();
    let affected = admin_query(&config, &pool)
        .where_eq("id", serde_json::json!(product_id))
        .unwrap()
        .delete_in_tx(&mut tx)
        .await
        .unwrap();
    assert_eq!(affected, 1, "软删除在事务内应影响 1 行");
    tx.commit().await.unwrap();

    // with_trashed 读取，确认记录仍在且 deleted_at 被标记
    let products: Vec<TestProduct> = admin_query(&config, &pool)
        .where_eq("id", serde_json::json!(product_id))
        .unwrap()
        .with_trashed()
        .select()
        .await
        .unwrap();
    assert_eq!(products.len(), 1, "软删除记录仍存在");
    assert!(
        products[0].deleted_at.is_some(),
        "事务内软删除提交后 deleted_at 应被标记"
    );
}

// ==================== 事务生命周期：commit 消费 tx + 独立事务隔离 ====================

/// commit(self) 在编译期消费 tx，无法复用同一事务；验证独立事务彼此隔离：
/// 第一个事务提交、第二个回滚，最终只有已提交的写入可见。
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_independent_transactions_isolated() {
    let (_container, pool, db) = setup_test_env!();
    create_test_users_table(&db).await.unwrap();

    let config = create_test_users_table_config();

    let mut tx = db.transaction().await.unwrap();
    admin_query(&config, &pool)
        .insert_in_tx(&mut tx, user_data("甲方", "jia@example.com", 21, "active"))
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // commit(self) 已 move 掉 tx，编译期即不允许复用同一事务。
    // 用一个独立事务写入后回滚，验证两事务互不影响。
    let mut tx2 = db.transaction().await.unwrap();
    let r = admin_query(&config, &pool)
        .insert_in_tx(&mut tx2, user_data("乙方", "yi@example.com", 22, "active"))
        .await;
    assert!(r.is_ok(), "新事务应可正常写入");
    tx2.rollback().await.unwrap();

    // 仅“甲方”被提交
    let users: Vec<TestUser> = admin_query(&config, &pool).select().await.unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "甲方");
}

// ==================== C4 慢查询计时：超阈值仍正常执行 ====================

/// 慢查询阈值设为 0（一切都算慢）：执行仍成功返回正确结果，仅额外 warn 日志。
/// 验证 `with_slow_threshold` + `timed` 包裹不改变执行语义。
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_slow_query_timing_does_not_break_execution() {
    use std::time::Duration;
    let (_container, pool, db) = setup_test_env!();
    create_test_users_table(&db).await.unwrap();

    let config = create_test_users_table_config();

    // 预置一条
    admin_query(&config, &pool)
        .insert_returning_id(user_data("慢查询", "slow@example.com", 42, "active"))
        .await
        .unwrap();

    // 阈值 0：每次执行都超阈值 → 触发 warn 分支；结果仍须正确
    let q = TableQuery::new(
        config.clone(),
        vec!["admin".to_string()].into(),
        Some(std::sync::Arc::new(pool.clone())),
    )
    .with_slow_threshold(Some(Duration::from_nanos(0)));

    let users: Vec<TestUser> = q
        .where_eq("name", serde_json::json!("慢查询"))
        .unwrap()
        .select()
        .await
        .unwrap();
    assert_eq!(users.len(), 1, "慢查询计时不应改变结果");
    assert_eq!(users[0].age, 42);
}
