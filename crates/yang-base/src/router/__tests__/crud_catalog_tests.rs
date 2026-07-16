use crate::action::{ActionContext, TypedHandler};
#[cfg(feature = "token")]
use crate::action::{GlobalTools, Request, User};
use crate::error::BaseError;
use crate::router::{Api, ModuleRouter};
#[cfg(feature = "openapi")]
use crate::router::{AppRouter, OpenApiInfo};
use crate::table::{Field, Table, TableDefinition};
#[cfg(feature = "token")]
use crate::token::TokenManager;
use async_trait::async_trait;
#[cfg(feature = "token")]
use jsonwebtoken::Algorithm;
use schemars::schema::RootSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[cfg(feature = "token")]
use std::sync::Arc;
use yang_base_derive::Action;

#[derive(Deserialize, schemars::JsonSchema)]
struct CustomInput {
    marker: bool,
}

#[derive(Serialize, schemars::JsonSchema)]
struct CustomOutput {
    accepted: bool,
}

#[derive(Action)]
#[action(name = "custom", display_name = "自定义接口")]
struct CustomAction;

#[async_trait]
impl TypedHandler for CustomAction {
    type Input = CustomInput;
    type Output = CustomOutput;

    async fn handle(
        &self,
        _ctx: ActionContext,
        input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Ok(CustomOutput {
            accepted: input.marker,
        })
    }
}

fn users_definition() -> TableDefinition {
    Table::new("users")
        .label("用户表")
        .fields([
            Field::id("id").filterable().sortable(),
            Field::string("username", 64)
                .required()
                .filterable()
                .sortable(),
            Field::integer("age").not_filterable().not_sortable(),
            Field::string("password_hash", 255).required().secret(),
            Field::created_at("created_at")
                .not_filterable()
                .not_sortable(),
        ])
        .build()
        .expect("users 表定义应有效")
}

fn audit_logs_definition() -> TableDefinition {
    Table::new("audit_logs")
        .label("审计日志")
        .fields([
            Field::id("log_id").filterable().sortable(),
            Field::string("event", 128).required().filterable(),
        ])
        .build()
        .expect("audit_logs 表定义应有效")
}

fn schema_value(schema: &RootSchema) -> Value {
    serde_json::to_value(schema).expect("RootSchema 应可序列化")
}

fn field_enum(schema: &Value) -> Vec<Value> {
    schema["properties"]["field"]["enum"]
        .as_array()
        .expect("field 应投影为 enum")
        .clone()
}

#[test]
fn crud_catalog_projects_the_bound_table_contract_for_all_builtin_actions() {
    let definition = users_definition();
    let expected_create = definition.input_schema();
    let expected_record = definition.output_schema();
    let module = ModuleRouter::new("users", "用户")
        .table(definition.clone())
        .api(Api::post("/api/users/custom", CustomAction))
        .expect("自定义 API 注册应成功")
        .crud()
        .expect("CRUD 注册应成功");

    let descriptor = module.descriptor().expect("descriptor 构建应成功");
    let action = |name: &str| {
        descriptor
            .actions
            .iter()
            .find(|action| action.name == name)
            .unwrap_or_else(|| panic!("应存在 {name} Action"))
    };

    let add_input = schema_value(&action("add").input_schema);
    assert_eq!(add_input, expected_create);
    for name in ["add", "put", "del"] {
        assert_eq!(action(name).permissions, vec!["users:write"]);
    }
    for name in ["get", "select", "table"] {
        assert_eq!(action(name).permissions, vec!["users:read"]);
    }
    assert_eq!(
        schema_value(&action("add").output_schema)["properties"]["id"]["format"],
        "uint64"
    );

    let put_input = schema_value(&action("put").input_schema);
    assert_eq!(put_input["properties"]["id"]["type"], "integer");
    assert_eq!(
        put_input["properties"]["data"]["properties"],
        expected_create["properties"]
    );
    assert!(put_input["properties"]["data"]["required"].is_null());
    assert_eq!(put_input["properties"]["data"]["minProperties"], 1);

    for name in ["del", "get"] {
        let input = schema_value(&action(name).input_schema);
        assert_eq!(input["properties"]["id"]["type"], "integer");
        assert_eq!(input["required"], json!(["id"]));
    }
    assert_eq!(schema_value(&action("get").output_schema), expected_record);

    let select_input = schema_value(&action("select").input_schema);
    let definitions = select_input["definitions"]
        .as_object()
        .expect("select schema 应包含 definitions");
    assert_eq!(
        field_enum(&definitions["OrderByItem"]),
        vec![json!("id"), json!("username")]
    );
    let where_schema = &definitions["WhereCondition"];
    let mut where_field_enums = Vec::new();
    collect_field_enums(where_schema, &mut where_field_enums);
    assert!(!where_field_enums.is_empty());
    assert!(where_field_enums
        .iter()
        .all(|values| values == &vec![json!("id"), json!("username")]));

    let select_output = schema_value(&action("select").output_schema);
    assert_eq!(
        select_output["properties"]["items"]["items"],
        expected_record
    );

    let table_output = schema_value(&action("table").output_schema);
    assert_eq!(table_output["properties"]["table_name"]["const"], "users");
    assert_eq!(table_output["properties"]["primary_key"]["const"], "id");
    assert_eq!(
        table_output["properties"]["input_schema"]["const"],
        expected_create
    );
    assert_eq!(
        table_output["properties"]["output_schema"]["const"],
        expected_record
    );

    assert_eq!(
        schema_value(&action("custom").input_schema),
        serde_json::to_value(schemars::schema_for!(CustomInput))
            .expect("自定义输入 schema 应可序列化")
    );
    assert_eq!(
        schema_value(&action("custom").output_schema),
        serde_json::to_value(schemars::schema_for!(CustomOutput))
            .expect("自定义输出 schema 应可序列化")
    );
}

