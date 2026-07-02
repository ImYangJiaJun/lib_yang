//! Action 权限类型定义
//!
//! 本模块保留 [`Permission`] 权限类型。Action 行为契约已迁移到类型化的
//! [`TypedHandler`](crate::action::TypedHandler) / [`TypedAction`](crate::action::TypedAction) /
//! [`DynAction`](crate::action::DynAction) 三层 trait（见 `action/typed.rs`），
//! 旧的对象安全 `Action` trait 已随 H-1 类型化重构移除。
//!
//! # 主要组件
//!
//! - `Permission`：权限类型，表示 action 所需的权限
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::action::Permission;
//!
//! let permission = Permission::from_static("user:create");
//! assert_eq!(permission.name(), "user:create");
//! ```

use std::borrow::Cow;

/// 权限匹配模式
///
/// 决定多个权限之间的逻辑关系：
/// - `All`（AND）：用户必须同时拥有全部权限（默认行为，向后兼容）
/// - `Any`（OR）：用户只需拥有其中任一权限
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PermissionMode {
    /// AND 语义：用户必须拥有全部权限（默认）
    #[default]
    All,
    /// OR 语义：用户只需拥有其中任一权限
    Any,
}

/// 权限类型
///
/// 表示 action 所需的权限，用于权限检查。
///
/// # 字段
///
/// - `name`: 权限名称（如 "user:create", "order:read"）
///
/// # 示例
///
/// ```rust,ignore
/// use yang_base::action::Permission;
///
/// // 创建权限
/// let permission = Permission::new("user:create");
/// assert_eq!(permission.name(), "user:create");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Permission {
    /// 权限名称，使用 Cow 支持零拷贝静态字符串和动态字符串
    name: Cow<'static, str>,
}

impl Permission {
    /// 创建新权限
    ///
    /// # 参数
    ///
    /// - `name`: 权限名称
    ///
    /// # 返回
    ///
    /// - 新的 Permission 实例
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::Permission;
    ///
    /// let permission = Permission::new("user:create");
    /// ```
    pub fn new(name: impl Into<String>) -> Self {
        // 动态字符串存储为 Cow::Owned（堆分配）
        Self {
            name: Cow::Owned(name.into()),
        }
    }

    /// 从静态字符串创建权限（零拷贝，无堆分配）
    ///
    /// # 参数
    ///
    /// - `name`: 静态字符串字面量
    ///
    /// # 返回
    ///
    /// - 新的 Permission 实例（内部使用 Cow::Borrowed，无堆分配）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::Permission;
    ///
    /// let permission = Permission::from_static("user:create");
    /// assert_eq!(permission.name(), "user:create");
    /// ```
    pub fn from_static(name: &'static str) -> Self {
        // 静态字符串存储为 Cow::Borrowed（零拷贝）
        Self {
            name: Cow::Borrowed(name),
        }
    }

    /// 获取权限名称
    ///
    /// # 返回
    ///
    /// - 权限名称字符串引用
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::Permission;
    ///
    /// let permission = Permission::new("user:create");
    /// assert_eq!(permission.name(), "user:create");
    /// ```
    pub fn name(&self) -> &str {
        // Cow<'static, str> 自动解引用为 &str
        &self.name
    }
}

impl std::fmt::Display for Permission {
    /// 输出权限名称，等同于 [`Permission::name()`]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

impl From<&'static str> for Permission {
    /// 从静态字符串创建权限（零拷贝，等同于 [`Permission::from_static`]）
    fn from(value: &'static str) -> Self {
        Self::from_static(value)
    }
}

impl From<String> for Permission {
    /// 从动态字符串创建权限（等同于 [`Permission::new`]）
    fn from(value: String) -> Self {
        Self::new(value)
    }
}
