//! 面向前端运行时的版本化 UI 契约。
//!
//! 本模块只定义声明式数据，不包含组件路径、脚本或权限判定。请求级权限过滤由
//! 上层 projector 在构造 [`UiCatalog`] 前完成，避免把未授权 Action 暴露给前端。
//!
//! # 租户维度决策
//!
//! 目录投影当前不包含租户维度，这是有意决策：同一身份在不同租户下看到相同的
//! 契约结构，租户隔离由数据层（`tenant_key` 字段、租户中间件与 TableQuery 数据
//! 范围）独立强制。如此可避免目录 revision 随租户组合爆炸，也避免租户存在性经
//! 契约差异泄漏。后续若确需按租户裁剪视图，接入点为请求级投影入口
//! [`BuiltApp::ui_catalog`](crate::definition::BuiltApp::ui_catalog)（其实现位于
//! `definition/builder.rs` 的 registry 投影），在那里按请求上下文裁剪即可，
//! 本模块的契约类型无需变更。

use super::{ActionSpec, FieldRef, ParamSource};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// 当前 UI 契约版本。
pub const UI_SCHEMA_VERSION: &str = "2.0";

/// 与存储类型解耦的前端控件提示。
///
/// 该值只表达建议展示方式，不改变字段验证、权限或数据库语义。消费者遇到新版本
/// 未知值时必须降级为 [`Json`](Self::Json)，而不是拒绝整个页面契约。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WidgetHint {
    /// 单行文本。
    Text,
    /// 多行文本。
    Textarea,
    /// 密码输入。
    Password,
    /// 邮箱输入。
    Email,
    /// URL 输入。
    Url,
    /// 颜色输入。
    Color,
    /// 富文本或编辑器输入。
    Editor,
    /// 整数输入。
    Integer,
    /// 定点小数输入。
    Decimal,
    /// 布尔开关。
    Switch,
    /// 固定枚举单选。
    Radio,
    /// 关系选项选择器。
    RelationSelect,
    /// 树关系选择器。
    TreeSelect,
    /// 日期时间输入。
    DateTime,
    /// 安全的通用 JSON/text fallback。
    #[default]
    #[serde(other)]
    Json,
}

/// Action 成功响应的静态类别。
///
/// 该值决定前端的安全展示通道；它描述 Action 契约，不从某次运行时响应猜测。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ActionResponseKind {
    /// 普通 JSON 响应。
    #[default]
    Json,
    /// 文件下载。
    Download,
    /// 浏览器内文件预览。
    Preview,
    /// HTTP 重定向。
    Redirect,
}

/// Action 在通用业务 View 中的展示位置。
///
/// 未知值必须降级到工具栏，避免在没有行或批量选择上下文时错误传参。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ActionPlacement {
    /// 单行上下文操作。
    Row,
    /// 多选记录后的批量操作。
    Bulk,
    /// View 级工具栏操作，也是未知值的安全降级。
    #[default]
    #[serde(other)]
    Toolbar,
}

/// Action 的声明式交互方式。
///
/// 该值不包含组件路径或动态代码。未知值降级为直接调用，由默认 Action 演示层
/// 负责收集参数和展示响应。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ActionInteraction {
    /// 根据 Action 输入契约生成表单后调用。
    Form,
    /// 调用后下载文件。
    Download,
    /// 调用后在浏览器内预览文件。
    Preview,
    /// 调用后执行服务端声明的安全跳转。
    Navigate,
    /// 交给前端白名单注册的自定义 View。
    Custom,
    /// 直接调用 Action，也是未知值的安全降级。
    #[default]
    #[serde(other)]
    Invoke,
}

/// 危险或不可逆 Action 的二次确认文案。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ActionConfirmation {
    /// 确认框标题。
    pub title: String,
    /// 确认框正文。
    pub message: String,
}

impl ActionConfirmation {
    /// 创建二次确认文案。
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
        }
    }
}

/// 通用 View 对 Action 的非安全性可用状态提示。
///
/// 未声明表示正常可用。未知状态按 disabled 处理，避免前端在无法理解新状态时误触发
/// 操作。该提示只改善界面体验，服务端仍必须独立执行授权和业务前置条件。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AvailabilityState {
    /// 从界面隐藏，但不能据此推断 Action 不可直接调用。
    Hidden,
    /// 显示为禁用，也是未知值的安全降级。
    #[default]
    #[serde(other)]
    Disabled,
}

/// Action 的展示可用性与用户可见原因。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AvailabilityHint {
    /// 隐藏或禁用提示。
    pub state: AvailabilityState,
    /// 用户可见原因；构建期拒绝空白和超长内容。
    pub reason: String,
}

impl AvailabilityHint {
    /// 创建禁用提示。
    pub fn disabled(reason: impl Into<String>) -> Self {
        Self {
            state: AvailabilityState::Disabled,
            reason: reason.into(),
        }
    }

    /// 创建隐藏提示。
    pub fn hidden(reason: impl Into<String>) -> Self {
        Self {
            state: AvailabilityState::Hidden,
            reason: reason.into(),
        }
    }
}

/// View 构建期声明的 Action 展示语义。
///
/// [`Custom`](ActionInteraction::Custom) 必须同时声明稳定 `view_id`；其它交互禁止
/// 携带 `view_id`，避免把它误用为物理文件路径。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionPresentationSpec {
    /// Action 展示位置。
    pub placement: ActionPlacement,
    /// Action 交互方式。
    pub interaction: ActionInteraction,
    /// 可选二次确认。
    pub confirmation: Option<ActionConfirmation>,
    /// 可选的非安全性可用提示。
    pub availability: Option<AvailabilityHint>,
    /// 前端白名单注册表中的稳定标识。
    pub view_id: Option<String>,
}

impl ActionPresentationSpec {
    /// 创建显式展示声明。
    pub fn new(placement: ActionPlacement, interaction: ActionInteraction) -> Self {
        Self {
            placement,
            interaction,
            confirmation: None,
            availability: None,
            view_id: None,
        }
    }

    /// 设置二次确认文案。
    #[must_use]
    pub fn confirmation(mut self, confirmation: ActionConfirmation) -> Self {
        self.confirmation = Some(confirmation);
        self
    }

    /// 设置展示可用性提示。
    #[must_use]
    pub fn availability(mut self, availability: AvailabilityHint) -> Self {
        self.availability = Some(availability);
        self
    }

    /// 设置自定义 View 的稳定白名单标识。
    #[must_use]
    pub fn view_id(mut self, view_id: impl Into<String>) -> Self {
        self.view_id = Some(view_id.into());
        self
    }
}

