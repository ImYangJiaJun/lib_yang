use super::*;
use crate::action::{ActionContext, TypedHandler};
use crate::error::BaseError;
use crate::tools::{Tools, ToolsBuilder};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
struct NoopInput {}

#[derive(Debug, Serialize, JsonSchema)]
struct NoopOutput {}

#[derive(Debug, Deserialize, JsonSchema)]
struct EchoInput {
    value: i64,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
struct EchoOutput {
    value: i64,
}

crate::params! {
    #[deny_unknown_fields]
pub CreateUserInput {
        username: Str::new().require(true).max_length(64),
        status: Radio::<i8>::new().require(true).options([(1, "启用"), (0, "禁用")]),
        note: Text::new(),
    }
}

crate::params! {
    #[deny_unknown_fields]
    MultiSourceInput {
        title: Str::new().require(true),
        #[param(source = query)]
        page: Int::new().require(true),
        #[param(source = path)]
        user_id: Key::new().require(true),
        #[param(source = header)]
        x_trace_id: Str::new().require(true),
    }
}

#[derive(crate::Action)]
#[action(
    name = "create_user",
    display_name = "创建用户",
    method = "POST",
    path = "/native/users",
    success_status = 201,
    public
)]
struct CreateUserAction;

#[async_trait]
impl crate::action::Action for CreateUserAction {
    type Input = CreateUserInput;
    type Output = EchoOutput;

    async fn index(
        &self,
        _ctx: ActionContext,
        input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Ok(EchoOutput {
            value: i64::from(input.status),
        })
    }
}

#[derive(crate::Action)]
#[action(name = "linked_create", path = "/native/users/linked")]
struct LinkedCreateAction {
    link: ActionLink<CreateUserInput, EchoOutput>,
}

#[async_trait]
impl crate::action::Action for LinkedCreateAction {
    type Input = CreateUserInput;
    type Output = EchoOutput;

    fn calls(&self) -> Vec<ActionRef> {
        vec![self.link.reference().clone()]
    }

    fn bind_registry(&self, registry: &Registry) -> Result<(), BaseError> {
        self.link.bind(registry)
    }

    async fn index(
        &self,
        ctx: ActionContext,
        input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        ctx.plugins()?.api_run(self.link.handle()?, input).await
    }
}

#[derive(crate::Action)]
#[action(name = "noop", public)]
struct NoopAction;

#[derive(crate::Action)]
#[action(
    name = "upload",
    request_media = "multipart",
    content_types("application/pdf"),
    max_files = 1,
    max_file_bytes = 1024,
    max_total_bytes = 2048
)]
struct MultipartNoopAction;

#[derive(crate::Action)]
#[action(name = "echo", public)]
struct EchoAction;

#[async_trait]
impl TypedHandler for NoopAction {
    type Input = NoopInput;
    type Output = NoopOutput;

    async fn handle(
        &self,
        _ctx: ActionContext,
        _input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Ok(NoopOutput {})
    }
}

#[async_trait]
impl crate::action::Action for MultipartNoopAction {
    type Input = CreateUserInput;
    type Output = EchoOutput;

    async fn index(
        &self,
        _ctx: ActionContext,
        input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Ok(EchoOutput {
            value: i64::from(input.status),
        })
    }
}

#[async_trait]
impl TypedHandler for EchoAction {
    type Input = EchoInput;
    type Output = EchoOutput;

    async fn handle(
        &self,
        _ctx: ActionContext,
        input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Ok(EchoOutput { value: input.value })
    }
}

fn test_tools() -> Tools {
    ToolsBuilder::new().build().expect("测试 Tools 应构建成功")
}

fn addon(value: &str) -> AddonName {
    AddonName::new(value).expect("测试 Addon 名称应有效")
}

fn module(value: &str) -> ModuleName {
    ModuleName::new(value).expect("测试 Module 名称应有效")
}

