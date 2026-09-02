//! 默认 Action 演示页契约：参数来源、单参数与整页 schema 及其 `ActionSpec` 投影。

use super::hints::ActionResponseKind;
use crate::definition::{ActionSpec, ParamSource};
use schemars::JsonSchema;
use serde::Serialize;

/// Action 参数在 HTTP 请求中的来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum UiParamSource {
    /// JSON body。
    Body,
    /// Query string。
    Query,
    /// Path 参数。
    Path,
    /// Header。
    Header,
}

impl From<ParamSource> for UiParamSource {
    fn from(source: ParamSource) -> Self {
        match source {
            ParamSource::Body => Self::Body,
            ParamSource::Query => Self::Query,
            ParamSource::Path => Self::Path,
            ParamSource::Header => Self::Header,
        }
    }
}

/// 默认 Action 演示页需要的单个参数契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ActionDemoParamSchema {
    /// 参数名。
    pub name: String,
    /// 参数来源。
    pub source: UiParamSource,
    /// 是否必填；path 参数即使定义遗漏，也始终为必填。
    pub required: bool,
    /// 用户可见标题。
    pub title: String,
    /// 参数帮助说明。
    pub description: String,
}

/// 单个 Action 的默认演示页契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ActionDemoSchema {
    /// 全局唯一 operation id。
    pub operation_id: String,
    /// 用户可见标题。
    pub title: String,
    /// Action 业务说明。
    pub description: String,
    /// 标准大写 HTTP method。
    pub method: String,
    /// 已校验的服务端路由模板。
    pub path: String,
    /// 参数来源与展示信息。
    pub params: Vec<ActionDemoParamSchema>,
    /// Handler Input 的 JSON Schema。
    pub input_schema: serde_json::Value,
    /// Handler Output 的 JSON Schema。
    pub output_schema: serde_json::Value,
    /// 请求媒体类型。
    pub request_media_type: crate::definition::ActionMediaType,
    /// multipart 资源与类型限制；JSON Action 为 `None`。
    pub multipart: Option<crate::definition::MultipartSpec>,
    /// 成功响应的展示类别。
    pub response_kind: ActionResponseKind,
    /// 是否必须先建立认证身份。
    pub requires_auth: bool,
}

impl From<&ActionSpec> for ActionDemoSchema {
    fn from(action: &ActionSpec) -> Self {
        let params = action
            .params
            .iter()
            .map(|param| {
                let name = param.name.to_string();
                ActionDemoParamSchema {
                    title: if param.presentation.title.is_empty() {
                        name.clone()
                    } else {
                        param.presentation.title.clone()
                    },
                    name,
                    source: param.source.into(),
                    required: param.required || param.source == ParamSource::Path,
                    description: param.presentation.description.clone(),
                }
            })
            .collect();
        Self {
            operation_id: action.route.operation_id.clone(),
            title: action.display_name.clone(),
            description: action.description.clone(),
            method: action.route.method.as_str().to_string(),
            path: action.route.path.clone(),
            params,
            input_schema: action.input_schema.clone(),
            output_schema: action.output_schema.clone(),
            request_media_type: action.request_media_type,
            multipart: action.multipart.clone(),
            response_kind: action.response_kind,
            requires_auth: !action.is_public,
        }
    }
}
