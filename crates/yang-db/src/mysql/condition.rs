use chrono::NaiveDateTime;
use serde_json::Value as JsonValue;

use super::identifier::quote_identifier;
use crate::error::DbError;

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
pub fn condition_to_sql(condition: &Condition, params: &mut Vec<SqlValue>) -> String {
    let mut out = String::new();
    write_condition_to_sql(condition, &mut out, params);
    out
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
pub fn write_condition_to_sql(cond: &Condition, out: &mut String, params: &mut Vec<SqlValue>) {
    match cond {
        Condition::Eq(field, value) => {
            params.push(value.clone());
            *out += &format!("{} = ?", safe_quote_identifier(field));
        }
        Condition::Ne(field, value) => {
            params.push(value.clone());
            *out += &format!("{} != ?", safe_quote_identifier(field));
        }
        Condition::Gt(field, value) => {
            params.push(value.clone());
            *out += &format!("{} > ?", safe_quote_identifier(field));
        }
        Condition::Lt(field, value) => {
            params.push(value.clone());
            *out += &format!("{} < ?", safe_quote_identifier(field));
        }
        Condition::Gte(field, value) => {
            params.push(value.clone());
            *out += &format!("{} >= ?", safe_quote_identifier(field));
        }
        Condition::Lte(field, value) => {
            params.push(value.clone());
            *out += &format!("{} <= ?", safe_quote_identifier(field));
        }
        Condition::In(field, values) => {
            if values.is_empty() {
                out.push_str("1 = 0");
                return;
            }
            *out += &format!("{} IN (", safe_quote_identifier(field));
            for (i, v) in values.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push('?');
                params.push(v.clone());
            }
            out.push(')');
        }
        Condition::Between(field, start, end) => {
            params.push(start.clone());
            params.push(end.clone());
            *out += &format!("{} BETWEEN ? AND ?", safe_quote_identifier(field));
        }
        Condition::Like(field, pattern) => {
            params.push(SqlValue::String(pattern.clone()));
            *out += &format!("{} LIKE ?", safe_quote_identifier(field));
        }
        Condition::IsNull(field) => {
            *out += &format!("{} IS NULL", safe_quote_identifier(field));
        }
        Condition::IsNotNull(field) => {
            *out += &format!("{} IS NOT NULL", safe_quote_identifier(field));
        }
        Condition::And(conditions) => {
            if conditions.is_empty() {
                out.push_str("1 = 1");
                return;
            }
            if conditions.len() == 1 {
                write_condition_to_sql(&conditions[0], out, params);
                return;
            }
            out.push('(');
            for (i, c) in conditions.iter().enumerate() {
                if i > 0 {
                    out.push_str(" AND ");
                }
                write_condition_to_sql(c, out, params);
            }
            out.push(')');
        }
        Condition::Or(conditions) => {
            if conditions.is_empty() {
                out.push_str("1 = 0");
                return;
            }
            if conditions.len() == 1 {
                write_condition_to_sql(&conditions[0], out, params);
                return;
            }
            out.push('(');
            for (i, c) in conditions.iter().enumerate() {
                if i > 0 {
                    out.push_str(" OR ");
                }
                write_condition_to_sql(c, out, params);
            }
            out.push(')');
        }
    }
}

