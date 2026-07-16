# lib_yang — 待修复问题 & 改进 Backlog

**生成日期**：2026-05-25
**来源**：基于完整代码审查 + 架构评估对话（见 AGENTS.md）
**覆盖范围**：本文仅覆盖 yang-db/yang-base。yang-pcg 问题追踪见 `crates/yang-pcg/AGENTS.md` 和 `crates/yang-pcg/docs/PRODUCTION_AUDIT_2026-06-24.md`。

> 优先级：🔴 Critical（生产风险）/ 🟠 High（设计缺陷）/ 🟡 Medium（代码质量）/ 🟢 Low（改进建议）
> 状态：✅ 已完成 / 🟨 部分完成 / ⏳ 待处理
> 最近更新：2026-06-27，对 yang-base/yang-db 进行生产就绪度再审（综合评分 71/100，判定 CONDITIONAL），新增 NEW-35~NEW-44 共 10 项发现，含 clippy 门禁修复回归（高优先）等；yang-pcg 不在本轮范围。

## 2026-07-15 完成度对账

本节只对账，不重写下方历史审计。判定来源为 `docs/YANG_BASE_DB_COMPLETENESS_PLAN.md` 与对应实现/测试提交，日期均为 2026-07-15。

- [已完成] NEW-35、NEW-36、NEW-37、NEW-38、NEW-39、NEW-40、NEW-41、NEW-42、NEW-43、NEW-44：已分别由安全边界、错误链、敏感信息脱敏、方言对称性、feature/MSRV/CI 门禁及设计约束条目覆盖。
- [已完成] yang-base/yang-db 完整度计划 P0-01 至 P5-01：以计划内逐点 `DONE` 状态、对抗性测试和独立 Git 提交为准。
- [已失效] “yang-db 包含 MySqlPool 裸指针 unsafe”的旧描述：当前生产源码不再存在该实现，保留历史条目仅用于说明审计来源。
- [已失效] 将 SQLite、MSSQL、备份/恢复视作本轮缺失：这些能力已在支持矩阵中明确列为 non-goal，需要真实消费者与独立 RFC 才会进入范围。

yang-pcg 的 NEW-20~NEW-34 不属于本次 yang-base/yang-db 对账，状态保持不变。

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

**状态**：✅ 已完成，并在 yang-base 0.2.0 收口为 schema-first 公共边界。原计划见 `docs/superpowers/plans/2026-05-27-action-typed-system.md`；当前实现以源码和 0.2.0 文档为准：

- 自定义业务 Action 采用三层 trait：`TypedHandler` 用户手写，`#[derive(Action)]` 生成 `TypedAction` / `ActionMeta`，blanket impl 提供 `DynAction` 擦除层。
- 应用表由 `Table` + `Field` 构建为不可变 `TableDefinition`；字段、权限、索引、关系和输入/输出 JSON Schema 由同一定义产生。
- 六个内置 Action（add/del/get/put/select/table）是非泛型 handler。动态行统一使用透明 JSON object `Record`，主键、表名和字段权限在运行期从绑定定义读取；put/get/del/select 仍使用明确 DTO 固定请求外形。
- `ModuleRouter::table(definition).crud()` 注册标准 CRUD；自定义端点通过 `Api` 与 `ModuleRouter::api` / `apis` 原子注册，避免 Action 与 route 元数据漂移。
- `AppRouter::catalog()` 提供确定性的 Action schema / route 快照，可选 `openapi` feature 从中投影 OpenAPI 3.1。

**当前验收套件**：

- `tests/schema_snapshots.rs` 锁定 `TableDefinition::input_schema()` / `output_schema()`。
- `tests/typed_action_integration.rs` 使用真实 MySQL/Redis 跑通 add → get → put → select → del → table，输入输出按 `Record` 契约断言。
- table/query/router 的单元与集成测试覆盖定义校验、权限、where 树、分页、事务和 `.crud()` 注册。
- `tests/release_docs_contract.rs` 防止发布文档重新出现已删除的应用模型。

**顺带修复的真实 bug**：`ActionContext::table_query()` 早期曾硬编码 `pool: None`，导致 builtin 经 router 派发时 DB 操作返回 `DatabaseNotInitialized`。当前路径会从 `GlobalDatabase` 注入共享连接池，并携带用户角色、request id 与慢查询阈值。

