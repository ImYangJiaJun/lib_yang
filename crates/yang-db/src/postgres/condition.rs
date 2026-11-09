#![allow(deprecated)]

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use serde_json::Value as JsonValue;

use crate::DbError;

/// 受控列比较操作符。
#[derive(Debug, Clone, Copy)]
pub enum ComparisonOperator {
    /// 等于。
    Eq,
    /// 不等于。
    Ne,
    /// 大于。
    Gt,
    /// 小于。
    Lt,
    /// 大于等于。
    Gte,
    /// 小于等于。
    Lte,
}

impl ComparisonOperator {
    fn parse(op: &str) -> Result<Self, DbError> {
        match op {
            "=" => Ok(Self::Eq),
            "!=" => Ok(Self::Ne),
            ">" => Ok(Self::Gt),
            "<" => Ok(Self::Lt),
            ">=" => Ok(Self::Gte),
            "<=" => Ok(Self::Lte),
            _ => Err(DbError::UnsupportedOperator(op.to_string())),
        }
    }

    fn as_sql(self) -> &'static str {
        match self {
            Self::Eq => "=",
            Self::Ne => "!=",
            Self::Gt => ">",
            Self::Lt => "<",
            Self::Gte => ">=",
            Self::Lte => "<=",
        }
    }
}

/// 只能由安全标识符和绑定条件构造的 SELECT 子查询。
#[derive(Debug, Clone)]
pub struct Subquery {
    table: String,
    field: String,
    conditions: Vec<Condition>,
}

impl Subquery {
    /// 创建只投影一个字段的受控子查询。
    pub fn new(table: &str, field: &str) -> Result<Self, DbError> {
        super::identifier::quote_identifier(table)?;
        super::identifier::quote_qualified(field)?;
        Ok(Self {
            table: table.to_string(),
            field: field.to_string(),
            conditions: Vec::new(),
        })
    }

    /// 添加字段与绑定值的 AND 条件。
    pub fn where_value<V>(mut self, field: &str, op: &str, value: V) -> Result<Self, DbError>
    where
        V: Into<SqlValue>,
    {
        super::identifier::quote_qualified(field)?;
        let value = value.into();
        let condition = match op {
            "=" => Condition::Eq(field.to_string(), value),
            "!=" => Condition::Ne(field.to_string(), value),
            ">" => Condition::Gt(field.to_string(), value),
            "<" => Condition::Lt(field.to_string(), value),
            ">=" => Condition::Gte(field.to_string(), value),
            "<=" => Condition::Lte(field.to_string(), value),
            _ => return Err(DbError::UnsupportedOperator(op.to_string())),
        };
        self.conditions.push(condition);
        Ok(self)
    }

    /// 添加两个已校验列之间的 AND 比较，用于关联子查询。
    pub fn where_column(mut self, left: &str, op: &str, right: &str) -> Result<Self, DbError> {
        super::identifier::quote_qualified(left)?;
        super::identifier::quote_qualified(right)?;
        self.conditions.push(Condition::ColumnComparison(
            left.to_string(),
            ComparisonOperator::parse(op)?,
            right.to_string(),
        ));
        Ok(self)
    }
}

/// SQL 值类型（PostgreSQL）
///
/// 与 MySQL 后端的 `SqlValue` 结构一致，独立定义以保持 `postgres` 模块自包含，
/// 避免跨方言耦合。变体集合与 `From` 转换均与 MySQL 版本保持对齐。
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum SqlValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    Json(JsonValue),
    DateTime(NaiveDateTime),
    Timestamp(i64),
    /// NUMERIC / DECIMAL，按精确十进制绑定，避免经 f64 丢精度。
    Decimal(bigdecimal::BigDecimal),
    /// UUID（`uuid` 扩展类型）。
    Uuid(uuid::Uuid),
    /// TIMESTAMPTZ（带时区）。
    DateTimeTz(DateTime<Utc>),
    /// DATE。
    Date(NaiveDate),
    /// TIME。
    Time(NaiveTime),
    /// INTERVAL，单位为微秒。
    Interval(i64),
    /// PostgreSQL 数组（`type[]`），元素按顺序作为单个数组绑定参数传输，
    /// 不展开为多个标量参数。可承载的 From 转换见 `Vec<i64>` / `Vec<String>` /
    /// `Vec<f64>` / `Vec<bool>`。
    Array(Vec<SqlValue>),
    /// PostgreSQL `inet` 文本值；渲染时追加 `::inet`。
    Inet(String),
    /// PostgreSQL `cidr` 文本值；渲染时追加 `::cidr`。
    Cidr(String),
    /// PostgreSQL `macaddr` 文本值；渲染时追加 `::macaddr`。
    MacAddr(String),
    /// PostgreSQL `macaddr8` 文本值；渲染时追加 `::macaddr8`。
    MacAddr8(String),
    /// 范围文本值（如 `[1,10)`）；按文本绑定并追加 `::int4range`。
    ///
    /// 目前仅内置默认的 32 位整数范围转换；其它范围类型需要在 SQL 文本中显式
    /// 指定转换（`$1::tsrange` 等）。
    Range(String),
    /// 多范围文本值（如 `{[1,10),[20,30)}`）；按文本绑定并追加
    /// `::int4multirange`（PostgreSQL 14+）。
    Multirange(String),
}

