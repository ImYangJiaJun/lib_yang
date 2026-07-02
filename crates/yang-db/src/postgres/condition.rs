use chrono::NaiveDateTime;
use serde_json::Value as JsonValue;

use super::identifier::quote_identifier;

/// SQL 值类型（PostgreSQL）
///
/// 与 MySQL 后端的 `SqlValue` 结构一致，独立定义以保持 `postgres` 模块自包含，
/// 避免跨方言耦合。变体集合与 `From` 转换均与 MySQL 版本保持对齐。
#[derive(Debug, Clone)]
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
fn push_placeholder(params: &mut Vec<SqlValue>, value: SqlValue) -> usize {
    params.push(value);
    params.len()
}

/// 将条件转换为 SQL 字符串和参数列表（借用版本）
///
/// # 参数
/// - condition: 要转换的条件
/// - params: 用于收集参数的可变向量（占位符编号基于其当前长度）
///
/// # 返回
/// - SQL 字符串片段
pub fn condition_to_sql(condition: &Condition, params: &mut Vec<SqlValue>) -> String {
    // 借用版本是消费版本的薄委托：整树 clone 一次后交给 owned 实现，
    // 二者共享同一套 match 分支，避免逻辑重复。输出 SQL 与参数顺序完全一致。
    condition_to_sql_owned(condition.clone(), params)
}

/// 安全转义字段标识符，失败时回退到原始值并记录警告。
///
/// 这是对 `quote_identifier` 的 defense-in-depth 包装：合法标识符返回双引号包裹形式，
/// 非法标识符（如 a.b 限定名或含特殊字符的表达式）回退到原始值并记录 `log::warn!`。
/// 注意: `quote_identifier` 仅处理单段标识符，`a.b` 限定名需用 `quote_qualified`。
fn safe_quote_identifier(field: &str) -> String {
    quote_identifier(field).unwrap_or_else(|e| {
        log::warn!("无法转义字段标识符 {field:?}: {e}，使用原始值");
        field.to_string()
    })
}

/// 将消费版本的条件直接写入 SQL 字符串，避免 And/Or 分支的中间 Vec 分配
///
/// 与 [`condition_to_sql`] 的借用版本不同，本函数消费传入的 `Condition`，
/// 对堆分配类型直接 push 到 params 中，无需 clone。
/// And/Or 分支直接写入 `out`，消除了 `Vec<String>` 中间分配（PERF-8）。
/// In 分支直接逐个生成 `$N` 占位符，消除了 `Vec<String>` 中间收集（PERF-8）。
fn write_condition_to_sql_owned(
    condition: Condition,
    out: &mut String,
    params: &mut Vec<SqlValue>,
) {
    match condition {
        Condition::Eq(field, value) => {
            let idx = push_placeholder(params, value);
            *out += &format!("{} = ${}", safe_quote_identifier(&field), idx);
        }
        Condition::Ne(field, value) => {
            let idx = push_placeholder(params, value);
            *out += &format!("{} != ${}", safe_quote_identifier(&field), idx);
        }
        Condition::Gt(field, value) => {
            let idx = push_placeholder(params, value);
            *out += &format!("{} > ${}", safe_quote_identifier(&field), idx);
        }
        Condition::Lt(field, value) => {
            let idx = push_placeholder(params, value);
            *out += &format!("{} < ${}", safe_quote_identifier(&field), idx);
        }
        Condition::Gte(field, value) => {
            let idx = push_placeholder(params, value);
            *out += &format!("{} >= ${}", safe_quote_identifier(&field), idx);
        }
        Condition::Lte(field, value) => {
            let idx = push_placeholder(params, value);
            *out += &format!("{} <= ${}", safe_quote_identifier(&field), idx);
        }
        Condition::In(field, values) => {
            if values.is_empty() {
                out.push_str("1 = 0");
                return;
            }
            *out += &format!("{} IN (", safe_quote_identifier(&field));
            for (i, v) in values.into_iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                let idx = push_placeholder(params, v);
                *out += &format!("${}", idx);
            }
            out.push(')');
        }
        Condition::Between(field, start, end) => {
            let idx_start = push_placeholder(params, start);
            let idx_end = push_placeholder(params, end);
            *out += &format!(
                "{} BETWEEN ${} AND ${}",
                safe_quote_identifier(&field),
                idx_start,
                idx_end
            );
        }
        Condition::Like(field, pattern) => {
            let idx = push_placeholder(params, SqlValue::String(pattern));
            *out += &format!("{} LIKE ${}", safe_quote_identifier(&field), idx);
        }
        Condition::IsNull(field) => {
            *out += &format!("{} IS NULL", safe_quote_identifier(&field));
        }
        Condition::IsNotNull(field) => {
            *out += &format!("{} IS NOT NULL", safe_quote_identifier(&field));
        }
        Condition::And(mut conditions) => {
            if conditions.is_empty() {
                out.push_str("1 = 1");
                return;
            }
            if conditions.len() == 1 {
                write_condition_to_sql_owned(conditions.remove(0), out, params);
                return;
            }
            out.push('(');
            for (i, c) in conditions.into_iter().enumerate() {
                if i > 0 {
                    out.push_str(" AND ");
                }
                write_condition_to_sql_owned(c, out, params);
            }
            out.push(')');
        }
        Condition::Or(mut conditions) => {
            if conditions.is_empty() {
                out.push_str("1 = 0");
                return;
            }
            if conditions.len() == 1 {
                write_condition_to_sql_owned(conditions.remove(0), out, params);
                return;
            }
            out.push('(');
            for (i, c) in conditions.into_iter().enumerate() {
                if i > 0 {
                    out.push_str(" OR ");
                }
                write_condition_to_sql_owned(c, out, params);
            }
            out.push(')');
        }
    }
}

pub fn condition_to_sql_owned(condition: Condition, params: &mut Vec<SqlValue>) -> String {
    let mut out = String::new();
    write_condition_to_sql_owned(condition, &mut out, params);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(sql, "1 = 0");
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
