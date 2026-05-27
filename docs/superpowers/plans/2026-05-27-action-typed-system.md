# Action 系统类型化重构 实施计划（H-1）

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 yang-base 的 Action 系统从 `serde_json::Value` 字符串契约重构为编译期端到端类型化（`TypedHandler` + `#[derive(TableEntity)]` + `#[derive(Action)]`），消除运行时字段名/类型错误。

**Architecture:** 三层 trait（用户写 `TypedHandler` → 派生宏生成 `TypedAction` → blanket impl 桥接到 `Action` dyn 擦除层）。新增 `yang-base-derive` proc-macro crate 派生 `TableEntity`/`Action`，自动生成 `Field`/`WhereCond` 枚举以及运行时 `TableConfig`。Builtin CRUD 全部泛型化为 `Action<T: TableEntity>`，一行 `table_typed::<T>()` 注册全套。

**Tech Stack:** Rust 2021、`async-trait`、`schemars`（JSON Schema 派生）、`syn` + `quote` + `darling` + `proc-macro-error`（派生宏）、`trybuild`（编译失败测试）、`insta`（schema 快照）。

**对应 Spec：** `docs/superpowers/specs/2026-05-27-action-typed-system-design.md`

**约束：**
- 基础库尚未投产，**允许破坏性变更**。每个任务结束时整个 workspace 必须能编译通过、`cargo test --lib -p yang-base` 不出现 build error；功能上可允许"旧 API 的某个测试在这步暂未恢复"，但绝不允许整个 crate 编译失败。
- 实施期间，**先把旧 builtin/旧 Action trait 的引用都注释或暂禁**（feature gate `legacy_action`），等步骤 5 重写完成后再彻底删除。这避免每一步都得修一遍下游测试。

---

## 文件结构

### 新增文件

| 文件 | 职责 |
|---|---|
| `crates/yang-base-derive/Cargo.toml` | 派生宏 crate 元信息（`proc-macro = true`） |
| `crates/yang-base-derive/src/lib.rs` | 入口：导出 `#[derive(TableEntity)]` 与 `#[derive(Action)]` |
| `crates/yang-base-derive/src/table_entity.rs` | `TableEntity` 派生实现（生成 Field/WhereCond/impl） |
| `crates/yang-base-derive/src/action.rs` | `Action` 派生实现（解析 `#[action(...)]` 属性） |
| `crates/yang-base-derive/src/util.rs` | 共用辅助（属性解析、Rust 类型 → FieldType 映射） |
| `crates/yang-base/src/action/typed.rs` | `TypedHandler`、`TypedAction`、`Action`、`ActionMeta`、blanket impl |
| `crates/yang-base/src/table/entity.rs` | `TableEntity` trait、`WhereOp<V>`、`AsColumnName`、`IntoSqlCondition`、`SqlCondition`、`SqlOp` |
| `crates/yang-base/src/action/sql_bridge.rs` | `apply_sql_condition`、`count_with_conditions` 等内部 helper |
| `crates/yang-base/tests/typed_action_integration.rs` | 端到端 CRUD 集成测试 |
| `crates/yang-base/tests/compile_fail/where_invalid_field.rs` | trybuild：非法字段名 variant |
| `crates/yang-base/tests/compile_fail/where_type_mismatch.rs` | trybuild：类型不匹配 |
| `crates/yang-base/tests/compile_fail/like_on_int.rs` | trybuild：数值字段用 Like |
| `crates/yang-base/tests/compile_fail/missing_primary_key.rs` | trybuild：派生缺主键 |
| `crates/yang-base/tests/trybuild.rs` | trybuild 入口 |
| `crates/yang-base/tests/schema_snapshots.rs` | insta 快照测试 |
| `crates/yang-base/tests/snapshots/` | insta 快照目录（自动生成） |

### 修改文件

| 文件 | 改动概要 |
|---|---|
| `Cargo.toml`（workspace） | 添加 `crates/yang-base-derive` 成员；workspace.dependencies 加 `schemars`、`syn`、`quote`、`darling`、`proc-macro-error`、`trybuild`、`insta` |
| `crates/yang-base/Cargo.toml` | 加依赖 `schemars`、`yang-base-derive`；dev-dep 加 `trybuild`、`insta` |
| `crates/yang-base/src/lib.rs` | `pub use` 派生宏；导出新 trait |
| `crates/yang-base/src/action/mod.rs` | 导出 `typed`、`sql_bridge`；逐步切换默认导出到新 trait |
| `crates/yang-base/src/action/action_trait.rs` | 重命名为 `Action`（dyn）+ `TypedHandler`/`TypedAction`；旧定义先迁移到 `legacy.rs` 暂留至步骤 8 |
| `crates/yang-base/src/action/context.rs` | 移除 `param`/`param_optional`/`param_or_default`/`query_param`；新增 `extract_input` |
| `crates/yang-base/src/action/response.rs` | 不动签名；只在 doc 里更新示例 |
| `crates/yang-base/src/action/builtin/*.rs` | 全部重写：泛型化为 `Action<T: TableEntity>`，实现 `TypedHandler` |
| `crates/yang-base/src/router/module_router.rs` | 新增 `table_typed::<T>()`；`dispatch` 改为读 `ActionMeta`；旧 `register_builtin_actions` 删除 |
| `crates/yang-base/src/table/mod.rs` | 导出 `entity` 模块 |
| `crates/yang-base/src/table/table_query.rs` | 补缺失的 `where_lt`/`where_lte`/`where_gt`/`where_gte`/`where_ne`/`where_between`/`where_null`/`where_not_null` 方法；新增 `count(self)` |
| `crates/yang-base/src/error/mod.rs` | 不动（沿用既有 `ParamInvalid` 等） |
| `docs/BACKLOG.md` | 步骤 8 末尾把 H-1 状态从 ⏳ 改为 ✅ |
| `docs/yang-base.md`、`crates/yang-base/AGENTS.md` | 步骤 8 更新到新 API |

---

## 任务清单

按 spec §11 的 8 步切分，每步独立合并、独立通过 `cargo test`。

---

### Task 1：补齐 TableQuery 缺失的 where 方法 + count

**目的：** 后续 builtin 重写依赖 `where_lt`/`where_between`/`where_ne`/`where_null`/`where_not_null`/`count` 等方法。这些是低风险的纯增量，先做。

**Files:**
- Modify: `crates/yang-base/src/table/table_query.rs`（参照已存在的 `where_eq` 实现）
- Modify: `crates/yang-base/src/table/query_params.rs`（如需要新增 `Condition::Ne` 等枚举变体）
- Test: `crates/yang-base/src/table/__tests__/table_query_test.rs`

- [ ] **Step 1.1：先看当前 query_params.rs 的 Condition 枚举有哪些变体**

Run: `grep -nE "^\s+(Eq|Ne|Lt|Lte|Gt|Gte|In|NotIn|Like|Between|IsNull|IsNotNull)\b" crates/yang-base/src/table/query_params.rs`

记录现有 variant，确定哪些需要补。Spec 要求最终支持：Eq, Ne, Lt, Lte, Gt, Gte, In, NotIn, Like, Between, IsNull, IsNotNull。

- [ ] **Step 1.2：写一个失败测试覆盖新方法**

文件：`crates/yang-base/src/table/__tests__/table_query_test.rs`，在末尾加测试：

```rust
#[test]
fn test_new_where_methods_build_sql() {
    use crate::table::{TableConfig, FieldConfig, FieldType};
    let config = std::sync::Arc::new(
        TableConfig::new("users")
            .field(FieldConfig::new("id", FieldType::BigInt))
            .field(FieldConfig::new("age", FieldType::Integer))
            .field(FieldConfig::new("name", FieldType::String { max_length: 50 }))
    );
    let q = crate::table::TableQuery::new_without_pool(config)
        .where_lt("age", serde_json::json!(18)).unwrap()
        .where_gte("id", serde_json::json!(1)).unwrap()
        .where_ne("name", serde_json::json!("admin")).unwrap()
        .where_between("id", serde_json::json!(10), serde_json::json!(20)).unwrap()
        .where_null("name").unwrap()
        .where_not_null("age").unwrap();
    // 简单断言：SQL 包含预期片段
    let (sql, _) = q.build_select_sql_for_test().expect("build sql");
    assert!(sql.contains("`age` < ?"));
    assert!(sql.contains("`id` >= ?"));
    assert!(sql.contains("`name` <> ?"));
    assert!(sql.contains("`id` BETWEEN ? AND ?"));
    assert!(sql.contains("`name` IS NULL"));
    assert!(sql.contains("`age` IS NOT NULL"));
}
```

如果 `TableQuery::new_without_pool` 或 `build_select_sql_for_test` 不存在，先暴露为 `pub(crate)` 测试 helper（参考现有测试如何构造 TableQuery）。

- [ ] **Step 1.3：运行测试验证失败**

```bash
cargo test --lib -p yang-base table::__tests__::table_query_test::test_new_where_methods_build_sql
```

期望：编译失败（缺少 `where_lt` 等方法）或运行失败。

- [ ] **Step 1.4：在 `query_params.rs` 的 `Condition` 枚举里补全缺失变体**

补全为：

```rust
pub enum Condition {
    Eq(String, SqlParam),
    Ne(String, SqlParam),
    Lt(String, SqlParam),
    Lte(String, SqlParam),
    Gt(String, SqlParam),
    Gte(String, SqlParam),
    In(String, Vec<SqlParam>),
    NotIn(String, Vec<SqlParam>),
    Like(String, String),
    Between(String, SqlParam, SqlParam),
    IsNull(String),
    IsNotNull(String),
}
```

- [ ] **Step 1.5：在 `table_query.rs` 补对应方法**

参考已有 `where_eq` 模式（验证字段、push Condition），逐个补：

```rust
pub fn where_ne(mut self, field: &str, value: Value) -> Result<Self, BaseError> {
    self.table_config.validate_field(field)?;
    self.query_params.conditions.push(Condition::Ne(field.to_string(), value.into()));
    Ok(self)
}
// where_lt / where_lte / where_gt / where_gte 同上，只是变体不同
// where_between：两个 value
pub fn where_between(mut self, field: &str, lo: Value, hi: Value) -> Result<Self, BaseError> { ... }
// where_null / where_not_null：无 value
pub fn where_null(mut self, field: &str) -> Result<Self, BaseError> { ... }
pub fn where_not_null(mut self, field: &str) -> Result<Self, BaseError> { ... }
// where_not_in
pub fn where_not_in(mut self, field: &str, values: Vec<Value>) -> Result<Self, BaseError> { ... }
```

- [ ] **Step 1.6：更新 `build_select_sql` 里的 WHERE 子句构建，处理所有新变体**

定位 `append_where_to_sql`（或类似 helper），把 match 分支扩展为覆盖 `Ne`/`Lt`/`Lte`/`Gt`/`Gte`/`NotIn`/`Between`/`IsNull`/`IsNotNull` 全部新变体。SQL 模板：

- `Ne`：`field <> ?`
- `Lt`/`Lte`/`Gt`/`Gte`：`field </<=/>/>= ?`
- `NotIn`：`field NOT IN (?, ?, ...)`
- `Between`：`field BETWEEN ? AND ?`
- `IsNull`：`field IS NULL`
- `IsNotNull`：`field IS NOT NULL`

字段名通过 `self.quote_identifier(field)?` 转义（与现有代码一致）。

- [ ] **Step 1.7：新增 `count` 方法**

```rust
pub async fn count(self) -> Result<u64, BaseError> {
    let pool = self.pool.as_ref().ok_or(BaseError::DatabaseNotInitialized)?;
    let (mut sql, params) = self.build_select_sql()?;
    // 替换 SELECT ... FROM 部分为 SELECT COUNT(*) FROM；去掉 ORDER BY / LIMIT
    let count_sql = rewrite_to_count(&sql);  // 简单 helper
    let mut query = sqlx::query_scalar::<_, i64>(&count_sql);
    for param in params { query = Self::bind_param_scalar(query, &param); }
    let n = query.fetch_one(pool.as_ref()).await
        .map_err(|e| BaseError::DatabaseQueryFailed(yang_db::DbError::from(e)))?;
    Ok(n as u64)
}
```

如果 `rewrite_to_count` 太脆弱，改为复制 `build_select_sql` 写一个 `build_count_sql` 专用方法：