fn action(value: &str) -> ActionName {
    ActionName::new(value).expect("测试 Action 名称应有效")
}

fn table(value: &str) -> TableName {
    TableName::new(value).expect("测试 Table 名称应有效")
}

fn field(value: &str) -> FieldName {
    FieldName::new(value).expect("测试 Field 名称应有效")
}

fn view(value: &str) -> ViewName {
    ViewName::new(value).expect("测试 View 名称应有效")
}

fn action_ref(module_name: &str, action_name: &str) -> ActionRef {
    ActionRef::new(module(module_name), action(action_name))
}

fn field_ref(table_name: &str, field_name: &str) -> FieldRef {
    FieldRef::new(table(table_name), field(field_name))
}

#[test]
fn fields_macro_builds_the_native_field_collection() {
    let fields = crate::fields! {
        id => Key::new().title("ID"),
        username => Str::new().require(true).max_length(64),
        amount => Decimal::new().precision(18, 2),
    };
    assert_eq!(fields.as_slice().len(), 3);
    assert_eq!(fields.as_slice()[1].name.as_str(), "username");
    assert_eq!(fields.as_slice()[2].kind, FieldKind::Decimal);
}

#[test]
fn params_macro_generates_typed_input_and_native_params() {
    let input: CreateUserInput = serde_json::from_value(serde_json::json!({
        "username": "alice",
        "status": 1
    }))
    .expect("强类型参数应只反序列化一次");
    assert_eq!(input.username, "alice");
    assert_eq!(input.status, 1);
    assert!(input.note.is_none());
    let params = <CreateUserInput as ParamInput>::params();
    assert_eq!(params.as_slice().len(), 3);
    assert!(params.as_slice()[0].required);
    assert_eq!(params.as_slice()[2].source, ParamSource::Body);
    assert_eq!(
        <CreateUserAction as crate::action::Action>::params()
            .as_slice()
            .len(),
        3
    );
}

#[test]
fn name_and_reference_macros_build_controlled_native_types() {
    assert_eq!(crate::addon!("account").as_str(), "account");
    assert_eq!(crate::module!("account.user").as_str(), "account.user");
    assert_eq!(crate::table!("users").as_str(), "users");
    assert_eq!(crate::field!("users.id").to_string(), "users.id");
    assert_eq!(
        crate::action!("account.user.login").to_string(),
        "account.user.login"
    );
    assert_eq!(
        crate::view!("account.user.default").to_string(),
        "account.user.default"
    );
    const KEY: crate::action::ContextKey<i64> = crate::context_key!("tenant_id");
    assert_eq!(KEY.name(), "tenant_id");
}

#[test]
fn params_macro_decodes_body_query_path_and_header_from_one_definition() {
    let mut request = crate::action::Request::new(serde_json::json!({"title": "hello"}))
        .query("page", "3")
        .path_param("user_id", "42")
        .header("X-Trace-Id", "trace-1");

    let input = <MultiSourceInput as ParamInput>::decode(&mut request)
        .expect("四种来源应按 ParamSpec 合并为强类型 Input");
    assert_eq!(input.title, "hello");
    assert_eq!(input.page, 3);
    assert_eq!(input.user_id, 42);
    assert_eq!(input.x_trace_id, "trace-1");

    let params = <MultiSourceInput as ParamInput>::params();
    assert_eq!(params.as_slice()[0].source, ParamSource::Body);
    assert_eq!(params.as_slice()[1].source, ParamSource::Query);
    assert_eq!(params.as_slice()[2].source, ParamSource::Path);
    assert_eq!(params.as_slice()[3].source, ParamSource::Header);
}

fn get_action(name: &str, path: &str, operation_id: &str) -> ActionSpec {
    ActionSpec::new(
        action(name),
        RouteSpec::new(HttpMethod::Get, path, operation_id),
    )
    .public(true)
}

struct NativeUserModule;

impl Module for NativeUserModule {
    fn name(&self) -> ModuleName {
        module("native.user")
    }

