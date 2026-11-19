//! PostgreSQL 参数绑定辅助：未类型化 NULL、绑定类型签名与语句缓存键。
//!
//! sqlx 0.8 的预编译语句缓存只按 SQL 文本索引。同一条 SQL 在不同调用间可能
//! 绑定不同类型（NULL / TEXT / INT8 / FLOAT8 等）；缓存命中后 sqlx 仍按首次
//! PARSE 定型的参数类型发送二进制值，导致 22P03 / 08P01，或把 FLOAT8 的位模式
//! 当成 INT8 静默查询到错误结果。
//!
//! 这里给每次执行附加一个只由「参数绑定类型」决定的注释前缀，使不同参数类型
//! 组合使用不同的缓存项；相同组合仍复用同一条预编译语句。

use sqlx::postgres::{PgArgumentBuffer, PgTypeInfo};
use sqlx::{Encode, Type};
use std::borrow::Cow;

use super::condition::SqlValue;

/// 未类型化 NULL 参数。
///
/// PARSE 阶段发送参数类型 OID 0（未指定），让 PostgreSQL 依据 SQL 上下文推断
/// 参数类型（与字符串字面量一致）。这使 `$1::text`、`$1::uuid`、
/// `$1::timestamptz`、`$1::numeric`、`$1::int + 1` 等显式 cast 场景都能接受
/// NULL；若固定绑定成 `int4` 或 `text`，这些 cast 会报 42883 / 42804。
/// 绑定值本身编码为 NULL（长度 -1），不写入任何数据。
#[derive(Debug, Clone, Copy)]
pub(crate) struct PgUntypedNull;

impl Type<sqlx::Postgres> for PgUntypedNull {
    fn type_info() -> PgTypeInfo {
        // OID 0 = 未指定类型，由服务端推断。
        PgTypeInfo::with_oid(sqlx::postgres::types::Oid(0))
    }
}

impl Encode<'_, sqlx::Postgres> for PgUntypedNull {
    fn encode_by_ref(
        &self,
        _buffer: &mut PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        Ok(sqlx::encode::IsNull::Yes)
    }

    fn size_hint(&self) -> usize {
        0
    }
}

const FNV_OFFSET_A: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_OFFSET_B: u64 = 0x9e37_79b9_7f4a_7c15;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// 双通道 FNV-1a：输出 128 位签名，避免 64 位哈希碰撞复用错误缓存项。
struct SignatureHash {
    a: u64,
    b: u64,
}

impl SignatureHash {
    fn new() -> Self {
        Self {
            a: FNV_OFFSET_A,
            b: FNV_OFFSET_B,
        }
    }

    fn write(&mut self, byte: u8) {
        self.a = (self.a ^ u64::from(byte)).wrapping_mul(FNV_PRIME);
        self.b = (self.b ^ u64::from(byte))
            .wrapping_mul(FNV_PRIME)
            .rotate_left(7);
    }

    fn write_str(&mut self, value: &str) {
        for byte in value.bytes() {
            self.write(byte);
        }
    }

    fn write_separator(&mut self) {
        self.write(0xff);
    }

    fn finish(&self) -> u128 {
        (u128::from(self.a) << 64) | u128::from(self.b)
    }
}

/// `serde_json::Value` 参数的绑定类型签名；无参数返回 `None`。
pub(crate) fn json_signature(params: &[serde_json::Value]) -> Option<u128> {
    if params.is_empty() {
        return None;
    }
    let mut hash = SignatureHash::new();
    for param in params {
        let code = match param {
            serde_json::Value::String(_) => "s",
            serde_json::Value::Number(number) => {
                match crate::json_param::classify_json_number(number) {
                    Some(crate::json_param::JsonNumber::Int(_)) => "i",
                    Some(crate::json_param::JsonNumber::Uint(_)) => "u",
                    Some(crate::json_param::JsonNumber::Float(_)) => "f",
                    None => "n",
                }
            }
            serde_json::Value::Bool(_) => "b",
            serde_json::Value::Null => "n",
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => "j",
        };
        hash.write_str(code);
        hash.write_separator();
    }
    Some(hash.finish())
}

/// `SqlValue` 参数的绑定类型签名；无参数返回 `None`。
pub(crate) fn sql_value_signature(params: &[SqlValue]) -> Option<u128> {
    if params.is_empty() {
        return None;
    }
    let mut hash = SignatureHash::new();
    for param in params {
        write_sql_value_code(&mut hash, param);
        hash.write_separator();
    }
    Some(hash.finish())
}

