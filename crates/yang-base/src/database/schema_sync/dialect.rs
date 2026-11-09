//! 声明式 schema 同步的方言渲染与类型兼容判定。
//!
//! 本模块只做纯字符串/纯逻辑计算，不接触数据库连接；MySQL 分支逐字节复用
//! [`super::render`] 的既有渲染，PostgreSQL 分支输出双引号标识符、无 `ENGINE`
//! 与内联 `KEY` 的 DDL。数据库访问由 [`super::driver`] 分派。

use super::model::DesiredIndex;
use crate::error::BaseError;
use crate::table::{
    CheckConfig, FieldConfig, FieldType, ForeignKeyConfig, SchemaColumn, SchemaIssueKind,
    SchemaValidationReport, TableConfig, TableDefinition,
};
#[cfg(feature = "postgres")]
use crate::table::{SchemaIssue, MAX_VARCHAR_LENGTH};
use std::collections::BTreeMap;

#[cfg(feature = "mysql")]
use super::render as mysql_render;

/// 目标数据库方言。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SchemaDialect {
    /// MySQL（历史行为，逐字节兼容）。
    #[cfg(feature = "mysql")]
    MySql,
    /// PostgreSQL。
    #[cfg(feature = "postgres")]
    PostgreSql,
}

impl SchemaDialect {
    /// 校验并转义单个标识符。
    pub(super) fn quote_identifier(self, identifier: &str) -> Result<String, BaseError> {
        let quoted = match self {
            #[cfg(feature = "mysql")]
            Self::MySql => yang_db::mysql::quote_identifier(identifier),
            #[cfg(feature = "postgres")]
            Self::PostgreSql => yang_db::postgres::quote_identifier(identifier),
        };
        quoted.map_err(|error| {
            BaseError::ConfigError(format!("非法数据库标识符 {identifier:?}: {error}"))
        })
    }

    /// 校验并转义表名（PostgreSQL 支持 `schema.table`）。
    pub(super) fn quote_table(self, table: &str) -> Result<String, BaseError> {
        match self {
            // MySQL 保持历史行为：表名只能是单段标识符。
            #[cfg(feature = "mysql")]
            Self::MySql => self.quote_identifier(table),
            #[cfg(feature = "postgres")]
            Self::PostgreSql => yang_db::postgres::quote_table(table).map_err(|error| {
                BaseError::ConfigError(format!("非法数据库表名 {table:?}: {error}"))
            }),
        }
    }

