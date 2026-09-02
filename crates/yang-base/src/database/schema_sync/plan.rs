use super::inspect::load_existing_schema;
use super::model::{ExistingTableSchema, SchemaPreflightCheck, TableSyncPlan};
use super::render::{
    desired_indexes, existing_column_name, expression_for_existing_schema, foreign_key_matches,
    normalize_check_expression, quote_identifier, quote_string, render_check, render_column,
    render_create_table, render_foreign_key, validate_table_config,
};
use super::{SchemaSyncChange, SchemaSyncChangeKind};
use crate::error::BaseError;
use crate::table::{FieldConfig, FieldType, SchemaIssueKind, TableDefinition};
use sqlx::mysql::MySqlConnection;
use std::collections::BTreeMap;

pub(super) async fn plan_locked(
    connection: &mut MySqlConnection,
    definitions: &[&TableDefinition],
) -> Result<Vec<TableSyncPlan>, BaseError> {
    let mut plans = Vec::with_capacity(definitions.len());
    for definition in definitions {
        let existing = load_existing_schema(connection, definition).await?;
        let plan = plan_table_sync(definition, &existing)?;
        if plan.statements.len() != plan.changes.len() {
            return Err(BaseError::DatabaseInitFailed(format!(
                "表 {} 的 schema 计划内部不一致",
                definition.name()
            )));
        }
        plans.push(plan);
    }
    Ok(plans)
}

pub(super) fn report_tables(definitions: &[&TableDefinition]) -> Vec<String> {
    definitions
        .iter()
        .map(|definition| definition.name().to_string())
        .collect()
}

