//! Action Trait 定义
//!
//! 定义所有 action 必须实现的统一接口。
//!
//! # 主要组件
//!
//! - `Action`：Action trait，定义 action 的基本行为和元数据
//! - `Permission`：权限类型，表示 action 所需的权限
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::action::{Action, ActionContext, ApiResponse, Permission};
//! use yang_base::error::BaseError;
//! use async_trait::async_trait;
//!
//! // 定义自定义 action
//! pub struct MyAction;
//!
//! #[async_trait]
//! impl Action for MyAction {
//!     async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError> {
//!         // 获取参数
//!         let name: String = context.param("name")?;
//!         
//!         // 执行业务逻辑
//!         // ...
//!         
//!         Ok(ApiResponse::success(
//!             serde_json::json!({ "message": format!("Hello, {}!", name) }),
//!             "操作成功"
//!         ))
//!     }
//!     
//!     fn name(&self) -> &str {
//!         "my_action"
//!     }
//!     
//!     fn display_name(&self) -> &str {
//!         "我的操作"
//!     }
//!     
//!     fn description(&self) -> &str {
//!         "这是一个自定义操作示例"
//!     }
//!     
//!     fn permissions(&self) -> &[Permission] {
//!         &[Permission::new("my_action:execute")]
//!     }
//! }
//! ```

use crate::error::BaseError;
use async_trait::async_trait;
use std::borrow::Cow;

use super::{ActionContext, ApiResponse};

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
///
/// // 检查权限名称
/// if permission.name() == "user:create" {
///     println!("这是创建用户的权限");
/// }
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

/// Action 接口
///
/// 所有 action 必须实现此 trait，定义 action 的执行逻辑和元数据。
///
/// # 必须实现的方法
///
/// - `execute`: 执行 action 的业务逻辑（异步方法）
/// - `name`: 返回 action 名称
///
/// # 可选实现的方法（提供默认实现）
///
/// - `display_name`: 返回 action 显示名称（默认返回 name）
/// - `description`: 返回 action 描述（默认返回空字符串）
/// - `permissions`: 返回 action 所需权限列表（默认返回空列表）
/// - `params_schema`: 返回参数结构 Schema（默认返回 None）
/// - `is_public`: 标记是否为公开 action（默认返回 false）
///
/// # 示例
///
/// ```rust,ignore
/// use yang_base::action::{Action, ActionContext, ApiResponse, Permission};
/// use yang_base::error::BaseError;
/// use async_trait::async_trait;
/// use serde_json::json;
///
/// // 定义登录 action
/// pub struct LoginAction;
///
/// #[async_trait]
/// impl Action for LoginAction {
///     async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError> {
///         // 获取参数
///         let username: String = context.param("username")?;
///         let password: String = context.param("password")?;
///         
///         // 验证用户
///         // ...
///         
///         Ok(ApiResponse::success(
///             json!({ "token": "xxx", "user": { "id": 1, "username": username } }),
///             "登录成功"
///         ))
///     }
///     
///     fn name(&self) -> &str {
///         "login"
///     }
///     
///     fn display_name(&self) -> &str {
///         "用户登录"
///     }
///     
///     fn description(&self) -> &str {
///         "用户通过用户名和密码登录系统"
///     }
///     
///     fn is_public(&self) -> bool {
///         true // 登录不需要认证
///     }
///     
///     fn params_schema(&self) -> Option<serde_json::Value> {
///         Some(json!({
///             "type": "object",
///             "properties": {
///                 "username": { "type": "string", "description": "用户名" },
///                 "password": { "type": "string", "description": "密码" }
///             },
///             "required": ["username", "password"]
///         }))
///     }
/// }
/// ```
#[async_trait]
pub trait Action: Send + Sync {
    /// 执行 action
    ///
    /// 这是 action 的核心方法，包含具体的业务逻辑实现。
    ///
    /// # 参数
    ///
    /// - `context`: Action 执行上下文，包含请求信息、用户信息和全局工具
    ///
    /// # 返回
    ///
    /// - `Ok(ApiResponse)`: 执行成功，返回 API 响应
    /// - `Err(BaseError)`: 执行失败，返回错误信息
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::{Action, ActionContext, ApiResponse};
    /// use yang_base::error::BaseError;
    /// use async_trait::async_trait;
    /// use serde_json::json;
    ///
    /// pub struct GetUserAction;
    ///
    /// #[async_trait]
    /// impl Action for GetUserAction {
    ///     async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError> {
    ///         // 获取用户 ID
    ///         let user_id: i64 = context.param("id")?;
    ///         
    ///         // 查询用户
    ///         let user = query_user_by_id(user_id).await?;
    ///         
    ///         Ok(ApiResponse::success(user, "获取成功"))
    ///     }
    ///     
    ///     fn name(&self) -> &str {
    ///         "get_user"
    ///     }
    /// }
    /// ```
    async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError>;

