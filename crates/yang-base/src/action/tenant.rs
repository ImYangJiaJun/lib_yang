//! 请求租户解析中间件。
//!
//! 客户端提供的租户 ID 只是不可信候选；应用实现的 [`TenantResolver`] 必须结合
//! 已认证用户、租户成员关系和业务状态完成校验，再返回可信 [`TenantContext`]。

use super::{ActionContext, ApiResponse, TenantContext, TenantId};
use crate::error::BaseError;
use crate::router::{Middleware, Next};
use async_trait::async_trait;

/// 默认租户候选请求头。
pub const TENANT_ID_HEADER: &str = "x-tenant-id";

/// 将不可信租户候选解析为可信请求租户上下文。
///
/// `requested` 仅表示客户端想访问的租户，不能证明当前用户属于该租户。实现者必须
/// 依据已认证用户和服务端事实源校验成员关系；若允许系统级绕过，也必须在这里检查
/// 独立的系统权限后显式返回 [`TenantContext::system`]。
#[async_trait]
pub trait TenantResolver: Send + Sync + 'static {
    /// 解析当前请求的可信租户上下文。
    ///
    /// `None` 表示客户端未指定租户；实现者可选择安全的默认租户，也可拒绝请求。
    async fn resolve(
        &self,
        context: &ActionContext,
        requested: Option<TenantId>,
    ) -> Result<TenantContext, BaseError>;
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
    async fn handle(
        &self,
        context: ActionContext,
        next: Next<'_>,
    ) -> Result<ApiResponse, BaseError> {
        let requested = requested_tenant(&context)?;
        let tenant = self.resolver.resolve(&context, requested).await?;
        next.run(context.with_tenant(tenant)).await
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
        system: bool,
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
            let tenant = context.tenant()?;
            Ok(TenantProbeOutput {
                tenant_id: tenant.id().map(TenantId::get),
                system: tenant.is_system(),
            })
        }
    }

    struct MembershipResolver;

    #[async_trait]
    impl TenantResolver for MembershipResolver {
        async fn resolve(
            &self,
            context: &ActionContext,
            requested: Option<TenantId>,
        ) -> Result<TenantContext, BaseError> {
            let user = context
                .authenticated_user()
                .ok_or_else(|| BaseError::Unauthorized("租户解析需要已认证用户".to_string()))?;
            if user.has_role("system") {
                return Ok(TenantContext::system());
            }

            let tenant = requested.unwrap_or_else(|| TenantId::new(10));
            if user.id == 7 && tenant == TenantId::new(10) {
                Ok(TenantContext::new(tenant))
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

    fn tenant_response(response: ApiResponse) -> (Option<i64>, bool) {
        let data = response.data.expect("租户探针应返回 JSON data");
        (
            data.get("tenant_id").and_then(serde_json::Value::as_i64),
            data.get("system")
                .and_then(serde_json::Value::as_bool)
                .expect("租户探针应返回 system 布尔值"),
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
        assert_eq!(tenant_response(selected), (Some(10), false));

        let defaulted = dispatch_as(&app, Request::new(serde_json::json!({})), Some(user))
            .await
            .expect("resolver 应能安全选择服务端默认租户");
        assert_eq!(tenant_response(defaulted), (Some(10), false));
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

        let response = dispatch_as(&app, Request::new(serde_json::json!({})), Some(system_user))
            .await
            .expect("系统角色应由可信 resolver 映射为 system 上下文");
        assert_eq!(tenant_response(response), (None, true));

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
}
