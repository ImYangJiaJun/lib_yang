# lib_yang — 待修复问题 & 改进 Backlog

**生成日期**：2026-05-25
**来源**：基于完整代码审查 + 架构评估对话（见 AGENTS.md）

> 优先级：🔴 Critical（生产风险）/ 🟠 High（设计缺陷）/ 🟡 Medium（代码质量）/ 🟢 Low（改进建议）
> 状态：✅ 已完成 / 🟨 部分完成 / ⏳ 待处理
> 最近更新：2026-05-31，基于当前工作区实现与 `cargo check -p yang-base --all-features`、`cargo test --lib -p yang-base`（326 passed / 8 ignored）验证结果。本轮同步了 H-1（核心完成）、H-3、H-4、H-5、L-2 的实现进展。

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

### ✅ [H-1] builtin Action 使用 serde_json::Value 而非具体类型

**文件**：
- `crates/yang-base/src/action/builtin/select.rs`
- `crates/yang-base/src/action/builtin/get.rs`

**状态**：✅ 已完成。Action 系统已重构为 `TypedHandler` + `#[derive(TableEntity)]` + `#[derive(Action)]` 的端到端类型化方案（计划见 `docs/superpowers/plans/2026-05-27-action-typed-system.md`，Task 1-8 全部完成）：
- 三层 trait（`TypedHandler` 用户手写 → `TypedAction` 派生 → `DynAction` 擦除层 + blanket impl），见 `action/typed.rs`。
- 派生宏 crate `yang-base-derive` 提供 `#[derive(TableEntity)]`（生成 `Field`/`WhereCond` 枚举 + 运行时 `TableConfig`）与 `#[derive(Action)]`（生成 `TypedAction` impl + `ActionMeta`）。
- 六个内置 Action（add/del/get/put/select/table）全部泛型化为 `XxxAction<T: TableEntity>`，输入输出契约由 `TypedHandler::{Input,Output}` 编译期固定；字段名通过 `T::Field` 封闭枚举保证，杜绝任意字符串列名拼接。
- `ModuleRouter::table_typed::<T>()` 一行注册全套 CRUD；`dispatch` 改读 `ActionMeta`，见 `router/module_router.rs`。

**Task 8 验收套件（本会话补齐）**：
- trybuild 编译失败用例 4 个（缺主键 / 非法字段名 / 类型不匹配 / Like on int），`tests/trybuild.rs` + `tests/compile_fail/`，`.stderr` 基线已生成并复跑校验通过。
- insta schema 快照（实体 + `SelectQuery<T>`），`tests/schema_snapshots.rs`，锁定封闭字段枚举 + 按列类型化 WhereOp。
- testcontainers 端到端 CRUD 集成测试 `tests/typed_action_integration.rs`（`#[ignore]`，需 Docker），跑通 add→get→put→select→del→table。
- 删除旧 `Action` trait（`action_trait.rs` 仅留 `Permission`），清理对应死测试。

**顺带修复的真实 bug**：`ActionContext::table_query()` 原硬编码 `pool: None`，导致类型化 builtin 经 router 派发时所有 DB 操作返回 `DatabaseNotInitialized`（功能性死代码）。已修：yang-db 新增 `Database::pool()`，`table_query()` 在 mysql feature 下从 `GlobalDatabase` 注入共享连接池。

**原始问题**：`SelectAction::execute()` 和 `GetAction::execute()` 返回的数据是 `serde_json::Value`（通过 `DynamicRow` 序列化），而不是用户定义的具体 Rust 类型。这导致：
- 编译期无类型检查
- 运行时反序列化错误只能在调用方发现
- 无法生成准确的 API 文档/Schema

**影响**：Action 系统的类型安全性弱，增加集成层出错概率。

**修复方向**：
- 提供泛型版本：`SelectAction<T: for<'r> sqlx::FromRow<'r, MySqlRow> + Serialize>`
- 或通过关联类型让 Action trait 声明输出类型
- 近期可行方案：至少在 execute 返回前做一次 schema 验证

---

### ✅ [H-2] Redis Pipeline/Transaction 是自定义实现而非 redis::pipe()

**文件**：
- `crates/yang-db/src/redis/pipeline.rs`
- `crates/yang-db/src/redis/transaction.rs`

