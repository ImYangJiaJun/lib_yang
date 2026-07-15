//! 运行时路由注册表的只读、确定性 API 描述快照。

use crate::action::PermissionMode;
use crate::error::BaseError;

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
    /// 创建并校验最小路由描述。
    pub fn new(
        method: impl Into<String>,
        path: impl Into<String>,
        operation_id: impl Into<String>,
    ) -> Result<Self, BaseError> {
        let method = method.into().trim().to_ascii_uppercase();
        let path = path.into().trim().to_string();
        let operation_id = operation_id.into().trim().to_string();
        let descriptor = Self {
            method,
            path,
            operation_id,
            request_content_types: vec!["application/json".to_string()],
            response_content_types: vec!["application/json".to_string()],
            success_status: 200,
            tags: Vec::new(),
        };
        descriptor.validate()?;
        Ok(descriptor)
    }

    /// 重新校验公开字段，防止构造后修改绕过注册期约束。
    pub fn validate(&self) -> Result<(), BaseError> {
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

    /// 设置请求 Content-Type 列表。
    pub fn with_request_content_types(mut self, values: Vec<String>) -> Result<Self, BaseError> {
        validate_non_blank_unique("request content type", &values, false)?;
        self.request_content_types = values;
        Ok(self)
    }

    /// 设置响应 Content-Type 列表。
    pub fn with_response_content_types(mut self, values: Vec<String>) -> Result<Self, BaseError> {
        validate_non_blank_unique("response content type", &values, false)?;
        self.response_content_types = values;
        Ok(self)
    }

    /// 设置成功状态码（100..=599）。
    pub fn with_success_status(mut self, status: u16) -> Result<Self, BaseError> {
        if !(100..=599).contains(&status) {
            return Err(BaseError::ConfigError(
                "success status 必须在 100..=599".to_string(),
            ));
        }
        self.success_status = status;
        Ok(self)
    }

    /// 设置文档标签。
    pub fn with_tags(mut self, tags: Vec<String>) -> Result<Self, BaseError> {
        validate_non_blank_unique("route tag", &tags, true)?;
        self.tags = tags;
        Ok(self)
    }
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
