//! PutAction - 更新数据 Action
//!
//! 根据主键更新数据库表中的记录。
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::action::builtin::PutAction;
//! use yang_base::action::{Action, ActionContext};
//! use yang_base::table::TableConfig;
//! use serde_json::json;
//! use std::sync::Arc;
//!
//! let table_config = Arc::new(TableConfig::new("users"));
//! let action = PutAction::new(table_config);
//!
//! // 在 ActionContext 中使用
//! let response = action.execute(context).await?;
//! ```

use crate::action::{Action, ActionContext, ApiResponse};
use crate::error::BaseError;
use crate::table::TableConfig;
use async_trait::async_trait;
use std::sync::Arc;

/// PutAction - 更新数据
///
/// 从请求中获取主键值和更新数据，更新数据库表中的记录。
///
/// # 请求参数
///
/// - 主键字段：主键值（从请求体中获取）
/// - `data`: JSON 对象，包含要更新的字段和值
///
/// # 返回
///
/// - 成功：返回影响行数
/// - 失败：返回错误信息
pub struct PutAction {
    /// 表配置
    table_config: Arc<TableConfig>,
}

impl PutAction {
    /// 创建新的 PutAction
    ///
    /// # 参数
    ///
    /// - `table_config`: 表配置
    ///
    /// # 返回
    ///
    /// - 新的 PutAction 实例
    pub fn new(table_config: Arc<TableConfig>) -> Self {
        Self { table_config }
    }
}

#[async_trait]
impl Action for PutAction {
    async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError> {
        // 获取主键字段名
        let pk_field = &self.table_config.primary_key;

        // 获取主键值
        let pk_value: serde_json::Value = context.param(pk_field)?;

        // 获取 data 参数
        let data: serde_json::Value = context.param("data")?;

        // 确保 data 是对象类型
        let data_obj = data.as_object().ok_or_else(|| {
            BaseError::ParamInvalid("data".to_string(), "必须是对象类型".to_string())
        })?;

        // 转换为 HashMap
        let mut data_map = std::collections::HashMap::new();
        for (k, v) in data_obj {
            data_map.insert(k.clone(), v.clone());
        }

        // 创建查询构建器
        let mut query = context.table_query()?;

        // 添加主键 WHERE 条件
        query = query.where_eq(pk_field.as_str(), pk_value)?;

        // 执行更新操作
        let affected = query.update(data_map).await?;

        // 返回成功响应
        Ok(ApiResponse::success(
            serde_json::json!({ "affected": affected }),
            "更新成功",
        ))
    }

    fn name(&self) -> &str {
        "put"
    }

    fn display_name(&self) -> &str {
        "更新数据"
    }

    fn description(&self) -> &str {
        "根据主键更新数据表中的记录"
    }

    fn params_schema(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "data": {
                    "type": "object",
                    "description": "要更新的数据对象"
                }
            },
            "required": ["data"]
        }))
    }
}
