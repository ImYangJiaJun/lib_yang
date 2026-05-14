# 实现计划：yang-base 功能完整性与调用链路优化

## 概述

本计划将 `design.md` 中的设计方案拆分为可增量实施的编码任务。任务按照设计中的"实施顺序建议"组织：

1. **基础设施先行**：错误体系改造、Workspace 依赖统一、Feature Gate（为后续重构提供基础）
2. **无依赖独立修复**：孤儿文件清理、`BUILTIN_ACTION_NAMES`、`quote_identifier`、SQL 拼接重构、Validator 正则缓存、锁中毒、时钟异常
3. **单组件重构**：TokenManager 安全加固、ApiResponse 错误传播、ActionContext 参数访问与借用优化、DynamicRow + 内置 Action 修复、ModuleRouter 改进
4. **架构性新增**：AppRouter、GlobalTools 全局单例、HttpClientConfig、PluginManager 依赖图与 JSON Schema
5. **yang-db 关联前置工作**：DatabaseInitializer 参数化查询、GlobalRedis API 补齐与参数统一
6. **测试基础设施与文档**：testcontainers 复用、unwrap → expect、凭证环境变量化、文档章节补全

每个标记为 `*` 的子任务为可选任务（包括所有属性测试、单元测试、集成测试），核心实现子任务不带 `*`。属性测试一一对应 design.md 中的 18 个 Correctness Properties。

## 任务

- [ ] 1. 基础设施：错误体系改造与依赖结构调整
  - [x] 1.1 调整 Workspace 与 Feature Gate
    - 在根 `Cargo.toml` 增加 `[workspace.dependencies]` 集中声明 `tokio`、`serde`、`serde_json`、`thiserror`、`log`、`chrono`、`uuid`、`regex`、`sqlx`、`reqwest`、`jsonwebtoken`、`jsonschema`、`serde_urlencoded`
    - 在根 `Cargo.toml` 增加 `[workspace.lints.rust]` 与 `[workspace.lints.clippy]`，配置 `unused_must_use`、`unsafe_code`、`unwrap_used` 等 lint 等级
    - 修正 `crates/yang-db/Cargo.toml` 与 `crates/yang-pcg/Cargo.toml` 中的 `edition = "2024"` 为 `edition = "2021"`
    - 改造 `crates/yang-base/Cargo.toml` 使 `jsonwebtoken`、`reqwest`、`sqlx`、`regex`、`jsonschema`、`serde_urlencoded` 为 optional 并通过 `default = ["token", "http", "mysql", "validator", "plugin-schema"]` feature 控制
    - 验证 `cargo build --no-default-features` 与默认 features 均通过
    - _Requirements: 34.1, 34.2, 34.3, 34.4, 38.1, 38.2, 38.3_

  - [-] 1.2 新增结构化错误变体并补齐错误码
    - 在 `crates/yang-base/src/error/mod.rs` 新增 `BaseError::HttpClientAlreadyInitialized`、`BaseError::HttpClientNotInitialized` 变体
    - 为新变体在 `BaseError::code()` 中分配错误码 `300005`、`300006`
    - _Requirements: 32.1, 32.2, 32.3_

  - [~] 1.3 改造 BaseError 数据库/HTTP/Token 变体使用 `#[source]` 持有底层错误
    - 将 `DatabaseQueryFailed`、`DatabaseExecuteFailed`、`DatabaseTransactionFailed` 改为持有 `#[source] yang_db::DbError`
    - 将 `HttpRequestFailed`、`HttpClientCreateFailed` 改为持有 `#[source] reqwest::Error`
    - 将 `TokenVerifyFailed`、`TokenParseFailed`、`TokenGenerateFailed` 改为持有 `#[source] jsonwebtoken::errors::Error`
    - 同步更新构造点（含测试代码约 60 处构造）以保持兼容
    - _Requirements: 35.1, 35.2, 35.3_

  - [~] 1.4 完善 `From<yang_db::DbError>` 分类映射
    - 在 `error/mod.rs` 中实现 `From<DbError>`，对 17 个 `DbError` 变体穷尽匹配
    - 区分查询类（`QueryError`/`TableNotFound`/`RowNotFound`/`ColumnNotFound`/`TypeConversionError`/`DeserializationError`/`UnsupportedOperator`/`Unknown`）→ `DatabaseQueryFailed`
    - 区分执行类（`ConstraintError`/`SqlSyntaxError`/`MissingWhereClause`/`MissingGroupByClause`/`SerializationError`）→ `DatabaseExecuteFailed`
    - 区分事务类（`TransactionError`）→ `DatabaseTransactionFailed`
    - 区分连接类（`ConnectionError`）→ `DatabaseConnectionFailed`
    - 区分 Redis 类（`RedisConnectionError`/`RedisCommandError`/`RedisPoolError`/`RedisTypeConversionError`/`RedisTimeoutError`）→ `RedisOperationFailed`
    - _Requirements: 10.2_

  - [~] 1.5 新增 `From<jsonwebtoken::errors::Error>` 实现
    - 在 `error/mod.rs` 中按 `ErrorKind` 分类：`ExpiredSignature` → `TokenExpired`，`InvalidToken`/`InvalidSignature` → `TokenVerifyFailed`，其他 → `TokenParseFailed`
    - _Requirements: 10.1, 10.3, 10.4_

  - [~] 1.6 属性测试：`From` 错误转换映射正确性
    - **Property 6: From 错误转换映射正确性**
    - **Validates: Requirements 10.1, 10.2, 10.3, 10.4**
    - 使用 `proptest` 对所有 `jsonwebtoken::errors::ErrorKind` 与 `yang_db::DbError` 变体进行映射断言

  - [~] 1.7 属性测试：错误链 `source()` 可遍历性
    - **Property 7: 错误链 source() 可遍历性**
    - **Validates: Requirements 35.1, 35.2, 35.3, 35.4**
    - 构造由底层错误转换得到的 `BaseError` 实例，断言 `error.source()` 可至少遍历 2 层

