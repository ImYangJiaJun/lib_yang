//! AddAction - 新增数据 Action
//!
//! 从请求中获取数据并插入到数据库表中。
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::action::builtin::AddAction;
//! use yang_base::action::{Action, ActionContext};
//! use yang_base::table::TableConfig;
//! use serde_json::json;
//! use std::sync::Arc;
//!
//! let table_config = Arc::new(TableConfig::new("users"));
//! let action = AddAction::new(table_config);
//!
//! // 在 ActionContext 中使用
//! let response = action.execute(context).await?;
//! ```

use crate::action::{Action, ActionContext, ApiResponse};
use crate::error::BaseError;
use crate::table::TableConfig;
use async_trait::async_trait;
use std::sync::Arc;

/// AddAction - 新增数据
///
/// 从请求体中获取 data 参数，验证后插入到数据库表中。
///
/// # 请求参数
///
/// - `data`: JSON 对象，包含要插入的字段和值
///
/// # 返回
///
/// - 成功：返回影响行数
/// - 失败：返回错误信息
#[allow(dead_code)]
pub struct AddAction {
    /// 表配置
    table_config: Arc<TableConfig>,
}

impl AddAction {
    /// 创建新的 AddAction
    ///
    /// # 参数
    ///
    /// - `table_config`: 表配置
    ///
    /// # 返回
    ///
    /// - 新的 AddAction 实例
    pub fn new(table_config: Arc<TableConfig>) -> Self {
        Self { table_config }
    }
}

#[async_trait]
impl Action for AddAction {
    async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError> {
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
        let query = context.table_query()?;

        // 执行插入操作
        let affected = query.insert(data_map).await?;

        // 返回成功响应（序列化失败时通过 ? 传播错误）
        Ok(ApiResponse::success(
            serde_json::json!({ "affected": affected }),
            "新增成功",
        )?)
    }

    fn name(&self) -> &str {
        "add"
    }

    fn display_name(&self) -> &str {
        "新增数据"
    }

    fn description(&self) -> &str {
        "向数据表中添加一条新记录"
    }

    fn params_schema(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "data": {
                    "type": "object",
                    "description": "要插入的数据对象"
                }
            },
            "required": ["data"]
        }))
    }
}
