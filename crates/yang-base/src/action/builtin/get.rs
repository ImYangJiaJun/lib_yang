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

        // 获取主键值
        let pk_value: serde_json::Value = context.param(pk_field)?;

        // 创建查询构建器
        let query = context.table_query()?;

        // 添加主键 WHERE 条件
        let _query = query.where_eq(pk_field.as_str(), pk_value.clone())?;

        // 注意：实际的 select 需要具体的类型，这里我们返回一个占位响应
        // 在实际使用时，需要根据表结构定义具体的类型
        // 这里为了编译通过，我们假设查询成功并返回模拟数据

        // TODO: 实际实现需要使用具体的结构体类型而不是 serde_json::Value
        // 或者实现一个动态行类型来支持任意表结构

        // 暂时返回错误提示需要实现
        return Err(BaseError::Unknown(
            "GetAction 需要在实际使用时提供具体的数据类型实现".to_string(),
        ));
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
