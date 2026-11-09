use super::dialect::SchemaDialect;
use super::driver::{SchemaConnection, SchemaDriver};
use super::model::{
    existing_column_name, foreign_key_matches, DesiredIndex, ExistingIndex, ExistingTableSchema,
    PlanStep, SchemaPreflightCheck, TableSyncPlan,
};
use super::{SchemaSyncChange, SchemaSyncChangeKind};
use crate::error::BaseError;
use crate::table::{FieldConfig, FieldType, SchemaIssueKind, TableDefinition};
use std::collections::{BTreeMap, BTreeSet};

pub(super) async fn plan_locked(
    driver: SchemaDriver,
    connection: &mut SchemaConnection<'_>,
    definitions: &[&TableDefinition],
) -> Result<Vec<TableSyncPlan>, BaseError> {
    #[cfg(feature = "postgres")]
    ensure_nulls_not_distinct_supported(driver, connection, definitions).await?;
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
    // 本批新增列按表汇总：外键预检可能引用同一批另一张表里尚未创建的列。
    let added_columns_by_table = plans
        .iter()
        .flat_map(|plan| plan.changes.iter())
        .filter(|change| change.kind == SchemaSyncChangeKind::AddedColumn)
        .fold(
            BTreeMap::<String, BTreeSet<String>>::new(),
            |mut map, change| {
                map.entry(change.table.clone())
                    .or_default()
                    .insert(change.object.clone());
                map
            },
        );
    for plan in &mut plans {
        plan.preflight.retain(|check| {
            let SchemaPreflightCheck::ForeignKey {
                referenced_table,
                referenced_columns,
                ..
            } = check
            else {
                return true;
            };
            // 被引用列属于同一批另一张表的新增列时，只读预检同样无法执行。
            !added_columns_by_table
                .get(referenced_table)
                .is_some_and(|columns| {
                    referenced_columns
                        .iter()
                        .any(|column| columns.contains(column))
                })
        });
    }
    Ok(plans)
}

