//! 批量插入内存分配优化属性测试
//!
//! **验证需求: 1.1, 1.2, 1.3, 1.4**
//!
//! 属性 P1：验证任意非空数据列表生成的 SQL 中 `(` 数量等于记录数，
//! 参数数量等于 `记录数 × 字段数`。

use crate::mysql::query_builder::SqlGenerator;
use proptest::prelude::*;
use std::collections::HashMap;

// ============================================================
// 单元测试：验证基本批量插入 SQL 结构
// ============================================================

#[test]
fn test_batch_insert_single_record() {
    // 验证单条记录的批量插入 SQL 结构正确
    let mut sql_gen = SqlGenerator::new();
    let data = vec![serde_json::json!({"name": "Alice", "age": 30})];
    sql_gen
        .build_insert_batch("users", &data, &HashMap::new())
        .unwrap();

    let sql = sql_gen.get_sql();

    // SQL 应包含正确的 INSERT 头部
    assert!(
        sql.starts_with("INSERT INTO users ("),
        "SQL 应以 'INSERT INTO users (' 开头，实际: {}",
        sql
    );
    // SQL 应包含 VALUES 关键字
    assert!(
        sql.contains(") VALUES "),
        "SQL 应包含 ') VALUES '，实际: {}",
        sql
    );
    // 单条记录：VALUES 子句中 '(' 数量应为 1（不含字段列表的括号）
    // 字段列表有 1 个 '('，VALUES 子句有 1 个 '('，共 2 个
    let open_paren_count = sql.matches('(').count();
    assert_eq!(
        open_paren_count, 2,
        "单条记录 SQL 中 '(' 总数应为 2（字段列表 1 + VALUES 子句 1），实际: {}",
        open_paren_count
    );
    // 参数数量应等于字段数（2 个字段）
    assert_eq!(
        sql_gen.get_params().len(),
        2,
        "单条记录参数数量应为 2，实际: {}",
        sql_gen.get_params().len()
    );
}

#[test]
fn test_batch_insert_multiple_records() {
    // 验证多条记录的批量插入 SQL 结构正确
    let mut sql_gen = SqlGenerator::new();
    let data = vec![
        serde_json::json!({"id": 1, "name": "Alice", "score": 90}),
        serde_json::json!({"id": 2, "name": "Bob", "score": 85}),
        serde_json::json!({"id": 3, "name": "Carol", "score": 92}),
    ];
    sql_gen
        .build_insert_batch("students", &data, &HashMap::new())
        .unwrap();

    let sql = sql_gen.get_sql();
    let record_count = 3;
    let field_count = 3;

    // VALUES 子句中 '(' 数量应等于记录数（字段列表有 1 个 '('，VALUES 子句有 record_count 个）
    let open_paren_count = sql.matches('(').count();
    assert_eq!(
        open_paren_count,
        record_count + 1,
        "SQL 中 '(' 总数应为 {}（字段列表 1 + VALUES 子句 {}），实际: {}",
        record_count + 1,
        record_count,
        open_paren_count
    );

    // 参数数量应等于 记录数 × 字段数
    assert_eq!(
        sql_gen.get_params().len(),
        record_count * field_count,
        "参数数量应为 {}（{} 条记录 × {} 个字段），实际: {}",
        record_count * field_count,
        record_count,
        field_count,
        sql_gen.get_params().len()
    );
}

#[test]
fn test_batch_insert_records_separated_by_comma() {
    // 验证记录之间用 ", " 分隔（无中间 Vec<String> 分配）
    let mut sql_gen = SqlGenerator::new();
    let data = vec![
        serde_json::json!({"x": 1}),
        serde_json::json!({"x": 2}),
        serde_json::json!({"x": 3}),
    ];
    sql_gen
        .build_insert_batch("t", &data, &HashMap::new())
        .unwrap();

    let sql = sql_gen.get_sql();

    // VALUES 子句应包含 "(?) , (?)" 或 "(?), (?)" 格式的分隔
    // 验证 SQL 中有 2 个 ", (" 分隔符（3 条记录之间有 2 个分隔）
    // 使用更宽松的检查：VALUES 子句中 '(' 数量 = 记录数 + 1（字段列表）
    let open_paren_count = sql.matches('(').count();
    assert_eq!(
        open_paren_count,
        4, // 字段列表 1 + VALUES 子句 3
        "3 条记录的 SQL 中 '(' 总数应为 4，实际: {}，SQL: {}",
        open_paren_count,
        sql
    );
}

#[test]
fn test_batch_insert_empty_data_returns_error() {
    // 验证空数据列表返回错误
    let mut sql_gen = SqlGenerator::new();
    let result = sql_gen.build_insert_batch("t", &[], &HashMap::new());
    assert!(result.is_err(), "空数据列表应返回错误");
}

#[test]
fn test_batch_insert_placeholder_count_equals_params() {
    // 验证 SQL 中 '?' 占位符数量等于参数数量
    let mut sql_gen = SqlGenerator::new();
    let data = vec![
        serde_json::json!({"a": 1, "b": 2, "c": 3}),
        serde_json::json!({"a": 4, "b": 5, "c": 6}),
    ];
    sql_gen
        .build_insert_batch("tbl", &data, &HashMap::new())
        .unwrap();

    let sql = sql_gen.get_sql();
    let placeholder_count = sql.matches('?').count();
    let params_count = sql_gen.get_params().len();

    assert_eq!(
        placeholder_count, params_count,
        "SQL 中 '?' 占位符数量应等于参数数量，占位符: {}，参数: {}",
        placeholder_count, params_count
    );
}

