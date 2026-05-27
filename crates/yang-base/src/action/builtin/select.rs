#![cfg(any())]
//! SelectAction - 查询列表 Action
//!
//! 根据查询参数分页查询数据库表中的记录列表。
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::action::builtin::SelectAction;
//! use yang_base::action::{Action, ActionContext};
//! use yang_base::table::TableConfig;
//! use serde_json::json;
//! use std::sync::Arc;
//!
//! let table_config = Arc::new(TableConfig::new("users"));
//! let action = SelectAction::new(table_config);
//!
//! // 在 ActionContext 中使用
//! let response = action.execute(context).await?;
//! ```

use crate::action::{Action, ActionContext, ApiResponse};
use crate::error::BaseError;
use crate::table::{SortOrder, TableConfig};
use async_trait::async_trait;
use std::sync::Arc;

/// SelectAction - 查询列表
///
/// 从请求中解析查询参数，执行分页查询并返回结果列表。
///
/// # 请求参数
///
/// - `fields`: 可选，要查询的字段列表
/// - `where`: 可选，筛选条件
/// - `order_by`: 可选，排序规则
/// - `page`: 可选，页码（默认 1，范围 1..=i64::MAX）
/// - `page_size`: 可选，每页大小（默认 10，范围 1..=100）
///
/// # 返回
///
/// - 成功：返回分页结果（包含数据列表、总数、页码等）
/// - 失败：返回错误信息
#[allow(dead_code)]
pub struct SelectAction {
    /// 表配置
    table_config: Arc<TableConfig>,
}

impl SelectAction {
    /// 创建新的 SelectAction
    ///
    /// # 参数
    ///
    /// - `table_config`: 表配置
    ///
    /// # 返回
    ///
    /// - 新的 SelectAction 实例
    pub fn new(table_config: Arc<TableConfig>) -> Self {
        Self { table_config }
    }
}

/// 安全解析分页参数
///
/// 从 ActionContext 中读取指定参数，进行范围校验，并安全转换为 usize。
///
/// # 参数
///
/// - `ctx`: Action 执行上下文
/// - `key`: 参数名
/// - `default`: 参数不存在时的默认值
/// - `min`: 允许的最小值（含）
/// - `max`: 允许的最大值（含）
///
/// # 返回
///
/// - `Ok(usize)`: 转换成功
/// - `Err(BaseError::ParamInvalid)`: 参数值超出范围或无法转换为 usize
fn parse_paging_param(
    ctx: &ActionContext,
    key: &str,
    default: i64,
    min: i64,
    max: i64,
) -> Result<usize, BaseError> {
    // 从请求体中获取参数，不存在时使用默认值
    let raw_value: i64 = ctx.param_optional::<i64>(key).unwrap_or(default);

    // 范围校验：越界时返回 ParamInvalid 错误
    if raw_value < min || raw_value > max {
        return Err(BaseError::ParamInvalid(
            key.to_string(),
            format!(
                "参数值 {} 超出允许范围 [{}, {}]",
                raw_value, min, max
            ),
        ));
    }

    // 使用 usize::try_from 替代 as usize，避免截断
    usize::try_from(raw_value).map_err(|_| {
        BaseError::ParamInvalid(
            key.to_string(),
            format!("参数值 {} 无法转换为 usize", raw_value),
        )
    })
}

#[async_trait]
impl Action for SelectAction {
    async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError> {
        // 通过 parse_paging_param 安全获取分页参数
        // page: 默认 1，范围 1..=i64::MAX
        let page = parse_paging_param(&context, "page", 1, 1, i64::MAX)?;
        // page_size: 默认 10，范围 1..=100
        let page_size = parse_paging_param(&context, "page_size", 10, 1, 100)?;

        // 创建查询构建器
        let mut query = context.table_query()?;

        // 应用字段选择
        if let Some(fields) = context.param_optional::<Vec<String>>("fields") {
            let field_refs: Vec<&str> = fields.iter().map(|s| s.as_str()).collect();
            query = query.select_fields(&field_refs)?;
        }

        // 应用筛选条件
        if let Some(where_conditions) = context.param_optional::<serde_json::Value>("where") {
            if let Some(obj) = where_conditions.as_object() {
                for (field, value) in obj {
                    query = query.where_eq(field.as_str(), value.clone())?;
                }
            }
        }

        // 应用排序规则
        if let Some(order_by) = context.param_optional::<Vec<serde_json::Value>>("order_by") {
            for order in order_by {
                if let Some(obj) = order.as_object() {
                    if let (Some(field), Some(direction)) = (obj.get("field"), obj.get("direction"))
                    {
                        let field_str = field.as_str().ok_or_else(|| {
                            BaseError::ParamInvalid(
                                "order_by".to_string(),
                                "field 必须是字符串".to_string(),
                            )
                        })?;
                        let direction_str = direction.as_str().ok_or_else(|| {
                            BaseError::ParamInvalid(
                                "order_by".to_string(),
                                "direction 必须是字符串".to_string(),
                            )
                        })?;

                        let sort_order = match direction_str.to_lowercase().as_str() {
                            "asc" => SortOrder::Asc,
                            "desc" => SortOrder::Desc,
                            _ => {
                                return Err(BaseError::ParamInvalid(
                                    "order_by".to_string(),
                                    "direction 必须是 'asc' 或 'desc'".to_string(),
                                ))
                            }
                        };

                        query = query.order_by(field_str, sort_order)?;
                    }
                }
            }
        }

        // 设置分页参数
        let query = query.page(page, page_size)?;

        // 使用 DynamicRow 类型执行分页查询
        #[cfg(feature = "mysql")]
        {
            use crate::table::DynamicRow;

            let result = query.paginate::<DynamicRow>().await?;

            // 将分页结果转换为 JSON 并返回
            return ApiResponse::success(result, "查询成功");
        }

        // 未启用 mysql feature 时返回错误
        #[cfg(not(feature = "mysql"))]
        {
            let _ = query;
            Err(BaseError::Unknown(
                "SelectAction 需要启用 mysql feature".to_string(),
            ))
        }
    }

    fn name(&self) -> &str {
        "select"
    }

    fn display_name(&self) -> &str {
        "查询列表"
    }

    fn description(&self) -> &str {
        "分页查询数据表中的记录列表"
    }

    fn params_schema(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "fields": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "要查询的字段列表"
                },
                "where": {
                    "type": "object",
                    "description": "筛选条件"
                },
                "order_by": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "field": { "type": "string" },
                            "direction": { "type": "string", "enum": ["asc", "desc"] }
                        }
                    },
                    "description": "排序规则"
                },
                "page": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "页码（默认 1）"
                },
                "page_size": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 100,
                    "description": "每页大小（默认 10，最大 100）"
                }
            }
        }))
    }
}
