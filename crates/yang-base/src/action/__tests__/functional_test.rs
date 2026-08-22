//! 函数式 Action 注册 API（`ModuleSpec::action_fn`）契约测试。
//!
//! 核心断言：同一业务分别以 `#[derive(Action)]` 与函数式通道注册时，
//! Catalog（ActionSpec）、OpenAPI 投影、dispatch 行为、强类型内部调用、
//! 权限/公开位与按 ActionRef 的中间件匹配完全一致。

use crate::action::{ActionContext, ApiResponse, PermissionMode, User};
use crate::definition::{
    ActionName, ActionRef, AddonName, AddonSpec, AppBuilder, BuiltApp, HttpMethod, ModuleName,
    ModuleSpec,
};
use crate::error::BaseError;
use crate::router::{Middleware, Next};
use crate::tools::ToolsBuilder;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Serialize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

crate::params! {
    #[deny_unknown_fields]
    pub FnEchoInput {
        value: crate::definition::Int::new().require(true),
    }
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct FnEchoOutput {
    value: i64,
}

/// 与函数式 Handler 等价的 derive 对照 Action。
#[derive(crate::Action)]
#[action(
    name = "echo",
    display_name = "回声",
    description = "原样返回输入值",
    method = "POST",
    path = "/echo",
    permissions("test:echo"),
    permission_mode = "any"
)]
struct DeriveEchoAction;

#[async_trait]
impl crate::action::Action for DeriveEchoAction {
    type Input = FnEchoInput;
    type Output = FnEchoOutput;

    async fn index(
        &self,
        _ctx: ActionContext,
        input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Ok(FnEchoOutput { value: input.value })
    }
}

/// 函数式 Handler：与 `DeriveEchoAction` 的业务逻辑一致。
async fn echo_handler(_ctx: ActionContext, input: FnEchoInput) -> Result<FnEchoOutput, BaseError> {
    Ok(FnEchoOutput { value: input.value })
}

fn action_name(value: &str) -> ActionName {
    ActionName::new(value).expect("测试 Action 名称应有效")
}

fn module_name(value: &str) -> ModuleName {
    ModuleName::new(value).expect("测试 Module 名称应有效")
}

fn echo_ref() -> ActionRef {
    ActionRef::new(module_name("test.echo"), action_name("echo"))
}

fn build_app(module: ModuleSpec) -> BuiltApp {
    AppBuilder::new()
        .addon(
            AddonSpec::new(AddonName::new("test").expect("测试 Addon 名称应有效")).module(module),
        )
        .build(ToolsBuilder::new().build().expect("测试 Tools 应构建成功"))
        .expect("测试应用应构建成功")
}

fn derive_app() -> BuiltApp {
    build_app(ModuleSpec::new(module_name("test.echo")).native_action(DeriveEchoAction))
}

fn functional_app() -> BuiltApp {
    build_app(
        ModuleSpec::new(module_name("test.echo"))
            .action_fn(action_name("echo"), echo_handler)
            .route(HttpMethod::Post, "/echo")
            .display_name("回声")
            .description("原样返回输入值")
            .permissions(["test:echo"])
            .permission_mode(PermissionMode::Any)
            .register(),
    )
}

/// 携带 `test:echo` 权限的已认证请求上下文。
fn privileged_context(app: &BuiltApp, body: serde_json::Value) -> ActionContext {
    app.context(crate::action::Request::new(body))
        .with_user(User::new(1, "tester").with_permissions(["test:echo"]))
}

#[test]
fn functional_action_matches_derive_catalog_contract() {
    let derive = derive_app();
    let functional = functional_app();
    let derive_spec = &derive.catalog().addons()[0].modules[0].actions()[0];
    let fn_spec = &functional.catalog().addons()[0].modules[0].actions()[0];
    assert_eq!(
        derive_spec, fn_spec,
        "函数式与 derive 通道应产出完全一致的 ActionSpec"
    );
    // 关键字段单独断言，便于回归时快速定位漂移点
    assert_eq!(fn_spec.route.operation_id, "test.echo.echo");
    assert_eq!(fn_spec.permissions, ["test:echo"]);
    assert_eq!(fn_spec.permission_mode, PermissionMode::Any);
    assert!(!fn_spec.is_public);
    assert_eq!(fn_spec.params.len(), 1);
    assert!(
        !fn_spec.input_schema.is_null() && !fn_spec.output_schema.is_null(),
        "Schema 应在注册期填充，运行期不依赖 meta() 占位实现"
    );
}

