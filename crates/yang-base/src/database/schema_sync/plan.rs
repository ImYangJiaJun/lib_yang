use super::dialect::SchemaDialect;
use super::driver::{SchemaConnection, SchemaDriver};
use super::model::{
    existing_column_name, foreign_key_matches, ExistingTableSchema, PlanStep, SchemaPreflightCheck,
    TableSyncPlan,
};
use super::{SchemaSyncChange, SchemaSyncChangeKind};
use crate::error::BaseError;
use crate::table::{FieldConfig, FieldType, SchemaIssueKind, TableDefinition};
use std::collections::BTreeMap;

pub(super) async fn plan_locked(
    driver: SchemaDriver,
    connection: &mut SchemaConnection<'_>,
    definitions: &[&TableDefinition],
) -> Result<Vec<TableSyncPlan>, BaseError> {
    let mut plans = Vec::with_capacity(definitions.len());
    for definition in definitions {
        let existing = driver.introspect(connection, definition).await?;
        let plan = driver.plan_changes(definition, &existing)?;
        if plan.steps.len() != plan.changes.len() {
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

/// MySQL 兼容入口：既有单元测试直接调用本函数。
#[cfg(all(test, feature = "mysql"))]
pub(crate) fn plan_table_sync(
    definition: &TableDefinition,
    existing: &ExistingTableSchema,
) -> Result<TableSyncPlan, BaseError> {
    plan_table_sync_with(SchemaDialect::MySql, definition, existing)
}

/// 按方言计算单表变更计划。
pub(crate) fn plan_table_sync_with(
    dialect: SchemaDialect,
    definition: &TableDefinition,
    existing: &ExistingTableSchema,
) -> Result<TableSyncPlan, BaseError> {
    let config = definition.shared_config();
    let table = config.as_ref();
    dialect.validate_table_config(table)?;
    if !existing.exists {
        let mut statements = Vec::new();
        let mut changes = Vec::new();
        let mut steps = Vec::new();
        let create_statements = dialect.render_create_table_statements(table, false)?;
        push_step(
            &mut statements,
            &mut steps,
            &mut changes,
            create_statements,
            SchemaSyncChange {
                table: table.table_name.clone(),
                object: table.table_name.clone(),
                kind: SchemaSyncChangeKind::CreatedTable,
            },
        );
        for foreign_key in &table.foreign_keys {
            let statement = dialect.render_add_foreign_key(table, foreign_key)?;
            push_step(
                &mut statements,
                &mut steps,
                &mut changes,
                vec![statement],
                SchemaSyncChange {
                    table: table.table_name.clone(),
                    object: foreign_key.name.clone(),
                    kind: SchemaSyncChangeKind::AddedForeignKey,
                },
            );
        }
        return Ok(TableSyncPlan {
            statements,
            changes,
            steps,
            preflight: Vec::new(),
        });
    }

    let mut statements = Vec::new();
    let mut changes = Vec::new();
    let mut steps = Vec::new();
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
            let statement = dialect.render_rename_column(table, legacy_name, &field.name)?;
            push_step(
                &mut statements,
                &mut steps,
                &mut changes,
                vec![statement],
                SchemaSyncChange {
                    table: table.table_name.clone(),
                    object: format!("{legacy_name}->{}", field.name),
                    kind: SchemaSyncChangeKind::RenamedColumn,
                },
            );
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
        let statement = dialect.render_add_column(table, field)?;
        push_step(
            &mut statements,
            &mut steps,
            &mut changes,
            vec![statement],
            SchemaSyncChange {
                table: table.table_name.clone(),
                object: field.name.clone(),
                kind: SchemaSyncChangeKind::AddedColumn,
            },
        );
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
    let report = dialect.validate_schema(table, &effective_columns);
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
                        .map(|column| column.data_type.as_str())
                        .unwrap_or_default();
                    dialect.is_unsafe_type_change(&field.field_type, data_type)
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
        let quoted_existing = dialect.quote_identifier(&existing_name)?;
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
                        .map(|value| dialect.quote_string(value))
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
        let kinds = issues.iter().map(|issue| issue.kind).collect::<Vec<_>>();
        let column_statements =
            dialect.render_modify_column(table, &existing_name, field, &kinds)?;
        push_step(
            &mut statements,
            &mut steps,
            &mut changes,
            column_statements,
            SchemaSyncChange {
                table: table.table_name.clone(),
                object: field.name.clone(),
                kind: SchemaSyncChangeKind::ModifiedColumn,
            },
        );
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
        let statement = dialect.render_add_primary_key(table)?;
        push_step(
            &mut statements,
            &mut steps,
            &mut changes,
            vec![statement],
            SchemaSyncChange {
                table: table.table_name.clone(),
                object: table.primary_key.clone(),
                kind: SchemaSyncChangeKind::AddedPrimaryKey,
            },
        );
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
    for desired in dialect.desired_indexes(table)? {
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
        let statement = dialect.render_add_index(table, &desired)?;
        push_step(
            &mut statements,
            &mut steps,
            &mut changes,
            vec![statement],
            SchemaSyncChange {
                table: table.table_name.clone(),
                object: desired.name.clone(),
                kind: SchemaSyncChangeKind::AddedIndex,
            },
        );
        if desired.unique {
            // 新加的列还没有任何数据，不可能有重复值——跳过 preflight 检查，
            // 否则 preflight SQL 会引用尚未添加的列，导致 "Unknown column" 报错。
            let added_columns: std::collections::BTreeSet<&str> = changes
                .iter()
                .filter(|change| change.kind == SchemaSyncChangeKind::AddedColumn)
                .map(|change| change.object.as_str())
                .collect();
            let all_new = desired
                .columns
                .iter()
                .all(|column| added_columns.contains(column.as_str()));
            if !all_new {
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
    }

    for check in &table.checks {
        if let Some(existing_check) = existing.checks.iter().find(|item| item.name == check.name) {
            if dialect.normalize_check_expression(&existing_check.expression)
                == dialect.normalize_check_expression(&check.expression)
            {
                continue;
            }
            return Err(BaseError::DatabaseInitFailed(format!(
                "表 {} 的 CHECK {} 已存在但表达式不同，拒绝覆盖: actual={}, expected={}",
                table.table_name,
                check.name,
                dialect.normalize_check_expression(&existing_check.expression),
                dialect.normalize_check_expression(&check.expression)
            )));
        }
        if existing.checks.iter().any(|item| {
            dialect.normalize_check_expression(&item.expression)
                == dialect.normalize_check_expression(&check.expression)
        }) {
            continue;
        }
        let statement = dialect.render_add_check(table, check)?;
        push_step(
            &mut statements,
            &mut steps,
            &mut changes,
            vec![statement],
            SchemaSyncChange {
                table: table.table_name.clone(),
                object: check.name.clone(),
                kind: SchemaSyncChangeKind::AddedCheck,
            },
        );
        preflight.push(SchemaPreflightCheck::Check {
            object: check.name.clone(),
            expression: dialect.expression_for_existing_schema(&check.expression, &renamed_columns),
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
        let statement = dialect.render_add_foreign_key(table, foreign_key)?;
        push_step(
            &mut statements,
            &mut steps,
            &mut changes,
            vec![statement],
            SchemaSyncChange {
                table: table.table_name.clone(),
                object: foreign_key.name.clone(),
                kind: SchemaSyncChangeKind::AddedForeignKey,
            },
        );
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
        steps,
        preflight,
    })
}

fn push_step(
    statements: &mut Vec<String>,
    steps: &mut Vec<PlanStep>,
    changes: &mut Vec<SchemaSyncChange>,
    step_statements: Vec<String>,
    change: SchemaSyncChange,
) {
    statements.extend(step_statements.iter().cloned());
    steps.push(PlanStep {
        statements: step_statements,
        change: change.clone(),
    });
    changes.push(change);
}