```rust
fn build_count_sql(&self) -> Result<(String, Vec<SqlParam>), BaseError> {
    let mut sql = String::from("SELECT COUNT(*) FROM `");
    sql.push_str(&self.table_config.table_name);
    sql.push('`');
    let mut params = Vec::new();
    self.append_where_to_sql(&mut sql, &mut params)?;
    Ok((sql, params))
}
```

推荐用第二种，明确且无脆弱字符串替换。

- [ ] **Step 1.8：运行新测试**

```bash
cargo test --lib -p yang-base table::__tests__::table_query_test::test_new_where_methods_build_sql
```

期望：PASS。

- [ ] **Step 1.9：跑完整 lib 测试，确保没回归**

```bash
cargo test --lib -p yang-base
```

期望：全部 PASS。

- [ ] **Step 1.10：commit**

```bash
git add crates/yang-base/src/table/query_params.rs crates/yang-base/src/table/table_query.rs crates/yang-base/src/table/__tests__/table_query_test.rs
git commit -m "feat(yang-base): 补齐 TableQuery 的 ne/lt/lte/gt/gte/between/null/not_null/count 方法

为后续 Action 类型化重构 (H-1) 提供 SQL 条件构造的完整能力。"
```

---

### Task 2：新增 `schemars` 依赖 + `TableEntity`/`WhereOp`/`SqlCondition` 类型骨架（不含派生宏）

**目的：** 把 spec §4.1–§4.2 的类型定义落地为可编译代码，先用一个手写实现验证设计自洽。这一步不引入派生宏，是 spec §11 步骤 2。

**Files:**
- Modify: `Cargo.toml`（workspace 加 schemars）
- Modify: `crates/yang-base/Cargo.toml`（dep 加 schemars）
- Create: `crates/yang-base/src/table/entity.rs`
- Modify: `crates/yang-base/src/table/mod.rs`
- Test: `crates/yang-base/src/table/__tests__/entity_test.rs`

- [ ] **Step 2.1：workspace 加 schemars 依赖**

修改 `Cargo.toml`（workspace），在 `[workspace.dependencies]` 添加：

```toml
# JSON Schema 派生
schemars = { version = "0.8", features = ["chrono"] }
```

- [ ] **Step 2.2：yang-base 引用 schemars**

修改 `crates/yang-base/Cargo.toml` 的 `[dependencies]`，添加：

```toml
schemars = { workspace = true }
```

- [ ] **Step 2.3：创建 `entity.rs` 文件**

文件：`crates/yang-base/src/table/entity.rs`，内容（完整可编译）：

```rust
//! TableEntity 类型化基础设施
//!
//! 提供 `TableEntity` trait 以及 `WhereOp<V>`、`SqlCondition` 等类型，
//! 用于 Action 系统的端到端类型安全（H-1）。

use crate::table::TableConfig;
use serde::{Deserialize, Serialize};
use std::hash::Hash;

/// 表实体契约：用户定义的行类型（一般通过 `#[derive(TableEntity)]` 派生）。
///
/// 把"行 struct"、"字段名枚举"、"where 条件枚举"、"主键类型"统一在一个 trait 下，
/// 让整个 Action 系统能从一个 `T` 推导出全部类型化契约。
#[cfg(feature = "mysql")]
pub trait TableEntity:
    serde::de::DeserializeOwned
    + serde::Serialize
    + schemars::JsonSchema
    + for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow>
    + Send + Sync + Unpin + 'static
{
    type Pk: serde::de::DeserializeOwned + serde::Serialize + schemars::JsonSchema + Send + Sync + 'static;
    type Field: AsColumnName + serde::de::DeserializeOwned + serde::Serialize
        + schemars::JsonSchema + Copy + Eq + Hash + Send + Sync + 'static;
    type WhereCond: IntoSqlCondition + serde::de::DeserializeOwned
        + schemars::JsonSchema + Send + Sync + 'static;

    const TABLE_NAME: &'static str;
    const PK_FIELD: &'static str;

    fn table_config() -> &'static TableConfig;
}

/// 字段名 → 静态列名字符串。所有判别式都映射到 `&'static str`，
/// 杜绝列名拼接 SQL 注入。
pub trait AsColumnName {
    fn column_name(&self) -> &'static str;
}

/// where 条件 → SqlCondition（运行期 SQL 片段描述）。
pub trait IntoSqlCondition {
    fn into_sql_condition(self) -> SqlCondition;
}

/// 通用 where 操作符。
///
/// JSON 形态由 `#[serde(tag = "op", content = "value")]` 决定：
/// `{ "op": "eq", "value": 42 }` / `{ "op": "in", "value": [1,2,3] }` /
/// `{ "op": "between", "value": [10, 20] }` / `{ "op": "is_null" }`
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(tag = "op", content = "value", rename_all = "snake_case")]
pub enum WhereOp<V> {
    Eq(V),
    Ne(V),
    Lt(V),
    Lte(V),
    Gt(V),
    Gte(V),
    In(Vec<V>),
    NotIn(Vec<V>),
    Between(V, V),
    IsNull,
    IsNotNull,
}

/// 字符串字段专用 where 操作符（额外含 Like）。
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(tag = "op", content = "value", rename_all = "snake_case")]
pub enum StringWhereOp {
    Eq(String),
    Ne(String),
    Lt(String),
    Lte(String),
    Gt(String),
    Gte(String),
    In(Vec<String>),
    NotIn(Vec<String>),
    Between(String, String),
    Like(String),
    IsNull,
    IsNotNull,
}

/// 运行时 SQL 条件描述。column 是 `'static str`，绝对安全。
#[derive(Debug, Clone)]
pub struct SqlCondition {
    pub column: &'static str,
    pub op: SqlOp,
    pub params: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Copy)]
pub enum SqlOp {
    Eq, Ne, Lt, Lte, Gt, Gte,
    In, NotIn,
    Between,
    Like,
    IsNull, IsNotNull,
}

impl<V: serde::Serialize> WhereOp<V> {
    /// 把通用 WhereOp 转为 SqlCondition（给定列名）。供派生宏生成的 IntoSqlCondition 调用。
    pub fn to_sql_condition(self, column: &'static str) -> SqlCondition {
        match self {
            WhereOp::Eq(v)  => SqlCondition { column, op: SqlOp::Eq,  params: vec![to_v(v)] },
            WhereOp::Ne(v)  => SqlCondition { column, op: SqlOp::Ne,  params: vec![to_v(v)] },
            WhereOp::Lt(v)  => SqlCondition { column, op: SqlOp::Lt,  params: vec![to_v(v)] },
            WhereOp::Lte(v) => SqlCondition { column, op: SqlOp::Lte, params: vec![to_v(v)] },
            WhereOp::Gt(v)  => SqlCondition { column, op: SqlOp::Gt,  params: vec![to_v(v)] },
            WhereOp::Gte(v) => SqlCondition { column, op: SqlOp::Gte, params: vec![to_v(v)] },
            WhereOp::In(vs) => SqlCondition { column, op: SqlOp::In,
                params: vs.into_iter().map(to_v).collect() },
            WhereOp::NotIn(vs) => SqlCondition { column, op: SqlOp::NotIn,
                params: vs.into_iter().map(to_v).collect() },
            WhereOp::Between(a, b) => SqlCondition { column, op: SqlOp::Between,
                params: vec![to_v(a), to_v(b)] },
            WhereOp::IsNull => SqlCondition { column, op: SqlOp::IsNull, params: vec![] },
            WhereOp::IsNotNull => SqlCondition { column, op: SqlOp::IsNotNull, params: vec![] },
        }
    }
}

impl StringWhereOp {
    pub fn to_sql_condition(self, column: &'static str) -> SqlCondition {
        match self {
            StringWhereOp::Like(p) => SqlCondition { column, op: SqlOp::Like,
                params: vec![serde_json::Value::String(p)] },
            // 其余复用 WhereOp 的语义：手动展开
            StringWhereOp::Eq(v) => WhereOp::Eq(v).to_sql_condition(column),
            StringWhereOp::Ne(v) => WhereOp::Ne(v).to_sql_condition(column),
            StringWhereOp::Lt(v) => WhereOp::Lt(v).to_sql_condition(column),
            StringWhereOp::Lte(v) => WhereOp::Lte(v).to_sql_condition(column),
            StringWhereOp::Gt(v) => WhereOp::Gt(v).to_sql_condition(column),
            StringWhereOp::Gte(v) => WhereOp::Gte(v).to_sql_condition(column),
            StringWhereOp::In(vs) => WhereOp::In(vs).to_sql_condition(column),
            StringWhereOp::NotIn(vs) => WhereOp::NotIn(vs).to_sql_condition(column),
            StringWhereOp::Between(a, b) => WhereOp::Between(a, b).to_sql_condition(column),
            StringWhereOp::IsNull => WhereOp::<String>::IsNull.to_sql_condition(column),
            StringWhereOp::IsNotNull => WhereOp::<String>::IsNotNull.to_sql_condition(column),
        }
    }
}

fn to_v<V: serde::Serialize>(v: V) -> serde_json::Value {
    serde_json::to_value(v).unwrap_or(serde_json::Value::Null)
}
```

- [ ] **Step 2.4：导出 entity 模块**

修改 `crates/yang-base/src/table/mod.rs`，加：

```rust
pub mod entity;
pub use entity::{
    AsColumnName, IntoSqlCondition, SqlCondition, SqlOp,
    WhereOp, StringWhereOp,
};
#[cfg(feature = "mysql")]
pub use entity::TableEntity;
```

- [ ] **Step 2.5：写一个手写 TestEntity 测试，验证设计可用**

文件：`crates/yang-base/src/table/__tests__/entity_test.rs`（新建）

```rust
//! 手写 TableEntity 实现验证（步骤 2，无派生宏）
#![cfg(feature = "mysql")]

use crate::table::entity::*;
use crate::table::{TableConfig, FieldConfig, FieldType};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema, sqlx::FromRow)]
pub struct TestUser {
    pub id: i64,
    pub username: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TestUserField { Id, Username }

impl AsColumnName for TestUserField {
    fn column_name(&self) -> &'static str {
        match self {
            TestUserField::Id => "id",
            TestUserField::Username => "username",
        }
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(tag = "field", content = "cond", rename_all = "snake_case")]
pub enum TestUserWhere {
    Id(WhereOp<i64>),
    Username(StringWhereOp),
}

impl IntoSqlCondition for TestUserWhere {
    fn into_sql_condition(self) -> SqlCondition {
        match self {
            TestUserWhere::Id(op) => op.to_sql_condition("id"),
            TestUserWhere::Username(op) => op.to_sql_condition("username"),
        }
    }
}

impl TableEntity for TestUser {
    type Pk = i64;
    type Field = TestUserField;
    type WhereCond = TestUserWhere;
    const TABLE_NAME: &'static str = "test_users";
    const PK_FIELD: &'static str = "id";
    fn table_config() -> &'static TableConfig {
        static C: OnceLock<TableConfig> = OnceLock::new();
        C.get_or_init(|| TableConfig::new("test_users")
            .primary_key("id")
            .field(FieldConfig::new("id", FieldType::BigInt))
            .field(FieldConfig::new("username", FieldType::String { max_length: 50 })))
    }
}

#[test]
fn test_where_op_deserialize() {
    let json = r#"{"op":"eq","value":42}"#;
    let op: WhereOp<i64> = serde_json::from_str(json).unwrap();
    assert!(matches!(op, WhereOp::Eq(42)));
}

#[test]
fn test_where_op_in_deserialize() {
    let json = r#"{"op":"in","value":[1,2,3]}"#;
    let op: WhereOp<i64> = serde_json::from_str(json).unwrap();
    assert!(matches!(op, WhereOp::In(_)));
}

#[test]
fn test_test_user_where_deserialize() {
    let json = r#"{"field":"id","cond":{"op":"eq","value":42}}"#;
    let cond: TestUserWhere = serde_json::from_str(json).unwrap();
    let sql_cond = cond.into_sql_condition();
    assert_eq!(sql_cond.column, "id");
    assert!(matches!(sql_cond.op, SqlOp::Eq));
}

#[test]
fn test_invalid_field_rejected() {
    let json = r#"{"field":"unknown","cond":{"op":"eq","value":42}}"#;
    let result: Result<TestUserWhere, _> = serde_json::from_str(json);
    assert!(result.is_err(), "未知字段名必须反序列化失败");
}

#[test]
fn test_string_like_works() {
    let json = r#"{"field":"username","cond":{"op":"like","value":"%alice%"}}"#;
    let cond: TestUserWhere = serde_json::from_str(json).unwrap();
    let sql_cond = cond.into_sql_condition();
    assert_eq!(sql_cond.column, "username");
    assert!(matches!(sql_cond.op, SqlOp::Like));
    assert_eq!(sql_cond.params[0].as_str(), Some("%alice%"));
}
```

注册测试模块：在 `crates/yang-base/src/table/__tests__/mod.rs` 加 `mod entity_test;`（如该文件不存在则查看实际结构，在 lib.rs 或 `table/mod.rs` 的 `#[cfg(test)] mod __tests__` 里 plug 进去）。

- [ ] **Step 2.6：编译并跑测试**

```bash
cargo test --lib -p yang-base table::__tests__::entity_test
```

