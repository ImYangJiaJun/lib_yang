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

use crate::error::BaseError;
use crate::table::{QueryParams, SortOrder, TableConfig, WhereCondition};
#[cfg(feature = "mysql")]
use serde::Serialize;
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

impl TableQuery {
    #[cfg(feature = "mysql")]
    fn compile_predicate(
        &self,
        condition: &WhereCondition,
    ) -> Result<yang_db::Predicate, BaseError> {
        let field = |name: &str| {
            self.table_config
                .get_field_ref(name)
                .cloned()
                .ok_or_else(|| {
                    BaseError::FieldNotFound(self.table_config.table_name.clone(), name.to_string())
                })
        };
        Ok(match condition {
            WhereCondition::Eq { field: name, value } => {
                yang_db::Predicate::Compare(field(name)?, yang_db::CompareOp::Eq, value.clone())
            }
            WhereCondition::Ne { field: name, value } => {
                yang_db::Predicate::Compare(field(name)?, yang_db::CompareOp::Ne, value.clone())
            }
            WhereCondition::Gt { field: name, value } => {
                yang_db::Predicate::Compare(field(name)?, yang_db::CompareOp::Gt, value.clone())
            }
            WhereCondition::Gte { field: name, value } => {
                yang_db::Predicate::Compare(field(name)?, yang_db::CompareOp::Gte, value.clone())
            }
            WhereCondition::Lt { field: name, value } => {
                yang_db::Predicate::Compare(field(name)?, yang_db::CompareOp::Lt, value.clone())
            }
            WhereCondition::Lte { field: name, value } => {
                yang_db::Predicate::Compare(field(name)?, yang_db::CompareOp::Lte, value.clone())
            }
            WhereCondition::Like {
                field: name,
                pattern,
            } => yang_db::Predicate::Compare(
                field(name)?,
                yang_db::CompareOp::Like,
                Value::String(pattern.clone()),
            ),
            WhereCondition::In {
                field: name,
                values,
            } => yang_db::Predicate::In(field(name)?, values.clone()),
            WhereCondition::NotIn {
                field: name,
                values,
            } => yang_db::Predicate::NotIn(field(name)?, values.clone()),
            WhereCondition::Between {
                field: name,
                lo,
                hi,
            } => yang_db::Predicate::Between(field(name)?, lo.clone(), hi.clone()),
            WhereCondition::IsNull { field: name } => yang_db::Predicate::IsNull(field(name)?),
            WhereCondition::IsNotNull { field: name } => {
                yang_db::Predicate::IsNotNull(field(name)?)
            }
            WhereCondition::And { conditions } => yang_db::Predicate::And(
                conditions
                    .iter()
                    .map(|value| self.compile_predicate(value))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            WhereCondition::Or { conditions } => yang_db::Predicate::Or(
                conditions
                    .iter()
                    .map(|value| self.compile_predicate(value))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        })
    }

    #[cfg(feature = "mysql")]
    fn compile_db_query(&self) -> Result<yang_db::QueryBuilder<'_>, BaseError> {
        let pool = self
            .pool
            .as_deref()
            .ok_or(BaseError::DatabaseNotInitialized)?;
        self.apply_db_plan(yang_db::QueryBuilder::from_pool(
            pool,
            &self.table_config.table_ref,
        ))
    }

    #[cfg(feature = "mysql")]
    fn apply_db_plan<'a>(
        &self,
        mut query: yang_db::QueryBuilder<'a>,
    ) -> Result<yang_db::QueryBuilder<'a>, BaseError> {
        let selected = self.query_params.fields.as_ref().map_or_else(
            || {
                self.default_read_fields()
                    .map(|values| values.into_iter().map(str::to_string).collect())
            },
            |values| Ok(values.clone()),
        )?;
        for name in selected {
            let field = self.table_config.get_field_ref(&name).ok_or_else(|| {
                BaseError::FieldNotFound(self.table_config.table_name.clone(), name.clone())
            })?;
            query = query.field(field);
        }
        for condition in &self.query_params.where_conditions {
            query = query
                .where_predicate(&self.compile_predicate(condition)?)
                .map_err(BaseError::DatabaseQueryFailed)?;
        }
        if !self.include_trashed {
            if let Some(name) = &self.table_config.soft_delete_field {
                let field = self.table_config.get_field_ref(name).ok_or_else(|| {
                    BaseError::FieldNotFound(self.table_config.table_name.clone(), name.clone())
                })?;
                query = query.where_null(field);
            }
        }
        let orders = if self.query_params.order_by.is_empty() {
            &self.table_config.default_order
        } else {
            &self.query_params.order_by
        };
        for (name, order) in orders {
            let field = self.table_config.get_field_ref(name).ok_or_else(|| {
                BaseError::FieldNotFound(self.table_config.table_name.clone(), name.clone())
            })?;
            let order = match order {
                SortOrder::Asc => yang_db::SortOrder::Asc,
                SortOrder::Desc => yang_db::SortOrder::Desc,
            };
            query = query.order(field, order);
        }
        if let Some(page_size) = self.query_params.page_size {
            let page = self.query_params.page.unwrap_or(1).max(1);
            let limit = u64::try_from(page_size).map_err(|_| {
                BaseError::ParamInvalid("page_size".to_string(), "分页大小超出范围".to_string())
            })?;
            let offset = u64::try_from((page - 1).saturating_mul(page_size)).map_err(|_| {
                BaseError::ParamInvalid("page".to_string(), "分页偏移超出范围".to_string())
            })?;
            query = query.limit(limit).offset(offset);
        }
        Ok(query)
    }

    /// 对字段名进行反引号转义
    ///
    /// 对合法标识符添加反引号，内部反引号转义为双反引号。
    /// 非法字段名返回 `BaseError::FieldNotFound`。
    ///
    /// # 参数
    ///
    /// - `field`: 字段名
    ///
    /// # 返回
    ///
    /// - `Ok(String)`: 转义后的字段名，如 `` `field_name` ``
    /// - `Err(BaseError)`: 字段名非法
    #[cfg(feature = "mysql")]
    #[cfg(test)]
    fn quote_identifier(&self, field: &str) -> Result<String, BaseError> {
        self.table_config
            .get_field_ref(field)
            .map(|field| field.mysql_quoted().to_string())
            .ok_or_else(|| {
                BaseError::FieldNotFound(self.table_config.table_name.clone(), field.to_string())
            })
    }

    /// 对表名进行反引号转义（与字段名走同一条校验/转义路径）
    ///
    /// 防止非法表名破坏引号边界。表名虽来自开发者配置而非终端用户输入，
    /// 但统一转义可消除字段名与表名处理的不一致（防御纵深）。
    ///
    /// # 返回
    ///
    /// - `Ok(String)`: 转义后的表名，如 `` `users` ``
    /// - `Err(BaseError)`: 表名非法
    #[cfg(feature = "mysql")]
    #[cfg(test)]
    fn quoted_table_name(&self) -> Result<String, BaseError> {
        Ok(format!("`{}`", self.table_config.table_ref.as_str()))
    }

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

    /// 校验字段存在，并且当前角色具有读取权限。
    fn validate_read_field(&self, field_name: &str) -> Result<(), BaseError> {
        let field_config = self.table_config.get_field(field_name).ok_or_else(|| {
            BaseError::FieldNotFound(self.table_config.table_name.clone(), field_name.to_string())
        })?;

        if !field_config.permissions.can_read(&self.user_roles_set) {
            return Err(BaseError::FieldPermissionDenied(
                self.table_config.table_name.clone(),
                field_name.to_string(),
                "用户无读取权限".to_string(),
            ));
        }

        Ok(())
    }

    #[cfg(feature = "mysql")]
    fn default_read_fields(&self) -> Result<Vec<&str>, BaseError> {
        let mut fields: Vec<&str> = self
            .table_config
            .fields
            .iter()
            .filter_map(|(name, field)| {
                (!field.hidden && field.permissions.can_read(&self.user_roles_set))
                    .then_some(name.as_str())
            })
            .collect();
        fields.sort_unstable();
        if fields.is_empty() {
            return Err(BaseError::FieldPermissionDenied(
                self.table_config.table_name.clone(),
                "*".to_string(),
                "当前角色没有可读字段".to_string(),
            ));
        }
        Ok(fields)
    }

    #[cfg(feature = "mysql")]
    pub(crate) fn ensure_readable_projection(&self) -> Result<(), BaseError> {
        self.default_read_fields().map(|_| ())
    }

    /// 选择要查询的字段。
    ///
    /// 会校验字段存在，并校验当前用户角色具备这些字段的读取权限。
    pub fn select_fields(mut self, fields: &[&str]) -> Result<Self, BaseError> {
        if fields.is_empty() {
            return Err(BaseError::ParamInvalid(
                "fields".to_string(),
                "查询字段列表不能为空".to_string(),
            ));
        }

        // 验证每个字段
        for field_name in fields {
            self.validate_read_field(field_name)?;
        }

        // 设置字段列表
        self.query_params.fields = Some(fields.iter().map(|s| s.to_string()).collect());

        Ok(self)
    }

    /// 添加等于条件 (WHERE field = value)
    ///
    /// 添加 WHERE 等于条件，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的筛选权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    /// - `value`：比较值
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use serde_json::json;
    ///
    /// let query = query.where_eq("status", json!("active"))?;
    /// ```
    pub fn where_eq(self, field: &str, value: Value) -> Result<Self, BaseError> {
        self.push_where_condition(WhereCondition::Eq {
            field: field.to_string(),
            value,
        })
    }

    /// 添加包含条件 (WHERE field IN (values))
    ///
    /// 添加 WHERE IN 条件，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的筛选权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    /// - `values`：值列表
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use serde_json::json;
    ///
    /// let query = query.where_in("status", vec![json!(1), json!(2), json!(3)])?;
    /// ```
    pub fn where_in(self, field: &str, values: Vec<Value>) -> Result<Self, BaseError> {
        if values.is_empty() {
            return Err(BaseError::ParamInvalid(
                "values".to_string(),
                "IN 列表不能为空".to_string(),
            ));
        }

        // QRY-2: IN 列表元素数上限
        if values.len() > Self::MAX_IN_LIST_SIZE {
            return Err(BaseError::ParamInvalid(
                "values".to_string(),
                format!(
                    "IN 列表元素数 {} 超过上限 {}",
                    values.len(),
                    Self::MAX_IN_LIST_SIZE
                ),
            ));
        }

        self.push_where_condition(WhereCondition::In {
            field: field.to_string(),
            values,
        })
    }

    /// 添加模糊匹配条件 (WHERE field LIKE pattern)
    ///
    /// 添加 WHERE LIKE 条件，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的筛选权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    /// - `pattern`：匹配模式，支持 % 和 _ 通配符
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let query = query.where_like("name", "%alice%")?;
    /// ```
    pub fn where_like(self, field: &str, pattern: String) -> Result<Self, BaseError> {
        // QRY-1: LIKE pattern 长度上限
        if pattern.len() > Self::MAX_LIKE_PATTERN_LEN {
            return Err(BaseError::ParamInvalid(
                "pattern".to_string(),
                format!(
                    "LIKE pattern 长度 {} 超过上限 {}",
                    pattern.len(),
                    Self::MAX_LIKE_PATTERN_LEN
                ),
            ));
        }

        self.push_where_condition(WhereCondition::Like {
            field: field.to_string(),
            pattern,
        })
    }

    /// 添加模糊匹配条件 (WHERE field LIKE '%keyword%')
    ///
    /// 便捷方法：自动将 `keyword` 用 `%` 包裹，并转义其中的 `%` 和 `_` 通配符，
    /// 避免用户输入中的通配符被解释为 SQL LIKE 语法。
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    /// - `keyword`：搜索关键词（无需手动加 `%`）
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败（字段不存在 / 无权限 / 转义后超长）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // WHERE name LIKE '%alice%'，且 "%" / "_" 被转义
    /// let query = query.where_contains("name", "alice")?;
    /// ```
    pub fn where_contains(mut self, field: &str, keyword: &str) -> Result<Self, BaseError> {
        if keyword.trim().is_empty() {
            return Err(BaseError::ParamInvalid(
                "keyword".to_string(),
                "搜索关键词不能为空".to_string(),
            ));
        }

        // 转义 LIKE 通配符：% → \%，_ → \_
        let escaped = keyword
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%{}%", escaped);

        // 复用 where_like 的校验（含 pattern 长度上限 + 字段权限）
        self = self.where_like(field, pattern)?;
        Ok(self)
    }

    /// 添加不等于条件 (WHERE field <> value)
    ///
    /// 添加 WHERE 不等于条件，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的筛选权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    /// - `value`：比较值
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    pub fn where_ne(self, field: &str, value: Value) -> Result<Self, BaseError> {
        self.push_where_condition(WhereCondition::Ne {
            field: field.to_string(),
            value,
        })
    }

    /// 添加小于条件 (WHERE field < value)
    ///
    /// 添加 WHERE 小于条件，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的筛选权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    /// - `value`：比较值
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    pub fn where_lt(self, field: &str, value: Value) -> Result<Self, BaseError> {
        self.push_where_condition(WhereCondition::Lt {
            field: field.to_string(),
            value,
        })
    }

    /// 添加小于等于条件 (WHERE field <= value)
    ///
    /// 添加 WHERE 小于等于条件，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的筛选权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    /// - `value`：比较值
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    pub fn where_lte(self, field: &str, value: Value) -> Result<Self, BaseError> {
        self.push_where_condition(WhereCondition::Lte {
            field: field.to_string(),
            value,
        })
    }

    /// 添加大于条件 (WHERE field > value)
    ///
    /// 添加 WHERE 大于条件，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的筛选权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    /// - `value`：比较值
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    pub fn where_gt(self, field: &str, value: Value) -> Result<Self, BaseError> {
        self.push_where_condition(WhereCondition::Gt {
            field: field.to_string(),
            value,
        })
    }

    /// 添加大于等于条件 (WHERE field >= value)
    ///
    /// 添加 WHERE 大于等于条件，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的筛选权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    /// - `value`：比较值
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    pub fn where_gte(self, field: &str, value: Value) -> Result<Self, BaseError> {
        self.push_where_condition(WhereCondition::Gte {
            field: field.to_string(),
            value,
        })
    }

    /// 添加区间条件 (WHERE field BETWEEN lo AND hi)
    ///
    /// 添加 WHERE BETWEEN 条件，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的筛选权限
    ///
    /// 当 `lo > hi` 时 BETWEEN 返回空集（MySQL 标准行为，框架不做特殊处理）。
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    /// - `lo`：区间下界（包含）
    /// - `hi`：区间上界（包含）
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    pub fn where_between(self, field: &str, lo: Value, hi: Value) -> Result<Self, BaseError> {
        self.push_where_condition(WhereCondition::Between {
            field: field.to_string(),
            lo,
            hi,
        })
    }

    /// 添加空值判断 (WHERE field IS NULL)
    ///
    /// 添加 WHERE IS NULL 条件，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的筛选权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    pub fn where_null(self, field: &str) -> Result<Self, BaseError> {
        self.push_where_condition(WhereCondition::IsNull {
            field: field.to_string(),
        })
    }

    /// 添加非空值判断 (WHERE field IS NOT NULL)
    ///
    /// 添加 WHERE IS NOT NULL 条件，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的筛选权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    pub fn where_not_null(self, field: &str) -> Result<Self, BaseError> {
        self.push_where_condition(WhereCondition::IsNotNull {
            field: field.to_string(),
        })
    }

    /// 添加不在列表条件 (WHERE field NOT IN (values))
    ///
    /// 添加 WHERE NOT IN 条件，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的筛选权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    /// - `values`：排除值列表
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    pub fn where_not_in(self, field: &str, values: Vec<Value>) -> Result<Self, BaseError> {
        if values.is_empty() {
            return Err(BaseError::ParamInvalid(
                "values".to_string(),
                "NOT IN 列表不能为空".to_string(),
            ));
        }

        // QRY-2: NOT IN 列表元素数上限
        if values.len() > Self::MAX_IN_LIST_SIZE {
            return Err(BaseError::ParamInvalid(
                "values".to_string(),
                format!(
                    "NOT IN 列表元素数 {} 超过上限 {}",
                    values.len(),
                    Self::MAX_IN_LIST_SIZE
                ),
            ));
        }

        self.push_where_condition(WhereCondition::NotIn {
            field: field.to_string(),
            values,
        })
    }

    /// 添加排序规则 (ORDER BY field direction)
    ///
    /// 添加排序规则，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的排序权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    /// - `order`：排序方向 (Asc 或 Desc)
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无排序权限
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::table::SortOrder;
    ///
    /// let query = query.order_by("created_at", SortOrder::Desc)?;
    /// ```
    fn validate_order_field(&self, field: &str) -> Result<(), BaseError> {
        // 1. 检查字段是否存在
        let field_config = self.table_config.get_field(field).ok_or_else(|| {
            BaseError::FieldNotFound(self.table_config.table_name.clone(), field.to_string())
        })?;

        // 2. 字段级排序开关：标记为不可排序的字段直接拒绝（先于角色权限，
        //    确保 `.sortable(false)` 是硬约束而非可被空角色列表绕过的软提示）
        if !field_config.sortable {
            return Err(BaseError::FieldPermissionDenied(
                self.table_config.table_name.clone(),
                field.to_string(),
                "字段不允许排序".to_string(),
            ));
        }

        // 3. 检查用户是否有排序权限
        if !field_config.permissions.can_sort(&self.user_roles_set) {
            return Err(BaseError::FieldPermissionDenied(
                self.table_config.table_name.clone(),
                field.to_string(),
                "用户无排序权限".to_string(),
            ));
        }

        Ok(())
    }

    /// 添加排序规则。
    ///
    /// 会校验字段存在、字段允许排序，以及当前用户角色具备排序权限。
    pub fn order_by(mut self, field: &str, order: SortOrder) -> Result<Self, BaseError> {
        self.validate_order_field(field)?;

        // 添加排序规则
        self.query_params.order_by.push((field.to_string(), order));

        Ok(self)
    }

    /// 设置分页参数 (LIMIT and OFFSET)
    ///
    /// 设置查询的分页参数
    ///
    /// # 参数
    ///
    /// - `page`：当前页码，从 1 开始
    /// - `page_size`：每页大小
    ///
    /// # 返回值
    ///
    /// 返回 self 支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let query = query.page(1, 20)?;
    /// ```
    pub fn page(mut self, page: usize, page_size: usize) -> Result<Self, BaseError> {
        if page == 0 || page_size == 0 {
            return Err(BaseError::ParamInvalid(
                "page".to_string(),
                "页码与每页大小必须从 1 开始且大于 0".to_string(),
            ));
        }
        if page_size > MAX_TABLE_QUERY_PAGE_SIZE {
            return Err(BaseError::ParamInvalid(
                "page_size".to_string(),
                format!("每页大小不能超过 {}", MAX_TABLE_QUERY_PAGE_SIZE),
            ));
        }
        self.query_params.page = Some(page);
        self.query_params.page_size = Some(page_size);
        Ok(self)
    }

    /// 为服务端有界预取设置从首行开始的硬上限。
    ///
    /// 该入口只供 crate 内已经持有可信上限的算法使用（例如树查询的
    /// `max_nodes + 1` 截断检测），不接受终端用户分页参数，因此不套用公开分页的
    /// 100 行产品限制。
    #[cfg(feature = "mysql")]
    pub(crate) fn prefetch_limit(mut self, limit: usize) -> Result<Self, BaseError> {
        if limit == 0 {
            return Err(BaseError::ParamInvalid(
                "limit".to_string(),
                "预取上限必须大于 0".to_string(),
            ));
        }
        self.query_params.page = Some(1);
        self.query_params.page_size = Some(limit);
        Ok(self)
    }

    /// 对声明了 searchable 且当前角色可读的文本字段应用一次 OR LIKE 搜索。
    ///
    /// 关键词搜索只认独立的 searchable 位（[`crate::table::Field::searchable`]），与
    /// 结构化 where 的 filterable 校验（`validate_filter_field`）互不开放。搜索字段
    /// 来自表定义迭代（必然存在）且已逐一验证 `is_text` / `searchable` / 可读 /
    /// 非 hidden，因此这里自行构造 OR 组、不经过 `where_or` 的 filterable 门槛；
    /// 但 LIKE pattern 长度上限（QRY-1）仍在本地强制执行。
    pub fn search(mut self, keyword: Option<&str>) -> Result<Self, BaseError> {
        let Some(keyword) = keyword.map(str::trim).filter(|value| !value.is_empty()) else {
            return Ok(self);
        };
        let mut fields = self
            .table_config
            .fields
            .iter()
            .filter(|(_, field)| {
                field.field_type.is_text()
                    && field.searchable
                    && field.permissions.can_read(&self.user_roles_set)
                    && !field.hidden
            })
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        fields.sort();
        if fields.is_empty() {
            return Err(BaseError::PermissionDenied(format!(
                "表 {} 没有当前角色可搜索的文本字段",
                self.table_config.table_name
            )));
        }
        let pattern = format!("%{keyword}%");
        if pattern.len() > Self::MAX_LIKE_PATTERN_LEN {
            return Err(BaseError::ParamInvalid(
                "pattern".to_string(),
                format!(
                    "LIKE pattern 长度 {} 超过上限 {}",
                    pattern.len(),
                    Self::MAX_LIKE_PATTERN_LEN
                ),
            ));
        }
        let group = WhereCondition::Or {
            conditions: fields
                .into_iter()
                .map(|field| WhereCondition::Like {
                    field,
                    pattern: pattern.clone(),
                })
                .collect(),
        };
        self.query_params.where_conditions.push(group);
        Ok(self)
    }

    /// 在读取路径包含软删除记录
    ///
    /// 默认情况下，配置了 `soft_delete_field` 的表会在 select/count/paginate
    /// 时自动追加 `软删字段 IS NULL` 过滤，隐藏已软删行。调用本方法后，本次查询
    /// 读取全量数据（含已软删行）。
    ///
    /// # 返回值
    ///
    /// 返回 self 支持链式调用
    pub fn with_trashed(mut self) -> Self {
        self.include_trashed = true;
        self
    }

    /// 注入强制租户范围。该条件绕过业务筛选权限，但字段必须是定义中的 tenant key。
    pub(crate) fn scope_tenant(mut self, field: &str, value: Value) -> Result<Self, BaseError> {
        let config = self.table_config.get_field(field).ok_or_else(|| {
            BaseError::FieldNotFound(self.table_config.table_name.clone(), field.to_string())
        })?;
        if !config.tenant_key {
            return Err(BaseError::ConfigError(format!(
                "字段 {}.{} 未声明为 tenant_key",
                self.table_config.table_name, field
            )));
        }
        config.field_type.validate(field, &value)?;
        self.query_params.where_conditions.push(WhereCondition::Eq {
            field: field.to_string(),
            value: value.clone(),
        });
        self.tenant_scope = Some((field.to_string(), value));
        Ok(self)
    }

    /// 注入受信的主键等值条件。
    ///
    /// 内置 get/put/del 的主键定位是 Action 自有寻址机制，不是调用方可选的结构化
    /// 筛选，因此与 [`Self::scope_tenant`] 一样绕过 filterable 业务筛选权限；
    /// 值仍按主键字段类型校验（null 交由渲染器规范化为 IS NULL，匹配不到记录）。
    #[cfg(feature = "mysql")]
    pub(crate) fn where_primary_key_eq(mut self, value: Value) -> Result<Self, BaseError> {
        let field = self.table_config.primary_key.clone();
        let config = self.table_config.get_field(&field).ok_or_else(|| {
            BaseError::FieldNotFound(self.table_config.table_name.clone(), field.clone())
        })?;
        if !value.is_null() {
            config.field_type.validate(&field, &value)?;
        }
        self.query_params
            .where_conditions
            .push(WhereCondition::Eq { field, value });
        Ok(self)
    }

    /// 验证筛选字段的权限
    ///
    /// 内部辅助方法，用于验证字段是否存在以及用户是否有筛选权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    ///
    /// # 返回值
    ///
    /// - `Ok(())`：验证通过
    /// - `Err(BaseError)`：验证失败
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    fn validate_filter_field(&self, field: &str) -> Result<(), BaseError> {
        // 1. 检查字段是否存在
        let field_config = self.table_config.get_field(field).ok_or_else(|| {
            BaseError::FieldNotFound(self.table_config.table_name.clone(), field.to_string())
        })?;

        // 2. 字段级筛选开关：标记为不可筛选的字段直接拒绝（先于角色权限，
        //    确保 `.filterable(false)` 是硬约束而非可被空角色列表绕过的软提示）
        if !field_config.filterable {
            return Err(BaseError::FieldPermissionDenied(
                self.table_config.table_name.clone(),
                field.to_string(),
                "字段不允许筛选".to_string(),
            ));
        }

        // 3. 检查用户是否有筛选权限
        if !field_config.permissions.can_filter(&self.user_roles_set) {
            return Err(BaseError::FieldPermissionDenied(
                self.table_config.table_name.clone(),
                field.to_string(),
                "用户无筛选权限".to_string(),
            ));
        }

        Ok(())
    }

    /// 通过同一校验边界追加任意 WHERE 条件，避免各链式入口出现校验差异。
    fn push_where_condition(mut self, condition: WhereCondition) -> Result<Self, BaseError> {
        self.validate_condition_tree(&condition, 0)?;
        self.query_params.where_conditions.push(condition);
        Ok(self)
    }

    /// 校验叶子条件的操作符与字段类型兼容，并验证每一个参与比较的值。
    fn validate_condition_values(
        &self,
        condition: &WhereCondition,
        field: &str,
    ) -> Result<(), BaseError> {
        let field_config = self.table_config.get_field(field).ok_or_else(|| {
            BaseError::FieldNotFound(self.table_config.table_name.clone(), field.to_string())
        })?;
        let field_type = &field_config.field_type;

        let reject_operator = |operator: &str| {
            BaseError::ParamInvalid(
                field.to_string(),
                format!("字段类型 {field_type:?} 不支持 {operator} 条件"),
            )
        };
        let validate = |value: &Value| field_type.validate(field, value);
        let is_orderable = matches!(
            field_type,
            crate::table::FieldType::String { .. }
                | crate::table::FieldType::Integer
                | crate::table::FieldType::BigInt
                | crate::table::FieldType::Float
                | crate::table::FieldType::Double
                | crate::table::FieldType::Decimal { .. }
                | crate::table::FieldType::Date
                | crate::table::FieldType::DateTime
                | crate::table::FieldType::Timestamp
                | crate::table::FieldType::Text
                | crate::table::FieldType::Enum { .. }
        );
        let is_textual = matches!(
            field_type,
            crate::table::FieldType::String { .. }
                | crate::table::FieldType::Text
                | crate::table::FieldType::Enum { .. }
        );

        match condition {
            WhereCondition::Eq { value, .. } | WhereCondition::Ne { value, .. } => {
                // NULL 比较由渲染器规范化为 IS NULL / IS NOT NULL，不作为字段值校验。
                if value.is_null() {
                    Ok(())
                } else {
                    validate(value)
                }
            }
            WhereCondition::In { values, .. } | WhereCondition::NotIn { values, .. } => {
                values.iter().try_for_each(validate)
            }
            WhereCondition::Like { .. } => {
                if is_textual {
                    Ok(())
                } else {
                    Err(reject_operator("LIKE"))
                }
            }
            WhereCondition::Gt { value, .. }
            | WhereCondition::Gte { value, .. }
            | WhereCondition::Lt { value, .. }
            | WhereCondition::Lte { value, .. } => {
                if !is_orderable {
                    return Err(reject_operator("范围比较"));
                }
                validate(value)
            }
            WhereCondition::Between { lo, hi, .. } => {
                if !is_orderable {
                    return Err(reject_operator("BETWEEN"));
                }
                validate(lo)?;
                validate(hi)
            }
            WhereCondition::IsNull { .. } | WhereCondition::IsNotNull { .. } => Ok(()),
            WhereCondition::And { .. } | WhereCondition::Or { .. } => Err(BaseError::ParamInvalid(
                "condition".to_string(),
                "逻辑组不能作为叶子条件校验".to_string(),
            )),
        }
    }

    /// 嵌套布尔条件的最大递归深度，防止深层嵌套（或恶意构造）爆栈。
    ///
    /// 校验期（`validate_condition_tree`）与渲染期（`render_condition`）共用同一上限。
    const MAX_WHERE_DEPTH: usize = 32;

    /// LIKE pattern 最大字节长度（QRY-1 防 DoS）。
    ///
    /// 超长 pattern 会导致 MySQL 索引失效、全表扫描放大；`where_like` /
    /// `where_contains` / 校验期 / 渲染期统一拦截。
    const MAX_LIKE_PATTERN_LEN: usize = 128;

    /// IN / NOT IN 列表最大元素数（QRY-2 防 DoS）。
    ///
    /// 过长的 IN 列表会导致 MySQL 优化器退化、解析/绑定开销放大；`where_in` /
    /// `where_not_in` / 校验期 / 渲染期统一拦截。
    const MAX_IN_LIST_SIZE: usize = 500;

    /// 递归校验一棵 WHERE 条件树的字段与筛选权限。
    ///
    /// 叶子条件校验其字段存在且当前角色可筛选；逻辑组（`And`/`Or`）递归下钻校验
    /// 每个子条件。深度超过 [`Self::MAX_WHERE_DEPTH`] 返回 `ParamInvalid` 而非
    /// panic，与渲染期保持一致的防爆栈上限。
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：某叶子字段不存在
    /// - `BaseError::FieldPermissionDenied`：某叶子字段无筛选权限
    /// - `BaseError::ParamInvalid`：嵌套层数超限
    fn validate_condition_tree(
        &self,
        condition: &WhereCondition,
        depth: usize,
    ) -> Result<(), BaseError> {
        if depth > Self::MAX_WHERE_DEPTH {
            return Err(BaseError::ParamInvalid(
                "where".to_string(),
                format!("嵌套布尔条件层数超过上限 {}", Self::MAX_WHERE_DEPTH),
            ));
        }

        match condition {
            WhereCondition::And { conditions } | WhereCondition::Or { conditions } => {
                // 空布尔组拒绝：空 And 渲染为 `1=1`、空 Or 渲染为 `1=0`，前者会使
                // `where_conditions` 非空从而绕过 UPDATE/DELETE 的全表写守卫，生成
                // `WHERE (1=1)` 全表改写。在校验期直接拒绝空组，杜绝该绕过路径。
                if conditions.is_empty() {
                    return Err(BaseError::ParamInvalid(
                        "where".to_string(),
                        "AND/OR 布尔组不能为空".to_string(),
                    ));
                }
                for child in conditions {
                    self.validate_condition_tree(child, depth + 1)?;
                }
                Ok(())
            }
            // 叶子：必有字段，校验存在性与筛选权限；同时校验 LIKE/IN 上限（QRY-1/QRY-2）
            leaf => {
                let field = leaf.field().ok_or_else(|| {
                    BaseError::ParamInvalid(
                        "condition".to_string(),
                        "条件节点缺少字段名".to_string(),
                    )
                })?;
                self.validate_filter_field(field)?;
                // LIKE pattern 长度上限（QRY-1）
                if let WhereCondition::Like { pattern, .. } = leaf {
                    if pattern.len() > Self::MAX_LIKE_PATTERN_LEN {
                        return Err(BaseError::ParamInvalid(
                            "pattern".to_string(),
                            format!(
                                "LIKE pattern 长度 {} 超过上限 {}",
                                pattern.len(),
                                Self::MAX_LIKE_PATTERN_LEN
                            ),
                        ));
                    }
                }
                // IN / NOT IN 列表元素数上限（QRY-2）
                if let WhereCondition::In { values, .. } | WhereCondition::NotIn { values, .. } =
                    leaf
                {
                    if values.is_empty() {
                        return Err(BaseError::ParamInvalid(
                            "values".to_string(),
                            "IN/NOT IN 列表不能为空".to_string(),
                        ));
                    }

                    if values.len() > Self::MAX_IN_LIST_SIZE {
                        return Err(BaseError::ParamInvalid(
                            "values".to_string(),
                            format!(
                                "IN/NOT IN 列表元素数 {} 超过上限 {}",
                                values.len(),
                                Self::MAX_IN_LIST_SIZE
                            ),
                        ));
                    }
                }
                self.validate_condition_values(leaf, field)
            }
        }
    }

    /// 添加一个 OR 逻辑组 (WHERE ... AND (c1 OR c2 OR ...))
    ///
    /// 组内每个子条件递归校验字段存在性与筛选权限；通过后整组以 `Or` 节点追加到
    /// 顶层条件列表，与既有条件以隐式 AND 连接。空组等价于恒假（`1=0`）。
    ///
    /// 子条件可由 [`WhereCondition`] 直接构造，亦可嵌套 `And`/`Or` 组（深度上限
    /// `MAX_WHERE_DEPTH`）。
    ///
    /// # 参数
    ///
    /// - `conditions`：OR 组的子条件列表
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：校验通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：字段不存在 / 无筛选权限 / 嵌套超限
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::table::WhereCondition;
    /// use serde_json::json;
    ///
    /// // WHERE status = 'active' AND (age >= 18 OR vip = true)
    /// let query = query
    ///     .where_eq("status", json!("active"))?
    ///     .where_or(vec![
    ///         WhereCondition::Gte { field: "age".into(), value: json!(18) },
    ///         WhereCondition::Eq { field: "vip".into(), value: json!(true) },
    ///     ])?;
    /// ```
    pub fn where_or(mut self, conditions: Vec<WhereCondition>) -> Result<Self, BaseError> {
        let group = WhereCondition::Or { conditions };
        // 递归校验整棵子树（含嵌套组）
        self.validate_condition_tree(&group, 0)?;
        self.query_params.where_conditions.push(group);
        Ok(self)
    }

    /// 添加一个 AND 逻辑组 (WHERE ... AND (c1 AND c2 AND ...))
    ///
    /// 语义同 [`TableQuery::where_or`]，但组内子条件以 AND 连接。主要用于在 OR 组
    /// 内部嵌套 AND 子组；顶层多个条件本就隐式 AND，单独使用通常无必要。空组等价
    /// 于恒真（`1=1`）。
    ///
    /// # 参数
    ///
    /// - `conditions`：AND 组的子条件列表
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：校验通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：字段不存在 / 无筛选权限 / 嵌套超限
    pub fn where_and(mut self, conditions: Vec<WhereCondition>) -> Result<Self, BaseError> {
        let group = WhereCondition::And { conditions };
        self.validate_condition_tree(&group, 0)?;
        self.query_params.where_conditions.push(group);
        Ok(self)
    }

    /// 追加一棵任意 WHERE 条件（叶子或 `And`/`Or` 组），递归校验后并入顶层条件。
    ///
    /// 这是以 [`WhereCondition`] 表示的类型化布尔树桥接到受保护层的统一入口：
    /// 整棵树先经 `validate_condition_tree` 递归校验字段存在性、筛选权限与
    /// 嵌套深度，通过后作为单个条件追加（与既有条件隐式 AND 连接）。
    ///
    /// # 参数
    ///
    /// - `condition`：任意 `WhereCondition`（叶子或逻辑组）
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：校验通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：字段不存在 / 无筛选权限 / 嵌套超限
    pub fn where_tree(mut self, condition: WhereCondition) -> Result<Self, BaseError> {
        self.validate_condition_tree(&condition, 0)?;
        self.query_params.where_conditions.push(condition);
        Ok(self)
    }

    /// 获取查询参数的引用
    ///
    /// 用于测试或调试，获取当前构建的查询参数
    ///
    /// # 返回值
    ///
    /// 返回查询参数的引用
    #[allow(dead_code)]
    pub fn get_query_params(&self) -> &QueryParams {
        &self.query_params
    }

    /// 应用通用列表参数，并复用本类型现有的字段/筛选/排序/分页权限校验。
    pub fn apply_params(mut self, mut params: QueryParams) -> Result<Self, BaseError> {
        params.normalize();
        if let Some(fields) = params.fields {
            let names = fields.iter().map(String::as_str).collect::<Vec<_>>();
            self = self.select_fields(&names)?;
        }
        for condition in params.where_conditions {
            self = self.where_tree(condition)?;
        }
        for (field, order) in params.order_by {
            self = self.order_by(&field, order)?;
        }
        if params.page.is_some() || params.page_size.is_some() {
            self = self.page(
                params.page.unwrap_or(1),
                params
                    .page_size
                    .unwrap_or(crate::table::query_params::DEFAULT_QUERY_PAGE_SIZE),
            )?;
        }
        Ok(self)
    }

    /// 获取表配置的引用
    ///
    /// 用于测试或调试，获取表配置
    ///
    /// # 返回值
    ///
    /// 返回表配置的引用
    #[allow(dead_code)]
    pub(crate) fn get_table_config(&self) -> &Arc<TableConfig> {
        &self.table_config
    }

    /// 获取用户角色列表的引用
    ///
    /// 用于测试或调试，获取用户角色列表
    ///
    /// # 返回值
    ///
    /// 返回用户角色列表的引用
    #[allow(dead_code)]
    pub fn get_user_roles(&self) -> &[String] {
        &self.user_roles
    }

    /// 创建仅有表配置、无数据库连接池的查询构建器（测试用）
    ///
    /// 使用空的用户角色列表，适合访问无权限限制字段的单元测试。
    #[cfg(all(test, feature = "mysql"))]
    pub(crate) fn new_without_pool(table_config: Arc<TableConfig>) -> Self {
        Self::new(table_config, Arc::from(vec![]), None)
    }
}

