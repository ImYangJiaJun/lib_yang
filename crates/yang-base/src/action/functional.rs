//! 函数式 Action：普通 async fn / 闭包作为 Handler 的类型擦除承载。
//!
//! 与 `#[derive(Action)]` 通道的差异只在定义期：函数式 Action 的 route/params/
//! 权限/Schema 全部由 `ModuleSpec::action_fn` 返回的终结式 Builder 写入
//! `ActionSpec`（唯一事实来源），运行期与派生通道共用同一条
//! `Registry::dispatch` / `Registry::call` 路径。

use super::action_trait::PermissionMode;
use super::meta::ActionMeta;
use super::{ActionContext, ApiResponse, DynAction};
use crate::definition::ParamInput;
use crate::error::BaseError;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::any::{Any, TypeId};
use std::future::Future;
use std::marker::PhantomData;
use std::sync::OnceLock;

/// 由普通函数承载的 Action。
///
/// 泛型 `I`/`O` 与 Handler 签名一一对应；`PhantomData<fn(I) -> O>` 只承担
/// 型变与 Send/Sync 自动实现的标记职责，不持有任何数据。
pub(crate) struct FnAction<F, I, O> {
    handler: F,
    marker: PhantomData<fn(I) -> O>,
}

impl<F, I, O> FnAction<F, I, O> {
    /// 包装业务函数；定义期元数据由调用方（`ActionFnBuilder`）写入 ActionSpec。
    pub(crate) fn new(handler: F) -> Self {
        Self {
            handler,
            marker: PhantomData,
        }
    }
}

/// `DynAction::meta` 的占位实现。
///
/// 函数式 Action 的元数据以注册期构建的 `ActionSpec` 为唯一事实来源：
/// `ActionFnBuilder::register` 已用 `schemars::schema_for!` 填充 input/output
/// Schema，使 `ActionSpec::bind_handler_contract` 跳过 `handler.meta()`；
/// 自身的 dispatch 也不读取本值。占位 `meta()` 仅为满足 trait 签名，
/// 无运行期语义。
fn placeholder_meta() -> &'static ActionMeta {
    static INPUT_SCHEMA: OnceLock<schemars::schema::RootSchema> = OnceLock::new();
    static OUTPUT_SCHEMA: OnceLock<schemars::schema::RootSchema> = OnceLock::new();
    static META: OnceLock<ActionMeta> = OnceLock::new();
    META.get_or_init(|| {
        ActionMeta::new(
            "<functional>",
            "<functional>",
            "函数式 Action 的占位元信息；真实定义以注册期 ActionSpec 为准",
            &[],
            PermissionMode::All,
            false,
            INPUT_SCHEMA.get_or_init(|| schemars::schema_for!(serde_json::Value)),
            OUTPUT_SCHEMA.get_or_init(|| schemars::schema_for!(serde_json::Value)),
        )
    })
}

/// 与 `TypedHandler` blanket 桥接同构的擦除实现。
#[async_trait]
impl<F, I, O, Fut> DynAction for FnAction<F, I, O>
where
    F: Fn(ActionContext, I) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<O, BaseError>> + Send,
    I: ParamInput + DeserializeOwned + JsonSchema + Send + 'static,
    O: Serialize + JsonSchema + Send + 'static,
{
    async fn dispatch(&self, ctx: ActionContext) -> Result<ApiResponse, BaseError> {
        // 与 blanket impl 同构：输入按 ParamInput 声明从 body/query/path/header
        // 只解码一次，输出经统一收口（ResponseBody 转附件，其余序列化进 data）。
        // operation/module/action 标签由外层 Registry::dispatch 的 span 记录，
        // 这里不再重复埋点。
        let mut ctx = ctx;
        let input = I::decode(&mut ctx.request)?;
        let output = (self.handler)(ctx, input).await?;
        super::response::wrap_dispatch_output(output, "成功")
    }

    fn meta(&self) -> &'static ActionMeta {
        placeholder_meta()
    }

    fn input_type_id(&self) -> TypeId {
        TypeId::of::<I>()
    }

    fn output_type_id(&self) -> TypeId {
        TypeId::of::<O>()
    }

    async fn call_boxed(
        &self,
        ctx: ActionContext,
        input: Box<dyn Any + Send>,
    ) -> Result<Box<dyn Any + Send>, BaseError> {
        let input = input.downcast::<I>().map_err(|_| {
            BaseError::ConfigError("函数式 Action 的内部调用输入类型不匹配".to_string())
        })?;
        let output = (self.handler)(ctx, *input).await?;
        Ok(Box::new(output))
    }
}

/// 函数式 CRUD 写 Handler 的 `DynAction` 桥接。
///
/// `ModuleSpec::crud_at_with_mutations` 用表定义为写 Action 生成动态 Schema 与
/// 权限契约，只接受 `DynAction` 实例；而 [`FnAction`] 仅供 `action_fn` 通道内部
/// 使用。本桥接把普通 async fn 形态的函数式 Handler 包装成 `DynAction`，与
/// derive 通道保持同构语义：请求体只解码一次，输出统一收口进 `data`。
/// Catalog 契约（动态表驱动 Schema、权限、success_status）仍由
/// `crud_at_with_mutations` 生成，不读取本桥接的占位元信息。
pub struct CrudFnAction<F, I, O> {
    name: &'static str,
    handler: F,
    marker: PhantomData<fn(I) -> O>,
}

impl<F, I, O> CrudFnAction<F, I, O> {
    /// 绑定 Action 名（仅用于 tracing span）与业务函数。
    pub fn new(name: &'static str, handler: F) -> Self {
        Self {
            name,
            handler,
            marker: PhantomData,
        }
    }
}

#[async_trait]
impl<F, I, O, Fut> DynAction for CrudFnAction<F, I, O>
where
    F: Fn(ActionContext, I) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<O, BaseError>> + Send,
    I: DeserializeOwned + Send + 'static,
    O: Serialize + Send + 'static,
{
    async fn dispatch(&self, ctx: ActionContext) -> Result<ApiResponse, BaseError> {
        use tracing::Instrument;

        let span = tracing::info_span!("handle", action = self.name);
        async {
            let mut ctx = ctx;
            // 与 derive 通道的 TypedHandler::decode_input 一致：body 只反序列化一次。
            let body = std::mem::take(&mut ctx.request.body);
            let input: I = serde_json::from_value(body)
                .map_err(|error| BaseError::ParamInvalid("body".to_string(), error.to_string()))?;
            let output = (self.handler)(ctx, input).await?;
            // 写 Action 输出均为普通 JSON（InsertResult/AffectedResult），统一进 data。
            ApiResponse::success(output, "成功")
        }
        .instrument(span)
        .await
    }

    fn meta(&self) -> &'static ActionMeta {
        placeholder_meta()
    }

    fn input_type_id(&self) -> TypeId {
        TypeId::of::<I>()
    }

    fn output_type_id(&self) -> TypeId {
        TypeId::of::<O>()
    }

    async fn call_boxed(
        &self,
        ctx: ActionContext,
        input: Box<dyn Any + Send>,
    ) -> Result<Box<dyn Any + Send>, BaseError> {
        let input = input.downcast::<I>().map_err(|_| {
            BaseError::ConfigError("函数式 CRUD 写 Action 的内部调用输入类型不匹配".to_string())
        })?;
        let output = (self.handler)(ctx, *input).await?;
        Ok(Box::new(output))
    }
}
