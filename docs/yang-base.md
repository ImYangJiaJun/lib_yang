# yang-base — 后端基础库文档

版本：0.1.1 | 许可：MIT OR Apache-2.0

## 概述

`yang-base` 是构建后端服务的基础库，集成插件系统、数据库全局访问、HTTP 客户端、JWT Token 管理、Action 系统和路由等核心能力。

```toml
[dependencies]
yang-base = { path = "../yang-base" }

# 或按需启用 feature
yang-base = { path = "../yang-base", features = ["token", "http", "mysql", "validator"] }
```

---

## Feature Gates

| Feature | 默认 | 依赖 | 说明 |
|---------|------|------|------|
| `token` | ✓ | jsonwebtoken | JWT Token 管理 |
| `http` | ✓ | reqwest, serde_urlencoded | HTTP 客户端 |
| `mysql` | ✓ | sqlx | MySQL 查询执行（TableQuery 执行层） |
| `validator` | ✓ | regex | 字段正则校验器 |
| `plugin-schema` | ✓ | jsonschema | 插件配置 JSON Schema 验证 |
| `metrics` | — | metrics | 运行期指标埋点门面（默认关闭，运行期由调用方挂 exporter） |

---

## 模块依赖图

```
yang-base
├── error          ← 被所有模块依赖
├── plugin         → error
├── database       → error, yang-db
├── action
│   ├── context    → table, token(opt)
│   ├── response   → error
│   ├── request
│   └── builtin    → database(mysql feature)
├── table          → error, yang-db(mysql feature)
├── router         → action, error
├── http           → error               (http feature)
└── token          → error               (token feature)
```

**依赖方向**：`yang-base → yang-db`（单向，yang-db 不依赖 yang-base）。

---

## 错误类型 (`BaseError`)

统一错误枚举，覆盖所有模块，附带数字错误码：

| 错误码段 | 模块 | 示例 |
|---------|------|------|
| `1xxxxx` | 插件管理 | `PluginNotFound`(100002), `PluginCircularDependency`(100006), `PluginShutdownFailed`(100008) |
| `2xxxxx` | MySQL 数据库 | `DatabaseNotInitialized`(200008), `DatabaseQueryFailed`(200003), `DatabaseTransactionFailed`(200009), `MissingWhereClause`(200010) |
| `21xxxx` | Redis | `RedisNotInitialized`(210003), `RedisOperationFailed`(210004) |
| `3xxxxx` | HTTP 客户端 | `HttpTimeout`(300004), `HttpClientNotInitialized`(300006), `HttpCircuitBreakerOpen`(300007) |
| `4xxxxx` | Token | `TokenExpired`(400005), `TokenVerifyFailed`(400003), `TokenRevoked`(400007) |
| `5xxxxx` | 序列化 | `JsonDeserializeFailed`(500002), `JsonSerializeFailed`(500001) |
| `6xxxxx` | 字段验证 | `FieldRequired`(600006), `ValidationFailed`(600005), `FieldNotFound`(600007), `FieldPermissionDenied`(600008) |
| `7xxxxx` | Action 系统 | `ActionNotFound`(700001), `PermissionDenied`(700002), `RecordNotFound`(700006), `Unauthorized`(700003), `UserNotFound`(700007), `InvalidPassword`(700008), `TableConfigNotSet`(700009) |
| `9xxxxx` | 通用 | `ConfigError`(900001), `IoError`(900002), `Unknown`(999999) |

> **注意**：上表为代表性错误码摘录，非穷举列表。完整错误码列表见 `crates/yang-base/src/error/mod.rs` 中 `BaseError::code()` 方法的 match 臂。`BaseError` 标注 `#[non_exhaustive]`，未来可能新增变体。

```rust
use yang_base::error::BaseError;

// 获取错误码
let error = BaseError::FieldRequired("username".to_string());
assert_eq!(error.code(), 600006);

// 错误链（source() 可遍历）
let db_err: BaseError = yang_db_error.into();
assert!(db_err.source().is_some());
```

**自动 From 转换**：`yang_db::DbError`, `serde_json::Error`, `reqwest::Error`(http feature), `jsonwebtoken::errors::Error`(token feature), `std::io::Error`

---

## Plugin 模块

插件系统支持注册、依赖管理、生命周期回调和配置 Schema 验证。

### Plugin Trait

```rust
#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;                              // 必须实现，唯一标识符
    fn version(&self) -> &str { "0.1.0" }               // 语义化版本
    fn dependencies(&self) -> Vec<&str> { vec![] }      // 依赖的其他插件名称
    fn init_sql(&self) -> Vec<String> { vec![] }         // 建表 SQL（建议含 IF NOT EXISTS）
    fn migration_sql(&self) -> Vec<(String, String)> { vec![] } // (版本号, SQL) 迁移脚本
    fn config_schema(&self) -> Option<JsonValue> { None } // JSON Schema 配置定义
    async fn on_register(&self) -> Result<(), Box<dyn Error>> { Ok(()) } // 注册回调
    async fn on_init(&self) -> Result<(), Box<dyn Error>> { Ok(()) }     // 初始化回调
    async fn on_shutdown(&self) -> Result<(), Box<dyn Error>> { Ok(()) } // 关闭回调
}
```

**版本号格式**：`YYYYMMDDHHMMSS`（迁移脚本）

### PluginManagerBuilder（构建阶段，可变）

```rust
let mut builder = PluginManagerBuilder::new();
builder.register(MyPlugin).await?;   // 注册（检查重名 + 调用 on_register）
let registry = builder.build()?;     // 消费构建器，生成 PluginRegistry
                                     // build() 检查：依赖完整性 + 循环依赖
```

