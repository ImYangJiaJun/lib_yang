# 设计文档：yang-db 与 yang-base 性能优化和代码简化

## 概述

本文档描述 yang-db 和 yang-base 两个 crate 的性能优化与代码简化的技术设计方案。优化目标涵盖内存分配减少、运行时 panic 消除、重复代码消除以及插件系统并发性能提升，所有改动保持公开 API 向后兼容。

---

## 架构概览

```
yang-db
├── src/mysql/
│   ├── query_builder.rs   ← 需求 1/2/3/6/7：SqlGenerator 优化、操作符错误处理、批次大小
│   ├── condition.rs       ← 需求 4/5：bind_param 宏、condition_to_sql_owned
│   └── error.rs           ← 需求 3：新增 UnsupportedOperator 变体
yang-base
└── src/plugin/
    └── mod.rs             ← 需求 8/9/10：PluginManagerBuilder/PluginRegistry、Permission 优化
```

---

## 模块设计

### 模块 1：SqlGenerator 内存分配优化（需求 1、2、6）

#### 1.1 预分配优化（需求 6）

**当前实现：**
```rust
pub(crate) fn new() -> Self {
    Self {
        sql: String::new(),
        params: Vec::new(),
    }
}
```

**优化后：**
```rust
pub(crate) fn new() -> Self {
    Self {
        sql: String::with_capacity(256),
        params: Vec::with_capacity(8),
    }
}
```

`clear()` 方法保持使用 `self.sql.clear()` 和 `self.params.clear()`，保留已分配容量，避免重复分配。

#### 1.2 批量插入内存优化（需求 1）

**当前实现（O(N) 中间分配）：**
```rust
let mut value_clauses = Vec::new();
for data in data_list {
    let mut placeholders = Vec::new();
    for field in &fields {
        placeholders.push("?".to_string());
        // ...
    }
    value_clauses.push(format!("({})", placeholders.join(", ")));
}
self.append(&value_clauses.join(", "));
```

**优化后（O(1) 中间分配）：**
```rust
for (record_idx, data) in data_list.iter().enumerate() {
    if record_idx > 0 {
        self.sql.push_str(", ");
    }
    self.sql.push('(');
    for (field_idx, field) in fields.iter().enumerate() {
        if field_idx > 0 {
            self.sql.push_str(", ");
        }
        self.sql.push('?');
        // 绑定参数...
    }
    self.sql.push(')');
}
```

消除了 `Vec<String>` 的 O(N) 中间分配，直接写入 `self.sql`。

#### 1.3 批量更新内存优化（需求 2）

**当前实现（O(M×N) 中间分配）：**
```rust
let mut set_parts = Vec::new();
for field in &update_fields {
    let mut when_parts = Vec::new();
    for record in records {
        when_parts.push(format!("WHEN {}=? THEN ?", id_field));
    }
    set_parts.push(format!("{} = CASE {} END", field, when_parts.join(" ")));
}
self.append(&set_parts.join(", "));
```

**优化后（O(M) 中间分配）：**
```rust
for (field_idx, field) in update_fields.iter().enumerate() {
    if field_idx > 0 {
        self.sql.push_str(", ");
    }
    self.sql.push_str(field);
    self.sql.push_str(" = CASE ");
    for record in records {
        self.sql.push_str("WHEN ");
        self.sql.push_str(id_field);
        self.sql.push_str("=? THEN ? ");
        // 绑定参数...
    }
    self.sql.push_str("END");
}
```

---

### 模块 2：操作符验证错误处理（需求 3）

#### 2.1 新增 DbError 变体

在 `crates/yang-db/src/error.rs` 中新增：

```rust
#[error("不支持的操作符: {0}")]
UnsupportedOperator(String),
```

#### 2.2 where_and / where_or 返回类型变更

**当前签名：**
```rust
pub fn where_and<V>(mut self, field: &str, op: &str, value: V) -> Self
pub fn where_or<V>(mut self, field: &str, op: &str, value: V) -> Self
```

**优化后签名：**
```rust
pub fn where_and<V>(mut self, field: &str, op: &str, value: V) -> Result<Self, DbError>
pub fn where_or<V>(mut self, field: &str, op: &str, value: V) -> Result<Self, DbError>
```

**向后兼容方法（保留 panic 行为）：**
```rust
pub fn where_and_unchecked<V>(mut self, field: &str, op: &str, value: V) -> Self
pub fn where_or_unchecked<V>(mut self, field: &str, op: &str, value: V) -> Self
```

`having_cond` 方法同样处理，新增 `having_cond_unchecked` 保持兼容。

#### 2.3 操作符匹配逻辑

