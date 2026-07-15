use crate::action::{ActionContext, TypedHandler};
use crate::error::BaseError;
use crate::router::{AppRouter, ModuleRouter, RouteDescriptor};
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

fn search_route(path: &str, operation_id: &str) -> RouteDescriptor {
    RouteDescriptor::new("POST", path, operation_id).expect("测试 route 应合法")
}

#[test]
fn module_descriptor_merges_route_and_action_meta() {
    let module = ModuleRouter::new("users", "用户")
        .register_action(SearchAction)
        .expect("Action 注册应成功")
        .register_route("search", search_route("/users/search", "users.search"))
        .expect("route 注册应成功");

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
fn descriptor_rejects_action_without_route() {
    let module = ModuleRouter::new("users", "用户")
        .register_action(SearchAction)
        .expect("Action 注册应成功");

    assert!(matches!(
        module.descriptor(),
        Err(BaseError::ConfigError(message)) if message.contains("search") && message.contains("route")
    ));
}

#[test]
fn route_registration_rejects_duplicate_route_and_operation() {
    let module = ModuleRouter::new("users", "用户")
        .register_action(SearchAction)
        .expect("search 注册应成功")
        .register_action(HealthAction)
        .expect("health 注册应成功")
        .register_route("search", search_route("/shared", "shared.operation"))
        .expect("首个 route 应成功");

    let duplicate_route = module.register_route(
        "health",
        RouteDescriptor::new("POST", "/shared", "health.operation").expect("route 应合法"),
    );
    assert!(matches!(
        duplicate_route,
        Err(BaseError::ConfigError(message)) if message.contains("route 冲突")
    ));

    let duplicate_operation = ModuleRouter::new("users", "用户")
        .register_action(SearchAction)
        .expect("search 注册应成功")
        .register_action(HealthAction)
        .expect("health 注册应成功")
        .register_route("search", search_route("/search", "shared.operation"))
        .expect("首个 route 应成功")
        .register_route(
            "health",
            RouteDescriptor::new("GET", "/health", "shared.operation").expect("route 应合法"),
        );
    assert!(matches!(
        duplicate_operation,
        Err(BaseError::ConfigError(message)) if message.contains("operation_id 冲突")
    ));
}

#[test]
fn route_registration_revalidates_mutated_public_descriptor() {
    let mut route = search_route("/users/search", "users.search");
    route.path = "/users?token=secret".to_string();
    let result = ModuleRouter::new("users", "用户")
        .register_action(SearchAction)
        .expect("Action 注册应成功")
        .register_route("search", route);

    assert!(matches!(result, Err(BaseError::ConfigError(message)) if message.contains("path")));
}

#[test]
fn app_catalog_is_sorted_and_rejects_cross_module_conflicts() {
    let zeta = ModuleRouter::new("zeta", "Z")
        .register_action(SearchAction)
        .expect("Action 注册应成功")
        .register_route("search", search_route("/zeta/search", "zeta.search"))
        .expect("route 注册应成功");
    let alpha = ModuleRouter::new("alpha", "A")
        .register_action(HealthAction)
        .expect("Action 注册应成功")
        .register_route(
            "health",
            RouteDescriptor::new("GET", "/health", "alpha.health").expect("route 应合法"),
        )
        .expect("route 注册应成功");
    let app = AppRouter::new()
        .register_module(zeta)
        .expect("zeta 注册应成功")
        .register_module(alpha)
        .expect("alpha 注册应成功");

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
        .register_action(SearchAction)
        .expect("Action 注册应成功")
        .register_route("search", search_route("/shared", "one.search"))
        .expect("route 注册应成功");
    let two = ModuleRouter::new("two", "Two")
        .register_action(SearchAction)
        .expect("Action 注册应成功")
        .register_route("search", search_route("/shared", "two.search"))
        .expect("route 注册应成功");
    let conflicted = AppRouter::new()
        .register_module(one)
        .expect("one 注册应成功")
        .register_module(two)
        .expect("two 注册应成功");

    assert!(matches!(
        conflicted.catalog(),
        Err(BaseError::ConfigError(message)) if message.contains("route 冲突")
    ));

    let first = ModuleRouter::new("first", "First")
        .register_action(SearchAction)
        .expect("Action 注册应成功")
        .register_route("search", search_route("/first", "shared.operation"))
        .expect("route 注册应成功");
    let second = ModuleRouter::new("second", "Second")
        .register_action(SearchAction)
        .expect("Action 注册应成功")
        .register_route("search", search_route("/second", "shared.operation"))
        .expect("route 注册应成功");
    let conflicted = AppRouter::new()
        .register_module(first)
        .expect("first 注册应成功")
        .register_module(second)
        .expect("second 注册应成功");
    assert!(matches!(
        conflicted.catalog(),
        Err(BaseError::ConfigError(message)) if message.contains("operation_id 冲突")
    ));
}
