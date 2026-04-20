# 实现计划：yang-base 系统模块管理

## 概述

本实现计划将 yang-base 系统模块管理的设计转化为可执行的编码任务。系统包含五个核心模块：模块管理（module）、数据库管理（database）、HTTP 客户端（http）、Token 管理（token）和错误处理（error）。

所有数据库相关操作都基于 yang-db 库实现，确保类型安全和统一的数据库访问接口。

实现将按照模块依赖关系进行，首先实现基础的错误处理模块，然后是模块管理和数据库管理（使用 yang-db），最后是 HTTP 客户端和 Token 管理模块。

## 任务列表

- [x] 1. 项目结构搭建和基础配置
  - 创建 yang-base crate 的模块目录结构
  - 配置 Cargo.toml 依赖项
  - 创建各模块的 mod.rs 文件
  - _需求: 1.1, 2.1, 16.1_

- [x] 2. 实现错误处理模块（error）
  - [x] 2.1 定义 BaseError 枚举类型
    - 实现所有错误变体（插件管理、数据库、HTTP、Token、序列化、通用错误）
    - 使用 thiserror 派生 Error trait
    - 添加中文错误消息
    - _需求: 10.1, 10.2_
  
  - [x] 2.2 编写错误处理单元测试
    - 测试错误消息格式
    - 测试错误类型转换（From trait）
    - _需求: 10.1, 10.2_

- [x] 3. 实现模块管理模块（module）
  - [x] 3.1 定义 Module trait
    - 实现 name、version、dependencies 方法
    - 实现 init_sql、migration_sql 方法
    - 实现 config_schema 方法
    - 实现生命周期钩子（on_register、on_init、on_shutdown）
    - 使用 async_trait 支持异步方法
    - _需求: 2.1, 2.2, 2.3, 2.4, 2.5, 12.1, 12.2, 12.3, 14.1_
  
  - [x] 3.2 实现 ModuleManager 结构体
    - 实现 new 和 default 方法
    - 实现 register 方法（模块注册和唯一性验证）
    - 实现 get 和 get_all 方法（模块查找）
    - 实现拓扑排序算法（topological_sort）
    - 实现 load_config 和 get_config 方法
    - 实现 validate_config 方法（JSON Schema 验证）
    - 实现 shutdown 方法
    - 使用 Arc<RwLock<HashMap>> 确保线程安全
    - _需求: 1.1, 1.2, 1.3, 1.4, 1.5, 8.1, 8.2, 8.3, 8.4, 8.5, 12.5, 14.2, 14.3, 14.4, 14.5, 15.1, 15.5_
  
  - [x] 3.3 编写模块管理单元测试
    - 测试模块注册和重复注册
    - 测试模块查找（存在和不存在）
    - 测试依赖关系验证
    - 测试拓扑排序算法
    - 测试配置加载和验证
    - _需求: 1.1, 1.2, 1.4, 1.5, 8.2, 8.3, 8.4, 8.5, 14.3, 14.4_

- [x] 4. 实现数据库管理模块（database）
  - [x] 4.1 实现 GlobalDatabase 结构体
    - 使用 OnceLock 实现全局单例
    - 实现 init 方法（初始化全局数据库）
    - 实现 get 方法（获取数据库实例）
    - 实现 table、query、execute、transaction 方法（封装 yang-db API）
    - _需求: 6.1, 6.2, 6.3, 6.4, 6.5, 13.1, 13.2, 13.3, 13.4, 13.5, 15.2, 15.3_
  
  - [x] 4.2 实现 DatabaseInitializer 结构体
    - 实现 new 方法
    - 实现 initialize_all 方法（遍历模块并初始化）
    - 实现 initialize_with_transaction 方法（事务模式）
    - 实现 initialize_without_transaction 方法（非事务模式）
    - 实现 create_migration_table 方法
    - 实现 run_migrations 和 run_migrations_in_tx 方法
    - 实现 is_migration_executed 和 record_migration 方法
    - _需求: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 5.1, 5.2, 5.3, 5.4, 5.5, 9.1, 9.2, 9.3, 9.4, 9.5, 11.1, 11.2, 11.3, 11.4, 11.5_
  
  - [x] 4.3 编写数据库管理集成测试
    - 测试全局数据库初始化
    - 测试数据库初始化流程（事务和非事务模式）
    - 测试迁移记录表创建
    - 测试迁移执行和幂等性
    - 使用 testcontainers 创建测试数据库
    - _需求: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 6.1, 6.4, 9.2, 9.3, 9.4, 11.2, 11.3, 11.4_

