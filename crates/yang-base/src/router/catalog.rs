//! 运行时路由注册表的只读、确定性 API 描述快照。

use crate::action::PermissionMode;
use crate::error::BaseError;
use std::collections::{HashMap, HashSet};

/// Action 对应的传输路由描述。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RouteDescriptor {
    /// 规范化为大写的请求方法。
    pub method: String,
    /// 不含 query/fragment 的路由路径。
    pub path: String,
    /// API 操作唯一标识。
    pub operation_id: String,
    /// 支持的请求 Content-Type。
    pub request_content_types: Vec<String>,
    /// 支持的响应 Content-Type。
    pub response_content_types: Vec<String>,
    /// 成功响应状态码。
    pub success_status: u16,
    /// 文档标签。
    pub tags: Vec<String>,
}

impl RouteDescriptor {
    /// 注册前统一校验路由描述。
    pub(crate) fn validate(&self) -> Result<(), BaseError> {
        if self.method.is_empty()
            || self.method != self.method.to_ascii_uppercase()
            || !self
                .method
                .bytes()
                .all(|byte| byte.is_ascii_alphabetic() || byte == b'-')
        {
            return Err(BaseError::ConfigError("route method 非法".to_string()));
        }
        if !self.path.starts_with('/')
            || self.path.contains(['?', '#'])
            || self.path.chars().any(char::is_whitespace)
        {
            return Err(BaseError::ConfigError(
                "route path 必须是无 query/fragment 的绝对路径".to_string(),
            ));
        }
        if self
            .path
            .split('/')
            .any(|segment| segment.starts_with([':', '*']))
        {
            return Err(BaseError::ConfigError(
                "route path 必须使用 Axum 0.8 的 {name}/{*name} 参数语法".to_string(),
            ));
        }
        let mut matcher = matchit::Router::new();
        matcher.insert(self.path.clone(), ()).map_err(|error| {
            BaseError::ConfigError(format!("route path 非法: {} ({error})", self.path))
        })?;
        if self.operation_id.trim().is_empty() {
            return Err(BaseError::ConfigError("operation_id 不能为空".to_string()));
        }
        validate_non_blank_unique("request content type", &self.request_content_types, false)?;
        validate_non_blank_unique("response content type", &self.response_content_types, false)?;
        validate_non_blank_unique("route tag", &self.tags, true)?;
        if !(100..=599).contains(&self.success_status) {
            return Err(BaseError::ConfigError(
                "success status 必须在 100..=599".to_string(),
            ));
        }
        Ok(())
    }
}

/// 与 Axum PathRouter 等价的路由模板注册检查。
///
/// 完全相同的 path 可以按不同 HTTP method 合并；参数名不同但匹配集合相同的模板
/// 会被 matchit 判定为冲突，避免把 panic 延迟到 transport 构建阶段。
#[derive(Default)]
pub(crate) struct RoutePatternRegistry {
    matcher: matchit::Router<()>,
    methods_by_path: HashMap<String, HashSet<String>>,
}

impl RoutePatternRegistry {
    pub(crate) fn insert(&mut self, route: &RouteDescriptor) -> Result<(), BaseError> {
        route.validate()?;

        if let Some(methods) = self.methods_by_path.get_mut(&route.path) {
            if !methods.insert(route.method.clone()) {
                return Err(route_conflict(route, None));
            }
            return Ok(());
        }

        self.matcher
            .insert(route.path.clone(), ())
            .map_err(|error| route_conflict(route, Some(error)))?;
        self.methods_by_path
            .insert(route.path.clone(), HashSet::from([route.method.clone()]));
        Ok(())
    }
}

fn route_conflict(route: &RouteDescriptor, source: Option<matchit::InsertError>) -> BaseError {
    let detail = source.map_or_else(String::new, |error| format!(" ({error})"));
    BaseError::ConfigError(format!(
        "route 冲突: {} {}{detail}",
        route.method, route.path
    ))
}

fn validate_non_blank_unique(
    label: &str,
    values: &[String],
    allow_empty: bool,
) -> Result<(), BaseError> {
    if (!allow_empty && values.is_empty()) || values.iter().any(|value| value.trim().is_empty()) {
        return Err(BaseError::ConfigError(format!("{label} 不能为空")));
    }
    let mut sorted = values.to_vec();
    sorted.sort();
    if sorted.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(BaseError::ConfigError(format!("{label} 不能重复")));
    }
    Ok(())
}

/// 合并 ActionMeta 与 RouteDescriptor 后的只读操作描述。
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ActionDescriptor {
    /// Action 唯一名称。
    pub name: String,
    /// 显示名称。
    pub display_name: String,
    /// 简介。
    pub description: String,
    /// Action 权限。
    pub permissions: Vec<String>,
    /// 权限组合模式。
    pub permission_mode: PermissionMode,
    /// 是否公开。
    pub is_public: bool,
    /// 运行时输入 Schema 的快照。
    pub input_schema: schemars::schema::RootSchema,
    /// 运行时输出 Schema 的快照。
    pub output_schema: schemars::schema::RootSchema,
    /// 唯一传输路由来源。
    pub route: RouteDescriptor,
}

/// 单模块只读描述。
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ModuleDescriptor {
    /// 模块名称。
    pub name: String,
    /// 模块显示名称。
    pub display_name: String,
    /// 模块默认权限。
    pub default_permissions: Vec<String>,
    /// 模块默认权限组合模式。
    pub default_permission_mode: PermissionMode,
    /// 按 Action 名称排序的操作。
    pub actions: Vec<ActionDescriptor>,
}

/// 应用级不可变 API 清单快照。
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ApiCatalog {
    /// 按模块名称排序的模块描述。
    pub modules: Vec<ModuleDescriptor>,
}
