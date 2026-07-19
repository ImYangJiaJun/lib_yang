use super::schema_sync::{plan_table_sync, ExistingIndex, ExistingTableSchema};
use super::{DatabaseInitializer, SchemaSyncChangeKind, SchemaSyncReport};
use crate::definition::{
    AddonName, AddonSpec, AppBuilder, FieldKind, FieldName, FieldSpec, ModuleName, ModuleSpec,
    TableName, TableSpec,
};
use crate::error::BaseError;
use crate::table::{Field, RelationType, SchemaColumn, Table, TableDefinition};
use crate::tools::ToolsBuilder;

fn account_table() -> TableDefinition {
    Table::new("accounts")
        .fields([
            Field::id("id"),
            Field::string("username", 32).required().unique(),
        ])
        .build()
        .expect("accounts 表定义应有效")
}

#[allow(dead_code)]
async fn public_schema_sync_api_typechecks(
    initializer: &DatabaseInitializer,
    definitions: &[&TableDefinition],
) -> Result<SchemaSyncReport, BaseError> {
    initializer.sync_table_definitions(definitions).await
}

#[allow(dead_code)]
async fn public_schema_plan_api_typechecks(
    initializer: &DatabaseInitializer,
    definitions: &[&TableDefinition],
) -> Result<SchemaSyncReport, BaseError> {
    initializer.plan_table_definitions(definitions).await
}

#[test]
fn built_app_exposes_compiled_table_definitions_in_module_order() {
    let table_spec = |name: &str| {
        TableSpec::new(TableName::new(name).expect("测试表名应有效")).field(FieldSpec::new(
            FieldName::new("id").expect("测试字段名应有效"),
            FieldKind::Key,
        ))
    };
    let app = AppBuilder::new()
        .addon(
            AddonSpec::new(AddonName::new("account").expect("测试 Addon 名称应有效"))
                .module(
                    ModuleSpec::new(
                        ModuleName::new("account.z_account").expect("测试 Module 名称应有效"),
                    )
                    .table(table_spec("accounts")),
                )
                .module(
                    ModuleSpec::new(
                        ModuleName::new("account.a_audit").expect("测试 Module 名称应有效"),
                    )
                    .table(table_spec("audit_logs")),
                ),
        )
        .build(ToolsBuilder::new().build().expect("测试 Tools 应有效"))
        .expect("应用定义应构建成功");

    let names: Vec<&str> = app
        .table_definitions()
        .iter()
        .map(TableDefinition::name)
        .collect();

    assert_eq!(names, vec!["audit_logs", "accounts"]);
}

#[test]
fn additive_plan_creates_missing_table_from_definition() {
    let table = account_table();
    let plan = plan_table_sync(&table, &ExistingTableSchema::missing())
        .expect("合法 TableDefinition 应生成建表计划");

    assert_eq!(plan.changes.len(), 1);
    assert_eq!(plan.changes[0].kind, SchemaSyncChangeKind::CreatedTable);
    assert!(plan.statements[0].contains("CREATE TABLE `accounts`"));
    assert!(plan.statements[0].contains("`id` BIGINT NOT NULL AUTO_INCREMENT"));
    assert!(plan.statements[0].contains("PRIMARY KEY (`id`)"));
    assert!(plan.statements[0].contains("UNIQUE KEY"));
}

#[test]
fn relation_field_uses_its_concrete_type_for_local_column_ddl() {
    let table = Table::new("orders")
        .fields([
            Field::id("id"),
            Field::bigint("account_id").required().relation(
                "accounts",
                "id",
                RelationType::ManyToOne,
            ),
        ])
        .build()
        .expect("关联字段定义应有效");

    let plan = plan_table_sync(&table, &ExistingTableSchema::missing())
        .expect("具体存储类型必须能生成本地列 DDL");

    assert!(plan.statements[0].contains("`account_id` BIGINT NOT NULL"));
}

#[test]
fn additive_plan_only_adds_missing_columns_and_indexes() {
    let existing = ExistingTableSchema::existing(
        vec![
            SchemaColumn::new("id", "bigint", "bigint", false, None, None)
                .with_auto_increment(true),
        ],
        vec!["id".to_string()],
        Vec::new(),
    );

    let table = account_table();
    let plan = plan_table_sync(&table, &existing).expect("兼容旧表应生成增量计划");

    assert_eq!(plan.changes.len(), 2);
    assert_eq!(plan.changes[0].kind, SchemaSyncChangeKind::AddedColumn);
    assert_eq!(plan.changes[0].object, "username");
    assert_eq!(plan.changes[1].kind, SchemaSyncChangeKind::AddedIndex);
    assert!(plan
        .statements
        .iter()
        .all(|statement| !statement.contains("DROP")));
}

#[test]
fn additive_plan_rejects_destructive_type_or_nullability_changes() {
    let existing = ExistingTableSchema::existing(
        vec![
            SchemaColumn::new("id", "bigint", "bigint", false, None, None)
                .with_auto_increment(true),
            SchemaColumn::new("username", "varchar", "varchar(8)", true, Some(8), None),
        ],
        vec!["id".to_string()],
        vec![ExistingIndex::new(
            "uk_accounts_username",
            true,
            vec!["username".to_string()],
        )],
    );

    let table = account_table();
    let error = plan_table_sync(&table, &existing).expect_err("缩窄类型和收紧 NULL 不能自动修改");

    assert!(matches!(error, BaseError::DatabaseInitFailed(message)
        if message.contains("accounts") && message.contains("username")));
}

#[test]
fn additive_plan_rejects_conflicting_named_index() {
    let existing = ExistingTableSchema::existing(
        vec![
            SchemaColumn::new("id", "bigint", "bigint", false, None, None)
                .with_auto_increment(true),
            SchemaColumn::new("username", "varchar", "varchar(32)", false, Some(32), None),
        ],
        vec!["id".to_string()],
        vec![ExistingIndex::new(
            "uk_accounts_username",
            false,
            vec!["username".to_string()],
        )],
    );

    let table = account_table();
    let error = plan_table_sync(&table, &existing).expect_err("同名索引定义冲突时不能自动覆盖");

    assert!(matches!(error, BaseError::DatabaseInitFailed(message)
        if message.contains("uk_accounts_username") && message.contains("定义不同")));
}

#[test]
fn additive_plan_rejects_missing_auto_increment_column_on_existing_table() {
    let existing = ExistingTableSchema::existing(Vec::new(), Vec::new(), Vec::new());

    let table = account_table();
    let error = plan_table_sync(&table, &existing).expect_err("已有表不能分步增加自增主键列");

    assert!(matches!(error, BaseError::DatabaseInitFailed(message)
        if message.contains("自增主键字段") && message.contains("人工处理")));
}

#[test]
fn additive_plan_rejects_existing_database_default_drift() {
    let existing = ExistingTableSchema::existing(
        vec![
            SchemaColumn::new("id", "bigint", "bigint", false, None, None)
                .with_auto_increment(true),
            SchemaColumn::new(
                "username",
                "varchar",
                "varchar(32)",
                false,
                Some(32),
                Some("guest".to_string()),
            ),
        ],
        vec!["id".to_string()],
        vec![ExistingIndex::new(
            "uk_accounts_username",
            true,
            vec!["username".to_string()],
        )],
    );

    let table = account_table();
    let error = plan_table_sync(&table, &existing).expect_err("数据库默认值与声明不同不能自动修改");

    assert!(matches!(error, BaseError::DatabaseInitFailed(message)
        if message.contains("accounts") && message.contains("username")
            && message.contains("DefaultMismatch")));
}
