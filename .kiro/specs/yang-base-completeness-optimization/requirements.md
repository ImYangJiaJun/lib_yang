# 需求文档

## 简介

本文档针对 `yang-base` crate 进行全面的功能完整性分析与调用链路优化。`yang-base` 是 YANG 后端框架的核心基础库，包含 action（动作系统）、database（数据库连接）、error（错误处理）、http（HTTP 客户端）、plugin（插件系统）、router（路由）、table（表配置）、token（JWT 令牌）八个模块。

经过深度代码扫描，发现以下核心问题：

1. **功能缺口**：`SelectAction` 和 `GetAction` 内置动作因泛型类型问题直接返回 `Err(Unknown)`，完全不可用；`GlobalTools` 缺少全局单例访问模式；`PluginManager` 的 JSON Schema 验证为空实现；`DatabaseInitializer` 的 SQL 拼接存在注入风险；`ModuleRouter::register_builtin_actions()` 在未设置 `table_config` 时会 panic。
2. **调用链路问题**：`ActionContext::table_query()` 每次调用都重新克隆 `TableConfig`；`GlobalTools` 需要在构建时传入 `TokenManager`，但没有全局访问入口；`ModuleRouter` 的 `dispatch` 方法中权限检查逻辑与 Action 执行耦合；`DatabaseInitializer` 的迁移记录使用字符串拼接而非参数化查询。
3. **API 一致性问题**：`ModuleRouter` 有 `table_config()` 方法但构建时用 `with_table_config()`，而文档示例中写的是 `.table_config()`；`GlobalRedis` 的 `del`/`exists`/`lpush` 等方法接受 `&[String]` 而非 `impl Into<String>`，与其他方法不一致。

---

## 词汇表

- **Yang_Base**：yang-base crate 整体系统
- **Action_System**：动作系统，包含 `Action` trait、`ActionContext`、内置 CRUD Actions
- **Plugin_System**：插件系统，包含 `Plugin` trait、`PluginManager`、`PluginManagerBuilder`、`PluginRegistry`
- **Router**：路由系统，即 `ModuleRouter`，负责 Action 的注册与分发
- **Table_System**：表配置系统，包含 `TableConfig`、`FieldConfig`、`TableQuery`
- **Database_Layer**：数据库层，包含 `GlobalDatabase`、`GlobalRedis`、`DatabaseInitializer`
- **Token_Manager**：JWT 令牌管理器，即 `TokenManager`
- **Http_Client**：HTTP 客户端，即 `HttpClient`
- **Global_Tools**：全局工具集合，即 `GlobalTools`，持有 `TokenManager` 和自定义工具
- **Dynamic_Row**：动态行类型，用于在不指定具体 Rust 结构体的情况下表示数据库查询结果
- **Action_Context**：Action 执行上下文，即 `ActionContext`
- **Module_Router**：模块路由器，即 `ModuleRouter`

---

## 需求

### 需求 1：修复内置 Action 的动态查询能力

**用户故事：** 作为框架使用者，我希望 `SelectAction` 和 `GetAction` 能够真正执行数据库查询并返回结果，而不是直接返回错误，以便我可以通过注册内置 Action 快速搭建 CRUD 接口。

#### 验收标准

1. WHEN `SelectAction::execute` 被调用，THE Action_System SHALL 使用 `Dynamic_Row`（基于 `serde_json::Value` 的行类型）执行分页查询并返回包含数据列表和分页信息的 `ApiResponse`。
2. WHEN `GetAction::execute` 被调用，THE Action_System SHALL 使用 `Dynamic_Row` 按主键查询单条记录并返回 `ApiResponse`。
3. IF 查询结果为空，THEN THE Action_System SHALL 在 `GetAction` 中返回 `BaseError::RecordNotFound` 错误。
4. THE Action_System SHALL 为 `Dynamic_Row` 实现 `sqlx::FromRow`，使其能够将 MySQL 行数据映射为 `serde_json::Value`。
5. WHEN `SelectAction` 应用分页参数，THE Action_System SHALL 确保 `page` 最小值为 1，`page_size` 范围为 1 到 100。

---

### 需求 2：消除 `register_builtin_actions` 的 panic 风险

**用户故事：** 作为框架使用者，我希望 `ModuleRouter::register_builtin_actions()` 在未设置 `table_config` 时返回错误而非 panic，以便我能在编译期或运行期得到明确的错误提示。

#### 验收标准

1. WHEN `ModuleRouter::register_builtin_actions()` 在未设置 `table_config` 的情况下被调用，THE Router SHALL 返回 `Err(BaseError::TableConfigNotSet)` 而非 panic。
2. THE Router SHALL 将 `register_builtin_actions` 的返回类型从 `Self` 改为 `Result<Self, BaseError>`。
3. WHEN `register_builtin_actions` 成功执行，THE Router SHALL 注册 add、put、del、get、select、table 六个内置 Action 并返回 `Ok(Self)`。

---

### 需求 3：统一 `GlobalTools` 的全局访问模式

**用户故事：** 作为框架使用者，我希望 `GlobalTools` 提供类似 `GlobalDatabase` 的全局单例初始化和访问接口，以便在 Action 执行时无需手动传递 `Arc<GlobalTools>`。

#### 验收标准

