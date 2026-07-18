//! PutAction - 按主键更新指定字段
#![cfg(feature = "mysql")]

use crate::action::builtin::add::AffectedResult;
use crate::action::{ActionContext, TypedHandler};
use crate::error::BaseError;
use crate::table::Record;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use yang_base_derive::Action;

/// PutAction 的输入：动态主键值 + 字段更新对象。
///
/// JSON 形态：
/// ```json
/// { "id": 1, "data": { "username": "alice", "age": 30 } }
/// ```
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PutInput {
    /// 主键值
    pub id: Value,
    /// 字段更新对象。空对象将被拒绝。
    pub data: Record,
}

/// 按主键更新记录。
#[derive(Action)]
#[action(
    name = "put",
    display_name = "更新数据",
    description = "按主键更新指定字段"
)]
pub struct PutAction;

impl PutAction {
    /// 创建 PutAction 实例。
    pub fn new() -> Self {
        Self
    }
}

impl Default for PutAction {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TypedHandler for PutAction {
    type Input = PutInput;
    type Output = AffectedResult;

    async fn handle(
        &self,
        ctx: ActionContext,
        input: PutInput,
    ) -> Result<AffectedResult, BaseError> {
        if input.data.as_map().is_empty() {
            return Err(BaseError::ParamInvalid(
                "data".into(),
                "至少需要一个字段".into(),
            ));
        }
        // 主键定位是 Action 自有寻址机制，绕过 filterable 业务筛选权限。
        let affected = ctx
            .table_query()?
            .where_primary_key_eq(input.id)?
            .update(input.data)
            .await?;
        Ok(AffectedResult { affected })
    }
}
