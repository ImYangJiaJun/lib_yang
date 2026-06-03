// PostgreSQL 基础用法示例
// 演示 connect + table().insert / select / upsert 的调用方式。
//
// 运行需要一个本地 PostgreSQL 实例：
//   cargo run --example pg_basic -p yang-db
// 在没有数据库时本示例不会被自动执行（仅用于演示 / 编译验证）。

use serde::{Deserialize, Serialize};
use serde_json::json;
use yang_db::postgres::Database;

/// 连接字符串（postgres://user:password@host:port/database）。
///
/// 本机 5432 端口常被占用，示例默认连 5433（测试用 Docker PostgreSQL）。
/// 可用环境变量 `PG_TEST_URL` 覆盖。
fn db_url() -> String {
    std::env::var("PG_TEST_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5433/test".to_string())
}

/// 演示用的行类型，select 返回时由 sqlx 从行解码
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
struct User {
    id: i32,
    name: String,
    age: Option<i32>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== PostgreSQL 基础用法示例 ===\n");

    // 1. 连接数据库
    let db = Database::connect(&db_url()).await?;
    println!("✓ 数据库连接成功\n");

    let table_name = "example_pg_users";

    // 准备表（PostgreSQL 用 SERIAL 自增主键）
    let _ = db.drop_table(table_name).await;
    db.create_table(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            name VARCHAR(100) NOT NULL,
            age INT
        )",
        table_name
    ))
    .await?;
    println!("✓ 测试表已创建\n");

    // 2. INSERT —— 通过 RETURNING 取回自增主键
    let new_id = db
        .table(table_name)
        .insert(&json!({"name": "张三", "age": 25}))
        .await?;
    println!("✓ INSERT 成功，新行 id = {}\n", new_id);

    // 3. UPSERT —— INSERT ... ON CONFLICT (id) DO UPDATE
    let affected = db
        .table(table_name)
        .on_conflict(&["id"])
        .upsert(&json!({"id": new_id as i64, "name": "张三(改)", "age": 26}))
        .await?;
    println!("✓ UPSERT 完成，影响 {} 行\n", affected);

    // 4. SELECT —— 带 WHERE 条件，解码为 User
    let users: Vec<User> = db
        .table(table_name)
        .field("id")
        .field("name")
        .field("age")
        .where_and("age", ">=", 18)?
        .order("id", true)
        .limit(10)
        .select()
        .await?;
    println!("✓ SELECT 命中 {} 行: {:?}\n", users.len(), users);

    // 清理
    let _ = db.drop_table(table_name).await;
    println!("=== 示例结束 ===");
    Ok(())
}