    fn table(&self) -> Option<TableName> {
        Some(table("native_user"))
    }

    fn fields(&self) -> Fields {
        crate::fields! {
            id => Key::new(),
            username => Str::new().require(true).max_length(64),
        }
    }

    fn actions(&self) -> Actions {
        crate::actions![CreateUserAction]
    }
}

struct NativeAddon;

impl Addon for NativeAddon {
    fn name(&self) -> AddonName {
        addon("native")
    }

    fn modules(&self) -> Modules {
        crate::modules![NativeUserModule]
    }
}

#[test]
fn addon_and_module_traits_feed_the_only_app_builder_path() {
    let app = AppBuilder::new()
        .addon(NativeAddon)
        .build(test_tools())
        .expect("原生 Addon/Module 应直接构建");
    assert_eq!(app.catalog().addons()[0].name.as_str(), "native");
    assert_eq!(app.table_definitions()[0].name(), "native_user");
    assert_eq!(app.compiled_views()[0].name().as_str(), "default");
    let action = &app.catalog().addons()[0].modules[0].actions()[0];
    assert_eq!(action.route.path, "/native/users");
    assert_eq!(action.success_status, 201);
    assert_eq!(action.params.len(), 3);
}

#[test]
fn native_multipart_contract_reaches_catalog_and_ui_projection() {
    let app = AppBuilder::new()
        .addon(
            AddonSpec::new(addon("upload"))
                .module(ModuleSpec::new(module("upload.file")).native_action(MultipartNoopAction)),
        )
        .build(test_tools())
        .expect("合法 multipart Action 应构建成功");
    let action = &app.catalog().addons()[0].modules[0].actions()[0];
    assert_eq!(action.request_media_type, ActionMediaType::Multipart);
    let multipart = action.multipart.as_ref().expect("Catalog 应保留上传限制");
    assert_eq!(multipart.max_files, 1);
    assert_eq!(multipart.max_file_bytes, 1024);
    assert_eq!(multipart.max_total_bytes, 2048);
    assert_eq!(multipart.allowed_content_types, ["application/pdf"]);

    let catalog = app
        .ui_catalog(
            &app.context(crate::action::Request::new(serde_json::Value::Null))
                .with_user(crate::action::User::new(1, "uploader")),
        )
        .expect("multipart UI Catalog revision 应可计算");
    assert_eq!(
        catalog.actions[0].request_media_type,
        ActionMediaType::Multipart
    );
    assert_eq!(
        catalog.actions[0]
            .multipart
            .as_ref()
            .map(|spec| spec.lifecycle),
        Some(UploadLifecycle::RequestScoped)
    );
}

#[test]
fn app_builder_rejects_unsafe_or_inconsistent_multipart_contracts() {
    let build = |action: ActionSpec| {
        AppBuilder::new()
            .addon(
                AddonSpec::new(addon("upload"))
                    .module(ModuleSpec::new(module("upload.file")).action(action, NoopAction)),
            )
            .build(test_tools())
    };
    let post = || {
        ActionSpec::new(
            action("upload"),
            RouteSpec::new(HttpMethod::Post, "/upload", "upload.file.upload"),
        )
    };
    let valid = || MultipartSpec::new(["application/pdf"]);

    let mut json_with_limits = post();
    json_with_limits.multipart = Some(valid());
    let mut missing_limits = post();
    missing_limits.request_media_type = ActionMediaType::Multipart;
    let get_upload = ActionSpec::new(
        action("upload"),
        RouteSpec::new(HttpMethod::Get, "/upload", "upload.file.upload"),
    )
    .multipart(valid());
    let zero_files = post().multipart(valid().max_files(0));
    let oversized_file = post().multipart(valid().max_file_bytes(4096).max_total_bytes(2048));
    let wildcard = post().multipart(MultipartSpec::new(["image/*"]));
    let duplicate = post().multipart(MultipartSpec::new(["image/png", "image/png"]));

    for (name, spec) in [
        ("json_with_limits", json_with_limits),
        ("missing_limits", missing_limits),
        ("get_upload", get_upload),
        ("zero_files", zero_files),
        ("oversized_file", oversized_file),
        ("wildcard", wildcard),
        ("duplicate", duplicate),
    ] {
        let error = match build(spec) {
            Ok(_) => panic!("{name} 必须在启动期失败"),
            Err(error) => error,
        };
        assert!(
            matches!(
                error,
                BuildError::InvalidReference {
                    kind: "Action request media",
                    ..
                }
            ),
            "{name} 返回了错误类型: {error}"
        );
    }
}