#[test]
fn functional_action_defaults_match_derive_defaults() {
    // 不显式声明 route/展示信息时，默认契约（POST /{name}、display_name 回退为
    // Action 名、无权限、非公开）与 derive 通道一致。
    #[derive(crate::Action)]
    #[action(name = "ping", public)]
    struct DerivePingAction;

    #[async_trait]
    impl crate::action::Action for DerivePingAction {
        type Input = FnEchoInput;
        type Output = FnEchoOutput;

        async fn index(
            &self,
            _ctx: ActionContext,
            input: Self::Input,
        ) -> Result<Self::Output, BaseError> {
            Ok(FnEchoOutput { value: input.value })
        }
    }

    async fn ping_handler(
        _ctx: ActionContext,
        input: FnEchoInput,
    ) -> Result<FnEchoOutput, BaseError> {
        Ok(FnEchoOutput { value: input.value })
    }

    let derive =
        build_app(ModuleSpec::new(module_name("test.ping")).native_action(DerivePingAction));
    let functional = build_app(
        ModuleSpec::new(module_name("test.ping"))
            .action_fn(action_name("ping"), ping_handler)
            .public()
            .register(),
    );
    let derive_spec = &derive.catalog().addons()[0].modules[0].actions()[0];
    let fn_spec = &functional.catalog().addons()[0].modules[0].actions()[0];
    assert_eq!(fn_spec.route.method, HttpMethod::Post);
    assert_eq!(fn_spec.route.path, "/ping");
    assert_eq!(fn_spec.route.operation_id, "test.ping.ping");
    assert_eq!(fn_spec.display_name, "ping");
    assert_eq!(
        derive_spec, fn_spec,
        "默认契约下两条通道的 ActionSpec 应完全一致"
    );
}

#[cfg(feature = "openapi")]
#[test]
fn functional_action_projects_identical_openapi() {
    use crate::definition::OpenApiInfo;

    let info = OpenApiInfo::new("测试", "1.0.0");
    let derive_doc = derive_app()
        .catalog()
        .to_openapi(info.clone())
        .expect("derive OpenAPI 投影应成功");
    let fn_doc = functional_app()
        .catalog()
        .to_openapi(info)
        .expect("函数式 OpenAPI 投影应成功");
    assert_eq!(derive_doc, fn_doc, "两条通道的 OpenAPI 投影应完全一致");
}

#[tokio::test]
async fn functional_dispatch_matches_derive_envelope() {
    let derive_app = derive_app();
    let fn_app = functional_app();
    let derive_handle = derive_app
        .registry()
        .resolve(&echo_ref())
        .expect("derive Action 应已注册");
    let fn_handle = fn_app
        .registry()
        .resolve(&echo_ref())
        .expect("函数式 Action 应已注册");

    let body = serde_json::json!({"value": 21});
    let derive_response = derive_app
        .registry()
        .dispatch(derive_handle, privileged_context(&derive_app, body.clone()))
        .await
        .expect("derive dispatch 应成功");
    let fn_response = fn_app
        .registry()
        .dispatch(fn_handle, privileged_context(&fn_app, body))
        .await
        .expect("函数式 dispatch 应成功");

    assert_eq!(fn_response.code, 0);
    assert_eq!(fn_response.message, "成功");
    assert_eq!(fn_response.data.as_ref().expect("应有 data")["value"], 21);
    assert!(
        fn_response.attachment.is_none(),
        "普通 JSON 输出不得携带附件"
    );
    assert_eq!(
        serde_json::to_value(&derive_response).expect("derive 响应应可序列化"),
        serde_json::to_value(&fn_response).expect("函数式响应应可序列化"),
        "两条通道的响应线格式应完全一致"
    );
}

#[tokio::test]
async fn functional_dispatch_decodes_input_with_same_rules() {
    let fn_app = functional_app();
    let handle = fn_app
        .registry()
        .resolve(&echo_ref())
        .expect("函数式 Action 应已注册");

    // 类型不符：ParamInvalid（与 derive 通道共用同一 ParamInput::decode）
    let wrong_type = fn_app
        .registry()
        .dispatch(
            handle,
            privileged_context(&fn_app, serde_json::json!({"value": "NaN"})),
        )
        .await;
    assert!(
        matches!(wrong_type, Err(BaseError::ParamInvalid(_, _))),
        "类型不符应返回 ParamInvalid: {wrong_type:?}"
    );

    // deny_unknown_fields：未知字段同样拒绝
    let unknown_field = fn_app
        .registry()
        .dispatch(
            handle,
            privileged_context(&fn_app, serde_json::json!({"value": 1, "extra": true})),
        )
        .await;
    assert!(
        matches!(unknown_field, Err(BaseError::ParamInvalid(_, _))),
        "未知字段应返回 ParamInvalid: {unknown_field:?}"
    );
}

