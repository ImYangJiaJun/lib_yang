//! TableConfig 单元测试

use crate::error::BaseError;
use crate::table::{FieldConfig, FieldType, SortOrder, TableConfig};

#[test]
fn test_table_config_new() {
    let table = TableConfig::new("users");
    assert_eq!(table.table_name, "users");
    assert_eq!(table.display_name, "");
    assert_eq!(table.primary_key, "id");
    assert_eq!(table.fields.len(), 0);
    assert_eq!(table.unique_indexes.len(), 0);
    assert_eq!(table.indexes.len(), 0);
    assert_eq!(table.default_order.len(), 0);
    assert_eq!(table.soft_delete_field, None);
    assert!(table.timestamp_fields.is_none());
}

#[test]
fn test_table_config_display_name() {
    let table = TableConfig::new("users").display_name("用户表");
    assert_eq!(table.display_name, "用户表");
}

#[test]
fn test_table_config_primary_key() {
    let table = TableConfig::new("users").primary_key("user_id");
    assert_eq!(table.primary_key, "user_id");
}

#[test]
fn test_table_config_add_field() {
    let table = TableConfig::new("users")
        .field(FieldConfig::new("id", FieldType::BigInt))
        .expect("有效字段配置应注册成功")
        .field(FieldConfig::new(
            "username",
            FieldType::String { max_length: 50 },
        ))
        .expect("有效字段配置应注册成功")
        .field(FieldConfig::new(
            "email",
            FieldType::String { max_length: 100 },
        ))
        .expect("有效字段配置应注册成功");

    assert_eq!(table.fields.len(), 3);
    assert!(table.fields.contains_key("id"));
    assert!(table.fields.contains_key("username"));
    assert!(table.fields.contains_key("email"));
}

#[test]
fn test_table_config_field_rejects_blank_name() {
    let err = TableConfig::new("users")
        .field(FieldConfig::new("   ", FieldType::BigInt))
        .expect_err("空白字段名不应被注册进表配置");

    assert!(matches!(
        err,
        BaseError::ConfigError(message) if message.contains("字段名称不能为空")
    ));
}

#[test]
fn test_table_config_unique_index() {
    let table = TableConfig::new("users")
        .unique_index(vec!["username".to_string()])
        .unique_index(vec!["email".to_string()]);

    assert_eq!(table.unique_indexes.len(), 2);
    assert_eq!(table.unique_indexes[0].fields, vec!["username"]);
    assert_eq!(table.unique_indexes[1].fields, vec!["email"]);
}

#[test]
fn test_table_config_index() {
    let table = TableConfig::new("users")
        .index(vec!["created_at".to_string()])
        .index(vec!["status".to_string(), "created_at".to_string()]);

    assert_eq!(table.indexes.len(), 2);
    assert_eq!(table.indexes[0].fields, vec!["created_at"]);
    assert_eq!(table.indexes[1].fields, vec!["status", "created_at"]);
}

#[test]
fn test_table_config_default_order() {
    let table = TableConfig::new("users").default_order(vec![
        ("created_at".to_string(), SortOrder::Desc),
        ("id".to_string(), SortOrder::Asc),
    ]);

    assert_eq!(table.default_order.len(), 2);
    assert_eq!(table.default_order[0].0, "created_at");
    assert_eq!(table.default_order[0].1, SortOrder::Desc);
    assert_eq!(table.default_order[1].0, "id");
    assert_eq!(table.default_order[1].1, SortOrder::Asc);
}

#[test]
fn test_table_config_soft_delete_field() {
    let table = TableConfig::new("users").soft_delete_field("deleted_at");
    assert_eq!(table.soft_delete_field, Some("deleted_at".to_string()));
}

#[test]
fn test_table_config_timestamps() {
    // 启用所有时间戳字段
    let table = TableConfig::new("users").timestamps(true, true, true);
    assert!(table.timestamp_fields.is_some());
    let timestamps = table.timestamp_fields.unwrap();
    assert_eq!(timestamps.created_at, Some("created_at".to_string()));
    assert_eq!(timestamps.updated_at, Some("updated_at".to_string()));
    assert_eq!(timestamps.deleted_at, Some("deleted_at".to_string()));

    // 只启用部分时间戳字段
    let table = TableConfig::new("users").timestamps(true, true, false);
    assert!(table.timestamp_fields.is_some());
    let timestamps = table.timestamp_fields.unwrap();
    assert_eq!(timestamps.created_at, Some("created_at".to_string()));
    assert_eq!(timestamps.updated_at, Some("updated_at".to_string()));
    assert_eq!(timestamps.deleted_at, None);

    // 不启用时间戳字段
    let table = TableConfig::new("users").timestamps(false, false, false);
    assert!(table.timestamp_fields.is_some());
    let timestamps = table.timestamp_fields.unwrap();
    assert_eq!(timestamps.created_at, None);
    assert_eq!(timestamps.updated_at, None);
    assert_eq!(timestamps.deleted_at, None);
}

#[test]
fn test_table_config_validate_field_success() {
    let table = TableConfig::new("users")
        .field(FieldConfig::new(
            "username",
            FieldType::String { max_length: 50 },
        ))
        .expect("有效字段配置应注册成功")
        .field(FieldConfig::new(
            "email",
            FieldType::String { max_length: 100 },
        ))
        .expect("有效字段配置应注册成功");

    assert!(table.validate_field("username").is_ok());
    assert!(table.validate_field("email").is_ok());
}

