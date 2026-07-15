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
}

impl SchemaColumn {
    /// 构造列快照；类型名称会统一转为 ASCII 小写。
    pub fn new(
        name: impl Into<String>,
        data_type: impl Into<String>,
        column_type: impl Into<String>,
        nullable: bool,
        max_length: Option<u64>,
    ) -> Self {
        Self {
            name: name.into(),
            data_type: data_type.into().to_ascii_lowercase(),
            column_type: column_type.into().to_ascii_lowercase(),
            nullable,
            max_length,
        }
    }

    /// 在完整列类型描述中标记 `AUTO_INCREMENT`。
    ///
    /// 该信息保存在 `column_type` 描述中，避免扩展公开结构字段而破坏 0.1.x
    /// 调用方的结构体字面量兼容性。
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

/// TableConfig 运行期契约与数据库列不兼容的原因。
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
}

/// 一项 schema 兼容性问题。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaIssue {
    /// TableConfig 字段名。
    pub field: String,
    /// 问题类型。
    pub kind: SchemaIssueKind,
    /// TableConfig 的期望描述。
    pub expected: String,
    /// 数据库实际描述；缺列时为 None。
    pub actual: Option<String>,
}

/// TableConfig 对当前数据库 schema 的只读验证报告。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaValidationReport {
    /// 被验证的表名。
    pub table: String,
    /// 确定性排序的问题列表。
    pub issues: Vec<SchemaIssue>,
}

impl SchemaValidationReport {
    /// 当前数据库是否足以承载 TableConfig 声明的运行期访问契约。
    pub fn is_compatible(&self) -> bool {
        self.issues.is_empty()
    }
}

impl TableConfig {
    /// 验证声明字段能否由给定数据库列提供。
    ///
    /// 这是只读、单向的运行期兼容检查：数据库额外列、索引、默认值、触发器等不属于
    /// TableConfig 的所有权范围，因此不会作为问题，也不会生成 ALTER/回滚 SQL。
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
            if field.required && column.nullable {
                issues.push(SchemaIssue {
                    field: field.name.clone(),
                    kind: SchemaIssueKind::NullabilityMismatch,
                    expected: "NOT NULL".to_string(),
                    actual: Some("NULL".to_string()),
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
        FieldType::Boolean => {
            matches!(data_type, "bool" | "boolean")
                || (data_type == "tinyint" && column.column_type.starts_with("tinyint(1)"))
        }
        FieldType::Date => data_type == "date",
        FieldType::DateTime => matches!(data_type, "datetime" | "timestamp"),
        FieldType::Timestamp => data_type == "bigint",
        FieldType::Json => data_type == "json",
        FieldType::Text => matches!(data_type, "tinytext" | "text" | "mediumtext" | "longtext"),
        FieldType::Enum { .. } => matches!(data_type, "enum" | "varchar" | "char"),
        FieldType::ForeignKey { .. } => true,
    }
}
