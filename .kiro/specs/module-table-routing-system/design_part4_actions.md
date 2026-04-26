# 设计文档：模块表路由系统 - 第4部分：Action 系统

## 6. Action 系统

### 6.1 Action Trait

```rust
/// Action 接口
///
/// 所有 action 必须实现此 trait
#[async_trait]
pub trait Action: Send + Sync {
    /// 执行 action
    ///
    /// # 参数
    /// - context: Action 执行上下文
    ///
    /// # 返回
    /// - Ok(ApiResponse): 执行成功
    /// - Err(BaseError): 执行失败
    async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError>;
    
    /// 获取 action 名称
    fn name(&self) -> &str;
    
    /// 获取 action 显示名称
    fn display_name(&self) -> &str {
        self.name()
    }
    
    /// 获取 action 描述
    fn description(&self) -> &str {
        ""
    }
    
    /// 获取权限要求
    fn permissions(&self) -> &[Permission] {
        &[]
    }
    
    /// 获取参数 Schema
    fn params_schema(&self) -> Option<serde_json::Value> {
        None
    }
    
    /// 是否为公开 action（不需要认证）
    fn is_public(&self) -> bool {
        false
    }
}
```

### 6.2 ActionContext 结构

```rust
/// Action 执行上下文
///
/// 包含请求信息、用户信息和全局工具
pub struct ActionContext {
    /// 请求数据
    pub request: Request,
    
    /// 当前用户（已认证）
    pub user: Option<User>,
    
    /// 全局工具
    pub tools: Arc<GlobalTools>,
    
    /// 表配置（如果 action 关联表）
    pub table_config: Option<Arc<TableConfig>>,
}

impl ActionContext {
    /// 创建新的上下文
    pub fn new(request: Request, tools: Arc<GlobalTools>) -> Self {
        Self {
            request,
            user: None,
            tools,
            table_config: None,
        }
    }
    
    /// 设置用户
    pub fn with_user(mut self, user: User) -> Self {
        self.user = Some(user);
        self
    }
    
    /// 设置表配置
    pub fn with_table_config(mut self, config: Arc<TableConfig>) -> Self {
        self.table_config = Some(config);
        self
    }
    
    /// 获取请求参数
    pub fn param<T: DeserializeOwned>(&self, key: &str) -> Result<T, BaseError> {
        self.request
            .body
            .get(key)
            .ok_or_else(|| BaseError::ParamMissing(key.to_string()))?
            .clone()
            .try_into()
            .map_err(|e| BaseError::ParamInvalid(key.to_string(), e.to_string()))
    }
    
    /// 获取可选参数
    pub fn param_optional<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.request
            .body
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }
    
    /// 创建表查询
    pub fn table_query(&self) -> Result<TableQuery, BaseError> {
        let config = self.table_config.as_ref()
            .ok_or(BaseError::TableConfigNotSet)?;
        
        let user_roles = self.user.as_ref()
            .map(|u| u.roles.clone())
            .unwrap_or_default();
        
        TableQuery::new(config.clone(), user_roles)
    }
    
    /// 获取用户角色
    pub fn user_roles(&self) -> Vec<String> {
        self.user.as_ref()
            .map(|u| u.roles.clone())
            .unwrap_or_default()
    }
}
```

### 6.3 Request 和 Response 结构

```rust
/// HTTP 请求
#[derive(Debug, Clone)]
pub struct Request {
    /// 请求体（JSON）
    pub body: serde_json::Value,
    
    /// 请求头
    pub headers: HashMap<String, String>,
    
    /// 查询参数
    pub query: HashMap<String, String>,
    
    /// 路径参数
    pub path_params: HashMap<String, String>,
}

impl Request {
    /// 创建新请求
    pub fn new(body: serde_json::Value) -> Self {
        Self {
            body,
            headers: HashMap::new(),
            query: HashMap::new(),
            path_params: HashMap::new(),
        }
    }
    
    /// 获取 Token
    pub fn token(&self) -> Option<&str> {
        self.headers
            .get("Authorization")
            .and_then(|v| v.strip_prefix("Bearer "))
    }
}

/// API 响应
#[derive(Debug, Clone, Serialize)]
pub struct ApiResponse {
    /// 状态码
    pub code: i32,
    
    /// 消息
    pub message: String,
    
    /// 数据
    pub data: Option<serde_json::Value>,
}

impl ApiResponse {
    /// 成功响应
    pub fn success(data: impl Serialize, message: impl Into<String>) -> Self {
        Self {
            code: 0,
            message: message.into(),
            data: Some(serde_json::to_value(data).unwrap_or(serde_json::Value::Null)),
        }
    }
    
    /// 失败响应
    pub fn fail(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }
    
    /// 从错误创建响应
    pub fn from_error(error: BaseError) -> Self {
        Self::fail(error.code(), error.to_string())
    }
}
```

