//! ModuleRouter 集成测试（H-1 类型化迁移后重写）
#![cfg(all(feature = "mysql", feature = "token"))]

use crate::action::{ActionContext, ApiResponse, GlobalTools, Request, User};
use crate::action::builtin::TableAction;
use crate::action::meta::ActionMeta;
use crate::action::{PermissionMode, TypedAction, TypedHandler};
use crate::error::BaseError;
use crate::router::{Middleware, ModuleRouter, Next, BUILTIN_ACTION_NAMES};
use crate::table::TableEntity;
use crate::token::TokenManager;
use async_trait::async_trait;
use jsonwebtoken::Algorithm;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;
use std::sync::{Arc, OnceLock};

// ──────────────────────────────────────────────────────────────────────────────
// 测试用类型化实体
// ──────────────────────────────────────────────────────────────────────────────

#[derive(
    Debug,
    Deserialize,
    Serialize,
    schemars::JsonSchema,
    sqlx::FromRow,
    yang_base_derive::TableEntity,
)]
#[table(name = "test_users", display_name = "测试用户表")]
pub struct TestUser {
    #[entity(primary_key)]
    pub id: i64,
    #[entity(max_length = 50, unique)]
    pub username: String,
    #[entity(max_length = 100)]
    pub email: Option<String>,
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

/// 构造一个注册了全部内置 Action 的路由器（带 table_config）。
fn router_with_builtins() -> ModuleRouter {
    ModuleRouter::new("user", "用户管理")
        .with_table_config(Arc::new(TestUser::table_config().clone()))
        .table_typed::<TestUser>()
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
    assert!(router.get_table_config().is_none());
    assert_eq!(router.action_names().len(), 0);
}

#[test]
fn test_module_router_with_table_config() {
    let router = ModuleRouter::new("user", "用户管理")
        .with_table_config(Arc::new(TestUser::table_config().clone()));

    assert!(router.get_table_config().is_some());
    assert_eq!(router.get_table_config().unwrap().table_name, "test_users");
}

#[test]
fn test_table_typed_registers_six_actions() {
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

/// 未设置 table_config 时 table_typed 返回 Err 而非 panic。
#[test]
fn test_table_typed_without_table_config() {
    // 验证需求: 2.1, 2.2
    let result = ModuleRouter::new("user", "用户管理").table_typed::<TestUser>();

    assert!(result.is_err());
    match result.err().unwrap() {
        BaseError::TableConfigNotSet => {}
        e => panic!("期望 TableConfigNotSet 错误，实际得到: {:?}", e),
    }
}

#[test]
fn test_register_action_rejects_duplicate_action_name() {
    let router = router_with_builtins();

    let result = router.register_action(TableAction::<TestUser>::new());

    assert!(matches!(
        result,
        Err(BaseError::ConfigError(msg)) if msg.contains("Action 已注册: table")
    ));
}

#[test]
fn test_register_action_rejects_blank_action_name() {
    let result = ModuleRouter::new("user", "用户管理").register_action(BlankNameAction);

    assert!(matches!(
        result,
        Err(BaseError::ConfigError(msg)) if msg.contains("Action 名称不能为空")
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
    let request = Request::new(json!({ "data": { "username": "alice" } }));
    let context = ActionContext::new(request, create_test_tools());

    // add 需要认证，但未提供用户信息 → Unauthorized（在触达数据库前返回）
    let result = router.dispatch("add", context).await;

    assert!(matches!(result, Err(BaseError::Unauthorized(_))));
}

#[tokio::test]
async fn test_dispatch_permission_denied() {
    let router = router_with_builtins().default_permissions(vec!["admin:access".to_string()]);

    let user = User {
        id: 1,
        username: "test_user".to_string(),
        nickname: "测试用户".to_string(),
        email: "test@example.com".to_string(),
        roles: HashSet::from(["user".to_string()]),
        permissions: HashSet::from(["user:read".to_string()]),
    };
    let request = Request::new(json!({ "data": { "username": "alice" } }));
    let context = ActionContext::new(request, create_test_tools()).with_user(user);

    // 用户缺少 admin:access → PermissionDenied（在触达数据库前返回）
    let result = router.dispatch("add", context).await;

    assert!(matches!(result, Err(BaseError::PermissionDenied(_))));
}

#[tokio::test]
async fn test_dispatch_with_sufficient_permissions_passes_authz() {
    let router = router_with_builtins().default_permissions(vec!["user:write".to_string()]);

    let user = create_test_user();
    let request = Request::new(json!({ "data": { "username": "alice" } }));
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

    let request = Request::new(json!({ "data": { "username": "alice" } }));
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

    let request = Request::new(json!({ "data": { "username": "alice" } }));
    let context = ActionContext::new(request, create_test_tools());

    let result = router.dispatch("add", context).await;
    assert!(result.is_ok());

    // 期望顺序：进入1 -> 进入2 -> (短路) -> 离开2 -> 离开1
    let recorded = log.lock().unwrap().clone();
    assert_eq!(recorded, vec![1, 2, 1002, 1001]);
}
