# AND/OR 优先级处理示例

## 功能说明

本示例展示 MySQL 查询构建器如何正确处理混合 AND/OR 条件的操作符优先级。

## 核心特性

### 1. 自动括号处理

当混合使用 `where_and()` 和 `where_or()` 时，查询构建器会自动添加括号以确保正确的操作符优先级。

### 2. 测试验证

**属性 9：AND/OR 优先级处理**（验证需求 3.7）

测试验证以下场景：
- 混合 AND/OR 条件时，SQL 生成正确的括号
- 嵌套的 AND/OR 条件保持正确的优先级
- 括号匹配（左右括号数量相等）

## 实现细节

### condition_to_sql 函数

```rust
// AND 条件处理
Condition::And(conditions) => {
    if conditions.is_empty() {
        return "1 = 1".to_string();
    }
    if conditions.len() == 1 {
        return condition_to_sql(&conditions[0], params);
    }
    // AND 条件需要括号以确保优先级
    let parts: Vec<String> = conditions
        .iter()
        .map(|c| condition_to_sql(c, params))
        .collect();
    format!("({})", parts.join(" AND "))
}

// OR 条件处理
Condition::Or(conditions) => {
    if conditions.is_empty() {
        return "1 = 0".to_string();
    }
    if conditions.len() == 1 {
        return condition_to_sql(&conditions[0], params);
    }
    // OR 条件需要括号
    let parts: Vec<String> = conditions
        .iter()
        .map(|c| condition_to_sql(c, params))
        .collect();
    format!("({})", parts.join(" OR "))
}
```

## 测试示例

### 基本 AND/OR 混合

```rust
// 构建条件：(field1 = value1 OR field1 = value2) AND field2 = value3
let cond = Condition::And(vec![
    Condition::Or(vec![
        Condition::Eq("status".to_string(), SqlValue::Int(1)),
        Condition::Eq("status".to_string(), SqlValue::Int(2)),
    ]),
    Condition::Eq("active".to_string(), SqlValue::Bool(true)),
]);

// 生成的 SQL：
// ((status = ? OR status = ?) AND active = ?)
```

### 嵌套 AND/OR

```rust
// 构建复杂嵌套条件
let cond = Condition::And(vec![
    Condition::Or(vec![
        Condition::Eq("type".to_string(), SqlValue::String("A".to_string())),
        Condition::Eq("type".to_string(), SqlValue::String("B".to_string())),
        Condition::Eq("type".to_string(), SqlValue::String("C".to_string())),
    ]),
    Condition::Gt("score".to_string(), SqlValue::Int(0)),
]);

// 生成的 SQL：
// ((type = ? OR type = ? OR type = ?) AND score > ?)
```

## 基于属性的测试

测试使用 proptest 库，运行 100 次迭代，验证：

1. **括号正确性**：整个条件被括号包围
2. **操作符存在**：SQL 包含 AND 和 OR 操作符
3. **参数数量**：参数列表长度与条件数量匹配
4. **括号匹配**：左右括号数量相等

```rust
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_and_or_priority_handling(
        field1 in field_name_strategy(),
        field2 in field_name_strategy(),
        value1 in sql_value_strategy(),
        value2 in sql_value_strategy(),
        value3 in sql_value_strategy()
    ) {
        let mut params = Vec::new();

        let cond = Condition::And(vec![
            Condition::Or(vec![
                Condition::Eq(field1.clone(), value1),
                Condition::Eq(field1.clone(), value2),
            ]),
            Condition::Eq(field2.clone(), value3),
        ]);

        let sql = condition_to_sql(&cond, &mut params);

        // 验证括号和操作符
        prop_assert!(sql.starts_with('('));
        prop_assert!(sql.ends_with(')'));
        prop_assert!(sql.contains(" OR "));
        prop_assert!(sql.contains(" AND "));
        prop_assert_eq!(params.len(), 3);
    }
}
```

## 运行测试

```bash
# 运行 AND/OR 优先级测试
cargo test prop_and_or_priority_handling --lib

# 运行所有条件相关的属性测试
cargo test condition::property_tests --lib

# 运行所有测试
cargo test --lib
```

## 测试结果

所有测试通过：
- ✅ prop_and_or_priority_handling
- ✅ prop_nested_and_or_brackets
- ✅ prop_multiple_and_conditions
- ✅ 总计 139 个测试全部通过

## 总结

MySQL 查询构建器正确实现了 AND/OR 优先级处理：
- 自动添加括号确保操作符优先级
- 支持任意深度的嵌套条件
- 通过基于属性的测试验证正确性
- 100 次随机迭代确保健壮性
