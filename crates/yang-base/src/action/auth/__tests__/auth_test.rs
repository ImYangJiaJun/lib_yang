#[cfg(test)]
mod tests {
    use crate::action::auth::*;
    use crate::action::{
        ActionContext, ApiResponse, DynAction, TypedAction, TypedHandler, UiCatalogAction, User,
    };
    use crate::definition::{
        ActionName, ActionRef, ActionSpec, AddonName, AddonSpec, AppBuilder, BuiltApp, HttpMethod,
        ModuleName, ModuleSpec, RouteSpec,
    };
    use crate::error::BaseError;
    use crate::router::middleware::MiddlewareScope;
    use crate::router::{Middleware, Next};
    use crate::token::TokenClaims;
    use async_trait::async_trait;
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use testcontainers::{runners::AsyncRunner, GenericImage};
    use yang_base_derive::Action;
    use yang_db::{RedisClient, RedisConfig};

    struct DummyVerifier;

    #[async_trait]
    impl CredentialVerifier for DummyVerifier {
        async fn verify(
            &self,
            _ctx: &ActionContext,
            input: &LoginInput,
        ) -> Result<VerifiedSubject, BaseError> {
            Ok(VerifiedSubject::new(format!("user:{}", input.username)))
        }
    }

    #[derive(Debug, Default, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields)]
    struct EmptyInput {}

    #[derive(Debug, Serialize, JsonSchema)]
    struct ProbeOutput {
        authenticated: bool,
    }

    #[derive(Action)]
    #[action(name = "protected_probe", display_name = "受保护探针")]
    struct ProtectedProbe;

    #[async_trait]
    impl TypedHandler for ProtectedProbe {
        type Input = EmptyInput;
        type Output = ProbeOutput;

        async fn handle(
            &self,
            ctx: ActionContext,
            _input: Self::Input,
        ) -> Result<Self::Output, BaseError> {
            Ok(ProbeOutput {
                authenticated: ctx.authenticated_user().is_some(),
            })
        }
    }

    #[derive(Action)]
    #[action(name = "public_probe", display_name = "公开探针", public)]
    struct PublicProbe;

    #[async_trait]
    impl TypedHandler for PublicProbe {
        type Input = EmptyInput;
        type Output = ProbeOutput;

        async fn handle(
            &self,
            ctx: ActionContext,
            _input: Self::Input,
        ) -> Result<Self::Output, BaseError> {
            Ok(ProbeOutput {
                authenticated: ctx.authenticated_user().is_some(),
            })
        }
    }

    struct CountingMiddleware(Arc<AtomicUsize>);

