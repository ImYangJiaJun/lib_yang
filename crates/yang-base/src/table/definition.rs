//! Schema-first 数据表定义。
//!
//! [`Table`] 与 [`Field`] 是应用侧唯一需要使用的构建器。字段名、数据库类型、
//! 验证、权限、索引和时间戳语义在同一处声明，最终由 [`Table::build`] 一次校验并
//! 生成不可变 [`TableDefinition`]。

use super::field_config::{Audience, FieldConfig, FieldPermissions, RelationConfig, RelationType};
use super::table_config::{IndexConfig, TableConfig, TimestampFields};
#[cfg(feature = "mysql")]
use super::TableQuery;
use super::{FieldType, SortOrder, Validator};
use crate::error::BaseError;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// 创建数据库列名引用，用于表级排序和复合索引配置。
pub fn col(name: impl Into<String>) -> ColumnName {
    ColumnName(name.into())
}

/// 数据库列名引用。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColumnName(String);

impl ColumnName {
    /// 创建升序规则。
    pub fn asc(self) -> Order {
        Order {
            field: self.0,
            direction: SortOrder::Asc,
        }
    }

    /// 创建降序规则。
    pub fn desc(self) -> Order {
        Order {
            field: self.0,
            direction: SortOrder::Desc,
        }
    }

    fn into_name(self) -> String {
        self.0
    }
}

impl From<&str> for ColumnName {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for ColumnName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// 表的默认排序条目。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    field: String,
    direction: SortOrder,
}

/// Schema-first 字段构建器。
///
/// 字段名和字段配置从构造函数开始就是一个整体，例如
/// `Field::string("username", 64).required().unique()`。
#[derive(Debug, Clone)]
#[must_use = "字段定义必须传给 Table::fields"]
pub struct Field {
    config: FieldConfig,
    primary_key: bool,
    unique: bool,
    unique_name: Option<String>,
    index: bool,
    index_name: Option<String>,
    created_at: bool,
    updated_at: bool,
    deleted_at: bool,
    soft_delete: bool,
    relation_display_fields: Option<Vec<String>>,
}

impl Field {
    fn new(name: impl Into<String>, field_type: FieldType) -> Self {
        Self {
            config: FieldConfig::new(name, field_type),
            primary_key: false,
            unique: false,
            unique_name: None,
            index: false,
            index_name: None,
            created_at: false,
            updated_at: false,
            deleted_at: false,
            soft_delete: false,
            relation_display_fields: None,
        }
    }

    /// 创建 64 位自增主键字段。
    pub fn id(name: impl Into<String>) -> Self {
        Self::new(name, FieldType::BigInt)
            .required()
            .primary_key()
            .auto_increment()
            .not_writable()
    }

    /// 创建定长上限字符串字段。
    pub fn string(name: impl Into<String>, max_length: usize) -> Self {
        Self::new(name, FieldType::String { max_length })
    }

    /// 创建 32 位整数字段。
    pub fn integer(name: impl Into<String>) -> Self {
        Self::new(name, FieldType::Integer)
    }

    /// 创建 64 位整数字段。
    pub fn bigint(name: impl Into<String>) -> Self {
        Self::new(name, FieldType::BigInt)
    }

    /// 创建 32 位浮点数字段。
    pub fn float(name: impl Into<String>) -> Self {
        Self::new(name, FieldType::Float)
    }

    /// 创建 64 位浮点数字段。
    pub fn double(name: impl Into<String>) -> Self {
        Self::new(name, FieldType::Double)
    }

    /// 创建布尔字段。
    pub fn boolean(name: impl Into<String>) -> Self {
        Self::new(name, FieldType::Boolean)
    }

    /// 创建日期字段。
    pub fn date(name: impl Into<String>) -> Self {
        Self::new(name, FieldType::Date)
    }

    /// 创建日期时间字段。
    pub fn datetime(name: impl Into<String>) -> Self {
        Self::new(name, FieldType::DateTime)
    }

    /// 创建 Unix 时间戳字段。
    pub fn timestamp(name: impl Into<String>) -> Self {
        Self::new(name, FieldType::Timestamp)
    }

    /// 创建 JSON 字段。
    pub fn json(name: impl Into<String>) -> Self {
        Self::new(name, FieldType::Json)
    }

    /// 创建长文本字段。
    pub fn text(name: impl Into<String>) -> Self {
        Self::new(name, FieldType::Text)
    }

    /// 创建枚举字段。
    pub fn enumeration<I, S>(name: impl Into<String>, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::new(
            name,
            FieldType::Enum {
                values: values.into_iter().map(Into::into).collect(),
            },
        )
    }

    /// 创建自动写入的创建时间字段。
    pub fn created_at(name: impl Into<String>) -> Self {
        let mut field = Self::bigint(name).required().not_writable();
        field.created_at = true;
        field
    }

    /// 创建自动写入的更新时间字段。
    pub fn updated_at(name: impl Into<String>) -> Self {
        let mut field = Self::bigint(name).required().not_writable();
        field.updated_at = true;
        field
    }

