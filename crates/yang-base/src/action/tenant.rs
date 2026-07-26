//! 请求租户解析中间件。
//!
//! 客户端提供的租户 ID 只是不可信候选；应用实现的 [`TenantResolver`] 必须结合
//! 已认证用户、租户成员关系和业务状态完成校验，再返回可信 [`TenantResolution`]。

use super::{
    ActionContext, ActorContext, ApiResponse, SystemTenantCapability, TenantContext, TenantId, User,
};
use crate::error::BaseError;
use crate::router::{Middleware, MiddlewareRole, Next};
use async_trait::async_trait;

/// 默认租户候选请求头。
pub const TENANT_ID_HEADER: &str = "x-tenant-id";

/// 可信 resolver 的互斥解析结果。
///
/// 普通租户始终携带不可选 [`TenantContext`]；系统访问携带独立且绑定 actor 的
/// [`SystemTenantCapability`]。代数和类型消除了 `Option<TenantId> + bool` 的非法状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenantResolution {
    /// 范围化普通租户访问。
    Tenant(TenantContext),
    /// 显式系统级访问。
    System(SystemTenantCapability),
}

impl TenantResolution {
    /// 在可信 resolver 中为已认证 system 角色签发系统 capability。
    pub fn system_for(user: &User) -> Result<Self, BaseError> {
        if !user.has_role("system") {
            return Err(BaseError::PermissionDenied(
                "只有已认证 system 角色可获授系统租户 capability".to_string(),
            ));
        }
        Ok(Self::System(SystemTenantCapability::issue(
            ActorContext::new(user.id),
        )))
    }
}

impl From<TenantContext> for TenantResolution {
    fn from(value: TenantContext) -> Self {
        Self::Tenant(value)
    }
}

/// 将不可信租户候选解析为可信请求租户上下文。
///
/// `requested` 仅表示客户端想访问的租户，不能证明当前用户属于该租户。实现者必须
/// 依据已认证用户和服务端事实源校验成员关系；若允许系统级绕过，也必须在这里检查
/// 独立的系统权限后显式返回 [`TenantResolution::System`]。
#[async_trait]
pub trait TenantResolver: Send + Sync + 'static {
    /// 解析当前请求的可信租户上下文。
    ///
    /// `None` 表示客户端未指定租户；实现者可选择安全的默认租户，也可拒绝请求。
    async fn resolve(
        &self,
        context: &ActionContext,
        requested: Option<TenantId>,
    ) -> Result<TenantResolution, BaseError>;
}

/// 在 Action 派发前解析并注入可信租户上下文。
///
/// 中间件读取 [`TENANT_ID_HEADER`]，只接受正整数。它不会自行信任该值，而是把候选
/// 交给 [`TenantResolver`]；resolver 成功返回后才写入 [`ActionContext`]。
///
/// 注册顺序必须放在身份认证中间件之后，使 resolver 能读取
/// [`ActionContext::authenticated_user`]。如果 resolver 允许匿名租户，它也必须显式
/// 实现该策略。
pub struct TenantResolverMiddleware<R> {
    resolver: R,
}

impl<R> TenantResolverMiddleware<R>
where
    R: TenantResolver,
{
    /// 创建租户解析中间件。
    pub fn new(resolver: R) -> Self {
        Self { resolver }
    }
}

#[async_trait]
impl<R> Middleware for TenantResolverMiddleware<R>
where
    R: TenantResolver,
{
    fn role(&self) -> MiddlewareRole {
        MiddlewareRole::TenantResolution
    }

    async fn handle(
        &self,
        context: ActionContext,
        next: Next<'_>,
    ) -> Result<ApiResponse, BaseError> {
        let requested = requested_tenant(&context)?;
        let resolution = self.resolver.resolve(&context, requested).await?;
        let context = match resolution {
            TenantResolution::Tenant(tenant) => context.with_tenant(tenant),
            TenantResolution::System(capability) => context.with_system_tenant(capability),
        };
        next.run(context).await
    }
}