- [ ] 2. 优先级 1：无外部依赖的独立修复
  - [~] 2.1 删除 `table_query_select.rs` 孤儿文件
    - 物理删除 `crates/yang-base/src/table/table_query_select.rs`
    - 同步更新 `crates/yang-base/docs/reference/project_structure.md` 中的引用
    - 确认 `crates/yang-base/src/table/mod.rs` 没有声明该模块
    - _Requirements: 13.1, 13.2, 22.1, 22.2_

  - [~] 2.2 引入 `BUILTIN_ACTION_NAMES` 常量
    - 在 `crates/yang-base/src/router/module_router.rs` 定义 `pub const BUILTIN_ACTION_NAMES: &[&str] = &["add", "put", "del", "get", "select", "table"]`
    - 检查并修正 6 个内置 Action 实现的 `Action::name()` 返回值与该常量保持一致
    - _Requirements: 17.1, 17.2_

  - [~] 2.3 属性测试：内置 Action 名称常量一致性
    - **Property 11: 内置 Action 名称常量一致性**
    - **Validates: Requirements 17.1, 17.2, 17.3**
    - 断言 6 个内置 Action 的 `name()` 与常量数组对应位置严格相等

  - [~] 2.4 实现 `quote_identifier` 与字段名白名单校验
    - 在 `crates/yang-base/src/table/table_query.rs` 中新增私有辅助函数 `is_valid_identifier(&str) -> bool` 与 `quote_identifier(&self, &str) -> Result<String, BaseError>`
    - `quote_identifier` 对合法标识符添加反引号并将内部反引号转义为双反引号；非法字段名（含分号、`--`、空白）或不在 `TableConfig` 中定义的字段返回 `BaseError::FieldNotFound`
    - _Requirements: 14.1, 14.2, 14.3_

  - [~] 2.5 属性测试：字段名转义正确性
    - **Property 2: 字段名转义正确性**
    - **Validates: Requirements 14.1, 14.2, 14.3**
    - 使用 `proptest` 对随机字符串测试 `quote_identifier` 的合法/非法分支

  - [~] 2.6 重构 `append_where_to_sql` 统一 SQL 拼接
    - 在 `table_query.rs` 中提取私有方法 `fn append_where_to_sql(&self, sql: &mut String, params: &mut Vec<SqlParam>) -> Result<(), BaseError>`
    - 集中处理 `Eq/In/Like/Gt/Gte/Lt/Lte/IsNull/IsNotNull` 9 种条件的 SQL 拼接
    - 改造 `build_count_sql`、`build_select_sql`、`build_update_sql_impl`、`build_delete_sql_impl`、`build_insert_sql` 调用此方法
    - 在所有 SQL 构建方法中通过 `quote_identifier` 处理字段名
    - 移除约 80 行重复代码
    - _Requirements: 13.3, 14.1, 14.4, 25.1, 25.2, 25.3_

  - [~] 2.7 属性测试：`WhereCondition` SQL 拼接一致性
    - **Property 3: WhereCondition SQL 拼接一致性**
    - **Validates: Requirements 13.3, 25.1, 25.2**
    - 使用 `proptest` 生成随机 `Vec<WhereCondition>`，断言重构前后产生的 `(sql_fragment, params)` 完全相等（重构前结果通过快照保存）

  - [~] 2.8 增强 `Validator::Email`/`Validator::Phone` 严格化与正则缓存
    - 在 `crates/yang-base/src/table/validator.rs` 中使用 `OnceLock<Regex>` 缓存 `EMAIL_REGEX`、`PHONE_REGEX`
    - `Validator::Email` 使用 `^[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}$` 校验
    - `Validator::Phone` 使用 E.164 正则 `^\+?[1-9]\d{1,14}$`
    - 新增 `Validator::EmailLoose`、`Validator::PhoneLoose` 弱校验变体保持向后兼容
    - 在 `Validator::Regex` 中通过 `OnceLock<RwLock<HashMap<String, Regex>>>` 缓存动态正则
    - _Requirements: 39.1, 39.2, 39.3, 39.4_

  - [~] 2.9 属性测试：`Validator::Email`/`Validator::Phone` 严格格式
    - **Property 15: Validator::Email / Validator::Phone 严格格式**
    - **Validates: Requirements 39.1, 39.2**
    - 使用 `proptest` 测试随机字符串与正则匹配的等价性

