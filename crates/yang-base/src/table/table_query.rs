//! 表查询构建器
//!
//! 提供基于 TableConfig 的类型安全查询构建器，支持字段权限验证和 CRUD 操作。
//!
//! # 主要组件
//!
//! - `TableQuery`：查询构建器，支持链式调用
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::table::{TableQuery, TableConfig, FieldConfig, FieldType};
//! use std::sync::Arc;
//!
//! // 创建表配置
//! let table_config = Arc::new(
//!     TableConfig::new("users")
//!         .field(FieldConfig::new("id", FieldType::BigInt)).expect("有效字段配置应注册成功")
//!         .field(FieldConfig::new("name", FieldType::String { max_length: 50 })).expect("有效字段配置应注册成功")
//!         .field(FieldConfig::new("email", FieldType::String { max_length: 100 })).expect("有效字段配置应注册成功")
//! );
//!
//! // 创建查询构建器
//! let query = TableQuery::new(
//!     table_config,
//!     vec!["user".to_string()],
//!     pool,
//! );
//!
//! // 链式调用构建查询
//! let result = query
//!     .select_fields(&["id", "name", "email"])?
//!     .where_eq("status", json!("active"))?
//!     .order_by("created_at", SortOrder::Desc)?
//!     .page(1, 20)?
//!     .execute()
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
/// 基于 TableConfig 创建类型安全的查询构建器，支持：
/// - 字段选择和权限验证
/// - WHERE 条件构建
/// - 排序规则
/// - 分页查询
///
/// # 字段
///
/// - `table_config`：表配置引用，包含字段定义和权限配置
/// - `user_roles`：用户角色列表，用于权限检查
/// - `query_params`：查询参数，包含字段选择、WHERE 条件、排序规则和分页参数
/// - `pool`：数据库连接池引用（预留，暂未使用）
///
/// # 示例
///
/// ```rust,ignore
/// use yang_base::table::{TableQuery, TableConfig, FieldConfig, FieldType};
/// use std::sync::Arc;
/// use serde_json::json;
///
/// let table_config = Arc::new(
///     TableConfig::new("users")
///         .field(FieldConfig::new("id", FieldType::BigInt)).expect("有效字段配置应注册成功")
///         .field(FieldConfig::new("name", FieldType::String { max_length: 50 })).expect("有效字段配置应注册成功")
/// );
///
/// let query = TableQuery::new(
///     table_config,
///     vec!["admin".to_string()],
///     pool,
/// );
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

    /// 是否允许无 WHERE 的全表 UPDATE/DELETE
    ///
    /// 默认 `false`：无 WHERE 的更新/删除会返回 [`BaseError::MissingWhereClause`]，
    /// 与 yang-db 的安全网对齐。置为 `true`（经 [`TableQuery::allow_full_table`]）
    /// 时显式放行全表操作。
    allow_full_table: bool,

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

/// 检查字符串是否为合法的 SQL 标识符
///
/// 合法标识符规则：
/// - 首字符必须是 ASCII 字母或下划线
/// - 后续字符必须是 ASCII 字母、数字或下划线
/// - 不能为空
/// - 不能包含分号、`--`、空白字符
#[cfg(feature = "mysql")]
fn is_valid_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

