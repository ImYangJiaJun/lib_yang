# 设计文档：模块表路由系统 - 第3部分：统一查询接口

## 5. 统一查询接口（TableQuery）

### 5.1 TableQuery 结构

```rust
/// 表查询构建器
///
/// 基于 yang-db 的类型安全查询构建器，集成 TableConfig 验证
pub struct TableQuery<'a> {
    /// 表配置
    config: Arc<TableConfig>,
    
    /// yang-db 查询构建器
    builder: QueryBuilder<'a>,
    
    /// 用户角色（用于权限检查）
    user_roles: Vec<String>,
}

impl<'a> TableQuery<'a> {
    /// 创建新的查询构建器
    pub fn new(config: Arc<TableConfig>, user_roles: Vec<String>) -> Result<Self, BaseError> {
        let builder = GlobalDatabase::table(&config.table_name)?;
        
        Ok(Self {
            config,
            builder,
            user_roles,
        })
    }
    
    /// 选择字段
    pub fn fields(mut self, fields: Vec<String>) -> Result<Self, BaseError> {
        // 验证字段存在性
        for field in &fields {
            self.config.validate_field(field)?;
            
            // 检查字段读取权限
            if let Some(field_config) = self.config.get_field(field) {
                if !field_config.permissions.can_read(&self.user_roles) {
                    return Err(BaseError::FieldPermissionDenied(field.clone()));
                }
            }
        }
        
        // 应用字段选择
        for field in fields {
            self.builder = self.builder.field(&field);
        }
        
        Ok(self)
    }
    
    /// 添加 WHERE 条件
    pub fn where_eq(mut self, field: String, value: serde_json::Value) -> Result<Self, BaseError> {
        // 验证字段
        self.config.validate_field(&field)?;
        
        // 检查筛选权限
        if let Some(field_config) = self.config.get_field(&field) {
            if !field_config.permissions.can_filter(&self.user_roles) {
                return Err(BaseError::FieldPermissionDenied(field.clone()));
            }
        }
        
        // 应用条件
        self.builder = self.builder.where_eq(&field, value);
        
        Ok(self)
    }
    
    /// 添加 WHERE IN 条件
    pub fn where_in(mut self, field: String, values: Vec<serde_json::Value>) -> Result<Self, BaseError> {
        self.config.validate_field(&field)?;
        
        if let Some(field_config) = self.config.get_field(&field) {
            if !field_config.permissions.can_filter(&self.user_roles) {
                return Err(BaseError::FieldPermissionDenied(field.clone()));
            }
        }
        
        self.builder = self.builder.where_in(&field, values);
        
        Ok(self)
    }
    
    /// 添加排序
    pub fn order_by(mut self, field: String, direction: OrderDirection) -> Result<Self, BaseError> {
        // 验证字段
        self.config.validate_field(&field)?;
        
        // 检查排序权限
        if let Some(field_config) = self.config.get_field(&field) {
            if !field_config.permissions.can_sort(&self.user_roles) {
                return Err(BaseError::FieldPermissionDenied(field.clone()));
            }
        }
        
        // 应用排序
        let dir = match direction {
            OrderDirection::Asc => yang_db::OrderDirection::Asc,
            OrderDirection::Desc => yang_db::OrderDirection::Desc,
        };
        self.builder = self.builder.order_by(&field, dir);
        
        Ok(self)
    }
    
    /// 设置分页
    pub fn paginate(mut self, page: usize, page_size: usize) -> Self {
        let offset = (page - 1) * page_size;
        self.builder = self.builder.limit(page_size).offset(offset);
        self
    }
    
    /// 执行查询
    pub async fn select<T>(self) -> Result<Vec<T>, BaseError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        self.builder
            .select()
            .await
            .map_err(|e| BaseError::DatabaseQueryFailed(e.to_string()))
    }
    
    /// 执行查询并返回分页结果
    pub async fn select_paginated<T>(
        self,
        page: usize,
        page_size: usize,
    ) -> Result<PaginatedResult<T>, BaseError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        // 获取总数
        let total = self.builder.clone().count().await
            .map_err(|e| BaseError::DatabaseQueryFailed(e.to_string()))?;
        
        // 获取数据
        let data = self.paginate(page, page_size).select().await?;
        
        Ok(PaginatedResult {
            data,
            total,
            page,
            page_size,
            total_pages: (total + page_size - 1) / page_size,
        })
    }
    
    /// 插入数据
    pub async fn insert(self, data: serde_json::Value) -> Result<u64, BaseError> {
        // 验证数据
        self.validate_insert_data(&data)?;
        
        // 执行插入
        self.builder
            .insert(data)
            .await
            .map_err(|e| BaseError::DatabaseExecuteFailed(e.to_string()))
    }
    
    /// 更新数据
    pub async fn update(self, data: serde_json::Value) -> Result<u64, BaseError> {
        // 验证数据
        self.validate_update_data(&data)?;
        
        // 执行更新
        self.builder
            .update(data)
            .await
            .map_err(|e| BaseError::DatabaseExecuteFailed(e.to_string()))
    }
    
    /// 删除数据
    pub async fn delete(self) -> Result<u64, BaseError> {
        // 如果配置了软删除，则更新删除时间字段
        if let Some(soft_delete_field) = &self.config.soft_delete_field {
            let now = chrono::Utc::now().timestamp();
            let data = serde_json::json!({ soft_delete_field: now });
            return self.update(data).await;
        }
        
        // 硬删除
        self.builder
            .delete()
            .await
            .map_err(|e| BaseError::DatabaseExecuteFailed(e.to_string()))
    }
    
    /// 验证插入数据
    fn validate_insert_data(&self, data: &serde_json::Value) -> Result<(), BaseError> {
        let obj = data.as_object()
            .ok_or_else(|| BaseError::InvalidData("数据必须是对象".to_string()))?;
        
        // 验证所有字段
        for field_config in &self.config.fields {
            let value = obj.get(&field_config.name).unwrap_or(&serde_json::Value::Null);
            
            // 检查写入权限
            if !field_config.permissions.can_write(&self.user_roles) {
                if !value.is_null() {
                    return Err(BaseError::FieldPermissionDenied(field_config.name.clone()));
                }
                continue;
            }
            
            // 验证字段值
            field_config.validate(value)?;
        }
        
        Ok(())
    }
    
    /// 验证更新数据
    fn validate_update_data(&self, data: &serde_json::Value) -> Result<(), BaseError> {
        let obj = data.as_object()
            .ok_or_else(|| BaseError::InvalidData("数据必须是对象".to_string()))?;
        
        // 只验证提供的字段
        for (field_name, value) in obj {
            if let Some(field_config) = self.config.get_field(field_name) {
                // 检查写入权限
                if !field_config.permissions.can_write(&self.user_roles) {
                    return Err(BaseError::FieldPermissionDenied(field_name.clone()));
                }
                
                // 验证字段值
                field_config.validate(value)?;
            } else {
                return Err(BaseError::FieldNotFound(field_name.clone()));
            }
        }
        
        Ok(())
    }
}
```

