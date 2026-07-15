# yang-base 项目结构解析

## 项目概述

yang-base 是 YANG 项目的基础库，提供插件管理、数据库访问、HTTP 客户端、JWT Token 管理等核心功能，用于构建后端服务器应用。

## 模块架构图

```
yang-base/
├── action/          # Action 系统 - 业务逻辑处理
├── database/        # 数据库管理 - MySQL 和 Redis
├── error/           # 错误处理 - 统一错误类型
├── http/            # HTTP 客户端 - 外部 API 调用
├── plugin/          # 插件管理 - 模块化扩展
├── router/          # 路由系统 - 请求分发
├── table/           # 表配置系统 - 数据表元数据
└── token/           # Token 管理 - JWT 认证
```

---

## 核心模块详解

### 1. 📦 action - Action 系统模块

**职责**: 提供业务逻辑处理框架，类似于 MVC 中的 Controller 层

**核心组件**:
- `Action` trait - 定义 Action 接口
- `ActionContext` - Action 执行上下文，包含请求、数据库、配置等
- `Request` - 请求封装（请求体、请求头、查询参数、路径参数）
- `ApiResponse` - 统一响应格式

**内置 Actions** (`builtin/`):
- `AddAction` - 添加数据（INSERT）
- `PutAction` - 更新数据（UPDATE）
- `DelAction` - 删除数据（DELETE）
- `GetAction` - 获取单条数据（SELECT WHERE id）
- `SelectAction` - 查询列表数据（SELECT with pagination）
- `TableAction` - 获取表配置信息

**使用场景**:
```rust
// 自定义 Action
struct LoginAction;

#[async_trait]
impl Action for LoginAction {
    async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError> {
        let username = context.param::<String>("username")?;
        let password = context.param::<String>("password")?;
        
        // 业务逻辑处理
        let user = authenticate(username, password).await?;
        
        Ok(ApiResponse::success(json!({"user": user}), "登录成功"))
    }
}
```

**文件结构**:
```
action/
├── action_trait.rs      # Action trait 定义
├── context.rs           # ActionContext 实现
├── request.rs           # Request 结构体
├── response.rs          # ApiResponse 结构体
├── builtin/             # 内置 Actions
│   ├── add.rs / add_action.rs
│   ├── put.rs / put_action.rs
│   ├── del.rs / del_action.rs
│   ├── get.rs / get_action.rs
│   ├── select.rs / select_action.rs
│   └── table.rs / table_action.rs
└── __tests__/           # 单元测试
```

---

### 2. 🗄️ database - 数据库管理模块

**职责**: 提供全局数据库访问接口，支持 MySQL 和 Redis

**核心组件**:
- `GlobalDatabase` - MySQL 全局单例
- `GlobalRedis` - Redis 全局单例
- `DatabaseInitializer` - 数据库初始化器（支持插件 SQL 脚本）

**功能特性**:
- 线程安全的全局单例模式（使用 `OnceLock`）
- 封装 `yang-db` 库的功能
- 支持查询构建器（QueryBuilder）
- 支持事务（Transaction）
- 支持原生 SQL 查询

**使用场景**:
```rust
// 初始化 MySQL
GlobalDatabase::init("mysql://user:pass@localhost/db", DatabaseConfig::default()).await?;

// 使用查询构建器
let users = GlobalDatabase::table("users")?
    .where_eq("status", "active")?
    .select::<User>()
    .await?;

// 初始化 Redis
GlobalRedis::init("redis://127.0.0.1:6379", RedisConfig::default()).await?;

// 使用 Redis
GlobalRedis::set("key", "value", Some(3600)).await?;
let value = GlobalRedis::get("key").await?;
```

**文件结构**:
```
database/
├── global.rs            # GlobalDatabase 实现
├── global_redis.rs      # GlobalRedis 实现
├── initializer.rs       # DatabaseInitializer 实现
└── mod.rs               # 模块导出
```

---

### 3. ⚠️ error - 错误处理模块

**职责**: 定义统一的错误类型，便于错误处理和传播

**核心组件**:
- `BaseError` - 统一错误枚举类型