**build() 错误**：
- `PluginDependencyMissing(plugin, dep)` — 依赖未注册
- `PluginCircularDependency(msg)` — 循环依赖（含涉及插件名称）

### PluginRegistry（运行阶段，无锁只读）

```rust
let plugin = registry.get("my_plugin");        // O(1) HashMap 查找，返回 Option<&Arc<dyn Plugin>>
let all = registry.get_all();                  // &[Arc<dyn Plugin>]，按依赖拓扑顺序，已缓存
let config = registry.get_config("my_plugin"); // Option<&JsonValue>
registry.shutdown().await?;                    // 逆序调用所有插件的 on_shutdown
```

### PluginManager（运行时动态注册，带 RwLock）

适用于需要在运行时动态注册插件的场景（较 Builder 有锁开销）：

```rust
let manager = PluginManager::new();
manager.register(MyPlugin).await?;
manager.load_config("my_plugin", json!({"key": "val"})).await?;
let plugin = manager.get("my_plugin").await;       // Option<Arc<dyn Plugin>>
let all = manager.get_all().await;                  // Vec<Arc<dyn Plugin>>（按拓扑排序）
let config = manager.get_config("my_plugin").await; // Option<JsonValue>
manager.shutdown().await?;
```

### 完整示例

```rust
use yang_base::plugin::{Plugin, PluginManagerBuilder};
use async_trait::async_trait;

struct UsersPlugin;

#[async_trait]
impl Plugin for UsersPlugin {
    fn name(&self) -> &str { "users" }

    fn init_sql(&self) -> Vec<String> {
        vec![
            "CREATE TABLE IF NOT EXISTS users (
                id BIGINT AUTO_INCREMENT PRIMARY KEY,
                username VARCHAR(50) NOT NULL UNIQUE,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4".to_string()
        ]
    }

    fn migration_sql(&self) -> Vec<(String, String)> {
        vec![
            ("20240101120000".to_string(),
             "ALTER TABLE users ADD COLUMN email VARCHAR(100)".to_string()),
        ]
    }
}

// 构建阶段
let mut builder = PluginManagerBuilder::new();
builder.register(UsersPlugin).await?;
let registry = builder.build()?;

// 运行阶段（无锁）
if let Some(plugin) = registry.get("users") {
    println!("版本: {}", plugin.version());
}
```

---

## Database 模块

对 `yang-db` 的全局单例封装，提供静态访问接口。

### GlobalDatabase（MySQL 全局单例）

```rust
use yang_base::database::GlobalDatabase;
use yang_db::DatabaseConfig;

// 初始化（只调用一次）
GlobalDatabase::init("mysql://user:pass@localhost/db", DatabaseConfig::default()).await?;

// 获取底层实例
let db: &'static Database = GlobalDatabase::get()?;

// 直接使用便捷方法
let builder: QueryBuilder = GlobalDatabase::table("users")?;

// 原生查询（T: sqlx::FromRow）
let users: Vec<User> = GlobalDatabase::query("SELECT * FROM users WHERE status=1").await?;

// 原生执行
let affected: u64 = GlobalDatabase::execute("DELETE FROM sessions WHERE expired=1").await?;

// 事务
let mut tx: Transaction = GlobalDatabase::transaction().await?;
tx.execute("INSERT INTO logs (msg) VALUES ('hello')").await?;
tx.commit().await?;
```

`GlobalDatabase::table()` 返回的 `QueryBuilder` 支持 yang-db 的全部链式 API（参见 yang-db.md）。

### GlobalRedis（Redis 全局单例）

```rust
use yang_base::database::GlobalRedis;
use yang_db::redis::RedisConfig;

// 初始化
GlobalRedis::init("redis://127.0.0.1:6379", RedisConfig::default()).await?;

// 获取底层客户端（访问完整 RedisClient API）
let client: &'static RedisClient = GlobalRedis::client()?;

// String 操作
GlobalRedis::set("key", "value", None).await?;              // 不过期
GlobalRedis::set("session", "data", Some(3600)).await?;     // 1小时过期
let val: Option<String> = GlobalRedis::get("key").await?;
GlobalRedis::del(&["k1", "k2"]).await?;
GlobalRedis::exists(&["k1"]).await?;
GlobalRedis::expire("key", 300).await?;
GlobalRedis::ttl("key").await?;
GlobalRedis::persist("key").await?;
GlobalRedis::keys("user:*").await?;
GlobalRedis::incr("counter").await?;
GlobalRedis::decr("counter").await?;
GlobalRedis::incrby("counter", 5).await?;
GlobalRedis::mget(&["k1", "k2"]).await?;
GlobalRedis::mset(&[("k1".into(), "v1".into())]).await?;

// Hash 操作
GlobalRedis::hset("user:1", "name", "Alice").await?;
GlobalRedis::hget("user:1", "name").await?;
GlobalRedis::hdel("user:1", &["name"]).await?;
GlobalRedis::hgetall("user:1").await?;
GlobalRedis::hexists("user:1", "name").await?;
GlobalRedis::hlen("user:1").await?;
GlobalRedis::hincrby("user:1", "score", 10).await?;

// List 操作
GlobalRedis::lpush("queue", &["task1", "task2"]).await?;
GlobalRedis::rpush("queue", &["task3"]).await?;
GlobalRedis::lpop("queue").await?;
GlobalRedis::rpop("queue").await?;
GlobalRedis::llen("queue").await?;
GlobalRedis::lrange("queue", 0, -1).await?;

// Set 操作
GlobalRedis::sadd("tags", &["rust", "backend"]).await?;
GlobalRedis::srem("tags", &["backend"]).await?;
GlobalRedis::sismember("tags", "rust").await?;
GlobalRedis::smembers("tags").await?;
GlobalRedis::scard("tags").await?;

// Sorted Set 操作
GlobalRedis::zadd("leaderboard", &[(100.0, "alice".into()), (200.0, "bob".into())]).await?;
GlobalRedis::zrem("leaderboard", &["alice"]).await?;
GlobalRedis::zcard("leaderboard").await?;
GlobalRedis::zrange("leaderboard", 0, -1).await?;
GlobalRedis::zrange_with_scores("leaderboard", 0, -1).await?;
GlobalRedis::zrevrange("leaderboard", 0, -1).await?;
GlobalRedis::zincrby("leaderboard", 50.0, "alice").await?;

// Pipeline / Transaction
let mut tx = GlobalRedis::transaction()?;
tx.set("k1", "v1").incr("counter");
let (_, count): ((), i64) = tx.exec().await?;

// Pipeline 通过底层 client 访问
let mut pipeline = GlobalRedis::client()?.pipeline();
pipeline.set("k1", "v1").set("k2", "v2");
let results = pipeline.execute().await?;
```

