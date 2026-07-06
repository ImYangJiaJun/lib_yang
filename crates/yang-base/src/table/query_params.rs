//! 查询参数结构
//!
//! 提供统一的查询参数定义，包括 WHERE 条件、排序规则和分页参数。
//!
//! # 主要组件
//!
//! - `WhereCondition`：WHERE 条件枚举
//! - `QueryParams`：查询参数结构体
//! - `PaginatedResult`：分页结果结构体
//!
//! # 示例
//!
//! ```rust
//! use yang_base::table::{WhereCondition, QueryParams, SortOrder};
//! use serde_json::json;
//!
//! // 创建查询参数
//! let mut params = QueryParams::new();
//!
//! // 添加字段选择
//! params.fields = Some(vec!["id".to_string(), "name".to_string()]);
//!
//! // 添加 WHERE 条件
//! params.where_conditions.push(WhereCondition::Eq {
//!     field: "status".to_string(),
//!     value: json!("active"),
//! });
//!
//! params.where_conditions.push(WhereCondition::In {
//!     field: "role".to_string(),
//!     values: vec![json!("admin"), json!("user")],
//! });
//!
//! // 添加排序规则
//! params.order_by.push(("created_at".to_string(), SortOrder::Desc));
//!
//! // 设置分页参数
//! params.page = Some(1);
//! params.page_size = Some(20);
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 默认每页条数。
pub const DEFAULT_QUERY_PAGE_SIZE: usize = 10;

/// 查询请求模型允许的最大每页条数。
pub const MAX_QUERY_PAGE_SIZE: usize = 100;

/// WHERE 条件枚举
///
/// 定义各种查询条件类型，支持常见的 SQL WHERE 子句操作。
///
/// # 变体
///
/// - `Eq`：等于条件 (field = value)
/// - `In`：包含于列表条件 (field IN (values))
/// - `Like`：模糊匹配条件 (field LIKE pattern)
/// - `Gt`：大于条件 (field > value)
/// - `Gte`：大于等于条件 (field >= value)
/// - `Lt`：小于条件 (field < value)
/// - `Lte`：小于等于条件 (field <= value)
/// - `IsNull`：空值判断 (field IS NULL)
/// - `IsNotNull`：非空值判断 (field IS NOT NULL)
///
/// # 示例
///
/// ```rust
/// use yang_base::table::WhereCondition;
/// use serde_json::json;
///
/// // 等于条件
/// let eq_condition = WhereCondition::Eq {
///     field: "status".to_string(),
///     value: json!("active"),
/// };
///
/// // 包含条件
/// let in_condition = WhereCondition::In {
///     field: "role".to_string(),
///     values: vec![json!("admin"), json!("user")],
/// };
///
/// // 模糊匹配
/// let like_condition = WhereCondition::Like {
///     field: "name".to_string(),
///     pattern: "%alice%".to_string(),
/// };
///
/// // 大于条件
/// let gt_condition = WhereCondition::Gt {
///     field: "age".to_string(),
///     value: json!(18),
/// };
///
/// // 空值判断
/// let is_null_condition = WhereCondition::IsNull {
///     field: "deleted_at".to_string(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum WhereCondition {
    /// 等于条件 (field = value)
    ///
    /// # 字段
    ///
    /// - `field`：字段名
    /// - `value`：比较值
    Eq {
        /// 字段名
        field: String,
        /// 比较值
        value: Value,
    },

    /// 包含于列表条件 (field IN (values))
    ///
    /// # 字段
    ///
    /// - `field`：字段名
    /// - `values`：值列表
    In {
        /// 字段名
        field: String,
        /// 值列表
        values: Vec<Value>,
    },

    /// 模糊匹配条件 (field LIKE pattern)
    ///
    /// # 字段
    ///
    /// - `field`：字段名
    /// - `pattern`：匹配模式，支持 % 和 _ 通配符
    Like {
        /// 字段名
        field: String,
        /// 匹配模式
        pattern: String,
    },

    /// 大于条件 (field > value)
    ///
    /// # 字段
    ///
    /// - `field`：字段名
    /// - `value`：比较值
    Gt {
        /// 字段名
        field: String,
        /// 比较值
        value: Value,
    },

    /// 大于等于条件 (field >= value)
    ///
    /// # 字段
    ///
    /// - `field`：字段名
    /// - `value`：比较值
    Gte {
        /// 字段名
        field: String,
        /// 比较值
        value: Value,
    },

    /// 小于条件 (field < value)
    ///
    /// # 字段
    ///
    /// - `field`：字段名
    /// - `value`：比较值
    Lt {
        /// 字段名
        field: String,
        /// 比较值
        value: Value,
    },

    /// 小于等于条件 (field <= value)
    ///
    /// # 字段
    ///
    /// - `field`：字段名
    /// - `value`：比较值
    Lte {
        /// 字段名
        field: String,
        /// 比较值
        value: Value,
    },

    /// 空值判断 (field IS NULL)
    ///
    /// # 字段
    ///
    /// - `field`：字段名
    IsNull {
        /// 字段名
        field: String,
    },

    /// 非空值判断 (field IS NOT NULL)
    ///
    /// # 字段
    ///
    /// - `field`：字段名
    IsNotNull {
        /// 字段名
        field: String,
    },

    /// 不等于条件 (field <> value)
    ///
    /// # 字段
    ///
    /// - `field`：字段名
    /// - `value`：比较值
    Ne {
        /// 字段名
        field: String,
        /// 比较值
        value: Value,
    },

    /// 区间条件 (field BETWEEN lo AND hi)
    ///
    /// # 字段
    ///
    /// - `field`：字段名
    /// - `lo`：区间下界
    /// - `hi`：区间上界
    Between {
        /// 字段名
        field: String,
        /// 区间下界
        lo: Value,
        /// 区间上界
        hi: Value,
    },

    /// 不在列表条件 (field NOT IN (values))
    ///
    /// # 字段
    ///
    /// - `field`：字段名
    /// - `values`：值列表
    NotIn {
        /// 字段名
        field: String,
        /// 值列表
        values: Vec<Value>,
    },

    /// 逻辑与组（嵌套子条件全部成立），SQL 渲染为 `(c1 AND c2 AND ...)`
    ///
    /// 空组等价于恒真（`1=1`），与顶层「无条件」语义一致。
    ///
    /// # 字段
    ///
    /// - `conditions`：子条件列表，递归可含 `And`/`Or` 组
    And {
        /// 子条件列表
        conditions: Vec<WhereCondition>,
    },

    /// 逻辑或组（嵌套子条件任一成立），SQL 渲染为 `(c1 OR c2 OR ...)`
    ///
    /// 空组等价于恒假（`1=0`），避免拼出非法的空括号。
    ///
    /// # 字段
    ///
    /// - `conditions`：子条件列表，递归可含 `And`/`Or` 组
    Or {
        /// 子条件列表
        conditions: Vec<WhereCondition>,
    },
}

