//! 表配置
//!
//! 提供数据表的完整配置功能，包括表结构、字段、索引、排序和软删除等。

use crate::error::BaseError;
use crate::table::FieldConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 表配置
///
/// 定义数据表的完整配置，包括：
/// - 表名和显示名称
/// - 主键字段
/// - 字段列表
/// - 索引配置（唯一索引、普通索引）
/// - 默认排序规则
/// - 软删除字段
/// - 时间戳字段配置
///
/// # 示例
///
/// ```rust
/// use yang_base::table::{TableConfig, FieldConfig, FieldType, SortOrder};
///
/// // 创建一个用户表配置
/// let table = TableConfig::new("users")
///     .display_name("用户表")
///     .primary_key("id")
///     .field(FieldConfig::new("id", FieldType::BigInt).required(true)).expect("有效字段配置应注册成功")
///     .field(FieldConfig::new("username", FieldType::String { max_length: 50 }).required(true)).expect("有效字段配置应注册成功")
///     .field(FieldConfig::new("email", FieldType::String { max_length: 100 }).required(true)).expect("有效字段配置应注册成功")
///     .unique_index(vec!["username".to_string()])
///     .unique_index(vec!["email".to_string()])
///     .default_order(vec![("created_at".to_string(), SortOrder::Desc)])
///     .soft_delete_field("deleted_at")
///     .timestamps(true, true, true);
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct TableConfig {
    /// 表名
    ///
    /// 数据库中的表名称，通常使用蛇形命名法（snake_case）
    pub table_name: String,

    /// 显示名称
    ///
    /// 用于前端展示的表名称，通常使用中文
    pub display_name: String,

    /// 主键字段名
    ///
    /// 表的主键字段，通常为 "id"
    pub primary_key: String,

    /// 字段配置列表
    ///
    /// 使用 HashMap 存储字段配置，键为字段名，值为字段配置
    pub fields: HashMap<String, FieldConfig>,

    /// 唯一索引配置列表
    ///
    /// 每个唯一索引由一个或多个字段组成
    pub unique_indexes: Vec<IndexConfig>,

    /// 普通索引配置列表
    ///
    /// 每个普通索引由一个或多个字段组成
    pub indexes: Vec<IndexConfig>,

    /// 默认排序规则
    ///
    /// 定义查询时的默认排序字段和排序方向
    pub default_order: Vec<(String, SortOrder)>,

    /// 软删除字段名
    ///
    /// 如果设置了软删除字段，删除操作将更新该字段而非物理删除记录
    pub soft_delete_field: Option<String>,

    /// 时间戳字段配置
    ///
    /// 定义创建时间、更新时间和删除时间字段
    pub timestamp_fields: Option<TimestampFields>,
}

impl TableConfig {
    /// 创建新的表配置
    ///
    /// # 参数
    ///
    /// - `table_name`: 表名
    ///
    /// # 返回值
    ///
    /// 返回一个新的 TableConfig 实例，使用默认配置
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::TableConfig;
    ///
    /// let table = TableConfig::new("users");
    /// assert_eq!(table.table_name, "users");
    /// assert_eq!(table.primary_key, "id");
    /// ```
    pub fn new(table_name: impl Into<String>) -> Self {
        Self {
            table_name: table_name.into(),
            display_name: String::new(),
            primary_key: "id".to_string(),
            fields: HashMap::new(),
            unique_indexes: Vec::new(),
            indexes: Vec::new(),
            default_order: Vec::new(),
            soft_delete_field: None,
            timestamp_fields: None,
        }
    }