**原始问题**：查询 Action 直接返回裸 `serde_json::Value`，缺少稳定的动态行类型和可复用 schema；调用方只能在运行期自行猜测对象结构。

**最终方案**：自定义业务 DTO 保持编译期强类型，动态表行显式使用 `Record`，表结构与 JSON Schema 由 `TableDefinition` 提供，二者职责不再混淆。

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

### ⏳ [M-1] 测试代码中 unwrap/expect 调用过多（~870+）

**文件**：`crates/yang-db/tests/`、`crates/yang-base/tests/`、`crates/yang-pcg/src/`（测试模块）

**问题**：全 workspace 集成测试和单元测试中存在大量 `.unwrap()` / `.expect()` 调用（yang-base ≈418 处、yang-pcg 约 254 处、yang-db 约 200+ 处，合计 ~870+），导致测试失败时错误信息不明确（thread panicked at 'called `Option::unwrap()` on a `None` value'，无上下文）。

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

## 🆕 yang-pcg 生产审计发现（NEW-20 ~ NEW-34）

> 以下条目来自 `crates/yang-pcg/docs/PRODUCTION_AUDIT_2026-06-24.md`，按优先级摘录关键发现。完整清单与修复路线图见审计报告第九节。

### 🔴 Critical

#### ⏳ [NEW-20] 确定性契约漏洞：DefaultHasher 跨 Rust 版本不稳定

**文件**：`crates/yang-pcg/src/rng.rs:131-134`、`crates/yang-pcg/src/digest.rs:71-75`

**问题**：种子派生和 ConfigDigest 均使用 `std::collections::hash_map::DefaultHasher`（SipHash 算法）。Rust 标准库明确声明 DefaultHasher 的内部算法不保证跨编译器版本稳定。若 Rust 版本升级导致算法变更，所有 `seed: None` 的兜底种子、所有 RNG 派生标签产生的子流将全部改变，等价于破坏所有历史 seed 复现性和黄金测试。

**修复方向**：将 DefaultHasher 替换为 FNV-1a 或 xxhash 等固定算法；更新 CLAUDE.md 声明覆盖全链路。

---

#### ⏳ [NEW-21] SemVer 兼容性：零处 `#[non_exhaustive]`

**文件**：`crates/yang-pcg/src/`（全部 `pub enum` 和 `pub struct`）

**问题**：yang-pcg 中完全没有使用 `#[non_exhaustive]` 标记。16 个公共枚举 + 75 个公共结构体在未来添加新变体/字段时将直接破坏下游编译。yang-base 已有 `#[non_exhaustive]` 先例（如 `FieldType`）。

**修复方向**：在所有公共 enum 上添加 `#[non_exhaustive]`；在所有公共 struct 上添加 `#[non_exhaustive]`，并配套提供 `pub fn new(...)` 构造函数或 Builder 模式。

---

### 🟠 High

#### ⏳ [NEW-22] NaN 权重静默绕过校验

**文件**：`crates/yang-pcg/src/config.rs:469-475`、`crates/yang-pcg/src/rng.rs:383-398`、`crates/yang-pcg/src/grammar/selector.rs:105`

**问题**：三环 NaN 传播链：配置校验中 `NaN.abs() > 0.01` 为 false（绕过）→ `choose_weighted` 中 `NaN <= 0.0` 为 false（不返回 None，fallthrough 到最后一项）→ 地形策略 `NaN as usize = 0`。恶意配置可产出静默错误结果。

**修复方向**：`ItemSpawnConfig::validate()` 入口显式拒绝 NaN（`is_nan()` 检查）；`choose_weighted` 和 `WeightedRuleSelector::select` 添加纵深防御。

---

#### ⏳ [NEW-23] serde_json 序列化失败静默吞咽

**文件**：`crates/yang-pcg/src/digest.rs:41,72`

**问题**：两处使用 `serde_json::to_string(config).unwrap_or_else(|_| String::new())`。若新增不可序列化字段，所有配置摘要退化为空字符串哈希，全部碰撞且零错误信号。

**修复方向**：将 `unwrap_or_else` 替换为显式错误传播（`.expect("GenerationConfig 必须可序列化")` 或返回 Result）。

---