### DatabaseInitializer（插件数据库初始化）

```rust
use yang_base::database::DatabaseInitializer;
use yang_db::Database;

let db = Database::connect("mysql://...").await?;
let manager = PluginManager::new();
// ... 注册插件 ...

let initializer = DatabaseInitializer::new(db, /* use_transaction= */ true);
initializer.initialize_all(&manager).await?;
```

**初始化流程**：
1. 创建 `_migrations` 迁移记录表（幂等）
2. 按依赖拓扑顺序遍历所有插件
3. 执行每个插件的 `init_sql()`
4. 执行每个插件新的 `migration_sql()`（已执行的版本自动跳过）
5. 调用每个插件的 `on_init()`

**迁移记录表结构**：
```sql
CREATE TABLE IF NOT EXISTS _migrations (
    id INT AUTO_INCREMENT PRIMARY KEY,
    module_name VARCHAR(255) NOT NULL,
    version VARCHAR(255) NOT NULL,
    executed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY unique_migration (module_name, version),
    INDEX idx_module_name (module_name)
)
```

### DatabaseBundle（统一初始化入口）

```rust
use yang_base::database::DatabaseBundle;
use yang_db::{DatabaseConfig, redis::RedisConfig};

// 统一初始化 MySQL + Redis（按固定顺序：先 MySQL 再 Redis，任一失败即返回）
DatabaseBundle::init(
    "mysql://user:pass@localhost/db",
    DatabaseConfig { max_connections: 20, ..Default::default() },
    "redis://127.0.0.1:6379",
    RedisConfig::default(),
).await?;

// 之后即可直接使用全局单例
let db = GlobalDatabase::get()?;
let redis = GlobalRedis::client()?;
```

相比分别调用 `GlobalDatabase::init()` + `GlobalRedis::init()`，`DatabaseBundle::init()` 的优势：
- 单一入口，不会遗漏任一初始化
- 固定初始化顺序，避免"半初始化"状态（MySQL 成功但 Redis 失败）
- 任一失败立即返回错误，不会产生部分初始化

---

## Token 模块（`feature = "token"`）

### TokenClaims（JWT 声明结构）

```rust
pub struct TokenClaims {
    pub iss: String,           // 签发者
    pub sub: String,           // 主题（用户 ID）
    pub aud: String,           // 受众
    pub exp: u64,              // 过期时间（Unix 时间戳）
    pub nbf: u64,              // 生效时间
    pub iat: u64,              // 签发时间
    pub jti: String,           // JWT 唯一 ID（uuid v4）
    pub token_type: String,    // "access" 或 "refresh"
    pub custom: serde_json::Value, // 自定义声明（#[serde(flatten)] 展平）
}
```

### TokenManager

```rust
use yang_base::token::TokenManager;
use jsonwebtoken::Algorithm;
```

**构造方法**：

```rust
// 对称加密（HS256/HS384/HS512）
let manager = TokenManager::new_symmetric(
    "secret_key",          // 密钥
    Algorithm::HS256,
    "my_app".to_string(),  // issuer
    "users".to_string(),   // audience
    3600,                  // access token 有效期（秒）
    86400,                 // refresh token 有效期（秒）
);

// 非对称加密（RS256/RS384/RS512）
let manager = TokenManager::new_asymmetric(
    "-----BEGIN RSA PRIVATE KEY-----\n...",
    "-----BEGIN PUBLIC KEY-----\n...",
    Algorithm::RS256,
    "my_app".to_string(),
    "users".to_string(),
    3600,
    86400,
)?;
```

**核心方法**：

```rust
// 生成 Access Token（含自定义声明）
let access = manager.generate_access_token(
    "user_123",
    json!({ "role": "admin", "permissions": ["read", "write"] }),
)?;

// 生成 Refresh Token
let refresh = manager.generate_refresh_token("user_123")?;

// 同时生成 Token 对
let (access, refresh) = manager.generate_token_pair(
    "user_123",
    json!({ "role": "admin" }),
)?;

// 验证 Token（检查签名 + 过期 + iss + aud，白名单防算法混淆攻击）
let claims: TokenClaims = manager.verify_token(&access)?;
println!("用户 ID: {}", claims.sub);
println!("自定义数据: {}", claims.custom["role"]);

// 检查 Token 是否即将过期（5 分钟内）
if manager.is_token_expiring_soon(&access, 300)? {
    // 执行刷新逻辑
}

// 用 Refresh Token 刷新 Access Token
let new_access = manager.refresh_access_token(&refresh, json!({ "role": "admin" }))?;

// ⚠️ 不安全解析（跳过签名验证，仅用于调试/日志）
// 严禁用于鉴权决策
let claims = manager.parse_token_unsafe(&token)?;
```

