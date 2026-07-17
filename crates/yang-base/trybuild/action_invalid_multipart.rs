use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use yang_base::action::{ActionContext, TypedHandler};
use yang_base::error::BaseError;

#[derive(Debug, Deserialize, JsonSchema)]
struct Input;

#[derive(Debug, Serialize, JsonSchema)]
struct Output;

#[derive(yang_base::Action)]
#[action(name = "upload", request_media = "multipart")]
struct InvalidMultipart;

#[async_trait]
impl TypedHandler for InvalidMultipart {
    type Input = Input;
    type Output = Output;

    async fn handle(&self, _ctx: ActionContext, _input: Input) -> Result<Output, BaseError> {
        Ok(Output)
    }
}

fn main() {}
