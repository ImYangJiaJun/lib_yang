//! Action 与传输路由的一体化注册项。

use super::RouteDescriptor;
use crate::action::{DynAction, TypedAction};
use crate::error::BaseError;
use std::sync::Arc;

/// 一条可注册的 API。
///
/// `Api` 把 handler 与 HTTP 元数据放在同一个值里，避免 Action 注册和字符串路由
/// 注册分离后发生漏配或名称漂移。除重复检查等模块级约束外，所有配置都可链式完成，
/// 最终只在 [`crate::router::ModuleRouter::api`] 注册时统一校验。
#[must_use = "Api 必须注册到 ModuleRouter"]
pub struct Api {
    action: Arc<dyn DynAction>,
    method: String,
    path: String,
    operation_id: Option<String>,
    request_content_types: Vec<String>,
    response_content_types: Vec<String>,
    success_status: u16,
    tags: Vec<String>,
}

impl Api {
    /// 创建自定义 HTTP 方法的 API。
    pub fn new<A>(method: impl Into<String>, path: impl Into<String>, action: A) -> Self
    where
        A: TypedAction,
    {
        Self {
            action: Arc::new(action),
            method: method.into(),
            path: path.into(),
            operation_id: None,
            request_content_types: vec!["application/json".to_string()],
            response_content_types: vec!["application/json".to_string()],
            success_status: 200,
            tags: Vec::new(),
        }
    }

    /// 创建 GET API。
    pub fn get<A>(path: impl Into<String>, action: A) -> Self
    where
        A: TypedAction,
    {
        Self::new("GET", path, action)
    }

    /// 创建 POST API。
    pub fn post<A>(path: impl Into<String>, action: A) -> Self
    where
        A: TypedAction,
    {
        Self::new("POST", path, action)
    }

    /// 创建 PUT API。
    pub fn put<A>(path: impl Into<String>, action: A) -> Self
    where
        A: TypedAction,
    {
        Self::new("PUT", path, action)
    }

    /// 创建 PATCH API。
    pub fn patch<A>(path: impl Into<String>, action: A) -> Self
    where
        A: TypedAction,
    {
        Self::new("PATCH", path, action)
    }

    /// 创建 DELETE API。
    pub fn delete<A>(path: impl Into<String>, action: A) -> Self
    where
        A: TypedAction,
    {
        Self::new("DELETE", path, action)
    }

    /// 覆盖默认的 `{module}.{action}` operation id。
    pub fn operation_id(mut self, operation_id: impl Into<String>) -> Self {
        self.operation_id = Some(operation_id.into());
        self
    }

    /// 设置成功响应状态码。
    pub fn status(mut self, status: u16) -> Self {
        self.success_status = status;
        self
    }

    /// 将成功响应状态码设置为 201 Created。
    pub fn created(self) -> Self {
        self.status(201)
    }

    /// 追加一个文档标签。
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// 设置全部文档标签。
    pub fn tags<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    /// 设置支持的请求 Content-Type。
    pub fn request_content_types<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.request_content_types = values.into_iter().map(Into::into).collect();
        self
    }

    /// 设置支持的响应 Content-Type。
    pub fn response_content_types<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.response_content_types = values.into_iter().map(Into::into).collect();
        self
    }

    pub(crate) fn into_parts(
        self,
        module_name: &str,
    ) -> Result<(Arc<dyn DynAction>, RouteDescriptor), BaseError> {
        let operation_id = self
            .operation_id
            .unwrap_or_else(|| format!("{}.{}", module_name, self.action.meta().name));
        let route = RouteDescriptor {
            method: self.method.trim().to_ascii_uppercase(),
            path: self.path.trim().to_string(),
            operation_id: operation_id.trim().to_string(),
            request_content_types: self.request_content_types,
            response_content_types: self.response_content_types,
            success_status: self.success_status,
            tags: self.tags,
        };
        route.validate()?;
        Ok((self.action, route))
    }
}