```rust
let condition = match op {
    "=" => Condition::Eq(field.to_string(), sql_value),
    "!=" => Condition::Ne(field.to_string(), sql_value),
    ">" => Condition::Gt(field.to_string(), sql_value),
    "<" => Condition::Lt(field.to_string(), sql_value),
    ">=" => Condition::Gte(field.to_string(), sql_value),
    "<=" => Condition::Lte(field.to_string(), sql_value),
    "like" | "LIKE" => { /* ... */ }
    _ => return Err(DbError::UnsupportedOperator(op.to_string())),
};
Ok(self)
```

---

### 模块 3：bind_param 宏消除重复（需求 4）

#### 3.1 内部宏定义

在 `query_builder.rs` 中定义内部宏：

```rust
/// 将 SqlValue 绑定到 sqlx 查询的内部宏
macro_rules! bind_value_match {
    ($query:expr, $param:expr) => {
        match $param {
            SqlValue::Null      => $query.bind(Option::<i32>::None),
            SqlValue::Bool(b)   => $query.bind(*b),
            SqlValue::Int(i)    => $query.bind(*i),
            SqlValue::Float(f)  => $query.bind(*f),
            SqlValue::String(s) => $query.bind(s.clone()),
            SqlValue::Bytes(b)  => $query.bind(b.clone()),
            SqlValue::Json(j)   => $query.bind(j.to_string()),
            SqlValue::DateTime(dt) => $query.bind(*dt),
            SqlValue::Timestamp(ts) => $query.bind(*ts),
        }
    };
}
```

#### 3.2 四个函数使用宏

```rust
fn bind_execute_param<'q>(query: Query<'q, ...>, param: &SqlValue) -> Query<'q, ...> {
    bind_value_match!(query, param)
}

fn bind_param<'q, T>(query: QueryAs<'q, ...>, param: &SqlValue) -> QueryAs<'q, ...> {
    bind_value_match!(query, param)
}

fn bind_scalar_param<'q, T>(query: QueryScalar<'q, ...>, param: &SqlValue) -> QueryScalar<'q, ...> {
    bind_value_match!(query, param)
}

fn bind_scalar_param_option<'q, T>(query: QueryScalar<'q, Option<T>, ...>, param: &SqlValue) -> ... {
    bind_value_match!(query, param)
}
```

未来新增 `SqlValue` 变体时，只需在宏中添加一个分支。

---

### 模块 4：condition_to_sql_owned（需求 5）

#### 4.1 新增函数签名

在 `crates/yang-db/src/mysql/condition.rs` 中新增：

```rust
/// 消费版本的条件转 SQL 函数，避免 clone 开销
///
/// # 参数
/// - condition: 要消费的条件（owned）
/// - params: 用于收集参数的可变向量
///
/// # 返回
/// - SQL 字符串片段
pub fn condition_to_sql_owned(condition: Condition, params: &mut Vec<SqlValue>) -> String {
    match condition {
        Condition::Eq(field, value) => {
            params.push(value);  // 直接 push，无需 clone
            format!("{} = ?", field)
        }
        Condition::In(field, values) => {
            if values.is_empty() {
                return "1 = 0".to_string();
            }
            let count = values.len();
            params.extend(values);  // 直接 extend，无需 clone
            let placeholders = vec!["?"; count].join(", ");
            format!("{} IN ({})", field, placeholders)
        }
        // ... 其他变体类似处理
        Condition::And(conditions) => {
            let parts: Vec<String> = conditions
                .into_iter()
                .map(|c| condition_to_sql_owned(c, params))
                .collect();
            format!("({})", parts.join(" AND "))
        }
        // ...
    }
}
```

现有 `condition_to_sql`（借用版本）保持不变。

---

### 模块 5：批量插入自定义批次大小（需求 7）

#### 5.1 新增方法

```rust
/// 批量插入，支持自定义批次大小
///
/// # 参数
/// - data: 要插入的数据切片
/// - batch_size: 每批最多插入的记录数（必须 > 0）
///
/// # 返回
/// - Ok(u64): 总受影响行数
/// - Err(DbError::SerializationError): batch_size 为 0 时
pub async fn insert_batch_with_size<T>(
    self,
    data: &[T],
    batch_size: usize,
) -> Result<u64, DbError>
where
    T: serde::Serialize,
{
    if batch_size == 0 {
        return Err(DbError::SerializationError(
            "batch_size 不能为 0".to_string(),
        ));
    }
    // 内部逻辑与 insert_batch 相同，使用传入的 batch_size 替代 INSERT_BATCH_SIZE
    // ...
}
```

