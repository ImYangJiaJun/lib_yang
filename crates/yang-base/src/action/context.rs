//! Action 执行上下文
//!
//! 提供 Action 执行所需的上下文环境，包含请求信息、用户信息、全局工具和表配置。
//!
//! # 主要组件
//!
//! - `ActionContext`：Action 执行上下文结构
//! - `User`：用户信息（占位符，后续实现）
//! - `GlobalTools`：全局工具集合（占位符，后续实现）

use crate::error::BaseError;
use crate::table::{TableConfig, TableQuery};
#[cfg(feature = "token")]
use crate::token::TokenManager;
use serde::de::DeserializeOwned;
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use super::Request;
use super::RequestId;

/// 全局 GlobalTools 单例
/// 使用 OnceLock 保证线程安全的一次性初始化
/// 内部存储 Arc<GlobalTools> 以支持废弃克隆
/// 注意：返回的是静态引用，不需要克隆
static GLOBAL_TOOLS: OnceLock<Arc<GlobalTools>> = OnceLock::new();

/// 用户信息
///
/// 标注 `#[non_exhaustive]`：未来新增字段不构成破坏性变更。
/// 请使用 [`User::new`] 构造，再通过字段赋值设置可选属性。
#[derive(Debug, Clone)]
#[non_exhaustive]
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
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.iter().any(|p| p == permission)
    }

    /// 检查是否有指定角色
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }

    /// 检查是否有任一角色
    pub fn has_any_role(&self, roles: &[String]) -> bool {
        roles.iter().any(|r| self.has_role(r))
    }
}

/// 全局工具集合
#[derive(Debug)]
pub struct GlobalTools {
    /// Token 管理器
    #[cfg(feature = "token")]
    token_manager: Arc<TokenManager>,
    /// 自定义工具注册表
    tools: Arc<RwLock<HashMap<String, Arc<dyn Any + Send + Sync>>>>,
}

