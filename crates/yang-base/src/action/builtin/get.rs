#![cfg(any())]
//! GetAction - 获取单条数据 Action
//!
//! 根据主键获取数据库表中的单条记录。
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::action::builtin::GetAction;
//! use yang_base::action::{Action, ActionContext};
//! use yang_base::table::TableConfig;
//! use serde_json::json;
//! use std::sync::Arc;
//!
//! let table_config = Arc::new(TableConfig::new("users"));
//! let action = GetAction::new(table_config);
//!
//! // 在 ActionContext 中使用
//! let response = action.execute(context).await?;
//! ```

use crate::action::{Action, ActionContext, ApiResponse};
use crate::error::BaseError;
use crate::table::TableConfig;
use async_trait::async_trait;
use std::sync::Arc;

/// GetAction - 获取单条数据
///
/// 从请求中获取主键值，查询并返回数据库表中的单条记录。
///
/// # 请求参数
///
/// - 主键字段：主键值（从请求体中获取）
///
/// # 返回
///
/// - 成功：返回单条记录
/// - 失败：返回错误信息（如记录不存在）
pub struct GetAction {
    /// 表配置
    table_config: Arc<TableConfig>,
}

impl GetAction {
    /// 创建新的 GetAction
    ///
    /// # 参数
    ///
    /// - `table_config`: 表配置
    ///
    /// # 返回
    ///
    /// - 新的 GetAction 实例
    pub fn new(table_config: Arc<TableConfig>) -> Self {
        Self { table_config }
    }
}

#[async_trait]
impl Action for GetAction {
    async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError> {
        // 获取主键字段名
        let pk_field = &self.table_config.primary_key;

        // 从请求体中获取主键值
        let pk_value: serde_json::Value = context.param(pk_field)?;

        // 创建查询构建器，添加主键 WHERE 条件
        let query = context
            .table_query()?
            .where_eq(pk_field.as_str(), pk_value)?;

        // 使用 DynamicRow 类型执行查询，获取可选的单条记录
        #[cfg(feature = "mysql")]
        {
            use crate::table::DynamicRow;

            let row = query.fetch_optional::<DynamicRow>().await?;

            // 查询结果为空时返回 RecordNotFound 错误
            match row {
                None => Err(BaseError::RecordNotFound(format!(
                    "表 {} 中主键为 {} 的记录不存在",
                    self.table_config.table_name, pk_field
                ))),
                Some(record) => ApiResponse::success(record, "获取成功"),
            }
        }

        // 未启用 mysql feature 时返回错误
        #[cfg(not(feature = "mysql"))]
        {
            let _ = query;
            Err(BaseError::Unknown(
                "GetAction 需要启用 mysql feature".to_string(),
            ))
        }
    }

    fn name(&self) -> &str {
        "get"
    }

    fn display_name(&self) -> &str {
        "获取数据"
    }

    fn description(&self) -> &str {
        "根据主键获取数据表中的单条记录"
    }
}