    /// 创建软删除时间字段。
    pub fn soft_delete(name: impl Into<String>) -> Self {
        let mut field = Self::bigint(name).not_writable();
        field.deleted_at = true;
        field.soft_delete = true;
        field
    }

    /// 设置展示名称。
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.config.display_name = label.into();
        self
    }

    /// 设置为必填字段。
    pub fn required(mut self) -> Self {
        self.config.required = true;
        self
    }

    /// 设置为可空字段。
    pub fn nullable(mut self) -> Self {
        self.config.required = false;
        self
    }

    /// 设置数据库默认值。
    pub fn default(mut self, value: impl Into<Value>) -> Self {
        self.config.default_value = Some(value.into());
        self
    }

    /// 设置数据库自增。
    pub fn auto_increment(mut self) -> Self {
        self.config.auto_increment = true;
        self
    }

    /// 设置为主键。
    pub fn primary_key(mut self) -> Self {
        self.primary_key = true;
        self
    }

    /// 添加单字段唯一索引。
    pub fn unique(mut self) -> Self {
        self.unique = true;
        self
    }

    /// 添加指定名称的单字段唯一索引。
    pub fn unique_named(mut self, name: impl Into<String>) -> Self {
        self.unique = true;
        self.unique_name = Some(name.into());
        self
    }

    /// 添加单字段普通索引。
    pub fn index(mut self) -> Self {
        self.index = true;
        self
    }

    /// 添加指定名称的单字段普通索引。
    pub fn index_named(mut self, name: impl Into<String>) -> Self {
        self.index = true;
        self.index_name = Some(name.into());
        self
    }

    /// 添加底层验证器。
    pub fn validator(mut self, validator: Validator) -> Self {
        self.config.validators.push(validator);
        self
    }

    /// 设置字符串最小长度。
    pub fn min_length(self, value: usize) -> Self {
        self.validator(Validator::MinLength(value))
    }

    /// 设置字符串最大长度验证。
    pub fn max_length(self, value: usize) -> Self {
        self.validator(Validator::MaxLength(value))
    }

    /// 同时设置字符串最小、最大长度验证。
    pub fn length(self, range: std::ops::RangeInclusive<usize>) -> Self {
        self.min_length(*range.start()).max_length(*range.end())
    }

    /// 设置数值最小值验证。
    pub fn min(self, value: f64) -> Self {
        self.validator(Validator::Min(value))
    }

    /// 设置数值最大值验证。
    pub fn max(self, value: f64) -> Self {
        self.validator(Validator::Max(value))
    }

    /// 设置严格邮箱验证。
    pub fn email(self) -> Self {
        self.validator(Validator::Email)
    }

    /// 设置严格手机号验证。
    pub fn phone(self) -> Self {
        self.validator(Validator::Phone)
    }

    /// 设置 URL 验证。
    pub fn url(self) -> Self {
        self.validator(Validator::Url)
    }

    /// 设置正则表达式验证。
    pub fn regex(self, pattern: impl Into<String>) -> Self {
        self.validator(Validator::Regex(pattern.into()))
    }

    /// 允许所有角色读取。
    pub fn readable(mut self) -> Self {
        self.config.permissions.readable = Audience::Everyone;
        self
    }

    /// 禁止任何角色读取。
    pub fn not_readable(mut self) -> Self {
        self.config.permissions.readable = Audience::Nobody;
        self
    }

    /// 仅允许指定角色读取。
    pub fn readable_by<I, S>(mut self, roles: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.config.permissions.readable = Audience::roles(roles);
        self
    }

    /// 仅允许指定角色写入。
    pub fn writable_by<I, S>(mut self, roles: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.config.permissions.writable = Audience::roles(roles);
        self
    }

    /// 允许所有角色写入。
    pub fn writable(mut self) -> Self {
        self.config.permissions.writable = Audience::Everyone;
        self
    }

    /// 禁止任何角色写入。
    pub fn not_writable(mut self) -> Self {
        self.config.permissions.writable = Audience::Nobody;
        self
    }

    /// 允许字段用于筛选。
    pub fn filterable(mut self) -> Self {
        self.config.filterable = true;
        self.config.permissions.filterable = Audience::Everyone;
        self
    }

    /// 禁止字段用于筛选。
    pub fn not_filterable(mut self) -> Self {
        self.config.filterable = false;
        self.config.permissions.filterable = Audience::Nobody;
        self
    }

    /// 仅允许指定角色使用字段筛选。
    pub fn filterable_by<I, S>(mut self, roles: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.config.filterable = true;
        self.config.permissions.filterable = Audience::roles(roles);
        self
    }

    /// 允许字段用于排序。
    pub fn sortable(mut self) -> Self {
        self.config.sortable = true;
        self.config.permissions.sortable = Audience::Everyone;
        self
    }

    /// 禁止字段用于排序。
    pub fn not_sortable(mut self) -> Self {
        self.config.sortable = false;
        self.config.permissions.sortable = Audience::Nobody;
        self
    }

    /// 仅允许指定角色使用字段排序。
    pub fn sortable_by<I, S>(mut self, roles: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.config.sortable = true;
        self.config.permissions.sortable = Audience::roles(roles);
        self
    }

    /// 设置敏感字段预设。
    ///
    /// 敏感字段不会进入默认 Record 投影或表 JSON Schema，并默认禁止读取、写入、
    /// 筛选和排序。内部服务若确实需要显式读取或写入，必须随后调用
    /// [`Field::readable_by`] / [`Field::writable_by`] 指定专用角色和显式字段列表。
    pub fn secret(mut self) -> Self {
        self.config.hidden = true;
        self.config.permissions = FieldPermissions {
            readable: Audience::Nobody,
            writable: Audience::Nobody,
            filterable: Audience::Nobody,
            sortable: Audience::Nobody,
        };
        self.config.filterable = false;
        self.config.sortable = false;
        self
    }

    /// 设置关联关系。
    pub fn relation(
        mut self,
        table: impl Into<String>,
        field: impl Into<String>,
        relation_type: RelationType,
    ) -> Self {
        self.config.relation = Some(RelationConfig {
            table: table.into(),
            field: field.into(),
            display_fields: Vec::new(),
            relation_type,
        });
        self
    }

    /// 设置关联展示字段。
    pub fn relation_display_fields<I, S>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.relation_display_fields = Some(fields.into_iter().map(Into::into).collect());
        self
    }
}