impl TableQuery {
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
    fn quote_identifier(&self, field: &str) -> Result<String, BaseError> {
        if !is_valid_identifier(field) {
            return Err(BaseError::FieldNotFound(
                self.table_config.table_name.clone(),
                field.to_string(),
            ));
        }
        // 反引号转义：内部反引号变双反引号
        Ok(format!("`{}`", field.replace('`', "``")))
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
    fn quoted_table_name(&self) -> Result<String, BaseError> {
        let name = &self.table_config.table_name;
        if !is_valid_identifier(name) {
            return Err(BaseError::FieldNotFound(
                name.clone(),
                "非法表名".to_string(),
            ));
        }
        Ok(format!("`{}`", name.replace('`', "``")))
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
    pub fn new(
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
            allow_full_table: false,
            pool,
            slow_threshold: None,
            request_id: None,
        }
    }

    /// 创建新的查询构建器（无数据库连接池）
    ///
    /// 当未启用 `mysql` feature 时使用此方法。
    #[cfg(not(feature = "mysql"))]
    pub fn new(
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
            allow_full_table: false,
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

    /// 选择查询字段
    ///
    /// 设置要查询的字段列表，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的读取权限
    ///
    /// # 参数
    ///
    /// - `fields`：字段名列表
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无读取权限
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::{TableQuery, TableConfig, FieldConfig, FieldType};
    /// use std::sync::Arc;
    ///
    /// // 创建表配置
    /// let table_config = Arc::new(
    ///     TableConfig::new("users")
    ///         .field(FieldConfig::new("id", FieldType::BigInt)).expect("有效字段配置应注册成功")
    ///         .field(FieldConfig::new("name", FieldType::String { max_length: 50 })).expect("有效字段配置应注册成功")
    ///         .field(FieldConfig::new("email", FieldType::String { max_length: 100 })).expect("有效字段配置应注册成功")
    /// );
    ///
    /// // 创建查询构建器（不需要数据库连接）
    /// let query = TableQuery::new(
    ///     table_config,
    ///     Arc::from(vec!["admin".to_string()]),
    ///     None,
    /// );
    ///
    /// // 选择存在的字段，应成功
    /// let result = query.select_fields(&["id", "name", "email"]);
    /// assert!(result.is_ok());
    ///
    /// // 选择不存在的字段，应返回错误
    /// let table_config2 = Arc::new(
    ///     TableConfig::new("users")
    ///         .field(FieldConfig::new("id", FieldType::BigInt)).expect("有效字段配置应注册成功")
    /// );
    /// let query2 = TableQuery::new(
    ///     table_config2,
    ///     Arc::from(vec!["admin".to_string()]),
    ///     None,
    /// );
    /// let result2 = query2.select_fields(&["nonexistent_field"]);
    /// assert!(result2.is_err());
    /// ```
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

    fn validate_all_fields_readable_for_roles(
        &self,
        roles: &HashSet<String>,
    ) -> Result<(), BaseError> {
        if let Some(field_name) = self
            .table_config
            .fields
            .iter()
            .filter_map(|(field_name, field_config)| {
                (!field_config.permissions.can_read(roles)).then_some(field_name)
            })
            .min()
        {
            return Err(BaseError::FieldPermissionDenied(
                self.table_config.table_name.clone(),
                field_name.clone(),
                "用户无读取权限".to_string(),
            ));
        }
        Ok(())
    }

    #[cfg(feature = "mysql")]
    fn validate_all_fields_readable(&self) -> Result<(), BaseError> {
        self.validate_all_fields_readable_for_roles(&self.user_roles_set)
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

    /// 强制校验当前用户对表内所有字段的读权限
    ///
    /// 内置 Get/Select Action 读取整实体（`SELECT *`）时，在执行查询前调用本方法，
    /// 确保不会把用户无权读取的字段一并返回。遍历表配置中的每个字段，对
    /// `readable_roles` 非空且用户不具备任一可读角色的字段返回
    /// [`BaseError::FieldPermissionDenied`]。与 [`TableQuery::select_fields`] 的
    /// `can_read` 判定机制保持一致。
    ///
    /// # 参数
    ///
    /// - `user`：当前用户（其 `roles` 用于权限判定）
    ///
    /// # 返回值
    ///
    /// - `Ok(())`：用户对全部字段可读
    /// - `Err(BaseError::FieldPermissionDenied)`：存在不可读字段
    pub fn ensure_fields_readable(&self, user: &crate::action::User) -> Result<(), BaseError> {
        self.validate_all_fields_readable_for_roles(&user.roles)
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
    pub fn where_eq(mut self, field: &str, value: Value) -> Result<Self, BaseError> {
        // 验证字段和权限
        self.validate_filter_field(field)?;

        // 添加 WHERE 条件
        self.query_params.where_conditions.push(WhereCondition::Eq {
            field: field.to_string(),
            value,
        });

        Ok(self)
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
    pub fn where_in(mut self, field: &str, values: Vec<Value>) -> Result<Self, BaseError> {
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

        // 验证字段和权限
        self.validate_filter_field(field)?;

        // 添加 WHERE 条件
        self.query_params.where_conditions.push(WhereCondition::In {
            field: field.to_string(),
            values,
        });

        Ok(self)
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
    pub fn where_like(mut self, field: &str, pattern: String) -> Result<Self, BaseError> {
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

        // 验证字段和权限
        self.validate_filter_field(field)?;

        // 添加 WHERE 条件
        self.query_params
            .where_conditions
            .push(WhereCondition::Like {
                field: field.to_string(),
                pattern,
            });

        Ok(self)
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
    pub fn where_ne(mut self, field: &str, value: Value) -> Result<Self, BaseError> {
        self.validate_filter_field(field)?;
        self.query_params.where_conditions.push(WhereCondition::Ne {
            field: field.to_string(),
            value,
        });
        Ok(self)
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
    pub fn where_lt(mut self, field: &str, value: Value) -> Result<Self, BaseError> {
        self.validate_filter_field(field)?;
        self.query_params.where_conditions.push(WhereCondition::Lt {
            field: field.to_string(),
            value,
        });
        Ok(self)
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
    pub fn where_lte(mut self, field: &str, value: Value) -> Result<Self, BaseError> {
        self.validate_filter_field(field)?;
        self.query_params
            .where_conditions
            .push(WhereCondition::Lte {
                field: field.to_string(),
                value,
            });
        Ok(self)
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
    pub fn where_gt(mut self, field: &str, value: Value) -> Result<Self, BaseError> {
        self.validate_filter_field(field)?;
        self.query_params.where_conditions.push(WhereCondition::Gt {
            field: field.to_string(),
            value,
        });
        Ok(self)
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
    pub fn where_gte(mut self, field: &str, value: Value) -> Result<Self, BaseError> {
        self.validate_filter_field(field)?;
        self.query_params
            .where_conditions
            .push(WhereCondition::Gte {
                field: field.to_string(),
                value,
            });
        Ok(self)
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
    pub fn where_between(mut self, field: &str, lo: Value, hi: Value) -> Result<Self, BaseError> {
        self.validate_filter_field(field)?;
        self.query_params
            .where_conditions
            .push(WhereCondition::Between {
                field: field.to_string(),
                lo,
                hi,
            });
        Ok(self)
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
    pub fn where_null(mut self, field: &str) -> Result<Self, BaseError> {
        self.validate_filter_field(field)?;
        self.query_params
            .where_conditions
            .push(WhereCondition::IsNull {
                field: field.to_string(),
            });
        Ok(self)
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
    pub fn where_not_null(mut self, field: &str) -> Result<Self, BaseError> {
        self.validate_filter_field(field)?;
        self.query_params
            .where_conditions
            .push(WhereCondition::IsNotNull {
                field: field.to_string(),
            });
        Ok(self)
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
    pub fn where_not_in(mut self, field: &str, values: Vec<Value>) -> Result<Self, BaseError> {
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

        self.validate_filter_field(field)?;
        self.query_params
            .where_conditions
            .push(WhereCondition::NotIn {
                field: field.to_string(),
                values,
            });
        Ok(self)
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

    /// 允许无 WHERE 条件的全表 UPDATE/DELETE
    ///
    /// 默认情况下，未设置任何 WHERE 条件的更新/删除会返回
    /// [`BaseError::MissingWhereClause`] 以防止误操作整表。调用本方法显式放行
    /// 全表操作（如批量初始化、清空表）。
    ///
    /// # 返回值
    ///
    /// 返回 self 支持链式调用
    pub fn allow_full_table(mut self) -> Self {
        self.allow_full_table = true;
        self
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
                Ok(())
            }
        }
    }

    /// 添加一个 OR 逻辑组 (WHERE ... AND (c1 OR c2 OR ...))
    ///
    /// 组内每个子条件递归校验字段存在性与筛选权限；通过后整组以 `Or` 节点追加到
    /// 顶层条件列表，与既有条件以隐式 AND 连接。空组等价于恒假（`1=0`）。
    ///
    /// 子条件可由 [`WhereCondition`] 直接构造，亦可嵌套 `And`/`Or` 组（深度上限
    /// [`Self::MAX_WHERE_DEPTH`]）。
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
    /// 这是类型化布尔树（[`Filter`](crate::table::Filter)）桥接到受保护层的统一入口：
    /// 整棵树先经 [`Self::validate_condition_tree`] 递归校验字段存在性、筛选权限与
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

    /// 获取表配置的引用
    ///
    /// 用于测试或调试，获取表配置
    ///
    /// # 返回值
    ///
    /// 返回表配置的引用
    #[allow(dead_code)]
    pub fn get_table_config(&self) -> &Arc<TableConfig> {
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
    #[cfg(test)]
    pub fn new_without_pool(table_config: Arc<TableConfig>) -> Self {
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
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::table::{TableQuery, TableConfig, FieldConfig, FieldType, PaginatedResult};
    /// use std::sync::Arc;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
    /// struct User {
    ///     id: i64,
    ///     name: String,
    ///     email: String,
    /// }
    ///
    /// # async fn example() -> Result<(), yang_base::error::BaseError> {
    /// let table_config = Arc::new(
    ///     TableConfig::new("users")
    ///         .field(FieldConfig::new("id", FieldType::BigInt)).expect("有效字段配置应注册成功")
    ///         .field(FieldConfig::new("name", FieldType::String { max_length: 50 })).expect("有效字段配置应注册成功")
    ///         .field(FieldConfig::new("email", FieldType::String { max_length: 100 })).expect("有效字段配置应注册成功")
    /// );
    ///
    /// let query = TableQuery::new(
    ///     table_config,
    ///     vec!["user".to_string()],
    ///     Some(pool),
    /// );
    ///
    /// // 执行分页查询
    /// let result: PaginatedResult<User> = query
    ///     .select_fields(&["id", "name", "email"])?
    ///     .where_eq("status", serde_json::json!("active"))?
    ///     .order_by("created_at", SortOrder::Desc)?
    ///     .page(1, 20)?
    ///     .paginate()
    ///     .await?;
    ///
    /// println!("总记录数: {}", result.total);
    /// println!("当前页: {}/{}", result.page, result.total_pages);
    /// println!("数据条数: {}", result.data.len());
    /// # Ok(())
    /// # }
    /// ```
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
    pub async fn paginate<T>(self) -> Result<crate::table::PaginatedResult<T>, BaseError>
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
        // 1. 检查数据库连接池是否存在
        let pool = self
            .pool
            .as_ref()
            .ok_or(BaseError::DatabaseNotInitialized)?;

        // 2. 构建 COUNT SQL 语句
        let (sql, params) = self.build_count_sql()?;

        // 3. 创建查询
        let mut query = sqlx::query_scalar::<_, i64>(&sql);

        // 4. 绑定参数
        for param in params {
            query = Self::bind_count_param(query, &param);
        }

        // 5. 执行查询（计时观测）
        let count = Self::timed(
            self.slow_threshold,
            self.request_id,
            &self.table_config.table_name,
            "count",
            query.fetch_one(pool.as_ref()),
        )
        .await
        .map_err(|e| BaseError::DatabaseQueryFailed(yang_db::DbError::from(e)))?;

        Ok(count as usize)
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
                sql.push_str(&format!("{} = ?", quoted));
                params.push(SqlParam::from_json(value)?);
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
                sql.push_str(&format!("{} <> ?", quoted));
                params.push(SqlParam::from_json(value)?);
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

    /// 构建 COUNT SQL 语句
    ///
    /// # 返回值
    ///
    /// 返回 (SQL 语句, 参数列表) 元组
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseQueryFailed`：SQL 构建失败
    fn build_count_sql(&self) -> Result<(String, Vec<SqlParam>), BaseError> {
        let mut sql = format!("SELECT COUNT(*) FROM {}", self.quoted_table_name()?);
        let mut params = Vec::new();

        // 通过统一方法拼接 WHERE 子句（读路径，应用软删过滤）
        self.append_where_to_sql(&mut sql, &mut params, true)?;

        Ok((sql, params))
    }

    /// 绑定参数到 COUNT 查询
    ///
    /// # 参数
    ///
    /// - `query`：sqlx 查询对象
    /// - `param`：SQL 参数值
    ///
    /// # 返回值
    ///
    /// 绑定参数后的查询对象
    fn bind_count_param<'q>(
        query: sqlx::query::QueryScalar<'q, sqlx::MySql, i64, sqlx::mysql::MySqlArguments>,
        param: &SqlParam,
    ) -> sqlx::query::QueryScalar<'q, sqlx::MySql, i64, sqlx::mysql::MySqlArguments> {
        match param {
            SqlParam::Null => query.bind(Option::<i32>::None),
            SqlParam::Bool(b) => query.bind(*b),
            SqlParam::Int(i) => query.bind(*i),
            SqlParam::Uint(u) => query.bind(*u),
            SqlParam::Float(f) => query.bind(*f),
            SqlParam::String(s) => query.bind(s.clone()),
            SqlParam::DateTime(dt) => query.bind(*dt),
            SqlParam::Bytes(b) => query.bind(b.clone()),
            SqlParam::Json(j) => query.bind(j.clone()),
        }
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
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::table::{TableQuery, TableConfig, FieldConfig, FieldType};
    /// use std::sync::Arc;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
    /// struct User {
    ///     id: i64,
    ///     name: String,
    ///     email: String,
    /// }
    ///
    /// # async fn example() -> Result<(), yang_base::error::BaseError> {
    /// let table_config = Arc::new(
    ///     TableConfig::new("users")
    ///         .field(FieldConfig::new("id", FieldType::BigInt)).expect("有效字段配置应注册成功")
    ///         .field(FieldConfig::new("name", FieldType::String { max_length: 50 })).expect("有效字段配置应注册成功")
    ///         .field(FieldConfig::new("email", FieldType::String { max_length: 100 })).expect("有效字段配置应注册成功")
    /// );
    ///
    /// let query = TableQuery::new(
    ///     table_config,
    ///     vec!["user".to_string()],
    ///     Some(pool),
    /// );
    ///
    /// // 执行查询
    /// let users: Vec<User> = query
    ///     .select_fields(&["id", "name", "email"])?
    ///     .where_eq("status", serde_json::json!("active"))?
    ///     .order_by("created_at", SortOrder::Desc)?
    ///     .select()
    ///     .await?;
    ///
    /// println!("找到 {} 个用户", users.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn select<T>(self) -> Result<Vec<T>, BaseError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        // 1. 检查数据库连接池是否存在
        let pool = self
            .pool
            .as_ref()
            .ok_or(BaseError::DatabaseNotInitialized)?;

        // 2. 构建 SQL 语句
        let (sql, params) = self.build_select_sql(None)?;

        // 3. 在连接池上执行查询（计时观测，慢查询超阈值 warn）
        Self::timed(
            self.slow_threshold,
            self.request_id,
            &self.table_config.table_name,
            "select",
            Self::run_fetch_all(pool.as_ref(), &sql, &params),
        )
        .await
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
    ///   或 [`GlobalDatabase::transaction`](crate::database::GlobalDatabase::transaction) 取得的活动事务
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseTransactionFailed`：事务已提交/回滚，连接不可用
    /// - `BaseError::DatabaseQueryFailed`：查询执行失败
    pub async fn select_in_tx<T>(self, tx: &mut yang_db::Transaction) -> Result<Vec<T>, BaseError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        // 构建 SQL（与普通路径一致，含字段权限/软删过滤）
        let (sql, params) = self.build_select_sql(None)?;

        // 借出事务底层连接执行；事务已结束则返回错误而非 panic
        let executor = tx.executor().ok_or_else(|| {
            BaseError::DatabaseTransactionFailed(yang_db::DbError::TransactionError(
                "事务已提交或回滚".to_string(),
            ))
        })?;

        Self::timed(
            self.slow_threshold,
            self.request_id,
            &self.table_config.table_name,
            "select_in_tx",
            Self::run_fetch_all(executor, &sql, &params),
        )
        .await
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
    fn build_select_sql(
        &self,
        hard_limit: Option<usize>,
    ) -> Result<(String, Vec<SqlParam>), BaseError> {
        let mut sql = String::from("SELECT ");
        let mut params = Vec::new();

        // 1. 字段列表（通过 quote_identifier 转义字段名）
        if let Some(fields) = &self.query_params.fields {
            if fields.is_empty() {
                self.validate_all_fields_readable()?;
                sql.push('*');
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
            self.validate_all_fields_readable()?;
            sql.push('*');
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

    /// 绑定参数到查询
    ///
    /// # 参数
    ///
    /// - `query`：sqlx 查询对象
    /// - `param`：SQL 参数值
    ///
    /// # 返回值
    ///
    /// 绑定参数后的查询对象
    fn bind_param<'q, T>(
        query: sqlx::query::QueryAs<'q, sqlx::MySql, T, sqlx::mysql::MySqlArguments>,
        param: &SqlParam,
    ) -> sqlx::query::QueryAs<'q, sqlx::MySql, T, sqlx::mysql::MySqlArguments>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        match param {
            SqlParam::Null => query.bind(Option::<i32>::None),
            SqlParam::Bool(b) => query.bind(*b),
            SqlParam::Int(i) => query.bind(*i),
            SqlParam::Uint(u) => query.bind(*u),
            SqlParam::Float(f) => query.bind(*f),
            SqlParam::String(s) => query.bind(s.clone()),
            SqlParam::DateTime(dt) => query.bind(*dt),
            SqlParam::Bytes(b) => query.bind(b.clone()),
            SqlParam::Json(j) => query.bind(j.clone()),
        }
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
    /// use yang_base::table::DynamicRow;
    ///
    /// # async fn example() -> Result<(), yang_base::error::BaseError> {
    /// // 按主键查询单条记录
    /// let row: Option<DynamicRow> = query
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
    pub async fn fetch_optional<T>(self) -> Result<Option<T>, BaseError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        // 1. 检查数据库连接池是否存在
        let pool = self
            .pool
            .as_ref()
            .ok_or(BaseError::DatabaseNotInitialized)?;

        // 2. 构建 SQL 语句（限制返回 1 条记录）
        let (sql, params) = self.build_select_sql(Some(1))?;

        // 3. 创建查询
        let mut query = sqlx::query_as::<_, T>(&sql);

        // 4. 绑定参数
        for param in params {
            query = Self::bind_param(query, &param);
        }

        // 5. 执行查询，返回可选结果（计时观测）
        let result = Self::timed(
            self.slow_threshold,
            self.request_id,
            &self.table_config.table_name,
            "fetch_optional",
            query.fetch_optional(pool.as_ref()),
        )
        .await
        .map_err(|e| BaseError::DatabaseQueryFailed(yang_db::DbError::from(e)))?;

        Ok(result)
    }

    /// 执行 INSERT 操作
    ///
    /// 插入数据到表中，包括以下步骤：
    /// 1. 验证所有字段值的合法性（使用 FieldConfig::validate）
    /// 2. 检查用户是否有字段的写入权限
    /// 3. 构建 INSERT SQL 语句
    /// 4. 执行插入操作
    /// 5. 返回影响行数
    ///
    /// # 参数
    ///
    /// - `data`：要插入的数据，格式为 HashMap<String, Value>
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
    /// use yang_base::table::{TableQuery, TableConfig, FieldConfig, FieldType};
    /// use std::sync::Arc;
    /// use std::collections::HashMap;
    /// use serde_json::json;
    ///
    /// # async fn example() -> Result<(), yang_base::error::BaseError> {
    /// let table_config = Arc::new(
    ///     TableConfig::new("users")
    ///         .field(FieldConfig::new("id", FieldType::BigInt)).expect("有效字段配置应注册成功")
    ///         .field(FieldConfig::new("name", FieldType::String { max_length: 50 }).required(true)).expect("有效字段配置应注册成功")
    ///         .field(FieldConfig::new("email", FieldType::String { max_length: 100 })).expect("有效字段配置应注册成功")
    /// );
    ///
    /// let query = TableQuery::new(
    ///     table_config,
    ///     vec!["user".to_string()],
    ///     Some(pool),
    /// );
    ///
    /// // 准备插入数据
    /// let mut data = HashMap::new();
    /// data.insert("name".to_string(), json!("张三"));
    /// data.insert("email".to_string(), json!("zhangsan@example.com"));
    ///
    /// // 执行插入
    /// let affected = query.insert(data).await?;
    /// println!("插入成功，影响行数: {}", affected);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn insert(
        self,
        data: std::collections::HashMap<String, Value>,
    ) -> Result<u64, BaseError> {
        // 1. 检查数据库连接池是否存在
        let pool = self
            .pool
            .as_ref()
            .ok_or(BaseError::DatabaseNotInitialized)?;

        // 2. 填充默认值/时间戳并校验（顺序：写权限→填充默认值→必填/类型校验）
        let data = self.prepare_and_validate_insert(data)?;

        // 3. 构建 INSERT SQL 语句
        let (sql, params) = self.build_insert_sql(&data)?;

        // 4. 在连接池上执行插入（计时观测）
        let result = Self::timed(
            self.slow_threshold,
            self.request_id,
            &self.table_config.table_name,
            "insert",
            Self::run_execute(pool.as_ref(), &sql, &params),
        )
        .await?;

        Ok(result.rows_affected())
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
        data: std::collections::HashMap<String, Value>,
    ) -> Result<u64, BaseError> {
        let data = self.prepare_and_validate_insert(data)?;
        let (sql, params) = self.build_insert_sql(&data)?;

        let executor = tx.executor().ok_or_else(|| {
            BaseError::DatabaseTransactionFailed(yang_db::DbError::TransactionError(
                "事务已提交或回滚".to_string(),
            ))
        })?;

        let result = Self::timed(
            self.slow_threshold,
            self.request_id,
            &self.table_config.table_name,
            "insert_in_tx",
            Self::run_execute(executor, &sql, &params),
        )
        .await?;
        Ok(result.rows_affected())
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
        data: std::collections::HashMap<String, Value>,
    ) -> Result<(u64, u64), BaseError> {
        let pool = self
            .pool
            .as_ref()
            .ok_or(BaseError::DatabaseNotInitialized)?;

        let data = self.prepare_and_validate_insert(data)?;
        let (sql, params) = self.build_insert_sql(&data)?;

        let result = Self::timed(
            self.slow_threshold,
            self.request_id,
            &self.table_config.table_name,
            "insert_returning_id",
            Self::run_execute(pool.as_ref(), &sql, &params),
        )
        .await?;

        Ok((result.rows_affected(), result.last_insert_id()))
    }

    /// 在事务中执行 INSERT 并返回自增主键
    ///
    /// 与 [`TableQuery::insert_returning_id`] 一致，但在事务内执行。批量写入或
    /// 「插入父行→用其主键插入子行」等需要拿到新 ID 再继续的原子场景使用。
    pub async fn insert_returning_id_in_tx(
        self,
        tx: &mut yang_db::Transaction,
        data: std::collections::HashMap<String, Value>,
    ) -> Result<(u64, u64), BaseError> {
        let data = self.prepare_and_validate_insert(data)?;
        let (sql, params) = self.build_insert_sql(&data)?;

        let executor = tx.executor().ok_or_else(|| {
            BaseError::DatabaseTransactionFailed(yang_db::DbError::TransactionError(
                "事务已提交或回滚".to_string(),
            ))
        })?;

        let result = Self::timed(
            self.slow_threshold,
            self.request_id,
            &self.table_config.table_name,
            "insert_returning_id_in_tx",
            Self::run_execute(executor, &sql, &params),
        )
        .await?;
        Ok((result.rows_affected(), result.last_insert_id()))
    }

    /// 填充默认值/时间戳并验证插入数据
    ///
    /// 处理顺序（修复 required+default 字段被误报 FieldRequired 的问题）：
    /// 1. 写权限校验：对调用方显式提供了非 null 值、但用户无写权限的字段拒绝
    /// 2. 填充默认值：data 中缺失或为 null 且配置了 `default_value` 的字段补默认值
    /// 3. 填充时间戳：`timestamp_fields` 配置且列存在、调用方未提供时，写入当前时间
    /// 4. 必填/类型/验证器校验：在补齐后的数据上执行
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

        // 1. 写权限校验：仅拦截"无权限却显式赋了非 null 值"的字段
        for (field_name, field_config) in &self.table_config.fields {
            if !field_config.permissions.can_write(&self.user_roles_set) {
                if let Some(v) = prepared.get(field_name) {
                    if !v.is_null() {
                        return Err(BaseError::FieldPermissionDenied(
                            self.table_config.table_name.clone(),
                            field_name.clone(),
                            "用户无写入权限".to_string(),
                        ));
                    }
                }
            }
        }

        // 2. 填充默认值（缺失或为 null 且配置了 default_value）
        for (field_name, field_config) in &self.table_config.fields {
            if let Some(default) = &field_config.default_value {
                let missing = prepared
                    .get(field_name)
                    .map(|v| v.is_null())
                    .unwrap_or(true);
                if missing {
                    prepared.insert(field_name.clone(), default.clone());
                }
            }
        }

        // 3. 填充创建/更新时间戳（列存在且调用方未提供时）
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

        // 4. 在补齐后的数据上执行必填/类型/验证器校验
        for (field_name, field_config) in &self.table_config.fields {
            if !field_config.permissions.can_write(&self.user_roles_set) {
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

            // 写入权限已在 validate_insert_data 集中校验（无权限且赋非 null 值会
            // 直接报错），此处不再二次跳过，保证 data 中所有字段一致入列。

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
    /// 1. 验证所有字段值的合法性（使用 FieldConfig::validate）
    /// 2. 检查用户是否有字段的写入权限
    /// 3. 构建 UPDATE SQL 语句
    /// 4. 应用已配置的 WHERE 条件
    /// 5. 执行更新操作
    /// 6. 返回影响行数
    ///
    /// # 参数
    ///
    /// - `data`：要更新的数据，格式为 HashMap<String, Value>
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
    /// use yang_base::table::{TableQuery, TableConfig, FieldConfig, FieldType};
    /// use std::sync::Arc;
    /// use std::collections::HashMap;
    /// use serde_json::json;
    ///
    /// # async fn example() -> Result<(), yang_base::error::BaseError> {
    /// let table_config = Arc::new(
    ///     TableConfig::new("users")
    ///         .field(FieldConfig::new("id", FieldType::BigInt)).expect("有效字段配置应注册成功")
    ///         .field(FieldConfig::new("name", FieldType::String { max_length: 50 })).expect("有效字段配置应注册成功")
    ///         .field(FieldConfig::new("email", FieldType::String { max_length: 100 })).expect("有效字段配置应注册成功")
    /// );
    ///
    /// let query = TableQuery::new(
    ///     table_config,
    ///     vec!["user".to_string()],
    ///     Some(pool),
    /// );
    ///
    /// // 准备更新数据
    /// let mut data = HashMap::new();
    /// data.insert("name".to_string(), json!("李四"));
    /// data.insert("email".to_string(), json!("lisi@example.com"));
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
    pub async fn update(
        self,
        data: std::collections::HashMap<String, Value>,
    ) -> Result<u64, BaseError> {
        // 1. 检查数据库连接池是否存在
        let pool = self
            .pool
            .as_ref()
            .ok_or(BaseError::DatabaseNotInitialized)?;

        // 2. 验证所有字段值的合法性和权限
        self.validate_update_data(&data)?;

        // 3. 构建 UPDATE SQL 语句
        let (sql, params) = self.build_update_sql(&data)?;

        // 4. 在连接池上执行更新（计时观测）
        let result = Self::timed(
            self.slow_threshold,
            self.request_id,
            &self.table_config.table_name,
            "update",
            Self::run_execute(pool.as_ref(), &sql, &params),
        )
        .await?;

        Ok(result.rows_affected())
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
        data: std::collections::HashMap<String, Value>,
    ) -> Result<u64, BaseError> {
        self.validate_update_data(&data)?;
        let (sql, params) = self.build_update_sql(&data)?;

        let executor = tx.executor().ok_or_else(|| {
            BaseError::DatabaseTransactionFailed(yang_db::DbError::TransactionError(
                "事务已提交或回滚".to_string(),
            ))
        })?;

        let result = Self::timed(
            self.slow_threshold,
            self.request_id,
            &self.table_config.table_name,
            "update_in_tx",
            Self::run_execute(executor, &sql, &params),
        )
        .await?;
        Ok(result.rows_affected())
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
        // 只验证提供的字段（与 INSERT 不同，UPDATE 不需要验证所有字段）
        for (field_name, value) in data {
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

            // 3. 验证字段值（包括类型检查和验证器检查）
            // 注意：UPDATE 操作中，字段不一定是必填的，因为我们只更新部分字段
            // 所以这里不检查 required 约束，只检查类型和验证器
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

    /// 构建 UPDATE SQL 语句（内部实现）
    #[cfg(not(test))]
    fn build_update_sql(
        &self,
        data: &std::collections::HashMap<String, Value>,
    ) -> Result<(String, Vec<SqlParam>), BaseError> {
        self.build_update_sql_impl(data)
    }

    /// 构建 UPDATE SQL 语句的实际实现
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
        if self.query_params.where_conditions.is_empty() && !self.allow_full_table {
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
    /// use yang_base::table::{TableQuery, TableConfig, FieldConfig, FieldType};
    /// use std::sync::Arc;
    /// use serde_json::json;
    ///
    /// # async fn example() -> Result<(), yang_base::error::BaseError> {
    /// // 配置了软删除字段的表
    /// let table_config = Arc::new(
    ///     TableConfig::new("users")
    ///         .field(FieldConfig::new("id", FieldType::BigInt)).expect("有效字段配置应注册成功")
    ///         .field(FieldConfig::new("name", FieldType::String { max_length: 50 })).expect("有效字段配置应注册成功")
    ///         .field(FieldConfig::new("deleted_at", FieldType::BigInt)).expect("有效字段配置应注册成功")
    ///         .soft_delete_field("deleted_at")  // 配置软删除字段
    /// );
    ///
    /// let query = TableQuery::new(
    ///     table_config,
    ///     vec!["admin".to_string()],
    ///     Some(pool),
    /// );
    ///
    /// // 执行软删除（实际上是 UPDATE deleted_at = <timestamp>）
    /// let affected = query
    ///     .where_eq("id", json!(1))?
    ///     .delete()
    ///     .await?;
    /// println!("删除成功，影响行数: {}", affected);
    ///
    /// // 未配置软删除字段的表将执行物理删除
    /// let table_config2 = Arc::new(
    ///     TableConfig::new("logs")
    ///         .field(FieldConfig::new("id", FieldType::BigInt)).expect("有效字段配置应注册成功")
    ///         .field(FieldConfig::new("message", FieldType::Text)).expect("有效字段配置应注册成功")
    ///         // 未配置 soft_delete_field
    /// );
    ///
    /// let query2 = TableQuery::new(
    ///     table_config2,
    ///     vec!["admin".to_string()],
    ///     Some(pool),
    /// );
    ///
    /// // 执行物理删除（实际上是 DELETE FROM logs WHERE ...）
    /// let affected2 = query2
    ///     .where_eq("id", json!(1))?
    ///     .delete()
    ///     .await?;
    /// println!("物理删除成功，影响行数: {}", affected2);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn delete(self) -> Result<u64, BaseError> {
        // 1. 检查是否配置了软删除字段
        if let Some(soft_delete_field) = &self.table_config.soft_delete_field {
            // 软删除：走 build_update_sql_impl 跳过 validate_update_data_impl
            // 的用户写权限检查，与 updated_at 自动写入语义对称。
            let now = chrono::Utc::now().timestamp();
            let mut data = std::collections::HashMap::new();
            data.insert(soft_delete_field.clone(), Value::Number(now.into()));
            let (sql, params) = self.build_update_sql_impl(&data)?;
            let pool = self
                .pool
                .as_ref()
                .ok_or(BaseError::DatabaseNotInitialized)?;
            let result = Self::timed(
                self.slow_threshold,
                self.request_id,
                &self.table_config.table_name,
                "delete",
                Self::run_execute(pool.as_ref(), &sql, &params),
            )
            .await?;
            return Ok(result.rows_affected());
        }

        // 2. 物理删除：检查连接池
        let pool = self
            .pool
            .as_ref()
            .ok_or(BaseError::DatabaseNotInitialized)?;

        // 3. 物理删除：构建 DELETE SQL 语句
        let (sql, params) = self.build_delete_sql()?;

        // 4. 在连接池上执行删除（计时观测）
        let result = Self::timed(
            self.slow_threshold,
            self.request_id,
            &self.table_config.table_name,
            "delete",
            Self::run_execute(pool.as_ref(), &sql, &params),
        )
        .await?;

        Ok(result.rows_affected())
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
        // 软删除：走 build_update_sql_impl 跳过 validate_update_data_impl
        // 的用户写权限检查，与 updated_at 自动写入语义对称。
        if let Some(soft_delete_field) = &self.table_config.soft_delete_field {
            let now = chrono::Utc::now().timestamp();
            let mut data = std::collections::HashMap::new();
            data.insert(soft_delete_field.clone(), Value::Number(now.into()));
            let (sql, params) = self.build_update_sql_impl(&data)?;
            let executor = tx.executor().ok_or_else(|| {
                BaseError::DatabaseTransactionFailed(yang_db::DbError::TransactionError(
                    "事务已提交或回滚".to_string(),
                ))
            })?;
            let result = Self::timed(
                self.slow_threshold,
                self.request_id,
                &self.table_config.table_name,
                "delete_in_tx",
                Self::run_execute(executor, &sql, &params),
            )
            .await?;
            return Ok(result.rows_affected());
        }

        let (sql, params) = self.build_delete_sql()?;

        let executor = tx.executor().ok_or_else(|| {
            BaseError::DatabaseTransactionFailed(yang_db::DbError::TransactionError(
                "事务已提交或回滚".to_string(),
            ))
        })?;

        let result = Self::timed(
            self.slow_threshold,
            self.request_id,
            &self.table_config.table_name,
            "delete_in_tx",
            Self::run_execute(executor, &sql, &params),
        )
        .await?;
        Ok(result.rows_affected())
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

    /// 构建 DELETE SQL 语句（内部实现）
    #[cfg(not(test))]
    fn build_delete_sql(&self) -> Result<(String, Vec<SqlParam>), BaseError> {
        self.build_delete_sql_impl()
    }

    /// 构建 DELETE SQL 语句的实际实现
    fn build_delete_sql_impl(&self) -> Result<(String, Vec<SqlParam>), BaseError> {
        // 表名走统一转义路径
        let mut sql = format!("DELETE FROM {}", self.quoted_table_name()?);
        let mut params = Vec::new();

        // WHERE 守卫：无 WHERE 且未显式放行全表，拒绝全表物理删除
        if self.query_params.where_conditions.is_empty() && !self.allow_full_table {
            return Err(BaseError::MissingWhereClause("DELETE".to_string()));
        }

        // 通过统一方法拼接 WHERE 子句（写路径，不应用软删过滤）
        self.append_where_to_sql(&mut sql, &mut params, false)?;

        Ok((sql, params))
    }

    /// 绑定参数到执行查询
    ///
    /// # 参数
    ///
    /// - `query`：sqlx 查询对象
    /// - `param`：SQL 参数值
    ///
    /// # 返回值
    ///
    /// 绑定参数后的查询对象
    fn bind_execute_param<'q>(
        query: sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments>,
        param: &SqlParam,
    ) -> sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments> {
        match param {
            SqlParam::Null => query.bind(Option::<i32>::None),
            SqlParam::Bool(b) => query.bind(*b),
            SqlParam::Int(i) => query.bind(*i),
            SqlParam::Uint(u) => query.bind(*u),
            SqlParam::Float(f) => query.bind(*f),
            SqlParam::String(s) => query.bind(s.clone()),
            SqlParam::DateTime(dt) => query.bind(*dt),
            SqlParam::Bytes(b) => query.bind(b.clone()),
            SqlParam::Json(j) => query.bind(j.clone()),
        }
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

    /// 在给定执行器上绑定参数并执行写语句（INSERT/UPDATE/DELETE 共用）
    ///
    /// 执行器既可以是 `&MySqlPool`（普通路径），也可以是
    /// `&mut sqlx::MySqlConnection`（事务路径，经 [`yang_db::Transaction::executor`]
    /// 取得）。如此一来，受保护层的写操作可以直接执行，也可纳入同一事务原子提交，
    /// 而所有权限/校验/软删/WHERE 守卫逻辑在调用方保持不变。
    ///
    /// 返回 `MySqlQueryResult`，由调用方按需提取 `rows_affected()` /
    /// `last_insert_id()`。
    async fn run_execute<'e, E>(
        executor: E,
        sql: &str,
        params: &[SqlParam],
    ) -> Result<sqlx::mysql::MySqlQueryResult, BaseError>
    where
        E: sqlx::Executor<'e, Database = sqlx::MySql>,
    {
        let mut query = sqlx::query(sql);
        for param in params {
            query = Self::bind_execute_param(query, param);
        }
        query
            .execute(executor)
            .await
            .map_err(|e| BaseError::DatabaseExecuteFailed(yang_db::DbError::from(e)))
    }

    /// 在给定执行器上绑定参数并执行 SELECT 查询，返回多行
    ///
    /// 与 [`TableQuery::run_execute`] 同理对执行器泛型，使 `select` 既能走连接池
    /// 也能在事务内执行（read-modify-write 场景）。
    async fn run_fetch_all<'e, E, T>(
        executor: E,
        sql: &str,
        params: &[SqlParam],
    ) -> Result<Vec<T>, BaseError>
    where
        E: sqlx::Executor<'e, Database = sqlx::MySql>,
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        let mut query = sqlx::query_as::<_, T>(sql);
        for param in params {
            query = Self::bind_param(query, param);
        }
        query
            .fetch_all(executor)
            .await
            .map_err(|e| BaseError::DatabaseQueryFailed(yang_db::DbError::from(e)))
    }
}

/// SQL 参数类型
///
/// 用于表示 SQL 查询中的参数值
#[cfg(feature = "mysql")]
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

#[cfg(feature = "mysql")]
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
    use std::collections::HashSet;
    use std::sync::Arc;

    fn test_query() -> TableQuery {
        let config = Arc::new(
            crate::table::TableConfig::new("users")
                .field(crate::table::FieldConfig::new(
                    "id",
                    crate::table::FieldType::Integer,
                ))
                .expect("有效字段配置应注册成功"),
        );
        let roles: Arc<[String]> = Arc::from(Vec::<String>::new());
        TableQuery::new(config, roles, None)
    }

    #[test]
    fn test_page_rejects_page_size_above_production_limit() {
        let err = test_query()
            .page(1, 101)
            .expect_err("page_size 超过 100 应被拒绝");

        assert!(matches!(err, BaseError::ParamInvalid(field, _) if field == "page_size"));
    }

    #[test]
    fn test_default_order_rejects_unsortable_field() {
        let config = Arc::new(
            crate::table::TableConfig::new("users")
                .field(
                    crate::table::FieldConfig::new("secret_rank", crate::table::FieldType::Integer)
                        .sortable(false),
                )
                .expect("有效字段配置应注册成功")
                .default_order(vec![(
                    "secret_rank".to_string(),
                    crate::table::SortOrder::Desc,
                )]),
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
    fn test_select_star_rejects_unreadable_field() {
        let protected_permissions = crate::table::FieldPermissions {
            readable_roles: HashSet::from(["admin".to_string()]),
            ..crate::table::FieldPermissions::default()
        };
        let config = Arc::new(
            crate::table::TableConfig::new("users")
                .field(crate::table::FieldConfig::new(
                    "id",
                    crate::table::FieldType::Integer,
                ))
                .expect("有效字段配置应注册成功")
                .field(
                    crate::table::FieldConfig::new(
                        "secret",
                        crate::table::FieldType::String { max_length: 64 },
                    )
                    .permissions(protected_permissions),
                )
                .expect("有效字段配置应注册成功"),
        );
        let roles: Arc<[String]> = Arc::from(Vec::<String>::new());
        let query = TableQuery::new(config, roles, None);

        let err = query
            .build_select_sql(None)
            .expect_err("SELECT * 不应返回用户无权读取的字段");

        assert!(
            matches!(err, BaseError::FieldPermissionDenied(table, field, _) if table == "users" && field == "secret")
        );
    }

    #[test]
    fn test_select_projection_permission_matrix() {
        let protected = crate::table::FieldPermissions {
            readable_roles: HashSet::from(["admin".to_string()]),
            ..crate::table::FieldPermissions::default()
        };

        let all_readable = Arc::new(
            crate::table::TableConfig::new("public_rows")
                .field(crate::table::FieldConfig::new(
                    "id",
                    crate::table::FieldType::Integer,
                ))
                .expect("公开字段配置应有效")
                .field(crate::table::FieldConfig::new(
                    "name",
                    crate::table::FieldType::String { max_length: 64 },
                ))
                .expect("公开字段配置应有效"),
        );
        let roles: Arc<[String]> = Arc::from(vec!["user".to_string()]);
        let (sql, _) = TableQuery::new(all_readable, Arc::clone(&roles), None)
            .build_select_sql(None)
            .expect("全部字段可读时 SELECT * 应成功");
        assert!(sql.starts_with("SELECT * FROM `public_rows`"));

        let partially_readable = Arc::new(
            crate::table::TableConfig::new("mixed_rows")
                .field(crate::table::FieldConfig::new(
                    "id",
                    crate::table::FieldType::Integer,
                ))
                .expect("公开字段配置应有效")
                .field(
                    crate::table::FieldConfig::new(
                        "secret",
                        crate::table::FieldType::String { max_length: 64 },
                    )
                    .permissions(protected.clone()),
                )
                .expect("受限字段配置应有效"),
        );
        let partial_err =
            TableQuery::new(Arc::clone(&partially_readable), Arc::clone(&roles), None)
                .build_select_sql(None)
                .expect_err("部分字段不可读时 SELECT * 应 fail-closed");
        assert!(
            matches!(partial_err, BaseError::FieldPermissionDenied(table, field, _) if table == "mixed_rows" && field == "secret")
        );

        let explicit_err = TableQuery::new(partially_readable, Arc::clone(&roles), None)
            .select_fields(&["id", "secret"])
            .expect_err("显式请求受限字段应被拒绝");
        assert!(
            matches!(explicit_err, BaseError::FieldPermissionDenied(table, field, _) if table == "mixed_rows" && field == "secret")
        );

        let none_readable = Arc::new(
            crate::table::TableConfig::new("private_rows")
                .field(
                    crate::table::FieldConfig::new("alpha", crate::table::FieldType::Integer)
                        .permissions(protected.clone()),
                )
                .expect("受限字段配置应有效")
                .field(
                    crate::table::FieldConfig::new("beta", crate::table::FieldType::Integer)
                        .permissions(protected),
                )
                .expect("受限字段配置应有效"),
        );
        let none_err = TableQuery::new(none_readable, roles, None)
            .build_select_sql(None)
            .expect_err("零字段可读时 SELECT * 应 fail-closed");
        assert!(matches!(
            none_err,
            BaseError::FieldPermissionDenied(table, _, _) if table == "private_rows"
        ));
    }

    #[test]
    fn test_unreadable_field_errors_are_deterministic() {
        let protected = crate::table::FieldPermissions {
            readable_roles: HashSet::from(["admin".to_string()]),
            ..crate::table::FieldPermissions::default()
        };
        let user = crate::action::User::new(1, "reader").with_roles(["user"]);

        for _ in 0..64 {
            let config = Arc::new(
                crate::table::TableConfig::new("secrets")
                    .field(
                        crate::table::FieldConfig::new(
                            "z_secret",
                            crate::table::FieldType::Integer,
                        )
                        .permissions(protected.clone()),
                    )
                    .expect("受限字段配置应有效")
                    .field(
                        crate::table::FieldConfig::new(
                            "a_secret",
                            crate::table::FieldType::Integer,
                        )
                        .permissions(protected.clone()),
                    )
                    .expect("受限字段配置应有效")
                    .field(
                        crate::table::FieldConfig::new(
                            "m_secret",
                            crate::table::FieldType::Integer,
                        )
                        .permissions(protected.clone()),
                    )
                    .expect("受限字段配置应有效"),
            );
            let roles: Arc<[String]> = Arc::from(vec!["user".to_string()]);
            let query = TableQuery::new(config, roles, None);

            let sql_err = query
                .build_select_sql(None)
                .expect_err("SELECT * 应拒绝受限字段");
            assert!(
                matches!(sql_err, BaseError::FieldPermissionDenied(_, field, _) if field == "a_secret")
            );

            let action_err = query
                .ensure_fields_readable(&user)
                .expect_err("整实体 Action 应拒绝受限字段");
            assert!(
                matches!(action_err, BaseError::FieldPermissionDenied(_, field, _) if field == "a_secret")
            );
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
