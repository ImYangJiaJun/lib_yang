//! PostgreSQL `pg_catalog` / `information_schema` introspection。
//!
//! 读取列、主键、唯一/普通索引、CHECK、外键、默认值、identity/serial、生成列与
//! 列注释，并把类型名归一化为内部 `SchemaColumn` 模型（`int4` → `integer`、
//! `character varying` → `varchar`、`numeric` → `decimal`、`timestamptz` 保留精度等）。

use super::model::{ExistingCheck, ExistingForeignKey, ExistingIndex, ExistingTableSchema};
use crate::error::BaseError;
use crate::table::{SchemaColumn, TableDefinition};
use sqlx::postgres::PgConnection;
use sqlx::FromRow;
use std::collections::{BTreeMap, BTreeSet};

pub(super) async fn load_existing_schema(
    connection: &mut PgConnection,
    definition: &TableDefinition,
) -> Result<ExistingTableSchema, BaseError> {
    let config = definition.shared_config();
    let table = config.as_ref();
    let (schema, table_name) = split_table_name(&table.table_name);

    let oid: Option<i64> = sqlx::query_scalar(
        "SELECT c.oid::bigint FROM pg_class c \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE c.relname = $1 AND n.nspname = COALESCE($2, current_schema()) \
           AND c.relkind IN ('r', 'p')",
    )
    .bind(table_name)
    .bind(schema)
    .fetch_optional(&mut *connection)
    .await
    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
    if oid.is_none() {
        return Ok(ExistingTableSchema::missing());
    }

    // PG18 起 `pg_attribute.attgenerated` 新增 `'v'`（virtual generated column）；
    // 低版本只有 `'s'`。版本探针用于把新语义显式门控，避免未来版本新增取值被
    // 当成普通列。`yang_db::postgres::Database::server_version_num` 需要 Database
    // 实例，而 introspection 只持有 `&mut PgConnection`；这里读取连接握手阶段缓存
    // 的同一口径版本号，缺失时退回 `SHOW server_version_num`。
    let version = server_version_num(connection).await?;

    #[derive(FromRow)]
    struct ColumnRow {
        column_name: String,
        type_name: String,
        formatted_type: String,
        not_null: bool,
        default_expr: Option<String>,
        identity_kind: String,
        generated_kind: String,
        comment: Option<String>,
        max_length: Option<i64>,
        collation_name: Option<String>,
    }

    // collation 只回读裸名 `collname`：公共 API（`FieldConfig::collation`）不支持
    // `schema.collation` 限定写法，拼 schema 前缀会让自建 collation 每次同步都被
    // 判为漂移并反复 ALTER。
    let column_rows: Vec<ColumnRow> = sqlx::query_as(
        "SELECT a.attname::text AS column_name, \
                t.typname::text AS type_name, \
                format_type(a.atttypid, a.atttypmod) AS formatted_type, \
                a.attnotnull AS not_null, \
                pg_get_expr(d.adbin, d.adrelid) AS default_expr, \
                a.attidentity::text AS identity_kind, \
                a.attgenerated::text AS generated_kind, \
                col_description(a.attrelid, a.attnum) AS comment, \
                CASE WHEN t.typname IN ('varchar', 'bpchar') AND a.atttypmod > 4 \
                     THEN (a.atttypmod - 4)::bigint ELSE NULL END AS max_length, \
                CASE WHEN a.attcollation <> t.typcollation AND a.attcollation <> 0 \
                     THEN co.collname::text ELSE NULL END AS collation_name \
         FROM pg_attribute a \
         JOIN pg_class c ON c.oid = a.attrelid \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         JOIN pg_type t ON t.oid = a.atttypid \
         LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum \
         LEFT JOIN pg_collation co ON co.oid = a.attcollation \
         WHERE c.relname = $1 AND n.nspname = COALESCE($2, current_schema()) \
           AND a.attnum > 0 AND NOT a.attisdropped \
         ORDER BY a.attnum",
    )
    .bind(table_name)
    .bind(schema)
    .fetch_all(&mut *connection)
    .await
    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;

    let mut comments = BTreeMap::new();
    let mut identity_columns = BTreeSet::new();
    let mut generated_columns = BTreeMap::new();
    let mut collations = BTreeMap::new();
    let mut columns = Vec::with_capacity(column_rows.len());
    for row in column_rows {
        let identity = !row.identity_kind.is_empty()
            || row
                .default_expr
                .as_deref()
                .is_some_and(|expression| expression.starts_with("nextval("));
        let generated = is_generated_column(version, &row.generated_kind);
        let (data_type, column_type) =
            normalize_pg_type(&row.type_name, &row.formatted_type, row.max_length);
        if identity {
            identity_columns.insert(row.column_name.clone());
        }
        if generated {
            // 生成表达式存放在 pg_attrdef，读回用于检测声明漂移。
            generated_columns.insert(
                row.column_name.clone(),
                row.default_expr.clone().unwrap_or_default(),
            );
        }
        // identity / serial / 生成列的默认值不是声明默认值，不能参与漂移判断。
        let database_default = if identity || generated {
            None
        } else {
            row.default_expr
        };
        if let Some(comment) = row.comment {
            comments.insert(row.column_name.clone(), comment);
        }
        if let Some(collation) = row.collation_name {
            collations.insert(row.column_name.clone(), collation);
        }
        columns.push(SchemaColumn::new(
            row.column_name,
            data_type,
            column_type,
            !row.not_null,
            row.max_length.and_then(|length| u64::try_from(length).ok()),
            database_default,
        ));
    }

    let primary_key: Vec<String> = sqlx::query_scalar(
        "SELECT a.attname::text \
         FROM pg_constraint con \
         JOIN pg_class c ON c.oid = con.conrelid \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         JOIN unnest(con.conkey) WITH ORDINALITY AS key(attnum, ord) ON true \
         JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = key.attnum \
         WHERE con.contype = 'p' AND c.relname = $1 \
           AND n.nspname = COALESCE($2, current_schema()) \
         ORDER BY key.ord",
    )
    .bind(table_name)
    .bind(schema)
    .fetch_all(&mut *connection)
    .await
    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;

    #[derive(FromRow)]
    struct IndexRow {
        index_name: String,
        is_unique: bool,
        is_valid: bool,
        nulls_not_distinct: bool,
        predicate: Option<String>,
        column_name: Option<String>,
        /// 表达式键（`attnum = 0`）的规范化文本；普通列键为 `NULL`。
        expression: Option<String>,
    }

    // 部分索引（`indpred`）与表达式键必须完整反射：只按列名比较会把这些索引
    // 误判成普通索引（部分索引）或空列索引（表达式索引），进而重复 CREATE 或
    // 静默跳过声明所需的完整唯一约束。
    //
    // `indnullsnotdistinct` 是 PG15 新增列，低版本必须显式退化为 false，
    // 否则反射查询会因未知列失败。
    let nulls_not_distinct_column = if version >= PG15_VERSION_NUM {
        "ix.indnullsnotdistinct"
    } else {
        "false"
    };
    let index_query = format!(
        "SELECT i.relname::text AS index_name, \
                ix.indisunique AS is_unique, \
                ix.indisvalid AS is_valid, \
                {nulls_not_distinct_column} AS nulls_not_distinct, \
                pg_get_expr(ix.indpred, ix.indrelid) AS predicate, \
                a.attname::text AS column_name, \
                CASE WHEN key.attnum = 0 \
                     THEN pg_get_indexdef(ix.indexrelid, key.ord::integer, true) \
                     ELSE NULL END AS expression \
         FROM pg_index ix \
         JOIN pg_class t ON t.oid = ix.indrelid \
         JOIN pg_namespace n ON n.oid = t.relnamespace \
         JOIN pg_class i ON i.oid = ix.indexrelid \
         JOIN unnest(ix.indkey::smallint[]) WITH ORDINALITY AS key(attnum, ord) ON true \
         LEFT JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = key.attnum \
         WHERE t.relname = $1 AND n.nspname = COALESCE($2, current_schema()) \
           AND NOT ix.indisprimary \
           AND NOT EXISTS ( \
               SELECT 1 FROM pg_constraint con \
               WHERE con.conindid = ix.indexrelid AND con.contype IN ('u', 'x', 'p') \
           ) \
           AND key.ord <= ix.indnkeyatts::bigint \
         ORDER BY i.relname, key.ord"
    );
    let index_rows: Vec<IndexRow> = sqlx::query_as(&index_query)
        .bind(table_name)
        .bind(schema)
        .fetch_all(&mut *connection)
        .await
        .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
    #[derive(Default)]
    struct ReflectedIndex {
        unique: bool,
        valid: bool,
        nulls_not_distinct: bool,
        predicate: Option<String>,
        columns: Vec<String>,
        expressions: Vec<String>,
    }
    let mut index_map: BTreeMap<String, ReflectedIndex> = BTreeMap::new();
    for row in index_rows {
        let entry = index_map.entry(row.index_name).or_default();
        entry.unique = row.is_unique;
        entry.valid = row.is_valid;
        entry.nulls_not_distinct = row.nulls_not_distinct;
        if entry.predicate.is_none() {
            entry.predicate = row.predicate;
        }
        if let Some(column_name) = row.column_name {
            entry.columns.push(column_name);
        }
        if let Some(expression) = row.expression {
            entry.expressions.push(expression);
        }
    }
    let indexes = index_map
        .into_iter()
        .map(|(name, reflected)| {
            let index = ExistingIndex::new(name, reflected.unique, reflected.columns)
                .with_valid(reflected.valid)
                .with_nulls_not_distinct(reflected.nulls_not_distinct)
                .with_expressions(reflected.expressions);
            match reflected.predicate {
                Some(predicate) => index.with_predicate(predicate),
                None => index,
            }
        })
        .collect();

    #[derive(FromRow)]
    struct CheckRow {
        constraint_name: String,
        definition: String,
        validated: bool,
    }

    let check_rows: Vec<CheckRow> = sqlx::query_as(
        "SELECT con.conname::text AS constraint_name, \
                pg_get_constraintdef(con.oid) AS definition, \
                con.convalidated AS validated \
         FROM pg_constraint con \
         JOIN pg_class c ON c.oid = con.conrelid \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE con.contype = 'c' AND c.relname = $1 \
           AND n.nspname = COALESCE($2, current_schema()) \
         ORDER BY con.conname",
    )
    .bind(table_name)
    .bind(schema)
    .fetch_all(&mut *connection)
    .await
    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
    let checks = check_rows
        .into_iter()
        .map(|row| ExistingCheck {
            name: row.constraint_name,
            expression: strip_check_definition(&row.definition),
            validated: row.validated,
        })
        .collect();

    #[derive(FromRow)]
    struct ForeignKeyRow {
        constraint_name: String,
        column_name: String,
        referenced_table: String,
        referenced_column: String,
        update_rule: String,
        delete_rule: String,
        validated: bool,
    }

    let foreign_key_rows: Vec<ForeignKeyRow> = sqlx::query_as(
        "SELECT con.conname::text AS constraint_name, \
                source.attname::text AS column_name, \
                CASE WHEN target_namespace.nspname = n.nspname \
                     THEN target.relname::text \
                     ELSE target_namespace.nspname || '.' || target.relname::text END \
                     AS referenced_table, \
                target_attribute.attname::text AS referenced_column, \
                con.confupdtype::text AS update_rule, \
                con.confdeltype::text AS delete_rule, \
                con.convalidated AS validated \
         FROM pg_constraint con \
         JOIN pg_class c ON c.oid = con.conrelid \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         JOIN pg_class target ON target.oid = con.confrelid \
         JOIN pg_namespace target_namespace ON target_namespace.oid = target.relnamespace \
         JOIN unnest(con.conkey) WITH ORDINALITY AS source_key(attnum, ord) ON true \
         JOIN pg_attribute source ON source.attrelid = c.oid AND source.attnum = source_key.attnum \
         JOIN unnest(con.confkey) WITH ORDINALITY AS target_key(attnum, ord) \
           ON target_key.ord = source_key.ord \
         JOIN pg_attribute target_attribute \
           ON target_attribute.attrelid = con.confrelid AND target_attribute.attnum = target_key.attnum \
         WHERE con.contype = 'f' AND c.relname = $1 \
           AND n.nspname = COALESCE($2, current_schema()) \
         ORDER BY con.conname, source_key.ord",
    )
    .bind(table_name)
    .bind(schema)
    .fetch_all(&mut *connection)
    .await
    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
    let mut foreign_key_map = BTreeMap::<String, ExistingForeignKey>::new();
    for row in foreign_key_rows {
        let foreign_key = foreign_key_map
            .entry(row.constraint_name.clone())
            .or_insert_with(|| ExistingForeignKey {
                name: row.constraint_name,
                columns: Vec::new(),
                referenced_table: row.referenced_table,
                referenced_columns: Vec::new(),
                update_rule: foreign_key_rule(&row.update_rule),
                delete_rule: foreign_key_rule(&row.delete_rule),
                validated: row.validated,
            });
        foreign_key.columns.push(row.column_name);
        foreign_key.referenced_columns.push(row.referenced_column);
    }
    let foreign_keys = foreign_key_map.into_values().collect();

    let quoted_table = yang_db::postgres::quote_table(&table.table_name).map_err(|error| {
        BaseError::ConfigError(format!("非法数据库表名 {:?}: {error}", table.table_name))
    })?;
    let has_rows: Option<i32> =
        sqlx::query_scalar(&format!("SELECT 1 FROM {quoted_table} LIMIT 1"))
            .fetch_optional(&mut *connection)
            .await
            .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;

    Ok(ExistingTableSchema::existing(columns, primary_key, indexes)
        .with_constraints(checks, foreign_keys)
        .with_comments(comments)
        .with_column_kinds(identity_columns, generated_columns)
        .with_collations(collations)
        .with_rows(has_rows.is_some()))
}