- [ ] 3. 锁中毒与时钟异常处理
  - [~] 3.1 实现 `current_unix_timestamp` 辅助函数
    - 在 `crates/yang-base/src/token/manager.rs` 中新增内部函数 `fn current_unix_timestamp() -> Result<u64, BaseError>`
    - 替换 `generate_access_token`、`generate_refresh_token`、`is_token_expiring_soon` 中的 `.unwrap()` 调用
    - 系统时钟早于 `UNIX_EPOCH` 时返回 `BaseError::TokenGenerateFailed` 或 `ConfigError`
    - _Requirements: 11.1, 11.2, 11.3_

  - [~] 3.2 处理 `HttpClient` 与 `GlobalTools` 锁中毒
    - 修改 `crates/yang-base/src/http/client.rs` 中 `set_default_token`、`get_default_token`，使用 `unwrap_or_else(|p| p.into_inner())` 处理 `RwLock::PoisonError`
    - 修改 `crates/yang-base/src/action/context.rs` 中 `register_tool`、`get_tool` 使用相同策略
    - _Requirements: 12.1, 12.2, 12.3_

  - [~] 3.3 单元测试：锁中毒与时钟异常恢复
    - 编写测试验证 `RwLock` 中毒后 `set_default_token`/`get_default_token` 仍能正确读写
    - _Requirements: 11, 12_

- [ ] 4. TokenManager 安全加固
  - [~] 4.1 显式 JWT 算法白名单
    - 修改 `verify_token` 显式设置 `validation.algorithms = vec![self.algorithm]`
    - 显式设置 `validation.required_spec_claims` 包含 `exp`、`iss`、`aud`
    - 设置 `validation.leeway = 0`
    - _Requirements: 16.1, 16.2_

  - [~] 4.2 `Debug` 实现确保不输出密钥
    - 检查 `TokenManager::Debug` 实现，确认输出不包含 `encoding_key`、`decoding_key` 字段或其值
    - 使用 `finish_non_exhaustive()` 收尾
    - _Requirements: 16.4_

  - [~] 4.3 为 `parse_token_unsafe` 补充 `# Safety` 章节
    - 在 `parse_token_unsafe` 文档注释中说明该方法不应用于鉴权决策
    - _Requirements: 16.3_

  - [~] 4.4 属性测试：`TokenManager::Debug` 输出安全性
    - **Property 10: TokenManager::Debug 输出安全性**
    - **Validates: Requirements 16.4**
    - 使用 `proptest` 生成随机密钥构造 `TokenManager`，断言 `format!("{:?}", tm)` 不含密钥子串

  - [~] 4.5 属性测试：JWT 算法白名单
    - **Property 16: JWT 算法白名单**
    - **Validates: Requirements 16.1, 16.2**
    - 构造不同算法签发的 Token，断言只有匹配算法可通过验证

- [ ] 5. ApiResponse 错误传播
  - [~] 5.1 改造 `ApiResponse::success` 为 `Result<Self, BaseError>`
    - 修改 `crates/yang-base/src/action/response.rs` 中 `success` 方法签名为 `pub fn success<T: Serialize>(data: T, message: impl Into<String>) -> Result<Self, BaseError>`
    - 序列化失败时返回 `BaseError::JsonSerializeFailed`
    - 移除原有的 `unwrap_or(serde_json::Value::Null)` 静默吞错逻辑
    - _Requirements: 18.1, 18.3_

  - [~] 5.2 提供 `success_value` 便捷构造器
    - 新增 `pub fn success_value(data: serde_json::Value, message: impl Into<String>) -> Self`
    - _Requirements: 18.2_

  - [~] 5.3 同步更新调用点
    - 修改所有调用 `ApiResponse::success` 的位置（含内置 Action、用户文档示例）以处理新的 `Result` 返回类型
    - _Requirements: 18.1_

  - [~] 5.4 属性测试：`ApiResponse::success` 序列化错误传播
    - **Property 12: ApiResponse::success 序列化错误传播**
    - **Validates: Requirements 18.1, 18.3**
    - 构造可序列化与不可序列化类型，断言行为符合规约