/// 查询条件（PostgreSQL）
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Condition {
    /// 相等
    Eq(String, SqlValue),
    /// 不等
    Ne(String, SqlValue),
    /// 大于
    Gt(String, SqlValue),
    /// 小于
    Lt(String, SqlValue),
    /// 大于等于
    Gte(String, SqlValue),
    /// 小于等于
    Lte(String, SqlValue),
    /// IN 条件
    In(String, Vec<SqlValue>),
    /// NOT IN 条件
    NotIn(String, Vec<SqlValue>),
    /// BETWEEN 条件
    Between(String, SqlValue, SqlValue),
    /// LIKE 条件
    Like(String, String),
    /// IS NULL 条件
    IsNull(String),
    /// IS NOT NULL 条件
    IsNotNull(String),
    /// 两个标识符之间的受控比较。
    ColumnComparison(String, ComparisonOperator, String),
    /// EXISTS 子查询。
    Exists(Box<Subquery>),
    /// NOT EXISTS 子查询。
    NotExists(Box<Subquery>),
    /// IN 子查询。
    InSubquery(String, Box<Subquery>),
    /// 字段与受控服务端表达式的比较（如 `updated_at < now()`）。
    FieldExpr(String, ComparisonOperator, crate::SqlExpr),
    /// JSON/JSONB 包含谓词：`field @> $n`（右值必须是 [`SqlValue::Json`]）。
    JsonContains(String, SqlValue),
    /// JSON/JSONB 被包含谓词：`field <@ $n`（右值必须是 [`SqlValue::Json`]）。
    JsonContainedBy(String, SqlValue),
    /// JSON/JSONB 顶层键存在谓词：`field ? $n`（键名以绑定参数传入）。
    JsonHasKey(String, String),
    /// 数组包含任一元素：`field = ANY($n)`（右值必须是 [`SqlValue::Array`]）。
    Any(String, SqlValue),
    /// 数组包含全部元素：`field = ALL($n)`（右值必须是 [`SqlValue::Array`]）。
    All(String, SqlValue),
    /// 范围包含：`field @> $n`（右值必须是范围或多范围文本值）。
    RangeContains(String, SqlValue),
    /// 范围重叠：`field && $n`（右值必须是范围或多范围文本值）。
    RangeOverlaps(String, SqlValue),
    /// JSONB 路径存在：`jsonb_path_exists((field)::jsonb, $n::jsonpath)`。
    JsonPathExists(String, String),
    /// JSONB 路径谓词：`jsonb_path_match((field)::jsonb, $n::jsonpath, $m::jsonb)`。
    JsonPathMatch(String, String, SqlValue),
    /// 全文检索：`to_tsvector('simple', field) @@ plainto_tsquery('simple', $n)`。
    FullText(String, String),
    /// 全文检索（`to_tsquery` 变体，允许布尔查询语法）。
    FullTextQuery(String, String),
    /// NOT 包裹；渲染为 `NOT (...)`。
    Not(Box<Condition>),
    /// AND 组合
    And(Vec<Condition>),
    /// OR 组合
    Or(Vec<Condition>),
}

// 实现 From trait 支持自动转换（与 MySQL 后端保持一致）
impl From<i32> for SqlValue {
    fn from(v: i32) -> Self {
        SqlValue::Int(v as i64)
    }
}

impl From<i64> for SqlValue {
    fn from(v: i64) -> Self {
        SqlValue::Int(v)
    }
}

impl From<u64> for SqlValue {
    fn from(v: u64) -> Self {
        // u64 顶半区（> i64::MAX）强转 i64 会静默环绕成负数（NEW-11）。PG 无无符号类型，
        // 此类值本就放不进 BIGINT，走十进制字符串：对 NUMERIC 列正确，对 int8 列会显式
        // 报错（值确实溢出），均优于静默改写成负数。
        if v > i64::MAX as u64 {
            SqlValue::String(v.to_string())
        } else {
            SqlValue::Int(v as i64)
        }
    }
}

impl From<f64> for SqlValue {
    fn from(v: f64) -> Self {
        SqlValue::Float(v)
    }
}

impl From<f32> for SqlValue {
    fn from(v: f32) -> Self {
        SqlValue::Float(v as f64)
    }
}

impl From<String> for SqlValue {
    fn from(v: String) -> Self {
        SqlValue::String(v)
    }
}

impl From<&str> for SqlValue {
    fn from(v: &str) -> Self {
        SqlValue::String(v.to_string())
    }
}

