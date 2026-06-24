//! 字段配置
//!
//! 提供数据表字段的配置功能，包括字段类型、验证规则、权限控制和关联表信息。

use crate::error::BaseError;
use crate::table::{FieldType, Validator};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// 字段配置
///
/// 定义单个字段的所有属性，包括：
/// - 字段名称和显示名称
/// - 字段类型
/// - 必填标记和默认值
/// - 验证器列表
/// - 字段级权限配置
/// - 关联表信息
///
/// # 示例
///
/// ```rust
/// use yang_base::table::{FieldConfig, FieldType, Validator, FieldPermissions};
/// use serde_json::json;
///
/// // 创建一个必填的用户名字段
/// let username_field = FieldConfig::new("username", FieldType::String { max_length: 50 })
///     .display_name("用户名")
///     .required(true)
///     .validator(Validator::MinLength(3))
///     .validator(Validator::MaxLength(50));
///
/// // 创建一个带默认值的状态字段
/// let status_field = FieldConfig::new("status", FieldType::Enum {
///     values: vec!["active".to_string(), "inactive".to_string()],
/// })
///     .display_name("状态")
///     .default_value(json!("active"));
///
/// // 创建一个带权限控制的邮箱字段
/// let email_field = FieldConfig::new("email", FieldType::String { max_length: 100 })
///     .display_name("邮箱")
///     .required(true)
///     .validator(Validator::Email)
///     .permissions(FieldPermissions {
///         readable_roles: HashSet::from(["admin".to_string(), "user".to_string()]),
///         writable_roles: HashSet::from(["admin".to_string()]),
///         filterable_roles: HashSet::new(),
///         sortable_roles: HashSet::new(),
///     });
/// ```
#[derive(Debug, Clone)]
pub struct FieldConfig {
    /// 字段名
    ///
    /// 数据库中的字段名称，通常使用蛇形命名法（snake_case）
    pub name: String,

    /// 显示名称
    ///
    /// 用于前端展示的字段名称，通常使用中文
    pub display_name: String,

    /// 字段类型
    ///
    /// 定义字段的数据类型，如字符串、整数、枚举等
    pub field_type: FieldType,

    /// 是否必填
    ///
    /// 如果为 true，则字段值不能为 null
    pub required: bool,

    /// 默认值
    ///
    /// 当插入数据时未提供该字段值时使用的默认值
    pub default_value: Option<serde_json::Value>,

    /// 验证器列表
    ///
    /// 用于验证字段值的验证器，按顺序执行
    pub validators: Vec<Validator>,

    /// 字段权限
    ///
    /// 定义不同角色对该字段的访问权限
    pub permissions: FieldPermissions,

    /// 是否可筛选
    ///
    /// 如果为 true，则该字段可以用于 WHERE 条件筛选
    pub filterable: bool,

    /// 是否可排序
    ///
    /// 如果为 true，则该字段可以用于 ORDER BY 排序
    pub sortable: bool,

    /// 关联表配置
    ///
    /// 用于外键字段，定义与其他表的关联关系
    pub relation: Option<RelationConfig>,
}

