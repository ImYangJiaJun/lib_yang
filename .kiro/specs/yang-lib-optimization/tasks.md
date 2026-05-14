# 任务列表：yang-db 与 yang-base 性能优化和代码简化

## 任务概览

本任务列表按照依赖关系排序，基础任务优先，上层任务在后。任务 4 必须在任务 5 之前完成（DbError 变体是操作符验证的前置条件）。

---

- [x] 1. SqlGenerator 预分配优化
  - 对应需求 6：减少 SQL 字符串构建过程中的重新分配次数
  - [x] 1.1 修改 `SqlGenerator::new()` 方法，将 `sql` 字段初始化从 `String::new()` 改为 `String::with_capacity(256)`
  - [x] 1.2 修改 `SqlGenerator::new()` 方法，将 `params` 字段初始化从 `Vec::new()` 改为 `Vec::with_capacity(8)`
  - [x] 1.3 检查并确认 `clear()` 方法使用 `self.sql.clear()` 和 `self.params.clear()`，保留已分配容量（不使用 `= String::new()` 或 `= Vec::new()`）
  - [x] 1.4 编写属性测试（P6）：验证预分配初始化与默认初始化生成相同的 SQL 和参数列表，容量变化不影响内容正确性

- [x] 2. 批量插入内存分配优化
  - 对应需求 1：消除 `build_insert_batch` 中 O(N) 的中间 `Vec<String>` 分配
  - [x] 2.1 重构 `build_insert_batch` 方法，移除中间 `Vec<String> value_clauses` 收集变量
  - [x] 2.2 改用 `self.sql.push('(')` / `self.sql.push_str(", ")` / `self.sql.push(')')` 直接写入 `self.sql`，替代 `format!("({})", placeholders.join(", "))` 模式
  - [x] 2.3 改用记录间直接追加 `, ` 分隔符的方式，替代最终的 `value_clauses.join(", ")` 调用
  - [x] 2.4 编写属性测试（P1）：验证任意非空数据列表生成的 SQL 中 `(` 数量等于记录数，参数数量等于 `记录数 × 字段数`

- [x] 3. 批量更新内存分配优化
  - 对应需求 2：消除 `build_update_batch` 中 O(M×N) 的中间字符串分配
  - [x] 3.1 重构 `build_update_batch` 方法，移除中间 `Vec<String> set_parts` 和 `Vec<String> when_parts` 收集变量
  - [x] 3.2 改用 `self.sql.push_str` 逐步追加 CASE WHEN 表达式，替代 `format!("WHEN {}=? THEN ?", id_field)` 收集再 join 的模式
  - [x] 3.3 验证重构后批量更新 N 条记录、M 个字段时，中间分配次数降至 O(M) 级别
  - [x] 3.4 编写单元测试：验证重构后生成的 SQL 结构与重构前完全一致

- [x] 4. 新增 UnsupportedOperator 错误变体
  - 对应需求 3 前置：为操作符验证提供专用错误类型
  - [x] 4.1 在 `crates/yang-db/src/error.rs` 的 `DbError` 枚举中新增 `UnsupportedOperator(String)` 变体
  - [x] 4.2 为新变体添加 `#[error("不支持的操作符: {0}")]` 属性，确保错误消息格式正确
  - [x] 4.3 在 `crates/yang-db/src/lib.rs` 中确认 `DbError` 已正确导出（无需额外修改）
  - [x] 4.4 编写单元测试：验证 `UnsupportedOperator` 变体的错误消息格式符合预期

