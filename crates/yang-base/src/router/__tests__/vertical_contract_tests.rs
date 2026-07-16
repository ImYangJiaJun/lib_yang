use crate::action::{
    ActionContext, ApiResponse, GlobalTools, Request, RequestMeta, TypedHandler, User,
};
use crate::error::BaseError;
use crate::plugin::{Plugin, PluginManagerBuilder};
use crate::router::{Api, AppRouter, Middleware, ModuleRouter, Next, OpenApiInfo};
use crate::table::{Field, Table, TableDefinition};
use crate::token::TokenManager;
use async_trait::async_trait;
use jsonwebtoken::Algorithm;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use yang_base_derive::Action;

struct DirectoryPlugin;

#[async_trait]
impl Plugin for DirectoryPlugin {
    fn name(&self) -> &str {
        "directory"
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
struct LookupInput {
    query: String,
}

#[derive(Serialize, schemars::JsonSchema)]
struct LookupOutput {
    query: String,
    table_query_ready: bool,
    method: Option<String>,
}

#[derive(Action)]
#[action(
    name = "lookup",
    display_name = "目录查询",
    description = "验证纵向契约",
    permissions("directory:read")
)]
struct LookupAction;

#[async_trait]
impl TypedHandler for LookupAction {
    type Input = LookupInput;
    type Output = LookupOutput;

    async fn handle(
        &self,
        ctx: ActionContext,
        input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        let _query = ctx.table_query()?;
        Ok(LookupOutput {
            query: input.query,
            table_query_ready: true,
            method: ctx.request_meta.method,
        })
    }
}

struct TestAuthMiddleware;

#[async_trait]
impl Middleware for TestAuthMiddleware {
    async fn handle(&self, ctx: ActionContext, next: Next<'_>) -> Result<ApiResponse, BaseError> {
        let user = User::new(1, "tester").with_permissions(["directory:read"]);
        next.run(ctx.with_user(user)).await
    }
}

fn test_tools() -> Arc<GlobalTools> {
    Arc::new(GlobalTools::new(TokenManager::new_symmetric(
        "vertical-contract-secret",
        Algorithm::HS256,
        "issuer".to_string(),
        "audience".to_string(),
        3600,
        7200,
    )))
}

fn directory_table() -> TableDefinition {
    Table::new("directory_entries")
        .fields([Field::id("id"), Field::string("name", 255).required()])
        .build()
        .expect("目录表定义应有效")
}

fn directory_module(with_auth: bool) -> ModuleRouter {
    let module = ModuleRouter::new("directory", "目录").table(directory_table());
    let module = if with_auth {
        module.middleware(TestAuthMiddleware)
    } else {
        module
    };
    module
        .api(Api::post("/directory/lookup", LookupAction).tag("directory"))
        .expect("API 注册应成功")
}

#[tokio::test]
async fn plugin_to_openapi_vertical_contract_without_database() {
    let mut plugins = PluginManagerBuilder::new();
    plugins
        .register(DirectoryPlugin)
        .await
        .expect("插件注册应成功");
    let registry = plugins.build().expect("插件 registry 构建应成功");
    assert!(registry.get("directory").is_some());

    let app = AppRouter::new()
        .module(directory_module(true))
        .expect("模块注册应成功");
    let context = ActionContext::new(
        Request::new(json!({ "query": "alice" }))
            .header("User-Agent", "vertical-contract-adapter/1.0"),
        test_tools(),
    )
    .with_request_meta(
        RequestMeta::new()
            .with_method("POST")
            .with_original_uri("https://api.example.test/directory/lookup")
            .with_scheme("https")
            .with_peer_addr("203.0.113.10:43120".parse().expect("peer 地址应合法"))
            .with_local_addr("10.0.0.8:443".parse().expect("local 地址应合法")),
    );

    let response = app
        .dispatch("directory", "lookup", context)
        .await
        .expect("完整派发应成功");
    assert_eq!(response.code, 0);
    assert_eq!(
        response
            .data
            .as_ref()
            .and_then(|data| data["query"].as_str()),
        Some("alice")
    );
    assert_eq!(
        response
            .data
            .as_ref()
            .and_then(|data| data["table_query_ready"].as_bool()),
        Some(true)
    );

    let catalog = app.catalog().expect("catalog 应成功");
    let openapi = catalog
        .to_openapi(OpenApiInfo::new("Directory API", "1.0.0"))
        .expect("OpenAPI 应成功");
    let runtime_input_schema = serde_json::to_value(&catalog.modules[0].actions[0].input_schema)
        .expect("运行时 Schema 应可序列化");
    assert_eq!(
        openapi["paths"]["/directory/lookup"]["post"]["operationId"],
        "directory.lookup"
    );
    assert_eq!(
        openapi["paths"]["/directory/lookup"]["post"]["x-permissions"],
        json!(["directory:read"])
    );
    assert_eq!(
        openapi["paths"]["/directory/lookup"]["post"]["requestBody"]["content"]["application/json"]
            ["schema"],
        runtime_input_schema
    );
}

#[tokio::test]
async fn vertical_contract_rejects_duplicate_plugin_unauthorized_and_invalid_input() {
    let mut plugins = PluginManagerBuilder::new();
    plugins
        .register(DirectoryPlugin)
        .await
        .expect("首次插件注册应成功");
    assert!(matches!(
        plugins.register(DirectoryPlugin).await,
        Err(BaseError::PluginAlreadyRegistered(name)) if name == "directory"
    ));

    let app = AppRouter::new()
        .module(directory_module(false))
        .expect("模块注册应成功");
    let context = ActionContext::new(Request::new(json!({ "query": "alice" })), test_tools());
    assert!(matches!(
        app.dispatch("directory", "lookup", context).await,
        Err(BaseError::Unauthorized(_))
    ));

    let app = AppRouter::new()
        .module(directory_module(true))
        .expect("模块注册应成功");
    let context = ActionContext::new(Request::new(json!({ "query": 42 })), test_tools());
    assert!(matches!(
        app.dispatch("directory", "lookup", context).await,
        Err(BaseError::ParamInvalid(field, _)) if field == "body"
    ));
}
