//! DelAction - 按主键删除记录
#![cfg(feature = "mysql")]

use crate::action::builtin::{add::AffectedResult, get::GetByPk};
use crate::action::{ActionContext, TypedHandler};
use crate::error::BaseError;
use async_trait::async_trait;
use yang_base_derive::Action;

/// 按主键删除记录。
///
/// # 授权模型（删除授权的非对称性）
///
/// 删除是否被允许，由 Action 级 / 模块级权限把关（路由鉴权与 [`Permission`] 判定），
/// 而**不做字段级写权限校验**——物理删除（DELETE）是整行操作，不涉及单个字段的可写性，
/// 因此这里不调用 `ensure_*_writable` 之类的字段检查。
///
/// 字段级 `can_write` 仅在软删除场景下作为副产物间接生效（软删通过 UPDATE 标记删除列，
/// 走的是更新路径上的字段写权限）；它并不是删除授权的把关点。
///
/// [`Permission`]: crate::action::Permission
#[derive(Action)]
#[action(
    name = "del",
    display_name = "删除数据",
    description = "按主键删除记录"
)]
pub struct DelAction;

impl DelAction {
    /// 创建 DelAction 实例。
    pub fn new() -> Self {
        Self
    }
}

impl Default for DelAction {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TypedHandler for DelAction {
    type Input = GetByPk;
    type Output = AffectedResult;

    async fn handle(
        &self,
        ctx: ActionContext,
        input: GetByPk,
    ) -> Result<AffectedResult, BaseError> {
        // 主键定位是 Action 自有寻址机制，绕过 filterable 业务筛选权限。
        let affected = ctx
            .table_query()?
            .where_primary_key_eq(input.id)?
            .delete()
            .await?;
        Ok(AffectedResult { affected })
    }
}
