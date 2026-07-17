//! 面向前端运行时的版本化 UI 契约。
//!
//! 本模块只定义声明式数据，不包含组件路径、脚本或权限判定。请求级权限过滤由
//! 上层 projector 在构造 [`UiCatalog`] 前完成，避免把未授权 Action 暴露给前端。

use super::{ActionSpec, ParamSource};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 当前 UI 契约版本。
pub const UI_SCHEMA_VERSION: &str = "1.1";

/// 与存储类型解耦的前端控件提示。
///
/// 该值只表达建议展示方式，不改变字段验证、权限或数据库语义。消费者遇到新版本
/// 未知值时必须降级为 [`Json`](Self::Json)，而不是拒绝整个页面契约。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WidgetHint {
    /// 单行文本。
    Text,
    /// 多行文本。
    Textarea,
    /// 密码输入。
    Password,
    /// 邮箱输入。
    Email,
    /// URL 输入。
    Url,
    /// 颜色输入。
    Color,
    /// 富文本或编辑器输入。
    Editor,
    /// 整数输入。
    Integer,
    /// 定点小数输入。
    Decimal,
    /// 布尔开关。
    Switch,
    /// 固定枚举单选。
    Radio,
    /// 关系选项选择器。
    RelationSelect,
    /// 树关系选择器。
    TreeSelect,
    /// 日期时间输入。
    DateTime,
    /// 安全的通用 JSON/text fallback。
    #[default]
    #[serde(other)]
    Json,
}

/// Action 成功响应的静态类别。
///
/// 该值决定前端的安全展示通道；它描述 Action 契约，不从某次运行时响应猜测。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ActionResponseKind {
    /// 普通 JSON 响应。
    #[default]
    Json,
    /// 文件下载。
    Download,
    /// 浏览器内文件预览。
    Preview,
    /// HTTP 重定向。
    Redirect,
}

/// Action 参数在 HTTP 请求中的来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum UiParamSource {
    /// JSON body。
    Body,
    /// Query string。
    Query,
    /// Path 参数。
    Path,
    /// Header。
    Header,
}

impl From<ParamSource> for UiParamSource {
    fn from(source: ParamSource) -> Self {
        match source {
            ParamSource::Body => Self::Body,
            ParamSource::Query => Self::Query,
            ParamSource::Path => Self::Path,
            ParamSource::Header => Self::Header,
        }
    }
}

/// 默认 Action 演示页需要的单个参数契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ActionDemoParamSchema {
    /// 参数名。
    pub name: String,
    /// 参数来源。
    pub source: UiParamSource,
    /// 是否必填；path 参数即使定义遗漏，也始终为必填。
    pub required: bool,
    /// 用户可见标题。
    pub title: String,
    /// 参数帮助说明。
    pub description: String,
}

/// 单个 Action 的默认演示页契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ActionDemoSchema {
    /// 全局唯一 operation id。
    pub operation_id: String,
    /// 用户可见标题。
    pub title: String,
    /// Action 业务说明。
    pub description: String,
    /// 标准大写 HTTP method。
    pub method: String,
    /// 已校验的服务端路由模板。
    pub path: String,
    /// 参数来源与展示信息。
    pub params: Vec<ActionDemoParamSchema>,
    /// Handler Input 的 JSON Schema。
    pub input_schema: serde_json::Value,
    /// Handler Output 的 JSON Schema。
    pub output_schema: serde_json::Value,
    /// 成功响应的展示类别。
    pub response_kind: ActionResponseKind,
    /// 是否必须先建立认证身份。
    pub requires_auth: bool,
}

/// 通用表格页的单列展示契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct TableColumnSchema {
    /// 行数据中的字段名。
    pub field: String,
    /// 用户可见标题。
    pub title: String,
    /// 字段帮助说明。
    pub description: String,
    /// 建议控件；未知值由消费者降级为 JSON/text。
    pub widget: WidgetHint,
    /// 输入时是否必填。
    pub required: bool,
    /// 是否允许作为筛选字段。
    pub filterable: bool,
    /// 是否允许排序。
    pub sortable: bool,
}

