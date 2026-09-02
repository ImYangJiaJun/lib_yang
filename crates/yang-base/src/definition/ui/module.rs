//! 账户身份与 Module 页面的构建期展示声明及请求级投影契约。

use super::action::{ActionPresentationSchema, ActionPresentationSpec};
use crate::definition::ActionRef;
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::BTreeMap;

/// 一个可切换账户身份的构建期展示声明。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountIdentitySpec {
    /// 跨模块稳定身份标识。
    pub id: String,
    /// 用户可见标题。
    pub title: String,
    /// 前端语义图标 token，不是组件或文件路径。
    pub icon: String,
    /// 身份切换器中的稳定顺序。
    pub order: i32,
}

impl AccountIdentitySpec {
    /// 创建账户身份展示声明。
    pub fn new(id: impl Into<String>, title: impl Into<String>, icon: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            icon: icon.into(),
            order: 0,
        }
    }

    /// 设置身份切换器中的稳定顺序。
    #[must_use]
    pub fn order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }
}

/// Module 的构建期展示声明。
///
/// 页面主 Action 与附加 Action 均使用强类型 [`ActionRef`]；前端只消费投影后的
/// operation id，不再从 Action 名称后缀猜测页面语义。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModulePresentationSpec {
    /// Module 所属账户身份。
    pub identity: AccountIdentitySpec,
    /// 用户可见标题。
    pub title: String,
    /// Module 业务说明。
    pub description: String,
    /// 前端语义图标 token。
    pub icon: String,
    /// 同一身份下的稳定顺序。
    pub order: i32,
    /// 可选页面主 Action。
    pub primary_action: Option<ActionRef>,
    /// 不属于 TableView 的页面级 Action 展示语义。
    pub action_presentations: BTreeMap<ActionRef, ActionPresentationSpec>,
}

impl ModulePresentationSpec {
    /// 创建 Module 展示声明。
    pub fn new(
        identity: AccountIdentitySpec,
        title: impl Into<String>,
        icon: impl Into<String>,
    ) -> Self {
        Self {
            identity,
            title: title.into(),
            description: String::new(),
            icon: icon.into(),
            order: 0,
            primary_action: None,
            action_presentations: BTreeMap::new(),
        }
    }

    /// 设置 Module 业务说明。
    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// 设置同一身份下的稳定顺序。
    #[must_use]
    pub fn order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    /// 设置页面主 Action。
    #[must_use]
    pub fn primary_action(mut self, action: ActionRef) -> Self {
        self.primary_action = Some(action);
        self
    }

    /// 声明一个页面级 Action 的展示语义。
    #[must_use]
    pub fn present_action(
        mut self,
        action: ActionRef,
        presentation: ActionPresentationSpec,
    ) -> Self {
        self.action_presentations.insert(action, presentation);
        self
    }
}

/// 请求级账户身份展示契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct AccountIdentitySchema {
    /// 跨模块稳定身份标识。
    pub id: String,
    /// 用户可见标题。
    pub title: String,
    /// 前端语义图标 token。
    pub icon: String,
    /// 身份切换器中的稳定顺序。
    pub order: i32,
}

/// 请求级 Module 页面展示契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ModulePresentationSchema {
    /// 全限定 Module ID。
    pub module_id: String,
    /// Module 所属账户身份。
    pub identity: AccountIdentitySchema,
    /// 用户可见标题。
    pub title: String,
    /// Module 业务说明。
    pub description: String,
    /// 前端语义图标 token。
    pub icon: String,
    /// 同一身份下的稳定顺序。
    pub order: i32,
    /// 当前请求有权访问的页面主 Action。
    pub primary_action: Option<String>,
    /// 当前请求有权访问的页面级 Actions。
    pub actions: Vec<String>,
    /// 页面级 Action 的显式展示语义。
    pub action_presentations: Vec<ActionPresentationSchema>,
    /// 当前请求有权访问且归属此 Module 的 TableView IDs。
    pub views: Vec<String>,
}