#### ⏳ [NEW-24] 布局重叠检测 O(n²) 重复扫描

**文件**：`crates/yang-pcg/src/layout/solver.rs:132-161,164-176`

**问题**：`nudge_clear` 在 while 循环中每次迭代都调用 `overlaps_any` 遍历全部 placed 列表做 AABB 检测，且每次创建临时 inflated RoomBounds（堆分配）。当前房间数 ≤40 可接受，但 100+ 房间会成为瓶颈。

**修复方向**：将 inflated bounds 提取到循环外；placed 集合使用 R-Tree 或空间哈希。

---

#### ⏳ [NEW-25] 错误链丢失

**文件**：`crates/yang-pcg/src/error.rs:171`

**问题**：`PcgError::Export::source_error` 为 `Option<String>`，底层 `serde_json::Error` 被转为字符串丢弃。下游无法通过 `Error::source()` 追溯根因。

**修复方向**：改为 `Option<Box<dyn std::error::Error>>` 并添加 `#[source]` 属性。

---

#### ⏳ [NEW-26] 公共 API 暴露面过大（19 个 pub mod 全开）

**文件**：`crates/yang-pcg/src/lib.rs`

**问题**：`backend`/`chunked`/`layout`/`topology`/`spawn`/`terrain`/`debug`/`validation`/`constraint`/`cache`/`grammar` 等 10+ 个内部模块全部 `pub mod`，下游可直接 `use yang_pcg::layout::solver::*`。

**修复方向**：改为 `pub(crate) mod`，仅通过 `lib.rs` 的 `pub use` 导出真正公开的类型。

---

#### ⏳ [NEW-27] 内部类型泄露

**文件**：`crates/yang-pcg/src/backend/mod.rs`、`crates/yang-pcg/src/spawn/mod.rs`、`crates/yang-pcg/src/terrain/mod.rs`

**问题**：`PipelineBackend` trait（5 方法）、`select_backend`、`TopDownBackend`、全部 5 个地形策略结构体、7 个 spawn 内部函数变体均 pub。

**修复方向**：PipelineBackend/select_backend/TopDownBackend 改为 `pub(crate)`；地形策略仅暴露 trait；spawn 函数改为 `pub(crate)`。

---

### 🟡 Medium

#### ⏳ [NEW-28] 三种模式 RNG 派生标签无集中契约/回归测试

**文件**：`crates/yang-pcg/src/generator.rs:77-78`、`crates/yang-pcg/src/chunked.rs:388,188`

**问题**：OfflineFullFloor 用单一 `"terrain"`、RuntimeChunked 用 `"terrain:{room_id}"`、HybridPrecompute 用 `"terrain:chunk:{chunk}:{room}"`。差异散落三处，无集中契约表或 goldfile 测试。若未来有人重构"统一"标签将静默破坏某一模式。

**修复方向**：在 `rng.rs` 顶部添加三种模式的完整派生标签契约表；添加 goldfile 确定性回归测试。

---

#### ⏳ [NEW-29] 地形策略回退共享 RNG 流（与 chunked 路径不对称）

**文件**：`crates/yang-pcg/src/terrain/mod.rs:57-62`

**问题**：主策略失败后 DefaultCarveStrategy 回退使用**同一个 rng 引用**，与 chunked 路径中 fallback 的 `derive("terrain:fallback:...")` 不对称。主策略 RNG 消费变化会传播到回退结果。

**修复方向**：在回退前 `rng.derive(&format!("fallback:{}", room.id))` 解耦。

---

#### ⏳ [NEW-30] gen_bool_with_probability 无概率范围校验

**文件**：`crates/yang-pcg/src/rng.rs:205-207`

**问题**：`gen_bool_with_probability` 将 probability 直接传递 rand crate，probability 不在 [0,1] 或 NaN 时 panic。

**修复方向**：添加显式校验：`<=0` → false，`>=1` → true，NaN → false/error。

---

#### ⏳ [NEW-31] choose_weighted 中 assert! 在生产代码 panic

**文件**：`crates/yang-pcg/src/rng.rs:377`

**问题**：`assert_eq!(slice.len(), weights.len())` 在生产代码中 panic。函数已返回 `Option`，应保持 `None` 语义一致性。

**修复方向**：改为 `if slice.len() != weights.len() { return None; }`。