impl From<bool> for SqlValue {
    fn from(v: bool) -> Self {
        SqlValue::Bool(v)
    }
}

impl From<Vec<u8>> for SqlValue {
    fn from(v: Vec<u8>) -> Self {
        SqlValue::Bytes(v)
    }
}

/// `BIGINT[]` / `INT8[]` 数组。
impl From<Vec<i64>> for SqlValue {
    fn from(v: Vec<i64>) -> Self {
        SqlValue::Array(v.into_iter().map(SqlValue::Int).collect())
    }
}

/// `TEXT[]` / `VARCHAR[]` 数组。
impl From<Vec<String>> for SqlValue {
    fn from(v: Vec<String>) -> Self {
        SqlValue::Array(v.into_iter().map(SqlValue::String).collect())
    }
}

/// `DOUBLE PRECISION[]` / `FLOAT8[]` 数组。
impl From<Vec<f64>> for SqlValue {
    fn from(v: Vec<f64>) -> Self {
        SqlValue::Array(v.into_iter().map(SqlValue::Float).collect())
    }
}

/// `BOOLEAN[]` / `BOOL[]` 数组。
impl From<Vec<bool>> for SqlValue {
    fn from(v: Vec<bool>) -> Self {
        SqlValue::Array(v.into_iter().map(SqlValue::Bool).collect())
    }
}

impl From<JsonValue> for SqlValue {
    fn from(v: JsonValue) -> Self {
        SqlValue::Json(v)
    }
}

impl From<NaiveDateTime> for SqlValue {
    fn from(v: NaiveDateTime) -> Self {
        SqlValue::DateTime(v)
    }
}

impl From<bigdecimal::BigDecimal> for SqlValue {
    fn from(v: bigdecimal::BigDecimal) -> Self {
        SqlValue::Decimal(v)
    }
}

impl From<uuid::Uuid> for SqlValue {
    fn from(v: uuid::Uuid) -> Self {
        SqlValue::Uuid(v)
    }
}

impl From<DateTime<Utc>> for SqlValue {
    fn from(v: DateTime<Utc>) -> Self {
        SqlValue::DateTimeTz(v)
    }
}

impl From<NaiveDate> for SqlValue {
    fn from(v: NaiveDate) -> Self {
        SqlValue::Date(v)
    }
}

impl From<NaiveTime> for SqlValue {
    fn from(v: NaiveTime) -> Self {
        SqlValue::Time(v)
    }
}

impl From<chrono::Duration> for SqlValue {
    fn from(v: chrono::Duration) -> Self {
        SqlValue::Interval(v.num_microseconds().unwrap_or_default())
    }
}

/// 以微秒构造 INTERVAL 值。
impl SqlValue {
    /// INTERVAL（微秒）。
    pub fn interval_micros(micros: i64) -> Self {
        SqlValue::Interval(micros)
    }
}

impl<T> From<Option<T>> for SqlValue
where
    T: Into<SqlValue>,
{
    fn from(v: Option<T>) -> Self {
        match v {
            Some(val) => val.into(),
            None => SqlValue::Null,
        }
    }
}

/// 将条件转换为 SQL 字符串和参数列表（借用版本）
///
/// # 参数
/// - condition: 要转换的条件
/// - params: 用于收集参数的可变向量（占位符编号基于其当前长度）
///
/// # 返回
/// - SQL 字符串片段
#[deprecated(
    since = "0.1.3",
    note = "使用 condition_to_sql_owned_checked 获取结构化错误"
)]
pub fn condition_to_sql(condition: &Condition, params: &mut Vec<SqlValue>) -> String {
    // 借用版本是消费版本的薄委托：整树 clone 一次后交给 owned 实现，
    // 二者共享同一套 match 分支，避免逻辑重复。输出 SQL 与参数顺序完全一致。
    condition_to_sql_owned(condition.clone(), params)
}

const FAIL_CLOSED_CONDITION: &str = "/* invalid condition */ 1 = 0";

/// 兼容的 infallible 入口只委托 checked renderer；失败时返回固定不可执行条件。
#[deprecated(
    since = "0.1.3",
    note = "使用 condition_to_sql_owned_checked 获取结构化错误"
)]
pub fn condition_to_sql_owned(condition: Condition, params: &mut Vec<SqlValue>) -> String {
    condition_to_sql_owned_checked(condition, params)
        .unwrap_or_else(|_| FAIL_CLOSED_CONDITION.to_string())
}

/// 严格版条件转 SQL：非法标识符返回 [`DbError::InvalidArgument`]。
///
/// 只接受单段或两段限定标识符，不接受表达式；失败时调用方参数保持不变。
///
/// # 返回
///
/// - `Ok(String)`: SQL 片段
/// - `Err(DbError::InvalidArgument)`: 包含非法标识符
pub fn condition_to_sql_owned_checked(
    condition: Condition,
    params: &mut Vec<SqlValue>,
) -> Result<String, DbError> {
    let rendered = render_condition_checked(condition, params.len())?;
    params.extend(rendered.params);
    Ok(rendered.sql)
}

