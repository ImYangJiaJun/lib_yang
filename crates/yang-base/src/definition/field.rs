//! BR 风格 Fields 定义及其原生 YANG 投影。

use super::{ActionRef, FieldKind, FieldName, FieldRef, TableName};
use crate::error::BaseError;
use crate::table::{Field, RelationType, Table as SchemaTable, TableDefinition};
use serde_json::Value;
use std::marker::PhantomData;

/// 字段的数据库存储语义。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StorageSpec {
    /// 是否允许缺失/NULL。
    pub required: bool,
    /// 字符串存储上限。
    pub max_length: Option<usize>,
    /// DECIMAL 精度。
    pub precision: Option<u8>,
    /// DECIMAL 小数位数。
    pub scale: Option<u8>,
    /// 数据库默认值。
    pub default: Option<Value>,
    /// 是否建立唯一索引。
    pub unique: bool,
    /// 是否建立普通索引。
    pub indexed: bool,
}

/// 字段的输入验证语义。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ValidationSpec {
    /// 字符最小长度。
    pub min_length: Option<usize>,
    /// 字符最大长度。
    pub max_length: Option<usize>,
    /// 数值下限，使用十进制文本避免元数据中的浮点漂移。
    pub minimum: Option<String>,
    /// 数值上限，使用十进制文本避免元数据中的浮点漂移。
    pub maximum: Option<String>,
    /// 可选正则表达式。
    pub pattern: Option<String>,
}

/// 字段读写与查询权限。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessRule {
    /// 所有调用者。
    Everyone,
    /// 不允许普通调用者。
    Nobody,
    /// 仅指定角色。
    Roles(Vec<String>),
}

/// 字段的访问控制语义。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessSpec {
    /// 读取规则。
    pub readable: AccessRule,
    /// 写入规则。
    pub writable: AccessRule,
    /// 是否允许筛选。
    pub searchable: bool,
    /// 是否允许排序。
    pub sortable: bool,
    /// 是否为敏感字段。
    pub secret: bool,
}

impl Default for AccessSpec {
    fn default() -> Self {
        Self {
            readable: AccessRule::Everyone,
            writable: AccessRule::Everyone,
            searchable: false,
            sortable: false,
            secret: false,
        }
    }
}

/// 字段的默认展示语义。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PresentationSpec {
    /// 用户可见标题。
    pub title: String,
    /// 帮助说明。
    pub description: String,
    /// 关系字段的默认展示列。
    pub display: Vec<FieldRef>,
}

/// 时间戳字段的自动写入语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimestampMode {
    /// 普通时间戳。
    Value,
    /// 插入时自动写入。
    CreatedAt,
    /// 更新时自动写入。
    UpdatedAt,
    /// 软删除标记。
    SoftDelete,
}

/// 单个字段的共享语义核心。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldSpec {
    /// 字段名。
    pub name: FieldName,
    /// 字段语义种类。
    pub kind: FieldKind,
    /// 数据库存储语义。
    pub storage: StorageSpec,
    /// 输入验证语义。
    pub validation: ValidationSpec,
    /// 访问控制语义。
    pub access: AccessSpec,
    /// 默认展示语义。
    pub presentation: PresentationSpec,
    /// 可选关系目标。
    pub relation: Option<FieldRef>,
    /// 可选选择器 Action。
    pub select: Option<ActionRef>,
    /// Radio 的值与标题。
    pub options: Vec<(String, String)>,
    /// 时间戳自动写入模式。
    pub timestamp_mode: TimestampMode,
    /// 是否为租户隔离键。
    pub tenant_key: bool,
}

impl FieldSpec {
    /// 创建最小字段定义。
    pub fn new(name: FieldName, kind: FieldKind) -> Self {
        Self {
            name,
            kind,
            storage: StorageSpec::default(),
            validation: ValidationSpec::default(),
            access: AccessSpec::default(),
            presentation: PresentationSpec::default(),
            relation: None,
            select: None,
            options: Vec::new(),
            timestamp_mode: TimestampMode::Value,
            tenant_key: false,
        }
    }

