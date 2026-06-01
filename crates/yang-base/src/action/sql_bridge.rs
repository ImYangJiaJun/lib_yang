//! 把 SqlCondition 桥接到 TableQuery
#![cfg(feature = "mysql")]

use crate::action::ActionContext;
use crate::error::BaseError;
use crate::table::{SqlCondition, SqlOp};
use crate::table::TableQuery;

/// 把单个 SqlCondition 应用到 TableQuery 上。
pub(crate) fn apply_sql_condition(
    q: TableQuery,
    cond: &SqlCondition,
) -> Result<TableQuery, BaseError> {
    Ok(match cond.op {
        SqlOp::Eq => q.where_eq(cond.column, cond.params[0].clone())?,
        SqlOp::Ne => q.where_ne(cond.column, cond.params[0].clone())?,
        SqlOp::Lt => q.where_lt(cond.column, cond.params[0].clone())?,
        SqlOp::Lte => q.where_lte(cond.column, cond.params[0].clone())?,
        SqlOp::Gt => q.where_gt(cond.column, cond.params[0].clone())?,
        SqlOp::Gte => q.where_gte(cond.column, cond.params[0].clone())?,
        SqlOp::In => q.where_in(cond.column, cond.params.clone())?,
        SqlOp::NotIn => q.where_not_in(cond.column, cond.params.clone())?,
        SqlOp::Between => q.where_between(
            cond.column,
            cond.params[0].clone(),
            cond.params[1].clone(),
        )?,
        SqlOp::Like => q.where_like(
            cond.column,
            cond.params[0].as_str().unwrap_or("").to_string(),
        )?,
        SqlOp::IsNull => q.where_null(cond.column)?,
        SqlOp::IsNotNull => q.where_not_null(cond.column)?,
    })
}

/// 给定相同的 where 条件，跑一次 SELECT COUNT(*) 计算总数（不分页）
pub(crate) async fn count_with_conditions(
    ctx: &ActionContext,
    conditions: &[SqlCondition],
) -> Result<u64, BaseError> {
    let mut q = ctx.table_query()?;
    for cond in conditions {
        q = apply_sql_condition(q, cond)?;
    }
    q.count().await
}