期望：5 个测试全 PASS。如有 `JsonSchema` derive 编译错，确认 schemars 0.8 的 derive 路径是 `schemars::JsonSchema`。

- [ ] **Step 2.7：跑完整 workspace 编译**

```bash
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

期望：无错误、无 warning。

- [ ] **Step 2.8：commit**

```bash
git add Cargo.toml crates/yang-base/Cargo.toml \
        crates/yang-base/src/table/entity.rs \
        crates/yang-base/src/table/mod.rs \
        crates/yang-base/src/table/__tests__/entity_test.rs \
        crates/yang-base/src/table/__tests__/mod.rs
git commit -m "feat(yang-base): 引入 TableEntity/WhereOp/SqlCondition 类型骨架

为 Action 系统类型化 (H-1) 提供基础类型契约。本步无派生宏，
通过手写 TestUser 实现验证设计自洽。"
```

---

### Task 3：新增 `TypedHandler`/`TypedAction`/`Action` 三层 trait + 改造 ActionContext

**目的：** spec §3。引入新 trait 层 + blanket impl + `ActionContext::extract_input`。**旧 `Action` trait 暂时保留**，新 trait 命名为 `TypedHandler`/`TypedAction`/`DynAction`（临时名）以避免冲突，等步骤 6 router 改造完成后再把 `DynAction` 重命名回 `Action`。

**Files:**
- Create: `crates/yang-base/src/action/typed.rs`
- Create: `crates/yang-base/src/action/meta.rs`
- Modify: `crates/yang-base/src/action/context.rs`
- Modify: `crates/yang-base/src/action/mod.rs`
- Test: `crates/yang-base/src/action/__tests__/typed_test.rs`

- [ ] **Step 3.1：在 `mod.rs` 加新模块声明**

修改 `crates/yang-base/src/action/mod.rs`，添加：

```rust
pub mod typed;
pub mod meta;

pub use typed::{TypedHandler, TypedAction, DynAction};
pub use meta::ActionMeta;
```

- [ ] **Step 3.2：创建 `meta.rs`**

文件：`crates/yang-base/src/action/meta.rs`

```rust
//! Action 运行时元信息聚合

use crate::action::action_trait::Permission;

/// 单个 Action 的静态元信息聚合体。
///
/// 由 `#[derive(Action)]` 在 `__meta_static()` 里通过 `OnceLock` 一次性构造。
pub struct ActionMeta {
    pub name: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub permissions: &'static [Permission],
    pub is_public: bool,
    pub input_schema: &'static schemars::schema::RootSchema,
    pub output_schema: &'static schemars::schema::RootSchema,
}
```

- [ ] **Step 3.3：创建 `typed.rs`**

文件：`crates/yang-base/src/action/typed.rs`

```rust
//! 类型化 Action 三层 trait
//!
//! - `TypedHandler`：用户唯一手写的 trait，处理 `Input → Output`
//! - `TypedAction`：元信息层（由 `#[derive(Action)]` 派生）
//! - `DynAction`：object-safe 擦除层，存入 router 派发
//!
//! 通过 blanket `impl<T: TypedAction> DynAction for T` 自动桥接。

use crate::action::{ActionContext, ApiResponse, meta::ActionMeta};
use crate::error::BaseError;
use async_trait::async_trait;

/// 用户业务逻辑 trait。`Input`/`Output` 是编译期契约。
#[async_trait]
pub trait TypedHandler: Send + Sync + 'static {
    type Input: serde::de::DeserializeOwned + schemars::JsonSchema + Send;
    type Output: serde::Serialize + schemars::JsonSchema + Send;

    async fn handle(&self, ctx: ActionContext, input: Self::Input) -> Result<Self::Output, BaseError>;
}

/// 元信息层。由 `#[derive(Action)]` 自动实现；用户不手写。
pub trait TypedAction: TypedHandler {
    fn name(&self) -> &'static str;
    fn display_name(&self) -> &'static str { self.name() }
    fn description(&self) -> &'static str { "" }
    fn permissions(&self) -> &'static [crate::action::action_trait::Permission] { &[] }
    fn is_public(&self) -> bool { false }
    fn input_schema(&self) -> &'static schemars::schema::RootSchema;
    fn output_schema(&self) -> &'static schemars::schema::RootSchema;
    fn meta_static(&self) -> &'static ActionMeta;
}

/// 擦除层：router 存 `Arc<dyn DynAction>` 派发。
#[async_trait]
pub trait DynAction: Send + Sync + 'static {
    async fn dispatch(&self, ctx: ActionContext) -> Result<ApiResponse, BaseError>;
    fn meta(&self) -> &'static ActionMeta;
}

/// Blanket 桥接：所有 TypedAction 自动是 DynAction。
#[async_trait]
impl<T: TypedAction> DynAction for T {
    async fn dispatch(&self, ctx: ActionContext) -> Result<ApiResponse, BaseError> {
        let input: T::Input = ctx.extract_input()?;
        let output = self.handle(ctx, input).await?;
        ApiResponse::success(output, "成功")
    }

    fn meta(&self) -> &'static ActionMeta {
        TypedAction::meta_static(self)
    }
}
```

- [ ] **Step 3.4：改造 `ActionContext`：移除 param 方法，新增 `extract_input`**

修改 `crates/yang-base/src/action/context.rs`：

1. 把 `pub fn param`, `pub fn param_optional`, `pub fn param_or_default`, `pub fn query_param` 全部删除。
2. 在 impl block 末尾增加：

```rust
impl ActionContext {
    /// 把整个请求体反序列化为 `I`。新类型化 Action 系统的统一参数提取入口。
    ///
    /// # 错误
    /// - `BaseError::ParamInvalid("body", ...)`: 反序列化失败（缺字段/类型错/未知字段等）
    pub fn extract_input<I: serde::de::DeserializeOwned>(&self) -> Result<I, BaseError> {
        let value = serde_json::Value::Object(self.request.body.clone());
        serde_json::from_value(value).map_err(|e| {
            BaseError::ParamInvalid("body".to_string(), e.to_string())
        })
    }
}
```

3. 检查文件顶部 import：删除不再用到的 `DeserializeOwned`、`FromStr` 等。

> **注意：** 旧 builtin（add/get/put/del/select/table）依赖 `ctx.param(...)`。本步删除后它们会编译失败。**对策**：把旧 builtin 文件整体加 `#![cfg(any())]` 临时禁用（让它们不参与编译），等步骤 5 重写。这是干净的过渡——见 step 3.6。

- [ ] **Step 3.5：写 typed.rs 的单元测试**

文件：`crates/yang-base/src/action/__tests__/typed_test.rs`（新建）

```rust
use crate::action::{ActionContext, typed::*, meta::ActionMeta, Request};
use crate::action::action_trait::Permission;
use crate::error::BaseError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};

#[derive(Deserialize, schemars::JsonSchema)]
struct EchoInput { msg: String }

#[derive(Serialize, schemars::JsonSchema)]
struct EchoOutput { echoed: String }

struct EchoAction;

#[async_trait]
impl TypedHandler for EchoAction {
    type Input = EchoInput;
    type Output = EchoOutput;
    async fn handle(&self, _ctx: ActionContext, input: EchoInput) -> Result<EchoOutput, BaseError> {
        Ok(EchoOutput { echoed: input.msg })
    }
}

impl TypedAction for EchoAction {
    fn name(&self) -> &'static str { "echo" }
    fn input_schema(&self) -> &'static schemars::schema::RootSchema {
        static S: OnceLock<schemars::schema::RootSchema> = OnceLock::new();
        S.get_or_init(|| schemars::schema_for!(EchoInput))
    }
    fn output_schema(&self) -> &'static schemars::schema::RootSchema {
        static S: OnceLock<schemars::schema::RootSchema> = OnceLock::new();
        S.get_or_init(|| schemars::schema_for!(EchoOutput))
    }
    fn meta_static(&self) -> &'static ActionMeta {
        static M: OnceLock<ActionMeta> = OnceLock::new();
        M.get_or_init(|| ActionMeta {
            name: "echo",
            display_name: "echo",
            description: "",
            permissions: &[],
            is_public: false,
            input_schema: Box::leak(Box::new(schemars::schema_for!(EchoInput))),
            output_schema: Box::leak(Box::new(schemars::schema_for!(EchoOutput))),
        })
    }
}

fn make_ctx(body_json: serde_json::Value) -> ActionContext {
    let map = body_json.as_object().cloned().unwrap_or_default();
    let request = Request::new("echo".to_string()).with_body(map);
    // GlobalTools 在测试中可能未初始化，这里使用一个 stub 构造路径。
    // 如 GlobalTools::stub_for_test 不存在则按现有测试模式构造。
    let tools = Arc::new(crate::action::GlobalTools::default());
    ActionContext::new(request, tools)
}

#[tokio::test]
async fn test_blanket_dispatch_roundtrip() {
    let ctx = make_ctx(serde_json::json!({"msg": "hi"}));
    let action: &dyn DynAction = &EchoAction;
    let response = action.dispatch(ctx).await.expect("dispatch ok");
    assert_eq!(response.code, 0);
    let data = response.data.unwrap();
    assert_eq!(data["echoed"], "hi");
}

#[tokio::test]
async fn test_extract_input_missing_field() {
    let ctx = make_ctx(serde_json::json!({}));
    let result: Result<EchoInput, _> = ctx.extract_input();
    assert!(result.is_err());
    if let Err(BaseError::ParamInvalid(field, _)) = result {
        assert_eq!(field, "body");
    } else {
        panic!("expected ParamInvalid");
    }
}

#[tokio::test]
async fn test_meta_accessible_through_dyn() {
    let action: &dyn DynAction = &EchoAction;
    assert_eq!(action.meta().name, "echo");
    assert!(!action.meta().is_public);
}
```

注：`GlobalTools::default()` / `Request::new(...).with_body(...)` 如果实际构造方式不同，参考 `crates/yang-base/src/action/__tests__/context_test.rs` 现有 stub 模式。

- [ ] **Step 3.6：暂禁旧 builtin（让 crate 编译通过）**

在以下每个文件的最顶部加 `#![cfg(any())]`（永远求值为 false 的 cfg）：

- `crates/yang-base/src/action/builtin/add.rs`
- `crates/yang-base/src/action/builtin/get.rs`
- `crates/yang-base/src/action/builtin/put.rs`
- `crates/yang-base/src/action/builtin/del.rs`
- `crates/yang-base/src/action/builtin/select.rs`
- `crates/yang-base/src/action/builtin/table.rs`

修改 `crates/yang-base/src/action/builtin/mod.rs`，把 `pub use` 全部注释，并在顶部加：

```rust
//! 暂禁——步骤 5 之后用新类型化 builtin 替换
#![allow(dead_code, unused_imports)]
```

`crates/yang-base/src/router/module_router.rs::register_builtin_actions` 内部如果直接引用了旧 builtin，把整个方法体改为：

```rust
pub fn register_builtin_actions(self) -> Result<Self, BaseError> {
    // 步骤 5 之后用 table_typed::<T>() 替换；此方法暂废
    Err(BaseError::Unknown("旧 register_builtin_actions 在 H-1 重构期间禁用，请使用 table_typed::<T>()".into()))
}
```

- [ ] **Step 3.7：编译并跑测试**

```bash
cargo build --workspace
cargo test --lib -p yang-base action::__tests__::typed_test
cargo test --lib -p yang-base   # 跑全套，确认旧 builtin 禁用没影响别处
```

期望：全 PASS。可能需要修一些下游测试（依赖 `ctx.param`、依赖旧 builtin 的测试），把这些测试也临时加 `#[ignore = "H-1 重构期间停用"]`，并在 commit message 中列清单。

- [ ] **Step 3.8：commit**

```bash
git add -A crates/yang-base/src/action/
git commit -m "feat(yang-base): 引入 TypedHandler/TypedAction/DynAction 三层 trait

- ActionContext 新增 extract_input；移除 param/param_optional/param_or_default/query_param
- 旧 builtin 与依赖 param 的测试通过 cfg(any())/ignore 暂禁，步骤 5 重写
- 单测覆盖：blanket dispatch、extract_input 错误传播、dyn meta 访问"
```

---

### Task 4：新建 `yang-base-derive` proc-macro crate + 实现 `#[derive(TableEntity)]`

**目的：** spec §11 步骤 3。把步骤 2 的手写 TestUser 实现替换为派生宏自动生成。

**Files:**
- Create: `crates/yang-base-derive/Cargo.toml`
- Create: `crates/yang-base-derive/src/lib.rs`
- Create: `crates/yang-base-derive/src/table_entity.rs`
- Create: `crates/yang-base-derive/src/util.rs`
- Modify: `Cargo.toml`（workspace 加 syn/quote/darling/proc-macro-error/proc-macro2）
- Modify: `crates/yang-base/Cargo.toml`（dep 加 yang-base-derive）
- Modify: `crates/yang-base/src/lib.rs`（re-export 派生宏）
- Modify: `crates/yang-base/src/table/__tests__/entity_test.rs`（改用派生宏）

