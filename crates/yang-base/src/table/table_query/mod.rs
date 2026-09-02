//! 表查询构建器
//!
//! 提供由 [`super::TableDefinition`] 约束的查询构建器，支持字段权限验证和 CRUD。
//!
//! # 主要组件
//!
//! - `TableQuery`：查询构建器，支持链式调用
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::table::{Field, SortOrder, Table};
//! use serde_json::json;
//! use std::sync::Arc;
//!
//! let table = Table::new("users")
//!     .fields(vec![
//!         Field::id("id"),
//!         Field::string("name", 50),
//!         Field::string("email", 100),
//!     ])
//!     .build()?;
//!
//! let query = table.bind(Arc::new(pool)).query(["user"]);
//!
//! // 链式调用构建查询
//! let result = query
//!     .select_fields(&["id", "name", "email"])?
//!     .where_eq("status", json!("active"))?
//!     .order_by("created_at", SortOrder::Desc)?
//!     .page(1, 20)?
//!     .all()
//!     .await?;
//! ```

mod build;
mod filters;
#[cfg(feature = "mysql")]
mod plan;
#[cfg(feature = "mysql")]
mod read;
#[cfg(all(test, feature = "mysql"))]
mod sql_param;
#[cfg(all(test, feature = "mysql"))]
mod sql_render;
mod validation;
#[cfg(feature = "mysql")]
mod write;

// 保持 crate 内既有路径 `crate::table::table_query::SqlParam` 可用于测试
#[cfg(all(test, feature = "mysql"))]
pub(crate) use sql_param::SqlParam;

use crate::table::{QueryParams, TableConfig};
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;

/// 受保护表查询允许的最大单页大小。
///
/// 该上限必须在 `TableQuery` 底层执行，而不能只放在上层 action。自定义 action 或库调用方
/// 可直接调用 `ctx.table_query()?.page(...)`，底层没有上限会绕过内置 `SelectAction` 的保护，
/// 导致超大查询拖垮数据库或应用内存。
pub const MAX_TABLE_QUERY_PAGE_SIZE: usize = super::query_params::MAX_QUERY_PAGE_SIZE;

/// 表查询构建器
///
/// 基于不可变表定义创建受保护的查询构建器，支持：
/// - 字段选择和权限验证
/// - WHERE 条件构建
/// - 排序规则
/// - 分页查询
///
/// # 字段
///
/// - 内部表契约：由 [`super::TableDefinition`] 提供字段定义和权限配置
/// - `user_roles`：用户角色列表，用于权限检查
/// - `query_params`：查询参数，包含字段选择、WHERE 条件、排序规则和分页参数
/// - `pool`：数据库连接池引用（预留，暂未使用）
///
/// # 示例
///
/// ```rust,ignore
/// use yang_base::table::{Field, SortOrder, Table};
/// use std::sync::Arc;
/// use serde_json::json;
///
/// let table = Table::new("users")
///     .fields(vec![Field::id("id"), Field::string("name", 50)])
///     .build()?;
/// let query = table.bind(Arc::new(pool)).query(["admin"]);
///
/// // 选择字段
/// let query = query.select_fields(&["id", "name"])?;
///
/// // 添加 WHERE 条件
/// let query = query.where_eq("status", json!(1))?;
///
/// // 添加排序
/// let query = query.order_by("created_at", SortOrder::Desc)?;
///
/// // 设置分页
/// let query = query.page(1, 20)?;
/// ```
#[derive(Debug, Clone)]
pub struct TableQuery {
    /// 表配置引用
    ///
    /// 包含表结构、字段定义和权限配置
    table_config: Arc<TableConfig>,

    /// 用户角色列表
    ///
    /// 使用 Arc<[String]> 共享所有权，避免不必要的克隆
    user_roles: Arc<[String]>,
    /// 用户角色 HashSet 缓存（O(1) 查找，避免每次 can_read/can_write 时转换）
    user_roles_set: HashSet<String>,

    /// 查询参数
    ///
    /// 包含字段选择、WHERE 条件、排序规则和分页参数
    query_params: QueryParams,

    /// 是否在读取路径包含软删除记录
    ///
    /// 默认 `false`：配置了 `soft_delete_field` 的表，软删行不出现在
    /// select/count/paginate 结果中。置为 `true`（经 [`TableQuery::with_trashed`]）
    /// 时读取全量（含已软删行）。
    include_trashed: bool,

    /// 启动期定义并在请求期注入的租户范围；存在时所有读写均必须带该条件。
    tenant_scope: Option<(String, Value)>,

    /// 数据库连接池引用（预留）
    ///
    /// 暂未使用，预留用于后续实现 CRUD 操作
    #[cfg(feature = "mysql")]
    #[allow(dead_code)]
    pool: Option<Arc<sqlx::MySqlPool>>,

    /// 慢查询阈值（可观测性 C4）
    ///
    /// `Some(d)` 时，单次执行耗时超过 `d` 会 `tracing::warn!`；`None`（默认）
    /// 整个计时分支短路，热路径仅一次 `Instant::now()`，无分配。
    /// 由 [`ActionContext::table_query`](crate::action::ActionContext) 从
    /// [`ObservabilityConfig`](crate::observability::ObservabilityConfig) 注入。
    slow_threshold: Option<std::time::Duration>,

    /// 本次派发的运行期标识（用于慢查询日志串联），由 ActionContext 注入。
    request_id: Option<crate::action::RequestId>,
}