#[derive(Debug, Clone)]
struct PendingIndex {
    name: Option<String>,
    fields: Vec<String>,
    unique: bool,
}

/// Schema-first 表构建器。
#[derive(Debug, Clone)]
#[must_use = "表构建器必须调用 build"]
pub struct Table {
    name: String,
    label: Option<String>,
    fields: Vec<Field>,
    indexes: Vec<PendingIndex>,
    default_order: Vec<Order>,
}

impl Table {
    /// 创建表定义构建器。
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            label: None,
            fields: Vec::new(),
            indexes: Vec::new(),
            default_order: Vec::new(),
        }
    }

    /// 设置表展示名称。
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// 批量添加字段；支持数组、`Vec` 和任意字段迭代器。
    pub fn fields<I>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = Field>,
    {
        self.fields.extend(fields);
        self
    }

    /// 添加复合唯一索引。
    pub fn unique<I, C>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = C>,
        C: Into<ColumnName>,
    {
        self.indexes.push(PendingIndex {
            name: None,
            fields: fields
                .into_iter()
                .map(|field| field.into().into_name())
                .collect(),
            unique: true,
        });
        self
    }

    /// 添加指定名称的复合唯一索引。
    pub fn unique_named<I, C>(mut self, name: impl Into<String>, fields: I) -> Self
    where
        I: IntoIterator<Item = C>,
        C: Into<ColumnName>,
    {
        self.indexes.push(PendingIndex {
            name: Some(name.into()),
            fields: fields
                .into_iter()
                .map(|field| field.into().into_name())
                .collect(),
            unique: true,
        });
        self
    }

    /// 添加复合普通索引。
    pub fn index<I, C>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = C>,
        C: Into<ColumnName>,
    {
        self.indexes.push(PendingIndex {
            name: None,
            fields: fields
                .into_iter()
                .map(|field| field.into().into_name())
                .collect(),
            unique: false,
        });
        self
    }

    /// 添加指定名称的复合普通索引。
    pub fn index_named<I, C>(mut self, name: impl Into<String>, fields: I) -> Self
    where
        I: IntoIterator<Item = C>,
        C: Into<ColumnName>,
    {
        self.indexes.push(PendingIndex {
            name: Some(name.into()),
            fields: fields
                .into_iter()
                .map(|field| field.into().into_name())
                .collect(),
            unique: false,
        });
        self
    }

    /// 设置首个默认排序规则。
    pub fn default_order(mut self, order: Order) -> Self {
        self.default_order = vec![order];
        self
    }

    /// 追加默认排序规则。
    pub fn then_order(mut self, order: Order) -> Self {
        self.default_order.push(order);
        self
    }

    /// 校验并生成不可变表定义。
    pub fn build(self) -> Result<TableDefinition, BaseError> {
        validate_identifier("表", &self.name)?;
        if self.fields.is_empty() {
            return Err(BaseError::ConfigError(format!(
                "表 {} 至少需要一个字段",
                self.name
            )));
        }

        let mut field_names = HashSet::with_capacity(self.fields.len());
        let mut configs = HashMap::with_capacity(self.fields.len());
        let mut primary_key: Option<String> = None;
        let mut created_at: Option<String> = None;
        let mut updated_at: Option<String> = None;
        let mut deleted_at: Option<String> = None;
        let mut soft_delete: Option<String> = None;
        let mut indexes = self.indexes;

        for mut field in self.fields {
            validate_identifier("字段", &field.config.name)?;
            if !field_names.insert(field.config.name.clone()) {
                return Err(BaseError::ConfigError(format!(
                    "表 {} 存在重复字段: {}",
                    self.name, field.config.name
                )));
            }
            if field.config.display_name.is_empty() {
                field.config.display_name = field.config.name.clone();
            }
            if let Some(display_fields) = field.relation_display_fields.take() {
                let relation = field.config.relation.as_mut().ok_or_else(|| {
                    BaseError::ConfigError(format!(
                        "表 {} 的字段 {} 未定义 relation，不能配置关联展示字段",
                        self.name, field.config.name
                    ))
                })?;
                relation.display_fields = display_fields;
            }
            validate_field_shape(&self.name, &field)?;

            if field.primary_key && primary_key.replace(field.config.name.clone()).is_some() {
                return Err(BaseError::ConfigError(format!(
                    "表 {} 只能定义一个主键",
                    self.name
                )));
            }
            set_unique_role(
                &self.name,
                "created_at",
                field.created_at,
                &field.config.name,
                &mut created_at,
            )?;
            set_unique_role(
                &self.name,
                "updated_at",
                field.updated_at,
                &field.config.name,
                &mut updated_at,
            )?;
            set_unique_role(
                &self.name,
                "deleted_at",
                field.deleted_at,
                &field.config.name,
                &mut deleted_at,
            )?;
            set_unique_role(
                &self.name,
                "soft_delete",
                field.soft_delete,
                &field.config.name,
                &mut soft_delete,
            )?;

            if field.unique {
                indexes.push(PendingIndex {
                    name: field.unique_name.take(),
                    fields: vec![field.config.name.clone()],
                    unique: true,
                });
            }
            if field.index {
                indexes.push(PendingIndex {
                    name: field.index_name.take(),
                    fields: vec![field.config.name.clone()],
                    unique: false,
                });
            }
            configs.insert(field.config.name.clone(), field.config);
        }

        let primary_key = primary_key
            .ok_or_else(|| BaseError::ConfigError(format!("表 {} 必须定义一个主键", self.name)))?;

        let mut unique_indexes = Vec::new();
        let mut normal_indexes = Vec::new();
        let mut index_names = HashSet::new();
        for index in indexes {
            validate_index(&self.name, &field_names, &index, &mut index_names)?;
            let config = IndexConfig::new(index.name, index.fields);
            if index.unique {
                unique_indexes.push(config);
            } else {
                normal_indexes.push(config);
            }
        }

        let mut orders = Vec::with_capacity(self.default_order.len());
        for order in self.default_order {
            if !field_names.contains(&order.field) {
                return Err(BaseError::ConfigError(format!(
                    "表 {} 的默认排序字段不存在: {}",
                    self.name, order.field
                )));
            }
            orders.push((order.field, order.direction));
        }

        let timestamp_fields =
            if created_at.is_some() || updated_at.is_some() || deleted_at.is_some() {
                Some(TimestampFields::new(created_at, updated_at, deleted_at))
            } else {
                None
            };

        Ok(TableDefinition {
            config: Arc::new(TableConfig {
                table_name: self.name.clone(),
                display_name: self.label.unwrap_or(self.name),
                primary_key,
                fields: configs,
                unique_indexes,
                indexes: normal_indexes,
                default_order: orders,
                soft_delete_field: soft_delete,
                timestamp_fields,
            }),
        })
    }
}

