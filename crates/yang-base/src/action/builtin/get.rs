//! GetAction - 根据主键获取单条数据
#![cfg(feature = "mysql")]

use crate::action::{ActionContext, TypedHandler};
use crate::error::BaseError;
use crate::table::TableEntity;
use async_trait::async_trait;
use serde::Deserialize;
use std::marker::PhantomData;
use yang_base_derive::Action;

/// 按主键 ID 获取单条记录的输入。
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetByPk<PK> {
    /// 主键值
    pub id: PK,
}

/// 按主键获取单条记录。
#[derive(Action)]
#[action(name = "get", display_name = "获取数据", description = "根据主键获取单条记录")]
pub struct GetAction<T: TableEntity> {
    _phantom: PhantomData<T>,
}

impl<T: TableEntity> GetAction<T> {
    /// 创建 GetAction 实例。
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: TableEntity> Default for GetAction<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl<T: TableEntity> TypedHandler for GetAction<T> {
    type Input = GetByPk<T::Pk>;
    type Output = T;

    async fn handle(&self, ctx: ActionContext, input: GetByPk<T::Pk>) -> Result<T, BaseError> {
        let pk_value = serde_json::to_value(&input.id)
            .map_err(|e| BaseError::JsonSerializeFailed(e.to_string()))?;
        // 字段读权限强制：执行查询前确认当前用户对全部字段可读。
        let user = ctx
            .user
            .as_ref()
            .ok_or_else(|| BaseError::Unauthorized("需要登录".to_string()))?;
        let query = ctx.table_query()?;
        query.ensure_fields_readable(user)?;
        let query = query.where_eq(T::PK_FIELD, pk_value.clone())?;
        query.fetch_optional::<T>().await?.ok_or_else(|| {
            BaseError::RecordNotFound(format!(
                "{} 中主键 {}={} 的记录不存在",
                T::TABLE_NAME,
                T::PK_FIELD,
                pk_value
            ))
        })
    }
}
