//! 类型化 Action 三层 trait
//!
//! - `TypedHandler`: 用户唯一手写的 trait，处理 Input -> Output
//! - `TypedAction`: 元信息层（由 `#[derive(Action)]` 派生）
//! - `DynAction`: object-safe 擦除层，存入 router 派发
//!
//! 通过 `blanket impl<T: TypedAction> DynAction for T` 自动桥接。

use crate::action::action_trait::{Permission, PermissionMode};
use crate::action::{ActionContext, ApiResponse};
use crate::error::BaseError;
use async_trait::async_trait;
use std::any::{Any, TypeId};
use std::future::Future;
use std::pin::Pin;

use super::meta::ActionMeta;

/// 面向业务的永久 Action 接口：params 由强类型 Input 生成，业务只实现 index。
#[async_trait]
pub trait Action: Send + Sync + 'static {
    /// `params!` 生成的强类型输入。
    type Input: crate::definition::ParamInput
        + serde::de::DeserializeOwned
        + schemars::JsonSchema
        + Send
        + 'static;

    /// 强类型输出。
    type Output: serde::Serialize + schemars::JsonSchema + Send + 'static;

    /// 返回 Input 的唯一原生参数定义。
    fn params() -> crate::definition::Params {
        <Self::Input as crate::definition::ParamInput>::params()
    }

    /// 声明内部 Action 依赖；AppBuilder 在构建期交叉校验。
    fn calls(&self) -> Vec<crate::definition::ActionRef> {
        Vec::new()
    }

    /// 在 Registry 冻结后绑定 ActionLink；请求期不再解析名称。
    fn bind_registry(&self, _registry: &crate::definition::Registry) -> Result<(), BaseError> {
        Ok(())
    }

    /// 执行业务逻辑。
    async fn index(
        &self,
        ctx: ActionContext,
        input: Self::Input,
    ) -> Result<Self::Output, BaseError>;
}

#[async_trait]
impl<T> TypedHandler for T
where
    T: Action,
{
    type Input = T::Input;
    type Output = T::Output;

    async fn handle(
        &self,
        ctx: ActionContext,
        input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        self.index(ctx, input).await
    }

    fn handle_future(
        &self,
        ctx: ActionContext,
        input: Self::Input,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Output, BaseError>> + Send + '_>> {
        <T as Action>::index(self, ctx, input)
    }

    fn decode_input(&self, ctx: &mut ActionContext) -> Result<Self::Input, BaseError> {
        <Self::Input as crate::definition::ParamInput>::decode(&mut ctx.request)
    }

    fn bind_registry(&self, registry: &crate::definition::Registry) -> Result<(), BaseError> {
        <T as Action>::bind_registry(self, registry)
    }
}

/// 用户业务逻辑 trait。Input/Output 是编译期契约。
#[async_trait]
pub trait TypedHandler: Send + Sync + 'static {
    /// 输入类型（请求体反序列化目标）
    type Input: serde::de::DeserializeOwned + schemars::JsonSchema + Send + 'static;

    /// 输出类型（响应数据序列化源）
    type Output: serde::Serialize + schemars::JsonSchema + Send + 'static;

    /// 从请求构造 Input。旧 TypedHandler 默认读取 body；原生 Action 由 ParamInput
    /// 按 body/query/path/header 的静态定义覆盖该行为。
    fn decode_input(&self, ctx: &mut ActionContext) -> Result<Self::Input, BaseError> {
        ctx.extract_input_owned()
    }

    /// AppBuilder 构建期依赖绑定钩子。
    fn bind_registry(&self, _registry: &crate::definition::Registry) -> Result<(), BaseError> {
        Ok(())
    }

    /// 业务执行
    async fn handle(
        &self,
        ctx: ActionContext,
        input: Self::Input,
    ) -> Result<Self::Output, BaseError>;

    /// 返回 Handler future；原生 Action 覆盖此入口以避免 async-trait 适配层二次装箱。
    fn handle_future(
        &self,
        ctx: ActionContext,
        input: Self::Input,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Output, BaseError>> + Send + '_>> {
        self.handle(ctx, input)
    }
}

/// 元信息层。由 `#[derive(Action)]` 自动实现；用户不手写。
pub trait TypedAction: TypedHandler {
    /// Action 唯一标识
    fn name(&self) -> &'static str;

    /// 用户可见的显示名（默认返回 name）
    fn display_name(&self) -> &'static str {
        self.name()
    }

    /// 简介（默认返回空字符串）
    fn description(&self) -> &'static str {
        ""
    }

    /// 原子注册的 HTTP method；默认 POST。
    fn http_method(&self) -> crate::definition::HttpMethod {
        crate::definition::HttpMethod::Post
    }

    /// 原子注册的 HTTP path；未显式配置时由 Action 名生成 `/<name>`。
    fn path(&self) -> &'static str {
        ""
    }

    /// 成功响应状态码。
    fn success_status(&self) -> u16 {
        200
    }

    /// 所需权限列表（默认为空）
    fn permissions(&self) -> &'static [Permission] {
        &[]
    }

    /// 是否公开（默认 false）
    fn is_public(&self) -> bool {
        false
    }

    /// 权限匹配模式（默认 All，即 AND 语义）
    fn permission_mode(&self) -> PermissionMode {
        PermissionMode::default()
    }

    /// 入参 JSON Schema（OnceLock 生成）
    fn input_schema(&self) -> &'static schemars::schema::RootSchema;

    /// 出参 JSON Schema
    fn output_schema(&self) -> &'static schemars::schema::RootSchema;

    /// 静态元信息聚合体
    fn meta_static(&self) -> &'static ActionMeta;
}