fn requested_tenant(context: &ActionContext) -> Result<Option<TenantId>, BaseError> {
    let Some(raw) = context.request.get_header(TENANT_ID_HEADER) else {
        return Ok(None);
    };
    let value = raw.parse::<i64>().map_err(|_| {
        BaseError::ParamInvalid(
            TENANT_ID_HEADER.to_string(),
            "租户 ID 必须是正整数".to_string(),
        )
    })?;
    if value <= 0 {
        return Err(BaseError::ParamInvalid(
            TENANT_ID_HEADER.to_string(),
            "租户 ID 必须是正整数".to_string(),
        ));
    }
    Ok(Some(TenantId::new(value)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{Request, TypedHandler, User};
    use crate::definition::{
        ActionName, ActionRef, ActionSpec, AddonName, AddonSpec, AppBuilder, HttpMethod,
        ModuleName, ModuleSpec, RouteSpec,
    };
    use crate::tools::ToolsBuilder;
    use async_trait::async_trait;
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;
    use yang_base_derive::Action;

    #[derive(Debug, Default, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields)]
    struct EmptyInput {}

    #[derive(Debug, Serialize, JsonSchema)]
    struct TenantProbeOutput {
        tenant_id: Option<i64>,
        system_actor_id: Option<i64>,
    }

    #[derive(Action)]
    #[action(name = "tenant_probe", display_name = "租户探针")]
    struct TenantProbe;

    #[async_trait]
    impl TypedHandler for TenantProbe {
        type Input = EmptyInput;
        type Output = TenantProbeOutput;

        async fn handle(
            &self,
            context: ActionContext,
            _input: Self::Input,
        ) -> Result<Self::Output, BaseError> {
            match context.tenant() {
                Ok(tenant) => Ok(TenantProbeOutput {
                    tenant_id: Some(tenant.id().get()),
                    system_actor_id: None,
                }),
                Err(_) => Ok(TenantProbeOutput {
                    tenant_id: None,
                    system_actor_id: Some(context.system_tenant()?.actor().user_id()),
                }),
            }
        }
    }

    struct MembershipResolver;

    #[async_trait]
    impl TenantResolver for MembershipResolver {
        async fn resolve(
            &self,
            context: &ActionContext,
            requested: Option<TenantId>,
        ) -> Result<TenantResolution, BaseError> {
            let user = context
                .authenticated_user()
                .ok_or_else(|| BaseError::Unauthorized("租户解析需要已认证用户".to_string()))?;
            if user.has_role("system") {
                return TenantResolution::system_for(user);
            }

            let tenant = requested.unwrap_or_else(|| TenantId::new(10));
            if user.id == 7 && tenant == TenantId::new(10) {
                Ok(TenantContext::new(tenant).into())
            } else {
                Err(BaseError::PermissionDenied(format!(
                    "用户无权访问租户 {}",
                    tenant.get()
                )))
            }
        }
    }

    fn test_app() -> crate::definition::BuiltApp {
        let module_name = ModuleName::new("org.tenant").expect("测试 Module 名称应有效");
        let module = ModuleSpec::new(module_name)
            .middleware(TenantResolverMiddleware::new(MembershipResolver))
            .action(
                ActionSpec::new(
                    ActionName::new("tenant_probe").expect("测试 Action 名称应有效"),
                    RouteSpec::new(
                        HttpMethod::Get,
                        "/api/v1/org/tenant/probe",
                        "org.tenant.tenant_probe",
                    ),
                ),
                TenantProbe,
            );
        let tools = Arc::new(ToolsBuilder::new().build().expect("测试 Tools 应构建成功"));
        AppBuilder::new()
            .addon(
                AddonSpec::new(AddonName::new("org").expect("测试 Addon 名称应有效"))
                    .module(module),
            )
            .build(tools)
            .expect("租户解析测试应用应构建成功")
    }

    fn probe_handle(app: &crate::definition::BuiltApp) -> crate::definition::ActionHandle {
        app.registry()
            .resolve(&ActionRef::new(
                ModuleName::new("org.tenant").expect("测试 Module 名称应有效"),
                ActionName::new("tenant_probe").expect("测试 Action 名称应有效"),
            ))
            .expect("租户探针应已注册")
    }

    fn tenant_response(response: ApiResponse) -> (Option<i64>, Option<i64>) {
        let data = response.data.expect("租户探针应返回 JSON data");
        (
            data.get("tenant_id").and_then(serde_json::Value::as_i64),
            data.get("system_actor_id")
                .and_then(serde_json::Value::as_i64),
        )
    }

    async fn dispatch_as(
        app: &crate::definition::BuiltApp,
        request: Request,
        user: Option<User>,
    ) -> Result<ApiResponse, BaseError> {
        let mut context = app.context(request);
        if let Some(user) = user {
            context = context.with_user(user);
        }
        app.dispatch_context(probe_handle(app), context).await
    }

    #[tokio::test]
    async fn resolver_accepts_authorized_candidate_and_safe_default() {
        let app = test_app();
        let user = User::new(7, "member");

        let selected = dispatch_as(
            &app,
            Request::new(serde_json::json!({})).header(TENANT_ID_HEADER, "10"),
            Some(user.clone()),
        )
        .await
        .expect("成员应能选择所属租户");
        assert_eq!(tenant_response(selected), (Some(10), None));

        let defaulted = dispatch_as(&app, Request::new(serde_json::json!({})), Some(user))
            .await
            .expect("resolver 应能安全选择服务端默认租户");
        assert_eq!(tenant_response(defaulted), (Some(10), None));
    }

    #[tokio::test]
    async fn resolver_rejects_invalid_cross_tenant_and_unauthenticated_requests() {
        let app = test_app();
        let user = User::new(7, "member");

        for invalid in ["", "system", "0", "-1"] {
            let result = dispatch_as(
                &app,
                Request::new(serde_json::json!({})).header(TENANT_ID_HEADER, invalid),
                Some(user.clone()),
            )
            .await;
            assert!(matches!(
                result,
                Err(BaseError::ParamInvalid(field, _)) if field == TENANT_ID_HEADER
            ));
        }

        let cross_tenant = dispatch_as(
            &app,
            Request::new(serde_json::json!({})).header(TENANT_ID_HEADER, "20"),
            Some(user),
        )
        .await;
        assert!(matches!(
            cross_tenant,
            Err(BaseError::PermissionDenied(message)) if message.contains("租户 20")
        ));

        let unauthenticated = dispatch_as(
            &app,
            Request::new(serde_json::json!({})).header(TENANT_ID_HEADER, "10"),
            None,
        )
        .await;
        assert!(matches!(unauthenticated, Err(BaseError::Unauthorized(_))));
    }

    #[tokio::test]
    async fn system_context_can_only_come_from_trusted_resolver() {
        let app = test_app();
        let system_user = User::new(1, "root").with_roles(["system"]);
        assert!(matches!(
            TenantResolution::system_for(&User::new(7, "member")),
            Err(BaseError::PermissionDenied(_))
        ));

        let response = dispatch_as(&app, Request::new(serde_json::json!({})), Some(system_user))
            .await
            .expect("系统角色应由可信 resolver 映射为 system 上下文");
        assert_eq!(tenant_response(response), (None, Some(1)));

        let forged = dispatch_as(
            &app,
            Request::new(serde_json::json!({})).header(TENANT_ID_HEADER, "system"),
            Some(User::new(7, "member")),
        )
        .await;
        assert!(matches!(
            forged,
            Err(BaseError::ParamInvalid(field, _)) if field == TENANT_ID_HEADER
        ));
    }

    /// I-6：真实 `TokenAuthMiddleware` → `TenantResolverMiddleware` 链路的组合测试。
    ///
    /// 不再用 `with_user` 绕过认证直接注入身份，而是让请求依次经过认证与租户
    /// 两个中间件，锁定「认证必须先于租户注册」的协作语义与 fail-closed 行为。
    /// 有效 Token 的成功注入路径依赖 Redis 撤销存储，由 `action::auth` 的
    /// `#[ignore]` Docker 测试覆盖；本模块锁定无 Docker 环境下的链路行为。
    #[cfg(feature = "token")]
    mod token_auth_chain {
        use super::*;
        use crate::action::TokenAuthMiddleware;

        /// 无撤销存储（未配置 cache）的测试 Tools。
        fn chain_tools() -> Arc<crate::tools::Tools> {
            Arc::new(
                ToolsBuilder::new()
                    .token(crate::token::TokenManager::new_symmetric(
                        "tenant-chain-test-secret",
                        jsonwebtoken::Algorithm::HS256,
                        "test-issuer".to_string(),
                        "test-audience".to_string(),
                        3600,
                        7200,
                    ))
                    .build()
                    .expect("测试 Tools 应构建成功"),
            )
        }

        fn chain_handle(
            app: &crate::definition::BuiltApp,
            module: &str,
            action: &str,
        ) -> crate::definition::ActionHandle {
            app.registry()
                .resolve(&ActionRef::new(
                    ModuleName::new(module).expect("测试 Module 名称应有效"),
                    ActionName::new(action).expect("测试 Action 名称应有效"),
                ))
                .expect("探针 Action 应已注册")
        }

        fn chain_access_token(app: &crate::definition::BuiltApp) -> String {
            app.tools()
                .token()
                .expect("测试应用应配置 TokenManager")
                .generate_access_token("user-7", serde_json::json!({}))
                .expect("测试 Access Token 应生成成功")
        }

        /// 构造「认证 → 租户」链路应用：受保护的租户探针。
        fn protected_chain_app() -> crate::definition::BuiltApp {
            let module = ModuleSpec::new(
                ModuleName::new("org.tenant_chain").expect("测试 Module 名称应有效"),
            )
            .middleware(TokenAuthMiddleware::new(|claims| {
                User::new(7, claims.sub.clone())
            }))
            .middleware(TenantResolverMiddleware::new(MembershipResolver))
            .action(
                ActionSpec::new(
                    ActionName::new("tenant_probe").expect("测试 Action 名称应有效"),
                    RouteSpec::new(
                        HttpMethod::Get,
                        "/api/v1/org/tenant-chain/probe",
                        "org.tenant_chain.tenant_probe",
                    ),
                ),
                TenantProbe,
            );
            AppBuilder::new()
                .addon(
                    AddonSpec::new(AddonName::new("org").expect("测试 Addon 名称应有效"))
                        .module(module),
                )
                .build(chain_tools())
                .expect("链路测试应用应构建成功")
        }

        #[test]
        fn build_rejects_tenant_before_token_authentication() {
            let module = ModuleSpec::new(
                ModuleName::new("org.tenant_bad_order").expect("测试 Module 名称应有效"),
            )
            .middleware(TenantResolverMiddleware::new(MembershipResolver))
            .middleware(TokenAuthMiddleware::new(|claims| {
                User::new(7, claims.sub.clone())
            }))
            .action(
                ActionSpec::new(
                    ActionName::new("tenant_probe").expect("测试 Action 名称应有效"),
                    RouteSpec::new(
                        HttpMethod::Get,
                        "/api/v1/org/tenant-bad-order/probe",
                        "org.tenant_bad_order.tenant_probe",
                    ),
                ),
                TenantProbe,
            );
            let error = AppBuilder::new()
                .addon(
                    AddonSpec::new(AddonName::new("org").expect("测试 Addon 名称应有效"))
                        .module(module),
                )
                .build(chain_tools())
                .expect_err("租户解析先于认证必须在构建期失败");
            assert!(matches!(
                error,
                crate::definition::BuildError::InvalidReference {
                    kind: "Middleware order",
                    ..
                }
            ));
        }

        /// 真实链路端到端：认证中间件先于租户中间件执行，任一环节失败即短路。
        #[tokio::test]
        async fn token_auth_runs_before_tenant_resolver_and_fails_closed() {
            let app = protected_chain_app();
            let handle = chain_handle(&app, "org.tenant_chain", "tenant_probe");

            // 无 Authorization 头：认证中间件短路。若顺序相反（租户先执行），
            // 报错会是 resolver 的「租户解析需要已认证用户」而非缺少 Token。
            let missing = app
                .dispatch(
                    handle,
                    Request::new(serde_json::json!({})).header(TENANT_ID_HEADER, "10"),
                )
                .await;
            assert!(matches!(
                missing,
                Err(BaseError::Unauthorized(message)) if message.contains("Authorization Bearer Token")
            ));

            // 无效 Token：签名校验失败即短路，请求不会到达租户 resolver。
            let invalid = app
                .dispatch(
                    handle,
                    Request::new(serde_json::json!({}))
                        .header("Authorization", "Bearer invalid-token")
                        .header(TENANT_ID_HEADER, "10"),
                )
                .await;
            assert!(matches!(invalid, Err(BaseError::TokenVerifyFailed(_))));

            // 签名有效但未配置撤销存储：verify_token_checked 不降级跳过撤销检查，
            // 以 RedisNotInitialized fail-closed，请求仍不会到达租户 resolver。
            let token = chain_access_token(&app);
            let no_store = app
                .dispatch(
                    handle,
                    Request::new(serde_json::json!({}))
                        .header("Authorization", format!("Bearer {token}"))
                        .header(TENANT_ID_HEADER, "10"),
                )
                .await;
            assert!(
                matches!(no_store, Err(BaseError::RedisNotInitialized)),
                "无撤销存储时不得降级放行: {no_store:?}"
            );
        }

        /// 同时观察租户上下文与认证状态的公开探针。
        #[derive(Debug, Serialize, JsonSchema)]
        struct TenantAuthProbeOutput {
            tenant_id: i64,
            authenticated: bool,
        }

        #[derive(Action)]
        #[action(name = "tenant_auth_probe", display_name = "租户认证探针", public)]
        struct TenantAuthProbe;

        #[async_trait]
        impl TypedHandler for TenantAuthProbe {
            type Input = EmptyInput;
            type Output = TenantAuthProbeOutput;

            async fn handle(
                &self,
                context: ActionContext,
                _input: Self::Input,
            ) -> Result<Self::Output, BaseError> {
                let tenant = context.tenant()?;
                Ok(TenantAuthProbeOutput {
                    tenant_id: tenant.id().get(),
                    authenticated: context.authenticated_user().is_some(),
                })
            }
        }

        /// 显式允许匿名的 resolver：匿名请求忽略客户端候选，落入服务端选择的
        /// 访客默认租户；已认证成员沿用 MembershipResolver 的候选校验规则。
        struct GuestOrMemberResolver;

        #[async_trait]
        impl TenantResolver for GuestOrMemberResolver {
            async fn resolve(
                &self,
                context: &ActionContext,
                requested: Option<TenantId>,
            ) -> Result<TenantResolution, BaseError> {
                let Some(user) = context.authenticated_user() else {
                    return Ok(TenantContext::new(TenantId::new(1)).into());
                };
                let tenant = requested.unwrap_or_else(|| TenantId::new(10));
                if user.id == 7 && tenant == TenantId::new(10) {
                    Ok(TenantContext::new(tenant).into())
                } else {
                    Err(BaseError::PermissionDenied(format!(
                        "用户无权访问租户 {}",
                        tenant.get()
                    )))
                }
            }
        }

        /// 构造三方组合应用：公开 Action + 可选认证 + 租户中间件。
        fn public_chain_app() -> crate::definition::BuiltApp {
            let module = ModuleSpec::new(
                ModuleName::new("org.tenant_public").expect("测试 Module 名称应有效"),
            )
            .middleware(
                TokenAuthMiddleware::new(|claims| User::new(7, claims.sub.clone()))
                    .authenticate_public_actions(),
            )
            .middleware(TenantResolverMiddleware::new(GuestOrMemberResolver))
            .action(
                ActionSpec::new(
                    ActionName::new("tenant_auth_probe").expect("测试 Action 名称应有效"),
                    RouteSpec::new(
                        HttpMethod::Get,
                        "/api/v1/org/tenant-public/probe",
                        "org.tenant_public.tenant_auth_probe",
                    ),
                )
                .public(true),
                TenantAuthProbe,
            );
            AppBuilder::new()
                .addon(
                    AddonSpec::new(AddonName::new("org").expect("测试 Addon 名称应有效"))
                        .module(module),
                )
                .build(chain_tools())
                .expect("三方组合测试应用应构建成功")
        }

        fn probe_observation(response: ApiResponse) -> (i64, bool) {
            let data = response.data.expect("探针应返回 JSON data");
            (
                data.get("tenant_id")
                    .and_then(serde_json::Value::as_i64)
                    .expect("探针应返回 tenant_id"),
                data.get("authenticated")
                    .and_then(serde_json::Value::as_bool)
                    .expect("探针应返回 authenticated 布尔值"),
            )
        }

        /// 三方组合：匿名可通行并由租户中间件注入访客上下文；认证信息缺失或
        /// 无效时一律 fail-closed，绝不降级为匿名。
        #[tokio::test]
        async fn public_action_with_optional_auth_and_tenant_middleware_compose() {
            let app = public_chain_app();
            let handle = chain_handle(&app, "org.tenant_public", "tenant_auth_probe");

            // 完全匿名（Authorization 头整个缺失）：可选认证放行，租户中间件
            // 注入 resolver 显式选择的访客默认租户，公开 Action 正常执行。
            let anonymous = app
                .dispatch(
                    handle,
                    Request::new(serde_json::json!({})).header(TENANT_ID_HEADER, "10"),
                )
                .await
                .expect("匿名请求应经可选认证放行并获得访客租户上下文");
            assert_eq!(probe_observation(anonymous), (1, false));

            // 非 Bearer 的 Authorization 头：不降级为匿名。
            let wrong_scheme = app
                .dispatch(
                    handle,
                    Request::new(serde_json::json!({}))
                        .header("Authorization", "Basic credentials"),
                )
                .await;
            assert!(matches!(
                wrong_scheme,
                Err(BaseError::Unauthorized(message)) if message.contains("Authorization Bearer Token")
            ));

            // 无效 Bearer Token：不降级为匿名，租户 resolver 不会执行。
            let invalid = app
                .dispatch(
                    handle,
                    Request::new(serde_json::json!({}))
                        .header("Authorization", "Bearer invalid-token"),
                )
                .await;
            assert!(matches!(invalid, Err(BaseError::TokenVerifyFailed(_))));

            // 有效 Token 但无撤销存储：同样 fail-closed，不降级为匿名。
            let token = chain_access_token(&app);
            let no_store = app
                .dispatch(
                    handle,
                    Request::new(serde_json::json!({}))
                        .header("Authorization", format!("Bearer {token}")),
                )
                .await;
            assert!(
                matches!(no_store, Err(BaseError::RedisNotInitialized)),
                "无撤销存储时不得降级为匿名: {no_store:?}"
            );
        }
    }
}
