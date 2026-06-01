//! AddAction - 插入单条记录
#![cfg(feature = "mysql")]

use crate::action::{ActionContext, TypedHandler};
use crate::error::BaseError;
use crate::table::TableEntity;
use async_trait::async_trait;
use serde::Serialize;
use std::collections::HashMap;
use std::marker::PhantomData;
use yang_base_derive::Action;

/// 通用受影响行数返回值。
#[derive(Serialize, schemars::JsonSchema)]
pub struct AffectedResult {
    /// 受影响行数
    pub affected: u64,
}

/// 插入一条记录。Input 是整个实体（用户决定 Pk 字段是否 Option/自增）。
#[derive(Action)]
#[action(name = "add", display_name = "新增数据", description = "向表中插入一条记录")]
pub struct AddAction<T: TableEntity> {
    _phantom: PhantomData<T>,
}

impl<T: TableEntity> AddAction<T> {
    /// 创建 AddAction 实例。
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: TableEntity> Default for AddAction<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl<T: TableEntity> TypedHandler for AddAction<T> {
    type Input = T;
    type Output = AffectedResult;

    async fn handle(&self, ctx: ActionContext, input: T) -> Result<AffectedResult, BaseError> {
        let value = serde_json::to_value(&input)
            .map_err(|e| BaseError::JsonSerializeFailed(e.to_string()))?;
        let map: HashMap<String, serde_json::Value> = match value {
            serde_json::Value::Object(m) => m.into_iter().collect(),
            _ => {
                return Err(BaseError::ParamInvalid(
                    "body".into(),
                    "实体必须序列化为对象".into(),
                ))
            }
        };
        let affected = ctx.table_query()?.insert(map).await?;
        Ok(AffectedResult { affected })
    }
}