1. THE Global_Tools SHALL 提供 `GlobalTools::init(token_manager: TokenManager) -> Result<(), BaseError>` 静态方法，使用 `OnceLock` 初始化全局单例。
2. THE Global_Tools SHALL 提供 `GlobalTools::get() -> Result<&'static GlobalTools, BaseError>` 静态方法，返回全局实例引用。
3. IF `GlobalTools::init` 被重复调用，THEN THE Global_Tools SHALL 返回 `BaseError::ConfigError("GlobalTools 已初始化".to_string())`。
4. IF `GlobalTools::get` 在初始化前被调用，THEN THE Global_Tools SHALL 返回 `BaseError::ConfigError("GlobalTools 未初始化".to_string())`。
5. WHERE 全局单例已初始化，THE Router SHALL 在 `ModuleRouter::dispatch` 中能够通过 `GlobalTools::get()` 自动获取工具，无需外部注入。

---

### 需求 4：修复 `DatabaseInitializer` 的 SQL 注入风险

**用户故事：** 作为框架使用者，我希望 `DatabaseInitializer` 在记录迁移版本时使用参数化查询，以避免 SQL 注入风险。

#### 验收标准

1. THE Database_Layer SHALL 在 `record_migration`、`is_migration_executed`、`run_migrations_in_tx` 中使用 `yang_db` 的参数化查询接口（`query_with_params` 或等效方法）替代字符串拼接 SQL。
2. IF `yang_db` 不支持参数化查询，THEN THE Database_Layer SHALL 对 `module_name` 和 `version` 参数进行转义处理，并在代码注释中标注此为临时方案。
3. THE Database_Layer SHALL 确保迁移记录的插入操作在事务模式下与迁移 SQL 在同一事务中执行。

---

### 需求 5：统一 `GlobalRedis` 的参数类型

**用户故事：** 作为框架使用者，我希望 `GlobalRedis` 的所有方法参数类型保持一致，以便调用时不需要手动构造 `Vec<String>`。

#### 验收标准

1. THE Database_Layer SHALL 将 `GlobalRedis::del`、`GlobalRedis::exists`、`GlobalRedis::lpush`、`GlobalRedis::rpush`、`GlobalRedis::sadd`、`GlobalRedis::srem`、`GlobalRedis::zadd`、`GlobalRedis::hdel` 的批量参数类型统一为接受 `impl IntoIterator<Item = impl Into<String>>` 或保持 `&[impl AsRef<str>]`。
2. THE Database_Layer SHALL 确保单值操作方法（`set`、`get`、`hset`、`hget` 等）继续接受 `impl Into<String>` 参数。
3. WHEN 调用 `GlobalRedis::del(&["key1", "key2"])` 时，THE Database_Layer SHALL 正确执行删除操作，无需调用方构造 `Vec<String>`。

---

### 需求 6：修复 `ModuleRouter` 的 API 命名一致性

**用户故事：** 作为框架使用者，我希望 `ModuleRouter` 的构建方法命名与文档示例一致，以便按照文档示例直接使用。

#### 验收标准

1. THE Router SHALL 将 `ModuleRouter::with_table_config` 方法保留，同时添加 `table_config` 作为别名方法（或将 `with_table_config` 重命名为 `table_config`），与文档示例保持一致。
2. THE Router SHALL 确保 `ModuleRouter` 的所有构建方法（`new`、`table_config`/`with_table_config`、`default_permissions`、`register_action`、`register_builtin_actions`）均支持链式调用。
3. THE Router SHALL 在 `mod.rs` 的文档注释示例中使用与实际 API 一致的方法名。

---

### 需求 7：为 `PluginManager` 实现 JSON Schema 配置验证

**用户故事：** 作为插件开发者，我希望 `PluginManager::load_config` 能够真正验证插件配置是否符合插件定义的 JSON Schema，以便在配置错误时得到明确的错误提示。

#### 验收标准

1. THE Plugin_System SHALL 使用 `jsonschema` crate（或等效方案）实现 `PluginManager::validate_config` 方法，对配置 JSON 进行 Schema 验证。
2. WHEN 插件配置不符合 Schema，THE Plugin_System SHALL 返回 `BaseError::PluginConfigInvalid(plugin_name, validation_error_message)`。
3. WHEN 插件未定义 `config_schema`（返回 `None`），THE Plugin_System SHALL 跳过验证直接存储配置。
4. THE Plugin_System SHALL 在 `Cargo.toml` 中添加 `jsonschema` 依赖，并在注释中说明用途。

---

### 需求 8：优化 `ActionContext` 的参数访问链路

**用户故事：** 作为 Action 开发者，我希望 `ActionContext` 提供更丰富的参数访问方法，以便在 Action 中能够方便地获取路径参数、查询参数，并支持带默认值的参数获取。

#### 验收标准

1. THE Action_System SHALL 为 `ActionContext` 添加 `path_param<T: DeserializeOwned>(&self, key: &str) -> Result<T, BaseError>` 方法，从 `request.path_params` 中获取并反序列化路径参数。
2. THE Action_System SHALL 为 `ActionContext` 添加 `query_param<T: FromStr>(&self, key: &str) -> Result<T, BaseError>` 方法，从 `request.query` 中获取并解析查询参数。
3. THE Action_System SHALL 为 `ActionContext` 添加 `param_or<T: DeserializeOwned>(&self, key: &str, default: T) -> T` 方法，在参数不存在时返回默认值。
4. WHEN `path_param` 或 `query_param` 的目标键不存在，THE Action_System SHALL 返回 `BaseError::ParamMissing(key.to_string())`。
5. WHEN `path_param` 或 `query_param` 的值无法转换为目标类型，THE Action_System SHALL 返回 `BaseError::ParamInvalid(key.to_string(), reason)`。

---

### 需求 9：为 `ModuleRouter` 添加多路由聚合能力

**用户故事：** 作为框架使用者，我希望能够将多个 `ModuleRouter` 聚合到一个顶层路由器中，通过模块名前缀分发请求，以便构建多模块的后端服务。