### 6.4 内置 Actions

#### 6.4.1 AddAction（新增）

```rust
/// 新增 action
pub struct AddAction {
    config: Arc<TableConfig>,
}

impl AddAction {
    pub fn new(config: Arc<TableConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Action for AddAction {
    fn name(&self) -> &str {
        "add"
    }
    
    fn display_name(&self) -> &str {
        "新增"
    }
    
    fn permissions(&self) -> &[Permission] {
        &self.config.permissions.create_permissions
    }
    
    async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError> {
        // 获取数据
        let data: serde_json::Value = context.param("data")?;
        
        // 创建查询
        let affected = context
            .table_query()?
            .insert(data)
            .await?;
        
        Ok(ApiResponse::success(
            serde_json::json!({ "affected": affected }),
            "新增成功"
        ))
    }
}
```

#### 6.4.2 PutAction（更新）

```rust
/// 更新 action
pub struct PutAction {
    config: Arc<TableConfig>,
}

impl PutAction {
    pub fn new(config: Arc<TableConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Action for PutAction {
    fn name(&self) -> &str {
        "put"
    }
    
    fn display_name(&self) -> &str {
        "更新"
    }
    
    fn permissions(&self) -> &[Permission] {
        &self.config.permissions.update_permissions
    }
    
    async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError> {
        // 获取主键值
        let id: serde_json::Value = context.param(&self.config.primary_key)?;
        
        // 获取更新数据
        let data: serde_json::Value = context.param("data")?;
        
        // 执行更新
        let affected = context
            .table_query()?
            .where_eq(self.config.primary_key.clone(), id)?
            .update(data)
            .await?;
        
        Ok(ApiResponse::success(
            serde_json::json!({ "affected": affected }),
            "更新成功"
        ))
    }
}
```

#### 6.4.3 DelAction（删除）

```rust
/// 删除 action
pub struct DelAction {
    config: Arc<TableConfig>,
}

impl DelAction {
    pub fn new(config: Arc<TableConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Action for DelAction {
    fn name(&self) -> &str {
        "del"
    }
    
    fn display_name(&self) -> &str {
        "删除"
    }
    
    fn permissions(&self) -> &[Permission] {
        &self.config.permissions.delete_permissions
    }
    
    async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError> {
        // 获取主键值
        let id: serde_json::Value = context.param(&self.config.primary_key)?;
        
        // 执行删除
        let affected = context
            .table_query()?
            .where_eq(self.config.primary_key.clone(), id)?
            .delete()
            .await?;
        
        Ok(ApiResponse::success(
            serde_json::json!({ "affected": affected }),
            "删除成功"
        ))
    }
}
```

#### 6.4.4 GetAction（获取单条）

```rust
/// 获取单条记录 action
pub struct GetAction {
    config: Arc<TableConfig>,
}

impl GetAction {
    pub fn new(config: Arc<TableConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Action for GetAction {
    fn name(&self) -> &str {
        "get"
    }
    
    fn display_name(&self) -> &str {
        "获取详情"
    }
    
    fn permissions(&self) -> &[Permission] {
        &self.config.permissions.read_permissions
    }
    
    async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError> {
        // 获取主键值
        let id: serde_json::Value = context.param(&self.config.primary_key)?;
        
        // 查询数据
        let mut results = context
            .table_query()?
            .where_eq(self.config.primary_key.clone(), id)?
            .select::<serde_json::Value>()
            .await?;
        
        if results.is_empty() {
            return Err(BaseError::RecordNotFound);
        }
        
        Ok(ApiResponse::success(results.remove(0), "获取成功"))
    }
}
```

#### 6.4.5 SelectAction（列表查询）