/// 不可变的数据表定义。
#[derive(Debug, Clone)]
pub struct TableDefinition {
    pub(crate) config: Arc<TableConfig>,
}

impl TableDefinition {
    /// 返回数据库表名。
    pub fn name(&self) -> &str {
        &self.config.table_name
    }

    /// 返回展示名称。
    pub fn label(&self) -> &str {
        &self.config.display_name
    }

    /// 返回主键字段名。
    pub fn primary_key(&self) -> &str {
        &self.config.primary_key
    }

    /// 返回字段数量。
    pub fn field_count(&self) -> usize {
        self.config.fields.len()
    }

    /// 按数据库字段名读取元数据。
    pub fn field(&self, name: &str) -> Option<FieldMetadata<'_>> {
        self.config
            .fields
            .get(name)
            .map(|config| FieldMetadata { config })
    }

    /// 按字段名排序返回全部字段元数据。
    pub fn fields(&self) -> Vec<FieldMetadata<'_>> {
        let mut fields: Vec<_> = self
            .config
            .fields
            .values()
            .map(|config| FieldMetadata { config })
            .collect();
        fields.sort_by(|left, right| left.name().cmp(right.name()));
        fields
    }

    /// 返回软删除字段名。
    pub fn soft_delete_field(&self) -> Option<&str> {
        self.config.soft_delete_field.as_deref()
    }

    /// 生成供 API catalog 使用的全局写入侧 JSON Schema。
    ///
    /// 该 schema 是所有角色可写字段的并集；面向当前请求返回结构时应使用
    /// `Self::input_schema_for_roles`。
    pub fn input_schema(&self) -> Value {
        self.json_schema(false, None)
    }

    /// 生成供 API catalog 使用的全局读取侧 JSON Schema。
    ///
    /// 该 schema 是所有角色可读字段的并集；面向当前请求返回结构时应使用
    /// `Self::output_schema_for_roles`。
    pub fn output_schema(&self) -> Value {
        self.json_schema(true, None)
    }

    /// 按当前用户角色生成写入侧 JSON Schema。
    pub fn input_schema_for_roles(&self, roles: &HashSet<String>) -> Value {
        self.json_schema(false, Some(roles))
    }

    /// 按当前用户角色生成读取侧 JSON Schema。
    pub fn output_schema_for_roles(&self, roles: &HashSet<String>) -> Value {
        self.json_schema(true, Some(roles))
    }

    /// 返回指定数据库字段的底层 JSON Schema，供内置 API 描述投影使用。
    #[cfg(feature = "mysql")]
    pub(crate) fn field_schema(&self, name: &str) -> Option<Value> {
        self.config.fields.get(name).map(field_json_schema)
    }

    fn json_schema(&self, output: bool, roles: Option<&HashSet<String>>) -> Value {
        let mut properties = Map::new();
        let mut required = Vec::new();
        let mut names: Vec<_> = self.config.fields.keys().collect();
        names.sort();
        for name in names {
            let field = &self.config.fields[name];
            let generated = field.auto_increment
                || self
                    .config
                    .timestamp_fields
                    .as_ref()
                    .is_some_and(|timestamps| {
                        timestamps.created_at.as_deref() == Some(name.as_str())
                            || timestamps.updated_at.as_deref() == Some(name.as_str())
                            || timestamps.deleted_at.as_deref() == Some(name.as_str())
                    })
                || self.config.soft_delete_field.as_deref() == Some(name.as_str());
            let audience = if output {
                &field.permissions.readable
            } else {
                &field.permissions.writable
            };
            let audience_allows = roles.map_or_else(
                || !matches!(audience, Audience::Nobody),
                |roles| audience.allows(roles),
            );
            if field.hidden || !audience_allows || (!output && generated) {
                continue;
            }
            properties.insert(name.clone(), field_json_schema(field));
            if field.required && (output || (!generated && field.default_value.is_none())) {
                required.push(Value::String(name.clone()));
            }
        }
        json!({
            "type": "object",
            "title": self.config.display_name,
            "properties": properties,
            "required": required,
            "additionalProperties": false,
        })
    }

    #[cfg(feature = "mysql")]
    pub(crate) fn config(&self) -> &TableConfig {
        &self.config
    }

    pub(crate) fn shared_config(&self) -> Arc<TableConfig> {
        Arc::clone(&self.config)
    }

    /// 校验实际数据库列是否兼容当前表定义。
    pub fn validate_schema(
        &self,
        columns: &[super::SchemaColumn],
    ) -> super::SchemaValidationReport {
        self.config.validate_schema(columns)
    }

    /// 将表定义绑定到 MySQL 连接池。
    #[cfg(feature = "mysql")]
    pub fn bind(&self, pool: Arc<sqlx::MySqlPool>) -> TableHandle {
        TableHandle {
            definition: self.clone(),
            pool,
        }
    }
}