#### 验收标准

1. THE Router SHALL 提供 `AppRouter` 结构体，支持通过 `register_module(router: ModuleRouter)` 方法注册多个模块路由器。
2. WHEN `AppRouter::dispatch(module_name, action_name, context)` 被调用，THE Router SHALL 查找对应的 `ModuleRouter` 并将请求转发给它。
3. IF 指定的 `module_name` 不存在，THEN THE Router SHALL 返回 `BaseError::ActionNotFound(format!("模块不存在: {}", module_name))`。
4. THE Router SHALL 为 `AppRouter` 提供 `module_names() -> Vec<String>` 方法，返回所有已注册的模块名称列表。
5. THE Router SHALL 确保 `AppRouter` 是线程安全的（实现 `Send + Sync`）。

---

### 需求 10：完善错误处理的 `From` 转换链路

**用户故事：** 作为框架使用者，我希望 `BaseError` 能够自动从常见的第三方错误类型转换，以便在使用 `?` 操作符时减少手动 `map_err` 调用。

#### 验收标准

1. THE Yang_Base SHALL 为 `BaseError` 实现 `From<jsonwebtoken::errors::Error>`，将 JWT 错误映射到对应的 `TokenVerifyFailed`、`TokenExpired`、`TokenParseFailed` 等变体。
2. THE Yang_Base SHALL 为 `BaseError` 实现 `From<yang_db::DbError>`，确保覆盖所有 `DbError` 变体（当前实现统一映射为 `DatabaseQueryFailed`，应区分查询失败和执行失败）。
3. WHEN `jsonwebtoken::errors::ErrorKind::ExpiredSignature` 发生，THE Yang_Base SHALL 将其转换为 `BaseError::TokenExpired`。
4. WHEN `jsonwebtoken::errors::ErrorKind::InvalidToken` 或 `InvalidSignature` 发生，THE Yang_Base SHALL 将其转换为 `BaseError::TokenVerifyFailed`。

---

## 第二轮深度扫描追加需求（11-40）

以下需求来自对 `crates/yang-base/src/` 全量源码的深度静态分析，涉及安全性、并发、错误处理、资源管理、性能、API 一致性、模块化、文档与测试质量等多个维度。每条需求都附带具体的代码位置（文件:行号）。

---

### 需求 11：消除 `TokenManager` 中的 `unwrap()` panic 路径

**用户故事：** 作为框架使用者，我希望 `TokenManager` 在系统时钟异常时返回错误而不是 panic，以便在嵌入式或时钟回拨场景下系统仍能可控地降级运行。

#### 验收标准

1. THE Token_Manager SHALL 在 `generate_access_token`（`src/token/manager.rs:209-212`）、`generate_refresh_token`（`src/token/manager.rs:250-253`）和 `is_token_expiring_soon`（`src/token/manager.rs:397-400`）中将 `SystemTime::now().duration_since(UNIX_EPOCH).unwrap()` 改为 `map_err` 返回 `BaseError::TokenGenerateFailed("系统时钟异常".to_string())`。
2. WHEN 系统时钟早于 `UNIX_EPOCH`，THE Token_Manager SHALL 返回错误而非 panic。
3. THE Token_Manager SHALL 提供一个内部辅助函数 `current_unix_timestamp() -> Result<u64, BaseError>`，统一所有时间戳获取路径。

---

### 需求 12：消除 `HttpClient` 与 `GlobalTools` 中 `RwLock::*().unwrap()` 的中毒 panic

**用户故事：** 作为框架使用者，我希望 `HttpClient` 和 `GlobalTools` 在锁中毒（poisoned）时仍能优雅处理，以便单个 panic 不会扩散导致整个进程崩溃。

#### 验收标准

1. THE Http_Client SHALL 在 `set_default_token`（`src/http/client.rs:135`）和 `get_default_token`（`src/http/client.rs:141`）中通过 `lock().unwrap_or_else(|p| p.into_inner())` 处理中毒锁，或将 `Arc<RwLock<Option<String>>>` 改为基于 `arc-swap` 或 `OnceLock` 的无锁结构。
2. THE Global_Tools SHALL 在 `register_tool`（`src/action/context.rs:252`）和 `get_tool`（`src/action/context.rs:276`）中以同样的策略处理 `RwLock` 中毒。
3. THE Yang_Base SHALL 不在生产代码路径中使用 `std::sync::RwLock::*().unwrap()` 这种会 panic 的写法。

---

### 需求 13：移除重复的 `select` 实现与定义冲突

**用户故事：** 作为框架维护者，我希望 `TableQuery::select` 和 `build_select_sql` 仅有一份实现，以避免编译错误或维护时改一处忘改另一处导致行为不一致。

#### 验收标准

1. THE Table_System SHALL 删除 `src/table/table_query_select.rs` 整文件（与 `src/table/table_query.rs` 的 `select<T>`、`build_select_sql`、`SqlParam` 完全重复定义，在被同时引入时会触发 `E0119` 重复实现错误）。
2. THE Table_System SHALL 在 `src/table/mod.rs` 中确认不再 `mod table_query_select;`（当前 `mod.rs:40` 已未声明，但磁盘上的孤儿文件 `table_query_select.rs:217` 定义了私有 `enum SqlParam` 与 `table_query.rs:1632` 的 `pub(crate) enum SqlParam` 概念冲突）。
3. THE Table_System SHALL 抽取 9 处重复的 `WhereCondition` 到 SQL 拼接逻辑（`build_count_sql`、`build_select_sql`、`build_update_sql_impl`、`build_delete_sql_impl`），合并为单个 `append_where_clause(&mut sql, &mut params)` 辅助方法。