impl GlobalTools {
    /// 创建新的全局工具集合（启用 token feature）
    #[cfg(feature = "token")]
    pub fn new(token_manager: TokenManager) -> Self {
        Self {
            token_manager: Arc::new(token_manager),
            tools: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 创建新的全局工具集合（未启用 token feature）
    #[cfg(not(feature = "token"))]
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            tools: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册自定义工具
    pub fn register_tool<T: Any + Send + Sync>(&self, name: &str, tool: Arc<T>) {
        // 使用 unwrap_or_else 处理锁中毒：即使锁中毒也能恢复数据并继续注册
        let mut tools = self.tools.write().unwrap_or_else(|p| p.into_inner());
        tools.insert(name.to_string(), tool);
    }

    /// 获取已注册的工具
    pub fn get_tool<T: Any + Send + Sync>(&self, name: &str) -> Option<Arc<T>> {
        // 使用 unwrap_or_else 处理锁中毒：即使锁中毒也能恢复数据并继续读取
        let tools = self.tools.read().unwrap_or_else(|p| p.into_inner());
        tools
            .get(name)
            .and_then(|tool| tool.clone().downcast::<T>().ok())
    }

    /// 获取 Token 管理器
    #[cfg(feature = "token")]
    pub fn token_manager(&self) -> &TokenManager {
        &self.token_manager
    }

    /// 初始化全局 GlobalTools 单例（启用 token feature）
    ///
    /// 使用 `OnceLock` 保证只能初始化一次，线程安全。
    ///
    /// # 参数
    ///
    /// - `token_manager`: JWT 令牌管理器
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 初始化成功
    /// - `Err(BaseError::ConfigError)`: 已经初始化过，重复调用
    ///
    /// # 错误
    ///
    /// - `BaseError::ConfigError("GlobalTools 已初始化")`: 重复初始化时返回
    #[cfg(feature = "token")]
    pub fn init(token_manager: TokenManager) -> Result<(), BaseError> {
        // 尝试设置全局单例，若已初始化则返回错误
        GLOBAL_TOOLS
            .set(Arc::new(GlobalTools::new(token_manager)))
            .map_err(|_| BaseError::ConfigError("GlobalTools 已初始化".to_string()))
    }

    /// 初始化全局 GlobalTools 单例（未启用 token feature）
    ///
    /// 使用 `OnceLock` 保证只能初始化一次，线程安全。
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 初始化成功
    /// - `Err(BaseError::ConfigError)`: 已经初始化过，重复调用
    ///
    /// # 错误
    ///
    /// - `BaseError::ConfigError("GlobalTools 已初始化")`: 重复初始化时返回
    #[cfg(not(feature = "token"))]
    pub fn init() -> Result<(), BaseError> {
        // 尝试设置全局单例，若已初始化则返回错误
        GLOBAL_TOOLS
            .set(Arc::new(GlobalTools::new()))
            .map_err(|_| BaseError::ConfigError("GlobalTools 已初始化".to_string()))
    }

    /// 获取全局 GlobalTools 单例引用
    ///
    /// # 返回
    ///
    /// - `Ok(&'static GlobalTools)`: 全局实例的静态引用
    /// - `Err(BaseError::ConfigError)`: 尚未初始化
    ///
    /// # 错误
    ///
    /// - `BaseError::ConfigError("GlobalTools 未初始化")`: 在调用 `init` 之前调用此方法时返回
    pub fn get() -> Result<&'static GlobalTools, BaseError> {
        // 获取全局单例，若未初始化则返回错误
        GLOBAL_TOOLS
            .get()
            .map(|arc| arc.as_ref())
            .ok_or_else(|| BaseError::ConfigError("GlobalTools 未初始化".to_string()))
    }

    /// 获取全局 GlobalTools 的 Arc 引用（内部使用）
    ///
    /// 返回 Arc 克隆，允许将全局单例嵌入到 ActionContext 中
    pub(crate) fn get_arc() -> Result<Arc<GlobalTools>, BaseError> {
        // 获取全局单例的 Arc 克隆（廉价操作）
        GLOBAL_TOOLS
            .get()
            .cloned()
            .ok_or_else(|| BaseError::ConfigError("GlobalTools 未初始化".to_string()))
    }
}

/// Action 执行上下文
///
/// 包含 Action 执行所需的所有信息，包括请求数据、用户信息、全局工具和表配置。
///
/// 标注 `#[non_exhaustive]`：未来新增运行期字段（如 trace_id）不构成破坏性变更，
/// 调用方请用 [`ActionContext::new`] / [`ActionContext::new_with_global_tools`]
/// 构造，而非结构体字面量。
#[derive(Debug)]
#[non_exhaustive]
pub struct ActionContext {
    /// 请求数据
    pub request: Request,
    /// 当前用户（已认证）
    // SAFETY: 此字段为 pub(crate)，仅 crate 内中间件（如 TokenAuthMiddleware）可注入。
    // 外部 crate 无法直接设置此字段，防止绕过认证。
    pub(crate) user: Option<User>,
    /// 全局工具
    pub tools: Arc<GlobalTools>,
    /// 表配置（如果 action 关联表）
    pub table_config: Option<Arc<TableConfig>>,
    /// 本次派发的运行期标识，用于串联日志/span/metrics/审计。
    ///
    /// 由 `new`/`new_with_global_tools` 默认生成；`RequestIdMiddleware` 在洋葱链
    /// 最外层会按上游 `X-Request-Id` 透传或重新生成并 `span.record`。
    pub request_id: RequestId,
    /// 本次派发所属模块名（由 `ModuleRouter::dispatch` 注入）。
    ///
    /// 仅用于可观测性标注（metrics `module` 标签、日志）。模块数量有界，作为 metrics
    /// 标签不构成高基数问题。未经路由直接构造的上下文为 `None`。
    pub module: Option<String>,
    /// PERF-13: 缓存用户角色的 Arc 副本，避免 table_query() 每次重新 Arc 化。
    /// 在 `with_user()` 时一次性构建，后续 `table_query()` 仅需 `Arc::clone`（O(1)）。
    #[allow(dead_code)]
    cached_roles: Arc<[String]>,
}

impl ActionContext {
    /// 创建新的上下文
    pub fn new(request: Request, tools: Arc<GlobalTools>) -> Self {
        Self {
            request,
            user: None,
            tools,
            table_config: None,
            request_id: RequestId::generate(),
            module: None,
            cached_roles: Arc::from(Vec::new()),
        }
    }

    /// 使用全局单例创建新的上下文
    ///
    /// 自动从全局单例获取 `GlobalTools`，无需手动传入。
    ///
    /// # 参数
    ///
    /// - `request`: 请求数据
    ///
    /// # 返回
    ///
    /// - `Ok(ActionContext)`: 创建成功
    /// - `Err(BaseError::ConfigError)`: 全局单例未初始化
    ///
    /// # 错误
    ///
    /// - `BaseError::ConfigError("GlobalTools 未初始化")`: 在调用 `GlobalTools::init` 之前调用此方法时返回
    pub fn new_with_global_tools(request: Request) -> Result<Self, BaseError> {
        // 从全局单例获取 GlobalTools 的 Arc 克隆（廉价操作）
        let tools = GlobalTools::get_arc()?;
        Ok(Self {
            request,
            user: None,
            tools,
            table_config: None,
            request_id: RequestId::generate(),
            module: None,
            cached_roles: Arc::from(Vec::new()),
        })
    }