/// 擦除层：router 存 `Arc<dyn DynAction>` 派发。
///
/// 名称刻意与 crate 根的 `#[derive(Action)]` 宏区分；业务代码无需直接实现本 trait。
#[async_trait]
pub trait DynAction: Send + Sync + 'static {
    /// 派发：从 ctx 中提取输入、执行业务逻辑、封装响应
    async fn dispatch(&self, ctx: ActionContext) -> Result<ApiResponse, BaseError>;

    /// 获取静态元信息
    fn meta(&self) -> &'static ActionMeta;

    /// 返回强类型输入的 TypeId；手写擦除实现默认不支持内部强类型调用。
    fn input_type_id(&self) -> TypeId {
        TypeId::of::<()>()
    }

    /// 返回强类型输出的 TypeId；手写擦除实现默认不支持内部强类型调用。
    fn output_type_id(&self) -> TypeId {
        TypeId::of::<()>()
    }

    /// 使用已经反序列化的输入直接调用 Handler，不经过 JSON。
    async fn call_boxed(
        &self,
        _ctx: ActionContext,
        _input: Box<dyn Any + Send>,
    ) -> Result<Box<dyn Any + Send>, BaseError> {
        Err(BaseError::ConfigError(
            "该 Action 未提供强类型内部调用入口".to_string(),
        ))
    }

    /// Registry 冻结后的构建期绑定钩子。
    fn bind_registry(&self, _registry: &crate::definition::Registry) -> Result<(), BaseError> {
        Ok(())
    }
}

/// Blanket 桥接：所有 TypedAction 自动是 DynAction。
#[async_trait]
impl<T: TypedAction> DynAction for T {
    async fn dispatch(&self, ctx: ActionContext) -> Result<ApiResponse, BaseError> {
        use tracing::Instrument;

        let action_name = self.meta().name;
        // handler span：静态 span 名 + 借用的 action 名，成功路径零分配
        let span = tracing::info_span!("handle", action = action_name);

        // feature="metrics" 时在唯一必经边界埋点；关闭时整段 #[cfg] 消失零开销
        #[cfg(feature = "metrics")]
        let start = std::time::Instant::now();
        // 在 ctx 被移动进 async 块前，捕获 module 标签（NEW-2：跨模块同名 Action 区分）。
        // 未经路由的上下文 module 为 None，回退为 "unknown" 保持低基数。
        #[cfg(feature = "metrics")]
        let module: String = ctx.module.clone().unwrap_or_else(|| "unknown".to_string());

        let result: Result<ApiResponse, BaseError> = async {
            let mut ctx = ctx;
            let input: T::Input = self.decode_input(&mut ctx)?;
            let output = self.handle_future(ctx, input).await?;
            ApiResponse::success(output, "成功")
        }
        .instrument(span)
        .await;

        #[cfg(feature = "metrics")]
        {
            let elapsed = start.elapsed().as_secs_f64();
            metrics::histogram!(
                "yang_action_duration_seconds",
                "module" => module.clone(),
                "action" => action_name,
            )
            .record(elapsed);
            match &result {
                Ok(_) => {
                    metrics::counter!(
                        "yang_action_requests_total",
                        "module" => module.clone(),
                        "action" => action_name,
                        "status" => "ok",
                    )
                    .increment(1);
                }
                Err(e) => {
                    metrics::counter!(
                        "yang_action_requests_total",
                        "module" => module.clone(),
                        "action" => action_name,
                        "status" => "error",
                    )
                    .increment(1);
                    metrics::counter!(
                        "yang_action_errors_total",
                        "module" => module,
                        "action" => action_name,
                        "code" => e.code_str(),
                    )
                    .increment(1);
                }
            }
        }

        result
    }

    fn meta(&self) -> &'static ActionMeta {
        TypedAction::meta_static(self)
    }

    fn input_type_id(&self) -> TypeId {
        TypeId::of::<T::Input>()
    }

    fn output_type_id(&self) -> TypeId {
        TypeId::of::<T::Output>()
    }

    async fn call_boxed(
        &self,
        ctx: ActionContext,
        input: Box<dyn Any + Send>,
    ) -> Result<Box<dyn Any + Send>, BaseError> {
        let input = input.downcast::<T::Input>().map_err(|_| {
            BaseError::ConfigError(format!("Action {} 的内部调用输入类型不匹配", self.name()))
        })?;
        let output = self.handle_future(ctx, *input).await?;
        Ok(Box::new(output))
    }

    fn bind_registry(&self, registry: &crate::definition::Registry) -> Result<(), BaseError> {
        TypedHandler::bind_registry(self, registry)
    }
}