    /// 设置显示名称
    ///
    /// # 参数
    ///
    /// - `name`: 显示名称
    ///
    /// # 返回值
    ///
    /// 返回 `Result<Self, BaseError>`，支持通过 `?` 或 `expect` 继续链式调用
    ///
    /// # 错误
    ///
    /// - `BaseError::ConfigError`：字段名为空白字符串
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::TableConfig;
    ///
    /// let table = TableConfig::new("users")
    ///     .display_name("用户表");
    /// assert_eq!(table.display_name, "用户表");
    /// ```
    pub fn display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = name.into();
        self
    }

    /// 设置主键字段
    ///
    /// # 参数
    ///
    /// - `key`: 主键字段名
    ///
    /// # 返回值
    ///
    /// 返回 self，支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::TableConfig;
    ///
    /// let table = TableConfig::new("users")
    ///     .primary_key("user_id");
    /// assert_eq!(table.primary_key, "user_id");
    /// ```
    pub fn primary_key(mut self, key: impl Into<String>) -> Self {
        self.primary_key = key.into();
        self
    }

    /// 添加字段配置
    ///
    /// # 参数
    ///
    /// - `field`: 字段配置
    ///
    /// # 返回值
    ///
    /// 返回 self，支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::{TableConfig, FieldConfig, FieldType};
    ///
    /// let table = TableConfig::new("users")
    ///     .field(FieldConfig::new("id", FieldType::BigInt)).expect("有效字段配置应注册成功")
    ///     .field(FieldConfig::new("username", FieldType::String { max_length: 50 })).expect("有效字段配置应注册成功");
    /// assert_eq!(table.fields.len(), 2);
    /// ```
    pub fn field(mut self, field: FieldConfig) -> Result<Self, BaseError> {
        Self::validate_field_config(&field)?;
        self.fields.insert(field.name.clone(), field);
        Ok(self)
    }

    /// 批量添加字段配置
    ///
    /// # 参数
    ///
    /// - `fields`: 字段配置列表
    ///
    /// # 返回值
    ///
    /// 返回 `Result<Self, BaseError>`，支持通过 `?` 或 `expect` 继续链式调用
    ///
    /// # 错误
    ///
    /// - `BaseError::ConfigError`：任一字段名为空白字符串
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::{TableConfig, FieldConfig, FieldType};
    ///
    /// let table = TableConfig::new("users")
    ///     .fields(vec![
    ///         FieldConfig::new("id", FieldType::BigInt).required(true),
    ///         FieldConfig::new("username", FieldType::String { max_length: 50 }).required(true),
    ///         FieldConfig::new("email", FieldType::String { max_length: 100 }).required(true),
    ///     ]).expect("有效字段配置应注册成功");
    /// assert_eq!(table.fields.len(), 3);
    /// ```
    pub fn fields(mut self, fields: Vec<FieldConfig>) -> Result<Self, BaseError> {
        for field in fields {
            self = self.field(field)?;
        }
        Ok(self)
    }

    /// 从迭代器批量添加字段配置
    ///
    /// # 参数
    ///
    /// - `fields`: 字段配置迭代器
    ///
    /// # 返回值
    ///
    /// 返回 `Result<Self, BaseError>`，支持通过 `?` 或 `expect` 继续链式调用
    ///
    /// # 错误
    ///
    /// - `BaseError::ConfigError`：任一字段名为空白字符串
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::{TableConfig, FieldConfig, FieldType};
    ///
    /// let field_configs = vec![
    ///     FieldConfig::new("id", FieldType::BigInt),
    ///     FieldConfig::new("username", FieldType::String { max_length: 50 }),
    /// ];
    ///
    /// let table = TableConfig::new("users")
    ///     .fields_from_iter(field_configs.into_iter()).expect("有效字段配置应注册成功");
    /// assert_eq!(table.fields.len(), 2);
    /// ```
    pub fn fields_from_iter<I>(mut self, fields: I) -> Result<Self, BaseError>
    where
        I: IntoIterator<Item = FieldConfig>,
    {
        for field in fields {
            self = self.field(field)?;
        }
        Ok(self)
    }

    fn validate_field_config(field: &FieldConfig) -> Result<(), BaseError> {
        if field.name.trim().is_empty() {
            return Err(BaseError::ConfigError("字段名称不能为空".to_string()));
        }

        Ok(())
    }

    /// 添加唯一索引
    ///
    /// # 参数
    ///
    /// - `fields`: 索引字段列表
    ///
    /// # 返回值
    ///
    /// 返回 self，支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::TableConfig;
    ///
    /// let table = TableConfig::new("users")
    ///     .unique_index(vec!["username".to_string()])
    ///     .unique_index(vec!["email".to_string()]);
    /// assert_eq!(table.unique_indexes.len(), 2);
    /// ```
    pub fn unique_index(mut self, fields: Vec<String>) -> Self {
        self.unique_indexes.push(IndexConfig { name: None, fields });
        self
    }

    /// 添加普通索引
    ///
    /// # 参数
    ///
    /// - `fields`: 索引字段列表
    ///
    /// # 返回值
    ///
    /// 返回 self，支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::TableConfig;
    ///
    /// let table = TableConfig::new("users")
    ///     .index(vec!["created_at".to_string()])
    ///     .index(vec!["status".to_string(), "created_at".to_string()]);
    /// assert_eq!(table.indexes.len(), 2);
    /// ```
    pub fn index(mut self, fields: Vec<String>) -> Self {
        self.indexes.push(IndexConfig { name: None, fields });
        self
    }

    /// 设置默认排序规则
    ///
    /// # 参数
    ///
    /// - `order`: 排序规则列表，每项为 (字段名, 排序方向)
    ///
    /// # 返回值
    ///
    /// 返回 self，支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::{TableConfig, SortOrder};
    ///
    /// let table = TableConfig::new("users")
    ///     .default_order(vec![
    ///         ("created_at".to_string(), SortOrder::Desc),
    ///         ("id".to_string(), SortOrder::Asc),
    ///     ]);
    /// assert_eq!(table.default_order.len(), 2);
    /// ```
    pub fn default_order(mut self, order: Vec<(String, SortOrder)>) -> Self {
        self.default_order = order;
        self
    }

    /// 设置软删除字段
    ///
    /// # 参数
    ///
    /// - `field`: 软删除字段名
    ///
    /// # 返回值
    ///
    /// 返回 self，支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::TableConfig;
    ///
    /// let table = TableConfig::new("users")
    ///     .soft_delete_field("deleted_at");
    /// assert_eq!(table.soft_delete_field, Some("deleted_at".to_string()));
    /// ```
    pub fn soft_delete_field(mut self, field: impl Into<String>) -> Self {
        self.soft_delete_field = Some(field.into());
        self
    }

    /// 设置时间戳字段配置
    ///
    /// # 参数
    ///
    /// - `created_at`: 是否启用创建时间字段
    /// - `updated_at`: 是否启用更新时间字段
    /// - `deleted_at`: 是否启用删除时间字段
    ///
    /// # 返回值
    ///
    /// 返回 self，支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::TableConfig;
    ///
    /// let table = TableConfig::new("users")
    ///     .timestamps(true, true, true);
    /// assert!(table.timestamp_fields.is_some());
    /// ```
    pub fn timestamps(mut self, created_at: bool, updated_at: bool, deleted_at: bool) -> Self {
        self.timestamp_fields = Some(TimestampFields {
            created_at: if created_at {
                Some("created_at".to_string())
            } else {
                None
            },
            updated_at: if updated_at {
                Some("updated_at".to_string())
            } else {
                None
            },
            deleted_at: if deleted_at {
                Some("deleted_at".to_string())
            } else {
                None
            },
        });
        self
    }

    /// 验证字段是否存在
    ///
    /// # 参数
    ///
    /// - `field_name`: 字段名
    ///
    /// # 返回值
    ///
    /// 如果字段存在返回 Ok(())，否则返回 FieldNotFound 错误
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::{TableConfig, FieldConfig, FieldType};
    ///
    /// let table = TableConfig::new("users")
    ///     .field(FieldConfig::new("username", FieldType::String { max_length: 50 })).expect("有效字段配置应注册成功");
    ///
    /// assert!(table.validate_field("username").is_ok());
    /// assert!(table.validate_field("nonexistent").is_err());
    /// ```
    pub fn validate_field(&self, field_name: &str) -> Result<(), BaseError> {
        if self.fields.contains_key(field_name) {
            Ok(())
        } else {
            Err(BaseError::FieldNotFound(
                self.table_name.clone(),
                field_name.to_string(),
            ))
        }
    }

    /// 获取字段配置
    ///
    /// # 参数
    ///
    /// - `field_name`: 字段名
    ///
    /// # 返回值
    ///
    /// 如果字段存在返回字段配置的引用，否则返回 None
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::{TableConfig, FieldConfig, FieldType};
    ///
    /// let table = TableConfig::new("users")
    ///     .field(FieldConfig::new("username", FieldType::String { max_length: 50 })).expect("有效字段配置应注册成功");
    ///
    /// let field = table.get_field("username");
    /// assert!(field.is_some());
    /// assert_eq!(field.unwrap().name, "username");
    /// ```
    pub fn get_field(&self, field_name: &str) -> Option<&FieldConfig> {
        self.fields.get(field_name)
    }

    /// 验证查询参数
    ///
    /// 验证查询中引用的所有字段是否存在于表配置中
    ///
    /// # 参数
    ///
    /// - `field_names`: 字段名列表
    ///
    /// # 返回值
    ///
    /// 如果所有字段都存在返回 Ok(())，否则返回第一个不存在的字段的错误
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::{TableConfig, FieldConfig, FieldType};
    ///
    /// let table = TableConfig::new("users")
    ///     .field(FieldConfig::new("username", FieldType::String { max_length: 50 })).expect("有效字段配置应注册成功")
    ///     .field(FieldConfig::new("email", FieldType::String { max_length: 100 })).expect("有效字段配置应注册成功");
    ///
    /// assert!(table.validate_query(&["username", "email"]).is_ok());
    /// assert!(table.validate_query(&["username", "nonexistent"]).is_err());
    /// ```
    pub fn validate_query(&self, field_names: &[&str]) -> Result<(), BaseError> {
        for field_name in field_names {
            self.validate_field(field_name)?;
        }
        Ok(())
    }
}

