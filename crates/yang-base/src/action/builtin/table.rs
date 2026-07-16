//! TableAction - 返回表的元信息
#![cfg(feature = "mysql")]

use crate::action::{ActionContext, TypedHandler};
use crate::error::BaseError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use yang_base_derive::Action;

/// TableAction 的空输入（接受 `{}`）。
#[derive(Deserialize, schemars::JsonSchema, Default)]
#[serde(default)]
pub struct EmptyInput {}

/// 表元信息响应。
#[derive(Serialize, schemars::JsonSchema)]
pub struct TableSchemaResponse {
    /// 表名
    pub table_name: String,
    /// 主键字段名
    pub primary_key: String,
    /// 表的可写字段 JSON Schema
    pub input_schema: serde_json::Value,
    /// 表的可读记录 JSON Schema（供参考，并非某具体 Action 的真实出参）
    pub output_schema: serde_json::Value,
}

/// 返回表的元信息。
#[derive(Action)]
#[action(
    name = "table",
    display_name = "表元信息",
    description = "返回表结构与字段 schema"
)]
pub struct TableAction;

impl TableAction {
    /// 创建 TableAction 实例。
    pub fn new() -> Self {
        Self
    }
}

impl Default for TableAction {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TypedHandler for TableAction {
    type Input = EmptyInput;
    type Output = TableSchemaResponse;

    async fn handle(
        &self,
        ctx: ActionContext,
        _input: EmptyInput,
    ) -> Result<TableSchemaResponse, BaseError> {
        let roles = ctx.user_roles_set().cloned().unwrap_or_default();
        let definition = ctx.table_definition()?;
        Ok(TableSchemaResponse {
            table_name: definition.name().to_string(),
            primary_key: definition.primary_key().to_string(),
            input_schema: definition.input_schema_for_roles(&roles),
            output_schema: definition.output_schema_for_roles(&roles),
        })
    }
}
