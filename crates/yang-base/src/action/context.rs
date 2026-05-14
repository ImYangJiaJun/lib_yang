//! Action 执行上下文
//!
//! 提供 Action 执行所需的上下文环境，包含请求信息、用户信息、全局工具和表配置。
//!
//! # 主要组件
//!
//! - `ActionContext`：Action 执行上下文结构
//! - `User`：用户信息（占位符，后续实现）
//! - `GlobalTools`：全局工具集合（占位符，后续实现）
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::action::{ActionContext, Request};
//! use yang_base::table::TableConfig;
//! use serde_json::json;
//! use std::sync::Arc;
//!
//! // 创建请求
//! let request = Request::new(json!({
//!     "username": "alice",
//!     "age": 30
//! }));
//!
//! // 创建上下文
//! let tools = Arc::new(GlobalTools::new());
//! let mut context = ActionContext::new(request, tools);
//!
//! // 设置表配置
//! let table_config = Arc::new(TableConfig::new("users"));
//! context = context.with_table_config(table_config);
//!
//! // 获取参数
//! let username: String = context.param("username")?;
//! let age: i64 = context.param("age")?;
//!
//! // 获取可选参数
//! let email: Option<String> = context.param_optional("email");
//!
//! // 创建表查询
//! let query = context.table_query()?;
//! ```

use crate::error::BaseError;
use crate::table::{TableConfig, TableQuery};
#[cfg(feature = "token")]
use crate::token::TokenManager;
use serde::de::DeserializeOwned;
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::Request;

/// 用户信息（占位符）
///
/// 此结构体在后续阶段实现，当前仅作为占位符使用。
///
/// # 字段
///
/// - `id`: 用户 ID
/// - `username`: 用户名
/// - `nickname`: 昵称
/// - `email`: 邮箱
/// - `roles`: 角色列表
/// - `permissions`: 权限列表
#[derive(Debug, Clone)]
pub struct User {
    /// 用户 ID
    pub id: i64,

    /// 用户名
    pub username: String,

    /// 昵称
    pub nickname: String,

    /// 邮箱
    pub email: String,

    /// 角色列表
    pub roles: Vec<String>,

    /// 权限列表
    pub permissions: Vec<String>,
}

impl User {
    /// 创建新用户
    ///
    /// # 参数
    ///
    /// - `id`: 用户 ID
    /// - `username`: 用户名
    ///
    /// # 返回
    ///
    /// - 新的 User 实例
    pub fn new(id: i64, username: impl Into<String>) -> Self {
        Self {
            id,
            username: username.into(),
            nickname: String::new(),
            email: String::new(),
            roles: Vec::new(),
            permissions: Vec::new(),
        }
    }

    /// 检查是否有指定权限
    ///
    /// # 参数
    ///
    /// - `permission`: 权限名称
    ///
    /// # 返回
    ///
    /// - `true`: 有权限
    /// - `false`: 无权限
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.contains(&permission.to_string())
    }

    /// 检查是否有指定角色
    ///
    /// # 参数
    ///
    /// - `role`: 角色名称
    ///
    /// # 返回
    ///
    /// - `true`: 有角色
    /// - `false`: 无角色
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.contains(&role.to_string())
    }

    /// 检查是否有任一角色
    ///
    /// # 参数
    ///
    /// - `roles`: 角色列表
    ///
    /// # 返回
    ///
    /// - `true`: 有任一角色
    /// - `false`: 无任何角色
    pub fn has_any_role(&self, roles: &[String]) -> bool {
        roles.iter().any(|r| self.has_role(r))
    }
}

