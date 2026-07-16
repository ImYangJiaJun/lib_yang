//! 编译后的字段配置
//!
//! 应用侧通过 [`super::Field`] 声明字段；本模块只保存构建完成后的执行期表示。

use crate::error::BaseError;
use crate::table::{FieldType, Validator};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Schema-first DSL 构建后的内部字段配置。
#[derive(Debug, Clone)]
#[non_exhaustive]
pub(crate) struct FieldConfig {
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

    /// 是否由数据库自动生成递增值。
    ///
    /// 仅允许用于整数类型的主键字段；schema 同步器会在启动期校验该约束。
    pub auto_increment: bool,

    /// 验证器列表
    ///
    /// 用于验证字段值的验证器，按顺序执行
    pub validators: Vec<Validator>,

    /// 字段权限
    ///
    /// 定义不同角色对该字段的访问权限
    pub permissions: FieldPermissions,

    /// 是否从通用记录输出和表 JSON Schema 中隐藏。
    pub(crate) hidden: bool,

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
    pub(crate) fn new(name: impl Into<String>, field_type: FieldType) -> Self {
        Self {
            name: name.into(),
            display_name: String::new(),
            field_type,
            required: false,
            default_value: None,
            auto_increment: false,
            validators: Vec::new(),
            permissions: FieldPermissions::default(),
            hidden: false,
            filterable: true,
            sortable: true,
            relation: None,
        }
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
    pub(crate) fn validate(&self, value: &serde_json::Value) -> Result<(), BaseError> {
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

/// 字段操作的访问范围。
///
/// 使用显式的 `Everyone` / `Nobody`，避免旧版“空角色集合等于公开”导致无法表达
/// 禁止访问的歧义。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "roles", rename_all = "snake_case")]
pub(crate) enum Audience {
    /// 所有调用方都可以访问。
    #[default]
    Everyone,
    /// 所有调用方都不可以访问。
    Nobody,
    /// 至少拥有其中一个角色时可以访问。
    Roles(HashSet<String>),
}

impl Audience {
    /// 创建角色受限的访问范围。
    pub(crate) fn roles<I, S>(roles: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::Roles(roles.into_iter().map(Into::into).collect())
    }

    /// 判断给定角色集合是否满足访问范围。
    pub(crate) fn allows(&self, user_roles: &HashSet<String>) -> bool {
        match self {
            Self::Everyone => true,
            Self::Nobody => false,
            Self::Roles(roles) => user_roles.iter().any(|role| roles.contains(role)),
        }
    }
}

/// 字段权限配置。
///
/// 四类操作分别使用明确访问范围；默认全部公开。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct FieldPermissions {
    /// 读取权限。
    pub readable: Audience,
    /// 写入权限。
    pub writable: Audience,
    /// 筛选权限。
    pub filterable: Audience,
    /// 排序权限。
    pub sortable: Audience,
}

impl FieldPermissions {
    /// 检查用户是否有读取权限
    ///
    /// # 参数
    ///
    /// - `user_roles`: 用户的角色集合（HashSet，支持 O(1) 查找）
    ///
    /// # 返回值
    ///
    /// 如果用户有读取权限返回 true，否则返回 false
    ///
    pub(crate) fn can_read(&self, user_roles: &HashSet<String>) -> bool {
        self.readable.allows(user_roles)
    }

    /// 检查用户是否有写入权限
    ///
    /// # 参数
    ///
    /// - `user_roles`: 用户的角色集合（HashSet，支持 O(1) 查找）
    ///
    /// # 返回值
    ///
    /// 如果用户有写入权限返回 true，否则返回 false
    ///
    #[cfg(any(feature = "mysql", test))]
    pub(crate) fn can_write(&self, user_roles: &HashSet<String>) -> bool {
        self.writable.allows(user_roles)
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
    pub(crate) fn can_filter(&self, user_roles: &HashSet<String>) -> bool {
        self.filterable.allows(user_roles)
    }

    /// 检查用户是否有排序权限
    ///
    /// # 参数
    ///
    /// - `user_roles`: 用户的角色集合（HashSet，支持 O(1) 查找）
    ///
    /// # 返回值
    ///
    /// 如果用户有排序权限返回 true，否则返回 false
    ///
    pub(crate) fn can_sort(&self, user_roles: &HashSet<String>) -> bool {
        self.sortable.allows(user_roles)
    }
}

/// 关联表配置
///
/// 定义字段与其他表的关联关系，用于外键字段。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RelationConfig {
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

    /// 多对一关联
    ///
    /// 多条当前表记录关联目标表的一条记录，常用于普通外键列
    ManyToOne,

    /// 多对多关联
    ///
    /// 多条记录对应另一个表的多条记录，通常需要中间表
    ManyToMany,
}
