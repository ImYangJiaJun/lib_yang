//! Schema-first 数据表定义。
//!
//! [`Table`] 与 [`Field`] 是应用侧唯一需要使用的构建器。字段名、数据库类型、
//! 验证、权限、索引和时间戳语义在同一处声明，最终由 [`Table::build`] 一次校验并
//! 生成不可变 [`TableDefinition`]。

use super::field_config::{Audience, FieldConfig, FieldPermissions, RelationConfig, RelationType};
use super::table_config::{
    CheckConfig, ForeignKeyConfig, IndexConfig, OpaqueColumnConfig, TableConfig, TimestampFields,
};
#[cfg(any(feature = "mysql", feature = "postgres"))]
use super::TableQuery;
use super::{FieldType, RegexCache, SortOrder, Validator, MAX_VARCHAR_LENGTH};
use crate::datasource::DatasourceId;
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

    /// 声明当前字段由一个旧列原位改名而来；同步器不会复制或删除数据。
    pub fn renamed_from(mut self, legacy_name: impl Into<String>) -> Self {
        self.config.renamed_from = Some(legacy_name.into());
        self
    }

    /// 声明 PostgreSQL 生成列（`GENERATED ALWAYS AS (<expr>) STORED`）。
    ///
    /// 表达式属于受信任的应用定义，不得来自请求或运行时配置；生成列同时会
    /// 被标记为不可写，避免内置 CRUD 尝试写入数据库自算的值。
    pub fn generated(mut self, expression: impl Into<String>) -> Self {
        self.config.generated = Some(expression.into());
        self.config.permissions.writable = Audience::Nobody;
        self
    }

    /// 设置数据库列注释（PostgreSQL 通过 `COMMENT ON COLUMN` 同步）。
    pub fn comment(mut self, comment: impl Into<String>) -> Self {
        self.config.comment = Some(comment.into());
        self
    }

    /// 设置列排序规则（PostgreSQL `COLLATE`）。
    pub fn collation(mut self, collation: impl Into<String>) -> Self {
        self.config.collation = Some(collation.into());
        self
    }

    /// 声明复用 PostgreSQL 原生枚举类型（如 `order_status`）。
    ///
    /// 变体值仍由 [`Field::enumeration`] 声明；同步器会创建缺失的类型并补齐
    /// 缺失的枚举值。MySQL 方言在渲染 DDL 时明确拒绝该声明。
    pub fn native_enum(mut self, type_name: impl Into<String>) -> Self {
        self.config.native_enum = Some(type_name.into());
        self
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

    /// 创建定点小数字段。
    pub fn decimal(name: impl Into<String>, precision: u8, scale: u8) -> Self {
        Self::new(name, FieldType::Decimal { precision, scale })
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

    /// 创建 UUID 字段（MySQL 渲染为 `CHAR(36)`，PostgreSQL 为 `uuid`）。
    pub fn uuid(name: impl Into<String>) -> Self {
        Self::new(name, FieldType::Uuid)
    }

    /// 创建二进制字段（运行期 Base64 字符串；MySQL `BLOB`，PostgreSQL `BYTEA`）。
    pub fn bytes(name: impl Into<String>) -> Self {
        Self::new(name, FieldType::Bytes)
    }

    /// 创建标量元素数组字段（仅 PostgreSQL 支持；MySQL 渲染时明确拒绝）。
    pub fn array(name: impl Into<String>, element: FieldType) -> Self {
        Self::new(name, FieldType::Array(Box::new(element)))
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

    /// 允许字段用于关键词搜索（OR LIKE）。
    ///
    /// 与结构化筛选的 [`Field::filterable`] 位相互独立；只有文本字段
    /// （`Field::string` / `Field::text`）会实际参与搜索，定义层在构建期
    /// 会对非文本字段的 searchable 声明报错。
    pub fn searchable(mut self) -> Self {
        self.config.searchable = true;
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

    /// 将字段标记为租户隔离键。
    pub fn tenant_key(mut self) -> Self {
        self.config.tenant_key = true;
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
        self.config.searchable = false;
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
    expressions: Vec<String>,
    predicate: Option<String>,
    unique: bool,
    nulls_not_distinct: bool,
}

#[derive(Debug, Clone)]
struct PendingCheck {
    name: String,
    expression: String,
}

#[derive(Debug, Clone)]
struct PendingForeignKey {
    name: String,
    columns: Vec<String>,
    referenced_table: String,
    referenced_columns: Vec<String>,
}

/// Schema-first 表构建器。
#[derive(Debug, Clone)]
#[must_use = "表构建器必须调用 build"]
pub struct Table {
    name: String,
    label: Option<String>,
    datasource: Option<DatasourceId>,
    primary_key_order: Vec<String>,
    fields: Vec<Field>,
    opaque_columns: Vec<OpaqueColumnConfig>,
    indexes: Vec<PendingIndex>,
    checks: Vec<PendingCheck>,
    foreign_keys: Vec<PendingForeignKey>,
    default_order: Vec<Order>,
}

impl Table {
    /// 创建表定义构建器。
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            label: None,
            datasource: None,
            primary_key_order: Vec::new(),
            fields: Vec::new(),
            opaque_columns: Vec::new(),
            indexes: Vec::new(),
            checks: Vec::new(),
            foreign_keys: Vec::new(),
            default_order: Vec::new(),
        }
    }

    /// 设置表展示名称。
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// 设置表所属逻辑数据源（默认 `default`）。
    pub fn datasource(mut self, id: DatasourceId) -> Self {
        self.datasource = Some(id);
        self
    }

    /// 覆盖复合主键的列顺序（用于抵消字段排序）。
    pub fn primary_key_order(mut self, order: Vec<String>) -> Self {
        self.primary_key_order = order;
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

    /// 声明一个框架不建模的存量列（仅 PostgreSQL）。
    ///
    /// 该列必须已存在且 `format_type` 与 `sql_type` 一致；框架不改写、不读取、
    /// 不写入，也不投影到通用 CRUD/UI。用于 `inet`、`citext`、自定义 domain 等
    /// 无法安全映射为通用字段类型的列。
    pub fn opaque_column(mut self, name: impl Into<String>, sql_type: impl Into<String>) -> Self {
        self.opaque_columns.push(OpaqueColumnConfig {
            name: name.into(),
            sql_type: sql_type.into(),
        });
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
            expressions: Vec::new(),
            predicate: None,
            unique: true,
            nulls_not_distinct: false,
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
            expressions: Vec::new(),
            predicate: None,
            unique: true,
            nulls_not_distinct: false,
        });
        self
    }

    /// 添加复合唯一索引，并声明 PostgreSQL 15+ 的 `NULLS NOT DISTINCT` 语义。
    ///
    /// 空值互不异：两行该索引列均为 NULL 时会触发唯一冲突。仅 PostgreSQL 支持；
    /// MySQL 在 schema 同步时明确拒绝该标志。
    pub fn unique_nulls_not_distinct<I, C>(mut self, fields: I) -> Self
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
            expressions: Vec::new(),
            predicate: None,
            unique: true,
            nulls_not_distinct: true,
        });
        self
    }

    /// 添加指定名称的复合唯一索引，并声明 `NULLS NOT DISTINCT` 语义。
    pub fn unique_named_nulls_not_distinct<I, C>(
        mut self,
        name: impl Into<String>,
        fields: I,
    ) -> Self
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
            expressions: Vec::new(),
            predicate: None,
            unique: true,
            nulls_not_distinct: true,
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
            expressions: Vec::new(),
            predicate: None,
            unique: false,
            nulls_not_distinct: false,
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
            expressions: Vec::new(),
            predicate: None,
            unique: false,
            nulls_not_distinct: false,
        });
        self
    }

    /// 添加带受信表达式键与/或部分索引谓词的索引（仅 PostgreSQL 支持）。
    ///
    /// `fields` 与 `expressions` 至少一个非空；`predicate` 是 `WHERE` 后原样
    /// 渲染的受信 SQL 表达式。MySQL 在 schema 同步时明确拒绝部分/表达式索引。
    pub fn index_spec<I, C, S>(
        mut self,
        name: Option<String>,
        fields: I,
        expressions: impl IntoIterator<Item = S>,
        predicate: Option<String>,
        unique: bool,
        nulls_not_distinct: bool,
    ) -> Self
    where
        I: IntoIterator<Item = C>,
        C: Into<ColumnName>,
        S: Into<String>,
    {
        self.indexes.push(PendingIndex {
            name,
            fields: fields
                .into_iter()
                .map(|field| field.into().into_name())
                .collect(),
            expressions: expressions.into_iter().map(Into::into).collect(),
            predicate,
            unique,
            nulls_not_distinct,
        });
        self
    }

    /// 添加指定名称的 MySQL CHECK 约束。
    ///
    /// 表达式属于受信任的应用定义，不得来自请求或运行时配置。
    pub fn check_named(mut self, name: impl Into<String>, expression: impl Into<String>) -> Self {
        self.checks.push(PendingCheck {
            name: name.into(),
            expression: expression.into(),
        });
        self
    }

    /// 添加指定名称的复合外键；更新和删除规则固定为 `RESTRICT`。
    pub fn foreign_key_named<I, C, R, RC>(
        mut self,
        name: impl Into<String>,
        columns: I,
        referenced_table: impl Into<String>,
        referenced_columns: R,
    ) -> Self
    where
        I: IntoIterator<Item = C>,
        C: Into<ColumnName>,
        R: IntoIterator<Item = RC>,
        RC: Into<ColumnName>,
    {
        self.foreign_keys.push(PendingForeignKey {
            name: name.into(),
            columns: columns
                .into_iter()
                .map(|column| column.into().into_name())
                .collect(),
            referenced_table: referenced_table.into(),
            referenced_columns: referenced_columns
                .into_iter()
                .map(|column| column.into().into_name())
                .collect(),
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
        let mut legacy_names = HashSet::new();
        let mut configs = HashMap::with_capacity(self.fields.len());
        let mut primary_keys: Vec<String> = Vec::new();
        let mut created_at: Option<String> = None;
        let mut updated_at: Option<String> = None;
        let mut deleted_at: Option<String> = None;
        let mut soft_delete: Option<String> = None;
        let mut tenant_key: Option<String> = None;
        let mut indexes = self.indexes;
        let checks = self.checks;
        let foreign_keys = self.foreign_keys;
        // 启动期默认值校验复用同一个有界正则缓存（每表构建一次，构建完即丢弃）。
        let regex_cache = RegexCache::default();

        for mut field in self.fields {
            validate_identifier("字段", &field.config.name)?;
            if !field_names.insert(field.config.name.clone()) {
                return Err(BaseError::ConfigError(format!(
                    "表 {} 存在重复字段: {}",
                    self.name, field.config.name
                )));
            }
            if let Some(legacy_name) = field.config.renamed_from.as_deref() {
                validate_identifier("旧字段", legacy_name)?;
                if legacy_name == field.config.name || !legacy_names.insert(legacy_name.to_string())
                {
                    return Err(BaseError::ConfigError(format!(
                        "表 {} 的旧字段改名来源重复或未发生改名: {}",
                        self.name, legacy_name
                    )));
                }
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
            validate_field_shape(&self.name, &field, &regex_cache)?;

            if field.primary_key {
                primary_keys.push(field.config.name.clone());
            }
            if field.config.tenant_key && tenant_key.replace(field.config.name.clone()).is_some() {
                return Err(BaseError::ConfigError(format!(
                    "表 {} 只能定义一个租户隔离字段",
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
                    expressions: Vec::new(),
                    predicate: None,
                    unique: true,
                    nulls_not_distinct: false,
                });
            }
            if field.index {
                indexes.push(PendingIndex {
                    name: field.index_name.take(),
                    fields: vec![field.config.name.clone()],
                    expressions: Vec::new(),
                    predicate: None,
                    unique: false,
                    nulls_not_distinct: false,
                });
            }
            configs.insert(field.config.name.clone(), field.config);
        }

        let mut opaque_names = HashSet::new();
        for opaque in &self.opaque_columns {
            validate_identifier("Opaque 列", &opaque.name)?;
            if field_names.contains(&opaque.name) || !opaque_names.insert(opaque.name.clone()) {
                return Err(BaseError::ConfigError(format!(
                    "表 {} 的 opaque 列 {} 与字段或另一 opaque 列重名",
                    self.name, opaque.name
                )));
            }
            let sql_type = opaque.sql_type.trim();
            if sql_type.is_empty()
                || sql_type.chars().count() > 256
                || [";", "--", "/*", "*/", "\n", "\r", "\0"]
                    .iter()
                    .any(|marker| sql_type.contains(marker))
            {
                return Err(BaseError::ConfigError(format!(
                    "表 {} 的 opaque 列 {} 必须声明非空、长度受限且不含语句分隔符的类型文本",
                    self.name, opaque.name
                )));
            }
        }

        if let Some(legacy_name) = legacy_names
            .iter()
            .find(|legacy_name| field_names.contains(legacy_name.as_str()))
        {
            return Err(BaseError::ConfigError(format!(
                "表 {} 同时声明旧字段与改名后字段: {}",
                self.name, legacy_name
            )));
        }

        if !self.primary_key_order.is_empty() {
            let mut ordered = Vec::with_capacity(primary_keys.len());
            for name in &self.primary_key_order {
                match primary_keys.iter().position(|key| key == name) {
                    Some(position) => ordered.push(primary_keys.remove(position)),
                    None => {
                        return Err(BaseError::ConfigError(format!(
                            "表 {} 的主键顺序引用非主键字段: {name}",
                            self.name
                        )))
                    }
                }
            }
            ordered.append(&mut primary_keys);
            primary_keys = ordered;
        }

        let primary_key = primary_keys.first().cloned().unwrap_or_default();

        let mut unique_indexes = Vec::new();
        let mut normal_indexes = Vec::new();
        let mut index_names = HashSet::new();
        for index in indexes {
            validate_index(&self.name, &field_names, &index, &mut index_names)?;
            let config = IndexConfig::new(index.name, index.fields, index.nulls_not_distinct)
                .with_expressions(index.expressions)
                .with_predicate(index.predicate);
            if index.unique {
                unique_indexes.push(config);
            } else {
                normal_indexes.push(config);
            }
        }

        let mut constraint_names = HashSet::new();
        let checks = checks
            .into_iter()
            .map(|check| {
                validate_constraint_name(&self.name, &check.name, &mut constraint_names)?;
                validate_check_expression(&self.name, &check.name, &check.expression)?;
                Ok(CheckConfig {
                    name: check.name,
                    expression: check.expression,
                })
            })
            .collect::<Result<Vec<_>, BaseError>>()?;
        let foreign_keys = foreign_keys
            .into_iter()
            .map(|foreign_key| {
                validate_constraint_name(&self.name, &foreign_key.name, &mut constraint_names)?;
                validate_foreign_key(&self.name, &field_names, &foreign_key)?;
                Ok(ForeignKeyConfig {
                    name: foreign_key.name,
                    columns: foreign_key.columns,
                    referenced_table: foreign_key.referenced_table,
                    referenced_columns: foreign_key.referenced_columns,
                })
            })
            .collect::<Result<Vec<_>, BaseError>>()?;

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
        let table_ref = yang_db::TableRef::new(self.name.clone())
            .map_err(|error| BaseError::ConfigError(error.to_string()))?;
        let field_refs = field_names
            .iter()
            .map(|name| {
                yang_db::FieldRef::new(name.clone())
                    .map(|field| (name.clone(), field))
                    .map_err(|error| BaseError::ConfigError(error.to_string()))
            })
            .collect::<Result<HashMap<_, _>, _>>()?;

        Ok(TableDefinition {
            config: Arc::new(TableConfig {
                table_name: self.name.clone(),
                datasource: self.datasource.unwrap_or_default(),
                table_ref,
                display_name: self.label.unwrap_or(self.name),
                primary_key,
                primary_keys,
                fields: configs,
                opaque_columns: self.opaque_columns,
                field_refs,
                unique_indexes,
                indexes: normal_indexes,
                checks,
                foreign_keys,
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

    /// 返回表所属逻辑数据源。
    pub fn datasource(&self) -> &DatasourceId {
        &self.config.datasource
    }

    /// 返回展示名称。
    pub fn label(&self) -> &str {
        &self.config.display_name
    }

    /// 返回主键字段名。
    pub fn primary_key(&self) -> &str {
        &self.config.primary_key
    }

    /// 返回完整主键列（复合主键时大于一个元素）。
    pub fn primary_keys(&self) -> &[String] {
        &self.config.primary_keys
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

    /// 返回唯一租户隔离字段；表未声明租户字段时返回 `None`。
    pub fn tenant_key_field(&self) -> Option<&str> {
        self.config
            .fields
            .values()
            .find(|field| field.tenant_key)
            .map(|field| field.name.as_str())
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
    #[cfg(any(feature = "mysql", feature = "postgres"))]
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

    /// 将表定义绑定到 MySQL 连接池（兼容入口）。
    ///
    /// 内部会包装为 [`AnyPool::MySql`](crate::database::AnyPool)；需要 PostgreSQL
    /// 或其他运行时后端时请使用 [`TableDefinition::bind_pool`]。
    #[cfg(feature = "mysql")]
    pub fn bind(&self, pool: Arc<sqlx::MySqlPool>) -> TableHandle {
        self.bind_pool(crate::database::AnyPool::MySql(pool))
    }

    /// 将表定义绑定到任意受支持的数据库连接池（沿用表声明的数据源）。
    #[cfg(any(feature = "mysql", feature = "postgres"))]
    pub fn bind_pool(&self, pool: crate::database::AnyPool) -> TableHandle {
        self.bind_source_pool(crate::database::PoolHandle::new(
            self.datasource().clone(),
            pool,
        ))
    }

    /// 将表定义绑定到注册表中的具名数据源（推荐入口）。
    #[cfg(any(feature = "mysql", feature = "postgres"))]
    pub fn bind_datasource(&self, datasource: &crate::database::Datasource) -> TableHandle {
        self.bind_source_pool(crate::database::PoolHandle::new(
            datasource.id().clone(),
            datasource.database().pool(),
        ))
    }

    /// 将表定义绑定到带逻辑数据源身份的连接池句柄。
    #[cfg(any(feature = "mysql", feature = "postgres"))]
    pub fn bind_source_pool(&self, pool: crate::database::PoolHandle) -> TableHandle {
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

    /// 返回字段是否承担租户隔离键语义。
    pub fn is_tenant_key(self) -> bool {
        self.config.tenant_key
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

    /// 返回字段是否允许关键词搜索。
    pub fn is_searchable(self) -> bool {
        self.config.searchable
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
#[cfg(any(feature = "mysql", feature = "postgres"))]
#[derive(Debug, Clone)]
pub struct TableHandle {
    definition: TableDefinition,
    pool: crate::database::PoolHandle,
}

#[cfg(any(feature = "mysql", feature = "postgres"))]
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
        TableQuery::new_sourced(
            self.definition.shared_config(),
            roles,
            Some(self.pool.pool().clone()),
            self.pool.source().clone(),
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

/// 生成列表达式的括号平衡校验。
///
/// 跳过单引号字符串（含 `''` 双写与 `E'...'` 反斜杠转义）与双引号标识符；
/// 顶层右括号使深度变负即拒绝，结束时深度必须为 0。用于阻止
/// `price) STORED, ...` 这类闭合外层 `GENERATED ALWAYS AS (` 的注入。
fn has_balanced_generated_expression(expression: &str) -> bool {
    let bytes = expression.as_bytes();
    let mut depth: i32 = 0;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\'' => {
                let escape_string = is_escape_string_prefix(bytes, index);
                index += 1;
                while index < bytes.len() {
                    if escape_string && bytes[index] == b'\\' {
                        // E 字符串的反斜杠转义：跳过被转义的下一个字节。
                        index = (index + 2).min(bytes.len());
                        continue;
                    }
                    if bytes[index] == b'\'' {
                        if bytes.get(index + 1) == Some(&b'\'') {
                            index += 2;
                            continue;
                        }
                        index += 1;
                        break;
                    }
                    index += 1;
                }
            }
            b'"' => {
                index += 1;
                while index < bytes.len() {
                    if bytes[index] == b'"' {
                        if bytes.get(index + 1) == Some(&b'"') {
                            index += 2;
                            continue;
                        }
                        index += 1;
                        break;
                    }
                    index += 1;
                }
            }
            b'(' => {
                depth += 1;
                index += 1;
            }
            b')' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
                index += 1;
            }
            _ => index += 1,
        }
    }
    depth == 0
}

/// 判断 `'`（位于 `quote_index`）是否属于 PostgreSQL `E'...'` 转义字符串。
fn is_escape_string_prefix(bytes: &[u8], quote_index: usize) -> bool {
    if quote_index == 0 || !matches!(bytes[quote_index - 1], b'E' | b'e') {
        return false;
    }
    quote_index == 1 || !is_identifier_byte(bytes[quote_index - 2])
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$' || byte >= 0x80
}

/// CHECK 表达式的跨方言括号平衡校验。
///
/// CHECK 与生成列一样原样拼接进 `CONSTRAINT ... CHECK ({expr})`。表达式必须
/// 能作为单条括号平衡的布尔表达式嵌入外层括号；仅当 PostgreSQL 与 MySQL
/// 两种词法都判定越界（顶层右括号使深度变负，或结束时深度非 0 / 引号未闭合）
/// 时才拒绝，避免误伤任一方言独有的合法字面量。
fn has_balanced_check_expression(expression: &str) -> bool {
    // 两种词法各自完整扫描后取并集：MySQL 合法而 PostgreSQL 不认识的词法
    //（反引号、`#` 注释、反斜杠转义字符串）由 MySQL 扫描保住，反之
    //（E 字符串、U& 转义、美元引号）由 PostgreSQL 扫描保住。
    has_balanced_pg_check_expression(expression) || has_balanced_mysql_check_expression(expression)
}

/// PostgreSQL 词法下的括号平衡扫描。
///
/// 识别 `''` 双写、`E'...'` / `U&'...'` 反斜杠转义、`U&"..."` 与 `""` 双写、
/// 双引号标识符以及 `$tag$...$tag$` 美元引号；未闭合的引号视为越界。
pub(crate) fn has_balanced_pg_check_expression(expression: &str) -> bool {
    let bytes = expression.as_bytes();
    let mut depth: i32 = 0;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\'' => {
                let backslash_escapes =
                    is_escape_string_prefix(bytes, index) || is_unicode_escape_prefix(bytes, index);
                if !skip_quoted(bytes, &mut index, b'\'', backslash_escapes) {
                    return false;
                }
            }
            b'"' => {
                let backslash_escapes = is_unicode_escape_prefix(bytes, index);
                if !skip_quoted(bytes, &mut index, b'"', backslash_escapes) {
                    return false;
                }
            }
            b'$' => {
                let Some(tag_end) = dollar_quote_tag_end(bytes, index) else {
                    index += 1;
                    continue;
                };
                let tag = &bytes[index..tag_end];
                let mut cursor = tag_end;
                loop {
                    if cursor + tag.len() > bytes.len() {
                        // 未闭合的美元引号无法嵌入单条表达式。
                        return false;
                    }
                    if &bytes[cursor..cursor + tag.len()] == tag {
                        index = cursor + tag.len();
                        break;
                    }
                    cursor += 1;
                }
            }
            b'(' => {
                depth += 1;
                index += 1;
            }
            b')' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
                index += 1;
            }
            _ => index += 1,
        }
    }
    depth == 0
}

/// MySQL 词法下的括号平衡扫描。
///
/// 识别单/双引号字符串的反斜杠转义与引号双写、反引号标识符双写以及 `#`
/// 行注释；`--` 与 `/* */` 已由上层黑名单统一拒绝。未闭合的引号视为越界。
pub(crate) fn has_balanced_mysql_check_expression(expression: &str) -> bool {
    let bytes = expression.as_bytes();
    let mut depth: i32 = 0;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\'' => {
                if !skip_quoted(bytes, &mut index, b'\'', true) {
                    return false;
                }
            }
            b'"' => {
                if !skip_quoted(bytes, &mut index, b'"', true) {
                    return false;
                }
            }
            b'`' => {
                if !skip_quoted(bytes, &mut index, b'`', false) {
                    return false;
                }
            }
            b'#' => {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'(' => {
                depth += 1;
                index += 1;
            }
            b')' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
                index += 1;
            }
            _ => index += 1,
        }
    }
    depth == 0
}

/// 跳过一段由 `delimiter` 包围的字符串或标识符，成功时把 `index` 移到闭合
/// 引号之后；`backslash_escapes` 为真时识别反斜杠转义。两种方言都支持把
/// 引号写成双写来转义。未闭合返回 false。
fn skip_quoted(bytes: &[u8], index: &mut usize, delimiter: u8, backslash_escapes: bool) -> bool {
    *index += 1;
    while *index < bytes.len() {
        if backslash_escapes && bytes[*index] == b'\\' {
            *index = (*index + 2).min(bytes.len());
            continue;
        }
        if bytes[*index] == delimiter {
            if bytes.get(*index + 1) == Some(&delimiter) {
                *index += 2;
                continue;
            }
            *index += 1;
            return true;
        }
        *index += 1;
    }
    false
}

/// 判断引号（位于 `quote_index`）是否属于 PostgreSQL `U&'...'` / `U&"..."` 前缀。
fn is_unicode_escape_prefix(bytes: &[u8], quote_index: usize) -> bool {
    if quote_index < 2
        || bytes[quote_index - 1] != b'&'
        || !matches!(bytes[quote_index - 2], b'U' | b'u')
    {
        return false;
    }
    quote_index < 3 || !is_identifier_byte(bytes[quote_index - 3])
}

/// 识别 PostgreSQL 美元引号起始 `$tag$`，返回 tag（含首尾 `$`）的结束下标。
fn dollar_quote_tag_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut index = start + 1;
    let first = bytes.get(index)?;
    if *first == b'$' {
        // `$$...$$`：空 tag。
        return Some(index + 1);
    }
    if !(*first == b'_' || first.is_ascii_alphabetic() || *first >= 0x80) {
        return None;
    }
    index += 1;
    while let Some(byte) = bytes.get(index) {
        if byte.is_ascii_alphanumeric() || *byte == b'_' || *byte >= 0x80 {
            index += 1;
        } else {
            break;
        }
    }
    (bytes.get(index) == Some(&b'$')).then_some(index + 1)
}

fn validate_field_shape(table: &str, field: &Field, cache: &RegexCache) -> Result<(), BaseError> {
    match &field.config.field_type {
        FieldType::String { max_length } if !(1..=MAX_VARCHAR_LENGTH).contains(max_length) => {
            return Err(BaseError::ConfigError(format!(
                "表 {table} 的字符串字段 {} 长度必须在 1..={MAX_VARCHAR_LENGTH}（更长内容请使用 Text 字段）",
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
        FieldType::Decimal { precision, scale }
            if *precision == 0 || *precision > 65 || *scale > *precision =>
        {
            return Err(BaseError::ConfigError(format!(
                "表 {table} 的 DECIMAL 字段 {} 必须满足 1 <= precision <= 65 且 scale <= precision",
                field.config.name
            )));
        }
        FieldType::Array(element) if matches!(**element, FieldType::Array(_) | FieldType::Json) => {
            return Err(BaseError::ConfigError(format!(
                "表 {table} 的数组字段 {} 只支持标量元素，不支持嵌套数组或 JSON 元素",
                field.config.name
            )));
        }
        _ => {}
    }

    if let Some(type_name) = &field.config.native_enum {
        if !matches!(field.config.field_type, FieldType::Enum { .. }) {
            return Err(BaseError::ConfigError(format!(
                "表 {table} 的字段 {} 只有枚举类型可以声明 native_enum",
                field.config.name
            )));
        }
        validate_identifier("枚举类型", type_name)?;
        // PostgreSQL 标识符上限是 63 字节；通用校验的 64 字符上界不能替代。
        if type_name.len() > 63 {
            return Err(BaseError::ConfigError(format!(
                "表 {table} 的枚举类型名 {type_name} 超过 PostgreSQL 63 字节限制"
            )));
        }
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
    if let Some(expression) = &field.config.generated {
        let expression = expression.trim();
        if expression.is_empty()
            || expression.chars().count() > 2_048
            || [";", "--", "/*", "*/"]
                .iter()
                .any(|marker| expression.contains(marker))
            || !has_balanced_generated_expression(expression)
        {
            return Err(BaseError::ConfigError(format!(
                "表 {table} 的生成列 {} 必须是单条、非空、长度受限且括号平衡的受信表达式",
                field.config.name
            )));
        }
        if field.config.auto_increment || field.config.default_value.is_some() {
            return Err(BaseError::ConfigError(format!(
                "表 {table} 的生成列 {} 不能同时声明自增或默认值",
                field.config.name
            )));
        }
    }
    if let Some(collation) = &field.config.collation {
        if collation.trim().is_empty() || collation.len() > 128 {
            return Err(BaseError::ConfigError(format!(
                "表 {table} 的字段 {} 声明了空或过长的 collation",
                field.config.name
            )));
        }
    }
    validate_validator_configuration(table, &field.config)?;
    if let Some(default) = &field.config.default_value {
        if matches!(
            field.config.field_type,
            FieldType::Text | FieldType::Json | FieldType::Bytes | FieldType::Array(_)
        ) {
            return Err(BaseError::ConfigError(format!(
                "表 {table} 的 TEXT/JSON/BYTES/ARRAY 字段不能配置数据库默认值: {}",
                field.config.name
            )));
        }
        field.config.validate(default, cache)?;
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
            | FieldType::Decimal { .. }
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
    if index.fields.is_empty() && index.expressions.is_empty() {
        return Err(BaseError::ConfigError(format!(
            "表 {table} 的索引字段与表达式不能同时为空"
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
    for expression in &index.expressions {
        validate_check_expression(table, "索引表达式", expression).map_err(|_| {
            BaseError::ConfigError(format!(
                "表 {table} 的索引表达式必须是单条、非空且长度受限的受信 SQL: {expression}"
            ))
        })?;
    }
    if let Some(predicate) = &index.predicate {
        validate_check_expression(table, "部分索引谓词", predicate).map_err(|_| {
            BaseError::ConfigError(format!(
                "表 {table} 的部分索引谓词必须是单条、非空且长度受限的受信 SQL: {predicate}"
            ))
        })?;
    }
    if !index.expressions.is_empty() && index.name.is_none() {
        return Err(BaseError::ConfigError(format!(
            "表 {table} 的表达式索引必须显式声明名称"
        )));
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

fn validate_constraint_name(
    table: &str,
    name: &str,
    names: &mut HashSet<String>,
) -> Result<(), BaseError> {
    validate_identifier("约束", name)?;
    if !names.insert(name.to_string()) {
        return Err(BaseError::ConfigError(format!(
            "表 {table} 存在重复约束名: {name}"
        )));
    }
    Ok(())
}

fn validate_check_expression(table: &str, name: &str, expression: &str) -> Result<(), BaseError> {
    let expression = expression.trim();
    if expression.is_empty()
        || expression.chars().count() > 2_048
        || [";", "--", "/*", "*/"]
            .iter()
            .any(|marker| expression.contains(marker))
        || !has_balanced_check_expression(expression)
    {
        return Err(BaseError::ConfigError(format!(
            "表 {table} 的 CHECK {name} 必须是单条、非空、长度受限且括号平衡的受信表达式"
        )));
    }
    Ok(())
}

fn validate_foreign_key(
    table: &str,
    field_names: &HashSet<String>,
    foreign_key: &PendingForeignKey,
) -> Result<(), BaseError> {
    validate_identifier("外键目标表", &foreign_key.referenced_table)?;
    if foreign_key.columns.is_empty()
        || foreign_key.columns.len() != foreign_key.referenced_columns.len()
    {
        return Err(BaseError::ConfigError(format!(
            "表 {table} 的外键 {} 本地列与目标列必须非空且数量一致",
            foreign_key.name
        )));
    }
    let mut local = HashSet::new();
    let mut referenced = HashSet::new();
    for column in &foreign_key.columns {
        validate_identifier("外键列", column)?;
        if !field_names.contains(column) || !local.insert(column) {
            return Err(BaseError::ConfigError(format!(
                "表 {table} 的外键 {} 引用了缺失或重复的本地列 {column}",
                foreign_key.name
            )));
        }
    }
    for column in &foreign_key.referenced_columns {
        validate_identifier("外键目标列", column)?;
        if !referenced.insert(column) {
            return Err(BaseError::ConfigError(format!(
                "表 {table} 的外键 {} 重复引用目标列 {column}",
                foreign_key.name
            )));
        }
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
        FieldType::Decimal { precision, scale } => {
            schema.insert("type".into(), json!("string"));
            schema.insert("format".into(), json!("decimal"));
            schema.insert("x-precision".into(), json!(precision));
            schema.insert("x-scale".into(), json!(scale));
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
        FieldType::Uuid => {
            schema.insert("type".into(), json!("string"));
            schema.insert("format".into(), json!("uuid"));
        }
        FieldType::Bytes => {
            // Base64 文本；与 OpenAPI 的 `byte` format 对齐。
            schema.insert("type".into(), json!("string"));
            schema.insert("format".into(), json!("byte"));
        }
        FieldType::Array(element) => {
            schema.insert("type".into(), json!("array"));
            schema.insert("items".into(), storage_type_json_schema(element));
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

/// 数组元素的 JSON Schema 存储类型（不含字段级校验器与权限元数据）。
fn storage_type_json_schema(field_type: &FieldType) -> Value {
    match field_type {
        FieldType::String { max_length } => json!({
            "type": "string",
            "maxLength": max_length,
        }),
        FieldType::Text => json!({"type": "string"}),
        FieldType::Uuid => json!({"type": "string", "format": "uuid"}),
        FieldType::Bytes => json!({"type": "string", "format": "byte"}),
        FieldType::Enum { values } => json!({"type": "string", "enum": values}),
        FieldType::Decimal { precision, scale } => json!({
            "type": "string",
            "format": "decimal",
            "x-precision": precision,
            "x-scale": scale,
        }),
        FieldType::Integer | FieldType::BigInt | FieldType::Timestamp => json!({"type": "integer"}),
        FieldType::Float | FieldType::Double => json!({"type": "number"}),
        FieldType::Boolean => json!({"type": "boolean"}),
        FieldType::Date => json!({"type": "string", "format": "date"}),
        FieldType::DateTime => json!({"type": "string", "format": "date-time"}),
        // 嵌套数组/JSON 元素在构建期已被拒绝，这里保持空 schema 作为兜底。
        FieldType::Json | FieldType::Array(_) => json!({}),
    }
}

#[cfg(test)]
mod composite_primary_key_tests {
    use super::*;

    #[test]
    fn table_without_primary_key_is_allowed() {
        let definition = Table::new("no_pk")
            .fields([Field::string("name", 64).required()])
            .build()
            .expect("无主键表定义应有效");
        assert!(definition.primary_keys().is_empty());
        assert_eq!(definition.primary_key(), "");
    }

    #[test]
    fn composite_primary_key_keeps_all_columns() {
        let definition = Table::new("composite_pk")
            .fields([
                Field::bigint("tenant_id").required().primary_key(),
                Field::bigint("item_id").required().primary_key(),
                Field::string("name", 64),
            ])
            .build()
            .expect("复合主键表定义应有效");
        assert_eq!(definition.primary_key(), "tenant_id");
        assert_eq!(definition.primary_keys(), ["tenant_id", "item_id"]);
    }
}

#[cfg(test)]
mod generated_expression_tests {
    use super::*;

    fn generated_table(expression: &str) -> Result<TableDefinition, BaseError> {
        Table::new("generated_balance")
            .fields([
                Field::id("id"),
                Field::integer("price").required(),
                Field::string("name", 32),
                Field::bigint("total").generated(expression),
            ])
            .build()
    }

    #[test]
    fn balanced_generated_expressions_pass() {
        generated_table("price * 2").expect("简单表达式应通过");
        generated_table("coalesce(price, 0)").expect("函数调用应通过");
        // 字符串字面量中的右括号不是语法括号。
        generated_table("coalesce(name, ')')").expect("字符串字面量含右括号应通过");
        // E 字符串中的反斜杠转义不破坏扫描。
        generated_table(r"coalesce(name, E'\')')").expect("E 字符串转义应通过");
    }

    #[test]
    fn generated_expression_injection_is_rejected() {
        let error = generated_table("price) STORED, 注入 INTEGER")
            .expect_err("闭合外层括号的注入表达式必须拒绝");
        assert!(
            matches!(error, BaseError::ConfigError(ref message) if message.contains("生成列")),
            "实得错误: {error:?}"
        );
    }
}

#[cfg(test)]
mod check_expression_tests {
    use super::*;

    fn check_table(expression: &str) -> Result<TableDefinition, BaseError> {
        Table::new("check_balance")
            .fields([
                Field::id("id"),
                Field::integer("price").required(),
                Field::string("status", 32),
                Field::string("note", 64),
            ])
            .check_named("chk_balance", expression)
            .build()
    }

    #[test]
    fn legal_check_expressions_pass() {
        check_table("price > 0").expect("简单表达式应通过");
        check_table("price > 0 AND status <> 'banned'").expect("字符串字面量应通过");
        check_table("(price > 0)").expect("成对分组括号应通过");
        check_table("`parent_id` > 0").expect("反引号标识符应通过");
        // MySQL 反斜杠转义字符串：右括号属于字符串，不是语法括号。
        check_table(r"note <> 'it\'s)'").expect("MySQL 反斜杠转义字符串应通过");
        // MySQL 反引号标识符内的括号不参与计数。
        check_table("`amount)` > 0").expect("反引号内的括号应通过");
        // MySQL # 注释内不成对括号不参与计数（PG 扫描侧会因缺右括号失败）。
        check_table("price > 0 # note (policy 2024\nAND status <> 'cancelled'")
            .expect("MySQL # 注释应通过");
        // PostgreSQL 美元引号内的括号不参与计数。
        check_table("$q$a)b$q$").expect("PostgreSQL 美元引号应通过");
        // PostgreSQL E 字符串的反斜杠转义不破坏扫描。
        check_table(r"note <> E'\')'").expect("PostgreSQL E 字符串应通过");
    }

    #[test]
    fn check_expression_injection_is_rejected() {
        // RED: 现状仅黑名单 `; -- /* */`，该载荷可通过校验并在渲染时闭合
        // 外层 `CHECK (`，追加额外 DDL 子句。
        let error = check_table("price > 0), evil INT, CHECK (1=1")
            .expect_err("闭合外层括号的注入表达式必须拒绝");
        assert!(
            matches!(error, BaseError::ConfigError(ref message) if message.contains("CHECK")),
            "实得错误: {error:?}"
        );
    }
}