现有 `insert_batch` 方法保持不变，内部继续使用 `INSERT_BATCH_SIZE`（500）。

---

### 模块 6：PluginManager 分离构建/运行阶段（需求 8、9）

#### 6.1 整体架构

```
构建阶段                    运行阶段
PluginManagerBuilder  →  PluginRegistry
  register(plugin)         get(name)       → O(1) 无锁
  build()                  get_all()       → 返回缓存的排序结果
```

#### 6.2 PluginManagerBuilder

```rust
/// 插件管理器构建器（构建阶段使用）
pub struct PluginManagerBuilder {
    /// 插件存储（构建阶段可变）
    plugins: HashMap<String, Arc<dyn Plugin>>,
    /// 插件配置
    configs: HashMap<String, JsonValue>,
}

impl PluginManagerBuilder {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            configs: HashMap::new(),
        }
    }

    /// 注册插件（同步版本，构建阶段使用）
    pub async fn register<P: Plugin + 'static>(
        &mut self,
        plugin: P,
    ) -> Result<(), BaseError> {
        let name = plugin.name().to_string();
        if self.plugins.contains_key(&name) {
            return Err(BaseError::PluginAlreadyRegistered(name));
        }
        let plugin = Arc::new(plugin);
        plugin.on_register().await
            .map_err(|e| BaseError::PluginRegisterFailed(name.clone(), e.to_string()))?;
        self.plugins.insert(name, plugin);
        Ok(())
    }

    /// 消费 Builder，生成不可变的 PluginRegistry
    pub fn build(self) -> PluginRegistry {
        PluginRegistry::new(self.plugins, self.configs)
    }
}
```

#### 6.3 PluginRegistry（需求 8、9）

```rust
/// 插件注册表（运行阶段使用，无锁）
pub struct PluginRegistry {
    /// 不可变插件映射（无 RwLock）
    plugins: HashMap<String, Arc<dyn Plugin>>,
    /// 拓扑排序缓存（构建时计算一次）
    sorted_plugins: Vec<Arc<dyn Plugin>>,
    /// 插件配置
    configs: HashMap<String, JsonValue>,
}

impl PluginRegistry {
    fn new(
        plugins: HashMap<String, Arc<dyn Plugin>>,
        configs: HashMap<String, JsonValue>,
    ) -> Self {
        // 构建时执行一次拓扑排序并缓存
        let sorted_plugins = Self::compute_topological_sort(&plugins);
        Self { plugins, sorted_plugins, configs }
    }

    /// 查找插件（无锁，O(1)）
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Plugin>> {
        self.plugins.get(name)
    }

    /// 获取所有插件（返回缓存的排序结果，无需重新计算）
    pub fn get_all(&self) -> &[Arc<dyn Plugin>] {
        &self.sorted_plugins
    }

    /// 关闭所有插件（逆序）
    pub async fn shutdown(&self) -> Result<(), BaseError> {
        for plugin in self.sorted_plugins.iter().rev() {
            plugin.on_shutdown().await
                .map_err(|e| BaseError::PluginShutdownFailed(
                    plugin.name().to_string(), e.to_string()
                ))?;
        }
        Ok(())
    }

    /// 构建时执行拓扑排序（私有）
    fn compute_topological_sort(
        plugins: &HashMap<String, Arc<dyn Plugin>>,
    ) -> Vec<Arc<dyn Plugin>> {
        // Kahn 算法，与现有 topological_sort 逻辑相同
        // ...
    }
}
```

现有 `PluginManager` 保持不变，确保向后兼容。

---

### 模块 7：Permission 结构体简化（需求 10）

#### 7.1 当前结构（假设）

```rust
pub struct Permission {
    name: String,
}
```

#### 7.2 优化后结构

```rust
use std::borrow::Cow;

/// 权限结构体
pub struct Permission {
    /// 使用 Cow 支持零拷贝静态字符串和动态字符串
    name: Cow<'static, str>,
}

impl Permission {
    /// 从动态字符串创建（堆分配）
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: Cow::Owned(name.into()),
        }
    }

    /// 从静态字符串创建（零拷贝，无堆分配）
    pub fn from_static(name: &'static str) -> Self {
        Self {
            name: Cow::Borrowed(name),
        }
    }

    /// 获取权限名称（API 不变）
    pub fn name(&self) -> &str {
        &self.name
    }
}
```

`new()` 方法签名保持不变，`name()` 返回类型保持 `&str`，外部 API 完全兼容。

---

## 正确性属性（Property-Based Testing）

### 属性 P1：批量插入 SQL 结构正确性

