use super::SchemaSyncChange;
use crate::table::SchemaColumn;

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
        }
    }

    pub(super) fn with_rows(mut self, has_rows: bool) -> Self {
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
}

#[derive(Debug)]
pub(crate) struct TableSyncPlan {
    pub(crate) statements: Vec<String>,
    pub(crate) changes: Vec<SchemaSyncChange>,
    pub(super) preflight: Vec<SchemaPreflightCheck>,
}

#[derive(Debug)]
pub(super) enum SchemaPreflightCheck {
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
