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
//!         .field(FieldConfig::new("id", FieldType::BigInt))
//!         .field(FieldConfig::new("name", FieldType::String { max_length: 50 }))
//!         .field(FieldConfig::new("email", FieldType::String { max_length: 100 }))
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
use std::sync::Arc;

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
///         .field(FieldConfig::new("id", FieldType::BigInt))
///         .field(FieldConfig::new("name", FieldType::String { max_length: 50 }))
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

    /// 查询参数
    ///
    /// 包含字段选择、WHERE 条件、排序规则和分页参数
    query_params: QueryParams,

    /// 数据库连接池引用（预留）
    ///
    /// 暂未使用，预留用于后续实现 CRUD 操作
    #[cfg(feature = "mysql")]
    #[allow(dead_code)]
    pool: Option<Arc<sqlx::MySqlPool>>,
}

/// 检查字符串是否为合法的 SQL 标识符
///
/// 合法标识符规则：
/// - 首字符必须是 ASCII 字母或下划线
/// - 后续字符必须是 ASCII 字母、数字或下划线
/// - 不能为空
/// - 不能包含分号、`--`、空白字符
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
        Self {
            table_config,
            user_roles,
            query_params: QueryParams::new(),
            pool,
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
        Self {
            table_config,
            user_roles,
            query_params: QueryParams::new(),
        }
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
    ///         .field(FieldConfig::new("id", FieldType::BigInt))
    ///         .field(FieldConfig::new("name", FieldType::String { max_length: 50 }))
    ///         .field(FieldConfig::new("email", FieldType::String { max_length: 100 }))
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
    ///         .field(FieldConfig::new("id", FieldType::BigInt))
    /// );
    /// let query2 = TableQuery::new(
    ///     table_config2,
    ///     Arc::from(vec!["admin".to_string()]),
    ///     None,
    /// );
    /// let result2 = query2.select_fields(&["nonexistent_field"]);
    /// assert!(result2.is_err());
    /// ```
    pub fn select_fields(mut self, fields: &[&str]) -> Result<Self, BaseError> {
        // 验证每个字段
        for field_name in fields {
            // 1. 检查字段是否存在
            let field_config = self.table_config.get_field(field_name).ok_or_else(|| {
                BaseError::FieldNotFound(
                    self.table_config.table_name.clone(),
                    field_name.to_string(),
                )
            })?;

            // 2. 检查用户是否有读取权限
            if !field_config.permissions.can_read(&self.user_roles) {
                return Err(BaseError::FieldPermissionDenied(
                    self.table_config.table_name.clone(),
                    field_name.to_string(),
                    "用户无读取权限".to_string(),
                ));
            }
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
    pub fn where_between(
        mut self,
        field: &str,
        lo: Value,
        hi: Value,
    ) -> Result<Self, BaseError> {
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
    pub fn order_by(mut self, field: &str, order: SortOrder) -> Result<Self, BaseError> {
        // 1. 检查字段是否存在
        let field_config = self.table_config.get_field(field).ok_or_else(|| {
            BaseError::FieldNotFound(self.table_config.table_name.clone(), field.to_string())
        })?;

        // 2. 检查用户是否有排序权限
        if !field_config.permissions.can_sort(&self.user_roles) {
            return Err(BaseError::FieldPermissionDenied(
                self.table_config.table_name.clone(),
                field.to_string(),
                "用户无排序权限".to_string(),
            ));
        }

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
        self.query_params.page = Some(page);
        self.query_params.page_size = Some(page_size);
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

        // 2. 检查用户是否有筛选权限
        if !field_config.permissions.can_filter(&self.user_roles) {
            return Err(BaseError::FieldPermissionDenied(
                self.table_config.table_name.clone(),
                field.to_string(),
                "用户无筛选权限".to_string(),
            ));
        }

        Ok(())
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
    ///         .field(FieldConfig::new("id", FieldType::BigInt))
    ///         .field(FieldConfig::new("name", FieldType::String { max_length: 50 }))
    ///         .field(FieldConfig::new("email", FieldType::String { max_length: 100 }))
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
    pub async fn paginate<T>(self) -> Result<crate::table::PaginatedResult<T>, BaseError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin + Serialize,
    {
        // 1. 检查数据库连接池是否存在
        let _pool = self
            .pool
            .as_ref()
            .ok_or(BaseError::DatabaseNotInitialized)?;

        // 2. 获取分页参数，如果未设置则使用默认值
        let page = self.query_params.page.unwrap_or(1);
        let page_size = self.query_params.page_size.unwrap_or(20);

        // 3. 执行 COUNT(*) 查询获取总记录数
        let total = self.count_internal().await?;

        // 4. 如果总记录数为 0，直接返回空结果
        if total == 0 {
            return Ok(crate::table::PaginatedResult::empty(page, page_size));
        }

        // 5. 执行数据查询
        let data = self.select().await?;

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

        // 5. 执行查询
        let count = query
            .fetch_one(pool.as_ref())
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
    /// 集中处理 12 种 WHERE 条件（Eq/Ne/In/NotIn/Like/Gt/Gte/Lt/Lte/Between/IsNull/IsNotNull）的 SQL 拼接，
    /// 并通过 `quote_identifier` 对字段名进行反引号转义，防止 SQL 注入。
    ///
    /// # 参数
    ///
    /// - `sql`：目标 SQL 字符串（追加模式）
    /// - `params`：参数列表（追加模式）
    ///
    /// # 返回值
    ///
    /// - `Ok(())`：拼接成功
    /// - `Err(BaseError)`：字段名非法或参数转换失败
    fn append_where_to_sql(
        &self,
        sql: &mut String,
        params: &mut Vec<SqlParam>,
    ) -> Result<(), BaseError> {
        // 如果没有 WHERE 条件，直接返回
        if self.query_params.where_conditions.is_empty() {
            return Ok(());
        }

        sql.push_str(" WHERE ");
        let mut first = true;

        for condition in &self.query_params.where_conditions {
            if !first {
                sql.push_str(" AND ");
            }
            first = false;

            match condition {
                WhereCondition::Eq { field, value } => {
                    // 通过 quote_identifier 转义字段名，防止 SQL 注入
                    let quoted = self.quote_identifier(field)?;
                    sql.push_str(&format!("{} = ?", quoted));
                    params.push(SqlParam::from_json(value)?);
                }
                WhereCondition::In { field, values } => {
                    let quoted = self.quote_identifier(field)?;
                    let placeholders = vec!["?"; values.len()].join(", ");
                    sql.push_str(&format!("{} IN ({})", quoted, placeholders));
                    for value in values {
                        params.push(SqlParam::from_json(value)?);
                    }
                }
                WhereCondition::Like { field, pattern } => {
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
                    let quoted = self.quote_identifier(field)?;
                    let placeholders = vec!["?"; values.len()].join(", ");
                    sql.push_str(&format!("{} NOT IN ({})", quoted, placeholders));
                    for value in values {
                        params.push(SqlParam::from_json(value)?);
                    }
                }
            }
        }

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
        let mut sql = format!("SELECT COUNT(*) FROM `{}`", self.table_config.table_name);
        let mut params = Vec::new();

        // 通过统一方法拼接 WHERE 子句
        self.append_where_to_sql(&mut sql, &mut params)?;

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
            SqlParam::Float(f) => query.bind(*f),
            SqlParam::String(s) => query.bind(s.clone()),
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
    ///         .field(FieldConfig::new("id", FieldType::BigInt))
    ///         .field(FieldConfig::new("name", FieldType::String { max_length: 50 }))
    ///         .field(FieldConfig::new("email", FieldType::String { max_length: 100 }))
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
        let (sql, params) = self.build_select_sql()?;

        // 3. 创建查询
        let mut query = sqlx::query_as::<_, T>(&sql);

        // 4. 绑定参数
        for param in params {
            query = Self::bind_param(query, &param);
        }

        // 5. 执行查询
        let results = query
            .fetch_all(pool.as_ref())
            .await
            .map_err(|e| BaseError::DatabaseQueryFailed(yang_db::DbError::from(e)))?;

        Ok(results)
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
    fn build_select_sql(&self) -> Result<(String, Vec<SqlParam>), BaseError> {
        let mut sql = String::from("SELECT ");
        let mut params = Vec::new();

        // 1. 字段列表（通过 quote_identifier 转义字段名）
        if let Some(fields) = &self.query_params.fields {
            if fields.is_empty() {
                sql.push('*');
            } else {
                // 对每个字段名进行反引号转义
                let quoted_fields: Result<Vec<String>, BaseError> =
                    fields.iter().map(|f| self.quote_identifier(f)).collect();
                sql.push_str(&quoted_fields?.join(", "));
            }
        } else {
            sql.push('*');
        }

        // 2. FROM 子句（表名使用反引号保护）
        sql.push_str(&format!(" FROM `{}`", self.table_config.table_name));

        // 3. 通过统一方法拼接 WHERE 子句
        self.append_where_to_sql(&mut sql, &mut params)?;

        // 4. ORDER BY 子句（通过 quote_identifier 转义字段名）
        if !self.query_params.order_by.is_empty() {
            sql.push_str(" ORDER BY ");
            let order_clauses: Result<Vec<String>, BaseError> = self
                .query_params
                .order_by
                .iter()
                .map(|(field, order)| {
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

        // 5. LIMIT 和 OFFSET 子句
        if let (Some(page), Some(page_size)) = (self.query_params.page, self.query_params.page_size)
        {
            let offset = (page - 1) * page_size;
            sql.push_str(&format!(" LIMIT {} OFFSET {}", page_size, offset));
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
            SqlParam::Float(f) => query.bind(*f),
            SqlParam::String(s) => query.bind(s.clone()),
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
        let (sql, params) = self.build_select_sql()?;

        // 3. 创建查询
        let mut query = sqlx::query_as::<_, T>(&sql);

        // 4. 绑定参数
        for param in params {
            query = Self::bind_param(query, &param);
        }

        // 5. 执行查询，返回可选结果
        let result = query
            .fetch_optional(pool.as_ref())
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
    ///         .field(FieldConfig::new("id", FieldType::BigInt))
    ///         .field(FieldConfig::new("name", FieldType::String { max_length: 50 }).required(true))
    ///         .field(FieldConfig::new("email", FieldType::String { max_length: 100 }))
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

        // 2. 验证所有字段值的合法性和权限
        self.validate_insert_data(&data)?;

        // 3. 构建 INSERT SQL 语句
        let (sql, params) = self.build_insert_sql(&data)?;

        // 4. 创建查询
        let mut query = sqlx::query(&sql);

        // 5. 绑定参数
        for param in params {
            query = Self::bind_execute_param(query, &param);
        }

        // 6. 执行插入
        let result = query
            .execute(pool.as_ref())
            .await
            .map_err(|e| BaseError::DatabaseExecuteFailed(yang_db::DbError::from(e)))?;

        Ok(result.rows_affected())
    }

    /// 验证插入数据
    ///
    /// 验证所有字段值的合法性和用户权限
    ///
    /// # 参数
    ///
    /// - `data`：要插入的数据
    ///
    /// # 返回值
    ///
    /// - `Ok(())`：验证通过
    /// - `Err(BaseError)`：验证失败
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldRequired`：必填字段缺失
    /// - `BaseError::FieldPermissionDenied`：用户无字段写入权限
    /// - `BaseError::ValidationFailed`：字段值验证失败
    fn validate_insert_data(
        &self,
        data: &std::collections::HashMap<String, Value>,
    ) -> Result<(), BaseError> {
        // 遍历表配置中的所有字段
        for (field_name, field_config) in &self.table_config.fields {
            // 获取字段值，如果不存在则使用 null
            let value = data.get(field_name).unwrap_or(&Value::Null);

            // 检查写入权限
            if !field_config.permissions.can_write(&self.user_roles) {
                // 如果用户没有写入权限，但提供了非 null 值，则拒绝
                if !value.is_null() {
                    return Err(BaseError::FieldPermissionDenied(
                        self.table_config.table_name.clone(),
                        field_name.clone(),
                        "用户无写入权限".to_string(),
                    ));
                }
                // 如果值为 null，跳过该字段的验证
                continue;
            }

            // 验证字段值（包括必填检查、类型检查和验证器检查）
            field_config.validate(value)?;
        }

        Ok(())
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
        let mut fields = Vec::new();
        let mut placeholders = Vec::new();
        let mut params = Vec::new();

        // 遍历数据，构建字段列表和参数列表
        for (field_name, value) in data {
            // 检查字段是否存在于表配置中
            if !self.table_config.fields.contains_key(field_name) {
                return Err(BaseError::FieldNotFound(
                    self.table_config.table_name.clone(),
                    field_name.clone(),
                ));
            }

            // 检查用户是否有写入权限
            if let Some(field_config) = self.table_config.fields.get(field_name) {
                if !field_config.permissions.can_write(&self.user_roles) {
                    // 如果没有权限且值不为 null，跳过该字段
                    if !value.is_null() {
                        continue;
                    }
                }
            }

            // 对字段名进行反引号转义，防止 SQL 注入
            let quoted = self.quote_identifier(field_name)?;
            fields.push(quoted);
            placeholders.push("?".to_string());
            params.push(SqlParam::from_json(value)?);
        }

        // 构建 SQL 语句（表名使用反引号保护）
        let sql = format!(
            "INSERT INTO `{}` ({}) VALUES ({})",
            self.table_config.table_name,
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
    ///         .field(FieldConfig::new("id", FieldType::BigInt))
    ///         .field(FieldConfig::new("name", FieldType::String { max_length: 50 }))
    ///         .field(FieldConfig::new("email", FieldType::String { max_length: 100 }))
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

        // 4. 创建查询
        let mut query = sqlx::query(&sql);

        // 5. 绑定参数
        for param in params {
            query = Self::bind_execute_param(query, &param);
        }

        // 6. 执行更新
        let result = query
            .execute(pool.as_ref())
            .await
            .map_err(|e| BaseError::DatabaseExecuteFailed(yang_db::DbError::from(e)))?;

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
            if !field_config.permissions.can_write(&self.user_roles) {
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
        let mut set_clauses = Vec::new();
        let mut params = Vec::new();

        // 1. 构建 SET 子句（通过 quote_identifier 转义字段名）
        for (field_name, value) in data {
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

        // 2. 构建基本 UPDATE 语句（表名使用反引号保护）
        let mut sql = format!(
            "UPDATE `{}` SET {}",
            self.table_config.table_name,
            set_clauses.join(", ")
        );

        // 3. 通过统一方法拼接 WHERE 子句
        self.append_where_to_sql(&mut sql, &mut params)?;

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
    ///         .field(FieldConfig::new("id", FieldType::BigInt))
    ///         .field(FieldConfig::new("name", FieldType::String { max_length: 50 }))
    ///         .field(FieldConfig::new("deleted_at", FieldType::BigInt))
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
    ///         .field(FieldConfig::new("id", FieldType::BigInt))
    ///         .field(FieldConfig::new("message", FieldType::Text))
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
        // 1. 检查数据库连接池是否存在
        let pool = self
            .pool
            .as_ref()
            .ok_or(BaseError::DatabaseNotInitialized)?;

        // 2. 检查是否配置了软删除字段
        if let Some(soft_delete_field) = &self.table_config.soft_delete_field {
            // 软删除：执行 UPDATE 设置删除标记
            // 使用当前时间戳作为删除标记
            let now = chrono::Utc::now().timestamp();
            let mut data = std::collections::HashMap::new();
            data.insert(soft_delete_field.clone(), Value::Number(now.into()));

            // 调用 update 方法执行软删除
            return self.update(data).await;
        }

        // 3. 物理删除：构建 DELETE SQL 语句
        let (sql, params) = self.build_delete_sql()?;

        // 4. 创建查询
        let mut query = sqlx::query(&sql);

        // 5. 绑定参数
        for param in params {
            query = Self::bind_execute_param(query, &param);
        }

        // 6. 执行删除
        let result = query
            .execute(pool.as_ref())
            .await
            .map_err(|e| BaseError::DatabaseExecuteFailed(yang_db::DbError::from(e)))?;

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
        self.build_select_sql()
    }

    /// 构建 DELETE SQL 语句（内部实现）
    #[cfg(not(test))]
    fn build_delete_sql(&self) -> Result<(String, Vec<SqlParam>), BaseError> {
        self.build_delete_sql_impl()
    }

    /// 构建 DELETE SQL 语句的实际实现
    fn build_delete_sql_impl(&self) -> Result<(String, Vec<SqlParam>), BaseError> {
        // 表名使用反引号保护
        let mut sql = format!("DELETE FROM `{}`", self.table_config.table_name);
        let mut params = Vec::new();

        // 通过统一方法拼接 WHERE 子句
        self.append_where_to_sql(&mut sql, &mut params)?;

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
            SqlParam::Float(f) => query.bind(*f),
            SqlParam::String(s) => query.bind(s.clone()),
        }
    }
}

/// SQL 参数类型
///
/// 用于表示 SQL 查询中的参数值
#[cfg(feature = "mysql")]
#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub(crate) enum SqlParam {
    /// 空值
    Null,
    /// 布尔值
    Bool(bool),
    /// 整数
    Int(i64),
    /// 浮点数
    Float(f64),
    /// 字符串
    String(String),
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
                } else if let Some(f) = n.as_f64() {
                    Ok(SqlParam::Float(f))
                } else {
                    Err(BaseError::DatabaseQueryFailed(yang_db::DbError::QueryError(format!(
                        "不支持的数字类型: {}",
                        n
                    ))))
                }
            }
            Value::String(s) => Ok(SqlParam::String(s.clone())),
            _ => Err(BaseError::DatabaseQueryFailed(yang_db::DbError::QueryError(format!(
                "不支持的值类型: {:?}",
                value
            )))),
        }
    }
}
