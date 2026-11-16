#![cfg(feature = "mysql")]
#![allow(deprecated)]
// 数据库连接集成测试
//
// 运行约定：
// - 需要真实 MySQL 的用例统一标记 #[ignore]：默认 `cargo test -p yang-db` 不触库，
//   以 `-- --ignored` 运行时连接失败必须硬失败，杜绝“连不上库也算通过”的假绿。
// - 纯 SQL 生成/客户端参数校验用例走离线懒连接（connect_lazy + Database::from_pool），
//   默认测试即可执行并断言，不再因 30s 连接超时而假通过。
// - 7 个事务用例各自使用独立表名并自清理，默认并行执行时不会互相 DROP/CREATE 同一张表。
//
// 真实触库运行方式：
// MYSQL_TEST_URL=mysql://root:...@127.0.0.1:3306/test \
//   cargo test -p yang-db --test integration_database -- --ignored --test-threads=1
#![allow(dead_code)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use sqlx::mysql::MySqlPoolOptions;
use yang_db::{Database, DatabaseConfig};

/// 真实测试库连接串：优先 MYSQL_TEST_URL，未设置时回退到本机测试库。
fn test_db_url() -> String {
    std::env::var("MYSQL_TEST_URL")
        .unwrap_or_else(|_| "mysql://root:111111@localhost:3306/test".to_string())
}

/// 离线数据库实例：connect_lazy 只校验 URL，不建立真实连接。
///
/// 只做 SQL 生成或纯客户端参数校验的用例用它构造 `Database`，
/// 从而在无 MySQL 环境下也真正执行断言，而不是静默跳过。
fn offline_database() -> Database {
    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .connect_lazy("mysql://root:111111@localhost:3306/test")
        .expect("测试数据库 URL 应可解析");
    Database::from_pool(pool, DatabaseConfig::default()).expect("默认数据库配置应合法")
}

/// 真实触库连接：只允许在 #[ignore] 用例中调用；连接失败即测试失败。
async fn real_database() -> Database {
    Database::connect(&test_db_url())
        .await
        .expect("MySQL 测试库必须可连接（设置 MYSQL_TEST_URL 或启动本机测试库）")
}

#[tokio::test]
#[ignore = "需要真实 MySQL 实例"]
async fn test_connect_to_test_database() {
    let _db = real_database().await;
    println!("成功连接到测试数据库");
}

#[tokio::test]
#[ignore = "需要真实 MySQL 实例"]
async fn test_connect_with_custom_config() {
    // DatabaseConfig 为 #[non_exhaustive]：集成测试是独立 crate，用 default() + 字段赋值。
    let mut config = DatabaseConfig::default();
    config.max_connections = 5;
    config.connect_timeout = 10;
    config.idle_timeout = 300;
    config.enable_logging = true;

    let _db = Database::connect_with_config(&test_db_url(), config)
        .await
        .expect("使用自定义配置连接测试库失败");

    println!("成功使用自定义配置连接到测试数据库");
}

#[tokio::test]
#[ignore = "需要真实 MySQL 实例"]
async fn test_table_exists() {
    let db = real_database().await;
    let probe = yang_db::table!("table_exists_probe");

    let _ = db.drop_table(probe).await;
    assert!(
        !db.table_exists(probe).await.expect("检查表存在失败"),
        "清理后探针表不应存在"
    );

    db.create_table("CREATE TABLE table_exists_probe (id INT PRIMARY KEY)")
        .await
        .expect("创建探针表失败");
    assert!(
        db.table_exists(probe).await.expect("检查表存在失败"),
        "创建后探针表应存在"
    );

    db.drop_table(probe).await.expect("删除探针表失败");
    println!("表存在检查功能正常");
}

#[tokio::test]
#[ignore = "需要真实 MySQL 实例"]
async fn test_execute_simple_query() {
    let db = real_database().await;

    db.execute("SELECT 1").await.expect("执行简单查询失败");
    println!("成功执行简单查询");
}