pub(crate) fn render_condition_checked(
    condition: Condition,
    parameter_offset: usize,
) -> Result<crate::sql_types::RenderedCondition<SqlValue>, DbError> {
    // 渲染逻辑按方言共享于 crate::dialect；这里只做公开条件树 → 内部树的转换。
    // `parameter_offset` 保证 `$N` 编号接续调用方参数列表的既有长度。
    crate::dialect::render_condition(
        crate::dialect::POSTGRES,
        into_node(condition),
        parameter_offset,
    )
}

/// 将公开条件树转换为方言无关的内部渲染树（值原样移动，不复制）。
fn into_node(condition: Condition) -> crate::dialect::CondNode<SqlValue> {
    use crate::dialect::CondNode as Node;
    match condition {
        Condition::Eq(field, value) => Node::Eq(field, value),
        Condition::Ne(field, value) => Node::Ne(field, value),
        Condition::Gt(field, value) => Node::Gt(field, value),
        Condition::Lt(field, value) => Node::Lt(field, value),
        Condition::Gte(field, value) => Node::Gte(field, value),
        Condition::Lte(field, value) => Node::Lte(field, value),
        Condition::In(field, values) => Node::In(field, values),
        Condition::NotIn(field, values) => Node::NotIn(field, values),
        Condition::Between(field, start, end) => Node::Between(field, start, end),
        Condition::Like(field, pattern) => Node::Like(field, SqlValue::String(pattern)),
        Condition::IsNull(field) => Node::IsNull(field),
        Condition::IsNotNull(field) => Node::IsNotNull(field),
        Condition::ColumnComparison(left, op, right) => {
            Node::ColumnComparison(left, op.as_sql(), right)
        }
        Condition::Exists(subquery) => Node::Exists(Box::new(into_subquery_node(*subquery))),
        Condition::NotExists(subquery) => Node::NotExists(Box::new(into_subquery_node(*subquery))),
        Condition::InSubquery(field, subquery) => {
            Node::InSubquery(field, Box::new(into_subquery_node(*subquery)))
        }
        Condition::FieldExpr(field, operator, expression) => {
            Node::ExprComparison(field, operator.as_sql(), expression)
        }
        Condition::JsonContains(field, value) => Node::JsonContains(field, value),
        Condition::JsonContainedBy(field, value) => Node::JsonContainedBy(field, value),
        Condition::JsonHasKey(field, key) => Node::JsonHasKey(field, SqlValue::String(key)),
        Condition::Any(field, value) => Node::Any(field, value),
        Condition::All(field, value) => Node::All(field, value),
        Condition::RangeContains(field, value) => Node::RangeContains(field, value),
        Condition::RangeOverlaps(field, value) => Node::RangeOverlaps(field, value),
        Condition::JsonPathExists(field, path) => {
            Node::JsonPathExists(field, SqlValue::String(path))
        }
        Condition::JsonPathMatch(field, path, vars) => {
            Node::JsonPathMatch(field, SqlValue::String(path), vars)
        }
        Condition::FullText(field, query) => Node::FullText(field, SqlValue::String(query)),
        Condition::FullTextQuery(field, query) => {
            Node::FullTextQuery(field, SqlValue::String(query))
        }
        Condition::Not(condition) => Node::Not(Box::new(into_node(*condition))),
        Condition::And(conditions) => Node::And(conditions.into_iter().map(into_node).collect()),
        Condition::Or(conditions) => Node::Or(conditions.into_iter().map(into_node).collect()),
    }
}

/// 将受控子查询转换为内部渲染节点。
fn into_subquery_node(subquery: Subquery) -> crate::dialect::SubqueryNode<SqlValue> {
    crate::dialect::SubqueryNode {
        table: subquery.table,
        field: subquery.field,
        conditions: subquery.conditions.into_iter().map(into_node).collect(),
    }
}

