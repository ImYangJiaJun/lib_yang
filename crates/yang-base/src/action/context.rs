//! Action 执行上下文
//!
//! 提供 Action 执行所需的上下文环境，包含请求信息、用户信息、应用资源和表配置。
//!
//! # 主要组件
//!
//! - `ActionContext`：Action 执行上下文结构
//! - `User`：用户信息（占位符，后续实现）
//! - [`Tools`](crate::tools::Tools)：由当前应用实例显式拥有的冻结资源

use crate::definition::{Plugins, Registry};
use crate::error::BaseError;
use crate::table::{TableDefinition, TableQuery, Tables};
use crate::tools::Tools;
use serde::de::DeserializeOwned;
use std::collections::HashSet;
use std::sync::Arc;

use super::Request;
use super::RequestId;
use super::RequestMeta;
use super::{ActorContext, RequestContext, SystemTenantCapability, TenantContext};

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
    /// 角色集合（HashSet 保证 O(1) 查找）
    pub roles: HashSet<String>,
    /// 权限集合（HashSet 保证 O(1) 查找）
    pub permissions: HashSet<String>,
}

impl User {
    /// 创建新用户（空角色/权限）
    pub fn new(id: i64, username: impl Into<String>) -> Self {
        Self {
            id,
            username: username.into(),
            nickname: String::new(),
            email: String::new(),
            roles: HashSet::new(),
            permissions: HashSet::new(),
        }
    }

    /// 设置角色集合（替换现有角色）
    pub fn with_roles(mut self, roles: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.roles = roles.into_iter().map(Into::into).collect();
        self
    }

    /// 设置权限集合（替换现有权限）
    pub fn with_permissions(
        mut self,
        permissions: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.permissions = permissions.into_iter().map(Into::into).collect();
        self
    }

    /// 检查是否有指定权限（O(1) HashSet 查找）
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.contains(permission)
    }

    /// 检查是否有指定角色（O(1) HashSet 查找）
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.contains(role)
    }

    /// 检查是否有任一角色（O(1) 每角色）
    pub fn has_any_role(&self, roles: &[String]) -> bool {
        roles.iter().any(|r| self.has_role(r))
    }
}

/// 当前请求的互斥租户访问状态。
///
/// 单一代数和类型确保普通租户与系统 capability 不会同时存在，也避免用
/// `Option<TenantContext>` 表达系统绕过。
#[derive(Debug, Clone, Copy, Default)]
enum TenantAccess {
    #[default]
    Missing,
    Tenant(TenantContext),
    System(SystemTenantCapability),
}

/// Action 执行上下文
///
/// 包含 Action 执行所需的所有信息，包括请求数据、传输元数据、用户信息、应用资源和表配置。
///
/// 标注 `#[non_exhaustive]`：未来新增运行期字段（如 trace_id）不构成破坏性变更，
/// 调用方请用 [`ActionContext::new`] 构造，而非结构体字面量。
#[derive(Debug)]
#[non_exhaustive]
pub struct ActionContext {
    /// 请求数据
    pub request: Request,
    /// 与具体 Web 框架无关的传输元数据。
    pub request_meta: RequestMeta,
    /// 当前用户（已认证）
    // SAFETY: 此字段为 pub(crate)，仅 crate 内中间件（如 TokenAuthMiddleware）可注入。
    // 外部 crate 无法直接设置此字段，防止绕过认证。
    pub(crate) user: Option<User>,
    /// 当前应用实例拥有的冻结资源。
    tools: Arc<Tools>,
    /// 当前 App 的冻结 Action Registry；仅 BuiltApp 创建的上下文具备。
    registry: Option<Arc<Registry>>,
    /// 不可变表定义（如果 Action 关联表）。
    table_definition: Option<TableDefinition>,
    /// 本次派发的运行期标识，用于串联日志/span/metrics/审计。
    ///
    /// 由 `new` 默认生成；`RequestIdMiddleware` 在洋葱链
    /// 最外层会按上游 `X-Request-Id` 透传或重新生成并 `span.record`。
    pub request_id: RequestId,
    /// 本次派发所属模块名（由 `Registry::dispatch` 注入）。
    ///
    /// 仅用于可观测性标注（metrics `module` 标签、日志）。模块数量有界，作为 metrics
    /// 标签不构成高基数问题。未经路由直接构造的上下文为 `None`。
    pub module: Option<String>,
    /// 本次派发的 Action 名（由 `Registry::dispatch` 可信注入）。
    ///
    /// 与 `module` 共同标识当前实际执行目标；外部构造的上下文不能伪造此字段。
    action: Option<String>,
    /// PERF-13: 缓存用户角色的 Arc 副本，避免 table_query() 每次重新 Arc 化。
    /// 在 `with_user()` 时一次性构建，后续 `table_query()` 仅需 `Arc::clone`（O(1)）。
    cached_roles: Arc<[String]>,
    /// 当前请求独占的类型化扩展上下文。
    request_context: RequestContext,
    /// 普通租户、系统 capability 或缺失三者互斥的访问状态。
    tenant_access: TenantAccess,
}