---

### 需求 14：修复 `TableQuery` SQL 构建中字段名注入风险

**用户故事：** 作为框架使用者，我希望 `TableQuery` 在拼接字段名（用于 `SELECT field`、`WHERE field = ?`、`ORDER BY field`、`UPDATE SET field = ?`）时对字段名进行白名单校验或反引号转义，以防止恶意字段名注入 SQL。

#### 验收标准

1. THE Table_System SHALL 在 `build_select_sql`（`src/table/table_query.rs:812`）、`build_count_sql`（`src/table/table_query.rs:632`）、`build_update_sql_impl`（`src/table/table_query.rs:1245`）、`build_delete_sql_impl`（`src/table/table_query.rs:1544`）和 `build_insert_sql`（`src/table/table_query.rs:1078`）中，对所有字段名通过 `quote_identifier(field) -> String`（添加反引号并转义内部反引号）后再拼接进 SQL。
2. WHEN 字段名包含反引号、分号、`--`、空格等非合法标识符字符，THE Table_System SHALL 返回 `BaseError::FieldNotFound` 或 `BaseError::ParamInvalid`。
3. THE Table_System SHALL 复用 `TableConfig::validate_field` 校验所有 WHERE/ORDER BY 字段名是否在表配置中定义。
4. WHILE `WhereCondition::Like` 的 `pattern` 来自外部输入，THE Table_System SHALL 在用户上下文未提供 `escape_like` 参数时不自动添加 `%`，由调用方负责通配符。

---

### 需求 15：修复整数到 `usize` 的有损转换

**用户故事：** 作为框架使用者，我希望分页参数在用户传入负数或超大值时返回明确错误，而不是因 `as usize` 静默截断或溢出。

#### 验收标准

1. THE Action_System SHALL 在 `SelectAction::execute`（`src/action/builtin/select.rs:67-69`）中将 `param_optional::<i64>("page").unwrap_or(1) as usize` 改为使用 `try_into()` 并对负数、`> u32::MAX` 的值返回 `BaseError::ParamInvalid`。
2. THE Action_System SHALL 限制 `page >= 1` 且 `1 <= page_size <= 100`，超出范围返回 `BaseError::ParamInvalid("page_size", "必须在 1 到 100 之间")`。
3. THE Table_System SHALL 在 `count() -> Result<usize, BaseError>`（`src/table/table_query.rs:619`）的 `count as usize` 转换前检查 `count >= 0`，对负数返回 `BaseError::DatabaseQueryFailed`。

---

### 需求 16：增强 JWT 验证防御算法降级与字段过滤

**用户故事：** 作为安全敏感场景的使用者，我希望 `TokenManager::verify_token` 严格限制可接受算法集合，避免攻击者通过 `alg: none` 或换算法的方式绕过验证。

#### 验收标准

1. THE Token_Manager SHALL 在 `verify_token`（`src/token/manager.rs:325`）中将 `Validation::new(self.algorithm)` 改为显式仅允许构造时配置的单一算法（通过 `validation.algorithms = vec![self.algorithm]`）。
2. THE Token_Manager SHALL 拒绝 `alg = none` 的 Token，即使 jsonwebtoken 库默认禁止，也要在构造 `Validation` 时显式设置 `validation.required_spec_claims` 包含 `exp`、`iss`、`aud`。
3. THE Token_Manager SHALL 为 `parse_token_unsafe`（`src/token/manager.rs:361`）补充 `# Safety` 章节，明确警告该方法不应用于鉴权决策。
4. THE Token_Manager SHALL 在 `Debug` 实现（`src/token/manager.rs:489`）中确保不打印 `encoding_key` 和 `decoding_key` 内容（当前已正确，但需添加测试断言验证 Debug 输出不含 `encoding_key`/`decoding_key` 字段）。

---

### 需求 17：消除 `ModuleRouter` 与 `register_builtin_actions` 的重复字符串

**用户故事：** 作为框架维护者，我希望内置 Action 名称、模块名等重复字符串以常量形式集中定义，避免散落在多处导致拼写不一致。

#### 验收标准

1. THE Router SHALL 在 `src/router/module_router.rs` 中定义 `pub const BUILTIN_ACTION_NAMES: &[&str] = &["add", "put", "del", "get", "select", "table"];` 替代 `register_builtin_actions`（`src/router/module_router.rs:213-237`）中的 6 处字符串字面量。
2. THE Action_System SHALL 在每个内置 Action 实现中通过该常量与 `Action::name()` 返回值保持一致。
3. THE Router SHALL 通过常量驱动注册循环，避免 6 个 `self.actions.insert("<name>".to_string(), Box::new(...))` 重复模板代码。

---

### 需求 18：完善 `ApiResponse::success` 的序列化错误处理

**用户故事：** 作为框架使用者，我希望 `ApiResponse::success` 在 `data` 序列化失败时返回错误响应，而不是静默把数据替换成 `Null`。

#### 验收标准

1. THE Action_System SHALL 将 `ApiResponse::success`（`src/action/response.rs:122`）的签名改为 `pub fn success<T: Serialize>(data: T, message: impl Into<String>) -> Result<Self, BaseError>`，序列化失败时返回 `BaseError::JsonSerializeFailed`。
2. WHERE 调用方需要无错误版本的便捷构造函数，THE Action_System SHALL 同时提供 `success_value(data: serde_json::Value, message: impl Into<String>) -> Self` 接受已序列化好的 `Value`。
3. THE Action_System SHALL 移除 `serde_json::to_value(data).unwrap_or(serde_json::Value::Null)`（`src/action/response.rs:127`）中静默吞掉错误的逻辑。

---

