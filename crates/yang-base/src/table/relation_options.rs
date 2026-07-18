//! 关系选择器的稳定请求/响应 DTO。

use super::MAX_QUERY_PAGE_SIZE;
use crate::definition::{ParamInput, Params};
use crate::error::BaseError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

const DEFAULT_OPTIONS_LIMIT: usize = 20;
const MAX_SEARCH_CHARS: usize = 200;
const MAX_SELECTED_VALUES: usize = 100;
const MAX_FILTER_FIELDS: usize = 20;

/// 关系选项请求。
///
/// `filter` 只表示字段到值的声明式精确筛选；执行器仍必须按目标表字段白名单、当前
/// 用户权限和租户范围逐项校验，不能把字段名直接拼接进 SQL。
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct RelationOptionsRequest {
    /// 模糊搜索文本。
    pub search: Option<String>,
    /// 需要回填的已选主键；即使不在当前搜索页也应返回。
    pub selected: Vec<Value>,
    /// 声明式精确筛选字段。
    pub filter: BTreeMap<String, Value>,
    /// 从 1 开始的页码。
    pub page: usize,
    /// 每页数量，上限与通用查询一致。
    pub limit: usize,
}

impl Default for RelationOptionsRequest {
    fn default() -> Self {
        Self {
            search: None,
            selected: Vec::new(),
            filter: BTreeMap::new(),
            page: 1,
            limit: DEFAULT_OPTIONS_LIMIT,
        }
    }
}

impl ParamInput for RelationOptionsRequest {
    fn params() -> Params {
        Params::new()
    }

    /// 反序列化后立即执行边界校验：select Action 经 `Action::Input` 解码时 fail-closed，
    /// handler 不再依赖显式调用 [`RelationOptionsRequest::validate`] 才能守住传输边界。
    fn decode(request: &mut crate::action::Request) -> Result<Self, BaseError> {
        let input: Self = serde_json::from_value(std::mem::take(&mut request.body))
            .map_err(|error| BaseError::ParamInvalid("input".into(), error.to_string()))?;
        input.validate()?;
        Ok(input)
    }
}

impl RelationOptionsRequest {
    /// 校验传输层通用边界。
    ///
    /// 字段存在性、可筛选权限和租户隔离属于执行器职责，不能由本 DTO 绕过。
    pub fn validate(&self) -> Result<(), BaseError> {
        if self.page == 0 {
            return Err(BaseError::ParamInvalid(
                "page".to_string(),
                "页码必须从 1 开始".to_string(),
            ));
        }
        if self.limit == 0 || self.limit > MAX_QUERY_PAGE_SIZE {
            return Err(BaseError::ParamInvalid(
                "limit".to_string(),
                format!("每页数量必须在 1..={MAX_QUERY_PAGE_SIZE} 范围内"),
            ));
        }
        if self
            .search
            .as_ref()
            .is_some_and(|search| search.chars().count() > MAX_SEARCH_CHARS)
        {
            return Err(BaseError::ParamInvalid(
                "search".to_string(),
                format!("搜索文本不能超过 {MAX_SEARCH_CHARS} 个字符"),
            ));
        }
        if self.selected.len() > MAX_SELECTED_VALUES {
            return Err(BaseError::ParamInvalid(
                "selected".to_string(),
                format!("已选值不能超过 {MAX_SELECTED_VALUES} 个"),
            ));
        }
        if self
            .selected
            .iter()
            .any(|value| !matches!(value, Value::String(_) | Value::Number(_)))
        {
            return Err(BaseError::ParamInvalid(
                "selected".to_string(),
                "已选值只允许字符串或数字主键".to_string(),
            ));
        }
        if self.filter.len() > MAX_FILTER_FIELDS {
            return Err(BaseError::ParamInvalid(
                "filter".to_string(),
                format!("筛选字段不能超过 {MAX_FILTER_FIELDS} 个"),
            ));
        }
        if self
            .filter
            .keys()
            .any(|field| field.trim().is_empty() || field.len() > 128)
        {
            return Err(BaseError::ParamInvalid(
                "filter".to_string(),
                "筛选字段名不能为空且不能超过 128 字节".to_string(),
            ));
        }
        Ok(())
    }
}

/// 单个关系选项的稳定 `{value,label}` 形状。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RelationOption {
    /// 真实主键值。
    pub value: Value,
    /// 用户可见标签。
    pub label: String,
}