impl ActionContext {
    /// 创建新的上下文
    pub fn new(request: Request, tools: Arc<Tools>) -> Self {
        Self {
            request,
            request_meta: RequestMeta::default(),
            user: None,
            tools,
            registry: None,
            table_definition: None,
            request_id: RequestId::generate(),
            module: None,
            action: None,
            cached_roles: Arc::from(Vec::new()),
            request_context: RequestContext::default(),
            tenant_access: TenantAccess::Missing,
        }
    }

    /// 返回当前应用实例的冻结资源。
    pub fn tools(&self) -> &Tools {
        &self.tools
    }

    /// 获取当前应用配置的 HTTP 客户端（`http` feature）。
    ///
    /// 委托给 [`Tools::http`]：未配置时返回 `BaseError::HttpClientNotInitialized`，
    /// Tools 生命周期已结束时返回对应错误。
    #[cfg(feature = "http")]
    pub fn http(&self) -> Result<&crate::http::HttpClient, BaseError> {
        self.tools.http()
    }

    /// 从 Tools 配置槽解析慢查询阈值。
    ///
    /// 未注册 [`ObservabilityConfig`](crate::observability::ObservabilityConfig) 时返回
    /// `None`（关闭慢查询日志），与历史上全局单例未初始化的行为一致。
    fn slow_query_threshold(&self) -> Option<std::time::Duration> {
        self.tools
            .config::<crate::observability::ObservabilityConfig>()
            .ok()
            .and_then(|config| config.slow_query_threshold)
    }

    /// 返回当前请求独占的类型化上下文。
    pub fn request_context(&mut self) -> &mut RequestContext {
        &mut self.request_context
    }

    /// 注入普通租户 capability。
    pub fn with_tenant(mut self, tenant: TenantContext) -> Self {
        self.tenant_access = TenantAccess::Tenant(tenant);
        self
    }

    pub(crate) fn with_system_tenant(mut self, capability: SystemTenantCapability) -> Self {
        self.tenant_access = TenantAccess::System(capability);
        self
    }

    /// 返回不可选的普通租户 capability。
    pub fn tenant(&self) -> Result<TenantContext, BaseError> {
        match self.tenant_access {
            TenantAccess::Tenant(tenant) => Ok(tenant),
            TenantAccess::Missing | TenantAccess::System(_) => Err(BaseError::Unauthorized(
                "请求缺少普通租户 capability".to_string(),
            )),
        }
    }

    /// 返回当前请求已获授的系统级租户 capability。
    pub fn system_tenant(&self) -> Result<SystemTenantCapability, BaseError> {
        match self.tenant_access {
            TenantAccess::System(capability) => Ok(capability),
            TenantAccess::Missing | TenantAccess::Tenant(_) => Err(BaseError::PermissionDenied(
                "请求缺少系统租户 capability".to_string(),
            )),
        }
    }

    /// 返回当前操作者上下文。
    pub fn actor(&self) -> Result<ActorContext, BaseError> {
        self.user
            .as_ref()
            .map(|user| ActorContext::new(user.id))
            .ok_or_else(|| BaseError::Unauthorized("请求缺少已认证操作者".to_string()))
    }

    /// 返回请求标识。
    pub fn request_id(&self) -> RequestId {
        self.request_id
    }

    pub(crate) fn with_registry(mut self, registry: Arc<Registry>) -> Self {
        self.registry = Some(registry);
        self
    }

    /// 消费当前请求上下文并创建强类型内部 Action 调用器。
    pub fn plugins(self) -> Result<Plugins, BaseError> {
        let registry = self.registry.as_ref().cloned().ok_or_else(|| {
            BaseError::ConfigError("ActionContext 未绑定应用 Registry".to_string())
        })?;
        Ok(Plugins::new(registry, self))
    }

    /// 设置用户（链式调用）
    ///
    /// 仅供 crate 内中间件/内部使用。调用方有责任确保 user 已经过认证（如 TokenAuthMiddleware），
    /// 直接注入未验证的 User 将绕过所有鉴权。
    #[allow(dead_code)]
    pub(crate) fn with_user(mut self, user: User) -> Self {
        let mut roles = user.roles.iter().cloned().collect::<Vec<_>>();
        roles.sort_unstable();
        self.cached_roles = Arc::from(roles);
        self.user = Some(user);
        self
    }

