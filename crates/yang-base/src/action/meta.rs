//! Action 运行时元信息聚合
//!
//! 由 `#[derive(Action)]` 在 `meta_static()` 里通过 `OnceLock` 一次性构造，
//! 派发时不需要遍历 trait method 取元信息。

use crate::action::action_trait::Permission;

/// 单个 Action 的静态元信息聚合体。
pub struct ActionMeta {
    /// Action 唯一标识，路由 dispatch 时用
    pub name: &'static str,
    /// 用户可见的显示名
    pub display_name: &'static str,
    /// 简介
    pub description: &'static str,
    /// 所需权限列表（dispatch 时检查）
    pub permissions: &'static [Permission],
    /// 是否公开（true 则跳过权限/登录检查）
    pub is_public: bool,
    /// 入参 JSON Schema（OnceLock 生成）
    pub input_schema: &'static schemars::schema::RootSchema,
    /// 出参 JSON Schema
    pub output_schema: &'static schemars::schema::RootSchema,
}
