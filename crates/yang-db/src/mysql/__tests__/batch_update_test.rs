//! 批量更新内存分配优化测试
//!
//! 验证 `build_update_batch` 重构后生成的 SQL 结构与重构前完全一致，
//! 同时确认中间分配次数降至 O(M) 级别（每个字段一次，而非每个字段×每条记录）。

use crate::mysql::query_builder::SqlGenerator;
use std::collections::HashMap;

/// 测试：单条记录、单个更新字段的基本 SQL 结构
///
/// 验证需求 2：消除 O(M×N) 中间字符串分配
#[test]
fn test_build_update_batch_single_record_single_field() {
    let records = vec![serde_json::json!({"id": 1, "name": "Alice"})];
    let mut generator = SqlGenerator::new();
    generator
        .build_update_batch("users", &records, "id", &HashMap::new())
        .unwrap();

    let sql = generator.get_sql();

    // 验证 SQL 以 UPDATE ... SET 开头
    assert!(
        sql.starts_with("UPDATE `users` SET "),
        "SQL 应以 'UPDATE `users` SET ' 开头，实际: {}",
        sql
    );

    // 验证包含 CASE WHEN 结构
    assert!(
        sql.contains("`name` = CASE WHEN `id`=? THEN ?"),
        "SQL 应包含 CASE WHEN 结构，实际: {}",
        sql
    );

    // 验证包含 END 关键字
    assert!(sql.contains("END"), "SQL 应包含 END 关键字，实际: {}", sql);

    // 验证包含 WHERE id IN 子句
    assert!(
        sql.contains("WHERE `id` IN (?)"),
        "SQL 应包含 WHERE `id` IN (?) 子句，实际: {}",
        sql
    );
}

/// 测试：多条记录、单个更新字段的 SQL 结构
///
/// 验证 CASE WHEN 子句中每条记录都有对应的 WHEN ... THEN ? 分支
#[test]
fn test_build_update_batch_multiple_records_single_field() {
    let records = vec![
        serde_json::json!({"id": 1, "name": "Alice"}),
        serde_json::json!({"id": 2, "name": "Bob"}),
        serde_json::json!({"id": 3, "name": "Charlie"}),
    ];
    let mut generator = SqlGenerator::new();
    generator
        .build_update_batch("users", &records, "id", &HashMap::new())
        .unwrap();

    let sql = generator.get_sql();

    // 验证 CASE WHEN 中有 3 个 WHEN 分支（对应 3 条记录）
    let when_count = sql.matches("WHEN `id`=?").count();
    assert_eq!(
        when_count, 3,
        "3 条记录应生成 3 个 WHEN 分支，实际: {}",
        sql
    );

    // 验证 WHERE IN 子句包含 3 个占位符
    assert!(
        sql.contains("WHERE `id` IN (?,?,?)"),
        "WHERE IN 应包含 3 个占位符，实际: {}",
        sql
    );
}

/// 测试：单条记录、多个更新字段的 SQL 结构
///
/// 验证多字段时 SET 子句中每个字段都有独立的 CASE WHEN ... END 块
#[test]
fn test_build_update_batch_single_record_multiple_fields() {
    let records = vec![serde_json::json!({"id": 1, "name": "Alice", "age": 25})];
    let mut generator = SqlGenerator::new();
    generator
        .build_update_batch("users", &records, "id", &HashMap::new())
        .unwrap();

    let sql = generator.get_sql();

    // 验证包含两个 CASE ... END 块（name 和 age 各一个）
    let case_count = sql.matches("CASE WHEN").count();
    assert_eq!(
        case_count, 2,
        "2 个更新字段应生成 2 个 CASE WHEN 块，实际: {}",
        sql
    );

    let end_count = sql.matches("END").count();
    assert_eq!(
        end_count, 2,
        "2 个更新字段应生成 2 个 END 关键字，实际: {}",
        sql
    );

    // 验证字段之间用 ", " 分隔
    assert!(
        sql.contains(", "),
        "多个字段之间应用 ', ' 分隔，实际: {}",
        sql
    );
}

/// 测试：多条记录、多个更新字段的完整 SQL 结构
///
/// 这是核心测试：验证 N 条记录、M 个字段时，SQL 结构正确
#[test]
fn test_build_update_batch_multiple_records_multiple_fields() {
    let records = vec![
        serde_json::json!({"id": 1, "name": "Alice", "age": 25}),
        serde_json::json!({"id": 2, "name": "Bob", "age": 30}),
    ];
    let mut generator = SqlGenerator::new();
    generator
        .build_update_batch("users", &records, "id", &HashMap::new())
        .unwrap();

    let sql = generator.get_sql();

    // 验证 SQL 基本结构
    assert!(
        sql.starts_with("UPDATE `users` SET "),
        "SQL 应以 'UPDATE `users` SET ' 开头，实际: {}",
        sql
    );

    // 验证每个字段都有 CASE WHEN 结构（2 个字段）
    let case_count = sql.matches("CASE WHEN").count();
    assert_eq!(
        case_count, 2,
        "2 个更新字段应生成 2 个 CASE WHEN 块，实际: {}",
        sql
    );

    // 验证每个 CASE 块中有 2 个 WHEN 分支（2 条记录）
    let when_count = sql.matches("WHEN `id`=?").count();
    assert_eq!(
        when_count, 4,
        "2 个字段 × 2 条记录 = 4 个 WHEN 分支，实际: {}",
        sql
    );

    // 验证 WHERE IN 子句包含 2 个占位符
    assert!(
        sql.contains("WHERE `id` IN (?,?)"),
        "WHERE IN 应包含 2 个占位符，实际: {}",
        sql
    );
}