/// 请求级通用表格 View 契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct TableViewSchema {
    /// 稳定 View ID，不与前端文件路径绑定。
    pub view_id: String,
    /// 用户可见标题。
    pub title: String,
    /// 服务端表定义名。
    pub table: String,
    /// 当前用户可读的有序列。
    pub columns: Vec<TableColumnSchema>,
    /// 当前用户可调用的有序 Action operation IDs。
    pub actions: Vec<String>,
}

impl From<&ActionSpec> for ActionDemoSchema {
    fn from(action: &ActionSpec) -> Self {
        let params = action
            .params
            .iter()
            .map(|param| {
                let name = param.name.to_string();
                ActionDemoParamSchema {
                    title: if param.presentation.title.is_empty() {
                        name.clone()
                    } else {
                        param.presentation.title.clone()
                    },
                    name,
                    source: param.source.into(),
                    required: param.required || param.source == ParamSource::Path,
                    description: param.presentation.description.clone(),
                }
            })
            .collect();
        Self {
            operation_id: action.route.operation_id.clone(),
            title: action.display_name.clone(),
            description: action.description.clone(),
            method: action.route.method.as_str().to_string(),
            path: action.route.path.clone(),
            params,
            input_schema: action.input_schema.clone(),
            output_schema: action.output_schema.clone(),
            response_kind: action.response_kind,
            requires_auth: !action.is_public,
        }
    }
}

/// 一次请求返回给前端的 UI 目录契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct UiCatalog {
    /// UI schema 版本；前端必须按版本选择解析器。
    pub schema_version: &'static str,
    /// 当前请求有权访问的 Action 演示契约。
    pub actions: Vec<ActionDemoSchema>,
    /// 当前请求有权访问的通用表格 Views。
    pub table_views: Vec<TableViewSchema>,
}

impl UiCatalog {
    /// 从已经完成请求级过滤的 Action 集合构造目录，并按 operation id 稳定排序。
    pub fn new<I>(actions: I) -> Self
    where
        I: IntoIterator<Item = ActionDemoSchema>,
    {
        let mut actions = actions.into_iter().collect::<Vec<_>>();
        actions.sort_by(|left, right| left.operation_id.cmp(&right.operation_id));
        Self {
            schema_version: UI_SCHEMA_VERSION,
            actions,
            table_views: Vec::new(),
        }
    }

