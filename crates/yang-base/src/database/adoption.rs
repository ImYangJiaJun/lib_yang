//! 存量 PostgreSQL schema 的接入报告。
//!
//! 只读扫描表、列、主键与索引，为迁移人员输出 Markdown 报告与字段声明建议。
//! 不执行任何 DDL，也不写 `_yang_schema_binding`。

use crate::error::BaseError;
use sqlx::postgres::PgPool;
use sqlx::FromRow;
use std::collections::BTreeMap;

/// 单列报告。
#[derive(Debug, Clone)]
pub struct PgAdoptionColumn {
    /// 列名。
    pub name: String,
    /// `format_type` 规范化类型。
    pub formatted_type: String,
    /// `pg_type.typname`。
    pub type_name: String,
    /// `pg_type.typtype`（`b` 基础类型、`e` 枚举、`d` domain…）。
    pub type_kind: String,
    /// 是否允许 NULL。
    pub nullable: bool,
    /// 是否主键列。
    pub primary_key: bool,
    /// identity / serial 列。
    pub auto_increment: bool,
    /// 原生枚举标签。
    pub enum_labels: Vec<String>,
}

/// 单索引报告。
#[derive(Debug, Clone)]
pub struct PgAdoptionIndex {
    /// 索引名。
    pub name: String,
    /// 是否唯一。
    pub unique: bool,
    /// 列键。
    pub columns: Vec<String>,
    /// 表达式键。
    pub expressions: Vec<String>,
    /// 部分索引谓词。
    pub predicate: Option<String>,
}

/// 单表报告。
#[derive(Debug, Clone)]
pub struct PgAdoptionTable {
    /// 表名。
    pub name: String,
    /// 列。
    pub columns: Vec<PgAdoptionColumn>,
    /// 主键列（按声明顺序）。
    pub primary_key: Vec<String>,
    /// 索引。
    pub indexes: Vec<PgAdoptionIndex>,
}

/// 整个 schema 的接入报告。
#[derive(Debug, Clone)]
pub struct PgAdoptionReport {
    /// 扫描的 schema。
    pub schema: String,
    /// 表，按名称排序。
    pub tables: Vec<PgAdoptionTable>,
}