- [ ] 6. ActionContext 参数访问与借用优化
  - [~] 6.1 添加 `path_param`/`query_param`/`param_or` 方法
    - 在 `crates/yang-base/src/action/context.rs` 中新增 `path_param<T: DeserializeOwned>`、`query_param<T: FromStr>`、`param_or<T: DeserializeOwned>` 方法
    - 参数缺失返回 `BaseError::ParamMissing`，类型不匹配返回 `BaseError::ParamInvalid`
    - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5_

  - [~] 6.2 添加 `param_optional_strict` 方法
    - 实现 `param_optional_strict<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, BaseError>`
    - 在原 `param_optional` 实现中加入 `log::warn!` 记录类型不匹配
    - 在文档注释中说明 `param_optional` 与 `param_optional_strict` 的语义区别
    - _Requirements: 29.1, 29.2, 29.3_

  - [~] 6.3 优化 `ActionContext` 用户角色借用
    - 添加 `user_roles_slice(&self) -> &[String]` 方法返回借用而非克隆
    - _Requirements: 24.3_

  - [~] 6.4 重构 `TableQuery::new` 接受 `Arc<[String]>`
    - 修改 `crates/yang-base/src/table/table_query.rs` 中 `TableQuery` 的 `user_roles` 字段为 `Arc<[String]>`
    - 修改 `TableQuery::new` 签名接受 `Arc<[String]>` 类型用户角色
    - 修改 `ActionContext::table_query` 通过 `Arc::from(self.user_roles_slice().to_vec())` 转换
    - _Requirements: 24.1, 24.2_

  - [~] 6.5 属性测试：`ActionContext` 参数访问语义
    - **Property 4: ActionContext 参数访问语义**
    - **Validates: Requirements 8.1, 8.2, 8.3, 8.4, 8.5, 29.2**
    - 使用 `proptest` 构造随机 `path_params`、`query`、`body`，断言 `path_param`/`query_param`/`param_or`/`param_optional_strict` 行为

- [ ] 7. DynamicRow 与内置 Action 修复
  - [~] 7.1 创建 `DynamicRow` 类型
    - 新增 `crates/yang-base/src/table/dynamic_row.rs`
    - 定义 `pub struct DynamicRow { pub columns: serde_json::Map<String, serde_json::Value> }` 并实现 `Debug`、`Clone`、`Serialize`、`Deserialize`
    - 实现 `sqlx::FromRow<MySqlRow>`，按 MySQL 类型解码（INT/BIGINT → i64、FLOAT/DOUBLE → f64、VARCHAR → String、BOOLEAN → Bool、DATE/DATETIME → ISO 8601、NULL → Null、BLOB → Base64、JSON → Object/Array）
    - 在 `table/mod.rs` 中声明 `pub mod dynamic_row;`
    - _Requirements: 1.4_

  - [~] 7.2 单元测试：`DynamicRow` 类型映射
    - 测试每种 MySQL 类型到 JSON 类型的映射
    - _Requirements: 1.4_

  - [~] 7.3 实现 `parse_paging_param` 安全转换辅助函数
    - 在 `crates/yang-base/src/action/builtin/select.rs` 中新增 `fn parse_paging_param(ctx: &ActionContext, key: &str, default: i64, min: i64, max: i64) -> Result<usize, BaseError>`
    - 越界值返回 `BaseError::ParamInvalid`，使用 `usize::try_from` 替代 `as usize`
    - _Requirements: 1.5, 15.1, 15.2, 15.3_

  - [~] 7.4 修复 `SelectAction::execute`
    - 重写 `crates/yang-base/src/action/builtin/select.rs` 中的 `execute` 方法
    - 通过 `parse_paging_param` 获取 `page`（默认 1，范围 1..=i64::MAX）与 `page_size`（默认 10，范围 1..=100）
    - 通过 `context.table_query()` 构建查询，使用 `DynamicRow` 类型执行分页查询并返回 `ApiResponse`
    - _Requirements: 1.1, 1.5, 15.1, 15.2_

  - [~] 7.5 修复 `GetAction::execute`
    - 重写 `crates/yang-base/src/action/builtin/get.rs` 中的 `execute` 方法
    - 从主键参数获取值，使用 `DynamicRow` 类型执行 `fetch_optional` 查询
    - 查询结果为空时返回 `BaseError::RecordNotFound`，否则返回 `ApiResponse::success`
    - _Requirements: 1.2, 1.3_

  - [~] 7.6 属性测试：分页参数边界验证
    - **Property 1: 分页参数边界验证**
    - **Validates: Requirements 1.5, 15.1, 15.2, 15.3**
    - 使用 `proptest` 生成任意 `i64` 值测试 `parse_paging_param` 边界