- [ ] **Step 4.1：workspace 加派生宏依赖**

修改 `Cargo.toml`，`[workspace.dependencies]` 添加：

```toml
# 派生宏基础设施
proc-macro2 = "1.0"
syn = { version = "2.0", features = ["full", "extra-traits"] }
quote = "1.0"
darling = "0.20"
proc-macro-error = "1.0"
```

- [ ] **Step 4.2：创建派生 crate 目录与 Cargo.toml**

```bash
mkdir -p crates/yang-base-derive/src
```

文件：`crates/yang-base-derive/Cargo.toml`

```toml
[package]
name = "yang-base-derive"
version = "0.1.0"
edition = "2021"
description = "yang-base 类型化派生宏（TableEntity / Action）"
license = "MIT OR Apache-2.0"

[lib]
proc-macro = true

[lints]
workspace = true

[dependencies]
proc-macro2 = { workspace = true }
syn = { workspace = true }
quote = { workspace = true }
darling = { workspace = true }
proc-macro-error = { workspace = true }
```

workspace 已含 `members = ["crates/*"]`，无需修改 workspace 成员列表。

- [ ] **Step 4.3：创建 util.rs（属性解析与类型映射）**

文件：`crates/yang-base-derive/src/util.rs`

```rust
//! 派生宏共用工具

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{GenericArgument, PathArguments, Type, TypePath};

/// 把 Rust 字段类型映射到 FieldType 构造代码。
/// `Option<T>` 解包为 T 并由调用方设置 required=false。
pub fn rust_type_to_field_type(ty: &Type, max_length: usize) -> TokenStream {
    let (inner, _is_option) = unwrap_option(ty);
    let path = match inner {
        Type::Path(TypePath { path, .. }) => path,
        _ => return quote! { ::yang_base::table::FieldType::Json },
    };
    let last = match path.segments.last() {
        Some(s) => s.ident.to_string(),
        None => return quote! { ::yang_base::table::FieldType::Json },
    };
    match last.as_str() {
        "i32" | "u32" => quote! { ::yang_base::table::FieldType::Integer },
        "i64" | "u64" => quote! { ::yang_base::table::FieldType::BigInt },
        "f32" => quote! { ::yang_base::table::FieldType::Float },
        "f64" => quote! { ::yang_base::table::FieldType::Double },
        "bool" => quote! { ::yang_base::table::FieldType::Boolean },
        "String" => quote! { ::yang_base::table::FieldType::String { max_length: #max_length } },
        "NaiveDate" => quote! { ::yang_base::table::FieldType::Date },
        "NaiveDateTime" => quote! { ::yang_base::table::FieldType::DateTime },
        "DateTime" => quote! { ::yang_base::table::FieldType::Timestamp },
        "Value" => quote! { ::yang_base::table::FieldType::Json },
        _ => quote! { ::yang_base::table::FieldType::Json },   // fallback：用户自定义类型默认按 JSON 处理
    }
}

/// 判断是否是 String 字段（用于决定是否生成 Like 变体）。
pub fn is_string_type(ty: &Type) -> bool {
    let (inner, _) = unwrap_option(ty);
    if let Type::Path(TypePath { path, .. }) = inner {
        path.segments.last().map(|s| s.ident == "String").unwrap_or(false)
    } else {
        false
    }
}

/// 判断 `Option<T>`，返回 (实际类型, 是否 Option)。
pub fn unwrap_option(ty: &Type) -> (&Type, bool) {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(seg) = path.segments.last() {
            if seg.ident == "Option" {
                if let PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        return (inner, true);
                    }
                }
            }
        }
    }
    (ty, false)
}

/// 把 snake_case 转 PascalCase（生成枚举变体名）。
pub fn pascal_case(s: &str) -> String {
    let mut out = String::new();
    let mut cap = true;
    for c in s.chars() {
        if c == '_' { cap = true; continue; }
        if cap { out.extend(c.to_uppercase()); cap = false; } else { out.push(c); }
    }
    out
}
```

- [ ] **Step 4.4：创建 table_entity.rs（派生实现）**

文件：`crates/yang-base-derive/src/table_entity.rs`

```rust
use crate::util::{is_string_type, pascal_case, rust_type_to_field_type, unwrap_option};
use darling::{FromDeriveInput, FromField};
use proc_macro2::TokenStream;
use proc_macro_error::abort;
use quote::{format_ident, quote};
use syn::{DeriveInput, Type};

#[derive(FromDeriveInput)]
#[darling(attributes(table))]
struct TableOpts {
    name: String,
    #[darling(default)]
    display_name: Option<String>,
    #[darling(default)]
    soft_delete: Option<String>,
}

#[derive(FromField)]
#[darling(attributes(entity))]
struct FieldOpts {
    ident: Option<syn::Ident>,
    ty: syn::Type,
    #[darling(default)]
    primary_key: bool,
    #[darling(default)]
    max_length: Option<usize>,
    #[darling(default)]
    unique: bool,
    #[darling(default)]
    required: Option<bool>,
    #[darling(default)]
    column: Option<String>,
    #[darling(default)]
    skip: bool,
}

pub fn expand(input: DeriveInput) -> TokenStream {
    let struct_name = input.ident.clone();
    let opts = match TableOpts::from_derive_input(&input) {
        Ok(o) => o,
        Err(e) => return e.write_errors(),
    };

    let fields = match &input.data {
        syn::Data::Struct(s) => &s.fields,
        _ => abort!(input, "#[derive(TableEntity)] 只支持 struct"),
    };

    let mut field_opts: Vec<(String, FieldOpts)> = Vec::new();
    for f in fields {
        let opt = match FieldOpts::from_field(f) {
            Ok(o) => o,
            Err(e) => return e.write_errors(),
        };
        if opt.skip { continue; }
        let name = opt.ident.as_ref()
            .map(|i| i.to_string())
            .unwrap_or_else(|| abort!(f, "TableEntity 派生仅支持具名字段"));
        field_opts.push((name, opt));
    }

    // 找主键
    let pk_idx = field_opts.iter().position(|(_, o)| o.primary_key)
        .unwrap_or_else(|| abort!(input, "TableEntity 必须有一个字段标注 #[entity(primary_key)]"));
    let pk_count = field_opts.iter().filter(|(_, o)| o.primary_key).count();
    if pk_count > 1 {
        abort!(input, "TableEntity 只能有一个主键");
    }
    let pk_name = field_opts[pk_idx].0.clone();
    let pk_type = field_opts[pk_idx].1.ty.clone();
    let pk_column = field_opts[pk_idx].1.column.clone().unwrap_or_else(|| pk_name.clone());

    let table_name = opts.name.clone();
    let display_name = opts.display_name.unwrap_or_else(|| table_name.clone());

    // 派生 Field 枚举
    let field_enum_name = format_ident!("{}Field", struct_name);
    let field_variants: Vec<(syn::Ident, String, syn::Type, bool)> = field_opts.iter().map(|(n, o)| {
        let column = o.column.clone().unwrap_or_else(|| n.clone());
        let variant = format_ident!("{}", pascal_case(n));
        (variant, column, o.ty.clone(), is_string_type(&o.ty))
    }).collect();

    let field_variant_idents: Vec<_> = field_variants.iter().map(|(v, _, _, _)| v.clone()).collect();
    let field_columns: Vec<_> = field_variants.iter().map(|(_, c, _, _)| c.clone()).collect();

    // 派生 WhereCond 枚举
    let where_enum_name = format_ident!("{}Where", struct_name);
    let where_variants: Vec<TokenStream> = field_variants.iter().map(|(v, _, ty, is_str)| {
        let inner_ty = unwrap_option(ty).0.clone();
        if *is_str {
            quote! { #v(::yang_base::table::StringWhereOp) }
        } else {
            quote! { #v(::yang_base::table::WhereOp<#inner_ty>) }
        }
    }).collect();

    let where_match_arms: Vec<TokenStream> = field_variants.iter().map(|(v, column, _, _)| {
        let col_lit = column.as_str();
        quote! { Self::#v(op) => op.to_sql_condition(#col_lit) }
    }).collect();

    // 派生 TableConfig 构造代码
    let config_fields: Vec<TokenStream> = field_opts.iter().map(|(n, o)| {
        let column = o.column.clone().unwrap_or_else(|| n.clone());
        let ft = rust_type_to_field_type(&o.ty, o.max_length.unwrap_or(255));
        let (_inner, is_option) = unwrap_option(&o.ty);
        let required = o.required.unwrap_or(!is_option);
        quote! {
            config = config.field(
                ::yang_base::table::FieldConfig::new(#column, #ft).required(#required)
            );
        }
    }).collect();

    let unique_indexes: Vec<TokenStream> = field_opts.iter()
        .filter(|(_, o)| o.unique)
        .map(|(n, o)| {
            let column = o.column.clone().unwrap_or_else(|| n.clone());
            quote! { config = config.add_unique_index(::yang_base::table::IndexConfig::new(vec![#column.to_string()])); }
        }).collect();

    // 拼装最终输出
    quote! {
        // ===== Field 枚举 =====
        #[derive(::core::fmt::Debug, ::core::clone::Clone, ::core::marker::Copy,
                 ::core::cmp::PartialEq, ::core::cmp::Eq, ::core::hash::Hash,
                 ::serde::Serialize, ::serde::Deserialize, ::schemars::JsonSchema)]
        #[serde(rename_all = "snake_case")]
        pub enum #field_enum_name { #( #field_variant_idents ),* }

        impl ::yang_base::table::AsColumnName for #field_enum_name {
            fn column_name(&self) -> &'static str {
                match self {
                    #( Self::#field_variant_idents => #field_columns ),*
                }
            }
        }

        // ===== WhereCond 枚举 =====
        #[derive(::core::fmt::Debug, ::serde::Deserialize, ::schemars::JsonSchema)]
        #[serde(tag = "field", content = "cond", rename_all = "snake_case")]
        pub enum #where_enum_name {
            #( #where_variants ),*
        }

        impl ::yang_base::table::IntoSqlCondition for #where_enum_name {
            fn into_sql_condition(self) -> ::yang_base::table::SqlCondition {
                match self { #( #where_match_arms ),* }
            }
        }

        // ===== TableEntity 实现 =====
        impl ::yang_base::table::TableEntity for #struct_name {
            type Pk = #pk_type;
            type Field = #field_enum_name;
            type WhereCond = #where_enum_name;
            const TABLE_NAME: &'static str = #table_name;
            const PK_FIELD: &'static str = #pk_column;

            fn table_config() -> &'static ::yang_base::table::TableConfig {
                static CONFIG: ::std::sync::OnceLock<::yang_base::table::TableConfig> = ::std::sync::OnceLock::new();
                CONFIG.get_or_init(|| {
                    let mut config = ::yang_base::table::TableConfig::new(#table_name);
                    config = config.primary_key(#pk_column);
                    config = config.display_name(#display_name);
                    #( #config_fields )*
                    #( #unique_indexes )*
                    config
                })
            }
        }
    }
}
```

> 注意：如果 `TableConfig::add_unique_index` / `display_name` 方法不存在，先在 yang-base 端加上（一行 setter），保持本派生宏生成的代码可编译。

- [ ] **Step 4.5：创建 lib.rs 入口**

文件：`crates/yang-base-derive/src/lib.rs`

```rust
//! yang-base 派生宏入口。

use proc_macro::TokenStream;
use proc_macro_error::proc_macro_error;
use syn::{parse_macro_input, DeriveInput};

mod table_entity;
mod util;
// mod action;  // 步骤 5 引入

#[proc_macro_derive(TableEntity, attributes(table, entity))]
#[proc_macro_error]
pub fn derive_table_entity(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    table_entity::expand(input).into()
}
```

- [ ] **Step 4.6：yang-base 引入派生 crate 并 re-export**

修改 `crates/yang-base/Cargo.toml` 的 `[dependencies]`，添加：

```toml
yang-base-derive = { path = "../yang-base-derive", version = "0.1.0" }
```

修改 `crates/yang-base/src/lib.rs`，添加导出：

```rust
pub use yang_base_derive::TableEntity;
```

- [ ] **Step 4.7：补全 TableConfig 缺失的 setter**

检查 `crates/yang-base/src/table/table_config.rs`：

- `display_name(...)` 是否存在 chain setter？如不存在，添加：

```rust
pub fn display_name(mut self, name: impl Into<String>) -> Self {
    self.display_name = name.into();
    self
}
```

- `add_unique_index(...)` 是否存在？如不存在，添加：

```rust
pub fn add_unique_index(mut self, index: IndexConfig) -> Self {
    self.unique_indexes.push(index);
    self
}
```

