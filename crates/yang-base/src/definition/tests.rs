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

/// 含二进制文件字段的上传输入；`format: binary` 是构建期媒体类型校验的判据。
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct MultipartUploadInput {
    status: i8,
    file: crate::action::UploadedFile,
}

impl ParamInput for MultipartUploadInput {
    fn params() -> Params {
        Params::new()
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
    max_total_bytes = 131072
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
    type Input = MultipartUploadInput;
    type Output = EchoOutput;

    async fn index(
        &self,
        _ctx: ActionContext,
        input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Ok(EchoOutput {
            value: i64::from(input.status) + i64::try_from(input.file.size()).unwrap_or_default(),
        })
    }
}

#[derive(crate::Action)]
#[action(name = "json_upload", path = "/native/upload/json")]
struct JsonUploadAction;

#[async_trait]
impl crate::action::Action for JsonUploadAction {
    type Input = MultipartUploadInput;
    type Output = EchoOutput;

    async fn index(
        &self,
        _ctx: ActionContext,
        input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Ok(EchoOutput {
            value: i64::from(input.status) + i64::try_from(input.file.size()).unwrap_or_default(),
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

/// 符合关系选项契约的 select Action 夹具：输入/输出即稳定 DTO 对，
/// 经 `Action::Input` 解码时自动带上 `RelationOptionsRequest::validate` 边界校验。
#[derive(crate::Action)]
#[action(name = "select", public)]
struct RelationOptionsAction;

#[async_trait]
impl crate::action::Action for RelationOptionsAction {
    type Input = crate::table::RelationOptionsRequest;
    type Output = crate::table::RelationOptionsResponse;

    async fn index(
        &self,
        _ctx: ActionContext,
        input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Ok(crate::table::RelationOptionsResponse {
            items: Vec::new(),
            page: input.page,
            limit: input.limit,
            total: Some(0),
        })
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

#[test]
fn radio_builder_declares_filter_and_sort_access() {
    let fields = crate::fields! {
        status => Radio::<String>::new()
            .options([("active", "启用")])
            .filterable(true)
            .sortable(true),
    };
    let status = &fields.as_slice()[0];
    assert!(status.access.filterable);
    assert!(status.access.sortable);
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
    assert_eq!(multipart.max_total_bytes, 131072);
    assert_eq!(
        multipart.max_text_field_bytes,
        DEFAULT_MULTIPART_MAX_TEXT_FIELD_BYTES
    );
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

#[test]
fn app_builder_rejects_incoherent_text_field_limits() {
    // 文本字段上限的构建期校验必须以其自身原因拒绝（而非被后续校验顺带拦截）：
    // 手工 ActionSpec 的 input_schema 无文件字段，若上限校验缺失，构建会落到
    // “必须至少声明一个文件字段” 的原因上，断言 reference 可区分两者。
    let build = |multipart: MultipartSpec| {
        let spec = ActionSpec::new(
            action("upload"),
            RouteSpec::new(HttpMethod::Post, "/upload", "upload.file.upload"),
        )
        .multipart(multipart);
        AppBuilder::new()
            .addon(
                AddonSpec::new(addon("upload"))
                    .module(ModuleSpec::new(module("upload.file")).action(spec, NoopAction)),
            )
            .build(test_tools())
    };
    let cases: [(&str, MultipartSpec); 2] = [
        (
            "文本字段字节上限必须大于 0",
            MultipartSpec::new(["application/pdf"]).max_text_field_bytes(0),
        ),
        (
            "max_text_field_bytes 不能大于 max_total_bytes",
            MultipartSpec::new(["application/pdf"])
                .max_file_bytes(1024)
                .max_text_field_bytes(4096)
                .max_total_bytes(2048),
        ),
    ];
    for (reason, multipart) in cases {
        let error = match build(multipart) {
            Ok(_) => panic!("{reason} 必须在启动期失败"),
            Err(error) => error,
        };
        assert!(
            matches!(
                &error,
                BuildError::InvalidReference {
                    kind: "Action request media",
                    reference,
                } if reference.contains(reason)
            ),
            "拒绝原因必须是 {reason}: {error}"
        );
    }
}

#[test]
fn app_builder_rejects_json_action_with_binary_input_field() {
    // C-1：input_schema 含二进制文件字段（format: binary）的 Action 必须声明 multipart，
    // 否则客户端可经 JSON 通道伪造 UploadedFile 实例。
    let result = AppBuilder::new()
        .addon(
            AddonSpec::new(addon("upload"))
                .module(ModuleSpec::new(module("upload.file")).native_action(JsonUploadAction)),
        )
        .build(test_tools());
    let error = match result {
        Ok(_) => panic!("JSON Action 声明二进制文件字段必须在构建期失败"),
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
        "返回了错误类型: {error}"
    );
}

#[test]
fn app_builder_rejects_multipart_action_without_binary_input_field() {
    // C-1 反向：multipart Action 的输入必须至少声明一个文件字段，
    // 防止"声明了上传通道却没有文件落点"的错误契约进入 Catalog。
    let spec = ActionSpec::new(
        action("upload"),
        RouteSpec::new(HttpMethod::Post, "/upload", "upload.file.upload"),
    )
    .multipart(MultipartSpec::new(["application/pdf"]));
    let result = AppBuilder::new()
        .addon(
            AddonSpec::new(addon("upload"))
                .module(ModuleSpec::new(module("upload.file")).action(spec, NoopAction)),
        )
        .build(test_tools());
    let error = match result {
        Ok(_) => panic!("multipart Action 缺少文件字段必须在构建期失败"),
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
        "返回了错误类型: {error}"
    );
}

#[test]
fn binary_schema_scanner_resolves_local_refs_and_plain_fields() {
    // C-1 复审：传输层文本 part 拦截依赖同一扫描器；$ref/anyOf/items 必须递归解析。
    #[allow(dead_code)] // 字段仅为 schema 形状存在，不参与运行期读取
    #[derive(Debug, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields)]
    struct Bundle {
        note: String,
        file: crate::action::UploadedFile,
    }

    #[allow(dead_code)] // 字段仅为 schema 形状存在，不参与运行期读取
    #[derive(Debug, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields)]
    struct ScannerInput {
        title: String,
        files: Vec<crate::action::UploadedFile>,
        bundles: Vec<Bundle>,
        maybe_file: Option<crate::action::UploadedFile>,
    }

    let document = serde_json::to_value(schemars::schema_for!(ScannerInput))
        .expect("扫描器测试 schema 应可序列化");
    let properties = document.get("properties").expect("schema 应有 properties");
    let contains = |name: &str| {
        builder::schema_subtree_contains_binary(
            &document,
            properties.get(name).expect("字段应存在于 properties"),
        )
    };
    assert!(
        contains("files"),
        "Vec<UploadedFile> 的 items $ref 应解析出 binary"
    );
    assert!(
        contains("bundles"),
        "嵌套包装结构体的 $ref 链应解析出 binary"
    );
    assert!(
        contains("maybe_file"),
        "Option<UploadedFile> 的 anyOf $ref 应解析出 binary"
    );
    assert!(!contains("title"), "普通字符串字段不得误判为 binary");
    assert!(
        super::builder::schema_contains_binary_field(&document),
        "构建期整文档扫描应检测到 binary"
    );

    // 自引用类型不得死循环（循环保护）。
    #[allow(dead_code)] // 字段仅为 schema 形状存在，不参与运行期读取
    #[derive(Debug, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields)]
    struct Recursive {
        next: Option<Box<Recursive>>,
    }
    let recursive =
        serde_json::to_value(schemars::schema_for!(Recursive)).expect("自引用 schema 应可序列化");
    assert!(!super::builder::schema_contains_binary_field(&recursive));
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
        .action(org_select, RelationOptionsAction);
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
fn relation_options_action_must_implement_the_contract_dto_pair() {
    // I-4：UI 目录承诺 select Action 讲 RelationOptionsRequest/RelationOptionsResponse，
    // 输入/输出签名不符的 Action 接入关系选择器必须在构建期拒绝。
    let module = ModuleSpec::new(module("org.org"))
        .table(
            TableSpec::new(table("org_org"))
                .field(FieldSpec::new(field("id"), FieldKind::Key))
                .field(
                    FieldSpec::new(field("parent_id"), FieldKind::Table)
                        .relation(field_ref("org_org", "id"))
                        .select(action_ref("org.org", "select")),
                ),
        )
        .action(
            get_action("select", "/orgs/select", "org.org.select"),
            NoopAction,
        );
    let error = AppBuilder::new()
        .addon(AddonSpec::new(addon("org")).module(module))
        .build(test_tools())
        .expect_err("签名不符的 select Action 必须在构建期拒绝");
    assert!(
        matches!(
            &error,
            BuildError::InvalidReference {
                kind: "Relation Options Action",
                reference,
            } if reference.contains("RelationOptionsRequest")
        ),
        "拒绝原因必须指向契约类型: {error}"
    );
}

#[tokio::test]
async fn dispatch_select_action_enforces_permission_beyond_catalog_hiding() {
    // I-4：UI 目录按身份隐藏无权限的 select Action 只是投影层过滤；直接 dispatch
    // 必须由授权层拦截。已认证但缺少权限的用户必须得到 PermissionDenied，
    // 而不是未认证场景的 Unauthorized。
    let module = ModuleSpec::new(module("org.org"))
        .table(
            TableSpec::new(table("org_org"))
                .field(FieldSpec::new(field("id"), FieldKind::Key))
                .field(
                    FieldSpec::new(field("parent_id"), FieldKind::Table)
                        .relation(field_ref("org_org", "id"))
                        .select(action_ref("org.org", "select")),
                ),
        )
        .action(
            get_action("select", "/orgs/select", "org.org.select")
                .public(false)
                .permissions(["org.select"], crate::action::PermissionMode::All),
            RelationOptionsAction,
        );
    let app = AppBuilder::new()
        .addon(AddonSpec::new(addon("org")).module(module))
        .build(test_tools())
        .expect("契约合规的 select Action 应构建成功");
    let handle = app
        .registry()
        .resolve(&action_ref("org.org", "select"))
        .expect("select Action 应在构建期解析");

    let context = app
        .context(crate::action::Request::new(serde_json::json!({})))
        .with_user(crate::action::User::new(7, "outsider"));
    let error = app
        .dispatch_context(handle, context)
        .await
        .expect_err("无权限用户直接调用 select Action 必须被拒绝");
    assert!(
        matches!(error, BaseError::PermissionDenied(_)),
        "缺少权限必须返回 PermissionDenied: {error}"
    );

    // 对照：持有权限的用户通过授权并完成派发（同时走通 decode+validate 默认路径）。
    let context = app
        .context(crate::action::Request::new(serde_json::json!({})))
        .with_user(crate::action::User::new(8, "insider").with_permissions(["org.select"]));
    app.dispatch_context(handle, context)
        .await
        .expect("持有权限的用户应可调用 select Action");
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
        media["x-yang-multipart"]["max_text_field_bytes"],
        DEFAULT_MULTIPART_MAX_TEXT_FIELD_BYTES
    );
    assert_eq!(
        media["x-yang-multipart"]["allowed_content_types"][0],
        "application/pdf"
    );
    assert_eq!(
        media["x-yang-multipart"]["lifecycle"],
        serde_json::to_value(UploadLifecycle::RequestScoped).expect("上传生命周期应可序列化"),
        "OpenAPI 扩展必须序列化 spec.lifecycle 真实值而不是硬编码"
    );
}

#[cfg(feature = "openapi")]
#[test]
fn openapi_projects_response_kind_specific_success_contract() {
    let spec = |name: &str, path: &str, operation_id: &str, kind: ActionResponseKind| {
        ActionSpec::new(
            action(name),
            RouteSpec::new(HttpMethod::Get, path, operation_id),
        )
        .public(true)
        .response_kind(kind)
    };
    let app = AppBuilder::new()
        .addon(
            AddonSpec::new(addon("res")).module(
                ModuleSpec::new(module("res.kind"))
                    .action(
                        spec(
                            "json",
                            "/res/json",
                            "res.kind.json",
                            ActionResponseKind::Json,
                        ),
                        NoopAction,
                    )
                    .action(
                        spec(
                            "download",
                            "/res/download",
                            "res.kind.download",
                            ActionResponseKind::Download,
                        ),
                        NoopAction,
                    )
                    .action(
                        spec(
                            "preview",
                            "/res/preview",
                            "res.kind.preview",
                            ActionResponseKind::Preview,
                        ),
                        NoopAction,
                    )
                    .action(
                        spec(
                            "redirect",
                            "/res/redirect",
                            "res.kind.redirect",
                            ActionResponseKind::Redirect,
                        ),
                        NoopAction,
                    ),
            ),
        )
        .build(test_tools())
        .expect("response_kind OpenAPI 测试应用应构建成功");
    let document = app
        .catalog()
        .to_openapi(OpenApiInfo::new("YANG", "0.3"))
        .expect("Catalog 应生成 OpenAPI");

    let json_op = &document["paths"]["/res/json"]["get"];
    assert_eq!(json_op["x-yang-response-kind"], "json");
    assert!(
        json_op["responses"]["200"]["content"]["application/json"]["schema"]["properties"]["data"]
            .is_object(),
        "JSON Action 保持信封契约: {json_op}"
    );

    let download_op = &document["paths"]["/res/download"]["get"];
    assert_eq!(download_op["x-yang-response-kind"], "download");
    assert_eq!(
        download_op["responses"]["200"]["content"]["application/octet-stream"]["schema"],
        serde_json::json!({"type": "string", "format": "binary"}),
        "下载成功响应必须是二进制流而不是 JSON 信封: {download_op}"
    );
    assert!(
        download_op["responses"]["200"]["content"]
            .get("application/json")
            .is_none(),
        "下载 Action 不得声明 JSON 信封: {download_op}"
    );

    let preview_op = &document["paths"]["/res/preview"]["get"];
    assert_eq!(preview_op["x-yang-response-kind"], "preview");
    assert!(
        preview_op["responses"]["200"]["content"]["application/octet-stream"].is_object(),
        "预览成功响应必须是二进制流: {preview_op}"
    );

    let redirect_op = &document["paths"]["/res/redirect"]["get"];
    assert_eq!(redirect_op["x-yang-response-kind"], "redirect");
    assert!(
        redirect_op["responses"]["302"].is_object(),
        "重定向必须声明 3xx 成功响应: {redirect_op}"
    );
    assert!(
        redirect_op["responses"]["302"].get("content").is_none(),
        "重定向成功响应不得携带 content: {redirect_op}"
    );
    assert!(
        redirect_op["responses"]["302"]["headers"]["Location"].is_object(),
        "重定向必须声明 Location 响应头: {redirect_op}"
    );
    assert!(
        redirect_op["responses"].get("200").is_none(),
        "重定向不得声明 200 JSON 信封: {redirect_op}"
    );
}

/// searchable / filterable / sortable 在声明层是三个独立开关，`into_schema_field` 必须按位
/// 精确写入 table 层定义，不得折叠成单一 filterable 位，也不得继承 table 层的
/// 宽松默认值（fail-closed）。
#[test]
fn query_capability_bits_map_independently_to_table_definition() {
    let with_access = |name: &str, searchable: bool, filterable: bool, sortable: bool| {
        let mut field = FieldSpec::new(field(name), FieldKind::Str);
        field.access.searchable = searchable;
        field.access.filterable = filterable;
        field.access.sortable = sortable;
        field
    };
    let spec = TableSpec::new(table("org_doc"))
        .field(FieldSpec::new(field("id"), FieldKind::Key))
        .field(with_access("all", true, true, true))
        .field(with_access("search_only", true, false, false))
        .field(with_access("filter_only", false, true, false))
        .field(with_access("sort_only", false, false, true))
        .field(with_access("neither", false, false, false));
    let definition = spec.table_definition().expect("字段位映射表定义应有效");

    let all = definition.field("all").expect("all 字段应存在");
    assert!(all.is_searchable());
    assert!(all.is_filterable());
    assert!(all.is_sortable());

    let search_only = definition
        .field("search_only")
        .expect("search_only 字段应存在");
    assert!(search_only.is_searchable());
    assert!(
        !search_only.is_filterable(),
        "searchable 不得连带开放结构化筛选"
    );
    assert!(!search_only.is_sortable());

    let filter_only = definition
        .field("filter_only")
        .expect("filter_only 字段应存在");
    assert!(
        !filter_only.is_searchable(),
        "filterable 不得连带开放关键词搜索"
    );
    assert!(filter_only.is_filterable());
    assert!(!filter_only.is_sortable());

    let sort_only = definition.field("sort_only").expect("sort_only 字段应存在");
    assert!(!sort_only.is_searchable());
    assert!(!sort_only.is_filterable());
    assert!(sort_only.is_sortable());

    let neither = definition.field("neither").expect("neither 字段应存在");
    assert!(!neither.is_searchable());
    assert!(
        !neither.is_filterable(),
        "未声明 filterable 的字段必须 fail-closed"
    );
    assert!(
        !neither.is_sortable(),
        "未声明 sortable 的字段必须 fail-closed"
    );
}

#[cfg(feature = "mysql")]
#[test]
fn module_crud_at_uses_explicit_versioned_path_and_stable_operation_ids() {
    let spec = ModuleSpec::new(module("org.user"))
        .table(TableSpec::new(table("org_user")).field(FieldSpec::new(field("id"), FieldKind::Key)))
        .crud_at("/api/v1/org/users")
        .expect("版本化 CRUD 路径应有效");

    let action = |name: &str| {
        spec.actions()
            .iter()
            .find(|action| action.name.as_str() == name)
            .unwrap_or_else(|| panic!("应存在 org.user.{name}"))
    };
    assert_eq!(action("add").route.path, "/api/v1/org/users");
    assert_eq!(action("select").route.path, "/api/v1/org/users/query");
    assert_eq!(action("table").route.path, "/api/v1/org/users/schema");
    for name in ["add", "put", "del", "get", "select", "table"] {
        assert_eq!(action(name).route.operation_id, format!("org.user.{name}"));
    }

    let invalid = ModuleSpec::new(module("org.user"))
        .table(TableSpec::new(table("org_user")).field(FieldSpec::new(field("id"), FieldKind::Key)))
        .crud_at("api/v1/org/users");
    assert!(matches!(invalid, Err(BaseError::ConfigError(_))));
}

#[cfg(feature = "mysql")]
#[test]
fn module_crud_at_with_mutations_preserves_framework_contracts() {
    use crate::action::builtin::{AddAction, DelAction, PutAction};

    let spec = ModuleSpec::new(module("org.user"))
        .table(TableSpec::new(table("org_user")).field(FieldSpec::new(field("id"), FieldKind::Key)))
        .crud_at_with_mutations(
            "/api/v1/org/users",
            AddAction::new(),
            PutAction::new(),
            DelAction::new(),
        )
        .expect("自定义 writer 应复用框架 CRUD 契约");

    for name in ["add", "put", "del", "get", "select", "table"] {
        let action = spec
            .actions()
            .iter()
            .find(|action| action.name.as_str() == name)
            .unwrap_or_else(|| panic!("应存在 org.user.{name}"));
        assert!(
            !action.input_schema.is_null(),
            "{name} 应保留表驱动输入契约"
        );
        assert!(
            !action.output_schema.is_null(),
            "{name} 应保留表驱动输出契约"
        );
        assert_eq!(action.route.operation_id, format!("org.user.{name}"));
    }
}

#[test]
fn table_spec_projects_validated_composite_indexes() {
    let spec = TableSpec::new(table("org_user"))
        .field(FieldSpec::new(field("id"), FieldKind::Key))
        .field(FieldSpec::new(field("org_org"), FieldKind::Int))
        .field(FieldSpec::new(field("user_user"), FieldKind::Int))
        .unique_named(
            "uk_org_user_membership",
            [
                field_ref("org_user", "org_org"),
                field_ref("org_user", "user_user"),
            ],
        );
    let definition = spec.table_definition().expect("复合索引应编译成功");
    let index = &definition.shared_config().unique_indexes[0];

    assert_eq!(index.fields, ["org_org", "user_user"]);
    assert_eq!(index.name.as_deref(), Some("uk_org_user_membership"));

    let cross_table = TableSpec::new(table("org_user"))
        .field(FieldSpec::new(field("org_org"), FieldKind::Int))
        .unique([field_ref("org_org", "id")]);
    assert!(matches!(
        cross_table.table_definition(),
        Err(BaseError::ConfigError(message)) if message.contains("其他表字段")
    ));
}

/// 非文本字段（Str/Text 以外的 FieldKind）声明 searchable 必须在构建期报错：
/// 服务端搜索本就会跳过非文本字段，允许声明等于让 UI 契约说谎（fail-closed）。
#[test]
fn non_text_searchable_field_is_rejected_at_build_time() {
    let build_with = |kind: FieldKind| {
        let mut spec = FieldSpec::new(field("target"), kind);
        spec.access.searchable = true;
        AppBuilder::new()
            .addon(
                AddonSpec::new(addon("org")).module(
                    ModuleSpec::new(module("org.user")).table(
                        TableSpec::new(table("org_user"))
                            .field(FieldSpec::new(field("id"), FieldKind::Key))
                            .field(spec),
                    ),
                ),
            )
            .build(test_tools())
    };

    for kind in [FieldKind::Int, FieldKind::Timestamp, FieldKind::Switch] {
        assert!(
            matches!(
                build_with(kind),
                Err(BuildError::InvalidFieldDefinition { .. })
            ),
            "非文本字段 {kind:?} 声明 searchable 必须构建失败"
        );
    }
    for kind in [FieldKind::Str, FieldKind::Text] {
        assert!(
            build_with(kind).is_ok(),
            "文本字段 {kind:?} 声明 searchable 应构建成功"
        );
    }
}

/// TreeViewSpec 的 max_nodes 在启动期解析进 CompiledTreeView：缺省回退到服务端
/// 默认常量，显式配置覆盖；配置 0 在构建期拒绝（fail-closed）。
#[test]
fn tree_view_max_nodes_defaults_overrides_and_validates() {
    let field_ref = |name: &str| FieldRef::new(table("org_node"), field(name));
    let build = |tree: TreeViewSpec| {
        let module = ModuleSpec::new(module("org.node"))
            .table(
                TableSpec::new(table("org_node"))
                    .field(FieldSpec::new(field("id"), FieldKind::Key))
                    .field(FieldSpec::new(field("parent_id"), FieldKind::Int))
                    .field(FieldSpec::new(field("name"), FieldKind::Str)),
            )
            .view(
                ViewSpec::new(ViewName::new("main").expect("测试 View 名称应有效"))
                    .field(field_ref("id"))
                    .field(field_ref("parent_id"))
                    .field(field_ref("name"))
                    .tree(tree),
            );
        AppBuilder::new()
            .addon(AddonSpec::new(addon("org")).module(module))
            .build(test_tools())
    };
    let tree_spec =
        || TreeViewSpec::new(field_ref("id"), field_ref("parent_id"), field_ref("name"));

    let app = build(tree_spec()).expect("默认树 View 应构建成功");
    let tree = app.compiled_views()[0].tree().expect("树拓扑应已编译");
    assert_eq!(
        tree.max_nodes(),
        crate::table::DEFAULT_TREE_MAX_NODES,
        "缺省 max_nodes 必须回退到服务端默认常量"
    );

    let app = build(tree_spec().max_nodes(42)).expect("覆盖树节点上限应构建成功");
    let tree = app.compiled_views()[0].tree().expect("树拓扑应已编译");
    assert_eq!(tree.max_nodes(), 42);

    let rejected = build(tree_spec().max_nodes(0));
    assert!(
        matches!(
            rejected,
            Err(BuildError::InvalidReference {
                kind: "Tree View",
                ..
            })
        ),
        "max_nodes 为 0 必须构建期拒绝: {:?}",
        rejected.err()
    );
}