    /// 设置用户（链式调用）
    ///
    /// 仅供 crate 内中间件/内部使用。调用方有责任确保 user 已经过认证（如 TokenAuthMiddleware），
    /// 直接注入未验证的 User 将绕过所有鉴权。
    #[allow(dead_code)]
    pub fn with_user(mut self, user: User) -> Self {
        self.user = Some(user);
        self
    }

    /// 设置本次派发的 request_id（链式调用）
    ///
    /// 通常由 `RequestIdMiddleware` 在洋葱链最外层调用以透传上游标识；
    /// 业务侧一般无需手动设置。
    pub fn with_request_id(mut self, request_id: RequestId) -> Self {
        self.request_id = request_id;
        self
    }

    /// 设置所属模块名（链式调用）。
    ///
    /// 由 `ModuleRouter::dispatch` 注入，用于 metrics `module` 标签等可观测性标注。
    pub fn with_module(mut self, module: impl Into<String>) -> Self {
        self.module = Some(module.into());
        self
    }

    /// 设置表配置（链式调用）
    pub fn with_table_config(mut self, config: Arc<TableConfig>) -> Self {
        self.table_config = Some(config);
        self
    }

    /// 把整个请求体反序列化为 `I`。新类型化 Action 系统的统一参数提取入口。
    ///
    /// # 错误
    ///
    /// - `BaseError::ParamInvalid("body", ...)`: 反序列化失败（缺字段/类型错/未知字段等）
    pub fn extract_input<I: DeserializeOwned>(&self) -> Result<I, BaseError> {
        serde_json::from_value(self.request.body.clone())
            .map_err(|e| BaseError::ParamInvalid("body".to_string(), e.to_string()))
    }

    /// 零拷贝版本：用 `std::mem::take` 替代 clone 请求体。
    ///
    /// 调用后 `self.request.body` 变为 `Value::Null`，仅在不再需要 body 时使用。
    /// 仅限 crate 内部使用（`DynAction::dispatch` blanket impl 中 ctx 仅需传给 handle）。
    pub(crate) fn extract_input_owned<I: DeserializeOwned>(&mut self) -> Result<I, BaseError> {
        let body = std::mem::take(&mut self.request.body);
        serde_json::from_value(body)
            .map_err(|e| BaseError::ParamInvalid("body".to_string(), e.to_string()))
    }

    /// 获取请求体参数（可选，严格模式）
    ///
    /// 从请求体中获取指定参数：
    /// - 参数不存在：返回 `Ok(None)`
    /// - 参数存在且类型匹配：返回 `Ok(Some(T))`
    /// - 参数存在但类型不匹配：返回 `Err(BaseError::ParamInvalid)`
    ///
    /// > **H-1 迁移说明**：此方法是 H-1 重构期间保留的旧式严格可选提取变体。
    /// > 新代码请使用 [`ActionContext::extract_input`]，它通过强类型输入结构体统一处理所有参数提取。
    /// > 本方法将在 Task 6/7 完成后移除。
    ///
    /// # 参数
    ///
    /// - `key`: 参数名
    ///
    /// # 返回
    ///
    /// - `Ok(Some(T))`: 参数存在且类型匹配
    /// - `Ok(None)`: 参数不存在
    /// - `Err(BaseError::ParamInvalid)`: 参数存在但类型不匹配
    #[deprecated(note = "H-1 重构期间停用，将在 Task 6/7 后移除；请改用 extract_input")]
    #[allow(dead_code)]
    pub(crate) fn param_optional_strict<T: DeserializeOwned>(
        &self,
        key: &str,
    ) -> Result<Option<T>, BaseError> {
        match self.request.body.get(key) {
            // 参数不存在，返回 None
            None => Ok(None),
            // 参数存在，尝试反序列化
            Some(value) => serde_json::from_value::<T>(value.clone())
                .map(Some)
                .map_err(|_| {
                    BaseError::ParamInvalid(
                        key.to_string(),
                        format!("参数 '{}' 存在但无法转换为目标类型", key),
                    )
                }),
        }
    }

