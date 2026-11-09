use super::SchemaSyncChange;
use crate::table::{ForeignKeyConfig, SchemaColumn};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExistingIndex {
    pub(super) name: String,
    pub(super) unique: bool,
    pub(super) columns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExistingCheck {
    pub(super) name: String,
    pub(super) expression: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExistingForeignKey {
    pub(super) name: String,
    pub(super) columns: Vec<String>,
    pub(super) referenced_table: String,
    pub(super) referenced_columns: Vec<String>,
    pub(super) update_rule: String,
    pub(super) delete_rule: String,
}

impl ExistingIndex {
    pub(crate) fn new(name: impl Into<String>, unique: bool, columns: Vec<String>) -> Self {
        Self {
            name: name.into(),
            unique,
            columns,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExistingTableSchema {
    pub(super) exists: bool,
    pub(super) columns: Vec<SchemaColumn>,
    pub(super) primary_key: Vec<String>,
    pub(super) indexes: Vec<ExistingIndex>,
    pub(super) checks: Vec<ExistingCheck>,
    pub(super) foreign_keys: Vec<ExistingForeignKey>,
    pub(super) has_rows: bool,
    /// 列注释；MySQL 空实现，PostgreSQL 从 `pg_description` 读取。
    /// 当前 plan 不消费注释，保留 introspection 结果供调试与后续能力扩展。
    #[allow(dead_code)]
    pub(super) comments: BTreeMap<String, String>,
    /// 由 PostgreSQL identity / serial 默认值表达的列。
    #[allow(dead_code)]
    pub(super) identity_columns: BTreeSet<String>,
    /// PostgreSQL 生成列（`attgenerated = 's'`）。
    #[allow(dead_code)]
    pub(super) generated_columns: BTreeSet<String>,
}

impl ExistingTableSchema {
    pub(crate) fn missing() -> Self {
        Self {
            exists: false,
            columns: Vec::new(),
            primary_key: Vec::new(),
            indexes: Vec::new(),
            checks: Vec::new(),
            foreign_keys: Vec::new(),
            has_rows: false,
            comments: BTreeMap::new(),
            identity_columns: BTreeSet::new(),
            generated_columns: BTreeSet::new(),
        }
    }

    pub(crate) fn existing(
        columns: Vec<SchemaColumn>,
        primary_key: Vec<String>,
        indexes: Vec<ExistingIndex>,
    ) -> Self {
        Self {
            exists: true,
            columns,
            primary_key,
            indexes,
            checks: Vec::new(),
            foreign_keys: Vec::new(),
            has_rows: false,
            comments: BTreeMap::new(),
            identity_columns: BTreeSet::new(),
            generated_columns: BTreeSet::new(),
        }
    }

    pub(crate) fn with_rows(mut self, has_rows: bool) -> Self {
        self.has_rows = has_rows;
        self
    }

    pub(super) fn with_constraints(
        mut self,
        checks: Vec<ExistingCheck>,
        foreign_keys: Vec<ExistingForeignKey>,
    ) -> Self {
        self.checks = checks;
        self.foreign_keys = foreign_keys;
        self
    }

    /// 记录 PostgreSQL introspection 的列注释。
    #[cfg(feature = "postgres")]
    pub(super) fn with_comments(mut self, comments: BTreeMap<String, String>) -> Self {
        self.comments = comments;
        self
    }

    /// 记录 identity / serial 与生成列；plan 阶段通过 `column_type` 标记消费
    /// auto_increment，这里保留原始集合供调试与测试读取。
    #[cfg(feature = "postgres")]
    pub(super) fn with_column_kinds(
        mut self,
        identity_columns: BTreeSet<String>,
        generated_columns: BTreeSet<String>,
    ) -> Self {
        self.identity_columns = identity_columns;
        self.generated_columns = generated_columns;
        self
    }
}

#[derive(Debug)]
pub(crate) struct TableSyncPlan {
    /// 扁平语句列表；MySQL 与 `changes` 一一对应，PostgreSQL 修改列时为多语句。
    /// 生产路径按 `steps` 执行，本字段保留给既有单元测试断言 SQL 文本。
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) statements: Vec<String>,
    pub(crate) changes: Vec<SchemaSyncChange>,
    /// 每项变更对应的完整语句组，执行阶段按此顺序应用。
    pub(crate) steps: Vec<PlanStep>,
    pub(crate) preflight: Vec<SchemaPreflightCheck>,
}

/// 一项 schema 变更及其需要执行的全部语句。
#[derive(Debug)]
pub(crate) struct PlanStep {
    pub(crate) statements: Vec<String>,
    pub(crate) change: SchemaSyncChange,
}

#[derive(Debug)]
pub(crate) enum SchemaPreflightCheck {
    ColumnPredicate {
        object: String,
        predicate: String,
    },
    Check {
        object: String,
        expression: String,
    },
    ForeignKey {
        object: String,
        columns: Vec<String>,
        referenced_table: String,
        referenced_columns: Vec<String>,
    },
    UniqueIndex {
        object: String,
        columns: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DesiredIndex {
    pub(super) name: String,
    pub(super) unique: bool,
    pub(super) columns: Vec<String>,
}

/// 把显式改名映射应用到列名。
pub(super) fn existing_column_name(
    column: &str,
    renamed_columns: &BTreeMap<String, String>,
) -> String {
    renamed_columns
        .iter()
        .find_map(|(legacy, current)| (current == column).then(|| legacy.clone()))
        .unwrap_or_else(|| column.to_string())
}

/// 外键定义是否一致；删除/更新规则只接受 RESTRICT / NO ACTION。
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
