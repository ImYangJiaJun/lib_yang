# 需求文档：yang-db 与 yang-base 性能优化和代码简化

## 简介

本文档定义了 yang-db 和 yang-base 两个 Rust crate 的性能优化和代码简化需求。优化目标包括：减少不必要的内存分配、消除运行时 panic、消除重复代码、改善插件系统的运行时性能。所有优化必须保持向后兼容（公开 API 不能破坏性变更）。

## 术语表

- **QueryBuilder**：yang-db 中的 MySQL 查询构建器，提供链式 API 构建 SQL 语句
- **SqlGenerator**：QueryBuilder 内部使用的 SQL 生成器，负责将查询条件转换为 SQL 字符串和参数列表
- **SqlValue**：SQL 参数值的枚举类型，支持 Null、Bool、Int、Float、String、Bytes、Json、DateTime、Timestamp
- **Condition**：查询条件的枚举类型，支持 Eq、Ne、Gt、Lt、In、Between、Like、IsNull、And、Or 等
- **PluginManager**：yang-base 中的插件管理器，负责插件的注册、查找和生命周期管理
- **PluginRegistry**：优化后的不可变插件注册表，运行阶段无锁查找
- **DbError**：yang-db 中的数据库错误类型枚举
- **INSERT_BATCH_SIZE**：批量插入的默认批次大小常量（当前值为 500）
- **bind_param 函数族**：4 个功能相同但类型签名不同的参数绑定函数

## 需求

### 需求 1：批量插入内存分配优化

**用户故事：** 作为开发者，我希望批量插入操作减少中间内存分配，以便在大数据量场景下获得更好的性能。

#### 验收标准

1. WHEN `build_insert_batch` 被调用时，THE SqlGenerator SHALL 直接将 VALUES 子句写入 `self.sql` 字段，避免创建中间 `Vec<String>` 收集每条记录的占位符字符串
2. THE SqlGenerator SHALL 对每条记录使用 `self.sql.push_str("(")` 和逐个追加 `?` 占位符的方式生成 VALUES 子句，替代 `format!("({})", placeholders.join(", "))` 模式
3. THE SqlGenerator SHALL 在记录之间使用 `, ` 分隔符直接追加到 `self.sql`，替代最终的 `value_clauses.join(", ")` 调用
4. WHEN 批量插入 N 条记录时，THE SqlGenerator SHALL 产生 O(1) 次 `Vec<String>` 分配（仅字段名列表），替代当前的 O(N) 次中间 String 分配

### 需求 2：批量更新内存分配优化

**用户故事：** 作为开发者，我希望批量更新操作减少中间内存分配，以便在大数据量场景下获得更好的性能。

#### 验收标准

1. WHEN `build_update_batch` 被调用时，THE SqlGenerator SHALL 直接将 CASE WHEN 子句写入 `self.sql` 字段，避免为每个字段的每条记录创建 `format!("WHEN {}=? THEN ?", id_field)` 中间字符串
2. THE SqlGenerator SHALL 对 CASE WHEN 表达式使用 `self.sql.push_str` 逐步追加的方式生成，替代收集到 `Vec<String>` 再 join 的模式
3. WHEN 批量更新 N 条记录、M 个字段时，THE SqlGenerator SHALL 产生 O(M) 次中间分配（仅 SET 子句字段名），替代当前的 O(M×N) 次中间 String 分配

### 需求 3：操作符验证错误处理改进

**用户故事：** 作为开发者，我希望传入不支持的操作符时获得明确的错误返回而非程序崩溃，以便我的应用程序能够优雅地处理用户输入错误。

#### 验收标准

1. WHEN `where_and` 方法接收到不支持的操作符时，THE QueryBuilder SHALL 返回包含 `DbError` 的 `Result` 类型，替代当前的 `panic!("不支持的操作符: {}", op)` 行为
2. WHEN `where_or` 方法接收到不支持的操作符时，THE QueryBuilder SHALL 返回包含 `DbError` 的 `Result` 类型，替代当前的 `panic!` 行为
3. THE DbError 枚举 SHALL 新增 `UnsupportedOperator(String)` 变体，错误消息格式为 `"不支持的操作符: {op}"`
4. THE QueryBuilder SHALL 保持对已支持操作符（`=`、`!=`、`>`、`<`、`>=`、`<=`、`like`、`LIKE`）的现有行为不变
5. WHILE `where_and` 或 `where_or` 的返回类型变更为 `Result<Self, DbError>` 时，THE QueryBuilder SHALL 通过提供 `where_and_unchecked` 和 `where_or_unchecked` 方法（保持原有 panic 行为）确保向后兼容过渡

### 需求 4：bind_param 函数族宏消除重复

**用户故事：** 作为维护者，我希望消除 4 个 bind_param 函数中完全相同的 match 分支重复代码，以便未来新增 SqlValue 变体时只需修改一处。

#### 验收标准

1. THE yang-db crate SHALL 定义一个内部宏（如 `bind_value_match!`），封装 SqlValue 各变体到 `.bind()` 调用的映射逻辑
2. THE `bind_param`、`bind_execute_param`、`bind_scalar_param`、`bind_scalar_param_option` 四个函数 SHALL 使用该宏替代手写的 match 分支
3. WHEN 新增 SqlValue 变体时，THE 维护者 SHALL 只需在宏定义中添加一个分支即可完成所有 4 个函数的更新
4. THE 宏重构 SHALL 保持所有 4 个函数的外部签名和行为完全不变

