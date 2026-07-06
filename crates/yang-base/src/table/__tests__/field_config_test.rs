//! FieldConfig 单元测试

use crate::error::BaseError;
use crate::table::{
    FieldConfig, FieldPermissions, FieldType, RelationConfig, RelationType, Validator,
};
use serde_json::json;
use std::collections::HashSet;

#[test]
fn test_field_config_new() {
    let field = FieldConfig::new("username", FieldType::String { max_length: 50 });

    assert_eq!(field.name, "username");
    assert_eq!(field.display_name, "");
    assert!(!field.required);
    assert!(field.default_value.is_none());
    assert!(field.validators.is_empty());
    assert!(field.filterable);
    assert!(field.sortable);
    assert!(field.relation.is_none());
}

#[test]
fn test_field_config_builder() {
    let field = FieldConfig::new("email", FieldType::String { max_length: 100 })
        .display_name("邮箱")
        .required(true)
        .default_value(json!("default@example.com"))
        .validator(Validator::Email)
        .filterable(false)
        .sortable(false);

    assert_eq!(field.name, "email");
    assert_eq!(field.display_name, "邮箱");
    assert!(field.required);
    assert_eq!(field.default_value, Some(json!("default@example.com")));
    assert_eq!(field.validators.len(), 1);
    assert!(!field.filterable);
    assert!(!field.sortable);
}

#[test]
fn test_field_config_with_permissions() {
    let permissions = FieldPermissions {
        readable_roles: HashSet::from(["admin".to_string()]),
        writable_roles: HashSet::from(["admin".to_string()]),
        filterable_roles: HashSet::new(),
        sortable_roles: HashSet::new(),
    };

    let field = FieldConfig::new("salary", FieldType::Double).permissions(permissions);

    assert_eq!(field.permissions.readable_roles.len(), 1);
    assert_eq!(field.permissions.writable_roles.len(), 1);
}

#[test]
fn test_field_config_with_relation() {
    let relation = RelationConfig {
        table: "users".to_string(),
        field: "id".to_string(),
        display_fields: vec!["username".to_string(), "email".to_string()],
        relation_type: RelationType::OneToOne,
    };

    let field = FieldConfig::new("user_id", FieldType::BigInt).relation(relation);

    assert!(field.relation.is_some());
    let rel = field.relation.unwrap();
    assert_eq!(rel.table, "users");
    assert_eq!(rel.field, "id");
    assert_eq!(rel.display_fields.len(), 2);
    assert_eq!(rel.relation_type, RelationType::OneToOne);
}

// ==================== validate 方法测试 ====================

#[test]
fn test_validate_required_field_success() {
    let field = FieldConfig::new("username", FieldType::String { max_length: 50 }).required(true);

    // 正常情况
    assert!(field.validate(&json!("alice")).is_ok());
}

#[test]
fn test_validate_required_field_null() {
    let field = FieldConfig::new("username", FieldType::String { max_length: 50 }).required(true);

    // 必填字段为 null
    let result = field.validate(&json!(null));
    assert!(result.is_err());

    if let Err(BaseError::FieldRequired(field_name)) = result {
        assert_eq!(field_name, "username");
    } else {
        panic!("期望 FieldRequired 错误");
    }
}

#[test]
fn test_validate_optional_field_null() {
    let field = FieldConfig::new("nickname", FieldType::String { max_length: 50 }).required(false);

    // 可选字段为 null，应该通过验证
    assert!(field.validate(&json!(null)).is_ok());
}

#[test]
fn test_validate_field_type() {
    let field = FieldConfig::new("age", FieldType::Integer).required(true);

    // 正确的类型
    assert!(field.validate(&json!(25)).is_ok());

    // 错误的类型
    assert!(field.validate(&json!("not a number")).is_err());
}

#[test]
fn test_validate_with_validators() {
    let field = FieldConfig::new("username", FieldType::String { max_length: 50 })
        .required(true)
        .validator(Validator::MinLength(3))
        .validator(Validator::MaxLength(20));

    // 正常情况
    assert!(field.validate(&json!("alice")).is_ok());

    // 长度不足
    assert!(field.validate(&json!("ab")).is_err());

    // 长度超出
    assert!(field
        .validate(&json!("this_is_a_very_long_username"))
        .is_err());
}

#[test]
fn test_validate_chain() {
    let field = FieldConfig::new("email", FieldType::String { max_length: 100 })
        .required(true)
        .validator(Validator::Email);

    // 正常情况
    assert!(field.validate(&json!("user@example.com")).is_ok());

    // 必填字段为 null
    assert!(field.validate(&json!(null)).is_err());

    // 类型错误
    assert!(field.validate(&json!(123)).is_err());

    // 邮箱格式错误
    assert!(field.validate(&json!("invalid")).is_err());
}

