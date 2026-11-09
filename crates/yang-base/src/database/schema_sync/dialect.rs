//! 声明式 schema 同步的方言渲染与类型兼容判定。
//!
//! 本模块只做纯字符串/纯逻辑计算，不接触数据库连接；MySQL 分支逐字节复用
//! [`super::render`] 的既有渲染，PostgreSQL 分支输出双引号标识符、无 `ENGINE`
//! 与内联 `KEY` 的 DDL。数据库访问由 [`super::driver`] 分派。

use super::model::{DesiredIndex, ExistingTableSchema};
use crate::error::BaseError;
use crate::table::{
    CheckConfig, FieldConfig, FieldType, ForeignKeyConfig, SchemaIssueKind, SchemaValidationReport,
    TableConfig, TableDefinition,
};
#[cfg(feature = "postgres")]
use crate::table::{SchemaColumn, SchemaIssue, POSTGRES_MAX_VARCHAR_LENGTH};
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

    /// 校验并引用 PostgreSQL collation 名称。
    ///
    /// collation 名称允许 ICU 的连字符形式（如 `en-US-x-icu`）与 libc 的点号形式
    /// （如 `en_US.utf8`），因此不能直接复用只接受 `[A-Za-z0-9_]` 的标识符校验。
    /// 这里把整个名称当作**单个**标识符双引号包裹（点号是名称的一部分，不是
    /// schema 限定符），仍拒绝引号/NUL/空白/分号并限制 63 字节。
    /// 本方法不支持 `schema.collation` 限定写法。
    ///
    /// 大小写任意的 `default` 统一渲染为 `"default"`：PostgreSQL 只识别小写
    /// `COLLATE "default"`，`"DEFAULT"`/`"Default"` 会报 42704；而归一化比较
    /// 已把该名称视为“类型默认”，两者必须一致。
    #[cfg(feature = "postgres")]
    pub(super) fn quote_collation(self, collation: &str) -> Result<String, BaseError> {
        let trimmed = collation.trim();
        if trimmed.is_empty()
            || trimmed.chars().any(|character| {
                character == '"'
                    || character == '\0'
                    || character.is_whitespace()
                    || character == ';'
            })
        {
            return Err(BaseError::ConfigError(format!(
                "非法 collation 名称: {collation:?}"
            )));
        }
        ensure_postgres_identifier_bytes("collation", trimmed)?;
        if trimmed.eq_ignore_ascii_case("default") {
            return Ok("\"default\"".to_string());
        }
        Ok(format!("\"{trimmed}\""))
    }

    /// 是否支持列注释同步；MySQL 需要在 MODIFY COLUMN 中重述完整类型，暂不支持。
    pub(super) fn supports_column_comments(self) -> bool {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql => false,
            #[cfg(feature = "postgres")]
            Self::PostgreSql => true,
        }
    }

    /// 渲染 PostgreSQL `COMMENT ON COLUMN`。
    pub(super) fn render_comment_on_column(
        self,
        table: &TableConfig,
        field: &FieldConfig,
    ) -> Result<String, BaseError> {
        let comment = field
            .comment
            .as_deref()
            .ok_or_else(|| BaseError::ConfigError(format!("字段 {} 未声明注释", field.name)))?;
        Ok(format!(
            "COMMENT ON COLUMN {}.{} IS {}",
            self.quote_table(&table.table_name)?,
            self.quote_identifier(&field.name)?,
            self.quote_string(comment)
        ))
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
        self.validate_identifier_limits(table)?;
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
            match self {
                #[cfg(feature = "mysql")]
                Self::MySql => {
                    if field.generated.is_some() || field.collation.is_some() {
                        return Err(BaseError::ConfigError(format!(
                            "表 {} 的字段 {} 使用了 PostgreSQL 专属的 generated/collation 声明",
                            table.table_name, field.name
                        )));
                    }
                }
                #[cfg(feature = "postgres")]
                Self::PostgreSql => {
                    if field.generated.is_some() && field.auto_increment {
                        return Err(BaseError::ConfigError(format!(
                            "表 {} 的生成列 {} 不能同时声明自增",
                            table.table_name, field.name
                        )));
                    }
                    if let Some(collation) = &field.collation {
                        self.quote_collation(collation)?;
                    }
                }
            }
            let _ = self.render_column(field)?;
        }
        let _ = self.desired_indexes(table)?;
        let _ = self.desired_checks(table)?;
        Ok(())
    }

    /// PostgreSQL 的标识符上限是 63 字节（NUL 终止），超长会被服务端静默截断。
    ///
    /// 用户显式声明的标识符一律 fail-closed；自动生成的索引名在 `desired_indexes`
    /// 中用稳定 hash 截断，避免超长表名/列名生成不可复现的名字。
    fn validate_identifier_limits(self, table: &TableConfig) -> Result<(), BaseError> {
        #[cfg(feature = "postgres")]
        if matches!(self, Self::PostgreSql) {
            for part in table.table_name.split('.') {
                ensure_postgres_identifier_bytes("表名", part)?;
            }
            for field in table.fields.values() {
                ensure_postgres_identifier_bytes("字段名", &field.name)?;
            }
            for index in table.unique_indexes.iter().chain(table.indexes.iter()) {
                if let Some(name) = &index.name {
                    ensure_postgres_identifier_bytes("索引名", name)?;
                }
            }
            for check in &table.checks {
                ensure_postgres_identifier_bytes("约束名", &check.name)?;
            }
            for foreign_key in &table.foreign_keys {
                ensure_postgres_identifier_bytes("约束名", &foreign_key.name)?;
                for part in foreign_key.referenced_table.split('.') {
                    ensure_postgres_identifier_bytes("外键目标表名", part)?;
                }
                for column in foreign_key
                    .columns
                    .iter()
                    .chain(foreign_key.referenced_columns.iter())
                {
                    ensure_postgres_identifier_bytes("外键列名", column)?;
                }
            }
        }
        #[cfg(not(feature = "postgres"))]
        let _ = table;
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
        let definitions = normalized
            .into_values()
            .map(|(definition, _)| definition)
            .collect::<Vec<_>>();
        self.validate_index_namespace(&definitions)?;
        Ok(definitions)
    }

    /// PostgreSQL 索引名在 schema 内唯一；跨表重复会让第二次 `CREATE INDEX` 失败。
    pub(super) fn validate_index_namespace(
        self,
        definitions: &[&TableDefinition],
    ) -> Result<(), BaseError> {
        #[cfg(feature = "postgres")]
        if matches!(self, Self::PostgreSql) {
            let mut seen: BTreeMap<(Option<String>, String), String> = BTreeMap::new();
            for definition in definitions {
                let config = definition.shared_config();
                let table = config.as_ref();
                let schema = table
                    .table_name
                    .split_once('.')
                    .map(|(schema, _)| schema.to_string());
                for index in self.desired_indexes(table)? {
                    let key = (schema.clone(), index.name.clone());
                    match seen.get(&key) {
                        Some(existing_table) if existing_table != &table.table_name => {
                            return Err(BaseError::ConfigError(format!(
                                "索引名 {} 在同一 schema 内被表 {} 与 {} 重复使用",
                                index.name, existing_table, table.table_name
                            )));
                        }
                        Some(_) => {}
                        None => {
                            seen.insert(key, table.table_name.clone());
                        }
                    }
                }
            }
        }
        #[cfg(not(feature = "postgres"))]
        let _ = definitions;
        Ok(())
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
                    statements.push(self.render_create_index_on_table(table, &index, false)?);
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
                for check in self.desired_checks(table)? {
                    definitions.push(self.render_check(&check)?);
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
                let mut definition = format!("{} {sql_type}", self.quote_identifier(&field.name)?);
                if let Some(collation) = &field.collation {
                    definition.push_str(" COLLATE ");
                    definition.push_str(&self.quote_collation(collation)?);
                }
                definition.push(' ');
                definition.push_str(if field.required { "NOT NULL" } else { "NULL" });
                if field.auto_increment {
                    definition.push_str(" GENERATED BY DEFAULT AS IDENTITY");
                }
                if let Some(expression) = &field.generated {
                    definition.push_str(" GENERATED ALWAYS AS (");
                    definition.push_str(expression);
                    definition.push_str(") STORED");
                    return Ok(definition);
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
            // 防御性断言：构建期已强制同一上界；PostgreSQL 方言按自己的上限再次
            // 校验，避免双 feature 构建受 MySQL 上界影响。
            FieldType::String { max_length }
                if (1..=POSTGRES_MAX_VARCHAR_LENGTH).contains(max_length) =>
            {
                Ok(format!("VARCHAR({max_length})"))
            }
            FieldType::String { max_length } => Err(BaseError::ConfigError(format!(
                "字段 {} 的 VARCHAR 长度必须在 1..={POSTGRES_MAX_VARCHAR_LENGTH}: {}",
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
            // PostgreSQL 没有内联 ENUM；用 varchar + CHECK 表达，取值约束由
            // `desired_checks` 生成的隐式 CHECK 保证，不再 fail-closed。
            FieldType::Enum { values } => {
                if values.is_empty() || values.iter().any(|value| value.is_empty()) {
                    return Err(BaseError::ConfigError(format!(
                        "枚举字段 {} 必须声明非空值",
                        field_name
                    )));
                }
                let max_length = values
                    .iter()
                    .map(|value| value.chars().count())
                    .max()
                    .unwrap_or(1)
                    .max(1);
                if max_length > POSTGRES_MAX_VARCHAR_LENGTH {
                    return Err(BaseError::ConfigError(format!(
                        "枚举字段 {} 的取值超过 VARCHAR 上限 {POSTGRES_MAX_VARCHAR_LENGTH}",
                        field_name
                    )));
                }
                Ok(format!("VARCHAR({max_length})"))
            }
            FieldType::Uuid => Ok("UUID".to_string()),
            FieldType::Bytes => Ok("BYTEA".to_string()),
            // 数组元素类型递归渲染；嵌套数组与 JSON 元素无法稳定映射为数据库
            // 数组字面量，明确拒绝（构建期已拦截，这里是渲染期兜底）。
            FieldType::Array(element) => match element.as_ref() {
                FieldType::Array(_) | FieldType::Json => Err(BaseError::ConfigError(format!(
                    "字段 {} 的数组元素类型 {} 不受支持：仅允许标量元素",
                    field_name,
                    element.display_name()
                ))),
                element => Ok(format!("{}[]", self.render_type(element, field_name)?)),
            },
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
        existing_collation: Option<&str>,
    ) -> Result<Vec<String>, BaseError> {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql => {
                let _ = (existing_name, issues, existing_collation);
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
                let collation_changed =
                    !collations_equal(field.collation.as_deref(), existing_collation);
                if issues.contains(&SchemaIssueKind::IncompatibleType) || collation_changed {
                    let sql_type = self.render_type(&field.field_type, &field.name)?;
                    let collate = match &field.collation {
                        Some(collation) => {
                            format!(" COLLATE {}", self.quote_collation(collation)?)
                        }
                        // 显式 collation 被移除时重置到类型默认排序规则。
                        None if existing_collation.is_some() => " COLLATE \"default\"".to_string(),
                        None => String::new(),
                    };
                    statements.push(format!(
                        "ALTER TABLE {quoted_table} ALTER COLUMN {quoted_column} \
                         TYPE {sql_type}{collate} USING {quoted_column}::{sql_type}"
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

    /// 渲染索引创建。PostgreSQL 不允许内联 KEY；`concurrent` 用于已有表在线建索引。
    #[cfg(feature = "postgres")]
    pub(super) fn render_create_index_on_table(
        self,
        table: &TableConfig,
        index: &DesiredIndex,
        concurrent: bool,
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
        let concurrently = if concurrent { " CONCURRENTLY" } else { "" };
        // PG15+：NULLS NOT DISTINCT 位于列清单之后。
        let nulls_clause = if index.nulls_not_distinct {
            " NULLS NOT DISTINCT"
        } else {
            ""
        };
        Ok(format!(
            "CREATE {kind}{concurrently} {} ON {} ({columns}){nulls_clause}",
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
            Self::PostgreSql => self.render_create_index_on_table(table, index, true),
        }
    }

    /// 给已存在表添加索引是否必须脱离事务执行（PostgreSQL `CONCURRENTLY`）。
    pub(super) fn add_index_is_concurrent(self) -> bool {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql => false,
            #[cfg(feature = "postgres")]
            Self::PostgreSql => true,
        }
    }

    /// 渲染 `DROP INDEX CONCURRENTLY`（用于自愈 INVALID 索引）。
    pub(super) fn render_drop_index_concurrently(
        self,
        table: &TableConfig,
        index_name: &str,
    ) -> Result<String, BaseError> {
        let quoted_index = self.quote_identifier(index_name)?;
        let qualified = match table.table_name.split_once('.') {
            Some((schema, _)) => format!("{}.{quoted_index}", self.quote_identifier(schema)?),
            None => quoted_index,
        };
        Ok(format!("DROP INDEX CONCURRENTLY IF EXISTS {qualified}"))
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
    ///
    /// PostgreSQL 先 `NOT VALID` 再 `VALIDATE CONSTRAINT`（在线 DDL：添加阶段只取
    /// 短暂 `ACCESS EXCLUSIVE`，验证阶段仅 `SHARE UPDATE EXCLUSIVE`）；MySQL 单语句。
    pub(super) fn render_add_check(
        self,
        table: &TableConfig,
        check: &CheckConfig,
    ) -> Result<Vec<String>, BaseError> {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql => Ok(vec![format!(
                "ALTER TABLE {} ADD {}",
                self.quote_table(&table.table_name)?,
                self.render_check(check)?
            )]),
            #[cfg(feature = "postgres")]
            Self::PostgreSql => {
                let quoted_table = self.quote_table(&table.table_name)?;
                let quoted_name = self.quote_identifier(&check.name)?;
                Ok(vec![
                    format!(
                        "ALTER TABLE {quoted_table} ADD CONSTRAINT {quoted_name} CHECK ({}) NOT VALID",
                        check.expression
                    ),
                    format!("ALTER TABLE {quoted_table} VALIDATE CONSTRAINT {quoted_name}"),
                ])
            }
        }
    }

    /// 渲染 `VALIDATE CONSTRAINT`（仅 PostgreSQL）。
    ///
    /// 用于在线 DDL 与自愈：CHECK / 外键先以 `NOT VALID` 添加，或既有约束仍处于
    /// `NOT VALID` 状态时，单独执行验证；该语句必须在事务提交后以 autocommit 执行。
    pub(super) fn render_validate_constraint(
        self,
        table: &TableConfig,
        constraint_name: &str,
    ) -> Result<String, BaseError> {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql => {
                let _ = table;
                Err(BaseError::ConfigError(format!(
                    "MySQL 不支持单独验证约束: {constraint_name}"
                )))
            }
            #[cfg(feature = "postgres")]
            Self::PostgreSql => Ok(format!(
                "ALTER TABLE {} VALIDATE CONSTRAINT {}",
                self.quote_table(&table.table_name)?,
                self.quote_identifier(constraint_name)?
            )),
        }
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
    ///
    /// 与 CHECK 一致，PostgreSQL 使用 `NOT VALID` + `VALIDATE CONSTRAINT` 降低
    /// 已有表加外键时的锁级别；MySQL 保持单语句。
    pub(super) fn render_add_foreign_key(
        self,
        table: &TableConfig,
        foreign_key: &ForeignKeyConfig,
    ) -> Result<Vec<String>, BaseError> {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql => Ok(vec![format!(
                "ALTER TABLE {} ADD {}",
                self.quote_table(&table.table_name)?,
                self.render_foreign_key(foreign_key)?
            )]),
            #[cfg(feature = "postgres")]
            Self::PostgreSql => {
                let quoted_table = self.quote_table(&table.table_name)?;
                let quoted_name = self.quote_identifier(&foreign_key.name)?;
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
                Ok(vec![
                    format!(
                        "ALTER TABLE {quoted_table} ADD CONSTRAINT {quoted_name} FOREIGN KEY \
                         ({columns}) REFERENCES {} ({referenced_columns}) NOT VALID",
                        self.quote_table(&foreign_key.referenced_table)?
                    ),
                    format!("ALTER TABLE {quoted_table} VALIDATE CONSTRAINT {quoted_name}"),
                ])
            }
        }
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
                        let name = match &index.name {
                            Some(explicit) => {
                                ensure_postgres_identifier_bytes("索引名", explicit)?;
                                explicit.clone()
                            }
                            None => truncate_generated_identifier(
                                format!(
                                    "{prefix}_{}_{}",
                                    table_name_part(&table.table_name),
                                    index.fields.join("_")
                                ),
                                63,
                            ),
                        };
                        self.quote_identifier(&name)?;
                        desired.push(DesiredIndex {
                            name,
                            unique,
                            columns: index.fields.clone(),
                            nulls_not_distinct: index.nulls_not_distinct,
                        });
                    }
                }
                Ok(desired)
            }
        }
    }

    /// 实际需要落库的 CHECK 约束：显式声明 + PostgreSQL 枚举字段的隐式 CHECK。
    ///
    /// 枚举在 PostgreSQL 下渲染为 `varchar + CHECK`，隐式约束必须和显式约束一样
    /// 参与建表、偏移检测与二次同步幂等判断。
    pub(super) fn desired_checks(self, table: &TableConfig) -> Result<Vec<CheckConfig>, BaseError> {
        let checks = table.checks.clone();
        #[cfg(feature = "postgres")]
        let mut checks = checks;
        #[cfg(feature = "postgres")]
        if matches!(self, Self::PostgreSql) {
            let mut fields: Vec<&FieldConfig> = table.fields.values().collect();
            fields.sort_by(|left, right| left.name.cmp(&right.name));
            for field in fields {
                let FieldType::Enum { values } = &field.field_type else {
                    continue;
                };
                let allowed = values
                    .iter()
                    .map(|value| self.quote_string(value))
                    .collect::<Vec<_>>()
                    .join(", ");
                checks.push(CheckConfig {
                    name: truncate_generated_identifier(
                        format!("chk_{}_{}", table_name_part(&table.table_name), field.name),
                        63,
                    ),
                    expression: format!("{} IN ({allowed})", self.quote_identifier(&field.name)?),
                });
            }
        }
        Ok(checks)
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
        existing: &ExistingTableSchema,
    ) -> SchemaValidationReport {
        match self {
            #[cfg(feature = "mysql")]
            Self::MySql => table.validate_schema(&existing.columns),
            #[cfg(feature = "postgres")]
            Self::PostgreSql => validate_postgres_schema(table, existing),
        }
    }
}

/// 取表名中的裸表部分；自动生成的索引/约束名不能包含 schema 限定符的点。
#[cfg(feature = "postgres")]
fn table_name_part(table_name: &str) -> &str {
    table_name.rsplit('.').next().unwrap_or(table_name)
}

/// 校验单个标识符不超过 PostgreSQL 的 63 字节上限（用户显式标识符 fail-closed）。
#[cfg(feature = "postgres")]
fn ensure_postgres_identifier_bytes(kind: &str, identifier: &str) -> Result<(), BaseError> {
    if identifier.len() > 63 {
        return Err(BaseError::ConfigError(format!(
            "{kind}超过 PostgreSQL 63 字节限制: {identifier}（{} 字节）",
            identifier.len()
        )));
    }
    Ok(())
}

/// 用稳定 hash 截断自动生成的标识符，保证多次同步得到同一个名字。
///
/// 截断保留前缀并用 FNV-1a 的 8 位十六进制后缀标识原值，避免不同表/列的
/// 自动名在截断后碰撞。
#[cfg(feature = "postgres")]
fn truncate_generated_identifier(name: String, max_bytes: usize) -> String {
    if name.len() <= max_bytes {
        return name;
    }
    let hash = format!("{:08x}", fnv1a_64(name.as_bytes()) as u32);
    let keep = max_bytes.saturating_sub(hash.len() + 1);
    let mut prefix = String::new();
    for character in name.chars() {
        if prefix.len() + character.len_utf8() > keep {
            break;
        }
        prefix.push(character);
    }
    format!("{prefix}_{hash}")
}

/// FNV-1a 64 位哈希；仅用于稳定生成标识符，不追求密码学强度。
#[cfg(feature = "postgres")]
fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// 比较声明侧与读回侧 collation；`None` 表示未显式指定（类型默认）。
///
/// 只规范化真正的前缀：去掉开头的 `pg_catalog.` 与首尾双引号；名称等于
/// `default`（大小写不敏感）时归一为 `None` —— inspect 在
/// `attcollation == typcollation` 时返回 NULL，声明 `COLLATE "default"` 必须
/// 与之相等，否则每次同步都会整表重写。其余名称按 PostgreSQL 引号标识符语义
/// **精确比较**（`c` 与 `C` 是不同 collation）。点号是 libc collation 名称的
/// 一部分（`en_US.utf8`），不能再按最后一个点截断，否则 `en_US.utf8` 与
/// `de_DE.utf8` 会被误判为相等。
#[cfg(feature = "postgres")]
pub(super) fn collations_equal(desired: Option<&str>, actual: Option<&str>) -> bool {
    normalized_collation(desired) == normalized_collation(actual)
}

#[cfg(feature = "postgres")]
fn normalized_collation(name: Option<&str>) -> Option<String> {
    let normalized = normalize_collation_name(name?);
    if normalized.eq_ignore_ascii_case("default") {
        None
    } else {
        Some(normalized)
    }
}

#[cfg(feature = "postgres")]
fn normalize_collation_name(name: &str) -> String {
    let trimmed = name.trim().trim_matches('"');
    let without_catalog = trimmed.strip_prefix("pg_catalog.").unwrap_or(trimmed);
    without_catalog.trim_matches('"').to_string()
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
    existing: &ExistingTableSchema,
) -> SchemaValidationReport {
    let columns = &existing.columns;
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
        // 自增判定完全来自 pg_catalog 的 identity / serial 集合，不解析 column_type 文本。
        let actual_auto_increment = existing.identity_columns.contains(&field.name);
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
        let actual_generated = existing.generated_columns.get(&field.name);
        match (&field.generated, actual_generated) {
            (None, None) => {}
            (Some(expected), Some(actual)) => {
                if normalize_postgres_check_expression(expected)
                    != normalize_postgres_check_expression(actual)
                {
                    issues.push(SchemaIssue {
                        field: field.name.clone(),
                        kind: SchemaIssueKind::IncompatibleType,
                        expected: format!("GENERATED ALWAYS AS ({expected}) STORED"),
                        actual: Some(format!("GENERATED ALWAYS AS ({actual}) STORED")),
                    });
                }
            }
            (Some(expected), None) => issues.push(SchemaIssue {
                field: field.name.clone(),
                kind: SchemaIssueKind::IncompatibleType,
                expected: format!("GENERATED ALWAYS AS ({expected}) STORED"),
                actual: Some("普通列".to_string()),
            }),
            (None, Some(_)) => issues.push(SchemaIssue {
                field: field.name.clone(),
                kind: SchemaIssueKind::IncompatibleType,
                expected: "普通列".to_string(),
                actual: Some("生成列".to_string()),
            }),
        }
        let existing_collation = existing.collations.get(&field.name).map(String::as_str);
        if !collations_equal(field.collation.as_deref(), existing_collation) {
            issues.push(SchemaIssue {
                field: field.name.clone(),
                kind: SchemaIssueKind::IncompatibleType,
                expected: field
                    .collation
                    .clone()
                    .unwrap_or_else(|| "默认 collation".to_string()),
                actual: Some(
                    existing_collation.map_or_else(|| "默认 collation".to_string(), str::to_string),
                ),
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
        FieldType::Enum { values } => {
            let needed = values
                .iter()
                .map(|value| value.chars().count())
                .max()
                .unwrap_or(1)
                .max(1) as u64;
            match data_type {
                "text" => true,
                "varchar" | "char" => column.max_length.is_some_and(|actual| actual >= needed),
                _ => false,
            }
        }
        FieldType::Uuid => data_type == "uuid",
        FieldType::Bytes => data_type == "bytea",
        // 数组的 data_type 由 introspection 归一化为 `<内部元素名>[]`；未知元素
        // 类型保留 `_xxx` 原名，永远不匹配，从而 fail-closed。元素级修饰
        // （varchar 长度 / numeric 精度）只存在于完整列类型中，需单独比较。
        FieldType::Array(element) => {
            data_type == format!("{}[]", postgres_internal_type_name(element))
                && postgres_array_element_type_compatible(element, &column.column_type)
        }
    }
}

/// 比较数组列的元素长度/精度：`character varying(10)[]`、`numeric(12,3)[]`。
///
/// `data_type` 只保留 `varchar[]` / `decimal[]` 这样的元素基名，元素上限与
/// 精度必须从 `column_type` 解析；元素类型未声明修饰时视为兼容。
#[cfg(feature = "postgres")]
fn postgres_array_element_type_compatible(element: &FieldType, column_type: &str) -> bool {
    let Some(inner) = column_type.trim().strip_suffix("[]") else {
        return false;
    };
    let inner = inner.trim();
    match element {
        FieldType::String { max_length } => {
            postgres_character_type_length(inner).is_some_and(|actual| actual >= *max_length as u64)
        }
        FieldType::Enum { values } => {
            let needed = values
                .iter()
                .map(|value| value.chars().count())
                .max()
                .unwrap_or(1)
                .max(1) as u64;
            postgres_character_type_length(inner).is_some_and(|actual| actual >= needed)
        }
        FieldType::Decimal { precision, scale } => {
            inner.eq_ignore_ascii_case(&format!("numeric({precision},{scale})"))
                || inner.eq_ignore_ascii_case(&format!("decimal({precision},{scale})"))
        }
        _ => true,
    }
}

/// 从 `character varying(10)` / `varchar(10)` / `char(5)` 提取字符上限。
#[cfg(feature = "postgres")]
fn postgres_character_type_length(inner: &str) -> Option<u64> {
    let open = inner.rfind('(')?;
    let close = inner.rfind(')')?;
    if close <= open {
        return None;
    }
    inner[open + 1..close].trim().parse().ok()
}

/// 把声明字段类型映射为 introspection 使用的内部标量名（不含数组后缀）。
#[cfg(feature = "postgres")]
fn postgres_internal_type_name(field_type: &FieldType) -> &'static str {
    match field_type {
        FieldType::Integer => "integer",
        FieldType::BigInt => "bigint",
        FieldType::Float => "float",
        FieldType::Double => "double",
        FieldType::Decimal { .. } => "decimal",
        FieldType::Boolean => "boolean",
        FieldType::Date => "date",
        FieldType::DateTime => "timestamptz",
        FieldType::Timestamp => "bigint",
        FieldType::Json => "jsonb",
        FieldType::Text => "text",
        FieldType::String { .. } | FieldType::Enum { .. } => "varchar",
        FieldType::Uuid => "uuid",
        FieldType::Bytes => "bytea",
        // 嵌套数组在声明/渲染阶段已被拒绝；返回不可能匹配的名字兜底。
        FieldType::Array(_) => "array",
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
///
/// PostgreSQL 会在 `pg_get_constraintdef` 中给布尔操作数补分组括号，例如声明
/// `score >= 0 AND status <> 'banned'` 读回为
/// `((score >= 0) AND (status <> 'banned'))`。为此归一化期间先保留单个空格做词法
/// 分析，折叠包裹原子/原子比较/AND 分组的冗余括号，最后再统一去空白；OR 分组与
/// `NOT` 下的 AND 分组保持原样，避免把 `(a AND b) OR c` 与 `a AND (b OR c)`
/// 误判相等。
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
    let spaced = normalize_postgres_check_spacing(&without_casts);
    let spaced = rewrite_postgres_any_array(&spaced);
    let spaced = strip_simple_identifier_parentheses(&spaced);
    let spaced = strip_redundant_group_parentheses(&spaced);
    let spaced = strip_outer_parentheses(spaced);
    remove_postgres_check_whitespace(&spaced)
}

/// 去双引号、折叠空白为单个空格、小写关键字；单引号字面量内容原样保留。
#[cfg(feature = "postgres")]
fn normalize_postgres_check_spacing(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut quoted = false;
    let mut pending_space = false;
    for character in input.chars() {
        if character == '\'' {
            if pending_space && !output.is_empty() {
                output.push(' ');
            }
            pending_space = false;
            quoted = !quoted;
            output.push(character);
            continue;
        }
        if quoted {
            output.push(character);
            continue;
        }
        if character == '"' {
            continue;
        }
        if character.is_whitespace() {
            pending_space = true;
            continue;
        }
        if pending_space && !output.is_empty() {
            output.push(' ');
        }
        pending_space = false;
        output.extend(character.to_lowercase());
    }
    output
}

/// 最终比较用：去掉字面量之外的空白。
#[cfg(feature = "postgres")]
fn remove_postgres_check_whitespace(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut quoted = false;
    for character in input.chars() {
        if character == '\'' {
            quoted = !quoted;
            output.push(character);
            continue;
        }
        if !quoted && character.is_whitespace() {
            continue;
        }
        output.push(character);
    }
    output
}

/// 去掉仅包裹单个标识符的括号：`(status) in (...)` → `status in (...)`。
///
/// PostgreSQL 会把 `IN` 改写成 `ANY (ARRAY[...])`，并在左侧标识符外加括号；
/// 只折叠“内容全是标识符字符”的括号，且不误伤函数调用参数（`lower(email)`）。
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
        if current == '('
            && !previous_postgres_character(&characters, index).is_some_and(is_identifier_character)
        {
            if let Some(close) = find_matching_parenthesis(&characters, index) {
                let inner = characters[index + 1..close].iter().collect::<String>();
                let trimmed = inner.trim();
                let simple = !trimmed.is_empty()
                    && trimmed
                        .chars()
                        .all(|value| value.is_ascii_alphanumeric() || value == '_' || value == '.')
                    && !trimmed
                        .chars()
                        .next()
                        .is_some_and(|value| value.is_ascii_digit());
                if simple {
                    output.push_str(trimmed);
                    index = close + 1;
                    continue;
                }
            }
        }
        output.push(current);
        index += 1;
    }
    output
}

/// 折叠 PostgreSQL 为布尔操作数补加的冗余分组括号：
/// 原子比较、不涉及 OR 的 AND 分组（`NOT` 下除外）。OR 分组保留，
/// 因为 `(a AND b) OR c` 与 `a AND (b OR c)` 的括号具有语义。
#[cfg(feature = "postgres")]
fn strip_redundant_group_parentheses(expression: &str) -> String {
    let mut current = expression.to_string();
    loop {
        let characters = current.chars().collect::<Vec<_>>();
        let mut index = 0;
        let mut quoted = false;
        let mut changed = false;
        while index < characters.len() {
            let character = characters[index];
            if character == '\'' {
                quoted = !quoted;
                index += 1;
                continue;
            }
            if quoted {
                index += 1;
                continue;
            }
            if character != '(' {
                index += 1;
                continue;
            }
            let Some(close) = find_matching_parenthesis(&characters, index) else {
                index += 1;
                continue;
            };
            let inner = characters[index + 1..close].iter().collect::<String>();
            if should_strip_postgres_group(&characters, index, &inner) {
                let mut next = String::with_capacity(current.len());
                next.extend(&characters[..index]);
                next.push_str(&inner);
                next.extend(&characters[close + 1..]);
                current = next;
                changed = true;
                break;
            }
            index += 1;
        }
        if !changed {
            return current;
        }
    }
}

#[cfg(feature = "postgres")]
fn should_strip_postgres_group(characters: &[char], open: usize, inner: &str) -> bool {
    let previous = previous_postgres_character(characters, open);
    if previous.is_some_and(|character| matches!(character, ')' | ']')) {
        return false;
    }
    let previous_word = previous_postgres_word(characters, open).map(|word| word.to_lowercase());
    if previous_word
        .as_deref()
        .is_some_and(|word| !matches!(word, "and" | "or" | "not"))
    {
        // 函数调用或类型应用的参数括号，保持原样。
        return false;
    }
    // 只有布尔上下文（表达式开头、`(` 后，或 AND/OR/NOT 之后）的括号才可能是
    // 冗余的。比较/算术运算符右侧的括号具有语义，如 `flag = (x AND y)`，
    // 折叠后会与 `(flag = x) AND y` 混淆。
    let boolean_context = matches!(previous, None | Some('(')) || previous_word.is_some();
    if !boolean_context {
        return false;
    }
    let (has_and, has_or, has_comparison) = postgres_group_tokens(inner);
    if has_and || has_or {
        // OR 分组的括号具有语义，保留；AND 分组只有在 NOT 下才需要括号。
        return has_and && !has_or && previous_word.as_deref() != Some("not");
    }
    has_comparison
}

/// 判断分组内部顶层是否出现 AND/OR 或比较运算。
#[cfg(feature = "postgres")]
fn postgres_group_tokens(inner: &str) -> (bool, bool, bool) {
    let characters = inner.chars().collect::<Vec<_>>();
    let mut depth = 0_i32;
    let mut quoted = false;
    let mut word = String::new();
    let mut has_and = false;
    let mut has_or = false;
    let mut has_comparison = false;
    let mut index = 0;
    while index < characters.len() {
        let character = characters[index];
        if character == '\'' {
            if depth == 0 {
                flush_postgres_group_word(
                    &mut word,
                    &mut has_and,
                    &mut has_or,
                    &mut has_comparison,
                );
            }
            quoted = !quoted;
            index += 1;
            continue;
        }
        if quoted {
            index += 1;
            continue;
        }
        if depth == 0 && is_identifier_character(character) {
            word.extend(character.to_lowercase());
            index += 1;
            continue;
        }
        flush_postgres_group_word(&mut word, &mut has_and, &mut has_or, &mut has_comparison);
        match character {
            '(' => depth += 1,
            ')' => depth -= 1,
            '<' | '>' | '=' if depth == 0 => has_comparison = true,
            _ => {}
        }
        index += 1;
    }
    flush_postgres_group_word(&mut word, &mut has_and, &mut has_or, &mut has_comparison);
    (has_and, has_or, has_comparison)
}

#[cfg(feature = "postgres")]
fn flush_postgres_group_word(
    word: &mut String,
    has_and: &mut bool,
    has_or: &mut bool,
    has_comparison: &mut bool,
) {
    if word.is_empty() {
        return;
    }
    match word.as_str() {
        "and" => *has_and = true,
        "or" => *has_or = true,
        "is" | "in" | "like" | "ilike" | "between" | "any" | "all" | "similar" | "not"
        | "exists" | "null" | "true" | "false" => *has_comparison = true,
        _ => {}
    }
    word.clear();
}

#[cfg(feature = "postgres")]
fn find_matching_parenthesis(characters: &[char], open: usize) -> Option<usize> {
    let mut depth = 0_i32;
    let mut quoted = false;
    for (index, character) in characters.iter().enumerate().skip(open) {
        if *character == '\'' {
            quoted = !quoted;
            continue;
        }
        if quoted {
            continue;
        }
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(feature = "postgres")]
fn is_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

#[cfg(feature = "postgres")]
fn previous_postgres_character(characters: &[char], index: usize) -> Option<char> {
    characters[..index]
        .iter()
        .rev()
        .find(|character| !character.is_whitespace())
        .copied()
}

#[cfg(feature = "postgres")]
fn previous_postgres_word(characters: &[char], index: usize) -> Option<String> {
    let mut cursor = index;
    while cursor > 0 && characters[cursor - 1].is_whitespace() {
        cursor -= 1;
    }
    let end = cursor;
    while cursor > 0 && is_identifier_character(characters[cursor - 1]) {
        cursor -= 1;
    }
    (cursor < end).then(|| characters[cursor..end].iter().collect())
}

#[cfg(feature = "postgres")]
fn rewrite_postgres_any_array(expression: &str) -> String {
    let mut current = expression.to_string();
    loop {
        let Some((start, inner, tail)) = find_postgres_any_array(&current) else {
            return current;
        };
        current = format!("{}in({inner}){}", &current[..start], &current[tail..]);
    }
}

/// 定位 `= ANY ((ARRAY[...]))`，返回 `=` 的偏移、数组元素文本与匹配右括号后的偏移。
#[cfg(feature = "postgres")]
fn find_postgres_any_array(input: &str) -> Option<(usize, String, usize)> {
    let bytes = input.as_bytes();
    let mut search = 0;
    loop {
        let start = search + input[search..].find('=')?;
        let mut index = start + 1;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if !input[index..].starts_with("any") {
            search = start + 1;
            continue;
        }
        index += "any".len();
        if input[index..]
            .chars()
            .next()
            .is_some_and(is_identifier_character)
        {
            search = start + 1;
            continue;
        }
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        // PostgreSQL 可能额外包一层括号：`= ANY ((ARRAY[...]))`。
        let mut depth = 0_i32;
        while index < bytes.len() && bytes[index] == b'(' {
            depth += 1;
            index += 1;
        }
        if depth == 0 {
            search = start + 1;
            continue;
        }
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if !input[index..].starts_with("array[") {
            search = start + 1;
            continue;
        }
        let inner_start = index + "array[".len();
        let close_offset = input[inner_start..].find(']')?;
        let inner = input[inner_start..inner_start + close_offset].to_string();
        let mut cursor = inner_start + close_offset + 1;
        while cursor < bytes.len() && depth > 0 {
            match bytes[cursor] {
                b'(' => depth += 1,
                b')' => depth -= 1,
                b']' | b' ' | b'\t' | b'\n' | b'\r' => {}
                _ => return None,
            }
            cursor += 1;
        }
        if depth == 0 {
            return Some((start, inner, cursor));
        }
        search = start + 1;
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

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(any(feature = "mysql", feature = "postgres"))]
    use crate::table::{Field, Table};

    #[cfg(feature = "postgres")]
    #[test]
    fn postgres_renders_uuid_bytes_and_scalar_arrays() {
        let dialect = SchemaDialect::PostgreSql;
        assert_eq!(
            dialect
                .render_type(&FieldType::Uuid, "uid")
                .expect("UUID 应可渲染"),
            "UUID"
        );
        assert_eq!(
            dialect
                .render_type(&FieldType::Bytes, "payload")
                .expect("BYTEA 应可渲染"),
            "BYTEA"
        );
        assert_eq!(
            dialect
                .render_type(&FieldType::array(FieldType::Integer), "numbers")
                .expect("int4[] 应可渲染"),
            "INTEGER[]"
        );
        assert_eq!(
            dialect
                .render_type(&FieldType::array(FieldType::Text), "tags")
                .expect("text[] 应可渲染"),
            "TEXT[]"
        );
        assert_eq!(
            dialect
                .render_type(&FieldType::array(FieldType::Uuid), "uids")
                .expect("uuid[] 应可渲染"),
            "UUID[]"
        );

        // 嵌套数组与 JSON 元素明确拒绝。
        assert!(dialect
            .render_type(&FieldType::array(FieldType::array(FieldType::Text)), "bad")
            .is_err());
        assert!(dialect
            .render_type(&FieldType::array(FieldType::Json), "bad")
            .is_err());
    }

    /// PostgreSQL 方言按自己的上界校验：VARCHAR(100000) 在 PG 构建下必须可渲染。
    #[cfg(feature = "postgres")]
    #[test]
    fn postgres_ddl_accepts_varchar_above_mysql_limit() {
        let dialect = SchemaDialect::PostgreSql;
        assert_eq!(
            dialect
                .render_type(
                    &FieldType::String {
                        max_length: 100_000
                    },
                    "body"
                )
                .expect("PostgreSQL VARCHAR(100000) 应可渲染"),
            "VARCHAR(100000)"
        );
        assert!(dialect
            .render_type(
                &FieldType::String {
                    max_length: 10_485_761
                },
                "body"
            )
            .is_err());
    }

    /// 双 feature 构建：构建期接受 PostgreSQL 上界，但 MySQL 方言必须按
    /// MySQL 上界拒绝，不能把非法 DDL 发到 MySQL。
    #[cfg(all(feature = "mysql", feature = "postgres"))]
    #[test]
    fn dual_feature_mysql_ddl_rejects_varchar_above_mysql_limit() {
        let definition = Table::new("dual_varchar")
            .fields([Field::id("id"), Field::string("body", 100_000)])
            .build()
            .expect("双 feature 构建期应接受 PostgreSQL 上界");
        let config = definition.shared_config();
        assert!(SchemaDialect::MySql
            .render_create_table(config.as_ref(), false)
            .is_err());
    }

    #[cfg(feature = "mysql")]
    #[test]
    fn mysql_renders_uuid_as_char36_and_bytes_as_blob() {
        let definition = Table::new("mysql_types")
            .fields([Field::id("id"), Field::uuid("uid"), Field::bytes("payload")])
            .build()
            .expect("MySQL 类型声明应有效");
        let config = definition.shared_config();
        let uid = config.get_field("uid").expect("uid 应存在");
        let payload = config.get_field("payload").expect("payload 应存在");
        assert!(SchemaDialect::MySql
            .render_column(uid)
            .expect("UUID 应可渲染")
            .contains("CHAR(36)"));
        assert!(SchemaDialect::MySql
            .render_column(payload)
            .expect("BYTES 应可渲染")
            .contains("BLOB"));
    }

    #[cfg(feature = "mysql")]
    #[test]
    fn mysql_rejects_array_columns() {
        let definition = Table::new("mysql_array")
            .fields([Field::id("id"), Field::array("numbers", FieldType::Integer)])
            .build()
            .expect("数组声明本身合法");
        let config = definition.shared_config();
        let numbers = config.get_field("numbers").expect("numbers 应存在");
        assert!(SchemaDialect::MySql.render_column(numbers).is_err());
    }

    #[cfg(feature = "postgres")]
    #[test]
    fn postgres_renders_nulls_not_distinct_unique_index() {
        let definition = Table::new("pg_nnd")
            .fields([Field::id("id"), Field::string("code", 32)])
            .unique_nulls_not_distinct(["code"])
            .build()
            .expect("NULLS NOT DISTINCT 声明应有效");
        let config = definition.shared_config();
        let dialect = SchemaDialect::PostgreSql;
        let indexes = dialect
            .desired_indexes(config.as_ref())
            .expect("索引计划应成功");
        assert_eq!(indexes.len(), 1);
        assert!(indexes[0].nulls_not_distinct);

        let sql = dialect
            .render_add_index(config.as_ref(), &indexes[0])
            .expect("索引 DDL 应可渲染");
        assert!(
            sql.contains("UNIQUE INDEX") && sql.contains("NULLS NOT DISTINCT"),
            "DDL 必须携带 NULLS NOT DISTINCT: {sql}"
        );
    }

    #[cfg(feature = "mysql")]
    #[test]
    fn mysql_rejects_nulls_not_distinct_unique_index() {
        let definition = Table::new("mysql_nnd")
            .fields([Field::id("id"), Field::string("code", 32)])
            .unique_nulls_not_distinct(["code"])
            .build()
            .expect("定义层允许跨方言声明");
        let config = definition.shared_config();
        assert!(
            SchemaDialect::MySql
                .desired_indexes(config.as_ref())
                .is_err(),
            "MySQL 必须明确拒绝 NULLS NOT DISTINCT"
        );
    }

    /// 数组列的元素长度/精度必须参与兼容性比较，不能只看 `varchar[]`/`decimal[]`。
    #[cfg(feature = "postgres")]
    #[test]
    fn postgres_array_element_length_and_precision_are_compared() {
        let varchar10 = FieldType::array(FieldType::String { max_length: 10 });
        assert!(postgres_type_compatible(
            &varchar10,
            &SchemaColumn::new(
                "tags",
                "varchar[]",
                "character varying(10)[]",
                true,
                None,
                None
            )
        ));
        assert!(!postgres_type_compatible(
            &varchar10,
            &SchemaColumn::new(
                "tags",
                "varchar[]",
                "character varying(5)[]",
                true,
                None,
                None
            )
        ));
        // 无元素长度的裸 varchar[] 无法证明满足声明，必须判为不兼容。
        assert!(!postgres_type_compatible(
            &varchar10,
            &SchemaColumn::new("tags", "varchar[]", "varchar[]", true, None, None)
        ));

        let decimal = FieldType::array(FieldType::Decimal {
            precision: 12,
            scale: 3,
        });
        assert!(postgres_type_compatible(
            &decimal,
            &SchemaColumn::new("amounts", "decimal[]", "decimal(12,3)[]", true, None, None)
        ));
        assert!(!postgres_type_compatible(
            &decimal,
            &SchemaColumn::new("amounts", "decimal[]", "decimal(12,4)[]", true, None, None)
        ));
        assert!(!postgres_type_compatible(
            &decimal,
            &SchemaColumn::new("amounts", "decimal[]", "decimal(10,3)[]", true, None, None)
        ));

        // 无修饰的元素类型仍按基名比较。
        let ints = FieldType::array(FieldType::Integer);
        assert!(postgres_type_compatible(
            &ints,
            &SchemaColumn::new("numbers", "integer[]", "integer[]", true, None, None)
        ));
        assert!(!postgres_type_compatible(
            &ints,
            &SchemaColumn::new("numbers", "bigint[]", "bigint[]", true, None, None)
        ));
    }
}