---

### 🟢 Low

#### ⏳ [NEW-32] Box\<dyn TerrainStrategy\> 每房间堆分配 + 虚表

**文件**：`crates/yang-pcg/src/terrain/selector.rs:35-59`

**问题**：对每个房间调用 `Box::new(strategy)` 产生堆分配+虚表。所有策略类型均为 ZST（unit struct）。

**修复方向**：改用 `enum TerrainStrategyKind` + match 分发消除虚表调度和堆分配。

---

#### ⏳ [NEW-33] spawn 模块双份冗余实现

**文件**：`crates/yang-pcg/src/spawn/mod.rs:63-102 vs 112-171`

**问题**：`generate_spawns`（生产路径）和 `generate_spawns_with_debug`（调试路径）是独立函数体而非 tracked 包装 non-tracked。修改一处而忘记同步另一处会破坏 `set_debug(true)` 不改变输出的契约。

**修复方向**：将 debug 版本改为生产版本 + 附加 debug 收集的包装器。

---

#### ⏳ [NEW-34] TerrainStrategy trait 缺 Send + Sync 超约束

**文件**：`crates/yang-pcg/src/terrain/strategy.rs:46`

**问题**：`TerrainStrategy` trait 无 `Send + Sync` 约束——`Box<dyn TerrainStrategy>` 不会被编译器视为 Send，直接阻塞 rayon 并行化。所有当前实现者为 ZST（自动 Send+Sync），添加约束无破坏性。

**修复方向**：在 trait 定义添加 `+ Send + Sync` 约束。

---

## 🆕 2026-06-27 再审新发现（yang-base/yang-db）

> 来源：2026-06-27 生产就绪度再审（综合评分 71/100，判定 CONDITIONAL）。完整报告见 `docs/audit/2026-06-27-yang-base-db-reaudit.md`。以下条目为本轮新发现或确认仍开放的项。

### 🔴 Critical

#### ⏳ [NEW-35] clippy 门禁 RED：yang-db lib 内部调用自身 #[deprecated] execute() 未加 #[allow(deprecated)]（修复回归）

**文件**：`crates/yang-db/src/mysql/database.rs:281,289,300`、`crates/yang-db/src/postgres/database.rs:264,272,283`、`crates/yang-db/src/mysql/condition.rs:166`

**问题**：上轮修复将 `Database::execute()/query()`、`Transaction::execute()` 标记为 `#[deprecated]`，但 `init()`/`create_table()`/`drop_table()` 这三个内部调用方未同步加 `#[allow(deprecated)]`。`doc_lazy_continuation` lint 亦在 `mysql/condition.rs:166` 触发。结果：项目唯一被 README 钦定的质量门 `cargo clippy --all-targets --all-features -- -D warnings` 退出 101，lib 7 个 error，是上轮修复扫尾引入的回归。

**修复方向**：为 `init()`/`create_table()`/`drop_table()` 内对 `self.execute()` 的调用加 `#[allow(deprecated)]`（或将 DDL 工具改走未弃用的内部私有执行函数）；修正 `condition.rs:166` doc 列表缩进。目标：`cargo clippy --all-targets --all-features -- -D warnings` 转绿，这是宣称生产就绪的硬前置。

---

### 🟠 High

#### ⏳ [NEW-36] condition safe_quote_identifier 对非法标识符静默回退 RAW，直接消费 yang-db 公有 API 的调用方存在可达注入旁路

**文件**：`crates/yang-db/src/mysql/condition.rs:171-176`（`postgres/condition.rs` 同构）

**问题**：`condition_to_sql_owned` 对非法标识符仅 `log::warn` 后输出 RAW 字段，而非返回错误。该回退为支持 `a.b` 限定名等 JOIN 表达式而设计，但同一代码路径对恶意载荷（如 `'; DROP TABLE t;--`）同样回退输出 RAW。yang-base 的 `TableDefinition` + `TableQuery` 路径会先校验用户字段名，但直接使用 yang-db 公有 API 并传入外部字符串的调用方仍面临真实注入面。

**修复方向**：为 `condition_to_sql_owned` 增 checked 变体，非法标识符返回 `DbError::InvalidArgument` 而非 RAW 输出；保留 RAW 回退仅给显式标注的限定名 API（如 `quote_qualified`）。同时在 `lib.rs` 重导出 `quote_identifier`/`quote_qualified` 便于下游调用方自检。