/// 全局工具集合
///
/// 提供全局共享的工具和服务，包括 Token 管理器和自定义工具注册。
///
/// # 功能
///
/// - 提供 Token 管理器
/// - 支持自定义工具注册和获取
/// - 线程安全的工具访问
///
/// # 示例
///
/// ```rust,ignore
/// use yang_base::action::GlobalTools;
/// use yang_base::token::TokenManager;
/// use jsonwebtoken::Algorithm;
/// use std::sync::Arc;
///
/// // 创建 Token 管理器
/// let token_manager = TokenManager::new_symmetric(
///     "secret_key",
///     Algorithm::HS256,
///     "issuer".to_string(),
///     "audience".to_string(),
///     3600,
///     86400,
/// );
///
/// // 创建全局工具
/// let tools = GlobalTools::new(token_manager);
///
/// // 注册自定义工具
/// let redis_client = Arc::new("redis://localhost".to_string());
/// tools.register_tool("redis", redis_client);
///
/// // 获取工具
/// let redis: Option<Arc<String>> = tools.get_tool("redis");
/// ```
#[derive(Debug)]
pub struct GlobalTools {
    /// Token 管理器
    #[cfg(feature = "token")]
    token_manager: Arc<TokenManager>,

    /// 自定义工具注册表
    /// Key: 工具名称, Value: 工具实例
    tools: Arc<RwLock<HashMap<String, Arc<dyn Any + Send + Sync>>>>,
}