    /// 渲染字符串字面量。
    pub(super) fn quote_string(self, value: &str) -> String {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql => mysql_render::quote_string(value),
            #[cfg(feature = "postgres")]
            Self::PostgreSql => format!("'{}'", value.replace('\'', "''")),
        }
    }

    /// 预检 SQL 中把主键/列值转成文本的 CAST 目标类型。
    pub(super) fn cast_text_target(self) -> &'static str {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql => "CHAR",
            #[cfg(feature = "postgres")]
            Self::PostgreSql => "TEXT",
        }
    }

    /// 校验表定义；调用方在生成计划前统一调用。
    pub(super) fn validate_table_config(self, table: &TableConfig) -> Result<(), BaseError> {
        self.quote_table(&table.table_name)?;
        if table.fields.is_empty() {
            return Err(BaseError::ConfigError(format!(
                "表 {} 没有声明字段",
                table.table_name
            )));
        }
        let primary = table.fields.get(&table.primary_key).ok_or_else(|| {
            BaseError::ConfigError(format!(
                "表 {} 的主键字段 {} 未声明",
                table.table_name, table.primary_key
            ))
        })?;
        if !primary.required {
            return Err(BaseError::ConfigError(format!(
                "表 {} 的主键字段 {} 必须 required",
                table.table_name, table.primary_key
            )));
        }
        for field in table.fields.values() {
            self.quote_identifier(&field.name)?;
            if field.auto_increment
                && (field.name != table.primary_key
                    || !matches!(field.field_type, FieldType::Integer | FieldType::BigInt))
            {
                return Err(BaseError::ConfigError(format!(
                    "表 {} 的 auto_increment 仅允许用于整数主键字段: {}",
                    table.table_name, field.name
                )));
            }
            if field.auto_increment && field.default_value.is_some() {
                return Err(BaseError::ConfigError(format!(
                    "表 {} 的自增字段不能同时声明默认值: {}",
                    table.table_name, field.name
                )));
            }
            let _ = self.render_column(field)?;
        }
        let _ = self.desired_indexes(table)?;
        Ok(())
    }

    /// 对同名多份表定义生成签名，签名不同即报配置冲突。
    pub(super) fn table_signature(self, table: &TableConfig) -> Result<String, BaseError> {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql => mysql_render::render_create_table(table, true),
            #[cfg(feature = "postgres")]
            Self::PostgreSql => Ok(self.render_create_table_statements(table, true)?.join("; ")),
        }
    }

    /// 归一化并去重表定义。
    pub(super) fn normalize_definitions<'a>(
        self,
        definitions: &'a [&'a TableDefinition],
    ) -> Result<Vec<&'a TableDefinition>, BaseError> {
        let mut normalized: BTreeMap<&str, (&TableDefinition, String)> = BTreeMap::new();
        for definition in definitions {
            // `TableDefinition::config()` 仅在 mysql feature 下导出；这里统一走
            // `shared_config()`，让 PostgreSQL-only 构建也能读取同一份编译配置。
            let config = definition.shared_config();
            let table = config.as_ref();
            self.validate_table_config(table)?;
            let signature = self.table_signature(table)?;
            if let Some((_, existing_signature)) = normalized.get(definition.name()) {
                if existing_signature != &signature {
                    return Err(BaseError::ConfigError(format!(
                        "表配置冲突: {} 在多个模块中声明不同 schema",
                        table.table_name
                    )));
                }
                continue;
            }
            normalized.insert(definition.name(), (*definition, signature));
        }
        Ok(normalized
            .into_values()
            .map(|(definition, _)| definition)
            .collect())
    }

    /// 建表需要的全部语句。
    ///
    /// MySQL 把索引内联在 `CREATE TABLE` 中，因此返回单条语句；PostgreSQL 不允许
    /// 内联 `KEY`，索引单独用 `CREATE INDEX` 创建。
    pub(super) fn render_create_table_statements(
        self,
        table: &TableConfig,
        include_foreign_keys: bool,
    ) -> Result<Vec<String>, BaseError> {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql => Ok(vec![self.render_create_table(table, include_foreign_keys)?]),
            #[cfg(feature = "postgres")]
            Self::PostgreSql => {
                let mut statements = vec![self.render_create_table(table, include_foreign_keys)?];
                for index in self.desired_indexes(table)? {
                    statements.push(self.render_create_index_on_table(table, &index)?);
                }
                Ok(statements)
            }
        }
    }

    /// 渲染单条 `CREATE TABLE`（不含独立索引）。
    pub(super) fn render_create_table(
        self,
        table: &TableConfig,
        include_foreign_keys: bool,
    ) -> Result<String, BaseError> {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql => mysql_render::render_create_table(table, include_foreign_keys),
            #[cfg(feature = "postgres")]
            Self::PostgreSql => {
                let mut fields: Vec<&FieldConfig> = table.fields.values().collect();
                fields.sort_by(|left, right| left.name.cmp(&right.name));
                let mut definitions = fields
                    .into_iter()
                    .map(|field| self.render_column(field))
                    .collect::<Result<Vec<_>, _>>()?;
                definitions.push(format!(
                    "PRIMARY KEY ({})",
                    self.quote_identifier(&table.primary_key)?
                ));
                for check in &table.checks {
                    definitions.push(self.render_check(check)?);
                }
                if include_foreign_keys {
                    for foreign_key in &table.foreign_keys {
                        definitions.push(self.render_foreign_key(foreign_key)?);
                    }
                }
                Ok(format!(
                    "CREATE TABLE {} ({})",
                    self.quote_table(&table.table_name)?,
                    definitions.join(", ")
                ))
            }
        }
    }

    /// 渲染列定义。
    pub(super) fn render_column(self, field: &FieldConfig) -> Result<String, BaseError> {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql => mysql_render::render_column(field),
            #[cfg(feature = "postgres")]
            Self::PostgreSql => {
                let sql_type = self.render_type(&field.field_type, &field.name)?;
                let mut definition = format!(
                    "{} {sql_type} {}",
                    self.quote_identifier(&field.name)?,
                    if field.required { "NOT NULL" } else { "NULL" }
                );
                if field.auto_increment {
                    definition.push_str(" GENERATED BY DEFAULT AS IDENTITY");
                }
                if let Some(default) = &field.default_value {
                    if default.is_null() && field.required {
                        return Err(BaseError::ConfigError(format!(
                            "必填字段 {} 不能声明 NULL 默认值",
                            field.name
                        )));
                    }
                    if !default.is_null() {
                        field.field_type.validate(&field.name, default)?;
                    }
                    definition.push_str(" DEFAULT ");
                    definition.push_str(&render_postgres_default(default)?);
                }
                Ok(definition)
            }
        }
    }

    /// 渲染 PostgreSQL 列类型。
    #[cfg(feature = "postgres")]
    pub(super) fn render_type(
        self,
        field_type: &FieldType,
        field_name: &str,
    ) -> Result<String, BaseError> {
        match field_type {
            // 防御性断言：构建期已强制同一上界，保留 Err 避免绕过构建期渲染非法 DDL。
            FieldType::String { max_length } if (1..=MAX_VARCHAR_LENGTH).contains(max_length) => {
                Ok(format!("VARCHAR({max_length})"))
            }
            FieldType::String { max_length } => Err(BaseError::ConfigError(format!(
                "字段 {} 的 VARCHAR 长度必须在 1..={MAX_VARCHAR_LENGTH}: {}",
                field_name, max_length
            ))),
            FieldType::Integer => Ok("INTEGER".to_string()),
            FieldType::BigInt => Ok("BIGINT".to_string()),
            FieldType::Float => Ok("REAL".to_string()),
            FieldType::Double => Ok("DOUBLE PRECISION".to_string()),
            FieldType::Decimal { precision, scale } => Ok(format!("NUMERIC({precision},{scale})")),
            FieldType::Boolean => Ok("BOOLEAN".to_string()),
            FieldType::Date => Ok("DATE".to_string()),
            FieldType::DateTime => Ok("TIMESTAMPTZ".to_string()),
            // 时间戳在本项目中是 Unix 秒，与 MySQL 的 BIGINT 对齐。
            FieldType::Timestamp => Ok("BIGINT".to_string()),
            FieldType::Json => Ok("JSONB".to_string()),
            FieldType::Text => Ok("TEXT".to_string()),
            // PostgreSQL 没有内联 ENUM；静默降级会丢失取值约束，因此 fail-closed。
            FieldType::Enum { .. } => Err(BaseError::ConfigError(format!(
                "PostgreSQL schema 同步暂不支持枚举字段 {}；请改用 Text 字段加显式 CHECK",
                field_name
            ))),
        }
    }

    /// 渲染 RENAME COLUMN。
    pub(super) fn render_rename_column(
        self,
        table: &TableConfig,
        legacy: &str,
        current: &str,
    ) -> Result<String, BaseError> {
        Ok(format!(
            "ALTER TABLE {} RENAME COLUMN {} TO {}",
            self.quote_table(&table.table_name)?,
            self.quote_identifier(legacy)?,
            self.quote_identifier(current)?
        ))
    }

    /// 渲染 ADD COLUMN。
    pub(super) fn render_add_column(
        self,
        table: &TableConfig,
        field: &FieldConfig,
    ) -> Result<String, BaseError> {
        Ok(format!(
            "ALTER TABLE {} ADD COLUMN {}",
            self.quote_table(&table.table_name)?,
            self.render_column(field)?
        ))
    }

    /// 渲染修改列；PostgreSQL 会拆成 TYPE / NOT NULL / DEFAULT 多条语句。
    pub(super) fn render_modify_column(
        self,
        table: &TableConfig,
        existing_name: &str,
        field: &FieldConfig,
        issues: &[SchemaIssueKind],
    ) -> Result<Vec<String>, BaseError> {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql => {
                let _ = (existing_name, issues);
                Ok(vec![format!(
                    "ALTER TABLE {} MODIFY COLUMN {}",
                    self.quote_table(&table.table_name)?,
                    self.render_column(field)?
                )])
            }
            #[cfg(feature = "postgres")]
            Self::PostgreSql => {
                let quoted_table = self.quote_table(&table.table_name)?;
                let quoted_column = self.quote_identifier(existing_name)?;
                let mut statements = Vec::new();
                if issues.contains(&SchemaIssueKind::IncompatibleType) {
                    let sql_type = self.render_type(&field.field_type, &field.name)?;
                    statements.push(format!(
                        "ALTER TABLE {quoted_table} ALTER COLUMN {quoted_column} \
                         TYPE {sql_type} USING {quoted_column}::{sql_type}"
                    ));
                }
                if issues.contains(&SchemaIssueKind::NullabilityMismatch) {
                    statements.push(format!(
                        "ALTER TABLE {quoted_table} ALTER COLUMN {quoted_column} {}",
                        if field.required {
                            "SET NOT NULL"
                        } else {
                            "DROP NOT NULL"
                        }
                    ));
                }
                if issues.contains(&SchemaIssueKind::DefaultMismatch) {
                    match &field.default_value {
                        Some(value) if !value.is_null() => statements.push(format!(
                            "ALTER TABLE {quoted_table} ALTER COLUMN {quoted_column} \
                             SET DEFAULT {}",
                            render_postgres_default(value)?
                        )),
                        _ => statements.push(format!(
                            "ALTER TABLE {quoted_table} ALTER COLUMN {quoted_column} DROP DEFAULT"
                        )),
                    }
                }
                Ok(statements)
            }
        }
    }

    /// 渲染 ADD PRIMARY KEY。
    pub(super) fn render_add_primary_key(self, table: &TableConfig) -> Result<String, BaseError> {
        Ok(format!(
            "ALTER TABLE {} ADD PRIMARY KEY ({})",
            self.quote_table(&table.table_name)?,
            self.quote_identifier(&table.primary_key)?
        ))
    }

    /// 渲染在新建表上创建索引（PostgreSQL 不允许内联 KEY）。
    #[cfg(feature = "postgres")]
    pub(super) fn render_create_index_on_table(
        self,
        table: &TableConfig,
        index: &DesiredIndex,
    ) -> Result<String, BaseError> {
        let columns = index
            .columns
            .iter()
            .map(|column| self.quote_identifier(column))
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");
        let kind = if index.unique {
            "UNIQUE INDEX"
        } else {
            "INDEX"
        };
        Ok(format!(
            "CREATE {kind} {} ON {} ({columns})",
            self.quote_identifier(&index.name)?,
            self.quote_table(&table.table_name)?
        ))
    }

    /// 渲染给已存在表添加索引。
    pub(super) fn render_add_index(
        self,
        table: &TableConfig,
        index: &DesiredIndex,
    ) -> Result<String, BaseError> {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql => {
                let columns = index
                    .columns
                    .iter()
                    .map(|column| self.quote_identifier(column))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ");
                let kind = if index.unique {
                    "UNIQUE INDEX"
                } else {
                    "INDEX"
                };
                Ok(format!(
                    "ALTER TABLE {} ADD {kind} {} ({columns})",
                    self.quote_table(&table.table_name)?,
                    self.quote_identifier(&index.name)?
                ))
            }
            #[cfg(feature = "postgres")]
            Self::PostgreSql => self.render_create_index_on_table(table, index),
        }
    }

    /// 渲染 CHECK 约束。
    pub(super) fn render_check(self, check: &CheckConfig) -> Result<String, BaseError> {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql => mysql_render::render_check(check),
            #[cfg(feature = "postgres")]
            Self::PostgreSql => Ok(format!(
                "CONSTRAINT {} CHECK ({})",
                self.quote_identifier(&check.name)?,
                check.expression
            )),
        }
    }

    /// 渲染 ADD CONSTRAINT ... CHECK。
    pub(super) fn render_add_check(
        self,
        table: &TableConfig,
        check: &CheckConfig,
    ) -> Result<String, BaseError> {
        Ok(format!(
            "ALTER TABLE {} ADD {}",
            self.quote_table(&table.table_name)?,
            self.render_check(check)?
        ))
    }

    /// 渲染外键约束。
    pub(super) fn render_foreign_key(
        self,
        foreign_key: &ForeignKeyConfig,
    ) -> Result<String, BaseError> {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql => mysql_render::render_foreign_key(foreign_key),
            #[cfg(feature = "postgres")]
            Self::PostgreSql => {
                let columns = foreign_key
                    .columns
                    .iter()
                    .map(|column| self.quote_identifier(column))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ");
                let referenced_columns = foreign_key
                    .referenced_columns
                    .iter()
                    .map(|column| self.quote_identifier(column))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ");
                Ok(format!(
                    "CONSTRAINT {} FOREIGN KEY ({columns}) REFERENCES {} ({referenced_columns})",
                    self.quote_identifier(&foreign_key.name)?,
                    self.quote_table(&foreign_key.referenced_table)?
                ))
            }
        }
    }

    /// 渲染 ADD CONSTRAINT ... FOREIGN KEY。
    pub(super) fn render_add_foreign_key(
        self,
        table: &TableConfig,
        foreign_key: &ForeignKeyConfig,
    ) -> Result<String, BaseError> {
        Ok(format!(
            "ALTER TABLE {} ADD {}",
            self.quote_table(&table.table_name)?,
            self.render_foreign_key(foreign_key)?
        ))
    }

    /// 计算声明的索引（含自动命名与名称长度校验）。
    pub(super) fn desired_indexes(
        self,
        table: &TableConfig,
    ) -> Result<Vec<DesiredIndex>, BaseError> {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql => mysql_render::desired_indexes(table),
            #[cfg(feature = "postgres")]
            Self::PostgreSql => {
                let mut desired = Vec::new();
                for (unique, indexes) in [
                    (true, table.unique_indexes.as_slice()),
                    (false, table.indexes.as_slice()),
                ] {
                    for index in indexes {
                        if index.fields.is_empty() {
                            return Err(BaseError::ConfigError(format!(
                                "表 {} 包含空索引",
                                table.table_name
                            )));
                        }
                        for field in &index.fields {
                            if !table.fields.contains_key(field) {
                                return Err(BaseError::ConfigError(format!(
                                    "表 {} 的索引引用未声明字段: {}",
                                    table.table_name, field
                                )));
                            }
                        }
                        let prefix = if unique { "uk" } else { "idx" };
                        let name = index.name.clone().unwrap_or_else(|| {
                            format!("{prefix}_{}_{}", table.table_name, index.fields.join("_"))
                        });
                        // PostgreSQL 标识符上限是 63 字节（NUL 终止），超长会被服务端
                        // 静默截断，导致下次同步看到不同索引名而反复尝试创建，必须提前拒绝。
                        if name.len() > 63 {
                            return Err(BaseError::ConfigError(format!(
                                "索引名超过 PostgreSQL 63 字节限制，请显式命名: {name}"
                            )));
                        }
                        self.quote_identifier(&name)?;
                        desired.push(DesiredIndex {
                            name,
                            unique,
                            columns: index.fields.clone(),
                        });
                    }
                }
                Ok(desired)
            }
        }
    }

    /// 归一化 CHECK 表达式用于幂等比较。
    pub(super) fn normalize_check_expression(self, expression: &str) -> String {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql => mysql_render::normalize_check_expression(expression),
            #[cfg(feature = "postgres")]
            Self::PostgreSql => normalize_postgres_check_expression(expression),
        }
    }

    /// 把声明表达式中的列名换成改名前的旧列名。
    pub(super) fn expression_for_existing_schema(
        self,
        expression: &str,
        renamed_columns: &BTreeMap<String, String>,
    ) -> String {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql => renamed_columns.iter().fold(
                expression.to_string(),
                |expression, (legacy, current)| {
                    expression.replace(&format!("`{current}`"), &format!("`{legacy}`"))
                },
            ),
            #[cfg(feature = "postgres")]
            Self::PostgreSql => renamed_columns.iter().fold(
                expression.to_string(),
                |expression, (legacy, current)| {
                    expression.replace(&format!("\"{current}\""), &format!("\"{legacy}\""))
                },
            ),
        }
    }

    /// 类型漂移是否安全到可以自动修复；返回 `true` 表示必须拒绝。
    pub(super) fn is_unsafe_type_change(self, field_type: &FieldType, data_type: &str) -> bool {
        match field_type {
            FieldType::String { .. } => !matches!(
                data_type,
                "char" | "varchar" | "text" | "mediumtext" | "longtext"
            ),
            FieldType::Enum { .. } => !matches!(data_type, "char" | "varchar" | "enum" | "text"),
            _ => true,
        }
    }

    /// 只读验证表定义与数据库列是否兼容。
    pub(super) fn validate_schema(
        self,
        table: &TableConfig,
        columns: &[SchemaColumn],
    ) -> SchemaValidationReport {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql => table.validate_schema(columns),
            #[cfg(feature = "postgres")]
            Self::PostgreSql => validate_postgres_schema(table, columns),
        }
    }
}

