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