/// 数据库执行方法（需要启用 `mysql` feature）
#[cfg(feature = "mysql")]
impl TableQuery {
    /// 执行分页查询操作
    ///
    /// 执行分页查询，包括以下步骤：
    /// 1. 执行 COUNT(*) 查询获取总记录数
    /// 2. 计算 LIMIT 和 OFFSET
    /// 3. 执行数据查询
    /// 4. 计算总页数
    /// 5. 构建并返回 PaginatedResult
    ///
    /// # 类型参数
    ///
    /// - `T`：结果类型，必须实现 `sqlx::FromRow` 和 `Serialize` trait
    ///
    /// # 返回值
    ///
    /// - `Ok(PaginatedResult<T>)`：查询成功，返回分页结果
    /// - `Err(BaseError)`：查询失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseNotInitialized`：数据库未初始化
    /// - `BaseError::DatabaseQueryFailed`：查询执行失败
    ///
    fn with_effective_pagination(self) -> Result<(Self, usize, usize), BaseError> {
        let page = self.query_params.page.unwrap_or(1);
        let page_size = self
            .query_params
            .page_size
            .unwrap_or(super::query_params::DEFAULT_QUERY_PAGE_SIZE);
        let query = self.page(page, page_size)?;

        Ok((query, page, page_size))
    }

