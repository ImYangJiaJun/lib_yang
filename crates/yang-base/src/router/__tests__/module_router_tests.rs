//! ModuleRouter 集成测试（H-1 类型化迁移后重写）
#![cfg(all(feature = "mysql", feature = "token"))]

use crate::action::builtin::TableAction;
use crate::action::meta::ActionMeta;
use crate::action::{ActionContext, ApiResponse, GlobalTools, Request, User};
use crate::action::{Permission, PermissionMode, TypedAction, TypedHandler};
use crate::error::BaseError;
use crate::router::{Api, Middleware, ModuleRouter, Next, BUILTIN_ACTION_NAMES};
use crate::table::{Field, Table, TableDefinition};
use crate::token::TokenManager;
use async_trait::async_trait;
use jsonwebtoken::Algorithm;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;
use std::sync::{Arc, OnceLock};

// ──────────────────────────────────────────────────────────────────────────────
// 测试用表定义
// ──────────────────────────────────────────────────────────────────────────────

fn test_user_table() -> TableDefinition {
    Table::new("test_users")
        .label("测试用户表")
        .fields([
            Field::bigint("id").required().primary_key(),
            Field::string("username", 50).required().unique(),
            Field::string("email", 100).nullable(),
        ])
        .build()
        .expect("test_users 表定义应有效")
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct BlankActionInput {}

#[derive(Serialize, schemars::JsonSchema)]
struct BlankActionOutput {}

struct BlankNameAction;

#[async_trait]
impl TypedHandler for BlankNameAction {
    type Input = BlankActionInput;
    type Output = BlankActionOutput;

    async fn handle(
        &self,
        _ctx: ActionContext,
        _input: BlankActionInput,
    ) -> Result<BlankActionOutput, BaseError> {
        Ok(BlankActionOutput {})
    }
}

impl TypedAction for BlankNameAction {
    fn name(&self) -> &'static str {
        "   "
    }

    fn input_schema(&self) -> &'static schemars::schema::RootSchema {
        static S: OnceLock<schemars::schema::RootSchema> = OnceLock::new();
        S.get_or_init(|| schemars::schema_for!(BlankActionInput))
    }

    fn output_schema(&self) -> &'static schemars::schema::RootSchema {
        static S: OnceLock<schemars::schema::RootSchema> = OnceLock::new();
        S.get_or_init(|| schemars::schema_for!(BlankActionOutput))
    }

    fn meta_static(&self) -> &'static ActionMeta {
        static M: OnceLock<ActionMeta> = OnceLock::new();
        M.get_or_init(|| ActionMeta {
            name: "   ",
            display_name: "空白 Action",
            description: "",
            permissions: &[],
            permission_mode: PermissionMode::All,
            is_public: false,
            input_schema: self.input_schema(),
            output_schema: self.output_schema(),
        })
    }
}

struct BlankPermissionAction;

#[async_trait]
impl TypedHandler for BlankPermissionAction {
    type Input = BlankActionInput;
    type Output = BlankActionOutput;

    async fn handle(
        &self,
        _ctx: ActionContext,
        _input: BlankActionInput,
    ) -> Result<BlankActionOutput, BaseError> {
        Ok(BlankActionOutput {})
    }
}

impl TypedAction for BlankPermissionAction {
    fn name(&self) -> &'static str {
        "blank_permission"
    }

    fn input_schema(&self) -> &'static schemars::schema::RootSchema {
        static S: OnceLock<schemars::schema::RootSchema> = OnceLock::new();
        S.get_or_init(|| schemars::schema_for!(BlankActionInput))
    }

    fn output_schema(&self) -> &'static schemars::schema::RootSchema {
        static S: OnceLock<schemars::schema::RootSchema> = OnceLock::new();
        S.get_or_init(|| schemars::schema_for!(BlankActionOutput))
    }

    fn meta_static(&self) -> &'static ActionMeta {
        static M: OnceLock<ActionMeta> = OnceLock::new();
        M.get_or_init(|| {
            let permissions: &'static [Permission] =
                Box::leak(vec![Permission::from_static("   ")].into_boxed_slice());
            ActionMeta {
                name: "blank_permission",
                display_name: "空白权限 Action",
                description: "",
                permissions,
                permission_mode: PermissionMode::All,
                is_public: false,
                input_schema: self.input_schema(),
                output_schema: self.output_schema(),
            }
        })
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// 测试辅助
// ──────────────────────────────────────────────────────────────────────────────