    /// 设置必填语义。
    pub fn required(mut self, required: bool) -> Self {
        self.storage.required = required;
        self
    }

    /// 返回字段是否必填。
    pub fn is_required(&self) -> bool {
        self.storage.required
    }

    /// 设置关系目标。
    pub fn relation(mut self, target: FieldRef) -> Self {
        self.relation = Some(target);
        self
    }

    /// 设置选择器 Action。
    pub fn select(mut self, action: ActionRef) -> Self {
        self.select = Some(action);
        self
    }

    /// 将字段标记为租户隔离键。
    pub fn tenant_key(mut self, tenant_key: bool) -> Self {
        self.tenant_key = tenant_key;
        self
    }

    fn into_schema_field(self) -> Result<Field, BaseError> {
        let name = self.name.to_string();
        let mut field = match self.kind {
            FieldKind::Key => Field::id(name),
            FieldKind::Str => Field::string(name, self.storage.max_length.unwrap_or(255)),
            FieldKind::Text => Field::text(name),
            FieldKind::Int => Field::bigint(name),
            FieldKind::Decimal => Field::decimal(
                name,
                self.storage.precision.unwrap_or(18),
                self.storage.scale.unwrap_or(2),
            ),
            FieldKind::Switch => Field::boolean(name),
            FieldKind::Radio => {
                Field::enumeration(name, self.options.iter().map(|(value, _)| value.clone()))
            }
            FieldKind::Table | FieldKind::Tree => Field::bigint(name),
            FieldKind::Timestamp => match self.timestamp_mode {
                TimestampMode::Value => Field::timestamp(name),
                TimestampMode::CreatedAt => Field::created_at(name),
                TimestampMode::UpdatedAt => Field::updated_at(name),
                TimestampMode::SoftDelete => Field::soft_delete(name),
            },
        };

        if self.kind != FieldKind::Key && self.storage.required {
            field = field.required();
        }
        if let Some(default) = self.storage.default {
            field = field.default(default);
        }
        if self.storage.unique {
            field = field.unique();
        }
        if self.storage.indexed {
            field = field.index();
        }
        if let Some(value) = self.validation.min_length {
            field = field.min_length(value);
        }
        if let Some(value) = self.validation.max_length {
            field = field.max_length(value);
        }
        if let Some(pattern) = self.validation.pattern {
            field = field.regex(pattern);
        }
        if self.access.secret {
            field = field.secret();
        }
        field = apply_read_rule(field, self.access.readable);
        field = apply_write_rule(field, self.access.writable);
        if self.access.searchable {
            field = field.filterable();
        }
        if self.access.sortable {
            field = field.sortable();
        }
        if self.tenant_key {
            field = field.tenant_key();
        }
        if !self.presentation.title.is_empty() {
            field = field.label(self.presentation.title);
        }
        if let Some(target) = self.relation {
            let relation_type = if self.kind == FieldKind::Tree {
                RelationType::OneToMany
            } else {
                RelationType::ManyToOne
            };
            field = field.relation(
                target.table().to_string(),
                target.field().to_string(),
                relation_type,
            );
        }
        Ok(field)
    }
}

fn apply_read_rule(field: Field, rule: AccessRule) -> Field {
    match rule {
        AccessRule::Everyone => field.readable(),
        AccessRule::Nobody => field.not_readable(),
        AccessRule::Roles(roles) => field.readable_by(roles),
    }
}

fn apply_write_rule(field: Field, rule: AccessRule) -> Field {
    match rule {
        AccessRule::Everyone => field.writable(),
        AccessRule::Nobody => field.not_writable(),
        AccessRule::Roles(roles) => field.writable_by(roles),
    }
}

/// 有序、强类型字段集合。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Fields(Vec<FieldSpec>);