    /// 获取路径参数（必填）
    ///
    /// 从 `request.path_params` 中获取指定路径参数，并尝试反序列化为目标类型。
    ///
    /// # 参数
    ///
    /// - `key`: 路径参数名
    ///
    /// # 返回
    ///
    /// - `Ok(T)`: 参数值
    /// - `Err(BaseError::ParamMissing)`: 路径参数不存在
    /// - `Err(BaseError::ParamInvalid)`: 参数值无法转换为目标类型
    pub fn path_param<T: DeserializeOwned>(&self, key: &str) -> Result<T, BaseError> {
        // 从路径参数中获取字符串值
        let raw = self
            .request
            .path_params
            .get(key)
            .ok_or_else(|| BaseError::ParamMissing(key.to_string()))?;

        // 先尝试将原始字符串直接反序列化为目标类型（支持数字、布尔等 JSON 字面量）。
        // 若失败，则包裹为 JSON 字符串再尝试（数字形式的值如 "123" 在 String 目标下仍应可用）。
        serde_json::from_str::<T>(raw)
            .or_else(|_| serde_json::from_value(serde_json::Value::String(raw.clone())))
            .map_err(|_| {
                BaseError::ParamInvalid(
                    key.to_string(),
                    format!("路径参数 '{}' 无法转换为目标类型，原始值: {}", key, raw),
                )
            })
    }

    /// 创建表查询构建器
    ///
    /// 基于当前上下文的表配置和用户角色创建 TableQuery 实例。启用 `mysql`
    /// feature 时，自动从 [`GlobalDatabase`](crate::database::GlobalDatabase)
    /// 注入共享连接池，使内置 CRUD Action 能直接执行数据库操作；若全局数据库
    /// 尚未初始化，则连接池为 `None`，相关执行方法会在调用时返回
    /// `BaseError::DatabaseNotInitialized`。
    ///
    /// # 返回
    ///
    /// - `Ok(TableQuery)`: 查询构建器
    /// - `Err(BaseError::TableConfigNotSet)`: 表配置未设置
    pub fn table_query(&self) -> Result<TableQuery, BaseError> {
        let config = self
            .table_config
            .as_ref()
            .ok_or(BaseError::TableConfigNotSet)?;

        // 通过 user_roles_slice 获取借用，再转换为 Arc<[String]>
        let user_roles: Arc<[String]> = Arc::from(self.user_roles_slice().to_vec());

        // 启用 mysql feature 时，从全局数据库注入连接池（未初始化则为 None）。
        #[cfg(feature = "mysql")]
        let pool = crate::database::GlobalDatabase::get()
            .ok()
            .map(|db| Arc::new(db.pool().clone()));
        #[cfg(not(feature = "mysql"))]
        let pool = None;

        // 注入可观测性：慢查询阈值（全局配置）+ 本次派发 request_id，
        // 使受保护层执行边界能在超阈值时 warn 并串联 request_id。
        let slow_threshold = crate::observability::ObservabilityConfig::get().slow_query_threshold;
        Ok(TableQuery::new(config.clone(), user_roles, pool)
            .with_slow_threshold(slow_threshold)
            .with_request_id(self.request_id))
    }

    /// 开启一个数据库事务（受保护层多步写的原子作用域）
    ///
    /// 返回 [`yang_db::Transaction`]，可传入 `TableQuery` 的 `*_in_tx` 系列方法
    /// （`insert_in_tx`/`update_in_tx`/`delete_in_tx`/`select_in_tx` 等），使受
    /// 权限/校验/软删保护的多步写在同一事务内原子提交或整体回滚。调用方负责
    /// 显式 `commit()`；若 `Transaction` 在未提交时被 drop，sqlx 会尽力回滚。
    ///
    /// 仅在启用 `mysql` feature 时可用。
    ///
    /// # 返回
    ///
    /// - `Ok(Transaction)`：活动事务
    /// - `Err(BaseError::DatabaseNotInitialized)`：全局数据库未初始化
    /// - `Err(BaseError::DatabaseTransactionFailed)`：开启事务失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let mut tx = ctx.begin_transaction().await?;
    /// let id = ctx.table_query()?.insert_returning_id_in_tx(&mut tx, parent).await?.1;
    /// ctx.table_query()?.insert_in_tx(&mut tx, child_with(id)).await?;
    /// tx.commit().await?;
    /// ```
    #[cfg(feature = "mysql")]
    pub async fn begin_transaction(&self) -> Result<yang_db::Transaction, BaseError> {
        crate::database::GlobalDatabase::transaction().await
    }

    /// 获取用户角色列表（克隆）
    ///
    /// # 返回
    ///
    /// - 用户角色列表（如果用户未登录则返回空列表）
    pub fn user_roles(&self) -> Vec<String> {
        self.user
            .as_ref()
            .map(|u| u.roles.clone())
            .unwrap_or_default()
    }

    /// 获取用户角色列表（借用切片，避免克隆）
    ///
    /// 返回用户角色的借用切片，避免不必要的内存分配。
    /// 如果用户未登录则返回空切片。
    ///
    /// # 返回
    ///
    /// - `&[String]`: 用户角色切片引用
    pub fn user_roles_slice(&self) -> &[String] {
        self.user
            .as_ref()
            .map(|u| u.roles.as_slice())
            .unwrap_or(&[])
    }
}
