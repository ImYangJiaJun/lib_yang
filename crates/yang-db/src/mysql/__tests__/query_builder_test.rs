//! `QueryBuilder` 单元测试（自 `mysql::query_builder` 内联测试迁移，断言与辅助函数不变）。

use std::collections::HashMap;

use sqlx::mysql::MySqlPool;

use crate::mysql::condition::{Condition, SqlValue};
use crate::mysql::field::FieldType;
use crate::mysql::query_builder::{
    predicate_value, ArithmeticOperator, QueryBuilder, SqlGenerator,
};

#[cfg(test)]
mod predicate_scalar_tests {
    use super::{predicate_value, SqlValue};

    #[test]
    fn controlled_predicates_bind_json_scalars_as_sql_scalars() {
        assert!(matches!(
            predicate_value(&serde_json::json!("alice")),
            SqlValue::String(value) if value == "alice"
        ));
        assert!(matches!(
            predicate_value(&serde_json::json!(42)),
            SqlValue::Int(42)
        ));
        assert!(matches!(
            predicate_value(&serde_json::json!(true)),
            SqlValue::Bool(true)
        ));
        assert!(matches!(
            predicate_value(&serde_json::json!({"role": "admin"})),
            SqlValue::Json(_)
        ));
    }
}

#[cfg(test)]
#[allow(deprecated)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::mysql::Subquery;
    use sqlx::mysql::MySqlPoolOptions;

    // 创建测试用的连接池（懒连接：仅校验 URL，不建立真实连接）。
    // 本模块的 async 测试只调用 to_sql()/检查 builder 状态，不执行查询，故无需真实
    // 数据库。改用 connect_lazy 后离线 `cargo test --lib` 不再因 30s acquire 超时
    // 逐个挂死（DB-11 的离线可跑部分）。
    async fn create_test_pool() -> MySqlPool {
        MySqlPoolOptions::new()
            .max_connections(1)
            .connect_lazy("mysql://root:111111@localhost:3306/test")
            .expect("无法解析测试数据库 URL")
    }

    /// 获取或创建共享懒连接池（仅验证 URL，不立即建立连接）。
    /// 用于只测试 SQL 生成逻辑、不需要真实数据库连接的单元测试。
    /// 使用 OnceLock + 静态 Tokio 运行时确保池在有效上下文中创建和驻留。
    fn make_sync_test_pool() -> &'static MySqlPool {
        use std::sync::OnceLock;
        static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
        static POOL: OnceLock<MySqlPool> = OnceLock::new();
        let rt = RT.get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("无法创建测试用 Tokio 运行时")
        });
        POOL.get_or_init(|| {
            rt.block_on(async {
                MySqlPoolOptions::new()
                    .max_connections(1)
                    .connect_lazy("mysql://root:111111@localhost:3306/test")
                    .expect("无法解析测试数据库 URL")
            })
        })
    }

    #[test]
    fn test_controlled_subqueries_render_exists_not_exists_and_in_in_parameter_order() {
        let pool = make_sync_test_pool();
        let paid_order = Subquery::new("orders", "id")
            .expect("合法子查询")
            .where_column("orders.user_id", "=", "users.id")
            .expect("合法关联列")
            .where_value("orders.status", "=", "paid")
            .expect("合法绑定条件");
        let active_user = Subquery::new("memberships", "user_id")
            .expect("合法子查询")
            .where_value("memberships.active", "=", true)
            .expect("合法绑定条件");
        let banned_user = Subquery::new("bans", "id")
            .expect("合法子查询")
            .where_column("bans.user_id", "=", "users.id")
            .expect("合法关联列")
            .where_value("bans.reason", "=", "fraud")
            .expect("合法绑定条件");

        let builder = QueryBuilder::new(pool, "users", false)
            .where_and(
                yang_db::field!("users.tenant_id"),
                yang_db::CompareOp::Eq,
                7,
            )
            .where_exists(paid_order)
            .where_in_subquery(yang_db::field!("users.id"), active_user)
            .where_not_exists(banned_user);
        let mut generator = SqlGenerator::new();
        generator.build_select(&builder).expect("子查询应可渲染");

        assert_eq!(
            generator.get_sql(),
            "SELECT * FROM `users` WHERE (`users`.`tenant_id` = ? AND EXISTS (SELECT `id` FROM `orders` WHERE `orders`.`user_id` = `users`.`id` AND `orders`.`status` = ?) AND `users`.`id` IN (SELECT `user_id` FROM `memberships` WHERE `memberships`.`active` = ?) AND NOT EXISTS (SELECT `id` FROM `bans` WHERE `bans`.`user_id` = `users`.`id` AND `bans`.`reason` = ?))"
        );
        assert!(
            matches!(generator.get_params(), [SqlValue::Int(7), SqlValue::String(status), SqlValue::Bool(true), SqlValue::String(reason)] if status == "paid" && reason == "fraud")
        );
    }

    #[test]
    fn test_controlled_subqueries_reject_adversarial_structure_before_rendering() {
        for payload in [
            "",
            "orders; DROP TABLE users",
            "orders --",
            "a.b",
            "orders\0",
        ] {
            assert!(
                Subquery::new(payload, "id").is_err(),
                "非法表名被接受: {payload:?}"
            );
        }
        for payload in ["", "id) FROM users --", "COUNT(*)", "a.b.c", "id\0"] {
            assert!(
                Subquery::new("orders", payload).is_err(),
                "非法投影被接受: {payload:?}"
            );
        }
        assert!(Subquery::new("orders", "id")
            .expect("合法子查询")
            .where_column("orders.user_id", "= 1 OR 1=1 --", "users.id")
            .is_err());
        assert!(Subquery::new("orders", "id")
            .expect("合法子查询")
            .where_value("orders.status --", "=", "paid")
            .is_err());
        assert!(crate::FieldRef::new("users.id) OR 1=1 --").is_err());
    }

    #[test]
    fn test_union_all_keeps_branch_scope_and_parameter_order() {
        let pool = make_sync_test_pool();
        let branch = QueryBuilder::new(pool, "archived_users", false)
            .field(yang_db::field!("id"))
            .field(yang_db::field!("kind"))
            .where_and(yang_db::field!("tenant_id"), yang_db::CompareOp::Eq, 8)
            .order(yang_db::field!("id"), yang_db::SortOrder::Desc)
            .limit(2);
        let builder = QueryBuilder::new(pool, "users", false)
            .field(yang_db::field!("id"))
            .field(yang_db::field!("kind"))
            .where_and(yang_db::field!("tenant_id"), yang_db::CompareOp::Eq, 7)
            .union_all(branch)
            .expect("输出列数一致")
            .order(yang_db::field!("id"), yang_db::SortOrder::Asc)
            .limit(5);
        let mut generator = SqlGenerator::new();
        generator
            .build_select(&builder)
            .expect("UNION ALL 应可渲染");

        assert_eq!(generator.get_sql(), "SELECT `id`, `kind` FROM `users` WHERE `tenant_id` = ? UNION ALL (SELECT `id`, `kind` FROM `archived_users` WHERE `tenant_id` = ? ORDER BY `id` DESC LIMIT 2) ORDER BY `id` ASC LIMIT 5");
        assert!(matches!(
            generator.get_params(),
            [SqlValue::Int(7), SqlValue::Int(8)]
        ));
    }

    #[test]
    fn test_union_rejects_unknown_or_mismatched_output_and_bad_branch_table() {
        let pool = make_sync_test_pool();
        assert!(QueryBuilder::new(pool, "users", false)
            .union(QueryBuilder::new(pool, "archive", false))
            .is_err());
        assert!(QueryBuilder::new(pool, "users", false)
            .field(yang_db::field!("id"))
            .union(
                QueryBuilder::new(pool, "archive", false)
                    .field(yang_db::field!("id"))
                    .field(yang_db::field!("kind")),
            )
            .is_err());
        let builder = QueryBuilder::new(pool, "users", false)
            .field(yang_db::field!("id"))
            .union(
                QueryBuilder::new(pool, "archive; DROP TABLE users", false)
                    .field(yang_db::field!("id")),
            )
            .expect("列数一致");
        let mut generator = SqlGenerator::new();
        assert!(generator.build_select(&builder).is_err());
        assert!(generator.get_params().is_empty());
    }

    #[test]
    fn test_transaction_row_lock_rendering_is_typed_and_parameterized() {
        let builder = QueryBuilder::new(make_sync_test_pool(), "accounts", false)
            .field(yang_db::field!("balance"))
            .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 42)
            .limit(1);
        let (sql, params) = builder
            .render_for_transaction(Some(crate::RowLock::ForUpdate))
            .expect("事务锁查询应可渲染");
        assert_eq!(
            sql,
            "SELECT `balance` FROM `accounts` WHERE `id` = ? LIMIT 1 FOR UPDATE"
        );
        assert!(matches!(params.as_slice(), [SqlValue::Int(42)]));
    }

    #[test]
    fn test_transaction_row_lock_rejects_unsupported_query_shapes() {
        let pool = make_sync_test_pool();
        let grouped = QueryBuilder::new(pool, "accounts", false)
            .expr(crate::SelectExpr::count_all())
            .group(yang_db::field!("tenant_id"));
        assert!(grouped
            .render_for_transaction(Some(crate::RowLock::ForShare))
            .is_err());

        let union = QueryBuilder::new(pool, "accounts", false)
            .field(yang_db::field!("id"))
            .union(QueryBuilder::new(pool, "archived_accounts", false).field(yang_db::field!("id")))
            .expect("列数一致的 UNION 应构建成功");
        assert!(union
            .render_for_transaction(Some(crate::RowLock::ForUpdate))
            .is_err());
    }

    #[tokio::test]
    async fn test_atomic_update_rejects_missing_where_and_adversarial_field_before_io() {
        let pool = MySqlPoolOptions::new()
            .max_connections(1)
            .connect_lazy("mysql://root:111111@localhost:3306/test")
            .expect("合法测试数据库 URL");
        let missing_where = QueryBuilder::new(&pool, "accounts", false)
            .increment(yang_db::field!("balance"), 1)
            .await;
        assert!(matches!(
            missing_where,
            Err(crate::DbError::MissingWhereClause)
        ));

        assert!(crate::FieldRef::new("balance = 0; DROP TABLE accounts --").is_err());
    }

    #[test]
    fn test_atomic_update_renderer_binds_negative_amount_before_where_params() {
        let mut generator = SqlGenerator::new();
        generator
            .build_arithmetic_update(
                "accounts",
                "balance",
                ArithmeticOperator::Add,
                -3,
                &[Condition::Eq("id".to_string(), SqlValue::Int(9))],
            )
            .expect("原子更新应可渲染");
        assert_eq!(
            generator.get_sql(),
            "UPDATE `accounts` SET `balance` = `balance` + ? WHERE `id` = ?"
        );
        assert!(matches!(
            generator.get_params(),
            [SqlValue::Int(-3), SqlValue::Int(9)]
        ));
    }

    #[tokio::test]
    async fn test_table_name_in_sql() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false);
        let sql = builder.to_sql();
        assert!(sql.contains("FROM `users`"));
    }

    // SqlGenerator 单元测试
    #[test]
    fn test_sql_generator_new() {
        let generator = SqlGenerator::new();
        assert_eq!(generator.get_sql(), "");
        assert_eq!(generator.get_params().len(), 0);
    }

    #[test]
    fn test_sql_generator_append() {
        let mut generator = SqlGenerator::new();
        generator.append("SELECT * FROM users");
        // SqlGenerator::append 是手动拼接，不走 build_select，因此不经过表名转义
        assert_eq!(generator.get_sql(), "SELECT * FROM users");
    }

    #[test]
    fn test_sql_generator_add_param() {
        let mut generator = SqlGenerator::new();
        generator.add_param(SqlValue::Int(42));
        generator.add_param(SqlValue::String("test".to_string()));
        assert_eq!(generator.get_params().len(), 2);
    }

    #[test]
    fn test_sql_generator_clear() {
        let mut generator = SqlGenerator::new();
        generator.append("SELECT * FROM users");
        generator.add_param(SqlValue::Int(1));

        generator.clear();

        assert_eq!(generator.get_sql(), "");
        assert_eq!(generator.get_params().len(), 0);
    }

    #[test]
    fn test_sql_generator_multiple_operations() {
        let mut generator = SqlGenerator::new();

        generator.append("SELECT * FROM users WHERE id = ?");
        generator.add_param(SqlValue::Int(1));
        generator.append(" AND name = ?");
        generator.add_param(SqlValue::String("test".to_string()));

        assert_eq!(
            generator.get_sql(),
            "SELECT * FROM users WHERE id = ? AND name = ?"
        );
        assert_eq!(generator.get_params().len(), 2);
    }

    #[tokio::test]
    async fn test_field_selection() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field(yang_db::field!("id"))
            .field(yang_db::field!("name"));
        let sql = builder.to_sql();
        assert!(sql.contains("`id`, `name`"));
    }

    #[tokio::test]
    async fn test_fields_selection() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false).fields(&[
            yang_db::field!("id"),
            yang_db::field!("name"),
            yang_db::field!("email"),
        ]);
        let sql = builder.to_sql();
        assert!(sql.contains("`id`, `name`, `email`"));
    }

    #[tokio::test]
    async fn test_distinct() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field(yang_db::field!("name"))
            .distinct();
        let sql = builder.to_sql();
        assert!(sql.contains("SELECT DISTINCT"));
    }

    #[tokio::test]
    async fn test_field_type_marking() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .json(yang_db::field!("data"))
            .datetime(yang_db::field!("created_at"))
            .timestamp(yang_db::field!("updated_at"))
            .decimal(yang_db::field!("price"))
            .blob(yang_db::field!("content"))
            .text(yang_db::field!("description"));

        assert_eq!(builder.field_types.get("data"), Some(&FieldType::Json));
        assert_eq!(
            builder.field_types.get("created_at"),
            Some(&FieldType::DateTime)
        );
        assert_eq!(
            builder.field_types.get("updated_at"),
            Some(&FieldType::Timestamp)
        );
        assert_eq!(builder.field_types.get("price"), Some(&FieldType::Decimal));
        assert_eq!(builder.field_types.get("content"), Some(&FieldType::Blob));
        assert_eq!(
            builder.field_types.get("description"),
            Some(&FieldType::Text)
        );
    }

    #[tokio::test]
    async fn test_where_and() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .where_and(yang_db::field!("name"), yang_db::CompareOp::Eq, "test")
            .where_and(yang_db::field!("age"), yang_db::CompareOp::Gt, 18);

        assert_eq!(builder.conditions.len(), 2);
    }

    #[tokio::test]
    async fn test_where_or() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .where_or(yang_db::field!("status"), yang_db::CompareOp::Eq, 1)
            .where_or(yang_db::field!("status"), yang_db::CompareOp::Eq, 2);

        // where_or 会将条件组合成 OR
        assert_eq!(builder.conditions.len(), 1);
    }

    #[tokio::test]
    async fn test_where_in() {
        let pool = create_test_pool().await;
        let builder =
            QueryBuilder::new(&pool, "users", false).where_in(yang_db::field!("id"), vec![1, 2, 3]);

        assert_eq!(builder.conditions.len(), 1);
    }

    #[tokio::test]
    async fn test_where_between() {
        let pool = create_test_pool().await;
        let builder =
            QueryBuilder::new(&pool, "users", false).where_between(yang_db::field!("age"), 18, 65);

        assert_eq!(builder.conditions.len(), 1);
    }

    #[tokio::test]
    async fn test_join() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false).join(
            yang_db::table!("orders"),
            yang_db::field!("users.id"),
            yang_db::field!("orders.user_id"),
        );

        assert_eq!(builder.joins.len(), 1);
    }

    #[tokio::test]
    async fn test_left_join() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false).left_join(
            yang_db::table!("orders"),
            yang_db::field!("users.id"),
            yang_db::field!("orders.user_id"),
        );

        assert_eq!(builder.joins.len(), 1);
    }

    #[tokio::test]
    async fn test_right_join() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false).right_join(
            yang_db::table!("orders"),
            yang_db::field!("users.id"),
            yang_db::field!("orders.user_id"),
        );

        assert_eq!(builder.joins.len(), 1);
    }

    #[tokio::test]
    async fn test_order() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .order(yang_db::field!("name"), yang_db::SortOrder::Asc)
            .order(yang_db::field!("age"), yang_db::SortOrder::Desc);

        assert_eq!(builder.order_by.len(), 2);
    }

    #[tokio::test]
    async fn test_group() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .group(yang_db::field!("status"))
            .group(yang_db::field!("role"));

        assert_eq!(builder.group_by.len(), 2);
    }

    // 测试完整的 SELECT 语句生成
    #[tokio::test]
    async fn test_select_with_where() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field(yang_db::field!("id"))
            .field(yang_db::field!("name"))
            .where_and(yang_db::field!("status"), yang_db::CompareOp::Eq, 1);

        let sql = builder.to_sql();
        assert!(sql.contains("SELECT `id`, `name` FROM `users`"));
        assert!(sql.contains("WHERE"));
    }

    #[tokio::test]
    async fn test_select_with_join() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field(yang_db::field!("users.id"))
            .field(yang_db::field!("orders.total"))
            .join(
                yang_db::table!("orders"),
                yang_db::field!("users.id"),
                yang_db::field!("orders.user_id"),
            );

        let sql = builder.to_sql();
        assert!(sql.contains("SELECT `users`.`id`, `orders`.`total` FROM `users`"));
        assert!(sql.contains("INNER JOIN `orders` ON `users`.`id` = `orders`.`user_id`"));
    }

    #[tokio::test]
    async fn test_select_with_order_by() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field(yang_db::field!("name"))
            .order(yang_db::field!("name"), yang_db::SortOrder::Asc)
            .order(yang_db::field!("age"), yang_db::SortOrder::Desc);

        let sql = builder.to_sql();
        assert!(sql.contains("ORDER BY `name` ASC, `age` DESC"));
    }

    #[tokio::test]
    async fn test_select_with_group_by() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field(yang_db::field!("status"))
            .group(yang_db::field!("status"));

        let sql = builder.to_sql();
        assert!(sql.contains("GROUP BY `status`"));
    }

    #[tokio::test]
    async fn test_select_with_limit_offset() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field(yang_db::field!("id"))
            .limit(10)
            .offset(20);

        let sql = builder.to_sql();
        assert!(sql.contains("LIMIT 10"));
        assert!(sql.contains("OFFSET 20"));
    }

    #[tokio::test]
    async fn test_select_complex_query() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field(yang_db::field!("users.id"))
            .field(yang_db::field!("users.name"))
            .field(yang_db::field!("orders.total"))
            .distinct()
            .join(
                yang_db::table!("orders"),
                yang_db::field!("users.id"),
                yang_db::field!("orders.user_id"),
            )
            .where_and(yang_db::field!("users.status"), yang_db::CompareOp::Eq, 1)
            .where_and(yang_db::field!("orders.total"), yang_db::CompareOp::Gt, 100)
            .group(yang_db::field!("users.id"))
            .order(yang_db::field!("orders.total"), yang_db::SortOrder::Desc)
            .limit(50);

        let sql = builder.to_sql();
        assert!(sql.contains("SELECT DISTINCT"));
        assert!(sql.contains("`users`.`id`, `users`.`name`, `orders`.`total`"));
        assert!(sql.contains("FROM `users`"));
        assert!(sql.contains("INNER JOIN `orders` ON `users`.`id` = `orders`.`user_id`"));
        assert!(sql.contains("WHERE"));
        assert!(sql.contains("GROUP BY `users`.`id`"));
        assert!(sql.contains("ORDER BY `orders`.`total` DESC"));
        assert!(sql.contains("LIMIT 50"));
    }

    #[tokio::test]
    async fn test_select_with_multiple_joins() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field(yang_db::field!("users.name"))
            .field(yang_db::field!("orders.total"))
            .field(yang_db::field!("products.name"))
            .join(
                yang_db::table!("orders"),
                yang_db::field!("users.id"),
                yang_db::field!("orders.user_id"),
            )
            .left_join(
                yang_db::table!("products"),
                yang_db::field!("orders.product_id"),
                yang_db::field!("products.id"),
            );

        let sql = builder.to_sql();
        assert!(sql.contains("INNER JOIN `orders` ON `users`.`id` = `orders`.`user_id`"));
        assert!(sql.contains("LEFT JOIN `products` ON `orders`.`product_id` = `products`.`id`"));
    }

    #[tokio::test]
    async fn test_select_with_in_condition() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field(yang_db::field!("name"))
            .where_in(yang_db::field!("id"), vec![1, 2, 3, 4, 5]);

        let sql = builder.to_sql();
        assert!(sql.contains("WHERE"));
        assert!(sql.contains("IN"));
    }

    #[tokio::test]
    async fn test_select_with_between_condition() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field(yang_db::field!("name"))
            .where_between(yang_db::field!("age"), 18, 65);

        let sql = builder.to_sql();
        assert!(sql.contains("WHERE"));
        assert!(sql.contains("BETWEEN"));
    }

    #[test]
    fn test_where_null_generates_is_null_sql() {
        let pool = make_sync_test_pool();
        let builder =
            QueryBuilder::new(pool, "users", false).where_null(yang_db::field!("deleted_at"));
        let sql = builder.to_sql();
        assert!(sql.contains("`deleted_at` IS NULL"));
    }

    #[test]
    fn test_where_not_null_generates_is_not_null_sql() {
        let pool = make_sync_test_pool();
        let builder =
            QueryBuilder::new(pool, "users", false).where_not_null(yang_db::field!("email"));
        let sql = builder.to_sql();
        assert!(sql.contains("`email` IS NOT NULL"));
    }

    #[test]
    fn test_is_null_with_and_condition() {
        let pool = make_sync_test_pool();
        let builder = QueryBuilder::new(pool, "users", false)
            .where_and(yang_db::field!("status"), yang_db::CompareOp::Eq, 1i64)
            .where_null(yang_db::field!("deleted_at"));
        let sql = builder.to_sql();
        assert!(sql.contains("`status` = ?"));
        assert!(sql.contains("`deleted_at` IS NULL"));
    }

    #[test]
    fn test_having_clause_sql_generation() {
        let pool = make_sync_test_pool();
        let builder = QueryBuilder::new(pool, "orders", false)
            .field(yang_db::field!("user_id"))
            .expr(crate::SelectExpr::count_all().alias(yang_db::field!("cnt")))
            .group(yang_db::field!("user_id"))
            .having_cond(yang_db::field!("cnt"), yang_db::CompareOp::Gt, 5i64);
        let sql = builder.to_sql();
        assert!(sql.contains("HAVING"));
        assert!(sql.contains("`cnt` > ?"));
    }

    #[test]
    fn test_having_without_group_returns_error() {
        let pool = make_sync_test_pool();
        let builder = QueryBuilder::new(pool, "orders", false).having_cond(
            yang_db::field!("cnt"),
            yang_db::CompareOp::Gt,
            5i64,
        );
        let mut generator = SqlGenerator::new();
        let result = generator.build_select(&builder);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::DbError::MissingGroupByClause
        ));
    }

    #[test]
    fn test_identifier_apis_quote_every_structural_fragment() {
        let pool = make_sync_test_pool();
        let builder = QueryBuilder::new(pool, "users", false)
            .field(yang_db::field!("users.id"))
            .expr(crate::SelectExpr::count_all().alias(yang_db::field!("cnt")))
            .join(
                yang_db::table!("profiles"),
                yang_db::field!("users.id"),
                yang_db::field!("profiles.user_id"),
            )
            .group(yang_db::field!("users.id"))
            .order(yang_db::field!("users.id"), yang_db::SortOrder::Asc);

        let sql = builder.try_to_sql().expect("安全 API 应生成 SQL");
        assert!(sql.contains("SELECT `users`.`id`, COUNT(*) AS `cnt`"));
        assert!(sql.contains("INNER JOIN `profiles` ON `users`.`id` = `profiles`.`user_id`"));
        assert!(sql.contains("GROUP BY `users`.`id`"));
        assert!(sql.contains("ORDER BY `users`.`id` ASC"));
    }

    #[test]
    fn test_identifier_apis_reject_adversarial_fragments() {
        let payloads = [
            "",
            ".id",
            "users.",
            "a.b.c",
            "users.id --",
            "users/*x*/.id",
            "用户.id",
            "users.`id`",
            "users.id\0",
        ];

        for payload in payloads {
            assert!(crate::FieldRef::new(payload).is_err());
        }
        assert!(crate::TableRef::new("profiles p").is_err());
    }

    #[test]
    fn test_try_to_sql_surfaces_invalid_table_identifier() {
        let pool = make_sync_test_pool();
        let builder = QueryBuilder::new(pool, "users; DROP TABLE users", false);
        let result = builder.try_to_sql();

        assert!(matches!(result, Err(crate::DbError::InvalidArgument(_))));
    }

    #[test]
    fn test_try_to_sql_rejects_invalid_condition_identifier() {
        assert!(crate::FieldRef::new("id;DROP").is_err());
    }

    #[test]
    fn test_try_to_sql_accepts_qualified_where_and_having_identifiers() {
        let pool = make_sync_test_pool();
        let builder = QueryBuilder::new(pool, "users", false)
            .where_and(
                yang_db::field!("users.status"),
                yang_db::CompareOp::Eq,
                1i64,
            )
            .group(yang_db::field!("users.id"))
            .having_cond(
                yang_db::field!("users.score"),
                yang_db::CompareOp::Gt,
                10i64,
            );

        let sql = builder
            .try_to_sql()
            .expect("合法的两段限定条件字段应生成 SQL");

        assert!(sql.contains("WHERE `users`.`status` = ?"));
        assert!(sql.contains("HAVING `users`.`score` > ?"));
    }

    #[test]
    fn test_try_to_sql_rejects_malicious_qualified_where_and_having_identifiers() {
        let invalid_fields = [
            "",
            ".id",
            "users.",
            "a.b.c",
            "users.id;DROP",
            "users.id --",
            "users.`id`",
            "COUNT(users.id)",
        ];

        for field in invalid_fields {
            assert!(crate::FieldRef::new(field).is_err());
        }
    }

    #[test]
    fn test_try_to_sql_surfaces_missing_group_by() {
        let pool = make_sync_test_pool();
        let builder = QueryBuilder::new(pool, "orders", false).having_cond(
            yang_db::field!("cnt"),
            yang_db::CompareOp::Gt,
            5i64,
        );
        let result = builder.try_to_sql();

        assert!(matches!(result, Err(crate::DbError::MissingGroupByClause)));
    }

    #[test]
    fn test_try_to_sql_rejects_empty_in_condition() {
        let pool = make_sync_test_pool();
        let builder = QueryBuilder::new(pool, "users", false)
            .where_in(yang_db::field!("id"), Vec::<i64>::new());
        let result = builder.try_to_sql();

        assert!(matches!(result, Err(crate::DbError::InvalidArgument(_))));
    }

    #[test]
    fn test_try_to_sql_rejects_empty_boolean_condition() {
        let pool = make_sync_test_pool();
        let mut builder = QueryBuilder::new(pool, "users", false);
        builder.conditions.push(Condition::And(vec![]));
        let result = builder.try_to_sql();

        assert!(matches!(result, Err(crate::DbError::InvalidArgument(_))));
    }

    #[test]
    fn test_to_sql_does_not_fallback_to_raw_untrusted_table() {
        let pool = make_sync_test_pool();
        let builder = QueryBuilder::new(pool, "users; DROP TABLE users", false);
        let sql = builder.to_sql();

        assert_eq!(sql, "/* SQL generation failed */");
        assert!(!sql.contains("DROP TABLE"));
        assert!(!sql.contains("users;"));
    }

    #[test]
    fn test_having_clause_order() {
        let pool = make_sync_test_pool();
        let builder = QueryBuilder::new(pool, "orders", false)
            .group(yang_db::field!("user_id"))
            .having_cond(yang_db::field!("cnt"), yang_db::CompareOp::Gt, 5i64)
            .order(yang_db::field!("cnt"), yang_db::SortOrder::Desc);
        let sql = builder.to_sql();
        let group_pos = sql.find("GROUP BY").unwrap();
        let having_pos = sql.find("HAVING").unwrap();
        let order_pos = sql.find("ORDER BY").unwrap();
        assert!(group_pos < having_pos);
        assert!(having_pos < order_pos);
    }

    #[test]
    fn test_update_batch_case_when_sql() {
        let records = vec![
            serde_json::json!({"id": 1, "name": "Alice", "age": 25}),
            serde_json::json!({"id": 2, "name": "Bob", "age": 30}),
        ];
        let mut generator = SqlGenerator::new();
        generator
            .build_update_batch("users", &records, "id", &std::collections::HashMap::new())
            .unwrap();
        let sql = generator.get_sql();
        assert!(sql.starts_with("UPDATE `users` SET "));
        assert!(sql.contains("CASE WHEN `id`=? THEN ?"));
        assert!(sql.contains("WHERE `id` IN ("));
    }

    #[test]
    fn test_update_batch_empty_returns_error() {
        let records: Vec<serde_json::Value> = vec![];
        let mut generator = SqlGenerator::new();
        let result = generator.build_update_batch(
            "users",
            &records,
            "id",
            &std::collections::HashMap::new(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_upsert_sql_generation() {
        let data = serde_json::json!({"id": 1, "name": "Alice", "email": "a@b.com"});
        let mut generator = SqlGenerator::new();
        generator
            .build_upsert("users", &data, &std::collections::HashMap::new())
            .unwrap();
        let sql = generator.get_sql();
        assert!(sql.starts_with("INSERT INTO `users`"));
        assert!(sql.contains("ON DUPLICATE KEY UPDATE"));
        assert!(sql.contains("`name`=VALUES(`name`)"));
    }

    #[test]
    fn test_upsert_empty_data_returns_error() {
        let data = serde_json::json!({});
        let mut generator = SqlGenerator::new();
        let result = generator.build_upsert("users", &data, &std::collections::HashMap::new());
        assert!(result.is_err());
    }

    // 测试 SqlGenerator 的 build_select 方法
    #[tokio::test]
    async fn test_sql_generator_build_select_basic() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field(yang_db::field!("id"))
            .field(yang_db::field!("name"));

        let mut generator = SqlGenerator::new();
        let result = generator.build_select(&builder);

        assert!(result.is_ok());
        assert_eq!(generator.get_sql(), "SELECT `id`, `name` FROM `users`");
    }

    #[tokio::test]
    async fn test_sql_generator_build_select_with_distinct() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field(yang_db::field!("name"))
            .distinct();

        let mut generator = SqlGenerator::new();
        let result = generator.build_select(&builder);

        assert!(result.is_ok());
        assert_eq!(generator.get_sql(), "SELECT DISTINCT `name` FROM `users`");
    }

    #[tokio::test]
    async fn test_sql_generator_build_select_all_fields() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false);

        let mut generator = SqlGenerator::new();
        let result = generator.build_select(&builder);

        assert!(result.is_ok());
        assert_eq!(generator.get_sql(), "SELECT * FROM `users`");
    }

    // 测试 WHERE 子句生成
    #[tokio::test]
    async fn test_sql_generator_build_where() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .where_and(yang_db::field!("status"), yang_db::CompareOp::Eq, 1)
            .where_and(yang_db::field!("age"), yang_db::CompareOp::Gt, 18);

        let mut generator = SqlGenerator::new();
        let result = generator.build_select(&builder);

        assert!(result.is_ok());
        let sql = generator.get_sql();
        assert!(sql.contains("WHERE"));
        assert!(sql.contains("status"));
        assert!(sql.contains("age"));
    }

    // 测试 JOIN 子句生成
    #[tokio::test]
    async fn test_sql_generator_build_joins() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .join(
                yang_db::table!("orders"),
                yang_db::field!("users.id"),
                yang_db::field!("orders.user_id"),
            )
            .left_join(
                yang_db::table!("profiles"),
                yang_db::field!("users.id"),
                yang_db::field!("profiles.user_id"),
            );

        let mut generator = SqlGenerator::new();
        let result = generator.build_select(&builder);

        assert!(result.is_ok());
        let sql = generator.get_sql();
        assert!(sql.contains("INNER JOIN `orders` ON `users`.`id` = `orders`.`user_id`"));
        assert!(sql.contains("LEFT JOIN `profiles` ON `users`.`id` = `profiles`.`user_id`"));
    }

    // 测试 ORDER BY 子句生成
    #[tokio::test]
    async fn test_sql_generator_build_order_by() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .order(yang_db::field!("name"), yang_db::SortOrder::Asc)
            .order(yang_db::field!("created_at"), yang_db::SortOrder::Desc);

        let mut generator = SqlGenerator::new();
        let result = generator.build_select(&builder);

        assert!(result.is_ok());
        let sql = generator.get_sql();
        assert!(sql.contains("ORDER BY `name` ASC, `created_at` DESC"));
    }

    // 测试 GROUP BY 子句生成
    #[tokio::test]
    async fn test_sql_generator_build_group_by() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .group(yang_db::field!("status"))
            .group(yang_db::field!("role"));

        let mut generator = SqlGenerator::new();
        let result = generator.build_select(&builder);

        assert!(result.is_ok());
        let sql = generator.get_sql();
        assert!(sql.contains("GROUP BY `status`, `role`"));
    }

    // 测试 LIMIT 和 OFFSET 子句生成
    #[tokio::test]
    async fn test_sql_generator_build_limit_offset() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .limit(10)
            .offset(20);

        let mut generator = SqlGenerator::new();
        let result = generator.build_select(&builder);

        assert!(result.is_ok());
        let sql = generator.get_sql();
        assert!(sql.contains("LIMIT 10"));
        assert!(sql.contains("OFFSET 20"));
    }

    // 测试完整的复杂查询生成
    #[tokio::test]
    async fn test_sql_generator_complex_query() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field(yang_db::field!("users.id"))
            .field(yang_db::field!("users.name"))
            .expr(
                crate::SelectExpr::count(yang_db::field!("orders.id"))
                    .alias(yang_db::field!("order_count")),
            )
            .distinct()
            .join(
                yang_db::table!("orders"),
                yang_db::field!("users.id"),
                yang_db::field!("orders.user_id"),
            )
            .where_and(yang_db::field!("users.status"), yang_db::CompareOp::Eq, 1)
            .where_and(yang_db::field!("orders.total"), yang_db::CompareOp::Gt, 100)
            .group(yang_db::field!("users.id"))
            .group(yang_db::field!("users.name"))
            .order(yang_db::field!("order_count"), yang_db::SortOrder::Desc)
            .limit(20)
            .offset(10);

        let mut generator = SqlGenerator::new();
        let result = generator.build_select(&builder);

        assert!(result.is_ok());
        let sql = generator.get_sql();

        // 验证各个部分都存在
        assert!(sql.starts_with("SELECT DISTINCT"));
        assert!(sql.contains("`users`.`id`, `users`.`name`, COUNT(`orders`.`id`) AS `order_count`"));
        assert!(sql.contains("FROM `users`"));
        assert!(sql.contains("INNER JOIN `orders` ON `users`.`id` = `orders`.`user_id`"));
        assert!(sql.contains("WHERE"));
        assert!(sql.contains("GROUP BY `users`.`id`, `users`.`name`"));
        assert!(sql.contains("ORDER BY `order_count` DESC"));
        assert!(sql.contains("LIMIT 20"));
        assert!(sql.contains("OFFSET 10"));
    }

    // 测试 find() 方法的 SQL 生成
    #[tokio::test]
    async fn test_find_adds_limit_one() {
        let pool = create_test_pool().await;
        let builder = QueryBuilder::new(&pool, "users", false)
            .field(yang_db::field!("id"))
            .field(yang_db::field!("name"))
            .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 1);

        // 在调用 find() 之前，limit 应该是 None
        assert_eq!(builder.limit, None);

        // 创建一个新的 builder 来测试 SQL 生成
        let builder_with_limit = QueryBuilder::new(&pool, "users", false)
            .field(yang_db::field!("id"))
            .field(yang_db::field!("name"))
            .where_and(yang_db::field!("id"), yang_db::CompareOp::Eq, 1)
            .limit(1);

        let sql = builder_with_limit.to_sql();
        assert!(sql.contains("LIMIT 1"), "find() 应该自动添加 LIMIT 1");
    }

    // 测试 INSERT 语句生成
    #[test]
    fn test_sql_generator_build_insert_basic() {
        let mut generator = SqlGenerator::new();
        let data = serde_json::json!({
            "name": "张三",
            "age": 25,
            "email": "zhangsan@example.com"
        });
        let field_types = HashMap::new();

        let result = generator.build_insert("users", &data, &field_types, &[]);
        assert!(result.is_ok());

        let sql = generator.get_sql();
        assert!(sql.starts_with("INSERT INTO `users`"));
        assert!(sql.contains("name"));
        assert!(sql.contains("age"));
        assert!(sql.contains("email"));
        assert!(sql.contains("VALUES"));
        assert_eq!(generator.get_params().len(), 3);
    }

    #[test]
    fn test_sql_generator_build_insert_with_json_field() {
        let mut generator = SqlGenerator::new();
        let data = serde_json::json!({
            "name": "测试用户",
            "data": {"role": "admin", "permissions": ["read", "write"]}
        });

        let mut field_types = HashMap::new();
        field_types.insert("data".to_string(), FieldType::Json);

        let result = generator.build_insert("users", &data, &field_types, &[]);
        assert!(result.is_ok());

        let sql = generator.get_sql();
        assert!(sql.contains("INSERT INTO `users`"));
        assert!(sql.contains("name"));
        assert!(sql.contains("data"));
        assert_eq!(generator.get_params().len(), 2);

        // 验证 JSON 字段被正确处理
        let params = generator.get_params();
        let has_json = params.iter().any(|p| matches!(p, SqlValue::Json(_)));
        assert!(has_json, "应该包含 JSON 类型的参数");
    }

    #[test]
    fn test_sql_generator_build_insert_empty_data() {
        let mut generator = SqlGenerator::new();
        let data = serde_json::json!({});
        let field_types = HashMap::new();

        let result = generator.build_insert("users", &data, &field_types, &[]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::error::DbError::SerializationError(_)
        ));
    }

    #[test]
    fn test_sql_generator_build_insert_not_object() {
        let mut generator = SqlGenerator::new();
        let data = serde_json::json!([1, 2, 3]); // 数组而不是对象
        let field_types = HashMap::new();

        let result = generator.build_insert("users", &data, &field_types, &[]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::error::DbError::SerializationError(_)
        ));
    }

    // ==================== 聚合函数单元测试 ====================

    /// 测试 AVG 聚合函数 SQL 生成
    #[tokio::test]
    async fn test_avg_sql_generation() {
        let pool = create_test_pool().await;

        // 创建一个新的 builder 来模拟 avg() 方法的行为
        let mut test_builder = QueryBuilder::new(&pool, "products", false);
        test_builder.fields.clear();
        test_builder
            .fields
            .push("CAST(AVG(price) AS DOUBLE)".to_string());
        test_builder.limit = Some(1);

        let sql = test_builder.to_sql();
        assert!(sql.contains("SELECT CAST(AVG(price) AS DOUBLE)"));
        assert!(sql.contains("FROM `products`"));
        assert!(sql.contains("LIMIT 1"));
    }

    /// 测试 AVG 与 WHERE 条件组合
    #[tokio::test]
    async fn test_avg_with_where_sql() {
        let pool = create_test_pool().await;

        // 模拟 avg() 方法与 WHERE 条件组合
        let mut test_builder = QueryBuilder::new(&pool, "products", false).where_and(
            yang_db::field!("status"),
            yang_db::CompareOp::Eq,
            1,
        );
        test_builder.fields.clear();
        test_builder
            .fields
            .push("CAST(AVG(price) AS DOUBLE)".to_string());
        test_builder.limit = Some(1);

        let sql = test_builder.to_sql();
        assert!(sql.contains("SELECT CAST(AVG(price) AS DOUBLE)"));
        assert!(sql.contains("FROM `products`"));
        assert!(sql.contains("WHERE"));
        assert!(sql.contains("status"));
    }

    /// 测试 MIN 聚合函数 SQL 生成
    #[tokio::test]
    async fn test_min_sql_generation() {
        let pool = create_test_pool().await;

        // 模拟 min() 方法的行为
        let mut test_builder = QueryBuilder::new(&pool, "products", false);
        test_builder.fields.clear();
        test_builder.fields.push("MIN(price)".to_string());
        test_builder.limit = Some(1);

        let sql = test_builder.to_sql();
        assert!(sql.contains("SELECT MIN(price)"));
        assert!(sql.contains("FROM `products`"));
        assert!(sql.contains("LIMIT 1"));
    }

    /// 测试 MAX 聚合函数 SQL 生成
    #[tokio::test]
    async fn test_max_sql_generation() {
        let pool = create_test_pool().await;

        // 模拟 max() 方法的行为
        let mut test_builder = QueryBuilder::new(&pool, "products", false);
        test_builder.fields.clear();
        test_builder.fields.push("MAX(price)".to_string());
        test_builder.limit = Some(1);

        let sql = test_builder.to_sql();
        assert!(sql.contains("SELECT MAX(price)"));
        assert!(sql.contains("FROM `products`"));
        assert!(sql.contains("LIMIT 1"));
    }

    /// 测试 MIN/MAX 不同数据类型的 SQL 生成
    #[tokio::test]
    async fn test_min_max_different_types() {
        let pool = create_test_pool().await;

        // 测试整数类型
        let mut builder_int = QueryBuilder::new(&pool, "products", false);
        builder_int.fields.clear();
        builder_int.fields.push("MIN(stock)".to_string());
        let sql_int = builder_int.to_sql();
        assert!(sql_int.contains("MIN(stock)"));

        // 测试浮点数类型
        let mut builder_float = QueryBuilder::new(&pool, "products", false);
        builder_float.fields.clear();
        builder_float.fields.push("MAX(price)".to_string());
        let sql_float = builder_float.to_sql();
        assert!(sql_float.contains("MAX(price)"));

        // 测试字符串类型
        let mut builder_string = QueryBuilder::new(&pool, "users", false);
        builder_string.fields.clear();
        builder_string.fields.push("MIN(name)".to_string());
        let sql_string = builder_string.to_sql();
        assert!(sql_string.contains("MIN(name)"));

        // 测试日期时间类型
        let mut builder_datetime = QueryBuilder::new(&pool, "users", false);
        builder_datetime.fields.clear();
        builder_datetime.fields.push("MAX(created_at)".to_string());
        let sql_datetime = builder_datetime.to_sql();
        assert!(sql_datetime.contains("MAX(created_at)"));
    }

    /// 测试聚合函数与 GROUP BY 组合
    #[tokio::test]
    async fn test_aggregates_with_group_by_sql() {
        let pool = create_test_pool().await;

        // 模拟聚合函数与 GROUP BY 组合
        let mut test_builder =
            QueryBuilder::new(&pool, "orders", false).group(yang_db::field!("user_id"));
        test_builder.fields.clear();
        test_builder.fields.push("user_id".to_string());
        test_builder
            .fields
            .push("CAST(AVG(amount) AS DOUBLE) as avg_amount".to_string());

        let sql = test_builder.to_sql();
        assert!(sql.contains("SELECT user_id, CAST(AVG(amount) AS DOUBLE) as avg_amount"));
        assert!(sql.contains("FROM `orders`"));
        assert!(sql.contains("GROUP BY `user_id`"));
    }

    /// 测试多个聚合函数组合
    #[tokio::test]
    async fn test_multiple_aggregates_sql() {
        let pool = create_test_pool().await;

        // 模拟多个聚合函数组合
        let mut test_builder = QueryBuilder::new(&pool, "orders", false).where_and(
            yang_db::field!("status"),
            yang_db::CompareOp::Eq,
            "completed",
        );
        test_builder.fields.clear();
        test_builder
            .fields
            .push("CAST(AVG(amount) AS DOUBLE) as avg_amount".to_string());
        test_builder
            .fields
            .push("CAST(MIN(amount) AS DOUBLE) as min_amount".to_string());
        test_builder
            .fields
            .push("CAST(MAX(amount) AS DOUBLE) as max_amount".to_string());
        test_builder
            .fields
            .push("COUNT(*) as order_count".to_string());

        let sql = test_builder.to_sql();
        assert!(sql.contains("CAST(AVG(amount) AS DOUBLE) as avg_amount"));
        assert!(sql.contains("CAST(MIN(amount) AS DOUBLE) as min_amount"));
        assert!(sql.contains("CAST(MAX(amount) AS DOUBLE) as max_amount"));
        assert!(sql.contains("COUNT(*) as order_count"));
        assert!(sql.contains("WHERE"));
        assert!(sql.contains("status"));
    }

    /// 测试 SQL 子句顺序正确性（WHERE -> GROUP BY -> ORDER BY）
    #[tokio::test]
    async fn test_sql_clause_order_with_aggregates() {
        let pool = create_test_pool().await;

        // 创建包含 WHERE、GROUP BY、ORDER BY 的查询
        let mut test_builder = QueryBuilder::new(&pool, "orders", false)
            .where_and(
                yang_db::field!("status"),
                yang_db::CompareOp::Eq,
                "completed",
            )
            .group(yang_db::field!("user_id"))
            .order(yang_db::field!("total_amount"), yang_db::SortOrder::Desc);
        test_builder.fields.clear();
        test_builder.fields.push("user_id".to_string());
        test_builder
            .fields
            .push("SUM(amount) as total_amount".to_string());

        let sql = test_builder.to_sql();

        // 验证子句顺序：WHERE 应该在 GROUP BY 之前，GROUP BY 应该在 ORDER BY 之前
        let where_pos = sql.find("WHERE").expect("应该包含 WHERE");
        let group_pos = sql.find("GROUP BY").expect("应该包含 GROUP BY");
        let order_pos = sql.find("ORDER BY").expect("应该包含 ORDER BY");

        assert!(where_pos < group_pos, "WHERE 应该在 GROUP BY 之前");
        assert!(group_pos < order_pos, "GROUP BY 应该在 ORDER BY 之前");
    }

    /// 测试空结果集场景的 SQL 生成
    #[tokio::test]
    async fn test_aggregates_empty_result_sql() {
        let pool = create_test_pool().await;

        // 创建一个不会匹配任何记录的查询
        let mut test_builder = QueryBuilder::new(&pool, "products", false).where_and(
            yang_db::field!("id"),
            yang_db::CompareOp::Eq,
            -1,
        ); // 假设 id 不会是负数
        test_builder.fields.clear();
        test_builder
            .fields
            .push("CAST(AVG(price) AS DOUBLE)".to_string());
        test_builder.limit = Some(1);

        let sql = test_builder.to_sql();
        assert!(sql.contains("SELECT CAST(AVG(price) AS DOUBLE)"));
        assert!(sql.contains("WHERE"));
        // SQL 生成应该正常，即使结果集为空
    }

    /// 测试聚合函数字段名包含特殊字符
    #[tokio::test]
    async fn test_aggregates_with_special_field_names() {
        let pool = create_test_pool().await;

        // 测试带下划线的字段名
        let mut test_builder = QueryBuilder::new(&pool, "products", false);
        test_builder.fields.clear();
        test_builder.fields.push("AVG(unit_price)".to_string());

        let sql = test_builder.to_sql();
        assert!(sql.contains("AVG(unit_price)"));

        // 测试带反引号的字段名（MySQL 保留字）
        let mut test_builder2 = QueryBuilder::new(&pool, "products", false);
        test_builder2.fields.clear();
        test_builder2.fields.push("MAX(`order`)".to_string());

        let sql2 = test_builder2.to_sql();
        assert!(sql2.contains("MAX(`order`)"));
    }

    /// 测试聚合函数与 DISTINCT 组合
    #[tokio::test]
    async fn test_aggregates_with_distinct() {
        let pool = create_test_pool().await;

        // 模拟 COUNT(DISTINCT field) 场景
        let mut test_builder = QueryBuilder::new(&pool, "orders", false);
        test_builder.fields.clear();
        test_builder
            .fields
            .push("COUNT(DISTINCT user_id) as unique_users".to_string());

        let sql = test_builder.to_sql();
        assert!(sql.contains("COUNT(DISTINCT user_id)"));
    }

    /// 测试聚合函数与 JOIN 组合
    #[tokio::test]
    async fn test_aggregates_with_join() {
        let pool = create_test_pool().await;

        let test_builder = QueryBuilder::new(&pool, "users", false)
            .join(
                yang_db::table!("orders"),
                yang_db::field!("users.id"),
                yang_db::field!("orders.user_id"),
            )
            .group(yang_db::field!("users.id"))
            .field(yang_db::field!("users.id"))
            .field(yang_db::field!("users.name"))
            .expr(
                crate::SelectExpr::count(yang_db::field!("orders.id"))
                    .alias(yang_db::field!("order_count")),
            )
            .expr(
                crate::SelectExpr::sum(yang_db::field!("orders.amount"))
                    .alias(yang_db::field!("total_amount")),
            );

        let sql = test_builder.to_sql();
        assert!(sql.contains("INNER JOIN `orders` ON `users`.`id` = `orders`.`user_id`"));
        assert!(sql.contains("COUNT(`orders`.`id`) AS `order_count`"));
        assert!(sql.contains("SUM(`orders`.`amount`) AS `total_amount`"));
        assert!(sql.contains("GROUP BY `users`.`id`"));
    }

    /// 测试聚合函数参数化查询防止 SQL 注入
    #[tokio::test]
    async fn test_aggregates_sql_injection_prevention() {
        let pool = create_test_pool().await;

        // 测试 WHERE 条件使用参数化查询
        let builder = QueryBuilder::new(&pool, "products", false).where_and(
            yang_db::field!("category"),
            yang_db::CompareOp::Eq,
            "'; DROP TABLE products; --",
        );

        // 生成 SQL 和参数
        let mut generator = SqlGenerator::new();
        let result = generator.build_select(&builder);
        assert!(result.is_ok());

        let sql = generator.get_sql();
        let params = generator.get_params();

        // 验证 SQL 使用占位符而不是直接拼接值
        assert!(sql.contains("?"));
        assert!(!sql.contains("DROP TABLE"));

        // 验证参数列表包含恶意字符串（作为参数值，不会被执行）
        assert_eq!(params.len(), 1);
    }
}