impl WhereCondition {
    /// 获取条件涉及的字段名
    ///
    /// 叶子条件返回 `Some(字段名)`；逻辑组（`And`/`Or`）无单一字段，返回 `None`。
    ///
    /// # 返回值
    ///
    /// 叶子返回 `Some(&str)`，组节点返回 `None`
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::WhereCondition;
    /// use serde_json::json;
    ///
    /// let condition = WhereCondition::Eq {
    ///     field: "status".to_string(),
    ///     value: json!("active"),
    /// };
    ///
    /// assert_eq!(condition.field(), Some("status"));
    /// ```
    pub fn field(&self) -> Option<&str> {
        match self {
            WhereCondition::Eq { field, .. }
            | WhereCondition::In { field, .. }
            | WhereCondition::Like { field, .. }
            | WhereCondition::Gt { field, .. }
            | WhereCondition::Gte { field, .. }
            | WhereCondition::Lt { field, .. }
            | WhereCondition::Lte { field, .. }
            | WhereCondition::IsNull { field }
            | WhereCondition::IsNotNull { field }
            | WhereCondition::Ne { field, .. }
            | WhereCondition::Between { field, .. }
            | WhereCondition::NotIn { field, .. } => Some(field),
            // 逻辑组无单一字段
            WhereCondition::And { .. } | WhereCondition::Or { .. } => None,
        }
    }
}

/// 查询参数结构体
///
/// 包含完整的查询参数，包括字段选择、WHERE 条件、排序规则和分页参数。
///
/// # 字段
///
/// - `fields`：字段选择列表，None 表示选择所有字段
/// - `where_conditions`：WHERE 条件列表
/// - `order_by`：排序规则列表，元组格式为 (字段名, 排序方向)
/// - `page`：当前页码，从 1 开始
/// - `page_size`：每页大小
///
/// # 示例
///
/// ```rust
/// use yang_base::table::{QueryParams, WhereCondition, SortOrder};
/// use serde_json::json;
///
/// let mut params = QueryParams::new();
///
/// // 选择特定字段
/// params.fields = Some(vec!["id".to_string(), "name".to_string(), "email".to_string()]);
///
/// // 添加 WHERE 条件
/// params.where_conditions.push(WhereCondition::Eq {
///     field: "status".to_string(),
///     value: json!("active"),
/// });
///
/// params.where_conditions.push(WhereCondition::Gt {
///     field: "age".to_string(),
///     value: json!(18),
/// });
///
/// // 添加排序规则
/// params.order_by.push(("created_at".to_string(), SortOrder::Desc));
/// params.order_by.push(("name".to_string(), SortOrder::Asc));
///
/// // 设置分页参数
/// params.page = Some(1);
/// params.page_size = Some(20);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QueryParams {
    /// 字段选择列表
    ///
    /// - `None`：选择所有字段
    /// - `Some(vec)`：选择指定字段
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<String>>,

    /// WHERE 条件列表
    ///
    /// 多个条件之间使用 AND 连接
    #[serde(default)]
    pub where_conditions: Vec<WhereCondition>,

    /// 排序规则列表
    ///
    /// 元组格式为 (字段名, 排序方向)
    #[serde(default)]
    pub order_by: Vec<(String, crate::table::SortOrder)>,

    /// 当前页码
    ///
    /// 从 1 开始，None 表示不分页
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<usize>,

    /// 每页大小
    ///
    /// None 表示使用默认值或不分页
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<usize>,
}