- [x] 5. Checkpoint - 核心模块验证
  - 确保所有测试通过，询问用户是否有问题

- [x] 6. 实现 HTTP 客户端模块（http）
  - [x] 6.1 实现 HttpClient 结构体
    - 实现 new 方法（创建客户端）
    - 实现 init_global 和 global 方法（全局单例）
    - 实现 set_default_token 方法
    - 实现 request、get、post、put、delete、patch 方法
    - 使用 reqwest 作为底层 HTTP 库
    - _需求: 16.1, 16.2, 16.3, 16.4, 16.5_
  
  - [x] 6.2 实现 RequestBuilder 结构体
    - 实现 new 方法
    - 实现 header、headers、content_type、bearer_token、user_agent 方法
    - 实现 query 和 queries 方法
    - 实现 json、form、body、text 方法（请求体设置）
    - 实现 timeout 方法
    - 实现 send 方法（发送请求）
    - _需求: 17.1, 17.2, 17.3, 17.4, 17.5, 18.1, 18.2, 18.3, 18.4, 18.5, 19.1, 19.2, 19.3, 19.4, 19.5_
  
  - [x] 6.3 实现 Response 结构体
    - 实现 new 方法
    - 实现 status、is_success、headers 方法
    - 实现 text、bytes、json 方法（响应体解析）
    - _需求: 20.1, 20.2, 20.3, 20.4, 20.5_
  
  - [x] 6.4 编写 HTTP 客户端单元测试
    - 测试请求构建器的链式调用
    - 测试请求头设置
    - 测试查询参数设置
    - 测试请求体序列化
    - 使用 mockito 模拟 HTTP 服务
    - _需求: 17.1, 17.2, 18.1, 18.2, 19.1, 19.2_

- [x] 7. 实现 Token 管理模块（token）
  - [x] 7.1 定义 TokenClaims 结构体
    - 定义标准声明字段（iss、sub、aud、exp、nbf、iat、jti）
    - 定义 token_type 字段
    - 定义 custom 字段（自定义声明）
    - 派生 Serialize 和 Deserialize trait
    - _需求: 21.3, 21.4, 23.2, 23.3_
  
  - [x] 7.2 实现 TokenManager 结构体
    - 实现 new_symmetric 方法（对称加密）
    - 实现 new_asymmetric 方法（非对称加密）
    - 实现 generate_access_token 方法
    - 实现 generate_refresh_token 方法
    - 实现 generate_token_pair 方法
    - 实现 verify_token 方法（验证签名、过期时间、签发者、受众）
    - 实现 parse_token_unsafe 方法（不验证签名）
    - 实现 is_token_expiring_soon 方法
    - 实现 refresh_access_token 方法
    - 使用 jsonwebtoken crate
    - _需求: 21.1, 21.2, 21.3, 21.4, 21.5, 22.1, 22.2, 22.3, 22.4, 22.5, 23.1, 23.2, 23.3, 23.4, 23.5, 24.1, 24.2, 24.3, 24.4, 24.5_
  
  - [x] 7.3 编写 Token 管理单元测试
    - 测试对称加密 Token 生成和验证
    - 测试非对称加密 Token 生成和验证
    - 测试 Token 过期验证
    - 测试 Token 刷新机制
    - 测试自定义声明的序列化和反序列化
    - _需求: 21.1, 21.2, 21.5, 22.1, 22.2, 22.3, 22.5, 24.1, 24.2, 24.5_
  
  - [x] 7.4 编写 Token round-trip 属性测试
    - **属性 1: Token 生成-验证-解析一致性**
    - **验证需求: 21.5, 22.1, 23.1**
    - 使用 proptest 生成随机用户 ID 和自定义声明
    - 验证生成的 Token 能够正确验证
    - 验证解析的声明与原始声明一致
    - 验证 Token 类型正确
    - _需求: 21.5, 22.1, 23.1, 23.2, 23.3_

- [x] 8. 实现数据模型和配置结构体
  - [x] 8.1 定义 ModuleMetadata 结构体
    - 定义模块元数据字段
    - 派生 Serialize 和 Deserialize trait
    - _需求: 1.3, 2.1, 2.2, 2.3_
  
  - [x] 8.2 定义 MigrationRecord 结构体
    - 定义迁移记录字段
    - 派生 sqlx::FromRow trait
    - _需求: 9.2, 9.5_
  
  - [x] 8.3 定义 HttpConfig 结构体
    - 定义 HTTP 客户端配置字段
    - 实现 Default trait
    - _需求: 17.3, 17.4_
  
  - [x] 8.4 定义 TokenConfig 结构体
    - 定义 Token 配置字段
    - 实现 Default trait
    - _需求: 21.1, 21.2, 21.3_

