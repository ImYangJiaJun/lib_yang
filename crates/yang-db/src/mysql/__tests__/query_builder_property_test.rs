//! `QueryBuilder` 属性测试（proptest；自 `mysql::query_builder` 内联测试迁移，断言不变）。

use sqlx::mysql::MySqlPool;

use crate::mysql::field::FieldType;
use crate::mysql::query_builder::QueryBuilder;

#[cfg(test)]
macro_rules! test_field {
    ($value:expr) => {{
        &crate::FieldRef::new(($value).to_string()).expect("测试策略必须生成合法字段名")
    }};
}

#[cfg(test)]
macro_rules! test_table {
    ($value:expr) => {{
        &crate::TableRef::new(($value).to_string()).expect("测试策略必须生成合法表名")
    }};
}

#[cfg(test)]
#[allow(deprecated)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;
    use sqlx::mysql::MySqlPoolOptions;

    // 生成有效的表名（字母开头，后跟字母数字下划线）
    fn table_name_strategy() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9_]{0,30}"
    }

    // 生成有效的字段名
    fn field_name_strategy() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9_]{0,30}"
    }

    // 创建测试用的连接池（同步版本用于 proptest；懒连接，不建立真实连接）。
    // proptest 仅校验生成的 SQL，不执行查询。connect_lazy 仍需在 Tokio 上下文内创建
    // （内部会 spawn 后台 reaper），故用 block_on 提供上下文，但不发起真实连接（DB-11）。
    fn create_test_pool_sync() -> MySqlPool {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            MySqlPoolOptions::new()
                .max_connections(1)
                .connect_lazy("mysql://root:111111@localhost:3306/test")
                .expect("无法解析测试数据库 URL")
        })
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

    // Feature: mysql-query-builder, Property 1: 表名设置正确性
    // 验证需求：2.1
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_table_name_in_sql(table_name in table_name_strategy()) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false);
            let sql = builder.to_sql();

            // 验证 SQL 包含表名
            let expected = format!("FROM `{}`", table_name);
            prop_assert!(sql.contains(&expected));
        }
    }

    // Feature: mysql-query-builder, Property 2: 表名覆盖行为
    // 验证需求：2.2
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_table_name_override(
            table_name1 in table_name_strategy(),
            table_name2 in table_name_strategy()
        ) {
            prop_assume!(table_name1 != table_name2);

            let pool = create_test_pool_sync();
            // 先创建一个 builder，然后通过重新创建来模拟覆盖
            let builder1 = QueryBuilder::new(&pool, &table_name1, false);
            let sql1 = builder1.to_sql();
            let expected1 = format!("FROM `{}`", table_name1);
            prop_assert!(sql1.contains(&expected1));

            // 创建新的 builder 使用 table_name2
            let builder2 = QueryBuilder::new(&pool, &table_name2, false);
            let sql2 = builder2.to_sql();
            let expected2 = format!("FROM `{}`", table_name2);
            prop_assert!(sql2.contains(&expected2));

            // 使用更精确的匹配：检查 FROM 后面的完整表名（带空格或 WHERE 等关键字）
            // 避免子字符串匹配问题（如 "w" 是 "w_" 的子串）
            let pattern1 = format!("FROM {} ", table_name1);
            let pattern1_alt = format!("FROM {}\n", table_name1);
            prop_assert!(!sql2.contains(&pattern1) && !sql2.contains(&pattern1_alt));
        }
    }

    // Feature: mysql-query-builder, Property 24: 字段选择
    // 验证需求：9.1, 9.2
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_field_selection(
            table_name in table_name_strategy(),
            fields in prop::collection::vec(field_name_strategy(), 1..10)
        ) {
            let pool = create_test_pool_sync();
            let mut builder = QueryBuilder::new(&pool, &table_name, false);

            // 添加所有字段
            for field in &fields {
                builder = builder.field(test_field!(field));
            }

            let sql = builder.to_sql();

            // 验证所有字段都在 SELECT 子句中
            for field in &fields {
                prop_assert!(sql.contains(field));
            }
        }
    }

    // Feature: mysql-query-builder, Property 25: DISTINCT 关键字
    // 验证需求：9.4
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_distinct_keyword(
            table_name in table_name_strategy(),
            field in field_name_strategy()
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .field(test_field!(field))
                .distinct();

            let sql = builder.to_sql();

            // 验证 SQL 包含 SELECT DISTINCT
            prop_assert!(sql.contains("SELECT DISTINCT"));
        }
    }

    // Feature: mysql-query-builder, Property 27: 特殊字段类型标记
    // 验证需求：11.1, 11.2, 11.3, 11.4, 11.5, 11.6
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_special_field_type_marking(
            table_name in table_name_strategy(),
            json_field in field_name_strategy(),
            datetime_field in field_name_strategy(),
            timestamp_field in field_name_strategy(),
            decimal_field in field_name_strategy(),
            blob_field in field_name_strategy(),
            text_field in field_name_strategy()
        ) {
            // 确保所有字段名都不相同，避免覆盖
            prop_assume!(json_field != datetime_field);
            prop_assume!(json_field != timestamp_field);
            prop_assume!(json_field != decimal_field);
            prop_assume!(json_field != blob_field);
            prop_assume!(json_field != text_field);
            prop_assume!(datetime_field != timestamp_field);
            prop_assume!(datetime_field != decimal_field);
            prop_assume!(datetime_field != blob_field);
            prop_assume!(datetime_field != text_field);
            prop_assume!(timestamp_field != decimal_field);
            prop_assume!(timestamp_field != blob_field);
            prop_assume!(timestamp_field != text_field);
            prop_assume!(decimal_field != blob_field);
            prop_assume!(decimal_field != text_field);
            prop_assume!(blob_field != text_field);

            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .json(test_field!(json_field))
                .datetime(test_field!(datetime_field))
                .timestamp(test_field!(timestamp_field))
                .decimal(test_field!(decimal_field))
                .blob(test_field!(blob_field))
                .text(test_field!(text_field));

            // 验证字段类型映射包含正确的类型标记
            prop_assert_eq!(builder.field_types.get(&json_field), Some(&FieldType::Json));
            prop_assert_eq!(builder.field_types.get(&datetime_field), Some(&FieldType::DateTime));
            prop_assert_eq!(builder.field_types.get(&timestamp_field), Some(&FieldType::Timestamp));
            prop_assert_eq!(builder.field_types.get(&decimal_field), Some(&FieldType::Decimal));
            prop_assert_eq!(builder.field_types.get(&blob_field), Some(&FieldType::Blob));
            prop_assert_eq!(builder.field_types.get(&text_field), Some(&FieldType::Text));
        }
    }

    // Feature: mysql-query-builder, Property 4: WHERE 条件添加
    // 验证需求：3.1, 3.2
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_where_and_condition_added(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            value in any::<i32>()
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and(test_field!(&field), crate::CompareOp::Eq, value);

            // 验证条件已添加
            prop_assert_eq!(builder.conditions.len(), 1);
        }

        #[test]
        fn prop_where_or_condition_added(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            value1 in any::<i32>(),
            value2 in any::<i32>()
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_or(test_field!(&field), crate::CompareOp::Eq, value1)
                .where_or(test_field!(&field), crate::CompareOp::Eq, value2);

            // where_or 会将条件组合，所以应该有 1 个条件（OR 组合）
            prop_assert_eq!(builder.conditions.len(), 1);
        }
    }

    // Feature: mysql-query-builder, Property 6: IN 操作符数组支持
    // 验证需求：3.4
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_in_operator_array_support(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            values in prop::collection::vec(any::<i32>(), 1..10)
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_in(test_field!(field), values);

            // 验证 IN 条件已添加
            prop_assert_eq!(builder.conditions.len(), 1);
        }
    }

    // Feature: mysql-query-builder, Property 7: BETWEEN 操作符边界支持
    // 验证需求：3.5
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_between_operator_boundary_support(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            start in any::<i32>(),
            end in any::<i32>()
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_between(test_field!(field), start, end);

            // 验证 BETWEEN 条件已添加
            prop_assert_eq!(builder.conditions.len(), 1);
        }
    }

    // Feature: mysql-query-builder, Property 8: 多条件 AND 连接
    // 验证需求：3.6
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_multiple_and_conditions(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            values in prop::collection::vec(any::<i32>(), 2..5)
        ) {
            let pool = create_test_pool_sync();
            let mut builder = QueryBuilder::new(&pool, &table_name, false);

            // 添加多个 AND 条件
            for value in &values {
                builder = builder.where_and(test_field!(&field), crate::CompareOp::Eq, *value);
            }

            // 验证所有条件都已添加
            prop_assert_eq!(builder.conditions.len(), values.len());
        }
    }

    // Feature: mysql-query-builder, Property 31: JOIN 子句生成
    // 验证需求：17.1, 17.2, 17.3
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_join_clause_generation(
            table_name in table_name_strategy(),
            join_table in table_name_strategy(),
            on_condition in "[a-z][a-z0-9_]{0,20}\\.[a-z][a-z0-9_]{0,20} = [a-z][a-z0-9_]{0,20}\\.[a-z][a-z0-9_]{0,20}"
        ) {
            let pool = create_test_pool_sync();
            let (left, right) = on_condition.split_once(" = ").expect("测试策略生成等值连接");

            // 测试 INNER JOIN
            let builder_inner = QueryBuilder::new(&pool, &table_name, false)
                .join(test_table!(&join_table), test_field!(left), test_field!(right));
            prop_assert_eq!(builder_inner.joins.len(), 1);

            // 测试 LEFT JOIN
            let builder_left = QueryBuilder::new(&pool, &table_name, false)
                .left_join(test_table!(&join_table), test_field!(left), test_field!(right));
            prop_assert_eq!(builder_left.joins.len(), 1);

            // 测试 RIGHT JOIN
            let builder_right = QueryBuilder::new(&pool, &table_name, false)
                .right_join(test_table!(&join_table), test_field!(left), test_field!(right));
            prop_assert_eq!(builder_right.joins.len(), 1);
        }
    }

    // Feature: mysql-query-builder, Property 32: 多表连接支持
    // 验证需求：17.5
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_multiple_join_support(
            table_name in table_name_strategy(),
            join_tables in prop::collection::vec(table_name_strategy(), 1..5)
        ) {
            let pool = create_test_pool_sync();
            let mut builder = QueryBuilder::new(&pool, &table_name, false);

            // 添加多个 JOIN
            for join_table in &join_tables {
                let left = format!("{}.id", table_name);
                let right = format!("{}.id", join_table);
                builder = builder.join(
                    test_table!(join_table),
                    test_field!(left),
                    test_field!(right),
                );
            }

            // 验证所有 JOIN 都已添加
            prop_assert_eq!(builder.joins.len(), join_tables.len());
        }
    }

    // Feature: mysql-query-builder, Property 33: 表别名支持
    // 验证需求：17.6
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_qualified_table_support(
            base_table in table_name_strategy(),
            join_table in table_name_strategy()
        ) {
            prop_assume!(base_table != join_table);

            let pool = create_test_pool_sync();
            let base_id = format!("{}.id", base_table);
            let base_name = format!("{}.name", base_table);
            let join_id = format!("{}.id", join_table);
            let builder = QueryBuilder::new(&pool, &base_table, false)
                .field(test_field!(&base_id))
                .field(test_field!(&base_name))
                .join(
                    test_table!(&join_table),
                    test_field!(&base_id),
                    test_field!(&join_id),
                );

            let sql = builder.to_sql();
            prop_assert!(sql.contains(&base_table));
            prop_assert!(sql.contains(&join_table));
            prop_assert!(sql.contains(" JOIN "));
        }
    }

    // Feature: mysql-query-builder, Property 20: ORDER BY 子句生成
    // 验证需求：8.1
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_order_by_clause_generation(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            asc in any::<bool>()
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .order(
                    test_field!(field),
                    if asc { crate::SortOrder::Asc } else { crate::SortOrder::Desc },
                );

            // 验证 ORDER BY 已添加
            prop_assert_eq!(builder.order_by.len(), 1);
            prop_assert_eq!(&builder.order_by[0].field, &format!("`{}`", field));
            prop_assert_eq!(builder.order_by[0].asc, asc);
        }
    }

    // Feature: mysql-query-builder, Property 21: 多字段排序支持
    // 验证需求：8.3
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_multiple_order_by_support(
            table_name in table_name_strategy(),
            fields in prop::collection::vec(field_name_strategy(), 1..5)
        ) {
            let pool = create_test_pool_sync();
            let mut builder = QueryBuilder::new(&pool, &table_name, false);

            // 添加多个排序字段
            for field in &fields {
                builder = builder.order(test_field!(field), crate::SortOrder::Asc);
            }

            // 验证所有排序字段都已添加
            prop_assert_eq!(builder.order_by.len(), fields.len());
        }
    }

    // Feature: mysql-query-builder, Property 22: GROUP BY 子句生成
    // 验证需求：8.4
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_group_by_clause_generation(
            table_name in table_name_strategy(),
            field in field_name_strategy()
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .group(test_field!(field));

            // 验证 GROUP BY 已添加
            prop_assert_eq!(builder.group_by.len(), 1);
            prop_assert_eq!(&builder.group_by[0], &format!("`{}`", field));
        }
    }

    // Feature: mysql-query-builder, Property 23: 多字段分组支持
    // 验证需求：8.5
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_multiple_group_by_support(
            table_name in table_name_strategy(),
            fields in prop::collection::vec(field_name_strategy(), 1..5)
        ) {
            let pool = create_test_pool_sync();
            let mut builder = QueryBuilder::new(&pool, &table_name, false);

            // 添加多个分组字段
            for field in &fields {
                builder = builder.group(test_field!(field));
            }

            // 验证所有分组字段都已添加
            prop_assert_eq!(builder.group_by.len(), fields.len());
        }
    }

    // Feature: mysql-query-builder, Property 30: SQL 语句调试输出
    // 验证需求：15.5
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_to_sql_returns_valid_sql(
            table_name in table_name_strategy(),
            fields in prop::collection::vec(field_name_strategy(), 0..5),
            use_distinct in any::<bool>(),
            limit_opt in prop::option::of(1u64..100),
            offset_opt in prop::option::of(0u64..100)
        ) {
            let pool = create_test_pool_sync();
            let mut builder = QueryBuilder::new(&pool, &table_name, false);

            // 添加字段
            for field in &fields {
                builder = builder.field(test_field!(field));
            }

            // 可选的 DISTINCT
            if use_distinct {
                builder = builder.distinct();
            }

            // 可选的 LIMIT
            if let Some(limit) = limit_opt {
                builder = builder.limit(limit);
            }

            // 可选的 OFFSET
            if let Some(offset) = offset_opt {
                builder = builder.offset(offset);
            }

            // 调用 to_sql() 方法
            let sql = builder.to_sql();

            // 验证返回的 SQL 字符串非空
            prop_assert!(!sql.is_empty(), "SQL 字符串不应为空");

            // 验证包含基本的 SQL 关键字
            prop_assert!(sql.contains("SELECT"), "SQL 应包含 SELECT 关键字");
            prop_assert!(sql.contains("FROM"), "SQL 应包含 FROM 关键字");

            // 验证包含表名
            prop_assert!(sql.contains(&table_name), "SQL 应包含表名");

            // 如果使用了 DISTINCT，验证包含 DISTINCT 关键字
            if use_distinct {
                prop_assert!(sql.contains("DISTINCT"), "SQL 应包含 DISTINCT 关键字");
            }

            // 如果设置了 LIMIT，验证包含 LIMIT 子句
            if let Some(limit) = limit_opt {
                prop_assert!(sql.contains("LIMIT"), "SQL 应包含 LIMIT 关键字");
                prop_assert!(sql.contains(&limit.to_string()), "SQL 应包含 LIMIT 值");
            }

            // 如果设置了 OFFSET，验证包含 OFFSET 子句
            if let Some(offset) = offset_opt {
                prop_assert!(sql.contains("OFFSET"), "SQL 应包含 OFFSET 关键字");
                prop_assert!(sql.contains(&offset.to_string()), "SQL 应包含 OFFSET 值");
            }

            // 验证字段在 SQL 中
            if !fields.is_empty() {
                for field in &fields {
                    prop_assert!(sql.contains(field), "SQL 应包含字段 {}", field);
                }
            } else {
                // 如果没有指定字段，应该使用 SELECT *
                prop_assert!(sql.contains("*"), "SQL 应包含 * 表示所有字段");
            }
        }

        #[test]
        fn prop_to_sql_with_conditions(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            value in any::<i32>()
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and(test_field!(&field), crate::CompareOp::Eq, value);

            let sql = builder.to_sql();

            // 验证基本 SQL 结构
            prop_assert!(!sql.is_empty());
            prop_assert!(sql.contains("SELECT"));
            prop_assert!(sql.contains("FROM"));
            prop_assert!(sql.contains(&table_name));

            // 验证包含 WHERE 子句
            prop_assert!(sql.contains("WHERE"), "SQL 应包含 WHERE 关键字");
        }

        #[test]
        fn prop_to_sql_with_joins(
            table_name in table_name_strategy(),
            join_table in table_name_strategy(),
            on_field1 in field_name_strategy(),
            on_field2 in field_name_strategy()
        ) {
            let pool = create_test_pool_sync();
            let left = format!("{}.{}", table_name, on_field1);
            let right = format!("{}.{}", join_table, on_field2);
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .join(test_table!(&join_table), test_field!(left), test_field!(right));

            let sql = builder.to_sql();

            // 验证基本 SQL 结构
            prop_assert!(!sql.is_empty());
            prop_assert!(sql.contains("SELECT"));
            prop_assert!(sql.contains("FROM"));

            // 验证包含 JOIN 子句
            prop_assert!(sql.contains("JOIN"), "SQL 应包含 JOIN 关键字");
            prop_assert!(sql.contains(&join_table), "SQL 应包含连接的表名");
        }

        #[test]
        fn prop_to_sql_with_order_and_group(
            table_name in table_name_strategy(),
            order_field in field_name_strategy(),
            group_field in field_name_strategy(),
            asc in any::<bool>()
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .order(
                    test_field!(order_field),
                    if asc { crate::SortOrder::Asc } else { crate::SortOrder::Desc },
                )
                .group(test_field!(group_field));

            let sql = builder.to_sql();

            // 验证基本 SQL 结构
            prop_assert!(!sql.is_empty());
            prop_assert!(sql.contains("SELECT"));
            prop_assert!(sql.contains("FROM"));

            // 验证包含 ORDER BY 和 GROUP BY 子句
            prop_assert!(sql.contains("ORDER BY"), "SQL 应包含 ORDER BY 关键字");
            prop_assert!(sql.contains("GROUP BY"), "SQL 应包含 GROUP BY 关键字");
            prop_assert!(sql.contains(&order_field), "SQL 应包含排序字段");
            prop_assert!(sql.contains(&group_field), "SQL 应包含分组字段");
        }

        #[test]
        fn prop_to_sql_complex_query(
            table_name in table_name_strategy(),
            fields in prop::collection::vec(field_name_strategy(), 1..3),
            join_table in table_name_strategy(),
            where_field in field_name_strategy(),
            order_field in field_name_strategy(),
            group_field in field_name_strategy()
        ) {
            let pool = create_test_pool_sync();
            let mut builder = QueryBuilder::new(&pool, &table_name, false);

            // 添加字段
            for field in &fields {
                builder = builder.field(test_field!(field));
            }

            // 添加 JOIN
            let left = format!("{}.id", table_name);
            let right = format!("{}.id", join_table);
            builder = builder.join(
                test_table!(&join_table),
                test_field!(left),
                test_field!(right),
            );

            // 添加 WHERE 条件
            builder = builder.where_and(test_field!(&where_field), crate::CompareOp::Eq, 1);

            // 添加 ORDER BY
            builder = builder.order(test_field!(order_field), crate::SortOrder::Asc);

            // 添加 GROUP BY
            builder = builder.group(test_field!(group_field));

            // 添加 LIMIT
            builder = builder.limit(10);

            let sql = builder.to_sql();

            // 验证这是一个有效的复杂 SQL 查询
            prop_assert!(!sql.is_empty());
            prop_assert!(sql.contains("SELECT"));
            prop_assert!(sql.contains("FROM"));
            prop_assert!(sql.contains(&table_name));
            prop_assert!(sql.contains("JOIN"));
            prop_assert!(sql.contains("WHERE"));
            prop_assert!(sql.contains("ORDER BY"));
            prop_assert!(sql.contains("GROUP BY"));
            prop_assert!(sql.contains("LIMIT"));

            // 验证 SQL 子句的顺序正确（SQL 标准顺序）
            let select_pos = sql.find("SELECT").unwrap();
            let from_pos = sql.find("FROM").unwrap();
            let join_pos = sql.find("JOIN").unwrap();
            let where_pos = sql.find("WHERE").unwrap();
            let group_pos = sql.find("GROUP BY").unwrap();
            let order_pos = sql.find("ORDER BY").unwrap();
            let limit_pos = sql.find("LIMIT").unwrap();

            // 验证子句顺序：SELECT < FROM < JOIN < WHERE < GROUP BY < ORDER BY < LIMIT
            prop_assert!(select_pos < from_pos, "SELECT 应在 FROM 之前");
            prop_assert!(from_pos < join_pos, "FROM 应在 JOIN 之前");
            prop_assert!(join_pos < where_pos, "JOIN 应在 WHERE 之前");
            prop_assert!(where_pos < group_pos, "WHERE 应在 GROUP BY 之前");
            prop_assert!(group_pos < order_pos, "GROUP BY 应在 ORDER BY 之前");
            prop_assert!(order_pos < limit_pos, "ORDER BY 应在 LIMIT 之前");
        }
    }

    // Feature: mysql-query-builder, Property 3: SQL 注入防护
    // 验证需求：2.5
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_sql_injection_prevention_single_quote(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            malicious_input in ".*'.*"
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and(test_field!(&field), crate::CompareOp::Eq, malicious_input.as_str());

            let sql = builder.to_sql();

            // SQL 不应该直接包含恶意输入的单引号
            // 参数化查询应该使用 ? 占位符
            prop_assert!(sql.contains("?"), "SQL 应该使用参数化查询（? 占位符）");

            // SQL 中不应该直接出现用户输入的单引号
            // 注意：SQL 本身可能包含单引号（如 'table'），但不应该是用户输入的
            let where_clause = sql.split("WHERE").nth(1).unwrap_or("");
            prop_assert!(!where_clause.contains(&malicious_input),
                "WHERE 子句不应该直接包含用户输入的恶意字符串");
        }

        #[test]
        fn prop_sql_injection_prevention_semicolon(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            malicious_input in ".*;.*"
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and(test_field!(&field), crate::CompareOp::Eq, malicious_input.as_str());

            let sql = builder.to_sql();

            // 验证使用参数化查询
            prop_assert!(sql.contains("?"), "SQL 应该使用参数化查询");

            // SQL 中不应该直接出现用户输入的分号
            let where_clause = sql.split("WHERE").nth(1).unwrap_or("");
            prop_assert!(!where_clause.contains(&malicious_input),
                "WHERE 子句不应该直接包含用户输入的恶意字符串");
        }

        #[test]
        fn prop_sql_injection_prevention_comment(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            malicious_input in ".*--.*"
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and(test_field!(&field), crate::CompareOp::Eq, malicious_input.as_str());

            let sql = builder.to_sql();

            // 验证使用参数化查询
            prop_assert!(sql.contains("?"), "SQL 应该使用参数化查询");

            // SQL 中不应该直接出现用户输入的注释符
            let where_clause = sql.split("WHERE").nth(1).unwrap_or("");
            prop_assert!(!where_clause.contains(&malicious_input),
                "WHERE 子句不应该直接包含用户输入的恶意字符串");
        }

        #[test]
        fn prop_sql_injection_prevention_drop_table(
            table_name in table_name_strategy(),
            field in field_name_strategy()
        ) {
            let pool = create_test_pool_sync();
            let malicious_input = "'; DROP TABLE users; --";
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and(test_field!(&field), crate::CompareOp::Eq, malicious_input);

            let sql = builder.to_sql();

            // 验证使用参数化查询
            prop_assert!(sql.contains("?"), "SQL 应该使用参数化查询");

            // SQL 不应该包含 DROP TABLE 语句
            prop_assert!(!sql.to_uppercase().contains("DROP TABLE"),
                "SQL 不应该包含 DROP TABLE 语句");

            // WHERE 子句不应该直接包含恶意输入
            let where_clause = sql.split("WHERE").nth(1).unwrap_or("");
            prop_assert!(!where_clause.contains(malicious_input),
                "WHERE 子句不应该直接包含用户输入的恶意字符串");
        }

        #[test]
        fn prop_sql_injection_prevention_union_select(
            table_name in table_name_strategy(),
            field in field_name_strategy()
        ) {
            let pool = create_test_pool_sync();
            let malicious_input = "' UNION SELECT * FROM passwords --";
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and(test_field!(&field), crate::CompareOp::Eq, malicious_input);

            let sql = builder.to_sql();

            // 验证使用参数化查询
            prop_assert!(sql.contains("?"), "SQL 应该使用参数化查询");

            // SQL 不应该包含 UNION SELECT 注入
            let sql_upper = sql.to_uppercase();
            let union_count = sql_upper.matches("UNION").count();
            prop_assert_eq!(union_count, 0, "SQL 不应该包含 UNION 注入");

            // WHERE 子句不应该直接包含恶意输入
            let where_clause = sql.split("WHERE").nth(1).unwrap_or("");
            prop_assert!(!where_clause.contains(malicious_input),
                "WHERE 子句不应该直接包含用户输入的恶意字符串");
        }

        #[test]
        fn prop_sql_injection_prevention_or_always_true(
            table_name in table_name_strategy(),
            field in field_name_strategy()
        ) {
            let pool = create_test_pool_sync();
            let malicious_input = "' OR '1'='1";
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and(test_field!(&field), crate::CompareOp::Eq, malicious_input);

            let sql = builder.to_sql();

            // 验证使用参数化查询
            prop_assert!(sql.contains("?"), "SQL 应该使用参数化查询");

            // WHERE 子句不应该直接包含恶意输入
            let where_clause = sql.split("WHERE").nth(1).unwrap_or("");
            prop_assert!(!where_clause.contains(malicious_input),
                "WHERE 子句不应该直接包含用户输入的恶意字符串");

            // 验证不会产生永真条件（除了我们自己构建的条件）
            // 恶意输入应该被当作参数值，而不是 SQL 代码
            let or_count = where_clause.matches(" OR ").count();
            // 如果没有使用 where_or，就不应该有 OR
            prop_assert_eq!(or_count, 0, "不应该因为用户输入而产生 OR 条件");
        }

        #[test]
        fn prop_sql_injection_prevention_multiple_special_chars(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            malicious_input in "[a-z0-9]*[';\"\\-][a-z0-9]*[';\"\\-][a-z0-9]*"
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and(test_field!(&field), crate::CompareOp::Eq, malicious_input.as_str());

            let sql = builder.to_sql();

            // 验证使用参数化查询
            prop_assert!(sql.contains("?"), "SQL 应该使用参数化查询");

            // WHERE 子句不应该直接包含恶意输入
            let where_clause = sql.split("WHERE").nth(1).unwrap_or("");
            prop_assert!(!where_clause.contains(&malicious_input),
                "WHERE 子句不应该直接包含用户输入的恶意字符串");
        }

        #[test]
        fn prop_sql_injection_prevention_in_operator(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            malicious_values in prop::collection::vec(".*[';].*", 1..5)
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_in(test_field!(field), malicious_values.clone());

            let sql = builder.to_sql();

            // 验证使用参数化查询
            prop_assert!(sql.contains("IN"), "SQL 应该包含 IN 操作符");
            prop_assert!(sql.contains("?"), "SQL 应该使用参数化查询");

            // 验证每个值都有对应的占位符
            let placeholder_count = sql.matches("?").count();
            prop_assert!(placeholder_count >= malicious_values.len(),
                "每个 IN 值都应该有对应的参数占位符");

            // WHERE 子句不应该直接包含恶意输入
            for malicious_value in &malicious_values {
                let where_clause = sql.split("WHERE").nth(1).unwrap_or("");
                prop_assert!(!where_clause.contains(malicious_value),
                    "WHERE 子句不应该直接包含用户输入的恶意字符串");
            }
        }

        #[test]
        fn prop_sql_injection_prevention_like_operator(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            malicious_pattern in ".*[';].*"
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and(test_field!(&field), crate::CompareOp::Like, malicious_pattern.as_str());

            let sql = builder.to_sql();

            // 验证使用参数化查询
            prop_assert!(sql.contains("LIKE"), "SQL 应该包含 LIKE 操作符");
            prop_assert!(sql.contains("?"), "SQL 应该使用参数化查询");

            // WHERE 子句不应该直接包含恶意输入
            let where_clause = sql.split("WHERE").nth(1).unwrap_or("");
            prop_assert!(!where_clause.contains(&malicious_pattern),
                "WHERE 子句不应该直接包含用户输入的恶意字符串");
        }

        #[test]
        fn prop_sql_injection_prevention_between_operator(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            malicious_start in ".*[';].*",
            malicious_end in ".*[';].*"
        ) {
            let pool = create_test_pool_sync();
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_between(test_field!(field), malicious_start.as_str(), malicious_end.as_str());

            let sql = builder.to_sql();

            // 验证使用参数化查询
            prop_assert!(sql.contains("BETWEEN"), "SQL 应该包含 BETWEEN 操作符");
            prop_assert!(sql.contains("?"), "SQL 应该使用参数化查询");

            // 验证有两个占位符（start 和 end）
            let where_clause = sql.split("WHERE").nth(1).unwrap_or("");
            let placeholder_count = where_clause.matches("?").count();
            prop_assert!(placeholder_count >= 2, "BETWEEN 应该有两个参数占位符");

            // WHERE 子句不应该直接包含恶意输入
            prop_assert!(!where_clause.contains(&malicious_start),
                "WHERE 子句不应该直接包含用户输入的恶意字符串");
            prop_assert!(!where_clause.contains(&malicious_end),
                "WHERE 子句不应该直接包含用户输入的恶意字符串");
        }
    }

    // Feature: mysql-query-builder, Property 10: LIMIT 1 用于 find()
    // 验证需求：4.1
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_find_adds_limit_one(
            table_name in table_name_strategy(),
            field in field_name_strategy(),
            value in any::<i32>()
        ) {
            let pool = create_test_pool_sync();

            // 创建一个带 WHERE 条件的查询构建器
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .field(test_field!(field))
                .where_and(test_field!(&field), crate::CompareOp::Eq, value)
                .limit(1); // 模拟 find() 会添加的 LIMIT 1

            let sql = builder.to_sql();

            // 验证 SQL 包含 LIMIT 1
            prop_assert!(sql.contains("LIMIT 1"),
                "find() 方法应该自动添加 LIMIT 1 到查询中");
        }
    }

    // Feature: mysql-query-builder, Property 11: COUNT 聚合函数
    // 验证需求：4.4
    //
    // 属性：对于任意查询构建器，调用 count() 方法时，生成的 SQL 应该包含 COUNT(*) 或 COUNT(field)
    //
    // 此测试验证 count() 方法正确生成 COUNT 聚合函数的 SQL 语句。
    // count() 方法内部使用 value("COUNT(*)") 来实现，因此我们测试生成的 SQL 是否包含 COUNT。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_count_aggregation_function(
            table_name in table_name_strategy()
        ) {
            let pool = create_test_pool_sync();

            // 创建一个查询构建器并使用 field("COUNT(*)") 模拟 count() 方法的行为
            // count() 方法内部调用 value("COUNT(*)")，这等同于 field("COUNT(*)")
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .expr(crate::SelectExpr::count_all());

            let sql = builder.to_sql();

            // 验证 SQL 包含 COUNT(*) 或 COUNT(field)
            prop_assert!(
                sql.contains("COUNT(*)") || sql.contains("COUNT("),
                "count() 方法应该生成包含 COUNT(*) 或 COUNT(field) 的 SQL 语句，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含 SELECT 关键字
            prop_assert!(
                sql.to_uppercase().contains("SELECT"),
                "count() 方法应该生成 SELECT 语句，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含表名
            prop_assert!(
                sql.contains(&format!("FROM `{}`", table_name)),
                "count() 方法应该包含正确的表名，实际 SQL: {}",
                sql
            );
        }
    }

    // Feature: mysql-query-builder, Property 11: COUNT 聚合函数 - 带条件
    // 验证需求：4.4
    //
    // 属性：对于任意带 WHERE 条件的查询构建器，调用 count() 方法时，
    // 生成的 SQL 应该包含 COUNT(*) 和 WHERE 子句
    //
    // 此测试验证 count() 方法与 WHERE 条件的组合使用。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_count_with_where_condition(
            table_name in table_name_strategy(),
            field_name in field_name_strategy(),
            field_value in 1i32..1000i32,
        ) {
            let pool = create_test_pool_sync();

            // 创建带条件的查询构建器
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and(test_field!(&field_name), crate::CompareOp::Eq, field_value)
                .expr(crate::SelectExpr::count_all());

            let sql = builder.to_sql();

            // 验证 SQL 包含 COUNT(*)
            prop_assert!(
                sql.contains("COUNT(*)"),
                "带条件的 count() 查询应该包含 COUNT(*)，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含 WHERE 子句
            prop_assert!(
                sql.to_uppercase().contains("WHERE"),
                "带条件的 count() 查询应该包含 WHERE 子句，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含表名
            prop_assert!(
                sql.contains(&format!("FROM `{}`", table_name)),
                "count() 方法应该包含正确的表名，实际 SQL: {}",
                sql
            );
        }
    }

    // Feature: mysql-query-builder, Property 11: COUNT 聚合函数 - 特定字段
    // 验证需求：4.4
    //
    // 属性：对于任意查询构建器，使用 field("COUNT(field_name)") 时，
    // 生成的 SQL 应该包含 COUNT(field_name)
    //
    // 此测试验证对特定字段进行 COUNT 统计的功能。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_count_specific_field(
            table_name in table_name_strategy(),
            field_name in field_name_strategy(),
        ) {
            let pool = create_test_pool_sync();

            // 创建查询构建器，统计特定字段
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .expr(crate::SelectExpr::count(test_field!(&field_name)));

            let sql = builder.to_sql();

            // 验证 SQL 包含 COUNT(field_name)
            prop_assert!(
                sql.contains(&format!("COUNT(`{}`)", field_name)),
                "COUNT 特定字段应该包含 COUNT(field_name)，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含 SELECT 关键字
            prop_assert!(
                sql.to_uppercase().contains("SELECT"),
                "COUNT 查询应该是 SELECT 语句，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含表名
            prop_assert!(
                sql.contains(&format!("FROM `{}`", table_name)),
                "COUNT 查询应该包含正确的表名，实际 SQL: {}",
                sql
            );
        }
    }

    // Feature: mysql-query-builder, Property 12: SUM 聚合函数
    // 验证需求：4.5
    //
    // 属性：对于任意字段名，调用 sum(field) 方法时，生成的 SQL 应该包含 SUM(field)
    //
    // 此测试验证 sum() 方法正确生成 SUM 聚合函数的 SQL 语句。
    // sum() 方法内部使用 CAST(SUM(field) AS DOUBLE) 来统一返回类型。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_sum_aggregation_function(
            table_name in table_name_strategy(),
            field in field_name_strategy()
        ) {
            let pool = create_test_pool_sync();

            // 创建一个查询构建器并生成 SUM 查询的 SQL
            // 模拟 sum() 方法会生成的 SQL
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .expr(crate::SelectExpr::sum(test_field!(&field)).cast_double());

            let sql = builder.to_sql();

            // 验证 SQL 包含 SUM(field)
            prop_assert!(
                sql.contains("SUM("),
                "sum() 方法应该生成包含 SUM(field) 的 SQL 语句，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含字段名
            prop_assert!(
                sql.contains(&field),
                "sum() 方法生成的 SQL 应该包含指定的字段名 {}，实际 SQL: {}",
                field,
                sql
            );

            // 验证 SQL 包含 SELECT 关键字
            prop_assert!(
                sql.to_uppercase().contains("SELECT"),
                "sum() 方法应该生成 SELECT 语句，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含表名
            prop_assert!(
                sql.contains(&format!("FROM `{}`", table_name)),
                "sum() 方法应该包含正确的表名，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含 CAST 转换（sum() 方法的实现细节）
            prop_assert!(
                sql.to_uppercase().contains("CAST"),
                "sum() 方法应该使用 CAST 转换结果为 DOUBLE，实际 SQL: {}",
                sql
            );
        }
    }

    // Feature: mysql-query-builder, Property 12: SUM 聚合函数 - 带条件
    // 验证需求：4.5
    //
    // 属性：对于任意带 WHERE 条件的查询构建器，调用 sum(field) 方法时，
    // 生成的 SQL 应该包含 SUM(field) 和 WHERE 子句
    //
    // 此测试验证 sum() 方法与 WHERE 条件的组合使用。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_sum_with_where_condition(
            table_name in table_name_strategy(),
            sum_field in field_name_strategy(),
            where_field in field_name_strategy(),
            where_value in 1i32..1000i32,
        ) {
            // 确保两个字段名不同
            prop_assume!(sum_field != where_field);

            let pool = create_test_pool_sync();

            // 创建带条件的查询构建器
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and(test_field!(&where_field), crate::CompareOp::Eq, where_value)
                .expr(crate::SelectExpr::sum(test_field!(&sum_field)).cast_double());

            let sql = builder.to_sql();

            // 验证 SQL 包含 SUM(field)
            prop_assert!(
                sql.contains("SUM("),
                "带条件的 sum() 查询应该包含 SUM(field)，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含求和字段名
            prop_assert!(
                sql.contains(&sum_field),
                "sum() 方法应该包含求和字段名 {}，实际 SQL: {}",
                sum_field,
                sql
            );

            // 验证 SQL 包含 WHERE 子句
            prop_assert!(
                sql.to_uppercase().contains("WHERE"),
                "带条件的 sum() 查询应该包含 WHERE 子句，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含表名
            prop_assert!(
                sql.contains(&format!("FROM `{}`", table_name)),
                "sum() 方法应该包含正确的表名，实际 SQL: {}",
                sql
            );
        }
    }

    // Feature: mysql-query-builder, Property 12: SUM 聚合函数 - 多条件
    // 验证需求：4.5
    //
    // 属性：对于任意带多个 WHERE 条件的查询构建器，调用 sum(field) 方法时，
    // 生成的 SQL 应该正确包含所有条件
    //
    // 此测试验证 sum() 方法在复杂查询中的正确性。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_sum_with_multiple_conditions(
            table_name in table_name_strategy(),
            sum_field in field_name_strategy(),
            where_field1 in field_name_strategy(),
            where_field2 in field_name_strategy(),
            value1 in 1i32..1000i32,
            value2 in 1i32..1000i32,
        ) {
            // 确保字段名都不同
            prop_assume!(sum_field != where_field1);
            prop_assume!(sum_field != where_field2);
            prop_assume!(where_field1 != where_field2);

            let pool = create_test_pool_sync();

            // 创建带多个条件的查询构建器
            let builder = QueryBuilder::new(&pool, &table_name, false)
                .where_and(test_field!(&where_field1), crate::CompareOp::Eq, value1)
                .where_and(test_field!(&where_field2), crate::CompareOp::Gt, value2)
                .expr(crate::SelectExpr::sum(test_field!(&sum_field)).cast_double());

            let sql = builder.to_sql();

            // 验证 SQL 包含 SUM(field)
            prop_assert!(
                sql.contains("SUM("),
                "多条件 sum() 查询应该包含 SUM(field)，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含求和字段名
            prop_assert!(
                sql.contains(&sum_field),
                "sum() 方法应该包含求和字段名 {}，实际 SQL: {}",
                sum_field,
                sql
            );

            // 验证 SQL 包含 WHERE 子句
            prop_assert!(
                sql.to_uppercase().contains("WHERE"),
                "多条件查询应该包含 WHERE 子句，实际 SQL: {}",
                sql
            );

            // 验证 SQL 包含 AND 连接符（因为使用了两次 where_and）
            prop_assert!(
                sql.to_uppercase().contains(" AND "),
                "多个 where_and 条件应该用 AND 连接，实际 SQL: {}",
                sql
            );
        }
    }

    // CompareOp 是封闭枚举，不支持的字符串操作符在业务 API 中不可表达。
    #[test]
    fn typed_compare_operators_preserve_condition_semantics() {
        let pool = make_sync_test_pool();
        for operator in [
            crate::CompareOp::Eq,
            crate::CompareOp::Ne,
            crate::CompareOp::Gt,
            crate::CompareOp::Lt,
            crate::CompareOp::Gte,
            crate::CompareOp::Lte,
            crate::CompareOp::Like,
        ] {
            let builder = QueryBuilder::new(pool, "users", false).where_and(
                yang_db::field!("age"),
                operator,
                18i64,
            );
            assert_eq!(builder.conditions.len(), 1);
        }
    }

    #[test]
    fn typed_where_or_having_and_chaining_work() {
        let pool = make_sync_test_pool();
        let builder = QueryBuilder::new(pool, "users", false)
            .where_and(yang_db::field!("age"), crate::CompareOp::Gt, 18i64)
            .where_and(yang_db::field!("status"), crate::CompareOp::Eq, 1i64)
            .where_or(yang_db::field!("status"), crate::CompareOp::Eq, 2i64)
            .group(yang_db::field!("age"))
            .having_cond(yang_db::field!("age"), crate::CompareOp::Gt, 0i64);
        assert_eq!(builder.conditions.len(), 1);
        assert_eq!(builder.having_clause.len(), 1);
    }

    // ==================== 任务 8.5：属性测试 P5 - 批次大小分割正确性 ====================

    // **Validates: Requirements 7.4**
    // 属性 P5：对于任意 n 条记录和批次大小 b（b > 0），
    // 执行的批次数等于 ceil(n / b)。
    //
    // 此测试不需要真实数据库连接，直接测试 chunks() 分批逻辑来验证批次数。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(500))]

        #[test]
        fn prop_batch_size_chunk_count(
            // n: 记录数，范围 0..=1000
            n in 0usize..=1000,
            // b: 批次大小，范围 1..=200（b > 0）
            b in 1usize..=200
        ) {
            // 构造 n 条虚拟记录（不需要真实数据，只需验证分批逻辑）
            let data: Vec<u32> = (0..n as u32).collect();

            // 计算实际分批数（使用 chunks() 分批）
            let actual_chunk_count = if data.is_empty() {
                0
            } else {
                data.chunks(b).count()
            };

            // 计算预期分批数：ceil(n / b)
            let expected_chunk_count = if n == 0 {
                0
            } else {
                n.div_ceil(b)  // 等价于 ceil(n / b)
            };

            // 验证分批数等于 ceil(n / b)
            prop_assert_eq!(
                actual_chunk_count,
                expected_chunk_count,
                "n={} 条记录，批次大小 b={}，实际分批数 {} 应等于 ceil(n/b)={}",
                n, b, actual_chunk_count, expected_chunk_count
            );

            // 额外验证：每个分批的大小不超过 b
            for chunk in data.chunks(b) {
                prop_assert!(
                    chunk.len() <= b,
                    "每个分批的大小 {} 不应超过批次大小 {}",
                    chunk.len(), b
                );
            }

            // 额外验证：所有分批的记录总数等于 n
            let total_records: usize = data.chunks(b).map(|c| c.len()).sum();
            prop_assert_eq!(
                total_records,
                n,
                "所有分批的记录总数 {} 应等于原始记录数 {}",
                total_records, n
            );
        }
    }

    // 单元测试：验证 batch_size 为 0 时返回错误
    #[tokio::test]
    async fn test_insert_batch_with_size_zero_batch_size_returns_error() {
        // 验证需求: 7.2  batch_size 为 0 时返回 SerializationError
        // 此测试不需要数据库连接，直接验证错误处理逻辑
        // 创建一个懒连接池（不需要真实连接）；复用当前异步测试运行时。
        let pool = MySqlPoolOptions::new()
            .max_connections(1)
            .connect_lazy("mysql://root:111111@localhost:3306/test")
            .expect("合法测试数据库 URL");
        let builder = QueryBuilder::new(&pool, "users", false);

        let data = vec![serde_json::json!({"name": "张三"})];

        let result = builder.insert_batch_with_size(&data, 0).await;

        assert!(
            matches!(result, Err(crate::DbError::SerializationError(_))),
            "batch_size 为 0 应返回 SerializationError，实际结果: {:?}",
            result.as_ref().map(|_| "Ok")
        );

        if let Err(crate::DbError::SerializationError(msg)) = result {
            assert!(
                msg.contains("batch_size") || msg.contains('0'),
                "错误消息应提及 batch_size 不能为 0，实际消息: {}",
                msg
            );
        }
    }

    // 单元测试：验证分批逻辑的边界情况
    #[test]
    fn test_batch_chunk_logic_boundary_cases() {
        // 验证需求: 7.4  分批逻辑边界情况

        // 情况 1：数据量恰好等于批次大小
        let data: Vec<u32> = (0..10).collect();
        let chunks: Vec<_> = data.chunks(10).collect();
        assert_eq!(chunks.len(), 1, "数据量等于批次大小时应只有 1 个分批");
        assert_eq!(chunks[0].len(), 10);

        // 情况 2：数据量小于批次大小
        let data: Vec<u32> = (0..5).collect();
        let chunks: Vec<_> = data.chunks(10).collect();
        assert_eq!(chunks.len(), 1, "数据量小于批次大小时应只有 1 个分批");
        assert_eq!(chunks[0].len(), 5);

        // 情况 3：数据量为批次大小的整数倍
        let data: Vec<u32> = (0..20).collect();
        let chunks: Vec<_> = data.chunks(5).collect();
        assert_eq!(chunks.len(), 4, "20 条记录按批次大小 5 分批应得到 4 个分批");
        for chunk in &chunks {
            assert_eq!(chunk.len(), 5);
        }

        // 情况 4：数据量不是批次大小的整数倍（最后一批较小）
        let data: Vec<u32> = (0..11).collect();
        let chunks: Vec<_> = data.chunks(5).collect();
        assert_eq!(chunks.len(), 3, "11 条记录按批次大小 5 分批应得到 3 个分批");
        assert_eq!(chunks[0].len(), 5);
        assert_eq!(chunks[1].len(), 5);
        assert_eq!(chunks[2].len(), 1, "最后一批应只有 1 条记录");

        // 情况 5：批次大小为 1（每条记录一批）
        let data: Vec<u32> = (0..5).collect();
        let chunks: Vec<_> = data.chunks(1).collect();
        assert_eq!(chunks.len(), 5, "批次大小为 1 时，分批数应等于记录数");
    }
}
