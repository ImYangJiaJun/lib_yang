# yang-base

YANG 基础库，提供插件管理、数据库访问（MySQL + Redis）、HTTP 客户端和 JWT Token 管理等核心功能。

## ✨ 功能特性

### 🗄️ 数据库管理（database）
- **MySQL 支持**
  - 全局数据库访问（GlobalDatabase）
  - 类型安全的查询构建器
  - 事务支持
  - 连接池管理
  - 数据库迁移
  
- **Redis 支持** ⭐ 新增
  - 全局 Redis 访问（GlobalRedis）
  - String、Hash、List、Set、Sorted Set 操作
  - 过期时间管理
  - 连接池管理
  - 29+ Redis 操作方法

### 🔌 插件管理（plugin）
- 插件注册和管理
- 插件依赖关系解析
- 插件生命周期管理
- 插件配置管理

### 🌐 HTTP 客户端（http）
- 灵活的请求构建器
- 支持常用 HTTP 方法（GET、POST、PUT、DELETE、PATCH）
- 请求头和查询参数管理
- JSON/表单数据序列化
- 响应处理和解析

### 🔐 Token 管理（token）
- JWT Token 生成
- Token 验证和解析
- 对称/非对称加密支持
- Token 刷新机制
- 自定义声明支持

### ⚠️ 错误处理（error）
- 统一错误类型（BaseError）
- 详细的错误上下文
- 中文错误消息
- 错误码支持

### 📋 表配置系统（table）
- 表结构定义
- 字段类型验证
- 字段权限控制

### 🎯 Action 系统（action）
- Action 注册和执行
- 统一的响应格式
- 参数验证

## 🚀 快速开始

### 安装

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
yang-base = { path = "../yang-base" }
yang-db = { path = "../yang-db" }
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

默认 feature 提供完整的 MySQL、Redis、Token、HTTP、validator 与 plugin-schema 能力。最小部署可精确选择：

```toml
# 仅核心插件/路由/表元数据，不引入数据库与网络驱动
yang-base = { version = "0.1.2", default-features = false }

# 仅 MySQL
yang-base = { version = "0.1.2", default-features = false, features = ["mysql"] }

# 仅 Redis
yang-base = { version = "0.1.2", default-features = false, features = ["redis"] }

# Token 撤销依赖 Redis，因此 token 会自动启用 redis
yang-base = { version = "0.1.2", default-features = false, features = ["token"] }
```

`yang-base` 通过 `default-features = false` 依赖 `yang-db`，只转发实际选中的后端 feature；docs.rs 使用 all-features 构建完整 API。

### 基本使用

```rust
use yang_base::database::{GlobalDatabase, GlobalRedis};
use yang_base::error::BaseError;
use yang_db::{DatabaseConfig, redis::RedisConfig};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
struct User {
    id: i32,
    name: String,
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), BaseError> {
    // 初始化 MySQL
    GlobalDatabase::init(
        "mysql://root:password@localhost/mydb",
        DatabaseConfig::default()
    ).await?;
    
    // 初始化 Redis
    GlobalRedis::init(
        "redis://127.0.0.1:6379",
        RedisConfig::default()
    ).await?;
    
    // 查询数据库
    let users: Vec<User> = GlobalDatabase::table("users")?
        .where_and("age", ">=", 18)
        .select()
        .await?;
    
    // 使用 Redis 缓存
    GlobalRedis::set("user_count", &users.len().to_string(), Some(300)).await?;
    
    // 获取缓存
    if let Some(count) = GlobalRedis::get("user_count").await? {
        println!("用户数量: {}", count);
    }
    
    Ok(())
}
```

## 📚 文档

- **[完整使用指南](./USAGE_GUIDE.md)** - 详细的功能说明和示例
- **[快速参考](./QUICK_REFERENCE.md)** - 常用操作速查表
- **[Redis 功能指南](./REDIS_GUIDE.md)** - Redis 操作详细说明
- **[Action 系统示例](./src/action/ACTION_EXAMPLES.md)** - Action 系统使用示例

## 🏗️ 项目结构

