//! TypedHandler / TypedAction / DynAction 单元测试
#![cfg(feature = "token")]

use crate::action::meta::ActionMeta;
use crate::action::typed::{DynAction, TypedAction, TypedHandler};
use crate::action::{ActionContext, Request};
use crate::error::BaseError;
use crate::token::TokenManager;
use crate::tools::ToolsBuilder;
use async_trait::async_trait;
use jsonwebtoken::Algorithm;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};

// ──────────────────────────────────────────────────────────────────────────────
// 测试用 Echo Action
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct EchoInput {
    msg: String,
}

#[derive(Serialize, schemars::JsonSchema)]
struct EchoOutput {
    echoed: String,
}

struct EchoAction;

#[async_trait]
impl TypedHandler for EchoAction {
    type Input = EchoInput;
    type Output = EchoOutput;

    async fn handle(&self, _ctx: ActionContext, input: EchoInput) -> Result<EchoOutput, BaseError> {
        Ok(EchoOutput { echoed: input.msg })
    }
}

impl TypedAction for EchoAction {
    fn name(&self) -> &'static str {
        "echo"
    }

    fn input_schema(&self) -> &'static schemars::schema::RootSchema {
        static S: OnceLock<schemars::schema::RootSchema> = OnceLock::new();
        S.get_or_init(|| schemars::schema_for!(EchoInput))
    }

    fn output_schema(&self) -> &'static schemars::schema::RootSchema {
        static S: OnceLock<schemars::schema::RootSchema> = OnceLock::new();
        S.get_or_init(|| schemars::schema_for!(EchoOutput))
    }

    fn meta_static(&self) -> &'static ActionMeta {
        static M: OnceLock<ActionMeta> = OnceLock::new();
        M.get_or_init(|| {
            static I: OnceLock<schemars::schema::RootSchema> = OnceLock::new();
            static O: OnceLock<schemars::schema::RootSchema> = OnceLock::new();
            ActionMeta {
                name: "echo",
                display_name: "echo",
                description: "",
                permissions: &[],
                permission_mode: crate::action::action_trait::PermissionMode::All,
                is_public: false,
                input_schema: I.get_or_init(|| schemars::schema_for!(EchoInput)),
                output_schema: O.get_or_init(|| schemars::schema_for!(EchoOutput)),
            }
        })
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// 辅助函数
// ──────────────────────────────────────────────────────────────────────────────

fn make_ctx(body_json: serde_json::Value) -> ActionContext {
    let token_manager = TokenManager::new_symmetric(
        "test_secret_key",
        Algorithm::HS256,
        "test_issuer".to_string(),
        "test_audience".to_string(),
        3600,
        86400,
    );
    let request = Request::new(body_json);
    let tools = Arc::new(
        ToolsBuilder::new()
            .token(token_manager)
            .build()
            .expect("测试 Tools 应构建成功"),
    );
    ActionContext::new(request, tools)
}

// ──────────────────────────────────────────────────────────────────────────────
// 测试
// ──────────────────────────────────────────────────────────────────────────────

/// blanket dispatch 完整 roundtrip：dispatch -> handle -> ApiResponse
#[tokio::test]
async fn test_blanket_dispatch_roundtrip() {
    let ctx = make_ctx(serde_json::json!({"msg": "hi"}));
    let action: &dyn DynAction = &EchoAction;
    let response = action.dispatch(ctx).await.expect("dispatch ok");
    assert_eq!(response.code, 0);
    let data = response.data.expect("data present");
    assert_eq!(data["echoed"], "hi");
}

/// TypedHandler 默认 body 解码在缺少必填字段时返回结构化错误。
#[tokio::test]
async fn test_extract_input_missing_field() {
    let ctx = make_ctx(serde_json::json!({}));
    let action: &dyn DynAction = &EchoAction;
    let result = action.dispatch(ctx).await;
    assert!(result.is_err(), "缺少必填字段应返回错误");
    match result.unwrap_err() {
        BaseError::ParamInvalid(field, _) => {
            assert_eq!(field, "body");
        }
        other => panic!("期望 ParamInvalid，实际: {:?}", other),
    }
}

/// meta 可通过 dyn DynAction 访问
#[test]
fn test_meta_accessible_through_dyn() {
    let action: &dyn DynAction = &EchoAction;
    assert_eq!(action.meta().name, "echo");
    assert!(!action.meta().is_public);
    assert!(action.meta().permissions.is_empty());
}