/// 请求级 Action 展示契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ActionPresentationSchema {
    /// 全局唯一 operation id。
    pub operation_id: String,
    /// 用户可见标题。
    pub title: String,
    /// Action 展示位置。
    pub placement: ActionPlacement,
    /// Action 交互方式。
    pub interaction: ActionInteraction,
    /// 可选二次确认。
    pub confirmation: Option<ActionConfirmation>,
    /// 可选的非安全性可用提示。
    pub availability: Option<AvailabilityHint>,
    /// 前端白名单注册表中的稳定标识；仅 custom 交互可用。
    pub view_id: Option<String>,
}

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
    pub request_media_type: super::ActionMediaType,
    /// multipart 资源与类型限制；JSON Action 为 `None`。
    pub multipart: Option<super::MultipartSpec>,
    /// 成功响应的展示类别。
    pub response_kind: ActionResponseKind,
    /// 是否必须先建立认证身份。
    pub requires_auth: bool,
}

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

/// 通用表格排序方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SortDirection {
    /// 降序。
    Desc,
    /// 升序，也是未知值的兼容降级。
    #[default]
    #[serde(other)]
    Asc,
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
    pub(crate) fn from_spec(spec: &super::ValidationSpec) -> Option<Self> {
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
    pub action_presentations: Vec<ActionPresentationSchema>,
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

/// 一次请求返回给前端的 UI 目录契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct UiCatalog {
    /// UI schema 版本；前端必须按版本选择解析器。
    pub schema_version: &'static str,
    /// 当前过滤后完整目录的确定性 SHA-256 修订标识。
    ///
    /// 消费端可用它判断缓存内容是否变化；身份或租户切换仍必须重新请求目录，不能
    /// 把 revision 当作授权凭据。
    pub revision: String,
    /// 当前请求有权访问的 Action 演示契约。
    pub actions: Vec<ActionDemoSchema>,
    /// 当前请求有权访问的通用表格 Views。
    pub table_views: Vec<TableViewSchema>,
}

impl UiCatalog {
    /// 从已经完成请求级过滤的 Action 集合构造目录，并按 operation id 稳定排序。
    pub fn new<I>(actions: I) -> Result<Self, crate::error::BaseError>
    where
        I: IntoIterator<Item = ActionDemoSchema>,
    {
        let mut actions = actions.into_iter().collect::<Vec<_>>();
        actions.sort_by(|left, right| left.operation_id.cmp(&right.operation_id));
        let mut catalog = Self {
            schema_version: UI_SCHEMA_VERSION,
            revision: String::new(),
            actions,
            table_views: Vec::new(),
        };
        catalog.refresh_revision()?;
        Ok(catalog)
    }

    pub(crate) fn with_table_views<I>(mut self, views: I) -> Result<Self, crate::error::BaseError>
    where
        I: IntoIterator<Item = TableViewSchema>,
    {
        self.table_views = views.into_iter().collect();
        self.table_views
            .sort_by(|left, right| left.view_id.cmp(&right.view_id));
        self.refresh_revision()?;
        Ok(self)
    }