/// 只读字段元数据视图。
#[derive(Debug, Clone, Copy)]
pub struct FieldMetadata<'a> {
    config: &'a FieldConfig,
}

impl<'a> FieldMetadata<'a> {
    /// 返回数据库字段名。
    pub fn name(self) -> &'a str {
        &self.config.name
    }

    /// 返回展示名称。
    pub fn label(self) -> &'a str {
        &self.config.display_name
    }

    /// 返回字段类型。
    pub fn field_type(self) -> &'a FieldType {
        &self.config.field_type
    }

    /// 返回字段是否必填。
    pub fn is_required(self) -> bool {
        self.config.required
    }

    /// 返回字段默认值。
    pub fn default_value(self) -> Option<&'a Value> {
        self.config.default_value.as_ref()
    }

    /// 返回字段是否由数据库自增生成。
    pub fn is_auto_increment(self) -> bool {
        self.config.auto_increment
    }

    /// 返回字段是否允许筛选。
    pub fn is_filterable(self) -> bool {
        self.config.filterable
    }

    /// 返回字段是否允许排序。
    pub fn is_sortable(self) -> bool {
        self.config.sortable
    }

    /// 返回字段是否从通用记录投影和表 JSON Schema 中隐藏。
    pub fn is_secret(self) -> bool {
        self.config.hidden
    }
}

/// 绑定数据库连接池后的表句柄。
#[cfg(feature = "mysql")]
#[derive(Debug, Clone)]
pub struct TableHandle {
    definition: TableDefinition,
    pool: Arc<sqlx::MySqlPool>,
}

#[cfg(feature = "mysql")]
impl TableHandle {
    /// 返回不可变表定义。
    pub fn definition(&self) -> &TableDefinition {
        &self.definition
    }