#[tokio::test]
#[ignore = "需要真实 MySQL 实例"]
async fn test_connection_pool_reuse() {
    let db = real_database().await;

    // 连续执行多个查询以验证连接池可复用；任一查询失败都应让测试变红。
    for i in 1..=5 {
        db.execute(&format!("SELECT {}", i))
            .await
            .unwrap_or_else(|error| panic!("查询 {i} 失败: {error}"));
        println!("查询 {} 成功", i);
    }
}

#[tokio::test]
async fn test_insert_batch_sql_generation() {
    // 纯 SQL 生成，不触库：离线懒连接使该用例默认 `cargo test` 就执行断言。
    let db = offline_database();
    let sql = db.table(yang_db::table!("test_users")).to_sql();
    assert!(
        sql.contains("test_users"),
        "生成的 SQL 应包含表名，实际: {sql}"
    );
    println!("生成的 SQL: {}", sql);

    // 注意：实际的批量插入需要表存在，这里只测试 SQL 生成
    println!("批量插入 SQL 生成测试完成");
}

#[tokio::test]
#[ignore = "需要真实 MySQL 实例"]
async fn test_insert_batch_with_real_table() {
    use serde_json::json;

    let db = real_database().await;

    // 创建测试表
    let create_table_sql = "
        CREATE TABLE IF NOT EXISTS test_batch_insert (
            id INT PRIMARY KEY AUTO_INCREMENT,
            name VARCHAR(100) NOT NULL,
            email VARCHAR(100),
            age INT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
    ";

    db.create_table(create_table_sql)
        .await
        .expect("创建测试表失败");
    println!("测试表创建成功");

    // 清空表
    db.execute("DELETE FROM test_batch_insert")
        .await
        .expect("清空测试表失败");

    // 批量插入数据
    let users = vec![
        json!({"name": "张三", "email": "zhangsan@example.com", "age": 25}),
        json!({"name": "李四", "email": "lisi@example.com", "age": 30}),
        json!({"name": "王五", "email": "wangwu@example.com", "age": 28}),
    ];

    let affected_rows = db
        .table(yang_db::table!("test_batch_insert"))
        .insert_batch(&users)
        .await
        .expect("批量插入失败");
    println!("批量插入成功，影响 {} 行", affected_rows);
    assert_eq!(affected_rows, 3, "应该插入 3 条记录");

    // 验证插入的数据
    let count: i64 = db
        .table(yang_db::table!("test_batch_insert"))
        .count()
        .await
        .expect("验证插入数据失败");
    assert_eq!(count, 3, "批量插入后应有 3 条记录");

    // 清理测试表
    db.drop_table(yang_db::table!("test_batch_insert"))
        .await
        .expect("清理测试表失败");
}

#[tokio::test]
async fn test_insert_batch_empty_data() {
    // 空数据校验发生在访问连接池之前，离线懒连接即可覆盖，默认测试实际执行断言。
    let db = offline_database();
    let empty_data: Vec<serde_json::Value> = vec![];

    let result = db
        .table(yang_db::table!("test_users"))
        .insert_batch(&empty_data)
        .await;

    match result {
        Ok(_) => panic!("空数据批量插入应该返回错误"),
        Err(e) => {
            println!("空数据批量插入正确返回错误: {}", e);
            assert!(e.to_string().contains("不能为空"));
        }
    }
}

// ==================== 事务中的查询执行测试 ====================
//
// 每个用例使用独立表名：此前 7 个用例共用 tx_test_users 且各自 DROP/CREATE，
// 并行执行时会互删表、断言到别的用例的数据（随机失败或假通过）。

#[tokio::test]
#[ignore = "需要真实 MySQL 实例"]
async fn test_transaction_insert() {
    let db = real_database().await;

    // 创建测试表
    let _ = db
        .execute("DROP TABLE IF EXISTS tx_test_users_insert")
        .await;
    db.execute(
        "CREATE TABLE tx_test_users_insert (
            id INT PRIMARY KEY AUTO_INCREMENT,
            name VARCHAR(100) NOT NULL,
            email VARCHAR(100),
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
    )
    .await
    .expect("创建事务测试表失败");

    // 开始事务
    let mut tx = db.transaction().await.expect("开始事务失败");

    // 在事务中插入数据
    let user_data = serde_json::json!({
        "name": "事务测试用户",
        "email": "tx_test@example.com"
    });

    let user_id = tx
        .table(yang_db::table!("tx_test_users_insert"))
        .insert(&user_data)
        .await
        .expect("事务中插入失败");
    println!("事务中插入成功，用户 ID: {}", user_id);

    // 提交事务
    tx.commit().await.expect("提交事务失败");

    // 验证数据已提交
    let count: i64 = db
        .table(yang_db::table!("tx_test_users_insert"))
        .count()
        .await
        .expect("查询事务提交后的记录数失败");
    assert_eq!(count, 1, "事务提交后应该有 1 条记录");
    println!("验证成功：事务提交后有 {} 条记录", count);

    // 清理测试表
    db.execute("DROP TABLE IF EXISTS tx_test_users_insert")
        .await
        .expect("清理事务测试表失败");
}

#[tokio::test]
#[ignore = "需要真实 MySQL 实例"]
async fn test_transaction_update() {
    let db = real_database().await;

    // 创建测试表
    let _ = db
        .execute("DROP TABLE IF EXISTS tx_test_users_update")
        .await;
    db.execute(
        "CREATE TABLE tx_test_users_update (
            id INT PRIMARY KEY AUTO_INCREMENT,
            name VARCHAR(100) NOT NULL,
            email VARCHAR(100),
            status INT DEFAULT 0
        )",
    )
    .await
    .expect("创建事务测试表失败");

    // 先插入一条数据
    let user_data = serde_json::json!({
        "name": "测试用户",
        "email": "test@example.com",
        "status": 0
    });

    let user_id = db
        .table(yang_db::table!("tx_test_users_update"))
        .insert(&user_data)
        .await
        .expect("插入测试数据失败");

    // 开始事务
    let mut tx = db.transaction().await.expect("开始事务失败");

    // 在事务中更新数据
    let update_data = serde_json::json!({
        "status": 1,
        "name": "已更新的用户"
    });

    let affected_rows = tx
        .table(yang_db::table!("tx_test_users_update"))
        .where_and(
            yang_db::field!("id"),
            yang_db::CompareOp::Eq,
            user_id as i64,
        )
        .expect("固定受控条件应合法")
        .update(&update_data)
        .await
        .expect("事务中更新失败");
    println!("事务中更新成功，影响 {} 行", affected_rows);
    assert_eq!(affected_rows, 1, "应该更新 1 行");

    // 提交事务
    tx.commit().await.expect("提交事务失败");
    println!("事务更新测试通过");

    // 清理测试表
    db.execute("DROP TABLE IF EXISTS tx_test_users_update")
        .await
        .expect("清理事务测试表失败");
}

