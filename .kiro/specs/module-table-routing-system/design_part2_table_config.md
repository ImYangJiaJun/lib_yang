# 设计文档：模块表路由系统 - 第2部分：表配置系统

## 4. 表配置系统（TableConfig）

### 4.1 TableConfig 结构

```rust
/// 表配置
///
/// 定义数据表的元数据、字段、索引和权限
#[derive(Debug, Clone)]
pub struct TableConfig {
    /// 表名
    pub table_name: String,
    
    /// 显示名称
    pub display_name: String,
    
    /// 字段配置列表
    pub fields: Vec<FieldConfig>,
    
    /// 主键字段名
    pub primary_key: String,
    
    /// 唯一索引配置
    pub unique_indexes: Vec<IndexConfig>,
    
    /// 普通索引配置
    pub indexes: Vec<IndexConfig>,
    
    /// 默认排序字段
    pub default_order: Vec<(String, OrderDirection)>,
    
    /// 软删除字段（可选）
    pub soft_delete_field: Option<String>,
    
    /// 时间戳字段
    pub timestamp_fields: TimestampFields,
    
    /// 权限配置
    pub permissions: PermissionConfig,
}

impl TableConfig {
    /// 创建新的表配置
    pub fn new(table_name: impl Into<String>) -> Self {
        Self {
            table_name: table_name.into(),
            display_name: String::new(),
            fields: Vec::new(),
            primary_key: "id".to_string(),
            unique_indexes: Vec::new(),
            indexes: Vec::new(),
            default_order: vec![("id".to_string(), OrderDirection::Desc)],
            soft_delete_field: Some("deleted_at".to_string()),
            timestamp_fields: TimestampFields::default(),
            permissions: PermissionConfig::default(),
        }
    }
    
    /// 设置显示名称
    pub fn display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = name.into();
        self
    }
    
    /// 添加字段
    pub fn field(mut self, field: FieldConfig) -> Self {
        self.fields.push(field);
        self
    }
    
    /// 批量添加字段
    pub fn fields(mut self, fields: Vec<FieldConfig>) -> Self {
        self.fields.extend(fields);
        self
    }
    
    /// 设置主键
    pub fn primary_key(mut self, key: impl Into<String>) -> Self {
        self.primary_key = key.into();
        self
    }
    
    /// 添加唯一索引
    pub fn unique_index(mut self, fields: Vec<String>) -> Self {
        self.unique_indexes.push(IndexConfig { fields });
        self
    }
    
    /// 添加普通索引
    pub fn index(mut self, fields: Vec<String>) -> Self {
        self.indexes.push(IndexConfig { fields });
        self
    }
    
    /// 设置默认排序
    pub fn default_order(mut self, order: Vec<(String, OrderDirection)>) -> Self {
        self.default_order = order;
        self
    }
    
    /// 获取字段配置
    pub fn get_field(&self, field_name: &str) -> Option<&FieldConfig> {
        self.fields.iter().find(|f| f.name == field_name)
    }
    
    /// 验证字段是否存在
    pub fn validate_field(&self, field_name: &str) -> Result<(), BaseError> {
        if self.get_field(field_name).is_none() {
            return Err(BaseError::FieldNotFound(field_name.to_string()));
        }
        Ok(())
    }
    
    /// 验证查询参数
    pub fn validate_query(&self, query: &QueryParams) -> Result<(), BaseError> {
        // 验证选择字段
        for field in &query.fields {
            self.validate_field(field)?;
        }
        
        // 验证筛选条件
        for condition in &query.where_conditions {
            self.validate_field(&condition.field)?;
        }
        
        // 验证排序字段
        for (field, _) in &query.order_by {
            self.validate_field(field)?;
        }
        
        Ok(())
    }
}
```

### 4.2 FieldConfig 结构

