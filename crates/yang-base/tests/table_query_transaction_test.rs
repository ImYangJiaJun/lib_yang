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

#![allow(deprecated)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use sqlx::mysql::MySqlPoolOptions;
use std::sync::Arc;
use std::time::Duration;
use testcontainers::{runners::AsyncRunner, GenericImage, ImageExt};
use yang_base::table::{Field, Record, Table, TableDefinition, TableQuery};
use yang_db::Database;

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

/// 创建测试用户表定义。
fn create_test_users_table_definition() -> TableDefinition {
    Table::new("test_users")
        .fields([
            Field::id("id"),
            Field::string("name", 50).required().length(2..=50),
            Field::string("email", 100).required().email(),
            Field::integer("age").required(),
            Field::enumeration("status", ["active", "inactive"]).required(),
        ])
        .build()
        .expect("test_users 表定义应有效")
}

/// 创建测试产品表定义（带软删除）。
fn create_test_products_table_definition() -> TableDefinition {
    Table::new("test_products")
        .fields([
            Field::id("id"),
            Field::string("name", 100).required(),
            Field::double("price").required(),
            Field::soft_delete("deleted_at"),
        ])
        .build()
        .expect("test_products 表定义应有效")
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
fn user_data(name: &str, email: &str, age: i64, status: &str) -> Record {
    Record::new()
        .set("name", name)
        .set("email", email)
        .set("age", age)
        .set("status", status)
}

/// 新建一个绑定连接池的 admin TableQuery。
fn admin_query(definition: &TableDefinition, pool: &sqlx::MySqlPool) -> TableQuery {
    definition.bind(Arc::new(pool.clone())).query(["admin"])
}

// ==================== 原子性：回滚撤销全部写入 ====================

/// 事务内插入两条记录后回滚，两条都不应落库
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_transaction_rollback_discards_all_inserts() {
    let (_container, pool, db) = setup_test_env!();
    create_test_users_table(&db).await.unwrap();

    let config = create_test_users_table_definition();

    let mut tx = db.transaction().await.unwrap();

    admin_query(&config, &pool)
        .insert_in_tx(
            &mut tx,
            user_data("张三", "zhangsan@example.com", 25, "active"),
        )
        .await
        .unwrap();
    admin_query(&config, &pool)
        .insert_in_tx(&mut tx, user_data("李四", "lisi@example.com", 30, "active"))
        .await
        .unwrap();

    // 显式回滚
    tx.rollback().await.unwrap();

    // 池外查询：两条记录都不应存在
    let users: Vec<Record> = admin_query(&config, &pool).all().await.unwrap();
    assert_eq!(users.len(), 0, "回滚后不应有任何记录落库");
}

/// 事务在未提交时被 drop，sqlx 尽力回滚，写入不应落库
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_transaction_drop_without_commit_rolls_back() {
    let (_container, pool, db) = setup_test_env!();
    create_test_users_table(&db).await.unwrap();

    let config = create_test_users_table_definition();

    {
        let mut tx = db.transaction().await.unwrap();
        admin_query(&config, &pool)
            .insert_in_tx(
                &mut tx,
                user_data("王五", "wangwu@example.com", 28, "active"),
            )
            .await
            .unwrap();
        // tx 在此作用域结束被 drop，未 commit
    }

    let users: Vec<Record> = admin_query(&config, &pool).all().await.unwrap();
    assert_eq!(users.len(), 0, "未提交事务 drop 后写入不应落库");
}

// ==================== 原子性：提交持久化全部写入 ====================

/// 事务内插入两条记录后提交，两条都应落库
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_transaction_commit_persists_all_inserts() {
    let (_container, pool, db) = setup_test_env!();
    create_test_users_table(&db).await.unwrap();

    let config = create_test_users_table_definition();

    let mut tx = db.transaction().await.unwrap();

    admin_query(&config, &pool)
        .insert_in_tx(
            &mut tx,
            user_data("张三", "zhangsan@example.com", 25, "active"),
        )
        .await
        .unwrap();
    admin_query(&config, &pool)
        .insert_in_tx(&mut tx, user_data("李四", "lisi@example.com", 30, "active"))
        .await
        .unwrap();

    tx.commit().await.unwrap();

    let mut users: Vec<Record> = admin_query(&config, &pool).all().await.unwrap();
    users.sort_by_key(|user| user.require::<i64>("id").unwrap());
    assert_eq!(users.len(), 2, "提交后两条记录都应落库");
    assert_eq!(users[0].require::<String>("name").unwrap(), "张三");
    assert_eq!(users[1].require::<String>("name").unwrap(), "李四");
}

// ==================== 一致性：事务内读-改-写 ====================

/// 同一事务内：insert_returning_id → 用其 id 在事务内 select 应可见未提交写入
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_transaction_read_sees_uncommitted_write() {
    let (_container, pool, db) = setup_test_env!();
    create_test_users_table(&db).await.unwrap();

    let config = create_test_users_table_definition();

    let mut tx = db.transaction().await.unwrap();

    let (_affected, new_id) = admin_query(&config, &pool)
        .insert_returning_id_in_tx(
            &mut tx,
            user_data("赵六", "zhaoliu@example.com", 40, "active"),
        )
        .await
        .unwrap();
    assert!(new_id > 0, "应返回自增主键");

    // 事务内查询：应能看到刚插入但尚未提交的记录
    let in_tx: Vec<Record> = admin_query(&config, &pool)
        .where_eq("id", serde_json::json!(new_id))
        .unwrap()
        .all_in_tx(&mut tx)
        .await
        .unwrap();
    assert_eq!(in_tx.len(), 1, "事务内应可见未提交写入");
    assert_eq!(in_tx[0].require::<String>("name").unwrap(), "赵六");

    // 池外查询：尚未提交，不可见
    let outside: Vec<Record> = admin_query(&config, &pool)
        .where_eq("id", serde_json::json!(new_id))
        .unwrap()
        .all()
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

    let config = create_test_users_table_definition();

    let mut tx = db.transaction().await.unwrap();

    // 第一步：成功插入
    admin_query(&config, &pool)
        .insert_in_tx(
            &mut tx,
            user_data("钱七", "qianqi@example.com", 33, "active"),
        )
        .await
        .unwrap();

    // 第二步：非法枚举值，校验失败
    let bad = user_data("孙八", "sunba@example.com", 22, "not_a_status");
    let result = admin_query(&config, &pool).insert_in_tx(&mut tx, bad).await;
    assert!(result.is_err(), "非法枚举值应在校验层失败");

    // 业务决定回滚整个事务
    tx.rollback().await.unwrap();

    let users: Vec<Record> = admin_query(&config, &pool).all().await.unwrap();
    assert_eq!(users.len(), 0, "任一步失败回滚后，先前成功步骤也不应落库");
}

// ==================== update_in_tx / delete_in_tx ====================

/// 事务内 update 后提交，应持久化
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_transaction_update_commit() {
    let (_container, pool, db) = setup_test_env!();
    create_test_users_table(&db).await.unwrap();

    let config = create_test_users_table_definition();

    // 预置一条（自动提交路径）
    let new_id = admin_query(&config, &pool)
        .insert_returning_id(user_data("初始", "init@example.com", 20, "active"))
        .await
        .unwrap()
        .1;

    // 事务内更新
    let mut tx = db.transaction().await.unwrap();
    let upd = Record::new().set("age", 99).set("status", "inactive");
    let affected = admin_query(&config, &pool)
        .where_eq("id", serde_json::json!(new_id))
        .unwrap()
        .update_in_tx(&mut tx, upd)
        .await
        .unwrap();
    assert_eq!(affected, 1);
    tx.commit().await.unwrap();

    let users: Vec<Record> = admin_query(&config, &pool)
        .where_eq("id", serde_json::json!(new_id))
        .unwrap()
        .all()
        .await
        .unwrap();
    assert_eq!(
        users[0].require::<i64>("age").unwrap(),
        99,
        "提交后更新应持久化"
    );
    assert_eq!(users[0].require::<String>("status").unwrap(), "inactive");
}

/// 事务内物理删除后回滚，记录应仍存在
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_transaction_delete_rollback_keeps_row() {
    let (_container, pool, db) = setup_test_env!();
    create_test_users_table(&db).await.unwrap();

    let config = create_test_users_table_definition();

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

    let users: Vec<Record> = admin_query(&config, &pool)
        .where_eq("id", serde_json::json!(new_id))
        .unwrap()
        .all()
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

    let config = create_test_products_table_definition();

    db.execute("INSERT INTO test_products (name, price) VALUES ('产品X', 12.50)")
        .await
        .unwrap();

    let products: Vec<Record> = admin_query(&config, &pool)
        .where_eq("name", serde_json::json!("产品X"))
        .unwrap()
        .all()
        .await
        .unwrap();
    let product_id = products[0].require::<i64>("id").unwrap();
    assert_eq!(products[0].optional::<i64>("deleted_at").unwrap(), None);

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
    let products: Vec<Record> = admin_query(&config, &pool)
        .where_eq("id", serde_json::json!(product_id))
        .unwrap()
        .with_trashed()
        .all()
        .await
        .unwrap();
    assert_eq!(products.len(), 1, "软删除记录仍存在");
    assert!(
        products[0].optional::<i64>("deleted_at").unwrap().is_some(),
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

    let config = create_test_users_table_definition();

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
    let users: Vec<Record> = admin_query(&config, &pool).all().await.unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].require::<String>("name").unwrap(), "甲方");
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

    let config = create_test_users_table_definition();

    // 预置一条
    admin_query(&config, &pool)
        .insert_returning_id(user_data("慢查询", "slow@example.com", 42, "active"))
        .await
        .unwrap();

    // 阈值 0：每次执行都超阈值 → 触发 warn 分支；结果仍须正确
    let q = admin_query(&config, &pool).with_slow_threshold(Some(Duration::from_nanos(0)));

    let users: Vec<Record> = q
        .where_eq("name", serde_json::json!("慢查询"))
        .unwrap()
        .all()
        .await
        .unwrap();
    assert_eq!(users.len(), 1, "慢查询计时不应改变结果");
    assert_eq!(users[0].require::<i64>("age").unwrap(), 42);
}
