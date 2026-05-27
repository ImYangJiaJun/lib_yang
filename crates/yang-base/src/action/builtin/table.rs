#![cfg(any())]
//! TableAction - 获取表元数据 Action
//!
//! 返回数据表的元数据信息，包括字段列表、索引、权限等。
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::action::builtin::TableAction;
//! use yang_base::action::{Action, ActionContext};
//! use yang_base::table::TableConfig;
//! use serde_json::json;
//! use std::sync::Arc;
//!
//! let table_config = Arc::new(TableConfig::new("users"));
//! let action = TableAction::new(table_config);
//!
//! // 在 ActionContext 中使用
//! let response = action.execute(context).await?;
//! ```

use crate::action::{Action, ActionContext, ApiResponse};
use crate::error::BaseError;
use crate::table::TableConfig;
use async_trait::async_trait;
use std::sync::Arc;

/// TableAction - 获取表元数据
///
/// 返回数据表的元数据信息，根据用户角色过滤字段权限。
/// 这是一个公开 Action，不需要认证。
///
/// # 返回
///
/// - 成功：返回表元数据（表名、字段列表、索引等）
/// - 失败：返回错误信息
pub struct TableAction {
    /// 表配置
    table_config: Arc<TableConfig>,
}

impl TableAction {
    /// 创建新的 TableAction
    ///
    /// # 参数
    ///
    /// - `table_config`: 表配置
    ///
    /// # 返回
    ///
    /// - 新的 TableAction 实例
    pub fn new(table_config: Arc<TableConfig>) -> Self {
        Self { table_config }
    }
}

#[async_trait]
impl Action for TableAction {
    async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError> {
        // 获取用户角色
        let user_roles = context.user_roles();

        // 构建字段元数据列表
        let mut fields = Vec::new();
        for (field_name, field_config) in &self.table_config.fields {
            // 检查字段读取权限
            let can_read = field_config.permissions.can_read(&user_roles);

            if can_read {
                fields.push(serde_json::json!({
                    "name": field_name,
                    "display_name": field_config.display_name,
                    "field_type": format!("{:?}", field_config.field_type),
                    "required": field_config.required,
                    "default_value": field_config.default_value,
                }));
            }
        }

        // 构建表元数据
        let metadata = serde_json::json!({
            "table_name": self.table_config.table_name,
            "display_name": self.table_config.display_name,
            "primary_key": self.table_config.primary_key,
            "fields": fields,
            "unique_indexes": self.table_config.unique_indexes,
            "indexes": self.table_config.indexes,
            "soft_delete_field": self.table_config.soft_delete_field,
        });

        // 返回成功响应（metadata 已是 serde_json::Value，直接使用 success_value 避免额外序列化）
        Ok(ApiResponse::success_value(metadata, "获取表元数据成功"))
    }

    fn name(&self) -> &str {
        "table"
    }

    fn display_name(&self) -> &str {
        "获取表元数据"
    }

    fn description(&self) -> &str {
        "返回数据表的元数据信息，包括字段列表、索引、权限等"
    }

    fn is_public(&self) -> bool {
        true // 表元数据是公开的
    }
}