    /// 创建带字段权限保护的查询构建器。
    pub fn query<I, S>(&self, roles: I) -> TableQuery
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let roles: Arc<[String]> =
            Arc::from(roles.into_iter().map(Into::into).collect::<Vec<String>>());
        TableQuery::new(
            self.definition.shared_config(),
            roles,
            Some(Arc::clone(&self.pool)),
        )
    }
}

fn validate_identifier(kind: &str, value: &str) -> Result<(), BaseError> {
    let mut chars = value.chars();
    let first = chars
        .next()
        .ok_or_else(|| BaseError::ConfigError(format!("{kind}名称不能为空")))?;
    if value.len() > 64
        || !(first == '_' || first.is_ascii_alphabetic())
        || !chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
    {
        return Err(BaseError::ConfigError(format!("非法{kind}名称: {value}")));
    }
    Ok(())
}

fn validate_field_shape(table: &str, field: &Field) -> Result<(), BaseError> {
    match &field.config.field_type {
        FieldType::String { max_length } if *max_length == 0 => {
            return Err(BaseError::ConfigError(format!(
                "表 {table} 的字符串字段 {} 长度必须大于 0",
                field.config.name
            )));
        }
        FieldType::Enum { values } => {
            let unique: HashSet<_> = values.iter().collect();
            if values.is_empty() || unique.len() != values.len() {
                return Err(BaseError::ConfigError(format!(
                    "表 {table} 的枚举字段 {} 必须提供非空且不重复的候选值",
                    field.config.name
                )));
            }
        }
        _ => {}
    }

    if field.config.auto_increment
        && (!field.primary_key
            || !matches!(
                field.config.field_type,
                FieldType::Integer | FieldType::BigInt
            ))
    {
        return Err(BaseError::ConfigError(format!(
            "表 {table} 的 auto_increment 仅允许用于整数主键字段: {}",
            field.config.name
        )));
    }
    if field.primary_key && !field.config.required {
        return Err(BaseError::ConfigError(format!(
            "表 {table} 的主键必须为必填字段: {}",
            field.config.name
        )));
    }
    if field.config.auto_increment && field.config.default_value.is_some() {
        return Err(BaseError::ConfigError(format!(
            "表 {table} 的自增字段不能配置默认值: {}",
            field.config.name
        )));
    }
    validate_validator_configuration(table, &field.config)?;
    if let Some(default) = &field.config.default_value {
        if matches!(field.config.field_type, FieldType::Text | FieldType::Json) {
            return Err(BaseError::ConfigError(format!(
                "表 {table} 的 TEXT/JSON 字段不能配置数据库默认值: {}",
                field.config.name
            )));
        }
        field.config.validate(default)?;
    }
    validate_audience(
        table,
        &field.config.name,
        "读取",
        &field.config.permissions.readable,
    )?;
    validate_audience(
        table,
        &field.config.name,
        "写入",
        &field.config.permissions.writable,
    )?;
    validate_audience(
        table,
        &field.config.name,
        "筛选",
        &field.config.permissions.filterable,
    )?;
    validate_audience(
        table,
        &field.config.name,
        "排序",
        &field.config.permissions.sortable,
    )?;
    if let Some(relation) = &field.config.relation {
        validate_identifier("关联表", &relation.table)?;
        validate_identifier("关联字段", &relation.field)?;
        for display_field in &relation.display_fields {
            validate_identifier("关联展示字段", display_field)?;
        }
    }
    if (field.created_at || field.updated_at || field.deleted_at || field.soft_delete)
        && !matches!(
            field.config.field_type,
            FieldType::Integer | FieldType::BigInt | FieldType::Timestamp
        )
    {
        return Err(BaseError::ConfigError(format!(
            "表 {table} 的时间戳语义字段必须是整数或 Timestamp: {}",
            field.config.name
        )));
    }
    Ok(())
}