#[tokio::test]
#[ignore = "需要真实 MySQL 实例"]
async fn test_transaction_delete() {
    let db = real_database().await;

    // 创建测试表
    let _ = db
        .execute("DROP TABLE IF EXISTS tx_test_users_delete")
        .await;
    db.execute(
        "CREATE TABLE tx_test_users_delete (
            id INT PRIMARY KEY AUTO_INCREMENT,
            name VARCHAR(100) NOT NULL,
            email VARCHAR(100)
        )",
    )
    .await
    .expect("创建事务测试表失败");

    // 先插入一条数据
    let user_data = serde_json::json!({
        "name": "待删除用户",
        "email": "delete@example.com"
    });

    let user_id = db
        .table(yang_db::table!("tx_test_users_delete"))
        .insert(&user_data)
        .await
        .expect("插入测试数据失败");

    // 开始事务
    let mut tx = db.transaction().await.expect("开始事务失败");

    // 在事务中删除数据
    let affected_rows = tx
        .table(yang_db::table!("tx_test_users_delete"))
        .where_and(
            yang_db::field!("id"),
            yang_db::CompareOp::Eq,
            user_id as i64,
        )
        .expect("固定受控条件应合法")
        .delete()
        .await
        .expect("事务中删除失败");
    println!("事务中删除成功，影响 {} 行", affected_rows);
    assert_eq!(affected_rows, 1, "应该删除 1 行");

    // 提交事务
    tx.commit().await.expect("提交事务失败");

    // 验证数据已删除
    let count: i64 = db
        .table(yang_db::table!("tx_test_users_delete"))
        .count()
        .await
        .expect("查询删除后的记录数失败");
    assert_eq!(count, 0, "事务提交后应该没有记录");
    println!("验证成功：事务提交后有 {} 条记录", count);

    // 清理测试表
    db.execute("DROP TABLE IF EXISTS tx_test_users_delete")
        .await
        .expect("清理事务测试表失败");
}