---

#### ⏳ [NEW-37] order_by/group_by/join ON 与 value()/create_table()/init() 裸 SQL 面无安全属性

**文件**：`crates/yang-db/src/mysql/query_builder.rs:244-283`（`build_joins`/`build_order_by`/`build_group_by`）、`:1655`（`value`）、`crates/yang-db/src/mysql/database.rs:272-291`（`init`/`create_table`）（PG 同构）

**问题**：`build_joins` 直接 `append(join.table)` 与 `append(join.on)` 无转义；`build_order_by`/`build_group_by` 字段不转义；`value(field)` 仅行内注释无 doc/deprecated；`create_table(create_sql)` 与 `init(sql_script)` 执行任意 DDL 却无 `#[deprecated]` 或安全属性，且其内部调用 `#[deprecated]` `execute()` 未 `#[allow]`（即 NEW-35 lib error 来源之一）。yang-base `TableQuery` 对 `ORDER BY` 已通过 `get_field` 校验，JOIN/value 路径未收口。

**修复方向**：为 `join ON`、`order/group` 字段提供 quoted 变体；`value()`/`create_table()`/`init()` 补 `#[deprecated]` 或安全文档；在 `lib.rs` 重导出 `quote_identifier`/`quote_qualified` 供调用方自检。

---

#### ⏳ [NEW-38] 敏感 DTO 全部 #[derive(Debug)] 明文，潜伏 CWE-312（先前审计 S-M8~M12 被误标为已修）

**文件**：`crates/yang-base/src/action/auth.rs:55`（`LoginInput.password`）、`:67`（`TokenPairResponse`）、`:76`（`RefreshInput`）、`:84`（`AccessTokenResponse`）、`:94`（`LogoutInput`）、`crates/yang-base/src/token/mod.rs:85`（`TokenClaims`）

**问题**：六个含明文 `password`/`access_token`/`refresh_token`/`jti` 字段的 DTO 均 `#[derive(Debug,...)]`，任意 `{:?}` 格式化即全量泄漏。当前审计走 FNV-1a 指纹，无生产 `{:?}` 调用，但一旦 tracing/日志框架启用 debug 格式化即触发 CWE-312。仅 `TokenManager` 本体手写了 Debug 遮蔽。2026-06-24 yang-base 审计将 S-M8~M12 标为已修，本轮复核证实实际仍开放。

**修复方向**：为以上六个 DTO 手写 `impl Debug` 输出脱敏占位符（如 `password: "***"`），或用 `secrecy` crate 包装 `password`/`token` 字段。

---

### 🟡 Medium

#### ⏳ [NEW-39] PG Transaction 缺 impl Drop，未提交事务丢弃无诊断日志（与 MySQL 不对称）

**文件**：`crates/yang-db/src/postgres/transaction.rs`（全文无 `Drop`）

**问题**：MySQL Transaction 在 `mysql/transaction.rs:223-231` 实现了 `Drop`，未提交时输出 `log::warn!`。PG Transaction 无 `impl Drop`——sqlx 底层仍自动回滚，不致数据损坏，但缺少诊断日志，形成可观测性不对称。

**修复方向**：参照 MySQL 为 PG `Transaction` 实现 `Drop`，在 tx 未提交时 `log::warn!`。

---

#### ⏳ [NEW-40] PG SqlValue 漏 #[non_exhaustive] + PG Transaction::execute 漏 #[deprecated]（方言不一致 SemVer 回归）

**文件**：`crates/yang-db/src/postgres/condition.rs:10-11`（PG `SqlValue`）、`crates/yang-db/src/postgres/transaction.rs`（PG `Transaction::execute`）

**问题**：MySQL `SqlValue`（`mysql/condition.rs:8`）已标 `#[non_exhaustive]`，PG 孪生未标，未来加变体即破坏下游 exhaustive-match，属 SemVer 破坏风险。MySQL `Transaction::execute` 已标 `#[deprecated]`，PG 漏标，方言不一致。

**修复方向**：为 PG `SqlValue` 补 `#[non_exhaustive]`；为 PG `Transaction::execute` 补 `#[deprecated]`（对齐 MySQL）。

---