    pub(crate) fn with_table_views<I>(mut self, views: I) -> Self
    where
        I: IntoIterator<Item = TableViewSchema>,
    {
        self.table_views = views.into_iter().collect();
        self.table_views
            .sort_by(|left, right| left.view_id.cmp(&right.view_id));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{ActionContext, PermissionMode, Request, TypedHandler, User};
    use crate::definition::{
        AccessRule, ActionName, ActionRef, AddonName, AddonSpec, AppBuilder, FieldKind, FieldName,
        FieldRef, FieldSpec, HttpMethod, ModuleName, ModuleSpec, ParamSpec, RouteSpec, TableName,
        TableSpec, ViewName, ViewSpec,
    };
    use crate::error::BaseError;
    use crate::tools::ToolsBuilder;
    use async_trait::async_trait;
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    #[derive(Debug, Deserialize, schemars::JsonSchema)]
    struct NoopInput {}

    #[derive(Debug, Serialize, schemars::JsonSchema)]
    struct NoopOutput {}

    #[derive(crate::Action)]
    #[action(name = "noop", public)]
    struct NoopAction;

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

    fn action(name: &str, operation_id: &str) -> ActionSpec {
        ActionSpec::new(
            ActionName::new(name).expect("测试 Action 名称应有效"),
            RouteSpec::new(HttpMethod::Post, format!("/{name}"), operation_id),
        )
    }

    #[test]
    fn ui_catalog_serializes_stable_minimal_action_contract() {
        let mut protected = action("export", "org.user.export")
            .display_name("导出用户")
            .description("导出当前租户用户")
            .response_kind(ActionResponseKind::Download)
            .param(
                ParamSpec::new(
                    FieldName::new("tenant_id").expect("测试字段名应有效"),
                    ParamSource::Path,
                )
                .required(false),
            )
            .param(ParamSpec::new(
                FieldName::new("filter").expect("测试字段名应有效"),
                ParamSource::Body,
            ))
            .param(ParamSpec::new(
                FieldName::new("search").expect("测试字段名应有效"),
                ParamSource::Query,
            ))
            .param(ParamSpec::new(
                FieldName::new("request_id").expect("测试字段名应有效"),
                ParamSource::Header,
            ));
        protected.input_schema =
            json!({"type": "object", "properties": {"filter": {"type": "string"}}});
        protected.output_schema = json!({"type": "string", "format": "binary"});
        let public = action("health", "health.check").public(true);
        let catalog = UiCatalog::new([
            ActionDemoSchema::from(&protected),
            ActionDemoSchema::from(&public),
        ]);

        let value = serde_json::to_value(catalog).expect("UI Catalog 应可序列化");
        assert_eq!(value["schema_version"], UI_SCHEMA_VERSION);
        assert_eq!(value["actions"][0]["operation_id"], "health.check");
        assert_eq!(value["actions"][0]["requires_auth"], false);
        assert_eq!(value["actions"][1]["response_kind"], "download");
        assert_eq!(value["actions"][1]["params"][0]["source"], "path");
        assert_eq!(value["actions"][1]["params"][0]["required"], true);
        assert_eq!(value["actions"][1]["params"][0]["title"], "tenant_id");
        assert_eq!(value["actions"][1]["params"][1]["source"], "body");
        assert_eq!(value["actions"][1]["params"][2]["source"], "query");
        assert_eq!(value["actions"][1]["params"][3]["source"], "header");
        assert_eq!(value["actions"][1]["input_schema"], protected.input_schema);
        assert_eq!(
            value["actions"][1]["output_schema"],
            protected.output_schema
        );
        assert_eq!(value["actions"][1]["method"], "POST");
    }

    #[test]
    fn action_demo_does_not_leak_internal_permissions_calls_or_tags() {
        let spec = action("remove", "org.user.remove")
            .permissions(["org.user.delete"], crate::action::PermissionMode::All)
            .tag("internal")
            .calls(crate::definition::ActionRef::new(
                crate::definition::ModuleName::new("audit.log").expect("测试 Module 名称应有效"),
                ActionName::new("write").expect("测试 Action 名称应有效"),
            ));
        let value = serde_json::to_value(ActionDemoSchema::from(&spec))
            .expect("ActionDemoSchema 应可序列化");

        assert_eq!(value["requires_auth"], json!(true));
        assert!(value.get("permissions").is_none());
        assert!(value.get("permission_mode").is_none());
        assert!(value.get("calls").is_none());
        assert!(value.get("tags").is_none());
    }

    #[tokio::test]
    async fn request_projection_reuses_dispatch_authorization_policy() {
        let module = ModuleSpec::new(ModuleName::new("org.user").expect("测试 Module 名称应有效"))
            .default_permissions(["module:access"], PermissionMode::All)
            .action(
                action("public", "org.user.public")
                    .public(true)
                    .permissions(["never:granted"], PermissionMode::All),
                NoopAction,
            )
            .action(action("member", "org.user.member"), NoopAction)
            .action(
                action("all", "org.user.all")
                    .permissions(["record:read", "record:write"], PermissionMode::All),
                NoopAction,
            )
            .action(
                action("any", "org.user.any")
                    .permissions(["record:read", "record:write"], PermissionMode::Any),
                NoopAction,
            );
        let app = AppBuilder::new()
            .addon(
                AddonSpec::new(AddonName::new("org").expect("测试 Addon 名称应有效"))
                    .module(module),
            )
            .build(ToolsBuilder::new().build().expect("测试 Tools 应构建成功"))
            .expect("测试 App 应构建成功");

        let anonymous = app.context(Request::new(serde_json::Value::Null));
        assert_eq!(
            operation_ids(app.ui_catalog(&anonymous)),
            ["org.user.public"]
        );

        let module_only = app
            .context(Request::new(serde_json::Value::Null))
            .with_user(User::new(1, "module").with_permissions(["module:access"]));
        assert_eq!(
            operation_ids(app.ui_catalog(&module_only)),
            ["org.user.member", "org.user.public"]
        );

        let any = app
            .context(Request::new(serde_json::Value::Null))
            .with_user(User::new(2, "any").with_permissions(["module:access", "record:read"]));
        assert_eq!(
            operation_ids(app.ui_catalog(&any)),
            ["org.user.any", "org.user.member", "org.user.public"]
        );

        let all = app
            .context(Request::new(serde_json::Value::Null))
            .with_user(User::new(3, "all").with_permissions([
                "module:access",
                "record:read",
                "record:write",
            ]));
        assert_eq!(
            operation_ids(app.ui_catalog(&all)),
            [
                "org.user.all",
                "org.user.any",
                "org.user.member",
                "org.user.public"
            ]
        );

        let action_only = app
            .context(Request::new(serde_json::Value::Null))
            .with_user(User::new(4, "action").with_permissions(["record:read", "record:write"]));
        assert_eq!(
            operation_ids(app.ui_catalog(&action_only)),
            ["org.user.public"]
        );

        let all_handle = app
            .registry()
            .resolve(&ActionRef::new(
                ModuleName::new("org.user").expect("测试 Module 名称应有效"),
                ActionName::new("all").expect("测试 Action 名称应有效"),
            ))
            .expect("all Action 应已注册");
        let denied = app
            .dispatch_context(
                all_handle,
                app.context(Request::new(json!({}))).with_user(
                    User::new(4, "action").with_permissions(["record:read", "record:write"]),
                ),
            )
            .await;
        assert!(matches!(denied, Err(BaseError::PermissionDenied(_))));

        let allowed = app
            .dispatch_context(
                all_handle,
                app.context(Request::new(json!({}))).with_user(
                    User::new(3, "all").with_permissions([
                        "module:access",
                        "record:read",
                        "record:write",
                    ]),
                ),
            )
            .await;
        assert!(allowed.is_ok(), "目录可见的 Action 应通过同一授权策略");

        let public_handle = app
            .registry()
            .resolve(&ActionRef::new(
                ModuleName::new("org.user").expect("测试 Module 名称应有效"),
                ActionName::new("public").expect("测试 Action 名称应有效"),
            ))
            .expect("public Action 应已注册");
        let public = app
            .dispatch_context(public_handle, app.context(Request::new(json!({}))))
            .await;
        assert!(
            public.is_ok(),
            "public Action 应同时绕过模块与 Action 权限组"
        );
    }

    fn operation_ids(catalog: UiCatalog) -> Vec<String> {
        catalog
            .actions
            .into_iter()
            .map(|action| action.operation_id)
            .collect()
    }

    #[test]
    fn widget_hint_maps_field_semantics_without_changing_storage_kind() {
        let field = |kind| FieldSpec::new(FieldName::new("value").expect("字段名应有效"), kind);

        assert_eq!(field(FieldKind::Key).widget_hint(), WidgetHint::Integer);
        assert_eq!(field(FieldKind::Str).widget_hint(), WidgetHint::Text);
        assert_eq!(field(FieldKind::Text).widget_hint(), WidgetHint::Textarea);
        assert_eq!(field(FieldKind::Int).widget_hint(), WidgetHint::Integer);
        assert_eq!(field(FieldKind::Decimal).widget_hint(), WidgetHint::Decimal);
        assert_eq!(field(FieldKind::Switch).widget_hint(), WidgetHint::Switch);
        assert_eq!(field(FieldKind::Radio).widget_hint(), WidgetHint::Radio);
        assert_eq!(
            field(FieldKind::Table).widget_hint(),
            WidgetHint::RelationSelect
        );
        assert_eq!(field(FieldKind::Tree).widget_hint(), WidgetHint::TreeSelect);
        assert_eq!(
            field(FieldKind::Timestamp).widget_hint(),
            WidgetHint::DateTime
        );
    }

    #[test]
    fn widget_hint_explicit_override_and_unknown_value_have_safe_fallbacks() {
        let mut secret = FieldSpec::new(
            FieldName::new("secret").expect("字段名应有效"),
            FieldKind::Str,
        );
        secret.access.secret = true;
        assert_eq!(secret.widget_hint(), WidgetHint::Password);

        secret.presentation.widget = Some(WidgetHint::Email);
        assert_eq!(secret.widget_hint(), WidgetHint::Email);
        assert_eq!(secret.kind, FieldKind::Str, "控件提示不得改变字段数据种类");

        let unknown: WidgetHint =
            serde_json::from_value(json!("future_spatial_editor")).expect("未知提示应安全解析");
        assert_eq!(unknown, WidgetHint::Json);
        assert_eq!(
            serde_json::to_value(unknown).expect("fallback 应可序列化"),
            json!("json")
        );
    }

    #[test]
    fn table_view_projection_filters_module_fields_and_actions_with_same_request_identity() {
        let module_name = ModuleName::new("org.member").expect("测试 Module 名称应有效");
        let table_name = TableName::new("org_member").expect("测试 Table 名称应有效");
        let field_ref = |name: &str| {
            FieldRef::new(
                table_name.clone(),
                FieldName::new(name).expect("测试字段名应有效"),
            )
        };
        let action_ref = |name: &str| {
            ActionRef::new(
                module_name.clone(),
                ActionName::new(name).expect("测试 Action 名称应有效"),
            )
        };

        let mut name = FieldSpec::new(
            FieldName::new("name").expect("测试字段名应有效"),
            FieldKind::Str,
        );
        name.presentation.title = "名称".to_string();
        name.access.searchable = true;
        name.access.sortable = true;
        let mut admin_note = FieldSpec::new(
            FieldName::new("admin_note").expect("测试字段名应有效"),
            FieldKind::Text,
        );
        admin_note.access.readable = AccessRule::Roles(vec!["admin".to_string()]);
        let mut secret = FieldSpec::new(
            FieldName::new("secret").expect("测试字段名应有效"),
            FieldKind::Str,
        );
        secret.access.secret = true;
        secret.access.readable = AccessRule::Everyone;

        let view = ViewSpec::new(ViewName::new("main").expect("测试 View 名称应有效"))
            .field(field_ref("name"))
            .field(field_ref("admin_note"))
            .field(field_ref("secret"))
            .action(action_ref("list"))
            .action(action_ref("edit"));
        let module = ModuleSpec::new(module_name)
            .table(
                TableSpec::new(table_name)
                    .title("组织成员")
                    .field(FieldSpec::new(
                        FieldName::new("id").expect("测试字段名应有效"),
                        FieldKind::Key,
                    ))
                    .field(name)
                    .field(admin_note)
                    .field(secret),
            )
            .default_permissions(["module:view"], PermissionMode::All)
            .action(action("list", "org.member.list"), NoopAction)
            .action(
                action("edit", "org.member.edit").permissions(["member:edit"], PermissionMode::All),
                NoopAction,
            )
            .view(view);
        let app = AppBuilder::new()
            .addon(
                AddonSpec::new(AddonName::new("org").expect("测试 Addon 名称应有效"))
                    .module(module),
            )
            .build(ToolsBuilder::new().build().expect("测试 Tools 应构建成功"))
            .expect("TableView 测试应用应构建成功");

        let anonymous = app.ui_catalog(&app.context(Request::new(json!({}))));
        assert!(
            anonymous.table_views.is_empty(),
            "匿名请求不得看到受保护 View"
        );

        let member = app.ui_catalog(
            &app.context(Request::new(json!({})))
                .with_user(User::new(7, "member").with_permissions(["module:view"])),
        );
        assert_eq!(member.table_views.len(), 1);
        assert_eq!(member.table_views[0].view_id, "org.member.main");
        assert_eq!(member.table_views[0].table, "org_member");
        assert_eq!(member.table_views[0].columns.len(), 1);
        assert_eq!(member.table_views[0].columns[0].field, "name");
        assert_eq!(member.table_views[0].columns[0].widget, WidgetHint::Text);
        assert!(member.table_views[0].columns[0].filterable);
        assert!(member.table_views[0].columns[0].sortable);
        assert_eq!(member.table_views[0].actions, ["org.member.list"]);

        let admin = app.ui_catalog(
            &app.context(Request::new(json!({}))).with_user(
                User::new(8, "admin")
                    .with_roles(["admin"])
                    .with_permissions(["module:view", "member:edit"]),
            ),
        );
        assert_eq!(
            admin.table_views[0]
                .columns
                .iter()
                .map(|column| column.field.as_str())
                .collect::<Vec<_>>(),
            ["name", "admin_note"],
            "角色字段应出现，但 secret 字段即使 readable 也不得投影"
        );
        assert_eq!(
            admin.table_views[0].actions,
            ["org.member.list", "org.member.edit"]
        );
    }
}
