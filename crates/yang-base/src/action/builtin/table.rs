//! TableAction - 返回表的元信息
#![cfg(feature = "mysql")]

use crate::action::{ActionContext, TypedHandler};
use crate::error::BaseError;
use crate::table::TableEntity;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
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
    /// 入参 JSON Schema（实体本身）
    pub input_schema: serde_json::Value,
    /// 出参 JSON Schema（该表行实体 schema，供参考，并非某具体 Action 的真实出参）
    pub output_schema: serde_json::Value,
}

/// 返回表的元信息。
#[derive(Action)]
#[action(
    name = "table",
    display_name = "表元信息",
    description = "返回表结构与字段 schema"
)]
pub struct TableAction<T: TableEntity> {
    _phantom: PhantomData<T>,
}

impl<T: TableEntity> TableAction<T> {
    /// 创建 TableAction 实例。
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: TableEntity> Default for TableAction<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl<T: TableEntity> TypedHandler for TableAction<T> {
    type Input = EmptyInput;
    type Output = TableSchemaResponse;

    async fn handle(
        &self,
        _ctx: ActionContext,
        _input: EmptyInput,
    ) -> Result<TableSchemaResponse, BaseError> {
        let schema = schemars::schema_for!(T);
        let schema_value = serde_json::to_value(&schema)
            .map_err(|e| BaseError::JsonSerializeFailed(e.to_string()))?;
        Ok(TableSchemaResponse {
            table_name: T::TABLE_NAME.to_string(),
            primary_key: T::PK_FIELD.to_string(),
            input_schema: schema_value.clone(),
            output_schema: schema_value,
        })
    }
}
