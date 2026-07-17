//! #[derive(Action)] 派生宏测试

use crate::action::{ActionContext, DynAction, TypedAction, TypedHandler};
use crate::definition::{ActionResponseKind, UploadLifecycle};
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
    permissions("system:ping"),
    response_kind = "redirect"
)]
pub struct PingAction;

#[derive(Action)]
#[action(
    name = "upload_avatar",
    request_media = "multipart",
    content_types("image/png", "image/jpeg"),
    max_fields = 4,
    max_files = 2,
    max_file_bytes = 1048576,
    max_total_bytes = 2097152
)]
pub struct UploadAvatarAction;

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

#[async_trait]
impl TypedHandler for UploadAvatarAction {
    type Input = PingInput;
    type Output = PingOutput;

    async fn handle(&self, _ctx: ActionContext, input: PingInput) -> Result<PingOutput, BaseError> {
        Ok(PingOutput {
            reply: format!("uploaded: {}", input.msg),
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
    assert_eq!(a.response_kind(), ActionResponseKind::Redirect);
}

#[test]
fn test_derive_action_meta_static_dyn() {
    let a: &dyn DynAction = &PingAction;
    let m = a.meta();
    assert_eq!(m.name, "ping");
    assert!(m.is_public);
    let v_in = serde_json::to_value(m.input_schema).unwrap();
    assert!(v_in.is_object(), "input_schema 应序列化为 JSON object");
    let v_out = serde_json::to_value(m.output_schema).unwrap();
    assert!(v_out.is_object(), "output_schema 应序列化为 JSON object");
}

#[test]
fn test_derive_action_multipart_contract() {
    let spec = UploadAvatarAction
        .multipart_spec()
        .expect("multipart Action 应生成上传契约");
    assert_eq!(spec.max_fields, 4);
    assert_eq!(spec.max_files, 2);
    assert_eq!(spec.max_file_bytes, 1_048_576);
    assert_eq!(spec.max_total_bytes, 2_097_152);
    assert_eq!(spec.allowed_content_types, ["image/png", "image/jpeg"]);
    assert_eq!(spec.lifecycle, UploadLifecycle::RequestScoped);
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
    assert_eq!(
        a.display_name(),
        "minimal",
        "display_name 缺失时应回退到 name"
    );
    assert_eq!(a.description(), "");
    assert!(!a.is_public(), "默认非公开");
    assert!(a.permissions().is_empty());
    assert_eq!(a.response_kind(), ActionResponseKind::Json);
}
