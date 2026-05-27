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
    + Send
    + Sync
    + Unpin
    + 'static
{
    /// 主键类型（i64 / String / Uuid 等）。
    type Pk: serde::de::DeserializeOwned
        + serde::Serialize
        + schemars::JsonSchema
        + Send
        + Sync
        + 'static;

    /// 字段名枚举（派生生成）。所有合法列名的封闭集合。
    /// `Eq + Hash` bound 让 `T::Field` 可用作 `HashSet`/`HashMap` 的 key。
    type Field: AsColumnName
        + serde::de::DeserializeOwned
        + serde::Serialize
        + schemars::JsonSchema
        + Copy
        + Eq
        + Hash
        + Send
        + Sync
        + 'static;

    /// where 条件枚举（派生生成）。每个变体绑定字段类型。
    type WhereCond: IntoSqlCondition
        + serde::de::DeserializeOwned
        + schemars::JsonSchema
        + Send
        + Sync
        + 'static;

    /// 数据库表名，对应 SQL `FROM` 子句。
    const TABLE_NAME: &'static str;
    /// 主键列名，对应 SQL `WHERE <PK> = ?`。
    const PK_FIELD: &'static str;

    /// 运行时表配置。OnceLock 缓存，全程序生成一次。
    fn table_config() -> &'static TableConfig;
}

/// 字段名 → 静态列名字符串。所有判别式都映射到 `&'static str`，
/// 杜绝列名拼接 SQL 注入。
pub trait AsColumnName {
    /// 返回对应数据库列名的静态字符串引用。
    fn column_name(&self) -> &'static str;
}

/// where 条件 → SqlCondition（运行期 SQL 片段描述）。
pub trait IntoSqlCondition {
    /// 将类型化 where 条件转换为运行时 SQL 描述。
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
    /// 等于（`= v`）。
    Eq(V),
    /// 不等于（`<> v`）。
    Ne(V),
    /// 小于（`< v`）。
    Lt(V),
    /// 小于等于（`<= v`）。
    Lte(V),
    /// 大于（`> v`）。
    Gt(V),
    /// 大于等于（`>= v`）。
    Gte(V),
    /// 在集合中（`IN (v1, v2, ...)`）。
    In(Vec<V>),
    /// 不在集合中（`NOT IN (v1, v2, ...)`）。
    NotIn(Vec<V>),
    /// 区间（`BETWEEN a AND b`）。
    Between(V, V),
    /// 为 NULL（`IS NULL`）。
    IsNull,
    /// 不为 NULL（`IS NOT NULL`）。
    IsNotNull,
}

/// 字符串字段专用 where 操作符（额外含 Like）。
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(tag = "op", content = "value", rename_all = "snake_case")]
pub enum StringWhereOp {
    /// 等于（`= v`）。
    Eq(String),
    /// 不等于（`<> v`）。
    Ne(String),
    /// 小于（`< v`）。
    Lt(String),
    /// 小于等于（`<= v`）。
    Lte(String),
    /// 大于（`> v`）。
    Gt(String),
    /// 大于等于（`>= v`）。
    Gte(String),
    /// 在集合中（`IN (v1, v2, ...)`）。
    In(Vec<String>),
    /// 不在集合中（`NOT IN (v1, v2, ...)`）。
    NotIn(Vec<String>),
    /// 区间（`BETWEEN a AND b`）。
    Between(String, String),
    /// 模糊匹配（`LIKE pattern`）。
    Like(String),
    /// 为 NULL（`IS NULL`）。
    IsNull,
    /// 不为 NULL（`IS NOT NULL`）。
    IsNotNull,
}

/// 运行时 SQL 条件描述。column 是 `'static str`，绝对安全。
#[derive(Debug, Clone)]
pub struct SqlCondition {
    /// 列名（`'static str`，来自 `AsColumnName::column_name`）。
    pub column: &'static str,
    /// SQL 操作符。
    pub op: SqlOp,
    /// 绑定参数（JSON 编码，供查询构建器消费）。
    pub params: Vec<serde_json::Value>,
}

/// SQL 操作符枚举，对应 WHERE 子句中的各类比较/范围/空值条件。
#[derive(Debug, Clone, Copy)]
pub enum SqlOp {
    /// 等于（`=`）。
    Eq,
    /// 不等于（`<>`）。
    Ne,
    /// 小于（`<`）。
    Lt,
    /// 小于等于（`<=`）。
    Lte,
    /// 大于（`>`）。
    Gt,
    /// 大于等于（`>=`）。
    Gte,
    /// 在集合中（`IN`）。
    In,
    /// 不在集合中（`NOT IN`）。
    NotIn,
    /// 区间（`BETWEEN`）。
    Between,
    /// 模糊匹配（`LIKE`）。
    Like,
    /// 为 NULL（`IS NULL`）。
    IsNull,
    /// 不为 NULL（`IS NOT NULL`）。
    IsNotNull,
}

impl<V: serde::Serialize> WhereOp<V> {
    /// 把通用 WhereOp 转为 SqlCondition（给定列名）。供派生宏生成的 IntoSqlCondition 调用。
    pub fn to_sql_condition(self, column: &'static str) -> SqlCondition {
        match self {
            WhereOp::Eq(v) => SqlCondition { column, op: SqlOp::Eq, params: vec![to_v(v)] },
            WhereOp::Ne(v) => SqlCondition { column, op: SqlOp::Ne, params: vec![to_v(v)] },
            WhereOp::Lt(v) => SqlCondition { column, op: SqlOp::Lt, params: vec![to_v(v)] },
            WhereOp::Lte(v) => SqlCondition { column, op: SqlOp::Lte, params: vec![to_v(v)] },
            WhereOp::Gt(v) => SqlCondition { column, op: SqlOp::Gt, params: vec![to_v(v)] },
            WhereOp::Gte(v) => SqlCondition { column, op: SqlOp::Gte, params: vec![to_v(v)] },
            WhereOp::In(vs) => SqlCondition {
                column,
                op: SqlOp::In,
                params: vs.into_iter().map(to_v).collect(),
            },
            WhereOp::NotIn(vs) => SqlCondition {
                column,
                op: SqlOp::NotIn,
                params: vs.into_iter().map(to_v).collect(),
            },
            WhereOp::Between(a, b) => SqlCondition {
                column,
                op: SqlOp::Between,
                params: vec![to_v(a), to_v(b)],
            },
            WhereOp::IsNull => SqlCondition { column, op: SqlOp::IsNull, params: vec![] },
            WhereOp::IsNotNull => SqlCondition { column, op: SqlOp::IsNotNull, params: vec![] },
        }
    }
}

impl StringWhereOp {
    /// 把字符串专用操作符转为 SqlCondition（给定列名）。
    pub fn to_sql_condition(self, column: &'static str) -> SqlCondition {
        match self {
            StringWhereOp::Like(p) => SqlCondition {
                column,
                op: SqlOp::Like,
                params: vec![serde_json::Value::String(p)],
            },
            // 其余复用 WhereOp 的语义
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
    serde_json::to_value(v).expect("WhereOp 参数序列化失败：Serialize 实现有缺陷")
}
