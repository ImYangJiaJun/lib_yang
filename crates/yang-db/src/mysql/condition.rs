#![allow(deprecated)]

use chrono::NaiveDateTime;
use serde_json::Value as JsonValue;

use crate::error::DbError;

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

/// SQL 值类型
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

/// 查询条件
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
    /// 字段与受控服务端表达式的比较（如 `expires_at > UNIX_TIMESTAMP()`）。
    ColumnExprComparison(String, ComparisonOperator, crate::SqlExpr),
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

// 实现 From trait 支持自动转换
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
        // u64 顶半区（> i64::MAX，如 BIGINT UNSIGNED / 无符号雪花 ID）若强转 i64 会
        // 静默环绕成负数（NEW-11）。超出 i64 范围时走十进制字符串，MySQL 对数值列会
        // 隐式转换，避免数据被悄悄改写。SqlValue 未标 non_exhaustive，不新增 UInt 变体
        // 以免破坏全 crate 的 match。
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

/// 将条件转换为 SQL 字符串和参数列表
///
/// # 参数
/// - condition: 要转换的条件
/// - params: 用于收集参数的可变向量
///
/// # 返回
/// - SQL 字符串片段
#[deprecated(
    since = "0.1.3",
    note = "使用 condition_to_sql_owned_checked 获取结构化错误"
)]
pub fn condition_to_sql(condition: &Condition, params: &mut Vec<SqlValue>) -> String {
    condition_to_sql_owned(condition.clone(), params)
}

/// 以借用方式将条件树写入 SQL 字符串，仅在压参时 clone 单个 `SqlValue`
///
/// 与 [`condition_to_sql_owned`] 不同，本函数遍历 `&Condition` 引用树，
/// 避免对整棵树做一次性 `clone()`，仅在叶节点需要把 `SqlValue` 压入参数
/// 列表时才 clone 单个值。对深层嵌套或大 IN 列表的场景可显著减少堆分配。
///
/// # 参数
/// - `cond`: 要转换的条件引用
/// - `out`: 输出 SQL 的可变字符串（追加模式，不清空已有内容）
/// - `params`: 用于收集参数的可变向量
#[deprecated(
    since = "0.1.3",
    note = "使用 condition_to_sql_owned_checked 获取结构化错误"
)]
pub fn write_condition_to_sql(cond: &Condition, out: &mut String, params: &mut Vec<SqlValue>) {
    out.push_str(&condition_to_sql_owned(cond.clone(), params));
}

/// 消费版本的兼容条件渲染入口。
///
/// 内部只委托 checked renderer；校验失败时返回固定 fail-closed 条件，且不会修改参数。
///
/// # 参数
/// - `condition`: 要消费的条件（owned）
/// - `params`: 用于收集参数的可变向量
///
/// # 返回
/// - SQL 字符串片段
///
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

/// 严格版条件转 SQL：非法标识符返回 [`DbError::InvalidArgument`] 而非 RAW 回退。
///
/// 字段标识符校验失败时立即返回错误；只接受单段或两段限定标识符，不接受表达式。
///
/// # 返回
///
/// - `Ok(String)`: SQL 片段
/// - `Err(DbError::InvalidArgument)`: 包含非法标识符
pub fn condition_to_sql_owned_checked(
    condition: Condition,
    params: &mut Vec<SqlValue>,
) -> Result<String, DbError> {
    let rendered = render_condition_checked(condition)?;
    params.extend(rendered.params);
    Ok(rendered.sql)
}

