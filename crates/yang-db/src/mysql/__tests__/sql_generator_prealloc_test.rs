//! SqlGenerator 预分配优化属性测试
//!
//! **验证需求: 6.1, 6.2, 6.3**
//!
//! 属性 P6：验证预分配初始化与默认初始化生成相同的 SQL 和参数列表，
//! 容量变化不影响内容正确性。

use crate::mysql::query_builder::SqlGenerator;
use proptest::prelude::*;
use std::collections::HashMap;

// ============================================================
// 单元测试：验证预分配容量初始值
// ============================================================

#[test]
fn test_new_sql_capacity_is_256() {
    // 验证需求 6.1：sql 字段预分配 256 字节
    let sql_gen = SqlGenerator::new();
    assert!(
        sql_gen.sql_capacity() >= 256,
        "sql 字段容量应 >= 256，实际为 {}",
        sql_gen.sql_capacity()
    );
}

#[test]
fn test_new_params_capacity_is_8() {
    // 验证需求 6.2：params 字段预分配 8 个槽位
    let sql_gen = SqlGenerator::new();
    assert!(
        sql_gen.params_capacity() >= 8,
        "params 字段容量应 >= 8，实际为 {}",
        sql_gen.params_capacity()
    );
}

#[test]
fn test_new_sql_content_is_empty() {
    // 验证新建的 SqlGenerator sql 内容为空
    let sql_gen = SqlGenerator::new();
    assert!(
        sql_gen.get_sql().is_empty(),
        "新建的 SqlGenerator sql 应为空"
    );
}

#[test]
fn test_new_params_content_is_empty() {
    // 验证新建的 SqlGenerator params 内容为空
    let sql_gen = SqlGenerator::new();
    assert!(
        sql_gen.get_params().is_empty(),
        "新建的 SqlGenerator params 应为空"
    );
}

#[test]
fn test_clear_preserves_capacity() {
    // 验证需求 6.3：clear() 方法保留已分配容量
    let mut sql_gen = SqlGenerator::new();

    // 先写入一些内容以触发可能的扩容
    let data = serde_json::json!({"name": "test_user", "age": 18, "email": "test@example.com"});
    sql_gen
        .build_insert("users", &data, &HashMap::new())
        .unwrap();

    let sql_cap_before = sql_gen.sql_capacity();
    let params_cap_before = sql_gen.params_capacity();

    // 调用 clear 后容量应保留（不小于清空前）
    sql_gen.clear_for_test();

    assert!(
        sql_gen.sql_capacity() >= sql_cap_before,
        "clear() 后 sql 容量不应减少，清空前 {}，清空后 {}",
        sql_cap_before,
        sql_gen.sql_capacity()
    );
    assert!(
        sql_gen.params_capacity() >= params_cap_before,
        "clear() 后 params 容量不应减少，清空前 {}，清空后 {}",
        params_cap_before,
        sql_gen.params_capacity()
    );

    // 内容应为空
    assert!(sql_gen.get_sql().is_empty(), "clear() 后 sql 内容应为空");
    assert!(
        sql_gen.get_params().is_empty(),
        "clear() 后 params 内容应为空"
    );
}

#[test]
fn test_clear_does_not_use_new_allocation() {
    // 验证需求 6.3：clear() 不使用 = String::new() 或 = Vec::new()
    // 通过检查 clear 后容量仍 >= 初始预分配值来间接验证
    let mut sql_gen = SqlGenerator::new();

    // 写入数据
    let data = serde_json::json!({"field1": "value1", "field2": 42});
    sql_gen.build_insert("tbl", &data, &HashMap::new()).unwrap();

    // clear 后容量应 >= 预分配值（256 和 8）
    sql_gen.clear_for_test();

    assert!(
        sql_gen.sql_capacity() >= 256,
        "clear() 后 sql 容量应 >= 256（预分配值），实际为 {}",
        sql_gen.sql_capacity()
    );
    assert!(
        sql_gen.params_capacity() >= 8,
        "clear() 后 params 容量应 >= 8（预分配值），实际为 {}",
        sql_gen.params_capacity()
    );
}

// ============================================================
// 属性测试 P6：预分配不影响 SQL 内容正确性
// ============================================================