**错误**：`TokenExpired`(400005), `TokenVerifyFailed`(400003), `TokenTypeInvalid`(400006), `TokenKeyInvalid`(400001), `TokenRevoked`(400007)

### Token 撤销与黑名单机制（`feature = "token"`）

`TokenManager` 提供基于 Redis 的 Token 撤销与黑名单机制：

```rust
// 撤销单个 Token（将 jti 写入 Redis 黑名单，TTL = token 剩余有效期）
manager.revoke_token(&access_token).await?;

// 按已验证 claims 撤销该 Token（将 claims.jti 写入黑名单，与 revoke_token 等价但省略重复验证）
manager.revoke_claims(&claims).await?;

// 按用户（sub）批量撤销其全部旧 Token（写入 min_iat 水位线，用于改密/强制下线）
manager.revoke_by_subject(&claims.sub).await?;

// 查询某用户的最小有效签发时间水位线
let min_iat: Option<u64> = manager.subject_min_iat(&claims.sub).await?;

// 检查 Token 是否已被撤销
let revoked: bool = manager.is_revoked(&jti).await?;

// 带黑名单检查的验证（推荐用于需要登出/撤销能力的鉴权路径）
let claims = manager.verify_token_checked(&access_token).await?;
```

**黑名单存储**：Redis key 格式 `token:blacklist:{jti}`，TTL = `exp - now`（Token 过期后自动清理，无需手动维护）。

**重要区分**：
- `verify_token()` — 标准签名+过期校验，**不查**黑名单（向后兼容）
- `verify_token_checked()` — 签名+过期校验 **+ 黑名单检查**（需要撤销能力的鉴权路径必须用此方法）

**错误**：`TokenRevoked`(400007)

---

## HTTP 模块（`feature = "http"`）

### HttpClientConfig

```rust
pub struct HttpClientConfig {
    pub timeout_secs: u64,              // 默认 30
    pub pool_max_idle_per_host: usize,  // 默认 10
    pub pool_idle_timeout_secs: u64,    // 默认 90
    pub user_agent: Option<String>,     // 自定义 UA，默认 None
    pub accept_invalid_certs: bool,     // 默认 false（生产环境不应设为 true）
    pub proxy_url: Option<String>,      // 代理 URL，默认 None
    pub circuit_breaker: Option<CircuitBreakerConfig>, // 熔断器策略，默认 None（不启用）
}
```

### HttpClient

`Clone` 时复用同一连接池（内部是 `Arc` 克隆）。

```rust
use yang_base::http::HttpClient;

// 全局单例（推荐）
HttpClient::init_global(30)?;
let client = HttpClient::global()?;

// 自定义配置
let client = HttpClient::with_config(HttpClientConfig {
    timeout_secs: 60,
    pool_max_idle_per_host: 20,
    user_agent: Some("MyApp/1.0".into()),
    ..Default::default()
})?;

// 设置默认 Bearer Token
client.set_default_token("your_jwt_token".into());

// GET 请求
let resp = client
    .get("https://api.example.com/users")
    .query("page", "1")
    .query("size", "10")
    .send()
    .await?;

// POST JSON
let resp = client
    .post("https://api.example.com/users")
    .bearer_token("jwt_token")
    .json(&serde_json::json!({ "name": "Alice" }))?
    .send()
    .await?;

// POST 表单
let resp = client
    .post("https://api.example.com/form")
    .form(vec![("username", "alice"), ("password", "secret")])
    .send()
    .await?;

// 响应处理
println!("状态码: {}", resp.status());
println!("成功: {}", resp.is_success());

let text = resp.text().await?;
// 或
let bytes = resp.bytes().await?;
// 或
let user: User = resp.json::<User>().await?;
```

**RequestBuilder 链式方法**：

| 方法 | 说明 |
|------|------|
| `.header(name, value)` / `.headers(vec)` | 设置请求头（header 解析错误累积，send 时返回） |
| `.content_type(v)` | 设置 Content-Type |
| `.bearer_token(t)` | 设置 Authorization: Bearer |
| `.user_agent(ua)` | 设置 User-Agent |
| `.query(k, v)` / `.queries(vec)` | 查询参数 |
| `.json(&T) -> Result<Self>` | JSON 请求体，自动设置 Content-Type |
| `.form(vec)` | 表单请求体（application/x-www-form-urlencoded） |
| `.body(Vec<u8>)` | 原始字节请求体 |
| `.text(str)` | 文本请求体（text/plain） |
| `.timeout(secs)` | 覆盖超时时间 |
| `.send() -> Result<Response>` | **发送请求** |

### 重试与熔断配置

**重试 + 指数退避**（`RetryConfig`）：

```rust
use yang_base::http::RetryConfig;

let resp = client
    .get("https://api.example.com/data")
    .retry(RetryConfig {
        max_retries: 3,                              // 最多重试 3 次
        retry_on: vec![502, 503, 504],               // 仅这些状态码触发重试（Vec<u16>，非 Option）
        backoff_ms: 100,                              // 初始退避 100ms，每次翻倍
    })
    .send()
    .await?;
// 默认值：max_retries=3, retry_on=[502,503,504], backoff_ms=100；默认不重试需显式传入 RetryConfig。
// 启用后对连接/超时错误与命中 retry_on 的状态码按指数退避重试
```

**熔断器**（`CircuitBreaker`，按目标 host 分键）：

