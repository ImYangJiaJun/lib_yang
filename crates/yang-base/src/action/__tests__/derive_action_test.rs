//! #[derive(Action)] 派生宏测试

use crate::action::{ActionContext, DynAction, TypedAction, TypedHandler};
use crate::error::BaseError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use yang_base_derive::Action;

#[derive(Deserialize, schemars::JsonSchema)]
pub struct PingInput {
    msg: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PingOutput {
    reply: String,
}

#[derive(Action)]
#[action(
    name = "ping",
    public,
    display_name = "心跳",
    description = "测试连通性",
    permissions("system:ping")
)]
pub struct PingAction;

#[async_trait]
impl TypedHandler for PingAction {
    type Input = PingInput;
    type Output = PingOutput;
    async fn handle(&self, _ctx: ActionContext, input: PingInput) -> Result<PingOutput, BaseError> {
        Ok(PingOutput {
            reply: format!("pong: {}", input.msg),
        })
    }
}

#[test]
fn test_derive_action_meta_correct() {
    let a = PingAction;
    assert_eq!(a.name(), "ping");
    assert_eq!(a.display_name(), "心跳");
    assert_eq!(a.description(), "测试连通性");
    assert!(a.is_public());
    let perms = a.permissions();
    assert_eq!(perms.len(), 1);
    assert_eq!(perms[0].name(), "system:ping");
}

#[test]
fn test_derive_action_meta_static_dyn() {
    let a: &dyn DynAction = &PingAction;
    let m = a.meta();
    assert_eq!(m.name, "ping");
    assert!(m.is_public);
    // schema 非空
    let v = serde_json::to_value(m.input_schema).unwrap();
    assert!(v.is_object());
}

#[test]
fn test_derive_action_default_values() {
    // 没有 permissions / 没有 description / 没有 display_name 的最小例子
    #[derive(Deserialize, schemars::JsonSchema)]
    struct EmptyInput;

    #[derive(Serialize, schemars::JsonSchema)]
    struct EmptyOutput;

    #[derive(Action)]
    #[action(name = "minimal")]
    struct MinimalAction;

    #[async_trait]
    impl TypedHandler for MinimalAction {
        type Input = EmptyInput;
        type Output = EmptyOutput;
        async fn handle(
            &self,
            _ctx: ActionContext,
            _: EmptyInput,
        ) -> Result<EmptyOutput, BaseError> {
            Ok(EmptyOutput)
        }
    }

    let a = MinimalAction;
    assert_eq!(a.name(), "minimal");
    assert_eq!(a.display_name(), "minimal", "display_name 缺失时应回退到 name");
    assert_eq!(a.description(), "");
    assert!(!a.is_public(), "默认非公开");
    assert!(a.permissions().is_empty());
}
