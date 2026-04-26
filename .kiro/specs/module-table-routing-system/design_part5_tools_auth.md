# 设计文档：模块表路由系统 - 第5部分：全局工具与权限认证

## 7. 全局工具系统（GlobalTools）

### 7.1 GlobalTools 结构

```rust
/// 全局工具集合
///
/// 提供可扩展的全局工具注册机制
pub struct GlobalTools {
    /// Token 管理器
    pub token_manager: Arc<TokenManager>,
    
    /// 自定义工具
    custom_tools: Arc<RwLock<HashMap<String, Arc<dyn Any + Send + Sync>>>>,
}

impl GlobalTools {
    /// 创建新的全局工具集合
    pub fn new(token_manager: TokenManager) -> Self {
        Self {
            token_manager: Arc::new(token_manager),
            custom_tools: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// 注册自定义工具
    pub async fn register_tool<T: Any + Send + Sync>(
        &self,
        name: impl Into<String>,
        tool: T,
    ) {
        let mut tools = self.custom_tools.write().await;
        tools.insert(name.into(), Arc::new(tool));
    }
    
    /// 获取自定义工具
    pub async fn get_tool<T: Any + Send + Sync>(&self, name: &str) -> Option<Arc<T>> {
        let tools = self.custom_tools.read().await;
        tools.get(name)
            .and_then(|tool| tool.clone().downcast::<T>().ok())
    }
}
```

### 7.2 扩展工具示例

#### 7.2.1 Redis 工具

```rust
/// Redis 工具
pub struct RedisTools {
    client: redis::Client,
}

impl RedisTools {
    /// 创建新的 Redis 工具
    pub fn new(url: &str) -> Result<Self, BaseError> {
        let client = redis::Client::open(url)
            .map_err(|e| BaseError::RedisConnectionFailed(e.to_string()))?;
        
        Ok(Self { client })
    }
    
    /// 获取连接
    pub async fn get_connection(&self) -> Result<redis::aio::Connection, BaseError> {
        self.client
            .get_async_connection()
            .await
            .map_err(|e| BaseError::RedisConnectionFailed(e.to_string()))
    }
    
    /// 设置缓存
    pub async fn set(&self, key: &str, value: &str, ttl: usize) -> Result<(), BaseError> {
        let mut conn = self.get_connection().await?;
        redis::cmd("SETEX")
            .arg(key)
            .arg(ttl)
            .arg(value)
            .query_async(&mut conn)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }
    
    /// 获取缓存
    pub async fn get(&self, key: &str) -> Result<Option<String>, BaseError> {
        let mut conn = self.get_connection().await?;
        redis::cmd("GET")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| BaseError::RedisOperationFailed(e.to_string()))
    }
}

// 使用示例
async fn use_redis_example(tools: &GlobalTools) -> Result<(), BaseError> {
    // 获取 Redis 工具
    let redis = tools.get_tool::<RedisTools>("redis").await
        .ok_or(BaseError::ToolNotFound("redis".to_string()))?;
    
    // 设置缓存
    redis.set("user:1", "张三", 3600).await?;
    
    // 获取缓存
    if let Some(value) = redis.get("user:1").await? {
        println!("缓存值: {}", value);
    }
    
    Ok(())
}
```

#### 7.2.2 消息队列工具

```rust
/// 消息队列工具
pub struct MessageQueueTools {
    // 实现细节...
}

impl MessageQueueTools {
    /// 发送消息
    pub async fn send(&self, topic: &str, message: &str) -> Result<(), BaseError> {
        // 实现...
        Ok(())
    }
    
    /// 订阅消息
    pub async fn subscribe(&self, topic: &str) -> Result<(), BaseError> {
        // 实现...
        Ok(())
    }
}
```

## 8. 权限认证系统

### 8.1 User 结构