/// 索引配置
///
/// 定义数据库索引的配置信息
///
/// # 示例
///
/// ```rust
/// use yang_base::table::IndexConfig;
///
/// let index = IndexConfig {
///     name: Some("idx_username".to_string()),
///     fields: vec!["username".to_string()],
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct IndexConfig {
    /// 索引名称
    ///
    /// 如果为 None，将自动生成索引名称
    pub name: Option<String>,

    /// 索引字段列表
    ///
    /// 组成索引的字段名列表，支持复合索引
    pub fields: Vec<String>,
}

/// 排序方向
///
/// 定义查询结果的排序方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum SortOrder {
    /// 升序排序
    Asc,
    /// 降序排序
    Desc,
}

/// 时间戳字段配置
///
/// 定义表的时间戳字段，包括创建时间、更新时间和删除时间
///
/// # 示例
///
/// ```rust
/// use yang_base::table::TimestampFields;
///
/// let timestamps = TimestampFields {
///     created_at: Some("created_at".to_string()),
///     updated_at: Some("updated_at".to_string()),
///     deleted_at: Some("deleted_at".to_string()),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TimestampFields {
    /// 创建时间字段名
    ///
    /// 记录创建时自动设置的时间戳字段
    pub created_at: Option<String>,

    /// 更新时间字段名
    ///
    /// 记录更新时自动更新的时间戳字段
    pub updated_at: Option<String>,

    /// 删除时间字段名
    ///
    /// 软删除时设置的时间戳字段
    pub deleted_at: Option<String>,
}