- [ ] 8. ModuleRouter 改进
  - [~] 8.1 `register_builtin_actions` 改为 `Result` 返回
    - 修改 `crates/yang-base/src/router/module_router.rs` 中 `register_builtin_actions` 签名为 `pub fn register_builtin_actions(self) -> Result<Self, BaseError>`
    - 未设置 `table_config` 时返回 `Err(BaseError::TableConfigNotSet)`
    - 通过 `BUILTIN_ACTION_NAMES` 常量驱动注册循环
    - 移除 `# Panics` 章节
    - _Requirements: 2.1, 2.2, 2.3, 17.3_

  - [~] 8.2 重命名 `table_config` getter 解决命名冲突
    - 将 `ModuleRouter::table_config(&self) -> Option<Arc<TableConfig>>` 重命名为 `get_table_config(&self) -> Option<&Arc<TableConfig>>`
    - 保留 `with_table_config` 作为 builder setter
    - 新增 `table_config` 作为链式 setter 别名（委托给 `with_table_config`），使文档示例可用
    - _Requirements: 6.1, 6.2, 23.1, 23.2_

  - [~] 8.3 更新 `ModuleRouter` 文档示例
    - 修正 `crates/yang-base/src/router/mod.rs` 与 `crates/yang-base/src/router/module_router.rs` 文档注释中错误的 API 用法
    - 通过 `cargo doc` 验证文档示例编译通过
    - _Requirements: 6.3, 23.3_

  - [~] 8.4 同步更新调用点
    - 修改所有调用 `register_builtin_actions` 的位置以处理 `Result` 返回类型（含示例与测试）
    - _Requirements: 2.2_

- [ ] 9. 检查点 - 确保前置组件测试全部通过
  - 运行 `cargo clippy --all-targets --all-features -- -D warnings`
  - 运行 `cargo test --lib -p yang-base`
  - 运行 `cargo doc --no-deps`
  - 确保所有测试通过，遇到问题向用户求助。

- [ ] 10. AppRouter 多模块聚合
  - [~] 10.1 创建 `AppRouter` 结构
    - 新增 `crates/yang-base/src/router/app_router.rs`
    - 定义 `pub struct AppRouter { modules: HashMap<String, ModuleRouter> }`
    - 实现 `new`、`register_module(router) -> Self`、`module_names() -> Vec<String>`
    - 在 `crates/yang-base/src/router/mod.rs` 中导出 `AppRouter`
    - _Requirements: 9.1, 9.4_

  - [~] 10.2 实现 `AppRouter::dispatch`
    - 实现 `pub async fn dispatch(&self, module_name: &str, action_name: &str, context: ActionContext) -> Result<ApiResponse, BaseError>`
    - 模块不存在时返回 `BaseError::ActionNotFound(format!("模块不存在: {}", module_name))`
    - 确保 `AppRouter` 自动满足 `Send + Sync`
    - _Requirements: 9.2, 9.3, 9.5_

  - [~] 10.3 属性测试：`AppRouter` 路由分发正确性
    - **Property 5: AppRouter 路由分发正确性**
    - **Validates: Requirements 9.2, 9.3, 9.4**
    - 使用 `proptest` 生成随机模块名集合，断言 `dispatch` 与 `module_names` 行为一致

- [ ] 11. GlobalTools 全局单例
  - [~] 11.1 实现 `GlobalTools::init` 与 `GlobalTools::get`
    - 在 `crates/yang-base/src/action/context.rs` 中添加 `static GLOBAL_TOOLS: OnceLock<GlobalTools> = OnceLock::new();`
    - 实现 `pub fn init(token_manager: TokenManager) -> Result<(), BaseError>`，重复初始化返回 `BaseError::ConfigError("GlobalTools 已初始化")`
    - 实现 `pub fn get() -> Result<&'static GlobalTools, BaseError>`，未初始化返回 `BaseError::ConfigError("GlobalTools 未初始化")`
    - _Requirements: 3.1, 3.2, 3.3, 3.4_

  - [~] 11.2 集成全局单例到 `ModuleRouter::dispatch`
    - 在 `ModuleRouter::dispatch` 中当未提供外部 `tools` 参数时通过 `GlobalTools::get()` 自动获取
    - _Requirements: 3.5_

  - [~] 11.3 单元测试：`GlobalTools` 单例
    - 测试 `init`/`get` 重复初始化、未初始化场景
    - _Requirements: 3_