### 需求 19：补充 `PluginManager` 拓扑排序的循环依赖检测

**用户故事：** 作为插件开发者，我希望 `PluginManager` 在检测到循环依赖时返回 `PluginCircularDependency` 错误，而不是默默地按 `usize::MAX` 排序产生不确定行为。

#### 验收标准

1. THE Plugin_System SHALL 在 `PluginRegistry::compute_topological_sort`（`src/plugin/mod.rs:818-870`）中对比 `sorted_names.len()` 与 `plugins.len()`，当 `sorted_names.len() < plugins.len()` 时返回 `BaseError::PluginCircularDependency`。
2. THE Plugin_System SHALL 将 `compute_topological_sort` 返回类型改为 `Result<Vec<Arc<dyn Plugin>>, BaseError>`，并将 `PluginRegistry::new` 与 `PluginManagerBuilder::build` 改为返回 `Result`。
3. THE Plugin_System SHALL 在错误信息中包含未能排序的插件名集合（即依赖环上的节点）。
4. THE Plugin_System SHALL 同时修正 `PluginManager::topological_sort`（`src/plugin/mod.rs:391-433`），逻辑保持一致。

---

### 需求 20：检测插件依赖缺失

**用户故事：** 作为插件开发者，我希望注册的插件在依赖未注册时立即得到 `PluginDependencyMissing` 错误，而不是在数据库初始化时才失败。

#### 验收标准

1. THE Plugin_System SHALL 在 `PluginManagerBuilder::build()`（`src/plugin/mod.rs:611`）中遍历每个插件的 `dependencies()`，检查依赖是否都在 `plugins` HashMap 中。
2. IF 插件 `X` 声明依赖 `Y` 但 `Y` 未注册，THEN THE Plugin_System SHALL 返回 `BaseError::PluginDependencyMissing("X".to_string(), "Y".to_string())`。
3. THE Plugin_System SHALL 将 `PluginManagerBuilder::build()` 签名改为 `pub fn build(self) -> Result<PluginRegistry, BaseError>`。

---

### 需求 21：替换示例中的 `panic!` 为 `assert!(matches!(...))`

**用户故事：** 作为框架使用者，我希望文档示例和测试断言模式不通过 `panic!` 表达失败，而采用 `assert!(matches!(...))` 检查具体错误变体，以提高可读性与可移植性。

#### 验收标准

1. THE Yang_Base SHALL 将 `field_type.rs` 测试中的三处 `panic!("期望 ... 错误")`（`src/table/field_type.rs:673, 893, 946`）改为 `assert!(matches!(result, Err(BaseError::StringTooLong(_, _, _))))` 风格。
2. THE Yang_Base SHALL 将 `token::__tests__::manager_test.rs:173, 275, 398` 中三处 `panic!("期望 ... 错误")` 同样改造。
3. THE Yang_Base SHALL 在 `tests/` 目录下的集成测试中将 `.unwrap()` 替换为 `.expect("<上下文说明>")`，以便失败时输出可定位的错误信息（涉及 `tests/table_query_paginate_test.rs`、`tests/table_query_crud_test.rs`，至少 40 处）。

---

### 需求 22：移除 `table_query_select.rs` 孤儿模块文件

**用户故事：** 作为框架维护者，我希望仓库中不存在已废弃的孤儿源文件，以避免误读和未来意外被 `mod` 引入。

#### 验收标准

1. THE Table_System SHALL 物理删除 `crates/yang-base/src/table/table_query_select.rs` 文件（其内容已被 `table_query.rs` 完全覆盖，且 `table/mod.rs` 未声明该模块）。
2. THE Yang_Base SHALL 同步更新 `docs/reference/project_structure.md:357` 中关于 `table_query_select.rs` 的引用。
3. THE Yang_Base SHALL 增加一条 cargo lint 检查（或 CI 步骤）：使用 `cargo udeps` 与 `cargo +nightly rustc -- -W dead_code -W unused_imports`，禁止再出现类似孤儿文件。

---

### 需求 23：修复 `with_table_config` 与 `table_config` 命名混淆

**用户故事：** 作为框架使用者，我希望 `ModuleRouter` 的链式构建器方法 `with_table_config(...)`（`src/router/module_router.rs:130`）与同名 getter `table_config(&self) -> Option<Arc<TableConfig>>`（`src/router/module_router.rs:340`）不会因命名冲突造成调用歧义。

#### 验收标准

1. THE Router SHALL 将 getter 方法重命名为 `get_table_config(&self) -> Option<Arc<TableConfig>>` 或 `current_table_config(&self) -> Option<&Arc<TableConfig>>`。
2. THE Router SHALL 保留 `with_table_config` 作为 builder setter 的唯一形式，并删除文档示例中错误的 `.table_config(...)` setter 用法（`src/router/mod.rs:18`、`src/router/module_router.rs:25`）。
3. THE Router SHALL 通过 `cargo doc` 验证文档示例可通过编译。

---

### 需求 24：补全 `ActionContext` 的请求上下文克隆开销

**用户故事：** 作为 Action 开发者，我希望 `ActionContext::table_query()` 不在每次调用时克隆整个 `Arc<TableConfig>`，并避免 `user_roles.clone()` 产生不必要的堆分配。

#### 验收标准

1. THE Action_System SHALL 将 `ActionContext::table_query()`（`src/action/context.rs:467`）改为返回 `Result<TableQuery, BaseError>`，但 `TableQuery::new` 接收 `user_roles: Vec<String>` 改为 `user_roles: &[String]` 并在内部用 `Cow::Borrowed`/`Cow::Owned` 表达。
2. WHERE 用户角色不会被外部修改，THE Table_System SHALL 在 `TableQuery` 内将 `user_roles` 改为 `Arc<[String]>`，避免每个 `with_table_config` 都触发深 clone。
3. THE Action_System SHALL 让 `ActionContext::user_roles()`（`src/action/context.rs:493`）返回 `&[String]` 而非 `Vec<String>`。

