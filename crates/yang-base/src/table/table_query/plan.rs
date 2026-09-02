//! 查询计划编译：将受保护的查询参数翻译为 `yang_db` 谓词与 `QueryBuilder`。

#![cfg(feature = "mysql")]

use super::TableQuery;
use crate::error::BaseError;
use crate::table::{SortOrder, WhereCondition};
use serde_json::Value;

impl TableQuery {
    #[cfg(feature = "mysql")]
    fn compile_predicate(
        &self,
        condition: &WhereCondition,
    ) -> Result<yang_db::Predicate, BaseError> {
        let field = |name: &str| {
            self.table_config
                .get_field_ref(name)
                .cloned()
                .ok_or_else(|| {
                    BaseError::FieldNotFound(self.table_config.table_name.clone(), name.to_string())
                })
        };
        Ok(match condition {
            WhereCondition::Eq { field: name, value } => {
                yang_db::Predicate::Compare(field(name)?, yang_db::CompareOp::Eq, value.clone())
            }
            WhereCondition::Ne { field: name, value } => {
                yang_db::Predicate::Compare(field(name)?, yang_db::CompareOp::Ne, value.clone())
            }
            WhereCondition::Gt { field: name, value } => {
                yang_db::Predicate::Compare(field(name)?, yang_db::CompareOp::Gt, value.clone())
            }
            WhereCondition::Gte { field: name, value } => {
                yang_db::Predicate::Compare(field(name)?, yang_db::CompareOp::Gte, value.clone())
            }
            WhereCondition::Lt { field: name, value } => {
                yang_db::Predicate::Compare(field(name)?, yang_db::CompareOp::Lt, value.clone())
            }
            WhereCondition::Lte { field: name, value } => {
                yang_db::Predicate::Compare(field(name)?, yang_db::CompareOp::Lte, value.clone())
            }
            WhereCondition::Like {
                field: name,
                pattern,
            } => yang_db::Predicate::Compare(
                field(name)?,
                yang_db::CompareOp::Like,
                Value::String(pattern.clone()),
            ),
            WhereCondition::In {
                field: name,
                values,
            } => yang_db::Predicate::In(field(name)?, values.clone()),
            WhereCondition::NotIn {
                field: name,
                values,
            } => yang_db::Predicate::NotIn(field(name)?, values.clone()),
            WhereCondition::Between {
                field: name,
                lo,
                hi,
            } => yang_db::Predicate::Between(field(name)?, lo.clone(), hi.clone()),
            WhereCondition::IsNull { field: name } => yang_db::Predicate::IsNull(field(name)?),
            WhereCondition::IsNotNull { field: name } => {
                yang_db::Predicate::IsNotNull(field(name)?)
            }
            WhereCondition::And { conditions } => yang_db::Predicate::And(
                conditions
                    .iter()
                    .map(|value| self.compile_predicate(value))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            WhereCondition::Or { conditions } => yang_db::Predicate::Or(
                conditions
                    .iter()
                    .map(|value| self.compile_predicate(value))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        })
    }

    #[cfg(feature = "mysql")]
    pub(super) fn compile_db_query(&self) -> Result<yang_db::QueryBuilder<'_>, BaseError> {
        let pool = self
            .pool
            .as_deref()
            .ok_or(BaseError::DatabaseNotInitialized)?;
        self.apply_db_plan(yang_db::QueryBuilder::from_pool(
            pool,
            &self.table_config.table_ref,
        ))
    }

    #[cfg(feature = "mysql")]
    pub(super) fn apply_db_plan<'a>(
        &self,
        mut query: yang_db::QueryBuilder<'a>,
    ) -> Result<yang_db::QueryBuilder<'a>, BaseError> {
        let selected = self.query_params.fields.as_ref().map_or_else(
            || {
                self.default_read_fields()
                    .map(|values| values.into_iter().map(str::to_string).collect())
            },
            |values| Ok(values.clone()),
        )?;
        for name in selected {
            let field = self.table_config.get_field_ref(&name).ok_or_else(|| {
                BaseError::FieldNotFound(self.table_config.table_name.clone(), name.clone())
            })?;
            query = query.field(field);
        }
        for condition in &self.query_params.where_conditions {
            query = query
                .where_predicate(&self.compile_predicate(condition)?)
                .map_err(BaseError::DatabaseQueryFailed)?;
        }
        if !self.include_trashed {
            if let Some(name) = &self.table_config.soft_delete_field {
                let field = self.table_config.get_field_ref(name).ok_or_else(|| {
                    BaseError::FieldNotFound(self.table_config.table_name.clone(), name.clone())
                })?;
                query = query.where_null(field);
            }
        }
        let orders = if self.query_params.order_by.is_empty() {
            &self.table_config.default_order
        } else {
            &self.query_params.order_by
        };
        for (name, order) in orders {
            let field = self.table_config.get_field_ref(name).ok_or_else(|| {
                BaseError::FieldNotFound(self.table_config.table_name.clone(), name.clone())
            })?;
            let order = match order {
                SortOrder::Asc => yang_db::SortOrder::Asc,
                SortOrder::Desc => yang_db::SortOrder::Desc,
            };
            query = query.order(field, order);
        }
        if let Some(page_size) = self.query_params.page_size {
            let page = self.query_params.page.unwrap_or(1).max(1);
            let limit = u64::try_from(page_size).map_err(|_| {
                BaseError::ParamInvalid("page_size".to_string(), "分页大小超出范围".to_string())
            })?;
            let offset = u64::try_from((page - 1).saturating_mul(page_size)).map_err(|_| {
                BaseError::ParamInvalid("page".to_string(), "分页偏移超出范围".to_string())
            })?;
            query = query.limit(limit).offset(offset);
        }
        Ok(query)
    }
}
