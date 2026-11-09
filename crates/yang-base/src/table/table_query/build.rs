//! 构造与运行期注入：两个 `new` 构造器（按 `mysql` feature 区分连接池签名）、
//! 慢查询阈值 / request_id 链式注入，以及测试专用的无连接池构造器。

use super::TableQuery;
use crate::error::BaseError;
use crate::table::{QueryParams, RegexCache, SortOrder, TableConfig, WhereCondition};
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;

impl TableQuery {
    /// 创建新的查询构建器
    ///
    /// # 参数
    ///
    /// - `table_config`：表配置引用
    /// - `user_roles`：用户角色列表（`Arc<[String]>` 共享所有权）
    /// - `pool`：运行时数据库连接池句柄（MySQL / PostgreSQL 按后端分派，可选）
    ///
    /// # 返回值
    ///
    /// 返回新的 TableQuery 实例
    #[cfg(any(feature = "mysql", feature = "postgres"))]
    pub(crate) fn new(
        table_config: Arc<TableConfig>,
        user_roles: Arc<[String]>,
        pool: Option<crate::database::AnyPool>,
    ) -> Self {
        let user_roles_set: HashSet<String> = user_roles.iter().cloned().collect();
        Self {
            table_config,
            user_roles,
            user_roles_set,
            query_params: QueryParams::new(),
            nulls_orders: std::collections::HashMap::new(),
            any_conditions: Vec::new(),
            include_trashed: false,
            tenant_scope: None,
            pool,
            slow_threshold: None,
            request_id: None,
            regex_cache: RegexCache::default(),
        }
    }

    /// 创建新的查询构建器（无数据库连接池）
    ///
    /// 当未启用任何数据库 feature 时使用此方法。
    #[cfg(not(any(feature = "mysql", feature = "postgres")))]
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
            nulls_orders: std::collections::HashMap::new(),
            any_conditions: Vec::new(),
            include_trashed: false,
            tenant_scope: None,
            slow_threshold: None,
            request_id: None,
            regex_cache: RegexCache::default(),
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

    /// 注入共享有界正则缓存（链式）。通常由 `ActionContext::table_query()` 从
    /// [`crate::tools::Tools`] 注入；未注入时使用默认容量缓存。
    pub fn with_regex_cache(mut self, cache: RegexCache) -> Self {
        self.regex_cache = cache;
        self
    }

    /// 按字段排序并指定 NULL 的位置。
    ///
    /// 字段/权限校验与 [`TableQuery::order_by`] 完全一致。PostgreSQL 渲染原生
    /// `NULLS FIRST/LAST`；MySQL 用 `field IS NULL` 排序表达式模拟（NULL 标记列
    /// 前置于主排序键）。
    pub fn order_nulls(
        mut self,
        field: &str,
        order: SortOrder,
        nulls_first: bool,
    ) -> Result<Self, BaseError> {
        self.validate_order_field(field)?;
        self.query_params.order_by.push((field.to_string(), order));
        self.nulls_orders.insert(field.to_string(), nulls_first);
        Ok(self)
    }

    /// 追加 `IN` 条件，并按参数上限自动分块。
    ///
    /// 与 [`TableQuery::where_in`] 的 500 条上限不同，本入口把超长列表拆成多个
    /// `IN` 子句并以 `OR` 连接，避免单条语句参数数量无限增长。
    pub fn where_in_chunked(self, field: &str, values: Vec<Value>) -> Result<Self, BaseError> {
        if values.is_empty() {
            return Err(BaseError::ParamInvalid(
                "values".to_string(),
                "IN 列表不能为空".to_string(),
            ));
        }
        let chunks = values
            .chunks(Self::MAX_IN_LIST_SIZE)
            .map(|chunk| WhereCondition::In {
                field: field.to_string(),
                values: chunk.to_vec(),
            })
            .collect::<Vec<_>>();
        let condition = match chunks.len() {
            1 => chunks.into_iter().next().expect("len == 1"),
            _ => WhereCondition::Or { conditions: chunks },
        };
        self.where_tree(condition)
    }