/// 拆分 `schema.table`；未限定 schema 时返回 `None`，由 `current_schema()` 解析。
fn split_table_name(table_name: &str) -> (Option<&str>, &str) {
    match table_name.split_once('.') {
        Some((schema, table)) => (Some(schema), table),
        None => (None, table_name),
    }
}

/// PostgreSQL 18 的 `server_version_num` 下界。
const PG18_VERSION_NUM: u32 = 180_000;

/// PostgreSQL 15 的 `server_version_num` 下界（`NULLS NOT DISTINCT`、`indnullsnotdistinct`）。
pub(super) const PG15_VERSION_NUM: u32 = 150_000;

/// 读取当前连接的 `server_version_num`。
///
/// 优先使用 sqlx 握手阶段缓存的版本号（零额外往返，与
/// `yang_db::postgres::Database::server_version_num` 同一口径）；缺失时退回
/// `SHOW server_version_num`。
pub(super) async fn server_version_num(connection: &mut PgConnection) -> Result<u32, BaseError> {
    if let Some(version) = connection.server_version_num() {
        return Ok(version);
    }
    let raw: String = sqlx::query_scalar("SHOW server_version_num")
        .fetch_one(&mut *connection)
        .await
        .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
    parse_server_version_num(&raw)
}

/// 把 `SHOW server_version_num` 的文本结果解析为整数（如 `160015`）。
fn parse_server_version_num(raw: &str) -> Result<u32, BaseError> {
    raw.trim().parse::<u32>().map_err(|error| {
        BaseError::DatabaseInitFailed(format!(
            "无法解析 PostgreSQL server_version_num {raw:?}: {error}"
        ))
    })
}

