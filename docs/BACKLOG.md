# lib_yang — 待修复问题 & 改进 Backlog

**生成日期**：2026-05-25
**来源**：基于完整代码审查 + 架构评估对话（见 AGENTS.md）

> 优先级：🔴 Critical（生产风险）/ 🟠 High（设计缺陷）/ 🟡 Medium（代码质量）/ 🟢 Low（改进建议）
> 状态：✅ 已完成 / 🟨 部分完成 / ⏳ 待处理
> 最近更新：2026-05-27，基于当前工作区实现与 `cargo test --lib -p yang-base`、`cargo test --lib -p yang-db` 验证结果。

---

## 🔴 Critical — 生产风险

### ✅ [C-1] RedisConfig 连接池参数静默失效

**文件**：`crates/yang-db/src/redis/config.rs`、`crates/yang-db/src/redis/client.rs`

**状态**：✅ 已完成。`RedisClient::connect_with_config()` 已将 `RedisConfig::max_connections`、`connect_timeout`、`wait_timeout` 写入 `deadpool_redis::PoolConfig`，并新增 `pool_status().max_size` 验证。

**问题**：`RedisConfig` 中定义的 `pool_size`、`min_idle`、`idle_timeout` 等连接池参数在构建 `deadpool_redis::Pool` 时**未被读取**，实际连接池使用库默认值。用户配置了参数却完全不生效，属于静默失效（silent no-op）。

**影响**：
- 生产环境无法通过配置控制连接池大小，高并发下可能连接耗尽
- 调试困难：配置已设置却行为未改变，排查成本高

**修复方向**：
```rust
// client.rs connect_with_config 中，将 config 参数实际传入 Pool builder
let pool_config = deadpool_redis::Config {
    url: Some(url.into()),
    pool: Some(deadpool::managed::PoolConfig {
        max_size: config.pool_size,
        timeouts: deadpool::managed::Timeouts {
            wait: Some(std::time::Duration::from_secs(config.idle_timeout)),
            ..Default::default()
        },
        ..Default::default()
    }),
    ..Default::default()
};
```

**验证**：写单元测试断言不同 `pool_size` 下 `pool_status().max_size` 返回正确值。

---

### ✅ [C-2] yang-db 包含未审查的 unsafe 代码

**文件**：`crates/yang-db/src/mysql/` 内某处（MySqlPool 裸指针操作）

**状态**：✅ 已完成。`yang-db` 已将 `unsafe_code` 提升为 `deny`，原测试中的 `MaybeUninit` 裸指针池替换为 `connect_lazy()` 懒连接池。

**问题**：`yang-db` 的 `Cargo.toml` 中 `unsafe_code = "allow"`，注释说明是"用于 MySqlPool 的裸指针操作"，但裸指针操作在数据库连接池场景下存在潜在的并发安全风险（Use-After-Free、数据竞争）。

**影响**：
- 潜在 UB（Undefined Behavior），在高并发下可能表现为随机崩溃或数据损坏
- 违反 Rust 内存安全保证

**修复方向**：
1. 定位具体 unsafe 块，评估是否必要
2. 若必要：添加 `# Safety` 注释说明不变量
3. 若不必要：重构为 safe 代码（sqlx 的 `MySqlPool` 本身已是 `Clone + Send + Sync`，通常不需要裸指针）
4. 将 `unsafe_code = "allow"` 改为 `unsafe_code = "warn"` 并逐一审查

---

## 🟠 High — 设计缺陷

### ⏳ [H-1] builtin Action 使用 serde_json::Value 而非具体类型

**文件**：
- `crates/yang-base/src/action/builtin/select.rs`
- `crates/yang-base/src/action/builtin/get.rs`

**问题**：`SelectAction::execute()` 和 `GetAction::execute()` 返回的数据是 `serde_json::Value`（通过 `DynamicRow` 序列化），而不是用户定义的具体 Rust 类型。这导致：
- 编译期无类型检查
- 运行时反序列化错误只能在调用方发现
- 无法生成准确的 API 文档/Schema

**影响**：Action 系统的类型安全性弱，增加集成层出错概率。

**修复方向**：
- 提供泛型版本：`SelectAction<T: for<'r> sqlx::FromRow<'r, MySqlRow> + Serialize>`
- 或通过关联类型让 Action trait 声明输出类型
- 近期可行方案：至少在 execute 返回前做一次 schema 验证

---

### ⏳ [H-2] Redis Pipeline/Transaction 是自定义实现而非 redis::pipe()

**文件**：
- `crates/yang-db/src/redis/pipeline.rs`
- `crates/yang-db/src/redis/transaction.rs`