impl GlobalTools {
    /// 创建新的全局工具集合
    ///
    /// # 参数
    ///
    /// - `token_manager`: Token 管理器
    ///
    /// # 返回
    ///
    /// - 新的 GlobalTools 实例
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::GlobalTools;
    /// use yang_base::token::TokenManager;
    /// use jsonwebtoken::Algorithm;
    ///
    /// let token_manager = TokenManager::new_symmetric(
    ///     "secret_key",
    ///     Algorithm::HS256,
    ///     "issuer".to_string(),
    ///     "audience".to_string(),
    ///     3600,
    ///     86400,
    /// );
    ///
    /// let tools = GlobalTools::new(token_manager);
    /// ```
    #[cfg(feature = "token")]
    pub fn new(token_manager: TokenManager) -> Self {
        Self {
            token_manager: Arc::new(token_manager),
            tools: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 创建新的全局工具集合（无 Token 管理器）
    ///
    /// 当未启用 `token` feature 时使用此方法创建 GlobalTools。
    ///
    /// # 返回
    ///
    /// - 新的 GlobalTools 实例
    #[cfg(not(feature = "token"))]
    pub fn new() -> Self {
        Self {
            tools: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册自定义工具
    ///
    /// # 参数
    ///
    /// - `name`: 工具名称
    /// - `tool`: 工具实例
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use std::sync::Arc;
    ///
    /// let redis_client = Arc::new("redis://localhost".to_string());
    /// tools.register_tool("redis", redis_client);
    /// ```
    pub fn register_tool<T: Any + Send + Sync>(&self, name: &str, tool: Arc<T>) {
        let mut tools = self.tools.write().unwrap();
        tools.insert(name.to_string(), tool);
    }

    /// 获取已注册的工具
    ///
    /// # 参数
    ///
    /// - `name`: 工具名称
    ///
    /// # 返回
    ///
    /// - `Some(Arc<T>)`: 工具实例
    /// - `None`: 工具不存在或类型不匹配
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let redis: Option<Arc<String>> = tools.get_tool("redis");
    /// if let Some(client) = redis {
    ///     println!("Redis URL: {}", client);
    /// }
    /// ```
    pub fn get_tool<T: Any + Send + Sync>(&self, name: &str) -> Option<Arc<T>> {
        let tools = self.tools.read().unwrap();
        tools
            .get(name)
            .and_then(|tool| tool.clone().downcast::<T>().ok())
    }

    /// 获取 Token 管理器
    ///
    /// # 返回
    ///
    /// - Token 管理器引用
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let token_manager = tools.token_manager();
    /// let token = token_manager.generate_access_token(
    ///     "user_123",
    ///     serde_json::json!({"role": "admin"}),
    /// )?;
    /// ```
    #[cfg(feature = "token")]
    pub fn token_manager(&self) -> &TokenManager {
        &self.token_manager
    }
}

/// Action 执行上下文
///
/// 包含 Action 执行所需的所有信息，包括请求数据、用户信息、全局工具和表配置。
///
/// # 字段
///
/// - `request`: 请求数据
/// - `user`: 当前用户（已认证）
/// - `tools`: 全局工具
/// - `table_config`: 表配置（如果 action 关联表）
///
/// # 示例
///
/// ```rust,ignore
/// use yang_base::action::{ActionContext, Request};
/// use yang_base::table::TableConfig;
/// use serde_json::json;
/// use std::sync::Arc;
///
/// // 创建请求
/// let request = Request::new(json!({
///     "username": "alice",
///     "email": "alice@example.com"
/// }));
///
/// // 创建上下文
/// let tools = Arc::new(GlobalTools::new());
/// let context = ActionContext::new(request, tools);
///
/// // 获取参数
/// let username: String = context.param("username")?;
///
/// // 获取可选参数
/// let phone: Option<String> = context.param_optional("phone");
/// ```
#[derive(Debug)]
pub struct ActionContext {
    /// 请求数据
    pub request: Request,

    /// 当前用户（已认证）
    pub user: Option<User>,

    /// 全局工具
    pub tools: Arc<GlobalTools>,

    /// 表配置（如果 action 关联表）
    pub table_config: Option<Arc<TableConfig>>,
}

impl ActionContext {
    /// 创建新的上下文
    ///
    /// # 参数
    ///
    /// - `request`: 请求数据
    /// - `tools`: 全局工具
    ///
    /// # 返回
    ///
    /// - 新的 ActionContext 实例
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::{ActionContext, Request};
    /// use serde_json::json;
    /// use std::sync::Arc;
    ///
    /// let request = Request::new(json!({ "name": "Alice" }));
    /// let tools = Arc::new(GlobalTools::new());
    /// let context = ActionContext::new(request, tools);
    /// ```
    pub fn new(request: Request, tools: Arc<GlobalTools>) -> Self {
        Self {
            request,
            user: None,
            tools,
            table_config: None,
        }
    }

    /// 设置用户
    ///
    /// # 参数
    ///
    /// - `user`: 用户信息
    ///
    /// # 返回
    ///
    /// - 修改后的 ActionContext 实例（支持链式调用）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::{ActionContext, Request, User};
    /// use serde_json::json;
    /// use std::sync::Arc;
    ///
    /// let request = Request::new(json!({}));
    /// let tools = Arc::new(GlobalTools::new());
    /// let user = User::new(1, "alice");
    ///
    /// let context = ActionContext::new(request, tools)
    ///     .with_user(user);
    /// ```
    pub fn with_user(mut self, user: User) -> Self {
        self.user = Some(user);
        self
    }

    /// 设置表配置
    ///
    /// # 参数
    ///
    /// - `config`: 表配置
    ///
    /// # 返回
    ///
    /// - 修改后的 ActionContext 实例（支持链式调用）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::{ActionContext, Request};
    /// use yang_base::table::TableConfig;
    /// use serde_json::json;
    /// use std::sync::Arc;
    ///
    /// let request = Request::new(json!({}));
    /// let tools = Arc::new(GlobalTools::new());
    /// let table_config = Arc::new(TableConfig::new("users"));
    ///
    /// let context = ActionContext::new(request, tools)
    ///     .with_table_config(table_config);
    /// ```
    pub fn with_table_config(mut self, config: Arc<TableConfig>) -> Self {
        self.table_config = Some(config);
        self
    }

    /// 获取请求参数（必填）
    ///
    /// 从请求体中获取指定参数，如果参数不存在或类型不匹配则返回错误。
    ///
    /// # 参数
    ///
    /// - `key`: 参数名
    ///
    /// # 返回
    ///
    /// - `Ok(T)`: 参数值
    /// - `Err(BaseError::ParamMissing)`: 参数不存在
    /// - `Err(BaseError::ParamInvalid)`: 参数类型不匹配
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::{ActionContext, Request};
    /// use serde_json::json;
    /// use std::sync::Arc;
    ///
    /// let request = Request::new(json!({
    ///     "username": "alice",
    ///     "age": 30
    /// }));
    ///
    /// let tools = Arc::new(GlobalTools::new());
    /// let context = ActionContext::new(request, tools);
    ///
    /// // 获取字符串参数
    /// let username: String = context.param("username")?;
    /// assert_eq!(username, "alice");
    ///
    /// // 获取整数参数
    /// let age: i64 = context.param("age")?;
    /// assert_eq!(age, 30);
    ///
    /// // 参数不存在
    /// let result: Result<String, _> = context.param("email");
    /// assert!(result.is_err());
    /// ```
    pub fn param<T: DeserializeOwned>(&self, key: &str) -> Result<T, BaseError> {
        let value = self
            .request
            .body
            .get(key)
            .ok_or_else(|| BaseError::ParamMissing(key.to_string()))?;

        serde_json::from_value(value.clone()).map_err(|_| {
            BaseError::ParamInvalid(key.to_string(), "无法将参数转换为目标类型".to_string())
        })
    }

    /// 获取请求参数（可选）
    ///
    /// 从请求体中获取指定参数，如果参数不存在或类型不匹配则返回 None。
    ///
    /// # 参数
    ///
    /// - `key`: 参数名
    ///
    /// # 返回
    ///
    /// - `Some(T)`: 参数值
    /// - `None`: 参数不存在或类型不匹配
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::{ActionContext, Request};
    /// use serde_json::json;
    /// use std::sync::Arc;
    ///
    /// let request = Request::new(json!({
    ///     "username": "alice"
    /// }));
    ///
    /// let tools = Arc::new(GlobalTools::new());
    /// let context = ActionContext::new(request, tools);
    ///
    /// // 获取存在的参数
    /// let username: Option<String> = context.param_optional("username");
    /// assert_eq!(username, Some("alice".to_string()));
    ///
    /// // 获取不存在的参数
    /// let email: Option<String> = context.param_optional("email");
    /// assert_eq!(email, None);
    /// ```
    pub fn param_optional<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.request
            .body
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// 创建表查询构建器
    ///
    /// 基于当前上下文的表配置和用户角色创建 TableQuery 实例。
    ///
    /// # 返回
    ///
    /// - `Ok(TableQuery)`: 查询构建器
    /// - `Err(BaseError::TableConfigNotSet)`: 表配置未设置
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::{ActionContext, Request};
    /// use yang_base::table::TableConfig;
    /// use serde_json::json;
    /// use std::sync::Arc;
    ///
    /// let request = Request::new(json!({}));
    /// let tools = Arc::new(GlobalTools::new());
    /// let table_config = Arc::new(TableConfig::new("users"));
    ///
    /// let context = ActionContext::new(request, tools)
    ///     .with_table_config(table_config);
    ///
    /// // 创建查询构建器
    /// let query = context.table_query()?;
    /// ```
    pub fn table_query(&self) -> Result<TableQuery, BaseError> {
        let config = self
            .table_config
            .as_ref()
            .ok_or(BaseError::TableConfigNotSet)?;

        let user_roles = self
            .user
            .as_ref()
            .map(|u| u.roles.clone())
            .unwrap_or_default();

        Ok(TableQuery::new(config.clone(), user_roles, None))
    }

    /// 获取用户角色列表
    ///
    /// # 返回
    ///
    /// - 用户角色列表（如果用户未登录则返回空列表）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::action::{ActionContext, Request, User};
    /// use serde_json::json;
    /// use std::sync::Arc;
    ///
    /// let request = Request::new(json!({}));
    /// let tools = Arc::new(GlobalTools::new());
    ///
    /// let mut user = User::new(1, "alice");
    /// user.roles = vec!["admin".to_string(), "user".to_string()];
    ///
    /// let context = ActionContext::new(request, tools)
    ///     .with_user(user);
    ///
    /// let roles = context.user_roles();
    /// assert_eq!(roles, vec!["admin", "user"]);
    /// ```
    pub fn user_roles(&self) -> Vec<String> {
        self.user
            .as_ref()
            .map(|u| u.roles.clone())
            .unwrap_or_default()
    }
}