#[test]
fn test_validate_enum_field() {
    let field = FieldConfig::new(
        "status",
        FieldType::Enum {
            values: vec!["active".to_string(), "inactive".to_string()],
        },
    )
    .required(true);

    // 正常情况
    assert!(field.validate(&json!("active")).is_ok());
    assert!(field.validate(&json!("inactive")).is_ok());

    // 枚举值无效
    assert!(field.validate(&json!("invalid")).is_err());
}

// ==================== FieldPermissions 测试 ====================

#[test]
fn test_field_permissions_default() {
    let permissions = FieldPermissions::default();

    // 默认情况下，所有角色都可以访问
    let user_roles = HashSet::from(["user".to_string()]);
    assert!(permissions.can_read(&user_roles));
    assert!(permissions.can_write(&user_roles));
    assert!(permissions.can_filter(&user_roles));
    assert!(permissions.can_sort(&user_roles));
}

#[test]
fn test_field_permissions_can_read() {
    let permissions = FieldPermissions {
        readable_roles: HashSet::from(["admin".to_string(), "user".to_string()]),
        ..Default::default()
    };

    // 有权限的角色
    assert!(permissions.can_read(&HashSet::from(["admin".to_string()])));
    assert!(permissions.can_read(&HashSet::from(["user".to_string()])));

    // 没有权限的角色
    assert!(!permissions.can_read(&HashSet::from(["guest".to_string()])));

    // 多个角色，其中一个有权限
    assert!(permissions.can_read(&HashSet::from(["guest".to_string(), "user".to_string()])));
}

#[test]
fn test_field_permissions_can_write() {
    let permissions = FieldPermissions {
        writable_roles: HashSet::from(["admin".to_string()]),
        ..Default::default()
    };

    // 有权限的角色
    assert!(permissions.can_write(&HashSet::from(["admin".to_string()])));

    // 没有权限的角色
    assert!(!permissions.can_write(&HashSet::from(["user".to_string()])));
}

#[test]
fn test_field_permissions_can_filter() {
    let permissions = FieldPermissions {
        filterable_roles: HashSet::from(["admin".to_string()]),
        ..Default::default()
    };

    // 有权限的角色
    assert!(permissions.can_filter(&HashSet::from(["admin".to_string()])));

    // 没有权限的角色
    assert!(!permissions.can_filter(&HashSet::from(["user".to_string()])));
}

#[test]
fn test_field_permissions_can_sort() {
    let permissions = FieldPermissions {
        sortable_roles: HashSet::from(["admin".to_string()]),
        ..Default::default()
    };

    // 有权限的角色
    assert!(permissions.can_sort(&HashSet::from(["admin".to_string()])));

    // 没有权限的角色
    assert!(!permissions.can_sort(&HashSet::from(["user".to_string()])));
}

#[test]
fn test_field_permissions_empty_roles() {
    let permissions = FieldPermissions {
        readable_roles: HashSet::new(),
        writable_roles: HashSet::new(),
        filterable_roles: HashSet::new(),
        sortable_roles: HashSet::new(),
    };

    // 空列表表示所有人都可以访问
    let any_roles = HashSet::from(["any_role".to_string()]);
    assert!(permissions.can_read(&any_roles));
    assert!(permissions.can_write(&any_roles));
    assert!(permissions.can_filter(&any_roles));
    assert!(permissions.can_sort(&any_roles));
}

// ==================== RelationConfig 测试 ====================

#[test]
fn test_relation_config_one_to_one() {
    let relation = RelationConfig {
        table: "users".to_string(),
        field: "id".to_string(),
        display_fields: vec!["username".to_string()],
        relation_type: RelationType::OneToOne,
    };

    assert_eq!(relation.table, "users");
    assert_eq!(relation.field, "id");
    assert_eq!(relation.display_fields.len(), 1);
    assert_eq!(relation.relation_type, RelationType::OneToOne);
}

#[test]
fn test_relation_config_one_to_many() {
    let relation = RelationConfig {
        table: "posts".to_string(),
        field: "user_id".to_string(),
        display_fields: vec!["title".to_string(), "content".to_string()],
        relation_type: RelationType::OneToMany,
    };

    assert_eq!(relation.relation_type, RelationType::OneToMany);
}

#[test]
fn test_relation_config_many_to_many() {
    let relation = RelationConfig {
        table: "tags".to_string(),
        field: "id".to_string(),
        display_fields: vec!["name".to_string()],
        relation_type: RelationType::ManyToMany,
    };

    assert_eq!(relation.relation_type, RelationType::ManyToMany);
}
