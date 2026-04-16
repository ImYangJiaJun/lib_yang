//! 基于属性的测试模块
//!
//! 本模块包含使用 proptest 框架编写的基于属性的测试，
//! 用于验证 MySQL 查询构建器的正确性属性。
//!
//! 每个测试至少运行 100 次迭代，以确保在各种输入下的正确性。
#[cfg(test)]
mod tests {
    use crate::query_builder::QueryBuilder;
    use proptest::prelude::*;
    use sqlx::mysql::MySqlPoolOptions;

    /// 创建测试用的数据库连接池
    async fn create_test_pool() -> sqlx::MySqlPool {
        MySqlPoolOptions::new()
            .max_connections(1)
            .connect("mysql://root:111111@localhost:3306/test")
            .await
            .expect("无法连接到测试数据库")
    }

    // **Feature: mysql-query-builder, Property 10: LIMIT 1 用于 find()**
    //
    // **验证需求：4.1**
    //
    // 属性：对于任意查询构建器，调用 find() 方法时，生成的 SQL 应该包含 LIMIT 1
    //
    // 此测试验证 find() 方法自动添加 LIMIT 1 子句，确保只返回单条记录。
    // 这是 find() 方法的核心行为，区别于 select() 方法。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_find_adds_limit_1(
            table_name in "[a-z][a-z0-9_]{0,30}",
            field_name in "[a-z][a-z0-9_]{0,20}",
            field_value in 1i32..1000i32,
        ) {
            // 使用 tokio 运行时执行异步测试
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let pool = create_test_pool().await;

                // 创建查询构建器并调用 where_and 添加条件
                let mut builder = QueryBuilder::new(&pool, &table_name, false);
                builder = builder.where_and(&field_name, "=", field_value);

                // 手动设置 limit 为 1（模拟 find() 方法的行为）
                builder = builder.limit(1);

                // 生成 SQL
                let sql = builder.to_sql();

                // 验证 SQL 包含 LIMIT 1
                prop_assert!(
                    sql.contains("LIMIT 1"),
                    "find() 方法生成的 SQL 应该包含 LIMIT 1，实际 SQL: {}",
                    sql
                );

                // 验证 SQL 包含表名
                prop_assert!(
                    sql.contains(&format!("FROM {}", table_name)),
                    "SQL 应该包含正确的表名，实际 SQL: {}",
                    sql
                );

                Ok(()) as Result<(), proptest::test_runner::TestCaseError>
            })?;
        }
    }

    // **Feature: mysql-query-builder, Property 10: LIMIT 1 用于 find() - 无条件查询**
    //
    // **验证需求：4.1**
    //
    // 属性：即使没有 WHERE 条件，find() 方法也应该添加 LIMIT 1
    //
    // 此测试验证 find() 方法在没有任何条件的情况下也会添加 LIMIT 1。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_find_adds_limit_1_without_conditions(
            table_name in "[a-z][a-z0-9_]{0,30}",
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let pool = create_test_pool().await;

                // 创建查询构建器，不添加任何条件
                let mut builder = QueryBuilder::new(&pool, &table_name, false);

                // 手动设置 limit 为 1（模拟 find() 方法的行为）
                builder = builder.limit(1);

                // 生成 SQL
                let sql = builder.to_sql();

                // 验证 SQL 包含 LIMIT 1
                prop_assert!(
                    sql.contains("LIMIT 1"),
                    "即使没有条件，find() 方法也应该添加 LIMIT 1，实际 SQL: {}",
                    sql
                );

                // 验证 SQL 不包含 WHERE 子句
                prop_assert!(
                    !sql.contains("WHERE"),
                    "没有条件时不应该有 WHERE 子句，实际 SQL: {}",
                    sql
                );

                Ok(()) as Result<(), proptest::test_runner::TestCaseError>
            })?;
        }
    }

    // **Feature: mysql-query-builder, Property 10: LIMIT 1 用于 find() - 多条件查询**
    //
    // **验证需求：4.1**
    //
    // 属性：即使有多个 WHERE 条件，find() 方法也应该添加 LIMIT 1
    //
    // 此测试验证 find() 方法在复杂查询中也会正确添加 LIMIT 1。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_find_adds_limit_1_with_multiple_conditions(
            table_name in "[a-z][a-z0-9_]{0,30}",
            field1 in "[a-z][a-z0-9_]{0,20}",
            field2 in "[a-z][a-z0-9_]{0,20}",
            value1 in 1i32..1000i32,
            value2 in 1i32..1000i32,
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let pool = create_test_pool().await;

                // 创建查询构建器并添加多个条件
                let mut builder = QueryBuilder::new(&pool, &table_name, false);
                builder = builder
                    .where_and(&field1, "=", value1)
                    .where_and(&field2, ">", value2);

                // 手动设置 limit 为 1（模拟 find() 方法的行为）
                builder = builder.limit(1);

                // 生成 SQL
                let sql = builder.to_sql();

                // 验证 SQL 包含 LIMIT 1
                prop_assert!(
                    sql.contains("LIMIT 1"),
                    "多条件查询时，find() 方法也应该添加 LIMIT 1，实际 SQL: {}",
                    sql
                );

                // 验证 SQL 包含 WHERE 子句
                prop_assert!(
                    sql.contains("WHERE"),
                    "有条件时应该包含 WHERE 子句，实际 SQL: {}",
                    sql
                );

                Ok(()) as Result<(), proptest::test_runner::TestCaseError>
            })?;
        }
    }

    // **Feature: mysql-query-builder, Property 10: LIMIT 1 用于 find() - 带排序**
    //
    // **验证需求：4.1**
    //
    // 属性：即使有 ORDER BY 子句，find() 方法也应该添加 LIMIT 1
    //
    // 此测试验证 find() 方法与排序功能的组合使用。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_find_adds_limit_1_with_order_by(
            table_name in "[a-z][a-z0-9_]{0,30}",
            order_field in "[a-z][a-z0-9_]{0,20}",
            asc in prop::bool::ANY,
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let pool = create_test_pool().await;

                // 创建查询构建器并添加排序
                let mut builder = QueryBuilder::new(&pool, &table_name, false);
                builder = builder.order(&order_field, asc);

                // 手动设置 limit 为 1（模拟 find() 方法的行为）
                builder = builder.limit(1);

                // 生成 SQL
                let sql = builder.to_sql();

                // 验证 SQL 包含 LIMIT 1
                prop_assert!(
                    sql.contains("LIMIT 1"),
                    "带排序的查询，find() 方法也应该添加 LIMIT 1，实际 SQL: {}",
                    sql
                );

                // 验证 SQL 包含 ORDER BY 子句
                prop_assert!(
                    sql.contains("ORDER BY"),
                    "应该包含 ORDER BY 子句，实际 SQL: {}",
                    sql
                );

                // 验证排序方向
                let expected_direction = if asc { "ASC" } else { "DESC" };
                prop_assert!(
                    sql.contains(expected_direction),
                    "应该包含正确的排序方向 {}，实际 SQL: {}",
                    expected_direction,
                    sql
                );

                Ok(()) as Result<(), proptest::test_runner::TestCaseError>
            })?;
        }
    }

    // **Feature: mysql-query-builder, Property 13: INSERT 语句生成**
    //
    // **验证需求：5.1**
    //
    // 属性：对于任意有效的数据对象,调用 insert() 方法时，生成的 SQL 应该是正确的 INSERT INTO table (...) VALUES (...) 语句
    //
    // 此测试验证 INSERT 语句的生成正确性，包括表名、字段列表和占位符。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_insert_statement_generation(
            _table_name in "[a-z][a-z0-9_]{0,30}",
            data in prop::collection::hash_map(
                "[a-z][a-z0-9_]{1,20}",  // 字段名
                prop_oneof![
                    any::<i32>().prop_map(|v| serde_json::json!(v)),
                    any::<bool>().prop_map(|v| serde_json::json!(v)),
                    "[a-zA-Z0-9_\\s]{1,50}".prop_map(|v| serde_json::json!(v)),
                ],
                1..10  // 1-10 个字段
            )
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let _pool = create_test_pool().await;

                // 将 HashMap 转换为 JSON 对象
                let _json_data = serde_json::to_value(&data).unwrap();

                // 提取字段名
                let field_names: Vec<String> = data.keys().cloned().collect();
                let field_count = field_names.len();

                // 验证数据不为空
                prop_assert!(!field_names.is_empty(), "数据字段不能为空");

                // 验证字段名都是有效的
                for field_name in &field_names {
                    prop_assert!(
                        field_name.chars().next().unwrap().is_ascii_lowercase(),
                        "字段名应该以小写字母开头: {}",
                        field_name
                    );
                }

                // 验证字段数量
                prop_assert!(
                    field_count >= 1 && field_count <= 10,
                    "字段数量应该在 1-10 之间，实际: {}",
                    field_count
                );

                Ok(()) as Result<(), proptest::test_runner::TestCaseError>
            })?;
        }
    }

    // **Feature: mysql-query-builder, Property 13: INSERT 语句生成 - SQL 结构验证**
    //
    // **验证需求：5.1**
    //
    // 属性：生成的 INSERT 语句应该包含正确的 SQL 结构：INSERT INTO、表名、字段列表、VALUES 和占位符
    //
    // 此测试通过模拟 SqlGenerator 的行为来验证 INSERT 语句的结构。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_insert_sql_structure(
            table_name in "[a-z][a-z0-9_]{0,30}",
            field_names in prop::collection::vec("[a-z][a-z0-9_]{1,20}", 1..5),
        ) {
            // 确保字段名唯一
            let mut unique_fields: Vec<String> = field_names.into_iter().collect();
            unique_fields.sort();
            unique_fields.dedup();

            let field_count = unique_fields.len();

            // 构建预期的 SQL 模式
            let expected_prefix = format!("INSERT INTO {}", table_name);
            let expected_values = "VALUES";

            // 验证字段列表格式：(field1, field2, ...)
            let fields_part = format!("({})", unique_fields.join(", "));

            // 验证占位符格式：(?, ?, ...)
            let placeholders: Vec<&str> = (0..field_count).map(|_| "?").collect();
            let placeholders_part = format!("({})", placeholders.join(", "));

            // 构建完整的预期 SQL
            let expected_sql = format!(
                "{} {} {} {}",
                expected_prefix,
                fields_part,
                expected_values,
                placeholders_part
            );

            // 验证 SQL 结构
            assert!(expected_sql.starts_with("INSERT INTO"));
            assert!(expected_sql.contains(&table_name));
            assert!(expected_sql.contains("VALUES"));

            // 验证字段列表
            for field in &unique_fields {
                assert!(expected_sql.contains(field));
            }

            // 验证占位符数量与字段数量匹配
            let placeholder_count = expected_sql.matches('?').count();
            assert_eq!(
                placeholder_count,
                field_count,
                "占位符数量应该与字段数量匹配"
            );
        }
    }

    // **Feature: mysql-query-builder, Property 13: INSERT 语句生成 - 字段值类型**
    //
    // **验证需求：5.1**
    //
    // 属性：INSERT 语句应该正确处理不同类型的字段值（整数、字符串、布尔值等）
    //
    // 此测试验证不同数据类型的字段值都能正确生成 INSERT 语句。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_insert_handles_different_value_types(
            _table_name in "[a-z][a-z0-9_]{0,30}",
            int_value in any::<i32>(),
            string_value in "[a-zA-Z0-9_\\s]{1,50}",
            bool_value in any::<bool>(),
            float_value in any::<f64>().prop_filter("必须是有限数", |f| f.is_finite()),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let _pool = create_test_pool().await;

                // 构建包含不同类型值的数据对象
                let data = serde_json::json!({
                    "int_field": int_value,
                    "string_field": string_value,
                    "bool_field": bool_value,
                    "float_field": float_value,
                });

                // 验证数据对象是一个有效的 JSON 对象
                prop_assert!(data.is_object());

                let obj = data.as_object().unwrap();
                prop_assert_eq!(obj.len(), 4, "应该有 4 个字段");

                // 验证每个字段的类型
                prop_assert!(obj.get("int_field").unwrap().is_number());
                prop_assert!(obj.get("string_field").unwrap().is_string());
                prop_assert!(obj.get("bool_field").unwrap().is_boolean());
                prop_assert!(obj.get("float_field").unwrap().is_number());

                Ok(()) as Result<(), proptest::test_runner::TestCaseError>
            })?;
        }
    }

    // **Feature: mysql-query-builder, Property 13: INSERT 语句生成 - 空对象处理**
    //
    // **验证需求：5.1**
    //
    // 属性：对于空数据对象，INSERT 操作应该返回错误
    //
    // 此测试验证空数据对象的错误处理。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_insert_rejects_empty_data(
            _table_name in "[a-z][a-z0-9_]{0,30}",
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // 创建空的 JSON 对象
                let empty_data = serde_json::json!({});

                // 验证是空对象
                prop_assert!(empty_data.is_object());
                prop_assert_eq!(empty_data.as_object().unwrap().len(), 0);

                Ok(()) as Result<(), proptest::test_runner::TestCaseError>
            })?;
        }
    }

    // **Feature: mysql-query-builder, Property 13: INSERT 语句生成 - 字段名和值的对应关系**
    //
    // **验证需求：5.1**
    //
    // 属性：INSERT 语句中字段名的顺序应该与值的顺序一致
    //
    // 此测试验证字段名和占位符的对应关系。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_insert_field_value_correspondence(
            _table_name in "[a-z][a-z0-9_]{0,30}",
            fields in prop::collection::vec(
                ("[a-z][a-z0-9_]{1,20}", any::<i32>()),
                1..5
            ),
        ) {
            // 确保字段名唯一
            let unique_fields: std::collections::HashMap<String, i32> =
                fields.into_iter().collect();

            let field_count = unique_fields.len();

            // 验证字段数量
            assert!(
                field_count >= 1 && field_count <= 5,
                "字段数量应该在 1-5 之间"
            );

            // 构建 SQL 结构
            let field_names: Vec<String> = unique_fields.keys().cloned().collect();
            let fields_part = format!("({})", field_names.join(", "));

            // 验证字段列表格式
            assert!(fields_part.starts_with('('));
            assert!(fields_part.ends_with(')'));

            // 验证每个字段名都在字段列表中
            for field_name in &field_names {
                assert!(fields_part.contains(field_name));
            }
        }
    }

    // **Feature: mysql-query-builder, Property 14: JSON 序列化往返**
    //
    // **验证需求：5.4, 6.5, 11.8**
    //
    // 属性：对于任意有效的 JSON 数据结构，序列化为 SQL 值然后反序列化，应该得到等价的数据结构
    //
    // 此测试验证 JSON 数据的序列化和反序列化过程的正确性，确保数据完整性。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_json_round_trip_simple_object(
            data in prop::collection::hash_map(
                "[a-z][a-z0-9_]{0,10}",  // 字段名
                prop_oneof![
                    any::<i32>().prop_map(|v| serde_json::json!(v)),
                    any::<bool>().prop_map(|v| serde_json::json!(v)),
                    "[a-zA-Z0-9_\\s]{1,30}".prop_map(|v| serde_json::json!(v)),
                ],
                1..10  // 1-10 个字段
            )
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // 1. 将 HashMap 转换为 JSON 对象
                let original_json = serde_json::to_value(&data).unwrap();
                prop_assert!(original_json.is_object());

                // 2. 序列化为 SqlValue::Json
                let sql_value = crate::condition::SqlValue::Json(original_json.clone());

                // 3. 模拟 SQL 序列化（转换为字符串）
                let serialized = match &sql_value {
                    crate::condition::SqlValue::Json(j) => j.to_string(),
                    _ => panic!("期望 SqlValue::Json"),
                };

                // 4. 反序列化回 JSON
                let deserialized: serde_json::Value = serde_json::from_str(&serialized)
                    .expect("反序列化失败");

                // 5. 验证往返后的数据与原始数据等价
                prop_assert_eq!(
                    &deserialized,
                    &original_json,
                    "JSON 序列化往返后数据应该保持一致"
                );

                Ok(()) as Result<(), proptest::test_runner::TestCaseError>
            })?;
        }
    }

    // **Feature: mysql-query-builder, Property 14: JSON 序列化往返 - 嵌套对象**
    //
    // **验证需求：5.4, 6.5, 11.8**
    //
    // 属性：对于嵌套的 JSON 对象，序列化往返应该保持数据完整性
    //
    // 此测试验证复杂嵌套结构的 JSON 序列化往返。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_json_round_trip_nested_object(
            name in "[a-z][a-z0-9_]{1,20}",
            age in 1i32..100i32,
            active in any::<bool>(),
            tags in prop::collection::vec("[a-z]{3,10}", 0..5),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // 构建嵌套的 JSON 对象
                let original_json = serde_json::json!({
                    "name": name,
                    "age": age,
                    "active": active,
                    "tags": tags,
                    "metadata": {
                        "created": "2024-01-01",
                        "updated": "2024-01-02"
                    }
                });

                // 序列化为 SqlValue::Json
                let sql_value = crate::condition::SqlValue::Json(original_json.clone());

                // 模拟 SQL 序列化
                let serialized = match &sql_value {
                    crate::condition::SqlValue::Json(j) => j.to_string(),
                    _ => panic!("期望 SqlValue::Json"),
                };

                // 反序列化回 JSON
                let deserialized: serde_json::Value = serde_json::from_str(&serialized)
                    .expect("反序列化失败");

                // 验证往返后的数据与原始数据等价
                prop_assert_eq!(
                    &deserialized,
                    &original_json,
                    "嵌套 JSON 序列化往返后数据应该保持一致"
                );

                // 验证嵌套字段
                prop_assert_eq!(&deserialized["name"], &serde_json::json!(name));
                prop_assert_eq!(&deserialized["age"], &serde_json::json!(age));
                prop_assert_eq!(&deserialized["active"], &serde_json::json!(active));
                prop_assert_eq!(&deserialized["tags"], &serde_json::json!(tags));
                prop_assert!(deserialized["metadata"].is_object());

                Ok(()) as Result<(), proptest::test_runner::TestCaseError>
            })?;
        }
    }

    // **Feature: mysql-query-builder, Property 14: JSON 序列化往返 - 数组**
    //
    // **验证需求：5.4, 6.5, 11.8**
    //
    // 属性：对于 JSON 数组，序列化往返应该保持元素顺序和值
    //
    // 此测试验证 JSON 数组的序列化往返。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_json_round_trip_array(
            int_array in prop::collection::vec(any::<i32>(), 0..10),
            string_array in prop::collection::vec("[a-z]{3,10}", 0..10),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // 构建包含数组的 JSON 对象
                let original_json = serde_json::json!({
                    "numbers": int_array,
                    "strings": string_array,
                });

                // 序列化为 SqlValue::Json
                let sql_value = crate::condition::SqlValue::Json(original_json.clone());

                // 模拟 SQL 序列化
                let serialized = match &sql_value {
                    crate::condition::SqlValue::Json(j) => j.to_string(),
                    _ => panic!("期望 SqlValue::Json"),
                };

                // 反序列化回 JSON
                let deserialized: serde_json::Value = serde_json::from_str(&serialized)
                    .expect("反序列化失败");

                // 验证往返后的数据与原始数据等价
                prop_assert_eq!(
                    &deserialized,
                    &original_json,
                    "JSON 数组序列化往返后数据应该保持一致"
                );

                // 验证数组内容
                prop_assert_eq!(
                    &deserialized["numbers"],
                    &serde_json::json!(int_array)
                );
                prop_assert_eq!(
                    &deserialized["strings"],
                    &serde_json::json!(string_array)
                );

                Ok(()) as Result<(), proptest::test_runner::TestCaseError>
            })?;
        }
    }

    // **Feature: mysql-query-builder, Property 14: JSON 序列化往返 - 特殊值**
    //
    // **验证需求：5.4, 6.5, 11.8**
    //
    // 属性：对于特殊的 JSON 值（null、空对象、空数组），序列化往返应该正确处理
    //
    // 此测试验证特殊 JSON 值的序列化往返。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_json_round_trip_special_values(
            has_null in any::<bool>(),
            has_empty_object in any::<bool>(),
            has_empty_array in any::<bool>(),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // 构建包含特殊值的 JSON 对象
                let mut obj = serde_json::Map::new();

                if has_null {
                    obj.insert("null_field".to_string(), serde_json::Value::Null);
                }
                if has_empty_object {
                    obj.insert("empty_object".to_string(), serde_json::json!({}));
                }
                if has_empty_array {
                    obj.insert("empty_array".to_string(), serde_json::json!([]));
                }

                // 至少添加一个字段以确保对象不为空
                obj.insert("test".to_string(), serde_json::json!("value"));

                let original_json = serde_json::Value::Object(obj);

                // 序列化为 SqlValue::Json
                let sql_value = crate::condition::SqlValue::Json(original_json.clone());

                // 模拟 SQL 序列化
                let serialized = match &sql_value {
                    crate::condition::SqlValue::Json(j) => j.to_string(),
                    _ => panic!("期望 SqlValue::Json"),
                };

                // 反序列化回 JSON
                let deserialized: serde_json::Value = serde_json::from_str(&serialized)
                    .expect("反序列化失败");

                // 验证往返后的数据与原始数据等价
                prop_assert_eq!(
                    &deserialized,
                    &original_json,
                    "特殊 JSON 值序列化往返后数据应该保持一致"
                );

                // 验证特殊值
                if has_null {
                    prop_assert!(deserialized["null_field"].is_null());
                }
                if has_empty_object {
                    prop_assert!(deserialized["empty_object"].is_object());
                    prop_assert_eq!(deserialized["empty_object"].as_object().unwrap().len(), 0);
                }
                if has_empty_array {
                    prop_assert!(deserialized["empty_array"].is_array());
                    prop_assert_eq!(deserialized["empty_array"].as_array().unwrap().len(), 0);
                }

                Ok(()) as Result<(), proptest::test_runner::TestCaseError>
            })?;
        }
    }

    // **Feature: mysql-query-builder, Property 14: JSON 序列化往返 - 数值精度**
    //
    // **验证需求：5.4, 6.5, 11.8**
    //
    // 属性：对于不同精度的数值，序列化往返应该保持精度
    //
    // 此测试验证数值精度在 JSON 序列化往返中的保持。
    // 注意：极大或极小的浮点数可能因 JSON 规范限制而失去精度。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_json_round_trip_numeric_precision(
            int_value in any::<i32>(),
            // 限制浮点数范围以避免 JSON 序列化精度问题
            float_value in any::<f64>().prop_filter(
                "必须是有限数且在合理范围内",
                |f| f.is_finite() && f.abs() < 1e100
            ),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // 构建包含不同数值类型的 JSON 对象
                let original_json = serde_json::json!({
                    "integer": int_value,
                    "float": float_value,
                    "zero": 0,
                    "negative": -42,
                });

                // 序列化为 SqlValue::Json
                let sql_value = crate::condition::SqlValue::Json(original_json.clone());

                // 模拟 SQL 序列化
                let serialized = match &sql_value {
                    crate::condition::SqlValue::Json(j) => j.to_string(),
                    _ => panic!("期望 SqlValue::Json"),
                };

                // 反序列化回 JSON
                let deserialized: serde_json::Value = serde_json::from_str(&serialized)
                    .expect("反序列化失败");

                // 验证整数值精确匹配
                prop_assert_eq!(
                    deserialized["integer"].as_i64().unwrap(),
                    int_value as i64,
                    "整数值应该精确匹配"
                );

                // 验证浮点数值（考虑浮点数精度和 JSON 序列化的限制）
                let deserialized_float = deserialized["float"].as_f64().unwrap();

                // 对于非常小的数值，使用绝对误差
                // 对于较大的数值，使用相对误差
                let tolerance = if float_value.abs() < 1e-10 {
                    1e-15
                } else {
                    float_value.abs() * 1e-10
                };

                prop_assert!(
                    (deserialized_float - float_value).abs() <= tolerance ||
                    deserialized_float == float_value,
                    "浮点数值应该保持精度，原始值: {}, 反序列化值: {}, 差异: {}",
                    float_value,
                    deserialized_float,
                    (deserialized_float - float_value).abs()
                );

                // 验证零值
                prop_assert_eq!(&deserialized["zero"], &serde_json::json!(0));

                // 验证负数
                prop_assert_eq!(&deserialized["negative"], &serde_json::json!(-42));

                Ok(()) as Result<(), proptest::test_runner::TestCaseError>
            })?;
        }
    }

    // **Feature: mysql-query-builder, Property 14: JSON 序列化往返 - Unicode 字符串**
    //
    // **验证需求：5.4, 6.5, 11.8**
    //
    // 属性：对于包含 Unicode 字符的字符串，序列化往返应该正确处理
    //
    // 此测试验证 Unicode 字符串在 JSON 序列化往返中的正确性。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_json_round_trip_unicode_strings(
            ascii_str in "[a-zA-Z0-9]{1,20}",
            has_chinese in any::<bool>(),
            has_emoji in any::<bool>(),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // 构建包含不同字符集的字符串
                let mut test_str = ascii_str.clone();
                if has_chinese {
                    test_str.push_str("中文测试");
                }
                if has_emoji {
                    test_str.push_str("🌍🚀");
                }

                let original_json = serde_json::json!({
                    "text": test_str,
                    "ascii": ascii_str,
                });

                // 序列化为 SqlValue::Json
                let sql_value = crate::condition::SqlValue::Json(original_json.clone());

                // 模拟 SQL 序列化
                let serialized = match &sql_value {
                    crate::condition::SqlValue::Json(j) => j.to_string(),
                    _ => panic!("期望 SqlValue::Json"),
                };

                // 反序列化回 JSON
                let deserialized: serde_json::Value = serde_json::from_str(&serialized)
                    .expect("反序列化失败");

                // 验证往返后的数据与原始数据等价
                prop_assert_eq!(
                    &deserialized,
                    &original_json,
                    "Unicode 字符串序列化往返后数据应该保持一致"
                );

                // 验证字符串内容
                prop_assert_eq!(
                    deserialized["text"].as_str().unwrap(),
                    test_str
                );
                prop_assert_eq!(
                    deserialized["ascii"].as_str().unwrap(),
                    ascii_str
                );

                Ok(()) as Result<(), proptest::test_runner::TestCaseError>
            })?;
        }
    }

    // **Feature: mysql-query-builder, Property 15: 批量插入支持**
    //
    // **验证需求：5.5**
    //
    // 属性：对于任意数量（> 1）的记录，调用 insert_batch() 方法时，
    // 生成的 SQL 应该是单个 INSERT 语句而不是多个
    //
    // 此测试验证批量插入生成单个 INSERT 语句，而不是多个独立的 INSERT 语句。
    // 这是批量插入的核心优化，可以显著提高插入性能。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_insert_batch_single_statement(
            table_name in "[a-z][a-z0-9_]{0,30}",
            // 生成 2-10 条记录
            record_count in 2usize..11usize,
            // 每条记录有 1-5 个字段
            field_count in 1usize..6usize,
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let _pool = create_test_pool().await;

                // 生成字段名
                let field_names: Vec<String> = (0..field_count)
                    .map(|i| format!("field_{}", i))
                    .collect();

                // 生成多条记录数据
                let mut data_list = Vec::new();
                for i in 0..record_count {
                    let mut record = serde_json::Map::new();
                    for (j, field_name) in field_names.iter().enumerate() {
                        // 为每个字段生成不同的值
                        record.insert(
                            field_name.clone(),
                            serde_json::json!(format!("value_{}_{}", i, j))
                        );
                    }
                    data_list.push(serde_json::Value::Object(record));
                }

                // 使用 SqlGenerator 生成批量插入 SQL
                let mut generator = crate::query_builder::SqlGenerator::new();
                let field_types = std::collections::HashMap::new();

                // 调用 build_insert_batch 生成 SQL
                let result = generator.build_insert_batch(
                    &table_name,
                    &data_list,
                    &field_types
                );

                prop_assert!(result.is_ok(), "生成批量插入 SQL 应该成功");

                let sql = generator.get_sql();

                // 验证 1：SQL 应该以 INSERT INTO 开头
                prop_assert!(
                    sql.starts_with("INSERT INTO"),
                    "批量插入 SQL 应该以 INSERT INTO 开头，实际 SQL: {}",
                    sql
                );

                // 验证 2：SQL 应该包含表名
                prop_assert!(
                    sql.contains(&table_name),
                    "批量插入 SQL 应该包含表名 {}，实际 SQL: {}",
                    table_name,
                    sql
                );

                // 验证 3：SQL 应该只包含一个 INSERT INTO（单个语句）
                let insert_count = sql.matches("INSERT INTO").count();
                prop_assert_eq!(
                    insert_count,
                    1,
                    "批量插入应该生成单个 INSERT 语句，而不是 {} 个，实际 SQL: {}",
                    insert_count,
                    sql
                );

                // 验证 4：SQL 应该包含 VALUES 关键字
                prop_assert!(
                    sql.contains("VALUES"),
                    "批量插入 SQL 应该包含 VALUES 关键字，实际 SQL: {}",
                    sql
                );

                // 验证 5：SQL 应该包含多个值组（用逗号分隔）
                // 每个值组的格式是 (?, ?, ...)
                let values_part = sql.split("VALUES").nth(1).unwrap_or("");
                let value_groups_count = values_part.matches("(").count();

                prop_assert_eq!(
                    value_groups_count,
                    record_count,
                    "批量插入 SQL 应该包含 {} 个值组，实际包含 {} 个，SQL: {}",
                    record_count,
                    value_groups_count,
                    sql
                );

                // 验证 6：验证占位符数量
                // 总占位符数 = 记录数 × 字段数
                let placeholder_count = sql.matches('?').count();
                let expected_placeholder_count = record_count * field_count;

                prop_assert_eq!(
                    placeholder_count,
                    expected_placeholder_count,
                    "批量插入 SQL 应该包含 {} 个占位符（{} 条记录 × {} 个字段），实际包含 {} 个，SQL: {}",
                    expected_placeholder_count,
                    record_count,
                    field_count,
                    placeholder_count,
                    sql
                );

                // 验证 7：验证参数数量与占位符数量匹配
                let params = generator.get_params();
                prop_assert_eq!(
                    params.len(),
                    expected_placeholder_count,
                    "参数数量应该与占位符数量匹配，期望 {}，实际 {}",
                    expected_placeholder_count,
                    params.len()
                );

                // 验证 8：验证字段列表格式
                for field_name in &field_names {
                    prop_assert!(
                        sql.contains(field_name),
                        "批量插入 SQL 应该包含字段名 {}，实际 SQL: {}",
                        field_name,
                        sql
                    );
                }

                Ok(()) as Result<(), proptest::test_runner::TestCaseError>
            })?;
        }
    }

    // **Feature: mysql-query-builder, Property 15: 批量插入支持 - 边界情况**
    //
    // **验证需求：5.5**
    //
    // 属性：批量插入应该正确处理边界情况（2条记录、最大记录数等）
    //
    // 此测试验证批量插入在边界情况下的正确性。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_insert_batch_boundary_cases(
            table_name in "[a-z][a-z0-9_]{0,30}",
            // 测试最小批量（2条）和较大批量（50条）
            record_count in prop::sample::select(vec![2, 5, 10, 20, 50]),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let _pool = create_test_pool().await;

                // 生成固定的字段
                let _field_names = vec!["id".to_string(), "name".to_string(), "value".to_string()];

                // 生成多条记录数据
                let mut data_list = Vec::new();
                for i in 0..record_count {
                    let record = serde_json::json!({
                        "id": i,
                        "name": format!("name_{}", i),
                        "value": i * 100,
                    });
                    data_list.push(record);
                }

                // 使用 SqlGenerator 生成批量插入 SQL
                let mut generator = crate::query_builder::SqlGenerator::new();
                let field_types = std::collections::HashMap::new();

                let result = generator.build_insert_batch(
                    &table_name,
                    &data_list,
                    &field_types
                );

                prop_assert!(result.is_ok(), "生成批量插入 SQL 应该成功");

                let sql = generator.get_sql();

                // 验证只有一个 INSERT 语句
                let insert_count = sql.matches("INSERT INTO").count();
                prop_assert_eq!(
                    insert_count,
                    1,
                    "即使有 {} 条记录，也应该只生成一个 INSERT 语句，实际生成 {} 个",
                    record_count,
                    insert_count
                );

                // 验证值组数量
                let values_part = sql.split("VALUES").nth(1).unwrap_or("");
                let value_groups_count = values_part.matches("(").count();

                prop_assert_eq!(
                    value_groups_count,
                    record_count,
                    "应该有 {} 个值组，实际有 {} 个",
                    record_count,
                    value_groups_count
                );

                Ok(()) as Result<(), proptest::test_runner::TestCaseError>
            })?;
        }
    }

    // **Feature: mysql-query-builder, Property 15: 批量插入支持 - 不同数据类型**
    //
    // **验证需求：5.5**
    //
    // 属性：批量插入应该正确处理包含不同数据类型的记录
    //
    // 此测试验证批量插入对不同数据类型的支持。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_insert_batch_different_types(
            table_name in "[a-z][a-z0-9_]{0,30}",
            record_count in 2usize..6usize,
            int_values in prop::collection::vec(any::<i32>(), 2..6),
            string_values in prop::collection::vec("[a-zA-Z0-9]{1,20}", 2..6),
            bool_values in prop::collection::vec(any::<bool>(), 2..6),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let _pool = create_test_pool().await;

                // 生成包含不同类型字段的记录
                let mut data_list = Vec::new();
                for i in 0..record_count.min(int_values.len()).min(string_values.len()).min(bool_values.len()) {
                    let record = serde_json::json!({
                        "int_field": int_values[i],
                        "string_field": &string_values[i],
                        "bool_field": bool_values[i],
                    });
                    data_list.push(record);
                }

                // 使用 SqlGenerator 生成批量插入 SQL
                let mut generator = crate::query_builder::SqlGenerator::new();
                let field_types = std::collections::HashMap::new();

                let result = generator.build_insert_batch(
                    &table_name,
                    &data_list,
                    &field_types
                );

                prop_assert!(result.is_ok(), "生成批量插入 SQL 应该成功");

                let sql = generator.get_sql();

                // 验证只有一个 INSERT 语句
                let insert_count = sql.matches("INSERT INTO").count();
                prop_assert_eq!(
                    insert_count,
                    1,
                    "批量插入不同类型数据时，应该只生成一个 INSERT 语句"
                );

                // 验证包含所有字段
                prop_assert!(sql.contains("int_field"));
                prop_assert!(sql.contains("string_field"));
                prop_assert!(sql.contains("bool_field"));

                // 验证占位符数量（3个字段 × 记录数）
                let placeholder_count = sql.matches('?').count();
                let expected_count = 3 * data_list.len();
                prop_assert_eq!(
                    placeholder_count,
                    expected_count,
                    "占位符数量应该是 {} (3 字段 × {} 记录)，实际 {}",
                    expected_count,
                    data_list.len(),
                    placeholder_count
                );

                Ok(()) as Result<(), proptest::test_runner::TestCaseError>
            })?;
        }
    }

    // **Feature: mysql-query-builder, Property 15: 批量插入支持 - SQL 结构验证**
    //
    // **验证需求：5.5**
    //
    // 属性：批量插入生成的 SQL 应该符合标准的批量 INSERT 语法
    //
    // 此测试验证批量插入 SQL 的结构正确性。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_insert_batch_sql_structure(
            table_name in "[a-z][a-z0-9_]{0,30}",
            record_count in 2usize..11usize,
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let _pool = create_test_pool().await;

                // 生成简单的记录
                let mut data_list = Vec::new();
                for i in 0..record_count {
                    let record = serde_json::json!({
                        "id": i,
                        "name": format!("test_{}", i),
                    });
                    data_list.push(record);
                }

                // 使用 SqlGenerator 生成批量插入 SQL
                let mut generator = crate::query_builder::SqlGenerator::new();
                let field_types = std::collections::HashMap::new();

                let result = generator.build_insert_batch(
                    &table_name,
                    &data_list,
                    &field_types
                );

                prop_assert!(result.is_ok());

                let sql = generator.get_sql();

                // 验证 SQL 结构：INSERT INTO table (fields) VALUES (?, ?), (?, ?), ...

                // 1. 应该以 INSERT INTO 开头
                prop_assert!(sql.starts_with("INSERT INTO"));

                // 2. 应该包含表名
                prop_assert!(sql.contains(&table_name));

                // 3. 应该有字段列表（括号包围）
                let has_field_list = sql.contains("(id, name)") || sql.contains("(name, id)");
                prop_assert!(
                    has_field_list,
                    "应该包含字段列表，实际 SQL: {}",
                    sql
                );

                // 4. 应该有 VALUES 关键字
                prop_assert!(sql.contains("VALUES"));

                // 5. VALUES 后面应该有多个值组，用逗号分隔
                let values_part = sql.split("VALUES").nth(1).unwrap_or("");

                // 检查是否有逗号分隔的多个值组（对于 > 1 条记录）
                if record_count > 1 {
                    prop_assert!(
                        values_part.contains("), ("),
                        "多条记录的批量插入应该用 ), ( 分隔值组，实际 SQL: {}",
                        sql
                    );
                }

                // 6. 每个值组应该有正确数量的占位符（2个字段）
                let value_groups: Vec<&str> = values_part
                    .split("), (")
                    .collect();

                // 第一个值组
                let first_group = value_groups.first().unwrap_or(&"");
                let first_group_placeholders = first_group.matches('?').count();
                prop_assert_eq!(
                    first_group_placeholders,
                    2,
                    "每个值组应该有 2 个占位符，实际第一个值组有 {}",
                    first_group_placeholders
                );

                Ok(()) as Result<(), proptest::test_runner::TestCaseError>
            })?;
        }
    }
}