**状态**：✅ 已满足（核查于 2026-05-31，无需改动）。复核现有代码后确认：`RedisPipeline` 与 `RedisTransaction` **已经**直接包装原生 `redis::Pipeline`（字段 `pipe: redis::Pipeline`，构造用 `redis::pipe()`，事务额外 `pipe.atomic()`），所有 add-command 方法委托给 `self.pipe.set/get/del/...`，执行走 `self.pipe.query_async(...)`。没有任何手写 RESP 协议或手动命令缓冲（grep `\r\n` / `format!("*` / `Vec<u8>` 均无命中）。命令集子集问题也不存在——两者都提供 `cmd(&mut self, cmd: redis::Cmd)` 逃生口，可添加任意命令。本条原始描述（"手动维护命令列表并自行构建 Redis 协议"）与当前代码不符，判定为陈旧条目。

**原始问题（已不成立）**：`RedisPipeline` 和 `RedisTransaction` 手动维护命令列表并自行构建 Redis 协议，而 `redis` crate 已提供经过充分测试的 `redis::pipe()` 和 `redis::Script` API。

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

### ✅ [H-3] GlobalDatabase / GlobalRedis 无统一初始化入口

**文件**：`crates/yang-base/src/database/bundle.rs`

**状态**：✅ 已完成。新增 `DatabaseBundle::init(mysql_url, mysql_config, redis_url, redis_config)` 统一入口，按固定顺序（先 MySQL 再 Redis）初始化两个全局单例，任一失败即返回，避免"半初始化"状态。已接入 `database/mod.rs`（feature `mysql`，导出 `DatabaseBundle`）。

**原始问题**：`GlobalDatabase::init()` 和 `GlobalRedis::init()` 是两个独立调用，没有统一的应用启动入口。用户容易漏掉其中一个，且初始化顺序无约束。

**影响**：
- 启动代码分散，容易遗漏
- 无法在编译期检测"数据库已初始化但 Redis 未初始化"的状态

**修复方向**：提供 `DatabaseBundle::init(mysql_url, mysql_config, redis_url, redis_config)` 统一入口，或提供 `AppBuilder` 模式统一组装所有全局单例。

---

### ✅ [H-4] Token 系统缺少 Token 撤销/黑名单机制

**文件**：`crates/yang-base/src/token/revocation.rs`

**状态**：✅ 已完成（方案 A：Redis 黑名单）。`TokenManager` 新增 `revoke_token` / `revoke_claims` / `is_revoked` / `verify_token_checked`：撤销时把 `jti` 写入 Redis（key `token:blacklist:{jti}`，TTL = `exp - now`，过期自动消失），校验时在标准签名/过期校验外额外查黑名单。`verify_token` 本身**不查**黑名单（保持向后兼容）；需要支持登出/撤销的鉴权路径用 `verify_token_checked`。已接入 `token/mod.rs`。

**原始问题**：`TokenManager` 支持 `refresh_access_token()`，但没有 Token 撤销机制。一旦 JWT 签发出去，在过期前无法使其失效（如用户登出、密码修改、强制下线场景）。

**影响**：安全风险——用户登出后 Token 仍然有效至过期。

**修复方向**：
- 方案 A（推荐）：配合 Redis 维护 Token 黑名单（存 `jti`，TTL = Token 剩余有效期）
- 方案 B：使用短期 Access Token（< 5 分钟）+ 长期 Refresh Token，登出时只撤销 Refresh Token
- `TokenManager` 可提供 `revoke_token(jti, ttl)` 接口，内部写 Redis 黑名单

---

### ✅ [H-5] Router 层缺少中间件/拦截器机制

**文件**：`crates/yang-base/src/router/middleware.rs`、`crates/yang-base/src/router/module_router.rs`

**状态**：✅ 已完成（洋葱模型）。新增 `Middleware` trait 与 `Next` 句柄：每个中间件拿到 `ActionContext` 与代表"调用链剩余部分"的 `Next`，可在 `next.run(ctx)` 前后插入逻辑或短路返回。`ModuleRouter::middleware(m)` 注册；`dispatch` 把中间件链作为**最外层**，链尾执行 `authorize_and_dispatch`（内置鉴权 + Action 派发），因此日志/限流/自定义认证可观察并干预所有请求（含会被鉴权拒绝的请求）。因 `ActionContext` 不可 Clone，链以**移动**方式传递 ctx，全链路只有一份上下文。已接入 `router/mod.rs`（导出 `Middleware` / `Next`）。