#[test]
fn test_table_config_validate_field_not_found() {
    let table = TableConfig::new("users")
        .field(FieldConfig::new(
            "username",
            FieldType::String { max_length: 50 },
        ))
        .expect("有效字段配置应注册成功");

    let result = table.validate_field("nonexistent");
    assert!(result.is_err());

    if let Err(BaseError::FieldNotFound(table_name, field_name)) = result {
        assert_eq!(table_name, "users");
        assert_eq!(field_name, "nonexistent");
    } else {
        panic!("期望 FieldNotFound 错误");
    }
}

#[test]
fn test_table_config_get_field_success() {
    let table = TableConfig::new("users")
        .field(FieldConfig::new(
            "username",
            FieldType::String { max_length: 50 },
        ))
        .expect("有效字段配置应注册成功");

    let field = table.get_field("username");
    assert!(field.is_some());
    assert_eq!(field.unwrap().name, "username");
}

#[test]
fn test_table_config_get_field_not_found() {
    let table = TableConfig::new("users")
        .field(FieldConfig::new(
            "username",
            FieldType::String { max_length: 50 },
        ))
        .expect("有效字段配置应注册成功");

    let field = table.get_field("nonexistent");
    assert!(field.is_none());
}

#[test]
fn test_table_config_validate_query_success() {
    let table = TableConfig::new("users")
        .field(FieldConfig::new(
            "username",
            FieldType::String { max_length: 50 },
        ))
        .expect("有效字段配置应注册成功")
        .field(FieldConfig::new(
            "email",
            FieldType::String { max_length: 100 },
        ))
        .expect("有效字段配置应注册成功")
        .field(FieldConfig::new("age", FieldType::Integer))
        .expect("有效字段配置应注册成功");

    assert!(table.validate_query(&["username", "email"]).is_ok());
    assert!(table.validate_query(&["username", "age"]).is_ok());
    assert!(table.validate_query(&["username", "email", "age"]).is_ok());
}

#[test]
fn test_table_config_validate_query_field_not_found() {
    let table = TableConfig::new("users")
        .field(FieldConfig::new(
            "username",
            FieldType::String { max_length: 50 },
        ))
        .expect("有效字段配置应注册成功")
        .field(FieldConfig::new(
            "email",
            FieldType::String { max_length: 100 },
        ))
        .expect("有效字段配置应注册成功");

    let result = table.validate_query(&["username", "nonexistent"]);
    assert!(result.is_err());

    if let Err(BaseError::FieldNotFound(table_name, field_name)) = result {
        assert_eq!(table_name, "users");
        assert_eq!(field_name, "nonexistent");
    } else {
        panic!("期望 FieldNotFound 错误");
    }
}

#[test]
fn test_table_config_complete_example() {
    // 创建一个完整的用户表配置
    let table = TableConfig::new("users")
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
        .expect("有效字段配置应注册成功")
        .field(
            FieldConfig::new(
                "status",
                FieldType::Enum {
                    values: vec!["active".to_string(), "inactive".to_string()],
                },
            )
            .display_name("状态")
            .required(true),
        )
        .expect("有效字段配置应注册成功")
        .unique_index(vec!["username".to_string()])
        .unique_index(vec!["email".to_string()])
        .index(vec!["status".to_string()])
        .index(vec!["created_at".to_string()])
        .default_order(vec![
            ("created_at".to_string(), SortOrder::Desc),
            ("id".to_string(), SortOrder::Asc),
        ])
        .soft_delete_field("deleted_at")
        .timestamps(true, true, true);

    // 验证表配置
    assert_eq!(table.table_name, "users");
    assert_eq!(table.display_name, "用户表");
    assert_eq!(table.primary_key, "id");
    assert_eq!(table.fields.len(), 4);
    assert_eq!(table.unique_indexes.len(), 2);
    assert_eq!(table.indexes.len(), 2);
    assert_eq!(table.default_order.len(), 2);
    assert_eq!(table.soft_delete_field, Some("deleted_at".to_string()));
    assert!(table.timestamp_fields.is_some());

    // 验证字段存在性
    assert!(table.validate_field("id").is_ok());
    assert!(table.validate_field("username").is_ok());
    assert!(table.validate_field("email").is_ok());
    assert!(table.validate_field("status").is_ok());

    // 验证查询参数
    assert!(table
        .validate_query(&["username", "email", "status"])
        .is_ok());
}

#[test]
fn test_table_config_chain_methods() {
    // 测试链式调用
    let table = TableConfig::new("products")
        .display_name("产品表")
        .primary_key("product_id")
        .field(FieldConfig::new("product_id", FieldType::BigInt))
        .expect("有效字段配置应注册成功")
        .field(FieldConfig::new(
            "name",
            FieldType::String { max_length: 100 },
        ))
        .expect("有效字段配置应注册成功")
        .field(FieldConfig::new("price", FieldType::Double))
        .expect("有效字段配置应注册成功")
        .unique_index(vec!["name".to_string()])
        .index(vec!["price".to_string()])
        .default_order(vec![("name".to_string(), SortOrder::Asc)])
        .timestamps(true, true, false);

    assert_eq!(table.table_name, "products");
    assert_eq!(table.display_name, "产品表");
    assert_eq!(table.primary_key, "product_id");
    assert_eq!(table.fields.len(), 3);
}

#[test]
fn test_sort_order_equality() {
    assert_eq!(SortOrder::Asc, SortOrder::Asc);
    assert_eq!(SortOrder::Desc, SortOrder::Desc);
    assert_ne!(SortOrder::Asc, SortOrder::Desc);
}