fn create_test_user() -> User {
    User {
        id: 1,
        username: "test_user".to_string(),
        nickname: "测试用户".to_string(),
        email: "test@example.com".to_string(),
        roles: HashSet::from(["admin".to_string()]),
        permissions: HashSet::from([
            "user:read".to_string(),
            "user:write".to_string(),
            "user:delete".to_string(),
        ]),
    }
}

fn create_test_tools() -> Arc<GlobalTools> {
    let token_manager = TokenManager::new_symmetric(
        "test_secret_key",
        Algorithm::HS256,
        "test_issuer".to_string(),
        "test_audience".to_string(),
        3600,
        86400,
    );
    Arc::new(GlobalTools::new(token_manager))
}

/// 构造一个绑定表定义并注册全部内置 Action 的路由器。
fn router_with_builtins() -> ModuleRouter {
    ModuleRouter::new("user", "用户管理")
        .table(test_user_table())
        .crud()
        .expect("注册内置 Actions 应该成功")
}

// ──────────────────────────────────────────────────────────────────────────────
// 构建器与注册
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_module_router_new() {
    let router = ModuleRouter::new("user", "用户管理");

    assert_eq!(router.module_name(), "user");
    assert_eq!(router.display_name(), "用户管理");
    assert!(router.table_definition().is_none());
    assert_eq!(router.action_names().len(), 0);
}

#[test]
fn test_module_router_with_table_definition() {
    let router = ModuleRouter::new("user", "用户管理").table(test_user_table());

    assert!(router.table_definition().is_some());
    assert_eq!(
        router.table_definition().expect("表定义应存在").name(),
        "test_users"
    );
}

#[test]
fn test_crud_registers_six_actions() {
    let router = router_with_builtins();

    let action_names = router.action_names();
    assert_eq!(action_names.len(), 6);
    for name in BUILTIN_ACTION_NAMES {
        assert!(
            action_names.contains(&name.to_string()),
            "内置 Action '{}' 应该已注册",
            name
        );
    }
}

#[test]
fn test_builtin_action_names_consistency() {
    let router = router_with_builtins();
    assert_eq!(router.action_names().len(), BUILTIN_ACTION_NAMES.len());
}

/// 未设置表定义时 crud 返回 Err 而非 panic。
#[test]
fn test_crud_without_table_definition() {
    // 验证需求: 2.1, 2.2
    let result = ModuleRouter::new("user", "用户管理").crud();

    assert!(matches!(result, Err(BaseError::TableDefinitionNotSet)));
}

#[test]
fn test_default_permissions_rejects_blank_permission_name() {
    let result = ModuleRouter::new("user", "用户管理")
        .default_permissions(vec!["user:read".to_string(), "   ".to_string()]);

    assert!(matches!(
        result,
        Err(BaseError::ConfigError(msg)) if msg.contains("默认权限名称不能为空")
    ));
}

#[test]
fn test_default_permissions_rejects_duplicate_permission_name() {
    let result = ModuleRouter::new("user", "用户管理")
        .default_permissions(vec!["user:read".to_string(), "user:read".to_string()]);

    assert!(matches!(
        result,
        Err(BaseError::ConfigError(msg)) if msg.contains("默认权限重复: user:read")
    ));
}

#[test]
fn test_api_rejects_duplicate_action_name() {
    let router = router_with_builtins();

    let result = router.api(Api::get("/other/schema", TableAction::new()));

    assert!(matches!(
        result,
        Err(BaseError::ConfigError(msg)) if msg.contains("Action 已注册: table")
    ));
}

#[test]
fn test_api_rejects_blank_action_name() {
    let result = ModuleRouter::new("user", "用户管理").api(Api::post("/blank", BlankNameAction));

    assert!(matches!(
        result,
        Err(BaseError::ConfigError(msg)) if msg.contains("Action 名称不能为空")
    ));
}

#[test]
fn test_api_rejects_blank_permission_name() {
    let result = ModuleRouter::new("user", "用户管理")
        .api(Api::post("/blank-permission", BlankPermissionAction));

    assert!(matches!(
        result,
        Err(BaseError::ConfigError(msg)) if msg.contains("Action 权限名称不能为空")
    ));
}

// ──────────────────────────────────────────────────────────────────────────────
// dispatch 鉴权路径（不触达数据库）
// ──────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_dispatch_action_not_found() {
    let router = router_with_builtins();
    let context = ActionContext::new(Request::new(json!({})), create_test_tools());

    let result = router.dispatch("nonexistent", context).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        BaseError::ActionNotFound(name) => assert_eq!(name, "nonexistent"),
        e => panic!("期望 ActionNotFound 错误，实际: {:?}", e),
    }
}