/// 分页关系选项响应。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RelationOptionsResponse {
    /// 当前页选项；回填项可由执行器合并并去重。
    pub items: Vec<RelationOption>,
    /// 当前页码。
    pub page: usize,
    /// 实际每页数量。
    pub limit: usize,
    /// 可选总数；执行器不做 count 时为 `None`。
    pub total: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Request;
    use serde_json::json;

    #[test]
    fn defaults_and_wire_shape_are_stable() {
        let request: RelationOptionsRequest =
            serde_json::from_value(json!({})).expect("空请求应应用稳定默认值");
        assert_eq!(request.page, 1);
        assert_eq!(request.limit, DEFAULT_OPTIONS_LIMIT);
        request.validate().expect("默认请求应有效");

        let response = RelationOptionsResponse {
            items: vec![RelationOption {
                value: json!(7),
                label: "Alice".to_string(),
            }],
            page: 1,
            limit: 20,
            total: Some(1),
        };
        assert_eq!(
            serde_json::to_value(response).expect("响应应可序列化"),
            json!({
                "items": [{"value": 7, "label": "Alice"}],
                "page": 1,
                "limit": 20,
                "total": 1
            })
        );
    }

    #[test]
    fn selected_supports_string_and_number_ids_but_rejects_structured_values() {
        let valid: RelationOptionsRequest = serde_json::from_value(json!({
            "selected": [7, "user-8"]
        }))
        .expect("字符串和数字主键应可解析");
        valid.validate().expect("字符串和数字主键应有效");

        for selected in [
            json!([null]),
            json!([true]),
            json!([[1]]),
            json!([{"id": 1}]),
        ] {
            let request: RelationOptionsRequest = serde_json::from_value(json!({
                "selected": selected
            }))
            .expect("结构化值应在显式验证阶段拒绝");
            assert!(matches!(
                request.validate(),
                Err(BaseError::ParamInvalid(field, _)) if field == "selected"
            ));
        }
    }

    #[test]
    fn pagination_search_and_filter_are_bounded() {
        let cases = [
            ("page", json!({"page": 0})),
            ("limit", json!({"limit": 0})),
            ("limit", json!({"limit": MAX_QUERY_PAGE_SIZE + 1})),
            (
                "search",
                json!({"search": "x".repeat(MAX_SEARCH_CHARS + 1)}),
            ),
            ("filter", json!({"filter": {"": 1}})),
        ];
        for (expected, value) in cases {
            let request: RelationOptionsRequest =
                serde_json::from_value(value).expect("边界请求应在验证阶段处理");
            assert!(matches!(
                request.validate(),
                Err(BaseError::ParamInvalid(field, _)) if field == expected
            ));
        }

        let unknown = serde_json::from_value::<RelationOptionsRequest>(json!({"offset": 0}));
        assert!(unknown.is_err(), "未知字段不得被静默忽略");
    }

    #[test]
    fn request_is_directly_decodable_as_action_body_input() {
        let mut request = Request::new(json!({
            "search": "alice",
            "selected": [7],
            "filter": {"status": "active"},
            "page": 2,
            "limit": 30
        }));

        let input = <RelationOptionsRequest as ParamInput>::decode(&mut request)
            .expect("关系选项 DTO 应可直接作为 Action body 输入");
        input.validate().expect("合法请求应通过边界校验");
        assert_eq!(input.search.as_deref(), Some("alice"));
        assert_eq!(input.selected, vec![json!(7)]);
        assert_eq!(input.page, 2);
        assert_eq!(input.limit, 30);
        assert!(request.body.is_null(), "输入解码后 body 应被一次性消费");
    }

    #[test]
    fn decode_rejects_over_limit_body_without_manual_validate() {
        // I-4：validate 必须在 decode 默认路径上 fail-closed，handler 忘调 validate
        // 也不能放行超限输入。
        let mut request = Request::new(json!({"limit": MAX_QUERY_PAGE_SIZE + 1}));
        let error = <RelationOptionsRequest as ParamInput>::decode(&mut request)
            .expect_err("超限 body 必须在 decode 阶段直接拒绝");
        assert!(
            matches!(&error, BaseError::ParamInvalid(field, _) if field == "limit"),
            "拒绝原因必须指向 limit 边界: {error}"
        );

        let mut request = Request::new(json!({"page": 0}));
        let error = <RelationOptionsRequest as ParamInput>::decode(&mut request)
            .expect_err("非法页码必须在 decode 阶段直接拒绝");
        assert!(
            matches!(&error, BaseError::ParamInvalid(field, _) if field == "page"),
            "拒绝原因必须指向 page 边界: {error}"
        );
    }
}