#### ⏳ [NEW-41] cargo fmt 全树 80 文件漂移 + 无 cargo-audit 依赖漏洞扫描

**文件**：`crates/yang-pcg/src/validation.rs`（最重 35 块）等 80 文件、共约 290 diff 块（"290" 为全体文件 hunk 总数，非单文件）；工具链缺 `cargo-audit`

**问题**：`cargo fmt --all -- --check` 退出 1，80 文件漂移。`cargo audit` 不可用（未安装），历史 rsa RUSTSEC-2023-0071（经 sqlx-mysql 引入）未被检测。项目有意无 CI，fmt 漂移不致流水线失败，但降低代码可读性；依赖漏洞扫描缺失属安全审计盲区。

**修复方向**：执行 `cargo fmt --all` 一次性消除漂移；安装 `cargo-audit` 并执行一次，确认/记录 rsa RUSTSEC-2023-0071 处置决策。

---

### 🟢 Low

#### ⏳ [NEW-42] GlobalRedis init/health_check 及操作方法用 e.to_string() 截断错误链

**文件**：`crates/yang-base/src/database/global_redis.rs:107`（`init`，含 TODO(P1-4)）、`:157`（`health_check`）

**问题**：`init` 用 `RedisConnectionFailed(e.to_string())` 截断错误链（TODO 注释指向尚不存在的变体）；`health_check` 用 `RedisOperationFailed(e.to_string())` 截断；约 30 个 Redis 操作方法绕开已存在的 `RedisOperationDbError` `From` 路径，丢失 `source()` 链。属可观测性缺陷，非功能故障。

**修复方向**：在 `error/mod.rs` 新增 `RedisConnectionDbError(#[source] yang_db::DbError)` 变体；`health_check` 及各操作方法改经 `From` 路径保留 `source()` 链。

---

#### ⏳ [NEW-43] String/Bytes/JSON bind 单次 clone 未消除（写路径额外内存分配）

**文件**：`crates/yang-db/src/mysql/query_builder.rs:30,32`、`crates/yang-db/src/postgres/query_builder.rs:35,37`、`crates/yang-db/src/mysql/transaction.rs:553,554`、`crates/yang-db/src/postgres/transaction.rs:552-576`

**问题**：`String`/`Bytes`/`JSON` 类型在 bind 路径每个值各有一次 `clone()`（取 `&SqlValue` 引用后 clone 一次再交给 bind），高频写路径有额外内存分配。非正确性问题，但可在不改接口的情况下消除。

**修复方向**：将 bind 路径从 `value.clone().into()` 重构为直接消费或借用，在可借用处避免该次 clone，减少高频写路径分配。

---

#### ⏳ [NEW-44] u64 as i64 非饱和转换 + verify_token_checked EXISTS+GET 两步非原子（设计权衡，建议文档化）

**文件**：`crates/yang-base/src/token/revocation.rs:83,131`（u64 as i64）、`:178`（EXISTS via is_revoked）、`:182`（GET via subject_min_iat）

**问题**：`u64 as i64` 在 `exp` 超过 `i64::MAX`（约 2^63 秒，现实不可达）时非饱和截断，是 code smell 非生产缺陷。`verify_token_checked` 在 `:178`（EXISTS，is_revoked）与 `:182`（GET，subject_min_iat）分两步非原子查询，存在亚毫秒 TOCTOU 窗口，属 JWT-over-Redis 标准设计权衡。两者均非阻断性问题，但应在代码中文档化设计决策。

**修复方向**：`u64 as i64` 改 `i64::try_from(...).unwrap_or(i64::MAX)` 消除 code smell；`verify_token_checked` EXISTS+GET 两步添加注释说明 TOCTOU 窗口为已知权衡。

---

## 汇总表

