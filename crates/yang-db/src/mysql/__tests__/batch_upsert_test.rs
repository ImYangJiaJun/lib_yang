//! 批量 UPSERT 的生成器与批大小推导测试
//!
//! 全部为纯 SQL 文本 / 参数断言，不连库（懒连接池只校验 URL）。安全网是
//! `single_row_upsert_batch_matches_upsert_text`：`append_upsert_tail` 抽取后，
//! 单行 `build_upsert` 的输出必须**逐字符不变**。

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use crate::mysql::query_builder::{derive_upsert_batch_size, QueryBuilder, SqlGenerator};
use crate::{FieldRef, SqlExpr, TableRef};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::collections::HashMap;

/// 懒连接池：只校验 URL，不建立真实连接；拒绝类用例在触达数据库前就返回。
fn lazy_pool() -> MySqlPool {
    MySqlPoolOptions::new()
        .max_connections(1)
        .connect_lazy("mysql://root:111111@localhost:3306/test")
        .expect("无法解析测试数据库 URL")
}

fn field(name: &str) -> FieldRef {
    FieldRef::new(name).expect("测试字段名必须合法")
}

fn table(name: &str) -> TableRef {
    TableRef::new(name).expect("测试表名必须合法")
}

fn fields(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_string()).collect()
}

#[test]
fn upsert_batch_sql_shape() {
    let data = vec![
        serde_json::json!({"name": "Alice", "age": 30, "email": "a@b.com"}),
        serde_json::json!({"name": "Bob", "age": 31, "email": "b@b.com"}),
    ];
    let mut generator = SqlGenerator::new();
    generator
        .build_upsert_batch("users", &data, &HashMap::new(), &fields(&["name"]))
        .unwrap();

    let sql = generator.get_sql();
    assert!(
        sql.starts_with("INSERT INTO `users` ("),
        "SQL 应以 'INSERT INTO `users` (' 开头，实际: {sql}"
    );
    // 尾巴只带白名单里的 `name`，且列序回归 BTreeMap（age, email, name）
    assert!(
        sql.contains(") VALUES (?, ?, ?), (?, ?, ?) ON DUPLICATE KEY UPDATE `name`=VALUES(`name`)"),
        "SQL 形状不符，实际: {sql}"
    );
    // '(' 计数 = 字段列表 1 + VALUES 两行 2 + VALUES() 尾巴 1
    assert_eq!(sql.matches('(').count(), 4, "括号计数不符，实际: {sql}");
    // ODKU 尾巴不产生占位符：参数数恒 = 行数 × 列数
    assert_eq!(generator.get_params().len(), 6, "实际: {sql}");
    // 未列入赋值列的列绝不能出现在尾巴里（created_at 一类列的回归钉子）
    assert!(
        !sql.contains("VALUES(`age`)"),
        "赋值列白名单被突破，实际: {sql}"
    );
}

#[test]
fn upsert_batch_tail_follows_insert_column_order() {
    let data = vec![serde_json::json!({"name": "Alice", "email": "a@b.com", "age": 30})];
    let mut generator = SqlGenerator::new();
    // 传入顺序刻意与 INSERT 列序（age, email, name）相反
    generator
        .build_upsert_batch("users", &data, &HashMap::new(), &fields(&["name", "email"]))
        .unwrap();

    let sql = generator.get_sql();
    assert!(
        sql.ends_with("ON DUPLICATE KEY UPDATE `email`=VALUES(`email`), `name`=VALUES(`name`)"),
        "尾巴列序应按 INSERT 列序收敛，实际: {sql}"
    );
}

#[test]
fn upsert_batch_rejects_heterogeneous_rows() {
    use crate::error::DbError;

    let data = vec![
        serde_json::json!({"name": "Alice", "age": 30}),
        serde_json::json!({"name": "Bob", "email": "b@b.com"}),
    ];
    let mut generator = SqlGenerator::new();
    let result = generator.build_upsert_batch("users", &data, &HashMap::new(), &fields(&["name"]));

    match result {
        Err(DbError::InvalidArgument(message)) => {
            assert!(
                message.contains("第 1 条"),
                "错误消息应含行号，实际: {message}"
            );
        }
        other => panic!("异构行集应返回 InvalidArgument，实得 {other:?}"),
    }
}

#[test]
fn upsert_batch_rejects_empty_rows() {
    use crate::error::DbError;

    let data: Vec<serde_json::Value> = vec![];
    let mut generator = SqlGenerator::new();
    let result = generator.build_upsert_batch("users", &data, &HashMap::new(), &fields(&["name"]));

    assert!(
        matches!(result, Err(DbError::SerializationError(_))),
        "空行集应返回 SerializationError，实得 {result:?}"
    );
}