```rust
use yang_base::http::{HttpClientConfig, CircuitBreakerConfig};

let client = HttpClient::with_config(HttpClientConfig {
    circuit_breaker: Some(CircuitBreakerConfig {
        failure_threshold: 5,   // 连续失败 5 次 → 熔断打开（默认）
        cooldown_secs: 30,      // 冷却 30 秒后放行探测（默认）
        success_threshold: 1,   // 连续成功 1 次 → 恢复（默认）
    }),
    ..Default::default()
})?;

// 当目标 host 熔断打开时，请求立即返回 HttpCircuitBreakerOpen(host)，不发网络请求
// 不同 host 独立熔断，一个故障上游不影响其他健康 host
```

**三态状态机**：Closed（累计连续失败达阈值 → Open）/ Open（快速失败，冷却后放行探测 → HalfOpen）/ HalfOpen（累计 success_threshold 次成功 → Closed，任一失败 → 重新 Open）。

**失败判定**：传输错误与 5xx 记失败；2xx/3xx/4xx 记成功（服务端正常拒绝不算上游故障）。`send()` 在每次发送前做准入检查，命中熔断打开时不发请求直接返回错误，与重试逻辑正交组合（熔断打开属于不可重试错误）。

**错误**：`HttpCircuitBreakerOpen`(300007)

---

## Table 模块

Table 系统是 yang-base 最完整的模块，提供声明式表配置、字段类型和验证、以及带权限控制的查询构建器。

### FieldType（字段类型）

```rust
pub enum FieldType {
    String { max_length: usize },   // VARCHAR
    Integer,                         // INT (i32)
    BigInt,                          // BIGINT (i64)
    Float,                           // FLOAT (f32)
    Double,                          // DOUBLE (f64)
    Boolean,                         // TINYINT(1)
    Date,                            // DATE
    DateTime,                        // DATETIME
    Timestamp,                       // TIMESTAMP (Unix 秒)
    Json,                            // JSON
    Text,                            // TEXT（无长度限制）
    Enum { values: Vec<String> },   // ENUM（预定义可选值）
    ForeignKey { table: String, field: String }, // 外键
}
```

**辅助方法**：
- `.display_name() -> &str` — 中文类型名
- `.is_numeric() -> bool` — Integer/BigInt/Float/Double
- `.is_temporal() -> bool` — Date/DateTime/Timestamp
- `.is_text() -> bool` — String/Text
- `.validate(field_name, &Value) -> Result<(), BaseError>` — 类型级校验

### Validator（字段验证器）

```rust
pub enum Validator {
    MinLength(usize),    // 最小字符数
    MaxLength(usize),    // 最大字符数
    Min(f64),            // 最小数值
    Max(f64),            // 最大数值
    Email,               // 严格邮箱格式（需 validator feature）
    EmailLoose,          // 宽松邮箱（仅检查 @）
    Phone,               // 严格 E.164 格式（需 validator feature）
    PhoneLoose,          // 宽松电话（仅数字和连字符）
    Url,                 // URL（必须以 http:// 或 https:// 开头）
    Regex(String),       // 自定义正则（需 validator feature，自动缓存）
    Custom(ValidatorFn), // 自定义函数 Arc<dyn Fn(&str, &Value) -> Result<(), BaseError>>
}
```

> **Feature 降级**：未启用 `validator` feature 时，`Email`/`Phone` 自动降级为宽松模式；`Regex` 返回错误提示需启用 feature。

### FieldConfig（字段配置）

```rust
pub struct FieldConfig {
    pub name: String,
    pub display_name: String,
    pub field_type: FieldType,
    pub required: bool,
    pub default_value: Option<serde_json::Value>,
    pub validators: Vec<Validator>,
    pub permissions: FieldPermissions,
    pub filterable: bool,   // 默认 true（是否可用于 WHERE 条件）
    pub sortable: bool,     // 默认 true（是否可用于 ORDER BY）
    pub relation: Option<RelationConfig>,
}
```

**链式构建**：

```rust
FieldConfig::new("email", FieldType::String { max_length: 100 })
    .display_name("邮箱")
    .required(true)
    .validator(Validator::Email)
    .validator(Validator::MaxLength(100))
    .permissions(FieldPermissions {
        readable_roles: vec![],               // 空 = 所有人可读
        writable_roles: vec!["admin".into()], // 仅 admin 可写
        ..Default::default()
    })
```

**FieldPermissions**：

```rust
pub struct FieldPermissions {
    pub readable_roles: Vec<String>,    // 空 = 允许所有人
    pub writable_roles: Vec<String>,
    pub filterable_roles: Vec<String>,
    pub sortable_roles: Vec<String>,
}
// 权限检查方法（空列表 = 允许所有）
.can_read(user_roles: &[String]) -> bool
.can_write(user_roles: &[String]) -> bool
.can_filter(user_roles: &[String]) -> bool
.can_sort(user_roles: &[String]) -> bool
```

**RelationConfig** / **RelationType**：

```rust
pub struct RelationConfig {
    pub table: String,
    pub field: String,
    pub display_fields: Vec<String>,
    pub relation_type: RelationType,
}

pub enum RelationType { OneToOne, OneToMany, ManyToMany }
```

### TableConfig（表配置）

```rust
pub struct TableConfig {
    pub table_name: String,
    pub display_name: String,
    pub primary_key: String,                          // 默认 "id"
    pub fields: HashMap<String, FieldConfig>,
    pub unique_indexes: Vec<IndexConfig>,
    pub indexes: Vec<IndexConfig>,
    pub default_order: Vec<(String, SortOrder)>,
    pub soft_delete_field: Option<String>,            // 软删除字段名
    pub timestamp_fields: Option<TimestampFields>,    // 自动时间戳
}

pub struct TimestampFields {
    pub created_at: Option<String>,   // 创建时间字段名
    pub updated_at: Option<String>,   // 更新时间字段名
    pub deleted_at: Option<String>,   // 删除时间字段名（软删除）
}

pub enum SortOrder { Asc, Desc }
```