```rust
/// 用户信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// 用户 ID
    pub id: i64,
    
    /// 用户名
    pub username: String,
    
    /// 昵称
    pub nickname: String,
    
    /// 邮箱
    pub email: Option<String>,
    
    /// 角色列表
    pub roles: Vec<String>,
    
    /// 权限列表
    pub permissions: Vec<String>,
}

impl User {
    /// 检查是否有指定权限
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.contains(&permission.name)
    }
    
    /// 检查是否有指定角色
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.contains(&role.to_string())
    }
    
    /// 检查是否有任一角色
    pub fn has_any_role(&self, roles: &[String]) -> bool {
        roles.iter().any(|r| self.has_role(r))
    }
}
```

### 8.2 认证中间件

```rust
/// 认证中间件
///
/// 从请求中提取 Token 并验证，设置用户信息到上下文
pub struct AuthMiddleware {
    token_manager: Arc<TokenManager>,
}

impl AuthMiddleware {
    pub fn new(token_manager: Arc<TokenManager>) -> Self {
        Self { token_manager }
    }
    
    /// 验证请求
    pub async fn authenticate(&self, request: &Request) -> Result<Option<User>, BaseError> {
        // 获取 Token
        let token = match request.token() {
            Some(t) => t,
            None => return Ok(None), // 无 Token，返回 None
        };
        
        // 验证 Token
        let claims = self.token_manager.verify_token(token)?;
        
        // 从数据库加载用户信息
        let users = GlobalDatabase::table("users")?
            .where_eq("id", serde_json::json!(claims.user_id))
            .select::<User>()
            .await?;
        
        if users.is_empty() {
            return Err(BaseError::UserNotFound);
        }
        
        Ok(Some(users[0].clone()))
    }
}
```

### 8.3 权限检查流程

```mermaid
sequenceDiagram
    participant Client as 客户端
    participant Middleware as 认证中间件
    participant TM as TokenManager
    participant DB as 数据库
    participant Router as ModuleRouter
    participant Action as Action
    
    Client->>Middleware: 请求（带 Token）
    Middleware->>TM: 验证 Token
    TM-->>Middleware: Token Claims
    Middleware->>DB: 查询用户信息
    DB-->>Middleware: User
    Middleware->>Router: 转发请求（带 User）
    Router->>Router: 检查模块权限
    Router->>Action: 执行 Action
    Action->>Action: 检查 Action 权限
    Action->>Action: 检查字段权限
    Action-->>Client: 响应
```

## 9. 错误处理

### 9.1 BaseError 扩展