#[tokio::test]
async fn action_link_is_bound_once_and_internal_call_has_no_json_roundtrip() {
    let target = action_ref("native.user", "create_user");
    let link = ActionLink::<CreateUserInput, EchoOutput>::new(target);
    let module = ModuleSpec::new(module("native.user"))
        .native_action(CreateUserAction)
        .native_action(LinkedCreateAction { link: link.clone() });
    let app = AppBuilder::new()
        .addon(AddonSpec::new(addon("native")).module(module))
        .build(test_tools())
        .expect("ActionLink 应在 AppBuilder 构建期绑定");

    let input = CreateUserInput {
        username: "alice".to_string(),
        status: 1,
        note: None,
    };
    let output = app
        .context(crate::action::Request::new(serde_json::Value::Null))
        .plugins()
        .expect("BuiltApp context 应绑定 Registry")
        .api_run(link.handle().expect("Link 应已解析为 slot"), input)
        .await
        .expect("内部调用应直接传递 Rust 值");
    assert_eq!(output, EchoOutput { value: 1 });
}

fn org_app() -> AppBuilder {
    let org_select = get_action("select", "/orgs/select", "org.org.select");
    let user_add = ActionSpec::new(
        action("add"),
        RouteSpec::new(HttpMethod::Post, "/org/users", "org.user.add"),
    )
    .public(true)
    .param(
        ParamSpec::new(field("username"), ParamSource::Body)
            .from_field(field_ref("org_user", "username"))
            .required(true),
    )
    .calls(action_ref("org.org", "select"));

    let org_module = ModuleSpec::new(module("org.org"))
        .table(
            TableSpec::new(table("org_org"))
                .field(FieldSpec::new(field("id"), FieldKind::Key))
                .field(FieldSpec::new(field("name"), FieldKind::Str).required(true)),
        )
        .action(org_select, NoopAction);
    let user_module = ModuleSpec::new(module("org.user"))
        .table(
            TableSpec::new(table("org_user"))
                .field(FieldSpec::new(field("id"), FieldKind::Key))
                .field(FieldSpec::new(field("username"), FieldKind::Str).required(true))
                .field(
                    FieldSpec::new(field("org_org"), FieldKind::Table)
                        .relation(field_ref("org_org", "id"))
                        .select(action_ref("org.org", "select"))
                        .tenant_key(true),
                ),
        )
        .action(user_add, NoopAction)
        .view(
            ViewSpec::new(view("list"))
                .field(field_ref("org_user", "username"))
                .action(action_ref("org.user", "add")),
        );

    AppBuilder::new().addon(
        AddonSpec::new(addon("org"))
            .module(user_module)
            .module(org_module),
    )
}

