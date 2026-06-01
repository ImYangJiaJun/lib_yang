//! DelAction - 按主键删除记录
#![cfg(feature = "mysql")]

use crate::action::builtin::{add::AffectedResult, get::GetByPk};
use crate::action::{ActionContext, TypedHandler};
use crate::error::BaseError;
use crate::table::TableEntity;
use async_trait::async_trait;
use std::marker::PhantomData;
use yang_base_derive::Action;

/// 按主键删除记录。
#[derive(Action)]
#[action(name = "del", display_name = "删除数据", description = "按主键删除记录")]
pub struct DelAction<T: TableEntity> {
    _phantom: PhantomData<T>,
}

impl<T: TableEntity> DelAction<T> {
    /// 创建 DelAction 实例。
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: TableEntity> Default for DelAction<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl<T: TableEntity> TypedHandler for DelAction<T> {
    type Input = GetByPk<T::Pk>;
    type Output = AffectedResult;

    async fn handle(
        &self,
        ctx: ActionContext,
        input: GetByPk<T::Pk>,
    ) -> Result<AffectedResult, BaseError> {
        let pk_value = serde_json::to_value(&input.id)
            .map_err(|e| BaseError::JsonSerializeFailed(e.to_string()))?;
        let affected = ctx
            .table_query()?
            .where_eq(T::PK_FIELD, pk_value)?
            .delete()
            .await?;
        Ok(AffectedResult { affected })
    }
}