    /// 获取已认证用户的只读引用。
    ///
    /// 外部调用方只能读取认证结果，不能手动注入用户；用户注入仅由 crate 内受信任
    /// 中间件（如 `TokenAuthMiddleware`）完成。
    pub fn authenticated_user(&self) -> Option<&User> {
        self.user.as_ref()
    }

    /// 按当前请求认证身份返回可访问的版本化 UI 目录。
    ///
    /// 目录过滤与真实 Action dispatch 复用同一份构建期授权策略。
    pub fn ui_catalog(&self) -> Result<crate::definition::UiCatalog, BaseError> {
        let registry = self.registry.as_ref().ok_or_else(|| {
            BaseError::ConfigError("ActionContext 未绑定应用 Registry".to_string())
        })?;
        registry.ui_catalog(self)
    }

    /// 设置本次派发的 request_id（链式调用）
    ///
    /// 通常由 `RequestIdMiddleware` 在洋葱链最外层调用以透传上游标识；
    /// 业务侧一般无需手动设置。
    pub fn with_request_id(mut self, request_id: RequestId) -> Self {
        self.request_id = request_id;
        self
    }

    /// 注入由传输适配器构造的请求元数据（链式调用）。
    pub fn with_request_meta(mut self, request_meta: RequestMeta) -> Self {
        self.request_meta = request_meta;
        self
    }

    /// 设置所属模块名（链式调用）。
    ///
    /// 由 `Registry::dispatch` 注入，用于 metrics `module` 标签等可观测性标注。
    pub fn with_module(mut self, module: impl Into<String>) -> Self {
        self.module = Some(module.into());
        self
    }

    /// 返回当前实际派发的 `module` 与 `action`。
    ///
    /// 该身份由 `Registry` 在进入中间件链前覆盖注入，可供安全中间件绑定目标；
    /// 未经 Registry 派发而直接构造的上下文返回 `None`。
    pub fn dispatch_target(&self) -> Option<(&str, &str)> {
        Some((self.module.as_deref()?, self.action.as_deref()?))
    }

    /// 覆盖注入当前实际派发目标，防止调用方预置的可观测性字段参与安全判断。
    pub(crate) fn with_dispatch_target(
        mut self,
        module: impl Into<String>,
        action: impl Into<String>,
    ) -> Self {
        self.module = Some(module.into());
        self.action = Some(action.into());
        self
    }

    /// 设置表定义（链式调用）。
    pub fn with_table_definition(mut self, definition: TableDefinition) -> Self {
        self.table_definition = Some(definition);
        self
    }

