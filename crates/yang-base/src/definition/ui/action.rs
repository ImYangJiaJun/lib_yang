//! Action 展示语义：二次确认、可用性提示与构建期/请求级展示契约。

use super::hints::{ActionInteraction, ActionPlacement, AvailabilityState};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
    /// 行上下文记录标识应写入的 Action 参数。
    pub record_parameter: Option<String>,
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
            record_parameter: None,
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

    /// 设置行上下文记录标识对应的 Action 参数。
    #[must_use]
    pub fn record_parameter(mut self, parameter: impl Into<String>) -> Self {
        self.record_parameter = Some(parameter.into());
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
    /// 行上下文记录标识应写入的 Action 参数；仅 row 位置可用。
    pub record_parameter: Option<String>,
}