- [ ] **Step 4.8：把步骤 2 的手写 TestUser 改为派生**

修改 `crates/yang-base/src/table/__tests__/entity_test.rs`：

把整段手写的 enum/impl 块删除，替换为：

```rust
use serde::{Deserialize, Serialize};
use yang_base_derive::TableEntity;  // 或 use crate as yang_base; 看是否需要别名（见 step 4.9）

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema, sqlx::FromRow, TableEntity)]
#[table(name = "test_users")]
pub struct TestUser {
    #[entity(primary_key)]
    pub id: i64,
    #[entity(max_length = 50, unique)]
    pub username: String,
}
```

保留所有原 `#[test]` 函数不变（它们应在派生后的代码上同样通过）。

- [ ] **Step 4.9：处理 crate 内自引用问题**

派生宏内部用 `::yang_base::table::...` 路径——但 yang-base 自己的内部测试里这条路径不存在（自己不能用 `::yang_base`）。两种处理：

1. **简单方案：** 在 `yang-base/src/lib.rs` 顶部加：
   ```rust
   extern crate self as yang_base;
   ```
   这样 crate 内部测试也能用 `::yang_base::...` 路径。

2. **替代：** 让派生宏接受 `#[table(crate = "...")]` 可配置 crate 名。本计划采用方案 1（简单且业界惯例）。

加好后重跑编译。

- [ ] **Step 4.10：跑测试**

```bash
cargo build --workspace
cargo test --lib -p yang-base table::__tests__::entity_test
```

期望：步骤 2 写的 5 个测试在派生宏生成的代码上全部 PASS。

- [ ] **Step 4.11：commit**

```bash
git add Cargo.toml crates/yang-base-derive/ crates/yang-base/Cargo.toml \
        crates/yang-base/src/lib.rs crates/yang-base/src/table/table_config.rs \
        crates/yang-base/src/table/__tests__/entity_test.rs
git commit -m "feat(yang-base-derive): 新增 #[derive(TableEntity)] 派生宏

- 自动生成 Field/WhereCond 枚举 + TableEntity 实现 + 运行时 TableConfig
- TestUser 例子从手写实现切换到派生，原有 5 个单测全部通过
- 补全 TableConfig::display_name 与 add_unique_index setter"
```

---

### Task 5：实现 `#[derive(Action)]` 派生宏

**目的：** spec §11 步骤 4。

**Files:**
- Create: `crates/yang-base-derive/src/action.rs`
- Modify: `crates/yang-base-derive/src/lib.rs`
- Modify: `crates/yang-base/src/lib.rs`（re-export `Action` 派生宏）
- Test: `crates/yang-base/src/action/__tests__/derive_action_test.rs`

- [ ] **Step 5.1：写 action.rs**

文件：`crates/yang-base-derive/src/action.rs`

```rust
use darling::{FromDeriveInput, FromMeta};
use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

#[derive(FromDeriveInput)]
#[darling(attributes(action))]
struct ActionOpts {
    name: String,
    #[darling(default)]
    display_name: Option<String>,
    #[darling(default)]
    description: Option<String>,
    #[darling(default)]
    public: bool,
    #[darling(default)]
    permissions: Option<PermissionList>,
}

#[derive(Debug, Default)]
struct PermissionList(Vec<String>);

impl FromMeta for PermissionList {
    fn from_list(items: &[darling::ast::NestedMeta]) -> darling::Result<Self> {
        let mut out = Vec::new();
        for item in items {
            if let darling::ast::NestedMeta::Lit(syn::Lit::Str(s)) = item {
                out.push(s.value());
            } else {
                return Err(darling::Error::custom("permissions 项必须是字符串字面量"));
            }
        }
        Ok(PermissionList(out))
    }
}

pub fn expand(input: DeriveInput) -> TokenStream {
    let opts = match ActionOpts::from_derive_input(&input) {
        Ok(o) => o,
        Err(e) => return e.write_errors(),
    };
    let struct_name = input.ident;
    let name = opts.name.clone();
    let display_name = opts.display_name.unwrap_or_else(|| name.clone());
    let description = opts.description.unwrap_or_default();
    let is_public = opts.public;
    let perms: Vec<String> = opts.permissions.unwrap_or_default().0;

    let perm_consts: Vec<TokenStream> = perms.iter().map(|p| {
        quote! { ::yang_base::action::action_trait::Permission::from_static(#p) }
    }).collect();

    quote! {
        impl ::yang_base::action::TypedAction for #struct_name {
            fn name(&self) -> &'static str { #name }
            fn display_name(&self) -> &'static str { #display_name }
            fn description(&self) -> &'static str { #description }
            fn is_public(&self) -> bool { #is_public }

            fn permissions(&self) -> &'static [::yang_base::action::action_trait::Permission] {
                static PERMS: ::std::sync::OnceLock<::std::vec::Vec<::yang_base::action::action_trait::Permission>> = ::std::sync::OnceLock::new();
                PERMS.get_or_init(|| vec![ #( #perm_consts ),* ])
            }

            fn input_schema(&self) -> &'static ::schemars::schema::RootSchema {
                static S: ::std::sync::OnceLock<::schemars::schema::RootSchema> = ::std::sync::OnceLock::new();
                S.get_or_init(|| ::schemars::schema_for!(<Self as ::yang_base::action::TypedHandler>::Input))
            }

            fn output_schema(&self) -> &'static ::schemars::schema::RootSchema {
                static S: ::std::sync::OnceLock<::schemars::schema::RootSchema> = ::std::sync::OnceLock::new();
                S.get_or_init(|| ::schemars::schema_for!(<Self as ::yang_base::action::TypedHandler>::Output))
            }

            fn meta_static(&self) -> &'static ::yang_base::action::ActionMeta {
                static M: ::std::sync::OnceLock<::yang_base::action::ActionMeta> = ::std::sync::OnceLock::new();
                M.get_or_init(|| ::yang_base::action::ActionMeta {
                    name: #name,
                    display_name: #display_name,
                    description: #description,
                    permissions: <Self as ::yang_base::action::TypedAction>::permissions(unsafe {
                        // OnceLock 安全：仅用 self-less 访问 permissions()
                        &*(::std::ptr::null::<Self>())
                    }),
                    is_public: #is_public,
                    input_schema: <Self as ::yang_base::action::TypedAction>::input_schema(unsafe {
                        &*(::std::ptr::null::<Self>())
                    }),
                    output_schema: <Self as ::yang_base::action::TypedAction>::output_schema(unsafe {
                        &*(::std::ptr::null::<Self>())
                    }),
                })
            }
        }
    }
}
```

> **重要：** 上面 `meta_static` 里用 `&*null` 是不可行的 UB。改为生成一个 `Self::__meta_init()` 关联函数，内部直接复制字符串字面量 + 调用 `schema_for!` 字面调用（不通过 trait method）。重写为：

```rust
        impl ::yang_base::action::TypedAction for #struct_name {
            fn name(&self) -> &'static str { #name }
            fn display_name(&self) -> &'static str { #display_name }
            fn description(&self) -> &'static str { #description }
            fn is_public(&self) -> bool { #is_public }

            fn permissions(&self) -> &'static [::yang_base::action::action_trait::Permission] {
                static PERMS: ::std::sync::OnceLock<::std::vec::Vec<::yang_base::action::action_trait::Permission>> = ::std::sync::OnceLock::new();
                PERMS.get_or_init(|| vec![ #( #perm_consts ),* ])
            }

            fn input_schema(&self) -> &'static ::schemars::schema::RootSchema {
                <Self as ::yang_base::action::TypedAction>::__input_schema_static()
            }

            fn output_schema(&self) -> &'static ::schemars::schema::RootSchema {
                <Self as ::yang_base::action::TypedAction>::__output_schema_static()
            }

            fn meta_static(&self) -> &'static ::yang_base::action::ActionMeta {
                static M: ::std::sync::OnceLock<::yang_base::action::ActionMeta> = ::std::sync::OnceLock::new();
                M.get_or_init(|| ::yang_base::action::ActionMeta {
                    name: #name,
                    display_name: #display_name,
                    description: #description,
                    permissions: {
                        static PERMS: ::std::sync::OnceLock<::std::vec::Vec<::yang_base::action::action_trait::Permission>> = ::std::sync::OnceLock::new();
                        PERMS.get_or_init(|| vec![ #( #perm_consts ),* ])
                    },
                    is_public: #is_public,
                    input_schema: {
                        static S: ::std::sync::OnceLock<::schemars::schema::RootSchema> = ::std::sync::OnceLock::new();
                        S.get_or_init(|| ::schemars::schema_for!(<#struct_name as ::yang_base::action::TypedHandler>::Input))
                    },
                    output_schema: {
                        static S: ::std::sync::OnceLock<::schemars::schema::RootSchema> = ::std::sync::OnceLock::new();
                        S.get_or_init(|| ::schemars::schema_for!(<#struct_name as ::yang_base::action::TypedHandler>::Output))
                    },
                })
            }
        }
```

把这个版本作为最终生成代码（删掉前一个含 `unsafe` 的草稿）。注意每个 `static` 都在独立 block 中，避免 `OnceLock` 同名冲突。

- [ ] **Step 5.2：在 lib.rs 注册派生宏**

修改 `crates/yang-base-derive/src/lib.rs`：

```rust
mod action;

#[proc_macro_derive(Action, attributes(action))]
#[proc_macro_error]
pub fn derive_action(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    action::expand(input).into()
}
```

- [ ] **Step 5.3：在 yang-base 导出**

修改 `crates/yang-base/src/lib.rs`：

```rust
pub use yang_base_derive::{TableEntity, Action};
```

- [ ] **Step 5.4：写派生测试**

文件：`crates/yang-base/src/action/__tests__/derive_action_test.rs`

```rust
use crate::action::{ActionContext, TypedHandler, TypedAction, DynAction, Request};
use crate::error::BaseError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use yang_base_derive::Action;

#[derive(Deserialize, schemars::JsonSchema)]
struct PingInput { msg: String }

#[derive(Serialize, schemars::JsonSchema)]
struct PingOutput { reply: String }

#[derive(Action)]
#[action(name = "ping", public, display_name = "心跳", description = "测试连通性",
         permissions("system:ping"))]
pub struct PingAction;

#[async_trait]
impl TypedHandler for PingAction {
    type Input = PingInput;
    type Output = PingOutput;
    async fn handle(&self, _ctx: ActionContext, input: PingInput) -> Result<PingOutput, BaseError> {
        Ok(PingOutput { reply: format!("pong: {}", input.msg) })
    }
}

#[test]
fn test_derive_action_meta_correct() {
    let a = PingAction;
    assert_eq!(a.name(), "ping");
    assert_eq!(a.display_name(), "心跳");
    assert_eq!(a.description(), "测试连通性");
    assert!(a.is_public());
    let perms = a.permissions();
    assert_eq!(perms.len(), 1);
    assert_eq!(perms[0].name(), "system:ping");
}

#[test]
fn test_derive_action_meta_static_dyn() {
    let a: &dyn DynAction = &PingAction;
    let m = a.meta();
    assert_eq!(m.name, "ping");
    assert!(m.is_public);
    // schema 非空
    let v = serde_json::to_value(m.input_schema).unwrap();
    assert!(v.is_object());
}
```

注册测试模块（在 `crates/yang-base/src/action/__tests__/mod.rs` 加 `mod derive_action_test;`）。

- [ ] **Step 5.5：跑测试**

```bash
cargo test --lib -p yang-base action::__tests__::derive_action_test
```

期望：PASS。

- [ ] **Step 5.6：commit**

```bash
git add -A crates/yang-base-derive/ crates/yang-base/src/
git commit -m "feat(yang-base-derive): 新增 #[derive(Action)] 派生宏

- 解析 #[action(name, display_name, description, public, permissions(...))]
- 自动生成 TypedAction impl + ActionMeta 静态聚合
- 用 OnceLock 惰性生成 input/output schema，全程序仅一次"
```

---

### Task 6：六个内置 Action 重写

**目的：** spec §5.1–§5.6 + §11 步骤 5。

**Files:**
- Modify（实质上是重写）：
  - `crates/yang-base/src/action/builtin/get.rs`
  - `crates/yang-base/src/action/builtin/add.rs`
  - `crates/yang-base/src/action/builtin/put.rs`
  - `crates/yang-base/src/action/builtin/del.rs`
  - `crates/yang-base/src/action/builtin/select.rs`
  - `crates/yang-base/src/action/builtin/table.rs`
  - `crates/yang-base/src/action/builtin/mod.rs`
- Create: `crates/yang-base/src/action/sql_bridge.rs`

每个 builtin 一个独立 step group（6.1–6.6），按 Get → Add → Del → Put → Select → Table 顺序。

- [ ] **Step 6.0：先实现 sql_bridge.rs（其余 builtin 都依赖）**