    /// 获取 action 名称
    ///
    /// 返回 action 的唯一标识符，用于路由匹配。
    ///
    /// # 返回
    ///
    /// - action 名称字符串引用
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::Action;
    ///
    /// pub struct AddAction;
    ///
    /// impl Action for AddAction {
    ///     fn name(&self) -> &str {
    ///         "add"
    ///     }
    ///     
    ///     // ... 其他方法
    /// }
    /// ```
    fn name(&self) -> &str;

    /// 获取 action 显示名称
    ///
    /// 返回 action 的友好显示名称，用于 UI 展示。
    /// 默认实现返回 `name()` 的值。
    ///
    /// # 返回
    ///
    /// - action 显示名称字符串引用
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::Action;
    ///
    /// pub struct AddAction;
    ///
    /// impl Action for AddAction {
    ///     fn name(&self) -> &str {
    ///         "add"
    ///     }
    ///     
    ///     fn display_name(&self) -> &str {
    ///         "新增记录"
    ///     }
    ///     
    ///     // ... 其他方法
    /// }
    /// ```
    fn display_name(&self) -> &str {
        self.name()
    }

    /// 获取 action 描述
    ///
    /// 返回 action 的详细描述信息，用于文档生成和 UI 提示。
    /// 默认实现返回空字符串。
    ///
    /// # 返回
    ///
    /// - action 描述字符串引用
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::Action;
    ///
    /// pub struct AddAction;
    ///
    /// impl Action for AddAction {
    ///     fn name(&self) -> &str {
    ///         "add"
    ///     }
    ///     
    ///     fn description(&self) -> &str {
    ///         "向数据表中添加一条新记录"
    ///     }
    ///     
    ///     // ... 其他方法
    /// }
    /// ```
    fn description(&self) -> &str {
        ""
    }

    /// 获取权限要求
    ///
    /// 返回执行此 action 所需的权限列表。
    /// 默认实现返回空列表（不需要任何权限）。
    ///
    /// # 返回
    ///
    /// - 权限列表切片引用
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::{Action, Permission};
    ///
    /// pub struct DeleteUserAction;
    ///
    /// impl Action for DeleteUserAction {
    ///     fn name(&self) -> &str {
    ///         "delete_user"
    ///     }
    ///     
    ///     fn permissions(&self) -> &[Permission] {
    ///         &[
    ///             Permission::new("user:delete"),
    ///             Permission::new("admin:access")
    ///         ]
    ///     }
    ///     
    ///     // ... 其他方法
    /// }
    /// ```
    fn permissions(&self) -> &[Permission] {
        &[]
    }

    /// 获取参数 Schema
    ///
    /// 返回 action 参数的 JSON Schema 定义，用于参数验证和文档生成。
    /// 默认实现返回 None（不提供 Schema）。
    ///
    /// # 返回
    ///
    /// - `Some(Value)`: 参数 Schema（JSON 格式）
    /// - `None`: 不提供 Schema
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::Action;
    /// use serde_json::json;
    ///
    /// pub struct CreateUserAction;
    ///
    /// impl Action for CreateUserAction {
    ///     fn name(&self) -> &str {
    ///         "create_user"
    ///     }
    ///     
    ///     fn params_schema(&self) -> Option<serde_json::Value> {
    ///         Some(json!({
    ///             "type": "object",
    ///             "properties": {
    ///                 "username": {
    ///                     "type": "string",
    ///                     "minLength": 3,
    ///                     "maxLength": 20,
    ///                     "description": "用户名"
    ///                 },
    ///                 "email": {
    ///                     "type": "string",
    ///                     "format": "email",
    ///                     "description": "邮箱地址"
    ///                 },
    ///                 "age": {
    ///                     "type": "integer",
    ///                     "minimum": 0,
    ///                     "maximum": 150,
    ///                     "description": "年龄"
    ///                 }
    ///             },
    ///             "required": ["username", "email"]
    ///         }))
    ///     }
    ///     
    ///     // ... 其他方法
    /// }
    /// ```
    fn params_schema(&self) -> Option<serde_json::Value> {
        None
    }

    /// 是否为公开 action
    ///
    /// 标记此 action 是否为公开访问（不需要认证）。
    /// 默认实现返回 false（需要认证）。
    ///
    /// # 返回
    ///
    /// - `true`: 公开 action，不需要认证
    /// - `false`: 需要认证
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::Action;
    ///
    /// pub struct LoginAction;
    ///
    /// impl Action for LoginAction {
    ///     fn name(&self) -> &str {
    ///         "login"
    ///     }
    ///     
    ///     fn is_public(&self) -> bool {
    ///         true // 登录不需要认证
    ///     }
    ///     
    ///     // ... 其他方法
    /// }
    ///
    /// pub struct GetProfileAction;
    ///
    /// impl Action for GetProfileAction {
    ///     fn name(&self) -> &str {
    ///         "get_profile"
    ///     }
    ///     
    ///     fn is_public(&self) -> bool {
    ///         false // 获取个人信息需要认证（默认值）
    ///     }
    ///     
    ///     // ... 其他方法
    /// }
    /// ```
    fn is_public(&self) -> bool {
        false
    }
}