- [x] 5. 操作符验证错误处理
  - 对应需求 3：消除运行时 panic，改为返回 `Result` 类型
  - 依赖任务 4 完成
  - [x] 5.1 修改 `where_and` 方法返回类型从 `Self` 改为 `Result<Self, DbError>`，遇到不支持操作符时返回 `Err(DbError::UnsupportedOperator(op.to_string()))`
  - [x] 5.2 修改 `where_or` 方法返回类型从 `Self` 改为 `Result<Self, DbError>`，同上处理
  - [x] 5.3 修改 `having_cond` 方法返回类型从 `Self` 改为 `Result<Self, DbError>`，同上处理
  - [x] 5.4 新增 `where_and_unchecked` 方法，保持原有 panic 行为，确保向后兼容
  - [x] 5.5 新增 `where_or_unchecked` 方法，保持原有 panic 行为，确保向后兼容
  - [x] 5.6 新增 `having_cond_unchecked` 方法，保持原有 panic 行为，确保向后兼容
  - [x] 5.7 编写属性测试（P2）：验证任意不在支持集合 `{=, !=, >, <, >=, <=, like, LIKE}` 中的操作符字符串，`where_and` 必须返回 `Err(DbError::UnsupportedOperator(_))`
  - [x] 5.8 编写单元测试：验证已支持操作符的现有行为不变

- [x] 6. bind_param 宏消除重复
  - 对应需求 4：消除 4 个 bind_param 函数中完全相同的 match 分支重复代码
  - [x] 6.1 在 `crates/yang-db/src/mysql/query_builder.rs` 中定义内部宏 `bind_value_match!`，封装 `SqlValue` 各变体到 `.bind()` 调用的映射逻辑
  - [x] 6.2 重构 `bind_execute_param` 函数，使用 `bind_value_match!` 宏替代手写 match 分支
  - [x] 6.3 重构 `bind_param` 函数，使用 `bind_value_match!` 宏替代手写 match 分支
  - [x] 6.4 重构 `bind_scalar_param` 函数，使用 `bind_value_match!` 宏替代手写 match 分支
  - [x] 6.5 重构 `bind_scalar_param_option` 函数，使用 `bind_value_match!` 宏替代手写 match 分支
  - [x] 6.6 运行 `cargo check` 和 `cargo test --lib -p yang-db` 验证编译通过且行为不变

- [x] 7. condition_to_sql_owned 函数
  - 对应需求 5：提供消费版本的条件转 SQL 函数，避免不必要的 clone 开销
  - [x] 7.1 在 `crates/yang-db/src/mysql/condition.rs` 中新增 `condition_to_sql_owned` 公开函数，签名为 `pub fn condition_to_sql_owned(condition: Condition, params: &mut Vec<SqlValue>) -> String`
  - [x] 7.2 实现函数体：消费传入的 `Condition`，对 `SqlValue::String`、`SqlValue::Bytes`、`SqlValue::Json` 等堆分配类型直接 `push`（无需 clone），对 `Condition::And` / `Condition::Or` 递归调用自身
  - [x] 7.3 确认现有 `condition_to_sql` 借用版本保持不变
  - [x] 7.4 在 `crates/yang-db/src/lib.rs` 中导出 `condition_to_sql_owned`
  - [x] 7.5 编写属性测试（P3）：验证对任意 `Condition`，`condition_to_sql_owned(c.clone(), &mut p1)` 生成的 SQL 字符串与 `condition_to_sql(&c, &mut p2)` 完全相同，且参数列表长度相等

- [x] 8. 批量插入自定义批次大小
  - 对应需求 7：允许调用方根据场景自定义批次大小
  - [x] 8.1 在 `QueryBuilder` 上新增 `insert_batch_with_size` 异步公开方法，签名为 `pub async fn insert_batch_with_size<T: serde::Serialize>(self, data: &[T], batch_size: usize) -> Result<u64, DbError>`
  - [x] 8.2 在方法入口处检查 `batch_size == 0`，若为 0 则返回 `Err(DbError::SerializationError("batch_size 不能为 0".to_string()))`
  - [x] 8.3 实现分批逻辑：使用 `data.chunks(batch_size)` 分批，每批调用内部插入逻辑，累加受影响行数
  - [x] 8.4 确认现有 `insert_batch` 方法保持不变，内部继续使用 `INSERT_BATCH_SIZE`（500）
  - [x] 8.5 编写属性测试（P5）：验证对任意 `n` 条记录和批次大小 `b`（b > 0），执行的批次数等于 `ceil(n / b)`