文件：`crates/yang-base/src/action/sql_bridge.rs`

```rust
//! 把 SqlCondition 桥接到 TableQuery
#![cfg(feature = "mysql")]

use crate::action::ActionContext;
use crate::error::BaseError;
use crate::table::entity::{SqlCondition, SqlOp, TableEntity};
use crate::table::TableQuery;

pub(crate) fn apply_sql_condition(
    mut q: TableQuery,
    cond: &SqlCondition,
) -> Result<TableQuery, BaseError> {
    q = match cond.op {
        SqlOp::Eq      => q.where_eq(cond.column, cond.params[0].clone())?,
        SqlOp::Ne      => q.where_ne(cond.column, cond.params[0].clone())?,
        SqlOp::Lt      => q.where_lt(cond.column, cond.params[0].clone())?,
        SqlOp::Lte     => q.where_lte(cond.column, cond.params[0].clone())?,
        SqlOp::Gt      => q.where_gt(cond.column, cond.params[0].clone())?,
        SqlOp::Gte     => q.where_gte(cond.column, cond.params[0].clone())?,
        SqlOp::In      => q.where_in(cond.column, cond.params.clone())?,
        SqlOp::NotIn   => q.where_not_in(cond.column, cond.params.clone())?,
        SqlOp::Between => q.where_between(cond.column, cond.params[0].clone(), cond.params[1].clone())?,
        SqlOp::Like    => q.where_like(cond.column,
                            cond.params[0].as_str().unwrap_or("").to_string())?,
        SqlOp::IsNull  => q.where_null(cond.column)?,
        SqlOp::IsNotNull => q.where_not_null(cond.column)?,
    };
    Ok(q)
}

pub(crate) async fn count_with_conditions<T: TableEntity>(
    ctx: &ActionContext,
    conditions: &[SqlCondition],
) -> Result<u64, BaseError> {
    let mut q = ctx.table_query()?;
    for cond in conditions {
        q = apply_sql_condition(q, cond)?;
    }
    q.count().await
}
```

在 `crates/yang-base/src/action/mod.rs` 加 `pub mod sql_bridge;`。

- [ ] **Step 6.1：重写 get.rs（GetAction）**

去掉 `#![cfg(any())]` 暂禁标记，替换为：

```rust
//! GetAction - 根据主键获取单条数据
#![cfg(feature = "mysql")]

use crate::action::{ActionContext, TypedHandler};
use crate::error::BaseError;
use crate::table::TableEntity;
use async_trait::async_trait;
use serde::Deserialize;
use std::marker::PhantomData;
use yang_base_derive::Action;

#[derive(Deserialize, schemars::JsonSchema)]
pub struct GetByPk<PK> { pub id: PK }

#[derive(Action)]
#[action(name = "get", display_name = "获取数据", description = "根据主键获取单条记录")]
pub struct GetAction<T: TableEntity> { _phantom: PhantomData<T> }

impl<T: TableEntity> GetAction<T> {
    pub fn new() -> Self { Self { _phantom: PhantomData } }
}

#[async_trait]
impl<T: TableEntity> TypedHandler for GetAction<T> {
    type Input = GetByPk<T::Pk>;
    type Output = T;
    async fn handle(&self, ctx: ActionContext, input: GetByPk<T::Pk>) -> Result<T, BaseError> {
        let pk_value = serde_json::to_value(&input.id)
            .map_err(|e| BaseError::JsonSerializeFailed(e.to_string()))?;
        let query = ctx.table_query()?.where_eq(T::PK_FIELD, pk_value)?;
        query.fetch_optional::<T>().await?.ok_or_else(||
            BaseError::RecordNotFound(format!(
                "{} 中主键 {} 的记录不存在", T::TABLE_NAME, T::PK_FIELD
            ))
        )
    }
}
```

> **派生宏限制：** `#[derive(Action)]` 当前不处理泛型 `<T: TableEntity>`。需要在 `yang-base-derive/src/action.rs` 的 `expand` 里读取 `input.generics` 并把 `where_clause`/`type_params` 透传到 `impl` 块。回到 Task 5 的代码：

在 action.rs `expand` 末尾，把 `impl ::yang_base::action::TypedAction for #struct_name {` 改为：

```rust
let (impl_g, ty_g, where_clause) = input.generics.split_for_impl();
quote! {
    impl #impl_g ::yang_base::action::TypedAction for #struct_name #ty_g #where_clause {
        ...
    }
}
```

`schema_for!(<Self as TypedHandler>::Input)` 在泛型 impl 里是合法的——schema 会按具体 `T` 实例化生成。

修改 Task 5 已写好的代码，做这个泛型透传。重跑：

```bash
cargo build -p yang-base
```

期望：编译通过。

- [ ] **Step 6.2：写 GetAction 的集成 stub 测试（不联库的纯类型测试）**

文件：`crates/yang-base/src/action/builtin/__tests__/get_test.rs`（新建）

```rust
#![cfg(feature = "mysql")]
use crate::action::{builtin::get::GetAction, TypedHandler, TypedAction};
use crate::table::TableEntity;

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema, sqlx::FromRow, yang_base_derive::TableEntity)]
#[table(name = "test_users")]
struct TestUser {
    #[entity(primary_key)] id: i64,
    name: String,
}

#[test]
fn test_get_action_name_is_get() {
    let a: GetAction<TestUser> = GetAction::new();
    assert_eq!(a.name(), "get");
}

#[test]
fn test_get_action_input_is_get_by_pk_i64() {
    // 编译期断言：Input 应能反序列化 {"id": 42}
    let _: <GetAction<TestUser> as TypedHandler>::Input =
        serde_json::from_value(serde_json::json!({"id": 42})).unwrap();
}
```

注册：`crates/yang-base/src/action/builtin/__tests__/mod.rs` 加 `mod get_test;`（如不存在则创建）。

- [ ] **Step 6.3：跑 GetAction 测试**

```bash
cargo test --lib -p yang-base action::builtin::__tests__::get_test
```

期望：PASS。

- [ ] **Step 6.4：实现 AddAction（同节奏）**

文件 `crates/yang-base/src/action/builtin/add.rs`：

```rust
#![cfg(feature = "mysql")]
use crate::action::{ActionContext, TypedHandler};
use crate::error::BaseError;
use crate::table::TableEntity;
use async_trait::async_trait;
use serde::Serialize;
use std::marker::PhantomData;
use yang_base_derive::Action;

#[derive(Serialize, schemars::JsonSchema)]
pub struct AffectedResult { pub affected: u64 }

#[derive(Action)]
#[action(name = "add", display_name = "新增数据", description = "向表中插入一条记录")]
pub struct AddAction<T: TableEntity> { _phantom: PhantomData<T> }

impl<T: TableEntity> AddAction<T> {
    pub fn new() -> Self { Self { _phantom: PhantomData } }
}

#[async_trait]
impl<T: TableEntity> TypedHandler for AddAction<T> {
    type Input = T;
    type Output = AffectedResult;
    async fn handle(&self, ctx: ActionContext, input: T) -> Result<AffectedResult, BaseError> {
        let value = serde_json::to_value(&input)
            .map_err(|e| BaseError::JsonSerializeFailed(e.to_string()))?;
        let map = match value {
            serde_json::Value::Object(m) => m.into_iter().collect::<std::collections::HashMap<_, _>>(),
            _ => return Err(BaseError::ParamInvalid("body".into(), "实体必须序列化为对象".into())),
        };
        let affected = ctx.table_query()?.insert(map).await?;
        Ok(AffectedResult { affected })
    }
}
```

写对应 stub 测试（参考 6.2 模板）。运行：

```bash
cargo test --lib -p yang-base action::builtin::__tests__::add_test
```

- [ ] **Step 6.5：实现 DelAction**

```rust
#![cfg(feature = "mysql")]
use crate::action::{builtin::{add::AffectedResult, get::GetByPk}, ActionContext, TypedHandler};
use crate::error::BaseError;
use crate::table::TableEntity;
use async_trait::async_trait;
use std::marker::PhantomData;
use yang_base_derive::Action;

#[derive(Action)]
#[action(name = "del", display_name = "删除数据", description = "按主键删除记录")]
pub struct DelAction<T: TableEntity> { _phantom: PhantomData<T> }

impl<T: TableEntity> DelAction<T> {
    pub fn new() -> Self { Self { _phantom: PhantomData } }
}

#[async_trait]
impl<T: TableEntity> TypedHandler for DelAction<T> {
    type Input = GetByPk<T::Pk>;
    type Output = AffectedResult;
    async fn handle(&self, ctx: ActionContext, input: GetByPk<T::Pk>) -> Result<AffectedResult, BaseError> {
        let pk_value = serde_json::to_value(&input.id)
            .map_err(|e| BaseError::JsonSerializeFailed(e.to_string()))?;
        let affected = ctx.table_query()?
            .where_eq(T::PK_FIELD, pk_value)?
            .delete()
            .await?;
        Ok(AffectedResult { affected })
    }
}
```

写 stub 测试，跑 PASS。

- [ ] **Step 6.6：实现 PutAction**

```rust
#![cfg(feature = "mysql")]
use crate::action::{builtin::add::AffectedResult, ActionContext, TypedHandler};
use crate::error::BaseError;
use crate::table::entity::AsColumnName;
use crate::table::TableEntity;
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use std::marker::PhantomData;
use yang_base_derive::Action;

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PutInput<T: TableEntity> {
    pub id: T::Pk,
    /// 字段更新对。JSON 形态: [["username","alice"], ["email","a@b.com"]]
    pub data: Vec<(T::Field, serde_json::Value)>,
}

#[derive(Action)]
#[action(name = "put", display_name = "更新数据", description = "按主键更新指定字段")]
pub struct PutAction<T: TableEntity> { _phantom: PhantomData<T> }

impl<T: TableEntity> PutAction<T> {
    pub fn new() -> Self { Self { _phantom: PhantomData } }
}

#[async_trait]
impl<T: TableEntity> TypedHandler for PutAction<T> {
    type Input = PutInput<T>;
    type Output = AffectedResult;
    async fn handle(&self, ctx: ActionContext, input: PutInput<T>) -> Result<AffectedResult, BaseError> {
        if input.data.is_empty() {
            return Err(BaseError::ParamInvalid("data".into(), "至少需要一个字段".into()));
        }
        let pk_value = serde_json::to_value(&input.id)
            .map_err(|e| BaseError::JsonSerializeFailed(e.to_string()))?;
        let data: HashMap<String, serde_json::Value> = input.data.into_iter()
            .map(|(field, value)| (field.column_name().to_string(), value))
            .collect();
        let affected = ctx.table_query()?
            .where_eq(T::PK_FIELD, pk_value)?
            .update(data)
            .await?;
        Ok(AffectedResult { affected })
    }
}
```

写 stub 测试，跑 PASS。

- [ ] **Step 6.7：实现 SelectAction**

```rust
#![cfg(feature = "mysql")]
use crate::action::{ActionContext, TypedHandler, sql_bridge::{apply_sql_condition, count_with_conditions}};
use crate::error::BaseError;
use crate::table::entity::{AsColumnName, IntoSqlCondition, SqlCondition, TableEntity};
use crate::table::SortOrder;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use yang_base_derive::Action;

fn default_page() -> u32 { 1 }
fn default_page_size() -> u32 { 10 }
fn default_sort_order() -> SortOrder { SortOrder::Asc }

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectQuery<T: TableEntity> {
    #[serde(default = "default_page")] pub page: u32,
    #[serde(default = "default_page_size")] pub page_size: u32,
    #[serde(default)] pub fields: Option<Vec<T::Field>>,
    #[serde(default, rename = "where")] pub where_clause: Vec<T::WhereCond>,
    #[serde(default)] pub order_by: Vec<OrderByItem<T>>,
    #[serde(default)] pub count_total: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct OrderByItem<T: TableEntity> {
    pub field: T::Field,
    #[serde(default = "default_sort_order")] pub direction: SortOrder,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct SelectResult<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub total: Option<u64>,
}

#[derive(Action)]
#[action(name = "select", display_name = "查询列表", description = "分页+多条件 AND 查询")]
pub struct SelectAction<T: TableEntity> { _phantom: PhantomData<T> }

impl<T: TableEntity> SelectAction<T> {
    pub fn new() -> Self { Self { _phantom: PhantomData } }
}

#[async_trait]
impl<T: TableEntity> TypedHandler for SelectAction<T> {
    type Input = SelectQuery<T>;
    type Output = SelectResult<T>;

    async fn handle(&self, ctx: ActionContext, input: SelectQuery<T>)
        -> Result<SelectResult<T>, BaseError>
    {
        if input.page == 0 || input.page_size == 0 || input.page_size > 100 {
            return Err(BaseError::ParamInvalid("page/page_size".into(),
                "page>=1, 1<=page_size<=100".into()));
        }

        let conditions: Vec<SqlCondition> = input.where_clause.into_iter()
            .map(|c| c.into_sql_condition()).collect();

        let total = if input.count_total {
            Some(count_with_conditions::<T>(&ctx, &conditions).await?)
        } else { None };

        let mut q = ctx.table_query()?;
        if let Some(fields) = input.fields {
            let names: Vec<&str> = fields.iter().map(|f| f.column_name()).collect();
            q = q.select_fields(&names)?;
        }
        for cond in &conditions {
            q = apply_sql_condition(q, cond)?;
        }
        for OrderByItem { field, direction } in input.order_by {
            q = q.order_by(field.column_name(), direction)?;
        }
        // 假设 TableQuery 有 paginate(page, page_size).fetch_all 链式 API；
        // 如实际 API 是 set_page/set_page_size + fetch_all，调整为对应形式。
        let items: Vec<T> = q
            .paginate(input.page as usize, input.page_size as usize)?
            .fetch_all::<T>().await?;
        Ok(SelectResult { items, page: input.page, page_size: input.page_size, total })
    }
}
```

