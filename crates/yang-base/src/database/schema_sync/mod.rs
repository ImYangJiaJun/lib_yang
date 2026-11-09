//! 基于模块 [`TableDefinition`] 的启动期保数据 schema 演进。
//!
//! 同步器创建缺失结构，并支持显式列改名、受控列修改、唯一索引、CHECK 与外键；
//! 永不自动删除未知列、索引或约束。任何可能被旧数据阻止的变更都会先只读扫描，
//! 返回表、对象和主键后拒绝全部 DDL。
//!
//! 方言差异集中在 [`dialect::SchemaDialect`]，数据库访问由 [`driver::SchemaDriver`]
//! 分派：MySQL 使用早前行为（`GET_LOCK`、DDL 隐式提交）；PostgreSQL 使用
//! `pg_advisory_lock(bigint)` 串行化多实例启动，并把全部 DDL 放入单事务，失败整体回滚。

mod dialect;
mod driver;
#[cfg(feature = "mysql")]
mod inspect;
mod model;
#[cfg(feature = "postgres")]
mod pg;
mod plan;
mod preflight;
#[cfg(feature = "mysql")]
mod render;
mod sync;

#[cfg(test)]
mod __tests__;

#[cfg(all(test, feature = "mysql"))]
pub(super) use model::SchemaPreflightCheck;
#[cfg(test)]
pub(super) use model::{ExistingIndex, ExistingTableSchema};
#[cfg(all(test, feature = "mysql"))]
pub(super) use plan::plan_table_sync;
#[cfg(all(test, feature = "postgres"))]
pub(super) use plan::plan_table_sync_with;

/// schema 同步变更类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SchemaSyncChangeKind {
    /// 创建整张表。
    CreatedTable,
    /// 增加字段。
    AddedColumn,
    /// 原位改名字段并保留全部数据。
    RenamedColumn,
    /// 在旧数据预检通过后修改字段类型、NULL 或默认值。
    ModifiedColumn,
    /// 增加主键。
    AddedPrimaryKey,
    /// 增加普通或唯一索引。
    AddedIndex,
    /// 增加 CHECK 约束。
    AddedCheck,
    /// 增加外键约束。
    AddedForeignKey,
}

/// 一项实际执行的 schema 变更。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SchemaSyncChange {
    /// 表名。
    pub table: String,
    /// 变更对象；建表时等于表名，其它变更为字段或索引名。
    pub object: String,
    /// 变更类型。
    pub kind: SchemaSyncChangeKind,
}

/// 启动期 schema 同步报告。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct SchemaSyncReport {
    /// 已检查的表名，按名称确定性排序。
    pub tables: Vec<String>,
    /// 本次实际执行的 additive 变更。
    pub changes: Vec<SchemaSyncChange>,
}

/// 一项阻止 Schema 更新的旧数据问题。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SchemaDataViolation {
    /// 问题表。
    pub table: String,
    /// 将要增加的约束或索引名。
    pub object: String,
    /// 命中的主键文本，最多返回 20 条且按主键排序。
    pub primary_keys: Vec<String>,
}

/// 只读 Schema 预检结果。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct SchemaPreflightReport {
    /// 待执行的结构变更。
    pub plan: SchemaSyncReport,
    /// 必须人工处理的旧数据问题。
    pub violations: Vec<SchemaDataViolation>,
}

impl SchemaPreflightReport {
    /// 是否可以安全进入 DDL 阶段。
    pub fn is_safe(&self) -> bool {
        self.violations.is_empty()
    }
}

impl SchemaSyncReport {
    /// 是否没有执行任何 DDL。
    pub fn is_noop(&self) -> bool {
        self.changes.is_empty()
    }
}
