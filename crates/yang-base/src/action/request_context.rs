//! 请求拥有的类型化上下文；异步切换线程时随 `ActionContext` 一起移动。

use crate::error::BaseError;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;

/// 类型化请求上下文键。
pub struct ContextKey<T> {
    name: &'static str,
    marker: PhantomData<fn() -> T>,
}

impl<T> ContextKey<T> {
    /// 创建静态上下文键。
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            marker: PhantomData,
        }
    }

    /// 返回诊断名称。
    pub const fn name(self) -> &'static str {
        self.name
    }
}

impl<T> Copy for ContextKey<T> {}

impl<T> Clone for ContextKey<T> {
    fn clone(&self) -> Self {
        *self
    }
}

struct ContextValue {
    type_id: TypeId,
    value: Box<dyn Any + Send + Sync>,
}

/// 当前请求独占的类型化扩展值集合。
#[derive(Default)]
pub struct RequestContext {
    values: HashMap<&'static str, ContextValue>,
}

impl RequestContext {
    /// 插入或替换一个类型化值。
    pub fn insert<T>(&mut self, key: ContextKey<T>, value: T)
    where
        T: Send + Sync + 'static,
    {
        self.values.insert(
            key.name,
            ContextValue {
                type_id: TypeId::of::<T>(),
                value: Box::new(value),
            },
        );
    }

    /// 读取一个类型化值；名称相同但类型不同时返回 `None`。
    pub fn get<T>(&self, key: ContextKey<T>) -> Option<&T>
    where
        T: Send + Sync + 'static,
    {
        let value = self.values.get(key.name)?;
        (value.type_id == TypeId::of::<T>())
            .then(|| value.value.downcast_ref::<T>())
            .flatten()
    }

    /// 读取必需值，并把缺失或类型冲突转换成结构化错误。
    pub fn require<T>(&self, key: ContextKey<T>) -> Result<&T, BaseError>
    where
        T: Send + Sync + 'static,
    {
        self.get(key)
            .ok_or_else(|| BaseError::ConfigError(format!("请求上下文缺少类型化值: {}", key.name)))
    }
}

impl fmt::Debug for RequestContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RequestContext")
            .field("keys", &self.values.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// 租户主键。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TenantId(i64);

impl TenantId {
    /// 创建租户主键。
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    /// 返回数据库整数值。
    pub const fn get(self) -> i64 {
        self.0
    }
}

/// 高频租户上下文；system 模式是唯一允许绕过租户条件的显式入口。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TenantContext {
    id: Option<TenantId>,
    system: bool,
}

impl TenantContext {
    /// 创建普通租户上下文。
    pub const fn new(id: TenantId) -> Self {
        Self {
            id: Some(id),
            system: false,
        }
    }

    /// 创建显式系统上下文。
    pub const fn system() -> Self {
        Self {
            id: None,
            system: true,
        }
    }

    /// 返回租户主键；system 上下文返回 `None`。
    pub const fn id(self) -> Option<TenantId> {
        self.id
    }

    /// 返回是否允许绕过租户隔离。
    pub const fn is_system(self) -> bool {
        self.system
    }
}

/// 高频操作者上下文。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActorContext {
    user_id: i64,
}

impl ActorContext {
    /// 创建操作者上下文。
    pub const fn new(user_id: i64) -> Self {
        Self { user_id }
    }

    /// 返回用户主键。
    pub const fn user_id(self) -> i64 {
        self.user_id
    }
}

/// 创建静态类型化上下文键。
#[macro_export]
macro_rules! context_key {
    ($name:literal) => {
        $crate::action::ContextKey::new($name)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    const TENANT_ID: ContextKey<TenantId> = ContextKey::new("tenant_id");

    #[test]
    fn request_context_is_type_checked() {
        let mut context = RequestContext::default();
        context.insert(TENANT_ID, TenantId::new(7));
        assert!(matches!(
            context.require(TENANT_ID).map(|value| value.get()),
            Ok(7)
        ));
        assert!(context
            .get(ContextKey::<String>::new("tenant_id"))
            .is_none());
    }
}