#[tokio::test]
async fn test_dispatch_unauthorized() {
    let router = router_with_builtins();
    let request = Request::new(json!({ "username": "alice" }));
    let context = ActionContext::new(request, create_test_tools());

    // add 需要认证，但未提供用户信息 → Unauthorized（在触达数据库前返回）
    let result = router.dispatch("add", context).await;

    assert!(matches!(result, Err(BaseError::Unauthorized(_))));
}

#[tokio::test]
async fn test_dispatch_permission_denied() {
    let router = router_with_builtins()
        .default_permissions(vec!["admin:access".to_string()])
        .expect("有效默认权限应设置成功");

    let user = User {
        id: 1,
        username: "test_user".to_string(),
        nickname: "测试用户".to_string(),
        email: "test@example.com".to_string(),
        roles: HashSet::from(["user".to_string()]),
        permissions: HashSet::from(["user:read".to_string()]),
    };
    let request = Request::new(json!({ "username": "alice" }));
    let context = ActionContext::new(request, create_test_tools()).with_user(user);

    // 用户缺少 admin:access → PermissionDenied（在触达数据库前返回）
    let result = router.dispatch("add", context).await;

    assert!(matches!(result, Err(BaseError::PermissionDenied(_))));
}

#[tokio::test]
async fn test_dispatch_with_sufficient_permissions_passes_authz() {
    let router = router_with_builtins()
        .default_permissions(vec!["user:write".to_string()])
        .expect("有效默认权限应设置成功");

    let user = create_test_user();
    let request = Request::new(json!({ "username": "alice" }));
    let context = ActionContext::new(request, create_test_tools()).with_user(user);

    // 鉴权通过后会因无数据库连接而失败，但绝不应是鉴权类错误
    let result = router.dispatch("add", context).await;
    if let Err(e) = result {
        assert!(!matches!(e, BaseError::Unauthorized(_)));
        assert!(!matches!(e, BaseError::PermissionDenied(_)));
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// 中间件机制（H-5）
// ──────────────────────────────────────────────────────────────────────────────

/// 短路中间件：不调用 next，直接返回成功，避免触达数据库。
struct ShortCircuitMiddleware {
    payload: String,
}

#[async_trait]
impl Middleware for ShortCircuitMiddleware {
    async fn handle(&self, _ctx: ActionContext, _next: Next<'_>) -> Result<ApiResponse, BaseError> {
        Ok(ApiResponse::success_value(
            json!({ "from": self.payload }),
            "ok",
        ))
    }
}

/// 记录进入/离开顺序的中间件。
struct OrderRecordingMiddleware {
    id: usize,
    log: Arc<std::sync::Mutex<Vec<usize>>>,
}

#[async_trait]
impl Middleware for OrderRecordingMiddleware {
    async fn handle(&self, ctx: ActionContext, next: Next<'_>) -> Result<ApiResponse, BaseError> {
        self.log.lock().unwrap().push(self.id);
        let result = next.run(ctx).await;
        // 离开时记录负向标记（id + 1000）以区分进入/离开
        self.log.lock().unwrap().push(self.id + 1000);
        result
    }
}

#[tokio::test]
async fn test_middleware_short_circuit() {
    let router = router_with_builtins().middleware(ShortCircuitMiddleware {
        payload: "intercepted".to_string(),
    });

    let request = Request::new(json!({ "username": "alice" }));
    let context = ActionContext::new(request, create_test_tools());

    // 短路中间件在鉴权后立即返回，不触达 add 的数据库逻辑，也不报 Unauthorized
    let result = router.dispatch("add", context).await;
    assert!(result.is_ok(), "短路中间件应直接返回成功: {:?}", result);
}

#[tokio::test]
async fn test_middleware_onion_order() {
    let log = Arc::new(std::sync::Mutex::new(Vec::new()));

    // 先注册的最先进入、最后离开（洋葱模型）；链尾用短路中间件终止，避免 DB。
    let router = router_with_builtins()
        .middleware(OrderRecordingMiddleware {
            id: 1,
            log: log.clone(),
        })
        .middleware(OrderRecordingMiddleware {
            id: 2,
            log: log.clone(),
        })
        .middleware(ShortCircuitMiddleware {
            payload: "end".to_string(),
        });

    let request = Request::new(json!({ "username": "alice" }));
    let context = ActionContext::new(request, create_test_tools());

    let result = router.dispatch("add", context).await;
    assert!(result.is_ok());

    // 期望顺序：进入1 -> 进入2 -> (短路) -> 离开2 -> 离开1
    let recorded = log.lock().unwrap().clone();
    assert_eq!(recorded, vec![1, 2, 1002, 1001]);
}
