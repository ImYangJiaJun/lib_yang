//! 声明式数据库结构同步入口。
//!
//! [`DatabaseInitializer`] 持有运行时容器 [`super::AnyDatabase`]，具体方言与
//! 连接类型由 schema 同步驱动按后端分派。

use super::AnyDatabase;
use crate::datasource::DatasourceId;
use crate::error::BaseError;
use crate::table::TableDefinition;
use crate::tools::Tools;
use std::collections::BTreeMap;

/// 按表声明的逻辑数据源分组执行只读 schema 预检。
///
/// 每个数据源使用独立连接与独立锁；返回逐源结果。任一源失败不会丢失其它源的
/// 结果，调用方必须逐源判断。
pub async fn preflight_table_definitions_grouped_outcomes(
    tools: &Tools,
    definitions: &[&TableDefinition],
) -> BTreeMap<DatasourceId, Result<super::SchemaPreflightReport, BaseError>> {
    let mut outcomes = BTreeMap::new();
    for (source, group) in group_definitions(definitions) {
        let outcome = match tools.database_named(&source) {
            Ok(database) => {
                DatabaseInitializer::from_any(database.clone())
                    .preflight_table_definitions(&group)
                    .await
            }
            Err(error) => Err(error),
        };
        outcomes.insert(source, outcome);
    }
    outcomes
}

/// 按表声明的逻辑数据源分组执行只读 schema 预检；任一源失败立即返回错误。
pub async fn preflight_table_definitions_grouped(
    tools: &Tools,
    definitions: &[&TableDefinition],
) -> Result<BTreeMap<DatasourceId, super::SchemaPreflightReport>, BaseError> {
    collect_outcomes(preflight_table_definitions_grouped_outcomes(tools, definitions).await)
}

/// 按表声明的逻辑数据源分组执行 schema 同步。
///
/// 每个数据源使用独立连接、锁与事务；返回逐源结果，调用方必须把部分成功
/// 当成逐源结果处理，不能假设跨源整体回滚。
pub async fn sync_table_definitions_grouped_outcomes(
    tools: &Tools,
    definitions: &[&TableDefinition],
) -> BTreeMap<DatasourceId, Result<super::SchemaSyncReport, BaseError>> {
    let mut outcomes = BTreeMap::new();
    for (source, group) in group_definitions(definitions) {
        let outcome = match tools.database_named(&source) {
            Ok(database) => {
                DatabaseInitializer::from_any(database.clone())
                    .sync_table_definitions(&group)
                    .await
            }
            Err(error) => Err(error),
        };
        outcomes.insert(source, outcome);
    }
    outcomes
}

/// 按表声明的逻辑数据源分组执行 schema 同步；任一源失败立即返回错误。
pub async fn sync_table_definitions_grouped(
    tools: &Tools,
    definitions: &[&TableDefinition],
) -> Result<BTreeMap<DatasourceId, super::SchemaSyncReport>, BaseError> {
    collect_outcomes(sync_table_definitions_grouped_outcomes(tools, definitions).await)
}

fn collect_outcomes<T>(
    outcomes: BTreeMap<DatasourceId, Result<T, BaseError>>,
) -> Result<BTreeMap<DatasourceId, T>, BaseError> {
    let mut reports = BTreeMap::new();
    for (source, outcome) in outcomes {
        reports.insert(source, outcome?);
    }
    Ok(reports)
}

fn group_definitions<'a>(
    definitions: &[&'a TableDefinition],
) -> BTreeMap<DatasourceId, Vec<&'a TableDefinition>> {
    let mut groups: BTreeMap<DatasourceId, Vec<&'a TableDefinition>> = BTreeMap::new();
    for definition in definitions {
        groups
            .entry(definition.datasource().clone())
            .or_default()
            .push(*definition);
    }
    groups
}

/// 持有数据库连接并执行 [`crate::table::TableDefinition`] 声明的结构同步。
pub struct DatabaseInitializer {
    /// 显式拥有的数据库实例。
    db: AnyDatabase,
}

impl DatabaseInitializer {
    /// 用 MySQL 连接构造同步器（保持既有调用方式）。
    ///
    /// 逻辑源固定为 `default`；具名数据源必须使用
    /// [`Self::from_any`] 并传入 `tools.database_named(id)?` 的容器。
    #[cfg(feature = "mysql")]
    pub fn new(db: yang_db::mysql::Database) -> Self {
        Self::from_any(AnyDatabase::mysql(db))
    }

    /// 用 PostgreSQL 连接构造同步器。
    ///
    /// 逻辑源固定为 `default`；接入具名源时同样应使用 [`Self::from_any`]。
    #[cfg(feature = "postgres")]
    pub fn new_postgres(db: yang_db::postgres::Database) -> Self {
        Self::from_any(AnyDatabase::postgres(db))
    }

    /// 用运行时数据库容器构造同步器。
    pub fn from_any(db: AnyDatabase) -> Self {
        Self { db }
    }

    /// 返回底层数据库容器。
    pub(crate) fn db(&self) -> &AnyDatabase {
        &self.db
    }
}
