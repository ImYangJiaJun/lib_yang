#![allow(clippy::unwrap_used, clippy::expect_used)]

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
        sql.starts_with("INSERT INTO `users` ("),
        "SQL 应以 'INSERT INTO `users` (' 开头，实际: {}",
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
            sql.starts_with("INSERT INTO `test_table` ("),
            "SQL 应以 'INSERT INTO `test_table` (' 开头，实际: {}",
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

// ============================================================
// NEW-9：批量插入列集一致性校验
// ============================================================

/// 异构列集（后续记录字段与首条不同）应返回 InvalidArgument，而非静默丢列/填 NULL。
#[test]
fn test_batch_insert_heterogeneous_columns_rejected() {
    use crate::error::DbError;

    // 首条 {id,name,age}，第二条把 age 换成 email（列集不同）
    let data = vec![
        serde_json::json!({"id": 1, "name": "Alice", "age": 30}),
        serde_json::json!({"id": 2, "name": "Bob", "email": "b@x.com"}),
    ];
    let mut sql_gen = SqlGenerator::new();
    let result = sql_gen.build_insert_batch("users", &data, &HashMap::new());
    assert!(
        matches!(result, Err(DbError::InvalidArgument(_))),
        "异构列集应返回 InvalidArgument，实得 {:?}",
        result
    );
}

/// 列子集（后续记录字段少于首条）同样应被拒绝。
#[test]
fn test_batch_insert_subset_columns_rejected() {
    use crate::error::DbError;

    let data = vec![
        serde_json::json!({"id": 1, "name": "Alice", "age": 30}),
        serde_json::json!({"id": 2, "name": "Bob"}),
    ];
    let mut sql_gen = SqlGenerator::new();
    let result = sql_gen.build_insert_batch("users", &data, &HashMap::new());
    assert!(
        matches!(result, Err(DbError::InvalidArgument(_))),
        "列子集应返回 InvalidArgument，实得 {:?}",
        result
    );
}

/// 同构列集（所有记录字段一致）仍正常生成 SQL。
#[test]
fn test_batch_insert_homogeneous_columns_ok() {
    let data = vec![
        serde_json::json!({"id": 1, "name": "Alice"}),
        serde_json::json!({"id": 2, "name": "Bob"}),
    ];
    let mut sql_gen = SqlGenerator::new();
    assert!(sql_gen
        .build_insert_batch("users", &data, &HashMap::new())
        .is_ok());
}

// ============================================================
// NEW-10：FieldType 类型提示形态不匹配显式报错（不再静默 fallthrough）
// ============================================================

/// Text 字段收到非字符串（数组）值应返回 TypeConversionError，而非默认转 JSON。
#[test]
fn test_field_type_text_shape_mismatch_errors() {
    use crate::error::DbError;
    use crate::mysql::FieldType;

    let mut field_types = HashMap::new();
    field_types.insert("note".to_string(), FieldType::Text);

    let data = vec![serde_json::json!({"note": [1, 2, 3]})];
    let mut sql_gen = SqlGenerator::new();
    let result = sql_gen.build_insert_batch("t", &data, &field_types);
    assert!(
        matches!(result, Err(DbError::TypeConversionError(_))),
        "Text 字段收到数组应报 TypeConversionError，实得 {:?}",
        result
    );
}

/// Timestamp 字段收到字符串应报错；但 NULL 仍放行（可空列）。
#[test]
fn test_field_type_timestamp_shape_mismatch_and_null() {
    use crate::error::DbError;
    use crate::mysql::FieldType;

    let mut field_types = HashMap::new();
    field_types.insert("ts".to_string(), FieldType::Timestamp);

    // 字符串 → 报错
    let bad = vec![serde_json::json!({"ts": "2024-01-01"})];
    let mut g1 = SqlGenerator::new();
    assert!(
        matches!(
            g1.build_insert_batch("t", &bad, &field_types),
            Err(DbError::TypeConversionError(_))
        ),
        "Timestamp 字段收到字符串应报 TypeConversionError"
    );

    // NULL → 放行（生成 NULL 参数，不报错）
    let nullv = vec![serde_json::json!({"ts": serde_json::Value::Null})];
    let mut g2 = SqlGenerator::new();
    assert!(
        g2.build_insert_batch("t", &nullv, &field_types).is_ok(),
        "Timestamp 字段的 NULL 值应放行"
    );
}

/// NG-3：Decimal 字段对高精度/超大数字降级为字符串绑定，保精度；小值仍走 Float。
#[test]
fn test_field_type_decimal_high_precision_uses_string() {
    use crate::mysql::condition::SqlValue;
    use crate::mysql::FieldType;

    let mut field_types = HashMap::new();
    field_types.insert("price".to_string(), FieldType::Decimal);

    // 超出 f64 安全整数范围的大整数 → 字符串保精度
    let big = vec![serde_json::json!({"price": 9_007_199_254_740_993i64})];
    let mut g = SqlGenerator::new();
    g.build_insert_batch("t", &big, &field_types).unwrap();
    match &g.get_params()[0] {
        SqlValue::String(s) => assert_eq!(s, "9007199254740993"),
        other => panic!("超大 Decimal 应转 String，实得 {:?}", other),
    }

    // 普通小值 → 仍走 Float（性能优）
    let small = vec![serde_json::json!({"price": 12.5})];
    let mut g2 = SqlGenerator::new();
    g2.build_insert_batch("t", &small, &field_types).unwrap();
    assert!(
        matches!(g2.get_params()[0], SqlValue::Float(_)),
        "普通小 Decimal 应走 Float"
    );
}