```rust
/// 字段配置
#[derive(Debug, Clone)]
pub struct FieldConfig {
    /// 字段名
    pub name: String,
    
    /// 显示名称
    pub display_name: String,
    
    /// 字段类型
    pub field_type: FieldType,
    
    /// 是否必填
    pub required: bool,
    
    /// 默认值
    pub default_value: Option<serde_json::Value>,
    
    /// 验证规则
    pub validators: Vec<Validator>,
    
    /// 字段权限
    pub permissions: FieldPermissions,
    
    /// 是否可筛选
    pub filterable: bool,
    
    /// 是否可排序
    pub sortable: bool,
    
    /// 关联表配置（用于外键字段）
    pub relation: Option<RelationConfig>,
}

impl FieldConfig {
    /// 创建新的字段配置
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
    pub fn display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = name.into();
        self
    }
    
    /// 设置必填
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }
    
    /// 设置默认值
    pub fn default_value(mut self, value: serde_json::Value) -> Self {
        self.default_value = Some(value);
        self
    }
    
    /// 添加验证器
    pub fn validator(mut self, validator: Validator) -> Self {
        self.validators.push(validator);
        self
    }
    
    /// 设置字段权限
    pub fn permissions(mut self, permissions: FieldPermissions) -> Self {
        self.permissions = permissions;
        self
    }
    
    /// 设置关联表
    pub fn relation(mut self, relation: RelationConfig) -> Self {
        self.relation = Some(relation);
        self
    }
    
    /// 验证字段值
    pub fn validate(&self, value: &serde_json::Value) -> Result<(), BaseError> {
        // 检查必填
        if self.required && value.is_null() {
            return Err(BaseError::FieldRequired(self.name.clone()));
        }
        
        // 类型验证
        self.field_type.validate(value)?;
        
        // 自定义验证器
        for validator in &self.validators {
            validator.validate(&self.name, value)?;
        }
        
        Ok(())
    }
}
```

### 4.3 FieldType 枚举

```rust
/// 字段类型
#[derive(Debug, Clone, PartialEq)]
pub enum FieldType {
    /// 字符串
    String { max_length: usize },
    
    /// 整数
    Integer,
    
    /// 长整数
    BigInt,
    
    /// 浮点数
    Float,
    
    /// 双精度浮点数
    Double,
    
    /// 布尔值
    Boolean,
    
    /// 日期
    Date,
    
    /// 日期时间
    DateTime,
    
    /// 时间戳
    Timestamp,
    
    /// JSON
    Json,
    
    /// 枚举
    Enum { values: Vec<String> },
    
    /// 外键（关联其他表）
    ForeignKey { table: String, field: String },
    
    /// 文本（大文本）
    Text,
}

impl FieldType {
    /// 验证值类型
    pub fn validate(&self, value: &serde_json::Value) -> Result<(), BaseError> {
        use serde_json::Value;
        
        match (self, value) {
            (FieldType::String { max_length }, Value::String(s)) => {
                if s.len() > *max_length {
                    return Err(BaseError::FieldTooLong(s.len(), *max_length));
                }
            }
            (FieldType::Integer, Value::Number(n)) => {
                if !n.is_i64() {
                    return Err(BaseError::InvalidFieldType("integer".to_string()));
                }
            }
            (FieldType::Float | FieldType::Double, Value::Number(_)) => {}
            (FieldType::Boolean, Value::Bool(_)) => {}
            (FieldType::Json, Value::Object(_) | Value::Array(_)) => {}
            (FieldType::Enum { values }, Value::String(s)) => {
                if !values.contains(s) {
                    return Err(BaseError::InvalidEnumValue(s.clone(), values.clone()));
                }
            }
            _ => {
                if !value.is_null() {
                    return Err(BaseError::InvalidFieldType(format!("{:?}", self)));
                }
            }
        }
        
        Ok(())
    }
}
```

### 4.4 Validator 枚举

```rust
/// 验证器
#[derive(Debug, Clone)]
pub enum Validator {
    /// 最小长度
    MinLength(usize),
    
    /// 最大长度
    MaxLength(usize),
    
    /// 最小值
    Min(f64),
    
    /// 最大值
    Max(f64),
    
    /// 正则表达式
    Regex(String),
    
    /// 邮箱格式
    Email,
    
    /// 手机号格式
    Phone,
    
    /// URL 格式
    Url,
    
    /// 自定义验证函数
    Custom(Arc<dyn Fn(&str, &serde_json::Value) -> Result<(), BaseError> + Send + Sync>),
}

impl Validator {
    /// 执行验证
    pub fn validate(&self, field_name: &str, value: &serde_json::Value) -> Result<(), BaseError> {
        use serde_json::Value;
        
        match self {
            Validator::MinLength(min) => {
                if let Value::String(s) = value {
                    if s.len() < *min {
                        return Err(BaseError::ValidationFailed(
                            field_name.to_string(),
                            format!("最小长度为 {}", min),
                        ));
                    }
                }
            }
            Validator::MaxLength(max) => {
                if let Value::String(s) = value {
                    if s.len() > *max {
                        return Err(BaseError::ValidationFailed(
                            field_name.to_string(),
                            format!("最大长度为 {}", max),
                        ));
                    }
                }
            }
            Validator::Min(min) => {
                if let Value::Number(n) = value {
                    if let Some(v) = n.as_f64() {
                        if v < *min {
                            return Err(BaseError::ValidationFailed(
                                field_name.to_string(),
                                format!("最小值为 {}", min),
                            ));
                        }
                    }
                }
            }
            Validator::Max(max) => {
                if let Value::Number(n) = value {
                    if let Some(v) = n.as_f64() {
                        if v > *max {
                            return Err(BaseError::ValidationFailed(
                                field_name.to_string(),
                                format!("最大值为 {}", max),
                            ));
                        }
                    }
                }
            }
            Validator::Email => {
                if let Value::String(s) = value {
                    if !s.contains('@') {
                        return Err(BaseError::ValidationFailed(
                            field_name.to_string(),
                            "无效的邮箱格式".to_string(),
                        ));
                    }
                }
            }
            Validator::Custom(func) => {
                func(field_name, value)?;
            }
            _ => {}
        }
        
        Ok(())
    }
}
```

