//! 受字段权限、租户范围与分页边界保护的默认关系 options Action。
#![cfg(feature = "mysql")]

use crate::action::{Action as ActionHandler, ActionContext};
use crate::error::BaseError;
use crate::table::{Record, RelationOption, RelationOptionsRequest, RelationOptionsResponse};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashSet;
use yang_base_derive::Action;

/// 默认关系 options 查询器。
///
/// 注册方只声明目标值字段与展示字段；字段存在性、读取/筛选权限、关键词搜索白名单、
/// 租户范围和分页上限均由 `TableQuery` 与 `RelationOptionsRequest` 在服务端强制执行。
#[derive(Debug, Action)]
#[action(
    name = "relation_options",
    display_name = "关系选项",
    description = "搜索并回填受权限保护的关系选项",
    method = "POST",
    path = "/relation-options"
)]
pub struct RelationOptionsAction {
    value_field: String,
    label_fields: Vec<String>,
}

impl RelationOptionsAction {
    /// 创建默认关系 options 查询器。
    ///
    /// Action 默认受保护；调用方可通过自定义 `ActionSpec` 设置路由和权限，但不能
    /// 绕过字段与租户校验。
    pub fn new<I, S>(value_field: impl Into<String>, label_fields: I) -> Result<Self, BaseError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let value_field = value_field.into();
        let label_fields = label_fields.into_iter().map(Into::into).collect::<Vec<_>>();
        if value_field.trim().is_empty() {
            return Err(BaseError::ConfigError(
                "关系 options value_field 不能为空".to_string(),
            ));
        }
        if label_fields.is_empty() || label_fields.iter().any(|field| field.trim().is_empty()) {
            return Err(BaseError::ConfigError(
                "关系 options 至少需要一个非空 label_field".to_string(),
            ));
        }
        Ok(Self {
            value_field,
            label_fields,
        })
    }

    fn select_fields(&self) -> Vec<&str> {
        std::iter::once(self.value_field.as_str())
            .chain(self.label_fields.iter().map(String::as_str))
            .collect()
    }

    fn option_from_record(&self, record: &Record) -> Result<RelationOption, BaseError> {
        let value = record.get(&self.value_field).cloned().ok_or_else(|| {
            BaseError::ConfigError(format!(
                "关系 options 结果缺少 value_field {}",
                self.value_field
            ))
        })?;
        if !matches!(value, Value::String(_) | Value::Number(_)) {
            return Err(BaseError::ConfigError(format!(
                "关系 options value_field {} 必须是字符串或数字",
                self.value_field
            )));
        }
        let label = self
            .label_fields
            .iter()
            .filter_map(|field| record.get(field))
            .filter(|value| !value.is_null())
            .map(|value| match value {
                Value::String(value) => value.clone(),
                value => value.to_string(),
            })
            .collect::<Vec<_>>()
            .join(" · ");
        Ok(RelationOption {
            label: if label.is_empty() {
                value.to_string()
            } else {
                label
            },
            value,
        })
    }
}

#[async_trait]
impl ActionHandler for RelationOptionsAction {
    type Input = RelationOptionsRequest;
    type Output = RelationOptionsResponse;

    async fn index(
        &self,
        context: ActionContext,
        input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        if context.user.is_none() {
            return Err(BaseError::Unauthorized("需要登录".to_string()));
        }

        let fields = self.select_fields();
        let mut page_query = context.table_query()?.select_fields(&fields)?;
        for (field, value) in &input.filter {
            page_query = page_query.where_eq(field, value.clone())?;
        }
        page_query = page_query.search(input.search.as_deref())?;
        let total = page_query.clone().count().await?;
        let page_rows = page_query.page(input.page, input.limit)?.all().await?;

        let mut options = Vec::new();
        let mut seen_values = HashSet::new();
        for record in &page_rows {
            let option = self.option_from_record(record)?;
            let key = serde_json::to_string(&option.value).map_err(|error| {
                BaseError::ConfigError(format!("关系 options value 无法编码: {error}"))
            })?;
            if seen_values.insert(key) {
                options.push(option);
            }
        }

        if !input.selected.is_empty() {
            let mut selected_query = context.table_query()?.select_fields(&fields)?;
            for (field, value) in &input.filter {
                selected_query = selected_query.where_eq(field, value.clone())?;
            }
            let selected_rows = selected_query
                .where_in(&self.value_field, input.selected)?
                .all()
                .await?;
            for record in &selected_rows {
                let option = self.option_from_record(record)?;
                let key = serde_json::to_string(&option.value).map_err(|error| {
                    BaseError::ConfigError(format!("关系 options value 无法编码: {error}"))
                })?;
                if seen_values.insert(key) {
                    options.push(option);
                }
            }
        }

        Ok(RelationOptionsResponse {
            items: options,
            page: input.page,
            limit: input.limit,
            total: Some(total),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_and_wire_mapping_fail_closed() {
        assert!(RelationOptionsAction::new("", ["name"]).is_err());
        assert!(RelationOptionsAction::new("id", Vec::<String>::new()).is_err());

        let action = RelationOptionsAction::new("id", ["code", "name"])
            .expect("合法关系 options 配置应构建");
        let record = Record::new()
            .set("id", 7)
            .set("code", "A")
            .set("name", "Alice");
        let option = action
            .option_from_record(&record)
            .expect("合法关系记录应映射");
        assert_eq!(option.value, serde_json::json!(7));
        assert_eq!(option.label, "A · Alice");

        let invalid = Record::new().set("id", serde_json::json!({"nested": true}));
        assert!(action.option_from_record(&invalid).is_err());
    }
}