    /// 执行分页查询。
    ///
    /// 未显式设置分页时会使用默认页码和默认每页大小，并确保数据查询带有 `LIMIT/OFFSET`。
    pub(crate) async fn paginate<T>(self) -> Result<crate::table::PaginatedResult<T>, BaseError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin + Serialize,
    {
        // 1. 检查数据库连接池是否存在
        let _pool = self
            .pool
            .as_ref()
            .ok_or(BaseError::DatabaseNotInitialized)?;

        // 2. 获取分页参数，如果未设置则使用默认值，并写回数据查询
        let (query, page, page_size) = self.with_effective_pagination()?;

        // 3. 执行 COUNT(*) 查询获取总记录数
        let total = query.count_internal().await?;

        // 4. 如果总记录数为 0，直接返回空结果
        if total == 0 {
            return Ok(crate::table::PaginatedResult::empty(page, page_size));
        }

        // 5. 执行数据查询
        let data = query.select().await?;

        // 6. 构建并返回 PaginatedResult
        Ok(crate::table::PaginatedResult::new(
            data, total, page, page_size,
        ))
    }

    /// 执行分页查询并返回 schema-first [`Record`](crate::table::Record)。
    pub async fn paginate_records(
        self,
    ) -> Result<crate::table::PaginatedResult<crate::table::Record>, BaseError> {
        self.paginate::<crate::table::Record>().await
    }

