#![cfg(any())]
//! DelAction - 删除数据 Action
//!
//! 根据主键删除数据库表中的记录（支持软删除）。
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::action::builtin::DelAction;
//! use yang_base::action::{Action, ActionContext};
//! use yang_base::table::TableConfig;
//! use serde_json::json;
//! use std::sync::Arc;
//!
//! let table_config = Arc::new(TableConfig::new("users"));
//! let action = DelAction::new(table_config);
//!
//! // 在 ActionContext 中使用
//! let response = action.execute(context).await?;
//! ```

use crate::action::{Action, ActionContext, ApiResponse};
use crate::error::BaseError;
use crate::table::TableConfig;
use async_trait::async_trait;
use std::sync::Arc;

/// DelAction - 删除数据
///
/// 从请求中获取主键值，删除数据库表中的记录。
/// 如果配置了软删除字段，则执行软删除；否则执行物理删除。
///
/// # 请求参数
///
/// - 主键字段：主键值（从请求体中获取）
///
/// # 返回
///
/// - 成功：返回影响行数
/// - 失败：返回错误信息
pub struct DelAction {
    /// 表配置
    table_config: Arc<TableConfig>,
}

impl DelAction {
    /// 创建新的 DelAction
    ///
    /// # 参数
    ///
    /// - `table_config`: 表配置
    ///
    /// # 返回
    ///
    /// - 新的 DelAction 实例
    pub fn new(table_config: Arc<TableConfig>) -> Self {
        Self { table_config }
    }
}

#[async_trait]
impl Action for DelAction {
    async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError> {
        // 获取主键字段名
        let pk_field = &self.table_config.primary_key;

        // 获取主键值
        let pk_value: serde_json::Value = context.param(pk_field)?;

        // 创建查询构建器
        let mut query = context.table_query()?;

        // 添加主键 WHERE 条件
        query = query.where_eq(pk_field.as_str(), pk_value)?;

        // 执行删除操作（自动处理软删除）
        let affected = query.delete().await?;

        // 返回成功响应（序列化失败时通过 ? 传播错误）
        Ok(ApiResponse::success(
            serde_json::json!({ "affected": affected }),
            "删除成功",
        )?)
    }

    fn name(&self) -> &str {
        "del"
    }

    fn display_name(&self) -> &str {
        "删除数据"
    }

    fn description(&self) -> &str {
        "根据主键删除数据表中的记录（支持软删除）"
    }
}