**问题**：`RedisPipeline` 和 `RedisTransaction` 手动维护命令列表并自行构建 Redis 协议，而 `redis` crate 已提供经过充分测试的 `redis::pipe()` 和 `redis::Script` API。

**影响**：
- 重复造轮子，维护负担高
- 自定义实现覆盖的命令集是 `redis::pipe()` 的子集，功能受限
- 潜在的协议实现错误

**修复方向**：
```rust
// 当前：手动维护 Vec<redis::Cmd>
// 目标：直接包装 redis::Pipeline
pub struct RedisPipeline {
    inner: redis::Pipeline,
    client: RedisClient,
}

impl RedisPipeline {
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.inner.cmd("SET").arg(key.into()).arg(value.into());
        self
    }
    pub async fn execute(self) -> Result<Vec<RedisValue>> {
        let mut conn = self.client.pool().get().await?;
        self.inner.query_async(&mut *conn).await.map_err(...)
    }
}
```

---

### ⏳ [H-3] GlobalDatabase / GlobalRedis 无统一初始化入口

**文件**：`crates/yang-base/src/database/`

**问题**：`GlobalDatabase::init()` 和 `GlobalRedis::init()` 是两个独立调用，没有统一的应用启动入口。用户容易漏掉其中一个，且初始化顺序无约束。

**影响**：
- 启动代码分散，容易遗漏
- 无法在编译期检测"数据库已初始化但 Redis 未初始化"的状态

**修复方向**：提供 `DatabaseBundle::init(mysql_url, mysql_config, redis_url, redis_config)` 统一入口，或提供 `AppBuilder` 模式统一组装所有全局单例。

---

### ⏳ [H-4] Token 系统缺少 Token 撤销/黑名单机制

**文件**：`crates/yang-base/src/token/manager.rs`

**问题**：`TokenManager` 支持 `refresh_access_token()`，但没有 Token 撤销机制。一旦 JWT 签发出去，在过期前无法使其失效（如用户登出、密码修改、强制下线场景）。

**影响**：安全风险——用户登出后 Token 仍然有效至过期。

**修复方向**：
- 方案 A（推荐）：配合 Redis 维护 Token 黑名单（存 `jti`，TTL = Token 剩余有效期）
- 方案 B：使用短期 Access Token（< 5 分钟）+ 长期 Refresh Token，登出时只撤销 Refresh Token
- `TokenManager` 可提供 `revoke_token(jti, ttl)` 接口，内部写 Redis 黑名单

---

### ⏳ [H-5] Router 层缺少中间件/拦截器机制

**文件**：`crates/yang-base/src/router/module_router.rs`

**问题**：`ModuleRouter::dispatch()` 中权限检查硬编码在 dispatch 流程里，没有可插拔的中间件机制。跨切面逻辑（日志、限流、请求追踪、自定义认证）无法优雅注入。

**影响**：业务特定的横切逻辑只能通过修改每个 Action 实现，代码重复。

**修复方向**：
```rust
pub trait Middleware: Send + Sync {
    async fn handle(&self, ctx: ActionContext, next: Next<'_>) -> Result<ApiResponse, BaseError>;
}

impl ModuleRouter {
    pub fn middleware(mut self, m: impl Middleware + 'static) -> Self { ... }
}
```

---

### ✅ [H-6] Cargo.toml 中 Edition 标注有矛盾

**文件**：
- `crates/yang-db/Cargo.toml`
- `crates/yang-pcg/Cargo.toml`
- `AGENTS.md`（NOTES 节）

**状态**：✅ 已完成。Workspace 与各 crate 当前统一为 `edition = "2021"`，`AGENTS.md` 已保留一致描述。

**问题**：`AGENTS.md` 的 NOTES 节仍写着 `edition = "2024"` 是 bug，但 CONVENTIONS 节已更新说 edition 是 2021。需要实际确认两个 crate 的 Cargo.toml 当前值。

**修复方向**：运行 `grep -r "^edition" crates/` 确认实际值，确保所有 crate 统一为 `edition = "2021"`，并删除 AGENTS.md NOTES 节中矛盾的描述。

---

## 🟡 Medium — 代码质量

### ⏳ [M-1] 测试代码中 unwrap() 调用过多（20+）

**文件**：`crates/yang-db/tests/`、`crates/yang-base/tests/`

**问题**：集成测试和单元测试中存在大量 `.unwrap()` 调用，导致测试失败时错误信息不明确（thread panicked at 'called `Option::unwrap()` on a `None` value'，无上下文）。

**修复方向**：
```rust
// 当前
let user = result.unwrap();

// 改为
let user = result.expect("查询用户应成功，数据库已初始化");
// 或
let user = result?;  // 配合 #[tokio::test] 返回 Result<(), Box<dyn Error>>
```