/// 声明了 `NULLS NOT DISTINCT` 时校验 PostgreSQL 版本（15+）。
///
/// 渲染层无法得知服务端版本；这里在任何计划/预检/同步开始前统一门控，
/// 避免把低版本不支持的 DDL 发给数据库。
#[cfg(feature = "postgres")]
async fn ensure_nulls_not_distinct_supported(
    driver: SchemaDriver,
    connection: &mut SchemaConnection<'_>,
    definitions: &[&TableDefinition],
) -> Result<(), BaseError> {
    if driver != SchemaDriver::PostgreSql {
        return Ok(());
    }
    let declared = definitions.iter().any(|definition| {
        definition
            .shared_config()
            .unique_indexes
            .iter()
            .any(|index| index.nulls_not_distinct)
    });
    if !declared {
        return Ok(());
    }
    match connection {
        SchemaConnection::PostgreSql(connection) => {
            let version = super::pg::server_version_num(connection).await?;
            if version < super::pg::PG15_VERSION_NUM {
                return Err(BaseError::DatabaseInitFailed(format!(
                    "NULLS NOT DISTINCT 需要 PostgreSQL 15+（当前 server_version_num={version}）"
                )));
            }
            Ok(())
        }
        #[allow(unreachable_patterns)]
        _ => Err(BaseError::DatabaseInitFailed(
            "schema driver 与数据库连接后端不一致".to_string(),
        )),
    }
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
            let foreign_key_statements = dialect.render_add_foreign_key(table, foreign_key)?;
            // PostgreSQL 的第 2 条（VALIDATE CONSTRAINT）推迟到外键事务提交后，
            // 避免 ADD 的 ShareRowExclusiveLock 持有到 VALIDATE 结束；MySQL 只有 1 条语句。
            push_step_deferred(
                &mut statements,
                &mut steps,
                &mut changes,
                foreign_key_statements,
                1,
                SchemaSyncChange {
                    table: table.table_name.clone(),
                    object: foreign_key.name.clone(),
                    kind: SchemaSyncChangeKind::AddedForeignKey,
                },
                false,
            );
        }
        if dialect.supports_column_comments() {
            let mut fields: Vec<&FieldConfig> = table.fields.values().collect();
            fields.sort_by(|left, right| left.name.cmp(&right.name));
            for field in fields {
                if field.comment.is_none() {
                    continue;
                }
                push_step(
                    &mut statements,
                    &mut steps,
                    &mut changes,
                    vec![dialect.render_comment_on_column(table, field)?],
                    SchemaSyncChange {
                        table: table.table_name.clone(),
                        object: field.name.clone(),
                        kind: SchemaSyncChangeKind::CommentedColumn,
                    },
                );
            }
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
    for field in &fields {
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
            && field.generated.is_none()
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

    // 生成列表达式无法安全地自动改写；存在性/表达式漂移一律 fail-closed。
    for field in &fields {
        // 本次刚新增（或由旧列改名而来）的列没有既存生成属性可比；
        // 其生成声明已由上面的 ADD COLUMN 语句落地，不应被误判为漂移。
        let existed_before = existing
            .columns
            .iter()
            .any(|column| column.name == field.name)
            || field.renamed_from.as_ref().is_some_and(|legacy_name| {
                existing
                    .columns
                    .iter()
                    .any(|column| column.name == *legacy_name)
            });
        if !existed_before {
            continue;
        }
        let actual = existing.generated_columns.get(&field.name).or_else(|| {
            field
                .renamed_from
                .as_ref()
                .and_then(|legacy| existing.generated_columns.get(legacy))
        });
        match (&field.generated, actual) {
            (None, None) => {}
            (Some(expected), Some(actual))
                if dialect.normalize_check_expression(expected)
                    == dialect.normalize_check_expression(actual) => {}
            (Some(_), Some(_)) => {
                return Err(BaseError::DatabaseInitFailed(format!(
                    "表 {} 的生成列 {} 表达式与声明不同，拒绝自动修改",
                    table.table_name, field.name
                )));
            }
            (Some(_), None) | (None, Some(_)) => {
                return Err(BaseError::DatabaseInitFailed(format!(
                    "表 {} 的字段 {} 的生成列属性与声明不同，拒绝自动修改",
                    table.table_name, field.name
                )));
            }
        }
    }

    let effective = existing.with_renamed_columns(&renamed_columns);
    let report = dialect.validate_schema(table, &effective);
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
                    let data_type = effective
                        .columns
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
        let existing_collation = effective.collations.get(field_name).map(String::as_str);
        let column_statements = dialect.render_modify_column(
            table,
            &existing_name,
            field,
            &kinds,
            existing_collation,
        )?;
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

    // 列注释只在声明了 comment 时同步；未声明不回删数据库已有注释。
    if dialect.supports_column_comments() {
        for field in &fields {
            let Some(comment) = &field.comment else {
                continue;
            };
            if effective.comments.get(&field.name) == Some(comment) {
                continue;
            }
            let statement = dialect.render_comment_on_column(table, field)?;
            push_step(
                &mut statements,
                &mut steps,
                &mut changes,
                vec![statement],
                SchemaSyncChange {
                    table: table.table_name.clone(),
                    object: field.name.clone(),
                    kind: SchemaSyncChangeKind::CommentedColumn,
                },
            );
        }
    }

    let desired_primary = vec![table.primary_key.clone()];
    let effective_primary = effective.primary_key.clone();
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

    // 本批新增的列：这些列在 DDL 落地前不存在，针对它们的预检 SQL 在只读
    // preflight（MySQL 预检先于 DDL、PostgreSQL `preflight_table_definitions`）
    // 中会引用尚不存在的列。索引/CHECK/外键的预检据此跳过。
    let added_columns = changes
        .iter()
        .filter(|change| change.kind == SchemaSyncChangeKind::AddedColumn)
        .map(|change| change.object.clone())
        .collect::<std::collections::BTreeSet<String>>();

    for desired in dialect.desired_indexes(table)? {
        let same_definition =
            |index: &ExistingIndex| plain_index_definition_matches(index, &desired);
        // 需要重建的 INVALID 索引名（PostgreSQL 并发建索引失败会留下它）。
        let mut rebuild = None;
        if let Some(existing_named) = effective
            .indexes
            .iter()
            .find(|index| index.name == desired.name)
        {
            if !same_definition(existing_named) {
                return Err(BaseError::DatabaseInitFailed(format!(
                    "表 {} 的索引 {} 已存在但定义不同，拒绝自动修改",
                    table.table_name, desired.name
                )));
            }
            if existing_named.valid {
                continue;
            }
            rebuild = Some(existing_named.name.clone());
        } else if let Some(matching) = effective
            .indexes
            .iter()
            .find(|index| plain_index_covers(index, &desired))
        {
            if matching.valid {
                continue;
            }
            rebuild = Some(matching.name.clone());
        }
        let concurrent = dialect.add_index_is_concurrent();
        let mut step_statements = Vec::new();
        if let Some(invalid_name) = &rebuild {
            step_statements.push(dialect.render_drop_index_concurrently(table, invalid_name)?);
        }
        step_statements.push(dialect.render_add_index(table, &desired)?);
        let is_rebuild = rebuild.is_some();
        push_step_with(
            &mut statements,
            &mut steps,
            &mut changes,
            step_statements,
            SchemaSyncChange {
                table: table.table_name.clone(),
                object: desired.name.clone(),
                kind: SchemaSyncChangeKind::AddedIndex,
            },
            concurrent,
        );
        if is_rebuild {
            tracing::warn!(
                table = %table.table_name,
                index = %desired.name,
                "检测到 INVALID 索引，已并发重建"
            );
        }
        if desired.unique {
            // 新增列还没有任何数据，不可能有重复值——跳过 preflight 检查，
            // 否则 preflight SQL 会引用尚未添加的列，导致 "列不存在" 报错。
            // 只读预检先于 DDL：只要索引引用任一本次新增列，整条预检 SQL 都不可执行，
            // 因此跳过整项；sync 路径仍由 DDL 后的约束校验兜底。
            let any_new = desired
                .columns
                .iter()
                .any(|column| added_columns.contains(column));
            if !any_new {
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

    for check in dialect.desired_checks(table)? {
        if let Some(existing_check) = existing.checks.iter().find(|item| item.name == check.name) {
            if dialect.normalize_check_expression(&existing_check.expression)
                == dialect.normalize_check_expression(&check.expression)
            {
                // 同名同表达式但仍是 NOT VALID：只补一条事务提交后的 VALIDATE。
                if !existing_check.validated {
                    push_validate_check(
                        &mut statements,
                        &mut steps,
                        &mut changes,
                        dialect,
                        table,
                        &check.name,
                    )?;
                }
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
        if let Some(matching) = existing.checks.iter().find(|item| {
            dialect.normalize_check_expression(&item.expression)
                == dialect.normalize_check_expression(&check.expression)
        }) {
            if !matching.validated {
                push_validate_check(
                    &mut statements,
                    &mut steps,
                    &mut changes,
                    dialect,
                    table,
                    &matching.name,
                )?;
            }
            continue;
        }
        let check_statements = dialect.render_add_check(table, &check)?;
        // PostgreSQL 的第 2 条（VALIDATE CONSTRAINT）推迟到事务提交后执行，
        // 避免 ACCESS EXCLUSIVE 锁持有到 COMMIT；MySQL 只有 1 条语句。
        push_step_deferred(
            &mut statements,
            &mut steps,
            &mut changes,
            check_statements,
            1,
            SchemaSyncChange {
                table: table.table_name.clone(),
                object: check.name.clone(),
                kind: SchemaSyncChangeKind::AddedCheck,
            },
            false,
        );
        // 与唯一索引一致：CHECK 表达式只要引用任一本次新增列，只读预检就
        // 无法看到该列，必然以 "列不存在" 失败；新增列尚无历史数据需要预检，
        // 直接跳过整项。sync 路径仍由 NOT VALID + deferred VALIDATE 兜底。
        if !expression_references_newly_added_column(&check.expression, &added_columns) {
            preflight.push(SchemaPreflightCheck::Check {
                object: check.name.clone(),
                expression: dialect
                    .expression_for_existing_schema(&check.expression, &renamed_columns),
            });
        }
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
                // 定义一致但仍是 NOT VALID：只补一条外键事务提交后的 VALIDATE。
                if !existing_foreign_key.validated {
                    push_validate_foreign_key(
                        &mut statements,
                        &mut steps,
                        &mut changes,
                        dialect,
                        table,
                        &foreign_key.name,
                    )?;
                }
                continue;
            }
            return Err(BaseError::DatabaseInitFailed(format!(
                "表 {} 的外键 {} 已存在但定义不同，拒绝覆盖",
                table.table_name, foreign_key.name
            )));
        }
        if let Some(matching) = existing
            .foreign_keys
            .iter()
            .find(|item| foreign_key_matches(item, foreign_key, &renamed_columns))
        {
            if !matching.validated {
                push_validate_foreign_key(
                    &mut statements,
                    &mut steps,
                    &mut changes,
                    dialect,
                    table,
                    &matching.name,
                )?;
            }
            continue;
        }
        let foreign_key_statements = dialect.render_add_foreign_key(table, foreign_key)?;
        // 与 CHECK 一致：ADD ... NOT VALID 留在外键事务内，VALIDATE 推迟到提交后。
        push_step_deferred(
            &mut statements,
            &mut steps,
            &mut changes,
            foreign_key_statements,
            1,
            SchemaSyncChange {
                table: table.table_name.clone(),
                object: foreign_key.name.clone(),
                kind: SchemaSyncChangeKind::AddedForeignKey,
            },
            false,
        );
        // 外键本地列只要包含任一本次新增列，只读预检 SQL 就会引用尚不存在的列；
        // 跳过整项。新列尚无数据（或仅默认值），无需在 DDL 前扫描。同批其他
        // 表的新增被引用列由 `plan_locked` 的跨表视图剔除。
        let any_new = foreign_key
            .columns
            .iter()
            .any(|column| added_columns.contains(column));
        if !any_new {
            preflight.push(SchemaPreflightCheck::ForeignKey {
                object: foreign_key.name.clone(),
                columns: existing_columns,
                referenced_table: foreign_key.referenced_table.clone(),
                referenced_columns: foreign_key.referenced_columns.clone(),
            });
        }
    }

    Ok(TableSyncPlan {
        statements,
        changes,
        steps,
        preflight,
    })
}

/// 声明的普通索引是否与既有索引定义完全一致。
///
/// 部分索引（`predicate`）与表达式索引（`expressions`）的约束语义不同于普通
/// 列索引，不能复用同名声明，必须 fail-closed。
fn plain_index_definition_matches(existing: &ExistingIndex, desired: &DesiredIndex) -> bool {
    existing.predicate.is_none()
        && existing.expressions.is_empty()
        && existing.unique == desired.unique
        && existing.nulls_not_distinct == desired.nulls_not_distinct
        && existing.columns == desired.columns
}

/// 既有普通索引能否满足声明索引（唯一索引可满足普通索引需求）。
///
/// `NULLS NOT DISTINCT` 比默认的 `NULLS DISTINCT` 更严格：严格索引可以满足
/// 宽松声明，反之不行。
fn plain_index_covers(existing: &ExistingIndex, desired: &DesiredIndex) -> bool {
    existing.predicate.is_none()
        && existing.expressions.is_empty()
        && existing.columns == desired.columns
        && (existing.unique == desired.unique || existing.unique)
        && (!desired.nulls_not_distinct || existing.nulls_not_distinct)
}

/// CHECK 表达式是否引用了本批新增列。
///
/// 只读预检先于 DDL 执行，引用任一新增列都会让预检 SQL 失败，因此任一命中即跳过。
/// 未引用任何新增列（含纯常量表达式）时返回 false，保留既有预检行为。
fn expression_references_newly_added_column(
    expression: &str,
    added_columns: &std::collections::BTreeSet<String>,
) -> bool {
    added_columns
        .iter()
        .any(|column| expression_references_column(expression, column))
}

/// 判断表达式是否以标识符形式引用了某列。
///
/// 遵循 PostgreSQL 标识符规则：未加双引号的标识符大小写不敏感（折叠为小写），
/// 双引号标识符精确匹配；单引号字符串字面量内的文本不参与匹配。
pub(super) fn expression_references_column(expression: &str, column: &str) -> bool {
    if column.is_empty() {
        return false;
    }
    let is_identifier = |character: char| character.is_alphanumeric() || character == '_';
    let characters = expression.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < characters.len() {
        let character = characters[index];
        if character == '\'' {
            // 字符串字面量：跳过直到配对的单引号，'' 是字面量内的转义引号。
            index += 1;
            while index < characters.len() {
                if characters[index] == '\'' {
                    if characters.get(index + 1) == Some(&'\'') {
                        index += 2;
                        continue;
                    }
                    index += 1;
                    break;
                }
                index += 1;
            }
            continue;
        }
        if character == '"' {
            // 双引号标识符：精确匹配，"" 是标识符内的转义双引号。
            let mut identifier = String::new();
            index += 1;
            while index < characters.len() {
                if characters[index] == '"' {
                    if characters.get(index + 1) == Some(&'"') {
                        identifier.push('"');
                        index += 2;
                        continue;
                    }
                    index += 1;
                    break;
                }
                identifier.push(characters[index]);
                index += 1;
            }
            if identifier == column {
                return true;
            }
            continue;
        }
        if is_identifier(character) {
            let start = index;
            while index < characters.len() && is_identifier(characters[index]) {
                index += 1;
            }
            let word = characters[start..index].iter().collect::<String>();
            if word.eq_ignore_ascii_case(column) {
                return true;
            }
            continue;
        }
        index += 1;
    }
    false
}

/// 自愈：为已存在但未验证的 CHECK 生成一条仅含 deferred `VALIDATE` 的步骤。
fn push_validate_check(
    statements: &mut Vec<String>,
    steps: &mut Vec<PlanStep>,
    changes: &mut Vec<SchemaSyncChange>,
    dialect: SchemaDialect,
    table: &crate::table::TableConfig,
    constraint_name: &str,
) -> Result<(), BaseError> {
    let validate = dialect.render_validate_constraint(table, constraint_name)?;
    push_step_deferred(
        statements,
        steps,
        changes,
        vec![validate],
        0,
        SchemaSyncChange {
            table: table.table_name.clone(),
            object: constraint_name.to_string(),
            kind: SchemaSyncChangeKind::ValidatedCheck,
        },
        false,
    );
    Ok(())
}

/// 自愈：为已存在但未验证的外键生成一条仅含 deferred `VALIDATE` 的步骤。
fn push_validate_foreign_key(
    statements: &mut Vec<String>,
    steps: &mut Vec<PlanStep>,
    changes: &mut Vec<SchemaSyncChange>,
    dialect: SchemaDialect,
    table: &crate::table::TableConfig,
    constraint_name: &str,
) -> Result<(), BaseError> {
    let validate = dialect.render_validate_constraint(table, constraint_name)?;
    push_step_deferred(
        statements,
        steps,
        changes,
        vec![validate],
        0,
        SchemaSyncChange {
            table: table.table_name.clone(),
            object: constraint_name.to_string(),
            kind: SchemaSyncChangeKind::ValidatedForeignKey,
        },
        false,
    );
    Ok(())
}

fn push_step(
    statements: &mut Vec<String>,
    steps: &mut Vec<PlanStep>,
    changes: &mut Vec<SchemaSyncChange>,
    step_statements: Vec<String>,
    change: SchemaSyncChange,
) {
    push_step_with(statements, steps, changes, step_statements, change, false);
}

fn push_step_with(
    statements: &mut Vec<String>,
    steps: &mut Vec<PlanStep>,
    changes: &mut Vec<SchemaSyncChange>,
    step_statements: Vec<String>,
    change: SchemaSyncChange,
    concurrent: bool,
) {
    let deferred_from = step_statements.len();
    push_step_deferred(
        statements,
        steps,
        changes,
        step_statements,
        deferred_from,
        change,
        concurrent,
    );
}

/// 把步骤语句拆成事务内与提交后（deferred）两段。
///
/// `deferred_from` 起（含）的语句进入 `deferred_statements`；`plan.statements`
/// 始终保存完整 SQL 文本以兼容既有单测。
fn push_step_deferred(
    statements: &mut Vec<String>,
    steps: &mut Vec<PlanStep>,
    changes: &mut Vec<SchemaSyncChange>,
    step_statements: Vec<String>,
    deferred_from: usize,
    change: SchemaSyncChange,
    concurrent: bool,
) {
    let deferred_from = deferred_from.min(step_statements.len());
    statements.extend(step_statements.iter().cloned());
    let deferred_statements = step_statements[deferred_from..].to_vec();
    let inline_statements = step_statements[..deferred_from].to_vec();
    steps.push(PlanStep {
        statements: inline_statements,
        deferred_statements,
        change: change.clone(),
        concurrent,
    });
    changes.push(change);
}