- [ ] 12. HttpClient 配置与请求构建器改进
  - [~] 12.1 引入 `HttpClientConfig` 与 `with_config`
    - 在 `crates/yang-base/src/http/client.rs` 中新增 `pub struct HttpClientConfig { timeout_secs, pool_max_idle_per_host, pool_idle_timeout_secs, user_agent, accept_invalid_certs, proxy_url }` 与 `Default` 实现
    - 新增 `pub fn with_config(cfg: HttpClientConfig) -> Result<Self, BaseError>`
    - 改造原 `HttpClient::new(timeout_secs)` 委托给 `with_config`
    - _Requirements: 26.1, 26.2_

  - [~] 12.2 改造 `init_global`/`global` 使用结构化错误
    - `init_global` 重复初始化返回 `BaseError::HttpClientAlreadyInitialized`
    - `global()` 未初始化返回 `BaseError::HttpClientNotInitialized`
    - _Requirements: 32.1, 32.2_

  - [~] 12.3 `RequestBuilder` header 错误累积
    - 在 `crates/yang-base/src/http/request.rs` 中新增 `header_errors: Vec<String>` 字段
    - 修改 `header`、`headers`、`bearer_token`、`content_type`、`user_agent` 方法在解析失败时累积错误而非静默忽略
    - 在 `send().await` 时若错误集合非空，返回包含每个非法 header 描述的 `BaseError::HttpRequestFailed` 或 `BaseError::ParamInvalid`
    - _Requirements: 26.3, 27.1, 27.2, 27.3_

  - [~] 12.4 `RequestBuilder::form` 改用 `serde_urlencoded`
    - 修改 `RequestBuilder::form` 使用 `serde_urlencoded::to_string` 序列化表单
    - 自动设置 `Content-Type: application/x-www-form-urlencoded`
    - _Requirements: 28.1, 28.2_

  - [~] 12.5 在 `RequestBuilder::send` 与 `HttpClient::request` 中明确连接池复用语义
    - 在文档注释中标明 `self.client.clone()` 是 `Arc::clone`，复用同一连接池
    - _Requirements: 30.1, 30.2_

  - [~] 12.6 属性测试：HTTP form URL 编码往返
    - **Property 13: HTTP form URL 编码往返**
    - **Validates: Requirements 28.1, 28.2, 28.3**
    - 使用 `proptest` 生成含 ASCII、URL-unsafe、UTF-8 字符的 `(key, value)` 对，断言编码后解码可还原

  - [~] 12.7 属性测试：`RequestBuilder` header 错误累积
    - **Property 14: RequestBuilder header 错误累积**
    - **Validates: Requirements 27.1, 27.2, 27.3**
    - 使用 `proptest` 生成合法/非法 header 序列，断言累积行为

  - [~] 12.8 集成测试：HTTP 客户端连接池复用
    - **Validates: Requirements 30.3**
    - 通过 `wiremock` 服务器统计连续 100 次请求的握手次数，断言握手次数显著少于 100

- [ ] 13. PluginManager 依赖图与 JSON Schema
  - [~] 13.1 `PluginManagerBuilder::build` 检查依赖完整性
    - 修改 `crates/yang-base/src/plugin/mod.rs` 中 `PluginManagerBuilder::build` 签名为 `pub fn build(self) -> Result<PluginRegistry, BaseError>`
    - 遍历每个插件的 `dependencies()`，依赖未注册时返回 `BaseError::PluginDependencyMissing(plugin, dep)`
    - _Requirements: 20.1, 20.2, 20.3_

  - [~] 13.2 拓扑排序检测循环依赖
    - 修改 `PluginRegistry::compute_topological_sort` 与 `PluginManager::topological_sort`
    - 当 `sorted_names.len() < plugins.len()` 时返回 `BaseError::PluginCircularDependency`，错误信息含未排序节点
    - 修改 `PluginRegistry::new` 与相关入口返回 `Result`
    - _Requirements: 19.1, 19.2, 19.3, 19.4_

  - [~] 13.3 `PluginManager::validate_config` 集成 `jsonschema`
    - 实现 `validate_config(&self, plugin_name: &str, config: &Value, schema: &Value) -> Result<(), BaseError>` 使用 `jsonschema::JSONSchema::compile` + `validate`
    - 配置不符合 Schema 时返回 `BaseError::PluginConfigInvalid(plugin_name, validation_errors_msg)`
    - 插件未定义 `config_schema`（返回 `None`）时跳过验证直接存储
    - 在 `Cargo.toml` 中添加 `jsonschema` 可选依赖（已在任务 1.1 中通过 `plugin-schema` feature 控制）
    - _Requirements: 7.1, 7.2, 7.3, 7.4_

  - [~] 13.4 属性测试：插件依赖图验证
    - **Property 8: 插件依赖图验证**
    - **Validates: Requirements 19.1, 19.2, 19.3, 20.1, 20.2**
    - 使用 `proptest` 生成随机依赖图（含缺失依赖、循环、合法），断言 `build` 返回值类型

  - [~] 13.5 属性测试：JSON Schema 配置验证一致性
    - **Property 9: JSON Schema 配置验证一致性**
    - **Validates: Requirements 7.1, 7.2**
    - 使用 `proptest` 生成 JSON 值与 Schema，断言与 `jsonschema::is_valid` 等价

