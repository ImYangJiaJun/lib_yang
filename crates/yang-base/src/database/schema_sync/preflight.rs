use super::dialect::SchemaDialect;
use super::driver::SchemaConnection;
use super::model::{SchemaPreflightCheck, TableSyncPlan};
use super::SchemaDataViolation;
use crate::error::BaseError;
use crate::table::{TableConfig, TableDefinition};

pub(super) async fn preflight_locked(
    dialect: SchemaDialect,
    connection: &mut SchemaConnection<'_>,
    definitions: &[&TableDefinition],
    plans: &[TableSyncPlan],
) -> Result<Vec<SchemaDataViolation>, BaseError> {
    let mut violations = Vec::new();
    for (definition, plan) in definitions.iter().zip(plans) {
        let config = definition.shared_config();
        let table = config.as_ref();
        for check in &plan.preflight {
            let primary_keys = preflight_primary_keys(dialect, connection, table, check).await?;
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
    dialect: SchemaDialect,
    connection: &mut SchemaConnection<'_>,
    table: &TableConfig,
    check: &SchemaPreflightCheck,
) -> Result<Vec<String>, BaseError> {
    let quoted_table = dialect.quote_table(&table.table_name)?;
    let primary_expr = row_identifier(dialect, table, None)?;
    let source_primary_expr = row_identifier(dialect, table, Some("source"))?;
    let statement = match check {
        SchemaPreflightCheck::ColumnPredicate { predicate, .. } => format!(
            "SELECT {primary_expr} FROM {quoted_table} \
             WHERE {predicate} ORDER BY {primary_expr} LIMIT 20"
        ),
        SchemaPreflightCheck::Check { expression, .. } => format!(
            "SELECT {primary_expr} FROM {quoted_table} \
             WHERE NOT ({expression}) ORDER BY {primary_expr} LIMIT 20"
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
                        dialect.quote_identifier(column)?,
                        dialect.quote_identifier(referenced)?
                    ))
                })
                .collect::<Result<Vec<_>, BaseError>>()?
                .join(" AND ");
            let non_null = columns
                .iter()
                .map(|column| {
                    Ok(format!(
                        "source.{} IS NOT NULL",
                        dialect.quote_identifier(column)?
                    ))
                })
                .collect::<Result<Vec<_>, BaseError>>()?
                .join(" AND ");
            let first_target = dialect.quote_identifier(&referenced_columns[0])?;
            format!(
                "SELECT {source_primary_expr} FROM {quoted_table} AS source \
                 LEFT JOIN {} AS target ON {joins} \
                 WHERE {non_null} AND target.{first_target} IS NULL \
                 ORDER BY {source_primary_expr} LIMIT 20",
                dialect.quote_table(referenced_table)?
            )
        }
        SchemaPreflightCheck::UniqueIndex { columns, .. } => {
            let quoted_columns = columns
                .iter()
                .map(|column| dialect.quote_identifier(column))
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
                "SELECT {source_primary_expr} FROM {quoted_table} AS source \
                 INNER JOIN (SELECT {group_columns} FROM {quoted_table} \
                 WHERE {non_null} GROUP BY {group_columns} HAVING COUNT(*) > 1) AS duplicates \
                 ON {join} ORDER BY {source_primary_expr} LIMIT 20"
            )
        }
    };
    connection.fetch_text_column(&statement).await
}

/// 构造可定位冲突行的表达式：主键列拼接；无主键表退化为字典序首个字段。
fn row_identifier(
    dialect: SchemaDialect,
    table: &TableConfig,
    qualifier: Option<&str>,
) -> Result<String, BaseError> {
    let columns: Vec<String> = if table.primary_keys.is_empty() {
        table.fields.keys().min().cloned().into_iter().collect()
    } else {
        table.primary_keys.clone()
    };
    if columns.is_empty() {
        return Ok("1".to_string());
    }
    let cast_target = dialect.cast_text_target();
    let prefix = qualifier
        .map(|value| format!("{value}."))
        .unwrap_or_default();
    let parts = columns
        .iter()
        .map(|column| {
            Ok(format!(
                "CAST({prefix}{} AS {cast_target})",
                dialect.quote_identifier(column)?
            ))
        })
        .collect::<Result<Vec<_>, BaseError>>()?;
    match dialect {
        #[cfg(feature = "mysql")]
        SchemaDialect::MySql => Ok(format!("CONCAT({})", parts.join(", "))),
        #[cfg(feature = "postgres")]
        SchemaDialect::PostgreSql => Ok(format!("concat_ws(',', {})", parts.join(", "))),
    }
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

#[cfg(all(test, feature = "postgres"))]
mod tests {
    use super::*;
    use crate::table::{Field, Table};

    #[test]
    fn row_identifier_covers_composite_and_no_primary_key() {
        let composite = Table::new("composite_rows")
            .fields([
                Field::bigint("tenant_id").required().primary_key(),
                Field::bigint("item_id").required().primary_key(),
            ])
            .build()
            .expect("复合主键表应有效");
        let label = row_identifier(
            SchemaDialect::PostgreSql,
            composite.shared_config().as_ref(),
            None,
        )
        .expect("复合主键行标识应可构造");
        assert!(label.contains("\"tenant_id\""), "{label}");
        assert!(label.contains("\"item_id\""), "{label}");

        let no_pk = Table::new("append_only")
            .fields([Field::string("name", 32).required()])
            .build()
            .expect("无主键表应有效");
        let label = row_identifier(
            SchemaDialect::PostgreSql,
            no_pk.shared_config().as_ref(),
            None,
        )
        .expect("无主键行标识应退化到首列");
        assert!(label.contains("\"name\""), "{label}");
    }
}
