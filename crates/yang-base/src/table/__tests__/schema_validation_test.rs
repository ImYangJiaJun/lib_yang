use crate::table::{Field, SchemaColumn, SchemaIssueKind, Table};

#[test]
fn table_definition_schema_validation_reports_only_required_runtime_contract_gaps() {
    let table = Table::new("users")
        .fields([
            Field::bigint("id").required().primary_key(),
            Field::string("name", 64).required(),
            Field::integer("age"),
        ])
        .build()
        .expect("测试表定义应有效");
    let columns = vec![
        SchemaColumn::new("id", "bigint", "bigint", false, None, None),
        SchemaColumn::new("name", "varchar", "varchar(32)", true, Some(32), None),
        SchemaColumn::new("database_only", "json", "json", true, None, None),
    ];

    let report = table.validate_schema(&columns);
    assert!(!report.is_compatible());
    assert_eq!(report.issues.len(), 3);
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.field == "age" && issue.kind == SchemaIssueKind::MissingColumn));
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.field == "name" && issue.kind == SchemaIssueKind::IncompatibleType));
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.field == "name" && issue.kind == SchemaIssueKind::NullabilityMismatch));
    assert!(report
        .issues
        .iter()
        .all(|issue| issue.field != "database_only"));
}

#[test]
fn compatible_schema_accepts_wider_storage_without_claiming_ddl_ownership() {
    let table = Table::new("users")
        .fields([
            Field::bigint("id").required().primary_key(),
            Field::string("name", 64),
            Field::boolean("enabled").required(),
        ])
        .build()
        .expect("测试表定义应有效");
    let columns = vec![
        SchemaColumn::new("id", "bigint", "bigint", false, None, None),
        SchemaColumn::new("name", "varchar", "varchar(255)", true, Some(255), None),
        SchemaColumn::new("enabled", "tinyint", "TINYINT(1)", false, None, None),
        SchemaColumn::new(
            "created_by_trigger",
            "timestamp",
            "timestamp",
            false,
            None,
            None,
        ),
    ];

    assert!(table.validate_schema(&columns).is_compatible());
}

#[test]
fn schema_validation_compares_nullability_in_both_directions() {
    let table = Table::new("profiles")
        .fields([Field::id("id"), Field::string("nickname", 32).nullable()])
        .build()
        .expect("测试表定义应有效");
    let columns = vec![
        SchemaColumn::new("id", "bigint", "bigint", false, None, None).with_auto_increment(true),
        SchemaColumn::new("nickname", "varchar", "varchar(32)", false, Some(32), None),
    ];

    let report = table.validate_schema(&columns);
    assert!(report.issues.iter().any(|issue| {
        issue.field == "nickname" && issue.kind == SchemaIssueKind::NullabilityMismatch
    }));
}

#[test]
fn schema_validation_requires_exact_storage_types_and_enum_candidates() {
    let table = Table::new("articles")
        .fields([
            Field::id("id"),
            Field::datetime("published_at").required(),
            Field::text("body").required(),
            Field::enumeration("status", ["Draft", "published"]).required(),
        ])
        .build()
        .expect("测试表定义应有效");
    let columns = vec![
        SchemaColumn::new("id", "bigint", "bigint", false, None, None).with_auto_increment(true),
        SchemaColumn::new("published_at", "timestamp", "timestamp", false, None, None),
        SchemaColumn::new("body", "tinytext", "tinytext", false, Some(255), None),
        SchemaColumn::new(
            "status",
            "enum",
            "enum('draft','published')",
            false,
            Some(9),
            None,
        ),
    ];

    let report = table.validate_schema(&columns);
    for field in ["published_at", "body", "status"] {
        assert!(report.issues.iter().any(|issue| {
            issue.field == field && issue.kind == SchemaIssueKind::IncompatibleType
        }));
    }
}

#[test]
fn schema_validation_parses_enum_candidates_without_losing_literal_case() {
    let table = Table::new("labels")
        .fields([
            Field::id("id"),
            Field::enumeration("value", ["Draft", "O'Reilly", r"path\leaf"]).required(),
        ])
        .build()
        .expect("测试表定义应有效");
    let columns = vec![
        SchemaColumn::new("id", "bigint", "bigint", false, None, None).with_auto_increment(true),
        SchemaColumn::new(
            "value",
            "enum",
            r"enum('Draft','O''Reilly','path\\leaf')",
            false,
            Some(8),
            None,
        ),
    ];

    assert!(table.validate_schema(&columns).is_compatible());
}

#[test]
fn schema_validation_normalizes_and_compares_database_defaults() {
    let table = Table::new("settings")
        .fields([
            Field::id("id"),
            Field::boolean("enabled").required().default(true),
            Field::integer("retries").required().default(3),
            Field::double("ratio").required().default(1.0),
            Field::string("mode", 16).required().default("safe"),
            Field::string("note", 64)
                .nullable()
                .default(serde_json::Value::Null),
        ])
        .build()
        .expect("测试表定义应有效");
    let mut columns = vec![
        SchemaColumn::new("id", "bigint", "bigint", false, None, None).with_auto_increment(true),
        SchemaColumn::new(
            "enabled",
            "tinyint",
            "tinyint(1)",
            false,
            None,
            Some("1".to_string()),
        ),
        SchemaColumn::new("retries", "int", "int", false, None, Some("03".to_string())),
        SchemaColumn::new(
            "ratio",
            "double",
            "double",
            false,
            None,
            Some("1".to_string()),
        ),
        SchemaColumn::new(
            "mode",
            "varchar",
            "varchar(16)",
            false,
            Some(16),
            Some("safe".to_string()),
        ),
        SchemaColumn::new("note", "varchar", "varchar(64)", true, Some(64), None),
    ];

    assert!(table.validate_schema(&columns).is_compatible());

    columns[1].database_default = Some("0".to_string());
    let report = table.validate_schema(&columns);
    assert!(report.issues.iter().any(|issue| {
        issue.field == "enabled" && issue.kind == SchemaIssueKind::DefaultMismatch
    }));
}