    /// 执行 COUNT 查询获取总记录数（内部方法，供 paginate 使用）
    ///
    /// 构建 COUNT(*) SQL 语句，应用已配置的 WHERE 条件，执行查询并返回总记录数。
    /// 返回 `usize` 以与 `PaginatedResult::new` 的 `total: usize` 参数直接匹配；
    /// 公开接口 `count()` 通过 `as u64` 适配。
    ///
    /// # 返回值
    ///
    /// - `Ok(usize)`：查询成功，返回总记录数
    /// - `Err(BaseError)`：查询失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseNotInitialized`：数据库未初始化
    /// - `BaseError::DatabaseQueryFailed`：查询执行失败
    async fn count_internal(&self) -> Result<usize, BaseError> {
        // COUNT 会泄露表基数，因此与 SELECT 使用相同的可读权限守卫。
        self.ensure_readable_projection()?;
        // 分页只约束当前页数据，不应改变总记录数。尤其是 OFFSET > 0 时，
        // MySQL 会把 COUNT(*) 的唯一结果行跳过，进而把非空结果误判为 0。
        let mut count_query = self.clone();
        count_query.query_params.page = None;
        count_query.query_params.page_size = None;
        let count = count_query
            .compile_db_query()?
            .count()
            .await
            .map_err(BaseError::DatabaseQueryFailed)?;
        usize::try_from(count).map_err(|_| {
            BaseError::DatabaseQueryFailed(yang_db::DbError::QueryError(
                "COUNT 结果超出 usize 范围".to_string(),
            ))
        })
    }

    /// 执行 COUNT 查询获取总记录数
    ///
    /// 构建 COUNT(*) SQL 语句，应用已配置的 WHERE 条件，执行查询并返回总记录数。
    ///
    /// # 返回值
    ///
    /// - `Ok(u64)`：查询成功，返回总记录数
    /// - `Err(BaseError)`：查询失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseNotInitialized`：数据库未初始化
    /// - `BaseError::DatabaseQueryFailed`：查询执行失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let total = query
    ///     .where_eq("status", serde_json::json!("active"))?
    ///     .count()
    ///     .await?;
    ///
    /// println!("总记录数: {}", total);
    /// ```
    pub async fn count(self) -> Result<u64, BaseError> {
        self.count_internal().await.map(|n| n as u64)
    }

    /// 统一拼接 WHERE 子句到 SQL 字符串
    ///
    /// 集中处理 12 种叶子条件（Eq/Ne/In/NotIn/Like/Gt/Gte/Lt/Lte/Between/IsNull/IsNotNull）
    /// 与两种逻辑组（And/Or）的 SQL 拼接：顶层 `where_conditions` 列表仍以隐式 AND 连接
    /// （向后兼容），逻辑组递归渲染并用括号包裹。所有字段名通过 `quote_identifier`
    /// 反引号转义，防止 SQL 注入。
    ///
    /// # 参数
    ///
    /// - `sql`：目标 SQL 字符串（追加模式）
    /// - `params`：参数列表（追加模式）
    /// - `apply_soft_delete`：是否追加软删过滤（仅读路径）
    ///
    /// # 返回值
    ///
    /// - `Ok(())`：拼接成功
    /// - `Err(BaseError)`：字段名非法、参数转换失败或嵌套层数超限
    #[cfg(test)]
    fn append_where_to_sql(
        &self,
        sql: &mut String,
        params: &mut Vec<SqlParam>,
        apply_soft_delete: bool,
    ) -> Result<(), BaseError> {
        // 计算是否需要追加软删过滤子句（仅读路径，且未显式 with_trashed）
        let has_soft_delete = apply_soft_delete
            && !self.include_trashed
            && self.table_config.soft_delete_field.is_some();

        // 既无 WHERE 条件也无需软删过滤，直接返回
        if self.query_params.where_conditions.is_empty() && !has_soft_delete {
            return Ok(());
        }

        sql.push_str(" WHERE ");
        let mut first = true;

        // 顶层条件以隐式 AND 连接；每个条件递归渲染（组节点自带括号）
        for condition in &self.query_params.where_conditions {
            if !first {
                sql.push_str(" AND ");
            }
            first = false;
            self.render_condition(condition, sql, params, 0)?;
        }

        // 读路径软删过滤：追加 `软删字段 IS NULL`（与已有条件用 AND 连接）
        if has_soft_delete {
            if let Some(field) = &self.table_config.soft_delete_field {
                let quoted = self.quote_identifier(field)?;
                if !first {
                    sql.push_str(" AND ");
                }
                sql.push_str(&format!("{} IS NULL", quoted));
            }
        }

        Ok(())
    }

    /// 递归渲染单个 WHERE 条件到 SQL（叶子直接拼接，And/Or 组括号包裹后递归）。
    ///
    /// `depth` 从 0 起递增，超过 [`Self::MAX_WHERE_DEPTH`] 返回 `ParamInvalid`
    /// 而非 panic，保证受保护层不因深嵌套输入崩溃。
    #[cfg(test)]
    fn render_condition(
        &self,
        condition: &WhereCondition,
        sql: &mut String,
        params: &mut Vec<SqlParam>,
        depth: usize,
    ) -> Result<(), BaseError> {
        if depth > Self::MAX_WHERE_DEPTH {
            return Err(BaseError::ParamInvalid(
                "where".to_string(),
                format!("嵌套布尔条件层数超过上限 {}", Self::MAX_WHERE_DEPTH),
            ));
        }

        match condition {
            WhereCondition::Eq { field, value } => {
                let quoted = self.quote_identifier(field)?;
                if value.is_null() {
                    sql.push_str(&format!("{} IS NULL", quoted));
                } else {
                    sql.push_str(&format!("{} = ?", quoted));
                    params.push(SqlParam::from_json(value)?);
                }
            }
            WhereCondition::In { field, values } => {
                // QRY-2 安全网：渲染期再次校验 IN 列表元素数上限
                if values.len() > Self::MAX_IN_LIST_SIZE {
                    return Err(BaseError::ParamInvalid(
                        "values".to_string(),
                        format!(
                            "IN 列表元素数 {} 超过上限 {}",
                            values.len(),
                            Self::MAX_IN_LIST_SIZE
                        ),
                    ));
                }
                let quoted = self.quote_identifier(field)?;
                if values.is_empty() {
                    // 空 IN 集合：等价于恒假，避免拼出非法的 `IN ()`
                    sql.push_str("1=0");
                } else {
                    let placeholders = vec!["?"; values.len()].join(", ");
                    sql.push_str(&format!("{} IN ({})", quoted, placeholders));
                    for value in values {
                        params.push(SqlParam::from_json(value)?);
                    }
                }
            }
            WhereCondition::Like { field, pattern } => {
                // QRY-1 安全网：渲染期再次校验 LIKE pattern 长度上限
                if pattern.len() > Self::MAX_LIKE_PATTERN_LEN {
                    return Err(BaseError::ParamInvalid(
                        "pattern".to_string(),
                        format!(
                            "LIKE pattern 长度 {} 超过上限 {}",
                            pattern.len(),
                            Self::MAX_LIKE_PATTERN_LEN
                        ),
                    ));
                }
                let quoted = self.quote_identifier(field)?;
                sql.push_str(&format!("{} LIKE ?", quoted));
                params.push(SqlParam::String(pattern.clone()));
            }
            WhereCondition::Gt { field, value } => {
                let quoted = self.quote_identifier(field)?;
                sql.push_str(&format!("{} > ?", quoted));
                params.push(SqlParam::from_json(value)?);
            }
            WhereCondition::Gte { field, value } => {
                let quoted = self.quote_identifier(field)?;
                sql.push_str(&format!("{} >= ?", quoted));
                params.push(SqlParam::from_json(value)?);
            }
            WhereCondition::Lt { field, value } => {
                let quoted = self.quote_identifier(field)?;
                sql.push_str(&format!("{} < ?", quoted));
                params.push(SqlParam::from_json(value)?);
            }
            WhereCondition::Lte { field, value } => {
                let quoted = self.quote_identifier(field)?;
                sql.push_str(&format!("{} <= ?", quoted));
                params.push(SqlParam::from_json(value)?);
            }
            WhereCondition::IsNull { field } => {
                let quoted = self.quote_identifier(field)?;
                sql.push_str(&format!("{} IS NULL", quoted));
            }
            WhereCondition::IsNotNull { field } => {
                let quoted = self.quote_identifier(field)?;
                sql.push_str(&format!("{} IS NOT NULL", quoted));
            }
            WhereCondition::Ne { field, value } => {
                let quoted = self.quote_identifier(field)?;
                if value.is_null() {
                    sql.push_str(&format!("{} IS NOT NULL", quoted));
                } else {
                    sql.push_str(&format!("{} <> ?", quoted));
                    params.push(SqlParam::from_json(value)?);
                }
            }
            WhereCondition::Between { field, lo, hi } => {
                let quoted = self.quote_identifier(field)?;
                sql.push_str(&format!("{} BETWEEN ? AND ?", quoted));
                params.push(SqlParam::from_json(lo)?);
                params.push(SqlParam::from_json(hi)?);
            }
            WhereCondition::NotIn { field, values } => {
                // QRY-2 安全网：渲染期再次校验 NOT IN 列表元素数上限
                if values.len() > Self::MAX_IN_LIST_SIZE {
                    return Err(BaseError::ParamInvalid(
                        "values".to_string(),
                        format!(
                            "NOT IN 列表元素数 {} 超过上限 {}",
                            values.len(),
                            Self::MAX_IN_LIST_SIZE
                        ),
                    ));
                }
                let quoted = self.quote_identifier(field)?;
                if values.is_empty() {
                    // 空 NOT IN 集合：等价于恒真，不排除任何行
                    sql.push_str("1=1");
                } else {
                    let placeholders = vec!["?"; values.len()].join(", ");
                    sql.push_str(&format!("{} NOT IN ({})", quoted, placeholders));
                    for value in values {
                        params.push(SqlParam::from_json(value)?);
                    }
                }
            }
            // 逻辑组：括号包裹，子条件以 AND/OR 连接，递归渲染（深度 +1）
            WhereCondition::And { conditions } => {
                self.render_group(conditions, " AND ", "1=1", sql, params, depth)?;
            }
            WhereCondition::Or { conditions } => {
                self.render_group(conditions, " OR ", "1=0", sql, params, depth)?;
            }
        }

        Ok(())
    }

