//! PutAction - 按主键更新指定字段
#![cfg(feature = "mysql")]

use crate::action::builtin::add::AffectedResult;
use crate::action::{ActionContext, TypedHandler};
use crate::error::BaseError;
use crate::table::AsColumnName;
use crate::table::TableEntity;
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use std::marker::PhantomData;
use yang_base_derive::Action;

/// PutAction 的输入：主键值 + 字段更新对列表。
///
/// JSON 形态：
/// ```json
/// { "id": 1, "data": [["username", "alice"], ["age", 30]] }
/// ```
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PutInput<T: TableEntity> {
    /// 主键值
    pub id: T::Pk,
    /// 字段更新对：(字段名枚举, 新值)。空数组将被拒绝。
    pub data: Vec<(T::Field, serde_json::Value)>,
}

/// 按主键更新记录。
#[derive(Action)]
#[action(
    name = "put",
    display_name = "更新数据",
    description = "按主键更新指定字段"
)]
pub struct PutAction<T: TableEntity> {
    _phantom: PhantomData<T>,
}

impl<T: TableEntity> PutAction<T> {
    /// 创建 PutAction 实例。
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: TableEntity> Default for PutAction<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl<T: TableEntity> TypedHandler for PutAction<T> {
    type Input = PutInput<T>;
    type Output = AffectedResult;

    async fn handle(
        &self,
        ctx: ActionContext,
        input: PutInput<T>,
    ) -> Result<AffectedResult, BaseError> {
        if input.data.is_empty() {
            return Err(BaseError::ParamInvalid(
                "data".into(),
                "至少需要一个字段".into(),
            ));
        }
        let pk_value = serde_json::to_value(&input.id)
            .map_err(|e| BaseError::JsonSerializeFailed(e.to_string()))?;
        let data: HashMap<String, serde_json::Value> = input
            .data
            .into_iter()
            .map(|(field, value)| (field.column_name().to_string(), value))
            .collect();
        let affected = ctx
            .table_query()?
            .where_eq(T::PK_FIELD, pk_value)?
            .update(data)
            .await?;
        Ok(AffectedResult { affected })
    }
}