---

### 需求 25：缩短重复的 SQL 拼接片段

**用户故事：** 作为框架维护者，我希望 9 处几乎完全一致的 `WhereCondition` 转 SQL 代码集中到一个函数中，以减少未来对 SQL 注入或新增条件类型时的修改成本。

#### 验收标准

1. THE Table_System SHALL 在 `src/table/table_query.rs` 中提取私有方法 `fn append_where_to_sql(&self, sql: &mut String, params: &mut Vec<SqlParam>) -> Result<(), BaseError>`，集中处理 `WhereCondition::Eq/In/Like/Gt/Gte/Lt/Lte/IsNull/IsNotNull` 的拼接。
2. THE Table_System SHALL 让 `build_count_sql`、`build_select_sql`、`build_update_sql_impl`、`build_delete_sql_impl` 全部调用该方法，单次重构移除约 80 行重复代码。
3. WHEN 新增 `Between`、`Not` 等条件类型，THE Table_System SHALL 仅需修改这一个方法。

---

### 需求 26：补全 HTTP 客户端连接复用与代理配置

**用户故事：** 作为框架使用者，我希望 `HttpClient::new` 接受连接池、代理、TLS、压缩等配置，避免重复创建 `reqwest::Client` 导致连接池失效。

#### 验收标准

1. THE Http_Client SHALL 将 `HttpClient::new(timeout_secs: u64)`（`src/http/client.rs:62`）替换为 `HttpClient::with_config(config: HttpClientConfig)`，其中 `HttpClientConfig` 至少包含 `timeout_secs`、`pool_max_idle_per_host`、`pool_idle_timeout_secs`、`user_agent`、`accept_invalid_certs`、`proxy_url`。
2. THE Http_Client SHALL 保留 `HttpClient::new(timeout_secs: u64)` 为便捷构造器，内部委托 `with_config(HttpClientConfig::default().timeout(...))`。
3. THE Http_Client SHALL 在 `RequestBuilder::header`（`src/http/request.rs:118`）中将无效 header 名称/值的静默忽略改为返回 `Result<Self, BaseError>` 或在文档中明确说明该静默行为。

---

### 需求 27：消除 `RequestBuilder::header` 静默吞错

**用户故事：** 作为 HTTP 客户端使用者，我希望传入非法 header 时立即得到错误，而不是返回看似正常但缺失了关键 header 的 builder。

#### 验收标准

1. THE Http_Client SHALL 在 `RequestBuilder::header`（`src/http/request.rs:118-126`）和 `headers`（`src/http/request.rs:144-156`）中检测 `HeaderName::from_bytes` 与 `HeaderValue::from_str` 的错误，将其累积到 builder 内部的 `header_errors: Vec<BaseError>`，并在 `send().await` 时一并返回。
2. WHERE 用户调用 `bearer_token`、`content_type`、`user_agent` 等便捷方法，THE Http_Client SHALL 同样累积错误。
3. WHEN `send().await` 时发现 builder 中累积了任何 header 错误，THE Http_Client SHALL 返回 `BaseError::HttpRequestFailed("无效请求头: ...".to_string())`。

---

### 需求 28：HTTP `form` 实现对值进行 URL 编码

**用户故事：** 作为框架使用者，我希望 `RequestBuilder::form` 自动对 key/value 进行 URL 编码，而不是直接 `format!("{}={}", k, v)` 拼接（容易在含 `&`、`=`、空格、中文时产生错误请求）。

#### 验收标准

1. THE Http_Client SHALL 修改 `RequestBuilder::form`（`src/http/request.rs:308-321`），将 `format!("{}={}", k, v)` 改为使用 `reqwest::RequestBuilder::form` 或 `form_urlencoded::Serializer`，确保所有特殊字符正确编码。
2. WHEN 表单值包含 `&`、`=`、空格、`%`、UTF-8 字符，THE Http_Client SHALL 生成符合 RFC 3986 的 `application/x-www-form-urlencoded` 请求体。
3. THE Http_Client SHALL 添加单元测试覆盖 ASCII、URL-unsafe、UTF-8 三类输入。

---

### 需求 29：补全 `ActionContext::param_optional` 对类型不匹配的可观测性

**用户故事：** 作为 Action 开发者，我希望 `param_optional` 在反序列化失败（区别于参数缺失）时通过日志告警，避免静默吞掉类型不匹配错误而难以调试。

#### 验收标准

1. THE Action_System SHALL 在 `ActionContext::param_optional`（`src/action/context.rs:444-453`）的 `from_value(...).ok()` 分支中添加 `log::warn!("参数 {} 反序列化失败，已视为不存在: {}", key, err)`。
2. THE Action_System SHALL 同时提供 `param_optional_strict<T>(&self, key: &str) -> Result<Option<T>, BaseError>`，将存在但类型不匹配的情况返回 `Err(BaseError::ParamInvalid)`，仅当字段不存在时返回 `Ok(None)`。
3. THE Action_System SHALL 在文档中明确说明 `param_optional` 与 `param_optional_strict` 的语义区别。

---

### 需求 30：确保 `RequestBuilder::send` 复用客户端而非每次新建

**用户故事：** 作为框架使用者，我希望 `RequestBuilder` 持有的 `Client` 是 `Arc<Client>` 或来自共享的 `HttpClient`，确保连接池在多个请求间复用而不是每次都触发 TLS 握手。

