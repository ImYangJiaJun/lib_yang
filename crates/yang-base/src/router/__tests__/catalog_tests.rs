use crate::action::{ActionContext, TypedHandler};
use crate::error::BaseError;
use crate::router::{Api, AppRouter, ModuleRouter};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use yang_base_derive::Action;

#[derive(Deserialize, schemars::JsonSchema)]
struct CatalogInput {
    query: String,
}

#[derive(Serialize, schemars::JsonSchema)]
struct CatalogOutput {
    total: u64,
}

#[derive(Action)]
#[action(
    name = "search",
    display_name = "搜索",
    description = "搜索用户",
    permissions("user:read")
)]
struct SearchAction;

#[async_trait]
impl TypedHandler for SearchAction {
    type Input = CatalogInput;
    type Output = CatalogOutput;

    async fn handle(
        &self,
        _ctx: ActionContext,
        input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        let _ = input.query;
        Ok(CatalogOutput { total: 0 })
    }
}

#[derive(Action)]
#[action(name = "health", public, display_name = "健康检查")]
struct HealthAction;

#[async_trait]
impl TypedHandler for HealthAction {
    type Input = CatalogInput;
    type Output = CatalogOutput;

    async fn handle(
        &self,
        _ctx: ActionContext,
        _input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Ok(CatalogOutput { total: 0 })
    }
}

#[test]
fn module_descriptor_merges_route_and_action_meta() {
    let module = ModuleRouter::new("users", "用户")
        .api(Api::post("/users/search", SearchAction))
        .expect("API 注册应成功");

    let descriptor = module.descriptor().expect("descriptor 构建应成功");
    assert_eq!(descriptor.name, "users");
    assert_eq!(descriptor.actions.len(), 1);
    let action = &descriptor.actions[0];
    assert_eq!(action.name, "search");
    assert_eq!(action.route.method, "POST");
    assert_eq!(action.route.path, "/users/search");
    assert_eq!(action.route.operation_id, "users.search");
    assert_eq!(action.permissions, vec!["user:read"]);
    assert!(!action.is_public);
    assert!(action.input_schema.schema.metadata.is_some());
    assert!(action.output_schema.schema.metadata.is_some());
}

#[test]
fn api_rejects_invalid_route_atomically() {
    let result =
        ModuleRouter::new("users", "用户").api(Api::post("users?token=secret", SearchAction));
    assert!(matches!(
        result,
        Err(BaseError::ConfigError(message)) if message.contains("path")
    ));
}

#[test]
fn api_rejects_paths_that_axum_would_panic_on() {
    for path in [
        "/users/:id",
        "/users/*rest",
        "/users/{id",
        "/users/{*rest}/tail",
    ] {
        let result = ModuleRouter::new("users", "用户").api(Api::get(path, HealthAction));
        assert!(
            matches!(result, Err(BaseError::ConfigError(message)) if message.contains("path")),
            "非法路径应在注册期拒绝: {path}"
        );
    }
}

#[test]
fn route_registration_rejects_duplicate_route_and_operation() {
    let module = ModuleRouter::new("users", "用户")
        .api(Api::post("/shared", SearchAction).operation_id("shared.operation"))
        .expect("首个 API 应成功");

    let duplicate_route =
        module.api(Api::post("/shared", HealthAction).operation_id("health.operation"));
    assert!(matches!(
        duplicate_route,
        Err(BaseError::ConfigError(message)) if message.contains("route 冲突")
    ));

    let duplicate_operation = ModuleRouter::new("users", "用户")
        .api(Api::post("/search", SearchAction).operation_id("shared.operation"))
        .expect("首个 API 应成功")
        .api(Api::get("/health", HealthAction).operation_id("shared.operation"));
    assert!(matches!(
        duplicate_operation,
        Err(BaseError::ConfigError(message)) if message.contains("operation_id 冲突")
    ));
}

#[test]
fn route_registration_matches_axum_template_conflict_semantics() {
    let semantic_conflict = ModuleRouter::new("users", "用户")
        .api(Api::get("/users/{id}", SearchAction))
        .expect("首个动态路由应成功")
        .api(Api::post("/users/{name}", HealthAction));
    assert!(matches!(
        semantic_conflict,
        Err(BaseError::ConfigError(message)) if message.contains("route 冲突")
    ));

    let same_path_different_methods = ModuleRouter::new("users", "用户")
        .api(Api::get("/users/{id}", SearchAction))
        .expect("GET 路由应成功")
        .api(Api::post("/users/{id}", HealthAction));
    assert!(same_path_different_methods.is_ok());
}

#[test]
fn api_rejects_invalid_status() {
    let result = ModuleRouter::new("users", "用户")
        .api(Api::post("/users/search", SearchAction).status(700));

    assert!(matches!(result, Err(BaseError::ConfigError(message)) if message.contains("status")));
}

#[test]
fn app_catalog_is_sorted_and_rejects_cross_module_conflicts() {
    let zeta = ModuleRouter::new("zeta", "Z")
        .api(Api::post("/zeta/search", SearchAction))
        .expect("API 注册应成功");
    let alpha = ModuleRouter::new("alpha", "A")
        .api(Api::get("/health", HealthAction))
        .expect("API 注册应成功");
    let app = AppRouter::new()
        .modules([zeta, alpha])
        .expect("模块注册应成功");

    let catalog = app.catalog().expect("catalog 构建应成功");
    assert_eq!(
        catalog
            .modules
            .iter()
            .map(|module| module.name.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha", "zeta"]
    );

    let one = ModuleRouter::new("one", "One")
        .api(Api::post("/shared", SearchAction))
        .expect("API 注册应成功");
    let two = ModuleRouter::new("two", "Two")
        .api(Api::post("/shared", SearchAction))
        .expect("API 注册应成功");
    let conflicted = AppRouter::new()
        .modules([one, two])
        .expect("模块注册应成功");

    assert!(matches!(
        conflicted.catalog(),
        Err(BaseError::ConfigError(message)) if message.contains("route 冲突")
    ));

    let first = ModuleRouter::new("first", "First")
        .api(Api::post("/first", SearchAction).operation_id("shared.operation"))
        .expect("API 注册应成功");
    let second = ModuleRouter::new("second", "Second")
        .api(Api::post("/second", SearchAction).operation_id("shared.operation"))
        .expect("API 注册应成功");
    let conflicted = AppRouter::new()
        .modules([first, second])
        .expect("模块注册应成功");
    assert!(matches!(
        conflicted.catalog(),
        Err(BaseError::ConfigError(message)) if message.contains("operation_id 冲突")
    ));
}

#[test]
fn app_catalog_rejects_cross_module_semantic_route_conflicts() {
    let one = ModuleRouter::new("one", "One")
        .api(Api::get("/users/{id}", SearchAction))
        .expect("首个模块路由应成功");
    let two = ModuleRouter::new("two", "Two")
        .api(Api::post("/users/{name}", SearchAction))
        .expect("单模块内路由应成功");
    let app = AppRouter::new()
        .modules([one, two])
        .expect("模块注册应成功");

    assert!(matches!(
        app.catalog(),
        Err(BaseError::ConfigError(message)) if message.contains("route 冲突")
    ));
}