/// 渲染 PostgreSQL 默认值。
#[cfg(feature = "postgres")]
fn render_postgres_default(value: &serde_json::Value) -> Result<String, BaseError> {
    match value {
        serde_json::Value::Null => Ok("NULL".to_string()),
        serde_json::Value::Bool(value) => Ok(if *value { "TRUE" } else { "FALSE" }.to_string()),
        serde_json::Value::Number(value) => Ok(value.to_string()),
        serde_json::Value::String(value) => Ok(format!("'{}'", value.replace('\'', "''"))),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => Err(BaseError::ConfigError(
            "数据库字段默认值只支持 null/bool/number/string".to_string(),
        )),
    }
}

#[cfg(feature = "postgres")]
fn validate_postgres_schema(
    table: &TableConfig,
    columns: &[SchemaColumn],
) -> SchemaValidationReport {
    let mut fields: Vec<_> = table.fields.values().collect();
    fields.sort_by(|left, right| left.name.cmp(&right.name));
    let mut issues = Vec::new();

    for field in fields {
        let Some(column) = columns.iter().find(|column| column.name == field.name) else {
            issues.push(SchemaIssue {
                field: field.name.clone(),
                kind: SchemaIssueKind::MissingColumn,
                expected: field.field_type.display_name().to_string(),
                actual: None,
            });
            continue;
        };
        if !postgres_type_compatible(&field.field_type, column) {
            issues.push(SchemaIssue {
                field: field.name.clone(),
                kind: SchemaIssueKind::IncompatibleType,
                expected: field.field_type.display_name().to_string(),
                actual: Some(column.column_type.clone()),
            });
        }
        let expected_nullable = !field.required;
        if expected_nullable != column.nullable {
            issues.push(SchemaIssue {
                field: field.name.clone(),
                kind: SchemaIssueKind::NullabilityMismatch,
                expected: if expected_nullable {
                    "NULL".to_string()
                } else {
                    "NOT NULL".to_string()
                },
                actual: Some(if column.nullable {
                    "NULL".to_string()
                } else {
                    "NOT NULL".to_string()
                }),
            });
        }
        let actual_auto_increment = column
            .column_type
            .split_whitespace()
            .any(|part| part.eq_ignore_ascii_case("auto_increment"));
        if field.auto_increment != actual_auto_increment {
            issues.push(SchemaIssue {
                field: field.name.clone(),
                kind: SchemaIssueKind::AutoIncrementMismatch,
                expected: if field.auto_increment {
                    "IDENTITY".to_string()
                } else {
                    "无 IDENTITY".to_string()
                },
                actual: Some(if actual_auto_increment {
                    "IDENTITY".to_string()
                } else {
                    "无 IDENTITY".to_string()
                }),
            });
        }
        let expected_default =
            normalize_postgres_declared_default(&field.field_type, field.default_value.as_ref());
        let actual_default = normalize_postgres_database_default(
            &field.field_type,
            column.database_default.as_deref(),
        );
        if expected_default != actual_default {
            issues.push(SchemaIssue {
                field: field.name.clone(),
                kind: SchemaIssueKind::DefaultMismatch,
                expected: describe_default(expected_default.as_deref()),
                actual: Some(describe_default(actual_default.as_deref())),
            });
        }
    }

    SchemaValidationReport {
        table: table.table_name.clone(),
        issues,
    }
}

