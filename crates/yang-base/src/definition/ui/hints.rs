//! 可降级的前端提示枚举：控件、响应类别、展示位置、交互方式、可用状态与排序方向。

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