    /// 渲染逻辑组：`(c1 <sep> c2 <sep> ...)`；空组用 `empty_literal` 兜底
    /// （And→`1=1` 恒真，Or→`1=0` 恒假），避免拼出非法空括号。
    #[cfg(test)]
    fn render_group(
        &self,
        conditions: &[WhereCondition],
        separator: &str,
        empty_literal: &str,
        sql: &mut String,
        params: &mut Vec<SqlParam>,
        depth: usize,
    ) -> Result<(), BaseError> {
        if conditions.is_empty() {
            sql.push_str(empty_literal);
            return Ok(());
        }

        sql.push('(');
        let mut first = true;
        for cond in conditions {
            if !first {
                sql.push_str(separator);
            }
            first = false;
            self.render_condition(cond, sql, params, depth + 1)?;
        }
        sql.push(')');
        Ok(())
    }

    /// 执行 SELECT 查询操作
    ///
    /// 使用 sqlx 构建 SELECT 语句，应用已配置的字段选择、WHERE 条件和排序规则，
    /// 执行查询并将结果反序列化为指定的泛型类型 T。
    ///
    /// # 类型参数
    ///
    /// - `T`：结果类型，必须实现 `sqlx::FromRow` trait
    ///
    /// # 返回值
    ///
    /// - `Ok(Vec<T>)`：查询成功，返回结果列表
    /// - `Err(BaseError)`：查询失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseNotInitialized`：数据库未初始化
    /// - `BaseError::DatabaseQueryFailed`：查询执行失败
    ///
    pub(crate) async fn select<T>(self) -> Result<Vec<T>, BaseError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        self.compile_db_query()?
            .select::<T>()
            .await
            .map_err(BaseError::DatabaseQueryFailed)
    }

    /// 查询全部匹配记录。
    pub async fn all(self) -> Result<Vec<crate::table::Record>, BaseError> {
        self.select::<crate::table::Record>().await
    }

    /// 在事务中执行 SELECT 查询并返回多条记录
    ///
    /// 与 [`TableQuery::select`] 完全一致的建句与权限/软删逻辑，但在调用方提供的
    /// `yang_db::Transaction` 内执行，使「读-改-写」场景的读取与后续写入处于同一
    /// 事务、看到一致快照。
    ///
    /// # 参数
    ///
    /// - `tx`：由 [`ActionContext::begin_transaction`](crate::action::ActionContext::begin_transaction)
    ///   或 [`Tools`](crate::tools::Tools) 所有数据库实例创建的活动事务
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseTransactionFailed`：事务已提交/回滚，连接不可用
    /// - `BaseError::DatabaseQueryFailed`：查询执行失败
    pub(crate) async fn select_in_tx<T>(
        self,
        tx: &mut yang_db::Transaction,
    ) -> Result<Vec<T>, BaseError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        let query = tx.table(&self.table_config.table_ref);
        self.apply_db_plan(query)?
            .select::<T>()
            .await
            .map_err(BaseError::DatabaseQueryFailed)
    }

    /// 在事务中查询全部匹配记录。
    pub async fn all_in_tx(
        self,
        tx: &mut yang_db::Transaction,
    ) -> Result<Vec<crate::table::Record>, BaseError> {
        self.select_in_tx::<crate::table::Record>(tx).await
    }

    /// 构建 SELECT SQL 语句
    ///
    /// # 返回值
    ///
    /// 返回 (SQL 语句, 参数列表) 元组
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseQueryFailed`：SQL 构建失败
    #[cfg(test)]
    fn build_select_sql(
        &self,
        hard_limit: Option<usize>,
    ) -> Result<(String, Vec<SqlParam>), BaseError> {
        let mut sql = String::from("SELECT ");
        let mut params = Vec::new();

        // 1. 字段列表（通过 quote_identifier 转义字段名）
        if let Some(fields) = &self.query_params.fields {
            if fields.is_empty() {
                let quoted_fields = self
                    .default_read_fields()?
                    .into_iter()
                    .map(|field| self.quote_identifier(field))
                    .collect::<Result<Vec<_>, _>>()?;
                sql.push_str(&quoted_fields.join(", "));
            } else {
                // 对每个字段名进行反引号转义
                let quoted_fields: Result<Vec<String>, BaseError> = fields
                    .iter()
                    .map(|f| {
                        self.validate_read_field(f)?;
                        self.quote_identifier(f)
                    })
                    .collect();
                sql.push_str(&quoted_fields?.join(", "));
            }
        } else {
            let quoted_fields = self
                .default_read_fields()?
                .into_iter()
                .map(|field| self.quote_identifier(field))
                .collect::<Result<Vec<_>, _>>()?;
            sql.push_str(&quoted_fields.join(", "));
        }

        // 2. FROM 子句（表名走统一转义路径）
        sql.push_str(&format!(" FROM {}", self.quoted_table_name()?));

        // 3. 通过统一方法拼接 WHERE 子句（读路径，应用软删过滤）
        self.append_where_to_sql(&mut sql, &mut params, true)?;

        // 4. ORDER BY 子句：显式 order_by 优先，否则回退到表配置的 default_order
        let order_source = if !self.query_params.order_by.is_empty() {
            Some(&self.query_params.order_by)
        } else if !self.table_config.default_order.is_empty() {
            Some(&self.table_config.default_order)
        } else {
            None
        };
        if let Some(orders) = order_source {
            sql.push_str(" ORDER BY ");
            let order_clauses: Result<Vec<String>, BaseError> = orders
                .iter()
                .map(|(field, order)| {
                    self.validate_order_field(field)?;
                    let direction = match order {
                        SortOrder::Asc => "ASC",
                        SortOrder::Desc => "DESC",
                    };
                    self.quote_identifier(field)
                        .map(|quoted| format!("{} {}", quoted, direction))
                })
                .collect();
            sql.push_str(&order_clauses?.join(", "));
        }

        // 5. LIMIT 和 OFFSET 子句（分页与 hard_limit 互斥，避免拼出两段 LIMIT）
        if let (Some(page), Some(page_size)) = (self.query_params.page, self.query_params.page_size)
        {
            // 使用 saturating_sub 防止 page==0 时 usize 下溢（纵深防御，
            // 主校验在 page() 入口；直接构造 query_params 的调用方也安全）
            let offset = page.saturating_sub(1).saturating_mul(page_size);
            sql.push_str(&format!(" LIMIT {} OFFSET {}", page_size, offset));
        } else if let Some(limit) = hard_limit {
            // 无分页时应用硬上限（如 fetch_optional 仅需 1 行）
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        Ok((sql, params))
    }

    /// 执行查询并返回可选的单条记录
    ///
    /// 执行 SELECT 查询，返回第一条匹配记录，如果没有匹配记录则返回 None。
    /// 通常与 `where_eq` 等条件方法配合使用，用于按主键查询单条记录。
    ///
    /// # 类型参数
    ///
    /// - `T`：结果类型，必须实现 `sqlx::FromRow` trait
    ///
    /// # 返回值
    ///
    /// - `Ok(Some(T))`：找到匹配记录
    /// - `Ok(None)`：没有匹配记录
    /// - `Err(BaseError)`：查询失败
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseNotInitialized`：数据库未初始化
    /// - `BaseError::DatabaseQueryFailed`：查询执行失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::table::Record;
    ///
    /// # async fn example() -> Result<(), yang_base::error::BaseError> {
    /// // 按主键查询单条记录
    /// let row: Option<Record> = query
    ///     .where_eq("id", serde_json::json!(1))?
    ///     .fetch_optional()
    ///     .await?;
    ///
    /// match row {
    ///     Some(r) => println!("找到记录: {:?}", r.columns),
    ///     None => println!("记录不存在"),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub(crate) async fn fetch_optional<T>(self) -> Result<Option<T>, BaseError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        self.compile_db_query()?
            .find::<T>()
            .await
            .map_err(BaseError::DatabaseQueryFailed)
    }

    /// 查询可选单条记录。
    pub async fn optional(self) -> Result<Option<crate::table::Record>, BaseError> {
        self.fetch_optional::<crate::table::Record>().await
    }

    /// 查询单条记录；没有匹配记录时返回 [`BaseError::RecordNotFound`]。
    pub async fn one(self) -> Result<crate::table::Record, BaseError> {
        let table_name = self.table_config.table_name.clone();
        self.optional()
            .await?
            .ok_or_else(|| BaseError::RecordNotFound(format!("表 {table_name} 中没有匹配记录")))
    }

    /// 执行 INSERT 操作
    ///
    /// 插入数据到表中，包括以下步骤：
    /// 1. 按表定义验证所有字段值
    /// 2. 检查用户是否有字段的写入权限
    /// 3. 构建 INSERT SQL 语句
    /// 4. 执行插入操作
    /// 5. 返回影响行数
    ///
    /// # 参数
    ///
    /// - `data`：要插入的 [`crate::table::Record`]
    ///
    /// # 返回值
    ///
    /// - `Ok(u64)`：插入成功，返回影响行数（通常为 1）
    /// - `Err(BaseError)`：插入失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseNotInitialized`：数据库未初始化
    /// - `BaseError::FieldRequired`：必填字段缺失
    /// - `BaseError::FieldPermissionDenied`：用户无字段写入权限
    /// - `BaseError::ValidationFailed`：字段值验证失败
    /// - `BaseError::DatabaseExecuteFailed`：数据库执行失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::table::Record;
    ///
    /// # async fn example() -> Result<(), yang_base::error::BaseError> {
    /// let data = Record::new()
    ///     .set("name", "张三")
    ///     .set("email", "zhangsan@example.com");
    ///
    /// // 执行插入
    /// let affected = query.insert(data).await?;
    /// println!("插入成功，影响行数: {}", affected);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn insert(self, data: crate::table::Record) -> Result<u64, BaseError> {
        // 填充默认值/时间戳并校验（顺序：写权限→填充默认值→必填/类型校验）
        let data = self.prepare_and_validate_insert(data.into_columns())?;
        self.compile_db_query()?
            .insert(&data)
            .await
            .map_err(BaseError::DatabaseExecuteFailed)?;
        Ok(1)
    }

    /// 在事务中执行 INSERT 操作
    ///
    /// 与 [`TableQuery::insert`] 完全一致的写权限校验/默认值填充/时间戳/必填校验
    /// 流程，但在调用方提供的事务内执行，可与其它写操作原子提交。
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseTransactionFailed`：事务已提交/回滚
    /// - 其余同 [`TableQuery::insert`]
    pub async fn insert_in_tx(
        self,
        tx: &mut yang_db::Transaction,
        data: crate::table::Record,
    ) -> Result<u64, BaseError> {
        let data = self.prepare_and_validate_insert(data.into_columns())?;
        let query = tx.table(&self.table_config.table_ref);
        self.apply_db_plan(query)?
            .insert(&data)
            .await
            .map_err(BaseError::DatabaseExecuteFailed)?;
        Ok(1)
    }

    /// 执行 INSERT 操作并返回自增主键
    ///
    /// 与 [`TableQuery::insert`] 完全一致的校验与拼 SQL 流程，但额外返回本次
    /// INSERT 产生的自增主键值（`last_insert_id`），便于调用方拿到新建记录 ID。
    ///
    /// # 参数
    ///
    /// - `data`：要插入的数据
    ///
    /// # 返回值
    ///
    /// - `Ok((affected, id))`：插入成功，返回 (影响行数, 自增主键值)。
    ///   表无自增列时 `id` 为 0。
    /// - `Err(BaseError)`：插入失败
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseNotInitialized`：数据库未初始化
    /// - `BaseError::FieldRequired`：必填字段缺失
    /// - `BaseError::FieldPermissionDenied`：用户无字段写入权限
    /// - `BaseError::DatabaseExecuteFailed`：数据库执行失败
    pub async fn insert_returning_id(
        self,
        data: crate::table::Record,
    ) -> Result<(u64, u64), BaseError> {
        let data = self.prepare_and_validate_insert(data.into_columns())?;
        let id = self
            .compile_db_query()?
            .insert(&data)
            .await
            .map_err(BaseError::DatabaseExecuteFailed)?;
        Ok((1, id))
    }

    /// 在事务中执行 INSERT 并返回自增主键
    ///
    /// 与 [`TableQuery::insert_returning_id`] 一致，但在事务内执行。批量写入或
    /// 「插入父行→用其主键插入子行」等需要拿到新 ID 再继续的原子场景使用。
    pub async fn insert_returning_id_in_tx(
        self,
        tx: &mut yang_db::Transaction,
        data: crate::table::Record,
    ) -> Result<(u64, u64), BaseError> {
        let data = self.prepare_and_validate_insert(data.into_columns())?;
        let query = tx.table(&self.table_config.table_ref);
        let id = self
            .apply_db_plan(query)?
            .insert(&data)
            .await
            .map_err(BaseError::DatabaseExecuteFailed)?;
        Ok((1, id))
    }

    /// 填充默认值/时间戳并验证插入数据
    ///
    /// 处理顺序（修复 required+default 字段被误报 FieldRequired 的问题）：
    /// 1. 字段与写权限校验：显式提交只读字段时拒绝；null 自增主键视为未提供
    /// 2. 规范化数据库生成字段：未提供或为 null 的自增字段交给数据库生成
    /// 3. 填充默认值：data 中缺失且配置了 `default_value` 的字段补默认值
    /// 4. 填充时间戳：`timestamp_fields` 配置且列存在、调用方未提供时，写入当前时间
    /// 5. 必填/类型/验证器校验：在补齐后的数据上执行
    ///
    /// # 参数
    ///
    /// - `data`：调用方提供的原始插入数据
    ///
    /// # 返回值
    ///
    /// - `Ok(HashMap)`：补齐默认值与时间戳后的最终插入数据
    /// - `Err(BaseError)`：权限或校验失败
    fn prepare_and_validate_insert(
        &self,
        data: std::collections::HashMap<String, Value>,
    ) -> Result<std::collections::HashMap<String, Value>, BaseError> {
        let mut prepared = data;

        // tenant key 只能由请求上下文注入，业务输入不得覆盖。
        if let Some((field, value)) = &self.tenant_scope {
            if prepared.contains_key(field) {
                return Err(BaseError::PermissionDenied(format!(
                    "禁止显式写入租户字段: {field}"
                )));
            }
            prepared.insert(field.clone(), value.clone());
        }

        // 1. 校验调用方显式提交的字段和写权限。只有 null 自增主键可视为“未提供”，
        // 其余只读字段即使提交 null 也必须拒绝，避免绕过字段边界并覆盖数据库默认值。
        for (field_name, value) in &prepared {
            let field_config = self.table_config.get_field(field_name).ok_or_else(|| {
                BaseError::FieldNotFound(self.table_config.table_name.clone(), field_name.clone())
            })?;
            let omitted_auto_increment = field_config.auto_increment && value.is_null();
            let injected_tenant = self
                .tenant_scope
                .as_ref()
                .is_some_and(|(tenant_field, _)| tenant_field == field_name);
            if !injected_tenant
                && !omitted_auto_increment
                && !field_config.permissions.can_write(&self.user_roles_set)
            {
                return Err(BaseError::FieldPermissionDenied(
                    self.table_config.table_name.clone(),
                    field_name.clone(),
                    "用户无写入权限".to_string(),
                ));
            }
        }

        // 2. 数据库生成的自增字段未提供或为 null 时，不进入 INSERT 字段列表。
        for (field_name, field_config) in &self.table_config.fields {
            if field_config.auto_increment
                && prepared
                    .get(field_name)
                    .map(serde_json::Value::is_null)
                    .unwrap_or(true)
            {
                prepared.remove(field_name);
            }
        }

        // 3. 填充默认值（仅缺失时；显式 null 仍受 nullable/required 约束）
        for (field_name, field_config) in &self.table_config.fields {
            if field_config.auto_increment {
                continue;
            }
            if let Some(default) = &field_config.default_value {
                if !prepared.contains_key(field_name) {
                    prepared.insert(field_name.clone(), default.clone());
                }
            }
        }

        // 4. 填充创建/更新时间戳（列存在且调用方未提供时）
        if let Some(ts) = &self.table_config.timestamp_fields {
            let now = chrono::Utc::now().timestamp();
            for ts_field in [&ts.created_at, &ts.updated_at].into_iter().flatten() {
                if self.table_config.fields.contains_key(ts_field) {
                    let missing = prepared.get(ts_field).map(|v| v.is_null()).unwrap_or(true);
                    if missing {
                        prepared.insert(ts_field.clone(), Value::Number(now.into()));
                    }
                }
            }
        }

        // 5. 在补齐后的数据上执行必填/类型/验证器校验
        for (field_name, field_config) in &self.table_config.fields {
            if !field_config.permissions.can_write(&self.user_roles_set) {
                continue;
            }
            if field_config.auto_increment && !prepared.contains_key(field_name) {
                continue;
            }
            let value = prepared.get(field_name).unwrap_or(&Value::Null);
            field_config.validate(value)?;
        }

        Ok(prepared)
    }

    /// 构建 INSERT SQL 语句
    ///
    /// # 参数
    ///
    /// - `data`：要插入的数据
    ///
    /// # 返回值
    ///
    /// 返回 (SQL 语句, 参数列表) 元组
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseQueryFailed`：SQL 构建失败
    #[cfg(test)]
    fn build_insert_sql(
        &self,
        data: &std::collections::HashMap<String, Value>,
    ) -> Result<(String, Vec<SqlParam>), BaseError> {
        let mut fields = Vec::with_capacity(data.len());
        let mut placeholders = Vec::with_capacity(data.len());
        let mut params = Vec::with_capacity(data.len());

        // 遍历数据，构建字段列表和参数列表
        for (field_name, value) in data {
            // 检查字段是否存在于表配置中
            if !self.table_config.fields.contains_key(field_name) {
                return Err(BaseError::FieldNotFound(
                    self.table_config.table_name.clone(),
                    field_name.clone(),
                ));
            }

            // 写入权限已在 prepare_and_validate_insert 集中校验，此处不再二次跳过，
            // 保证 data 中所有字段一致入列。

            // 对字段名进行反引号转义，防止 SQL 注入
            let quoted = self.quote_identifier(field_name)?;
            fields.push(quoted);
            placeholders.push("?".to_string());
            params.push(SqlParam::from_json(value)?);
        }

        // 构建 SQL 语句（表名走统一转义路径）
        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            self.quoted_table_name()?,
            fields.join(", "),
            placeholders.join(", ")
        );

        Ok((sql, params))
    }

    /// 执行 UPDATE 操作
    ///
    /// 更新表中的数据，包括以下步骤：
    /// 1. 按表定义验证所有字段值
    /// 2. 检查用户是否有字段的写入权限
    /// 3. 构建 UPDATE SQL 语句
    /// 4. 应用已配置的 WHERE 条件
    /// 5. 执行更新操作
    /// 6. 返回影响行数
    ///
    /// # 参数
    ///
    /// - `data`：要更新的 [`crate::table::Record`]
    ///
    /// # 返回值
    ///
    /// - `Ok(u64)`：更新成功，返回影响行数
    /// - `Err(BaseError)`：更新失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseNotInitialized`：数据库未初始化
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无字段写入权限
    /// - `BaseError::ValidationFailed`：字段值验证失败
    /// - `BaseError::DatabaseExecuteFailed`：数据库执行失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::table::Record;
    /// use serde_json::json;
    ///
    /// # async fn example() -> Result<(), yang_base::error::BaseError> {
    /// let data = Record::new()
    ///     .set("name", "李四")
    ///     .set("email", "lisi@example.com");
    ///
    /// // 执行更新（需要先设置 WHERE 条件）
    /// let affected = query
    ///     .where_eq("id", json!(1))?
    ///     .update(data)
    ///     .await?;
    /// println!("更新成功，影响行数: {}", affected);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update(self, data: crate::table::Record) -> Result<u64, BaseError> {
        let data = self.prepare_update_data(data.into_columns())?;
        self.compile_db_query()?
            .update(&data)
            .await
            .map_err(BaseError::DatabaseExecuteFailed)
    }

    /// 在事务中执行 UPDATE 操作
    ///
    /// 与 [`TableQuery::update`] 一致的字段校验/权限/WHERE 守卫/自动 `updated_at`
    /// 逻辑，但在事务内执行。
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseTransactionFailed`：事务已提交/回滚
    /// - 其余同 [`TableQuery::update`]
    pub async fn update_in_tx(
        self,
        tx: &mut yang_db::Transaction,
        data: crate::table::Record,
    ) -> Result<u64, BaseError> {
        let data = self.prepare_update_data(data.into_columns())?;
        let query = tx.table(&self.table_config.table_ref);
        self.apply_db_plan(query)?
            .update(&data)
            .await
            .map_err(BaseError::DatabaseExecuteFailed)
    }

    fn prepare_update_data(
        &self,
        data: std::collections::HashMap<String, Value>,
    ) -> Result<std::collections::HashMap<String, Value>, BaseError> {
        self.validate_update_data(&data)?;
        self.with_updated_timestamp(data)
    }

    fn with_updated_timestamp(
        &self,
        mut data: std::collections::HashMap<String, Value>,
    ) -> Result<std::collections::HashMap<String, Value>, BaseError> {
        if data.is_empty() {
            return Err(BaseError::ParamInvalid(
                "data".to_string(),
                "无可更新字段".to_string(),
            ));
        }
        if let Some(updated_at) = self
            .table_config
            .timestamp_fields
            .as_ref()
            .and_then(|fields| fields.updated_at.as_ref())
            .filter(|name| self.table_config.fields.contains_key(*name))
        {
            data.insert(
                updated_at.clone(),
                Value::Number(chrono::Utc::now().timestamp().into()),
            );
        }
        Ok(data)
    }

    /// 验证更新数据
    ///
    /// 验证所有要更新的字段值的合法性和用户权限
    ///
    /// # 参数
    ///
    /// - `data`：要更新的数据
    ///
    /// # 返回值
    ///
    /// - `Ok(())`：验证通过
    /// - `Err(BaseError)`：验证失败
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无字段写入权限
    /// - `BaseError::ValidationFailed`：字段值验证失败
    #[cfg(test)]
    pub fn validate_update_data(
        &self,
        data: &std::collections::HashMap<String, Value>,
    ) -> Result<(), BaseError> {
        self.validate_update_data_impl(data)
    }

    /// 验证更新数据（内部实现）
    #[cfg(not(test))]
    fn validate_update_data(
        &self,
        data: &std::collections::HashMap<String, Value>,
    ) -> Result<(), BaseError> {
        self.validate_update_data_impl(data)
    }

    /// 验证更新数据的实际实现
    fn validate_update_data_impl(
        &self,
        data: &std::collections::HashMap<String, Value>,
    ) -> Result<(), BaseError> {
        if data.is_empty() {
            return Err(BaseError::ParamInvalid(
                "data".to_string(),
                "至少需要一个更新字段".to_string(),
            ));
        }
        // 只验证提供的字段（与 INSERT 不同，UPDATE 不需要验证所有字段）
        for (field_name, value) in data {
            if self
                .tenant_scope
                .as_ref()
                .is_some_and(|(tenant_field, _)| tenant_field == field_name)
            {
                return Err(BaseError::PermissionDenied(format!(
                    "禁止修改租户字段: {field_name}"
                )));
            }
            // 1. 检查字段是否存在于表配置中
            let field_config = self.table_config.get_field(field_name).ok_or_else(|| {
                BaseError::FieldNotFound(self.table_config.table_name.clone(), field_name.clone())
            })?;

            // 2. 检查用户是否有写入权限
            if !field_config.permissions.can_write(&self.user_roles_set) {
                return Err(BaseError::FieldPermissionDenied(
                    self.table_config.table_name.clone(),
                    field_name.clone(),
                    "用户无写入权限".to_string(),
                ));
            }

            // 3. 验证显式提供的字段值。部分更新不要求提交其它必填字段，但若本字段
            // 被显式设为 null，仍执行 required 约束。
            field_config.validate(value)?;
        }

        Ok(())
    }

    /// 构建 UPDATE SQL 语句
    ///
    /// # 参数
    ///
    /// - `data`：要更新的数据
    ///
    /// # 返回值
    ///
    /// 返回 (SQL 语句, 参数列表) 元组
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseQueryFailed`：SQL 构建失败
    #[cfg(test)]
    #[allow(private_interfaces)]
    pub fn build_update_sql(
        &self,
        data: &std::collections::HashMap<String, Value>,
    ) -> Result<(String, Vec<SqlParam>), BaseError> {
        self.build_update_sql_impl(data)
    }

    /// 构建 UPDATE SQL 语句的实际实现
    #[cfg(test)]
    fn build_update_sql_impl(
        &self,
        data: &std::collections::HashMap<String, Value>,
    ) -> Result<(String, Vec<SqlParam>), BaseError> {
        // 0. 拒绝空更新：用户未提供任何可更新字段
        if data.is_empty() {
            return Err(BaseError::ParamInvalid(
                "data".to_string(),
                "无可更新字段".to_string(),
            ));
        }

        // 1. 自动刷新 updated_at（若已配置且列存在于表），仅在需要时克隆
        let need_timestamp = self
            .table_config
            .timestamp_fields
            .as_ref()
            .and_then(|ts| ts.updated_at.as_ref())
            .is_some_and(|updated_at| self.table_config.fields.contains_key(updated_at));

        let mut owned_data;
        let working: &std::collections::HashMap<String, Value> = if need_timestamp {
            owned_data = data.clone();
            // SAFETY: need_timestamp 为 true 意味着 timestamp_fields 和 updated_at 一定存在
            let ts = self.table_config.timestamp_fields.as_ref().unwrap();
            let updated_at = ts.updated_at.as_ref().unwrap();
            let now = chrono::Utc::now().timestamp();
            owned_data.insert(updated_at.clone(), Value::Number(now.into()));
            &owned_data
        } else {
            data
        };

        let mut set_clauses = Vec::with_capacity(working.len());
        let mut params = Vec::with_capacity(working.len());

        // 2. 构建 SET 子句（通过 quote_identifier 转义字段名）
        for (field_name, value) in working {
            // 检查字段是否存在于表配置中
            if !self.table_config.fields.contains_key(field_name) {
                return Err(BaseError::FieldNotFound(
                    self.table_config.table_name.clone(),
                    field_name.clone(),
                ));
            }

            // 对字段名进行反引号转义
            let quoted = self.quote_identifier(field_name)?;
            set_clauses.push(format!("{} = ?", quoted));
            params.push(SqlParam::from_json(value)?);
        }

        // 3. 构建基本 UPDATE 语句（表名走统一转义路径）
        let mut sql = format!(
            "UPDATE {} SET {}",
            self.quoted_table_name()?,
            set_clauses.join(", ")
        );

        // 4. WHERE 守卫：无 WHERE 且未显式放行全表，拒绝全表更新
        if self.query_params.where_conditions.is_empty() {
            return Err(BaseError::MissingWhereClause("UPDATE".to_string()));
        }

        // 5. 通过统一方法拼接 WHERE 子句（写路径，不应用软删过滤）
        self.append_where_to_sql(&mut sql, &mut params, false)?;

        Ok((sql, params))
    }

    /// 执行 DELETE 操作
    ///
    /// 删除表中的数据，支持软删除和物理删除两种模式：
    /// 1. 如果配置了软删除字段（soft_delete_field），执行 UPDATE 设置删除标记
    /// 2. 如果未配置软删除字段，执行物理删除
    /// 3. 应用已配置的 WHERE 条件
    /// 4. 返回影响行数
    ///
    /// # 返回值
    ///
    /// - `Ok(u64)`：删除成功，返回影响行数
    /// - `Err(BaseError)`：删除失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseNotInitialized`：数据库未初始化
    /// - `BaseError::DatabaseExecuteFailed`：数据库执行失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use serde_json::json;
    /// use yang_base::table::{Field, Table};
    ///
    /// # async fn example() -> Result<(), yang_base::error::BaseError> {
    /// let users = Table::new("users")
    ///     .fields(vec![
    ///         Field::id("id"),
    ///         Field::string("name", 50).required(),
    ///         Field::soft_delete("deleted_at"),
    ///     ])
    ///     .build()?;
    ///
    /// // 执行软删除（实际上是 UPDATE deleted_at = <timestamp>）
    /// let affected = users
    ///     .bind(pool)
    ///     .query(["admin"])
    ///     .where_eq("id", json!(1))?
    ///     .delete()
    ///     .await?;
    /// println!("删除成功，影响行数: {}", affected);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn delete(self) -> Result<u64, BaseError> {
        if let Some(soft_delete_field) = &self.table_config.soft_delete_field {
            let data = self.with_updated_timestamp(std::collections::HashMap::from([(
                soft_delete_field.clone(),
                Value::Number(chrono::Utc::now().timestamp().into()),
            )]))?;
            return self
                .compile_db_query()?
                .update(&data)
                .await
                .map_err(BaseError::DatabaseExecuteFailed);
        }
        self.compile_db_query()?
            .delete()
            .await
            .map_err(BaseError::DatabaseExecuteFailed)
    }

    /// 在事务中执行 DELETE 操作
    ///
    /// 与 [`TableQuery::delete`] 一致：配置了软删除字段时走 UPDATE 标记（同样在
    /// 事务内），否则物理删除；WHERE 守卫与软删语义完全复用。
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseTransactionFailed`：事务已提交/回滚
    /// - 其余同 [`TableQuery::delete`]
    pub async fn delete_in_tx(self, tx: &mut yang_db::Transaction) -> Result<u64, BaseError> {
        let query = tx.table(&self.table_config.table_ref);
        if let Some(soft_delete_field) = &self.table_config.soft_delete_field {
            let data = self.with_updated_timestamp(std::collections::HashMap::from([(
                soft_delete_field.clone(),
                Value::Number(chrono::Utc::now().timestamp().into()),
            )]))?;
            return self
                .apply_db_plan(query)?
                .update(&data)
                .await
                .map_err(BaseError::DatabaseExecuteFailed);
        }
        self.apply_db_plan(query)?
            .delete()
            .await
            .map_err(BaseError::DatabaseExecuteFailed)
    }

    /// 构建 DELETE SQL 语句
    ///
    /// # 返回值
    ///
    /// 返回 (SQL 语句, 参数列表) 元组
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseQueryFailed`：SQL 构建失败
    #[cfg(test)]
    #[allow(private_interfaces)]
    pub fn build_delete_sql(&self) -> Result<(String, Vec<SqlParam>), BaseError> {
        self.build_delete_sql_impl()
    }

    /// 构建 SELECT SQL 语句（仅供测试使用）
    #[cfg(test)]
    #[allow(private_interfaces)]
    pub fn build_select_sql_for_test(&self) -> Result<(String, Vec<SqlParam>), BaseError> {
        self.build_select_sql(None)
    }

    /// 构建 INSERT SQL 语句（仅供测试使用）
    ///
    /// 先经 `prepare_and_validate_insert` 补默认值/时间戳并校验，再拼 SQL，
    /// 等价于 `insert`/`insert_returning_id` 的建句路径，但无需数据库连接。
    #[cfg(test)]
    #[allow(private_interfaces)]
    pub fn build_insert_sql_for_test(
        &self,
        data: std::collections::HashMap<String, Value>,
    ) -> Result<(String, Vec<SqlParam>), BaseError> {
        let prepared = self.prepare_and_validate_insert(data)?;
        self.build_insert_sql(&prepared)
    }

    /// 构建 DELETE SQL 语句的实际实现
    #[cfg(test)]
    fn build_delete_sql_impl(&self) -> Result<(String, Vec<SqlParam>), BaseError> {
        // 表名走统一转义路径
        let mut sql = format!("DELETE FROM {}", self.quoted_table_name()?);
        let mut params = Vec::new();

        // WHERE 守卫：无 WHERE 且未显式放行全表，拒绝全表物理删除
        if self.query_params.where_conditions.is_empty() {
            return Err(BaseError::MissingWhereClause("DELETE".to_string()));
        }

        // 通过统一方法拼接 WHERE 子句（写路径，不应用软删过滤）
        self.append_where_to_sql(&mut sql, &mut params, false)?;

        Ok((sql, params))
    }

    /// 执行边界计时（慢查询观测，C4）。
    ///
    /// 包裹一次数据库执行 `fut`：`threshold` 为 `None`（默认）时仅一次
    /// `Instant::now()`，无额外分配、无日志；为 `Some(d)` 且实际耗时超过 `d` 时
    /// 发一条 `tracing::warn!`（含表名、操作、耗时毫秒、request_id）。
    ///
    /// 设计为关联函数（不借用 `&self`），以便在消费 `self` 的终端方法里，与
    /// 借用 `self.pool`/`sql`/`params` 的执行 future 共存而不冲突借用检查。
    /// SQL 文本**默认不记**（防泄漏 + 防高基数）。`op` 为静态操作名。
    #[cfg(test)]
    pub(crate) async fn timed<F, R>(
        threshold: Option<std::time::Duration>,
        request_id: Option<crate::action::RequestId>,
        table: &str,
        op: &'static str,
        fut: F,
    ) -> R
    where
        F: std::future::Future<Output = R>,
    {
        // 阈值未配置：直接 await，热路径仅一次分支判断，无 Instant
        let threshold = match threshold {
            Some(t) => t,
            None => return fut.await,
        };
        let start = std::time::Instant::now();
        let result = fut.await;
        let elapsed = start.elapsed();
        if elapsed >= threshold {
            let elapsed_ms = elapsed.as_secs_f64() * 1000.0;
            match request_id {
                Some(rid) => tracing::warn!(
                    table = %table,
                    op,
                    elapsed_ms,
                    request_id = %rid,
                    "慢查询",
                ),
                None => tracing::warn!(table = %table, op, elapsed_ms, "慢查询"),
            }
        }
        result
    }
}