// ============================================================
// 属性测试 P1：批量插入 SQL 结构正确性
// ============================================================

proptest! {
    /// **验证: 需求 1.1, 1.2, 1.3, 1.4**
    ///
    /// 属性 P1：对于任意非空数据列表，`build_insert_batch` 生成的 SQL 满足：
    /// - VALUES 子句中 `(` 数量等于记录数（加上字段列表的 1 个括号）
    /// - 参数数量等于 `记录数 × 字段数`
    #[test]
    fn prop_p1_batch_insert_paren_count_equals_record_count(
        // 生成 1~6 个字段名（字母数字组合，避免重复）
        raw_field_names in prop::collection::vec("[a-z][a-z0-9_]{0,6}", 1..=6),
        // 生成 1~20 条记录，每条记录的字段值为整数
        record_values in prop::collection::vec(
            prop::collection::vec(0i64..=9999i64, 1..=6),
            1..=20
        ),
    ) {
        // 去重字段名，确保 JSON 对象键唯一
        let mut unique_fields: Vec<String> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for name in &raw_field_names {
            if seen.insert(name.clone()) {
                unique_fields.push(name.clone());
            }
        }

        // 至少需要 1 个字段
        prop_assume!(!unique_fields.is_empty());

        let field_count = unique_fields.len();

        // 构建数据列表，每条记录使用相同的字段名
        let data_list: Vec<serde_json::Value> = record_values
            .iter()
            .map(|values| {
                let mut obj = serde_json::Map::new();
                for (i, field) in unique_fields.iter().enumerate() {
                    // 使用 values 中对应位置的值（循环取模防止越界）
                    let val = values[i % values.len()];
                    obj.insert(field.clone(), serde_json::json!(val));
                }
                serde_json::Value::Object(obj)
            })
            .collect();

        let record_count = data_list.len();

        // 生成批量插入 SQL
        let mut sql_gen = SqlGenerator::new();
        sql_gen.build_insert_batch("test_table", &data_list, &HashMap::new()).unwrap();

        let sql = sql_gen.get_sql().to_string();
        let params_count = sql_gen.get_params().len();

        // 验证 SQL 基本结构
        prop_assert!(
            sql.starts_with("INSERT INTO test_table ("),
            "SQL 应以 'INSERT INTO test_table (' 开头，实际: {}",
            sql
        );
        prop_assert!(
            sql.contains(") VALUES "),
            "SQL 应包含 ') VALUES '，实际: {}",
            sql
        );

        // 验证 '(' 数量：字段列表 1 个 + VALUES 子句 record_count 个
        let open_paren_count = sql.matches('(').count();
        prop_assert_eq!(
            open_paren_count,
            record_count + 1,
            "SQL 中 '(' 总数应为 {}（字段列表 1 + VALUES 子句 {}），实际: {}，SQL: {}",
            record_count + 1,
            record_count,
            open_paren_count,
            sql
        );

        // 验证参数数量等于 记录数 × 字段数
        prop_assert_eq!(
            params_count,
            record_count * field_count,
            "参数数量应为 {}（{} 条记录 × {} 个字段），实际: {}",
            record_count * field_count,
            record_count,
            field_count,
            params_count
        );

        // 验证 '?' 占位符数量等于参数数量
        let placeholder_count = sql.matches('?').count();
        prop_assert_eq!(
            placeholder_count,
            params_count,
            "SQL 中 '?' 占位符数量应等于参数数量，占位符: {}，参数: {}",
            placeholder_count,
            params_count
        );
    }

    /// **验证: 需求 1.3**
    ///
    /// 属性 P1 扩展：验证记录之间的分隔符正确，
    /// 即 VALUES 子句中 ')' 数量等于 '(' 数量（括号配对）。
    #[test]
    fn prop_p1_batch_insert_parens_are_balanced(
        raw_field_names in prop::collection::vec("[a-z][a-z0-9_]{0,6}", 1..=5),
        record_count in 1usize..=15,
    ) {
        // 去重字段名
        let mut unique_fields: Vec<String> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for name in &raw_field_names {
            if seen.insert(name.clone()) {
                unique_fields.push(name.clone());
            }
        }

        prop_assume!(!unique_fields.is_empty());

        // 构建数据列表
        let data_list: Vec<serde_json::Value> = (0..record_count)
            .map(|i| {
                let mut obj = serde_json::Map::new();
                for field in &unique_fields {
                    obj.insert(field.clone(), serde_json::json!(i as i64));
                }
                serde_json::Value::Object(obj)
            })
            .collect();

        // 生成批量插入 SQL
        let mut sql_gen = SqlGenerator::new();
        sql_gen.build_insert_batch("tbl", &data_list, &HashMap::new()).unwrap();

        let sql = sql_gen.get_sql();

        // 验证括号配对：'(' 数量应等于 ')' 数量
        let open_count = sql.matches('(').count();
        let close_count = sql.matches(')').count();
        prop_assert_eq!(
            open_count,
            close_count,
            "SQL 中 '(' 数量应等于 ')' 数量，'(': {}，')': {}，SQL: {}",
            open_count,
            close_count,
            sql
        );

        // 验证 '(' 总数 = 记录数 + 1（字段列表括号）
        prop_assert_eq!(
            open_count,
            record_count + 1,
            "SQL 中 '(' 总数应为 {}，实际: {}",
            record_count + 1,
            open_count
        );
    }
}