#[tokio::test]
#[ignore = "需要真实 MySQL 实例"]
async fn test_transaction_rollback_with_queries() {
    let db = real_database().await;

    // 创建测试表
    let _ = db
        .execute("DROP TABLE IF EXISTS tx_test_users_rollback")
        .await;
    db.execute(
        "CREATE TABLE tx_test_users_rollback (
            id INT PRIMARY KEY AUTO_INCREMENT,
            name VARCHAR(100) NOT NULL,
            email VARCHAR(100)
        )",
    )
    .await
    .expect("创建事务测试表失败");

    // 获取初始记录数
    let initial_count: i64 = db
        .table(yang_db::table!("tx_test_users_rollback"))
        .count()
        .await
        .expect("获取初始记录数失败");

    // 开始事务
    let mut tx = db.transaction().await.expect("开始事务失败");

    // 在事务中插入数据
    let user_data = serde_json::json!({
        "name": "临时用户",
        "email": "temp@example.com"
    });

    tx.table(yang_db::table!("tx_test_users_rollback"))
        .insert(&user_data)
        .await
        .expect("事务中插入失败");
    println!("事务中插入成功");

    // 回滚事务
    tx.rollback().await.expect("回滚事务失败");

    // 验证数据已回滚
    let final_count: i64 = db
        .table(yang_db::table!("tx_test_users_rollback"))
        .count()
        .await
        .expect("获取最终记录数失败");

    assert_eq!(
        initial_count, final_count,
        "回滚后记录数应该与初始记录数相同"
    );
    println!(
        "验证成功：回滚后记录数 {} = 初始记录数 {}",
        final_count, initial_count
    );

    // 清理测试表
    db.execute("DROP TABLE IF EXISTS tx_test_users_rollback")
        .await
        .expect("清理事务测试表失败");
}

#[tokio::test]
#[ignore = "需要真实 MySQL 实例"]
async fn test_transaction_multiple_operations() {
    let db = real_database().await;

    // 创建测试表
    let _ = db.execute("DROP TABLE IF EXISTS tx_test_users_multi").await;
    db.execute(
        "CREATE TABLE tx_test_users_multi (
            id INT PRIMARY KEY AUTO_INCREMENT,
            name VARCHAR(100) NOT NULL,
            email VARCHAR(100),
            status INT DEFAULT 0
        )",
    )
    .await
    .expect("创建事务测试表失败");

    // 开始事务
    let mut tx = db.transaction().await.expect("开始事务失败");

    // 操作 1: 插入用户
    let user1_data = serde_json::json!({
        "name": "用户1",
        "email": "user1@example.com",
        "status": 0
    });

    let user1_id = tx
        .table(yang_db::table!("tx_test_users_multi"))
        .insert(&user1_data)
        .await
        .expect("插入用户1失败");
    println!("插入用户1成功，ID: {}", user1_id);

    // 操作 2: 插入另一个用户
    let user2_data = serde_json::json!({
        "name": "用户2",
        "email": "user2@example.com",
        "status": 0
    });

    let user2_id = tx
        .table(yang_db::table!("tx_test_users_multi"))
        .insert(&user2_data)
        .await
        .expect("插入用户2失败");
    println!("插入用户2成功，ID: {}", user2_id);

    // 操作 3: 更新第一个用户
    let update_data = serde_json::json!({
        "status": 1
    });

    let affected = tx
        .table(yang_db::table!("tx_test_users_multi"))
        .where_and(
            yang_db::field!("id"),
            yang_db::CompareOp::Eq,
            user1_id as i64,
        )
        .expect("固定受控条件应合法")
        .update(&update_data)
        .await
        .expect("更新用户1失败");
    println!("更新用户1成功，影响 {} 行", affected);

    // 提交事务
    tx.commit().await.expect("提交事务失败");

    // 验证所有操作都已提交
    let count: i64 = db
        .table(yang_db::table!("tx_test_users_multi"))
        .count()
        .await
        .expect("获取记录数失败");
    assert_eq!(count, 2, "应该有 2 条记录");
    println!("验证成功：事务提交后有 {} 条记录", count);

    // 清理测试表
    db.execute("DROP TABLE IF EXISTS tx_test_users_multi")
        .await
        .expect("清理事务测试表失败");
}

