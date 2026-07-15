//! 批量配置表字段示例
//!
//! 展示如何使用 fields() 方法批量配置表字段，避免重复调用 field() 方法。

use serde_json::json;
use yang_base::table::{FieldConfig, FieldType, SortOrder, TableConfig, Validator};

fn main() {
    // 方法1：传统方式 - 逐个添加字段（不推荐）
    let _table_old_way = TableConfig::new("users")
        .display_name("用户表")
        .primary_key("id")
        .field(FieldConfig::new("id", FieldType::BigInt).required(true))
        .expect("有效字段配置应注册成功")
        .field(
            FieldConfig::new("username", FieldType::String { max_length: 50 })
                .display_name("用户名")
                .required(true),
        )
        .expect("有效字段配置应注册成功")
        .field(
            FieldConfig::new("email", FieldType::String { max_length: 100 })
                .display_name("邮箱")
                .required(true),
        )
        .expect("有效字段配置应注册成功");

    // 方法2：批量添加字段（推荐）
    let table_new_way = TableConfig::new("users")
        .display_name("用户表")
        .primary_key("id")
        .fields(vec![
            FieldConfig::new("id", FieldType::BigInt)
                .display_name("ID")
                .required(true),
            FieldConfig::new("username", FieldType::String { max_length: 50 })
                .display_name("用户名")
                .required(true)
                .validator(Validator::MinLength(3))
                .validator(Validator::MaxLength(50)),
            FieldConfig::new("email", FieldType::String { max_length: 100 })
                .display_name("邮箱")
                .required(true)
                .validator(Validator::Email),
            FieldConfig::new("age", FieldType::Integer)
                .display_name("年龄")
                .validator(Validator::Min(18.0))
                .validator(Validator::Max(100.0)),
            FieldConfig::new(
                "status",
                FieldType::Enum {
                    values: vec!["active".to_string(), "inactive".to_string()],
                },
            )
            .display_name("状态")
            .default_value(json!("active")),
        ])
        .expect("有效字段配置应注册成功")
        .unique_index(vec!["username".to_string()])
        .unique_index(vec!["email".to_string()])
        .default_order(vec![("created_at".to_string(), SortOrder::Desc)])
        .timestamps(true, true, true);

    println!("表名: {}", table_new_way.table_name);
    println!("显示名称: {}", table_new_way.display_name);
    println!("字段数量: {}", table_new_way.fields.len());
    println!("唯一索引数量: {}", table_new_way.unique_indexes.len());

    // 方法3：从迭代器添加字段
    let field_configs = vec![
        FieldConfig::new("id", FieldType::BigInt).required(true),
        FieldConfig::new("name", FieldType::String { max_length: 50 }).required(true),
        FieldConfig::new("description", FieldType::Text),
    ];

    let table_from_iter = TableConfig::new("products")
        .display_name("产品表")
        .fields_from_iter(field_configs)
        .expect("有效字段配置应注册成功");

    println!("\n产品表字段数量: {}", table_from_iter.fields.len());

    // 方法4：混合使用 - 先批量添加基本字段，再单独添加特殊字段
    let table_mixed = TableConfig::new("orders")
        .display_name("订单表")
        .fields(vec![
            FieldConfig::new("id", FieldType::BigInt).required(true),
            FieldConfig::new("order_no", FieldType::String { max_length: 50 }).required(true),
            FieldConfig::new("user_id", FieldType::BigInt).required(true),
            FieldConfig::new("amount", FieldType::Float)
                .required(true)
                .validator(Validator::Min(0.0))
                .validator(Validator::Max(999999.0)),
        ])
        .expect("有效字段配置应注册成功")
        // 单独添加复杂的 JSON 字段
        .field(
            FieldConfig::new("metadata", FieldType::Json)
                .display_name("元数据")
                .required(false),
        )
        .expect("有效字段配置应注册成功")
        .timestamps(true, true, false);

    println!("\n订单表字段数量: {}", table_mixed.fields.len());

    // 验证字段
    match table_new_way.validate_field("username") {
        Ok(_) => println!("\n✅ 字段 'username' 存在"),
        Err(e) => println!("\n❌ 错误: {}", e),
    }

    match table_new_way.validate_field("nonexistent") {
        Ok(_) => println!("✅ 字段 'nonexistent' 存在"),
        Err(e) => println!("❌ 字段 'nonexistent' 不存在: {}", e),
    }
}