#### 验收标准

1. THE Http_Client SHALL 确认 `RequestBuilder::client: Client`（`src/http/request.rs:68`）通过 `Client::clone()`（reqwest 内部 `Arc`）共享同一个底层连接池，并在文档中明确说明。
2. THE Http_Client SHALL 在 `HttpClient::request`（`src/http/client.rs:179`）的实现注释中标明 `self.client.clone()` 是廉价的（仅 `Arc::clone`）。
3. THE Http_Client SHALL 增加集成测试验证：连续 100 次 `client.get(url).send().await`，总体握手次数应明显少于 100（通过 wiremock 或本地 HTTP server 计数）。

---

### 需求 31：测试中外部环境依赖的可重入处理

**用户故事：** 作为框架维护者，我希望 `tests/` 中需要 Docker MySQL 的集成测试在单次测试运行内复用同一个容器，而非每个测试函数都启动一次 testcontainer。

#### 验收标准

1. THE Yang_Base SHALL 在 `tests/table_query_paginate_test.rs` 与 `tests/table_query_crud_test.rs` 中将 `setup_mysql().await` 改为通过 `tokio::sync::OnceCell` 共享一个 testcontainer 实例。
2. WHEN 多个测试串行运行（`--test-threads=1`），THE Yang_Base SHALL 仅启动一个 MySQL 容器，避免单测运行时间从 7 个测试乘以 30 秒降低到 1 次启动加 7 次约 3 秒查询。
3. THE Yang_Base SHALL 在 `tests/README.md` 中说明 `--test-threads=1` 的必要性以及容器复用方式。

---

### 需求 32：严格化全局单例的初始化错误信息

**用户故事：** 作为框架使用者，我希望 `GlobalDatabase`、`GlobalRedis`、`HttpClient::global` 在重复初始化、未初始化时返回结构化错误而非字符串拼接。

#### 验收标准

1. THE Database_Layer SHALL 将 `HttpClient::init_global` 重复初始化错误（`src/http/client.rs:96`）从 `BaseError::HttpClientCreateFailed("全局客户端已初始化".to_string())` 改为新增 `BaseError::HttpClientAlreadyInitialized` 变体，与 `DatabaseAlreadyInitialized`、`RedisAlreadyInitialized` 风格一致。
2. THE Database_Layer SHALL 同样将 `HttpClient::global()` 未初始化错误（`src/http/client.rs:121`）从 `HttpClientCreateFailed("全局客户端未初始化")` 改为新增 `HttpClientNotInitialized` 变体。
3. THE Yang_Base SHALL 为新增的两个错误变体补充 `code()` 错误码（300005、300006）和单元测试。

---

### 需求 33：禁止生产环境硬编码凭证

**用户故事：** 作为安全审计人员，我希望仓库中的 `.mcp.json` 不再包含明文密码 `"111111"`，所有凭证通过环境变量或专用密钥管理服务注入。

#### 验收标准

1. THE Yang_Base SHALL 将 `.mcp.json` 中硬编码的 MySQL 密码替换为环境变量占位符（如 MYSQL_TEST_PASSWORD），并在 `.gitignore` 中将真实凭证文件 `.mcp.local.json` 加入忽略列表。
2. THE Yang_Base SHALL 在 `tests/README.md` 与 `crates/yang-base/AGENTS.md` 中说明本地开发的密码注入流程。
3. WHEN 测试代码需要默认密码，THE Yang_Base SHALL 通过 `std::env::var("MYSQL_TEST_PASSWORD").unwrap_or_else(|_| "111111".to_string())` 提供回退，但不在仓库内提交真实密码。

---

### 需求 34：补全 Cargo feature gate 与依赖最小化

**用户故事：** 作为下游用户，我希望仅启用我需要的功能（如 MySQL 但不需要 JWT），以减小二进制体积和编译时间。

#### 验收标准

1. THE Yang_Base SHALL 在 `Cargo.toml` 中将 `jsonwebtoken`、`reqwest`、`sqlx`、`regex` 等依赖改为 optional，并通过 feature gate（`token`、`http`、`mysql`、`validator`）启用。
2. THE Yang_Base SHALL 默认 features 为 `["token", "http", "mysql", "validator"]`，保持现有行为不变。
3. THE Yang_Base SHALL 验证 `cargo build --no-default-features` 能通过编译（仅保留 `plugin`、`error`、`action` 核心模块）。
4. THE Yang_Base SHALL 在每条依赖后增加注释说明用途（当前 `Cargo.toml:13-43` 已有但不完整）。

---

### 需求 35：补充 `BaseError` 错误链 `source()` 实现

**用户故事：** 作为框架使用者，我希望通过 `error.source()` 链能追溯到原始底层错误（如 `sqlx::Error`、`reqwest::Error`），而不是仅得到字符串化后的扁平消息。

#### 验收标准

1. THE Yang_Base SHALL 改造 `BaseError`（`src/error/mod.rs`）变体，在 `DatabaseQueryFailed`、`DatabaseExecuteFailed`、`HttpRequestFailed` 等持有底层错误的变体中通过 `#[source]` 持有原 `Error` 而不是 `String`。
2. THE Yang_Base SHALL 在 `From<sqlx::Error>`、`From<reqwest::Error>`、`From<jsonwebtoken::errors::Error>` 实现中保留原始错误，而非 `e.to_string()`。
3. WHEN 上层调用 `error.source()`，THE Yang_Base SHALL 返回 `Some(&dyn Error)` 指向底层错误，便于使用 `anyhow::Error::chain` 输出完整链路。
4. THE Yang_Base SHALL 增加单元测试验证错误链可遍历至少 2 层（`BaseError -> sqlx::Error -> sqlx::error::DatabaseError`）。