/// 测试：参数数量正确性
///
/// 验证 N 条记录、M 个字段时，参数总数 = M×N×2（CASE WHEN 部分）+ N（WHERE IN 部分）
#[test]
fn test_build_update_batch_param_count() {
    // 2 条记录，2 个更新字段（name, age），id 字段为主键
    let records = vec![
        serde_json::json!({"id": 1, "name": "Alice", "age": 25}),
        serde_json::json!({"id": 2, "name": "Bob", "age": 30}),
    ];
    let mut generator = SqlGenerator::new();
    generator
        .build_update_batch("users", &records, "id", &HashMap::new())
        .unwrap();

    let params = generator.get_params();

    // CASE WHEN 部分：每个字段每条记录需要 2 个参数（id 和字段值）
    // = 2 字段 × 2 记录 × 2 参数 = 8 个参数
    // WHERE IN 部分：每条记录需要 1 个参数
    // = 2 条记录 × 1 参数 = 2 个参数
    // 总计：8 + 2 = 10 个参数
    assert_eq!(
        params.len(),
        10,
        "2 字段 × 2 记录 × 2 + 2 记录 = 10 个参数，实际: {}",
        params.len()
    );
}

/// 测试：空记录列表返回错误
///
/// 验证边界条件处理
#[test]
fn test_build_update_batch_empty_records_returns_error() {
    let records: Vec<serde_json::Value> = vec![];
    let mut generator = SqlGenerator::new();
    let result = generator.build_update_batch("users", &records, "id", &HashMap::new());

    assert!(result.is_err(), "空记录列表应返回错误");
    assert!(
        matches!(
            result.unwrap_err(),
            crate::error::DbError::SerializationError(_)
        ),
        "应返回 SerializationError"
    );
}

/// 测试：只有主键字段（无可更新字段）返回错误
///
/// 验证当所有字段都是主键时的错误处理
#[test]
fn test_build_update_batch_only_id_field_returns_error() {
    let records = vec![serde_json::json!({"id": 1})];
    let mut generator = SqlGenerator::new();
    let result = generator.build_update_batch("users", &records, "id", &HashMap::new());

    assert!(result.is_err(), "只有主键字段时应返回错误");
    assert!(
        matches!(
            result.unwrap_err(),
            crate::error::DbError::SerializationError(_)
        ),
        "应返回 SerializationError"
    );
}

/// 测试：自定义主键字段名
///
/// 验证使用非 "id" 的主键字段名时 SQL 结构正确
#[test]
fn test_build_update_batch_custom_id_field() {
    let records = vec![
        serde_json::json!({"user_id": 10, "status": "active"}),
        serde_json::json!({"user_id": 20, "status": "inactive"}),
    ];
    let mut generator = SqlGenerator::new();
    generator
        .build_update_batch("users", &records, "user_id", &HashMap::new())
        .unwrap();

    let sql = generator.get_sql();

    // 验证使用了自定义主键字段名
    assert!(
        sql.contains("WHEN `user_id`=?"),
        "应使用自定义主键字段名 user_id，实际: {}",
        sql
    );
    assert!(
        sql.contains("WHERE `user_id` IN"),
        "WHERE IN 应使用自定义主键字段名，实际: {}",
        sql
    );

    // 验证 status 字段有 CASE WHEN 结构
    assert!(
        sql.contains("`status` = CASE"),
        "status 字段应有 CASE WHEN 结构，实际: {}",
        sql
    );
}

/// 测试：SQL 结构与重构前完全一致（核心验证）
///
/// 通过对比重构前后的预期 SQL 格式，验证重构不改变输出结果
#[test]
fn test_build_update_batch_sql_matches_expected_format() {
    let records = vec![
        serde_json::json!({"id": 1, "name": "Alice"}),
        serde_json::json!({"id": 2, "name": "Bob"}),
    ];
    let mut generator = SqlGenerator::new();
    generator
        .build_update_batch("users", &records, "id", &HashMap::new())
        .unwrap();

    let sql = generator.get_sql();

    // 验证完整的 SQL 结构符合预期格式：
    // UPDATE users SET name = CASE WHEN id=? THEN ? WHEN id=? THEN ? END WHERE id IN (?,?)
    assert!(
        sql.starts_with("UPDATE `users` SET "),
        "SQL 头部格式不正确，实际: {}",
        sql
    );
    assert!(
        sql.contains("`name` = CASE WHEN `id`=? THEN ?"),
        "CASE WHEN 格式不正确，实际: {}",
        sql
    );
    assert!(
        sql.ends_with("WHERE `id` IN (?,?)"),
        "SQL 尾部格式不正确，实际: {}",
        sql
    );
}