**链式构建**：

```rust
let config = TableConfig::new("users")
    .display_name("用户表")
    .primary_key("id")
    .field(FieldConfig::new("id", FieldType::BigInt).display_name("ID"))
    .field(
        FieldConfig::new("username", FieldType::String { max_length: 50 })
            .display_name("用户名")
            .required(true)
            .validator(Validator::MinLength(3))
    )
    .field(
        FieldConfig::new("email", FieldType::String { max_length: 100 })
            .required(true)
            .validator(Validator::Email)
    )
    .field(FieldConfig::new("status", FieldType::Enum { values: vec!["active".into(), "banned".into()] }))
    .unique_index(vec!["username".into()])
    .unique_index(vec!["email".into()])
    .index(vec!["status".into()])
    .default_order(vec![("created_at".into(), SortOrder::Desc)])
    .soft_delete_field("deleted_at")
    .timestamps("created_at", "updated_at", "deleted_at");
```

**查询方法**：
- `.get_field(name) -> Option<&FieldConfig>`
- `.validate_field(name) -> Result<(), BaseError>` — 字段存在检查
- `.validate_query(fields: &[&str]) -> Result<(), BaseError>` — 批量字段检查

### QueryParams / WhereCondition / PaginatedResult

```rust
// WHERE 条件枚举
pub enum WhereCondition {
    Eq { field: String, value: Value },
    In { field: String, values: Vec<Value> },
    Like { field: String, pattern: String },
    Gt { field: String, value: Value },
    Gte { field: String, value: Value },
    Lt { field: String, value: Value },
    Lte { field: String, value: Value },
    IsNull { field: String },
    IsNotNull { field: String },
}

// 查询参数（TableQuery 内部使用）
pub struct QueryParams {
    pub fields: Option<Vec<String>>,
    pub where_conditions: Vec<WhereCondition>,
    pub order_by: Vec<(String, SortOrder)>,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}

// 分页结果
pub struct PaginatedResult<T: Serialize> {
    pub data: Vec<T>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
    pub total_pages: usize,   // 自动计算：ceil(total / page_size)
}
// 便捷方法
.has_next() -> bool    // page < total_pages
.has_prev() -> bool    // page > 1
```

### TableQuery（带权限的查询构建器，`feature = "mysql"`）

```rust
use yang_base::table::{TableQuery, SortOrder};
use std::sync::Arc;

let config = Arc::new(table_config);
let user_roles: Arc<[String]> = Arc::from(vec!["admin".to_string()]);

let query = TableQuery::new(config.clone(), user_roles, pool);
```

**链式查询构建**（每步检查字段权限）：

```rust
query
    .select_fields(&["id", "username", "email"])?  // 读取权限检查
    .where_eq("status", json!("active"))?           // 筛选权限检查
    .where_in("role", vec![json!("admin"), json!("user")])?
    .where_like("username", "%alice%".into())?
    .order_by("created_at", SortOrder::Desc)?       // 排序权限检查
    .page(1, 20)?;
```

**执行方法**（需 `mysql` feature）：

```rust
// 分页查询（自动执行 COUNT + SELECT）
let result: PaginatedResult<DynamicRow> = query.paginate::<DynamicRow>().await?;
// DynamicRow 是动态行类型，自动处理 BLOB base64 编码

// 列表查询
let rows: Vec<MyStruct> = query.select::<MyStruct>().await?;

// 单条查询
let row: Option<MyStruct> = query.fetch_optional::<MyStruct>().await?;

// 插入（含写入权限 + FieldType 值验证）
let affected = query.insert(data_map).await?;

// 更新（含写入权限 + 值验证）
let affected = query.update(data_map).await?;

// 删除（软删除 or 物理删除，取决于 TableConfig）
let affected = query.delete().await?;
```

**软删除行为**：若 `TableConfig::soft_delete_field` 已配置，`delete()` 执行 `UPDATE ... SET deleted_at = NOW()`；否则执行 `DELETE FROM`。

**SQL 注入防护**：字段名反引号转义 + 严格标识符校验 + 所有值参数化绑定。

### DynamicRow（动态行）

```rust
pub struct DynamicRow {
    pub fields: IndexMap<String, serde_json::Value>
}
```

实现 `sqlx::FromRow`，自动处理：
- BLOB → base64 编码字符串
- DateTime/Timestamp → ISO 8601 字符串
- JSON 字段 → 自动解析为 `serde_json::Value`

---

## Action 模块

Action 系统是请求处理的核心抽象，类似于 MVC 中的 Controller，但以可插拔的方式定义。