fn validate_validator_configuration(table: &str, field: &FieldConfig) -> Result<(), BaseError> {
    let string_compatible = matches!(
        field.field_type,
        FieldType::String { .. }
            | FieldType::Text
            | FieldType::Enum { .. }
            | FieldType::Date
            | FieldType::DateTime
    );
    let numeric_compatible = matches!(
        field.field_type,
        FieldType::Integer
            | FieldType::BigInt
            | FieldType::Float
            | FieldType::Double
            | FieldType::Timestamp
    );
    let mut min_length: Option<usize> = None;
    let mut max_length: Option<usize> = None;
    let mut minimum: Option<f64> = None;
    let mut maximum: Option<f64> = None;

    for validator in &field.validators {
        match validator {
            Validator::MinLength(value) => {
                ensure_validator_type(table, field, validator, string_compatible)?;
                min_length = Some(min_length.map_or(*value, |current| current.max(*value)));
            }
            Validator::MaxLength(value) => {
                ensure_validator_type(table, field, validator, string_compatible)?;
                max_length = Some(max_length.map_or(*value, |current| current.min(*value)));
            }
            Validator::Min(value) => {
                ensure_validator_type(table, field, validator, numeric_compatible)?;
                if !value.is_finite() {
                    return Err(BaseError::ConfigError(format!(
                        "表 {table} 的字段 {} 数值下限必须是有限数值",
                        field.name
                    )));
                }
                minimum = Some(minimum.map_or(*value, |current| current.max(*value)));
            }
            Validator::Max(value) => {
                ensure_validator_type(table, field, validator, numeric_compatible)?;
                if !value.is_finite() {
                    return Err(BaseError::ConfigError(format!(
                        "表 {table} 的字段 {} 数值上限必须是有限数值",
                        field.name
                    )));
                }
                maximum = Some(maximum.map_or(*value, |current| current.min(*value)));
            }
            Validator::Email
            | Validator::EmailLoose
            | Validator::Phone
            | Validator::PhoneLoose
            | Validator::Url => {
                ensure_validator_type(table, field, validator, string_compatible)?;
            }
            Validator::Regex(pattern) => {
                ensure_validator_type(table, field, validator, string_compatible)?;
                #[cfg(feature = "validator")]
                regex::Regex::new(pattern).map_err(|error| {
                    BaseError::ConfigError(format!(
                        "表 {table} 的字段 {} 正则表达式无效: {error}",
                        field.name
                    ))
                })?;
                #[cfg(not(feature = "validator"))]
                return Err(BaseError::ConfigError(format!(
                    "表 {table} 的字段 {} 使用正则验证器 {pattern:?} 时必须启用 validator feature",
                    field.name,
                )));
            }
            Validator::Custom(_) => {}
        }
    }

    if let (Some(min), Some(max)) = (min_length, max_length) {
        if min > max {
            return Err(BaseError::ConfigError(format!(
                "表 {table} 的字段 {} 最小长度不能大于最大长度",
                field.name
            )));
        }
    }
    if let (
        FieldType::String {
            max_length: storage_max,
        },
        Some(min),
    ) = (&field.field_type, min_length)
    {
        if min > *storage_max {
            return Err(BaseError::ConfigError(format!(
                "表 {table} 的字段 {} 最小长度不能超过字符串存储上限 {storage_max}",
                field.name
            )));
        }
    }
    if let (Some(min), Some(max)) = (minimum, maximum) {
        if min > max {
            return Err(BaseError::ConfigError(format!(
                "表 {table} 的字段 {} 数值下限不能大于上限",
                field.name
            )));
        }
    }
    Ok(())
}

fn ensure_validator_type(
    table: &str,
    field: &FieldConfig,
    validator: &Validator,
    compatible: bool,
) -> Result<(), BaseError> {
    if compatible {
        return Ok(());
    }
    Err(BaseError::ConfigError(format!(
        "表 {table} 的字段 {} 类型不能使用{}验证器",
        field.name,
        validator.display_name()
    )))
}

fn validate_audience(
    table: &str,
    field: &str,
    operation: &str,
    audience: &Audience,
) -> Result<(), BaseError> {
    if let Audience::Roles(roles) = audience {
        if roles.is_empty() || roles.iter().any(|role| role.trim().is_empty()) {
            return Err(BaseError::ConfigError(format!(
                "表 {table} 的字段 {field} 的{operation}角色不能为空"
            )));
        }
    }
    Ok(())
}

fn set_unique_role(
    table: &str,
    role: &str,
    enabled: bool,
    field: &str,
    target: &mut Option<String>,
) -> Result<(), BaseError> {
    if !enabled {
        return Ok(());
    }
    if let Some(existing) = target {
        return Err(BaseError::ConfigError(format!(
            "表 {table} 的 {role} 语义重复定义在 {existing} 和 {field}"
        )));
    }
    *target = Some(field.to_string());
    Ok(())
}

fn validate_index(
    table: &str,
    field_names: &HashSet<String>,
    index: &PendingIndex,
    index_names: &mut HashSet<String>,
) -> Result<(), BaseError> {
    if index.fields.is_empty() {
        return Err(BaseError::ConfigError(format!(
            "表 {table} 的索引字段不能为空"
        )));
    }
    let mut seen = HashSet::new();
    for field in &index.fields {
        if !field_names.contains(field) {
            return Err(BaseError::ConfigError(format!(
                "表 {table} 的索引字段不存在: {field}"
            )));
        }
        if !seen.insert(field) {
            return Err(BaseError::ConfigError(format!(
                "表 {table} 的索引重复引用字段: {field}"
            )));
        }
    }
    let prefix = if index.unique { "uk" } else { "idx" };
    let effective_name = index
        .name
        .clone()
        .unwrap_or_else(|| format!("{prefix}_{table}_{}", index.fields.join("_")));
    validate_identifier("索引", &effective_name)?;
    if !index_names.insert(effective_name.clone()) {
        return Err(BaseError::ConfigError(format!(
            "表 {table} 存在重复索引名: {effective_name}"
        )));
    }
    Ok(())
}

