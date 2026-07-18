//! DefinitionCatalog 到 OpenAPI 3.1 JSON 的确定性投影。

use super::{ActionSpec, DefinitionCatalog, FieldKind, ParamSource, ParamSpec};
use crate::action::PermissionMode;
use crate::error::BaseError;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

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

impl DefinitionCatalog {
    /// 从唯一原生定义确定性生成 OpenAPI 3.1 文档。
    pub fn to_openapi(&self, info: OpenApiInfo) -> Result<Value, BaseError> {
        if info.title.trim().is_empty() || info.version.trim().is_empty() {
            return Err(BaseError::ConfigError(
                "OpenAPI title/version 不能为空".to_string(),
            ));
        }
        let mut paths: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
        for action in self
            .addons()
            .iter()
            .flat_map(|addon| &addon.modules)
            .flat_map(|module| module.actions())
        {
            let method = action.route.method.as_str().to_ascii_lowercase();
            let operation = operation_json(action);
            if paths
                .entry(action.route.path.clone())
                .or_default()
                .insert(method, operation)
                .is_some()
            {
                return Err(BaseError::ConfigError(format!(
                    "route 冲突: {} {}",
                    action.route.method.as_str(),
                    action.route.path
                )));
            }
        }

        let mut info_value = json!({ "title": info.title, "version": info.version });
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
                "schemas": { "ApiError": error_envelope_schema() }
            }
        }))
    }
}

fn operation_json(action: &ActionSpec) -> Value {
    let security = if action.is_public {
        json!([])
    } else {
        json!([{ "bearerAuth": [] }])
    };
    let permission_mode = match action.permission_mode {
        PermissionMode::All => "all",
        PermissionMode::Any => "any",
    };
    let parameters: Vec<_> = action
        .params
        .iter()
        .filter_map(|param| {
            let location = match param.source {
                ParamSource::Query => Some("query"),
                ParamSource::Path => Some("path"),
                ParamSource::Header => Some("header"),
                ParamSource::Body => None,
            };
            location.map(|location| {
                json!({
                    "name": param.name.as_str(),
                    "in": location,
                    "required": param.required || param.source == ParamSource::Path,
                    "description": param.presentation.description,
                    "schema": param_schema(action, param)
                })
            })
        })
        .collect();
    let has_body = action.request_media_type == super::ActionMediaType::Multipart
        || action
            .params
            .iter()
            .any(|param| param.source == ParamSource::Body)
        || action.params.is_empty();
    let request_body = has_body.then(|| {
        let mut media_schema = json!({ "schema": body_schema(action) });
        let content_type = match action.request_media_type {
            super::ActionMediaType::Json => "application/json",
            super::ActionMediaType::Multipart => {
                if let Some(spec) = &action.multipart {
                    media_schema["x-yang-multipart"] = json!({
                        "max_fields": spec.max_fields,
                        "max_files": spec.max_files,
                        "max_file_bytes": spec.max_file_bytes,
                        "max_text_field_bytes": spec.max_text_field_bytes,
                        "max_total_bytes": spec.max_total_bytes,
                        "allowed_content_types": spec.allowed_content_types,
                        "lifecycle": "request_scoped"
                    });
                }
                "multipart/form-data"
            }
        };
        let mut content = Map::new();
        content.insert(content_type.to_string(), media_schema);
        json!({
            "required": action.params.iter().any(|param| param.source == ParamSource::Body && param.required),
            "content": content
        })
    });

    let mut operation = json!({
        "operationId": action.route.operation_id,
        "summary": action.display_name,
        "description": action.description,
        "tags": action.tags,
        "security": security,
        "x-public": action.is_public,
        "x-permissions": action.permissions,
        "x-permission-mode": permission_mode,
        "parameters": parameters,
        "responses": {
            action.success_status.to_string(): {
                "description": "成功",
                "content": {
                    "application/json": {
                        "schema": success_envelope_schema(action.output_schema.clone())
                    }
                }
            },
            "400": error_response("请求参数错误"),
            "401": error_response("未认证"),
            "403": error_response("权限不足"),
            "500": error_response("服务器内部错误")
        }
    });
    if let Some(request_body) = request_body {
        operation["requestBody"] = request_body;
    }
    operation
}

fn param_schema(action: &ActionSpec, param: &ParamSpec) -> Value {
    action
        .input_schema
        .get("properties")
        .and_then(|properties| properties.get(param.name.as_str()))
        .cloned()
        .unwrap_or_else(|| match param.kind {
            Some(
                FieldKind::Key
                | FieldKind::Int
                | FieldKind::Table
                | FieldKind::Tree
                | FieldKind::Timestamp,
            ) => {
                json!({"type": "integer", "format": "int64"})
            }
            Some(FieldKind::Switch) => json!({"type": "boolean"}),
            Some(FieldKind::Decimal) => json!({"type": "string", "format": "decimal"}),
            Some(FieldKind::Str | FieldKind::Text) => json!({"type": "string"}),
            Some(FieldKind::Radio) | None => Value::Bool(true),
        })
}

fn body_schema(action: &ActionSpec) -> Value {
    let body = action
        .params
        .iter()
        .filter(|param| param.source == ParamSource::Body)
        .collect::<Vec<_>>();
    if body.is_empty() && action.params.is_empty() {
        return action.input_schema.clone();
    }
    let properties = body
        .iter()
        .map(|param| (param.name.to_string(), param_schema(action, param)))
        .collect::<Map<_, _>>();
    let required = body
        .iter()
        .filter(|param| param.required)
        .map(|param| Value::String(param.name.to_string()))
        .collect::<Vec<_>>();
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
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

fn error_response(description: &str) -> Value {
    json!({
        "description": description,
        "content": {
            "application/json": { "schema": { "$ref": "#/components/schemas/ApiError" } }
        }
    })
}