fn write_sql_value_code(hash: &mut SignatureHash, value: &SqlValue) {
    match value {
        // 未类型化 NULL，由服务端按 SQL 上下文推断。
        SqlValue::Null => hash.write_str("n"),
        SqlValue::Bool(_) => hash.write_str("b"),
        // Timestamp 也按 i64 绑定。
        SqlValue::Int(_) | SqlValue::Timestamp(_) => hash.write_str("i"),
        SqlValue::Float(_) => hash.write_str("f"),
        // 以下变体均按 TEXT 绑定。
        SqlValue::String(_)
        | SqlValue::Inet(_)
        | SqlValue::Cidr(_)
        | SqlValue::MacAddr(_)
        | SqlValue::MacAddr8(_)
        | SqlValue::Range(_)
        | SqlValue::Multirange(_) => hash.write_str("s"),
        SqlValue::Bytes(_) => hash.write_str("y"),
        SqlValue::Json(_) => hash.write_str("j"),
        SqlValue::DateTime(_) => hash.write_str("d"),
        SqlValue::Decimal(_) => hash.write_str("m"),
        SqlValue::Uuid(_) => hash.write_str("u"),
        SqlValue::DateTimeTz(_) => hash.write_str("z"),
        SqlValue::Date(_) => hash.write_str("a"),
        SqlValue::Time(_) => hash.write_str("c"),
        SqlValue::Interval(_) => hash.write_str("v"),
        // 数组绑定类型由首个非 NULL 元素决定；空数组/全 NULL 数组按 TEXT[]。
        SqlValue::Array(values) => {
            hash.write_str("[");
            match values.iter().find(|value| !matches!(value, SqlValue::Null)) {
                Some(element) => write_sql_value_code(hash, element),
                None => hash.write_str("s"),
            }
            hash.write_str("]");
        }
        // 显式元素类型；不同元素类型绑定到不同的 Rust 数组类型。
        SqlValue::TypedArray { element_type, .. } => {
            hash.write_str("A");
            hash.write_str(element_type);
        }
    }
}

/// 将绑定类型签名编码为 SQL 注释前缀；无参数时原样返回。
///
/// 注释只含十六进制字符，不会改变 SQL 语义；它让 sqlx 的语句缓存按
/// 「SQL 文本 + 参数类型组合」分桶，避免首次 PARSE 的类型污染后续调用。
pub(crate) fn keyed_sql<'a>(sql: &'a str, signature: Option<u128>) -> Cow<'a, str> {
    match signature {
        Some(signature) => Cow::Owned(format!("/*yang-bind:{signature:032x}*/{sql}")),
        None => Cow::Borrowed(sql),
    }
}

/// `SqlValue` 参数列表的缓存键 SQL。
pub(crate) fn keyed_sql_for<'a>(sql: &'a str, params: &[SqlValue]) -> Cow<'a, str> {
    keyed_sql(sql, sql_value_signature(params))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_signature_separates_null_and_string() {
        let null = json_signature(&[serde_json::Value::Null]);
        let text = json_signature(&[serde_json::Value::String("12.5".to_string())]);
        let int = json_signature(&[serde_json::json!(7i64)]);
        let float = json_signature(&[serde_json::json!(1.5f64)]);
        assert!(null.is_some());
        assert_ne!(null, text);
        assert_ne!(text, int);
        assert_ne!(int, float);
        assert_eq!(json_signature(&[]), None);
    }

    #[test]
    fn sql_value_signature_separates_null_int_and_float() {
        let null = sql_value_signature(&[SqlValue::Null]);
        let int = sql_value_signature(&[SqlValue::Int(7)]);
        let float = sql_value_signature(&[SqlValue::Float(1.5)]);
        let text = sql_value_signature(&[SqlValue::String("7".to_string())]);
        assert!(null.is_some());
        assert_ne!(null, int);
        assert_ne!(int, float);
        assert_ne!(int, text);
        assert_eq!(sql_value_signature(&[]), None);
    }

    #[test]
    fn keyed_sql_prefixes_signature_only_for_non_empty_params() {
        let sql = "SELECT $1";
        assert_eq!(keyed_sql(sql, None), sql);
        let keyed = keyed_sql(sql, Some(0x1234));
        assert!(keyed.starts_with("/*yang-bind:"));
        assert!(keyed.ends_with(sql));
        assert_ne!(keyed, sql);
    }

    #[test]
    fn typed_array_signature_includes_element_type() {
        let int4 = sql_value_signature(&[SqlValue::TypedArray {
            element_type: "int4",
            values: vec![],
        }]);
        let int8 = sql_value_signature(&[SqlValue::TypedArray {
            element_type: "int8",
            values: vec![],
        }]);
        assert_ne!(int4, int8);
    }
}