pub(crate) fn plan_table_sync(
    definition: &TableDefinition,
    existing: &ExistingTableSchema,
) -> Result<TableSyncPlan, BaseError> {
    let table = definition.config();
    validate_table_config(table)?;
    if !existing.exists {
        let mut statements = vec![render_create_table(table, false)?];
        let mut changes = vec![SchemaSyncChange {
            table: table.table_name.clone(),
            object: table.table_name.clone(),
            kind: SchemaSyncChangeKind::CreatedTable,
        }];
        for foreign_key in &table.foreign_keys {
            statements.push(format!(
                "ALTER TABLE {} ADD {}",
                quote_identifier(&table.table_name)?,
                render_foreign_key(foreign_key)?
            ));
            changes.push(SchemaSyncChange {
                table: table.table_name.clone(),
                object: foreign_key.name.clone(),
                kind: SchemaSyncChangeKind::AddedForeignKey,
            });
        }
        return Ok(TableSyncPlan {
            statements,
            changes,
            preflight: Vec::new(),
        });
    }

    let quoted_table = quote_identifier(&table.table_name)?;
    let mut statements = Vec::new();
    let mut changes = Vec::new();
    let mut preflight = Vec::new();
    let mut renamed_columns = BTreeMap::new();
    let mut fields: Vec<&FieldConfig> = table.fields.values().collect();
    fields.sort_by(|left, right| left.name.cmp(&right.name));
    for field in fields {
        let current_exists = existing
            .columns
            .iter()
            .any(|column| column.name == field.name);
        let legacy_exists = field.renamed_from.as_ref().is_some_and(|legacy_name| {
            existing
                .columns
                .iter()
                .any(|column| column.name == *legacy_name)
        });
        if current_exists && legacy_exists {
            return Err(BaseError::DatabaseInitFailed(format!(
                "表 {} 同时存在旧列 {} 与目标列 {}，拒绝猜测应保留哪一列；请人工合并数据",
                table.table_name,
                field.renamed_from.as_deref().unwrap_or_default(),
                field.name
            )));
        }
        if current_exists {
            continue;
        }
        if let Some(legacy_name) = field.renamed_from.as_deref().filter(|_| legacy_exists) {
            statements.push(format!(
                "ALTER TABLE {quoted_table} RENAME COLUMN {} TO {}",
                quote_identifier(legacy_name)?,
                quote_identifier(&field.name)?
            ));
            changes.push(SchemaSyncChange {
                table: table.table_name.clone(),
                object: format!("{legacy_name}->{}", field.name),
                kind: SchemaSyncChangeKind::RenamedColumn,
            });
            renamed_columns.insert(legacy_name.to_string(), field.name.clone());
            continue;
        }
        if field.auto_increment {
            return Err(BaseError::DatabaseInitFailed(format!(
                "表 {} 已存在，缺失的自增主键字段 {} 不能安全地分步增加；请人工处理后再启动",
                table.table_name, field.name
            )));
        }
        if existing.has_rows
            && field.required
            && field.default_value.is_none()
            && !field.auto_increment
        {
            return Err(BaseError::DatabaseInitFailed(format!(
                "表 {} 已有数据，不能自动增加无默认值的必填字段 {}",
                table.table_name, field.name
            )));
        }
        statements.push(format!(
            "ALTER TABLE {quoted_table} ADD COLUMN {}",
            render_column(field)?
        ));
        changes.push(SchemaSyncChange {
            table: table.table_name.clone(),
            object: field.name.clone(),
            kind: SchemaSyncChangeKind::AddedColumn,
        });
    }

    let effective_columns = existing
        .columns
        .iter()
        .cloned()
        .map(|mut column| {
            if let Some(renamed) = renamed_columns.get(&column.name) {
                column.name = renamed.clone();
            }
            column
        })
        .collect::<Vec<_>>();
    let report = table.validate_schema(&effective_columns);
    let unsafe_issues = report
        .issues
        .iter()
        .filter(|issue| {
            if issue.kind == SchemaIssueKind::MissingColumn {
                return false;
            }
            let Some(field) = table.fields.get(&issue.field) else {
                return true;
            };
            match issue.kind {
                SchemaIssueKind::NullabilityMismatch | SchemaIssueKind::DefaultMismatch => false,
                SchemaIssueKind::IncompatibleType => {
                    let data_type = effective_columns
                        .iter()
                        .find(|column| column.name == issue.field)
                        .map(|column| column.data_type.as_str());
                    match field.field_type {
                        FieldType::String { .. } => !matches!(
                            data_type,
                            Some("char" | "varchar" | "text" | "mediumtext" | "longtext")
                        ),
                        FieldType::Enum { .. } => {
                            !matches!(data_type, Some("char" | "varchar" | "enum"))
                        }
                        _ => true,
                    }
                }
                SchemaIssueKind::AutoIncrementMismatch | SchemaIssueKind::MissingColumn => true,
            }
        })
        .collect::<Vec<_>>();
    if !unsafe_issues.is_empty() {
        let details = unsafe_issues
            .iter()
            .map(|issue| format!("{}:{:?}", issue.field, issue.kind))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(BaseError::DatabaseInitFailed(format!(
            "表 {} 存在不可自动修改的 schema 差异: {details}",
            table.table_name
        )));
    }
    let mut changed_fields = report
        .issues
        .iter()
        .filter(|issue| issue.kind != SchemaIssueKind::MissingColumn)
        .map(|issue| issue.field.as_str())
        .collect::<Vec<_>>();
    changed_fields.sort_unstable();
    changed_fields.dedup();
    for field_name in changed_fields {
        let field = &table.fields[field_name];
        let existing_name = existing_column_name(field_name, &renamed_columns);
        let quoted_existing = quote_identifier(&existing_name)?;
        let issues = report
            .issues
            .iter()
            .filter(|issue| issue.field == field_name)
            .collect::<Vec<_>>();
        if issues
            .iter()
            .any(|issue| issue.kind == SchemaIssueKind::NullabilityMismatch && field.required)
        {
            preflight.push(SchemaPreflightCheck::ColumnPredicate {
                object: field.name.clone(),
                predicate: format!("{quoted_existing} IS NULL"),
            });
        }
        if issues
            .iter()
            .any(|issue| issue.kind == SchemaIssueKind::IncompatibleType)
        {
            match &field.field_type {
                FieldType::String { max_length } => {
                    preflight.push(SchemaPreflightCheck::ColumnPredicate {
                        object: field.name.clone(),
                        predicate: format!(
                            "{quoted_existing} IS NOT NULL AND CHAR_LENGTH({quoted_existing}) > {max_length}"
                        ),
                    });
                }
                FieldType::Enum { values } => {
                    let allowed = values
                        .iter()
                        .map(|value| quote_string(value))
                        .collect::<Vec<_>>()
                        .join(", ");
                    preflight.push(SchemaPreflightCheck::ColumnPredicate {
                        object: field.name.clone(),
                        predicate: format!(
                            "{quoted_existing} IS NOT NULL AND {quoted_existing} NOT IN ({allowed})"
                        ),
                    });
                }
                _ => {}
            }
        }
        statements.push(format!(
            "ALTER TABLE {quoted_table} MODIFY COLUMN {}",
            render_column(field)?
        ));
        changes.push(SchemaSyncChange {
            table: table.table_name.clone(),
            object: field.name.clone(),
            kind: SchemaSyncChangeKind::ModifiedColumn,
        });
    }

    let desired_primary = vec![table.primary_key.clone()];
    let effective_primary = existing
        .primary_key
        .iter()
        .map(|column| {
            renamed_columns
                .get(column)
                .cloned()
                .unwrap_or_else(|| column.clone())
        })
        .collect::<Vec<_>>();
    if effective_primary.is_empty() {
        statements.push(format!(
            "ALTER TABLE {quoted_table} ADD PRIMARY KEY ({})",
            quote_identifier(&table.primary_key)?
        ));
        changes.push(SchemaSyncChange {
            table: table.table_name.clone(),
            object: table.primary_key.clone(),
            kind: SchemaSyncChangeKind::AddedPrimaryKey,
        });
    } else if effective_primary != desired_primary {
        return Err(BaseError::DatabaseInitFailed(format!(
            "表 {} 的主键 {:?} 与声明 {:?} 不一致，拒绝自动修改",
            table.table_name, effective_primary, desired_primary
        )));
    }

    let effective_indexes = existing
        .indexes
        .iter()
        .cloned()
        .map(|mut index| {
            for column in &mut index.columns {
                if let Some(renamed) = renamed_columns.get(column) {
                    *column = renamed.clone();
                }
            }
            index
        })
        .collect::<Vec<_>>();
    for desired in desired_indexes(table)? {
        if let Some(existing_named) = effective_indexes
            .iter()
            .find(|index| index.name == desired.name)
        {
            if existing_named.unique == desired.unique && existing_named.columns == desired.columns
            {
                continue;
            }
            return Err(BaseError::DatabaseInitFailed(format!(
                "表 {} 的索引 {} 已存在但定义不同，拒绝自动修改",
                table.table_name, desired.name
            )));
        }
        let already_exists = effective_indexes.iter().any(|index| {
            index.columns == desired.columns && (index.unique == desired.unique || index.unique)
        });
        if already_exists {
            continue;
        }
        let columns = desired
            .columns
            .iter()
            .map(|column| quote_identifier(column))
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");
        let kind = if desired.unique {
            "UNIQUE INDEX"
        } else {
            "INDEX"
        };
        statements.push(format!(
            "ALTER TABLE {quoted_table} ADD {kind} {} ({columns})",
            quote_identifier(&desired.name)?
        ));
        changes.push(SchemaSyncChange {
            table: table.table_name.clone(),
            object: desired.name.clone(),
            kind: SchemaSyncChangeKind::AddedIndex,
        });
        if desired.unique {
            preflight.push(SchemaPreflightCheck::UniqueIndex {
                object: desired.name,
                columns: desired
                    .columns
                    .into_iter()
                    .map(|column| existing_column_name(&column, &renamed_columns))
                    .collect(),
            });
        }
    }

    for check in &table.checks {
        if let Some(existing_check) = existing.checks.iter().find(|item| item.name == check.name) {
            if normalize_check_expression(&existing_check.expression)
                == normalize_check_expression(&check.expression)
            {
                continue;
            }
            return Err(BaseError::DatabaseInitFailed(format!(
                "表 {} 的 CHECK {} 已存在但表达式不同，拒绝覆盖: actual={}, expected={}",
                table.table_name,
                check.name,
                normalize_check_expression(&existing_check.expression),
                normalize_check_expression(&check.expression)
            )));
        }
        if existing.checks.iter().any(|item| {
            normalize_check_expression(&item.expression)
                == normalize_check_expression(&check.expression)
        }) {
            continue;
        }
        statements.push(format!(
            "ALTER TABLE {quoted_table} ADD {}",
            render_check(check)?
        ));
        changes.push(SchemaSyncChange {
            table: table.table_name.clone(),
            object: check.name.clone(),
            kind: SchemaSyncChangeKind::AddedCheck,
        });
        preflight.push(SchemaPreflightCheck::Check {
            object: check.name.clone(),
            expression: expression_for_existing_schema(&check.expression, &renamed_columns),
        });
    }

    for foreign_key in &table.foreign_keys {
        let existing_columns = foreign_key
            .columns
            .iter()
            .map(|column| existing_column_name(column, &renamed_columns))
            .collect::<Vec<_>>();
        if let Some(existing_foreign_key) = existing
            .foreign_keys
            .iter()
            .find(|item| item.name == foreign_key.name)
        {
            if foreign_key_matches(existing_foreign_key, foreign_key, &renamed_columns) {
                continue;
            }
            return Err(BaseError::DatabaseInitFailed(format!(
                "表 {} 的外键 {} 已存在但定义不同，拒绝覆盖",
                table.table_name, foreign_key.name
            )));
        }
        if existing
            .foreign_keys
            .iter()
            .any(|item| foreign_key_matches(item, foreign_key, &renamed_columns))
        {
            continue;
        }
        statements.push(format!(
            "ALTER TABLE {quoted_table} ADD {}",
            render_foreign_key(foreign_key)?
        ));
        changes.push(SchemaSyncChange {
            table: table.table_name.clone(),
            object: foreign_key.name.clone(),
            kind: SchemaSyncChangeKind::AddedForeignKey,
        });
        preflight.push(SchemaPreflightCheck::ForeignKey {
            object: foreign_key.name.clone(),
            columns: existing_columns,
            referenced_table: foreign_key.referenced_table.clone(),
            referenced_columns: foreign_key.referenced_columns.clone(),
        });
    }

    Ok(TableSyncPlan {
        statements,
        changes,
        preflight,
    })
}
