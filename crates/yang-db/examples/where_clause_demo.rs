/// WHERE 子句生成演示
///
/// 本示例演示 WHERE 子句的各种用法和生成的 SQL
use yang_db::condition::{Condition, SqlValue, condition_to_sql};

fn main() {
    println!("=== WHERE 子句生成演示 ===\n");

    // 1. 基本相等条件
    println!("1. 基本相等条件:");
    let mut params = Vec::new();
    let cond = Condition::Eq("name".to_string(), SqlValue::String("张三".to_string()));
    let sql = condition_to_sql(&cond, &mut params);
    println!("   SQL: WHERE {}", sql);
    println!("   参数数量: {}\n", params.len());

    // 2. 比较操作符
    println!("2. 比较操作符 (>, <, >=, <=, !=):");
    let mut params = Vec::new();
    let cond = Condition::And(vec![
        Condition::Gt("age".to_string(), SqlValue::Int(18)),
        Condition::Lte("age".to_string(), SqlValue::Int(65)),
    ]);
    let sql = condition_to_sql(&cond, &mut params);
    println!("   SQL: WHERE {}", sql);
    println!("   参数数量: {}\n", params.len());

    // 3. IN 操作符
    println!("3. IN 操作符:");
    let mut params = Vec::new();
    let cond = Condition::In(
        "status".to_string(),
        vec![SqlValue::Int(1), SqlValue::Int(2), SqlValue::Int(3)],
    );
    let sql = condition_to_sql(&cond, &mut params);
    println!("   SQL: WHERE {}", sql);
    println!("   参数数量: {}\n", params.len());

    // 4. BETWEEN 操作符
    println!("4. BETWEEN 操作符:");
    let mut params = Vec::new();
    let cond = Condition::Between(
        "price".to_string(),
        SqlValue::Float(100.0),
        SqlValue::Float(500.0),
    );
    let sql = condition_to_sql(&cond, &mut params);
    println!("   SQL: WHERE {}", sql);
    println!("   参数数量: {}\n", params.len());

    // 5. LIKE 操作符
    println!("5. LIKE 操作符:");
    let mut params = Vec::new();
    let cond = Condition::Like("email".to_string(), "%@example.com".to_string());
    let sql = condition_to_sql(&cond, &mut params);
    println!("   SQL: WHERE {}", sql);
    println!("   参数数量: {}\n", params.len());

    // 6. 多个 AND 条件
    println!("6. 多个 AND 条件:");
    let mut params = Vec::new();
    let cond = Condition::And(vec![
        Condition::Eq("status".to_string(), SqlValue::Int(1)),
        Condition::Gt("age".to_string(), SqlValue::Int(18)),
        Condition::Like("name".to_string(), "张%".to_string()),
    ]);
    let sql = condition_to_sql(&cond, &mut params);
    println!("   SQL: WHERE {}", sql);
    println!("   参数数量: {}\n", params.len());

    // 7. OR 条件
    println!("7. OR 条件:");
    let mut params = Vec::new();
    let cond = Condition::Or(vec![
        Condition::Eq("role".to_string(), SqlValue::String("admin".to_string())),
        Condition::Eq("role".to_string(), SqlValue::String("manager".to_string())),
    ]);
    let sql = condition_to_sql(&cond, &mut params);
    println!("   SQL: WHERE {}", sql);
    println!("   参数数量: {}\n", params.len());

    // 8. 复杂的 AND/OR 组合（验证优先级）
    println!("8. 复杂的 AND/OR 组合:");
    let mut params = Vec::new();
    // (name = '张三' OR name = '李四') AND age > 18
    let cond = Condition::And(vec![
        Condition::Or(vec![
            Condition::Eq("name".to_string(), SqlValue::String("张三".to_string())),
            Condition::Eq("name".to_string(), SqlValue::String("李四".to_string())),
        ]),
        Condition::Gt("age".to_string(), SqlValue::Int(18)),
    ]);
    let sql = condition_to_sql(&cond, &mut params);
    println!("   SQL: WHERE {}", sql);
    println!("   参数数量: {}", params.len());
    println!("   注意: OR 条件被括号包围，确保优先级正确\n");

    // 9. 嵌套的复杂条件
    println!("9. 嵌套的复杂条件:");
    let mut params = Vec::new();
    // ((status = 1 OR status = 2) AND age > 18) OR (role = 'admin')
    let cond = Condition::Or(vec![
        Condition::And(vec![
            Condition::Or(vec![
                Condition::Eq("status".to_string(), SqlValue::Int(1)),
                Condition::Eq("status".to_string(), SqlValue::Int(2)),
            ]),
            Condition::Gt("age".to_string(), SqlValue::Int(18)),
        ]),
        Condition::Eq("role".to_string(), SqlValue::String("admin".to_string())),
    ]);
    let sql = condition_to_sql(&cond, &mut params);
    println!("   SQL: WHERE {}", sql);
    println!("   参数数量: {}", params.len());
    println!("   注意: 多层嵌套的括号确保逻辑正确\n");

    // 10. 参数绑定防止 SQL 注入
    println!("10. 参数绑定防止 SQL 注入:");
    let mut params = Vec::new();
    let malicious_input = "'; DROP TABLE users; --";
    let cond = Condition::Eq(
        "username".to_string(),
        SqlValue::String(malicious_input.to_string()),
    );
    let sql = condition_to_sql(&cond, &mut params);
    println!("   恶意输入: {}", malicious_input);
    println!("   生成的 SQL: WHERE {}", sql);
    println!("   参数数量: {}", params.len());
    println!("   说明: 恶意输入被安全地绑定为参数，不会直接拼接到 SQL 中\n");

    println!("=== 演示完成 ===");
}