    fn refresh_revision(&mut self) -> Result<(), crate::error::BaseError> {
        let payload = serde_json::to_vec(&(
            self.schema_version,
            self.actions.as_slice(),
            self.table_views.as_slice(),
        ))
        .map_err(|error| crate::error::BaseError::JsonSerializeFailed(error.to_string()))?;
        let digest = Sha256::digest(payload);
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut revision = String::with_capacity(digest.len() * 2);
        for byte in digest {
            revision.push(HEX[usize::from(byte >> 4)] as char);
            revision.push(HEX[usize::from(byte & 0x0f)] as char);
        }
        self.revision = revision;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{ActionContext, PermissionMode, Request, TypedHandler, User};
    use crate::definition::{
        AccessRule, ActionName, ActionRef, AddonName, AddonSpec, AppBuilder, BuildError, FieldKind,
        FieldName, FieldRef, FieldSpec, HttpMethod, ModuleName, ModuleSpec, ParamSpec, RouteSpec,
        TableName, TableSpec, TreeViewSpec, ViewName, ViewSpec,
    };
    use crate::error::BaseError;
    use crate::tools::ToolsBuilder;
    use async_trait::async_trait;
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    #[derive(Debug, Deserialize, schemars::JsonSchema)]
    struct NoopInput {}

    #[derive(Debug, Serialize, schemars::JsonSchema)]
    struct NoopOutput {}

    #[derive(crate::Action)]
    #[action(name = "noop", public)]
    struct NoopAction;

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

    fn action(name: &str, operation_id: &str) -> ActionSpec {
        ActionSpec::new(
            ActionName::new(name).expect("测试 Action 名称应有效"),
            RouteSpec::new(HttpMethod::Post, format!("/{name}"), operation_id),
        )
    }

    #[test]
    fn ui_catalog_serializes_stable_minimal_action_contract() {
        let mut protected = action("export", "org.user.export")
            .display_name("导出用户")
            .description("导出当前租户用户")
            .response_kind(ActionResponseKind::Download)
            .param(
                ParamSpec::new(
                    FieldName::new("tenant_id").expect("测试字段名应有效"),
                    ParamSource::Path,
                )
                .required(false),
            )
            .param(ParamSpec::new(
                FieldName::new("filter").expect("测试字段名应有效"),
                ParamSource::Body,
            ))
            .param(ParamSpec::new(
                FieldName::new("search").expect("测试字段名应有效"),
                ParamSource::Query,
            ))
            .param(ParamSpec::new(
                FieldName::new("request_id").expect("测试字段名应有效"),
                ParamSource::Header,
            ));
        protected.input_schema =
            json!({"type": "object", "properties": {"filter": {"type": "string"}}});
        protected.output_schema = json!({"type": "string", "format": "binary"});
        let public = action("health", "health.check").public(true);
        let catalog = UiCatalog::new([
            ActionDemoSchema::from(&protected),
            ActionDemoSchema::from(&public),
        ])
        .expect("UI Catalog revision 应可计算");

        let value = serde_json::to_value(catalog).expect("UI Catalog 应可序列化");
        assert_eq!(value["schema_version"], UI_SCHEMA_VERSION);
        let revision = value["revision"]
            .as_str()
            .expect("UI Catalog 应携带 revision");
        assert_eq!(revision.len(), 64);
        assert!(revision.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(value["actions"][0]["operation_id"], "health.check");
        assert_eq!(value["actions"][0]["requires_auth"], false);
        assert_eq!(value["actions"][1]["response_kind"], "download");
        assert_eq!(value["actions"][1]["params"][0]["source"], "path");
        assert_eq!(value["actions"][1]["params"][0]["required"], true);
        assert_eq!(value["actions"][1]["params"][0]["title"], "tenant_id");
        assert_eq!(value["actions"][1]["params"][1]["source"], "body");
        assert_eq!(value["actions"][1]["params"][2]["source"], "query");
        assert_eq!(value["actions"][1]["params"][3]["source"], "header");
        assert_eq!(value["actions"][1]["input_schema"], protected.input_schema);
        assert_eq!(
            value["actions"][1]["output_schema"],
            protected.output_schema
        );
        assert_eq!(value["actions"][1]["method"], "POST");
    }

    #[test]
    fn catalog_revision_is_order_independent_and_content_sensitive() {
        let first = ActionDemoSchema::from(&action("first", "org.user.first"));
        let second = ActionDemoSchema::from(&action("second", "org.user.second"));
        let ordered =
            UiCatalog::new([first.clone(), second.clone()]).expect("有序目录 revision 应可计算");
        let reversed = UiCatalog::new([second.clone(), first]).expect("逆序目录 revision 应可计算");
        assert_eq!(ordered.actions, reversed.actions);
        assert_eq!(ordered.revision, reversed.revision);

        let mut changed = second;
        changed.title = "新的展示标题".to_string();
        let changed = UiCatalog::new([changed]).expect("变更目录 revision 应可计算");
        assert_ne!(ordered.revision, changed.revision);
    }

    #[test]
    fn catalog_json_schema_requires_version_revision_actions_and_views() {
        let schema = serde_json::to_value(schemars::schema_for!(UiCatalog))
            .expect("UiCatalog JSON Schema 应可序列化");
        let required = schema["required"]
            .as_array()
            .expect("UiCatalog schema.required 应存在");
        for field in ["schema_version", "revision", "actions", "table_views"] {
            assert!(
                required.iter().any(|value| value == field),
                "UiCatalog 运行时 schema 应要求字段 {field}: {schema}"
            );
        }

        let tree_schema = serde_json::to_value(schemars::schema_for!(TreeViewSchema))
            .expect("TreeViewSchema JSON Schema 应可序列化");
        let tree_required = tree_schema["required"]
            .as_array()
            .expect("TreeViewSchema schema.required 应存在");
        for field in ["id_field", "parent_field", "label_field"] {
            assert!(
                tree_required.iter().any(|value| value == field),
                "TreeViewSchema 运行时 schema 应要求字段 {field}: {tree_schema}"
            );
        }
    }

    #[test]
    fn action_demo_does_not_leak_internal_permissions_calls_or_tags() {
        let spec = action("remove", "org.user.remove")
            .permissions(["org.user.delete"], crate::action::PermissionMode::All)
            .tag("internal")
            .calls(crate::definition::ActionRef::new(
                crate::definition::ModuleName::new("audit.log").expect("测试 Module 名称应有效"),
                ActionName::new("write").expect("测试 Action 名称应有效"),
            ));
        let value = serde_json::to_value(ActionDemoSchema::from(&spec))
            .expect("ActionDemoSchema 应可序列化");

        assert_eq!(value["requires_auth"], json!(true));
        assert!(value.get("permissions").is_none());
        assert!(value.get("permission_mode").is_none());
        assert!(value.get("calls").is_none());
        assert!(value.get("tags").is_none());
    }

    #[tokio::test]
    async fn request_projection_reuses_dispatch_authorization_policy() {
        let module = ModuleSpec::new(ModuleName::new("org.user").expect("测试 Module 名称应有效"))
            .default_permissions(["module:access"], PermissionMode::All)
            .action(
                action("public", "org.user.public")
                    .public(true)
                    .permissions(["never:granted"], PermissionMode::All),
                NoopAction,
            )
            .action(action("member", "org.user.member"), NoopAction)
            .action(
                action("all", "org.user.all")
                    .permissions(["record:read", "record:write"], PermissionMode::All),
                NoopAction,
            )
            .action(
                action("any", "org.user.any")
                    .permissions(["record:read", "record:write"], PermissionMode::Any),
                NoopAction,
            );
        let app = AppBuilder::new()
            .addon(
                AddonSpec::new(AddonName::new("org").expect("测试 Addon 名称应有效"))
                    .module(module),
            )
            .build(ToolsBuilder::new().build().expect("测试 Tools 应构建成功"))
            .expect("测试 App 应构建成功");

        let anonymous = app.context(Request::new(serde_json::Value::Null));
        let anonymous_revision = app
            .ui_catalog(&anonymous)
            .expect("匿名 UI Catalog revision 应可计算")
            .revision;
        assert_eq!(
            anonymous_revision,
            app.ui_catalog(&anonymous)
                .expect("重复投影 revision 应可计算")
                .revision,
            "相同请求表示必须产生稳定 revision"
        );
        assert_eq!(
            operation_ids(app.ui_catalog(&anonymous)),
            ["org.user.public"]
        );

        let module_only = app
            .context(Request::new(serde_json::Value::Null))
            .with_user(User::new(1, "module").with_permissions(["module:access"]));
        assert_ne!(
            anonymous_revision,
            app.ui_catalog(&module_only)
                .expect("成员 UI Catalog revision 应可计算")
                .revision,
            "权限过滤后的不同表示必须使用不同 revision"
        );
        assert_eq!(
            operation_ids(app.ui_catalog(&module_only)),
            ["org.user.member", "org.user.public"]
        );

        let any = app
            .context(Request::new(serde_json::Value::Null))
            .with_user(User::new(2, "any").with_permissions(["module:access", "record:read"]));
        assert_eq!(
            operation_ids(app.ui_catalog(&any)),
            ["org.user.any", "org.user.member", "org.user.public"]
        );

        let all = app
            .context(Request::new(serde_json::Value::Null))
            .with_user(User::new(3, "all").with_permissions([
                "module:access",
                "record:read",
                "record:write",
            ]));
        assert_eq!(
            operation_ids(app.ui_catalog(&all)),
            [
                "org.user.all",
                "org.user.any",
                "org.user.member",
                "org.user.public"
            ]
        );

        let action_only = app
            .context(Request::new(serde_json::Value::Null))
            .with_user(User::new(4, "action").with_permissions(["record:read", "record:write"]));
        assert_eq!(
            operation_ids(app.ui_catalog(&action_only)),
            ["org.user.public"]
        );

        let all_handle = app
            .registry()
            .resolve(&ActionRef::new(
                ModuleName::new("org.user").expect("测试 Module 名称应有效"),
                ActionName::new("all").expect("测试 Action 名称应有效"),
            ))
            .expect("all Action 应已注册");
        let denied = app
            .dispatch_context(
                all_handle,
                app.context(Request::new(json!({}))).with_user(
                    User::new(4, "action").with_permissions(["record:read", "record:write"]),
                ),
            )
            .await;
        assert!(matches!(denied, Err(BaseError::PermissionDenied(_))));

        let allowed = app
            .dispatch_context(
                all_handle,
                app.context(Request::new(json!({}))).with_user(
                    User::new(3, "all").with_permissions([
                        "module:access",
                        "record:read",
                        "record:write",
                    ]),
                ),
            )
            .await;
        assert!(allowed.is_ok(), "目录可见的 Action 应通过同一授权策略");

        let public_handle = app
            .registry()
            .resolve(&ActionRef::new(
                ModuleName::new("org.user").expect("测试 Module 名称应有效"),
                ActionName::new("public").expect("测试 Action 名称应有效"),
            ))
            .expect("public Action 应已注册");
        let public = app
            .dispatch_context(public_handle, app.context(Request::new(json!({}))))
            .await;
        assert!(
            public.is_ok(),
            "public Action 应同时绕过模块与 Action 权限组"
        );
    }

    fn operation_ids(catalog: Result<UiCatalog, BaseError>) -> Vec<String> {
        let catalog = catalog.expect("UI Catalog revision 应可计算");
        catalog
            .actions
            .into_iter()
            .map(|action| action.operation_id)
            .collect()
    }

    #[test]
    fn widget_hint_maps_field_semantics_without_changing_storage_kind() {
        let field = |kind| FieldSpec::new(FieldName::new("value").expect("字段名应有效"), kind);

        assert_eq!(field(FieldKind::Key).widget_hint(), WidgetHint::Integer);
        assert_eq!(field(FieldKind::Str).widget_hint(), WidgetHint::Text);
        assert_eq!(field(FieldKind::Text).widget_hint(), WidgetHint::Textarea);
        assert_eq!(field(FieldKind::Int).widget_hint(), WidgetHint::Integer);
        assert_eq!(field(FieldKind::Decimal).widget_hint(), WidgetHint::Decimal);
        assert_eq!(field(FieldKind::Switch).widget_hint(), WidgetHint::Switch);
        assert_eq!(field(FieldKind::Radio).widget_hint(), WidgetHint::Radio);
        assert_eq!(
            field(FieldKind::Table).widget_hint(),
            WidgetHint::RelationSelect
        );
        assert_eq!(field(FieldKind::Tree).widget_hint(), WidgetHint::TreeSelect);
        assert_eq!(
            field(FieldKind::Timestamp).widget_hint(),
            WidgetHint::DateTime
        );
    }

    #[test]
    fn widget_hint_explicit_override_and_unknown_value_have_safe_fallbacks() {
        let mut secret = FieldSpec::new(
            FieldName::new("secret").expect("字段名应有效"),
            FieldKind::Str,
        );
        secret.access.secret = true;
        assert_eq!(secret.widget_hint(), WidgetHint::Password);

        secret.presentation.widget = Some(WidgetHint::Email);
        assert_eq!(secret.widget_hint(), WidgetHint::Email);
        assert_eq!(secret.kind, FieldKind::Str, "控件提示不得改变字段数据种类");

        let unknown: WidgetHint =
            serde_json::from_value(json!("future_spatial_editor")).expect("未知提示应安全解析");
        assert_eq!(unknown, WidgetHint::Json);
        assert_eq!(
            serde_json::to_value(unknown).expect("fallback 应可序列化"),
            json!("json")
        );
    }

    #[test]
    fn action_presentation_unknown_values_have_safe_fallbacks() {
        let placement: ActionPlacement =
            serde_json::from_value(json!("floating_palette")).expect("未知位置应安全解析");
        let interaction: ActionInteraction =
            serde_json::from_value(json!("execute_script")).expect("未知交互应安全解析");
        let availability: AvailabilityState =
            serde_json::from_value(json!("scheduled")).expect("未知可用状态应安全解析");
        let sort: SortDirection =
            serde_json::from_value(json!("randomized")).expect("未知排序方向应安全解析");

        assert_eq!(placement, ActionPlacement::Toolbar);
        assert_eq!(interaction, ActionInteraction::Invoke);
        assert_eq!(availability, AvailabilityState::Disabled);
        assert_eq!(sort, SortDirection::Asc);
    }

    #[tokio::test]
    async fn table_view_projection_filters_module_fields_and_actions_with_same_request_identity() {
        let module_name = ModuleName::new("org.member").expect("测试 Module 名称应有效");
        let table_name = TableName::new("org_member").expect("测试 Table 名称应有效");
        let field_ref = |name: &str| {
            FieldRef::new(
                table_name.clone(),
                FieldName::new(name).expect("测试字段名应有效"),
            )
        };
        let action_ref = |name: &str| {
            ActionRef::new(
                module_name.clone(),
                ActionName::new(name).expect("测试 Action 名称应有效"),
            )
        };

        let mut name = FieldSpec::new(
            FieldName::new("name").expect("测试字段名应有效"),
            FieldKind::Str,
        );
        name.presentation.title = "名称".to_string();
        name.access.searchable = true;
        name.access.filterable = true;
        name.access.sortable = true;
        let mut manager_id = FieldSpec::new(
            FieldName::new("manager_id").expect("测试字段名应有效"),
            FieldKind::Table,
        );
        manager_id.relation = Some(field_ref("id"));
        manager_id.select = Some(action_ref("options"));
        manager_id.presentation.display = vec![field_ref("name")];
        let mut parent_id = FieldSpec::new(
            FieldName::new("parent_id").expect("测试字段名应有效"),
            FieldKind::Int,
        );
        parent_id.access.readable = AccessRule::Roles(vec!["admin".to_string()]);
        let mut admin_note = FieldSpec::new(
            FieldName::new("admin_note").expect("测试字段名应有效"),
            FieldKind::Text,
        );
        admin_note.access.readable = AccessRule::Roles(vec!["admin".to_string()]);
        let mut secret = FieldSpec::new(
            FieldName::new("secret").expect("测试字段名应有效"),
            FieldKind::Str,
        );
        secret.access.secret = true;
        secret.access.readable = AccessRule::Everyone;
        secret.storage.required = true;
        let mut created_at = FieldSpec::new(
            FieldName::new("created_at").expect("测试字段名应有效"),
            FieldKind::Timestamp,
        );
        created_at.timestamp_mode = crate::definition::TimestampMode::CreatedAt;

        let view = ViewSpec::new(ViewName::new("main").expect("测试 View 名称应有效"))
            .field(field_ref("id"))
            .field(field_ref("parent_id"))
            .field(field_ref("name"))
            .field(field_ref("manager_id"))
            .field(field_ref("admin_note"))
            .field(field_ref("secret"))
            .field(field_ref("created_at"))
            .action(action_ref("list"))
            .present_action(
                action_ref("edit"),
                ActionPresentationSpec::new(ActionPlacement::Row, ActionInteraction::Form)
                    .confirmation(ActionConfirmation::new("确认修改", "将保存当前行的修改"))
                    .availability(AvailabilityHint::disabled("当前记录可能不允许修改")),
            )
            .tree(TreeViewSpec::new(
                field_ref("id"),
                field_ref("parent_id"),
                field_ref("name"),
            ))
            .default_sort(TableSortSpec::new(field_ref("name"), SortDirection::Asc));
        let module = ModuleSpec::new(module_name.clone())
            .table(
                TableSpec::new(table_name)
                    .title("组织成员")
                    .field(FieldSpec::new(
                        FieldName::new("id").expect("测试字段名应有效"),
                        FieldKind::Key,
                    ))
                    .field(parent_id)
                    .field(name)
                    .field(manager_id)
                    .field(admin_note)
                    .field(secret)
                    .field(created_at),
            )
            .default_permissions(["module:view"], PermissionMode::All)
            .action(action("list", "org.member.list"), NoopAction)
            .action(
                action("options", "org.member.options")
                    .permissions(["member:options"], PermissionMode::All),
                NoopAction,
            )
            .action(
                action("edit", "org.member.edit").permissions(["member:edit"], PermissionMode::All),
                NoopAction,
            )
            .view(view);
        let app = AppBuilder::new()
            .addon(
                AddonSpec::new(AddonName::new("org").expect("测试 Addon 名称应有效"))
                    .module(module),
            )
            .build(ToolsBuilder::new().build().expect("测试 Tools 应构建成功"))
            .expect("TableView 测试应用应构建成功");

        let compiled_tree = app.compiled_views()[0]
            .tree()
            .expect("显式树拓扑应在启动期预编译");
        assert_eq!(compiled_tree.id_field_name(), "id");
        assert_eq!(compiled_tree.parent_field_name(), "parent_id");
        assert_eq!(compiled_tree.label_field_name(), "name");

        let anonymous = app
            .ui_catalog(&app.context(Request::new(json!({}))))
            .expect("匿名 UI Catalog revision 应可计算");
        assert!(
            anonymous.table_views.is_empty(),
            "匿名请求不得看到受保护 View"
        );

        let member = app
            .ui_catalog(
                &app.context(Request::new(json!({})))
                    .with_user(User::new(7, "member").with_permissions(["module:view"])),
            )
            .expect("成员 UI Catalog revision 应可计算");
        assert_eq!(member.table_views.len(), 1);
        assert_eq!(member.table_views[0].view_id, "org.member.main");
        assert_eq!(member.table_views[0].table, "org_member");
        assert!(
            member.table_views[0].tree.is_none(),
            "任一拓扑字段不可读时必须安全降级为普通表格"
        );
        let member_wire =
            serde_json::to_value(&member.table_views[0]).expect("成员 TableView schema 应可序列化");
        assert!(
            member_wire.get("tree").is_none(),
            "不可用树拓扑不能以空壳契约泄漏给前端"
        );
        assert_eq!(
            member.table_views[0]
                .columns
                .iter()
                .map(|column| column.field.as_str())
                .collect::<Vec<_>>(),
            ["id", "name", "manager_id", "created_at"]
        );
        let name_column = &member.table_views[0].columns[1];
        assert_eq!(name_column.widget, WidgetHint::Text);
        assert!(name_column.filterable);
        assert!(name_column.sortable);
        assert_eq!(member.table_views[0].query.search_fields, ["name"]);
        assert_eq!(member.table_views[0].query.filter_fields, ["name"]);
        assert_eq!(member.table_views[0].query.default_sort.len(), 1);
        assert_eq!(member.table_views[0].query.default_sort[0].field, "name");
        assert_eq!(
            member.table_views[0].query.default_sort[0].direction,
            SortDirection::Asc
        );
        assert_eq!(
            member.table_views[0].query.default_page_size,
            crate::table::DEFAULT_QUERY_PAGE_SIZE
        );
        assert_eq!(
            member.table_views[0].query.max_page_size,
            crate::table::MAX_TABLE_QUERY_PAGE_SIZE
        );
        let member_relation = member.table_views[0]
            .columns
            .iter()
            .find(|column| column.field == "manager_id")
            .expect("成员目录应包含 manager_id");
        assert!(
            member_relation.relation.is_none(),
            "无 selector Action 权限时不得泄漏 operation id"
        );
        assert_eq!(member.table_views[0].actions, ["org.member.list"]);
        assert_eq!(member.table_views[0].action_presentations.len(), 1);
        let list_presentation = &member.table_views[0].action_presentations[0];
        assert_eq!(list_presentation.operation_id, "org.member.list");
        assert_eq!(list_presentation.placement, ActionPlacement::Toolbar);
        assert_eq!(list_presentation.interaction, ActionInteraction::Form);
        assert!(list_presentation.confirmation.is_none());
        assert!(list_presentation.view_id.is_none());

        let form = &member.table_views[0].form.fields;
        let form_field = |name: &str| {
            form.iter()
                .find(|field| field.field == name)
                .unwrap_or_else(|| panic!("表单应包含字段 {name}"))
        };
        assert!(form_field("id").read_only, "主键必须只读");
        assert!(form_field("created_at").read_only, "自动时间戳必须只读");
        assert!(
            form_field("admin_note").write_only,
            "不可读但可写字段不得预填"
        );
        let secret_form = form_field("secret");
        assert!(secret_form.write_only, "secret 字段必须只写");
        assert!(!secret_form.read_only);
        assert!(secret_form.required);
        assert_eq!(secret_form.widget, WidgetHint::Password);

        let admin = app
            .ui_catalog(
                &app.context(Request::new(json!({}))).with_user(
                    User::new(8, "admin")
                        .with_roles(["admin"])
                        .with_permissions(["module:view", "member:edit", "member:options"]),
                ),
            )
            .expect("管理员 UI Catalog revision 应可计算");
        assert_eq!(
            admin.table_views[0]
                .columns
                .iter()
                .map(|column| column.field.as_str())
                .collect::<Vec<_>>(),
            [
                "id",
                "parent_id",
                "name",
                "manager_id",
                "admin_note",
                "created_at"
            ],
            "角色字段应出现，但 secret 字段即使 readable 也不得投影"
        );
        assert_eq!(
            admin.table_views[0].tree.as_ref().map(|tree| (
                tree.id_field.as_str(),
                tree.parent_field.as_str(),
                tree.label_field.as_str(),
            )),
            Some(("id", "parent_id", "name"))
        );
        assert_eq!(
            admin.table_views[0].actions,
            ["org.member.list", "org.member.edit"]
        );
        let manager_relation = admin.table_views[0]
            .columns
            .iter()
            .find(|column| column.field == "manager_id")
            .and_then(|column| column.relation.as_ref())
            .expect("有权限时应投影关系 options 契约");
        assert_eq!(manager_relation.operation_id, "org.member.options");
        assert_eq!(manager_relation.value_field, "org_member.id");
        assert_eq!(manager_relation.label_fields, ["org_member.name"]);
        let manager_form_relation = admin.table_views[0]
            .form
            .fields
            .iter()
            .find(|field| field.field == "manager_id")
            .and_then(|field| field.relation.as_ref())
            .expect("表单应复用同一关系 options 契约");
        assert_eq!(manager_form_relation, manager_relation);
        assert_eq!(admin.table_views[0].action_presentations.len(), 2);
        let edit_presentation = &admin.table_views[0].action_presentations[1];
        assert_eq!(edit_presentation.operation_id, "org.member.edit");
        assert_eq!(edit_presentation.placement, ActionPlacement::Row);
        assert_eq!(edit_presentation.interaction, ActionInteraction::Form);
        assert_eq!(
            edit_presentation
                .confirmation
                .as_ref()
                .map(|confirmation| confirmation.title.as_str()),
            Some("确认修改")
        );
        assert_eq!(
            edit_presentation
                .availability
                .as_ref()
                .map(|hint| (hint.state, hint.reason.as_str())),
            Some((AvailabilityState::Disabled, "当前记录可能不允许修改"))
        );
        let admin_note = admin.table_views[0]
            .form
            .fields
            .iter()
            .find(|field| field.field == "admin_note")
            .expect("管理员表单应包含 admin_note");
        assert!(!admin_note.read_only);
        assert!(!admin_note.write_only);

        let edit_handle = app
            .registry()
            .resolve(&action_ref("edit"))
            .expect("edit Action 应已注册");
        let response = app
            .dispatch_context(
                edit_handle,
                app.context(Request::new(json!({}))).with_user(
                    User::new(8, "admin")
                        .with_roles(["admin"])
                        .with_permissions(["module:view", "member:edit", "member:options"]),
                ),
            )
            .await
            .expect("availability disabled 不能替代服务端授权或阻断真实派发");
        assert_eq!(response.code, 0);
    }

    #[test]
    fn tree_view_contract_rejects_implicit_or_ambiguous_topology() {
        let build = |include_parent: bool, duplicate_id: bool| {
            let module_name = ModuleName::new("org.unit").expect("测试 Module 名称应有效");
            let table_name = TableName::new("org_unit").expect("测试 Table 名称应有效");
            let field_ref = |name: &str| {
                FieldRef::new(
                    table_name.clone(),
                    FieldName::new(name).expect("测试字段名应有效"),
                )
            };
            let mut view = ViewSpec::new(ViewName::new("tree").expect("测试 View 名称应有效"))
                .field(field_ref("id"))
                .field(field_ref("name"));
            if include_parent {
                view = view.field(field_ref("parent_id"));
            }
            view = view.tree(TreeViewSpec::new(
                field_ref("id"),
                if duplicate_id {
                    field_ref("id")
                } else {
                    field_ref("parent_id")
                },
                field_ref("name"),
            ));
            let module = ModuleSpec::new(module_name)
                .table(
                    TableSpec::new(table_name)
                        .field(FieldSpec::new(
                            FieldName::new("id").expect("测试字段名应有效"),
                            FieldKind::Key,
                        ))
                        .field(FieldSpec::new(
                            FieldName::new("parent_id").expect("测试字段名应有效"),
                            FieldKind::Int,
                        ))
                        .field(FieldSpec::new(
                            FieldName::new("name").expect("测试字段名应有效"),
                            FieldKind::Str,
                        )),
                )
                .view(view);
            AppBuilder::new()
                .addon(
                    AddonSpec::new(AddonName::new("org").expect("测试 Addon 名称应有效"))
                        .module(module),
                )
                .build(ToolsBuilder::new().build().expect("测试 Tools 应构建成功"))
        };

        let missing = build(false, false).expect_err("树拓扑字段必须显式包含在 View 中");
        assert!(matches!(
            missing,
            BuildError::InvalidReference {
                kind: "Tree View Field",
                ..
            }
        ));

        let ambiguous = build(true, true).expect_err("树 id/parent 字段必须不同");
        assert!(matches!(
            ambiguous,
            BuildError::InvalidReference {
                kind: "Tree View",
                ..
            }
        ));
    }

    #[test]
    fn relation_options_action_requires_relation_target() {
        let module_name = ModuleName::new("org.unit").expect("测试 Module 名称应有效");
        let table_name = TableName::new("org_unit").expect("测试 Table 名称应有效");
        let options_ref = ActionRef::new(
            module_name.clone(),
            ActionName::new("options").expect("测试 Action 名称应有效"),
        );
        let broken = FieldSpec::new(
            FieldName::new("owner_id").expect("测试字段名应有效"),
            FieldKind::Str,
        )
        .select(options_ref);
        let module = ModuleSpec::new(module_name)
            .table(
                TableSpec::new(table_name)
                    .field(FieldSpec::new(
                        FieldName::new("id").expect("测试字段名应有效"),
                        FieldKind::Key,
                    ))
                    .field(broken),
            )
            .action(action("options", "org.unit.options"), NoopAction);
        let error = AppBuilder::new()
            .addon(
                AddonSpec::new(AddonName::new("org").expect("测试 Addon 名称应有效"))
                    .module(module),
            )
            .build(ToolsBuilder::new().build().expect("测试 Tools 应构建成功"))
            .expect_err("selector Action 缺少关系目标必须在启动期失败");
        assert!(
            matches!(
                error,
                BuildError::InvalidReference {
                    kind: "Relation Options Field",
                    ..
                }
            ),
            "实际错误: {error:?}"
        );
    }

    #[test]
    fn table_view_default_sort_requires_sortable_view_field() {
        let module_name = ModuleName::new("org.unit").expect("测试 Module 名称应有效");
        let table_name = TableName::new("org_unit").expect("测试 Table 名称应有效");
        let id_ref = FieldRef::new(
            table_name.clone(),
            FieldName::new("id").expect("测试字段名应有效"),
        );
        let view = ViewSpec::new(ViewName::new("list").expect("测试 View 名称应有效"))
            .field(id_ref.clone())
            .default_sort(TableSortSpec::new(id_ref, SortDirection::Desc));
        let module = ModuleSpec::new(module_name)
            .table(TableSpec::new(table_name).field(FieldSpec::new(
                FieldName::new("id").expect("测试字段名应有效"),
                FieldKind::Key,
            )))
            .view(view);
        let error = AppBuilder::new()
            .addon(
                AddonSpec::new(AddonName::new("org").expect("测试 Addon 名称应有效"))
                    .module(module),
            )
            .build(ToolsBuilder::new().build().expect("测试 Tools 应构建成功"))
            .expect_err("不可排序字段不能成为 View 默认排序");
        assert!(matches!(
            error,
            BuildError::InvalidReference {
                kind: "View Default Sort",
                ..
            }
        ));
    }

    #[test]
    fn custom_action_presentation_rejects_paths_missing_ids_and_response_mismatch() {
        let build = |presentation: ActionPresentationSpec| {
            let module_name = ModuleName::new("dms.task").expect("测试 Module 名称应有效");
            let action_ref = ActionRef::new(
                module_name.clone(),
                ActionName::new("flow").expect("测试 Action 名称应有效"),
            );
            let module = ModuleSpec::new(module_name)
                .table(
                    TableSpec::new(TableName::new("dms_task").expect("测试 Table 名称应有效"))
                        .field(FieldSpec::new(
                            FieldName::new("id").expect("测试字段名应有效"),
                            FieldKind::Key,
                        )),
                )
                .action(action("flow", "dms.task.flow"), NoopAction)
                .view(
                    ViewSpec::new(ViewName::new("main").expect("测试 View 名称应有效"))
                        .present_action(action_ref, presentation),
                );
            AppBuilder::new()
                .addon(
                    AddonSpec::new(AddonName::new("dms").expect("测试 Addon 名称应有效"))
                        .module(module),
                )
                .build(ToolsBuilder::new().build().expect("测试 Tools 应构建成功"))
        };

        let missing_id = build(ActionPresentationSpec::new(
            ActionPlacement::Toolbar,
            ActionInteraction::Custom,
        ))
        .expect_err("custom 交互缺少 view_id 必须在启动期失败");
        assert!(matches!(
            missing_id,
            BuildError::InvalidReference {
                kind: "Action Presentation",
                ..
            }
        ));

        let physical_path = build(
            ActionPresentationSpec::new(ActionPlacement::Toolbar, ActionInteraction::Custom)
                .view_id("../views/TaskFlow.vue"),
        )
        .expect_err("物理路径不得作为 custom view_id");
        assert!(matches!(
            physical_path,
            BuildError::InvalidReference {
                kind: "Action Presentation",
                ..
            }
        ));

        let blank_availability = build(
            ActionPresentationSpec::new(ActionPlacement::Toolbar, ActionInteraction::Invoke)
                .availability(AvailabilityHint::hidden("   ")),
        )
        .expect_err("空白 availability reason 必须在启动期失败");
        assert!(matches!(
            blank_availability,
            BuildError::InvalidReference {
                kind: "Action Presentation",
                ..
            }
        ));

        let mismatch = build(ActionPresentationSpec::new(
            ActionPlacement::Toolbar,
            ActionInteraction::Preview,
        ))
        .expect_err("JSON Action 不得伪装成文件预览");
        assert!(matches!(
            mismatch,
            BuildError::InvalidReference {
                kind: "Action Presentation",
                ..
            }
        ));

        let app = build(
            ActionPresentationSpec::new(ActionPlacement::Toolbar, ActionInteraction::Custom)
                .view_id("dms.task.flow"),
        )
        .expect("稳定限定 view_id 应通过构建期校验");
        let catalog = app
            .ui_catalog(
                &app.context(Request::new(json!({})))
                    .with_user(User::new(9, "designer")),
            )
            .expect("custom view UI Catalog revision 应可计算");
        assert_eq!(
            catalog.table_views[0].action_presentations[0]
                .view_id
                .as_deref(),
            Some("dms.task.flow")
        );
    }

    #[test]
    fn action_confirmation_rejects_blank_or_overlong_content() {
        let build = |confirmation: ActionConfirmation| {
            let module_name = ModuleName::new("dms.task").expect("测试 Module 名称应有效");
            let action_ref = ActionRef::new(
                module_name.clone(),
                ActionName::new("flow").expect("测试 Action 名称应有效"),
            );
            let module = ModuleSpec::new(module_name)
                .table(
                    TableSpec::new(TableName::new("dms_task").expect("测试 Table 名称应有效"))
                        .field(FieldSpec::new(
                            FieldName::new("id").expect("测试字段名应有效"),
                            FieldKind::Key,
                        )),
                )
                .action(action("flow", "dms.task.flow"), NoopAction)
                .view(
                    ViewSpec::new(ViewName::new("main").expect("测试 View 名称应有效"))
                        .present_action(
                            action_ref,
                            ActionPresentationSpec::new(
                                ActionPlacement::Toolbar,
                                ActionInteraction::Invoke,
                            )
                            .confirmation(confirmation),
                        ),
                );
            AppBuilder::new()
                .addon(
                    AddonSpec::new(AddonName::new("dms").expect("测试 Addon 名称应有效"))
                        .module(module),
                )
                .build(ToolsBuilder::new().build().expect("测试 Tools 应构建成功"))
        };
        let assert_invalid = |result: Result<crate::definition::BuiltApp, BuildError>,
                              reason: &str| {
            assert!(
                matches!(
                    result,
                    Err(BuildError::InvalidReference {
                        kind: "Action Presentation",
                        ..
                    })
                ),
                "{reason}"
            );
        };

        assert_invalid(
            build(ActionConfirmation::new("   ", "将执行危险操作")),
            "空白确认标题必须在启动期失败",
        );
        assert_invalid(
            build(ActionConfirmation::new("确认删除", "")),
            "空白确认正文必须在启动期失败",
        );
        assert_invalid(
            build(ActionConfirmation::new("题".repeat(501), "将执行危险操作")),
            "超长确认标题必须在启动期失败",
        );
        assert_invalid(
            build(ActionConfirmation::new("确认删除", "文".repeat(501))),
            "超长确认正文必须在启动期失败",
        );
        build(ActionConfirmation::new(
            "确认删除",
            "将删除当前记录，不可恢复",
        ))
        .expect("合法确认文案应通过构建期校验");
    }

    #[test]
    fn form_field_validation_serializes_only_declared_constraints() {
        assert_eq!(
            UI_SCHEMA_VERSION, "2.0",
            "校验提示是 UI 契约变更，必须递增 schema 主版本"
        );

        let validation = FormFieldValidationSchema {
            min_length: Some(2),
            max_length: Some(64),
            minimum: None,
            maximum: None,
            pattern: Some("^[a-z]+$".to_string()),
        };
        let wire = serde_json::to_value(&validation).expect("校验提示应可序列化");
        assert_eq!(
            wire,
            json!({"min_length": 2, "max_length": 64, "pattern": "^[a-z]+$"}),
            "未声明的约束不得出现在线上契约"
        );

        let mut field = FormFieldSchema {
            field: "name".to_string(),
            title: "名称".to_string(),
            description: String::new(),
            widget: WidgetHint::Text,
            required: true,
            read_only: false,
            write_only: false,
            relation: None,
            validation: Some(validation),
        };
        let wire = serde_json::to_value(&field).expect("表单字段应可序列化");
        assert_eq!(wire["validation"]["min_length"], 2);
        assert!(wire["validation"].get("minimum").is_none());

        field.validation = None;
        let wire = serde_json::to_value(&field).expect("无约束表单字段应可序列化");
        assert!(
            wire.get("validation").is_none(),
            "未声明约束的字段不得携带 validation 键"
        );
    }

    #[tokio::test]
    async fn form_projection_includes_validation_hints_and_filters_by_field_permission() {
        let module_name = ModuleName::new("org.profile").expect("测试 Module 名称应有效");
        let table_name = TableName::new("org_profile").expect("测试 Table 名称应有效");
        let field_ref = |name: &str| {
            FieldRef::new(
                table_name.clone(),
                FieldName::new(name).expect("测试字段名应有效"),
            )
        };

        let mut nickname = FieldSpec::new(
            FieldName::new("nickname").expect("测试字段名应有效"),
            FieldKind::Str,
        );
        nickname.validation.min_length = Some(2);
        nickname.validation.max_length = Some(64);
        nickname.validation.pattern = Some("^[a-z]+$".to_string());
        let mut score = FieldSpec::new(
            FieldName::new("score").expect("测试字段名应有效"),
            FieldKind::Decimal,
        );
        score.validation.minimum = Some("0".to_string());
        score.validation.maximum = Some("99.99".to_string());
        score.access.readable = AccessRule::Roles(vec!["admin".to_string()]);
        score.access.writable = AccessRule::Roles(vec!["admin".to_string()]);

        let view = ViewSpec::new(ViewName::new("main").expect("测试 View 名称应有效"))
            .field(field_ref("id"))
            .field(field_ref("nickname"))
            .field(field_ref("score"))
            .field(field_ref("bio"))
            .action(ActionRef::new(
                module_name.clone(),
                ActionName::new("list").expect("测试 Action 名称应有效"),
            ));
        let module = ModuleSpec::new(module_name)
            .table(
                TableSpec::new(table_name)
                    .field(FieldSpec::new(
                        FieldName::new("id").expect("测试字段名应有效"),
                        FieldKind::Key,
                    ))
                    .field(nickname)
                    .field(score)
                    .field(FieldSpec::new(
                        FieldName::new("bio").expect("测试字段名应有效"),
                        FieldKind::Text,
                    )),
            )
            .default_permissions(["module:view"], PermissionMode::All)
            .action(action("list", "org.profile.list"), NoopAction)
            .view(view);
        let app = AppBuilder::new()
            .addon(
                AddonSpec::new(AddonName::new("org").expect("测试 Addon 名称应有效"))
                    .module(module),
            )
            .build(ToolsBuilder::new().build().expect("测试 Tools 应构建成功"))
            .expect("校验提示测试应用应构建成功");

        let member = app
            .ui_catalog(
                &app.context(Request::new(json!({})))
                    .with_user(User::new(7, "member").with_permissions(["module:view"])),
            )
            .expect("成员 UI Catalog revision 应可计算");
        let member_form = &member.table_views[0].form.fields;
        let member_field = |name: &str| {
            member_form
                .iter()
                .find(|field| field.field == name)
                .unwrap_or_else(|| panic!("成员表单应包含字段 {name}"))
        };
        let nickname = member_field("nickname")
            .validation
            .as_ref()
            .expect("成员表单应投影 nickname 的校验提示");
        assert_eq!(nickname.min_length, Some(2));
        assert_eq!(nickname.max_length, Some(64));
        assert_eq!(nickname.pattern.as_deref(), Some("^[a-z]+$"));
        assert_eq!(nickname.minimum, None);
        assert_eq!(nickname.maximum, None);
        assert!(
            member_form.iter().all(|field| field.field != "score"),
            "无字段权限时整个字段（含校验提示）都不得投影"
        );
        let member_wire =
            serde_json::to_value(&member.table_views[0].form).expect("成员表单应可序列化");
        let bio_wire = member_wire["fields"]
            .as_array()
            .expect("表单字段应序列化为数组")
            .iter()
            .find(|field| field["field"] == "bio")
            .expect("成员表单应包含 bio");
        assert!(
            bio_wire.get("validation").is_none(),
            "未声明约束的字段不得携带 validation 键: {bio_wire}"
        );

        let admin = app
            .ui_catalog(
                &app.context(Request::new(json!({}))).with_user(
                    User::new(8, "admin")
                        .with_roles(["admin"])
                        .with_permissions(["module:view"]),
                ),
            )
            .expect("管理员 UI Catalog revision 应可计算");
        let score = admin.table_views[0]
            .form
            .fields
            .iter()
            .find(|field| field.field == "score")
            .and_then(|field| field.validation.as_ref())
            .expect("管理员表单应投影 score 的校验提示");
        assert_eq!(score.minimum.as_deref(), Some("0"));
        assert_eq!(score.maximum.as_deref(), Some("99.99"));
        assert_eq!(score.min_length, None);
    }
}
