#![allow(deprecated)]

use chrono::NaiveDateTime;
use serde_json::Value as JsonValue;

use crate::DbError;

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
    /// BETWEEN 条件
    Between(String, SqlValue, SqlValue),
    /// LIKE 条件
    Like(String, String),
    /// IS NULL 条件
    IsNull(String),
    /// IS NOT NULL 条件
    IsNotNull(String),
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

/// 压入一个参数并返回其 PostgreSQL 占位符编号（1 基）
///
/// PostgreSQL 使用编号占位符 `$1`、`$2` ……，编号由参数在最终绑定列表中的
/// 位置决定。本函数压入 `value` 后，返回 `params.len()` 作为编号，
/// 保证占位符编号与绑定顺序严格一致。调用方自行拼接 `$N` 到输出字符串，
/// 避免每次调用 `format!("${}", len)` 产生短命 String（PERF-9）。
fn push_placeholder(params: &mut Vec<SqlValue>, value: SqlValue, parameter_offset: usize) -> usize {
    params.push(value);
    parameter_offset + params.len()
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
    let mut rendered = crate::sql_types::RenderedCondition {
        sql: String::new(),
        params: Vec::new(),
    };
    write_condition_to_sql_owned_checked(
        condition,
        &mut rendered.sql,
        &mut rendered.params,
        parameter_offset,
    )?;
    Ok(rendered)
}

/// checked 版本的内部写入逻辑，使用 `quote_identifier(...)?` 传播错误。
fn write_condition_to_sql_owned_checked(
    condition: Condition,
    out: &mut String,
    params: &mut Vec<SqlValue>,
    parameter_offset: usize,
) -> Result<(), DbError> {
    let quote_identifier = super::identifier::quote_qualified;
    match condition {
        Condition::Eq(field, value) => {
            let idx = push_placeholder(params, value, parameter_offset);
            out.push_str(&format!("{} = ${}", quote_identifier(&field)?, idx));
            Ok(())
        }
        Condition::Ne(field, value) => {
            let idx = push_placeholder(params, value, parameter_offset);
            out.push_str(&format!("{} != ${}", quote_identifier(&field)?, idx));
            Ok(())
        }
        Condition::Gt(field, value) => {
            let idx = push_placeholder(params, value, parameter_offset);
            out.push_str(&format!("{} > ${}", quote_identifier(&field)?, idx));
            Ok(())
        }
        Condition::Lt(field, value) => {
            let idx = push_placeholder(params, value, parameter_offset);
            out.push_str(&format!("{} < ${}", quote_identifier(&field)?, idx));
            Ok(())
        }
        Condition::Gte(field, value) => {
            let idx = push_placeholder(params, value, parameter_offset);
            out.push_str(&format!("{} >= ${}", quote_identifier(&field)?, idx));
            Ok(())
        }
        Condition::Lte(field, value) => {
            let idx = push_placeholder(params, value, parameter_offset);
            out.push_str(&format!("{} <= ${}", quote_identifier(&field)?, idx));
            Ok(())
        }
        Condition::In(field, values) => {
            if values.is_empty() {
                return Err(DbError::InvalidArgument(format!(
                    "IN 条件 `{field}` 的值列表不能为空"
                )));
            }
            out.push_str(&format!("{} IN (", quote_identifier(&field)?));
            for (i, v) in values.into_iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                let idx = push_placeholder(params, v, parameter_offset);
                out.push_str(&format!("${}", idx));
            }
            out.push(')');
            Ok(())
        }
        Condition::Between(field, start, end) => {
            let idx_start = push_placeholder(params, start, parameter_offset);
            let idx_end = push_placeholder(params, end, parameter_offset);
            out.push_str(&format!(
                "{} BETWEEN ${} AND ${}",
                quote_identifier(&field)?,
                idx_start,
                idx_end
            ));
            Ok(())
        }
        Condition::Like(field, pattern) => {
            let idx = push_placeholder(params, SqlValue::String(pattern), parameter_offset);
            out.push_str(&format!("{} LIKE ${}", quote_identifier(&field)?, idx));
            Ok(())
        }
        Condition::IsNull(field) => {
            out.push_str(&format!("{} IS NULL", quote_identifier(&field)?));
            Ok(())
        }
        Condition::IsNotNull(field) => {
            out.push_str(&format!("{} IS NOT NULL", quote_identifier(&field)?));
            Ok(())
        }
        Condition::And(mut conditions) => {
            if conditions.is_empty() {
                return Err(DbError::InvalidArgument("AND 条件组不能为空".to_string()));
            }
            if conditions.len() == 1 {
                return write_condition_to_sql_owned_checked(
                    conditions.remove(0),
                    out,
                    params,
                    parameter_offset,
                );
            }
            out.push('(');
            for (i, c) in conditions.into_iter().enumerate() {
                if i > 0 {
                    out.push_str(" AND ");
                }
                write_condition_to_sql_owned_checked(c, out, params, parameter_offset)?;
            }
            out.push(')');
            Ok(())
        }
        Condition::Or(mut conditions) => {
            if conditions.is_empty() {
                return Err(DbError::InvalidArgument("OR 条件组不能为空".to_string()));
            }
            if conditions.len() == 1 {
                return write_condition_to_sql_owned_checked(
                    conditions.remove(0),
                    out,
                    params,
                    parameter_offset,
                );
            }
            out.push('(');
            for (i, c) in conditions.into_iter().enumerate() {
                if i > 0 {
                    out.push_str(" OR ");
                }
                write_condition_to_sql_owned_checked(c, out, params, parameter_offset)?;
            }
            out.push(')');
            Ok(())
        }
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
