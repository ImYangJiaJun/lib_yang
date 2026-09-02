#![allow(deprecated)]

use chrono::NaiveDateTime;
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
}
