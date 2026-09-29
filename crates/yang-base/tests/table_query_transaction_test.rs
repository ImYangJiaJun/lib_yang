//! TableQuery 事务传播（C1 / DB-5）集成测试
//!
//! 验证受保护层 `*_in_tx` 系列方法的原子性与一致性：
//! - 事务回滚时，事务内的所有写入全部撤销（原子性）
//! - 事务提交后，事务内的所有写入全部持久化
//! - 同一事务内「读-改-写」可见未提交的中间状态（一致快照）
//! - 任一步失败回滚后，先前步骤不落库（多步原子）
//! - 软删除在事务内同样走 UPDATE 标记
//! - 事务结束后复用 TableQuery 的 `*_in_tx` 返回错误而非 panic
//! - 批量 upsert（`INSERT ... ON DUPLICATE KEY UPDATE`）：赋值列白名单、`created_at`
//!   不被第二次写入刷新、`rows_affected` 语义、跨 chunk 失败整批回滚
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

// ==================== 批量 upsert（INSERT ... ON DUPLICATE KEY UPDATE） ====================

/// 批量 upsert 的测试表：`option_id` 上有唯一索引——ODKU 的冲突键由它决定。
async fn create_test_batch_options_table(db: &Database) -> Result<(), Box<dyn std::error::Error>> {
    db.execute(
        r#"
        CREATE TABLE IF NOT EXISTS test_batch_options (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            option_id VARCHAR(64) NOT NULL,
            label VARCHAR(100) NOT NULL,
            sort_order INT NOT NULL,
            created_at BIGINT NOT NULL,
            updated_at BIGINT NOT NULL,
            UNIQUE KEY uk_test_batch_options_option_id (option_id)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4
        "#,
    )
    .await?;
    Ok(())
}

/// 没有时间戳列的同一形状。
///
/// `rows_affected` 那条用例需要「命中且**整行**没有任何变化」的一拍，而带 `updated_at`
/// 时框架每轮都会写一个新时间戳 ⇒ 那一拍必然算「有变化」，测不到 CLIENT_FOUND_ROWS。
async fn create_test_batch_options_plain_table(
    db: &Database,
) -> Result<(), Box<dyn std::error::Error>> {
    db.execute(
        r#"
        CREATE TABLE IF NOT EXISTS test_batch_options_plain (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            option_id VARCHAR(64) NOT NULL,
            label VARCHAR(100) NOT NULL,
            sort_order INT NOT NULL,
            UNIQUE KEY uk_test_batch_options_plain_option_id (option_id)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4
        "#,
    )
    .await?;
    Ok(())
}

fn create_test_batch_options_definition() -> TableDefinition {
    Table::new("test_batch_options")
        .fields([
            Field::id("id"),
            Field::string("option_id", 64).required().unique(),
            Field::string("label", 100).required(),
            Field::integer("sort_order").required(),
            Field::created_at("created_at"),
            Field::updated_at("updated_at"),
        ])
        .build()
        .expect("test_batch_options 表定义应有效")
}

fn create_test_batch_options_plain_definition() -> TableDefinition {
    Table::new("test_batch_options_plain")
        .fields([
            Field::id("id"),
            Field::string("option_id", 64).required().unique(),
            Field::string("label", 100).required(),
            Field::integer("sort_order").required(),
        ])
        .build()
        .expect("test_batch_options_plain 表定义应有效")
}

/// 一行同构的批量 upsert 数据：`option_id` 是冲突键，`label` / `sort_order` 是业务值。
fn batch_option_row(option_id: &str, label: &str, sort_order: i64) -> Record {
    Record::new()
        .set("option_id", option_id)
        .set("label", label)
        .set("sort_order", sort_order)
}

/// 首轮插入、改 label 再批走 UPDATE：**`created_at` 原地不动**、`updated_at` 前进。
///
/// 这是「框架补的 created_at 不进赋值列」的实证：把它放进 ODKU 的赋值列，第二次导入
/// 会把创建时间刷成本轮时间，注释与文档里所有「created_at 只在新插入时写」就都成了空话。
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn batch_upsert_inserts_then_updates_without_touching_created_at() {
    let (_container, pool, db) = setup_test_env!();
    create_test_batch_options_table(&db).await.unwrap();
    let config = create_test_batch_options_definition();
    let update_columns = ["label", "sort_order"];

    // 首轮：两行全新 → 全插入
    let mut tx = db.transaction().await.unwrap();
    let affected = admin_query(&config, &pool)
        .upsert_batch_in_tx(
            &mut tx,
            vec![
                batch_option_row("opt_a", "甲", 1),
                batch_option_row("opt_b", "乙", 2),
            ],
            &update_columns,
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(affected, 2, "全插入 = N");

    let (created_before, updated_before): (i64, i64) = sqlx::query_as(
        "SELECT `created_at`, `updated_at` FROM `test_batch_options` WHERE `option_id` = 'opt_a'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(created_before > 0, "created_at 由框架在 prepare 阶段补齐");

    // 时间戳是**秒**：两轮落在同一秒时 updated_at 不会变，先跨过一秒再断言「前进」。
    tokio::time::sleep(Duration::from_millis(1_100)).await;

    // 第二轮：`opt_a` 改 label、`opt_b` 原样 —— 两行都命中唯一键，走 UPDATE 分支
    let mut tx = db.transaction().await.unwrap();
    let affected = admin_query(&config, &pool)
        .upsert_batch_in_tx(
            &mut tx,
            vec![
                batch_option_row("opt_a", "甲改", 1),
                batch_option_row("opt_b", "乙", 2),
            ],
            &update_columns,
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(affected >= 2, "两行都命中，实际 {affected}");

    let (created_after, updated_after, label_after, sort_after): (i64, i64, String, i64) =
        sqlx::query_as(
            "SELECT `created_at`, `updated_at`, `label`, `sort_order` \
             FROM `test_batch_options` WHERE `option_id` = 'opt_a'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        created_after, created_before,
        "**created_at 绝不能被第二次 upsert 刷新**（它不在 ODKU 的赋值列里）"
    );
    assert!(
        updated_after > updated_before,
        "updated_at 必须前进（它在赋值列里，语义同 with_updated_timestamp）"
    );
    assert_eq!(label_after, "甲改", "label 在赋值列里，走 UPDATE");
    assert_eq!(sort_after, 1);

    // 第三轮：原样再批 —— 业务列一个都不许变（`updated_at` 例外，它每轮都写）。
    let mut tx = db.transaction().await.unwrap();
    admin_query(&config, &pool)
        .upsert_batch_in_tx(
            &mut tx,
            vec![
                batch_option_row("opt_a", "甲改", 1),
                batch_option_row("opt_b", "乙", 2),
            ],
            &update_columns,
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let (created_again, label_again, sort_again): (i64, String, i64) = sqlx::query_as(
        "SELECT `created_at`, `label`, `sort_order` \
         FROM `test_batch_options` WHERE `option_id` = 'opt_a'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        created_again, created_before,
        "原样再批同样不许碰 created_at"
    );
    assert_eq!(label_again, "甲改");
    assert_eq!(sort_again, 1);
    let (rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM `test_batch_options`")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 2, "ODKU 只更新命中行，不长出新行");
}

/// `rows_affected` 的语义：全插入 = N、全更新且值有变 = 2N、全命中无变化 = N。
///
/// 最后一条是 sqlx 带 `CLIENT_FOUND_ROWS` 的实证：命中但不改的行计 1 而不是 0。
/// 用**没有时间戳列**的那张表正是为了让那一拍真的「整行无变化」（见建表注释）。
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn batch_upsert_rows_affected_semantics() {
    let (_container, pool, db) = setup_test_env!();
    create_test_batch_options_plain_table(&db).await.unwrap();
    let config = create_test_batch_options_plain_definition();
    let update_columns = ["label", "sort_order"];

    // ① 全插入：每行计 1
    let mut tx = db.transaction().await.unwrap();
    let affected = admin_query(&config, &pool)
        .upsert_batch_in_tx(
            &mut tx,
            vec![
                batch_option_row("opt_a", "甲", 1),
                batch_option_row("opt_b", "乙", 2),
                batch_option_row("opt_c", "丙", 3),
            ],
            &update_columns,
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(affected, 3, "全插入 = N");

    // ② 全更新且值有变：每行计 2
    let mut tx = db.transaction().await.unwrap();
    let affected = admin_query(&config, &pool)
        .upsert_batch_in_tx(
            &mut tx,
            vec![
                batch_option_row("opt_a", "甲2", 10),
                batch_option_row("opt_b", "乙2", 20),
                batch_option_row("opt_c", "丙2", 30),
            ],
            &update_columns,
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(affected, 6, "全更新且值有变 = 2N");

    // ③ 全命中但整行没有任何变化：CLIENT_FOUND_ROWS 把它计成 N（不是 0）
    let mut tx = db.transaction().await.unwrap();
    let affected = admin_query(&config, &pool)
        .upsert_batch_in_tx(
            &mut tx,
            vec![
                batch_option_row("opt_a", "甲2", 10),
                batch_option_row("opt_b", "乙2", 20),
                batch_option_row("opt_c", "丙2", 30),
            ],
            &update_columns,
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        affected, 3,
        "命中但值未变：CLIENT_FOUND_ROWS 下计 1 而不是 0"
    );
}

/// 赋值列是**白名单**：批里带了新值的列，只要不在白名单里，命中时就保持原值。
///
/// 「省略即保持原值」这条语义全靠它——白名单若退化成「行里带的列」，第二次导入会把
/// 调用方没打算动的列一并重置。
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn batch_upsert_only_writes_the_whitelisted_columns() {
    let (_container, pool, db) = setup_test_env!();
    create_test_batch_options_table(&db).await.unwrap();
    let config = create_test_batch_options_definition();

    let mut tx = db.transaction().await.unwrap();
    admin_query(&config, &pool)
        .upsert_batch_in_tx(
            &mut tx,
            vec![batch_option_row("opt_a", "甲", 1)],
            &["label"],
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let (created_before,): (i64,) =
        sqlx::query_as("SELECT `created_at` FROM `test_batch_options` WHERE `option_id` = 'opt_a'")
            .fetch_one(&pool)
            .await
            .unwrap();

    // 第二次：label 与 sort_order 都带了新值，但赋值列只点名 label
    let mut tx = db.transaction().await.unwrap();
    admin_query(&config, &pool)
        .upsert_batch_in_tx(
            &mut tx,
            vec![batch_option_row("opt_a", "甲改", 99)],
            &["label"],
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let (label, sort_order, created_after): (String, i64, i64) = sqlx::query_as(
        "SELECT `label`, `sort_order`, `created_at` \
         FROM `test_batch_options` WHERE `option_id` = 'opt_a'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(label, "甲改", "白名单里的列照常更新");
    assert_eq!(
        sort_order, 1,
        "**不在白名单里的列必须保持原值**（ODKU 只赋值白名单）"
    );
    assert_eq!(created_after, created_before, "created_at 不在赋值列里");
}

/// 跨 chunk 的原子性：批大小 = 1 时第 1 行先写出去、第 2 行在**服务端**失败
/// （超列宽 1406），整批仍必须随调用方事务一起回滚。
///
/// 这条刻意下沉到 `yang-db` 层：批大小只有 `upsert_batch_with_size` 能指定
/// （`TableQuery` 不暴露它），而它也是全仓唯一让「多行 ODKU + 分块」真的落进 MySQL 的地方。
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn batch_upsert_is_atomic_across_chunks() {
    let (_container, pool, db) = setup_test_env!();
    create_test_batch_options_table(&db).await.unwrap();
    let table = yang_db::TableRef::new("test_batch_options").expect("固定表名有效");
    let update_columns = vec!["label".to_string(), "sort_order".to_string()];

    let rows = vec![
        serde_json::json!({
            "option_id": "opt_1", "label": "甲", "sort_order": 1,
            "created_at": 1, "updated_at": 1,
        }),
        serde_json::json!({
            // 150 个汉字 = 450 字节，超过 `label VARCHAR(100)` ⇒ 服务端报 1406
            "option_id": "opt_2", "label": "乙".repeat(150), "sort_order": 2,
            "created_at": 1, "updated_at": 1,
        }),
    ];

    let mut tx = db.transaction().await.unwrap();
    let result = tx
        .table(&table)
        .upsert_batch_with_size(&rows, &update_columns, 1)
        .await;
    assert!(result.is_err(), "第 2 行超列宽，必须在服务端失败（1406）");
    tx.rollback().await.unwrap();

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM `test_batch_options`")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count, 0,
        "第 1 行已经在第 1 个 chunk 里写出去过，也必须随调用方事务回滚"
    );
}
