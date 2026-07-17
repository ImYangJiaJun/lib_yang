//! 请求级 UI 目录 Action。

use super::{Action, ActionContext};
use crate::definition::{ParamInput, Params, UiCatalog};
use crate::error::BaseError;
use async_trait::async_trait;
use serde::Deserialize;

/// UI 目录端点的空输入。
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UiCatalogInput {}

impl ParamInput for UiCatalogInput {
    fn params() -> Params {
        Params::new()
    }
}

/// 返回当前请求身份可访问 Action 的版本化目录。
///
/// 该 Action 标记为 public，使匿名调用至少能发现公开 Action；若模块中间件在
/// public Action 上注入了认证用户，同一端点会自动返回该用户可访问的完整目录。
#[derive(Debug, Default, crate::Action)]
#[action(
    name = "ui_catalog",
    display_name = "UI 目录",
    description = "返回当前请求身份可访问的 Action 演示契约",
    method = "GET",
    path = "/.well-known/yang/ui-catalog",
    public
)]
pub struct UiCatalogAction;

#[async_trait]
impl Action for UiCatalogAction {
    type Input = UiCatalogInput;
    type Output = UiCatalog;

    async fn index(
        &self,
        ctx: ActionContext,
        _input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        ctx.ui_catalog()
    }
}
