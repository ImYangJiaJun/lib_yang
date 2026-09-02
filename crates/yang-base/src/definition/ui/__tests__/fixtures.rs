//! UI 契约测试共享夹具。

use crate::action::{ActionContext, TypedHandler};
use crate::definition::{ActionName, ActionSpec, HttpMethod, RouteSpec};
use crate::error::BaseError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(super) struct NoopInput {}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(super) struct NoopOutput {}

#[derive(crate::Action)]
#[action(name = "noop", public)]
pub(super) struct NoopAction;

#[async_trait]
impl TypedHandler for NoopAction {
    type Input = NoopInput;
    type Output = NoopOutput;

    async fn handle(
        &self,
        _ctx: ActionContext,
        _input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Ok(NoopOutput {})
    }
}

/// 符合关系选项契约的 options Action 夹具：输入/输出即稳定 DTO 对。
#[derive(crate::Action)]
#[action(name = "options", public)]
pub(super) struct RelationOptionsAction;

#[async_trait]
impl crate::action::Action for RelationOptionsAction {
    type Input = crate::table::RelationOptionsRequest;
    type Output = crate::table::RelationOptionsResponse;

    async fn index(
        &self,
        _ctx: ActionContext,
        input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Ok(crate::table::RelationOptionsResponse {
            items: Vec::new(),
            page: input.page,
            limit: input.limit,
            total: Some(0),
        })
    }
}

pub(super) fn action(name: &str, operation_id: &str) -> ActionSpec {
    ActionSpec::new(
        ActionName::new(name).expect("测试 Action 名称应有效"),
        RouteSpec::new(HttpMethod::Post, format!("/{name}"), operation_id),
    )
}