```rust
/// 基础错误类型扩展
#[derive(Debug, thiserror::Error)]
pub enum BaseError {
    // ... 现有错误类型 ...
    
    /// Action 未找到
    #[error("Action 未找到: {0}")]
    ActionNotFound(String),
    
    /// 权限被拒绝
    #[error("权限被拒绝: {0}")]
    PermissionDenied(String),
    
    /// 未授权
    #[error("未授权访问")]
    Unauthorized,
    
    /// 字段未找到
    #[error("字段未找到: {0}")]
    FieldNotFound(String),
    
    /// 字段权限被拒绝
    #[error("字段权限被拒绝: {0}")]
    FieldPermissionDenied(String),
    
    /// 字段必填
    #[error("字段必填: {0}")]
    FieldRequired(String),
    
    /// 字段过长
    #[error("字段过长: 当前长度 {0}，最大长度 {1}")]
    FieldTooLong(usize, usize),
    
    /// 无效的字段类型
    #[error("无效的字段类型: {0}")]
    InvalidFieldType(String),
    
    /// 无效的枚举值
    #[error("无效的枚举值: {0}，可选值: {1:?}")]
    InvalidEnumValue(String, Vec<String>),
    
    /// 验证失败
    #[error("字段 {0} 验证失败: {1}")]
    ValidationFailed(String, String),
    
    /// 参数缺失
    #[error("参数缺失: {0}")]
    ParamMissing(String),
    
    /// 参数无效
    #[error("参数 {0} 无效: {1}")]
    ParamInvalid(String, String),
    
    /// 无效的数据
    #[error("无效的数据: {0}")]
    InvalidData(String),
    
    /// 记录未找到
    #[error("记录未找到")]
    RecordNotFound,
    
    /// 表配置未设置
    #[error("表配置未设置")]
    TableConfigNotSet,
    
    /// 工具未找到
    #[error("工具未找到: {0}")]
    ToolNotFound(String),
    
    /// 用户未找到
    #[error("用户未找到")]
    UserNotFound,
    
    /// 密码错误
    #[error("密码错误")]
    InvalidPassword,
    
    /// Redis 连接失败
    #[error("Redis 连接失败: {0}")]
    RedisConnectionFailed(String),
    
    /// Redis 操作失败
    #[error("Redis 操作失败: {0}")]
    RedisOperationFailed(String),
}

impl BaseError {
    /// 获取错误码
    pub fn code(&self) -> i32 {
        match self {
            BaseError::ActionNotFound(_) => 404001,
            BaseError::PermissionDenied(_) => 403001,
            BaseError::Unauthorized => 401001,
            BaseError::FieldNotFound(_) => 400001,
            BaseError::FieldPermissionDenied(_) => 403002,
            BaseError::FieldRequired(_) => 400002,
            BaseError::FieldTooLong(_, _) => 400003,
            BaseError::InvalidFieldType(_) => 400004,
            BaseError::InvalidEnumValue(_, _) => 400005,
            BaseError::ValidationFailed(_, _) => 400006,
            BaseError::ParamMissing(_) => 400007,
            BaseError::ParamInvalid(_, _) => 400008,
            BaseError::InvalidData(_) => 400009,
            BaseError::RecordNotFound => 404002,
            BaseError::TableConfigNotSet => 500001,
            BaseError::ToolNotFound(_) => 500002,
            BaseError::UserNotFound => 404003,
            BaseError::InvalidPassword => 401002,
            BaseError::RedisConnectionFailed(_) => 500003,
            BaseError::RedisOperationFailed(_) => 500004,
            _ => 500000,
        }
    }
}
```

## 10. 完整使用示例

### 10.1 定义插件

```rust
use yang_base::plugin::{Plugin, ModuleRouter};
use yang_base::table::{TableConfig, FieldConfig, FieldType, Validator};
use yang_base::action::{Action, LoginAction};

/// 用户管理插件
pub struct UserPlugin;

#[async_trait]
impl Plugin for UserPlugin {
    fn name(&self) -> &str {
        "user"
    }
    
    fn version(&self) -> &str {
        "1.0.0"
    }
    
    fn init_sql(&self) -> Vec<String> {
        vec![
            r#"
            CREATE TABLE IF NOT EXISTS users (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                username VARCHAR(50) NOT NULL UNIQUE,
                password_hash VARCHAR(255) NOT NULL,
                nickname VARCHAR(100) NOT NULL,
                email VARCHAR(100),
                status ENUM('active', 'inactive') DEFAULT 'active',
                created_at BIGINT NOT NULL,
                updated_at BIGINT NOT NULL,
                deleted_at BIGINT DEFAULT 0,
                INDEX idx_username (username),
                INDEX idx_status (status)
            )
            "#.to_string(),
        ]
    }
    
    fn modules(&self) -> Vec<ModuleRouter> {
        vec![
            // 用户模块
            ModuleRouter::new("user")
                .display_name("用户管理")
                .table_config(
                    TableConfig::new("users")
                        .display_name("用户表")
                        .field(
                            FieldConfig::new("id", FieldType::BigInt)
                                .display_name("用户ID")
                                .required(true)
                        )
                        .field(
                            FieldConfig::new("username", FieldType::String { max_length: 50 })
                                .display_name("用户名")
                                .required(true)
                                .validator(Validator::MinLength(3))
                                .validator(Validator::MaxLength(50))
                        )
                        .field(
                            FieldConfig::new("nickname", FieldType::String { max_length: 100 })
                                .display_name("昵称")
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
                                .default_value(serde_json::json!("active"))
                        )
                        .primary_key("id")
                        .unique_index(vec!["username".to_string()])
                        .index(vec!["status".to_string()])
                )
                .register_builtin_actions() // 注册 add, put, del, get, select, table
                .register_action("login", LoginAction), // 注册自定义 action
        ]
    }
}
```