impl Fields {
    /// 创建空字段集合。
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// 增加一个有名称的字段 Builder。
    pub fn field<B>(mut self, name: FieldName, builder: B) -> Self
    where
        B: IntoFieldSpec,
    {
        self.0.push(builder.into_field_spec(name));
        self
    }

    /// 返回字段定义。
    pub fn as_slice(&self) -> &[FieldSpec] {
        &self.0
    }

    pub(crate) fn into_vec(self) -> Vec<FieldSpec> {
        self.0
    }
}

/// 可转换为最终 FieldSpec 的类型化 Builder。
pub trait IntoFieldSpec {
    /// 绑定字段名并生成唯一原生定义。
    fn into_field_spec(self, name: FieldName) -> FieldSpec;
}

#[derive(Debug, Clone)]
struct FieldBuilder {
    kind: FieldKind,
    storage: StorageSpec,
    validation: ValidationSpec,
    access: AccessSpec,
    presentation: PresentationSpec,
    relation: Option<FieldRef>,
    select: Option<ActionRef>,
    options: Vec<(String, String)>,
    timestamp_mode: TimestampMode,
    tenant_key: bool,
}

impl FieldBuilder {
    fn new(kind: FieldKind) -> Self {
        Self {
            kind,
            storage: StorageSpec::default(),
            validation: ValidationSpec::default(),
            access: AccessSpec::default(),
            presentation: PresentationSpec::default(),
            relation: None,
            select: None,
            options: Vec::new(),
            timestamp_mode: TimestampMode::Value,
            tenant_key: false,
        }
    }

    fn build(self, name: FieldName) -> FieldSpec {
        FieldSpec {
            name,
            kind: self.kind,
            storage: self.storage,
            validation: self.validation,
            access: self.access,
            presentation: self.presentation,
            relation: self.relation,
            select: self.select,
            options: self.options,
            timestamp_mode: self.timestamp_mode,
            tenant_key: self.tenant_key,
        }
    }
}

macro_rules! simple_builder {
    ($name:ident, $kind:expr) => {
        #[doc = concat!(stringify!($name), " 字段 Builder。")]
        #[derive(Debug, Clone)]
        pub struct $name(FieldBuilder);

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl $name {
            /// 创建字段 Builder。
            pub fn new() -> Self {
                Self(FieldBuilder::new($kind))
            }

            /// 设置展示标题。
            pub fn title(mut self, value: impl Into<String>) -> Self {
                self.0.presentation.title = value.into();
                self
            }

            /// 设置帮助说明。
            pub fn description(mut self, value: impl Into<String>) -> Self {
                self.0.presentation.description = value.into();
                self
            }

            /// 设置必填。
            pub fn require(mut self, value: bool) -> Self {
                self.0.storage.required = value;
                self
            }

            /// 设置数据库默认值。
            pub fn default(mut self, value: impl Into<Value>) -> Self {
                self.0.storage.default = Some(value.into());
                self
            }

            /// 设置唯一索引。
            pub fn unique(mut self, value: bool) -> Self {
                self.0.storage.unique = value;
                self
            }

            /// 设置普通索引。
            pub fn indexed(mut self, value: bool) -> Self {
                self.0.storage.indexed = value;
                self
            }

            /// 设置是否允许筛选。
            pub fn searchable(mut self, value: bool) -> Self {
                self.0.access.searchable = value;
                self
            }

            /// 设置是否允许排序。
            pub fn sortable(mut self, value: bool) -> Self {
                self.0.access.sortable = value;
                self
            }

            /// 设置敏感字段语义。
            pub fn secret(mut self, value: bool) -> Self {
                self.0.access.secret = value;
                if value {
                    self.0.access.readable = AccessRule::Nobody;
                    self.0.access.writable = AccessRule::Nobody;
                }
                self
            }

            /// 设置是否可读。
            pub fn readable(mut self, value: bool) -> Self {
                self.0.access.readable = if value {
                    AccessRule::Everyone
                } else {
                    AccessRule::Nobody
                };
                self
            }

            /// 限制可读角色。
            pub fn readable_by<I, S>(mut self, roles: I) -> Self
            where
                I: IntoIterator<Item = S>,
                S: Into<String>,
            {
                self.0.access.readable =
                    AccessRule::Roles(roles.into_iter().map(Into::into).collect());
                self
            }

            /// 限制可写角色。
            pub fn writable_by<I, S>(mut self, roles: I) -> Self
            where
                I: IntoIterator<Item = S>,
                S: Into<String>,
            {
                self.0.access.writable =
                    AccessRule::Roles(roles.into_iter().map(Into::into).collect());
                self
            }

            /// 标记租户隔离键。
            pub fn tenant_key(mut self, value: bool) -> Self {
                self.0.tenant_key = value;
                self
            }
        }

        impl IntoFieldSpec for $name {
            fn into_field_spec(self, name: FieldName) -> FieldSpec {
                self.0.build(name)
            }
        }
    };
}

