//! 审批定义 `form` 的解析与控件契约。
//!
//! `form` 实测有两种形态：JSON 字符串与数组。两种都认；只认一种会在另一半响应上
//! 整个解析失败，表现为「审批定义没有表单」。

use serde::Deserialize;

use super::widget_rules;

/// 表单解析失败。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FormError {
    /// `form` 为空。
    #[error("审批定义没有返回表单（`form` 为空）")]
    Missing,
    /// JSON 解析失败。
    #[error("审批定义的表单解析失败：{0}")]
    Parse(String),
    /// 形态无法识别（非字符串/数组）。
    #[error("审批定义的表单形态无法识别：{0}")]
    UnexpectedType(&'static str),
    /// 表单里没有控件。
    #[error("审批定义的表单里没有任何控件")]
    Empty,
}

/// 审批定义 `form` 字段里的多态 `option` 取值。
///
/// **不要加单元变体**：untagged 单元变体会抢在数组与对象之前匹配，把两个变体
/// 全部挡死。
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum WidgetOptions {
    /// 固定选项数组（`radioV2` / `checkboxV2` 的真实形态）。
    Fixed(Vec<OptionItem>),
    /// 明细 / 关联审批等控件的配置对象。
    Config(serde_json::Value),
}

/// 一个固定选项：`text` 是可读文案，`value` 是提交值。
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct OptionItem {
    /// 可读文案。
    #[serde(default)]
    pub text: String,
    /// 提交值。
    #[serde(default)]
    pub value: String,
}

/// 审批定义 `form` 数组里的一项。
#[derive(Debug, Clone, Deserialize)]
#[serde(from = "FormWidgetRaw")]
pub struct FormWidget {
    /// 控件 id。
    pub id: String,
    /// 控件名（实测是服务端解好的可读中文）。
    pub name: String,
    /// 控件类型（`input` / `radioV2` / `fieldList` / …）。
    pub r#type: String,
    /// 是否必填。
    pub required: bool,
    /// 关联审批允许的定义 Code；其它控件保持原始形态。
    pub value: serde_json::Value,
    /// 多态选项；`None` 表示无选项。
    pub option: Option<WidgetOptions>,
    /// 外部数据源链接（识别级联链接态）。
    pub external_data: Option<serde_json::Value>,
    /// 子控件。
    pub children: Vec<FormWidget>,
}

/// [`FormWidget`] 的反序列化中间形态。
#[derive(Debug, Clone, Deserialize)]
struct FormWidgetRaw {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    r#type: String,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    value: serde_json::Value,
    #[serde(default)]
    option: Option<WidgetOptions>,
    /// `rename` 不能省：JSON 键是驼峰的 `externalData`。
    #[serde(default, rename = "externalData")]
    external_data: Option<serde_json::Value>,
    #[serde(default)]
    children: Vec<FormWidget>,
}

impl From<FormWidgetRaw> for FormWidget {
    fn from(raw: FormWidgetRaw) -> Self {
        let children = match widget_rules::group_child_ids(&raw.r#type) {
            Some(whitelist) => group_children(&raw, whitelist),
            None => raw.children,
        };
        Self {
            id: raw.id,
            name: raw.name,
            r#type: raw.r#type,
            required: raw.required,
            value: raw.value,
            option: raw.option,
            external_data: raw.external_data,
            children,
        }
    }
}

/// 解析控件组的子控件：丢弃不可见与白名单外项，按白名单稳定排序。
fn group_children(raw: &FormWidgetRaw, whitelist: &[&str]) -> Vec<FormWidget> {
    let parsed;
    let items: &[serde_json::Value] = match &raw.value {
        serde_json::Value::Array(items) => items,
        serde_json::Value::String(text) => {
            parsed = serde_json::from_str::<Vec<serde_json::Value>>(text).unwrap_or_default();
            &parsed
        }
        _ => return Vec::new(),
    };
    let mut children: Vec<FormWidget> = items
        .iter()
        .filter(|item| {
            item.get("visible")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true)
        })
        .filter_map(|item| serde_json::from_value::<FormWidget>(item.clone()).ok())
        .filter(|child| whitelist.contains(&child.id.as_str()))
        .collect();
    children.sort_by_key(|child| {
        whitelist
            .iter()
            .position(|id| *id == child.id)
            .unwrap_or(usize::MAX)
    });

    if raw.r#type == "workGroup" {
        let rule_linked = matches!(
            raw.option.as_ref(),
            Some(WidgetOptions::Config(option)) if option_flag_is_one(option.get("isSetRule"))
        );
        for child in &mut children {
            match child.id.as_str() {
                "widgetWorkGroupType" if rule_linked => child.required = false,
                "widgetWorkGroupTimeRangeFieldList" => child.required = true,
                _ => {}
            }
        }
    }
    children
}

