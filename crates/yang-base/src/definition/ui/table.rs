//! 通用 TableView/Form/Tree 的请求级契约与关系 options、排序、校验提示。

use super::hints::{SortDirection, WidgetHint};
use crate::definition::FieldRef;
use schemars::JsonSchema;
use serde::Serialize;

/// 关系字段的稳定 options 与展示契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct RelationOptionsSchema {
    /// 返回 [`RelationOptionsResponse`](crate::table::RelationOptionsResponse) 的 Action。
    pub operation_id: String,
    /// 关系值对应的限定目标字段。
    pub value_field: String,
    /// 用于解释已选值的限定展示字段。
    pub label_fields: Vec<String>,
}

/// View 构建期声明的默认排序。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSortSpec {
    /// 必须属于当前 View 且允许排序的字段。
    pub field: FieldRef,
    /// 默认排序方向。
    pub direction: SortDirection,
}

impl TableSortSpec {
    /// 创建默认排序声明。
    pub fn new(field: FieldRef, direction: SortDirection) -> Self {
        Self { field, direction }
    }
}

/// 请求级默认排序。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct TableSortSchema {
    /// 行数据中的本地字段名。
    pub field: String,
    /// 默认排序方向。
    pub direction: SortDirection,
}

/// 通用 TableView 的查询能力与服务端分页边界。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct TableQuerySchema {
    /// 当前用户可见且允许关键词搜索的字段。
    pub search_fields: Vec<String>,
    /// 当前用户可见且允许结构化条件筛选的字段。
    pub filter_fields: Vec<String>,
    /// 按声明顺序排列的默认排序；不可读字段会从请求级投影中移除。
    pub default_sort: Vec<TableSortSchema>,
    /// 默认分页大小。
    pub default_page_size: usize,
    /// 服务端强制执行的最大分页大小。
    pub max_page_size: usize,
}

/// 通用表格页的单列展示契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct TableColumnSchema {
    /// 行数据中的字段名。
    pub field: String,
    /// 用户可见标题。
    pub title: String,
    /// 字段帮助说明。
    pub description: String,
    /// 建议控件；未知值由消费者降级为 JSON/text。
    pub widget: WidgetHint,
    /// 输入时是否必填。
    pub required: bool,
    /// 是否允许关键词搜索。
    pub searchable: bool,
    /// 是否允许作为结构化筛选字段。
    pub filterable: bool,
    /// 是否允许排序。
    pub sortable: bool,
    /// 当前请求有权调用的关系 options 契约。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relation: Option<RelationOptionsSchema>,
}

/// 通用表单字段的输入校验提示。
///
/// 与存储类型解耦，仅作为前端预校验提示；服务端 Handler 仍必须独立执行权威校验。
/// 未声明的约束一律省略，不占用线上字节。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct FormFieldValidationSchema {
    /// 字符最小长度。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_length: Option<usize>,
    /// 字符最大长度。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_length: Option<usize>,
    /// 数值下限，十进制文本，避免浮点漂移。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<String>,
    /// 数值上限，十进制文本，避免浮点漂移。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<String>,
    /// 可选正则表达式。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
}

impl FormFieldValidationSchema {
    /// 从字段定义投影校验提示；字段未声明任何约束时返回 `None`。
    pub(crate) fn from_spec(spec: &crate::definition::ValidationSpec) -> Option<Self> {
        if spec.min_length.is_none()
            && spec.max_length.is_none()
            && spec.minimum.is_none()
            && spec.maximum.is_none()
            && spec.pattern.is_none()
        {
            return None;
        }
        Some(Self {
            min_length: spec.min_length,
            max_length: spec.max_length,
            minimum: spec.minimum.clone(),
            maximum: spec.maximum.clone(),
            pattern: spec.pattern.clone(),
        })
    }
}

/// 通用表单的单字段契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct FormFieldSchema {
    /// 提交数据中的字段名。
    pub field: String,
    /// 用户可见标题。
    pub title: String,
    /// 字段帮助说明。
    pub description: String,
    /// 建议输入控件。
    pub widget: WidgetHint,
    /// 当前表单中是否必填。
    pub required: bool,
    /// 当前用户只能查看，不能提交修改。
    pub read_only: bool,
    /// 字段只允许提交，前端不得从详情数据预填。
    pub write_only: bool,
    /// 当前请求有权调用的关系 options 契约。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relation: Option<RelationOptionsSchema>,
    /// 服务端声明的输入校验提示；随字段权限一并过滤，未声明约束时省略。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<FormFieldValidationSchema>,
}

/// 请求级通用表单契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct FormSchema {
    /// 按 View 定义顺序排列的字段。
    pub fields: Vec<FormFieldSchema>,
}

/// 请求级通用树 View 拓扑契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct TreeViewSchema {
    /// 节点唯一标识字段。
    pub id_field: String,
    /// 父节点标识字段；根节点应返回 null。
    pub parent_field: String,
    /// 节点用户可见标签字段。
    pub label_field: String,
    /// 服务端强制执行的单次树查询节点上限。
    pub max_nodes: usize,
}

/// 请求级通用表格 View 契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct TableViewSchema {
    /// 稳定 View ID，不与前端文件路径绑定。
    pub view_id: String,
    /// 用户可见标题。
    pub title: String,
    /// 服务端表定义名。
    pub table: String,
    /// 返回标准分页行数据的 Action operation ID。
    pub data_action: String,
    /// 当前用户可读的有序列。
    pub columns: Vec<TableColumnSchema>,
    /// 当前用户可读或可写的通用表单字段。
    pub form: FormSchema,
    /// 可选树拓扑；所需字段不可读时省略并安全降级为普通表格。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tree: Option<TreeViewSchema>,
    /// 搜索、筛选、默认排序和分页能力。
    pub query: TableQuerySchema,
    /// 当前用户可调用的有序 Action operation IDs。
    pub actions: Vec<String>,
    /// 当前用户可调用 Action 的声明式展示语义。
    pub action_presentations: Vec<super::action::ActionPresentationSchema>,
}