#[cfg(feature = "postgres")]
fn postgres_type_compatible(field_type: &FieldType, column: &SchemaColumn) -> bool {
    let data_type = column.data_type.as_str();
    match field_type {
        FieldType::String { max_length } => {
            matches!(data_type, "varchar" | "char")
                && column
                    .max_length
                    .is_some_and(|actual| actual >= *max_length as u64)
        }
        FieldType::Integer => matches!(data_type, "smallint" | "integer" | "int" | "bigint"),
        FieldType::BigInt => matches!(data_type, "bigint" | "int8"),
        FieldType::Float => matches!(data_type, "real" | "float" | "double" | "decimal"),
        FieldType::Double => matches!(data_type, "double" | "decimal"),
        FieldType::Decimal { precision, scale } => {
            data_type == "decimal"
                && column
                    .column_type
                    .eq_ignore_ascii_case(&format!("decimal({precision},{scale})"))
        }
        FieldType::Boolean => matches!(data_type, "bool" | "boolean"),
        FieldType::Date => data_type == "date",
        FieldType::DateTime => matches!(data_type, "timestamp" | "datetime" | "timestamptz"),
        FieldType::Timestamp => matches!(data_type, "bigint" | "int8"),
        FieldType::Json => matches!(data_type, "json" | "jsonb"),
        FieldType::Text => data_type == "text",
        FieldType::Enum { .. } => matches!(data_type, "enum" | "text" | "varchar" | "char"),
    }
}

