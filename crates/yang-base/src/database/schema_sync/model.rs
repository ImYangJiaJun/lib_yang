use super::SchemaSyncChange;
use crate::table::{ForeignKeyConfig, SchemaColumn};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExistingIndex {
    pub(super) name: String,
    pub(super) unique: bool,
    pub(super) columns: Vec<String>,
    /// PostgreSQL 15+ 唯一索引的 `NULLS NOT DISTINCT`（`pg_index.indnullsnotdistinct`）。
    /// MySQL 无法表达，恒为 false。
    pub(super) nulls_not_distinct: bool,
    /// PostgreSQL `pg_index.indisvalid`；并发创建失败会留下 INVALID 索引。
    pub(super) valid: bool,
    /// PostgreSQL 部分索引谓词（`pg_get_expr(indpred, indrelid)`）；普通索引为 `None`。
    ///
    /// 部分索引只约束满足谓词的行，不能等同于声明中的普通唯一/普通索引。
    pub(super) predicate: Option<String>,
    /// PostgreSQL 表达式键（`pg_get_indexdef(indexrelid, n, true)`），按索引键顺序排列。
    ///
    /// 普通列键仍记录在 [`Self::columns`]；混合索引会同时含两者。DSL 声明的索引
    /// 只有普通列键，因此表达式键非空时永远不满足声明比较。
    pub(super) expressions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExistingCheck {
    pub(super) name: String,
    pub(super) expression: String,
    /// PostgreSQL `pg_constraint.convalidated`；`NOT VALID` 添加后尚未验证时为 false。
    /// MySQL 无法表达该状态，恒为 true。
    pub(super) validated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExistingForeignKey {
    pub(super) name: String,
    pub(super) columns: Vec<String>,
    pub(super) referenced_table: String,
    pub(super) referenced_columns: Vec<String>,
    pub(super) update_rule: String,
    pub(super) delete_rule: String,
    /// PostgreSQL `pg_constraint.convalidated`；`NOT VALID` 添加后尚未验证时为 false。
    /// MySQL 无法表达该状态，恒为 true。
    pub(super) validated: bool,
}

impl ExistingIndex {
    pub(crate) fn new(name: impl Into<String>, unique: bool, columns: Vec<String>) -> Self {
        Self {
            name: name.into(),
            unique,
            columns,
            nulls_not_distinct: false,
            valid: true,
            predicate: None,
            expressions: Vec::new(),
        }
    }

    /// 记录 PostgreSQL 唯一索引的 `NULLS NOT DISTINCT` 语义。
    #[cfg(feature = "postgres")]
    pub(crate) fn with_nulls_not_distinct(mut self, nulls_not_distinct: bool) -> Self {
        self.nulls_not_distinct = nulls_not_distinct;
        self
    }

    /// 标记索引是否有效；PostgreSQL 并发创建失败会留下 INVALID 索引。
    #[cfg(feature = "postgres")]
    pub(crate) fn with_valid(mut self, valid: bool) -> Self {
        self.valid = valid;
        self
    }

    /// 记录 PostgreSQL 部分索引谓词。
    #[cfg(feature = "postgres")]
    pub(crate) fn with_predicate(mut self, predicate: String) -> Self {
        self.predicate = Some(predicate);
        self
    }

    /// 记录 PostgreSQL 表达式键。
    #[cfg(feature = "postgres")]
    pub(crate) fn with_expressions(mut self, expressions: Vec<String>) -> Self {
        self.expressions = expressions;
        self
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
    pub(super) comments: BTreeMap<String, String>,
    /// 由 PostgreSQL identity / serial 默认值表达的列（`pg_attribute.attidentity`）。
    pub(super) identity_columns: BTreeSet<String>,
    /// PostgreSQL 生成列（`attgenerated`）及其生成表达式。
    pub(super) generated_columns: BTreeMap<String, String>,
    /// PostgreSQL 列级 collation（`pg_attribute.attcollation`），仅在显式指定时记录。
    pub(super) collations: BTreeMap<String, String>,
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
            generated_columns: BTreeMap::new(),
            collations: BTreeMap::new(),
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
            generated_columns: BTreeMap::new(),
            collations: BTreeMap::new(),
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

    /// 记录 identity / serial 与生成列；plan 阶段使用这些中立集合判断自增与
    /// 生成列漂移，不再依赖 `column_type` 字符串。
    #[cfg(feature = "postgres")]
    pub(super) fn with_column_kinds(
        mut self,
        identity_columns: BTreeSet<String>,
        generated_columns: BTreeMap<String, String>,
    ) -> Self {
        self.identity_columns = identity_columns;
        self.generated_columns = generated_columns;
        self
    }

    /// 记录 PostgreSQL 列级 collation。
    #[cfg(feature = "postgres")]
    pub(super) fn with_collations(mut self, collations: BTreeMap<String, String>) -> Self {
        self.collations = collations;
        self
    }

    /// 返回把显式改名应用到列名/索引/元数据后的快照，供 plan 做差异检测。
    pub(super) fn with_renamed_columns(&self, renamed_columns: &BTreeMap<String, String>) -> Self {
        let rename = |name: &str| {
            renamed_columns
                .get(name)
                .cloned()
                .unwrap_or_else(|| name.to_string())
        };
        let mut effective = self.clone();
        for column in &mut effective.columns {
            column.name = rename(&column.name);
        }
        effective.primary_key = self.primary_key.iter().map(|name| rename(name)).collect();
        for index in &mut effective.indexes {
            index.columns = index.columns.iter().map(|name| rename(name)).collect();
        }
        effective.identity_columns = self
            .identity_columns
            .iter()
            .map(|name| rename(name))
            .collect();
        effective.generated_columns = self
            .generated_columns
            .iter()
            .map(|(name, expression)| (rename(name), expression.clone()))
            .collect();
        effective.collations = self
            .collations
            .iter()
            .map(|(name, collation)| (rename(name), collation.clone()))
            .collect();
        effective.comments = self
            .comments
            .iter()
            .map(|(name, comment)| (rename(name), comment.clone()))
            .collect();
        effective
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
    /// 事务阶段（或 MySQL 的与事务等价阶段）执行的语句。
    pub(crate) statements: Vec<String>,
    /// 必须在事务提交后以 autocommit 执行的语句（如 `VALIDATE CONSTRAINT`），
    /// 避免 `ACCESS EXCLUSIVE` 锁一直持有到 `COMMIT`。仍计入同一 change。
    pub(crate) deferred_statements: Vec<String>,
    pub(crate) change: SchemaSyncChange,
    /// PostgreSQL 并发 DDL（`CREATE/DROP INDEX CONCURRENTLY`）不能在事务块内执行。
    pub(crate) concurrent: bool,
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
    /// PostgreSQL 15+ 唯一索引的 `NULLS NOT DISTINCT` 语义。
    pub(super) nulls_not_distinct: bool,
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
