use super::schema_sync::{plan_table_sync, ExistingIndex, ExistingTableSchema};
use super::{DatabaseInitializer, SchemaSyncChangeKind, SchemaSyncReport};
use crate::error::BaseError;
use crate::router::{AppRouter, ModuleRouter};
use crate::table::{FieldConfig, FieldType, SchemaColumn, TableConfig};
use std::sync::Arc;

fn account_table() -> TableConfig {
    TableConfig::new("accounts")
        .primary_key("id")
        .field(
            FieldConfig::new("id", FieldType::BigInt)
                .required(true)
                .auto_increment(true),
        )
        .expect("id 字段配置应有效")
        .field(FieldConfig::new("username", FieldType::String { max_length: 32 }).required(true))
        .expect("username 字段配置应有效")
        .unique_index(vec!["username".to_string()])
}

#[allow(dead_code)]
async fn public_schema_sync_api_typechecks(
    initializer: &DatabaseInitializer,
    app: &AppRouter,
) -> Result<SchemaSyncReport, BaseError> {
    initializer.sync_app_schema(app).await
}

#[test]
fn app_router_exposes_table_configs_in_module_order() {
    let accounts = Arc::new(account_table());
    let sessions = Arc::new(
        TableConfig::new("account_sessions")
            .field(FieldConfig::new("id", FieldType::BigInt).required(true))
            .expect("session id 字段配置应有效"),
    );
    let audit = Arc::new(
        TableConfig::new("audit_logs")
            .field(FieldConfig::new("id", FieldType::BigInt).required(true))
            .expect("audit id 字段配置应有效"),
    );
    let app = AppRouter::new()
        .register_module(
            ModuleRouter::new("z_account", "账号")
                .with_table_config(accounts)
                .with_schema_table(sessions),
        )
        .expect("账号模块应注册成功")
        .register_module(ModuleRouter::new("a_audit", "审计").with_table_config(audit))
        .expect("审计模块应注册成功");

    let names: Vec<&str> = app
        .table_configs()
        .into_iter()
        .map(|table| table.table_name.as_str())
        .collect();

    assert_eq!(names, vec!["audit_logs", "account_sessions", "accounts"]);
}

#[test]
fn additive_plan_creates_missing_table_from_table_config() {
    let plan = plan_table_sync(&account_table(), &ExistingTableSchema::missing())
        .expect("合法 TableConfig 应生成建表计划");

    assert_eq!(plan.changes.len(), 1);
    assert_eq!(plan.changes[0].kind, SchemaSyncChangeKind::CreatedTable);
    assert!(plan.statements[0].contains("CREATE TABLE `accounts`"));
    assert!(plan.statements[0].contains("`id` BIGINT NOT NULL AUTO_INCREMENT"));
    assert!(plan.statements[0].contains("PRIMARY KEY (`id`)"));
    assert!(plan.statements[0].contains("UNIQUE KEY"));
}

#[test]
fn additive_plan_only_adds_missing_columns_and_indexes() {
    let existing = ExistingTableSchema::existing(
        vec![SchemaColumn::new("id", "bigint", "bigint", false, None).with_auto_increment(true)],
        vec!["id".to_string()],
        Vec::new(),
    );

    let plan = plan_table_sync(&account_table(), &existing).expect("兼容旧表应生成增量计划");

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
            SchemaColumn::new("id", "bigint", "bigint", false, None).with_auto_increment(true),
            SchemaColumn::new("username", "varchar", "varchar(8)", true, Some(8)),
        ],
        vec!["id".to_string()],
        vec![ExistingIndex::new(
            "uk_accounts_username",
            true,
            vec!["username".to_string()],
        )],
    );

    let error =
        plan_table_sync(&account_table(), &existing).expect_err("缩窄类型和收紧 NULL 不能自动修改");

    assert!(matches!(error, BaseError::DatabaseInitFailed(message)
        if message.contains("accounts") && message.contains("username")));
}

#[test]
fn additive_plan_rejects_conflicting_named_index() {
    let existing = ExistingTableSchema::existing(
        vec![
            SchemaColumn::new("id", "bigint", "bigint", false, None).with_auto_increment(true),
            SchemaColumn::new("username", "varchar", "varchar(32)", false, Some(32)),
        ],
        vec!["id".to_string()],
        vec![ExistingIndex::new(
            "uk_accounts_username",
            false,
            vec!["username".to_string()],
        )],
    );

    let error =
        plan_table_sync(&account_table(), &existing).expect_err("同名索引定义冲突时不能自动覆盖");

    assert!(matches!(error, BaseError::DatabaseInitFailed(message)
        if message.contains("uk_accounts_username") && message.contains("定义不同")));
}

#[test]
fn additive_plan_rejects_missing_auto_increment_column_on_existing_table() {
    let existing = ExistingTableSchema::existing(Vec::new(), Vec::new(), Vec::new());

    let error =
        plan_table_sync(&account_table(), &existing).expect_err("已有表不能分步增加自增主键列");

    assert!(matches!(error, BaseError::DatabaseInitFailed(message)
        if message.contains("自增主键字段") && message.contains("人工处理")));
}
