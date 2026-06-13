//! TableEntity 类型化基础设施
//!
//! 提供 `TableEntity` trait 以及 `WhereOp<V>`、`SqlCondition` 等类型，
//! 用于 Action 系统的端到端类型安全（H-1）。

#[cfg(feature = "mysql")]
use crate::table::TableConfig;
use serde::{Deserialize, Serialize};
#[cfg(feature = "mysql")]
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

impl SqlCondition {
    /// 把运行期 `SqlCondition` 转换为受保护层的 [`WhereCondition`](crate::table::WhereCondition)。
    ///
    /// 这是类型化布尔树（[`Filter`]）桥接到 `TableQuery` 的关键一跳：派生宏产出的
    /// 类型化叶子先 `into_sql_condition()` 成 `SqlCondition`，再经此转为受保护层
    /// 能递归校验/渲染的 `WhereCondition` 叶子。
    ///
    /// 参数个数不足时（理论上不会发生，派生宏保证）退化为恒真 `1=1`（`NotIn` 空集），
    /// 不 panic。
    pub fn into_where_condition(self) -> crate::table::WhereCondition {
        use crate::table::WhereCondition as WC;
        let SqlCondition { column, op, mut params } = self;
        let field = column.to_string();
        // 取第 n 个参数，缺失则用 Null 兜底（保持总量不 panic）
        let take = |params: &mut Vec<serde_json::Value>, i: usize| -> serde_json::Value {
            params.get(i).cloned().unwrap_or(serde_json::Value::Null)
        };
        match op {
            SqlOp::Eq => WC::Eq { field, value: take(&mut params, 0) },
            SqlOp::Ne => WC::Ne { field, value: take(&mut params, 0) },
            SqlOp::Lt => WC::Lt { field, value: take(&mut params, 0) },
            SqlOp::Lte => WC::Lte { field, value: take(&mut params, 0) },
            SqlOp::Gt => WC::Gt { field, value: take(&mut params, 0) },
            SqlOp::Gte => WC::Gte { field, value: take(&mut params, 0) },
            SqlOp::In => WC::In { field, values: params },
            SqlOp::NotIn => WC::NotIn { field, values: params },
            SqlOp::Between => {
                let lo = take(&mut params, 0);
                let hi = take(&mut params, 1);
                WC::Between { field, lo, hi }
            }
            SqlOp::Like => {
                let pattern = params
                    .first()
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                WC::Like { field, pattern }
            }
            SqlOp::IsNull => WC::IsNull { field },
            SqlOp::IsNotNull => WC::IsNotNull { field },
        }
    }
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
    serde_json::to_value(v).unwrap_or_else(|e| {
        // Serialize 实现缺陷理论上不会发生；降级为 Null 而非 panic，避免
        // 任意 Serialize 异常导致整个 dispatch 崩溃。
        log::error!("WhereOp 参数序列化失败，降级为 Null: {}", e);
        serde_json::Value::Null
    })
}

/// 类型化布尔过滤树（C2a 类型化层）。
///
/// 在类型化叶子 `W`（一般是 `T::WhereCond`，JSON 形态 `{"field":..,"cond":..}`）之上
/// 叠加 `And`/`Or` 嵌套结构，端到端保持字段名/操作符的编译期类型安全，同时支持任意
/// 深度的布尔组合。
///
/// # JSON 线格式（`#[serde(untagged)]`）
///
/// 一个节点是以下三者之一：
/// - AND 组：`{"and": [<子节点>, ...]}`
/// - OR 组：`{"or": [<子节点>, ...]}`
/// - 叶子：直接是 `W` 的形态，如 `{"field":"age","cond":{"op":"gte","value":18}}`
///
/// ```json
/// {
///   "or": [
///     {"field": "status", "cond": {"op": "eq", "value": "active"}},
///     {"and": [
///       {"field": "age", "cond": {"op": "gte", "value": 18}},
///       {"field": "vip", "cond": {"op": "eq", "value": true}}
///     ]}
///   ]
/// }
/// ```
///
/// # 与受保护层桥接
///
/// [`Filter::into_where_condition`] 把整棵树降解为受保护层的
/// [`WhereCondition`](crate::table::WhereCondition)，叶子经 `IntoSqlCondition` →
/// `SqlCondition` → `WhereCondition`，由 `TableQuery` 统一做递归权限校验与渲染。
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum Filter<W> {
    /// AND 组：`{"and": [...]}`，子节点全部成立。
    And {
        /// AND 子节点列表
        and: Vec<Filter<W>>,
    },
    /// OR 组：`{"or": [...]}`，子节点任一成立。
    Or {
        /// OR 子节点列表
        or: Vec<Filter<W>>,
    },
    /// 叶子节点：直接是类型化 where 条件 `W`。
    Leaf(W),
}

impl<W: IntoSqlCondition> Filter<W> {
    /// 把类型化布尔树降解为受保护层的 [`WhereCondition`](crate::table::WhereCondition)。
    ///
    /// 叶子经 `into_sql_condition()` → `into_where_condition()` 转换；组节点递归降解。
    /// 不做深度限制（受保护层 `TableQuery` 在校验/渲染期统一施加
    /// `MAX_WHERE_DEPTH` 上限并返回错误）。
    pub fn into_where_condition(self) -> crate::table::WhereCondition {
        match self {
            Filter::Leaf(w) => w.into_sql_condition().into_where_condition(),
            Filter::And { and } => crate::table::WhereCondition::And {
                conditions: and.into_iter().map(Filter::into_where_condition).collect(),
            },
            Filter::Or { or } => crate::table::WhereCondition::Or {
                conditions: or.into_iter().map(Filter::into_where_condition).collect(),
            },
        }
    }
}