> **如果 `paginate` API 不存在**：参考 spec §5.5 注释，使用现有 `TableQuery` 的 page/page_size 设置方式（grep `paginate|page|page_size` in `table_query.rs`），把上面 `paginate(...)` 一行调整为 `with_page(page).with_page_size(page_size)` 之类的实际可用 API。

写 stub 测试（重点：where_clause 的 JSON 反序列化和 SortOrder 的 default）。

- [ ] **Step 6.8：实现 TableAction**

```rust
#![cfg(feature = "mysql")]
use crate::action::{ActionContext, TypedHandler};
use crate::error::BaseError;
use crate::table::TableEntity;
use async_trait::async_trait;
use serde::Serialize;
use std::marker::PhantomData;
use yang_base_derive::Action;

#[derive(Serialize, schemars::JsonSchema)]
pub struct TableSchemaResponse {
    pub table_name: String,
    pub primary_key: String,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
}

#[derive(Action)]
#[action(name = "table", display_name = "表元信息", description = "返回表结构与字段 schema")]
pub struct TableAction<T: TableEntity> { _phantom: PhantomData<T> }

impl<T: TableEntity> TableAction<T> {
    pub fn new() -> Self { Self { _phantom: PhantomData } }
}

#[async_trait]
impl<T: TableEntity> TypedHandler for TableAction<T> {
    type Input = ();
    type Output = TableSchemaResponse;
    async fn handle(&self, _ctx: ActionContext, _input: ()) -> Result<TableSchemaResponse, BaseError> {
        Ok(TableSchemaResponse {
            table_name: T::TABLE_NAME.to_string(),
            primary_key: T::PK_FIELD.to_string(),
            input_schema: serde_json::to_value(schemars::schema_for!(T))
                .map_err(|e| BaseError::JsonSerializeFailed(e.to_string()))?,
            output_schema: serde_json::to_value(schemars::schema_for!(T))
                .map_err(|e| BaseError::JsonSerializeFailed(e.to_string()))?,
        })
    }
}
```

> **空 Input 提示：** `Input = ()` 在 `extract_input::<()>` 时要求请求体反序列化为单元类型。`serde_json::from_value(Value::Object(empty))` 对 `()` **会失败**。要么改 `Input = EmptyInput`（自定义空 struct + `default`），要么在 `ActionContext::extract_input` 里特判 `Self == ()`。本计划采用前者——加：

```rust
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(default)]
pub struct EmptyInput;

impl Default for EmptyInput { fn default() -> Self { Self } }
```

把 `type Input = ()` 改为 `type Input = EmptyInput`。

- [ ] **Step 6.9：更新 builtin/mod.rs**

```rust
//! 内置 CRUD Actions 模块（H-1 类型化后）
#![cfg(feature = "mysql")]

pub mod add;
pub mod del;
pub mod get;
pub mod put;
pub mod select;
pub mod table;

#[cfg(test)]
mod __tests__;

pub use add::{AddAction, AffectedResult};
pub use del::DelAction;
pub use get::{GetAction, GetByPk};
pub use put::{PutAction, PutInput};
pub use select::{SelectAction, SelectQuery, SelectResult, OrderByItem};
pub use table::{TableAction, TableSchemaResponse, EmptyInput};
```

- [ ] **Step 6.10：跑全 lib 测试**

```bash
cargo test --lib -p yang-base
cargo clippy --workspace --all-targets -- -D warnings
```

期望：全 PASS、无 warning。

- [ ] **Step 6.11：commit**

```bash
git add -A crates/yang-base/src/action/
git commit -m "feat(yang-base): 重写六个内置 Action 为泛型类型化版本

- Add/Del/Get/Put/Select/Table 全部基于 TableEntity<T>
- 新增 sql_bridge::apply_sql_condition + count_with_conditions
- 输入输出契约由 TypedHandler::{Input,Output} 编译期固定
- 字段名通过 T::Field 枚举封闭集合保证（无任意字符串拼接）"
```

---

### Task 7：ModuleRouter 集成 + `table_typed::<T>()`

**目的：** spec §6 + §11 步骤 6。

**Files:**
- Modify: `crates/yang-base/src/router/module_router.rs`
- Test: `crates/yang-base/src/router/__tests__/module_router_tests.rs`

- [ ] **Step 7.1：把 `Action` 重命名**

把 `crates/yang-base/src/action/typed.rs` 里 `DynAction` 重命名为 `Action`。同步修改：

- `crates/yang-base/src/action/mod.rs` 里的 `pub use typed::{..., DynAction}` → `pub use typed::{..., Action}`
- 旧的 `action_trait.rs` 里的 `pub trait Action` 重命名为 `pub trait LegacyAction`（暂留至步骤 8 全删）

确保 `cargo build -p yang-base` 通过。

- [ ] **Step 7.2：改写 ModuleRouter 的 actions 存储类型**

`module_router.rs` 内：

```rust
// 旧
actions: HashMap<String, Box<dyn Action>>,
// 改为
actions: HashMap<String, Arc<dyn crate::action::Action>>,
```

把 `register_action` 改为：

```rust
pub fn register<A: crate::action::Action + 'static>(mut self, action: A) -> Self {
    let arc: Arc<dyn crate::action::Action> = Arc::new(action);
    self.actions.insert(arc.meta().name.to_string(), arc);
    self
}
```

- [ ] **Step 7.3：实现 `table_typed::<T>()`**

```rust
impl ModuleRouter {
    #[cfg(feature = "mysql")]
    pub fn table_typed<T: crate::table::TableEntity>(mut self) -> Self {
        use crate::action::builtin::{AddAction, DelAction, GetAction, PutAction, SelectAction, TableAction};
        // 写入 table_config 给 ActionContext 使用
        self.table_config = Some(std::sync::Arc::new(T::table_config().clone()));
        self.register(GetAction::<T>::new())
            .register(AddAction::<T>::new())
            .register(PutAction::<T>::new())
            .register(DelAction::<T>::new())
            .register(SelectAction::<T>::new())
            .register(TableAction::<T>::new())
    }
}
```

- [ ] **Step 7.4：改写 dispatch 用 ActionMeta**

```rust
pub async fn dispatch(
    &self,
    action_name: &str,
    mut context: ActionContext,
) -> Result<ApiResponse, BaseError> {
    let action = self.actions.get(action_name)
        .ok_or_else(|| BaseError::ActionNotFound(action_name.to_string()))?;

    if let Some(table_config) = &self.table_config {
        context = context.with_table_config(table_config.clone());
    }

    let meta = action.meta();
    if !meta.is_public {
        let user = context.user.as_ref()
            .ok_or_else(|| BaseError::Unauthorized("需要登录".into()))?;

        if !self.default_permissions.is_empty()
            && !self.check_permissions(user, &self.default_permissions) {
            return Err(BaseError::PermissionDenied(format!(
                "缺少模块权限: {:?}", self.default_permissions
            )));
        }
        if !meta.permissions.is_empty() {
            let names: Vec<String> = meta.permissions.iter().map(|p| p.name().to_string()).collect();
            if !self.check_permissions(user, &names) {
                return Err(BaseError::PermissionDenied(format!(
                    "缺少 Action 权限: {:?}", names
                )));
            }
        }
    }

    action.dispatch(context).await
}
```

- [ ] **Step 7.5：暴露 list_actions**

```rust
pub fn list_actions(&self) -> Vec<&crate::action::ActionMeta> {
    self.actions.values().map(|a| a.meta()).collect()
}
```

- [ ] **Step 7.6：删除/重写旧 register_builtin_actions**

把 `register_builtin_actions` 整个方法移除（步骤 3 已把它返回错误占位，现在彻底删除）。

- [ ] **Step 7.7：更新 router 测试**

修改 `crates/yang-base/src/router/__tests__/module_router_tests.rs`：

- 找出依赖旧 Action trait 的测试，全部改写为使用 `#[derive(Action)] + TypedHandler` 的形式
- 关键测试：`test_dispatch_public_action_no_auth`、`test_dispatch_requires_login`、`test_dispatch_permission_denied`、`test_list_actions_returns_meta`

写一个对外用 `table_typed::<TestUser>()` 注册的测试：

```rust
#[tokio::test]
#[cfg(feature = "mysql")]
async fn test_table_typed_registers_six_builtins() {
    #[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema,
             sqlx::FromRow, yang_base_derive::TableEntity)]
    #[table(name = "test_users")]
    struct TU { #[entity(primary_key)] id: i64, name: String }

    let router = ModuleRouter::new("user", "用户管理").table_typed::<TU>();
    let names: Vec<String> = router.list_actions().iter().map(|m| m.name.to_string()).collect();
    for expected in ["get", "add", "put", "del", "select", "table"] {
        assert!(names.contains(&expected.to_string()), "缺少 {}", expected);
    }
}
```

- [ ] **Step 7.8：跑测试**

```bash
cargo test --lib -p yang-base
```

期望：全 PASS。

- [ ] **Step 7.9：commit**

```bash
git add -A crates/yang-base/src/
git commit -m "feat(yang-base): ModuleRouter 接入类型化 Action 系统

- DynAction 重命名为 Action（旧 Action 临时改名为 LegacyAction，下一步删除）
- 新增 table_typed::<T>() 一行注册全套 CRUD
- dispatch 改为读 ActionMeta 静态字段（性能微优）
- 新增 list_actions() 用于暴露模块下所有 Action 的元信息"
```

---

### Task 8：trybuild + schema 快照 + 集成测试 + 文档收尾

**目的：** spec §10、§11 步骤 7、步骤 8 + 验收清单。

**Files:**
- Modify: `Cargo.toml`（workspace dev-dependencies 加 trybuild、insta）
- Modify: `crates/yang-base/Cargo.toml`（dev-dep）
- Create: `crates/yang-base/tests/trybuild.rs`
- Create: `crates/yang-base/tests/compile_fail/*.rs`（4 个文件）
- Create: `crates/yang-base/tests/compile_fail/*.stderr`（对应 .stderr）
- Create: `crates/yang-base/tests/schema_snapshots.rs`
- Create: `crates/yang-base/tests/typed_action_integration.rs`
- Modify: `crates/yang-base/src/action/action_trait.rs`（删除 LegacyAction、清理）
- Modify: `docs/BACKLOG.md`
- Modify: `crates/yang-base/AGENTS.md`、`docs/yang-base.md`

- [ ] **Step 8.1：dev-dep**

`Cargo.toml`（workspace）`[workspace.dependencies]`：

```toml
trybuild = "1.0"
insta = { version = "1.39", features = ["json"] }
```

`crates/yang-base/Cargo.toml` 的 `[dev-dependencies]`：

```toml
trybuild = { workspace = true }
insta = { workspace = true }
```

- [ ] **Step 8.2：写 trybuild 入口**

文件：`crates/yang-base/tests/trybuild.rs`

```rust
#[test]
fn compile_fail_cases() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
```

- [ ] **Step 8.3：写 compile_fail 用例 1：非法字段名**

文件：`crates/yang-base/tests/compile_fail/where_invalid_field.rs`

```rust
use yang_base::table::TableEntity;
use yang_base_derive::TableEntity as DTE;

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema, sqlx::FromRow, DTE)]
#[table(name = "u")]
struct U { #[entity(primary_key)] id: i64, name: String }

fn main() {
    // 派生生成的 UWhere 只有 Id / Name 两个变体；NoSuchField 不存在
    let _ = UWhere::NoSuchField(yang_base::table::WhereOp::Eq(1));
}
```

把首次 `cargo test trybuild` 跑出的 .stderr 拷贝为 `where_invalid_field.stderr` 作为基线。

- [ ] **Step 8.4：写 compile_fail 用例 2：类型不匹配**

文件：`crates/yang-base/tests/compile_fail/where_type_mismatch.rs`

