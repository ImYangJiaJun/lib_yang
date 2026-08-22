//! 受控服务端表达式（`SqlExpr`）的 SQL 生成测试
//!
//! 覆盖：UPDATE SET 表达式赋值、INSERT VALUES 表达式列、WHERE 列↔表达式比较、
//! SELECT 标量表达式投影与事务行锁渲染。全部为纯 SQL 文本/参数断言，不依赖数据库
//! （懒连接池只校验 URL，不建立真实连接）。

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use crate::mysql::condition::{Condition, SqlValue};
use crate::mysql::query_builder::SqlGenerator;
use crate::{CompareOp, FieldRef, QueryBuilder, SqlExpr, TableRef};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::collections::HashMap;

/// 懒连接池：只校验 URL，不建立真实连接；本文件测试只生成 SQL。
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

#[test]
fn test_build_update_set_expr_only() {
    let mut generator = SqlGenerator::new();
    let data = serde_json::json!({});
    generator
        .build_update(
            "password_reset",
            &data,
            &HashMap::new(),
            &[Condition::Eq("id".to_string(), SqlValue::Int(7))],
            &[("used_at".to_string(), SqlExpr::unix_timestamp())],
        )
        .unwrap();

    assert_eq!(
        generator.get_sql(),
        "UPDATE `password_reset` SET `used_at` = UNIX_TIMESTAMP() WHERE `id` = ?"
    );
    // 表达式无参数，唯一参数来自 WHERE
    assert_eq!(generator.get_params().len(), 1);
    assert!(matches!(generator.get_params()[0], SqlValue::Int(7)));
}

#[test]
fn test_build_update_mixes_json_and_expr_assignments() {
    let mut generator = SqlGenerator::new();
    let data = serde_json::json!({"attempts": 1});
    generator
        .build_update(
            "password_reset",
            &data,
            &HashMap::new(),
            &[Condition::Eq("id".to_string(), SqlValue::Int(7))],
            &[("used_at".to_string(), SqlExpr::unix_timestamp())],
        )
        .unwrap();

    // JSON 列在前、表达式列在后，参数顺序与占位符一致
    assert_eq!(
        generator.get_sql(),
        "UPDATE `password_reset` SET `attempts` = ?, `used_at` = UNIX_TIMESTAMP() WHERE `id` = ?"
    );
    assert_eq!(generator.get_params().len(), 2);
    assert!(matches!(generator.get_params()[0], SqlValue::Int(1)));
    assert!(matches!(generator.get_params()[1], SqlValue::Int(7)));
}

#[test]
fn test_build_update_still_requires_where_with_exprs() {
    let mut generator = SqlGenerator::new();
    let data = serde_json::json!({});
    let result = generator.build_update(
        "password_reset",
        &data,
        &HashMap::new(),
        &[],
        &[("used_at".to_string(), SqlExpr::unix_timestamp())],
    );
    assert!(matches!(result, Err(crate::DbError::MissingWhereClause)));
}

#[test]
fn test_build_insert_values_expr() {
    let mut generator = SqlGenerator::new();
    let data = serde_json::json!({"user_id": 42});
    generator
        .build_insert(
            "login_token",
            &data,
            &HashMap::new(),
            &[("expires_at".to_string(), SqlExpr::unix_timestamp_add(900))],
        )
        .unwrap();

    assert_eq!(
        generator.get_sql(),
        "INSERT INTO `login_token` (`user_id`, `expires_at`) VALUES (?, UNIX_TIMESTAMP() + ?)"
    );
    let params = generator.get_params();
    assert_eq!(params.len(), 2);
    assert!(matches!(params[0], SqlValue::Int(42)));
    // 偏移秒数只进绑定参数，不进 SQL 文本
    assert!(matches!(params[1], SqlValue::Int(900)));
}

#[test]
fn test_build_insert_empty_data_allowed_only_with_exprs() {
    let data = serde_json::json!({});

    let mut generator = SqlGenerator::new();
    assert!(generator
        .build_insert("login_token", &data, &HashMap::new(), &[])
        .is_err());

    let mut generator = SqlGenerator::new();
    generator
        .build_insert(
            "login_token",
            &data,
            &HashMap::new(),
            &[("expires_at".to_string(), SqlExpr::unix_timestamp())],
        )
        .unwrap();
    assert_eq!(
        generator.get_sql(),
        "INSERT INTO `login_token` (`expires_at`) VALUES (UNIX_TIMESTAMP())"
    );
    assert!(generator.get_params().is_empty());
}

#[tokio::test]
async fn test_where_expr_renders_column_to_expression_comparison() {
    let pool = lazy_pool();
    let sql = QueryBuilder::from_pool(&pool, &table("password_reset"))
        .where_expr(
            &field("expires_at"),
            CompareOp::Gt,
            SqlExpr::unix_timestamp(),
        )
        .unwrap()
        .try_to_sql()
        .unwrap();

    assert_eq!(
        sql,
        "SELECT * FROM `password_reset` WHERE `expires_at` > UNIX_TIMESTAMP()"
    );
}

#[tokio::test]
async fn test_where_expr_rejects_like_operator() {
    let pool = lazy_pool();
    let result = QueryBuilder::from_pool(&pool, &table("password_reset")).where_expr(
        &field("expires_at"),
        CompareOp::Like,
        SqlExpr::unix_timestamp(),
    );
    assert!(matches!(
        result,
        Err(crate::DbError::UnsupportedOperator(_))
    ));
}

#[tokio::test]
async fn test_select_expr_without_plain_fields() {
    let pool = lazy_pool();
    let sql = QueryBuilder::from_pool(&pool, &table("password_reset"))
        .select_expr(SqlExpr::unix_timestamp_add(30), &field("deadline"))
        .try_to_sql()
        .unwrap();

    assert_eq!(
        sql,
        "SELECT UNIX_TIMESTAMP() + ? AS `deadline` FROM `password_reset`"
    );
}

#[tokio::test]
async fn test_locked_select_with_expr_projection_collects_params_in_order() {
    let pool = lazy_pool();
    let builder = QueryBuilder::from_pool(&pool, &table("password_reset"))
        .field(&field("id"))
        .select_expr(SqlExpr::unix_timestamp(), &field("server_now"))
        .where_expr(
            &field("expires_at"),
            CompareOp::Gt,
            SqlExpr::unix_timestamp_add(600),
        )
        .unwrap()
        .where_and(&field("consumed"), CompareOp::Eq, false);

    let (sql, params) = builder
        .render_for_transaction(Some(crate::RowLock::ForUpdate))
        .unwrap();

    assert_eq!(
        sql,
        "SELECT `id`, UNIX_TIMESTAMP() AS `server_now` FROM `password_reset` \
         WHERE (`expires_at` > UNIX_TIMESTAMP() + ? AND `consumed` = ?) FOR UPDATE"
    );
    // 参数顺序 = SQL 占位符顺序：表达式偏移在前，WHERE 绑定值在后
    assert_eq!(params.len(), 2);
    assert!(matches!(params[0], SqlValue::Int(600)));
    assert!(matches!(params[1], SqlValue::Bool(false)));
}
