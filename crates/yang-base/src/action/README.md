# Request 结构体使用指南

## 概述

`Request` 结构体用于封装 Action 系统的 HTTP 请求信息，包括请求体、请求头、查询参数和路径参数。

## 基本用法

### 创建请求

```rust
use yang_base::action::Request;
use serde_json::json;

// 创建带有请求体的请求
let request = Request::new(json!({
    "username": "alice",
    "email": "alice@example.com"
}));

// 创建空请求
let empty_request = Request::default();
```

### 添加请求头

```rust
// 单个请求头
let request = Request::new(json!({}))
    .header("Content-Type", "application/json")
    .header("User-Agent", "MyApp/1.0");

// 批量添加请求头
use std::collections::HashMap;

let mut headers = HashMap::new();
headers.insert("Content-Type".to_string(), "application/json".to_string());
headers.insert("Authorization".to_string(), "Bearer token123".to_string());

let request = Request::new(json!({}))
    .headers(headers);
```

### 添加查询参数

```rust
// 单个查询参数
let request = Request::new(json!({}))
    .query("page", "1")
    .query("limit", "10")
    .query("sort", "name");

// 批量添加查询参数
let mut query = HashMap::new();
query.insert("page".to_string(), "1".to_string());
query.insert("limit".to_string(), "10".to_string());

let request = Request::new(json!({}))
    .queries(query);
```

### 添加路径参数

```rust
// 单个路径参数
let request = Request::new(json!({}))
    .path_param("id", "123")
    .path_param("action", "update");

// 批量添加路径参数
let mut path_params = HashMap::new();
path_params.insert("id".to_string(), "123".to_string());
path_params.insert("action".to_string(), "update".to_string());

let request = Request::new(json!({}))
    .path_params(path_params);
```

## 高级用法

### 链式调用

```rust
let request = Request::new(json!({
    "username": "alice",
    "password": "secret"
}))
.header("Content-Type", "application/json")
.header("Authorization", "Bearer token123")
.query("page", "1")
.query("limit", "10")
.path_param("id", "123");
```

### 提取 Token

```rust
// 从 Authorization 头提取 Bearer Token
let request = Request::new(json!({}))
    .header("Authorization", "Bearer my_secret_token");

if let Some(token) = request.token() {
    println!("Token: {}", token); // 输出: Token: my_secret_token
}
```

### 获取参数值

```rust
let request = Request::new(json!({}))
    .header("Content-Type", "application/json")
    .query("page", "1")
    .path_param("id", "123");

// 获取请求头
assert_eq!(request.get_header("Content-Type"), Some("application/json"));

// 获取查询参数
assert_eq!(request.get_query("page"), Some("1"));

// 获取路径参数
assert_eq!(request.get_path_param("id"), Some("123"));
```

## 完整示例

```rust
use yang_base::action::Request;
use serde_json::json;

fn main() {
    // 创建一个完整的请求
    let request = Request::new(json!({
        "user": {
            "name": "Alice",
            "age": 30,
            "roles": ["admin", "user"]
        },
        "metadata": {
            "timestamp": 1234567890,
            "source": "web"
        }
    }))
    .header("Content-Type", "application/json")
    .header("Authorization", "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...")
    .header("User-Agent", "MyApp/1.0")
    .query("page", "1")
    .query("limit", "20")
    .query("sort", "created_at")
    .path_param("plugin", "user")
    .path_param("module", "profile")
    .path_param("action", "update");

    // 使用请求
    println!("请求体: {:?}", request.body);
    println!("Token: {:?}", request.token());
    println!("页码: {:?}", request.get_query("page"));
    println!("插件: {:?}", request.get_path_param("plugin"));
}
```

## 注意事项

1. **Token 提取**：`token()` 方法只支持 `Bearer` 格式的 Token，格式为 `Authorization: Bearer <token>`
2. **大小写敏感**：请求头名称支持 `Authorization` 和 `authorization` 两种形式
3. **参数覆盖**：重复添加同名参数会覆盖之前的值
4. **链式调用**：所有添加方法都返回 `Self`，支持链式调用
5. **默认值**：使用 `Request::default()` 创建的请求，body 为 `serde_json::Value::Null`

## 与 Action 系统集成

在 Action 系统中，Request 通常通过 ActionContext 传递：

```rust
use yang_base::action::Request;
use yang_base::error::BaseError;

async fn my_action(context: ActionContext) -> Result<ApiResponse, BaseError> {
    // 从 context 获取 request
    let request = &context.request;
    
    // 提取 Token
    let token = request.token()
        .ok_or(BaseError::Unauthorized)?;
    
    // 获取查询参数
    let page = request.get_query("page")
        .unwrap_or("1");
    
    // 访问请求体
    let username = request.body["username"]
        .as_str()
        .ok_or(BaseError::ParamMissing("username".to_string()))?;
    
    // ... 处理业务逻辑 ...
    
    Ok(ApiResponse::success(json!({"status": "ok"}), "操作成功"))
}
```
