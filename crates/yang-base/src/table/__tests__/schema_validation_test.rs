use crate::table::{FieldConfig, FieldType, SchemaColumn, SchemaIssueKind, TableConfig};

#[test]
fn table_config_schema_validation_reports_only_required_runtime_contract_gaps() {
    let table = TableConfig::new("users")
        .field(FieldConfig::new("id", FieldType::BigInt).required(true))
        .expect("合法字段")
        .field(FieldConfig::new("name", FieldType::String { max_length: 64 }).required(true))
        .expect("合法字段")
        .field(FieldConfig::new("age", FieldType::Integer))
        .expect("合法字段");
    let columns = vec![
        SchemaColumn::new("id", "bigint", "bigint", false, None),
        SchemaColumn::new("name", "varchar", "varchar(32)", true, Some(32)),
        SchemaColumn::new("database_only", "json", "json", true, None),
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
    let table = TableConfig::new("users")
        .field(FieldConfig::new(
            "name",
            FieldType::String { max_length: 64 },
        ))
        .expect("合法字段")
        .field(FieldConfig::new("enabled", FieldType::Boolean).required(true))
        .expect("合法字段");
    let columns = vec![
        SchemaColumn::new("name", "varchar", "varchar(255)", true, Some(255)),
        SchemaColumn::new("enabled", "tinyint", "tinyint(1)", false, None),
        SchemaColumn::new("created_by_trigger", "timestamp", "timestamp", false, None),
    ];

    assert!(table.validate_schema(&columns).is_compatible());
}