> ⚠️ **已过时 — 本节描述的是旧版对象安全 `Action` trait（H-1 重构前）**
>
> 以下 `Action` trait、`ApiResponse`、`Request`、`ActionContext` 等 API 的签名和用法写于 H-1 端到端类型化重构之前。当前 Action 系统已改为三层类型化架构，旧的 `Action` trait 已删除（`action_trait.rs` 仅保留 `Permission`）。以下内容仅供理解历史演进参考，**实际开发请以 `action/typed.rs` 和内置 Action 泛型实现为准**。
>
> **当前类型化 Action 系统摘要**：
>
> - **`TypedHandler` trait**（用户手写）：声明关联类型 `Input` / `Output`，编译期固定输入输出契约。实现 `TypedHandler` 的 struct 通过 `#[derive(Action)]` 自动获得 `TypedAction` impl 和 `ActionMeta`。
> - **`TypedAction` trait**（派生层）：由 `#[derive(Action)]` 自动生成，桥接 `TypedHandler` 的强类型世界与注册表所需的类型擦除层。
> - **`DynAction` trait**（类型擦除层）：注册表存储 `Arc<dyn DynAction>`，`ModuleRouter::dispatch` 走 dyn dispatch。`DynAction` 有一个 blanket impl：任何实现了 `TypedAction` 的类型自动实现 `DynAction`。
> - **`#[derive(TableEntity)]`**：生成 `<Name>Field` 封闭字段枚举 + `<Name>Where` 条件枚举 + 运行时 `TableConfig`，杜绝任意字符串列名拼接。
> - **`#[derive(Action)]`**：生成 `TypedAction` impl + `ActionMeta`（含 name、permissions、is_public 等元数据）。
> - **六个内置 Action**（`add`/`put`/`del`/`get`/`select`/`table`）已泛型化为 `XxxAction<T: TableEntity>`，`ModuleRouter::table_typed::<T>()` 一行注册全套 CRUD。
> - **认证内置 Action**（`token` feature）：`LoginAction<V>`（凭证校验委托 `CredentialVerifier`）、`RefreshAction`、`LogoutAction`。
> - 详细设计文档见 `docs/superpowers/plans/2026-05-27-action-typed-system.md`。

---

### 旧版 Action Trait（⚠️ 已删除，仅供参考）

> 以下 Permission、User、GlobalTools、Request、ActionContext、ApiResponse、内置 CRUD Actions 等均为旧版 API 文档，已随 H-1 重构被新类型化系统替代。当前实现请以 `action/typed.rs` 为准。

### Permission（权限类型）

```rust
pub struct Permission { /* name: Cow<'static, str> */ }

Permission::new("user:create")         // 动态字符串（堆分配）
Permission::from_static("user:create") // 静态字符串（零拷贝）
permission.name() -> &str
```

### User（用户信息）

```rust
pub struct User {
    pub id: i64,
    pub username: String,
    pub nickname: String,
    pub email: String,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
}

User::new(id, username)
.has_permission(perm: &str) -> bool
.has_role(role: &str) -> bool
.has_any_role(roles: &[String]) -> bool
```

### GlobalTools（全局工具集）

```rust
// 初始化（仅调用一次）
GlobalTools::init(token_manager)?;     // token feature
GlobalTools::init()?;                  // 无 token feature

// 获取全局引用
let tools: &'static GlobalTools = GlobalTools::get()?;

// 注册自定义工具
tools.register_tool("cache", Arc::new(my_cache));

// 获取工具（类型安全向下转型）
let cache: Option<Arc<MyCache>> = tools.get_tool::<MyCache>("cache");

// 获取 Token 管理器
let manager: &TokenManager = tools.token_manager(); // token feature
```

### Request（请求结构）

```rust
pub struct Request {
    pub body: serde_json::Map<String, Value>,  // 请求体参数
    pub headers: HashMap<String, String>,       // 请求头
    pub query: HashMap<String, String>,         // 查询参数
    pub path_params: HashMap<String, String>,   // 路径参数
}

Request::new(body: Value)
    .header("Authorization", "Bearer xxx")
    .query("page", "1")
    .path_param("id", "123")
    .token() -> Option<String>    // 从 Authorization: Bearer xxx 提取 Token
```

### ActionContext（执行上下文）

```rust
// 构建上下文
let ctx = ActionContext::new(request, Arc::new(tools));
// 或使用全局单例
let ctx = ActionContext::new_with_global_tools(request)?;
// 链式设置
let ctx = ctx.with_user(user).with_table_config(config);

// 参数提取
let name: String = ctx.param("name")?;                  // 必填，类型不匹配返回 Err
let age: Option<i64> = ctx.param_optional("age");       // 宽松，类型不匹配返回 None
let age: Option<i64> = ctx.param_optional_strict("age")?; // 严格，类型不匹配返回 Err
let id: i64 = ctx.path_param("id")?;                    // 路径参数
let page: usize = ctx.query_param("page")?;             // 查询参数（FromStr）
let size: usize = ctx.param_or("size", 10usize);        // 带默认值

// 创建 TableQuery
let query: TableQuery = ctx.table_query()?;
```

### ApiResponse（统一响应格式）

```rust
pub struct ApiResponse {
    pub code: i32,                    // 0=成功，非零=失败
    pub message: String,
    pub data: Option<serde_json::Value>,
}

// 成功响应（data 需序列化）
ApiResponse::success(user, "获取成功")?
// 成功响应（已有 Value，不会失败）
ApiResponse::success_value(json_val, "获取成功")
// 失败响应
ApiResponse::fail(700002, "权限不足")
// 从 BaseError 构建（自动提取错误码）
ApiResponse::from_error(error)
// 默认：code=0, message="OK"
ApiResponse::default()
```

### 内置 CRUD Actions（`feature = "mysql"`）

| Action 名 | 类型 | 行为 |
|-----------|------|------|
| `add` | `AddAction` | 从 `data` 参数插入一条记录 |
| `put` | `PutAction` | 从 `data` 参数按主键更新一条记录 |
| `del` | `DelAction` | 按主键删除一条记录（支持软删除） |
| `get` | `GetAction` | 按主键获取一条记录，不存在返回 `RecordNotFound` |
| `select` | `SelectAction` | 分页查询，支持 `fields`/`where`/`order_by`/`page`/`page_size` |
| `table` | `TableAction` | 返回表元数据（字段列表、类型、权限等） |