/// 判断 `pg_attribute.attgenerated` 取值是否代表生成列。
///
/// `'s'` 为 PG12+ 的 stored generated column；`'v'` 是 PG18 新增的 virtual
/// generated column，只在 PG18 及以上视为生成列，禁止在低版本提前接受。
fn is_generated_column(server_version_num: u32, generated_kind: &str) -> bool {
    match generated_kind {
        "s" => true,
        "v" => server_version_num >= PG18_VERSION_NUM,
        _ => false,
    }
}

/// 把 `pg_type.typname` + `format_type` 归一化为内部类型模型。
fn normalize_pg_type(
    type_name: &str,
    formatted_type: &str,
    max_length: Option<i64>,
) -> (String, String) {
    match type_name {
        "int2" => ("smallint".to_string(), "smallint".to_string()),
        "int4" => ("integer".to_string(), "integer".to_string()),
        "int8" => ("bigint".to_string(), "bigint".to_string()),
        "varchar" => (
            "varchar".to_string(),
            format!("varchar({})", max_length.unwrap_or_default()),
        ),
        "bpchar" => (
            "char".to_string(),
            format!("char({})", max_length.unwrap_or_default()),
        ),
        "text" => ("text".to_string(), "text".to_string()),
        "bool" => ("boolean".to_string(), "boolean".to_string()),
        "date" => ("date".to_string(), "date".to_string()),
        "timestamp" => ("datetime".to_string(), formatted_type.to_string()),
        "timestamptz" => ("timestamptz".to_string(), formatted_type.to_string()),
        "float4" => ("float".to_string(), "real".to_string()),
        "float8" => ("double".to_string(), "double precision".to_string()),
        "numeric" => (
            "decimal".to_string(),
            formatted_type.replace("numeric", "decimal"),
        ),
        "json" => ("json".to_string(), "json".to_string()),
        "jsonb" => ("jsonb".to_string(), "jsonb".to_string()),
        "uuid" => ("uuid".to_string(), "uuid".to_string()),
        "bytea" => ("bytea".to_string(), "bytea".to_string()),
        // 内置数组的 `udt_name` 形如 `_int4`；归一化为 `<内部元素名>[]`，
        // 未知元素类型保留 `_xxx` 原名（是否兼容由 `postgres_type_compatible`
        // 判为不匹配，从而 fail-closed，绝不静默降级为 text）。
        "_int2" => ("smallint[]".to_string(), "smallint[]".to_string()),
        "_int4" => ("integer[]".to_string(), "integer[]".to_string()),
        "_int8" => ("bigint[]".to_string(), "bigint[]".to_string()),
        "_float4" => ("float[]".to_string(), "real[]".to_string()),
        "_float8" => ("double[]".to_string(), "double precision[]".to_string()),
        "_numeric" => (
            "decimal[]".to_string(),
            formatted_type.replace("numeric", "decimal"),
        ),
        "_bool" => ("boolean[]".to_string(), "boolean[]".to_string()),
        "_text" => ("text[]".to_string(), "text[]".to_string()),
        "_varchar" => ("varchar[]".to_string(), formatted_type.to_string()),
        "_bpchar" => ("char[]".to_string(), formatted_type.to_string()),
        "_date" => ("date[]".to_string(), "date[]".to_string()),
        "_timestamp" => ("datetime[]".to_string(), formatted_type.to_string()),
        "_timestamptz" => ("timestamptz[]".to_string(), formatted_type.to_string()),
        "_uuid" => ("uuid[]".to_string(), "uuid[]".to_string()),
        "_bytea" => ("bytea[]".to_string(), "bytea[]".to_string()),
        "_json" => ("json[]".to_string(), "json[]".to_string()),
        "_jsonb" => ("jsonb[]".to_string(), "jsonb[]".to_string()),
        other => (other.to_ascii_lowercase(), formatted_type.to_string()),
    }
}

