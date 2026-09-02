use super::model::{DesiredIndex, ExistingForeignKey};
use crate::error::BaseError;
use crate::table::{
    CheckConfig, FieldConfig, FieldType, ForeignKeyConfig, TableConfig, TableDefinition,
};
use std::collections::BTreeMap;

pub(super) fn normalize_definitions<'a>(
    definitions: &'a [&'a TableDefinition],
) -> Result<Vec<&'a TableDefinition>, BaseError> {
    let mut normalized: BTreeMap<&str, (&TableDefinition, String)> = BTreeMap::new();
    for definition in definitions {
        let table = definition.config();
        validate_table_config(table)?;
        let signature = render_create_table(table, true)?;
        if let Some((_, existing_signature)) = normalized.get(table.table_name.as_str()) {
            if existing_signature != &signature {
                return Err(BaseError::ConfigError(format!(
                    "表配置冲突: {} 在多个模块中声明不同 schema",
                    table.table_name
                )));
            }
            continue;
        }
        normalized.insert(table.table_name.as_str(), (*definition, signature));
    }
    Ok(normalized
        .into_values()
        .map(|(definition, _)| definition)
        .collect())
}

pub(super) fn validate_table_config(table: &TableConfig) -> Result<(), BaseError> {
    quote_identifier(&table.table_name)?;
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
        quote_identifier(&field.name)?;
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
        let _ = render_column(field)?;
    }
    let _ = desired_indexes(table)?;
    Ok(())
}

pub(super) fn render_create_table(
    table: &TableConfig,
    include_foreign_keys: bool,
) -> Result<String, BaseError> {
    let mut fields: Vec<&FieldConfig> = table.fields.values().collect();
    fields.sort_by(|left, right| left.name.cmp(&right.name));
    let mut definitions = fields
        .into_iter()
        .map(render_column)
        .collect::<Result<Vec<_>, _>>()?;
    definitions.push(format!(
        "PRIMARY KEY ({})",
        quote_identifier(&table.primary_key)?
    ));
    for index in desired_indexes(table)? {
        let columns = index
            .columns
            .iter()
            .map(|column| quote_identifier(column))
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");
        let kind = if index.unique { "UNIQUE KEY" } else { "KEY" };
        definitions.push(format!(
            "{kind} {} ({columns})",
            quote_identifier(&index.name)?
        ));
    }
    for check in &table.checks {
        definitions.push(render_check(check)?);
    }
    if include_foreign_keys {
        for foreign_key in &table.foreign_keys {
            definitions.push(render_foreign_key(foreign_key)?);
        }
    }
    Ok(format!(
        "CREATE TABLE {} ({}) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
        quote_identifier(&table.table_name)?,
        definitions.join(", ")
    ))
}

pub(super) fn render_check(check: &CheckConfig) -> Result<String, BaseError> {
    Ok(format!(
        "CONSTRAINT {} CHECK ({})",
        quote_identifier(&check.name)?,
        check.expression
    ))
}

pub(super) fn render_foreign_key(foreign_key: &ForeignKeyConfig) -> Result<String, BaseError> {
    let columns = foreign_key
        .columns
        .iter()
        .map(|column| quote_identifier(column))
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    let referenced_columns = foreign_key
        .referenced_columns
        .iter()
        .map(|column| quote_identifier(column))
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    Ok(format!(
        "CONSTRAINT {} FOREIGN KEY ({columns}) REFERENCES {} ({referenced_columns})",
        quote_identifier(&foreign_key.name)?,
        quote_identifier(&foreign_key.referenced_table)?
    ))
}

pub(super) fn normalize_check_expression(expression: &str) -> String {
    let mut normalized = expression
        .chars()
        .filter(|character| !character.is_ascii_whitespace() && *character != '`')
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .replace("_utf8mb4", "")
        .replace("_utf8", "")
        .replace("\\'", "'");
    while has_redundant_outer_parentheses(&normalized) {
        normalized = normalized[1..normalized.len() - 1].to_string();
    }
    normalized
}

