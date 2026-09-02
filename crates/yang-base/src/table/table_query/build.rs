//! 构造与运行期注入：两个 `new` 构造器（按 `mysql` feature 区分连接池签名）、
//! 慢查询阈值 / request_id 链式注入，以及测试专用的无连接池构造器。

use super::TableQuery;
use crate::table::{QueryParams, TableConfig};
use std::collections::HashSet;
use std::sync::Arc;

impl TableQuery {
    /// 创建新的查询构建器
    ///
    /// # 参数
    ///
    /// - `table_config`：表配置引用
    /// - `user_roles`：用户角色列表（`Arc<[String]>` 共享所有权）
    /// - `pool`：数据库连接池引用（可选）
    ///
    /// # 返回值
    ///
    /// 返回新的 TableQuery 实例
    #[cfg(feature = "mysql")]
    pub(crate) fn new(
        table_config: Arc<TableConfig>,
        user_roles: Arc<[String]>,
        pool: Option<Arc<sqlx::MySqlPool>>,
    ) -> Self {
        let user_roles_set: HashSet<String> = user_roles.iter().cloned().collect();
        Self {
            table_config,
            user_roles,
            user_roles_set,
            query_params: QueryParams::new(),
            include_trashed: false,
            tenant_scope: None,
            pool,
            slow_threshold: None,
            request_id: None,
        }
    }

    /// 创建新的查询构建器（无数据库连接池）
    ///
    /// 当未启用 `mysql` feature 时使用此方法。
    #[cfg(not(feature = "mysql"))]
    pub(crate) fn new(
        table_config: Arc<TableConfig>,
        user_roles: Arc<[String]>,
        _pool: Option<()>,
    ) -> Self {
        let user_roles_set: HashSet<String> = user_roles.iter().cloned().collect();
        Self {
            table_config,
            user_roles,
            user_roles_set,
            query_params: QueryParams::new(),
            include_trashed: false,
            tenant_scope: None,
            slow_threshold: None,
            request_id: None,
        }
    }

    /// 设置慢查询阈值（链式）。超过该耗时的单次执行会 `tracing::warn!`。
    ///
    /// 通常由 `ActionContext::table_query()` 从全局 `ObservabilityConfig` 注入；
    /// 也可手动覆盖。`None`/不调用时不启用慢查询日志。
    pub fn with_slow_threshold(mut self, threshold: Option<std::time::Duration>) -> Self {
        self.slow_threshold = threshold;
        self
    }

    /// 设置本次查询关联的 request_id（链式），用于慢查询日志串联。
    pub fn with_request_id(mut self, request_id: crate::action::RequestId) -> Self {
        self.request_id = Some(request_id);
        self
    }

    /// 创建仅有表配置、无数据库连接池的查询构建器（测试用）
    ///
    /// 使用空的用户角色列表，适合访问无权限限制字段的单元测试。
    #[cfg(all(test, feature = "mysql"))]
    pub(crate) fn new_without_pool(table_config: Arc<TableConfig>) -> Self {
        Self::new(table_config, Arc::from(vec![]), None)
    }
}