**错误分类**:
- **1xxxxx**: 插件管理错误
- **2xxxxx**: 数据库错误（MySQL）
- **21xxxx**: Redis 错误
- **3xxxxx**: HTTP 客户端错误
- **4xxxxx**: Token 管理错误
- **5xxxxx**: 序列化错误
- **6xxxxx**: 字段验证错误
- **7xxxxx**: Action 系统错误
- **9xxxxx**: 通用错误

**使用场景**:
```rust
// 错误传播
fn validate_user(user: &User) -> Result<(), BaseError> {
    if user.name.is_empty() {
        return Err(BaseError::ParamInvalid("name".to_string(), "名称不能为空".to_string()));
    }
    Ok(())
}

// 错误码获取
let error = BaseError::DatabaseConnectionFailed("连接超时".to_string());
println!("错误码: {}", error.code()); // 输出: 200001
```

**文件结构**:
```
error/
└── mod.rs               # BaseError 定义
```

---

### 4. 🌐 http - HTTP 客户端模块

**职责**: 提供 HTTP 请求能力，用于调用外部 API

**核心组件**:
- `HttpClient` - HTTP 客户端（支持全局单例和独立实例）
- `RequestBuilder` - 请求构建器（链式调用）
- `Response` - 响应处理器

**功能特性**:
- 支持所有 HTTP 方法（GET、POST、PUT、DELETE、PATCH）
- 支持设置请求头、查询参数
- 支持多种请求体格式（JSON、表单、文本、字节流）
- 支持 Bearer Token 认证
- 支持自定义超时时间
- 支持默认 Token 设置

**使用场景**:
```rust
// 初始化全局客户端
HttpClient::init_global(30)?;

// GET 请求
let response = HttpClient::global()?
    .get("https://api.example.com/users")
    .query("page", "1")
    .bearer_token("your_token")
    .send()
    .await?;

// POST JSON 请求
let user = json!({"name": "Alice", "email": "alice@example.com"});
let response = HttpClient::global()?
    .post("https://api.example.com/users")
    .json(&user)?
    .send()
    .await?;
```

**文件结构**:
```
http/
├── client.rs            # HttpClient 实现
├── request.rs           # RequestBuilder 实现
├── response.rs          # Response 实现
└── __tests__/           # 单元测试
```

---

### 5. 🔌 plugin - 插件管理模块

**职责**: 提供插件注册、管理和生命周期控制

**核心组件**:
- `Plugin` trait - 插件接口定义
- `PluginManager` - 插件管理器

**功能特性**:
- 插件注册和查找
- 插件依赖管理（拓扑排序）
- 插件生命周期回调（on_register、on_init、on_shutdown）
- 插件配置管理（JSON Schema 验证）
- 数据库初始化 SQL 脚本支持
- 数据库迁移脚本支持

**使用场景**:
```rust
// 定义插件
struct UserPlugin;

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
            "CREATE TABLE IF NOT EXISTS users (id INT PRIMARY KEY, name VARCHAR(100))".to_string()
        ]
    }
    
    async fn on_init(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("用户插件初始化完成");
        Ok(())
    }
}

// 注册插件
let manager = PluginManager::new();
manager.register(UserPlugin).await?;
```

**文件结构**:
```
plugin/
└── mod.rs               # Plugin trait 和 PluginManager 实现
```

---

### 6. 🚦 router - 路由系统模块

**职责**: 提供请求路由和分发功能

**核心组件**:
- `ModuleRouter` - 模块路由器

**功能特性**:
- 注册和查找 Action
- 路由匹配（plugin/module/action）
- Action 执行

**使用场景**:
```rust
// 创建路由器
let router = ModuleRouter::new();

// 注册 Action
router.register("user", "profile", "get", Arc::new(GetProfileAction));
router.register("user", "profile", "update", Arc::new(UpdateProfileAction));

// 执行 Action
let response = router.execute("user", "profile", "get", context).await?;
```

**文件结构**:
```
router/
├── module_router.rs     # ModuleRouter 实现
└── __tests__/           # 单元测试
```

---

### 7. 📋 table - 表配置系统模块

**职责**: 提供数据表元数据管理和查询构建

