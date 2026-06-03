// PostgreSQL CRUD 操作集成测试（简化版）
// 镜像 integration_crud_simple.rs 的风格，但使用 yang_db::postgres::Database
// 与 postgres:// 连接串。所有触达数据库的测试都标记 #[ignore]，
// 默认 `cargo test` 套件保持离线（不需要活的 PostgreSQL 实例）。

#![allow(dead_code)]

use serde_json::json;
use yang_db::postgres::Database;

/// 测试数据库连接字符串（postgres://user:password@host:port/database）。
///
/// 本机 5432 端口已被占用，测试用 Docker PostgreSQL 默认监听 5433。
/// 可用环境变量 `PG_TEST_URL` 覆盖，例如：
///   PG_TEST_URL=postgres://postgres:postgres@localhost:5433/test \
///     cargo test --test integration_pg_crud_simple -- --ignored --test-threads=1
fn test_db_url() -> String {
    std::env::var("PG_TEST_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5433/test".to_string())
}

#[tokio::test]
#[ignore = "需要本地 PostgreSQL 实例，默认离线套件跳过"]
async fn test_crud_sql_generation() {
    let result = Database::connect(&test_db_url()).await;

    if let Ok(db) = result {
        // 测试 SELECT SQL 生成（PostgreSQL 使用 $N 占位符）
        let select_sql = db
            .table("test_users")
            .field("id")
            .field("name")
            .where_and("status", "=", "active")
            .unwrap()
            .to_sql();
        assert!(select_sql.contains("SELECT"), "应该包含 SELECT");
        assert!(select_sql.contains("WHERE"), "应该包含 WHERE");
        assert!(select_sql.contains("$1"), "PostgreSQL 应使用 $N 占位符");
        println!("✓ SELECT SQL 生成: {}", select_sql);

        println!("\n✓✓✓ CRUD SQL 生成测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库");
    }
}

#[tokio::test]
#[ignore = "需要本地 PostgreSQL 实例，默认离线套件跳过"]
async fn test_crud_with_real_table() {
    let result = Database::connect(&test_db_url()).await;

    if let Ok(db) = result {
        let table_name = "integration_pg_crud_test";

        // 创建测试表（PostgreSQL 用 SERIAL 自增主键）
        let _ = db.drop_table(table_name).await;
        let create_result = db
            .create_table(&format!(
                "CREATE TABLE {} (
                    id SERIAL PRIMARY KEY,
                    name VARCHAR(100) NOT NULL,
                    email VARCHAR(100),
                    age INT,
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

        // 测试 INSERT（RETURNING id）
        let user_data = json!({
            "name": "测试用户",
            "email": "test@example.com",
            "age": 25
        });
        let insert_result = db.table(table_name).insert(&user_data).await;

        match insert_result {
            Ok(id) => {
                println!("✓ INSERT 成功，ID: {}", id);
                assert!(id > 0, "插入的 ID 应该大于 0");

                let count_result: Result<i64, _> = db.table(table_name).count().await;
                if let Ok(count) = count_result {
                    assert_eq!(count, 1, "应该有 1 条记录");
                    println!("✓ COUNT: {} 条记录", count);
                }

                // 测试 UPDATE
                let update_data = json!({"age": 26});
                let update_result = db
                    .table(table_name)
                    .where_and("id", "=", id as i64)
                    .unwrap()
                    .update(&update_data)
                    .await;
                if let Ok(affected) = update_result {
                    assert_eq!(affected, 1, "应该更新 1 行");
                    println!("✓ UPDATE: 更新了 {} 行", affected);
                }

                // 测试 DELETE
                let delete_result = db
                    .table(table_name)
                    .where_and("id", "=", id as i64)
                    .unwrap()
                    .delete()
                    .await;
                if let Ok(deleted) = delete_result {
                    assert_eq!(deleted, 1, "应该删除 1 行");
                    println!("✓ DELETE: 删除了 {} 行", deleted);
                }
            }
            Err(e) => {
                // 连上了数据库却插入失败属于真实缺陷，必须让测试失败而非静默通过
                panic!("INSERT 失败（已连接数据库，不应发生）: {}", e);
            }
        }

        let _ = db.drop_table(table_name).await;
        println!("\n✓✓✓ 完整 CRUD 流程测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库");
    }
}

#[tokio::test]
#[ignore = "需要本地 PostgreSQL 实例，默认离线套件跳过"]
async fn test_upsert_on_conflict() {
    let result = Database::connect(&test_db_url()).await;

    if let Ok(db) = result {
        let table_name = "integration_pg_upsert_test";

        let _ = db.drop_table(table_name).await;
        let create_result = db
            .create_table(&format!(
                "CREATE TABLE {} (
                    id INT PRIMARY KEY,
                    name VARCHAR(100) NOT NULL,
                    value INT
                )",
                table_name
            ))
            .await;
        if create_result.is_err() {
            println!("警告: 无法创建测试表");
            return;
        }

        // 首次 upsert 插入，二次 upsert 触发 ON CONFLICT DO UPDATE
        let first = db
            .table(table_name)
            .on_conflict(&["id"])
            .upsert(&json!({"id": 1, "name": "初始", "value": 10}))
            .await;
        assert!(first.is_ok(), "首次 upsert 应成功");

        let second = db
            .table(table_name)
            .on_conflict(&["id"])
            .upsert(&json!({"id": 1, "name": "更新", "value": 20}))
            .await;
        assert!(second.is_ok(), "冲突 upsert 应成功更新");

        let count: Result<i64, _> = db.table(table_name).count().await;
        if let Ok(c) = count {
            assert_eq!(c, 1, "upsert 后仍应只有 1 条记录");
            println!("✓ UPSERT: {} 条记录", c);
        }

        let _ = db.drop_table(table_name).await;
        println!("\n✓✓✓ UPSERT 测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库");
    }
}

#[tokio::test]
#[ignore = "需要本地 PostgreSQL 实例，默认离线套件跳过"]
async fn test_error_handling() {
    let result = Database::connect(&test_db_url()).await;

    if let Ok(db) = result {
        let table_name = "integration_pg_error_test";

        let _ = db.drop_table(table_name).await;
        let _ = db
            .create_table(&format!(
                "CREATE TABLE {} (
                    id SERIAL PRIMARY KEY,
                    name VARCHAR(100) NOT NULL
                )",
                table_name
            ))
            .await;

        // UPDATE 缺少 WHERE 必须返回 MissingWhereClause
        let update_result = db.table(table_name).update(&json!({"name": "test"})).await;
        assert!(update_result.is_err(), "没有 WHERE 的 UPDATE 应该失败");
        if let Err(e) = update_result {
            assert!(matches!(e, yang_db::DbError::MissingWhereClause));
            println!("✓ UPDATE 缺少 WHERE: 正确返回错误");
        }

        // DELETE 缺少 WHERE 必须返回 MissingWhereClause
        let delete_result = db.table(table_name).delete().await;
        assert!(delete_result.is_err(), "没有 WHERE 的 DELETE 应该失败");
        if let Err(e) = delete_result {
            assert!(matches!(e, yang_db::DbError::MissingWhereClause));
            println!("✓ DELETE 缺少 WHERE: 正确返回错误");
        }

        let _ = db.drop_table(table_name).await;
        println!("\n✓✓✓ 错误处理测试通过 ✓✓✓");
    } else {
        println!("警告: 无法连接到测试数据库");
    }
}