### 5.2 查询参数结构

```rust
/// 查询参数
#[derive(Debug, Clone, Deserialize)]
pub struct QueryParams {
    /// 选择的字段列表（空表示所有字段）
    #[serde(default)]
    pub fields: Vec<String>,
    
    /// WHERE 条件
    #[serde(default)]
    pub where_conditions: Vec<WhereCondition>,
    
    /// 排序
    #[serde(default)]
    pub order_by: Vec<(String, OrderDirection)>,
    
    /// 分页：页码（从 1 开始）
    #[serde(default = "default_page")]
    pub page: usize,
    
    /// 分页：每页大小
    #[serde(default = "default_page_size")]
    pub page_size: usize,
}

fn default_page() -> usize {
    1
}

fn default_page_size() -> usize {
    20
}

/// WHERE 条件
#[derive(Debug, Clone, Deserialize)]
pub struct WhereCondition {
    /// 字段名
    pub field: String,
    
    /// 操作符
    pub operator: WhereOperator,
    
    /// 值
    pub value: serde_json::Value,
}

/// WHERE 操作符
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WhereOperator {
    /// 等于
    Eq,
    
    /// 不等于
    Ne,
    
    /// 大于
    Gt,
    
    /// 大于等于
    Gte,
    
    /// 小于
    Lt,
    
    /// 小于等于
    Lte,
    
    /// IN
    In,
    
    /// NOT IN
    NotIn,
    
    /// LIKE
    Like,
    
    /// IS NULL
    IsNull,
    
    /// IS NOT NULL
    IsNotNull,
}
```