**原始问题**：`ModuleRouter::dispatch()` 中权限检查硬编码在 dispatch 流程里，没有可插拔的中间件机制。跨切面逻辑（日志、限流、请求追踪、自定义认证）无法优雅注入。

**遗留**：中间件机制本身已可用；可补一个端到端中间件单测（短路 + 前后置）进一步加固，非阻塞。

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

### ✅ [M-3] 生产代码中存在 unwrap() 调用

**文件**：`crates/yang-db/`、`crates/yang-base/`（lints `unwrap_used`/`expect_used` = `warn`）

**状态**：✅ 审计完成（2026-05-31）。全量扫描两个 crate 的 `src/`（排除 `__tests__/`、`#[cfg(test)]`、`proptest!` 块），**真·生产路径** panic 点仅以下三类，逐一确认均为可接受的不变量保证或显式契约，无需改为返回 `Result`：

1. `yang-base/src/table/validator.rs:42,52` — `Regex::new(...).expect(...)`：正则均为编译期字符串字面量（邮箱、E.164 手机号），编译不可能失败，且包在 `OnceLock` 里只初始化一次。属文档化的 infallible 不变量。
2. `yang-base/src/table/entity.rs:235` — `to_v()` 对 `WhereOp` 操作数 `serde_json::to_value(...).expect(...)`：操作数为 `i64`/`i32`/`String`/`Vec` 等基础可序列化类型，序列化不会失败；`expect` 信息明确标注「Serialize 实现有缺陷」，是编程错误守卫而非运行时错误路径。
3. `yang-db/src/mysql/query_builder.rs:930,1005,1157` — `where_*_unchecked()` 系列的 `.unwrap_or_else(|e| panic!())`：这是**有意**的 `_unchecked` API，doc 注释明确「确定操作符合法的场景使用，否则 panic」，与切片索引越界 panic 同性质。已有对应的返回 `Result` 的 checked 版本（`where_and`/`where_or`/`having`）供常规路径使用。

扫描中其余 100+ 命中全部位于测试代码（`__tests__/*.rs`、源文件内 `#[cfg(test)] mod tests`、`proptest!` 宏块），属 M-1 范围，本条不处理。lints 维持 `warn`（不升 `deny`）以容纳测试代码现状，符合既有注释约定。

**原始问题（已审计澄清）**：`yang-db/Cargo.toml` 中 clippy lint 显式允许 `unwrap_used` 和 `expect_used`，意味着生产代码路径中可能存在 panic 点。→ 审计结论：生产路径的 panic 点均为受控不变量/显式契约，无未受控的隐式 panic。

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

### ✅ [L-2] Router 层缺少 refresh_token 内置 Action

**文件**：`crates/yang-base/src/action/auth.rs`

**状态**：✅ 已完成（feature `token`）。新增认证内置 Action：
- `LoginAction<V>`：校验凭证后签发 Token 对。凭证校验因项目而异，委托给业务实现的 `CredentialVerifier` trait，自身只负责"校验通过 → 签发 Token"。
- `RefreshAction`：用 Refresh Token 换新 Access Token，内部走 `verify_token_checked`（被拉黑的 refresh token 不能再刷新），并校验 `token_type == "refresh"`。
- `LogoutAction`：调用 `revoke_token` 写入 Redis 黑名单（依赖 H-4 的撤销机制）。

均通过 `#[derive(Action)]` 标注为 `public`，已接入 `action/mod.rs`（`pub use auth::{CredentialVerifier, LoginAction, LogoutAction, RefreshAction}`）。

**原始现状**：内置 Action 只有 CRUD（add/put/del/get/select/table），没有认证相关的内置 Action（login、logout、refresh_token）。

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

### ✅ [L-4] HttpClient 缺少重试 / 熔断 / 超时策略配置

**文件**：`crates/yang-base/src/http/{client,request,circuit_breaker}.rs`

**状态**：✅ 已完成（2026-05-31）。三要素全部到位：