impl PgAdoptionReport {
    /// 渲染为 Markdown。
    pub fn to_markdown(&self) -> String {
        let mut output = String::new();
        output.push_str(&format!(
            "# PostgreSQL schema 接入报告: `{}`\n\n",
            self.schema
        ));
        output.push_str(&format!("- 表数量: {}\n", self.tables.len()));
        output.push_str(&format!(
            "- 列数量: {}\n\n",
            self.tables
                .iter()
                .map(|table| table.columns.len())
                .sum::<usize>()
        ));
        for table in &self.tables {
            output.push_str(&format!("## `{}`\n\n", table.name));
            output.push_str("| 列 | 类型 | 可空 | 主键 | 建议声明 |\n|---|---|---|---|---|\n");
            for column in &table.columns {
                output.push_str(&format!(
                    "| `{}` | `{}` | {} | {} | {} |\n",
                    column.name,
                    column.formatted_type,
                    if column.nullable { "是" } else { "否" },
                    if column.primary_key { "是" } else { "" },
                    column.suggested_field().unwrap_or_else(|| {
                        format!(
                            "`opaque_column(\"{}\", \"{}\")`",
                            column.name, column.formatted_type
                        )
                    })
                ));
            }
            output.push('\n');
            if !table.primary_key.is_empty() {
                output.push_str(&format!(
                    "- 主键: {}\n",
                    table
                        .primary_key
                        .iter()
                        .map(|column| format!("`{column}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            for index in &table.indexes {
                output.push_str(&format!("- 索引 `{}`: {}\n", index.name, index.describe()));
            }
            output.push_str("\n> 默认值、CHECK、外键与列注释需要人工核对后补到定义中；opaque 列不会进入通用 CRUD/UI。\n\n");
        }
        output
    }
}

impl PgAdoptionColumn {
    /// 生成 schema-first DSL 的字段声明建议；无法安全映射时返回 `None`。
    pub fn suggested_field(&self) -> Option<String> {
        let required = if self.nullable { "" } else { ".required()" };
        let primary = if self.primary_key {
            ".primary_key()"
        } else {
            ""
        };
        let auto = if self.auto_increment {
            ".auto_increment()"
        } else {
            ""
        };
        let base = match self.type_name.as_str() {
            "int2" | "int4" => format!("Field::integer(\"{}\")", self.name),
            "int8" => format!("Field::bigint(\"{}\")", self.name),
            "float4" => format!("Field::float(\"{}\")", self.name),
            "float8" => format!("Field::double(\"{}\")", self.name),
            "bool" => format!("Field::boolean(\"{}\")", self.name),
            "date" => format!("Field::date(\"{}\")", self.name),
            "timestamptz" => format!("Field::datetime(\"{}\")", self.name),
            // 无时区 timestamp 与框架 DateTime（TIMESTAMPTZ）语义不同，不自动建议。
            "timestamp" => return None,
            "json" | "jsonb" => format!("Field::json(\"{}\")", self.name),
            "text" => format!("Field::text(\"{}\")", self.name),
            "uuid" => format!("Field::uuid(\"{}\")", self.name),
            "bytea" => format!("Field::bytes(\"{}\")", self.name),
            "varchar" | "bpchar" => {
                let length = type_length(&self.formatted_type)?;
                format!("Field::string(\"{}\", {length})", self.name)
            }
            "numeric" => {
                let (precision, scale) = numeric_precision_scale(&self.formatted_type)?;
                format!("Field::decimal(\"{}\", {precision}, {scale})", self.name)
            }
            "_int2" | "_int4" => format!("Field::array(\"{}\", FieldType::Integer)", self.name),
            "_int8" => format!("Field::array(\"{}\", FieldType::BigInt)", self.name),
            "_float4" => format!("Field::array(\"{}\", FieldType::Float)", self.name),
            "_float8" => format!("Field::array(\"{}\", FieldType::Double)", self.name),
            "_bool" => format!("Field::array(\"{}\", FieldType::Boolean)", self.name),
            "_text" => format!("Field::array(\"{}\", FieldType::Text)", self.name),
            "_uuid" => format!("Field::array(\"{}\", FieldType::Uuid)", self.name),
            "_bytea" => format!("Field::array(\"{}\", FieldType::Bytes)", self.name),
            _ if self.type_kind == "e" => {
                let values = self
                    .enum_labels
                    .iter()
                    .map(|value| format!("\"{value}\""))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "Field::enumeration(\"{}\", [{}]).native_enum(\"{}\")",
                    self.name, values, self.type_name
                )
            }
            _ => return None,
        };
        Some(format!("{base}{required}{primary}{auto}"))
    }
}

impl PgAdoptionIndex {
    fn describe(&self) -> String {
        let mut kind = if self.unique { "唯一" } else { "普通" };
        if self.predicate.is_some() {
            kind = if self.unique {
                "部分唯一"
            } else {
                "部分"
            };
        }
        if !self.expressions.is_empty() {
            kind = if self.unique {
                "表达式唯一"
            } else {
                "表达式"
            };
        }
        let keys = self
            .columns
            .iter()
            .map(|column| format!("`{column}`"))
            .chain(
                self.expressions
                    .iter()
                    .map(|expression| format!("`{expression}`")),
            )
            .collect::<Vec<_>>()
            .join(", ");
        let predicate = self
            .predicate
            .as_ref()
            .map(|predicate| format!(" WHERE {predicate}"))
            .unwrap_or_default();
        format!("{kind} ({keys}){predicate}")
    }
}

/// 只读扫描 PostgreSQL schema 并生成接入报告。
pub async fn describe_postgres_schema(
    database: &yang_db::postgres::Database,
    schema: Option<&str>,
) -> Result<PgAdoptionReport, BaseError> {
    fetch_report(database.pool(), schema).await
}

async fn fetch_report(pool: &PgPool, schema: Option<&str>) -> Result<PgAdoptionReport, BaseError> {
    #[derive(FromRow)]
    struct TableRow {
        table_name: String,
    }
    #[derive(FromRow)]
    struct ColumnRow {
        table_name: String,
        column_name: String,
        type_name: String,
        type_kind: String,
        formatted_type: String,
        nullable: bool,
        auto_increment: bool,
    }
    #[derive(FromRow)]
    struct EnumRow {
        type_name: String,
        label: String,
    }
    #[derive(FromRow)]
    struct PrimaryRow {
        table_name: String,
        column_name: String,
    }
    #[derive(FromRow)]
    struct IndexRow {
        table_name: String,
        index_name: String,
        is_unique: bool,
        predicate: Option<String>,
        column_name: Option<String>,
        expression: Option<String>,
    }

    let schema: String = sqlx::query_scalar("SELECT COALESCE($1::text, current_schema())")
        .bind(schema)
        .fetch_one(pool)
        .await
        .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;

    let table_rows: Vec<TableRow> = sqlx::query_as(
        "SELECT c.relname::text AS table_name \
         FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE n.nspname = $1 AND c.relkind IN ('r', 'p') AND NOT c.relispartition \
         ORDER BY c.relname",
    )
    .bind(&schema)
    .fetch_all(pool)
    .await
    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;

    let column_rows: Vec<ColumnRow> = sqlx::query_as(
        "SELECT c.relname::text AS table_name, a.attname::text AS column_name, \
                t.typname::text AS type_name, t.typtype::text AS type_kind, \
                format_type(a.atttypid, a.atttypmod) AS formatted_type, \
                NOT a.attnotnull AS nullable, \
                (a.attidentity <> '' OR COALESCE(pg_get_expr(d.adbin, d.adrelid) LIKE 'nextval(%', false)) \
                  AS auto_increment \
         FROM pg_attribute a \
         JOIN pg_class c ON c.oid = a.attrelid \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         JOIN pg_type t ON t.oid = a.atttypid \
         LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum \
         WHERE n.nspname = $1 AND c.relkind IN ('r', 'p') \
           AND a.attnum > 0 AND NOT a.attisdropped \
         ORDER BY c.relname, a.attnum",
    )
    .bind(&schema)
    .fetch_all(pool)
    .await
    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;

    let enum_rows: Vec<EnumRow> = sqlx::query_as(
        "SELECT t.typname::text AS type_name, e.enumlabel::text AS label \
         FROM pg_type t \
         JOIN pg_namespace n ON n.oid = t.typnamespace \
         JOIN pg_enum e ON e.enumtypid = t.oid \
         WHERE n.nspname = $1 ORDER BY t.typname, e.enumsortorder",
    )
    .bind(&schema)
    .fetch_all(pool)
    .await
    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
    let mut enum_labels = BTreeMap::<String, Vec<String>>::new();
    for row in enum_rows {
        enum_labels
            .entry(row.type_name)
            .or_default()
            .push(row.label);
    }

    let primary_rows: Vec<PrimaryRow> = sqlx::query_as(
        "SELECT c.relname::text AS table_name, a.attname::text AS column_name \
         FROM pg_constraint con \
         JOIN pg_class c ON c.oid = con.conrelid \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         JOIN unnest(con.conkey) WITH ORDINALITY AS key(attnum, ord) ON true \
         JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = key.attnum \
         WHERE con.contype = 'p' AND n.nspname = $1 \
         ORDER BY c.relname, key.ord",
    )
    .bind(&schema)
    .fetch_all(pool)
    .await
    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
    let mut primary_keys = BTreeMap::<String, Vec<String>>::new();
    for row in primary_rows {
        primary_keys
            .entry(row.table_name)
            .or_default()
            .push(row.column_name);
    }

    let index_rows: Vec<IndexRow> = sqlx::query_as(
        "SELECT c.relname::text AS table_name, i.relname::text AS index_name, \
                ix.indisunique AS is_unique, \
                pg_get_expr(ix.indpred, ix.indrelid) AS predicate, \
                a.attname::text AS column_name, \
                CASE WHEN key.attnum = 0 \
                     THEN pg_get_indexdef(ix.indexrelid, key.ord::integer, true) \
                     ELSE NULL END AS expression \
         FROM pg_index ix \
         JOIN pg_class c ON c.oid = ix.indrelid \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         JOIN pg_class i ON i.oid = ix.indexrelid \
         JOIN unnest(ix.indkey::smallint[]) WITH ORDINALITY AS key(attnum, ord) ON true \
         LEFT JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = key.attnum \
         WHERE n.nspname = $1 AND NOT ix.indisprimary \
           AND NOT EXISTS ( \
               SELECT 1 FROM pg_constraint con \
               WHERE con.conindid = ix.indexrelid AND con.contype IN ('u', 'x', 'p') \
           ) \
           AND key.ord <= ix.indnkeyatts::bigint \
         ORDER BY c.relname, i.relname, key.ord",
    )
    .bind(&schema)
    .fetch_all(pool)
    .await
    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
    let mut index_map =
        BTreeMap::<(String, String), (bool, Option<String>, Vec<String>, Vec<String>)>::new();
    for row in index_rows {
        let entry = index_map
            .entry((row.table_name, row.index_name))
            .or_insert_with(|| (row.is_unique, None, Vec::new(), Vec::new()));
        entry.0 = row.is_unique;
        if entry.1.is_none() {
            entry.1 = row.predicate;
        }
        if let Some(column) = row.column_name {
            entry.2.push(column);
        }
        if let Some(expression) = row.expression {
            entry.3.push(expression);
        }
    }

    let mut tables = table_rows
        .into_iter()
        // 框架自己的登记表不属于业务接入范围。
        .filter(|row| !row.table_name.starts_with("_yang_"))
        .map(|row| {
            let columns = column_rows
                .iter()
                .filter(|column| column.table_name == row.table_name)
                .map(|column| PgAdoptionColumn {
                    name: column.column_name.clone(),
                    formatted_type: column.formatted_type.clone(),
                    type_name: column.type_name.clone(),
                    type_kind: column.type_kind.clone(),
                    nullable: column.nullable,
                    primary_key: primary_keys
                        .get(&row.table_name)
                        .is_some_and(|keys| keys.contains(&column.column_name)),
                    auto_increment: column.auto_increment,
                    enum_labels: enum_labels
                        .get(&column.type_name)
                        .cloned()
                        .unwrap_or_default(),
                })
                .collect::<Vec<_>>();
            let indexes = index_map
                .iter()
                .filter(|((table_name, _), _)| table_name == &row.table_name)
                .map(
                    |((_, name), (unique, predicate, columns, expressions))| PgAdoptionIndex {
                        name: name.clone(),
                        unique: *unique,
                        columns: columns.clone(),
                        expressions: expressions.clone(),
                        predicate: predicate.clone(),
                    },
                )
                .collect();
            PgAdoptionTable {
                name: row.table_name.clone(),
                columns,
                primary_key: primary_keys
                    .get(&row.table_name)
                    .cloned()
                    .unwrap_or_default(),
                indexes,
            }
        })
        .collect::<Vec<_>>();
    tables.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(PgAdoptionReport { schema, tables })
}

fn type_length(formatted_type: &str) -> Option<usize> {
    formatted_type
        .rsplit_once('(')?
        .1
        .trim_end_matches(')')
        .parse()
        .ok()
}

fn numeric_precision_scale(formatted_type: &str) -> Option<(u8, u8)> {
    let inner = formatted_type.rsplit_once('(')?.1.trim_end_matches(')');
    let (precision, scale) = inner.split_once(',')?;
    Some((precision.trim().parse().ok()?, scale.trim().parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(type_name: &str, formatted_type: &str, type_kind: &str) -> PgAdoptionColumn {
        PgAdoptionColumn {
            name: "sample".to_string(),
            formatted_type: formatted_type.to_string(),
            type_name: type_name.to_string(),
            type_kind: type_kind.to_string(),
            nullable: false,
            primary_key: false,
            auto_increment: false,
            enum_labels: Vec::new(),
        }
    }

    #[test]
    fn common_types_map_to_schema_dsl() {
        assert_eq!(
            column("varchar", "character varying(32)", "b").suggested_field(),
            Some("Field::string(\"sample\", 32).required()".to_string())
        );
        assert_eq!(
            column("numeric", "numeric(12,3)", "b").suggested_field(),
            Some("Field::decimal(\"sample\", 12, 3).required()".to_string())
        );
        assert_eq!(
            column("_int8", "bigint[]", "b").suggested_field(),
            Some("Field::array(\"sample\", FieldType::BigInt).required()".to_string())
        );
    }

    #[test]
    fn enum_and_unsupported_types_are_distinguished() {
        let mut status = column("order_status", "order_status", "e");
        status.enum_labels = vec!["pending".to_string(), "paid".to_string()];
        assert_eq!(
            status.suggested_field(),
            Some(
                "Field::enumeration(\"sample\", [\"pending\", \"paid\"]).native_enum(\"order_status\").required()"
                    .to_string()
            )
        );
        assert_eq!(column("inet", "inet", "b").suggested_field(), None);
        // 无时区 timestamp 与框架 DateTime 语义不一致，必须留给人工决策。
        assert_eq!(
            column("timestamp", "timestamp without time zone", "b").suggested_field(),
            None
        );
    }
}