#[tokio::test]
#[ignore = "需要真实 MySQL 实例"]
async fn test_transaction_update_without_where_fails() {
    let db = real_database().await;

    // 创建测试表
    let _ = db
        .execute("DROP TABLE IF EXISTS tx_test_users_update_no_where")
        .await;
    db.execute(
        "CREATE TABLE tx_test_users_update_no_where (
            id INT PRIMARY KEY AUTO_INCREMENT,
            name VARCHAR(100) NOT NULL
        )",
    )
    .await
    .expect("创建事务测试表失败");

    // 开始事务
    let mut tx = db.transaction().await.expect("开始事务失败");

    // 尝试不带 WHERE 条件的更新（应该失败）
    let update_data = serde_json::json!({
        "name": "全部更新"
    });

    let update_result = tx
        .table(yang_db::table!("tx_test_users_update_no_where"))
        .update(&update_data)
        .await;

    // 验证返回 MissingWhereClause 错误
    assert!(update_result.is_err(), "不带 WHERE 条件的更新应该失败");

    if let Err(e) = update_result {
        println!("预期的错误: {}", e);
        assert!(
            matches!(e, yang_db::DbError::MissingWhereClause),
            "应该返回 MissingWhereClause 错误"
        );
    }

    // 回滚事务
    tx.rollback().await.expect("回滚事务失败");

    // 清理测试表
    db.execute("DROP TABLE IF EXISTS tx_test_users_update_no_where")
        .await
        .expect("清理事务测试表失败");
}

#[tokio::test]
#[ignore = "需要真实 MySQL 实例"]
async fn test_transaction_delete_without_where_fails() {
    let db = real_database().await;

    // 创建测试表
    let _ = db
        .execute("DROP TABLE IF EXISTS tx_test_users_delete_no_where")
        .await;
    db.execute(
        "CREATE TABLE tx_test_users_delete_no_where (
            id INT PRIMARY KEY AUTO_INCREMENT,
            name VARCHAR(100) NOT NULL
        )",
    )
    .await
    .expect("创建事务测试表失败");

    // 开始事务
    let mut tx = db.transaction().await.expect("开始事务失败");

    // 尝试不带 WHERE 条件的删除（应该失败）
    let delete_result = tx
        .table(yang_db::table!("tx_test_users_delete_no_where"))
        .delete()
        .await;

    // 验证返回 MissingWhereClause 错误
    assert!(delete_result.is_err(), "不带 WHERE 条件的删除应该失败");

    if let Err(e) = delete_result {
        println!("预期的错误: {}", e);
        assert!(
            matches!(e, yang_db::DbError::MissingWhereClause),
            "应该返回 MissingWhereClause 错误"
        );
    }

    // 回滚事务
    tx.rollback().await.expect("回滚事务失败");

    // 清理测试表
    db.execute("DROP TABLE IF EXISTS tx_test_users_delete_no_where")
        .await
        .expect("清理事务测试表失败");
}