- [x] 9. PluginManagerBuilder 与 PluginRegistry
  - 对应需求 8、9：分离构建/运行阶段，实现无锁查找和拓扑排序缓存
  - [x] 9.1 在 `crates/yang-base/src/plugin/mod.rs` 中实现 `PluginManagerBuilder` 结构体，包含 `plugins: HashMap<String, Arc<dyn Plugin>>` 和 `configs: HashMap<String, JsonValue>` 字段
  - [x] 9.2 为 `PluginManagerBuilder` 实现 `new()` 构造方法
  - [x] 9.3 为 `PluginManagerBuilder` 实现 `register` 异步方法：检查重名、调用 `on_register()`、插入 HashMap
  - [x] 9.4 为 `PluginManagerBuilder` 实现 `build()` 方法：消费自身，返回 `PluginRegistry`
  - [x] 9.5 在 `crates/yang-base/src/plugin/mod.rs` 中实现 `PluginRegistry` 结构体，包含 `plugins: HashMap<String, Arc<dyn Plugin>>`、`sorted_plugins: Vec<Arc<dyn Plugin>>`、`configs: HashMap<String, JsonValue>` 字段
  - [x] 9.6 为 `PluginRegistry` 实现私有 `new()` 构造方法：接收 plugins 和 configs，调用 `compute_topological_sort` 并缓存结果
  - [x] 9.7 为 `PluginRegistry` 实现 `get(name: &str) -> Option<&Arc<dyn Plugin>>` 方法（无锁 O(1) HashMap 查找）
  - [x] 9.8 为 `PluginRegistry` 实现 `get_all() -> &[Arc<dyn Plugin>]` 方法（直接返回缓存的排序结果引用）
  - [x] 9.9 为 `PluginRegistry` 实现 `shutdown()` 异步方法（逆序关闭所有插件）
  - [x] 9.10 实现私有 `compute_topological_sort` 方法（Kahn 算法，与现有 `PluginManager` 中的拓扑排序逻辑一致）
  - [x] 9.11 确认现有 `PluginManager` 结构体保持不变
  - [x] 9.12 在 `crates/yang-base/src/lib.rs` 中导出 `PluginManagerBuilder` 和 `PluginRegistry`
  - [x] 9.13 编写单元测试（P4）：验证 `PluginRegistry::get(name)` 的结果与构建前注册的插件一一对应，`get_all()` 返回缓存结果无需重新计算

- [x] 10. Permission Cow 优化
  - 对应需求 10：减少静态字符串场景下的堆分配，保持 API 完全兼容
  - [x] 10.1 定位 `Permission` 结构体所在文件（`crates/yang-base/src/` 下）
  - [x] 10.2 在文件顶部添加 `use std::borrow::Cow;` 导入
  - [x] 10.3 将 `Permission` 结构体的 `name` 字段类型从 `String` 改为 `Cow<'static, str>`
  - [x] 10.4 修改 `Permission::new(impl Into<String>)` 方法，将 `name` 存储为 `Cow::Owned(name.into())`，保持方法签名不变
  - [x] 10.5 新增 `Permission::from_static(name: &'static str)` 构造方法，将 `name` 存储为 `Cow::Borrowed(name)`，实现零拷贝
  - [x] 10.6 确认 `Permission::name()` 方法返回类型仍为 `&str`（`&self.name` 自动解引用 `Cow`）
  - [x] 10.7 运行 `cargo check -p yang-base` 验证编译通过，现有 API 调用无破坏性变更
  - [x] 10.8 编写单元测试：验证 `from_static` 创建的 `Permission` 与 `new` 创建的 `Permission` 在 `name()` 返回值上相同