- [x] 9. HTTP 客户端与 Token 集成
  - [x] 9.1 实现 HTTP 拦截器机制
    - 在 RequestBuilder 中添加拦截器支持
    - 实现 Token 自动添加拦截器
    - 实现 Token 自动刷新拦截器
    - _需求: 25.1, 25.2, 25.3, 25.4, 25.5_
  
  - [x] 9.2 编写 HTTP 与 Token 集成测试
    - 测试 Bearer Token 自动添加
    - 测试全局默认 Token 配置
    - 测试单个请求 Token 覆盖
    - _需求: 25.1, 25.2, 25.3_

- [x] 10. Checkpoint - 完整功能验证
  - 确保所有测试通过，询问用户是否有问题

- [x] 11. 完善文档和示例
  - [x] 11.1 编写模块级文档注释
    - 为每个模块添加概述和使用示例
    - 为公开 API 添加详细的文档注释
    - _需求: 所有需求_
  
  - [x] 11.2 创建使用示例
    - 创建 examples/module_example.rs（模块定义示例）
    - 创建 examples/database_init.rs（数据库初始化示例）
    - 创建 examples/http_client.rs（HTTP 客户端示例）
    - 创建 examples/token_manager.rs（Token 管理示例）
    - _需求: 所有需求_
  
  - [x] 11.3 更新 README.md
    - 添加功能概述
    - 添加快速开始指南
    - 添加 API 文档链接
    - _需求: 所有需求_

- [x] 12. 代码质量检查和优化
  - [x] 12.1 运行 cargo clippy 并修复警告
    - 修复所有 clippy 警告
    - 确保代码符合 Rust 最佳实践
  
  - [x] 12.2 运行 cargo fmt 格式化代码
    - 确保代码格式统一
  
  - [x] 12.3 运行 cargo test 确保所有测试通过
    - 运行单元测试
    - 运行集成测试
    - 运行属性测试
  
  - [x] 12.4 生成文档并检查
    - 运行 cargo doc
    - 检查文档完整性和准确性

- [x] 13. 最终验证
  - 确保所有测试通过，询问用户是否准备好发布

## 注意事项

### 依赖项

在 `Cargo.toml` 中需要添加以下依赖：

```toml
[dependencies]
yang-db = { path = "../yang-db" }
tokio = { version = "1.51.0", features = ["full"] }
async-trait = "0.1"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "2.0"
log = "0.4"
reqwest = { version = "0.12", features = ["json"] }
jsonwebtoken = "9.0"
uuid = { version = "1.0", features = ["v4"] }
sqlx = { version = "0.8.6", features = ["runtime-tokio-rustls", "mysql"] }
chrono = "0.4"

[dev-dependencies]
tokio = { version = "1.51.0", features = ["full", "test-util"] }
proptest = "1.5"
mockito = "1.0"
testcontainers = "0.15"
```

### 测试策略

- **单元测试**：测试各模块的独立功能
- **集成测试**：测试模块间的协作和端到端流程
- **属性测试**：测试 JWT Token 的通用属性
- **标记为可选的测试任务**（带 `*` 后缀）可以跳过以加快 MVP 开发

### 实现顺序

任务按照模块依赖关系排序：
1. 错误处理（基础）
2. 模块管理（核心）
3. 数据库管理（依赖模块管理，使用 yang-db 库）
4. HTTP 客户端（独立）
5. Token 管理（独立）
6. HTTP 与 Token 集成（依赖 HTTP 和 Token）

### 代码规范

- 所有代码注释使用中文
- 变量和函数使用蛇形命名法（snake_case）
- 结构体和枚举使用大驼峰命名法（PascalCase）
- 遵循 Rust 社区编码风格（rustfmt 标准）
- 避免使用 `unwrap()` 和 `expect()`，使用 `?` 操作符传播错误

### 并发安全

- 使用 `Arc<RwLock<T>>` 确保模块管理器的线程安全
- 使用 `OnceLock` 实现全局单例（GlobalDatabase、HttpClient）
- 数据库连接池本身是线程安全的（由 yang-db 库提供）

### 数据库操作

- 所有数据库相关操作都基于 yang-db 库实现
- DatabaseInitializer 使用 yang-db::Database 的方法执行 SQL
- GlobalDatabase 封装 yang-db::Database，提供全局访问接口
- 使用 yang-db 的查询构建器、事务管理等功能

### 错误处理

- 所有公开 API 返回 `Result<T, BaseError>`
- 在错误发生时记录详细的上下文信息
- 使用不同的日志级别（error、warn、info、debug）