---

### 需求 36：移除测试中 `unwrap()` 链导致的失败信息缺失

**用户故事：** 作为测试维护者，我希望集成测试在失败时能定位到具体的失败点，而非只看到 `panicked at 'called Result::unwrap() on Err'`。

#### 验收标准

1. THE Yang_Base SHALL 将 `tests/table_query_paginate_test.rs` 中至少 30 处 `.unwrap()` 替换为 `.expect("<具体上下文，如：分页查询第 1 页失败>")`。
2. THE Yang_Base SHALL 同样改造 `tests/table_query_crud_test.rs`、`tests/database_initializer_test.rs`、`tests/database_test.rs` 等集成测试。
3. THE Yang_Base SHALL 在 `crates/yang-base/AGENTS.md` 中加入约定：测试代码可使用 `.expect(...)`，但禁止使用裸 `.unwrap()`。

---

### 需求 37：为公开类型补全 `# Errors` 与 `# Panics` 章节

**用户故事：** 作为下游开发者，我希望每个返回 `Result` 的公开方法都明确列出可能的错误变体，每个可能 panic 的方法都标注 `# Panics`。

#### 验收标准

1. THE Yang_Base SHALL 为以下方法补充或完善 `# Errors` 章节：`TokenManager::*`、`HttpClient::*`、`GlobalDatabase::*`、`GlobalRedis::*`、`PluginManager::*`、`ModuleRouter::dispatch`。
2. THE Yang_Base SHALL 为 `ModuleRouter::register_builtin_actions`（`src/router/module_router.rs:213`）的 `# Panics` 章节（`src/router/module_router.rs:198-200`）移除（因需求 2 改为 `Result`，不再 panic）。
3. THE Yang_Base SHALL 通过 `cargo +nightly rustdoc -- -D missing_docs` 强制公开 API 100% 拥有文档（当前已有但不完整）。
4. THE Yang_Base SHALL 提供至少一个公开类型（如 `TableQuery`）的 `# Examples` 章节是 doctest 可执行的（去掉 `,ignore` 标记）。

---

### 需求 38：Edition 2024 与 Workspace 依赖统一

**用户故事：** 作为 workspace 维护者，我希望所有 crate 使用一致的 Rust edition 与共享依赖表，避免版本漂移和重复声明。

#### 验收标准

1. THE Yang_Base SHALL 将 workspace 根 `Cargo.toml` 中改为使用 `[workspace.dependencies]` 声明共享依赖（`tokio`、`serde`、`serde_json`、`thiserror`、`log`、`chrono`、`uuid`），并在各 crate 的 `Cargo.toml` 中通过 `tokio.workspace = true` 引用。
2. THE Yang_Base SHALL 确认 `crates/yang-db/Cargo.toml` 与 `crates/yang-pcg/Cargo.toml` 的 `edition = "2024"` 修正为 `edition = "2021"`（AGENTS.md 已标注此为已知 bug）。
3. THE Yang_Base SHALL 在 workspace 根 `Cargo.toml` 增加 `[workspace.lints]`，统一 `unused_must_use`、`missing_docs`、`unsafe_code` 等 lint 等级。

---

### 需求 39：消除 `Validator::Email/Phone/Url` 过弱的格式校验

**用户故事：** 作为框架使用者，我希望 `Validator::Email` 真正校验邮箱格式而非仅检查 `@`，`Phone` 真正校验手机号格式而非仅检查数字与连字符。

#### 验收标准

1. THE Table_System SHALL 在 `Validator::Email`（`src/table/validator.rs:445`）中使用预编译的邮箱正则表达式（如 `[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}`，需以 `^...$` 锚定）替代仅 `s.contains('@')` 的弱校验。
2. THE Table_System SHALL 在 `Validator::Phone`（`src/table/validator.rs:466`）中根据国家/地区配置正则，并默认使用 E.164 格式（`\+?[1-9]\d{1,14}`，需以 `^...$` 锚定）。
3. THE Table_System SHALL 使用 `once_cell::sync::Lazy` 或 `std::sync::OnceLock` 缓存编译后的 `Regex`，避免每次 validate 都重新编译（当前 `Validator::Regex` 在 `validator.rs:506` 每次调用都 `Regex::new`，性能损耗）。
4. WHERE 用户需要兼容旧逻辑，THE Table_System SHALL 提供 `Validator::EmailLoose`、`Validator::PhoneLoose` 弱校验变体。

---

### 需求 40：补全 `GlobalRedis` 缺失的核心 API

**用户故事：** 作为 Redis 使用者，我希望 `GlobalRedis` 暴露 `incr`、`decr`、`hincrby`、`zrange_with_scores`、`mget`、`mset`、`pipeline`、`script` 等核心 API，与 yang-db 保持完整对应关系。

#### 验收标准

1. THE Database_Layer SHALL 在 `src/database/global_redis.rs` 中补充至少以下 API：`incr`、`decr`、`incrby`、`hincrby`、`mget`、`mset`、`zrange_with_scores`、`zrevrange`、`zincrby`。
2. THE Database_Layer SHALL 暴露 `pipeline()` 与 `script()` 入口，返回 yang-db 提供的对应类型，便于用户构建复杂的 Redis 操作。
3. THE Database_Layer SHALL 补充关于序列化字节键（非 UTF-8）的 API（如 `set_bytes(key, value: &[u8])`），覆盖二进制数据写入场景。
4. THE Database_Layer SHALL 为每一个新增方法补充对应的单元测试（mock 或 ignore-by-default 的集成测试）。