fn option_flag_is_one(value: Option<&serde_json::Value>) -> bool {
    match value {
        Some(serde_json::Value::Number(number)) => number.as_i64() == Some(1),
        Some(serde_json::Value::Bool(flag)) => *flag,
        _ => false,
    }
}

impl FormWidget {
    /// 级联声明的来源控件 id（`linkageConfigs[].linkageWidgetID` 的最后一段）。
    pub fn parent_widget_ids(&self) -> Vec<&str> {
        self.external_data
            .as_ref()
            .and_then(|external| external.get("linkageConfigs"))
            .and_then(serde_json::Value::as_array)
            .map(|configs| {
                configs
                    .iter()
                    .filter_map(|config| config.get("linkageWidgetID"))
                    .filter_map(serde_json::Value::as_str)
                    .filter_map(|id| id.rsplit('.').next())
                    .filter(|id| !id.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 关联审批允许的定义 Code；`None` 表示定义明确声明不限制范围。
    pub fn connect_definition_codes(&self) -> std::result::Result<Option<Vec<String>>, String> {
        if self.r#type != "connect" {
            return Ok(None);
        }
        if let Some(WidgetOptions::Config(option)) = self.option.as_ref() {
            if option
                .get("notLimitScope")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
            {
                return Ok(None);
            }
        }
        if let Some(values) = self.value.as_array() {
            let codes = values
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|code| !code.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>();
            return if codes.is_empty() {
                Err("关联审批控件没有有效的审批定义 Code".to_string())
            } else {
                Ok(Some(codes))
            };
        }
        let Some(WidgetOptions::Config(option)) = self.option.as_ref() else {
            return Err("关联审批控件缺少关联范围配置".to_string());
        };
        let codes = option
            .get("definitionIds")
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|code| !code.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if codes.is_empty() {
            Err("关联审批控件没有有效的审批定义 Code，也未声明不限制范围".to_string())
        } else {
            Ok(Some(codes))
        }
    }

    /// 收集自己与全部后代。
    pub fn walk(&self) -> Vec<&FormWidget> {
        let mut out = vec![self];
        for child in &self.children {
            out.extend(child.walk());
        }
        out
    }

    /// 该控件是否链接到外部选项接口。
    ///
    /// `options_url_path` 由调用方传入（宿主路由前缀），crate 不硬编码产品路径。
    pub fn links_to_external_options(&self, options_url_path: &str) -> bool {
        let Some(external) = &self.external_data else {
            return false;
        };
        if let Some(url) = external
            .get("externalUrl")
            .and_then(serde_json::Value::as_str)
            .filter(|u| !u.is_empty())
        {
            return url.split_once("://").is_some_and(|(scheme, rest)| {
                matches!(scheme, "http" | "https")
                    && rest.split_once('/').is_some_and(|(host, path)| {
                        !host.is_empty()
                            && !host.chars().any(char::is_whitespace)
                            && format!("/{path}")
                                .split(['?', '#'])
                                .next()
                                .unwrap_or_default()
                                .strip_prefix(options_url_path)
                                .is_some_and(|key| {
                                    !key.is_empty()
                                        && key.bytes().all(|c| {
                                            c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-')
                                        })
                                })
                    })
            });
        }
        external
            .get("externalDataLinkage")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    }
}

/// 实测 `form` 既可能是 JSON 字符串也可能是数组，两种都认。
pub fn parse_form(form: &serde_json::Value) -> std::result::Result<Vec<FormWidget>, FormError> {
    let parsed: Vec<FormWidget> = match form {
        serde_json::Value::Null => return Err(FormError::Missing),
        serde_json::Value::String(raw) => {
            serde_json::from_str(raw).map_err(|error| FormError::Parse(error.to_string()))?
        }
        serde_json::Value::Array(_) => serde_json::from_value(form.clone())
            .map_err(|error| FormError::Parse(error.to_string()))?,
        other => {
            let name = match other {
                serde_json::Value::Bool(_) => "bool",
                serde_json::Value::Number(_) => "number",
                serde_json::Value::Object(_) => "object",
                _ => "unknown",
            };
            return Err(FormError::UnexpectedType(name));
        }
    };
    if parsed.is_empty() {
        return Err(FormError::Empty);
    }
    Ok(parsed)
}

/// 从 JSON 文本快照解析控件树。
pub fn parse_snapshot(snapshot: &str) -> std::result::Result<Vec<FormWidget>, FormError> {
    let form: serde_json::Value =
        serde_json::from_str(snapshot).map_err(|error| FormError::Parse(error.to_string()))?;
    parse_form(&form)
}

/// 审批定义 `form` 的**原文**快照文本（统一存数组形态）。
pub fn form_snapshot_text(form: Option<&serde_json::Value>) -> String {
    match form {
        Some(serde_json::Value::String(raw)) => raw.clone(),
        Some(value @ serde_json::Value::Array(_)) => {
            serde_json::to_string(value).unwrap_or_default()
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_array_and_string_forms() {
        let value = serde_json::json!([{"id": "w1", "name": "A", "type": "input"}]);
        assert_eq!(parse_form(&value).unwrap().len(), 1);
        assert_eq!(
            parse_form(&serde_json::Value::String(value.to_string()))
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn rejects_null_empty_and_unknown_shapes() {
        assert!(matches!(
            parse_form(&serde_json::Value::Null),
            Err(FormError::Missing)
        ));
        assert!(matches!(
            parse_form(&serde_json::json!([])),
            Err(FormError::Empty)
        ));
        assert!(matches!(
            parse_form(&serde_json::json!(1)),
            Err(FormError::UnexpectedType("number"))
        ));
    }

    #[test]
    fn option_has_four_shapes() {
        let form = parse_form(&serde_json::json!([
            {"id": "a", "type": "input"},
            {"id": "b", "type": "radioV2", "option": null},
            {"id": "c", "type": "radioV2", "option": [{"text": "A", "value": "1"}]},
            {"id": "d", "type": "connect", "option": {"notLimitScope": true}}
        ]))
        .unwrap();
        assert!(form[0].option.is_none());
        assert!(form[1].option.is_none());
        assert!(matches!(form[2].option, Some(WidgetOptions::Fixed(_))));
        assert!(matches!(form[3].option, Some(WidgetOptions::Config(_))));
    }

    #[test]
    fn control_group_children_are_parsed_sorted_and_filtered() {
        let form = parse_form(&serde_json::json!([{
            "id": "g",
            "type": "workGroup",
            "value": [
                {"id": "widgetWorkGroupReason", "type": "textarea", "visible": true},
                {"id": "other", "type": "input"},
                {"id": "widgetWorkGroupOvertimeWorkers", "type": "contact"},
                {"id": "widgetWorkGroupType", "type": "input", "visible": false}
            ]
        }]))
        .unwrap();
        let ids: Vec<&str> = form[0].children.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["widgetWorkGroupOvertimeWorkers", "widgetWorkGroupReason"]
        );
    }

    #[test]
    fn external_data_rename_is_preserved() {
        let form = parse_form(&serde_json::json!([{
            "id": "w",
            "type": "input",
            "externalData": {"externalUrl": "https://x/api/v1/feishu/approval/options/key"}
        }]))
        .unwrap();
        assert!(form[0].links_to_external_options("/api/v1/feishu/approval/options/"));
        assert!(!form[0].links_to_external_options("/other/"));
    }

    #[test]
    fn snapshot_text_normalizes_to_array() {
        let value = serde_json::json!([{"id": "w"}]);
        assert_eq!(form_snapshot_text(Some(&value)), value.to_string());
        let text = value.to_string();
        assert_eq!(
            form_snapshot_text(Some(&serde_json::Value::String(text.clone()))),
            text
        );
        assert_eq!(form_snapshot_text(None), "");
    }
}