### 4.5 权限配置

```rust
/// 字段权限
#[derive(Debug, Clone, Default)]
pub struct FieldPermissions {
    /// 可读角色列表（空表示所有人可读）
    pub readable_roles: Vec<String>,
    
    /// 可写角色列表（空表示所有人可写）
    pub writable_roles: Vec<String>,
    
    /// 可筛选角色列表
    pub filterable_roles: Vec<String>,
    
    /// 可排序角色列表
    pub sortable_roles: Vec<String>,
}

impl FieldPermissions {
    /// 检查是否可读
    pub fn can_read(&self, user_roles: &[String]) -> bool {
        self.readable_roles.is_empty() || user_roles.iter().any(|r| self.readable_roles.contains(r))
    }
    
    /// 检查是否可写
    pub fn can_write(&self, user_roles: &[String]) -> bool {
        self.writable_roles.is_empty() || user_roles.iter().any(|r| self.writable_roles.contains(r))
    }
    
    /// 检查是否可筛选
    pub fn can_filter(&self, user_roles: &[String]) -> bool {
        self.filterable_roles.is_empty() || user_roles.iter().any(|r| self.filterable_roles.contains(r))
    }
    
    /// 检查是否可排序
    pub fn can_sort(&self, user_roles: &[String]) -> bool {
        self.sortable_roles.is_empty() || user_roles.iter().any(|r| self.sortable_roles.contains(r))
    }
}

/// 表权限配置
#[derive(Debug, Clone, Default)]
pub struct PermissionConfig {
    /// 读取权限要求
    pub read_permissions: Vec<Permission>,
    
    /// 创建权限要求
    pub create_permissions: Vec<Permission>,
    
    /// 更新权限要求
    pub update_permissions: Vec<Permission>,
    
    /// 删除权限要求
    pub delete_permissions: Vec<Permission>,
}

/// 权限定义
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Permission {
    /// 权限名称
    pub name: String,
    
    /// 权限描述
    pub description: String,
}

impl Permission {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
        }
    }
    
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }
}
```

### 4.6 辅助结构

```rust
/// 索引配置
#[derive(Debug, Clone)]
pub struct IndexConfig {
    /// 索引字段列表
    pub fields: Vec<String>,
}

/// 排序方向
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderDirection {
    Asc,
    Desc,
}

/// 时间戳字段配置
#[derive(Debug, Clone)]
pub struct TimestampFields {
    /// 创建时间字段
    pub created_at: Option<String>,
    
    /// 更新时间字段
    pub updated_at: Option<String>,
    
    /// 删除时间字段（软删除）
    pub deleted_at: Option<String>,
}

impl Default for TimestampFields {
    fn default() -> Self {
        Self {
            created_at: Some("created_at".to_string()),
            updated_at: Some("updated_at".to_string()),
            deleted_at: Some("deleted_at".to_string()),
        }
    }
}

/// 关联配置
#[derive(Debug, Clone)]
pub struct RelationConfig {
    /// 关联表名
    pub table: String,
    
    /// 关联字段
    pub field: String,
    
    /// 显示字段列表
    pub display_fields: Vec<String>,
    
    /// 关联类型
    pub relation_type: RelationType,
}

/// 关联类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationType {
    /// 一对一
    OneToOne,
    
    /// 一对多
    OneToMany,
    
    /// 多对多
    ManyToMany,
}
```