**形式化描述：** 对于任意非空数据列表 `data_list`，`build_insert_batch` 生成的 SQL 满足：
- 包含 `INSERT INTO {table}` 前缀
- VALUES 子句中 `(` 的数量等于 `data_list.len()`
- 参数数量等于 `data_list.len() × fields.len()`

**测试策略：**
```rust
proptest! {
    fn prop_insert_batch_sql_structure(
        records in vec(any_json_object(), 1..50)
    ) {
        let mut gen = SqlGenerator::new();
        gen.build_insert_batch("t", &records, &HashMap::new()).unwrap();
        let sql = gen.get_sql();
        let open_paren_count = sql.matches('(').count() - 1; // 减去字段列表的括号
        prop_assert_eq!(open_paren_count, records.len());
        prop_assert_eq!(gen.get_params().len(), records.len() * field_count);
    }
}
```

### 属性 P2：操作符错误处理

**形式化描述：** 对于任意不在支持集合 `{=, !=, >, <, >=, <=, like, LIKE}` 中的操作符字符串 `op`，`where_and(field, op, value)` 必须返回 `Err(DbError::UnsupportedOperator(op))`。

**测试策略：**
```rust
proptest! {
    fn prop_unsupported_operator_returns_error(
        op in "[^=!><lLiIkKeE]{1,10}"
    ) {
        let result = builder.where_and("field", &op, 1i64);
        prop_assert!(matches!(result, Err(DbError::UnsupportedOperator(_))));
    }
}
```

### 属性 P3：condition_to_sql_owned 与 condition_to_sql 等价性

**形式化描述：** 对于任意 `Condition c`，`condition_to_sql_owned(c.clone(), &mut p1)` 生成的 SQL 字符串与 `condition_to_sql(&c, &mut p2)` 完全相同，且参数列表语义等价。

**测试策略：**
```rust
proptest! {
    fn prop_owned_equals_borrowed(cond in any_condition()) {
        let mut p1 = vec![];
        let mut p2 = vec![];
        let sql1 = condition_to_sql_owned(cond.clone(), &mut p1);
        let sql2 = condition_to_sql(&cond, &mut p2);
        prop_assert_eq!(sql1, sql2);
        prop_assert_eq!(p1.len(), p2.len());
    }
}
```

### 属性 P4：PluginRegistry 查找一致性

**形式化描述：** 对于任意插件集合，`PluginRegistry::get(name)` 的结果与构建前 `PluginManagerBuilder` 中注册的插件一一对应。

### 属性 P5：批次大小分割正确性

**形式化描述：** 对于任意 `n` 条记录和批次大小 `b`（b > 0），`insert_batch_with_size` 执行的批次数等于 `ceil(n / b)`，且总受影响行数等于 `n`。

### 属性 P6：SqlGenerator 预分配不影响正确性

**形式化描述：** 使用 `with_capacity` 初始化的 `SqlGenerator` 与使用 `new()` 初始化的生成相同的 SQL 和参数列表（容量变化不影响内容）。

---

## 向后兼容性保证

| 变更项 | 兼容策略 |
|--------|---------|
| `where_and` 返回类型变更为 `Result` | 新增 `where_and_unchecked` 保持原有 panic 行为 |
| `where_or` 返回类型变更为 `Result` | 新增 `where_or_unchecked` 保持原有 panic 行为 |
| `having_cond` 返回类型变更为 `Result` | 新增 `having_cond_unchecked` 保持原有 panic 行为 |
| 新增 `PluginRegistry` | 现有 `PluginManager` 保持不变 |
| 新增 `PluginManagerBuilder` | 现有 `PluginManager` 保持不变 |
| `Permission.name` 字段类型变更 | `name()` 方法返回 `&str` 不变，`new()` 签名不变 |
| `condition_to_sql_owned` 新增 | 现有 `condition_to_sql` 保持不变 |
| `insert_batch_with_size` 新增 | 现有 `insert_batch` 保持不变 |

---

## 文件变更清单

| 文件 | 变更类型 | 涉及需求 |
|------|---------|---------|
| `crates/yang-db/src/mysql/query_builder.rs` | 修改 | 1, 2, 3, 6, 7 |
| `crates/yang-db/src/mysql/condition.rs` | 修改 | 5 |
| `crates/yang-db/src/error.rs` | 修改 | 3 |
| `crates/yang-base/src/plugin/mod.rs` | 修改 | 8, 9 |
| `crates/yang-base/src/action/` | 修改（如 Permission 存在） | 10 |
| `crates/yang-db/src/lib.rs` | 修改（导出新公开 API） | 5, 7 |
| `crates/yang-base/src/lib.rs` | 修改（导出新公开 API） | 8, 9, 10 |