impl QueryParams {
    /// 创建新的查询参数
    ///
    /// # 返回值
    ///
    /// 返回空的查询参数实例
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::QueryParams;
    ///
    /// let params = QueryParams::new();
    /// assert!(params.fields.is_none());
    /// assert!(params.where_conditions.is_empty());
    /// assert!(params.order_by.is_empty());
    /// assert!(params.page.is_none());
    /// assert!(params.page_size.is_none());
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置字段选择列表
    ///
    /// # 参数
    ///
    /// - `fields`：字段名列表
    ///
    /// # 返回值
    ///
    /// 返回 self 以支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::QueryParams;
    ///
    /// let params = QueryParams::new()
    ///     .with_fields(vec!["id".to_string(), "name".to_string()]);
    ///
    /// assert_eq!(params.fields, Some(vec!["id".to_string(), "name".to_string()]));
    /// ```
    pub fn with_fields(mut self, fields: Vec<String>) -> Self {
        self.fields = Some(fields);
        self
    }

    /// 添加 WHERE 条件
    ///
    /// # 参数
    ///
    /// - `condition`：WHERE 条件
    ///
    /// # 返回值
    ///
    /// 返回 self 以支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::{QueryParams, WhereCondition};
    /// use serde_json::json;
    ///
    /// let params = QueryParams::new()
    ///     .with_condition(WhereCondition::Eq {
    ///         field: "status".to_string(),
    ///         value: json!("active"),
    ///     });
    ///
    /// assert_eq!(params.where_conditions.len(), 1);
    /// ```
    pub fn with_condition(mut self, condition: WhereCondition) -> Self {
        self.where_conditions.push(condition);
        self
    }

    /// 添加排序规则
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    /// - `order`：排序方向
    ///
    /// # 返回值
    ///
    /// 返回 self 以支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::{QueryParams, SortOrder};
    ///
    /// let params = QueryParams::new()
    ///     .with_order_by("created_at".to_string(), SortOrder::Desc);
    ///
    /// assert_eq!(params.order_by.len(), 1);
    /// ```
    pub fn with_order_by(mut self, field: String, order: crate::table::SortOrder) -> Self {
        self.order_by.push((field, order));
        self
    }

    /// 设置分页参数
    ///
    /// # 参数
    ///
    /// - `page`：当前页码，从 1 开始
    /// - `page_size`：每页大小
    ///
    /// # 返回值
    ///
    /// 返回 self 以支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::QueryParams;
    ///
    /// let params = QueryParams::new()
    ///     .with_pagination(1, 20);
    ///
    /// assert_eq!(params.page, Some(1));
    /// assert_eq!(params.page_size, Some(20));
    /// ```
    pub fn with_pagination(mut self, page: usize, page_size: usize) -> Self {
        self.page = Some(page);
        self.page_size = Some(page_size);
        self
    }

    /// 归一化分页参数
    ///
    /// 将 `page` 中小于 1 的值（含 0）归一为 1，避免直接构造 `QueryParams`
    /// 时 `page == 0` 导致 LIMIT/OFFSET 计算下溢。`page_size` 保持原值。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::QueryParams;
    ///
    /// let mut params = QueryParams::new().with_pagination(0, 20);
    /// params.normalize();
    /// assert_eq!(params.page, Some(1));
    /// ```
    pub fn normalize(&mut self) {
        if let Some(page) = self.page {
            if page == 0 {
                self.page = Some(1);
            }
        }
        if let Some(page_size) = self.page_size {
            if page_size == 0 {
                self.page_size = Some(DEFAULT_QUERY_PAGE_SIZE);
            } else if page_size > MAX_QUERY_PAGE_SIZE {
                self.page_size = Some(MAX_QUERY_PAGE_SIZE);
            }
        }
    }
}