impl FieldConfig {
    /// 创建新的字段配置
    ///
    /// # 参数
    ///
    /// - `name`: 字段名称
    /// - `field_type`: 字段类型
    ///
    /// # 返回值
    ///
    /// 返回一个新的 FieldConfig 实例，使用默认配置
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::{FieldConfig, FieldType};
    ///
    /// let field = FieldConfig::new("username", FieldType::String { max_length: 50 });
    /// assert_eq!(field.name, "username");
    /// assert!(!field.required);
    /// ```
    pub fn new(name: impl Into<String>, field_type: FieldType) -> Self {
        Self {
            name: name.into(),
            display_name: String::new(),
            field_type,
            required: false,
            default_value: None,
            validators: Vec::new(),
            permissions: FieldPermissions::default(),
            filterable: true,
            sortable: true,
            relation: None,
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
    /// 返回 self，支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::{FieldConfig, FieldType};
    ///
    /// let field = FieldConfig::new("username", FieldType::String { max_length: 50 })
    ///     .display_name("用户名");
    /// assert_eq!(field.display_name, "用户名");
    /// ```
    pub fn display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = name.into();
        self
    }

    /// 设置必填标记
    ///
    /// # 参数
    ///
    /// - `required`: 是否必填
    ///
    /// # 返回值
    ///
    /// 返回 self，支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::{FieldConfig, FieldType};
    ///
    /// let field = FieldConfig::new("email", FieldType::String { max_length: 100 })
    ///     .required(true);
    /// assert!(field.required);
    /// ```
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// 设置默认值
    ///
    /// # 参数
    ///
    /// - `value`: 默认值
    ///
    /// # 返回值
    ///
    /// 返回 self，支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::{FieldConfig, FieldType};
    /// use serde_json::json;
    ///
    /// let field = FieldConfig::new("status", FieldType::String { max_length: 20 })
    ///     .default_value(json!("active"));
    /// assert_eq!(field.default_value, Some(json!("active")));
    /// ```
    pub fn default_value(mut self, value: serde_json::Value) -> Self {
        self.default_value = Some(value);
        self
    }

    /// 添加验证器
    ///
    /// # 参数
    ///
    /// - `validator`: 验证器
    ///
    /// # 返回值
    ///
    /// 返回 self，支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::{FieldConfig, FieldType, Validator};
    ///
    /// let field = FieldConfig::new("age", FieldType::Integer)
    ///     .validator(Validator::Min(0.0))
    ///     .validator(Validator::Max(150.0));
    /// assert_eq!(field.validators.len(), 2);
    /// ```
    pub fn validator(mut self, validator: Validator) -> Self {
        self.validators.push(validator);
        self
    }

    /// 设置字段权限
    ///
    /// # 参数
    ///
    /// - `permissions`: 字段权限配置
    ///
    /// # 返回值
    ///
    /// 返回 self，支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::{FieldConfig, FieldType, FieldPermissions};
    ///
    /// let permissions = FieldPermissions {
    ///     readable_roles: HashSet::from(["admin".to_string()]),
    ///     writable_roles: HashSet::from(["admin".to_string()]),
    ///     filterable_roles: HashSet::new(),
    ///     sortable_roles: HashSet::new(),
    /// };
    ///
    /// let field = FieldConfig::new("salary", FieldType::Double)
    ///     .permissions(permissions);
    /// assert_eq!(field.permissions.readable_roles.len(), 1);
    /// ```
    pub fn permissions(mut self, permissions: FieldPermissions) -> Self {
        self.permissions = permissions;
        self
    }

    /// 设置关联表配置
    ///
    /// # 参数
    ///
    /// - `relation`: 关联表配置
    ///
    /// # 返回值
    ///
    /// 返回 self，支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::{FieldConfig, FieldType, RelationConfig, RelationType};
    ///
    /// let relation = RelationConfig {
    ///     table: "users".to_string(),
    ///     field: "id".to_string(),
    ///     display_fields: vec!["username".to_string(), "email".to_string()],
    ///     relation_type: RelationType::OneToOne,
    /// };
    ///
    /// let field = FieldConfig::new("user_id", FieldType::BigInt)
    ///     .relation(relation);
    /// assert!(field.relation.is_some());
    /// ```
    pub fn relation(mut self, relation: RelationConfig) -> Self {
        self.relation = Some(relation);
        self
    }

    /// 设置是否可筛选
    ///
    /// # 参数
    ///
    /// - `filterable`: 是否可筛选
    ///
    /// # 返回值
    ///
    /// 返回 self，支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::{FieldConfig, FieldType};
    ///
    /// let field = FieldConfig::new("password", FieldType::String { max_length: 255 })
    ///     .filterable(false);
    /// assert!(!field.filterable);
    /// ```
    pub fn filterable(mut self, filterable: bool) -> Self {
        self.filterable = filterable;
        self
    }

    /// 设置是否可排序
    ///
    /// # 参数
    ///
    /// - `sortable`: 是否可排序
    ///
    /// # 返回值
    ///
    /// 返回 self，支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::{FieldConfig, FieldType};
    ///
    /// let field = FieldConfig::new("description", FieldType::Text)
    ///     .sortable(false);
    /// assert!(!field.sortable);
    /// ```
    pub fn sortable(mut self, sortable: bool) -> Self {
        self.sortable = sortable;
        self
    }

    /// 验证字段值
    ///
    /// 执行完整的字段验证流程：
    /// 1. 检查必填约束
    /// 2. 执行字段类型验证
    /// 3. 依次执行所有配置的验证器
    ///
    /// # 参数
    ///
    /// - `value`: 要验证的值
    ///
    /// # 返回值
    ///
    /// 如果验证通过返回 Ok(())，否则返回相应的错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldRequired`: 字段为必填但值为 null
    /// - `BaseError::InvalidFieldType`: 字段类型验证失败
    /// - `BaseError::ValidationFailed`: 验证器验证失败
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::{FieldConfig, FieldType, Validator};
    /// use serde_json::json;
    ///
    /// let field = FieldConfig::new("username", FieldType::String { max_length: 50 })
    ///     .required(true)
    ///     .validator(Validator::MinLength(3));
    ///
    /// // 验证通过
    /// assert!(field.validate(&json!("alice")).is_ok());
    ///
    /// // 必填字段为 null
    /// assert!(field.validate(&json!(null)).is_err());
    ///
    /// // 长度不足
    /// assert!(field.validate(&json!("ab")).is_err());
    /// ```
    pub fn validate(&self, value: &serde_json::Value) -> Result<(), BaseError> {
        // 1. 检查必填约束
        if self.required && value.is_null() {
            return Err(BaseError::FieldRequired(self.name.clone()));
        }

        // 如果值为 null 且字段非必填，跳过后续验证
        if value.is_null() {
            return Ok(());
        }

        // 2. 执行字段类型验证
        self.field_type.validate(&self.name, value)?;

        // 3. 依次执行所有配置的验证器
        for validator in &self.validators {
            validator.validate(&self.name, value)?;
        }

        Ok(())
    }
}

/// 字段权限配置
///
/// 定义不同角色对字段的访问权限，包括读取、写入、筛选和排序权限。
/// 如果角色列表为空，表示允许所有用户访问。
///
/// # 示例
///
/// ```rust
/// use yang_base::table::FieldPermissions;
///
/// // 创建一个只有管理员可以读写的字段权限
/// let admin_only = FieldPermissions {
///     readable_roles: HashSet::from(["admin".to_string()]),
///     writable_roles: HashSet::from(["admin".to_string()]),
///     filterable_roles: HashSet::from(["admin".to_string()]),
///     sortable_roles: HashSet::from(["admin".to_string()]),
/// };
///
/// // 检查权限
/// let admin_roles = vec!["admin".to_string()];
/// let user_roles = vec!["user".to_string()];
///
/// assert!(admin_only.can_read(&admin_roles));
/// assert!(!admin_only.can_read(&user_roles));
///
/// // 创建一个所有人都可以访问的字段权限
/// let public = FieldPermissions::default();
/// assert!(public.can_read(&user_roles));
/// assert!(public.can_write(&user_roles));
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FieldPermissions {
    /// 可读角色集合
    ///
    /// 如果集合为空，表示所有用户都可以读取该字段
    pub readable_roles: HashSet<String>,

    /// 可写角色集合
    ///
    /// 如果集合为空，表示所有用户都可以写入该字段
    pub writable_roles: HashSet<String>,

    /// 可筛选角色集合
    ///
    /// 如果集合为空，表示所有用户都可以使用该字段进行筛选
    pub filterable_roles: HashSet<String>,

    /// 可排序角色集合
    ///
    /// 如果集合为空，表示所有用户都可以使用该字段进行排序
    pub sortable_roles: HashSet<String>,
}

impl FieldPermissions {
    /// 检查用户是否有读取权限
    ///
    /// # 参数
    ///
    /// - `user_roles`: 用户的角色列表
    ///
    /// # 返回值
    ///
    /// 如果用户有读取权限返回 true，否则返回 false
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::FieldPermissions;
    /// use std::collections::HashSet;
    ///
    /// let permissions = FieldPermissions {
    ///     readable_roles: HashSet::from(["admin".to_string(), "user".to_string()]),
    ///     ..Default::default()
    /// };
    ///
    /// assert!(permissions.can_read(&["admin".to_string()]));
    /// assert!(permissions.can_read(&["user".to_string()]));
    /// assert!(!permissions.can_read(&["guest".to_string()]));
    /// ```
    pub fn can_read(&self, user_roles: &[String]) -> bool {
        self.readable_roles.is_empty() || user_roles.iter().any(|r| self.readable_roles.contains(r))
    }