simple_builder!(Key, FieldKind::Key);
simple_builder!(Str, FieldKind::Str);
simple_builder!(Text, FieldKind::Text);
simple_builder!(Int, FieldKind::Int);
simple_builder!(Decimal, FieldKind::Decimal);
simple_builder!(Switch, FieldKind::Switch);
simple_builder!(Table, FieldKind::Table);
simple_builder!(Tree, FieldKind::Tree);
simple_builder!(Timestamp, FieldKind::Timestamp);

/// 密码参数 Builder；默认标记 secret 且不投影为可读字段。
#[derive(Debug, Clone)]
pub struct Password(FieldBuilder);

impl Default for Password {
    fn default() -> Self {
        Self::new()
    }
}

impl Password {
    /// 创建密码参数 Builder。
    pub fn new() -> Self {
        let mut inner = FieldBuilder::new(FieldKind::Str);
        inner.access.secret = true;
        inner.access.readable = AccessRule::Nobody;
        Self(inner)
    }

    /// 设置展示标题。
    pub fn title(mut self, value: impl Into<String>) -> Self {
        self.0.presentation.title = value.into();
        self
    }

    /// 设置必填。
    pub fn require(mut self, value: bool) -> Self {
        self.0.storage.required = value;
        self
    }

    /// 设置最小长度。
    pub fn min_length(mut self, value: usize) -> Self {
        self.0.validation.min_length = Some(value);
        self
    }

    /// 设置最大长度。
    pub fn max_length(mut self, value: usize) -> Self {
        self.0.storage.max_length = Some(value);
        self.0.validation.max_length = Some(value);
        self
    }
}

impl IntoFieldSpec for Password {
    fn into_field_spec(self, name: FieldName) -> FieldSpec {
        self.0.build(name)
    }
}

impl Str {
    /// 同时设置 VARCHAR 与输入校验上限。
    pub fn max_length(mut self, value: usize) -> Self {
        self.0.storage.max_length = Some(value);
        self.0.validation.max_length = Some(value);
        self
    }

    /// 设置输入最小长度。
    pub fn min_length(mut self, value: usize) -> Self {
        self.0.validation.min_length = Some(value);
        self
    }

    /// 设置正则校验。
    pub fn pattern(mut self, value: impl Into<String>) -> Self {
        self.0.validation.pattern = Some(value.into());
        self
    }
}

impl Decimal {
    /// 设置 DECIMAL 精度和小数位。
    pub fn precision(mut self, precision: u8, scale: u8) -> Self {
        self.0.storage.precision = Some(precision);
        self.0.storage.scale = Some(scale);
        self
    }
}

impl Timestamp {
    /// 标记为插入时自动写入的创建时间。
    pub fn created_at(mut self) -> Self {
        self.0.timestamp_mode = TimestampMode::CreatedAt;
        self
    }