/// 分页结果结构体
///
/// 包含分页查询的完整结果，包括数据列表和分页元信息。
///
/// # 类型参数
///
/// - `T`：数据项类型，必须实现 Serialize 和 Deserialize
///
/// # 字段
///
/// - `data`：数据列表
/// - `total`：总记录数
/// - `page`：当前页码，从 1 开始
/// - `page_size`：每页大小
/// - `total_pages`：总页数
///
/// # 示例
///
/// ```rust
/// use yang_base::table::PaginatedResult;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Serialize, Deserialize)]
/// struct User {
///     id: i64,
///     name: String,
/// }
///
/// let users = vec![
///     User { id: 1, name: "Alice".to_string() },
///     User { id: 2, name: "Bob".to_string() },
/// ];
///
/// let result = PaginatedResult::new(users, 100, 1, 20);
///
/// assert_eq!(result.data.len(), 2);
/// assert_eq!(result.total, 100);
/// assert_eq!(result.page, 1);
/// assert_eq!(result.page_size, 20);
/// assert_eq!(result.total_pages, 5);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResult<T>
where
    T: Serialize,
{
    /// 数据列表
    pub data: Vec<T>,

    /// 总记录数
    pub total: usize,

    /// 当前页码
    ///
    /// 从 1 开始
    pub page: usize,

    /// 每页大小
    pub page_size: usize,

    /// 总页数
    pub total_pages: usize,
}

impl<T> PaginatedResult<T>
where
    T: Serialize,
{
    /// 创建新的分页结果
    ///
    /// # 参数
    ///
    /// - `data`：数据列表
    /// - `total`：总记录数
    /// - `page`：当前页码，从 1 开始
    /// - `page_size`：每页大小
    ///
    /// # 返回值
    ///
    /// 返回分页结果实例，自动计算总页数
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::PaginatedResult;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Debug, Serialize, Deserialize)]
    /// struct User {
    ///     id: i64,
    ///     name: String,
    /// }
    ///
    /// let users = vec![
    ///     User { id: 1, name: "Alice".to_string() },
    ///     User { id: 2, name: "Bob".to_string() },
    /// ];
    ///
    /// let result = PaginatedResult::new(users, 100, 1, 20);
    ///
    /// assert_eq!(result.total_pages, 5);
    /// ```
    pub fn new(data: Vec<T>, total: usize, page: usize, page_size: usize) -> Self {
        let total_pages = if page_size > 0 {
            total.div_ceil(page_size)
        } else {
            0
        };

        Self {
            data,
            total,
            page,
            page_size,
            total_pages,
        }
    }

    /// 创建空的分页结果
    ///
    /// # 参数
    ///
    /// - `page`：当前页码
    /// - `page_size`：每页大小
    ///
    /// # 返回值
    ///
    /// 返回空的分页结果实例
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::PaginatedResult;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Debug, Serialize, Deserialize)]
    /// struct User {
    ///     id: i64,
    ///     name: String,
    /// }
    ///
    /// let result: PaginatedResult<User> = PaginatedResult::empty(1, 20);
    ///
    /// assert_eq!(result.data.len(), 0);
    /// assert_eq!(result.total, 0);
    /// assert_eq!(result.total_pages, 0);
    /// ```
    pub fn empty(page: usize, page_size: usize) -> Self {
        Self {
            data: Vec::new(),
            total: 0,
            page,
            page_size,
            total_pages: 0,
        }
    }

    /// 判断是否有下一页
    ///
    /// # 返回值
    ///
    /// 如果有下一页返回 true，否则返回 false
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::PaginatedResult;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Debug, Serialize, Deserialize)]
    /// struct User {
    ///     id: i64,
    ///     name: String,
    /// }
    ///
    /// let result: PaginatedResult<User> = PaginatedResult::new(vec![], 100, 1, 20);
    /// assert!(result.has_next());
    ///
    /// let result: PaginatedResult<User> = PaginatedResult::new(vec![], 100, 5, 20);
    /// assert!(!result.has_next());
    /// ```
    pub fn has_next(&self) -> bool {
        self.page < self.total_pages
    }

    /// 判断是否有上一页
    ///
    /// # 返回值
    ///
    /// 如果有上一页返回 true，否则返回 false
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::PaginatedResult;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Debug, Serialize, Deserialize)]
    /// struct User {
    ///     id: i64,
    ///     name: String,
    /// }
    ///
    /// let result: PaginatedResult<User> = PaginatedResult::new(vec![], 100, 1, 20);
    /// assert!(!result.has_prev());
    ///
    /// let result: PaginatedResult<User> = PaginatedResult::new(vec![], 100, 2, 20);
    /// assert!(result.has_prev());
    /// ```
    pub fn has_prev(&self) -> bool {
        self.page > 1
    }
}
