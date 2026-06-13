//! 把类型化 where 条件桥接到 TableQuery
#![cfg(feature = "mysql")]

use crate::action::ActionContext;
use crate::error::BaseError;
use crate::table::WhereCondition;

/// 给定一棵 WHERE 条件树（叶子或 And/Or 组），跑一次 SELECT COUNT(*) 计算总数。
///
/// 整棵树经 `TableQuery::where_tree` 递归校验（字段存在性/筛选权限/嵌套深度）后并入
/// 查询再 COUNT。C2a 布尔树的统一计数入口。
pub(crate) async fn count_with_tree(
    ctx: &ActionContext,
    condition: WhereCondition,
) -> Result<u64, BaseError> {
    let q = ctx.table_query()?.where_tree(condition)?;
    q.count().await
}