    /// 标记为更新时自动写入的更新时间。
    pub fn updated_at(mut self) -> Self {
        self.0.timestamp_mode = TimestampMode::UpdatedAt;
        self
    }

    /// 标记为软删除时间。
    pub fn soft_delete(mut self) -> Self {
        self.0.timestamp_mode = TimestampMode::SoftDelete;
        self
    }
}

macro_rules! relation_builder {
    ($name:ident) => {
        impl $name {
            /// 设置关系目标表主键。
            pub fn target(mut self, value: FieldRef) -> Self {
                self.0.relation = Some(value);
                self
            }

            /// 设置默认选择器 Action。
            pub fn select(mut self, value: ActionRef) -> Self {
                self.0.select = Some(value);
                self
            }

            /// 设置关系展示字段。
            pub fn display<I>(mut self, values: I) -> Self
            where
                I: IntoIterator<Item = FieldRef>,
            {
                self.0.presentation.display = values.into_iter().collect();
                self
            }
        }
    };
}

relation_builder!(Table);
relation_builder!(Tree);

/// 保留 Rust 值类型信息的 Radio Builder；运行时 Catalog 存储稳定字符串值。
#[derive(Debug, Clone)]
pub struct Radio<T> {
    inner: FieldBuilder,
    marker: PhantomData<fn() -> T>,
}

impl<T> Default for Radio<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Radio<T> {
    /// 创建 Radio Builder。
    pub fn new() -> Self {
        Self {
            inner: FieldBuilder::new(FieldKind::Radio),
            marker: PhantomData,
        }
    }

    /// 设置展示标题。
    pub fn title(mut self, value: impl Into<String>) -> Self {
        self.inner.presentation.title = value.into();
        self
    }

    /// 设置必填。
    pub fn require(mut self, value: bool) -> Self {
        self.inner.storage.required = value;
        self
    }

    /// 设置候选值和标题。
    pub fn options<I, V, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = (V, S)>,
        V: ToString,
        S: Into<String>,
    {
        self.inner.options = values
            .into_iter()
            .map(|(value, title)| (value.to_string(), title.into()))
            .collect();
        self
    }

    /// 设置默认值。
    pub fn default<V>(mut self, value: V) -> Self
    where
        V: ToString,
    {
        self.inner.storage.default = Some(Value::String(value.to_string()));
        self
    }
}

impl<T> IntoFieldSpec for Radio<T> {
    fn into_field_spec(self, name: FieldName) -> FieldSpec {
        self.inner.build(name)
    }
}

/// 表及其唯一字段定义。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSpec {
    /// 表名。
    pub name: TableName,
    /// 用户可见名称。
    pub title: String,
    /// 字段集合。
    pub fields: Vec<FieldSpec>,
}

impl TableSpec {
    /// 创建空表定义。
    pub fn new(name: TableName) -> Self {
        Self {
            name,
            title: String::new(),
            fields: Vec::new(),
        }
    }

    /// 设置表标题。
    pub fn title(mut self, value: impl Into<String>) -> Self {
        self.title = value.into();
        self
    }

    /// 使用 Fields 作为唯一字段来源。
    pub fn fields(mut self, fields: Fields) -> Self {
        self.fields = fields.into_vec();
        self
    }

    /// 增加一个已经命名的字段定义。
    #[must_use]
    pub fn field(mut self, field: FieldSpec) -> Self {
        self.fields.push(field);
        self
    }

    /// 生成 schema 同步和 TableQuery 使用的不可变执行产物。
    pub fn table_definition(&self) -> Result<TableDefinition, BaseError> {
        let mut table = SchemaTable::new(self.name.to_string());
        if !self.title.is_empty() {
            table = table.label(self.title.clone());
        }
        let fields = self
            .fields
            .clone()
            .into_iter()
            .map(FieldSpec::into_schema_field)
            .collect::<Result<Vec<_>, _>>()?;
        table.fields(fields).build()
    }
}
