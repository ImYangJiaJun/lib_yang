use super::{FieldType, TableConfig};

/// 数据库 introspection 得到的最小列快照。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaColumn {
    /// 列名。
    pub name: String,
    /// information_schema 的规范数据类型。
    pub data_type: String,
    /// 方言完整列类型，如 `varchar(255)` / `tinyint(1)`。
    pub column_type: String,
    /// 数据库是否允许 NULL。
    pub nullable: bool,
    /// 字符列最大长度。
    pub max_length: Option<u64>,
    /// information_schema 返回的数据库默认值；无默认值和 DEFAULT NULL 均为 None。
    pub database_default: Option<String>,
}

impl SchemaColumn {
    /// 构造列快照；类型名称会统一转为 ASCII 小写。
    pub fn new(
        name: impl Into<String>,
        data_type: impl Into<String>,
        column_type: impl Into<String>,
        nullable: bool,
        max_length: Option<u64>,
        database_default: Option<String>,
    ) -> Self {
        Self {
            name: name.into(),
            data_type: data_type.into().to_ascii_lowercase(),
            column_type: column_type.into(),
            nullable,
            max_length,
            database_default,
        }
    }

    /// 在完整列类型描述中标记 `AUTO_INCREMENT`。
    ///
    /// 该信息保存在 `column_type` 描述中，便于与 information_schema 的其它列属性
    /// 一起形成单个只读快照。
    pub fn with_auto_increment(mut self, auto_increment: bool) -> Self {
        let has_marker = self
            .column_type
            .split_whitespace()
            .any(|part| part.eq_ignore_ascii_case("auto_increment"));
        if auto_increment && !has_marker {
            self.column_type.push_str(" auto_increment");
        }
        self
    }
}

/// [`super::TableDefinition`] 与数据库列不兼容的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SchemaIssueKind {
    /// 声明字段在数据库中不存在。
    MissingColumn,
    /// 数据库列不能承载声明的运行期字段类型。
    IncompatibleType,
    /// 必填字段在数据库中仍允许 NULL。
    NullabilityMismatch,
    /// 声明要求自增，但数据库列没有自增属性，或反之。
    AutoIncrementMismatch,
    /// 声明默认值与数据库默认值不一致。
    DefaultMismatch,
}

/// 一项 schema 兼容性问题。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaIssue {
    /// 表定义中的字段名。
    pub field: String,
    /// 问题类型。
    pub kind: SchemaIssueKind,
    /// 表定义的期望描述。
    pub expected: String,
    /// 数据库实际描述；缺列时为 None。
    pub actual: Option<String>,
}

/// [`super::TableDefinition`] 对当前数据库 schema 的只读验证报告。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaValidationReport {
    /// 被验证的表名。
    pub table: String,
    /// 确定性排序的问题列表。
    pub issues: Vec<SchemaIssue>,
}

impl SchemaValidationReport {
    /// 当前数据库是否足以承载表定义声明的运行期访问契约。
    pub fn is_compatible(&self) -> bool {
        self.issues.is_empty()
    }
}

impl TableConfig {
    /// 验证声明字段能否由给定数据库列提供。
    ///
    /// 这是只读的运行期契约检查：数据库额外列、索引、触发器等不属于表定义的
    /// 所有权范围，因此不会作为问题，也不会生成 ALTER/回滚 SQL。
    pub fn validate_schema(&self, columns: &[SchemaColumn]) -> SchemaValidationReport {
        let mut fields: Vec<_> = self.fields.values().collect();
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
            if !is_type_compatible(&field.field_type, column) {
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
                        "AUTO_INCREMENT".to_string()
                    } else {
                        "非 AUTO_INCREMENT".to_string()
                    },
                    actual: Some(if actual_auto_increment {
                        "AUTO_INCREMENT".to_string()
                    } else {
                        "非 AUTO_INCREMENT".to_string()
                    }),
                });
            }
            let expected_default =
                normalize_declared_default(&field.field_type, field.default_value.as_ref());
            let actual_default =
                normalize_database_default(&field.field_type, column.database_default.as_deref());
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
            table: self.table_name.clone(),
            issues,
        }
    }
}

