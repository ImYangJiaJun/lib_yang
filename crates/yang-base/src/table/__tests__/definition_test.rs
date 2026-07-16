use crate::table::{col, Field, FieldType, RelationType, SortOrder, Table};
use serde_json::json;
use std::collections::HashSet;

fn users() -> crate::Result<crate::table::TableDefinition> {
    Table::new("users")
        .label("用户")
        .fields([
            Field::id("id"),
            Field::string("username", 64)
                .label("用户名")
                .required()
                .min_length(3)
                .unique(),
            Field::string("password_hash", 255)
                .required()
                .secret()
                .readable_by(["system"])
                .writable_by(["system"]),
            Field::string("status", 16)
                .required()
                .default("active")
                .index(),
            Field::created_at("created_at"),
            Field::updated_at("updated_at"),
            Field::soft_delete("deleted_at"),
        ])
        .default_order(col("id").desc())
        .build()
}

#[test]
fn schema_first_definition_keeps_name_and_config_together() {
    let table = users().expect("合法表定义应构建成功");

    assert_eq!(table.name(), "users");
    assert_eq!(table.label(), "用户");
    assert_eq!(table.primary_key(), "id");
    assert_eq!(table.field_count(), 7);
    assert_eq!(table.soft_delete_field(), Some("deleted_at"));

    let username = table.field("username").expect("username 应存在");
    assert_eq!(username.label(), "用户名");
    assert!(username.is_required());
    assert_eq!(username.field_type(), &FieldType::String { max_length: 64 });

    let password = table.field("password_hash").expect("password_hash 应存在");
    assert!(password.is_secret());
    assert!(!password.is_filterable());
    assert!(!password.is_sortable());
}

#[test]
fn schemas_exclude_secret_and_generated_fields() {
    let table = users().expect("合法表定义应构建成功");
    let output = table.output_schema();
    let input = table.input_schema();

    assert!(output["properties"].get("password_hash").is_none());
    assert!(input["properties"].get("password_hash").is_none());
    assert!(input["properties"].get("id").is_none());
    assert!(input["properties"].get("created_at").is_none());
    assert!(input["properties"].get("updated_at").is_none());
    assert!(input["properties"].get("deleted_at").is_none());
    assert_eq!(input["properties"]["status"]["default"], json!("active"));
    assert!(!input["required"]
        .as_array()
        .expect("required 应为数组")
        .contains(&json!("id")));
}

#[test]
fn role_scoped_schemas_only_expose_fields_allowed_for_current_roles() {
    let table = Table::new("notes")
        .fields([
            Field::id("id"),
            Field::string("title", 64).required(),
            Field::string("admin_note", 255)
                .required()
                .readable_by(["admin"])
                .writable_by(["admin"]),
            Field::string("member_note", 255)
                .readable_by(["member"])
                .writable_by(["member"]),
            Field::string("internal_note", 255)
                .not_readable()
                .not_writable(),
        ])
        .build()
        .expect("角色字段表定义应有效");
    let admin = HashSet::from(["admin".to_string()]);
    let anonymous = HashSet::new();

    let catalog_input = table.input_schema();
    let catalog_output = table.output_schema();
    assert!(catalog_input["properties"].get("admin_note").is_some());
    assert!(catalog_input["properties"].get("member_note").is_some());
    assert!(catalog_output["properties"].get("admin_note").is_some());
    assert!(catalog_output["properties"].get("member_note").is_some());

    let admin_input = table.input_schema_for_roles(&admin);
    let admin_output = table.output_schema_for_roles(&admin);
    assert!(admin_input["properties"].get("title").is_some());
    assert!(admin_input["properties"].get("admin_note").is_some());
    assert!(admin_input["properties"].get("member_note").is_none());
    assert!(admin_input["properties"].get("internal_note").is_none());
    assert!(admin_output["properties"].get("admin_note").is_some());
    assert!(admin_output["properties"].get("member_note").is_none());

    let anonymous_input = table.input_schema_for_roles(&anonymous);
    let anonymous_output = table.output_schema_for_roles(&anonymous);
    assert!(anonymous_input["properties"].get("title").is_some());
    assert!(anonymous_input["properties"].get("admin_note").is_none());
    assert!(anonymous_output["properties"].get("title").is_some());
    assert!(anonymous_output["properties"].get("admin_note").is_none());
}

#[test]
fn build_rejects_duplicate_fields() {
    let result = Table::new("users")
        .fields([Field::id("id"), Field::string("id", 32)])
        .build();

    assert!(result.is_err());
}

#[test]
fn build_rejects_missing_primary_key() {
    let result = Table::new("users")
        .fields([Field::string("username", 64).required()])
        .build();

    assert!(result.is_err());
}

#[test]
fn build_rejects_auto_increment_on_string() {
    let result = Table::new("users")
        .fields([Field::string("id", 32)
            .required()
            .primary_key()
            .auto_increment()])
        .build();

    assert!(result.is_err());
}

#[test]
fn build_rejects_unknown_index_field() {
    let result = Table::new("users")
        .fields(vec![Field::id("id")])
        .index(["missing"])
        .build();

    assert!(result.is_err());
}