- [ ] 14. yang-db 关联前置：参数化查询接口与 DatabaseInitializer
  - [~] 14.1 核对 `yang-db::Database` 参数化查询接口
    - 检查 `crates/yang-db/src/mysql/` 是否已暴露 `query_with_params` 与 `execute_with_params`（含事务版）
    - 若缺失，则在 `yang-db` 中补齐对应方法（保持现有接口签名风格），并向用户确认 yang-db 范围内的修改
    - _Requirements: 4.1_

  - [~] 14.2 改造 `DatabaseInitializer` 使用参数化查询
    - 修改 `crates/yang-base/src/database/initializer.rs` 中 `record_migration`、`is_migration_executed` 使用 `execute_with_params` 与 `query_with_params`
    - 改造 `run_migrations_in_tx` 中迁移记录的插入操作使用事务的参数化查询，确保与迁移 SQL 同事务
    - 移除字符串拼接 SQL
    - _Requirements: 4.1, 4.2, 4.3_

  - [~] 14.3 集成测试：迁移记录 SQL 注入安全性
    - **Property 17: 迁移记录 SQL 注入安全性**
    - **Validates: Requirements 4.1, 4.2**
    - 使用 `proptest` 生成含 SQL 元字符的字符串作为 `module_name` 与 `version`，断言写入后查询结果为字面值且其他表结构不受影响
    - 使用 testcontainers 启动 MySQL，标记 `#[ignore]`

- [ ] 15. yang-db 关联前置：Redis API 补齐与 GlobalRedis 改造
  - [~] 15.1 核对 `yang-db::RedisClient` API 完整性
    - 检查 `crates/yang-db/src/redis/` 是否暴露 `incr`/`decr`/`incrby`/`hincrby`/`mget`/`mset`/`zrange_with_scores`/`zrevrange`/`zincrby`/`set_bytes` 等方法
    - 若缺失，则在 `yang-db` 中补齐（参考已有 `RedisTransaction`、`RedisClient` 风格），并向用户确认变更范围
    - _Requirements: 40.1, 40.2, 40.3_

  - [~] 15.2 统一 `GlobalRedis` 批量参数类型
    - 修改 `crates/yang-base/src/database/global_redis.rs` 中 `del`、`exists`、`lpush`、`rpush`、`sadd`、`srem`、`zrem`、`hdel` 的批量参数为 `&[impl AsRef<str>]`，内部转为 `Vec<String>` 调用 yang-db
    - 单值方法 (`set`、`get`、`hset`、`hget` 等) 保持 `impl Into<String>` 不变
    - _Requirements: 5.1, 5.2, 5.3_

  - [~] 15.3 补齐 `GlobalRedis` 缺失 API
    - 新增 `incr`、`decr`、`incrby`、`hincrby`、`mget`、`mset`、`zrange_with_scores`、`zrevrange`、`zincrby` 方法
    - 新增 `set_bytes(key, &[u8])` 方法
    - _Requirements: 40.1, 40.3_

  - [~] 15.4 暴露 Pipeline / Transaction 入口
    - 新增 `GlobalRedis::transaction()` 返回 `yang_db::RedisTransaction`
    - 在文档注释中说明可直接使用 yang-db 的 Pipeline/Lua 类型
    - _Requirements: 40.2_

  - [~] 15.5 属性测试：`GlobalRedis` 批量参数兼容性
    - **Property 18: GlobalRedis 批量参数兼容性**
    - **Validates: Requirements 5.1, 5.2, 5.3**
    - 使用 `proptest` 生成 `&[&str]` 与 `&[String]` 输入，断言两种参数下产生等价命令副作用（通过 mock 客户端或 testcontainers）

  - [~] 15.6 集成测试：`GlobalRedis` 新增 API
    - 使用 testcontainers 启动 redis:7-alpine，标记 `#[ignore]`
    - 测试 `incr`/`decr`/`mget`/`mset`/`zincrby`/`set_bytes` 行为
    - _Requirements: 40_

