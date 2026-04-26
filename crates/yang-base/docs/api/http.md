# HTTP 客户端模块

## 概述

HTTP 客户端模块提供了灵活的 HTTP 请求构建和响应处理能力，基于 `reqwest` 库实现。

## 核心组件

### 1. HttpClient

HTTP 客户端核心，提供全局单例和实例两种使用方式。

**功能特性：**
- 支持创建独立客户端实例
- 支持全局单例模式
- 支持设置默认 Token
- 支持所有常用 HTTP 方法（GET、POST、PUT、DELETE、PATCH）
- 支持自定义超时时间

**使用示例：**

```rust
use yang_base::http::HttpClient;

// 方式 1: 创建独立客户端
let client = HttpClient::new(30)?;

// 方式 2: 使用全局客户端
HttpClient::init_global(30)?;
let client = HttpClient::global()?;

// 设置默认 Token
client.set_default_token("your_token".to_string());
```

### 2. RequestBuilder

请求构建器，提供链式调用接口构建 HTTP 请求。

**功能特性：**
- 链式调用设置请求参数
- 支持设置请求头（单个或批量）
- 支持设置查询参数（单个或批量）
- 支持多种请求体格式（JSON、表单、文本、字节流）
- 支持 Bearer Token 认证
- 支持自定义超时时间

**使用示例：**

```rust
// GET 请求
let response = client
    .get("https://api.example.com/users")
    .header("X-Custom-Header", "value")
    .query("page", "1")
    .query("limit", "10")
    .bearer_token("your_token")
    .timeout(60)
    .send()
    .await?;

// POST JSON 请求
#[derive(Serialize)]
struct User {
    name: String,
    email: String,
}

let user = User {
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
};

let response = client
    .post("https://api.example.com/users")
    .json(&user)?
    .send()
    .await?;

// POST 表单请求
let response = client
    .post("https://api.example.com/login")
    .form(vec![("username", "alice"), ("password", "secret")])
    .send()
    .await?;
```

### 3. Response

响应处理器，提供便捷的响应解析方法。

**功能特性：**
- 获取状态码
- 检查响应是否成功（2xx）
- 获取响应头
- 解析响应体（文本、字节流、JSON）

**使用示例：**

```rust
// 检查状态码
if response.is_success() {
    println!("请求成功，状态码: {}", response.status());
}

// 解析为文本
let text = response.text().await?;

// 解析为 JSON
#[derive(Deserialize)]
struct User {
    id: u64,
    name: String,
}

let user: User = response.json().await?;

// 获取字节流
let bytes = response.bytes().await?;
```

## 完整示例

### 示例 1: 基本 GET 请求

```rust
use yang_base::http::HttpClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化全局客户端
    HttpClient::init_global(30)?;
    
    // 发起 GET 请求
    let response = HttpClient::global()?
        .get("https://api.github.com/users/octocat")
        .user_agent("MyApp/1.0")
        .send()
        .await?;
    
    // 检查响应
    if response.is_success() {
        let text = response.text().await?;
        println!("响应: {}", text);
    }
    
    Ok(())
}
```

### 示例 2: POST JSON 数据

```rust
use yang_base::http::HttpClient;
use serde::{Serialize, Deserialize};

#[derive(Serialize)]
struct CreateUser {
    name: String,
    email: String,
}

#[derive(Deserialize)]
struct UserResponse {
    id: u64,
    name: String,
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    HttpClient::init_global(30)?;
    
    let new_user = CreateUser {
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };
    
    let response = HttpClient::global()?
        .post("https://api.example.com/users")
        .bearer_token("your_access_token")
        .json(&new_user)?
        .send()
        .await?;
    
    if response.is_success() {
        let user: UserResponse = response.json().await?;
        println!("创建用户成功: {} (ID: {})", user.name, user.id);
    }
    
    Ok(())
}
```

### 示例 3: 带查询参数的请求

```rust
use yang_base::http::HttpClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    HttpClient::init_global(30)?;
    
    let response = HttpClient::global()?
        .get("https://api.example.com/search")
        .queries(vec![
            ("q", "rust"),
            ("page", "1"),
            ("limit", "10"),
            ("sort", "relevance"),
        ])
        .send()
        .await?;
    
    let results = response.text().await?;
    println!("搜索结果: {}", results);
    
    Ok(())
}
```

### 示例 4: 自定义请求头

```rust
use yang_base::http::HttpClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    HttpClient::init_global(30)?;
    
    let response = HttpClient::global()?
        .get("https://api.example.com/data")
        .headers(vec![
            ("X-API-Key", "your_api_key"),
            ("X-Request-ID", "12345"),
            ("Accept", "application/json"),
        ])
        .send()
        .await?;
    
    let data = response.json::<serde_json::Value>().await?;
    println!("数据: {:?}", data);
    
    Ok(())
}
```

### 示例 5: 设置默认 Token

```rust
use yang_base::http::HttpClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化全局客户端
    HttpClient::init_global(30)?;
    
    // 设置默认 Token
    let client = HttpClient::global()?;
    client.set_default_token("default_access_token".to_string());
    
    // 所有请求都会自动使用默认 Token
    let response1 = client
        .get("https://api.example.com/profile")
        .send()
        .await?;
    
    // 可以为单个请求覆盖 Token
    let response2 = client
        .get("https://api.example.com/admin")
        .bearer_token("admin_token")
        .send()
        .await?;
    
    Ok(())
}
```

## 错误处理

所有 HTTP 操作都返回 `Result<T, BaseError>`，可以使用 `?` 操作符进行错误传播：

```rust
use yang_base::error::BaseError;
use yang_base::http::HttpClient;

async fn fetch_data() -> Result<String, BaseError> {
    let response = HttpClient::global()?
        .get("https://api.example.com/data")
        .send()
        .await?;
    
    let text = response.text().await?;
    Ok(text)
}
```

## 测试

模块包含完整的单元测试和集成测试：

```bash
# 运行所有 HTTP 模块测试
cargo test --package yang-base --lib http

# 运行特定测试
cargo test --package yang-base --lib http::__tests__::client_test
```

## 依赖

- `reqwest`: HTTP 客户端库
- `serde`: 序列化/反序列化
- `tokio`: 异步运行时

## 注意事项

1. **全局客户端初始化**：全局客户端只能初始化一次，重复初始化会返回错误
2. **超时设置**：默认超时时间为创建客户端时指定的值，可以为单个请求覆盖
3. **Token 管理**：默认 Token 可以被单个请求的 Token 覆盖
4. **错误处理**：所有网络错误都会被转换为 `BaseError`，便于统一处理

## 实现的需求

本模块实现了以下需求：

- **需求 16.1**: 支持 GET、POST、PUT、DELETE、PATCH 等常用 HTTP 方法 ✓
- **需求 16.2**: 支持异步请求模式 ✓
- **需求 16.3**: 提供全局静态访问接口 ✓
- **需求 16.4**: 返回包含状态码、响应头和响应体的响应对象 ✓
- **需求 16.5**: 请求失败时返回包含错误详情的错误信息 ✓
