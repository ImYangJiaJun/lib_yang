# yang-base

YANG 基础库，提供插件管理、数据库访问、HTTP 客户端和 JWT Token 管理等核心功能。

## 功能模块

### 1. 插件管理（plugin）
- 插件注册和管理
- 插件依赖关系解析
- 插件生命周期管理
- 插件配置管理

### 2. 数据库管理（database）
- 全局数据库访问
- 数据库初始化
- 数据库迁移管理
- 事务支持

### 3. HTTP 客户端（http）
- 灵活的请求构建器
- 支持常用 HTTP 方法（GET、POST、PUT、DELETE、PATCH）
- 请求头和查询参数管理
- JSON/表单数据序列化
- 响应处理和解析

### 4. Token 管理（token）
- JWT Token 生成
- Token 验证和解析
- 对称/非对称加密支持
- Token 刷新机制
- 自定义声明支持

### 5. 错误处理（error）
- 统一错误类型
- 详细的错误上下文
- 中文错误消息

## 项目结构

```
yang-base/
├── src/
│   ├── lib.rs           # 库入口
│   ├── error/           # 错误处理模块
│   │   └── mod.rs
│   ├── plugin/          # 插件管理模块
│   │   └── mod.rs
│   ├── database/        # 数据库管理模块
│   │   └── mod.rs
│   ├── http/            # HTTP 客户端模块
│   │   └── mod.rs
│   └── token/           # Token 管理模块
│       └── mod.rs
├── Cargo.toml           # 项目配置
└── README.md            # 项目说明
```

## 依赖项

### 核心依赖
- `yang-db`：YANG 数据库库
- `tokio`：异步运行时
- `async-trait`：异步 trait 支持
- `serde`/`serde_json`：序列化支持
- `thiserror`：错误处理
- `log`：日志记录

### HTTP 相关
- `reqwest`：HTTP 客户端

### Token 相关
- `jsonwebtoken`：JWT Token 处理

### 数据库相关
- `sqlx`：数据库驱动

### 工具库
- `uuid`：UUID 生成
- `chrono`：时间处理

### 开发依赖
- `proptest`：属性测试
- `mockito`：HTTP Mock
- `testcontainers`：容器化测试

## 开发状态

当前版本：0.0.1

项目处于开发阶段，各模块功能正在逐步实现中。

## 许可证

MIT OR Apache-2.0