```rust
/// 列表查询 action
pub struct SelectAction {
    config: Arc<TableConfig>,
}

impl SelectAction {
    pub fn new(config: Arc<TableConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Action for SelectAction {
    fn name(&self) -> &str {
        "select"
    }
    
    fn display_name(&self) -> &str {
        "列表查询"
    }
    
    fn permissions(&self) -> &[Permission] {
        &self.config.permissions.read_permissions
    }
    
    async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError> {
        // 解析查询参数
        let params: QueryParams = serde_json::from_value(context.request.body.clone())
            .map_err(|e| BaseError::ParamInvalid("query".to_string(), e.to_string()))?;
        
        // 构建查询
        let mut query = context.table_query()?;
        
        // 应用字段选择
        if !params.fields.is_empty() {
            query = query.fields(params.fields)?;
        }
        
        // 应用筛选条件
        for condition in params.where_conditions {
            match condition.operator {
                WhereOperator::Eq => {
                    query = query.where_eq(condition.field, condition.value)?;
                }
                WhereOperator::In => {
                    let values = condition.value.as_array()
                        .ok_or_else(|| BaseError::ParamInvalid("value".to_string(), "必须是数组".to_string()))?
                        .clone();
                    query = query.where_in(condition.field, values)?;
                }
                // ... 其他操作符
                _ => {}
            }
        }
        
        // 应用排序
        for (field, direction) in params.order_by {
            query = query.order_by(field, direction)?;
        }
        
        // 执行分页查询
        let result = query
            .select_paginated::<serde_json::Value>(params.page, params.page_size)
            .await?;
        
        Ok(ApiResponse::success(result, "查询成功"))
    }
}
```

#### 6.4.6 TableAction（表元数据）

```rust
/// 表元数据 action
///
/// 返回表的字段定义、权限配置等元数据，供前端动态生成表单和表格
pub struct TableAction {
    config: Arc<TableConfig>,
}

impl TableAction {
    pub fn new(config: Arc<TableConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Action for TableAction {
    fn name(&self) -> &str {
        "table"
    }
    
    fn display_name(&self) -> &str {
        "表元数据"
    }
    
    fn is_public(&self) -> bool {
        true // 元数据可以公开访问
    }
    
    async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError> {
        let user_roles = context.user_roles();
        
        // 构建字段元数据
        let fields: Vec<_> = self.config.fields.iter()
            .filter(|f| f.permissions.can_read(&user_roles))
            .map(|f| {
                serde_json::json!({
                    "name": f.name,
                    "display_name": f.display_name,
                    "type": format!("{:?}", f.field_type),
                    "required": f.required,
                    "filterable": f.filterable && f.permissions.can_filter(&user_roles),
                    "sortable": f.sortable && f.permissions.can_sort(&user_roles),
                    "writable": f.permissions.can_write(&user_roles),
                })
            })
            .collect();
        
        // 构建元数据
        let metadata = serde_json::json!({
            "table_name": self.config.table_name,
            "display_name": self.config.display_name,
            "primary_key": self.config.primary_key,
            "fields": fields,
            "default_order": self.config.default_order,
        });
        
        Ok(ApiResponse::success(metadata, "获取成功"))
    }
}
```

### 6.5 自定义 Action 示例

```rust
/// 自定义 action 示例：用户登录
pub struct LoginAction;

#[async_trait]
impl Action for LoginAction {
    fn name(&self) -> &str {
        "login"
    }
    
    fn display_name(&self) -> &str {
        "用户登录"
    }
    
    fn is_public(&self) -> bool {
        true // 登录不需要认证
    }
    
    async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError> {
        // 获取参数
        let username: String = context.param("username")?;
        let password: String = context.param("password")?;
        
        // 查询用户
        let users = GlobalDatabase::table("users")?
            .where_eq("username", serde_json::json!(username))
            .select::<User>()
            .await?;
        
        if users.is_empty() {
            return Err(BaseError::UserNotFound);
        }
        
        let user = &users[0];
        
        // 验证密码
        if !verify_password(&password, &user.password_hash) {
            return Err(BaseError::InvalidPassword);
        }
        
        // 生成 Token
        let token = context.tools.token_manager.generate_token(
            user.id,
            user.username.clone(),
            user.roles.clone(),
        )?;
        
        Ok(ApiResponse::success(
            serde_json::json!({
                "token": token,
                "user": user,
            }),
            "登录成功"
        ))
    }
}
```