    /// 返回当前 Action 绑定的表定义。
    pub fn table_definition(&self) -> Result<&TableDefinition, BaseError> {
        self.table_definition
            .as_ref()
            .ok_or(BaseError::TableDefinitionNotSet)
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

    /// 创建表查询构建器
    ///
    /// 基于当前上下文的表配置和用户角色创建 TableQuery 实例。启用 `mysql`
    /// feature 时，从当前上下文的 [`Tools`] 注入共享连接池，使内置 CRUD Action
    /// 能直接执行数据库操作；若应用未配置数据库，则返回
    /// `BaseError::DatabaseNotInitialized`。
    ///
    /// # 返回
    ///
    /// - `Ok(TableQuery)`: 查询构建器
    /// - `Err(BaseError::TableDefinitionNotSet)`: 表定义未设置
    fn base_table_query(&self, definition: &TableDefinition) -> Result<TableQuery, BaseError> {
        // 启用 mysql feature 时，从当前应用资源注入连接池。
        #[cfg(feature = "mysql")]
        let pool = self
            .tools
            .optional_mysql()?
            .map(|database| Arc::new(database.pool().clone()));
        #[cfg(not(feature = "mysql"))]
        let pool = None;

        // 注入可观测性：慢查询阈值（Tools 配置槽）+ 本次派发 request_id，
        // 使受保护层执行边界能在超阈值时 warn 并串联 request_id。
        let slow_threshold = self.slow_query_threshold();
        Ok(TableQuery::new(
            definition.shared_config(),
            Arc::clone(&self.cached_roles),
            pool,
        )
        .with_slow_threshold(slow_threshold)
        .with_request_id(self.request_id))
    }

    /// 创建默认失败关闭的表查询构建器。
    ///
    /// 带 tenant key 的表必须存在普通 [`TenantContext`]，并始终自动施加租户条件。
    /// 系统身份不会隐式绕过；全域访问必须显式调用
    /// [`ActionContext::system_table_query`]。
    pub fn table_query(&self) -> Result<TableQuery, BaseError> {
        let definition = self.table_definition()?;
        let mut query = self.base_table_query(definition)?;
        if let Some(field) = definition.tenant_key_field() {
            query = query.scope_tenant(field, serde_json::json!(self.tenant()?.id().get()))?;
        }
        Ok(query)
    }

    /// 使用显式、已绑定当前请求 actor 的系统 capability 创建全域表查询。
    ///
    /// 调用方必须先通过 [`ActionContext::system_tenant`] 取得 capability，再在具体
    /// repository 旁路中显式传回；普通 CRUD 只调用 [`ActionContext::table_query`]，
    /// 因而不会自动获得全租户访问。
    pub fn system_table_query(
        &self,
        capability: SystemTenantCapability,
    ) -> Result<TableQuery, BaseError> {
        if self.system_tenant()? != capability {
            return Err(BaseError::PermissionDenied(
                "系统租户 capability 与当前请求不匹配".to_string(),
            ));
        }
        let definition = self.table_definition()?;
        self.base_table_query(definition)
    }

    /// 创建 BR 心智连续的 Tables 入口。
    pub fn tables(&self) -> Result<Tables, BaseError> {
        self.table_query().map(Tables::new)
    }

    /// 使用显式系统 capability 创建全域 Tables 入口。
    pub fn system_tables(&self, capability: SystemTenantCapability) -> Result<Tables, BaseError> {
        self.system_table_query(capability).map(Tables::new)
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
    /// - `Err(BaseError::DatabaseNotInitialized)`：当前应用未配置数据库
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
        self.tools
            .mysql()?
            .transaction()
            .await
            .map_err(BaseError::DatabaseTransactionFailed)
    }

    /// 获取用户角色列表（克隆为 Vec）
    ///
    /// # 返回
    ///
    /// - 用户角色列表（如果用户未登录则返回空列表）
    pub fn user_roles(&self) -> Vec<String> {
        self.user
            .as_ref()
            .map(|u| u.roles.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// 获取用户角色集合引用
    ///
    /// 返回用户角色的 HashSet 引用，支持 O(1) 包含检查。
    /// 如果用户未登录则返回 `None`。
    ///
    /// # 返回
    ///
    /// - `Some(&HashSet<String>)`: 用户角色集合引用
    /// - `None`: 用户未登录
    pub fn user_roles_set(&self) -> Option<&HashSet<String>> {
        self.user.as_ref().map(|u| &u.roles)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observability::ObservabilityConfig;
    use crate::tools::ToolsBuilder;
    use std::time::Duration;

    fn empty_context() -> ActionContext {
        let tools = Arc::new(ToolsBuilder::new().build().expect("空 Tools 应构建成功"));
        ActionContext::new(Request::new(serde_json::json!({})), tools)
    }

    #[cfg(feature = "http")]
    #[test]
    fn http_shortcut_returns_client_when_configured() {
        let tools = Arc::new(
            ToolsBuilder::new()
                .http(crate::http::HttpClient::new(30).expect("测试 HTTP 客户端应创建成功"))
                .build()
                .expect("配置 HTTP 客户端后应构建成功"),
        );
        let context = ActionContext::new(Request::new(serde_json::json!({})), tools);

        let client = context.http().expect("已配置时 ctx.http() 应可用");
        let _builder = client.get("https://api.example.com/test");
    }

    #[cfg(feature = "http")]
    #[test]
    fn http_shortcut_errors_when_not_configured() {
        let context = empty_context();

        assert!(matches!(
            context.http(),
            Err(BaseError::HttpClientNotInitialized)
        ));
    }

    #[test]
    fn slow_query_threshold_is_none_without_registered_config() {
        let context = empty_context();

        assert_eq!(context.slow_query_threshold(), None);
    }

    #[test]
    fn slow_query_threshold_reads_from_tools_config_slot() {
        let threshold = Duration::from_millis(200);
        let tools = Arc::new(
            ToolsBuilder::new()
                .config(ObservabilityConfig::new().with_slow_query_threshold(threshold))
                .build()
                .expect("注册可观测性配置后应构建成功"),
        );
        let context = ActionContext::new(Request::new(serde_json::json!({})), tools);

        assert_eq!(context.slow_query_threshold(), Some(threshold));
    }

    #[test]
    fn authenticated_roles_are_cached_once_in_stable_order() {
        let context = empty_context()
            .with_user(User::new(1, "alice").with_roles(["operator", "admin", "operator"]));

        assert_eq!(
            context.cached_roles.as_ref(),
            ["admin".to_string(), "operator".to_string()]
        );
        assert_eq!(
            context.user_roles_set(),
            Some(&HashSet::from([
                "admin".to_string(),
                "operator".to_string()
            ]))
        );
    }

    #[test]
    fn ui_catalog_rejects_context_without_bound_registry() {
        let context = empty_context();

        assert!(matches!(
            context.ui_catalog(),
            Err(BaseError::ConfigError(message))
                if message == "ActionContext 未绑定应用 Registry"
        ));
    }
}
