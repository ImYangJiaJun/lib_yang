//! 构建期预解析的强类型 Action 调用入口。

use super::{Registry, TypedActionHandle};
use crate::action::ActionContext;
use crate::error::BaseError;
use std::sync::Arc;
use std::sync::OnceLock;

/// 构建期绑定、请求期仅复制 slot 的强类型内部 Action 引用。
pub struct ActionLink<I, O> {
    reference: super::ActionRef,
    handle: Arc<OnceLock<TypedActionHandle<I, O>>>,
}

impl<I, O> Clone for ActionLink<I, O> {
    fn clone(&self) -> Self {
        Self {
            reference: self.reference.clone(),
            handle: Arc::clone(&self.handle),
        }
    }
}

impl<I, O> ActionLink<I, O>
where
    I: Send + 'static,
    O: Send + 'static,
{
    /// 声明一个待 AppBuilder 解析的 ActionRef。
    pub fn new(reference: super::ActionRef) -> Self {
        Self {
            reference,
            handle: Arc::new(OnceLock::new()),
        }
    }

    /// 返回定义期引用，供 Action::calls 声明依赖。
    pub fn reference(&self) -> &super::ActionRef {
        &self.reference
    }

    /// 在 AppBuilder 构建期解析并冻结 slot。
    pub fn bind(&self, registry: &Registry) -> Result<(), BaseError> {
        let handle = registry.resolve_typed(&self.reference)?;
        self.handle.set(handle).map_err(|_| {
            BaseError::ConfigError(format!("ActionLink {} 被重复绑定", self.reference))
        })
    }

    /// 请求期取得已绑定 handle；仅执行 OnceLock 读取，不做名称查找。
    pub fn handle(&self) -> Result<TypedActionHandle<I, O>, BaseError> {
        self.handle.get().copied().ok_or_else(|| {
            BaseError::ConfigError(format!("ActionLink {} 尚未绑定", self.reference))
        })
    }
}

/// 请求内的强类型 Action 调用器。
///
/// `Plugins` 拥有当前 `ActionContext`，因此用户、租户、request id 与 Tools 会原样
/// 传给目标 Action。调用使用预解析 slot 和 Rust 值，不经过字符串分派或 JSON。
pub struct Plugins {
    registry: Arc<Registry>,
    context: ActionContext,
}

impl Plugins {
    pub(crate) fn new(registry: Arc<Registry>, context: ActionContext) -> Self {
        Self { registry, context }
    }

    /// 直接调用预解析 Action。
    pub async fn api_run<I, O>(
        self,
        handle: TypedActionHandle<I, O>,
        input: I,
    ) -> Result<O, BaseError>
    where
        I: Send + 'static,
        O: Send + 'static,
    {
        self.registry.call(handle, self.context, input).await
    }
}