fn has_redundant_outer_parentheses(expression: &str) -> bool {
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

pub(super) fn foreign_key_matches(
    existing: &ExistingForeignKey,
    desired: &ForeignKeyConfig,
    renamed_columns: &BTreeMap<String, String>,
) -> bool {
    let columns = existing
        .columns
        .iter()
        .map(|column| {
            renamed_columns
                .get(column)
                .cloned()
                .unwrap_or_else(|| column.clone())
        })
        .collect::<Vec<_>>();
    columns == desired.columns
        && existing.referenced_table == desired.referenced_table
        && existing.referenced_columns == desired.referenced_columns
        && matches!(existing.update_rule.as_str(), "RESTRICT" | "NO ACTION")
        && matches!(existing.delete_rule.as_str(), "RESTRICT" | "NO ACTION")
}

pub(super) fn existing_column_name(
    column: &str,
    renamed_columns: &BTreeMap<String, String>,
) -> String {
    renamed_columns
        .iter()
        .find_map(|(legacy, current)| (current == column).then(|| legacy.clone()))
        .unwrap_or_else(|| column.to_string())
}

pub(super) fn expression_for_existing_schema(
    expression: &str,
    renamed_columns: &BTreeMap<String, String>,
) -> String {
    renamed_columns
        .iter()
        .fold(expression.to_string(), |expression, (legacy, current)| {
            expression.replace(&format!("`{current}`"), &format!("`{legacy}`"))
        })
}

pub(super) fn render_column(field: &FieldConfig) -> Result<String, BaseError> {
    let sql_type = match &field.field_type {
        FieldType::String { max_length } if (1..=16_383).contains(max_length) => {
            format!("VARCHAR({max_length})")
        }
        FieldType::String { max_length } => {
            return Err(BaseError::ConfigError(format!(
                "字段 {} 的 VARCHAR 长度必须在 1..=16383: {}",
                field.name, max_length
            )))
        }
        FieldType::Integer => "INT".to_string(),
        FieldType::BigInt => "BIGINT".to_string(),
        FieldType::Float => "FLOAT".to_string(),
        FieldType::Double => "DOUBLE".to_string(),
        FieldType::Decimal { precision, scale } => format!("DECIMAL({precision},{scale})"),
        FieldType::Boolean => "TINYINT(1)".to_string(),
        FieldType::Date => "DATE".to_string(),
        FieldType::DateTime => "DATETIME".to_string(),
        FieldType::Timestamp => "BIGINT".to_string(),
        FieldType::Json => "JSON".to_string(),
        FieldType::Text => "TEXT".to_string(),
        FieldType::Enum { values } => {
            if values.is_empty() || values.iter().any(|value| value.is_empty()) {
                return Err(BaseError::ConfigError(format!(
                    "枚举字段 {} 必须声明非空值",
                    field.name
                )));
            }
            let mut sorted = values.clone();
            sorted.sort();
            if sorted.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(BaseError::ConfigError(format!(
                    "枚举字段 {} 包含重复值",
                    field.name
                )));
            }
            format!(
                "ENUM({})",
                values
                    .iter()
                    .map(|value| quote_string(value))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    };

    let mut definition = format!(
        "{} {sql_type} {}",
        quote_identifier(&field.name)?,
        if field.required { "NOT NULL" } else { "NULL" }
    );
    if field.auto_increment {
        definition.push_str(" AUTO_INCREMENT");
    }
    if let Some(default) = &field.default_value {
        if default.is_null() && field.required {
            return Err(BaseError::ConfigError(format!(
                "必填字段 {} 不能声明 NULL 默认值",
                field.name
            )));
        }
        if matches!(field.field_type, FieldType::Text | FieldType::Json) {
            return Err(BaseError::ConfigError(format!(
                "TEXT/JSON 字段 {} 不自动生成数据库默认值",
                field.name
            )));
        }
        if !default.is_null() {
            field.field_type.validate(&field.name, default)?;
        }
        definition.push_str(" DEFAULT ");
        definition.push_str(&render_default(default)?);
    }
    Ok(definition)
}

fn render_default(value: &serde_json::Value) -> Result<String, BaseError> {
    match value {
        serde_json::Value::Null => Ok("NULL".to_string()),
        serde_json::Value::Bool(value) => Ok(if *value { "1" } else { "0" }.to_string()),
        serde_json::Value::Number(value) => Ok(value.to_string()),
        serde_json::Value::String(value) => Ok(quote_string(value)),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => Err(BaseError::ConfigError(
            "数据库字段默认值只支持 null/bool/number/string".to_string(),
        )),
    }
}

pub(super) fn desired_indexes(table: &TableConfig) -> Result<Vec<DesiredIndex>, BaseError> {
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
            if name.len() > 64 {
                return Err(BaseError::ConfigError(format!(
                    "索引名超过 MySQL 64 字符限制，请显式命名: {name}"
                )));
            }
            quote_identifier(&name)?;
            desired.push(DesiredIndex {
                name,
                unique,
                columns: index.fields.clone(),
            });
        }
    }
    Ok(desired)
}

pub(super) fn quote_identifier(identifier: &str) -> Result<String, BaseError> {
    yang_db::mysql::quote_identifier(identifier).map_err(|error| {
        BaseError::ConfigError(format!("非法数据库标识符 {identifier:?}: {error}"))
    })
}

pub(super) fn quote_string(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "''"))
}

pub(super) fn schema_lock_name(database_name: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in database_name.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("yang_base_schema_{hash:016x}")
}
