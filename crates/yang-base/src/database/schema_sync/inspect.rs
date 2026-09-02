use super::model::{ExistingCheck, ExistingForeignKey, ExistingIndex, ExistingTableSchema};
use super::render::quote_identifier;
use crate::error::BaseError;
use crate::table::{SchemaColumn, TableDefinition};
use sqlx::mysql::MySqlConnection;
use sqlx::FromRow;
use std::collections::BTreeMap;

pub(super) async fn load_existing_schema(
    connection: &mut MySqlConnection,
    definition: &TableDefinition,
) -> Result<ExistingTableSchema, BaseError> {
    let table = definition.config();
    #[derive(FromRow)]
    struct ColumnRow {
        column_name: String,
        data_type: String,
        column_type: String,
        is_nullable: String,
        character_maximum_length: Option<i64>,
        column_default: Option<String>,
        extra: String,
    }

    #[derive(FromRow)]
    struct IndexRow {
        index_name: String,
        non_unique: i64,
        column_name: String,
    }

    #[derive(FromRow)]
    struct CheckRow {
        constraint_name: String,
        check_clause: String,
    }

    #[derive(FromRow)]
    struct ForeignKeyRow {
        constraint_name: String,
        column_name: String,
        referenced_table_name: String,
        referenced_column_name: String,
        update_rule: String,
        delete_rule: String,
    }

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name = ?",
    )
    .bind(&table.table_name)
    .fetch_one(&mut *connection)
    .await
    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
    if count == 0 {
        return Ok(ExistingTableSchema::missing());
    }

    let column_rows: Vec<ColumnRow> = sqlx::query_as(
        "SELECT CAST(COLUMN_NAME AS CHAR) AS column_name, CAST(DATA_TYPE AS CHAR) AS data_type, CAST(COLUMN_TYPE AS CHAR) AS column_type, CAST(IS_NULLABLE AS CHAR) AS is_nullable, CAST(CHARACTER_MAXIMUM_LENGTH AS SIGNED) AS character_maximum_length, CAST(COLUMN_DEFAULT AS CHAR) AS column_default, CAST(EXTRA AS CHAR) AS extra FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name = ? ORDER BY ORDINAL_POSITION",
    )
    .bind(&table.table_name)
    .fetch_all(&mut *connection)
    .await
    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
    let columns = column_rows
        .into_iter()
        .map(|row| {
            SchemaColumn::new(
                row.column_name,
                row.data_type,
                row.column_type,
                row.is_nullable.eq_ignore_ascii_case("YES"),
                row.character_maximum_length
                    .and_then(|length| u64::try_from(length).ok()),
                row.column_default,
            )
            .with_auto_increment(
                row.extra
                    .split_whitespace()
                    .any(|part| part.eq_ignore_ascii_case("auto_increment")),
            )
        })
        .collect();

    let primary_key: Vec<String> = sqlx::query_scalar(
        "SELECT CAST(COLUMN_NAME AS CHAR) FROM information_schema.statistics WHERE table_schema = DATABASE() AND table_name = ? AND index_name = 'PRIMARY' ORDER BY SEQ_IN_INDEX",
    )
    .bind(&table.table_name)
    .fetch_all(&mut *connection)
    .await
    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;

    let index_rows: Vec<IndexRow> = sqlx::query_as(
        "SELECT CAST(INDEX_NAME AS CHAR) AS index_name, CAST(NON_UNIQUE AS SIGNED) AS non_unique, CAST(COLUMN_NAME AS CHAR) AS column_name FROM information_schema.statistics WHERE table_schema = DATABASE() AND table_name = ? AND index_name <> 'PRIMARY' ORDER BY INDEX_NAME, SEQ_IN_INDEX",
    )
    .bind(&table.table_name)
    .fetch_all(&mut *connection)
    .await
    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
    let mut index_map: BTreeMap<String, (bool, Vec<String>)> = BTreeMap::new();
    for row in index_rows {
        let entry = index_map
            .entry(row.index_name)
            .or_insert_with(|| (row.non_unique == 0, Vec::new()));
        entry.1.push(row.column_name);
    }
    let indexes = index_map
        .into_iter()
        .map(|(name, (unique, columns))| ExistingIndex::new(name, unique, columns))
        .collect();

    let check_rows: Vec<CheckRow> = sqlx::query_as(
        "SELECT CAST(tc.CONSTRAINT_NAME AS CHAR) AS constraint_name, \
                CAST(cc.CHECK_CLAUSE AS CHAR) AS check_clause \
         FROM information_schema.TABLE_CONSTRAINTS AS tc \
         INNER JOIN information_schema.CHECK_CONSTRAINTS AS cc \
           ON cc.CONSTRAINT_SCHEMA = tc.CONSTRAINT_SCHEMA \
          AND cc.CONSTRAINT_NAME = tc.CONSTRAINT_NAME \
         WHERE tc.TABLE_SCHEMA = DATABASE() AND tc.TABLE_NAME = ? \
           AND tc.CONSTRAINT_TYPE = 'CHECK' \
         ORDER BY tc.CONSTRAINT_NAME",
    )
    .bind(&table.table_name)
    .fetch_all(&mut *connection)
    .await
    .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;
    let checks = check_rows
        .into_iter()
        .map(|row| ExistingCheck {
            name: row.constraint_name,
            expression: row.check_clause,
        })
        .collect();

    let foreign_key_rows: Vec<ForeignKeyRow> = sqlx::query_as(
        "SELECT CAST(kcu.CONSTRAINT_NAME AS CHAR) AS constraint_name, \
                CAST(kcu.COLUMN_NAME AS CHAR) AS column_name, \
                CAST(kcu.REFERENCED_TABLE_NAME AS CHAR) AS referenced_table_name, \
                CAST(kcu.REFERENCED_COLUMN_NAME AS CHAR) AS referenced_column_name, \
                CAST(rc.UPDATE_RULE AS CHAR) AS update_rule, \
                CAST(rc.DELETE_RULE AS CHAR) AS delete_rule \
         FROM information_schema.KEY_COLUMN_USAGE AS kcu \
         INNER JOIN information_schema.REFERENTIAL_CONSTRAINTS AS rc \
           ON rc.CONSTRAINT_SCHEMA = kcu.CONSTRAINT_SCHEMA \
          AND rc.CONSTRAINT_NAME = kcu.CONSTRAINT_NAME \
         WHERE kcu.TABLE_SCHEMA = DATABASE() AND kcu.TABLE_NAME = ? \
           AND kcu.REFERENCED_TABLE_NAME IS NOT NULL \
         ORDER BY kcu.CONSTRAINT_NAME, kcu.ORDINAL_POSITION",
    )
    .bind(&table.table_name)
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
                referenced_table: row.referenced_table_name,
                referenced_columns: Vec::new(),
                update_rule: row.update_rule,
                delete_rule: row.delete_rule,
            });
        foreign_key.columns.push(row.column_name);
        foreign_key
            .referenced_columns
            .push(row.referenced_column_name);
    }
    let foreign_keys = foreign_key_map.into_values().collect();

    let quoted_table = quote_identifier(&table.table_name)?;
    let has_rows: Option<i64> =
        sqlx::query_scalar(&format!("SELECT 1 FROM {quoted_table} LIMIT 1"))
            .fetch_optional(&mut *connection)
            .await
            .map_err(|error| BaseError::DatabaseQueryFailed(error.into()))?;

    Ok(ExistingTableSchema::existing(columns, primary_key, indexes)
        .with_constraints(checks, foreign_keys)
        .with_rows(has_rows.is_some()))
}
