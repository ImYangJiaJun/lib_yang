use crate::action::PermissionMode;
use crate::router::{ActionDescriptor, ApiCatalog, ModuleDescriptor, OpenApiInfo, RouteDescriptor};
use schemars::schema_for;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, schemars::JsonSchema)]
struct SearchInput {
    query: String,
}

#[derive(Serialize, schemars::JsonSchema)]
struct SearchOutput {
    total: u64,
}

fn action(name: &str, method: &str, is_public: bool) -> ActionDescriptor {
    ActionDescriptor {
        name: name.to_string(),
        display_name: format!("{name} display"),
        description: format!("{name} description"),
        permissions: if is_public {
            Vec::new()
        } else {
            vec!["user:read".to_string()]
        },
        permission_mode: PermissionMode::All,
        is_public,
        input_schema: schema_for!(SearchInput),
        output_schema: schema_for!(SearchOutput),
        route: RouteDescriptor::new(method, "/users", format!("users.{name}"))
            .expect("route 应合法")
            .with_tags(vec!["users".to_string()])
            .expect("tag 应合法"),
    }
}

#[test]
fn openapi_projection_maps_security_schema_and_errors() {
    assert_eq!(SearchInput { query: "q".into() }.query, "q");
    let catalog = ApiCatalog {
        modules: vec![ModuleDescriptor {
            name: "users".to_string(),
            display_name: "用户".to_string(),
            default_permissions: Vec::new(),
            default_permission_mode: PermissionMode::All,
            actions: vec![action("create", "POST", false), action("list", "GET", true)],
        }],
    };

    let document = catalog
        .to_openapi(OpenApiInfo::new("YANG API", "1.0.0"))
        .expect("OpenAPI 投影应成功");

    assert_eq!(document["openapi"], "3.1.0");
    assert_eq!(
        document["paths"]["/users"]["post"]["operationId"],
        "users.create"
    );
    assert_eq!(
        document["paths"]["/users"]["post"]["security"][0]["bearerAuth"],
        serde_json::json!([])
    );
    assert_eq!(
        document["paths"]["/users"]["get"]["security"],
        serde_json::json!([])
    );
    assert_eq!(
        document["paths"]["/users"]["post"]["x-permissions"],
        serde_json::json!(["user:read"])
    );
    assert!(
        document["paths"]["/users"]["post"]["requestBody"]["content"]["application/json"]["schema"]
            .is_object()
    );
    assert!(
        document["paths"]["/users"]["post"]["responses"]["200"]["content"]["application/json"]
            ["schema"]["properties"]["data"]
            .is_object()
    );
    for status in ["400", "401", "403", "500"] {
        assert!(document["paths"]["/users"]["post"]["responses"][status].is_object());
    }
    insta::assert_json_snapshot!("api_catalog_openapi", document);
}

#[test]
fn openapi_projection_rejects_mutated_catalog_conflicts() {
    let mut unsupported = action("connect", "POST", false);
    unsupported.route.method = "CONNECT".to_string();
    let catalog = ApiCatalog {
        modules: vec![ModuleDescriptor {
            name: "users".to_string(),
            display_name: "用户".to_string(),
            default_permissions: Vec::new(),
            default_permission_mode: PermissionMode::All,
            actions: vec![unsupported],
        }],
    };
    assert!(catalog
        .to_openapi(OpenApiInfo::new("YANG API", "1.0.0"))
        .is_err());

    let mut duplicate = action("list", "GET", true);
    duplicate.route.operation_id = "users.create".to_string();
    let catalog = ApiCatalog {
        modules: vec![ModuleDescriptor {
            name: "users".to_string(),
            display_name: "用户".to_string(),
            default_permissions: Vec::new(),
            default_permission_mode: PermissionMode::All,
            actions: vec![action("create", "POST", false), duplicate],
        }],
    };
    assert!(catalog
        .to_openapi(OpenApiInfo::new("YANG API", "1.0.0"))
        .is_err());
}
