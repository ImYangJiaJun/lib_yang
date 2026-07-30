//! 请求级 UI 目录 Action。

use super::{Action, ActionContext, ApiResponse};
use crate::definition::{ParamInput, Params};
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
    type Output = ApiResponse;

    async fn index(
        &self,
        ctx: ActionContext,
        _input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        let if_none_match = ctx.request.get_header("if-none-match").map(str::to_owned);
        let catalog = ctx.ui_catalog()?;
        let etag = format!("\"{}\"", catalog.revision);
        let response = if if_none_match
            .as_deref()
            .is_some_and(|value| etag_matches(value, &catalog.revision))
        {
            ApiResponse::default().with_http_status(304)?
        } else {
            ApiResponse::success(catalog, "获取 UI 目录成功")?
        };
        response
            .with_header("etag", etag)?
            .with_header("cache-control", "private, no-cache")?
            .with_header("vary", "authorization, x-tenant-id")
    }
}

fn etag_matches(header: &str, revision: &str) -> bool {
    header.split(',').any(|candidate| {
        let candidate = candidate.trim();
        if candidate == "*" {
            return true;
        }
        candidate
            .strip_prefix("W/")
            .unwrap_or(candidate)
            .trim()
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            == Some(revision)
    })
}

#[cfg(test)]
mod tests {
    use super::etag_matches;

    #[test]
    fn if_none_match_accepts_strong_weak_and_list_tags() {
        assert!(etag_matches("\"abc\"", "abc"));
        assert!(etag_matches("W/\"abc\"", "abc"));
        assert!(etag_matches("\"old\", W/\"abc\"", "abc"));
        assert!(etag_matches("*", "abc"));
        assert!(!etag_matches("\"old\"", "abc"));
    }
}