    /// `field = ANY($1)` 数组参数入口。
    ///
    /// PostgreSQL 后端把整个列表作为单个数组参数绑定，参数个数不随列表长度增长；
    /// 其它后端（或尚未注入连接池时）降级为 [`TableQuery::where_in_chunked`]。
    #[cfg_attr(not(feature = "postgres"), allow(unused_mut))]
    pub fn where_any(mut self, field: &str, values: Vec<Value>) -> Result<Self, BaseError> {
        if values.is_empty() {
            return Err(BaseError::ParamInvalid(
                "values".to_string(),
                "ANY 列表不能为空".to_string(),
            ));
        }
        // 逐块复用 IN 的校验（字段存在性/筛选权限/值类型），避免绕过受保护层。
        for chunk in values.chunks(Self::MAX_IN_LIST_SIZE) {
            self.validate_condition_tree(
                &WhereCondition::In {
                    field: field.to_string(),
                    values: chunk.to_vec(),
                },
                0,
            )?;
        }

        #[cfg(feature = "postgres")]
        if self
            .pool
            .as_ref()
            .is_some_and(|pool| pool.backend() == yang_db::BackendKind::PostgreSql)
        {
            self.any_conditions.push((field.to_string(), values));
            return Ok(self);
        }

        self.where_in_chunked(field, values)
    }

    /// 创建仅有表配置、无数据库连接池的查询构建器（测试用）
    ///
    /// 使用空的用户角色列表，适合访问无权限限制字段的单元测试。
    #[cfg(all(test, feature = "mysql"))]
    pub(crate) fn new_without_pool(table_config: Arc<TableConfig>) -> Self {
        Self::new(table_config, Arc::from(vec![]), None)
    }
}

#[cfg(all(test, feature = "mysql"))]
mod tests {
    use super::*;
    use crate::table::{Field, Table};

    fn query(table: Table) -> TableQuery {
        let definition = table.build().expect("测试表定义应有效");
        TableQuery::new(
            definition.shared_config(),
            Arc::from(vec!["admin".to_string()]),
            None,
        )
    }

    /// 超过 1000 条的 IN 列表必须拆成多个 `IN` 子句（每个不超过上限）。
    #[test]
    fn where_in_chunked_splits_large_lists() {
        let values: Vec<Value> = (0..1200).map(|i| serde_json::json!(i)).collect();
        let query = query(Table::new("tq_in_chunks").fields([Field::id("id")]))
            .where_in_chunked("id", values)
            .expect("分块 IN 应通过校验");

        assert_eq!(query.query_params.where_conditions.len(), 1);
        match &query.query_params.where_conditions[0] {
            WhereCondition::Or { conditions } => {
                assert_eq!(conditions.len(), 3, "1200 条应拆成 3 块");
                for chunk in conditions {
                    match chunk {
                        WhereCondition::In { values, .. } => {
                            assert!(!values.is_empty());
                            assert!(values.len() <= TableQuery::MAX_IN_LIST_SIZE);
                        }
                        other => panic!("分块元素应为 IN，实际: {other:?}"),
                    }
                }
            }
            other => panic!("超长 IN 应折叠为 OR 组，实际: {other:?}"),
        }
    }

    /// 非 PostgreSQL 后端上 `where_any` 降级为分块 IN，而不是报错。
    #[test]
    fn where_any_falls_back_to_chunked_in() {
        let values: Vec<Value> = (0..1200).map(|i| serde_json::json!(i)).collect();
        let query = query(Table::new("tq_any_fallback").fields([Field::id("id")]))
            .where_any("id", values)
            .expect("where_any 应通过校验");

        assert!(query.any_conditions.is_empty());
        assert!(matches!(
            query.query_params.where_conditions[0],
            WhereCondition::Or { .. }
        ));
    }
}