**核心组件**:
- `TableConfig` - 表配置（字段定义、主键、索引等）
- `FieldConfig` - 字段配置（类型、验证规则、权限等）
- `FieldType` - 字段类型枚举
- `TableQuery` - 表查询构建器（封装 yang-db QueryBuilder）
- `QueryParams` - 查询参数（分页、排序、筛选）
- `Validator` - 字段验证器

**功能特性**:
- 字段类型定义（字符串、整数、浮点数、布尔、日期、JSON 等）
- 字段验证（必填、长度、范围、正则、枚举等）
- 字段权限控制（只读、隐藏、角色权限）
- 查询构建（WHERE、ORDER BY、LIMIT、OFFSET）
- 批量字段配置

**使用场景**:
```rust
// 定义表配置
let mut table_config = TableConfig::new("users");
table_config.set_primary_key("id");

// 添加字段配置
let name_field = FieldConfig::new("name", FieldType::String)
    .required(true)
    .max_length(100)
    .label("用户名");
table_config.add_field(name_field);

// 使用查询构建器
let query = TableQuery::new(db, &table_config);
let users = query
    .where_eq("status", "active")?
    .order_by("created_at", SortOrder::Desc)?
    .page(1, 10)?
    .select::<User>()
    .await?;
```

**文件结构**:
```
table/
├── table_config.rs      # TableConfig 实现
├── field_config.rs      # FieldConfig 实现
├── field_type.rs        # FieldType 枚举
├── table_query.rs       # TableQuery 实现
├── query_params.rs      # QueryParams 实现
├── validator.rs         # Validator 实现
└── __tests__/           # 单元测试
```

---

### 8. 🔐 token - Token 管理模块

**职责**: 提供 JWT Token 生成、验证和管理

**核心组件**:
- `TokenManager` - Token 管理器
- `Claims` - JWT 声明（用户信息、过期时间等）

**功能特性**:
- JWT Token 生成
- JWT Token 验证
- Token 过期检查
- 自定义声明支持

**使用场景**:
```rust
// 创建 Token 管理器
let manager = TokenManager::new("your_secret_key")?;

// 生成 Token
let claims = Claims {
    sub: "user123".to_string(),
    exp: (chrono::Utc::now() + chrono::Duration::hours(24)).timestamp() as usize,
    ..Default::default()
};
let token = manager.generate(&claims)?;

// 验证 Token
let claims = manager.verify(&token)?;
println!("用户 ID: {}", claims.sub);
```

**文件结构**:
```
token/
├── manager.rs           # TokenManager 实现
└── __tests__/           # 单元测试
```

---

## 模块依赖关系

```
┌─────────────────────────────────────────────────────────┐
│                      应用层                              │
│  (使用 yang-base 构建的后端服务)                         │
└─────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│                    action (业务逻辑层)                   │
│  - Action trait                                         │
│  - ActionContext                                        │
│  - 内置 Actions (CRUD)                                  │
└─────────────────────────────────────────────────────────┘
         ↓              ↓              ↓              ↓
┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
│   database   │ │     http     │ │    token     │ │    table     │
│  (数据访问)  │ │  (外部调用)  │ │   (认证)     │ │  (元数据)    │
└──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘
         ↓                                                  ↓
┌──────────────┐                                   ┌──────────────┐
│    plugin    │                                   │   validator  │
│  (插件管理)  │                                   │  (字段验证)  │
└──────────────┘                                   └──────────────┘
         ↓
┌─────────────────────────────────────────────────────────┐
│                    error (错误处理层)                    │
│  - BaseError (统一错误类型)                              │
└─────────────────────────────────────────────────────────┘
         ↓
┌─────────────────────────────────────────────────────────┐
│                  yang-db (数据库抽象层)                  │
│  - QueryBuilder                                         │
│  - Transaction                                          │
│  - Redis Client                                         │
└─────────────────────────────────────────────────────────┘
```

---

## 测试结构

每个模块都包含完整的测试：

### 单元测试
位于各模块的 `__tests__/` 目录下：
```
src/
├── action/__tests__/
├── http/__tests__/
├── router/__tests__/
└── table/__tests__/
```