fn is_type_compatible(field_type: &FieldType, column: &SchemaColumn) -> bool {
    let data_type = column.data_type.as_str();
    match field_type {
        FieldType::String { max_length } => {
            matches!(data_type, "varchar" | "char")
                && column
                    .max_length
                    .is_some_and(|actual| actual >= *max_length as u64)
        }
        FieldType::Integer => matches!(
            data_type,
            "tinyint" | "smallint" | "mediumint" | "int" | "integer" | "bigint"
        ),
        FieldType::BigInt => data_type == "bigint",
        FieldType::Float => matches!(data_type, "float" | "double" | "decimal"),
        FieldType::Double => matches!(data_type, "double" | "decimal"),
        FieldType::Decimal { precision, scale } => {
            data_type == "decimal"
                && column
                    .column_type
                    .eq_ignore_ascii_case(&format!("decimal({precision},{scale})"))
        }
        FieldType::Boolean => {
            matches!(data_type, "bool" | "boolean")
                || (data_type == "tinyint"
                    && column
                        .column_type
                        .to_ascii_lowercase()
                        .starts_with("tinyint(1)"))
        }
        FieldType::Date => data_type == "date",
        FieldType::DateTime => data_type == "datetime",
        FieldType::Timestamp => data_type == "bigint",
        FieldType::Json => data_type == "json",
        FieldType::Text => matches!(data_type, "text" | "mediumtext" | "longtext"),
        FieldType::Enum { values } => {
            data_type == "enum"
                && parse_mysql_enum_values(&column.column_type)
                    .is_some_and(|actual| actual.as_slice() == values.as_slice())
        }
    }
}

fn normalize_declared_default(
    field_type: &FieldType,
    default: Option<&serde_json::Value>,
) -> Option<String> {
    let value = default?;
    if value.is_null() {
        return None;
    }
    match field_type {
        FieldType::Boolean => value
            .as_bool()
            .map(|value| if value { "1" } else { "0" }.to_string()),
        FieldType::Float | FieldType::Double => value.as_f64().map(normalize_float),
        FieldType::Decimal { .. } => match value {
            serde_json::Value::String(value) => Some(value.clone()),
            serde_json::Value::Number(value) => Some(value.to_string()),
            _ => None,
        },
        _ => match value {
            serde_json::Value::String(value) => Some(value.clone()),
            serde_json::Value::Number(value) => Some(value.to_string()),
            serde_json::Value::Bool(value) => Some(if *value { "1" } else { "0" }.to_string()),
            serde_json::Value::Null
            | serde_json::Value::Array(_)
            | serde_json::Value::Object(_) => None,
        },
    }
}

fn normalize_database_default(field_type: &FieldType, default: Option<&str>) -> Option<String> {
    let value = default?;
    match field_type {
        FieldType::Boolean => match value.to_ascii_lowercase().as_str() {
            "1" | "true" => Some("1".to_string()),
            "0" | "false" => Some("0".to_string()),
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
        _ => Some(value.to_string()),
    }
}

fn normalize_float(value: f64) -> String {
    if value == 0.0 {
        "0".to_string()
    } else {
        value.to_string()
    }
}

fn describe_default(default: Option<&str>) -> String {
    default.map_or_else(
        || "无默认值（等价于 DEFAULT NULL）".to_string(),
        |value| format!("规范默认值 {value:?}"),
    )
}

fn parse_mysql_enum_values(column_type: &str) -> Option<Vec<String>> {
    let column_type = column_type.trim();
    let opening = column_type.find('(')?;
    if !column_type[..opening].trim().eq_ignore_ascii_case("enum") || !column_type.ends_with(')') {
        return None;
    }
    let mut chars = column_type[opening + 1..column_type.len() - 1]
        .chars()
        .peekable();
    let mut values = Vec::new();

    loop {
        while chars
            .next_if(|character| character.is_whitespace())
            .is_some()
        {}
        if chars.peek().is_none() {
            break;
        }
        if chars.next()? != '\'' {
            return None;
        }

        let mut value = String::new();
        loop {
            match chars.next()? {
                '\\' => {
                    let escaped = chars.next()?;
                    value.push(match escaped {
                        '0' => '\0',
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        'Z' => '\u{001a}',
                        other => other,
                    });
                }
                '\'' if chars.peek() == Some(&'\'') => {
                    chars.next();
                    value.push('\'');
                }
                '\'' => break,
                character => value.push(character),
            }
        }
        values.push(value);

        while chars
            .next_if(|character| character.is_whitespace())
            .is_some()
        {}
        match chars.peek() {
            Some(',') => {
                chars.next();
            }
            None => break,
            Some(_) => return None,
        }
    }

    Some(values)
}