proptest! {
    /// **验证: 需求 6.1, 6.2**
    ///
    /// 属性 P6：对于任意合法的 INSERT 数据，
    /// 使用预分配容量的 SqlGenerator 生成的 SQL 结构正确，
    /// 容量变化不影响内容正确性。
    #[test]
    fn prop_p6_prealloc_insert_sql_correctness(
        // 生成 1~8 个字段名（字母数字组合）
        raw_field_names in prop::collection::vec("[a-z][a-z0-9_]{0,8}", 1..=8),
        // 生成对应的整数值
        field_values in prop::collection::vec(0i64..=1000i64, 1..=8),
    ) {
        // 取较小长度
        let count = raw_field_names.len().min(field_values.len());

        // 去重字段名（JSON 对象不允许重复键）
        let mut unique_fields: Vec<String> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for name in &raw_field_names[..count] {
            if seen.insert(name.clone()) {
                unique_fields.push(name.clone());
            }
        }

        if unique_fields.is_empty() {
            return Ok(());
        }

        // 构建 JSON 数据
        let mut obj = serde_json::Map::new();
        for (i, name) in unique_fields.iter().enumerate() {
            obj.insert(name.clone(), serde_json::json!(field_values[i]));
        }
        let data = serde_json::Value::Object(obj);

        // 使用预分配的 SqlGenerator 生成 SQL
        let mut sql_gen = SqlGenerator::new();
        sql_gen.build_insert("test_table", &data, &HashMap::new()).unwrap();

        let sql = sql_gen.get_sql().to_string();
        let params_len = sql_gen.get_params().len();

        // 验证 SQL 结构正确性
        prop_assert!(
            sql.starts_with("INSERT INTO test_table ("),
            "SQL 应以 'INSERT INTO test_table (' 开头，实际: {}",
            sql
        );
        prop_assert!(
            sql.contains(") VALUES ("),
            "SQL 应包含 ') VALUES ('，实际: {}",
            sql
        );
        prop_assert!(
            sql.ends_with(')'),
            "SQL 应以 ')' 结尾，实际: {}",
            sql
        );

        // 验证参数数量等于字段数量
        prop_assert_eq!(
            params_len,
            unique_fields.len(),
            "参数数量应等于字段数量，字段数: {}，参数数: {}",
            unique_fields.len(),
            params_len
        );

        // 验证 SQL 中 '?' 占位符数量等于字段数量
        let placeholder_count = sql.matches('?').count();
        prop_assert_eq!(
            placeholder_count,
            unique_fields.len(),
            "占位符数量应等于字段数量，字段数: {}，占位符数: {}",
            unique_fields.len(),
            placeholder_count
        );
    }

    /// **验证: 需求 6.3**
    ///
    /// 属性 P6 扩展：clear() 后重新使用 SqlGenerator，
    /// 生成的 SQL 内容与首次使用完全一致（容量保留不影响正确性）。
    #[test]
    fn prop_p6_clear_and_reuse_produces_same_sql(
        raw_field_names in prop::collection::vec("[a-z][a-z0-9_]{0,8}", 1..=5),
        field_values in prop::collection::vec(0i64..=100i64, 1..=5),
    ) {
        let count = raw_field_names.len().min(field_values.len());

        // 去重字段名
        let mut unique_fields: Vec<String> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for name in &raw_field_names[..count] {
            if seen.insert(name.clone()) {
                unique_fields.push(name.clone());
            }
        }

        if unique_fields.is_empty() {
            return Ok(());
        }

        // 构建 JSON 数据
        let mut obj = serde_json::Map::new();
        for (i, name) in unique_fields.iter().enumerate() {
            obj.insert(name.clone(), serde_json::json!(field_values[i]));
        }
        let data = serde_json::Value::Object(obj);

        let mut sql_gen = SqlGenerator::new();

        // 第一次生成
        sql_gen.build_insert("tbl", &data, &HashMap::new()).unwrap();
        let sql_first = sql_gen.get_sql().to_string();
        let params_len_first = sql_gen.get_params().len();

        // clear 后第二次生成（验证容量保留不影响结果）
        sql_gen.clear_for_test();
        sql_gen.build_insert("tbl", &data, &HashMap::new()).unwrap();
        let sql_second = sql_gen.get_sql().to_string();
        let params_len_second = sql_gen.get_params().len();

        // 两次生成结果应完全相同
        prop_assert_eq!(
            sql_first, sql_second,
            "clear() 后重新生成的 SQL 应与首次相同"
        );
        prop_assert_eq!(
            params_len_first, params_len_second,
            "clear() 后重新生成的参数数量应与首次相同"
        );
    }
}