#[cfg(test)]
#[allow(deprecated)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_checked_rejects_empty_in_condition() {
        let mut params = Vec::new();
        let result =
            condition_to_sql_owned_checked(Condition::In("id".to_string(), vec![]), &mut params);

        assert!(matches!(result, Err(crate::DbError::InvalidArgument(_))));
        assert!(params.is_empty());
    }

    #[test]
    fn test_checked_rejects_empty_boolean_condition() {
        let mut params = Vec::new();
        let result = condition_to_sql_owned_checked(Condition::And(vec![]), &mut params);

        assert!(matches!(result, Err(crate::DbError::InvalidArgument(_))));
        assert!(params.is_empty());
    }

    #[test]
    fn test_checked_failure_does_not_partially_mutate_caller_params() {
        let mut params = vec![SqlValue::Int(99)];
        let condition = Condition::And(vec![
            Condition::Eq("users.id".to_string(), SqlValue::Int(1)),
            Condition::Eq("users.id --".to_string(), SqlValue::Int(2)),
        ]);

        assert!(condition_to_sql_owned_checked(condition, &mut params).is_err());
        assert_eq!(params.len(), 1);
        assert!(matches!(params[0], SqlValue::Int(99)));
    }

    #[test]
    fn test_checked_placeholder_order_starts_after_existing_params() {
        let mut params = vec![SqlValue::Int(99)];
        let condition = Condition::And(vec![
            Condition::Eq("users.id".to_string(), SqlValue::Int(1)),
            Condition::Between(
                "users.score".to_string(),
                SqlValue::Int(10),
                SqlValue::Int(20),
            ),
        ]);

        let sql = match condition_to_sql_owned_checked(condition, &mut params) {
            Ok(sql) => sql,
            Err(error) => panic!("合法条件不应渲染失败: {error}"),
        };
        assert_eq!(
            sql,
            "(\"users\".\"id\" = $2 AND \"users\".\"score\" BETWEEN $3 AND $4)"
        );
        assert_eq!(params.len(), 4);
        assert!(matches!(params[0], SqlValue::Int(99)));
        assert!(matches!(params[1], SqlValue::Int(1)));
        assert!(matches!(params[2], SqlValue::Int(10)));
        assert!(matches!(params[3], SqlValue::Int(20)));
    }

    #[test]
    fn test_legacy_renderer_fails_closed_without_raw_payload_or_params() {
        let mut params = Vec::new();
        let sql = condition_to_sql_owned(
            Condition::Eq("id; DROP TABLE users".to_string(), SqlValue::Int(1)),
            &mut params,
        );

        assert_eq!(sql, FAIL_CLOSED_CONDITION);
        assert!(!sql.contains("DROP TABLE"));
        assert!(params.is_empty());
    }

    #[test]
    fn test_condition_eq_uses_dollar_placeholder() {
        let mut params = Vec::new();
        let cond = Condition::Eq("name".to_string(), SqlValue::String("test".to_string()));
        let sql = condition_to_sql(&cond, &mut params);
        assert_eq!(sql, "\"name\" = $1");
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn test_condition_in_numbered_sequentially() {
        let mut params = Vec::new();
        let cond = Condition::In(
            "id".to_string(),
            vec![SqlValue::Int(1), SqlValue::Int(2), SqlValue::Int(3)],
        );
        let sql = condition_to_sql(&cond, &mut params);
        assert_eq!(sql, "\"id\" IN ($1, $2, $3)");
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn test_condition_in_empty() {
        let mut params = Vec::new();
        let cond = Condition::In("id".to_string(), vec![]);
        let sql = condition_to_sql(&cond, &mut params);
        assert_eq!(sql, FAIL_CLOSED_CONDITION);
        assert_eq!(params.len(), 0);
    }

    #[test]
    fn test_condition_between() {
        let mut params = Vec::new();
        let cond = Condition::Between("age".to_string(), SqlValue::Int(18), SqlValue::Int(65));
        let sql = condition_to_sql(&cond, &mut params);
        assert_eq!(sql, "\"age\" BETWEEN $1 AND $2");
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn test_placeholder_continues_from_existing_params() {
        // 模拟 UPDATE：SET 子句已压入 2 个参数后再生成 WHERE，编号应接续为 $3
        let mut params = vec![SqlValue::Int(100), SqlValue::String("x".to_string())];
        let cond = Condition::Eq("id".to_string(), SqlValue::Int(1));
        let sql = condition_to_sql(&cond, &mut params);
        assert_eq!(sql, "\"id\" = $3");
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn test_condition_and_priority_and_numbering() {
        let mut params = Vec::new();
        // ("name" = $1 OR "name" = $2) AND "age" > $3
        let cond = Condition::And(vec![
            Condition::Or(vec![
                Condition::Eq("name".to_string(), SqlValue::String("test".to_string())),
                Condition::Eq("name".to_string(), SqlValue::String("demo".to_string())),
            ]),
            Condition::Gt("age".to_string(), SqlValue::Int(18)),
        ]);
        let sql = condition_to_sql(&cond, &mut params);
        assert_eq!(sql, "((\"name\" = $1 OR \"name\" = $2) AND \"age\" > $3)");
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn test_is_null_and_not_null_no_params() {
        let mut params = Vec::new();
        let cond = Condition::And(vec![
            Condition::IsNull("a".to_string()),
            Condition::IsNotNull("b".to_string()),
        ]);
        let sql = condition_to_sql(&cond, &mut params);
        assert_eq!(sql, "(\"a\" IS NULL AND \"b\" IS NOT NULL)");
        assert_eq!(params.len(), 0);
    }

    #[test]
    fn test_owned_equals_borrowed() {
        let cond = Condition::And(vec![
            Condition::Eq("a".to_string(), SqlValue::Int(1)),
            Condition::Like("b".to_string(), "%x%".to_string()),
        ]);
        let mut p1 = vec![];
        let mut p2 = vec![];
        let s1 = condition_to_sql_owned(cond.clone(), &mut p1);
        let s2 = condition_to_sql(&cond, &mut p2);
        assert_eq!(s1, s2);
        assert_eq!(p1.len(), p2.len());
        assert_eq!(format!("{:?}", p1), format!("{:?}", p2));
    }

    #[test]
    fn test_not_renders_and_composes_with_and_or() {
        let mut params = Vec::new();
        let condition = Condition::And(vec![
            Condition::Not(Box::new(Condition::Eq(
                "status".to_string(),
                SqlValue::String("deleted".to_string()),
            ))),
            Condition::Or(vec![
                Condition::Gt("age".to_string(), SqlValue::Int(18)),
                Condition::Not(Box::new(Condition::IsNull("email".to_string()))),
            ]),
        ]);
        let sql = condition_to_sql_owned_checked(condition, &mut params).unwrap();
        assert_eq!(
            sql,
            "(NOT (\"status\" = $1) AND (\"age\" > $2 OR NOT (\"email\" IS NULL)))"
        );
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn test_field_expr_placeholder_continues_after_existing_params() {
        let mut params = vec![SqlValue::Int(99)];
        let condition = Condition::FieldExpr(
            "updated_at".to_string(),
            ComparisonOperator::Lt,
            crate::SqlExpr::unix_timestamp_add(30),
        );
        let sql = condition_to_sql_owned_checked(condition, &mut params).unwrap();
        assert_eq!(sql, "\"updated_at\" < EXTRACT(EPOCH FROM now()) + $2");
        assert_eq!(params.len(), 2);
        assert!(matches!(params[1], SqlValue::Int(30)));
    }

    #[test]
    fn test_field_expr_without_param_does_not_consume_numbering() {
        let mut params = Vec::new();
        let condition = Condition::FieldExpr(
            "updated_at".to_string(),
            ComparisonOperator::Gte,
            crate::SqlExpr::now(),
        );
        let sql = condition_to_sql_owned_checked(condition, &mut params).unwrap();
        assert_eq!(sql, "\"updated_at\" >= now()");
        assert!(params.is_empty());
    }

    #[test]
    fn test_new_sql_value_from_conversions() {
        use chrono::TimeZone;

        let decimal = bigdecimal::BigDecimal::from(42);
        assert!(matches!(
            SqlValue::from(decimal),
            SqlValue::Decimal(value) if value.to_string() == "42"
        ));
        let uuid = uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        assert!(matches!(SqlValue::from(uuid), SqlValue::Uuid(_)));
        let date = chrono::NaiveDate::from_ymd_opt(2026, 11, 8).unwrap();
        assert!(matches!(SqlValue::from(date), SqlValue::Date(_)));
        let time = chrono::NaiveTime::from_hms_opt(12, 30, 0).unwrap();
        assert!(matches!(SqlValue::from(time), SqlValue::Time(_)));
        let tz = chrono::Utc.with_ymd_and_hms(2026, 11, 8, 12, 0, 0).unwrap();
        assert!(matches!(SqlValue::from(tz), SqlValue::DateTimeTz(_)));
        assert!(matches!(
            SqlValue::from(chrono::Duration::seconds(2)),
            SqlValue::Interval(2_000_000)
        ));
        assert!(matches!(
            SqlValue::interval_micros(-5),
            SqlValue::Interval(-5)
        ));
    }

    #[test]
    fn test_array_from_vec_conversions_preserve_element_types() {
        assert!(matches!(
            SqlValue::from(vec![1_i64, 2]),
            SqlValue::Array(values)
                if matches!(values.as_slice(), [SqlValue::Int(1), SqlValue::Int(2)])
        ));
        assert!(matches!(
            SqlValue::from(vec!["a".to_string(), "b".to_string()]),
            SqlValue::Array(values)
                if matches!(&values[0], SqlValue::String(value) if value == "a")
                    && matches!(&values[1], SqlValue::String(value) if value == "b")
        ));
        assert!(matches!(
            SqlValue::from(vec![1.5_f64, 2.5]),
            SqlValue::Array(values)
                if matches!(&values[0], SqlValue::Float(value) if *value == 1.5)
                    && matches!(&values[1], SqlValue::Float(value) if *value == 2.5)
        ));
        assert!(matches!(
            SqlValue::from(vec![true, false]),
            SqlValue::Array(values)
                if matches!(values.as_slice(), [SqlValue::Bool(true), SqlValue::Bool(false)])
        ));
        // 空数组是合法值（ANY 恒 false / ALL 恒 true 由 SQL 语义决定）。
        assert!(matches!(
            SqlValue::from(Vec::<i64>::new()),
            SqlValue::Array(values) if values.is_empty()
        ));
    }

    #[test]
    fn test_array_value_renders_as_one_param() {
        let mut params = Vec::new();
        let condition = Condition::Eq(
            "tags".to_string(),
            SqlValue::from(vec!["a".to_string(), "b".to_string()]),
        );
        let sql = condition_to_sql_owned_checked(condition, &mut params).unwrap();
        assert_eq!(sql, "\"tags\" = $1");
        assert_eq!(params.len(), 1, "数组必须作为单个绑定参数渲染");
        assert!(matches!(&params[0], SqlValue::Array(values) if values.len() == 2));
    }

    #[test]
    fn test_any_all_render_array_binding_and_numbering() {
        let mut params = vec![SqlValue::Int(99)];
        let condition = Condition::Any("tags".to_string(), SqlValue::from(vec!["a".to_string()]));
        let sql = condition_to_sql_owned_checked(condition, &mut params).unwrap();
        assert_eq!(sql, "\"tags\" = ANY($2)");
        assert_eq!(params.len(), 2);

        let mut params = Vec::new();
        let condition = Condition::All("nums".to_string(), SqlValue::from(vec![1_i64, 2]));
        let sql = condition_to_sql_owned_checked(condition, &mut params).unwrap();
        assert_eq!(sql, "\"nums\" = ALL($1)");
        assert_eq!(params.len(), 1);
        assert!(matches!(&params[0], SqlValue::Array(values) if values.len() == 2));
    }

    #[test]
    fn test_any_rejects_non_array_value_fail_closed() {
        let mut params = Vec::new();
        let error = condition_to_sql_owned_checked(
            Condition::Any("tags".to_string(), SqlValue::String("a".to_string())),
            &mut params,
        )
        .expect_err("ANY 只接受 SqlValue::Array");
        assert!(matches!(error, DbError::InvalidArgument(_)));
        assert!(params.is_empty(), "失败时不得污染调用方参数");

        let mut params = Vec::new();
        let error = condition_to_sql_owned_checked(
            Condition::All("tags".to_string(), SqlValue::Null),
            &mut params,
        )
        .expect_err("ALL 只接受 SqlValue::Array");
        assert!(matches!(error, DbError::InvalidArgument(_)));
        assert!(params.is_empty());
    }

    #[test]
    fn test_json_predicates_render_operators_and_bind_values() {
        let payload = SqlValue::Json(serde_json::json!({"role": "admin"}));

        let mut params = Vec::new();
        let sql = condition_to_sql_owned_checked(
            Condition::JsonContains("data".to_string(), payload),
            &mut params,
        )
        .unwrap();
        assert_eq!(sql, "(\"data\")::jsonb @> $1");
        assert_eq!(params.len(), 1);
        assert!(matches!(&params[0], SqlValue::Json(_)));

        let mut params = Vec::new();
        let sql = condition_to_sql_owned_checked(
            Condition::JsonContainedBy(
                "data".to_string(),
                SqlValue::Json(serde_json::json!({"role": "admin"})),
            ),
            &mut params,
        )
        .unwrap();
        assert_eq!(sql, "(\"data\")::jsonb <@ $1");
        assert_eq!(params.len(), 1);

        let mut params = Vec::new();
        let sql = condition_to_sql_owned_checked(
            Condition::JsonHasKey("data".to_string(), "weird'key".to_string()),
            &mut params,
        )
        .unwrap();
        assert_eq!(
            sql, "(\"data\")::jsonb ? $1",
            "键名必须参数化，不得拼接进 SQL 文本"
        );
        assert!(!sql.contains("weird'key"));
        assert!(matches!(&params[0], SqlValue::String(key) if key == "weird'key"));
    }

    #[test]
    fn test_json_predicate_null_renders_sql_null_without_param() {
        let mut params = Vec::new();
        let sql = condition_to_sql_owned_checked(
            Condition::JsonContains("data".to_string(), SqlValue::Null),
            &mut params,
        )
        .unwrap();
        assert_eq!(sql, "NULL");
        assert!(params.is_empty());
    }

    #[test]
    fn test_json_predicate_rejects_non_json_value() {
        let mut params = Vec::new();
        let error = condition_to_sql_owned_checked(
            Condition::JsonContainedBy("data".to_string(), SqlValue::Int(1)),
            &mut params,
        )
        .expect_err("JSON 谓词只接受 SqlValue::Json");
        assert!(matches!(error, DbError::InvalidArgument(_)));
        assert!(params.is_empty());
    }

    #[test]
    fn test_json_predicates_compose_and_number_sequentially() {
        let mut params = Vec::new();
        let condition = Condition::And(vec![
            Condition::JsonContains(
                "data".to_string(),
                SqlValue::Json(serde_json::json!({"role": "admin"})),
            ),
            Condition::JsonHasKey("data".to_string(), "active".to_string()),
        ]);
        let sql = condition_to_sql_owned_checked(condition, &mut params).unwrap();
        assert_eq!(sql, "((\"data\")::jsonb @> $1 AND (\"data\")::jsonb ? $2)");
        assert_eq!(params.len(), 2);
        assert!(matches!(&params[1], SqlValue::String(key) if key == "active"));
    }

    #[test]
    fn test_range_predicates_render_operators_and_explicit_casts() {
        let mut params = Vec::new();
        let sql = condition_to_sql_owned_checked(
            Condition::RangeContains("span".to_string(), SqlValue::Range("[1,10)".to_string())),
            &mut params,
        )
        .unwrap();
        assert_eq!(sql, "\"span\" @> $1::int4range");
        assert_eq!(params.len(), 1);
        assert!(matches!(&params[0], SqlValue::Range(text) if text == "[1,10)"));

        let mut params = vec![SqlValue::Int(99)];
        let sql = condition_to_sql_owned_checked(
            Condition::RangeOverlaps(
                "spans".to_string(),
                SqlValue::Multirange("{[1,10),[20,30)}".to_string()),
            ),
            &mut params,
        )
        .unwrap();
        assert_eq!(sql, "\"spans\" && $2::int4multirange");
        assert_eq!(params.len(), 2, "范围值必须作为单个绑定参数");
    }

    #[test]
    fn test_range_predicates_reject_non_range_values_and_null_fails_closed() {
        let mut params = Vec::new();
        let error = condition_to_sql_owned_checked(
            Condition::RangeContains("span".to_string(), SqlValue::String("[1,10)".to_string())),
            &mut params,
        )
        .expect_err("范围谓词只接受 Range/Multirange");
        assert!(matches!(error, DbError::InvalidArgument(_)));
        assert!(params.is_empty());

        let mut params = Vec::new();
        let sql = condition_to_sql_owned_checked(
            Condition::RangeOverlaps("span".to_string(), SqlValue::Null),
            &mut params,
        )
        .unwrap();
        assert_eq!(sql, "NULL");
        assert!(params.is_empty());
    }

    #[test]
    fn test_json_path_predicates_bind_path_and_vars() {
        let mut params = Vec::new();
        let sql = condition_to_sql_owned_checked(
            Condition::JsonPathExists("data".to_string(), "$.role".to_string()),
            &mut params,
        )
        .unwrap();
        assert_eq!(sql, "jsonb_path_exists((\"data\")::jsonb, $1::jsonpath)");
        assert!(!sql.contains("$.role"), "路径必须参数化，不得拼进 SQL");
        assert!(matches!(&params[0], SqlValue::String(path) if path == "$.role"));

        let mut params = Vec::new();
        let sql = condition_to_sql_owned_checked(
            Condition::JsonPathMatch(
                "data".to_string(),
                "$.age > $min".to_string(),
                SqlValue::Json(serde_json::json!({"min": 18})),
            ),
            &mut params,
        )
        .unwrap();
        assert_eq!(
            sql,
            "jsonb_path_match((\"data\")::jsonb, $1::jsonpath, $2::jsonb)"
        );
        assert_eq!(params.len(), 2);
        assert!(!sql.contains("min"));
    }

    #[test]
    fn test_full_text_predicates_use_simple_config_and_bind_query() {
        let mut params = Vec::new();
        let sql = condition_to_sql_owned_checked(
            Condition::FullText("body".to_string(), "hello world".to_string()),
            &mut params,
        )
        .unwrap();
        assert_eq!(
            sql,
            "to_tsvector('simple', \"body\") @@ plainto_tsquery('simple', $1)"
        );
        assert!(!sql.contains("hello world"));
        assert!(matches!(&params[0], SqlValue::String(query) if query == "hello world"));

        let mut params = Vec::new();
        let sql = condition_to_sql_owned_checked(
            Condition::FullTextQuery("body".to_string(), "hello & world".to_string()),
            &mut params,
        )
        .unwrap();
        assert_eq!(
            sql,
            "to_tsvector('simple', \"body\") @@ to_tsquery('simple', $1)"
        );
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn test_phase_c_conditions_number_sequentially_inside_and() {
        let mut params = Vec::new();
        let condition = Condition::And(vec![
            Condition::FullText("body".to_string(), "rust".to_string()),
            Condition::JsonPathExists("meta".to_string(), "$.ok".to_string()),
            Condition::RangeContains("span".to_string(), SqlValue::Range("[1,5)".to_string())),
        ]);
        let sql = condition_to_sql_owned_checked(condition, &mut params).unwrap();
        assert_eq!(params.len(), 3);
        assert!(sql.contains("$1"));
        assert!(sql.contains("$2::jsonpath"));
        assert!(sql.contains("$3::int4range"));
    }
}