#[test]
fn schema_first_field_policies_feed_runtime_validation_and_permissions() {
    let table = Table::new("employees")
        .fields([
            Field::id("id"),
            Field::string("email", 100)
                .required()
                .email()
                .readable_by(["admin", "user"])
                .writable_by(["admin"])
                .filterable_by(["admin"])
                .not_sortable(),
            Field::bigint("manager_id")
                .relation("employees", "id", RelationType::ManyToOne)
                .relation_display_fields(["email"]),
        ])
        .build()
        .expect("字段策略表定义应有效");
    let config = table.shared_config();
    let email = config.get_field("email").expect("email 应存在");
    let admin = HashSet::from(["admin".to_string()]);
    let user = HashSet::from(["user".to_string()]);

    assert!(email.validate(&json!("user@example.com")).is_ok());
    assert!(email.validate(&json!("invalid")).is_err());
    assert!(email.permissions.can_read(&admin));
    assert!(email.permissions.can_read(&user));
    assert!(email.permissions.can_write(&admin));
    assert!(!email.permissions.can_write(&user));
    assert!(email.permissions.can_filter(&admin));
    assert!(!email.permissions.can_filter(&user));
    assert!(!email.permissions.can_sort(&admin));

    let relation = config
        .get_field("manager_id")
        .and_then(|field| field.relation.as_ref())
        .expect("manager_id 应保留关联元数据");
    assert_eq!(relation.table, "employees");
    assert_eq!(relation.field, "id");
    assert_eq!(relation.display_fields, ["email"]);
    assert_eq!(relation.relation_type, RelationType::ManyToOne);

    let schema = table.output_schema();
    assert_eq!(
        schema["properties"]["manager_id"]["x-yang-relation"],
        json!({
            "table": "employees",
            "field": "id",
            "displayFields": ["email"],
            "type": "many_to_one"
        })
    );
}

#[test]
fn schema_first_table_semantics_feed_indexes_order_and_timestamps() {
    let table = users().expect("合法表定义应构建成功");
    let config = table.shared_config();

    assert_eq!(config.unique_indexes.len(), 1);
    assert_eq!(config.unique_indexes[0].fields, ["username"]);
    assert_eq!(config.indexes.len(), 1);
    assert_eq!(config.indexes[0].fields, ["status"]);
    assert_eq!(config.default_order, [("id".to_string(), SortOrder::Desc)]);
    assert_eq!(config.soft_delete_field.as_deref(), Some("deleted_at"));

    let timestamps = config.timestamp_fields.as_ref().expect("时间戳语义应存在");
    assert_eq!(timestamps.created_at.as_deref(), Some("created_at"));
    assert_eq!(timestamps.updated_at.as_deref(), Some("updated_at"));
    assert_eq!(timestamps.deleted_at.as_deref(), Some("deleted_at"));
}

#[test]
fn json_schema_matches_nullable_json_and_standard_validators() {
    let nickname = Field::string("nickname", 32).nullable().length(3..=16);
    #[cfg(feature = "validator")]
    let nickname = nickname.regex("^[a-z]+$");
    let table = Table::new("profiles")
        .fields([
            Field::id("id"),
            nickname,
            Field::integer("score").nullable().min(0.0).max(100.0),
            Field::json("settings").nullable(),
        ])
        .build()
        .expect("可选字段及验证器定义应有效");
    let schema = table.input_schema();

    assert_eq!(
        schema["properties"]["nickname"]["type"],
        json!(["string", "null"])
    );
    assert_eq!(schema["properties"]["nickname"]["minLength"], json!(3));
    assert_eq!(schema["properties"]["nickname"]["maxLength"], json!(16));
    #[cfg(feature = "validator")]
    assert_eq!(
        schema["properties"]["nickname"]["pattern"],
        json!("^[a-z]+$")
    );
    assert_eq!(schema["properties"]["score"]["minimum"], json!(0.0));
    assert_eq!(schema["properties"]["score"]["maximum"], json!(100.0));
    assert_eq!(
        schema["properties"]["settings"]["type"],
        json!(["object", "array", "null"])
    );
}

#[test]
fn build_rejects_invalid_validator_configuration() {
    let invalid_regex = Table::new("profiles")
        .fields([Field::id("id"), Field::string("nickname", 32).regex("[")])
        .build();
    assert!(invalid_regex.is_err());

    let wrong_type = Table::new("profiles")
        .fields([Field::id("id"), Field::string("nickname", 32).min(1.0)])
        .build();
    assert!(wrong_type.is_err());

    let reversed_range = Table::new("profiles")
        .fields([
            Field::id("id"),
            Field::string("nickname", 32).min_length(10).max_length(3),
        ])
        .build();
    assert!(reversed_range.is_err());
}

#[test]
fn build_rejects_defaults_schema_sync_cannot_render() {
    let result = Table::new("documents")
        .fields([Field::id("id"), Field::text("body").default("empty")])
        .build();

    assert!(result.is_err());
}

#[test]
fn build_rejects_duplicate_generated_index_names() {
    let result = Table::new("users")
        .fields([Field::id("id"), Field::string("username", 64)])
        .index(["username"])
        .index(["username"])
        .build();

    assert!(result.is_err());
}

#[test]
fn named_unique_index_is_escape_hatch_for_long_generated_name() {
    let table_name = "orders_with_a_very_long_but_valid_business_table_name";
    let field_name = "external_reference_identifier";

    let generated = Table::new(table_name)
        .fields([Field::id("id"), Field::string(field_name, 64)])
        .unique([field_name])
        .build();
    assert!(generated.is_err());

    let named = Table::new(table_name)
        .fields([Field::id("id"), Field::string(field_name, 64)])
        .unique_named("uk_orders_external_ref", [field_name])
        .build();
    assert!(named.is_ok());

    let field_named = Table::new("users")
        .fields([
            Field::id("id"),
            Field::string("email", 128).unique_named("uk_users_email"),
        ])
        .build()
        .expect("字段级唯一索引应支持显式命名");
    assert_eq!(
        field_named.shared_config().unique_indexes[0]
            .name
            .as_deref(),
        Some("uk_users_email")
    );
}