    /// 检查用户是否有写入权限
    ///
    /// # 参数
    ///
    /// - `user_roles`: 用户的角色列表
    ///
    /// # 返回值
    ///
    /// 如果用户有写入权限返回 true，否则返回 false
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::FieldPermissions;
    /// use std::collections::HashSet;
    ///
    /// let permissions = FieldPermissions {
    ///     writable_roles: HashSet::from(["admin".to_string()]),
    ///     ..Default::default()
    /// };
    ///
    /// assert!(permissions.can_write(&["admin".to_string()]));
    /// assert!(!permissions.can_write(&["user".to_string()]));
    /// ```
    pub fn can_write(&self, user_roles: &[String]) -> bool {
        self.writable_roles.is_empty() || user_roles.iter().any(|r| self.writable_roles.contains(r))
    }

    /// 检查用户是否有筛选权限
    ///
    /// # 参数
    ///
    /// - `user_roles`: 用户的角色列表
    ///
    /// # 返回值
    ///
    /// 如果用户有筛选权限返回 true，否则返回 false
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::FieldPermissions;
    /// use std::collections::HashSet;
    ///
    /// let permissions = FieldPermissions {
    ///     filterable_roles: HashSet::from(["admin".to_string()]),
    ///     ..Default::default()
    /// };
    ///
    /// assert!(permissions.can_filter(&["admin".to_string()]));
    /// assert!(!permissions.can_filter(&["user".to_string()]));
    /// ```
    pub fn can_filter(&self, user_roles: &[String]) -> bool {
        self.filterable_roles.is_empty()
            || user_roles.iter().any(|r| self.filterable_roles.contains(r))
    }

