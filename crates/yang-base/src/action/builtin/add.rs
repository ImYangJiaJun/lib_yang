//! AddAction - 插入单条记录
#![cfg(any(feature = "mysql", feature = "postgres"))]

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

/// `TableQuery::insert_returning` 的返回目标。
///
/// 与 `InsertResult` 同属插入契约，因此放在本模块；`table::table_query::write`
/// 通过它暴露 RETURNING 目标。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturningTarget<'a> {
    /// 只返回主键列（`RETURNING <pk>`）。
    Pk,
    /// 直接 `RETURNING <列...>`（单列/多列均可）；MySQL 仅支持只含整型主键的单列。
    Columns(&'a [&'a str]),
    /// 返回整行（`RETURNING *`）。
    All,
}

/// 新增记录的返回值：受影响行数 + 主键值。
///
/// `id` 是 JSON 值而不是固定 `u64`：PostgreSQL 的 `RETURNING` 可回填 UUID / text /
/// numeric 等任意类型主键；MySQL 无 `RETURNING`，只有整型主键能取回，
/// 非整型主键会返回明确错误，不会被截断或臆造。
#[derive(Serialize, schemars::JsonSchema)]
pub struct InsertResult {
    /// 受影响行数
    pub affected: u64,
    /// 新插入记录的主键值（整型为 JSON number，UUID/text/numeric 按真实类型）
    pub id: serde_json::Value,
}

impl InsertResult {
    /// 主键是整型时返回其 `u64` 值。
    pub fn as_u64(&self) -> Option<u64> {
        self.id
            .as_u64()
            .or_else(|| self.id.as_str().and_then(|value| value.parse().ok()))
    }

    /// 主键是字符串（UUID/text）时返回其 `&str` 值。
    pub fn as_str(&self) -> Option<&str> {
        self.id.as_str()
    }
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
        let (affected, id) = ctx.table_query()?.insert_returning_pk_value(input).await?;
        Ok(InsertResult { affected, id })
    }
}