#[cfg(feature = "postgres")]
fn normalize_postgres_declared_default(
    field_type: &FieldType,
    default: Option<&serde_json::Value>,
) -> Option<String> {
    let value = default?;
    if value.is_null() {
        return None;
    }
    match field_type {
        FieldType::Boolean => value.as_bool().map(|value| value.to_string()),
        FieldType::Float | FieldType::Double => value.as_f64().map(normalize_float),
        FieldType::Decimal { .. } => match value {
            serde_json::Value::String(value) => Some(value.clone()),
            serde_json::Value::Number(value) => Some(value.to_string()),
            _ => None,
        },
        _ => match value {
            serde_json::Value::String(value) => Some(value.clone()),
            serde_json::Value::Number(value) => Some(value.to_string()),
            serde_json::Value::Bool(value) => Some(value.to_string()),
            serde_json::Value::Null
            | serde_json::Value::Array(_)
            | serde_json::Value::Object(_) => None,
        },
    }
}

#[cfg(feature = "postgres")]
fn normalize_postgres_database_default(
    field_type: &FieldType,
    default: Option<&str>,
) -> Option<String> {
    let raw = default?;
    let stripped = strip_postgres_casts(raw.trim());
    let value = stripped.trim();
    match field_type {
        FieldType::Boolean => match value.to_ascii_lowercase().as_str() {
            "true" | "t" | "1" => Some("true".to_string()),
            "false" | "f" | "0" => Some("false".to_string()),
            _ => Some(value.to_string()),
        },
        FieldType::Integer | FieldType::BigInt | FieldType::Timestamp => {
            value.parse::<i64>().map_or_else(
                |_| Some(value.to_string()),
                |number| Some(number.to_string()),
            )
        }
        FieldType::Float | FieldType::Double => value
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite())
            .map_or_else(
                || Some(value.to_string()),
                |number| Some(normalize_float(number)),
            ),
        FieldType::Decimal { .. } => Some(value.to_string()),
        _ => Some(unquote_postgres_literal(value)),
    }
}