---

### ✅ [M-2] having_cond_unchecked 无操作符验证

**文件**：`crates/yang-db/src/mysql/query_builder.rs`

**状态**：✅ 已完成。`having_cond_unchecked()` 已标记 `#[deprecated]`，文档引导使用返回 `Result` 的 `having_cond()`。

**问题**：`having_cond_unchecked()` 接受任意字符串操作符而不验证，传入非法操作符会生成无效 SQL，只在数据库执行时才报错（而不是在构建时）。

**当前状态**：`having_cond()`（返回 `Result`）已有验证，但 `unchecked` 版本没有任何保护。

**修复方向**：在文档中明确标注 `having_cond_unchecked` 的使用限制，并考虑将其设为 `pub(crate)` 或添加 `#[deprecated]` 注解，引导用户使用 `having_cond`。

---

### 🟨 [M-3] 生产代码中存在 unwrap() 调用

**文件**：`crates/yang-db/`（lints 已通过 `unwrap_used = "allow"` 豁免）

**状态**：🟨 部分完成。`yang-db` 已将 `unwrap_used` / `expect_used` 从 `allow` 调整为 `warn`，并替换了已审查的单元素条件分支 `unwrap()`；完整生产路径 panic 点审计仍需继续。

**问题**：`yang-db/Cargo.toml` 中 clippy lint 显式允许 `unwrap_used` 和 `expect_used`，意味着生产代码路径中可能存在 panic 点。

**修复方向**：
1. 移除 `unwrap_used = "allow"` lint 豁免
2. 逐一将生产代码中的 `.unwrap()` 替换为 `?` 或 `.expect("不可能到达此处: ...")`（有充分 Safety 说明）
3. 测试代码可保留 `.unwrap()`

---

### ✅ [M-4] Workspace 无共享依赖表

**文件**：`Cargo.toml`（workspace 根）

**状态**：✅ 已完成。根 `Cargo.toml` 已增加 `[workspace.dependencies]`，主要公共依赖已通过 `workspace = true` 统一引用。

**问题**：各 crate 的依赖版本分散在各自的 `Cargo.toml` 中，`sqlx`、`tokio`、`serde` 等公共依赖版本需要手动保持同步，容易出现版本漂移。

**修复方向**：使用 Cargo workspace 的 `[workspace.dependencies]` 统一管理版本：
```toml
# workspace Cargo.toml
[workspace.dependencies]
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "mysql"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }

# 各 crate Cargo.toml
[dependencies]
sqlx = { workspace = true }
```

---

## 🟢 Low — 改进建议

### ✅ [L-1] GlobalDatabase::query / execute 缺乏参数化查询快捷方法

**文件**：`crates/yang-base/src/database/global.rs`

**状态**：✅ 已完成。`GlobalDatabase` 已新增 `query_with_params()` 与 `execute_with_params()` 委托方法，并覆盖未初始化错误测试。

**现状**：`GlobalDatabase::query()` 和 `GlobalDatabase::execute()` 只接受裸 SQL 字符串，没有 `query_with_params` / `execute_with_params` 的快捷封装（底层 `Database` 有，但 `GlobalDatabase` 未透传）。

**影响**：用户需要通过 `GlobalDatabase::get()?` 才能使用参数化查询，增加使用门槛，且容易促使用户用字符串拼接 SQL（SQL 注入风险）。

**修复**：在 `GlobalDatabase` 中添加 `query_with_params` 和 `execute_with_params` 的委托方法。

---

### ⏳ [L-2] Router 层缺少 refresh_token 内置 Action

**文件**：`crates/yang-base/src/action/builtin/`

**现状**：内置 Action 只有 CRUD（add/put/del/get/select/table），没有认证相关的内置 Action（login、logout、refresh_token）。

**影响**：每个使用 yang-base 的项目都需要重复实现 JWT 刷新逻辑。

**修复**：提供可选的 `AuthAction` 模块（feature gate），包含 `LoginAction`（验证 + 签发 Token）、`RefreshAction`（Refresh Token → 新 Access Token）、`LogoutAction`（撤销 Token）。

---

### ✅ [L-3] FieldType 对 Date/DateTime/Timestamp 缺乏 validate 实现

**文件**：`crates/yang-base/src/table/field_type.rs`

**状态**：✅ 已完成。`Date`、`DateTime`、`Timestamp` 已接入格式/类型校验，并新增对应单元测试。

**现状**：`FieldType::validate()` 对 `Date`、`DateTime`、`Timestamp`、`Text`、`ForeignKey` 类型直接返回 `Ok(())`（注释说"暂不验证"）。

