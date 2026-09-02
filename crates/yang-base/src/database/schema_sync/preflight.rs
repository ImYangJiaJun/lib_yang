use super::model::{SchemaPreflightCheck, TableSyncPlan};
use super::render::quote_identifier;
use super::SchemaDataViolation;
use crate::error::BaseError;
use crate::table::{TableConfig, TableDefinition};
use sqlx::mysql::MySqlConnection;

pub(super) async fn preflight_locked(
    connection: &mut MySqlConnection,
    definitions: &[&TableDefinition],
    plans: &[TableSyncPlan],
) -> Result<Vec<SchemaDataViolation>, BaseError> {
    let mut violations = Vec::new();
    for (definition, plan) in definitions.iter().zip(plans) {
        let table = definition.config();
        for check in &plan.preflight {
            let primary_keys = preflight_primary_keys(connection, table, check).await?;
            if !primary_keys.is_empty() {
                violations.push(SchemaDataViolation {
                    table: table.table_name.clone(),
                    object: match check {
                        SchemaPreflightCheck::ColumnPredicate { object, .. }
                        | SchemaPreflightCheck::Check { object, .. }
                        | SchemaPreflightCheck::ForeignKey { object, .. }
                        | SchemaPreflightCheck::UniqueIndex { object, .. } => object.clone(),
                    },
                    primary_keys,
                });
            }
        }
    }
    Ok(violations)
}

async fn preflight_primary_keys(
    connection: &mut MySqlConnection,
    table: &TableConfig,
    check: &SchemaPreflightCheck,
) -> Result<Vec<String>, BaseError> {
    let quoted_table = quote_identifier(&table.table_name)?;
    let quoted_primary = quote_identifier(&table.primary_key)?;
    let statement = match check {
        SchemaPreflightCheck::ColumnPredicate { predicate, .. } => format!(
            "SELECT CAST({quoted_primary} AS CHAR) FROM {quoted_table} \
             WHERE {predicate} ORDER BY {quoted_primary} LIMIT 20"
        ),
        SchemaPreflightCheck::Check { expression, .. } => format!(
            "SELECT CAST({quoted_primary} AS CHAR) FROM {quoted_table} \
             WHERE NOT ({expression}) ORDER BY {quoted_primary} LIMIT 20"
        ),
        SchemaPreflightCheck::ForeignKey {
            columns,
            referenced_table,
            referenced_columns,
            ..
        } => {
            let joins = columns
                .iter()
                .zip(referenced_columns)
                .map(|(column, referenced)| {
                    Ok(format!(
                        "source.{} = target.{}",
                        quote_identifier(column)?,
                        quote_identifier(referenced)?
                    ))
                })
                .collect::<Result<Vec<_>, BaseError>>()?
                .join(" AND ");
            let non_null = columns
                .iter()
                .map(|column| Ok(format!("source.{} IS NOT NULL", quote_identifier(column)?)))
                .collect::<Result<Vec<_>, BaseError>>()?
                .join(" AND ");
            let first_target = quote_identifier(&referenced_columns[0])?;
            format!(
                "SELECT CAST(source.{quoted_primary} AS CHAR) FROM {quoted_table} AS source \
                 LEFT JOIN {} AS target ON {joins} \
                 WHERE {non_null} AND target.{first_target} IS NULL \
                 ORDER BY source.{quoted_primary} LIMIT 20",
                quote_identifier(referenced_table)?
            )
        }
        SchemaPreflightCheck::UniqueIndex { columns, .. } => {
            let quoted_columns = columns
                .iter()
                .map(|column| quote_identifier(column))
                .collect::<Result<Vec<_>, _>>()?;
            let group_columns = quoted_columns.join(", ");
            let join = quoted_columns
                .iter()
                .map(|column| format!("source.{column} = duplicates.{column}"))
                .collect::<Vec<_>>()
                .join(" AND ");
            let non_null = quoted_columns
                .iter()
                .map(|column| format!("{column} IS NOT NULL"))
                .collect::<Vec<_>>()
                .join(" AND ");
            format!(
                "SELECT CAST(source.{quoted_primary} AS CHAR) FROM {quoted_table} AS source \
                 INNER JOIN (SELECT {group_columns} FROM {quoted_table} \
                 WHERE {non_null} GROUP BY {group_columns} HAVING COUNT(*) > 1) AS duplicates \
                 ON {join} ORDER BY source.{quoted_primary} LIMIT 20"
            )
        }
    };
    sqlx::query_scalar::<_, String>(&statement)
        .fetch_all(&mut *connection)
        .await
        .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))
}

pub(super) fn format_violations(violations: &[SchemaDataViolation]) -> String {
    let details = violations
        .iter()
        .map(|violation| {
            format!(
                "table={}, object={}, primary_keys=[{}]",
                violation.table,
                violation.object,
                violation.primary_keys.join(",")
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    format!("旧数据阻止数据库表结构更新，请人工处理后重试: {details}")
}