#[test]
fn build_resolves_action_slots_and_sorts_catalog() {
    let app = org_app().build(test_tools()).expect("完整定义应构建成功");
    let addons = app.catalog().addons();
    assert_eq!(addons.len(), 1);
    assert_eq!(addons[0].modules[0].name.as_str(), "org.org");
    assert_eq!(addons[0].modules[1].name.as_str(), "org.user");
    assert_eq!(app.registry().len(), 2);
    assert_eq!(app.compiled_views().len(), 2);
    assert_eq!(app.compiled_views()[0].module().as_str(), "org.org");
    assert_eq!(app.compiled_views()[0].name().as_str(), "default");
    assert_eq!(app.compiled_views()[0].fields()[0].as_str(), "org_org.id");
    assert_eq!(app.compiled_views()[1].module().as_str(), "org.user");
    assert_eq!(app.compiled_views()[1].name().as_str(), "list");
    assert_eq!(app.compiled_views()[1].actions()[0].slot(), 1);
    assert_eq!(
        app.registry()
            .resolve(&action_ref("org.org", "select"))
            .expect("引用应在构建期解析")
            .slot(),
        0
    );
    assert_eq!(
        app.registry()
            .resolve(&action_ref("org.user", "add"))
            .expect("引用应在构建期解析")
            .slot(),
        1
    );
}

#[tokio::test]
async fn registry_executes_the_handler_bound_by_the_same_definition() {
    let app = org_app().build(test_tools()).expect("完整定义应构建成功");
    let handle = app
        .registry()
        .resolve(&action_ref("org.user", "add"))
        .expect("ActionRef 应在构建期解析");

    let response = app
        .dispatch(handle, crate::action::Request::new(serde_json::json!({})))
        .await
        .expect("预绑定 Handler 应执行成功");

    assert_eq!(response.code, 0);
    assert_eq!(app.catalog().addons()[0].modules[1].actions().len(), 1);
    assert_eq!(app.registry().len(), 2);
}

#[tokio::test]
async fn plugins_calls_typed_action_without_json_round_trip() {
    let reference = action_ref("org.echo", "echo");
    let app = AppBuilder::new()
        .addon(
            AddonSpec::new(addon("org")).module(
                ModuleSpec::new(module("org.echo"))
                    .action(get_action("echo", "/echo", "org.echo.echo"), EchoAction),
            ),
        )
        .build(test_tools())
        .expect("应用定义应构建成功");
    let handle = app
        .registry()
        .resolve_typed::<EchoInput, EchoOutput>(&reference)
        .expect("强类型签名应在启动期匹配");
    let output = app
        .context(crate::action::Request::new(serde_json::Value::Null))
        .plugins()
        .expect("BuiltApp 上下文应绑定 Registry")
        .api_run(handle, EchoInput { value: 42 })
        .await
        .expect("内部调用应成功");

    assert_eq!(output, EchoOutput { value: 42 });
}

#[test]
fn typed_action_handle_rejects_mismatched_signature_at_startup() {
    let reference = action_ref("org.echo", "echo");
    let app = AppBuilder::new()
        .addon(
            AddonSpec::new(addon("org")).module(
                ModuleSpec::new(module("org.echo"))
                    .action(get_action("echo", "/echo", "org.echo.echo"), EchoAction),
            ),
        )
        .build(test_tools())
        .expect("应用定义应构建成功");

    assert!(app
        .registry()
        .resolve_typed::<NoopInput, NoopOutput>(&reference)
        .is_err());
}

#[test]
fn catalog_is_independent_of_addon_registration_order() {
    let alpha = AddonSpec::new(addon("alpha")).module(
        ModuleSpec::new(module("alpha.main"))
            .action(get_action("list", "/alpha", "alpha.main.list"), NoopAction),
    );
    let beta = AddonSpec::new(addon("beta")).module(
        ModuleSpec::new(module("beta.main"))
            .action(get_action("list", "/beta", "beta.main.list"), NoopAction),
    );
    let left = AppBuilder::new()
        .addon(beta.clone())
        .addon(alpha.clone())
        .build(test_tools())
        .expect("定义应构建成功");
    let right = AppBuilder::new()
        .addon(alpha)
        .addon(beta)
        .build(test_tools())
        .expect("定义应构建成功");
    assert_eq!(left.catalog(), right.catalog());
}