#[test]
fn replacing_the_main_table_after_crud_reprojects_the_builtin_contract_atomically() {
    let replacement = audit_logs_definition();
    let expected_input = replacement.input_schema();
    let module = ModuleRouter::new("users", "用户")
        .table(users_definition())
        .crud()
        .expect("CRUD 注册应成功")
        .table(replacement);

    let descriptor = module.descriptor().expect("descriptor 构建应成功");
    let add = descriptor
        .actions
        .iter()
        .find(|action| action.name == "add")
        .expect("应存在 add Action");
    let table = descriptor
        .actions
        .iter()
        .find(|action| action.name == "table")
        .expect("应存在 table Action");

    assert_eq!(schema_value(&add.input_schema), expected_input);
    assert_eq!(add.permissions, vec!["users:write"]);
    assert_eq!(
        schema_value(&table.output_schema)["properties"]["table_name"]["const"],
        "audit_logs"
    );
    assert_eq!(table.permissions, vec!["users:read"]);
    assert_eq!(
        module
            .table_definition()
            .expect("替换后的主表应存在")
            .name(),
        "audit_logs"
    );
}

#[cfg(feature = "token")]
fn test_tools() -> Arc<GlobalTools> {
    Arc::new(GlobalTools::new(TokenManager::new_symmetric(
        "crud_catalog_test_secret",
        Algorithm::HS256,
        "test_issuer".to_string(),
        "test_audience".to_string(),
        3600,
        86400,
    )))
}

#[cfg(feature = "token")]
#[tokio::test]
async fn crud_runtime_authorization_uses_the_same_generated_permissions_as_catalog() {
    let router = ModuleRouter::new("users", "用户")
        .table(users_definition())
        .crud()
        .expect("CRUD 注册应成功");

    let no_permissions =
        ActionContext::new(Request::new(json!({})), test_tools()).with_user(User::new(1, "alice"));
    assert!(matches!(
        router.dispatch("table", no_permissions).await,
        Err(BaseError::PermissionDenied(message)) if message.contains("users:read")
    ));

    let read_context = ActionContext::new(Request::new(json!({})), test_tools())
        .with_user(User::new(1, "alice").with_permissions(["users:read"]));
    assert!(router.dispatch("table", read_context).await.is_ok());

    let read_only_context =
        ActionContext::new(Request::new(json!({ "username": "alice" })), test_tools())
            .with_user(User::new(1, "alice").with_permissions(["users:read"]));
    assert!(matches!(
        router.dispatch("add", read_only_context).await,
        Err(BaseError::PermissionDenied(message)) if message.contains("users:write")
    ));
}

fn collect_field_enums(schema: &Value, enums: &mut Vec<Vec<Value>>) {
    match schema {
        Value::Object(object) => {
            if let Some(field_schema) = object
                .get("properties")
                .and_then(Value::as_object)
                .and_then(|properties| properties.get("field"))
            {
                enums.push(field_enum(
                    &json!({ "properties": { "field": field_schema } }),
                ));
            }
            for value in object.values() {
                collect_field_enums(value, enums);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_field_enums(value, enums);
            }
        }
        _ => {}
    }
}

#[cfg(feature = "openapi")]
#[test]
fn crud_openapi_uses_the_projected_table_contract() {
    let module = ModuleRouter::new("users", "用户")
        .table(users_definition())
        .crud()
        .expect("CRUD 注册应成功");
    let document = AppRouter::new()
        .module(module)
        .expect("模块注册应成功")
        .catalog()
        .expect("Catalog 构建应成功")
        .to_openapi(OpenApiInfo::new("YANG API", "0.2.0"))
        .expect("OpenAPI 投影应成功");

    let add = &document["paths"]["/api/users"]["post"];
    assert!(
        add["requestBody"]["content"]["application/json"]["schema"]["properties"]["username"]
            .is_object()
    );
    assert!(
        add["requestBody"]["content"]["application/json"]["schema"]["properties"]["password_hash"]
            .is_null()
    );

    let get_data = &document["paths"]["/api/users"]["get"]["responses"]["200"]["content"]
        ["application/json"]["schema"]["properties"]["data"];
    assert!(get_data["properties"]["username"].is_object());
    assert!(get_data["properties"]["password_hash"].is_null());

    let select_items = &document["paths"]["/api/users/query"]["post"]["responses"]["200"]
        ["content"]["application/json"]["schema"]["properties"]["data"]["properties"]["items"]
        ["items"];
    assert!(select_items["properties"]["username"].is_object());

    let table_data = &document["paths"]["/api/users/schema"]["get"]["responses"]["200"]["content"]
        ["application/json"]["schema"]["properties"]["data"];
    assert_eq!(table_data["properties"]["table_name"]["const"], "users");
}