### 5.3 分页结果结构

```rust
/// 分页结果
#[derive(Debug, Clone, Serialize)]
pub struct PaginatedResult<T> {
    /// 数据列表
    pub data: Vec<T>,
    
    /// 总记录数
    pub total: usize,
    
    /// 当前页码
    pub page: usize,
    
    /// 每页大小
    pub page_size: usize,
    
    /// 总页数
    pub total_pages: usize,
}

impl<T> PaginatedResult<T> {
    /// 是否有下一页
    pub fn has_next(&self) -> bool {
        self.page < self.total_pages
    }
    
    /// 是否有上一页
    pub fn has_prev(&self) -> bool {
        self.page > 1
    }
}
```

### 5.4 使用示例

```rust
// 示例：查询用户列表
async fn query_users_example() -> Result<(), BaseError> {
    // 创建表配置
    let config = Arc::new(
        TableConfig::new("users")
            .display_name("用户表")
            .field(
                FieldConfig::new("id", FieldType::Integer)
                    .display_name("用户ID")
                    .required(true)
            )
            .field(
                FieldConfig::new("name", FieldType::String { max_length: 50 })
                    .display_name("姓名")
                    .required(true)
            )
            .field(
                FieldConfig::new("email", FieldType::String { max_length: 100 })
                    .display_name("邮箱")
                    .validator(Validator::Email)
            )
            .field(
                FieldConfig::new("status", FieldType::Enum {
                    values: vec!["active".to_string(), "inactive".to_string()]
                })
                    .display_name("状态")
            )
    );
    
    // 创建查询
    let user_roles = vec!["admin".to_string()];
    let result = TableQuery::new(config, user_roles)?
        .fields(vec!["id".to_string(), "name".to_string(), "email".to_string()])?
        .where_eq("status".to_string(), serde_json::json!("active"))?
        .order_by("id".to_string(), OrderDirection::Desc)?
        .select_paginated::<User>(1, 20)
        .await?;
    
    println!("总记录数: {}", result.total);
    println!("当前页: {}/{}", result.page, result.total_pages);
    
    Ok(())
}

// 示例：插入用户
async fn insert_user_example() -> Result<(), BaseError> {
    let config = Arc::new(/* ... */);
    let user_roles = vec!["admin".to_string()];
    
    let data = serde_json::json!({
        "name": "张三",
        "email": "zhangsan@example.com",
        "status": "active"
    });
    
    let affected = TableQuery::new(config, user_roles)?
        .insert(data)
        .await?;
    
    println!("插入成功，影响行数: {}", affected);
    
    Ok(())
}

// 示例：更新用户
async fn update_user_example() -> Result<(), BaseError> {
    let config = Arc::new(/* ... */);
    let user_roles = vec!["admin".to_string()];
    
    let data = serde_json::json!({
        "status": "inactive"
    });
    
    let affected = TableQuery::new(config, user_roles)?
        .where_eq("id".to_string(), serde_json::json!(1))?
        .update(data)
        .await?;
    
    println!("更新成功，影响行数: {}", affected);
    
    Ok(())
}

// 示例：删除用户（软删除）
async fn delete_user_example() -> Result<(), BaseError> {
    let config = Arc::new(/* ... */);
    let user_roles = vec!["admin".to_string()];
    
    let affected = TableQuery::new(config, user_roles)?
        .where_eq("id".to_string(), serde_json::json!(1))?
        .delete()
        .await?;
    
    println!("删除成功，影响行数: {}", affected);
    
    Ok(())
}
```