| ID | 状态 | 优先级 | Crate | 文件 | 一句话描述 | 再审注记（2026-06-27） |
|----|------|--------|-------|------|------------|------------------------|
| C-1 | ✅ 已完成 | 🔴 Critical | yang-db | redis/client.rs | RedisConfig 连接池参数静默不生效 | pool params 修复已验证；GlobalRedis 错误链丢失见 NEW-42 |
| C-2 | ✅ 已完成 | 🔴 Critical | yang-db | mysql/（某处） | unsafe 裸指针代码未经充分审查 | — |
| H-1 | ✅ 已完成 | 🟠 High | yang-base | action/typed.rs + builtin/* | 端到端类型化（Task 1-8 全完成）+ table_query 连接池注入修复 | — |
| H-2 | ✅ 已满足 | 🟠 High | yang-db | redis/pipeline+transaction.rs | 复核确认已直接包装原生 redis::Pipeline，条目陈旧无需改动 | — |
| H-3 | ✅ 已完成 | 🟠 High | yang-base | database/bundle.rs | DatabaseBundle::init 统一初始化入口 | — |
| H-4 | ✅ 已完成 | 🟠 High | yang-base | token/revocation.rs | Token 撤销/黑名单机制（Redis jti 黑名单） | 撤销/轮换机制已验证；verify_token_checked 双 GET TOCTOU 见 NEW-44 |
| H-5 | ✅ 已完成 | 🟠 High | yang-base | router/middleware.rs | Router 中间件/拦截器（洋葱模型 Middleware/Next） | — |
| H-6 | ✅ 已完成 | 🟠 High | 全局 | Cargo.toml | Edition 标注可能存在不一致，需确认 | — |
| M-1 | ⏳ 待处理 | 🟡 Medium | 全局 | tests/ | 测试中 unwrap/expect 过多（~870+），错误信息不清 | 再审确认仍开放；clippy test 131 errors 与本项强相关 |
| M-2 | ✅ 已完成 | 🟡 Medium | yang-db | mysql/query_builder.rs | having_cond_unchecked 无操作符验证 | — |
| M-3 | ✅ 审计完成 | 🟡 Medium | yang-db/yang-base | （生产代码） | 生产路径 panic 点均为受控不变量/显式契约，无未受控 panic | — |
| M-4 | ✅ 已完成 | 🟡 Medium | 全局 | Cargo.toml | 无 workspace 共享依赖表，版本易漂移 | — |
| L-1 | ✅ 已完成 | 🟢 Low | yang-base | database/global.rs | GlobalDatabase 缺少参数化查询快捷方法 | — |
| L-2 | ✅ 已完成 | 🟢 Low | yang-base | action/auth.rs | 认证内置 Action（login/refresh/logout） | — |
| L-3 | ✅ 已完成 | 🟢 Low | yang-base | table/field_type.rs | Date/DateTime/Timestamp 字段类型未实现 validate | — |
| L-4 | ✅ 已完成 | 🟢 Low | yang-base | http/{client,request,circuit_breaker}.rs | 重试+退避+超时已有，本次补手写按-host 三态熔断器 | docs/yang-base.md 中 circuit_breaker 字段未同步文档（文档 stale）|
| L-5 | ✅ 已完成 | 🟢 Low | 文档 | AGENTS.md | NOTES 节 Edition 描述与 CONVENTIONS 节矛盾 | — |
| NEW-20 | ⏳ 待处理 | 🔴 Critical | yang-pcg | rng.rs / digest.rs | DefaultHasher 跨 Rust 版本不稳定，确定性契约漏洞 | — |
| NEW-21 | ⏳ 待处理 | 🔴 Critical | yang-pcg | （全量 pub enum/struct） | 零处 #[non_exhaustive]，SemVer 兼容性债 | — |
| NEW-22 | ⏳ 待处理 | 🟠 High | yang-pcg | config.rs / rng.rs / selector.rs | NaN 权重三环传播链静默绕过校验 | — |
| NEW-23 | ⏳ 待处理 | 🟠 High | yang-pcg | digest.rs | serde_json 序列化失败静默吞咽，摘要全碰撞 | — |
| NEW-24 | ⏳ 待处理 | 🟠 High | yang-pcg | layout/solver.rs | 布局重叠检测 O(n²) 重复扫描 + 临时堆分配 | — |
| NEW-25 | ⏳ 待处理 | 🟠 High | yang-pcg | error.rs | 错误链丢失，Export::source_error 为 String | — |
| NEW-26 | ⏳ 待处理 | 🟠 High | yang-pcg | lib.rs | 公共 API 暴露面过大，19 个 pub mod 全开 | — |
| NEW-27 | ⏳ 待处理 | 🟠 High | yang-pcg | backend/spawn/terrain/mod.rs | 内部类型泄露（PipelineBackend/策略/spawn 函数） | — |
| NEW-28 | ⏳ 待处理 | 🟡 Medium | yang-pcg | generator.rs / chunked.rs | 三种模式 RNG 标签无集中契约/回归测试 | — |
| NEW-29 | ⏳ 待处理 | 🟡 Medium | yang-pcg | terrain/mod.rs | 地形策略回退共享 RNG 流，跨模式不一致 | — |
| NEW-30 | ⏳ 待处理 | 🟡 Medium | yang-pcg | rng.rs | gen_bool_with_probability 无概率范围校验 | — |
| NEW-31 | ⏳ 待处理 | 🟡 Medium | yang-pcg | rng.rs | choose_weighted 中 assert! 在生产代码 panic | — |
| NEW-32 | ⏳ 待处理 | 🟢 Low | yang-pcg | terrain/selector.rs | Box<dyn TerrainStrategy> 每房间堆分配+虚表 | — |
| NEW-33 | ⏳ 待处理 | 🟢 Low | yang-pcg | spawn/mod.rs | spawn 双份冗余实现，修改同步风险 | — |
| NEW-34 | ⏳ 待处理 | 🟢 Low | yang-pcg | terrain/strategy.rs | TerrainStrategy trait 缺 Send+Sync 阻塞并行化 | — |
| NEW-35 | ⏳ 待处理 | 🔴 Critical | yang-db | mysql/database.rs + pg/database.rs + mysql/condition.rs | clippy 门禁 RED：lib 内部调用 #[deprecated] execute() 未 allow，修复回归 | 2026-06-27 再审新增 |
| NEW-36 | ⏳ 待处理 | 🟠 High | yang-db | mysql/condition.rs:171-176（pg 同构） | condition safe_quote_identifier 对非法标识符静默回退 RAW，直接消费 yang-db 的调用方存在注入旁路 | 2026-06-27 再审新增 |
| NEW-37 | ⏳ 待处理 | 🟠 High | yang-db | mysql/query_builder.rs:244-283,1655 + mysql/database.rs:272-291（pg 同构） | order_by/group_by/join ON 与 value()/create_table()/init() 裸 SQL 面无安全属性 | 2026-06-27 再审新增 |
| NEW-38 | ⏳ 待处理 | 🟠 High | yang-base | action/auth.rs:55,67,76,84,94 + token/mod.rs:85 | 敏感 DTO 全部 #[derive(Debug)] 明文（password/token），潜伏 CWE-312；先前审计 S-M8~M12 被误标为已修 | 2026-06-27 再审证伪"已修"标记 |
| NEW-39 | ⏳ 待处理 | 🟡 Medium | yang-db | postgres/transaction.rs | PG Transaction 缺 impl Drop，未提交事务丢弃无诊断日志（与 MySQL 不对称） | 2026-06-27 再审新增 |
| NEW-40 | ⏳ 待处理 | 🟡 Medium | yang-db | postgres/condition.rs:10-11 + postgres/transaction.rs | PG SqlValue 漏 #[non_exhaustive] + PG Transaction::execute 漏 #[deprecated]（方言不一致 SemVer 回归） | 2026-06-27 再审新增 |
| NEW-41 | ⏳ 待处理 | 🟡 Medium | 全局 | 全树 80 文件 + 工具链 | cargo fmt 80 文件漂移 + 无 cargo-audit，rsa RUSTSEC-2023-0071 未检测 | 2026-06-27 再审新增 |
| NEW-42 | ⏳ 待处理 | 🟢 Low | yang-base | database/global_redis.rs:107,157 | GlobalRedis init/health_check 及操作方法用 e.to_string() 截断错误链 | 2026-06-27 再审新增 |
| NEW-43 | ⏳ 待处理 | 🟢 Low | yang-db | mysql/query_builder.rs:30,32 + pg:35,37 + 事务路径 | String/Bytes/JSON bind 单次 clone 未消除，高频写路径额外内存分配 | 2026-06-27 再审新增 |
| NEW-44 | ⏳ 待处理 | 🟢 Low | yang-base | token/revocation.rs:83,131,178,182 | u64 as i64 非饱和转换 + verify_token_checked EXISTS+GET 两步非原子，建议文档化设计权衡 | 2026-06-27 再审新增 |