fn field_json_schema(field: &FieldConfig) -> Value {
    let mut schema = Map::new();
    let mut min_length: Option<usize> = None;
    let mut max_length: Option<usize> = None;
    let mut minimum: Option<f64> = None;
    let mut maximum: Option<f64> = None;
    let mut patterns = Vec::new();
    let mut runtime_validators = Vec::new();
    match &field.field_type {
        FieldType::String {
            max_length: storage_max,
        } => {
            schema.insert("type".into(), json!("string"));
            max_length = Some(*storage_max);
        }
        FieldType::Text => {
            schema.insert("type".into(), json!("string"));
        }
        FieldType::Integer | FieldType::BigInt | FieldType::Timestamp => {
            schema.insert("type".into(), json!("integer"));
        }
        FieldType::Float | FieldType::Double => {
            schema.insert("type".into(), json!("number"));
        }
        FieldType::Boolean => {
            schema.insert("type".into(), json!("boolean"));
        }
        FieldType::Date => {
            schema.insert("type".into(), json!("string"));
            schema.insert("format".into(), json!("date"));
        }
        FieldType::DateTime => {
            schema.insert("type".into(), json!("string"));
            schema.insert("format".into(), json!("date-time"));
        }
        FieldType::Json => {
            schema.insert("type".into(), json!(["object", "array"]));
        }
        FieldType::Enum { values } => {
            schema.insert("type".into(), json!("string"));
            schema.insert("enum".into(), json!(values));
        }
    }

    for validator in &field.validators {
        match validator {
            Validator::MinLength(value) => {
                min_length = Some(min_length.map_or(*value, |current| current.max(*value)));
            }
            Validator::MaxLength(value) => {
                max_length = Some(max_length.map_or(*value, |current| current.min(*value)));
            }
            Validator::Min(value) => {
                minimum = Some(minimum.map_or(*value, |current| current.max(*value)));
            }
            Validator::Max(value) => {
                maximum = Some(maximum.map_or(*value, |current| current.min(*value)));
            }
            Validator::Email => {
                #[cfg(feature = "validator")]
                patterns.push(r"^[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}$".to_string());
                #[cfg(not(feature = "validator"))]
                patterns.push("@".to_string());
            }
            Validator::EmailLoose => patterns.push("@".to_string()),
            Validator::Phone => {
                #[cfg(feature = "validator")]
                patterns.push(r"^\+?[1-9]\d{1,14}$".to_string());
                #[cfg(not(feature = "validator"))]
                patterns.push(r"^[0-9+\-]*$".to_string());
            }
            Validator::PhoneLoose => patterns.push(r"^[0-9\-]*$".to_string()),
            Validator::Url => patterns.push(r"^https?://".to_string()),
            Validator::Regex(pattern) => patterns.push(pattern.clone()),
            Validator::Custom(_) => runtime_validators.push("custom"),
        }
    }

    if let Some(value) = min_length {
        schema.insert("minLength".into(), json!(value));
    }
    if let Some(value) = max_length {
        schema.insert("maxLength".into(), json!(value));
    }
    if let Some(value) = minimum {
        schema.insert("minimum".into(), json!(value));
    }
    if let Some(value) = maximum {
        schema.insert("maximum".into(), json!(value));
    }
    match patterns.as_slice() {
        [] => {}
        [pattern] => {
            schema.insert("pattern".into(), json!(pattern));
        }
        patterns => {
            schema.insert(
                "allOf".into(),
                Value::Array(
                    patterns
                        .iter()
                        .map(|pattern| json!({ "pattern": pattern }))
                        .collect(),
                ),
            );
        }
    }
    if !runtime_validators.is_empty() {
        schema.insert(
            "x-yang-runtime-validators".into(),
            json!(runtime_validators),
        );
    }
    if let Some(relation) = &field.relation {
        let relation_type = match relation.relation_type {
            RelationType::OneToOne => "one_to_one",
            RelationType::OneToMany => "one_to_many",
            RelationType::ManyToOne => "many_to_one",
            RelationType::ManyToMany => "many_to_many",
        };
        schema.insert(
            "x-yang-relation".into(),
            json!({
                "table": relation.table,
                "field": relation.field,
                "displayFields": relation.display_fields,
                "type": relation_type,
            }),
        );
    }

    if !field.required {
        if let Some(field_types) = schema.get_mut("type") {
            match field_types {
                Value::String(field_type) => {
                    *field_types = json!([field_type.clone(), "null"]);
                }
                Value::Array(types) if !types.iter().any(|value| value == "null") => {
                    types.push(json!("null"));
                }
                _ => {}
            }
        }
        if let Some(Value::Array(values)) = schema.get_mut("enum") {
            values.push(Value::Null);
        }
    }
    if !field.display_name.is_empty() {
        schema.insert("title".into(), json!(field.display_name));
    }
    if let Some(default) = &field.default_value {
        schema.insert("default".into(), default.clone());
    }
    Value::Object(schema)
}