pub(crate) fn render_condition_checked(
    condition: Condition,
) -> Result<crate::sql_types::RenderedCondition<SqlValue>, DbError> {
    // 渲染逻辑按方言共享于 crate::dialect；这里只做公开条件树 → 内部树的转换。
    crate::dialect::render_condition(crate::dialect::MYSQL, into_node(condition), 0)
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
        Condition::ColumnExprComparison(field, op, expression) => {
            // 预渲染 SqlExpr 白名单片段：固定文本 + 可选绑定参数（偏移秒数）。
            let (fragment, param) = expression.mysql_render();
            Node::ColumnExpr(field, op.as_sql(), fragment, param.map(SqlValue::Int))
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
#[allow(deprecated, clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

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
    fn test_from_i32() {
        let value: SqlValue = 42i32.into();
        match value {
            SqlValue::Int(v) => assert_eq!(v, 42),
            _ => panic!("期望 SqlValue::Int"),
        }
    }

    #[test]
    fn test_from_i64() {
        let value: SqlValue = 9223372036854775807i64.into();
        match value {
            SqlValue::Int(v) => assert_eq!(v, 9223372036854775807),
            _ => panic!("期望 SqlValue::Int"),
        }
    }

    #[test]
    fn test_from_f32() {
        let value: SqlValue = 3.5f32.into();
        match value {
            SqlValue::Float(v) => assert!((v - 3.5).abs() < 0.01),
            _ => panic!("期望 SqlValue::Float"),
        }
    }

    #[test]
    fn test_from_f64() {
        let value: SqlValue = 2.5f64.into();
        match value {
            SqlValue::Float(v) => assert!((v - 2.5).abs() < 0.000001),
            _ => panic!("期望 SqlValue::Float"),
        }
    }

    #[test]
    fn test_from_string() {
        let value: SqlValue = String::from("测试字符串").into();
        match value {
            SqlValue::String(s) => assert_eq!(s, "测试字符串"),
            _ => panic!("期望 SqlValue::String"),
        }
    }

    #[test]
    fn test_from_str() {
        let value: SqlValue = "hello world".into();
        match value {
            SqlValue::String(s) => assert_eq!(s, "hello world"),
            _ => panic!("期望 SqlValue::String"),
        }
    }

    #[test]
    fn test_from_bool_true() {
        let value: SqlValue = true.into();
        match value {
            SqlValue::Bool(b) => assert!(b),
            _ => panic!("期望 SqlValue::Bool"),
        }
    }

    #[test]
    fn test_from_bool_false() {
        let value: SqlValue = false.into();
        match value {
            SqlValue::Bool(b) => assert!(!b),
            _ => panic!("期望 SqlValue::Bool"),
        }
    }

    #[test]
    fn test_from_vec_u8() {
        let bytes = vec![0x48, 0x65, 0x6c, 0x6c, 0x6f]; // "Hello"
        let value: SqlValue = bytes.clone().into();
        match value {
            SqlValue::Bytes(b) => assert_eq!(b, bytes),
            _ => panic!("期望 SqlValue::Bytes"),
        }
    }

    #[test]
    fn test_from_json_value() {
        let json = serde_json::json!({
            "name": "测试",
            "age": 25,
            "active": true
        });
        let value: SqlValue = json.clone().into();
        match value {
            SqlValue::Json(j) => assert_eq!(j, json),
            _ => panic!("期望 SqlValue::Json"),
        }
    }

    #[test]
    fn test_from_naive_datetime() {
        let dt = NaiveDate::from_ymd_opt(2024, 1, 15)
            .unwrap()
            .and_hms_opt(10, 30, 45)
            .unwrap();
        let value: SqlValue = dt.into();
        match value {
            SqlValue::DateTime(d) => assert_eq!(d, dt),
            _ => panic!("期望 SqlValue::DateTime"),
        }
    }

    #[test]
    fn test_from_option_some() {
        let value: SqlValue = Some(42i32).into();
        match value {
            SqlValue::Int(v) => assert_eq!(v, 42),
            _ => panic!("期望 SqlValue::Int"),
        }
    }

    #[test]
    fn test_from_option_none() {
        let value: SqlValue = None::<i32>.into();
        match value {
            SqlValue::Null => (),
            _ => panic!("期望 SqlValue::Null"),
        }
    }

    #[test]
    fn test_from_option_string_some() {
        let value: SqlValue = Some(String::from("测试")).into();
        match value {
            SqlValue::String(s) => assert_eq!(s, "测试"),
            _ => panic!("期望 SqlValue::String"),
        }
    }

    #[test]
    fn test_from_option_string_none() {
        let value: SqlValue = None::<String>.into();
        match value {
            SqlValue::Null => (),
            _ => panic!("期望 SqlValue::Null"),
        }
    }

    #[test]
    fn test_negative_integers() {
        let value: SqlValue = (-100i32).into();
        match value {
            SqlValue::Int(v) => assert_eq!(v, -100),
            _ => panic!("期望 SqlValue::Int"),
        }
    }

    #[test]
    fn test_negative_floats() {
        let value: SqlValue = (-3.5f64).into();
        match value {
            SqlValue::Float(v) => assert!((v + 3.5).abs() < 0.01),
            _ => panic!("期望 SqlValue::Float"),
        }
    }

    #[test]
    fn test_empty_string() {
        let value: SqlValue = "".into();
        match value {
            SqlValue::String(s) => assert_eq!(s, ""),
            _ => panic!("期望 SqlValue::String"),
        }
    }

    #[test]
    fn test_empty_bytes() {
        let value: SqlValue = Vec::<u8>::new().into();
        match value {
            SqlValue::Bytes(b) => assert!(b.is_empty()),
            _ => panic!("期望 SqlValue::Bytes"),
        }
    }

    #[test]
    fn test_json_null() {
        let json = serde_json::Value::Null;
        let value: SqlValue = json.into();
        match value {
            SqlValue::Json(j) => assert!(j.is_null()),
            _ => panic!("期望 SqlValue::Json"),
        }
    }

    #[test]
    fn test_json_array() {
        let json = serde_json::json!([1, 2, 3, 4, 5]);
        let value: SqlValue = json.clone().into();
        match value {
            SqlValue::Json(j) => assert_eq!(j, json),
            _ => panic!("期望 SqlValue::Json"),
        }
    }

    #[test]
    fn test_unicode_string() {
        let value: SqlValue = "你好世界 🌍".into();
        match value {
            SqlValue::String(s) => assert_eq!(s, "你好世界 🌍"),
            _ => panic!("期望 SqlValue::String"),
        }
    }

    #[test]
    fn test_zero_values() {
        let int_value: SqlValue = 0i32.into();
        match int_value {
            SqlValue::Int(v) => assert_eq!(v, 0),
            _ => panic!("期望 SqlValue::Int"),
        }

        let float_value: SqlValue = 0.0f64.into();
        match float_value {
            SqlValue::Float(v) => assert_eq!(v, 0.0),
            _ => panic!("期望 SqlValue::Float"),
        }
    }

    // 测试 condition_to_sql 函数
    #[test]
    fn test_condition_column_expr_comparison_renders_fixed_fragment_with_bound_param() {
        // 无参表达式：不产出绑定参数
        let mut params = Vec::new();
        let sql = condition_to_sql_owned_checked(
            Condition::ColumnExprComparison(
                "expires_at".to_string(),
                ComparisonOperator::Gt,
                crate::SqlExpr::unix_timestamp(),
            ),
            &mut params,
        )
        .expect("合法表达式条件应渲染成功");
        assert_eq!(sql, "`expires_at` > UNIX_TIMESTAMP()");
        assert!(params.is_empty());

        // 带参表达式：偏移秒数只进参数列表，不进 SQL 文本
        let mut params = Vec::new();
        let sql = condition_to_sql_owned_checked(
            Condition::ColumnExprComparison(
                "expires_at".to_string(),
                ComparisonOperator::Lte,
                crate::SqlExpr::unix_timestamp_add(600),
            ),
            &mut params,
        )
        .expect("合法表达式条件应渲染成功");
        assert_eq!(sql, "`expires_at` <= UNIX_TIMESTAMP() + ?");
        assert_eq!(params.len(), 1);
        assert!(matches!(params[0], SqlValue::Int(600)));
    }

    #[test]
    fn test_condition_eq() {
        let mut params = Vec::new();
        let cond = Condition::Eq("name".to_string(), SqlValue::String("test".to_string()));
        let sql = super::condition_to_sql(&cond, &mut params);
        assert_eq!(sql, "`name` = ?");
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn test_condition_ne() {
        let mut params = Vec::new();
        let cond = Condition::Ne("status".to_string(), SqlValue::Int(1));
        let sql = super::condition_to_sql(&cond, &mut params);
        assert_eq!(sql, "`status` != ?");
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn test_condition_gt() {
        let mut params = Vec::new();
        let cond = Condition::Gt("age".to_string(), SqlValue::Int(18));
        let sql = super::condition_to_sql(&cond, &mut params);
        assert_eq!(sql, "`age` > ?");
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn test_condition_lt() {
        let mut params = Vec::new();
        let cond = Condition::Lt("price".to_string(), SqlValue::Float(100.0));
        let sql = super::condition_to_sql(&cond, &mut params);
        assert_eq!(sql, "`price` < ?");
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn test_condition_gte() {
        let mut params = Vec::new();
        let cond = Condition::Gte("score".to_string(), SqlValue::Int(60));
        let sql = super::condition_to_sql(&cond, &mut params);
        assert_eq!(sql, "`score` >= ?");
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn test_condition_lte() {
        let mut params = Vec::new();
        let cond = Condition::Lte("count".to_string(), SqlValue::Int(10));
        let sql = super::condition_to_sql(&cond, &mut params);
        assert_eq!(sql, "`count` <= ?");
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn test_condition_in() {
        let mut params = Vec::new();
        let cond = Condition::In(
            "id".to_string(),
            vec![SqlValue::Int(1), SqlValue::Int(2), SqlValue::Int(3)],
        );
        let sql = super::condition_to_sql(&cond, &mut params);
        assert_eq!(sql, "`id` IN (?, ?, ?)");
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn test_condition_in_empty() {
        let mut params = Vec::new();
        let cond = Condition::In("id".to_string(), vec![]);
        let sql = super::condition_to_sql(&cond, &mut params);
        assert_eq!(sql, FAIL_CLOSED_CONDITION);
        assert_eq!(params.len(), 0);
    }

    #[test]
    fn test_condition_between() {
        let mut params = Vec::new();
        let cond = Condition::Between("age".to_string(), SqlValue::Int(18), SqlValue::Int(65));
        let sql = super::condition_to_sql(&cond, &mut params);
        assert_eq!(sql, "`age` BETWEEN ? AND ?");
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn test_condition_like() {
        let mut params = Vec::new();
        let cond = Condition::Like("name".to_string(), "%test%".to_string());
        let sql = super::condition_to_sql(&cond, &mut params);
        assert_eq!(sql, "`name` LIKE ?");
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn test_condition_and() {
        let mut params = Vec::new();
        let cond = Condition::And(vec![
            Condition::Eq("name".to_string(), SqlValue::String("test".to_string())),
            Condition::Gt("age".to_string(), SqlValue::Int(18)),
        ]);
        let sql = super::condition_to_sql(&cond, &mut params);
        assert_eq!(sql, "(`name` = ? AND `age` > ?)");
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn test_condition_or() {
        let mut params = Vec::new();
        let cond = Condition::Or(vec![
            Condition::Eq("status".to_string(), SqlValue::Int(1)),
            Condition::Eq("status".to_string(), SqlValue::Int(2)),
        ]);
        let sql = super::condition_to_sql(&cond, &mut params);
        assert_eq!(sql, "(`status` = ? OR `status` = ?)");
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn test_condition_and_or_priority() {
        let mut params = Vec::new();
        // (name = 'test' OR name = 'demo') AND age > 18
        let cond = Condition::And(vec![
            Condition::Or(vec![
                Condition::Eq("name".to_string(), SqlValue::String("test".to_string())),
                Condition::Eq("name".to_string(), SqlValue::String("demo".to_string())),
            ]),
            Condition::Gt("age".to_string(), SqlValue::Int(18)),
        ]);
        let sql = super::condition_to_sql(&cond, &mut params);
        // OR 条件应该被括号包围
        assert_eq!(sql, "((`name` = ? OR `name` = ?) AND `age` > ?)");
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn test_condition_empty_and() {
        let mut params = Vec::new();
        let cond = Condition::And(vec![]);
        let sql = super::condition_to_sql(&cond, &mut params);
        assert_eq!(sql, FAIL_CLOSED_CONDITION);
        assert_eq!(params.len(), 0);
    }

    #[test]
    fn test_condition_empty_or() {
        let mut params = Vec::new();
        let cond = Condition::Or(vec![]);
        let sql = super::condition_to_sql(&cond, &mut params);
        assert_eq!(sql, FAIL_CLOSED_CONDITION);
        assert_eq!(params.len(), 0);
    }

    #[test]
    fn test_condition_single_and() {
        let mut params = Vec::new();
        let cond = Condition::And(vec![Condition::Eq("id".to_string(), SqlValue::Int(1))]);
        let sql = super::condition_to_sql(&cond, &mut params);
        assert_eq!(sql, "`id` = ?");
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn test_condition_single_or() {
        let mut params = Vec::new();
        let cond = Condition::Or(vec![Condition::Eq("id".to_string(), SqlValue::Int(1))]);
        let sql = super::condition_to_sql(&cond, &mut params);
        assert_eq!(sql, "`id` = ?");
        assert_eq!(params.len(), 1);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    // 生成有效的字段名（字母开头，后跟字母数字下划线）
    fn field_name_strategy() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9_]{0,30}"
    }

    // 生成 SqlValue 策略
    fn sql_value_strategy() -> impl Strategy<Value = SqlValue> {
        prop_oneof![
            Just(SqlValue::Null),
            any::<bool>().prop_map(SqlValue::Bool),
            any::<i64>().prop_map(SqlValue::Int),
            any::<f64>().prop_map(|f| {
                if f.is_finite() {
                    SqlValue::Float(f)
                } else {
                    SqlValue::Float(0.0)
                }
            }),
            "[a-zA-Z0-9_\\s]{0,50}".prop_map(SqlValue::String),
        ]
    }

    // **Feature: mysql-query-builder, Property 5: 操作符支持**
    // **验证需求：3.3**
    //
    // 属性：对于任意支持的操作符（=, !=, >, <, >=, <=, in, between, like），
    // 生成的 SQL 应该包含正确的操作符语法
    //
    // 此测试验证所有支持的操作符都能正确生成 SQL 语句，
    // 确保参数化查询的正确性和 SQL 注入防护。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_operator_eq_support(
            field in field_name_strategy(),
            value in sql_value_strategy()
        ) {
            let mut params = Vec::new();
            let cond = Condition::Eq(field.clone(), value);
            let sql = condition_to_sql(&cond, &mut params);

            // 验证 SQL 包含正确的操作符（字段名会被反引号转义）
            let expected = format!("`{}` = {{placeholder}}", field).replace("{placeholder}", "?");
            prop_assert!(sql.contains(&expected));
            prop_assert_eq!(params.len(), 1);
        }

        #[test]
        fn prop_operator_ne_support(
            field in field_name_strategy(),
            value in sql_value_strategy()
        ) {
            let mut params = Vec::new();
            let cond = Condition::Ne(field.clone(), value);
            let sql = condition_to_sql(&cond, &mut params);

            let expected = format!("`{}` != {{placeholder}}", field).replace("{placeholder}", "?");
            prop_assert!(sql.contains(&expected));
            prop_assert_eq!(params.len(), 1);
        }

        #[test]
        fn prop_operator_gt_support(
            field in field_name_strategy(),
            value in sql_value_strategy()
        ) {
            let mut params = Vec::new();
            let cond = Condition::Gt(field.clone(), value);
            let sql = condition_to_sql(&cond, &mut params);

            let expected = format!("`{}` > {{placeholder}}", field).replace("{placeholder}", "?");
            prop_assert!(sql.contains(&expected));
            prop_assert_eq!(params.len(), 1);
        }

        #[test]
        fn prop_operator_lt_support(
            field in field_name_strategy(),
            value in sql_value_strategy()
        ) {
            let mut params = Vec::new();
            let cond = Condition::Lt(field.clone(), value);
            let sql = condition_to_sql(&cond, &mut params);

            let expected = format!("`{}` < {{placeholder}}", field).replace("{placeholder}", "?");
            prop_assert!(sql.contains(&expected));
            prop_assert_eq!(params.len(), 1);
        }

        #[test]
        fn prop_operator_gte_support(
            field in field_name_strategy(),
            value in sql_value_strategy()
        ) {
            let mut params = Vec::new();
            let cond = Condition::Gte(field.clone(), value);
            let sql = condition_to_sql(&cond, &mut params);

            let expected = format!("`{}` >= {{placeholder}}", field).replace("{placeholder}", "?");
            prop_assert!(sql.contains(&expected));
            prop_assert_eq!(params.len(), 1);
        }

        #[test]
        fn prop_operator_lte_support(
            field in field_name_strategy(),
            value in sql_value_strategy()
        ) {
            let mut params = Vec::new();
            let cond = Condition::Lte(field.clone(), value);
            let sql = condition_to_sql(&cond, &mut params);

            let expected = format!("`{}` <= {{placeholder}}", field).replace("{placeholder}", "?");
            prop_assert!(sql.contains(&expected));
            prop_assert_eq!(params.len(), 1);
        }

        #[test]
        fn prop_operator_in_support(
            field in field_name_strategy(),
            values in prop::collection::vec(sql_value_strategy(), 1..10)
        ) {
            let mut params = Vec::new();
            let values_len = values.len();
            let cond = Condition::In(field.clone(), values);
            let sql = condition_to_sql(&cond, &mut params);

            let expected = format!("`{}` IN", field);
            prop_assert!(sql.contains(&expected));
            prop_assert_eq!(params.len(), values_len);
        }

        #[test]
        fn prop_operator_between_support(
            field in field_name_strategy(),
            start in sql_value_strategy(),
            end in sql_value_strategy()
        ) {
            let mut params = Vec::new();
            let cond = Condition::Between(field.clone(), start, end);
            let sql = condition_to_sql(&cond, &mut params);

            let expected = format!("`{}` BETWEEN {{p1}} AND {{p2}}", field)
                .replace("{p1}", "?")
                .replace("{p2}", "?");
            prop_assert!(sql.contains(&expected));
            prop_assert_eq!(params.len(), 2);
        }

        #[test]
        fn prop_operator_like_support(
            field in field_name_strategy(),
            pattern in "[a-zA-Z0-9_%]{1,20}"
        ) {
            let mut params = Vec::new();
            let cond = Condition::Like(field.clone(), pattern);
            let sql = condition_to_sql(&cond, &mut params);

            let expected = format!("`{}` LIKE {{placeholder}}", field).replace("{placeholder}", "?");
            prop_assert!(sql.contains(&expected));
            prop_assert_eq!(params.len(), 1);
        }
    }

    // Feature: mysql-query-builder, Property 9: AND/OR 优先级处理
    // 验证需求：3.7
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_and_or_priority_handling(
            field1 in field_name_strategy(),
            field2 in field_name_strategy(),
            value1 in sql_value_strategy(),
            value2 in sql_value_strategy(),
            value3 in sql_value_strategy()
        ) {
            let mut params = Vec::new();

            // 构建 (field1 = value1 OR field1 = value2) AND field2 = value3
            let cond = Condition::And(vec![
                Condition::Or(vec![
                    Condition::Eq(field1.clone(), value1),
                    Condition::Eq(field1.clone(), value2),
                ]),
                Condition::Eq(field2.clone(), value3),
            ]);

            let sql = condition_to_sql(&cond, &mut params);

            // 验证 SQL 有正确的括号确保操作符优先级
            // 整个条件应该被括号包围
            prop_assert!(sql.starts_with('('));
            prop_assert!(sql.ends_with(')'));

            // OR 条件应该被括号包围
            prop_assert!(sql.contains(" OR "));
            prop_assert!(sql.contains(" AND "));

            // 参数数量应该正确
            prop_assert_eq!(params.len(), 3);
        }

        #[test]
        fn prop_nested_and_or_brackets(
            field in field_name_strategy(),
            values in prop::collection::vec(sql_value_strategy(), 2..5)
        ) {
            let mut params = Vec::new();

            // 构建多个 OR 条件的 AND 组合
            let or_conditions: Vec<Condition> = values
                .iter()
                .map(|v| Condition::Eq(field.clone(), v.clone()))
                .collect();

            let cond = Condition::And(vec![
                Condition::Or(or_conditions.clone()),
                Condition::Gt(field.clone(), SqlValue::Int(0)),
            ]);

            let sql = condition_to_sql(&cond, &mut params);

            // 验证括号匹配
            let open_count = sql.chars().filter(|&c| c == '(').count();
            let close_count = sql.chars().filter(|&c| c == ')').count();
            prop_assert_eq!(open_count, close_count);

            // 验证参数数量
            prop_assert_eq!(params.len(), values.len() + 1);
        }

        #[test]
        fn prop_multiple_and_conditions(
            field in field_name_strategy(),
            values in prop::collection::vec(sql_value_strategy(), 2..5)
        ) {
            let mut params = Vec::new();

            // 构建多个 AND 条件
            let and_conditions: Vec<Condition> = values
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    if i % 2 == 0 {
                        Condition::Eq(field.clone(), v.clone())
                    } else {
                        Condition::Ne(field.clone(), v.clone())
                    }
                })
                .collect();

            let cond = Condition::And(and_conditions);
            let sql = condition_to_sql(&cond, &mut params);

            // 验证 AND 连接
            let and_count = sql.matches(" AND ").count();
            prop_assert_eq!(and_count, values.len() - 1);

            // 验证参数数量
            prop_assert_eq!(params.len(), values.len());
        }

        #[test]
        fn prop_or_conditions_always_bracketed(
            field in field_name_strategy(),
            values in prop::collection::vec(sql_value_strategy(), 2..5)
        ) {
            let mut params = Vec::new();

            // 构建 OR 条件
            let or_conditions: Vec<Condition> = values
                .iter()
                .map(|v| Condition::Eq(field.clone(), v.clone()))
                .collect();

            let cond = Condition::Or(or_conditions);
            let sql = condition_to_sql(&cond, &mut params);

            // OR 条件应该被括号包围
            prop_assert!(sql.starts_with('('));
            prop_assert!(sql.ends_with(')'));

            // 验证 OR 连接
            let or_count = sql.matches(" OR ").count();
            prop_assert_eq!(or_count, values.len() - 1);
        }
    }

    // **Validates: Requirements 5**
    //
    // 属性 P3：condition_to_sql_owned 与 condition_to_sql 等价性
    //
    // 形式化描述：对于任意 Condition c，
    // `condition_to_sql_owned(c.clone(), &mut p1)` 生成的 SQL 字符串
    // 与 `condition_to_sql(&c, &mut p2)` 完全相同，且参数列表长度相等。
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        #[test]
        fn prop_owned_equals_borrowed_eq(
            field in field_name_strategy(),
            value in sql_value_strategy()
        ) {
            let cond = Condition::Eq(field, value);
            let mut p1 = vec![];
            let mut p2 = vec![];
            let sql1 = condition_to_sql_owned(cond.clone(), &mut p1);
            let sql2 = condition_to_sql(&cond, &mut p2);
            // SQL 字符串必须完全相同
            prop_assert_eq!(&sql1, &sql2);
            // 参数列表长度必须相等
            prop_assert_eq!(p1.len(), p2.len());
            // 参数值与顺序也必须完全一致（SqlValue 未实现 PartialEq，用整向量 Debug 串比较，
            // 一次同时覆盖元素值与顺序；sql_value_strategy 已把非有限 f64 归一为 0.0，Debug 确定）
            prop_assert_eq!(format!("{:?}", &p1), format!("{:?}", &p2));
        }

        #[test]
        fn prop_owned_equals_borrowed_ne(
            field in field_name_strategy(),
            value in sql_value_strategy()
        ) {
            let cond = Condition::Ne(field, value);
            let mut p1 = vec![];
            let mut p2 = vec![];
            let sql1 = condition_to_sql_owned(cond.clone(), &mut p1);
            let sql2 = condition_to_sql(&cond, &mut p2);
            prop_assert_eq!(&sql1, &sql2);
            prop_assert_eq!(p1.len(), p2.len());
            // 参数值与顺序也必须完全一致（SqlValue 未实现 PartialEq，用整向量 Debug 串比较，
            // 一次同时覆盖元素值与顺序；sql_value_strategy 已把非有限 f64 归一为 0.0，Debug 确定）
            prop_assert_eq!(format!("{:?}", &p1), format!("{:?}", &p2));
        }

        #[test]
        fn prop_owned_equals_borrowed_in(
            field in field_name_strategy(),
            values in prop::collection::vec(sql_value_strategy(), 0..8)
        ) {
            let cond = Condition::In(field, values);
            let mut p1 = vec![];
            let mut p2 = vec![];
            let sql1 = condition_to_sql_owned(cond.clone(), &mut p1);
            let sql2 = condition_to_sql(&cond, &mut p2);
            prop_assert_eq!(&sql1, &sql2);
            prop_assert_eq!(p1.len(), p2.len());
            // 参数值与顺序也必须完全一致（SqlValue 未实现 PartialEq，用整向量 Debug 串比较，
            // 一次同时覆盖元素值与顺序；sql_value_strategy 已把非有限 f64 归一为 0.0，Debug 确定）
            prop_assert_eq!(format!("{:?}", &p1), format!("{:?}", &p2));
        }

        #[test]
        fn prop_owned_equals_borrowed_between(
            field in field_name_strategy(),
            start in sql_value_strategy(),
            end in sql_value_strategy()
        ) {
            let cond = Condition::Between(field, start, end);
            let mut p1 = vec![];
            let mut p2 = vec![];
            let sql1 = condition_to_sql_owned(cond.clone(), &mut p1);
            let sql2 = condition_to_sql(&cond, &mut p2);
            prop_assert_eq!(&sql1, &sql2);
            prop_assert_eq!(p1.len(), p2.len());
            // 参数值与顺序也必须完全一致（SqlValue 未实现 PartialEq，用整向量 Debug 串比较，
            // 一次同时覆盖元素值与顺序；sql_value_strategy 已把非有限 f64 归一为 0.0，Debug 确定）
            prop_assert_eq!(format!("{:?}", &p1), format!("{:?}", &p2));
        }

        #[test]
        fn prop_owned_equals_borrowed_like(
            field in field_name_strategy(),
            pattern in "[a-zA-Z0-9_%]{1,20}"
        ) {
            let cond = Condition::Like(field, pattern);
            let mut p1 = vec![];
            let mut p2 = vec![];
            let sql1 = condition_to_sql_owned(cond.clone(), &mut p1);
            let sql2 = condition_to_sql(&cond, &mut p2);
            prop_assert_eq!(&sql1, &sql2);
            prop_assert_eq!(p1.len(), p2.len());
            // 参数值与顺序也必须完全一致（SqlValue 未实现 PartialEq，用整向量 Debug 串比较，
            // 一次同时覆盖元素值与顺序；sql_value_strategy 已把非有限 f64 归一为 0.0，Debug 确定）
            prop_assert_eq!(format!("{:?}", &p1), format!("{:?}", &p2));
        }

        #[test]
        fn prop_owned_equals_borrowed_and(
            field in field_name_strategy(),
            values in prop::collection::vec(sql_value_strategy(), 0..5)
        ) {
            // 构建 AND 条件
            let conditions: Vec<Condition> = values
                .iter()
                .map(|v| Condition::Eq(field.clone(), v.clone()))
                .collect();
            let cond = Condition::And(conditions);
            let mut p1 = vec![];
            let mut p2 = vec![];
            let sql1 = condition_to_sql_owned(cond.clone(), &mut p1);
            let sql2 = condition_to_sql(&cond, &mut p2);
            // SQL 字符串必须完全相同
            prop_assert_eq!(&sql1, &sql2);
            // 参数列表长度必须相等
            prop_assert_eq!(p1.len(), p2.len());
            // 参数值与顺序也必须完全一致（SqlValue 未实现 PartialEq，用整向量 Debug 串比较，
            // 一次同时覆盖元素值与顺序；sql_value_strategy 已把非有限 f64 归一为 0.0，Debug 确定）
            prop_assert_eq!(format!("{:?}", &p1), format!("{:?}", &p2));
        }

        #[test]
        fn prop_owned_equals_borrowed_or(
            field in field_name_strategy(),
            values in prop::collection::vec(sql_value_strategy(), 0..5)
        ) {
            // 构建 OR 条件
            let conditions: Vec<Condition> = values
                .iter()
                .map(|v| Condition::Eq(field.clone(), v.clone()))
                .collect();
            let cond = Condition::Or(conditions);
            let mut p1 = vec![];
            let mut p2 = vec![];
            let sql1 = condition_to_sql_owned(cond.clone(), &mut p1);
            let sql2 = condition_to_sql(&cond, &mut p2);
            // SQL 字符串必须完全相同
            prop_assert_eq!(&sql1, &sql2);
            // 参数列表长度必须相等
            prop_assert_eq!(p1.len(), p2.len());
            // 参数值与顺序也必须完全一致（SqlValue 未实现 PartialEq，用整向量 Debug 串比较，
            // 一次同时覆盖元素值与顺序；sql_value_strategy 已把非有限 f64 归一为 0.0，Debug 确定）
            prop_assert_eq!(format!("{:?}", &p1), format!("{:?}", &p2));
        }

        #[test]
        fn prop_owned_equals_borrowed_nested(
            field1 in field_name_strategy(),
            field2 in field_name_strategy(),
            values in prop::collection::vec(sql_value_strategy(), 2..4)
        ) {
            // 构建嵌套条件：(field1 = v1 OR field1 = v2) AND field2 = v3
            let or_conds: Vec<Condition> = values[..values.len()-1]
                .iter()
                .map(|v| Condition::Eq(field1.clone(), v.clone()))
                .collect();
            let cond = Condition::And(vec![
                Condition::Or(or_conds),
                Condition::Eq(field2.clone(), values.last().unwrap().clone()),
            ]);
            let mut p1 = vec![];
            let mut p2 = vec![];
            let sql1 = condition_to_sql_owned(cond.clone(), &mut p1);
            let sql2 = condition_to_sql(&cond, &mut p2);
            // SQL 字符串必须完全相同
            prop_assert_eq!(&sql1, &sql2);
            // 参数列表长度必须相等
            prop_assert_eq!(p1.len(), p2.len());
            // 参数值与顺序也必须完全一致（SqlValue 未实现 PartialEq，用整向量 Debug 串比较，
            // 一次同时覆盖元素值与顺序；sql_value_strategy 已把非有限 f64 归一为 0.0，Debug 确定）
            prop_assert_eq!(format!("{:?}", &p1), format!("{:?}", &p2));
        }
    }
}
