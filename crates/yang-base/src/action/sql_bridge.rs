//! 把类型化 where 条件桥接到 TableQuery
#![cfg(feature = "mysql")]

use crate::error::BaseError;
use crate::table::TableQuery;

/// 对已经完成搜索、筛选与租户注入的查询执行 COUNT。
///
/// 调用方传入同一数据查询的克隆，保证分页总数不会遗漏任一已生效条件。
pub(crate) async fn count_query(query: TableQuery) -> Result<u64, BaseError> {
    query.count().await
}
