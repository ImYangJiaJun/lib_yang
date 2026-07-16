//! AddAction - 插入单条记录
#![cfg(feature = "mysql")]

use crate::action::{ActionContext, TypedHandler};
use crate::error::BaseError;
use crate::table::Record;
use async_trait::async_trait;
use serde::Serialize;
use yang_base_derive::Action;

/// 通用受影响行数返回值。
#[derive(Serialize, schemars::JsonSchema)]
pub struct AffectedResult {
    /// 受影响行数
    pub affected: u64,
}

/// 新增记录的返回值：受影响行数 + 自增主键。
///
/// 相比仅返回受影响行数，额外携带本次 INSERT 产生的自增主键（`last_insert_id`），
/// 便于调用方拿到新建记录的 ID。
#[derive(Serialize, schemars::JsonSchema)]
pub struct InsertResult {
    /// 受影响行数
    pub affected: u64,
    /// 新插入记录的自增主键值（无自增列时为 0）
    pub id: u64,
}

/// 插入一条动态记录。
#[derive(Action)]
#[action(
    name = "add",
    display_name = "新增数据",
    description = "向表中插入一条记录"
)]
pub struct AddAction;

impl AddAction {
    /// 创建 AddAction 实例。
    pub fn new() -> Self {
        Self
    }
}

impl Default for AddAction {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TypedHandler for AddAction {
    type Input = Record;
    type Output = InsertResult;

    async fn handle(&self, ctx: ActionContext, input: Record) -> Result<InsertResult, BaseError> {
        let (affected, id) = ctx.table_query()?.insert_returning_id(input).await?;
        Ok(InsertResult { affected, id })
    }
}