- [ ] 16. 测试基础设施与代码质量
  - [~] 16.1 共享 testcontainers 实例
    - 新增 `crates/yang-base/tests/common/mod.rs`，使用 `tokio::sync::OnceCell` 提供 `shared_mysql()` 共享 MySQL 容器
    - 修改 `tests/table_query_paginate_test.rs`、`tests/table_query_crud_test.rs`、`tests/database_initializer_test.rs`、`tests/database_test.rs` 使用 `shared_mysql()`
    - 在 `tests/README.md` 中说明 `--test-threads=1` 与容器复用方式
    - _Requirements: 31.1, 31.2, 31.3_

  - [~] 16.2 集成测试 `unwrap` → `expect` 替换
    - 替换 `tests/table_query_paginate_test.rs`、`tests/table_query_crud_test.rs`、`tests/database_initializer_test.rs`、`tests/database_test.rs` 中至少 30 处 `.unwrap()` 为 `.expect("<具体上下文>")`
    - 在 `crates/yang-base/AGENTS.md` 中加入测试约定：禁止裸 `.unwrap()`
    - _Requirements: 21.3, 36.1, 36.2, 36.3_

  - [~] 16.3 测试中 `panic!` → `assert!(matches!(...))` 替换
    - 修改 `crates/yang-base/src/table/field_type.rs:673, 893, 946` 三处 `panic!("期望 ... 错误")` 为 `assert!(matches!(...))`
    - 修改 `crates/yang-base/src/token/__tests__/manager_test.rs:173, 275, 398` 同样改造
    - _Requirements: 21.1, 21.2_

  - [~] 16.4 凭证环境变量化
    - 修改 `.mcp.json` 移除明文密码，使用占位符或环境变量
    - 在 `.gitignore` 中加入 `.mcp.local.json`
    - 测试代码使用 `std::env::var("MYSQL_TEST_PASSWORD").unwrap_or_else(|_| "111111".to_string())`
    - 在 `tests/README.md` 与 `crates/yang-base/AGENTS.md` 中记录凭证注入流程
    - _Requirements: 33.1, 33.2, 33.3_

- [ ] 17. 文档与公开 API 完整性
  - [~] 17.1 补全 `# Errors` 章节
    - 为 `TokenManager`、`HttpClient`、`GlobalDatabase`、`GlobalRedis`、`PluginManager`、`ModuleRouter::dispatch` 等公开方法补充 `# Errors` 章节，列出可能的错误变体
    - _Requirements: 37.1_

  - [~] 17.2 移除已不再 panic 的 `# Panics` 章节
    - 移除 `register_builtin_actions` 文档中的 `# Panics` 章节
    - _Requirements: 37.2_

  - [~] 17.3 添加可执行 doctest
    - 为 `TableQuery` 至少一个公开方法补充 `# Examples` 章节，去除 `,ignore` 标记使 doctest 可执行
    - _Requirements: 37.4_

  - [~] 17.4 强制公开 API 100% 文档覆盖
    - 添加 `#![deny(missing_docs)]` 到 `crates/yang-base/src/lib.rs` 或通过 workspace lints 配置
    - 通过 `cargo doc --no-deps` 验证
    - _Requirements: 37.3_

- [ ] 18. 最终检查点 - 全量验证
  - 运行 `cargo fmt --all -- --check`
  - 运行 `cargo clippy --all-targets --all-features -- -D warnings`
  - 运行 `cargo test --lib --all-features`
  - 运行 `cargo test -p yang-base -- --ignored --test-threads=1`（需 Docker MySQL/Redis）
  - 运行 `cargo doc --no-deps --all-features`
  - 运行 `cargo build --no-default-features` 验证 feature gate 正确性
  - 确保所有测试通过，遇到问题向用户求助。

## 备注

- 标记为 `*` 的子任务为可选任务（包含所有属性测试、单元测试与部分集成测试），可在 MVP 阶段跳过
- 每个任务通过 `_Requirements: X.Y_` 标注追溯到具体需求条款
- 18 个属性测试一一对应 design.md 中的 18 个 Correctness Properties
- 检查点（任务 9、18）确保增量验证，集中处理累积问题
- 任务 14、15 涉及 `yang-db` 接口变更，实施前需向用户确认变更范围
- 整体执行顺序遵循 design.md 的"实施顺序建议"，前置任务先于依赖任务
- 关于属性测试库选择：使用 `proptest = "1.11"`，与 `yang-db` 现有依赖保持一致