**select Action 请求参数**（均可选）：

```json
{
  "fields": ["id", "username"],
  "where": { "status": "active" },
  "order_by": [{ "field": "created_at", "direction": "desc" }],
  "page": 1,
  "page_size": 20
}
```

**page_size 范围**：1~100（超出自动修正），**默认**：page=1, page_size=10。

**自定义 Action 示例**：

```rust
use yang_base::action::{Action, ActionContext, ApiResponse, Permission};
use yang_base::error::BaseError;
use async_trait::async_trait;

pub struct LoginAction;

#[async_trait]
impl Action for LoginAction {
    async fn execute(&self, ctx: ActionContext) -> Result<ApiResponse, BaseError> {
        let username: String = ctx.param("username")?;
        let password: String = ctx.param("password")?;

        // 业务逻辑...
        let (access, refresh) = ctx.tools.token_manager()
            .generate_token_pair(&user_id, json!({ "role": "user" }))?;

        ApiResponse::success(json!({ "access": access, "refresh": refresh }), "登录成功")
    }

    fn name(&self) -> &str { "login" }
    fn display_name(&self) -> &str { "用户登录" }
    fn is_public(&self) -> bool { true }  // 登录不需要认证
}
```

---

## Router 模块

### ModuleRouter（模块路由器）

```rust
use yang_base::router::{ModuleRouter, BUILTIN_ACTION_NAMES};

// BUILTIN_ACTION_NAMES = ["add", "put", "del", "get", "select", "table"]

let router = ModuleRouter::new("user", "用户管理")
    .with_table_config(Arc::new(table_config))
    .default_permissions(vec!["user:access".into()])
    .table_typed::<UserEntity>()?   // 一行注册全套类型化 CRUD（需 mysql feature）
    .register_action(LoginAction)?; // 注册自定义 Action

// 分发请求（完整权限检查流程）
let response = router.dispatch("add", context).await?;

// 使用全局单例自动构建 ActionContext
let response = router.dispatch_with_global("login", request).await?;

// Getter
router.module_name()      // &str
router.display_name()     // &str
router.action_names()     // Vec<String>
router.get_table_config() // Option<&Arc<TableConfig>>
```

**dispatch 权限检查流程**：
1. 查找 Action（不存在 → `ActionNotFound`）
2. 注入 `table_config` 到 context
3. `action.is_public()` → 直接执行
4. `context.user.is_some()` → 否则 `Unauthorized`
5. `default_permissions` 检查 → 否则 `PermissionDenied`
6. `action.permissions()` 检查 → 否则 `PermissionDenied`
7. 执行 `action.execute(context)`

### AppRouter（应用路由器）

```rust
use yang_base::router::AppRouter;

let app_router = AppRouter::new()
    .register_module(user_router)?
    .register_module(order_router)?;

app_router.module_names() // Vec<String>

// 两级分发：(module, action) → 返回 ApiResponse
let response = app_router.dispatch("user", "add", context).await?;
```

---

## 快速启动示例

```rust
use yang_base::{
    plugin::{Plugin, PluginManagerBuilder},
    database::{GlobalDatabase, GlobalRedis, DatabaseInitializer},
    action::{GlobalTools, Request},
    router::{AppRouter, ModuleRouter},
    table::{TableConfig, FieldConfig, FieldType},
    token::TokenManager,
    error::BaseError,
};
use yang_db::{DatabaseConfig, redis::RedisConfig};
use jsonwebtoken::Algorithm;
use std::sync::Arc;
use async_trait::async_trait;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 初始化数据库
    GlobalDatabase::init(
        "mysql://root:password@localhost/myapp",
        DatabaseConfig { max_connections: 20, ..Default::default() }
    ).await?;

    GlobalRedis::init("redis://127.0.0.1:6379", RedisConfig::default()).await?;

    // 2. 初始化全局工具
    let token_manager = TokenManager::new_symmetric(
        "my_secret_key", Algorithm::HS256,
        "myapp".into(), "users".into(), 3600, 86400
    );
    GlobalTools::init(token_manager)?;

    // 3. 注册插件 + 执行数据库迁移
    let mut builder = PluginManagerBuilder::new();
    builder.register(UsersPlugin).await?;
    let registry = builder.build()?;

    let db = GlobalDatabase::get()?.clone();  // 获取底层 Database 克隆
    let initializer = DatabaseInitializer::new(db, true);
    // initializer.initialize_all(&manager).await?;  // 使用 PluginManager 版本

    // 4. 配置表结构
    let users_config = Arc::new(
        TableConfig::new("users")
            .field(FieldConfig::new("id", FieldType::BigInt).display_name("ID"))
            .field(FieldConfig::new("username", FieldType::String { max_length: 50 })
                .required(true))
            .field(FieldConfig::new("email", FieldType::String { max_length: 100 })
                .required(true))
            .soft_delete_field("deleted_at")
            .timestamps("created_at", "updated_at", "deleted_at")
    );

    // 5. 配置路由
    let app_router = AppRouter::new()
        .register_module(
            ModuleRouter::new("user", "用户管理")
                .with_table_config(users_config)
                .table_typed::<UserEntity>()?
        )?;

    // 6. 处理请求（示例）
    let request = Request::new(serde_json::json!({ "data": { "username": "alice", "email": "a@b.com" } }));
    let context = yang_base::action::ActionContext::new_with_global_tools(request)?;
    let response = app_router.dispatch("user", "add", context).await?;

    println!("响应码: {}, 消息: {}", response.code, response.message);

    Ok(())
}
```
