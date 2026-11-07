//! 多维表格响应 DTO。
//!
//! 刻意**不**标 `#[non_exhaustive]`：宿主用结构体字面量构造，0.x 的破坏性变更
//! 走 minor。不要给它们实现 `Default`——「默认空快照」正是调用方要拒绝的形态。

use std::collections::BTreeMap;

use serde::Deserialize;

/// 一条记录。
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RecordItem {
    /// 记录 id。
    #[serde(default)]
    pub record_id: String,
    /// 字段值。键是字段名；值是多态的 union，不能 typed。
    #[serde(default)]
    pub fields: BTreeMap<String, serde_json::Value>,
    /// 记录创建人（`automatic_fields: true` 时返回，在 record 顶层）。
    #[serde(default)]
    pub created_by: Option<RecordPerson>,
}

/// 记录的系统人员字段。
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RecordPerson {
    /// open_id（请求体 `user_id_type=open_id`）。
    #[serde(default)]
    pub id: String,
    /// 姓名；缺通讯录字段权限时可能不返回。
    #[serde(default)]
    pub name: Option<String>,
}

/// 《列出记录》的 `data`。
#[derive(Debug, Clone, Deserialize)]
pub struct ListRecordsData {
    /// 是否还有下一页。
    #[serde(default)]
    pub has_more: bool,
    /// 下一页游标；`has_more=false` 时不出现。
    #[serde(default)]
    pub page_token: Option<String>,
    /// 总记录数；用于收敛断言。
    #[serde(default)]
    pub total: Option<i64>,
    /// 本页记录。
    #[serde(default)]
    pub items: Vec<RecordItem>,
}

/// 《列出数据表》的 `data`。
#[derive(Debug, Clone, Deserialize)]
pub struct BitableTablesData {
    /// 是否还有下一页。
    #[serde(default)]
    pub has_more: bool,
    /// 下一页游标。
    #[serde(default)]
    pub page_token: Option<String>,
    /// 总表数。
    #[serde(default)]
    pub total: Option<i64>,
    /// 本页数据表。
    #[serde(default)]
    pub items: Vec<BitableTableItem>,
}

/// 一张数据表的标识信息。
#[derive(Debug, Clone, Deserialize)]
pub struct BitableTableItem {
    /// 表 id。
    #[serde(default)]
    pub table_id: String,
    /// 表名。
    #[serde(default)]
    pub name: String,
}

/// 《列出视图》的 `data`。
#[derive(Debug, Clone, Deserialize)]
pub struct BitableViewsData {
    /// 是否还有下一页。
    #[serde(default)]
    pub has_more: bool,
    /// 下一页游标。
    #[serde(default)]
    pub page_token: Option<String>,
    /// 总视图数。
    #[serde(default)]
    pub total: Option<i64>,
    /// 本页视图。
    #[serde(default)]
    pub items: Vec<BitableViewItem>,
}

/// 一个视图的标识信息。
#[derive(Debug, Clone, Deserialize)]
pub struct BitableViewItem {
    /// 视图 id。
    #[serde(default)]
    pub view_id: String,
    /// 视图名。
    #[serde(default)]
    pub view_name: String,
    /// `grid` / `kanban` / `gallery` / `gantt` / `form`。
    #[serde(default)]
    pub view_type: String,
}

/// 《列出字段》的 `data`。
#[derive(Debug, Clone, Deserialize)]
pub struct ListFieldsData {
    /// 是否还有下一页。
    #[serde(default)]
    pub has_more: bool,
    /// 下一页游标。
    #[serde(default)]
    pub page_token: Option<String>,
    /// 总字段数。
    #[serde(default)]
    pub total: Option<i64>,
    /// 本页字段。
    #[serde(default)]
    pub items: Vec<FieldItem>,
}

/// 一个字段的标识信息。
#[derive(Debug, Clone, Deserialize)]
pub struct FieldItem {
    /// 是否主字段。
    #[serde(default)]
    pub is_primary: bool,
    /// 字段 id。
    #[serde(default)]
    pub field_id: String,
    /// 字段名。
    #[serde(default)]
    pub field_name: String,
    /// 官方字段类型码（1 文本 / 2 数字 / 3 单选 / 4 多选 / 5 日期 / …）。
    #[serde(default, rename = "type")]
    pub field_type: Option<i32>,
    /// 字段 UI 类型（`SingleSelect` / `MultiSelect` / `Text` …）。
    #[serde(default)]
    pub ui_type: Option<String>,
    /// 字段属性；单选/多选的选项嵌在 `property.options` 下。
    #[serde(default)]
    pub property: Option<FieldProperty>,
}

/// 字段属性。
#[derive(Debug, Clone, Deserialize)]
pub struct FieldProperty {
    /// 选项列表。
    #[serde(default)]
    pub options: Vec<PropertyOption>,
    /// 单向/双向关联列（type 18/21）指向的数据表 id。
    #[serde(default)]
    pub table_id: Option<String>,
}

/// 一个字段选项。
#[derive(Debug, Clone, Deserialize)]
pub struct PropertyOption {
    /// 选项名。
    #[serde(default)]
    pub name: String,
}

impl FieldItem {
    /// 该字段的选项名列表（trim 后去空）。
    pub fn option_names(&self) -> Vec<String> {
        self.property
            .as_ref()
            .map(|property| {
                property
                    .options
                    .iter()
                    .map(|option| option.name.trim().to_string())
                    .filter(|name| !name.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// 《查询记录》的 `data`。
#[derive(Debug, Clone, Deserialize)]
pub struct SearchRecordsData {
    /// 本页记录。
    #[serde(default)]
    pub items: Vec<RecordItem>,
    /// 是否还有下一页。
    #[serde(default)]
    pub has_more: bool,
    /// 下一页游标。
    #[serde(default)]
    pub page_token: Option<String>,
    /// 总记录数。
    #[serde(default)]
    pub total: Option<i64>,
}

/// 《批量获取记录》的 `data`。
#[derive(Debug, Clone, Deserialize)]
pub struct BatchGetRecordsData {
    /// 取回的记录。
    #[serde(default)]
    pub records: Vec<RecordItem>,
    /// 高级权限下被拒绝的记录 id。
    #[serde(default)]
    pub forbidden_record_ids: Vec<String>,
    /// 表里已经不存在的记录 id。
    #[serde(default)]
    pub absent_record_ids: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_property_reads_table_id_and_defaults_to_none() {
        let field: FieldItem = serde_json::from_value(serde_json::json!({
            "field_id": "fld1", "field_name": "关联", "type": 18,
            "property": {"table_id": "tblTarget"}
        }))
        .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            field
                .property
                .and_then(|property| property.table_id)
                .as_deref(),
            Some("tblTarget")
        );

        let field: FieldItem = serde_json::from_value(serde_json::json!({
            "field_id": "fld2", "field_name": "文本", "type": 1
        }))
        .unwrap_or_else(|error| panic!("{error}"));
        assert!(field.property.is_none());
    }
}