```rust
use yang_base::table::WhereOp;
use yang_base_derive::TableEntity as DTE;

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema, sqlx::FromRow, DTE)]
#[table(name = "u")]
struct U { #[entity(primary_key)] id: i64, name: String }

fn main() {
    // Id variant 持有 WhereOp<i64>；这里塞 &str，类型不匹配编译失败
    let _: UWhere = UWhere::Id(WhereOp::Eq("not an integer"));
}
```

- [ ] **Step 8.5：写 compile_fail 用例 3：Like on int**

文件：`crates/yang-base/tests/compile_fail/like_on_int.rs`

```rust
use yang_base::table::WhereOp;
use yang_base_derive::TableEntity as DTE;

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema, sqlx::FromRow, DTE)]
#[table(name = "u")]
struct U { #[entity(primary_key)] id: i64, name: String }

fn main() {
    // WhereOp 通用版本没有 Like 变体；只有 StringWhereOp 才有
    let _: WhereOp<i64> = WhereOp::Like("%foo%".into());
}
```

- [ ] **Step 8.6：写 compile_fail 用例 4：缺主键**

文件：`crates/yang-base/tests/compile_fail/missing_primary_key.rs`

```rust
use yang_base_derive::TableEntity as DTE;

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema, sqlx::FromRow, DTE)]
#[table(name = "u")]
struct U { id: i64, name: String }   // 缺 #[entity(primary_key)]

fn main() {}
```

- [ ] **Step 8.7：第一次跑 trybuild，记录基线 .stderr**

```bash
TRYBUILD=overwrite cargo test --test trybuild
```

trybuild 会把当前编译错误输出写为对应 `.stderr` 文件。**审查每个 .stderr**，确保错误信息合理（提到字段名、类型不匹配、`primary_key`），如果信息不够友好回到派生宏改进诊断。

复跑：

```bash
cargo test --test trybuild
```

期望：4 个 compile_fail 全 PASS（即对应 .rs 真的编译失败，且错误内容匹配 .stderr 快照）。

- [ ] **Step 8.8：写 schema 快照测试**

文件：`crates/yang-base/tests/schema_snapshots.rs`

```rust
#![cfg(feature = "mysql")]
use yang_base::action::builtin::SelectQuery;
use yang_base_derive::TableEntity as DTE;

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema, sqlx::FromRow, DTE)]
#[table(name = "test_users")]
struct TU {
    #[entity(primary_key)] id: i64,
    #[entity(max_length = 50)] username: String,
    age: i32,
}

#[test]
fn snapshot_entity_input_schema() {
    let s = schemars::schema_for!(TU);
    insta::assert_json_snapshot!("entity_TU_input", s);
}

#[test]
fn snapshot_select_query_schema() {
    let s = schemars::schema_for!(SelectQuery<TU>);
    insta::assert_json_snapshot!("select_query_TU", s);
}
```

第一次跑：

```bash
cargo test --test schema_snapshots
```

会生成 `tests/snapshots/<name>.snap.new`。审查内容（确认 SelectQuery 包含 `page/page_size/fields/where/order_by/count_total` 等字段；TU 包含 id/username/age），然后：

```bash
cargo insta accept
```

接受快照成为基线。

- [ ] **Step 8.9：写端到端集成测试**

文件：`crates/yang-base/tests/typed_action_integration.rs`

```rust
//! 端到端 CRUD 集成测试（H-1 验收）
//!
//! 使用 testcontainers 启动 MySQL 容器，跑完整 add → get → put → select → del → table 流程。
#![cfg(feature = "mysql")]

use std::sync::Arc;
use serde::{Deserialize, Serialize};
use yang_base::action::{ActionContext, Request, builtin::*, GlobalTools};
use yang_base::router::ModuleRouter;
use yang_base_derive::TableEntity as DTE;

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema, sqlx::FromRow, DTE)]
#[table(name = "typed_test_users")]
struct U {
    #[entity(primary_key)] id: i64,
    #[entity(max_length = 50)] username: String,
    age: i32,
}

async fn setup() -> (testcontainers::ContainerAsync<...>, Arc<GlobalTools>) {
    // 参照 crates/yang-base/tests/database_integration_test.rs 现有的容器启动模板
    todo!("参照已有容器启动 helper")
}

#[tokio::test]
async fn full_crud_cycle() {
    let (_container, tools) = setup().await;

    // 建表
    yang_base::database::GlobalDatabase::execute(
        "CREATE TABLE typed_test_users (id BIGINT PRIMARY KEY AUTO_INCREMENT, username VARCHAR(50) NOT NULL, age INT NOT NULL)"
    ).await.unwrap();

    let router = ModuleRouter::new("user", "用户").table_typed::<U>();

    // 1. add
    let req = Request::new("add".into()).with_body(serde_json::json!({
        "id": 1, "username": "alice", "age": 30
    }).as_object().cloned().unwrap());
    let ctx = ActionContext::new(req, tools.clone());
    let r = router.dispatch("add", ctx).await.unwrap();
    assert_eq!(r.code, 0);

    // 2. get
    let req = Request::new("get".into()).with_body(serde_json::json!({"id":1}).as_object().cloned().unwrap());
    let ctx = ActionContext::new(req, tools.clone());
    let r = router.dispatch("get", ctx).await.unwrap();
    let user: U = serde_json::from_value(r.data.unwrap()).unwrap();
    assert_eq!(user.username, "alice");

    // 3. put
    let req = Request::new("put".into()).with_body(serde_json::json!({
        "id": 1, "data": [["age", 31]]
    }).as_object().cloned().unwrap());
    let ctx = ActionContext::new(req, tools.clone());
    let r = router.dispatch("put", ctx).await.unwrap();
    let aff: AffectedResult = serde_json::from_value(r.data.unwrap()).unwrap();
    assert_eq!(aff.affected, 1);

    // 4. select
    let req = Request::new("select".into()).with_body(serde_json::json!({
        "page": 1, "page_size": 10,
        "where": [{"field":"username","cond":{"op":"like","value":"%alice%"}}],
        "count_total": true
    }).as_object().cloned().unwrap());
    let ctx = ActionContext::new(req, tools.clone());
    let r = router.dispatch("select", ctx).await.unwrap();
    let result: SelectResult<U> = serde_json::from_value(r.data.unwrap()).unwrap();
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].age, 31);
    assert_eq!(result.total, Some(1));

    // 5. del
    let req = Request::new("del".into()).with_body(serde_json::json!({"id":1}).as_object().cloned().unwrap());
    let ctx = ActionContext::new(req, tools.clone());
    let r = router.dispatch("del", ctx).await.unwrap();
    let aff: AffectedResult = serde_json::from_value(r.data.unwrap()).unwrap();
    assert_eq!(aff.affected, 1);

    // 6. table
    let req = Request::new("table".into()).with_body(Default::default());
    let ctx = ActionContext::new(req, tools.clone());
    let r = router.dispatch("table", ctx).await.unwrap();
    let schema: TableSchemaResponse = serde_json::from_value(r.data.unwrap()).unwrap();
    assert_eq!(schema.table_name, "typed_test_users");
    assert_eq!(schema.primary_key, "id");
}
```

> `setup()` 函数内部参照 `crates/yang-base/tests/database_integration_test.rs` 的 `testcontainers` 启动模板（搜出来对照即可）。

跑：

```bash
cargo test --test typed_action_integration -p yang-base
```

期望：PASS（需要 docker 运行）。

- [ ] **Step 8.10：清理 LegacyAction 与旧 action_trait.rs**

- 把 `crates/yang-base/src/action/action_trait.rs` 里的 `pub trait LegacyAction` 整个 trait + 默认方法删除
- 保留 `Permission` 类型（新系统仍使用）
- 修改文件头注释：声明这个文件现在只剩 `Permission`
- 删除 `register_builtin_actions` 已废的旧代码遗留
- 删除步骤 3 加在测试上的 `#[ignore = "H-1 重构期间停用"]`，把这些测试改写到新 API（如果已无意义则一并删除）

跑：

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
```

期望：全绿。

- [ ] **Step 8.11：更新 BACKLOG.md**

打开 `docs/BACKLOG.md`，把：

- `### ⏳ [H-1] builtin Action 使用 serde_json::Value 而非具体类型` 改为 `### ✅ [H-1] builtin Action 使用 serde_json::Value 而非具体类型`
- 在该节内顶部加一行：`**状态**：✅ 已完成。Action 系统已重构为 TypedHandler + #[derive(TableEntity)] + #[derive(Action)] 的端到端类型化方案，详见 docs/superpowers/specs/2026-05-27-action-typed-system-design.md。`
- 汇总表里 H-1 状态从 ⏳ 改为 ✅

- [ ] **Step 8.12：更新 AGENTS.md / yang-base.md**

修改 `crates/yang-base/AGENTS.md`：

- 找到 Action 系统相关章节，替换为新 API 的简短描述（`TypedHandler` / `#[derive(Action)]` / `#[derive(TableEntity)]` / `table_typed::<T>()`）
- 列出 spec 路径供未来 agent 查阅

修改 `docs/yang-base.md`：

- "Action 系统" 一节整体改写
- 主要变更：
  - 用户写 Action 的代码示例（一个简单 LoginAction）
  - 表实体的派生示例
  - 一行注册全套 CRUD 的示例

- [ ] **Step 8.13：跑验收清单**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
cargo test --test trybuild -p yang-base
cargo test --test schema_snapshots -p yang-base
cargo test --test typed_action_integration -p yang-base   # 需要 docker
```

每条都须 PASS。

- [ ] **Step 8.14：最终 commit**

```bash
git add -A
git commit -m "feat(yang-base): H-1 类型化 Action 系统完成

收尾：
- 4 个 trybuild compile_fail 用例（非法字段名 / 类型不匹配 / Like on int / 缺主键）
- 2 个 insta schema 快照（实体 / SelectQuery）
- 端到端集成测试 add → get → put → select → del → table
- 删除 LegacyAction、清理 register_builtin_actions
- 更新 BACKLOG / AGENTS / yang-base.md

完整设计见 docs/superpowers/specs/2026-05-27-action-typed-system-design.md"
```

---

## 自查清单（写完计划后人工核对）

- [x] 每个任务有具体文件路径 + 完整代码块
- [x] 每个任务有 `cargo test` 命令与期望输出
- [x] 每个任务以独立 commit 结束
- [x] 每个任务都能让 workspace 编译通过（旧代码暂禁不影响整体）
- [x] 引用的类型/方法在前序任务中已被定义（TableEntity → 任务 2；TypedHandler → 任务 3；派生宏 → 任务 4/5；builtin → 任务 6；router → 任务 7；测试 → 任务 8）
- [x] spec §11 的 8 个步骤一一对应到本计划的 8 个 Task
- [x] 验收清单（spec §14）全部通过 step 8.13 验证

---

## 已知风险与缓解

1. **`schema_for!` 在泛型 impl 里的行为**：`#[derive(Action)]` 派生宏假设 `schema_for!(<#struct_name as TypedHandler>::Input)` 在泛型 builtin 中能正确实例化。如果 schemars 0.8 对此有限制，回退方案是：派生宏生成不带 schema 的版本，运行时改用 `schemars::gen::SchemaGenerator` 手动构造。任务 5 实现时第一时间用一个泛型 Action 测试这点。

2. **`paginate(...)` API 是否存在**：步骤 6.7 中 `SelectAction` 假设 `TableQuery` 有 `paginate(page, page_size)` 链式方法；如果实际是 `with_page`/`with_page_size`，按实际改。grep `crates/yang-base/src/table/table_query.rs` 确认。

3. **`Request::with_body`/`GlobalTools::default()` stub**：所有测试假设有这些 helper。如不存在，参考 `crates/yang-base/src/action/__tests__/context_test.rs` 的实际 stub 方式调整。

4. **proc-macro-error 兼容性**：`proc-macro-error` 1.0 已不再维护；如果新 nightly Rust 警告，可替换为 `proc-macro-error2` 或回退到原生 `compile_error!`。

5. **派生宏的 schema 中泛型 TypeId 唯一性**：`schemars` 给泛型类型生成 schema 时基于 type name，多个 `SelectAction<UserA>`/`SelectAction<UserB>` 的 `OnceLock` 是按 type ID 区分的，互不干扰。

---

## 执行交接

**Plan complete and saved to `docs/superpowers/plans/2026-05-27-action-typed-system.md`. Two execution options:**

**1. Subagent-Driven (recommended)** - 我每个 Task 派一个全新的 subagent 实施，每完成一个 Task 在主会话审一次。隔离强、迭代快，适合本 8-task 重构。

**2. Inline Execution** - 在当前会话内顺序执行所有任务，每个 Task 结束做 checkpoint 让你审一遍。上下文连续，但主会话 context 消耗大。

**Which approach?**
