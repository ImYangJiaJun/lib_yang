//! GetAction - 根据主键获取单条数据
#![cfg(feature = "mysql")]

use crate::action::{ActionContext, TypedHandler};
use crate::error::BaseError;
use crate::table::Record;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use yang_base_derive::Action;

/// 按主键 ID 获取单条记录的输入。
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetByPk {
    /// 主键值
    pub id: Value,
}

/// 按主键获取单条记录。
#[derive(Action)]
#[action(
    name = "get",
    display_name = "获取数据",
    description = "根据主键获取单条记录"
)]
pub struct GetAction;

impl GetAction {
    /// 创建 GetAction 实例。
    pub fn new() -> Self {
        Self
    }
}

impl Default for GetAction {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TypedHandler for GetAction {
    type Input = GetByPk;
    type Output = Record;

    async fn handle(&self, ctx: ActionContext, input: GetByPk) -> Result<Record, BaseError> {
        let definition = ctx.table_definition()?;
        let table_name = definition.name().to_string();
        let primary_key = definition.primary_key().to_string();
        let pk_value = input.id;
        // 默认投影当前角色可读且非 secret 的字段；显式字段查询仍由 TableQuery 校验。
        if ctx.user.is_none() {
            return Err(BaseError::Unauthorized("需要登录".to_string()));
        }
        let query = ctx.table_query()?;
        let query = query.where_eq(&primary_key, pk_value.clone())?;
        query.optional().await?.ok_or_else(|| {
            BaseError::RecordNotFound(format!(
                "{} 中主键 {}={} 的记录不存在",
                table_name, primary_key, pk_value
            ))
        })
    }
}