/// SQL 参数类型
///
/// 用于表示 SQL 查询中的参数值
#[cfg(all(test, feature = "mysql"))]
#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
#[allow(dead_code)] // DateTime/Bytes/Json 已声明，待 from_json 外部构造路径落地
pub(crate) enum SqlParam {
    /// 空值
    Null,
    /// 布尔值
    Bool(bool),
    /// 整数
    Int(i64),
    /// 无符号整数（保留超出 i64 范围的 u64 值，避免精度丢失）
    Uint(u64),
    /// 浮点数
    Float(f64),
    /// 字符串
    String(String),
    /// 日期时间（ISO 8601 字符串解析）
    DateTime(chrono::NaiveDateTime),
    /// 二进制数据
    Bytes(Vec<u8>),
    /// JSON 值
    Json(serde_json::Value),
}

#[cfg(all(test, feature = "mysql"))]
impl SqlParam {
    /// 从 JSON 值创建 SQL 参数
    ///
    /// # 参数
    ///
    /// - `value`：JSON 值
    ///
    /// # 返回值
    ///
    /// - `Ok(SqlParam)`：转换成功
    /// - `Err(BaseError)`：转换失败
    fn from_json(value: &Value) -> Result<Self, BaseError> {
        match value {
            Value::Null => Ok(SqlParam::Null),
            Value::Bool(b) => Ok(SqlParam::Bool(*b)),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(SqlParam::Int(i))
                } else if let Some(u) = n.as_u64() {
                    // 超出 i64 范围的正整数，保留为 u64 避免精度丢失
                    Ok(SqlParam::Uint(u))
                } else if let Some(f) = n.as_f64() {
                    Ok(SqlParam::Float(f))
                } else {
                    Err(BaseError::DatabaseQueryFailed(
                        yang_db::DbError::QueryError(format!("不支持的数字类型: {}", n)),
                    ))
                }
            }
            Value::String(s) => {
                // QRY-5: 尝试解析为 DateTime（ISO 8601 格式）
                if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
                    .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S"))
                    .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f"))
                {
                    return Ok(SqlParam::DateTime(dt));
                }
                Ok(SqlParam::String(s.clone()))
            }
            Value::Array(_) => Ok(SqlParam::Json(value.clone())),
            Value::Object(_) => Ok(SqlParam::Json(value.clone())),
        }
    }
}

