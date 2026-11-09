//! 查询计划编译：将受保护的查询参数翻译为 `yang_db` 谓词与 `QueryBuilder`。
//!
//! 字段引用解析是后端无关的共享部分；MySQL 专属的谓词编译
//! （`compile_predicate`）与 `compile_db_query` / `apply_read_plan` /
//! `apply_write_plan` 仅在 `mysql` feature 下编译；PostgreSQL 对应实现见
//! `plan_pg.rs`（条件值需按原生类型绑定，因此构造 `PgCondition` 而非 `Predicate`）。

#![cfg(any(feature = "mysql", feature = "postgres"))]

use super::TableQuery;
use crate::error::BaseError;
#[cfg(feature = "mysql")]
use crate::table::SortOrder;
#[cfg(feature = "mysql")]
use crate::table::WhereCondition;
#[cfg(feature = "mysql")]
use serde_json::Value;

impl TableQuery {
    /// 返回本次查询绑定的运行时后端；未注入连接池时返回
    /// [`BaseError::DatabaseNotInitialized`]。
    pub(super) fn backend(&self) -> Result<yang_db::BackendKind, BaseError> {
        self.pool
            .as_ref()
            .map(crate::database::AnyPool::backend)
            .ok_or(BaseError::DatabaseNotInitialized)
    }

    /// 解析字段引用；字段名来自构建期校验过的表定义或请求期校验过的条件。
    pub(super) fn field_ref(&self, name: &str) -> Result<&yang_db::FieldRef, BaseError> {
        // MySQL 构建沿用 `TableConfig::get_field_ref` 既有入口；仅 PostgreSQL 时
        // 直接读取字段引用表，避免依赖 mysql feature 门控的方法。
        #[cfg(feature = "mysql")]
        let field = self.table_config.get_field_ref(name);
        #[cfg(not(feature = "mysql"))]
        let field = self.table_config.field_refs.get(name);
        field.ok_or_else(|| {
            BaseError::FieldNotFound(self.table_config.table_name.clone(), name.to_string())
        })
    }

    /// 把受保护的 WHERE 条件翻译为 `yang_db` 谓词（MySQL 路径）。
    #[cfg(feature = "mysql")]
    pub(super) fn compile_predicate(
        &self,
        condition: &WhereCondition,
    ) -> Result<yang_db::Predicate, BaseError> {
        Ok(match condition {
            WhereCondition::Eq { field: name, value } => yang_db::Predicate::Compare(
                self.field_ref(name)?.clone(),
                yang_db::CompareOp::Eq,
                value.clone(),
            ),
            WhereCondition::Ne { field: name, value } => yang_db::Predicate::Compare(
                self.field_ref(name)?.clone(),
                yang_db::CompareOp::Ne,
                value.clone(),
            ),
            WhereCondition::Gt { field: name, value } => yang_db::Predicate::Compare(
                self.field_ref(name)?.clone(),
                yang_db::CompareOp::Gt,
                value.clone(),
            ),
            WhereCondition::Gte { field: name, value } => yang_db::Predicate::Compare(
                self.field_ref(name)?.clone(),
                yang_db::CompareOp::Gte,
                value.clone(),
            ),
            WhereCondition::Lt { field: name, value } => yang_db::Predicate::Compare(
                self.field_ref(name)?.clone(),
                yang_db::CompareOp::Lt,
                value.clone(),
            ),
            WhereCondition::Lte { field: name, value } => yang_db::Predicate::Compare(
                self.field_ref(name)?.clone(),
                yang_db::CompareOp::Lte,
                value.clone(),
            ),
            WhereCondition::Like {
                field: name,
                pattern,
            } => yang_db::Predicate::Compare(
                self.field_ref(name)?.clone(),
                yang_db::CompareOp::Like,
                Value::String(pattern.clone()),
            ),
            WhereCondition::In {
                field: name,
                values,
            } => yang_db::Predicate::In(self.field_ref(name)?.clone(), values.clone()),
            WhereCondition::NotIn {
                field: name,
                values,
            } => yang_db::Predicate::NotIn(self.field_ref(name)?.clone(), values.clone()),
            WhereCondition::Between {
                field: name,
                lo,
                hi,
            } => yang_db::Predicate::Between(self.field_ref(name)?.clone(), lo.clone(), hi.clone()),
            WhereCondition::IsNull { field: name } => {
                yang_db::Predicate::IsNull(self.field_ref(name)?.clone())
            }
            WhereCondition::IsNotNull { field: name } => {
                yang_db::Predicate::IsNotNull(self.field_ref(name)?.clone())
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

    /// 读路径池绑定入口：绑定连接池后应用完整读计划（投影/WHERE/软删守卫/排序/分页）。
    #[cfg(feature = "mysql")]
    pub(super) fn compile_db_query(&self) -> Result<yang_db::QueryBuilder<'_>, BaseError> {
        let pool = self
            .pool
            .as_ref()
            .and_then(|pool| pool.as_mysql())
            .ok_or(BaseError::DatabaseNotInitialized)?;
        self.apply_read_plan(yang_db::QueryBuilder::from_pool(
            pool,
            &self.table_config.table_ref,
        ))
    }

    /// 写路径池绑定入口（与 [`TableQuery::compile_db_query`] 对称）。
    ///
    /// 写 SQL 不使用投影/排序/分页（yang-db 的 insert/update/delete 仅取
    /// table/field_types/conditions/expr_assignments），因此此处只应用
    /// WHERE/软删守卫，不得解析读投影，否则 write-only 角色会被读权限错误拒绝。
    #[cfg(feature = "mysql")]
    pub(super) fn compile_db_write_query(&self) -> Result<yang_db::QueryBuilder<'_>, BaseError> {
        let pool = self
            .pool
            .as_ref()
            .and_then(|pool| pool.as_mysql())
            .ok_or(BaseError::DatabaseNotInitialized)?;
        self.apply_write_plan(yang_db::QueryBuilder::from_pool(
            pool,
            &self.table_config.table_ref,
        ))
    }

    /// 读路径计划：投影 + WHERE/软删守卫 + 排序 + 分页。
    #[cfg(feature = "mysql")]
    pub(super) fn apply_read_plan<'a>(
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
            let field = self.field_ref(&name)?;
            query = query.field(field);
        }
        query = self.apply_where_guards(query)?;
        let orders = if self.query_params.order_by.is_empty() {
            &self.table_config.default_order
        } else {
            &self.query_params.order_by
        };
        for (name, order) in orders {
            let field = self.field_ref(name)?;
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

    /// 写路径计划：只保留 WHERE/软删守卫。
    #[cfg(feature = "mysql")]
    pub(super) fn apply_write_plan<'a>(
        &self,
        query: yang_db::QueryBuilder<'a>,
    ) -> Result<yang_db::QueryBuilder<'a>, BaseError> {
        self.apply_where_guards(query)
    }

    /// WHERE 条件 + 软删守卫（读写路径共享）。
    #[cfg(feature = "mysql")]
    fn apply_where_guards<'a>(
        &self,
        mut query: yang_db::QueryBuilder<'a>,
    ) -> Result<yang_db::QueryBuilder<'a>, BaseError> {
        for condition in &self.query_params.where_conditions {
            query = query
                .where_predicate(&self.compile_predicate(condition)?)
                .map_err(BaseError::DatabaseQueryFailed)?;
        }
        if !self.include_trashed {
            if let Some(name) = &self.table_config.soft_delete_field {
                let field = self.field_ref(name)?;
                query = query.where_null(field);
            }
        }
        Ok(query)
    }
}