#[test]
fn upsert_batch_rejects_empty_update_fields() {
    use crate::error::DbError;

    let data = vec![serde_json::json!({"name": "Alice"})];
    let mut generator = SqlGenerator::new();
    let result = generator.build_upsert_batch("users", &data, &HashMap::new(), &[]);

    match result {
        Err(DbError::InvalidArgument(message)) => {
            assert_eq!(message, "批量 upsert 的赋值列不能为空");
        }
        other => panic!("空赋值列应返回 InvalidArgument，实得 {other:?}"),
    }
}

#[test]
fn upsert_batch_rejects_update_field_outside_insert_columns() {
    use crate::error::DbError;

    let data = vec![serde_json::json!({"name": "Alice"})];
    let mut generator = SqlGenerator::new();
    let result = generator.build_upsert_batch(
        "users",
        &data,
        &HashMap::new(),
        &fields(&["name", "created_at"]),
    );

    match result {
        Err(DbError::InvalidArgument(message)) => {
            assert!(
                message.contains("`created_at`"),
                "错误消息应点名越界列，实际: {message}"
            );
        }
        other => panic!("赋值列超出插入列集应返回 InvalidArgument，实得 {other:?}"),
    }
}

#[test]
fn single_row_upsert_batch_matches_upsert_text() {
    let data = serde_json::json!({"id": 1, "name": "Alice", "email": "a@b.com"});
    let all_columns: Vec<String> = data.as_object().unwrap().keys().cloned().collect();

    let mut single = SqlGenerator::new();
    single
        .build_upsert("users", &data, &HashMap::new())
        .unwrap();

    let mut batch = SqlGenerator::new();
    batch
        .build_upsert_batch(
            "users",
            std::slice::from_ref(&data),
            &HashMap::new(),
            &all_columns,
        )
        .unwrap();

    // 抽取 append_upsert_tail 的硬要求：单行 build_upsert 输出**逐字节不变**。
    // 这条逐字断言是必须的——只比对两条路径会让共用助手的改动同时改掉两侧、一起保持相等。
    assert_eq!(
        single.get_sql(),
        "INSERT INTO `users` (`email`, `id`, `name`) VALUES (?, ?, ?) \
         ON DUPLICATE KEY UPDATE `email`=VALUES(`email`), `id`=VALUES(`id`), `name`=VALUES(`name`)"
    );
    // 赋值列 = 全列时两侧文本逐字符相同
    assert_eq!(single.get_sql(), batch.get_sql());
    assert_eq!(single.get_params().len(), batch.get_params().len());
}

#[test]
fn derive_upsert_batch_size_caps_by_column_count() {
    // 窄表（10 列）：65535 / 10 = 6553，远高于 requested，保持不变
    assert_eq!(derive_upsert_batch_size(10, 500), 500);
    // 边界：131 × 500 = 65500 ≤ 65535，恰好不缩批
    assert_eq!(derive_upsert_batch_size(131, 500), 500);
    // 宽表：65535 / 200 = 327
    assert_eq!(derive_upsert_batch_size(200, 500), 327);
    // 列数为 0（非对象数据，留给生成层报错）时不做缩批
    assert_eq!(derive_upsert_batch_size(0, 500), 500);
}

#[tokio::test]
async fn upsert_batch_with_size_rejects_zero_batch_size() {
    let pool = lazy_pool();
    let rows = vec![serde_json::json!({"name": "Alice"})];
    let result = QueryBuilder::from_pool(&pool, &table("users"))
        .upsert_batch_with_size(&rows, &fields(&["name"]), 0)
        .await;

    assert!(
        matches!(result, Err(crate::DbError::SerializationError(_))),
        "batch_size = 0 应返回 SerializationError，实得 {result:?}"
    );
}

#[tokio::test]
async fn upsert_batch_with_size_rejects_empty_data() {
    let pool = lazy_pool();
    let rows: Vec<serde_json::Value> = vec![];
    let result = QueryBuilder::from_pool(&pool, &table("users"))
        .upsert_batch_with_size(&rows, &fields(&["name"]), 500)
        .await;

    assert!(
        matches!(result, Err(crate::DbError::SerializationError(_))),
        "空数据应返回 SerializationError，实得 {result:?}"
    );
}

#[tokio::test]
async fn upsert_batch_rejects_set_expr() {
    let pool = lazy_pool();
    let rows = vec![serde_json::json!({"name": "Alice", "consumed_at": 0})];
    let result = QueryBuilder::from_pool(&pool, &table("users"))
        .set_expr(&field("consumed_at"), SqlExpr::unix_timestamp())
        .upsert_batch(&rows, &fields(&["name"]))
        .await;

    assert!(
        matches!(result, Err(crate::DbError::InvalidArgument(_))),
        "批量 upsert 携带 set_expr 应 fail-closed，实得 {result:?}"
    );
}