#[cfg(all(test, feature = "mysql"))]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn test_config(table: crate::table::Table) -> Arc<TableConfig> {
        table.build().expect("测试表定义应有效").shared_config()
    }

    fn test_query() -> TableQuery {
        let config = test_config(
            crate::table::Table::new("users")
                .fields([crate::table::Field::integer("id").required().primary_key()]),
        );
        let roles: Arc<[String]> = Arc::from(Vec::<String>::new());
        TableQuery::new(config, roles, None)
    }

    #[test]
    fn test_insert_omits_database_generated_auto_increment_field() {
        let config = test_config(crate::table::Table::new("accounts").fields([
            crate::table::Field::id("id"),
            crate::table::Field::string("username", 64).required(),
        ]));
        let roles: Arc<[String]> = Arc::from(Vec::<String>::new());
        let query = TableQuery::new(config, roles, None);
        let data = std::collections::HashMap::from([(
            "username".to_string(),
            Value::String("alice".to_string()),
        )]);

        let prepared = query
            .prepare_and_validate_insert(data)
            .expect("数据库生成的自增主键不应要求调用方提供");

        assert!(!prepared.contains_key("id"));

        let data_with_null_id = std::collections::HashMap::from([
            ("id".to_string(), Value::Null),
            ("username".to_string(), Value::String("bob".to_string())),
        ]);
        let prepared = query
            .prepare_and_validate_insert(data_with_null_id)
            .expect("null 自增主键应等价于未提供");

        assert!(!prepared.contains_key("id"));
    }

    #[test]
    fn tenant_scope_is_fail_closed_for_query_and_writes() {
        let config = test_config(
            crate::table::Table::new("tenant_rows").fields([
                crate::table::Field::id("id"),
                crate::table::Field::bigint("org_id")
                    .required()
                    .tenant_key(),
                crate::table::Field::string("name", 64).required(),
            ]),
        );
        let roles: Arc<[String]> = Arc::from(Vec::<String>::new());
        let query = TableQuery::new(config, roles, None)
            .scope_tenant("org_id", serde_json::json!(7))
            .expect("tenant scope 应有效");

        let (sql, params) = query.build_select_sql(None).expect("租户查询 SQL 应可构建");
        assert!(sql.contains("`org_id` = ?"));
        assert!(params.contains(&SqlParam::Int(7)));

        let prepared = query
            .prepare_and_validate_insert(
                [("name".to_string(), serde_json::json!("row"))]
                    .into_iter()
                    .collect(),
            )
            .expect("租户字段应由上下文注入");
        assert_eq!(prepared.get("org_id"), Some(&serde_json::json!(7)));

        let explicit_tenant = query.prepare_and_validate_insert(
            [
                ("name".to_string(), serde_json::json!("row")),
                ("org_id".to_string(), serde_json::json!(9)),
            ]
            .into_iter()
            .collect(),
        );
        assert!(matches!(
            explicit_tenant,
            Err(BaseError::PermissionDenied(_))
        ));
    }

    #[test]
    fn test_insert_rejects_explicit_null_for_non_writable_field() {
        let config = test_config(crate::table::Table::new("accounts").fields([
            crate::table::Field::id("id"),
            crate::table::Field::string("username", 64).required(),
            crate::table::Field::string("internal_note", 255).not_writable(),
        ]));
        let roles: Arc<[String]> = Arc::from(Vec::<String>::new());
        let query = TableQuery::new(config, roles, None);
        let data = std::collections::HashMap::from([
            ("username".to_string(), Value::String("alice".to_string())),
            ("internal_note".to_string(), Value::Null),
        ]);

        let error = query
            .prepare_and_validate_insert(data)
            .expect_err("显式提交只读字段时，即使值为 null 也必须拒绝");

        assert!(matches!(
            error,
            BaseError::FieldPermissionDenied(table, field, _)
                if table == "accounts" && field == "internal_note"
        ));
    }

    #[test]
    fn test_page_rejects_page_size_above_production_limit() {
        let err = test_query()
            .page(1, 101)
            .expect_err("page_size 超过 100 应被拒绝");

        assert!(matches!(err, BaseError::ParamInvalid(field, _) if field == "page_size"));
    }

    #[test]
    fn trusted_prefetch_limit_reaches_sql_without_public_page_cap() {
        let query = test_query()
            .prefetch_limit(10_001)
            .expect("可信树上限应允许 max_nodes + 1 探针");
        let (sql, _) = query.build_select_sql(None).expect("有界预取 SQL 应可构建");

        assert!(sql.ends_with("LIMIT 10001 OFFSET 0"), "实际 SQL: {sql}");
    }

    #[test]
    fn test_default_order_rejects_unsortable_field() {
        let config = test_config(
            crate::table::Table::new("users")
                .fields([
                    crate::table::Field::id("id"),
                    crate::table::Field::integer("secret_rank").not_sortable(),
                ])
                .default_order(crate::table::col("secret_rank").desc()),
        );
        let roles: Arc<[String]> = Arc::from(Vec::<String>::new());
        let query = TableQuery::new(config, roles, None);

        let err = query
            .build_select_sql(None)
            .expect_err("默认排序不应绕过 sortable(false)");

        assert!(
            matches!(err, BaseError::FieldPermissionDenied(table, field, _) if table == "users" && field == "secret_rank")
        );
    }

    #[test]
    fn test_default_projection_excludes_unreadable_and_secret_fields() {
        let config = test_config(
            crate::table::Table::new("users").fields([
                crate::table::Field::integer("id").required().primary_key(),
                crate::table::Field::string("name", 64),
                crate::table::Field::string("restricted", 64).readable_by(["admin"]),
                crate::table::Field::string("password_hash", 255)
                    .secret()
                    .readable_by(["user"]),
            ]),
        );
        let roles: Arc<[String]> = Arc::from(vec!["user".to_string()]);
        let query = TableQuery::new(config, roles, None);

        query
            .ensure_readable_projection()
            .expect("存在可读且非 secret 字段时默认投影应可用");
        let (sql, _) = query
            .build_select_sql(None)
            .expect("默认查询应只投影当前角色可读且非 secret 的字段");

        assert_eq!(sql, "SELECT `id`, `name` FROM `users`");
        assert!(!sql.contains("restricted"));
        assert!(!sql.contains("password_hash"));
    }

    #[test]
    fn test_select_projection_permission_matrix() {
        let all_readable = test_config(crate::table::Table::new("public_rows").fields([
            crate::table::Field::integer("id").required().primary_key(),
            crate::table::Field::string("name", 64),
        ]));
        let roles: Arc<[String]> = Arc::from(vec!["user".to_string()]);
        let (sql, _) = TableQuery::new(all_readable, Arc::clone(&roles), None)
            .build_select_sql(None)
            .expect("全部字段可读时默认投影应成功");
        assert_eq!(sql, "SELECT `id`, `name` FROM `public_rows`");

        let partially_readable = test_config(
            crate::table::Table::new("mixed_rows").fields([
                crate::table::Field::integer("id").required().primary_key(),
                crate::table::Field::string("restricted", 64).readable_by(["admin"]),
                crate::table::Field::string("password_hash", 255)
                    .secret()
                    .readable_by(["user"]),
            ]),
        );
        let (partial_sql, _) =
            TableQuery::new(Arc::clone(&partially_readable), Arc::clone(&roles), None)
                .build_select_sql(None)
                .expect("部分字段受限时默认投影应保留可读字段");
        assert_eq!(partial_sql, "SELECT `id` FROM `mixed_rows`");

        let explicit_err = TableQuery::new(partially_readable, Arc::clone(&roles), None)
            .select_fields(&["id", "restricted"])
            .expect_err("显式请求受限字段应被拒绝");
        assert!(
            matches!(explicit_err, BaseError::FieldPermissionDenied(table, field, _) if table == "mixed_rows" && field == "restricted")
        );

        let none_readable = test_config(
            crate::table::Table::new("private_rows").fields([
                crate::table::Field::integer("alpha")
                    .required()
                    .primary_key()
                    .readable_by(["admin"]),
                crate::table::Field::integer("beta").readable_by(["admin"]),
            ]),
        );
        let none_err = TableQuery::new(none_readable, roles, None)
            .build_select_sql(None)
            .expect_err("零字段可读时默认投影应 fail-closed");
        assert!(matches!(
            none_err,
            BaseError::FieldPermissionDenied(table, field, _) if table == "private_rows" && field == "*"
        ));
    }

    #[test]
    fn test_default_projection_is_deterministic_and_excludes_hidden_fields() {
        for _ in 0..64 {
            let config = test_config(
                crate::table::Table::new("secrets").fields([
                    crate::table::Field::id("id"),
                    crate::table::Field::integer("z_secret").readable_by(["admin"]),
                    crate::table::Field::integer("a_secret").readable_by(["admin"]),
                    crate::table::Field::integer("m_secret")
                        .secret()
                        .readable_by(["user"]),
                ]),
            );
            let roles: Arc<[String]> = Arc::from(vec!["user".to_string()]);
            let query = TableQuery::new(config, roles, None);

            let (sql, _) = query
                .build_select_sql(None)
                .expect("默认投影应保留公开字段");
            assert_eq!(sql, "SELECT `id` FROM `secrets`");
        }
    }

    #[test]
    fn test_effective_pagination_applies_default_limit_to_data_query_sql() {
        let (query, page, page_size) = test_query()
            .with_effective_pagination()
            .expect("默认分页参数应合法");

        assert_eq!(page, 1);
        assert_eq!(
            page_size,
            crate::table::query_params::DEFAULT_QUERY_PAGE_SIZE
        );

        let (sql, _) = query
            .build_select_sql(None)
            .expect("默认分页后的 SELECT SQL 应可构建");

        assert!(
            sql.contains(&format!(
                " LIMIT {} OFFSET 0",
                crate::table::query_params::DEFAULT_QUERY_PAGE_SIZE
            )),
            "分页数据查询必须包含默认 LIMIT，实际 SQL: {sql}"
        );
    }
}