**影响**：插入不合法的日期字符串（如 `"not-a-date"`）不会在 `TableQuery::insert()` 阶段报错，而是到 MySQL 执行时才报 SQL 错误，错误信息不友好。

**修复**：
```rust
FieldType::Date => {
    if let Some(s) = value.as_str() {
        chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map(|_| ())
            .map_err(|_| BaseError::InvalidFieldType(field_name.to_string(),
                format!("日期格式无效，期望 YYYY-MM-DD，实际: {}", s)))
    } else {
        Err(BaseError::InvalidFieldType(...))
    }
}
```

---

### ⏳ [L-4] HttpClient 缺少重试 / 熔断 / 超时策略配置

**文件**：`crates/yang-base/src/http/client.rs`

**现状**：`HttpClient` 只支持全局超时，没有请求级重试（`max_retries`、指数退避）、熔断（circuit breaker）、或按状态码重试的策略。

**影响**：对外 HTTP 调用遇到临时性 5xx 错误无法自动重试，需要调用方手动实现重试逻辑。

**修复方向**：引入 `tower` 中间件层或手动实现简单重试装饰器：
```rust
pub struct RetryConfig {
    pub max_retries: u32,
    pub retry_on: Vec<u16>,          // 按状态码重试
    pub backoff_ms: u64,             // 初始退避时间（指数退避）
}
```

---

### ✅ [L-5] AGENTS.md NOTES 节中 Edition 描述矛盾

**文件**：`AGENTS.md`

**状态**：✅ 已完成。Edition 描述已与当前 Cargo.toml 保持一致。

**问题**：NOTES 节仍保留 `"Edition 2024 bug": yang-db and yang-pcg specify edition = "2024" — needs fixing to "2021"`，但 CONVENTIONS 节已更新说 edition 是 2021。两处描述不一致，需要验证实际 Cargo.toml 并删除过时的 NOTES 条目。

---

## 汇总表

| ID | 状态 | 优先级 | Crate | 文件 | 一句话描述 |
|----|------|--------|-------|------|------------|
| C-1 | ✅ 已完成 | 🔴 Critical | yang-db | redis/client.rs | RedisConfig 连接池参数静默不生效 |
| C-2 | ✅ 已完成 | 🔴 Critical | yang-db | mysql/（某处） | unsafe 裸指针代码未经充分审查 |
| H-1 | ⏳ 待处理 | 🟠 High | yang-base | action/builtin/select+get.rs | 返回值使用 Value 而非具体类型，类型安全弱 |
| H-2 | ⏳ 待处理 | 🟠 High | yang-db | redis/pipeline+transaction.rs | 自定义 Pipeline/Transaction 实现应替换为 redis::pipe() |
| H-3 | ⏳ 待处理 | 🟠 High | yang-base | database/ | GlobalDatabase/GlobalRedis 无统一初始化入口 |
| H-4 | ⏳ 待处理 | 🟠 High | yang-base | token/manager.rs | 缺少 Token 撤销/黑名单机制 |
| H-5 | ⏳ 待处理 | 🟠 High | yang-base | router/module_router.rs | Router 层缺少中间件/拦截器机制 |
| H-6 | ✅ 已完成 | 🟠 High | 全局 | Cargo.toml | Edition 标注可能存在不一致，需确认 |
| M-1 | ⏳ 待处理 | 🟡 Medium | 全局 | tests/ | 测试中 unwrap() 过多，错误信息不清 |
| M-2 | ✅ 已完成 | 🟡 Medium | yang-db | mysql/query_builder.rs | having_cond_unchecked 无操作符验证 |
| M-3 | 🟨 部分完成 | 🟡 Medium | yang-db | （生产代码） | 生产路径存在 unwrap()，lints 已豁免 |
| M-4 | ✅ 已完成 | 🟡 Medium | 全局 | Cargo.toml | 无 workspace 共享依赖表，版本易漂移 |
| L-1 | ✅ 已完成 | 🟢 Low | yang-base | database/global.rs | GlobalDatabase 缺少参数化查询快捷方法 |
| L-2 | ⏳ 待处理 | 🟢 Low | yang-base | action/builtin/ | 缺少认证相关内置 Action（login/refresh/logout） |
| L-3 | ✅ 已完成 | 🟢 Low | yang-base | table/field_type.rs | Date/DateTime/Timestamp 字段类型未实现 validate |
| L-4 | ⏳ 待处理 | 🟢 Low | yang-base | http/client.rs | HttpClient 缺少重试/熔断策略 |
| L-5 | ✅ 已完成 | 🟢 Low | 文档 | AGENTS.md | NOTES 节 Edition 描述与 CONVENTIONS 节矛盾 |
