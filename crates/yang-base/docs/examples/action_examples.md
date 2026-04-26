# Action Trait 使用示例

本文档展示如何使用 Action Trait 定义和实现自定义 action。

## 基本示例

### 1. 简单的 Action

```rust
use yang_base::action::{Action, ActionContext, ApiResponse};
use yang_base::error::BaseError;
use async_trait::async_trait;
use serde_json::json;

pub struct HelloAction;

#[async_trait]
impl Action for HelloAction {
    async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError> {
        let name: String = context.param("name")?;
        
        Ok(ApiResponse::success(
            json!({ "message": format!("Hello, {}!", name) }),
            "操作成功"
        ))
    }
    
    fn name(&self) -> &str {
        "hello"
    }
}
```

### 2. 带权限的 Action

```rust
use yang_base::action::{Action, ActionContext, ApiResponse, Permission};
use yang_base::error::BaseError;
use async_trait::async_trait;
use serde_json::json;

pub struct DeleteUserAction;

#[async_trait]
impl Action for DeleteUserAction {
    async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError> {
        let user_id: i64 = context.param("id")?;
        
        // 执行删除逻辑
        // ...
        
        Ok(ApiResponse::success(
            json!({ "affected": 1 }),
            "删除成功"
        ))
    }
    
    fn name(&self) -> &str {
        "delete_user"
    }
    
    fn display_name(&self) -> &str {
        "删除用户"
    }
    
    fn description(&self) -> &str {
        "删除指定的用户账号"
    }
    
    fn permissions(&self) -> &[Permission] {
        &[
            Permission::new("user:delete"),
            Permission::new("admin:access")
        ]
    }
}
```

### 3. 公开 Action（不需要认证）

```rust
use yang_base::action::{Action, ActionContext, ApiResponse};
use yang_base::error::BaseError;
use async_trait::async_trait;
use serde_json::json;

pub struct LoginAction;

#[async_trait]
impl Action for LoginAction {
    async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError> {
        let username: String = context.param("username")?;
        let password: String = context.param("password")?;
        
        // 验证用户
        // ...
        
        Ok(ApiResponse::success(
            json!({
                "token": "xxx",
                "user": { "id": 1, "username": username }
            }),
            "登录成功"
        ))
    }
    
    fn name(&self) -> &str {
        "login"
    }
    
    fn display_name(&self) -> &str {
        "用户登录"
    }
    
    fn is_public(&self) -> bool {
        true // 登录不需要认证
    }
}
```

### 4. 带参数 Schema 的 Action

```rust
use yang_base::action::{Action, ActionContext, ApiResponse};
use yang_base::error::BaseError;
use async_trait::async_trait;
use serde_json::json;

pub struct CreateUserAction;

#[async_trait]
impl Action for CreateUserAction {
    async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError> {
        let username: String = context.param("username")?;
        let email: String = context.param("email")?;
        let age: Option<i64> = context.param_optional("age");
        
        // 创建用户
        // ...
        
        Ok(ApiResponse::success(
            json!({ "id": 1, "username": username, "email": email }),
            "创建成功"
        ))
    }
    
    fn name(&self) -> &str {
        "create_user"
    }
    
    fn display_name(&self) -> &str {
        "创建用户"
    }
    
    fn description(&self) -> &str {
        "创建新的用户账号"
    }
    
    fn params_schema(&self) -> Option<serde_json::Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "username": {
                    "type": "string",
                    "minLength": 3,
                    "maxLength": 20,
                    "description": "用户名"
                },
                "email": {
                    "type": "string",
                    "format": "email",
                    "description": "邮箱地址"
                },
                "age": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 150,
                    "description": "年龄（可选）"
                }
            },
            "required": ["username", "email"]
        }))
    }
}
```

## ActionContext 使用

### 获取参数

```rust
// 获取必填参数
let username: String = context.param("username")?;
let age: i64 = context.param("age")?;

// 获取可选参数
let email: Option<String> = context.param_optional("email");
let phone: Option<String> = context.param_optional("phone");
```

### 访问用户信息

```rust
// 获取当前用户
if let Some(user) = &context.user {
    println!("用户 ID: {}", user.id);
    println!("用户名: {}", user.username);
    
    // 检查权限
    if user.has_permission("admin:access") {
        // 执行管理员操作
    }
    
    // 检查角色
    if user.has_role("admin") {
        // 执行管理员操作
    }
}

// 获取用户角色列表
let roles = context.user_roles();
```

### 使用表查询

```rust
// 创建表查询构建器
let query = context.table_query()?;

// 执行查询
let users = query
    .fields(vec!["id".to_string(), "username".to_string()])?
    .where_eq("status".to_string(), json!("active"))?
    .order_by("created_at".to_string(), SortOrder::Desc)?
    .select::<serde_json::Value>()
    .await?;
```

## Permission 使用

```rust
use yang_base::action::Permission;

// 创建权限
let permission = Permission::new("user:create");

// 获取权限名称
assert_eq!(permission.name(), "user:create");

// 权限比较
let p1 = Permission::new("user:create");
let p2 = Permission::new("user:create");
assert_eq!(p1, p2);
```

## 完整示例

```rust
use yang_base::action::{Action, ActionContext, ApiResponse, Permission};
use yang_base::error::BaseError;
use async_trait::async_trait;
use serde_json::json;

/// 获取用户列表 Action
pub struct GetUsersAction;

#[async_trait]
impl Action for GetUsersAction {
    async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError> {
        // 获取分页参数
        let page: i64 = context.param_optional("page").unwrap_or(1);
        let page_size: i64 = context.param_optional("page_size").unwrap_or(10);
        
        // 获取筛选条件
        let status: Option<String> = context.param_optional("status");
        
        // 构建查询
        let mut query = context.table_query()?;
        
        // 应用筛选条件
        if let Some(status) = status {
            query = query.where_eq("status".to_string(), json!(status))?;
        }
        
        // 执行分页查询
        let result = query
            .order_by("created_at".to_string(), SortOrder::Desc)?
            .select_paginated::<serde_json::Value>(page, page_size)
            .await?;
        
        Ok(ApiResponse::success(result, "查询成功"))
    }
    
    fn name(&self) -> &str {
        "get_users"
    }
    
    fn display_name(&self) -> &str {
        "获取用户列表"
    }
    
    fn description(&self) -> &str {
        "分页查询用户列表，支持按状态筛选"
    }
    
    fn permissions(&self) -> &[Permission] {
        &[Permission::new("user:read")]
    }
    
    fn params_schema(&self) -> Option<serde_json::Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "page": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "页码（默认 1）"
                },
                "page_size": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 100,
                    "description": "每页数量（默认 10）"
                },
                "status": {
                    "type": "string",
                    "enum": ["active", "inactive", "banned"],
                    "description": "用户状态（可选）"
                }
            }
        }))
    }
}
```

## 注意事项

1. **异步方法**：`execute` 方法必须使用 `async_trait` 宏
2. **错误处理**：使用 `?` 操作符传播错误，返回 `BaseError`
3. **参数验证**：使用 `param` 获取必填参数，使用 `param_optional` 获取可选参数
4. **权限检查**：在 `permissions` 方法中定义所需权限，由路由器负责检查
5. **公开 Action**：登录、注册等不需要认证的 action 应设置 `is_public` 为 `true`
6. **参数 Schema**：提供 JSON Schema 可用于自动生成文档和前端表单