1. **请求级超时**：`RequestBuilder::timeout(secs)` 覆盖单次请求超时（已有）。
2. **重试 + 指数退避**：`RetryConfig { max_retries, retry_on, backoff_ms }` + `RequestBuilder::retry(cfg)`，默认不重试；启用后对连接/超时错误与命中 `retry_on` 的状态码按 `backoff_ms * 2^attempt` 退避重试（已有）。
3. **熔断器（本次新增）**：手写经典三态熔断器 `CircuitBreaker`（`http/circuit_breaker.rs`），**按目标 host 分键**——一个故障上游被熔断不影响其它健康 host。
   - 状态机：Closed（累计连续失败，达 `failure_threshold` → Open）/ Open（快速失败，冷却 `cooldown_secs` 后放行探测 → HalfOpen）/ HalfOpen（累计 `success_threshold` 次成功 → Closed，任一失败 → 重新 Open）。
   - 配置：`CircuitBreakerConfig { failure_threshold: 5, cooldown_secs: 30, success_threshold: 1 }`（默认值）。通过 `HttpClientConfig.circuit_breaker: Option<_>` 开启，**默认 None，向后兼容**。
   - 失败判定：传输错误与 5xx 记失败，2xx/3xx/4xx 记成功（服务端正常拒绝不算上游故障）。
   - 共享：状态用 `Arc<Mutex<HashMap>>`，随 `HttpClient`/`CircuitBreaker` 的 `clone()` 复用同一份；锁不跨 `.await`。
   - 新增错误 `BaseError::HttpCircuitBreakerOpen(host)`，错误码 `300007`。
   - 接入点：`RequestBuilder::send()` 在每次发送前做准入检查（`send_guarded`），命中 Open 直接返回 `HttpCircuitBreakerOpen` 不发请求；与重试逻辑正交组合（熔断打开属不可重试错误）。

**未做（有意，超出 L-4 范围）**：未引入 `tower` 中间件栈（手写实现已满足需求且零额外依赖）；HalfOpen 不做并发探测限流（轻量场景多个探测并发放行可接受）。

**测试**：`http/__tests__/circuit_breaker_test.rs` 10 个用例，用可注入时钟（`allow_at`/`on_failure_at`）模拟冷却，覆盖阈值打开、成功清零、per-host 隔离、冷却转半开、半开成功恢复/失败重开、阈值=1 立即熔断、clone 共享状态。yang-base lib 测试 322 → 332 全绿，clippy 干净，无 http feature 下 `error/mod.rs` 仍可编译。

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
| H-1 | ✅ 已完成 | 🟠 High | yang-base | action/typed.rs + builtin/* | 端到端类型化（Task 1-8 全完成）+ table_query 连接池注入修复 |
| H-2 | ✅ 已满足 | 🟠 High | yang-db | redis/pipeline+transaction.rs | 复核确认已直接包装原生 redis::Pipeline，条目陈旧无需改动 |
| H-3 | ✅ 已完成 | 🟠 High | yang-base | database/bundle.rs | DatabaseBundle::init 统一初始化入口 |
| H-4 | ✅ 已完成 | 🟠 High | yang-base | token/revocation.rs | Token 撤销/黑名单机制（Redis jti 黑名单） |
| H-5 | ✅ 已完成 | 🟠 High | yang-base | router/middleware.rs | Router 中间件/拦截器（洋葱模型 Middleware/Next） |
| H-6 | ✅ 已完成 | 🟠 High | 全局 | Cargo.toml | Edition 标注可能存在不一致，需确认 |
| M-1 | ⏳ 待处理 | 🟡 Medium | 全局 | tests/ | 测试中 unwrap() 过多，错误信息不清 |
| M-2 | ✅ 已完成 | 🟡 Medium | yang-db | mysql/query_builder.rs | having_cond_unchecked 无操作符验证 |
| M-3 | ✅ 审计完成 | 🟡 Medium | yang-db/yang-base | （生产代码） | 生产路径 panic 点均为受控不变量/显式契约，无未受控 panic |
| M-4 | ✅ 已完成 | 🟡 Medium | 全局 | Cargo.toml | 无 workspace 共享依赖表，版本易漂移 |
| L-1 | ✅ 已完成 | 🟢 Low | yang-base | database/global.rs | GlobalDatabase 缺少参数化查询快捷方法 |
| L-2 | ✅ 已完成 | 🟢 Low | yang-base | action/auth.rs | 认证内置 Action（login/refresh/logout） |
| L-3 | ✅ 已完成 | 🟢 Low | yang-base | table/field_type.rs | Date/DateTime/Timestamp 字段类型未实现 validate |
| L-4 | ✅ 已完成 | 🟢 Low | yang-base | http/{client,request,circuit_breaker}.rs | 重试+退避+超时已有，本次补手写按-host 三态熔断器 |
| L-5 | ✅ 已完成 | 🟢 Low | 文档 | AGENTS.md | NOTES 节 Edition 描述与 CONVENTIONS 节矛盾 |