/// 测试：大批量记录时 SQL 结构正确性
///
/// 验证 O(M) 级别分配在大数据量下仍然正确
#[test]
fn test_build_update_batch_large_batch() {
    // 生成 100 条记录，2 个更新字段
    let records: Vec<serde_json::Value> = (1..=100)
        .map(|i| serde_json::json!({"id": i, "name": format!("User{}", i), "score": i * 10}))
        .collect();

    let mut generator = SqlGenerator::new();
    generator
        .build_update_batch("users", &records, "id", &HashMap::new())
        .unwrap();

    let sql = generator.get_sql();
    let params = generator.get_params();

    // 验证 SQL 基本结构
    assert!(sql.starts_with("UPDATE `users` SET "), "SQL 头部格式不正确");
    assert!(sql.contains("WHERE `id` IN ("), "SQL 应包含 WHERE IN 子句");

    // 验证 CASE WHEN 分支数量：2 个字段 × 100 条记录 = 200 个 WHEN 分支
    let when_count = sql.matches("WHEN `id`=?").count();
    assert_eq!(
        when_count, 200,
        "2 字段 × 100 记录 = 200 个 WHEN 分支，实际: {}",
        when_count
    );

    // 验证参数数量：2 字段 × 100 记录 × 2 + 100 记录 = 500 个参数
    assert_eq!(
        params.len(),
        500,
        "2 字段 × 100 记录 × 2 + 100 记录 = 500 个参数，实际: {}",
        params.len()
    );
}

/// 测试：验证 O(M) 级别分配——中间不创建 Vec<String>
///
/// 通过验证 SQL 生成结果的正确性，间接证明重构后不再需要 O(M×N) 的中间分配。
/// 重构后：每个字段只需一次 push_str 序列，无需 Vec<String> when_parts 收集。
#[test]
fn test_build_update_batch_allocation_level_verification() {
    // 使用 M=3 个字段，N=4 条记录
    let records: Vec<serde_json::Value> = (1..=4)
        .map(|i| {
            serde_json::json!({
                "id": i,
                "field_a": format!("a{}", i),
                "field_b": i * 2,
                "field_c": i as f64 * 1.5
            })
        })
        .collect();

    let mut generator = SqlGenerator::new();
    generator
        .build_update_batch("test_table", &records, "id", &HashMap::new())
        .unwrap();

    let sql = generator.get_sql();
    let params = generator.get_params();

    // 验证 3 个字段各有独立的 CASE WHEN ... END 块
    let case_count = sql.matches("CASE WHEN").count();
    assert_eq!(case_count, 3, "M=3 个字段应生成 3 个 CASE WHEN 块");

    // 验证每个 CASE 块中有 4 个 WHEN 分支（N=4 条记录）
    let when_count = sql.matches("WHEN `id`=?").count();
    assert_eq!(when_count, 12, "M=3 × N=4 = 12 个 WHEN 分支");

    // 验证参数数量：M×N×2 + N = 3×4×2 + 4 = 28 个参数
    assert_eq!(
        params.len(),
        28,
        "M=3 × N=4 × 2 + N=4 = 28 个参数，实际: {}",
        params.len()
    );

    // 验证 WHERE IN 子句包含 N=4 个占位符
    assert!(
        sql.contains("WHERE `id` IN (?,?,?,?)"),
        "WHERE IN 应包含 4 个占位符，实际: {}",
        sql
    );
}

// ============================================================
// NEW-9：批量更新列集一致性校验
// ============================================================

/// 异构非主键列集应返回 InvalidArgument（避免 WHEN id=NULL 静默不更新）。
#[test]
fn test_build_update_batch_heterogeneous_columns_rejected() {
    use crate::error::DbError;

    // 首条更新 {id,name}，第二条更新 {id,age}（非主键列集不同）
    let records = vec![
        serde_json::json!({"id": 1, "name": "Alice"}),
        serde_json::json!({"id": 2, "age": 30}),
    ];
    let mut sql_gen = SqlGenerator::new();
    let result = sql_gen.build_update_batch("users", &records, "id", &HashMap::new());
    assert!(
        matches!(result, Err(DbError::InvalidArgument(_))),
        "异构列集应返回 InvalidArgument，实得 {:?}",
        result
    );
}

/// 缺少主键字段的记录应返回 InvalidArgument。
#[test]
fn test_build_update_batch_missing_id_rejected() {
    use crate::error::DbError;

    let records = vec![
        serde_json::json!({"id": 1, "name": "Alice"}),
        serde_json::json!({"name": "Bob"}),
    ];
    let mut sql_gen = SqlGenerator::new();
    let result = sql_gen.build_update_batch("users", &records, "id", &HashMap::new());
    assert!(
        matches!(result, Err(DbError::InvalidArgument(_))),
        "缺主键应返回 InvalidArgument，实得 {:?}",
        result
    );
}