### 需求 5：condition_to_sql 支持 owned 版本

**用户故事：** 作为开发者，我希望在不再需要 Condition 对象时避免不必要的 clone 开销，以便减少含有 String/Vec/JsonValue 的 SqlValue 的堆分配。

#### 验收标准

1. THE yang-db crate SHALL 提供 `condition_to_sql_owned` 公开函数，签名为 `fn condition_to_sql_owned(condition: Condition, params: &mut Vec<SqlValue>) -> String`
2. WHEN `condition_to_sql_owned` 被调用时，THE 函数 SHALL 消费传入的 Condition，将内部 SqlValue 直接 push 到 params 中，避免 `.clone()` 调用
3. THE 现有 `condition_to_sql` 函数（借用版本）SHALL 保持不变，确保向后兼容
4. WHEN Condition 包含 `SqlValue::String`、`SqlValue::Bytes` 或 `SqlValue::Json` 变体时，THE `condition_to_sql_owned` 函数 SHALL 相比 `condition_to_sql` 减少对应的堆分配

### 需求 6：SqlGenerator 预分配优化

**用户故事：** 作为开发者，我希望 SqlGenerator 创建时预分配合理的缓冲区容量，以便减少 SQL 字符串构建过程中的重新分配次数。

#### 验收标准

1. WHEN `SqlGenerator::new()` 被调用时，THE SqlGenerator SHALL 使用 `String::with_capacity(256)` 初始化 `sql` 字段，替代当前的 `String::new()`
2. WHEN `SqlGenerator::new()` 被调用时，THE SqlGenerator SHALL 使用 `Vec::with_capacity(8)` 初始化 `params` 字段，替代当前的 `Vec::new()`
3. THE SqlGenerator SHALL 在 `clear()` 方法中保留已分配的容量（使用 `self.sql.clear()` 和 `self.params.clear()`，不使用 `= String::new()` 或 `= Vec::new()`）

### 需求 7：批量插入自定义批次大小

**用户故事：** 作为开发者，我希望能够自定义批量插入的批次大小，以便根据不同场景（网络延迟、数据大小、MySQL max_allowed_packet）调整性能。

#### 验收标准

1. THE QueryBuilder SHALL 提供 `insert_batch_with_size` 公开方法，接受额外的 `batch_size: usize` 参数
2. WHEN `batch_size` 参数为 0 时，THE QueryBuilder SHALL 返回 `DbError::SerializationError` 错误
3. THE 现有 `insert_batch` 方法 SHALL 保持不变，内部使用默认的 `INSERT_BATCH_SIZE`（500）
4. WHEN 数据量超过指定的 `batch_size` 时，THE `insert_batch_with_size` 方法 SHALL 自动将数据分批执行，每批不超过 `batch_size` 条记录

### 需求 8：PluginManager 分离构建/运行阶段

**用户故事：** 作为开发者，我希望插件系统在运行阶段无需获取锁即可查找插件，以便在高并发场景下获得更好的性能。

#### 验收标准

1. THE yang-base crate SHALL 提供 `PluginManagerBuilder` 结构体，用于构建阶段的插件注册（可变操作）
2. THE yang-base crate SHALL 提供 `PluginRegistry` 结构体，用于运行阶段的插件查找（不可变、无锁）
3. WHEN `PluginManagerBuilder::build()` 被调用时，THE PluginManagerBuilder SHALL 消费自身并返回不可变的 `PluginRegistry`
4. THE `PluginRegistry` SHALL 使用 `HashMap<String, Arc<dyn Plugin>>` 存储插件（无 RwLock 包装）
5. WHEN `PluginRegistry::get()` 被调用时，THE PluginRegistry SHALL 直接通过 HashMap 查找返回结果，无需获取任何锁
6. THE 现有 `PluginManager` 结构体 SHALL 保持不变，确保向后兼容

### 需求 9：拓扑排序结果缓存

**用户故事：** 作为开发者，我希望插件的拓扑排序结果被缓存，以便多次调用 `get_all()` 时避免重复计算。

#### 验收标准

1. WHEN `PluginRegistry::build()` 完成时，THE PluginRegistry SHALL 在构建阶段执行一次拓扑排序并缓存结果
2. WHEN `PluginRegistry::get_all()` 被调用时，THE PluginRegistry SHALL 直接返回缓存的排序结果引用，无需重新计算
3. THE 现有 `PluginManager::get_all()` 方法 SHALL 保持当前行为不变（每次调用重新排序），确保向后兼容
4. THE PluginRegistry SHALL 将排序后的插件列表存储为 `Vec<Arc<dyn Plugin>>` 字段

### 需求 10：Permission 结构体简化

**用户故事：** 作为维护者，我希望评估 Permission 结构体的简化方案，以便减少不必要的堆分配同时保持 API 兼容性。

#### 验收标准

1. THE Permission 结构体 SHALL 新增 `from_static` 构造方法，接受 `&'static str` 参数，内部使用 `Cow<'static, str>` 存储以避免堆分配
2. THE Permission 结构体 SHALL 将内部 `name` 字段类型从 `String` 变更为 `Cow<'static, str>`
3. THE `Permission::new(impl Into<String>)` 方法 SHALL 保持不变，接受动态字符串并存储为 `Cow::Owned`
4. THE `Permission::name()` 方法 SHALL 继续返回 `&str`，保持外部 API 不变
5. WHEN 使用字面量字符串创建 Permission 时，THE `from_static` 方法 SHALL 避免堆分配（零拷贝）