```
yang-base/
├── src/
│   ├── lib.rs              # 库入口
│   ├── error/              # 错误处理模块
│   │   └── mod.rs
│   ├── plugin/             # 插件管理模块
│   │   └── mod.rs
│   ├── database/           # 数据库管理模块
│   │   ├── mod.rs
│   │   ├── global.rs       # MySQL 全局访问
│   │   ├── global_redis.rs # Redis 全局访问 ⭐
│   │   └── initializer.rs
│   ├── http/               # HTTP 客户端模块
│   │   └── mod.rs
│   ├── token/              # Token 管理模块
│   │   └── mod.rs
│   ├── table/              # 表配置系统
│   │   └── mod.rs
│   ├── action/             # Action 系统
│   │   └── mod.rs
│   └── router/             # 路由系统
│       └── mod.rs
├── Cargo.toml              # 项目配置
├── README.md               # 项目说明
├── USAGE_GUIDE.md          # 使用指南
├── QUICK_REFERENCE.md      # 快速参考
└── REDIS_GUIDE.md          # Redis 指南
```

## 📦 依赖项

### 核心依赖
- `yang-db` - YANG 数据库库（支持 MySQL 和 Redis）
- `tokio` - 异步运行时
- `async-trait` - 异步 trait 支持
- `serde`/`serde_json` - 序列化支持
- `thiserror` - 错误处理
- `log` - 日志记录

### HTTP 相关
- `reqwest` - HTTP 客户端

### Token 相关
- `jsonwebtoken` - JWT Token 处理

### 数据库相关
- `sqlx` - MySQL 数据库驱动

### 工具库
- `uuid` - UUID 生成
- `chrono` - 时间处理
- `regex` - 正则表达式

### 开发依赖
- `proptest` - 属性测试
- `mockito` - HTTP Mock
- `testcontainers` - 容器化测试
- `env_logger` - 日志输出

## 🎯 使用场景

### Web 应用后端
```rust
// 用户认证 + 缓存
let user = get_user_from_db(user_id).await?;
let token = TokenManager::generate(&claims, 3600)?;
GlobalRedis::set(&format!("session:{}", token), &user_id.to_string(), Some(3600)).await?;
```

### API 服务
```rust
// 数据查询 + 缓存
let cache_key = format!("api:users:{}", user_id);
if let Some(cached) = GlobalRedis::get(&cache_key).await? {
    return Ok(serde_json::from_str(&cached)?);
}
let user = GlobalDatabase::table("users")?.where_and("id", "=", user_id).select().await?;
GlobalRedis::set(&cache_key, &serde_json::to_string(&user)?, Some(300)).await?;
```

### 任务队列
```rust
// 生产者
GlobalRedis::lpush("tasks", &[task_json]).await?;

// 消费者
while let Some(task) = GlobalRedis::rpop("tasks").await? {
    process_task(&task).await?;
}
```

### 排行榜系统
```rust
// 更新分数
GlobalRedis::zadd("leaderboard", &[(score, player_id)]).await?;

// 获取排名
let top10 = GlobalRedis::zrange("leaderboard", 0, 9).await?;
```

## 🔧 配置示例

### 生产环境配置

```rust
// MySQL 配置
let db_config = DatabaseConfig {
    max_connections: 20,
    connect_timeout: 10,
    idle_timeout: 300,
    enable_logging: false,
};

// Redis 配置
let redis_config = RedisConfig::new(
    10,     // max_connections
    5,      // connect_timeout
    300,    // idle_timeout
    false   // enable_logging
);
```

## ✅ 测试

```bash
# 运行所有测试
cargo test --lib -p yang-base

# 运行特定模块测试
cargo test --lib -p yang-base error::tests

# 查看测试覆盖率
cargo test --lib -p yang-base -- --nocapture
```

当前测试状态：✅ 286 个测试全部通过

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

## 📄 许可证

MIT OR Apache-2.0

## 🔗 相关项目

- [yang-db](../yang-db) - YANG 数据库库
- [yang-pcg](../yang-pcg) - YANG 配置生成器

## 📝 更新日志

### v0.0.1 (当前版本)
- ✅ 插件管理系统
- ✅ MySQL 数据库支持
- ✅ Redis 缓存支持 ⭐ 新增
- ✅ HTTP 客户端
- ✅ JWT Token 管理
- ✅ 错误处理系统
- ✅ 表配置系统
- ✅ Action 系统
- ✅ 完整文档和示例

## 💡 最佳实践

1. **初始化顺序**：先初始化数据库和 Redis，再初始化其他组件
2. **错误处理**：使用 `?` 操作符传播错误，在顶层统一处理
3. **缓存策略**：合理设置 TTL，避免缓存雪崩
4. **连接池配置**：根据实际负载调整 `max_connections`
5. **日志记录**：开发环境开启 `enable_logging`，生产环境关闭
6. **安全性**：Token 密钥使用环境变量，不要硬编码
7. **性能优化**：使用批量操作减少网络往返

## 📞 联系方式

如有问题或建议，请提交 Issue。