/// `pg_get_constraintdef` 返回 `CHECK ((...))`（未验证约束还带 `NOT VALID`），
/// 去掉前缀与 `NOT VALID` 后缀保留表达式。
fn strip_check_definition(definition: &str) -> String {
    let trimmed = definition.trim();
    let without_prefix = if trimmed
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("check"))
    {
        trimmed[5..].trim_start()
    } else {
        trimmed
    };
    without_prefix
        .trim_end()
        .strip_suffix("NOT VALID")
        .map_or_else(
            || without_prefix.trim_end().to_string(),
            |value| value.trim_end().to_string(),
        )
}

/// 把 `pg_constraint.conf{upd,del}type` 的字符码还原成规则名。
fn foreign_key_rule(code: &str) -> String {
    match code {
        "r" => "RESTRICT",
        "c" => "CASCADE",
        "n" => "SET NULL",
        "d" => "SET DEFAULT",
        _ => "NO ACTION",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_pg_type_maps_catalog_names_to_internal_model() {
        assert_eq!(
            normalize_pg_type("int4", "integer", None),
            ("integer".to_string(), "integer".to_string())
        );
        assert_eq!(
            normalize_pg_type("int8", "bigint", None),
            ("bigint".to_string(), "bigint".to_string())
        );
        assert_eq!(
            normalize_pg_type("varchar", "character varying(32)", Some(32)),
            ("varchar".to_string(), "varchar(32)".to_string())
        );
        assert_eq!(
            normalize_pg_type("numeric", "numeric(10,2)", None),
            ("decimal".to_string(), "decimal(10,2)".to_string())
        );
        assert_eq!(
            normalize_pg_type("jsonb", "jsonb", None),
            ("jsonb".to_string(), "jsonb".to_string())
        );
        // 时间精度必须保留；类型语义按 DateTime 归一。
        assert_eq!(
            normalize_pg_type("timestamptz", "timestamp(3) with time zone", None),
            (
                "timestamptz".to_string(),
                "timestamp(3) with time zone".to_string()
            )
        );
    }

    #[test]
    fn normalize_pg_type_maps_uuid_bytea_and_arrays() {
        assert_eq!(
            normalize_pg_type("uuid", "uuid", None),
            ("uuid".to_string(), "uuid".to_string())
        );
        assert_eq!(
            normalize_pg_type("bytea", "bytea", None),
            ("bytea".to_string(), "bytea".to_string())
        );
        assert_eq!(
            normalize_pg_type("_int4", "integer[]", None),
            ("integer[]".to_string(), "integer[]".to_string())
        );
        assert_eq!(
            normalize_pg_type("_int8", "bigint[]", None),
            ("bigint[]".to_string(), "bigint[]".to_string())
        );
        assert_eq!(
            normalize_pg_type("_text", "text[]", None),
            ("text[]".to_string(), "text[]".to_string())
        );
        assert_eq!(
            normalize_pg_type("_bool", "boolean[]", None),
            ("boolean[]".to_string(), "boolean[]".to_string())
        );
        assert_eq!(
            normalize_pg_type("_uuid", "uuid[]", None),
            ("uuid[]".to_string(), "uuid[]".to_string())
        );
        assert_eq!(
            normalize_pg_type("_bytea", "bytea[]", None),
            ("bytea[]".to_string(), "bytea[]".to_string())
        );

        // 未知数组元素保持 `_xxx` 原名，后续兼容判定 fail-closed。
        assert_eq!(
            normalize_pg_type("_money", "money[]", None),
            ("_money".to_string(), "money[]".to_string())
        );
    }

    #[test]
    fn check_definition_and_foreign_key_rules_are_normalized() {
        assert_eq!(
            strip_check_definition("CHECK ((value > 0))"),
            "((value > 0))"
        );
        assert_eq!(
            strip_check_definition("CHECK ((status = ANY (ARRAY['a'::text])))"),
            "((status = ANY (ARRAY['a'::text])))"
        );
        assert_eq!(foreign_key_rule("r"), "RESTRICT");
        assert_eq!(foreign_key_rule("a"), "NO ACTION");
        assert_eq!(foreign_key_rule("c"), "CASCADE");
    }

    #[test]
    fn table_name_splits_schema_qualification() {
        assert_eq!(split_table_name("audit.users"), (Some("audit"), "users"));
        assert_eq!(split_table_name("users"), (None, "users"));
    }

    /// PG18 才有 virtual generated column（`attgenerated = 'v'`）；低版本必须忽略。
    #[test]
    fn generated_columns_are_version_gated() {
        assert!(!is_generated_column(170_000, ""));
        assert!(is_generated_column(160_015, "s"));
        assert!(!is_generated_column(170_000, "v"));
        assert!(is_generated_column(180_000, "v"));
        // 未知取值在任何版本都不当作生成列。
        assert!(!is_generated_column(180_000, "x"));
    }

    #[test]
    fn server_version_num_text_is_parsed() {
        assert_eq!(
            parse_server_version_num("160015").expect("PG16 版本号应可解析"),
            160_015
        );
        assert_eq!(
            parse_server_version_num(" 180000\n").expect("应容忍空白"),
            180_000
        );
        assert!(parse_server_version_num("not-a-version").is_err());
    }
}