#[tokio::test]
async fn functional_action_keeps_permission_and_public_semantics() {
    let fn_app = functional_app();
    let handle = fn_app
        .registry()
        .resolve(&echo_ref())
        .expect("函数式 Action 应已注册");

    // 未认证请求被拒绝（非公开 Action）
    let denied = fn_app
        .registry()
        .dispatch(
            handle,
            fn_app.context(crate::action::Request::new(serde_json::json!({"value": 1}))),
        )
        .await;
    assert!(
        matches!(denied, Err(BaseError::Unauthorized(_))),
        "未认证请求应返回 Unauthorized: {denied:?}"
    );

    // permission_mode = any：拥有任一声明权限即可通过
    let granted = fn_app
        .registry()
        .dispatch(
            handle,
            privileged_context(&fn_app, serde_json::json!({"value": 1})),
        )
        .await
        .expect("具备权限的调用应成功");
    assert_eq!(granted.code, 0);

    // 权限不足同样拒绝
    let outsider = fn_app
        .context(crate::action::Request::new(serde_json::json!({"value": 1})))
        .with_user(User::new(2, "outsider"));
    let denied = fn_app.registry().dispatch(handle, outsider).await;
    assert!(
        matches!(denied, Err(BaseError::PermissionDenied(_))),
        "权限不足应返回 PermissionDenied: {denied:?}"
    );
}

#[tokio::test]
async fn functional_action_supports_typed_internal_call() {
    let fn_app = functional_app();
    let handle = fn_app
        .registry()
        .resolve_typed::<FnEchoInput, FnEchoOutput>(&echo_ref())
        .expect("函数式 Action 的强类型签名应在构建期可解析");
    let output = fn_app
        .registry()
        .call(
            handle,
            privileged_context(&fn_app, serde_json::Value::Null),
            FnEchoInput { value: 7 },
        )
        .await
        .expect("内部调用应直接传递 Rust 值，不经过 JSON");
    assert_eq!(output, FnEchoOutput { value: 7 });

    // 签名不匹配在解析期拒绝
    assert!(fn_app
        .registry()
        .resolve_typed::<FnEchoInput, serde_json::Value>(&echo_ref())
        .is_err());
}

/// 按 ActionRef 限定目标的计数中间件。
struct CountingMiddleware {
    target: ActionRef,
    hits: Arc<AtomicUsize>,
}

#[async_trait]
impl Middleware for CountingMiddleware {
    fn target_action(&self) -> Option<&ActionRef> {
        Some(&self.target)
    }

    async fn handle(&self, ctx: ActionContext, next: Next<'_>) -> Result<ApiResponse, BaseError> {
        self.hits.fetch_add(1, Ordering::SeqCst);
        next.run(ctx).await
    }
}

#[tokio::test]
async fn middleware_target_matching_covers_functional_actions() {
    async fn noop_handler(
        _ctx: ActionContext,
        input: FnEchoInput,
    ) -> Result<FnEchoOutput, BaseError> {
        Ok(FnEchoOutput { value: input.value })
    }

    let target = ActionRef::new(module_name("test.probe"), action_name("watched"));
    let hits = Arc::new(AtomicUsize::new(0));
    let module = ModuleSpec::new(module_name("test.probe"))
        .middleware(CountingMiddleware {
            target: target.clone(),
            hits: Arc::clone(&hits),
        })
        .action_fn(action_name("watched"), noop_handler)
        .public()
        .register()
        .action_fn(action_name("plain"), noop_handler)
        .public()
        .register();
    let app = build_app(module);

    // 非目标 Action：中间件不命中
    let plain = app
        .registry()
        .resolve(&ActionRef::new(
            module_name("test.probe"),
            action_name("plain"),
        ))
        .expect("plain Action 应已注册");
    app.dispatch(
        plain,
        crate::action::Request::new(serde_json::json!({"value": 1})),
    )
    .await
    .expect("plain dispatch 应成功");
    assert_eq!(
        hits.load(Ordering::SeqCst),
        0,
        "非目标 Action 不得命中中间件"
    );

    // 目标 Action：中间件按 ActionRef 命中
    let watched = app
        .registry()
        .resolve(&target)
        .expect("watched Action 应已注册");
    app.dispatch(
        watched,
        crate::action::Request::new(serde_json::json!({"value": 1})),
    )
    .await
    .expect("watched dispatch 应成功");
    assert_eq!(hits.load(Ordering::SeqCst), 1, "目标 Action 应命中中间件");
}