/// 消费版本的条件转 SQL 函数，避免不必要的 clone 开销
///
/// 与 `condition_to_sql` 的借用版本相比，本函数消费传入的 `Condition`，
/// 对 `SqlValue::String`、`SqlValue::Bytes`、`SqlValue::Json` 等堆分配类型
/// 直接 push 到 params 中，无需 clone，从而减少堆分配次数。
///
/// # 参数
/// - `condition`: 要消费的条件（owned）
/// - `params`: 用于收集参数的可变向量
///
/// # 返回
/// - SQL 字符串片段
///
/// 安全转义字段标识符，失败时回退到原始值并记录警告。
///
/// 这是对 `quote_identifier` 的 defense-in-depth 包装：合法标识符返回反引号包裹形式，
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
/// 与 [`write_condition_to_sql`] 不同，本函数消费传入的 `Condition`，
/// 对堆分配类型直接 push 到 params 中，无需 clone。
/// And/Or 分支直接写入 `out`，消除了 `Vec<String>` 中间分配（PERF-8）。
/// In 分支直接逐个 push `?`，消除了 `vec!["?"; count]` 临时 Vec（PERF-7）。
fn write_condition_to_sql_owned(
    condition: Condition,
    out: &mut String,
    params: &mut Vec<SqlValue>,
) {
    match condition {
        Condition::Eq(field, value) => {
            params.push(value);
            *out += &format!("{} = ?", safe_quote_identifier(&field));
        }
        Condition::Ne(field, value) => {
            params.push(value);
            *out += &format!("{} != ?", safe_quote_identifier(&field));
        }
        Condition::Gt(field, value) => {
            params.push(value);
            *out += &format!("{} > ?", safe_quote_identifier(&field));
        }
        Condition::Lt(field, value) => {
            params.push(value);
            *out += &format!("{} < ?", safe_quote_identifier(&field));
        }
        Condition::Gte(field, value) => {
            params.push(value);
            *out += &format!("{} >= ?", safe_quote_identifier(&field));
        }
        Condition::Lte(field, value) => {
            params.push(value);
            *out += &format!("{} <= ?", safe_quote_identifier(&field));
        }
        Condition::In(field, values) => {
            if values.is_empty() {
                out.push_str("1 = 0");
                return;
            }
            let count = values.len();
            params.extend(values);
            *out += &format!("{} IN (", safe_quote_identifier(&field));
            for i in 0..count {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push('?');
            }
            out.push(')');
        }
        Condition::Between(field, start, end) => {
            params.push(start);
            params.push(end);
            *out += &format!("{} BETWEEN ? AND ?", safe_quote_identifier(&field));
        }
        Condition::Like(field, pattern) => {
            params.push(SqlValue::String(pattern));
            *out += &format!("{} LIKE ?", safe_quote_identifier(&field));
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

/// 严格版条件转 SQL：非法标识符返回 [`DbError::InvalidArgument`] 而非 RAW 回退。
///
/// 与 [`condition_to_sql_owned`] 行为一致，但字段标识符校验失败时**不**回退到原始值，
/// 而是立即返回错误，供调用方自行决定是否允许特定表达式（如 `a.b` 限定名）。
///
/// # 返回
///
/// - `Ok(String)`: SQL 片段
/// - `Err(DbError::InvalidArgument)`: 包含非法标识符
pub fn condition_to_sql_owned_checked(
    condition: Condition,
    params: &mut Vec<SqlValue>,
) -> Result<String, DbError> {
    let mut out = String::new();
    write_condition_to_sql_owned_checked(condition, &mut out, params)?;
    Ok(out)
}

/// checked 版本的内部写入逻辑，使用 `quote_identifier(...)?` 传播错误。
fn write_condition_to_sql_owned_checked(
    condition: Condition,
    out: &mut String,
    params: &mut Vec<SqlValue>,
) -> Result<(), DbError> {
    let quote_identifier = super::identifier::quote_qualified;
    match condition {
        Condition::Eq(field, value) => {
            params.push(value);
            out.push_str(&format!("{} = ?", quote_identifier(&field)?));
            Ok(())
        }
        Condition::Ne(field, value) => {
            params.push(value);
            out.push_str(&format!("{} != ?", quote_identifier(&field)?));
            Ok(())
        }
        Condition::Gt(field, value) => {
            params.push(value);
            out.push_str(&format!("{} > ?", quote_identifier(&field)?));
            Ok(())
        }
        Condition::Lt(field, value) => {
            params.push(value);
            out.push_str(&format!("{} < ?", quote_identifier(&field)?));
            Ok(())
        }
        Condition::Gte(field, value) => {
            params.push(value);
            out.push_str(&format!("{} >= ?", quote_identifier(&field)?));
            Ok(())
        }
        Condition::Lte(field, value) => {
            params.push(value);
            out.push_str(&format!("{} <= ?", quote_identifier(&field)?));
            Ok(())
        }
        Condition::In(field, values) => {
            if values.is_empty() {
                return Err(DbError::InvalidArgument(format!(
                    "IN 条件 `{field}` 的值列表不能为空"
                )));
            }
            let count = values.len();
            params.extend(values);
            out.push_str(&format!("{} IN (", quote_identifier(&field)?));
            for i in 0..count {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push('?');
            }
            out.push(')');
            Ok(())
        }
        Condition::Between(field, start, end) => {
            params.push(start);
            params.push(end);
            out.push_str(&format!("{} BETWEEN ? AND ?", quote_identifier(&field)?));
            Ok(())
        }
        Condition::Like(field, pattern) => {
            params.push(SqlValue::String(pattern));
            out.push_str(&format!("{} LIKE ?", quote_identifier(&field)?));
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
                return write_condition_to_sql_owned_checked(conditions.remove(0), out, params);
            }
            out.push('(');
            for (i, c) in conditions.into_iter().enumerate() {
                if i > 0 {
                    out.push_str(" AND ");
                }
                write_condition_to_sql_owned_checked(c, out, params)?;
            }
            out.push(')');
            Ok(())
        }
        Condition::Or(mut conditions) => {
            if conditions.is_empty() {
                return Err(DbError::InvalidArgument("OR 条件组不能为空".to_string()));
            }
            if conditions.len() == 1 {
                return write_condition_to_sql_owned_checked(conditions.remove(0), out, params);
            }
            out.push('(');
            for (i, c) in conditions.into_iter().enumerate() {
                if i > 0 {
                    out.push_str(" OR ");
                }
                write_condition_to_sql_owned_checked(c, out, params)?;
            }
            out.push(')');
            Ok(())
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
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
        assert_eq!(sql, "1 = 0");
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
        assert_eq!(sql, "1 = 1");
        assert_eq!(params.len(), 0);
    }

    #[test]
    fn test_condition_empty_or() {
        let mut params = Vec::new();
        let cond = Condition::Or(vec![]);
        let sql = super::condition_to_sql(&cond, &mut params);
        assert_eq!(sql, "1 = 0");
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