### 集成测试
位于项目根目录的 `tests/` 目录下：
```
tests/
├── database_test.rs
├── database_integration_test.rs
├── database_initializer_test.rs
├── error_test.rs
├── field_type_test.rs
├── plugin_test.rs
├── table_query_crud_test.rs
└── table_query_paginate_test.rs
```

### 示例代码
位于 `examples/` 目录下：
```
examples/
├── batch_field_config.rs
├── database_example.rs
├── database_initializer_example.rs
└── field_type_demo.rs
```

---

## 文档结构

### 根目录文档
```
crates/yang-base/
├── README.md                    # 项目主文档
├── USAGE_GUIDE.md               # 使用指南
├── QUICK_REFERENCE.md           # 快速参考
├── INSTALL.md                   # 安装指南
├── ASYNC_AWAIT_GUIDE.md         # 异步编程指南
├── BATCH_FIELD_CONFIG.md        # 批量字段配置
├── TABLE_CONFIG_GUIDE.md        # 表配置指南
└── REDIS_GUIDE.md               # Redis 使用指南
```

### 模块文档
```
src/
├── action/README.md             # Action 系统文档
├── action/ACTION_EXAMPLES.md    # Action 示例
├── action/builtin/README.md     # 内置 Actions 文档
├── database/README.md           # 数据库管理文档
└── http/README.md               # HTTP 客户端文档
```

---

## 设计原则

### 1. 模块化设计
每个模块职责单一，相互独立，便于维护和扩展。

### 2. 全局单例模式
数据库、HTTP 客户端等资源使用全局单例，避免重复创建连接。

### 3. 统一错误处理
所有模块使用统一的 `BaseError` 类型，便于错误传播和处理。

### 4. 异步优先
所有 I/O 操作都是异步的，基于 `tokio` 运行时。

### 5. 类型安全
充分利用 Rust 的类型系统，在编译期捕获错误。

### 6. 文档完整
所有公开 API 都有完整的中文文档注释。

---

## 使用流程

### 典型的应用启动流程

```rust
use yang_base::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 初始化日志
    env_logger::init();
    
    // 2. 初始化数据库
    database::GlobalDatabase::init(
        "mysql://user:pass@localhost/db",
        DatabaseConfig::default()
    ).await?;
    
    // 3. 初始化 Redis
    database::GlobalRedis::init(
        "redis://127.0.0.1:6379",
        RedisConfig::default()
    ).await?;
    
    // 4. 初始化 HTTP 客户端
    http::HttpClient::init_global(30)?;
    
    // 5. 创建插件管理器
    let plugin_manager = plugin::PluginManager::new();
    
    // 6. 注册插件
    plugin_manager.register(UserPlugin).await?;
    plugin_manager.register(OrderPlugin).await?;
    
    // 7. 初始化数据库表（执行插件的 init_sql）
    let initializer = database::DatabaseInitializer::new(plugin_manager);
    initializer.initialize().await?;
    
    // 8. 创建路由器
    let router = router::ModuleRouter::new();
    
    // 9. 注册 Actions
    router.register("user", "profile", "get", Arc::new(GetProfileAction));
    router.register("user", "profile", "update", Arc::new(UpdateProfileAction));
    
    // 10. 启动 HTTP 服务器
    // ... 启动 Actix-web 或其他 HTTP 框架
    
    Ok(())
}
```

---

## 总结

yang-base 是一个功能完整、设计良好的 Rust 后端基础库，提供了：

✅ **完整的数据访问层** - MySQL 和 Redis 支持  
✅ **灵活的业务逻辑框架** - Action 系统  
✅ **强大的插件机制** - 模块化扩展  
✅ **便捷的 HTTP 客户端** - 外部 API 调用  
✅ **安全的认证系统** - JWT Token 管理  
✅ **丰富的表配置系统** - 元数据管理和验证  
✅ **统一的错误处理** - BaseError 类型  
✅ **完整的测试覆盖** - 单元测试和集成测试  
✅ **详细的文档** - 中文文档和示例代码  

适合用于构建企业级 Rust 后端应用。