#[test]
fn duplicate_names_fail_during_build() {
    let duplicate_addon = AppBuilder::new()
        .addon(AddonSpec::new(addon("org")))
        .addon(AddonSpec::new(addon("org")))
        .build(test_tools());
    assert!(matches!(
        duplicate_addon,
        Err(BuildError::DuplicateName { kind: "Addon", .. })
    ));

    let duplicate_action = AppBuilder::new()
        .addon(
            AddonSpec::new(addon("org")).module(
                ModuleSpec::new(module("org.user"))
                    .action(get_action("list", "/users", "org.user.list"), NoopAction)
                    .action(
                        get_action("list", "/users/again", "org.user.list_again"),
                        NoopAction,
                    ),
            ),
        )
        .build(test_tools());
    assert!(matches!(
        duplicate_action,
        Err(BuildError::DuplicateName { kind: "Action", .. })
    ));
}

#[test]
fn missing_dependency_and_wrong_module_owner_fail() {
    let missing = AppBuilder::new()
        .addon(AddonSpec::new(addon("org")).depends_on(addon("user")))
        .build(test_tools());
    assert!(matches!(missing, Err(BuildError::DependencyMissing { .. })));

    let wrong_owner = AppBuilder::new()
        .addon(AddonSpec::new(addon("org")).module(ModuleSpec::new(module("user.account"))))
        .build(test_tools());
    assert!(matches!(
        wrong_owner,
        Err(BuildError::InvalidReference {
            kind: "Module owner",
            ..
        })
    ));
}

#[test]
fn invalid_field_and_action_references_fail() {
    let invalid_field = AppBuilder::new()
        .addon(
            AddonSpec::new(addon("org")).module(
                ModuleSpec::new(module("org.user")).table(
                    TableSpec::new(table("org_user")).field(
                        FieldSpec::new(field("org_org"), FieldKind::Table)
                            .relation(field_ref("missing_table", "id")),
                    ),
                ),
            ),
        )
        .build(test_tools());
    assert!(matches!(
        invalid_field,
        Err(BuildError::InvalidReference { kind: "Field", .. })
    ));

    let invalid_action = AppBuilder::new()
        .addon(
            AddonSpec::new(addon("org")).module(
                ModuleSpec::new(module("org.user")).action(
                    get_action("add", "/users", "org.user.add")
                        .calls(action_ref("org.user", "missing")),
                    NoopAction,
                ),
            ),
        )
        .build(test_tools());
    assert!(matches!(
        invalid_action,
        Err(BuildError::InvalidReference { kind: "Action", .. })
    ));
}

#[test]
fn route_conflicts_fail_but_different_methods_share_exact_path() {
    let conflict = AppBuilder::new()
        .addon(
            AddonSpec::new(addon("org")).module(
                ModuleSpec::new(module("org.user"))
                    .action(get_action("get", "/users/{id}", "org.user.get"), NoopAction)
                    .action(
                        get_action("find", "/users/{user_id}", "org.user.find"),
                        NoopAction,
                    ),
            ),
        )
        .build(test_tools());
    assert!(matches!(conflict, Err(BuildError::RouteConflict { .. })));

    let shared_path = AppBuilder::new()
        .addon(
            AddonSpec::new(addon("org")).module(
                ModuleSpec::new(module("org.user"))
                    .action(get_action("list", "/users", "org.user.list"), NoopAction)
                    .action(
                        ActionSpec::new(
                            action("add"),
                            RouteSpec::new(HttpMethod::Post, "/users", "org.user.add"),
                        ),
                        NoopAction,
                    ),
            ),
        )
        .build(test_tools());
    assert!(shared_path.is_ok());
}

