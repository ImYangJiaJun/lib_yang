//! `ApiCatalog` 到 OpenAPI 3.1 JSON 的可选投影。

use crate::action::PermissionMode;
use crate::error::BaseError;
use crate::router::{ActionDescriptor, ApiCatalog};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashSet};

/// OpenAPI 文档的基础信息。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct OpenApiInfo {
    /// API 标题。
    pub title: String,
    /// API 版本。
    pub version: String,
    /// 可选简介。
    pub description: Option<String>,
}

impl OpenApiInfo {
    /// 创建必填信息。
    pub fn new(title: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            version: version.into(),
            description: None,
        }
    }

    /// 设置简介。
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

impl ApiCatalog {
    /// 从只读 Catalog 确定性投影 OpenAPI 3.1 JSON。
    pub fn to_openapi(&self, info: OpenApiInfo) -> Result<Value, BaseError> {
        if info.title.trim().is_empty() || info.version.trim().is_empty() {
            return Err(BaseError::ConfigError(
                "OpenAPI title/version 不能为空".to_string(),
            ));
        }

        let mut modules: Vec<_> = self.modules.iter().collect();
        modules.sort_by(|left, right| left.name.cmp(&right.name));
        let mut paths: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
        let mut operation_ids = HashSet::new();

        for module in modules {
            let mut actions: Vec<_> = module.actions.iter().collect();
            actions.sort_by(|left, right| left.name.cmp(&right.name));
            for action in actions {
                action.route.validate()?;
                if !operation_ids.insert(action.route.operation_id.as_str()) {
                    return Err(BaseError::ConfigError(format!(
                        "operation_id 冲突: {}",
                        action.route.operation_id
                    )));
                }
                let method = openapi_method(&action.route.method)?;
                let operation = operation_json(action)?;
                let path_item = paths.entry(action.route.path.clone()).or_default();
                if path_item.insert(method.to_string(), operation).is_some() {
                    return Err(BaseError::ConfigError(format!(
                        "route 冲突: {} {}",
                        action.route.method, action.route.path
                    )));
                }
            }
        }

        let mut info_value = json!({
            "title": info.title,
            "version": info.version,
        });
        if let Some(description) = info.description {
            info_value["description"] = Value::String(description);
        }
        Ok(json!({
            "openapi": "3.1.0",
            "info": info_value,
            "paths": paths,
            "components": {
                "securitySchemes": {
                    "bearerAuth": {
                        "type": "http",
                        "scheme": "bearer",
                        "bearerFormat": "JWT"
                    }
                },
                "schemas": {
                    "ApiError": error_envelope_schema()
                }
            }
        }))
    }
}

fn openapi_method(method: &str) -> Result<&'static str, BaseError> {
    match method {
        "GET" => Ok("get"),
        "PUT" => Ok("put"),
        "POST" => Ok("post"),
        "DELETE" => Ok("delete"),
        "OPTIONS" => Ok("options"),
        "HEAD" => Ok("head"),
        "PATCH" => Ok("patch"),
        "TRACE" => Ok("trace"),
        _ => Err(BaseError::ConfigError(format!(
            "OpenAPI 不支持的 method: {method}"
        ))),
    }
}

fn operation_json(action: &ActionDescriptor) -> Result<Value, BaseError> {
    let input_schema = serde_json::to_value(&action.input_schema)
        .map_err(|error| BaseError::JsonSerializeFailed(error.to_string()))?;
    let output_schema = serde_json::to_value(&action.output_schema)
        .map_err(|error| BaseError::JsonSerializeFailed(error.to_string()))?;
    let request_content = content_map(&action.route.request_content_types, input_schema);
    let success_content = content_map(
        &action.route.response_content_types,
        success_envelope_schema(output_schema),
    );
    let error_content = content_map(
        &action.route.response_content_types,
        json!({ "$ref": "#/components/schemas/ApiError" }),
    );
    let permission_mode = match action.permission_mode {
        PermissionMode::All => "all",
        PermissionMode::Any => "any",
    };
    let security = if action.is_public {
        json!([])
    } else {
        json!([{ "bearerAuth": [] }])
    };
    Ok(json!({
        "operationId": action.route.operation_id,
        "summary": action.display_name,
        "description": action.description,
        "tags": action.route.tags,
        "security": security,
        "x-public": action.is_public,
        "x-permissions": action.permissions,
        "x-permission-mode": permission_mode,
        "requestBody": {
            "required": true,
            "content": request_content
        },
        "responses": {
            action.route.success_status.to_string(): {
                "description": "成功",
                "content": success_content
            },
            "400": error_response("请求参数错误", &error_content),
            "401": error_response("未认证", &error_content),
            "403": error_response("权限不足", &error_content),
            "500": error_response("服务器内部错误", &error_content)
        }
    }))
}

fn content_map(content_types: &[String], schema: Value) -> BTreeMap<String, Value> {
    content_types
        .iter()
        .map(|content_type| (content_type.clone(), json!({ "schema": schema.clone() })))
        .collect()
}

fn success_envelope_schema(data_schema: Value) -> Value {
    json!({
        "type": "object",
        "required": ["code", "message", "data"],
        "properties": {
            "code": { "type": "integer", "const": 0 },
            "message": { "type": "string" },
            "data": data_schema
        }
    })
}

fn error_envelope_schema() -> Value {
    json!({
        "type": "object",
        "required": ["code", "message"],
        "properties": {
            "code": { "type": "integer", "not": { "const": 0 } },
            "message": { "type": "string" },
            "data": true
        }
    })
}

fn error_response(description: &str, content: &BTreeMap<String, Value>) -> Value {
    json!({ "description": description, "content": content })
}