#[cfg(feature = "postgres")]
fn normalize_float(value: f64) -> String {
    if value == 0.0 {
        "0".to_string()
    } else {
        value.to_string()
    }
}

#[cfg(feature = "postgres")]
fn describe_default(default: Option<&str>) -> String {
    default.map_or_else(
        || "无默认值（等价于 DEFAULT NULL）".to_string(),
        |value| format!("规范默认值 {value:?}"),
    )
}

#[cfg(feature = "postgres")]
fn unquote_postgres_literal(value: &str) -> String {
    if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        value[1..value.len() - 1].replace("''", "'")
    } else {
        value.to_string()
    }
}

/// 去掉 PostgreSQL 表达式中的 `::type` 转换；单引号字符串内部原样保留。
#[cfg(feature = "postgres")]
fn strip_postgres_casts(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut characters = input.char_indices().peekable();
    let mut quoted = false;
    while let Some((_, character)) = characters.next() {
        if character == '\'' {
            quoted = !quoted;
            output.push(character);
            continue;
        }
        if quoted {
            output.push(character);
            continue;
        }
        if character != ':' || !matches!(characters.peek(), Some((_, ':'))) {
            output.push(character);
            continue;
        }
        characters.next();
        // 类型名主体
        while matches!(characters.peek(), Some((_, suffix)) if suffix.is_ascii_alphanumeric() || *suffix == '_')
        {
            characters.next();
        }
        // 多词类型（character varying / double precision / timestamp with time zone…）
        loop {
            let mut lookahead = characters.clone();
            while matches!(lookahead.peek(), Some((_, suffix)) if suffix.is_whitespace()) {
                lookahead.next();
            }
            let mut word = String::new();
            while let Some((_, suffix)) = lookahead.peek() {
                if suffix.is_ascii_alphabetic() || *suffix == '_' {
                    word.push(*suffix);
                    lookahead.next();
                } else {
                    break;
                }
            }
            if !matches!(
                word.to_ascii_lowercase().as_str(),
                "varying" | "precision" | "without" | "with" | "time" | "zone"
            ) {
                break;
            }
            while matches!(characters.peek(), Some((_, suffix)) if suffix.is_whitespace()) {
                characters.next();
            }
            for _ in 0..word.chars().count() {
                characters.next();
            }
        }
        // 精度/长度，如 numeric(10,2)
        if matches!(characters.peek(), Some((_, '('))) {
            let mut depth = 0_i32;
            for (_, suffix) in characters.by_ref() {
                match suffix {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
        // 数组后缀
        while matches!(characters.peek(), Some((_, '['))) {
            characters.next();
            if matches!(characters.peek(), Some((_, ']'))) {
                characters.next();
            }
        }
    }
    output
}

/// PostgreSQL CHECK 表达式归一化：去 `CHECK` 前缀、去类型转换、去双引号/空白、
/// 小写关键字，并把 `col = ANY (ARRAY[...])` 还原成 `col IN (...)`。
#[cfg(feature = "postgres")]
fn normalize_postgres_check_expression(expression: &str) -> String {
    let trimmed = expression.trim();
    let without_prefix = if trimmed
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("check"))
    {
        trimmed[5..].trim_start()
    } else {
        trimmed
    };
    let without_casts = strip_postgres_casts(without_prefix);

    let mut normalized = String::with_capacity(without_casts.len());
    let mut characters = without_casts.chars().peekable();
    let mut quoted = false;
    for character in characters.by_ref() {
        if character == '\'' {
            quoted = !quoted;
            normalized.push(character);
            continue;
        }
        if quoted {
            normalized.push(character);
            continue;
        }
        if character == '"' || character.is_whitespace() {
            continue;
        }
        normalized.extend(character.to_lowercase());
    }

    let normalized = rewrite_postgres_any_array(&normalized);
    let normalized = strip_simple_identifier_parentheses(&normalized);
    strip_outer_parentheses(normalized)
}

/// 去掉仅包裹单个标识符的括号：`(status)in(...)` → `statusin(...)`。
///
/// PostgreSQL 会把 `IN` 改写成 `ANY (ARRAY[...])`，并在左侧标识符外加括号；
/// 只折叠“内容全是标识符字符”的括号，避免改变 `(a+b)` 之类的语义。
#[cfg(feature = "postgres")]
fn strip_simple_identifier_parentheses(expression: &str) -> String {
    let characters = expression.chars().collect::<Vec<_>>();
    let mut output = String::with_capacity(expression.len());
    let mut index = 0;
    let mut quoted = false;
    while index < characters.len() {
        let current = characters[index];
        if current == '\'' {
            quoted = !quoted;
            output.push(current);
            index += 1;
            continue;
        }
        if quoted {
            output.push(current);
            index += 1;
            continue;
        }
        if current == '(' {
            if let Some(offset) = characters[index + 1..]
                .iter()
                .position(|value| *value == ')')
            {
                let inner = characters[index + 1..index + 1 + offset]
                    .iter()
                    .collect::<String>();
                let simple = !inner.is_empty()
                    && inner
                        .chars()
                        .all(|value| value.is_ascii_alphanumeric() || value == '_' || value == '.')
                    && !inner
                        .chars()
                        .next()
                        .is_some_and(|value| value.is_ascii_digit());
                if simple {
                    output.push_str(&inner);
                    index = index + offset + 2;
                    continue;
                }
            }
        }
        output.push(current);
        index += 1;
    }
    output
}

#[cfg(feature = "postgres")]
fn rewrite_postgres_any_array(expression: &str) -> String {
    let needle = "=any(";
    let mut current = expression.to_string();
    loop {
        let Some(start) = current.find(needle) else {
            return current;
        };
        let bytes = current.as_bytes();
        let mut index = start + "=any".len();
        // PostgreSQL 可能额外包一层括号：`= ANY ((ARRAY[...]))`。
        let mut depth = 0_i32;
        while index < bytes.len() && bytes[index] == b'(' {
            depth += 1;
            index += 1;
        }
        if depth == 0 || !current[index..].starts_with("array[") {
            return current;
        }
        let inner_start = index + "array[".len();
        let Some(close_offset) = current[inner_start..].find(']') else {
            return current;
        };
        let inner = current[inner_start..inner_start + close_offset].to_string();
        let mut cursor = inner_start + close_offset + 1;
        while cursor < bytes.len() && depth > 0 {
            match bytes[cursor] {
                b'(' => depth += 1,
                b')' => depth -= 1,
                b' ' | b'\t' | b']' => {}
                _ => return current,
            }
            cursor += 1;
        }
        current = format!("{}in({inner}){}", &current[..start], &current[cursor..]);
    }
}

/// 去掉冗余的最外层括号；仅当括号完整包住整个表达式时成立。
pub(super) fn has_redundant_outer_parentheses(expression: &str) -> bool {
    if !expression.starts_with('(') || !expression.ends_with(')') {
        return false;
    }
    let mut depth = 0_i32;
    let mut quoted = false;
    let mut previous_quote = false;
    for (index, character) in expression.char_indices() {
        if character == '\'' {
            if quoted && previous_quote {
                previous_quote = false;
                continue;
            }
            previous_quote = quoted;
            quoted = !quoted;
            continue;
        }
        previous_quote = false;
        if quoted {
            continue;
        }
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 && index + character.len_utf8() != expression.len() {
                    return false;
                }
            }
            _ => {}
        }
        if depth < 0 {
            return false;
        }
    }
    depth == 0 && !quoted
}

#[cfg(feature = "postgres")]
fn strip_outer_parentheses(mut expression: String) -> String {
    while has_redundant_outer_parentheses(&expression) {
        expression = expression[1..expression.len() - 1].to_string();
    }
    expression
}