    /// 检查用户是否有排序权限
    ///
    /// # 参数
    ///
    /// - `user_roles`: 用户的角色列表
    ///
    /// # 返回值
    ///
    /// 如果用户有排序权限返回 true，否则返回 false
    ///
    /// # 示例
    ///
    /// ```rust
    /// use yang_base::table::FieldPermissions;
    /// use std::collections::HashSet;
    ///
    /// let permissions = FieldPermissions {
    ///     sortable_roles: HashSet::from(["admin".to_string()]),
    ///     ..Default::default()
    /// };
    ///
    /// assert!(permissions.can_sort(&["admin".to_string()]));
    /// assert!(!permissions.can_sort(&["user".to_string()]));
    /// ```
    pub fn can_sort(&self, user_roles: &[String]) -> bool {
        self.sortable_roles.is_empty() || user_roles.iter().any(|r| self.sortable_roles.contains(r))
    }
}

/// 关联表配置
///
/// 定义字段与其他表的关联关系，用于外键字段。
///
/// # 示例
///
/// ```rust
/// use yang_base::table::{RelationConfig, RelationType};
///
/// // 定义一个一对一关联
/// let relation = RelationConfig {
///     table: "users".to_string(),
///     field: "id".to_string(),
///     display_fields: vec!["username".to_string(), "email".to_string()],
///     relation_type: RelationType::OneToOne,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationConfig {
    /// 关联的目标表名
    pub table: String,

    /// 关联的目标字段名
    pub field: String,

    /// 显示字段列表
    ///
    /// 当查询关联数据时，需要返回的字段列表
    pub display_fields: Vec<String>,

    /// 关联类型
    pub relation_type: RelationType,
}

/// 关联类型
///
/// 定义表之间的关联关系类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationType {
    /// 一对一关联
    ///
    /// 一条记录对应另一个表的一条记录
    OneToOne,

    /// 一对多关联
    ///
    /// 一条记录对应另一个表的多条记录
    OneToMany,

    /// 多对多关联
    ///
    /// 多条记录对应另一个表的多条记录，通常需要中间表
    ManyToMany,
}