#[test]
fn field_shape_and_path_param_are_validated() {
    let missing_relation = AppBuilder::new()
        .addon(
            AddonSpec::new(addon("org")).module(
                ModuleSpec::new(module("org.user")).table(
                    TableSpec::new(table("org_user"))
                        .field(FieldSpec::new(field("org_org"), FieldKind::Table)),
                ),
            ),
        )
        .build(test_tools());
    assert!(matches!(
        missing_relation,
        Err(BuildError::InvalidFieldDefinition { .. })
    ));

    let missing_path = AppBuilder::new()
        .addon(
            AddonSpec::new(addon("org")).module(
                ModuleSpec::new(module("org.user")).action(
                    get_action("get", "/users", "org.user.get")
                        .param(ParamSpec::new(field("id"), ParamSource::Path)),
                    NoopAction,
                ),
            ),
        )
        .build(test_tools());
    assert!(matches!(
        missing_path,
        Err(BuildError::InvalidReference {
            kind: "Path param",
            ..
        })
    ));
}

#[cfg(feature = "mysql")]
#[test]
fn module_crud_registers_six_actions_and_one_compiled_table() {
    let module = ModuleSpec::new(module("org.post"))
        .table(
            TableSpec::new(table("org_post"))
                .field(FieldSpec::new(field("id"), FieldKind::Key))
                .field(FieldSpec::new(field("title"), FieldKind::Str).required(true)),
        )
        .crud()
        .expect("合法主表应注册标准 CRUD");
    let app = AppBuilder::new()
        .addon(AddonSpec::new(addon("org")).module(module))
        .build(test_tools())
        .expect("CRUD 应用应构建成功");

    assert_eq!(app.registry().len(), 6);
    assert_eq!(app.table_definitions().len(), 1);
    assert_eq!(app.table_definitions()[0].name(), "org_post");
    for name in ["add", "put", "del", "get", "select", "table"] {
        assert!(app
            .registry()
            .resolve(&action_ref("org.post", name))
            .is_some());
    }
}

#[cfg(feature = "openapi")]
#[test]
fn definition_catalog_projects_typed_openapi_contract() {
    let action = ActionSpec::new(
        action("echo"),
        RouteSpec::new(HttpMethod::Post, "/echo/{id}", "org.echo.echo"),
    )
    .public(true)
    .param(ParamSpec::new(field("id"), ParamSource::Path).required(true));
    let app = AppBuilder::new()
        .addon(
            AddonSpec::new(addon("org"))
                .module(ModuleSpec::new(module("org.echo")).action(action, EchoAction)),
        )
        .build(test_tools())
        .expect("OpenAPI 样板应构建成功");
    let document = app
        .catalog()
        .to_openapi(OpenApiInfo::new("YANG", "0.3"))
        .expect("Catalog 应生成 OpenAPI");

    let operation = &document["paths"]["/echo/{id}"]["post"];
    assert_eq!(operation["operationId"], "org.echo.echo");
    assert_eq!(operation["parameters"][0]["in"], "path");
    assert_eq!(operation["x-public"], true);
    assert!(
        operation["responses"]["200"]["content"]["application/json"]["schema"]["properties"]
            ["data"]
            .is_object()
    );
}

#[cfg(feature = "openapi")]
#[test]
fn openapi_projects_multipart_content_type_and_resource_limits() {
    let app = AppBuilder::new()
        .addon(
            AddonSpec::new(addon("upload"))
                .module(ModuleSpec::new(module("upload.file")).native_action(MultipartNoopAction)),
        )
        .build(test_tools())
        .expect("multipart OpenAPI 测试应用应构建成功");
    let document = app
        .catalog()
        .to_openapi(OpenApiInfo::new("YANG", "0.3"))
        .expect("Catalog 应生成 multipart OpenAPI");
    let media =
        &document["paths"]["/upload"]["post"]["requestBody"]["content"]["multipart/form-data"];

    assert!(media["schema"].is_object());
    assert_eq!(media["x-yang-multipart"]["max_files"], 1);
    assert_eq!(media["x-yang-multipart"]["max_file_bytes"], 1024);
    assert_eq!(
        media["x-yang-multipart"]["allowed_content_types"][0],
        "application/pdf"
    );
    assert_eq!(media["x-yang-multipart"]["lifecycle"], "request_scoped");
}