### 10.2 初始化系统

```rust
use yang_base::plugin::PluginManager;
use yang_base::database::{GlobalDatabase, DatabaseInitializer};
use yang_base::token::TokenManager;
use yang_base::tools::GlobalTools;
use yang_db::DatabaseConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 初始化数据库
    GlobalDatabase::init(
        "mysql://root:password@localhost/test",
        DatabaseConfig::default()
    ).await?;
    
    // 2. 创建插件管理器
    let plugin_manager = PluginManager::new();
    
    // 3. 注册插件
    plugin_manager.register(UserPlugin).await?;
    
    // 4. 初始化数据库表
    let initializer = DatabaseInitializer::new();
    initializer.initialize_all(&plugin_manager).await?;
    
    // 5. 创建全局工具
    let token_manager = TokenManager::new("your-secret-key");
    let tools = Arc::new(GlobalTools::new(token_manager));
    
    // 6. 注册自定义工具（可选）
    let redis = RedisTools::new("redis://localhost")?;
    tools.register_tool("redis", redis).await;
    
    // 7. 启动 HTTP 服务器
    // ... 使用 actix-web 或其他框架 ...
    
    Ok(())
}
```

### 10.3 HTTP 路由处理

```rust
use actix_web::{web, App, HttpServer, HttpRequest, HttpResponse};

/// 处理 API 请求
async fn handle_api_request(
    req: HttpRequest,
    body: web::Json<serde_json::Value>,
    plugin_manager: web::Data<PluginManager>,
    tools: web::Data<Arc<GlobalTools>>,
) -> HttpResponse {
    // 解析路径：/api/{plugin}/{module}/{action}
    let path: Vec<&str> = req.path().trim_start_matches("/api/").split('/').collect();
    
    if path.len() != 3 {
        return HttpResponse::BadRequest().json(ApiResponse::fail(400, "无效的路径"));
    }
    
    let (plugin_name, module_name, action_name) = (path[0], path[1], path[2]);
    
    // 获取插件
    let plugin = match plugin_manager.get(plugin_name).await {
        Some(p) => p,
        None => return HttpResponse::NotFound().json(ApiResponse::fail(404, "插件未找到")),
    };
    
    // 获取模块
    let modules = plugin.modules();
    let module = match modules.iter().find(|m| m.module_name == module_name) {
        Some(m) => m,
        None => return HttpResponse::NotFound().json(ApiResponse::fail(404, "模块未找到")),
    };
    
    // 创建请求对象
    let mut request = Request::new(body.into_inner());
    for (key, value) in req.headers() {
        if let Ok(v) = value.to_str() {
            request.headers.insert(key.to_string(), v.to_string());
        }
    }
    
    // 创建上下文
    let mut context = ActionContext::new(request, tools.get_ref().clone());
    
    // 认证
    let auth_middleware = AuthMiddleware::new(tools.token_manager.clone());
    if let Ok(Some(user)) = auth_middleware.authenticate(&context.request).await {
        context = context.with_user(user);
    }
    
    // 设置表配置
    if let Some(config) = &module.table_config {
        context = context.with_table_config(config.clone());
    }
    
    // 分发请求
    match module.dispatch(action_name, context).await {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(error) => {
            let response = ApiResponse::from_error(error);
            HttpResponse::Ok().json(response)
        }
    }
}

#[actix_web::main]
async fn start_server() -> std::io::Result<()> {
    let plugin_manager = web::Data::new(/* ... */);
    let tools = web::Data::new(/* ... */);
    
    HttpServer::new(move || {
        App::new()
            .app_data(plugin_manager.clone())
            .app_data(tools.clone())
            .route("/api/{plugin}/{module}/{action}", web::post().to(handle_api_request))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
```
