//! Action 运行时元信息聚合
//!
//! 由 `#[derive(Action)]` 在 `meta_static()` 里通过 `OnceLock` 一次性构造，
//! 派发时不需要遍历 trait method 取元信息。

use crate::action::action_trait::{Permission, PermissionMode};

/// 单个 Action 的静态元信息聚合体。
///
/// 标注 `#[non_exhaustive]`：未来新增字段不构成破坏性变更。
/// 请使用 [`ActionMeta::new`] 构造。
#[non_exhaustive]
pub struct ActionMeta {
    /// Action 唯一标识，路由 dispatch 时用
    pub name: &'static str,
    /// 用户可见的显示名
    pub display_name: &'static str,
    /// 简介
    pub description: &'static str,
    /// 所需权限列表（dispatch 时检查）
    pub permissions: &'static [Permission],
    /// 权限匹配模式：All（AND）或 Any（OR）
    pub permission_mode: PermissionMode,
    /// 是否公开（true 则跳过权限/登录检查）
    pub is_public: bool,
    /// 入参 JSON Schema（OnceLock 生成）
    pub input_schema: &'static schemars::schema::RootSchema,
    /// 出参 JSON Schema
    pub output_schema: &'static schemars::schema::RootSchema,
}

impl ActionMeta {
    /// 构造 `ActionMeta`（`#[non_exhaustive]` 后的唯一公开构造入口）。
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        name: &'static str,
        display_name: &'static str,
        description: &'static str,
        permissions: &'static [Permission],
        permission_mode: PermissionMode,
        is_public: bool,
        input_schema: &'static schemars::schema::RootSchema,
        output_schema: &'static schemars::schema::RootSchema,
    ) -> Self {
        Self {
            name,
            display_name,
            description,
            permissions,
            permission_mode,
            is_public,
            input_schema,
            output_schema,
        }
    }
}
