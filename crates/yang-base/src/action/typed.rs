//! 类型化 Action 三层 trait
//!
//! - `TypedHandler`: 用户唯一手写的 trait，处理 Input -> Output
//! - `TypedAction`: 元信息层（由 `#[derive(Action)]` 派生）
//! - `DynAction`: object-safe 擦除层，存入 router 派发
//!
//! 通过 `blanket impl<T: TypedAction> DynAction for T` 自动桥接。

use crate::action::action_trait::Permission;
use crate::action::{ActionContext, ApiResponse};
use crate::error::BaseError;
use async_trait::async_trait;

use super::meta::ActionMeta;

/// 用户业务逻辑 trait。Input/Output 是编译期契约。
#[async_trait]
pub trait TypedHandler: Send + Sync + 'static {
    /// 输入类型（请求体反序列化目标）
    type Input: serde::de::DeserializeOwned + schemars::JsonSchema + Send;

    /// 输出类型（响应数据序列化源）
    type Output: serde::Serialize + schemars::JsonSchema + Send;

    /// 业务执行
    async fn handle(
        &self,
        ctx: ActionContext,
        input: Self::Input,
    ) -> Result<Self::Output, BaseError>;
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

    /// 所需权限列表（默认为空）
    fn permissions(&self) -> &'static [Permission] {
        &[]
    }

    /// 是否公开（默认 false）
    fn is_public(&self) -> bool {
        false
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
/// Task 7 重命名为 `Action`。当前临时名以避免与旧 `Action` trait 冲突。
#[async_trait]
pub trait DynAction: Send + Sync + 'static {
    /// 派发：从 ctx 中提取输入、执行业务逻辑、封装响应
    async fn dispatch(&self, ctx: ActionContext) -> Result<ApiResponse, BaseError>;

    /// 获取静态元信息
    fn meta(&self) -> &'static ActionMeta;
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

        let result: Result<ApiResponse, BaseError> = async {
            let input: T::Input = ctx.extract_input()?;
            let output = self.handle(ctx, input).await?;
            ApiResponse::success(output, "成功")
        }
        .instrument(span)
        .await;

        #[cfg(feature = "metrics")]
        {
            let elapsed = start.elapsed().as_secs_f64();
            metrics::histogram!("yang_action_duration_seconds", "action" => action_name)
                .record(elapsed);
            match &result {
                Ok(_) => {
                    metrics::counter!(
                        "yang_action_requests_total",
                        "action" => action_name,
                        "status" => "ok",
                    )
                    .increment(1);
                }
                Err(e) => {
                    metrics::counter!(
                        "yang_action_requests_total",
                        "action" => action_name,
                        "status" => "error",
                    )
                    .increment(1);
                    metrics::counter!(
                        "yang_action_errors_total",
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
}
