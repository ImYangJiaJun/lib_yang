//! 关系选择器的稳定请求/响应 DTO。

use super::MAX_QUERY_PAGE_SIZE;
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
}
