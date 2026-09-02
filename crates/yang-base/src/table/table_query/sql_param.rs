//! 测试专用 SQL 参数类型（`cfg(test)`）：`build_*_sql` 系列的绑定参数表示。

#![cfg(all(test, feature = "mysql"))]

use crate::error::BaseError;
use serde_json::Value;

/// SQL 参数类型
///
/// 用于表示 SQL 查询中的参数值
#[cfg(all(test, feature = "mysql"))]
#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
#[allow(dead_code)] // DateTime/Bytes/Json 已声明，待 from_json 外部构造路径落地
pub(crate) enum SqlParam {
    /// 空值
    Null,
    /// 布尔值
    Bool(bool),
    /// 整数
    Int(i64),
    /// 无符号整数（保留超出 i64 范围的 u64 值，避免精度丢失）
    Uint(u64),
    /// 浮点数
    Float(f64),
    /// 字符串
    String(String),
    /// 日期时间（ISO 8601 字符串解析）
    DateTime(chrono::NaiveDateTime),
    /// 二进制数据
    Bytes(Vec<u8>),
    /// JSON 值
    Json(serde_json::Value),
}

#[cfg(all(test, feature = "mysql"))]
impl SqlParam {
    /// 从 JSON 值创建 SQL 参数
    ///
    /// # 参数
    ///
    /// - `value`：JSON 值
    ///
    /// # 返回值
    ///
    /// - `Ok(SqlParam)`：转换成功
    /// - `Err(BaseError)`：转换失败
    pub(super) fn from_json(value: &Value) -> Result<Self, BaseError> {
        match value {
            Value::Null => Ok(SqlParam::Null),
            Value::Bool(b) => Ok(SqlParam::Bool(*b)),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(SqlParam::Int(i))
                } else if let Some(u) = n.as_u64() {
                    // 超出 i64 范围的正整数，保留为 u64 避免精度丢失
                    Ok(SqlParam::Uint(u))
                } else if let Some(f) = n.as_f64() {
                    Ok(SqlParam::Float(f))
                } else {
                    Err(BaseError::DatabaseQueryFailed(
                        yang_db::DbError::QueryError(format!("不支持的数字类型: {}", n)),
                    ))
                }
            }
            Value::String(s) => {
                // QRY-5: 尝试解析为 DateTime（ISO 8601 格式）
                if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
                    .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S"))
                    .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f"))
                {
                    return Ok(SqlParam::DateTime(dt));
                }
                Ok(SqlParam::String(s.clone()))
            }
            Value::Array(_) => Ok(SqlParam::Json(value.clone())),
            Value::Object(_) => Ok(SqlParam::Json(value.clone())),
        }
    }
}