    #[async_trait]
    impl Middleware for CountingMiddleware {
        async fn handle(
            &self,
            ctx: ActionContext,
            next: Next<'_>,
        ) -> Result<ApiResponse, BaseError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            next.run(ctx).await
        }
    }

    struct LegacyRefreshResolver;

    #[async_trait]
    impl RefreshClaimsResolver for LegacyRefreshResolver {
        async fn resolve(
            &self,
            _ctx: &ActionContext,
            sub: &str,
        ) -> Result<serde_json::Value, BaseError> {
            Ok(serde_json::json!({ "resolved_sub": sub }))
        }
    }

    struct CredentialVersionRefreshResolver;

    #[async_trait]
    impl RefreshClaimsResolver for CredentialVersionRefreshResolver {
        async fn resolve(
            &self,
            _ctx: &ActionContext,
            _sub: &str,
        ) -> Result<serde_json::Value, BaseError> {
            Err(BaseError::ConfigError(
                "完整 claims hook 不应回退到旧 resolve".to_string(),
            ))
        }

        async fn resolve_pair_from_claims(
            &self,
            _ctx: &ActionContext,
            claims: &TokenClaims,
        ) -> Result<TokenPairClaims, BaseError> {
            let credential_version = claims
                .custom
                .get("credential_version")
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| BaseError::Unauthorized("Refresh Token 缺少凭据版本".to_string()))?;
            if credential_version != 7 {
                return Err(BaseError::Unauthorized(
                    "Refresh Token 凭据版本已失效".to_string(),
                ));
            }
            Ok(TokenPairClaims::new(serde_json::json!({
                "credential_version": credential_version,
            })))
        }
    }

    fn refresh_claims(custom: serde_json::Value) -> TokenClaims {
        TokenClaims::new(
            "test-issuer",
            "user-7",
            "test-audience",
            u64::MAX,
            0,
            0,
            "refresh-jti",
            crate::token::TokenType::Refresh,
            custom,
        )
    }

    fn refresh_test_context() -> ActionContext {
        ActionContext::new(
            crate::action::Request::default(),
            Arc::new(
                crate::tools::ToolsBuilder::new()
                    .build()
                    .expect("Refresh hook 测试 Tools 应构建成功"),
            ),
        )
    }

    #[tokio::test]
    async fn refresh_claims_hook_rejects_stale_credential_version() {
        let error = CredentialVersionRefreshResolver
            .resolve_pair_from_claims(
                &refresh_test_context(),
                &refresh_claims(serde_json::json!({ "credential_version": 6 })),
            )
            .await
            .expect_err("旧凭据世代的 Refresh Token 必须被拒绝");

        assert!(matches!(error, BaseError::Unauthorized(message) if message.contains("已失效")));
    }

    #[tokio::test]
    async fn refresh_claims_hook_accepts_current_credential_version() {
        let pair = CredentialVersionRefreshResolver
            .resolve_pair_from_claims(
                &refresh_test_context(),
                &refresh_claims(serde_json::json!({ "credential_version": 7 })),
            )
            .await
            .expect("当前凭据世代的 Refresh Token 应生成新声明");

        assert_eq!(pair.access, serde_json::json!({ "credential_version": 7 }));
        assert_eq!(pair.refresh, serde_json::Value::Null);
    }

    #[tokio::test]
    async fn refresh_claims_hook_keeps_legacy_subject_resolver_compatible() {
        let pair = LegacyRefreshResolver
            .resolve_pair_from_claims(
                &refresh_test_context(),
                &refresh_claims(serde_json::Value::Null),
            )
            .await
            .expect("旧 resolver 应通过默认适配继续工作");

        assert_eq!(pair.access, serde_json::json!({ "resolved_sub": "user-7" }));
        assert_eq!(pair.refresh, serde_json::Value::Null);
    }

    struct RejectingClaimsValidator(Arc<AtomicUsize>);

    #[async_trait]
    impl TokenClaimsValidator for RejectingClaimsValidator {
        async fn validate(
            &self,
            _context: &ActionContext,
            _claims: &TokenClaims,
        ) -> Result<(), BaseError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(BaseError::Unauthorized(
                "应用级 Token 声明已失效".to_string(),
            ))
        }
    }

    fn test_token_manager() -> crate::token::TokenManager {
        crate::token::TokenManager::new_symmetric(
            "mixed-public-protected-actions-test-secret",
            jsonwebtoken::Algorithm::HS256,
            "test-issuer".to_string(),
            "test-audience".to_string(),
            3600,
            7200,
        )
    }

    fn test_tools() -> Arc<crate::tools::Tools> {
        Arc::new(
            crate::tools::ToolsBuilder::new()
                .token(test_token_manager())
                .build()
                .expect("测试 Tools 应构建成功"),
        )
    }

    fn optional_auth_app(tools: Arc<crate::tools::Tools>) -> BuiltApp {
        let module = ModuleSpec::new(
            ModuleName::new("account.optional_auth").expect("测试 Module 名称应有效"),
        )
        .middleware(
            TokenAuthMiddleware::new(|claims| User::new(7, claims.sub.clone()))
                .authenticate_public_actions(),
        )
        .action(
            ActionSpec::new(
                ActionName::new("public_probe").expect("测试 Action 名称应有效"),
                RouteSpec::new(
                    HttpMethod::Get,
                    "/api/v1/optional-auth/public",
                    "account.optional_auth.public_probe",
                ),
            )
            .public(true),
            PublicProbe,
        )
        .action(
            ActionSpec::new(
                ActionName::new("protected_probe").expect("测试 Action 名称应有效"),
                RouteSpec::new(
                    HttpMethod::Get,
                    "/api/v1/optional-auth/protected",
                    "account.optional_auth.protected_probe",
                ),
            ),
            ProtectedProbe,
        )
        .native_action(UiCatalogAction);

        AppBuilder::new()
            .addon(
                AddonSpec::new(AddonName::new("account").expect("测试 Addon 名称应有效"))
                    .module(module),
            )
            .build(tools)
            .expect("可选认证测试应用应构建成功")
    }

    fn optional_auth_ref(name: &str) -> ActionRef {
        ActionRef::new(
            ModuleName::new("account.optional_auth").expect("测试 Module 名称应有效"),
            ActionName::new(name).expect("测试 Action 名称应有效"),
        )
    }

    fn test_access_token(app: &BuiltApp) -> String {
        app.tools()
            .token()
            .expect("测试应用应配置 TokenManager")
            .generate_access_token("user-7", serde_json::json!({}))
            .expect("测试 Access Token 应生成成功")
    }

    fn response_authenticated(response: ApiResponse) -> bool {
        response
            .data
            .and_then(|data| {
                data.get("authenticated")
                    .and_then(serde_json::Value::as_bool)
            })
            .expect("探针响应应包含 authenticated 布尔值")
    }

    fn catalog_operation_ids(response: ApiResponse) -> Vec<String> {
        response
            .data
            .and_then(|data| data.get("actions").cloned())
            .and_then(|actions| actions.as_array().cloned())
            .expect("UI 目录响应应包含 actions 数组")
            .into_iter()
            .map(|action| {
                action
                    .get("operation_id")
                    .and_then(serde_json::Value::as_str)
                    .expect("UI 目录 Action 应包含 operation_id")
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn test_auth_actions_meta() {
        let login = LoginAction::new(DummyVerifier);
        assert_eq!(login.name(), "login");
        assert!(DynAction::meta(&login).is_public);

        let refresh = RefreshAction::<DefaultRefreshClaims>::default();
        assert_eq!(refresh.name(), "refresh");
        assert!(refresh.is_public());

        let logout = LogoutAction::new();
        assert_eq!(logout.name(), "logout");
        assert!(logout.is_public());
    }

    #[test]
    fn token_auth_middleware_accepts_fallible_user_projection() {
        fn assert_middleware(_: &impl Middleware) {}

        let middleware = TokenAuthMiddleware::new(|_claims: &TokenClaims| {
            Err::<User, BaseError>(BaseError::Unauthorized("Token subject 无效".to_string()))
        });

        assert_middleware(&middleware);
        assert_eq!(middleware.scope(), MiddlewareScope::ProtectedActions);
    }

    #[tokio::test]
    async fn application_claims_validator_never_runs_before_core_access_validation() {
        let validation_calls = Arc::new(AtomicUsize::new(0));
        let projection_calls = Arc::new(AtomicUsize::new(0));
        let projection_counter = Arc::clone(&projection_calls);
        let module_name =
            ModuleName::new("account.claims_validation").expect("测试 Module 名称应有效");
        let probe_ref = ActionRef::new(
            module_name.clone(),
            ActionName::new("protected_probe").expect("测试 Action 名称应有效"),
        );
        let module = ModuleSpec::new(module_name)
            .middleware(
                TokenAuthMiddleware::new(move |claims| {
                    projection_counter.fetch_add(1, Ordering::SeqCst);
                    User::new(7, claims.sub.clone())
                })
                .with_claims_validator(RejectingClaimsValidator(Arc::clone(&validation_calls))),
            )
            .action(
                ActionSpec::new(
                    ActionName::new("protected_probe").expect("测试 Action 名称应有效"),
                    RouteSpec::new(
                        HttpMethod::Get,
                        "/api/v1/claims-validation/protected",
                        "account.claims_validation.protected_probe",
                    ),
                ),
                ProtectedProbe,
            );
        let app = AppBuilder::new()
            .addon(
                AddonSpec::new(AddonName::new("account").expect("测试 Addon 名称应有效"))
                    .module(module),
            )
            .build(test_tools())
            .expect("声明校验测试应用应构建成功");
        let token = test_access_token(&app);
        let response = app
            .dispatch(
                app.registry()
                    .resolve(&probe_ref)
                    .expect("protected_probe 应已注册"),
                crate::action::Request::new(serde_json::json!({}))
                    .header("authorization", format!("Bearer {token}")),
            )
            .await;

        assert!(
            matches!(response, Err(BaseError::RedisNotInitialized)),
            "核心撤销检查缺失时必须先 fail-closed: {response:?}"
        );
        assert_eq!(
            validation_calls.load(Ordering::SeqCst),
            0,
            "核心 Access Token 验证失败前不得运行应用校验器"
        );
        assert_eq!(
            projection_calls.load(Ordering::SeqCst),
            0,
            "应用级校验失败后不得投影或注入用户"
        );
    }

    #[tokio::test]
    async fn public_and_protected_actions_share_auth_enabled_module() {
        let calls = Arc::new(AtomicUsize::new(0));
        let module_name = ModuleName::new("account.user").expect("测试 Module 名称应有效");
        let login_ref = ActionRef::new(
            module_name.clone(),
            ActionName::new("login").expect("测试 Action 名称应有效"),
        );
        let probe_ref = ActionRef::new(
            module_name.clone(),
            ActionName::new("protected_probe").expect("测试 Action 名称应有效"),
        );
        let module = ModuleSpec::new(module_name)
            .middleware(CountingMiddleware(Arc::clone(&calls)))
            .middleware(TokenAuthMiddleware::new(|claims| {
                User::new(1, claims.sub.clone())
            }))
            .action(
                ActionSpec::new(
                    ActionName::new("login").expect("测试 Action 名称应有效"),
                    RouteSpec::new(
                        HttpMethod::Post,
                        "/api/v1/users/login",
                        "account.user.login",
                    ),
                )
                .public(true),
                LoginAction::new(DummyVerifier),
            )
            .action(
                ActionSpec::new(
                    ActionName::new("protected_probe").expect("测试 Action 名称应有效"),
                    RouteSpec::new(
                        HttpMethod::Get,
                        "/api/v1/users/me",
                        "account.user.protected_probe",
                    ),
                ),
                ProtectedProbe,
            );
        let app = AppBuilder::new()
            .addon(
                AddonSpec::new(AddonName::new("account").expect("测试 Addon 名称应有效"))
                    .module(module),
            )
            .build(test_tools())
            .expect("同一模块应能注册公开与受保护 Action");
        let login_handle = app.registry().resolve(&login_ref).expect("login 应已注册");
        let probe_handle = app
            .registry()
            .resolve(&probe_ref)
            .expect("protected_probe 应已注册");

        let public_request = crate::action::Request::new(serde_json::json!({
            "username": "alice",
            "password": "correct-password"
        }));
        let public_response = app.dispatch(login_handle, public_request).await;
        assert!(
            public_response.is_ok(),
            "公开 Action 不应被 TokenAuthMiddleware 拦截: {public_response:?}"
        );

        let protected_response = app
            .dispatch(
                probe_handle,
                crate::action::Request::new(serde_json::json!({})),
            )
            .await;
        assert!(matches!(
            protected_response,
            Err(BaseError::Unauthorized(message))
                if message.contains("Authorization Bearer Token")
        ));
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "通用中间件应覆盖公开与受保护 Action"
        );
    }

    #[tokio::test]
    async fn optional_public_auth_distinguishes_absent_valid_and_invalid_credentials() {
        let app = optional_auth_app(test_tools());
        let public_handle = app
            .registry()
            .resolve(&optional_auth_ref("public_probe"))
            .expect("公开探针应已注册");

        let anonymous = app
            .dispatch(
                public_handle,
                crate::action::Request::new(serde_json::json!({})),
            )
            .await
            .expect("缺少 Authorization header 时公开 Action 应按匿名继续");
        assert!(!response_authenticated(anonymous));

        let invalid = app
            .dispatch(
                public_handle,
                crate::action::Request::new(serde_json::json!({}))
                    .header("Authorization", "Bearer invalid-token"),
            )
            .await;
        assert!(matches!(invalid, Err(BaseError::TokenVerifyFailed(_))));

        let wrong_scheme = app
            .dispatch(
                public_handle,
                crate::action::Request::new(serde_json::json!({}))
                    .header("Authorization", "Basic credentials"),
            )
            .await;
        assert!(matches!(
            wrong_scheme,
            Err(BaseError::Unauthorized(message))
                if message.contains("Authorization Bearer Token")
        ));
    }

    #[tokio::test]
    async fn optional_public_auth_projects_catalog_without_weakening_protected_actions() {
        let app = optional_auth_app(test_tools());
        let catalog_handle = app
            .registry()
            .resolve(&optional_auth_ref("ui_catalog"))
            .expect("UI 目录应已注册");
        let protected_handle = app
            .registry()
            .resolve(&optional_auth_ref("protected_probe"))
            .expect("受保护探针应已注册");

        let anonymous_catalog = app
            .dispatch(
                catalog_handle,
                crate::action::Request::new(serde_json::json!({})),
            )
            .await
            .expect("匿名用户应能读取公开目录");
        let anonymous_ids = catalog_operation_ids(anonymous_catalog);
        assert!(anonymous_ids.contains(&"account.optional_auth.public_probe".to_string()));
        assert!(!anonymous_ids.contains(&"account.optional_auth.protected_probe".to_string()));

        let protected_without_token = app
            .dispatch(
                protected_handle,
                crate::action::Request::new(serde_json::json!({})),
            )
            .await;
        assert!(matches!(
            protected_without_token,
            Err(BaseError::Unauthorized(message))
                if message.contains("Authorization Bearer Token")
        ));
    }

    /// I-6 调查锁定：`verify_token_checked` 在无撤销存储（`test_tools()` 未配置
    /// cache）时**不降级**跳过撤销检查，而是以 `RedisNotInitialized` fail-closed。
    /// 因此「有效 token → 注入身份 → 目录按身份投影」无法改写为非 Docker 单测，
    /// 仍由下方 `#[ignore]` Docker 测试覆盖；本测试锁定该结论，防止日后有人
    /// 把无存储行为改成静默放行。
    #[tokio::test]
    async fn optional_public_auth_without_revocation_store_fails_closed() {
        let app = optional_auth_app(test_tools());
        let token = test_access_token(&app);
        let public_handle = app
            .registry()
            .resolve(&optional_auth_ref("public_probe"))
            .expect("公开探针应已注册");

        let result = app
            .dispatch(
                public_handle,
                crate::action::Request::new(serde_json::json!({}))
                    .header("Authorization", format!("Bearer {token}")),
            )
            .await;
        assert!(
            matches!(result, Err(BaseError::RedisNotInitialized)),
            "无撤销存储时不得降级放行: {result:?}"
        );
    }

    #[tokio::test]
    #[ignore = "需要 Docker 启动 Redis 撤销存储（无存储时 fail-closed，见 optional_public_auth_without_revocation_store_fails_closed）"]
    async fn optional_public_auth_injects_valid_identity_into_catalog_projection() {
        let redis_image = GenericImage::new("redis", "7-alpine").with_wait_for(
            testcontainers::core::WaitFor::message_on_stdout("Ready to accept connections"),
        );
        let redis_container = redis_image.start().await.expect("Redis 测试容器应启动成功");
        let redis_port = redis_container
            .get_host_port_ipv4(6379)
            .await
            .expect("Redis 测试端口应可获取");
        let redis_url = format!("redis://127.0.0.1:{redis_port}");
        let cache = RedisClient::connect_with_config(&redis_url, RedisConfig::default())
            .await
            .expect("Redis 测试客户端应连接成功");
        let tools = Arc::new(
            crate::tools::ToolsBuilder::new()
                .cache(cache)
                .token(test_token_manager())
                .build()
                .expect("带撤销存储的测试 Tools 应构建成功"),
        );
        let app = optional_auth_app(tools);
        let token = test_access_token(&app);

        let public_handle = app
            .registry()
            .resolve(&optional_auth_ref("public_probe"))
            .expect("公开探针应已注册");
        let authenticated = app
            .dispatch(
                public_handle,
                crate::action::Request::new(serde_json::json!({}))
                    .header("Authorization", format!("Bearer {token}")),
            )
            .await
            .expect("合法 Access Token 应在公开 Action 注入用户");
        assert!(response_authenticated(authenticated));

        let catalog_handle = app
            .registry()
            .resolve(&optional_auth_ref("ui_catalog"))
            .expect("UI 目录应已注册");
        let authenticated_catalog = app
            .dispatch(
                catalog_handle,
                crate::action::Request::new(serde_json::json!({}))
                    .header("Authorization", format!("Bearer {token}")),
            )
            .await
            .expect("认证用户应能读取按身份投影的目录");
        let authenticated_ids = catalog_operation_ids(authenticated_catalog);
        assert!(authenticated_ids.contains(&"account.optional_auth.protected_probe".to_string()));
    }

    /// token 指纹稳定且不含原文（同输入同指纹，异输入异指纹）。
    #[test]
    fn test_token_fingerprint_stable_and_opaque() {
        let a = token_fingerprint("super-secret-access-token");
        let b = token_fingerprint("super-secret-access-token");
        let c = token_fingerprint("another-token");
        assert_eq!(a, b, "同一 token 指纹应稳定");
        assert_ne!(a, c, "不同 token 指纹应不同");
        assert_eq!(a.len(), 16, "指纹为 16 位十六进制");
        assert!(!a.contains("secret"), "指纹不得含原文");
    }

    /// 审计钩子注入：with_audit 可替换默认钩子，事件被记录且不含 token 原文。
    #[tokio::test]
    async fn test_audit_hook_records_without_leaking() {
        use std::sync::{Arc, Mutex};

        #[derive(Clone, Default)]
        struct RecordingHook {
            events: Arc<Mutex<Vec<AuthAuditEvent>>>,
        }
        #[async_trait]
        impl AuthAuditHook for RecordingHook {
            async fn on_success(&self, event: AuthAuditEvent) {
                self.events.lock().unwrap().push(event);
            }
            async fn on_failure(&self, event: AuthAuditEvent) {
                self.events.lock().unwrap().push(event);
            }
        }

        let hook = RecordingHook::default();
        let login = LoginAction::with_audit(DummyVerifier, hook.clone());
        // 仅验证构造 + 钩子类型注入成功（端到端派发在集成测试覆盖）
        assert_eq!(login.name(), "login");

        // 直接触发一次 on_success 验证事件落库且字段不含敏感原文
        hook.on_success(AuthAuditEvent {
            request_id: "abc".into(),
            action: "login",
            subject: Some("user:alice".into()),
            error_code: None,
        })
        .await;
        let events = hook.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, "login");
        assert_eq!(events[0].subject.as_deref(), Some("user:alice"));
    }
}
