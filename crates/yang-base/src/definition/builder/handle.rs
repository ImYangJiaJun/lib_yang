//! 预解析 Action slot 句柄与强类型内部调用句柄。

use std::marker::PhantomData;

/// 已预解析的 Action Registry slot。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActionHandle(pub(super) usize);

impl ActionHandle {
    /// 返回稳定 slot 索引。
    pub const fn slot(self) -> usize {
        self.0
    }
}

/// 带 Input/Output 类型信息的预解析内部调用句柄。
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypedActionHandle<I, O> {
    pub(super) raw: ActionHandle,
    pub(super) marker: PhantomData<fn(I) -> O>,
}

impl<I, O> Copy for TypedActionHandle<I, O> {}

impl<I, O> Clone for TypedActionHandle<I, O> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<I, O> TypedActionHandle<I, O> {
    /// 返回底层稳定 slot。
    pub const fn raw(self) -> ActionHandle {
        self.raw
    }
}
