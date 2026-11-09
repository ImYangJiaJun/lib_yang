use std::fmt;

/// Redis 值类型
///
/// 表示 Redis 中的各种数据类型，提供类型安全的值表示。
///
/// `Debug` 实现会对字符串/字节内容做脱敏，只输出类型与长度，避免把缓存值、
/// token 或用户数据写进日志。
#[derive(Clone, PartialEq)]
#[non_exhaustive]
pub enum RedisValue {
    /// 空值
    Nil,
    /// 整数
    Int(i64),
    /// 浮点数
    Float(f64),
    /// 字符串
    String(String),
    /// 字节数组（非 UTF-8 或显式二进制）
    Bytes(Vec<u8>),
    /// 数组
    Array(Vec<RedisValue>),
    /// 布尔值
    Bool(bool),
    /// RESP3 Map
    Map(Vec<(RedisValue, RedisValue)>),
    /// RESP3 Set
    Set(Vec<RedisValue>),
    /// RESP3 Verbatim String
    VerbatimString {
        /// 格式（如 `txt` / `mkd`）
        format: String,
        /// 文本内容
        text: String,
    },
    /// RESP3 Push（Pub/Sub 消息等）
    Push(Vec<RedisValue>),
    /// RESP3 Attribute
    Attribute {
        /// 被附着的值
        data: Box<RedisValue>,
        /// 属性键值对
        attributes: Vec<(RedisValue, RedisValue)>,
    },
    /// RESP3 BigNumber（超出 i64 的数值字符串）
    BigNumber(String),
    /// 服务端错误响应（仅当调用方显式按 Value 解码时出现）
    ServerError(String),
}

impl fmt::Debug for RedisValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RedisValue::Nil => f.write_str("Nil"),
            RedisValue::Int(value) => write!(f, "Int({value})"),
            RedisValue::Float(value) => write!(f, "Float({value})"),
            RedisValue::String(value) => write!(f, "String(<redacted {} bytes>)", value.len()),
            RedisValue::Bytes(value) => write!(f, "Bytes(<redacted {} bytes>)", value.len()),
            RedisValue::Array(values) => f.debug_list().entries(values).finish(),
            RedisValue::Bool(value) => write!(f, "Bool({value})"),
            RedisValue::Map(entries) => write!(f, "Map(<redacted {} entries>)", entries.len()),
            RedisValue::Set(values) => write!(f, "Set(<redacted {} items>)", values.len()),
            RedisValue::VerbatimString { format, text } => write!(
                f,
                "VerbatimString {{ format: {format:?}, text: <redacted {} bytes> }}",
                text.len()
            ),
            RedisValue::Push(values) => write!(f, "Push(<redacted {} items>)", values.len()),
            RedisValue::Attribute { attributes, .. } => {
                write!(f, "Attribute(<redacted {} entries>)", attributes.len())
            }
            RedisValue::BigNumber(value) => write!(f, "BigNumber({value})"),
            RedisValue::ServerError(_) => f.write_str("ServerError(<redacted>)"),
        }
    }
}

impl RedisValue {
    /// 借用字符串切片（零拷贝）
    ///
    /// 对 `String` 与 `VerbatimString` 变体返回 `&str`，无需堆分配。
    /// `Bytes` 变体不自动解码为 UTF-8，返回 `None`。
    ///
    /// # 返回
    /// - `Some(&str)`: 如果值是字符串类型
    /// - `None`: 如果值不是字符串类型
    ///
    /// # 示例
    /// ```
    /// use yang_db::RedisValue;
    ///
    /// let value = RedisValue::String("hello".to_string());
    /// assert_eq!(value.as_str(), Some("hello"));
    ///
    /// let nil = RedisValue::Nil;
    /// assert_eq!(nil.as_str(), None);
    /// ```
    pub fn as_str(&self) -> Option<&str> {
        match self {
            RedisValue::String(value) => Some(value.as_str()),
            RedisValue::VerbatimString { text, .. } => Some(text.as_str()),
            RedisValue::BigNumber(value) => Some(value.as_str()),
            _ => None,
        }
    }

    /// 转换为字符串（需要堆分配）
    ///
    /// 内部委托 [`as_str`](Self::as_str)，仅在确实需要 `String` 时使用；
    /// 若只需借用，请优先使用 `as_str()`。
    ///
    /// # 返回
    /// - `Some(String)`: 如果值是字符串类型
    /// - `None`: 如果值不是字符串类型
    ///
    /// # 示例
    /// ```
    /// use yang_db::RedisValue;
    ///
    /// let value = RedisValue::String("hello".to_string());
    /// assert_eq!(value.as_string(), Some("hello".to_string()));
    ///
    /// let nil = RedisValue::Nil;
    /// assert_eq!(nil.as_string(), None);
    /// ```
    pub fn as_string(&self) -> Option<String> {
        self.as_str().map(|value| value.to_owned())
    }

    /// 转换为整数
    ///
    /// 除 `Int` 外，也接受 RESP2 以字符串形式返回的数字（如 INCR 以外的
    /// 部分命令），解析失败返回 `None`。
    ///
    /// # 返回
    /// - `Some(i64)`: 如果值是整数或可解析的整数字符串
    /// - `None`: 其他类型或解析失败
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            RedisValue::Int(value) => Some(*value),
            RedisValue::String(value) | RedisValue::BigNumber(value) => value.parse().ok(),
            RedisValue::Bytes(value) => std::str::from_utf8(value).ok()?.parse().ok(),
            _ => None,
        }
    }

    /// 转换为浮点数
    ///
    /// 除 `Float` 外，也接受以字符串形式返回的数字。
    ///
    /// # 返回
    /// - `Some(f64)`: 如果值是浮点数或可解析的数字字符串
    /// - `None`: 其他类型或解析失败
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            RedisValue::Float(value) => Some(*value),
            RedisValue::String(value) | RedisValue::BigNumber(value) => value.parse().ok(),
            RedisValue::Bytes(value) => std::str::from_utf8(value).ok()?.parse().ok(),
            _ => None,
        }
    }

    /// 转换为布尔值
    ///
    /// # 返回
    /// - `Some(bool)`: 如果值是布尔类型
    /// - `None`: 如果值不是布尔类型
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            RedisValue::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// 转换为字节数组引用
    ///
    /// `String` / `VerbatimString` / `BigNumber` 返回其 UTF-8 字节，
    /// `Bytes` 返回原始字节。这样即使是「恰好是合法 UTF-8 的二进制」也能取回字节。
    ///
    /// # 返回
    /// - `Some(&[u8])`: 如果值可表示为字节
    /// - `None`: 其他类型（Nil/Int/Float/Bool/Array/Map/Set）
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            RedisValue::Bytes(value) => Some(value.as_slice()),
            RedisValue::String(value) => Some(value.as_bytes()),
            RedisValue::VerbatimString { text, .. } => Some(text.as_bytes()),
            RedisValue::BigNumber(value) => Some(value.as_bytes()),
            _ => None,
        }
    }

    /// 转换为拥有所有权的字节数组。
    pub fn as_bytes_owned(&self) -> Option<Vec<u8>> {
        self.as_bytes().map(|value| value.to_vec())
    }

    /// 转换为数组引用
    ///
    /// # 返回
    /// - `Some(&[RedisValue])`: 如果值是数组类型
    /// - `None`: 如果值不是数组类型
    pub fn as_array(&self) -> Option<&[RedisValue]> {
        match self {
            RedisValue::Array(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    /// 检查是否为 Nil
    ///
    /// # 返回
    /// - `true`: 如果值是 Nil
    /// - `false`: 如果值不是 Nil
    pub fn is_nil(&self) -> bool {
        matches!(self, RedisValue::Nil)
    }
}

// 实现 From trait 支持自动转换
impl From<String> for RedisValue {
    fn from(value: String) -> Self {
        RedisValue::String(value)
    }
}

impl From<&str> for RedisValue {
    fn from(value: &str) -> Self {
        RedisValue::String(value.to_string())
    }
}

impl From<i64> for RedisValue {
    fn from(value: i64) -> Self {
        RedisValue::Int(value)
    }
}

impl From<i32> for RedisValue {
    fn from(value: i32) -> Self {
        RedisValue::Int(value as i64)
    }
}

impl From<f64> for RedisValue {
    fn from(value: f64) -> Self {
        RedisValue::Float(value)
    }
}

impl From<bool> for RedisValue {
    fn from(value: bool) -> Self {
        RedisValue::Bool(value)
    }
}

impl From<Vec<u8>> for RedisValue {
    fn from(value: Vec<u8>) -> Self {
        RedisValue::Bytes(value)
    }
}

// 实现 redis::Value 到 RedisValue 的转换
impl From<redis::Value> for RedisValue {
    fn from(value: redis::Value) -> Self {
        match value {
            redis::Value::Nil => RedisValue::Nil,
            redis::Value::Int(value) => RedisValue::Int(value),
            redis::Value::BulkString(bytes) => match String::from_utf8(bytes) {
                Ok(value) => RedisValue::String(value),
                Err(error) => RedisValue::Bytes(error.into_bytes()),
            },
            redis::Value::Array(values) => {
                RedisValue::Array(values.into_iter().map(RedisValue::from).collect())
            }
            redis::Value::SimpleString(value) => RedisValue::String(value),
            redis::Value::Okay => RedisValue::Bool(true),
            redis::Value::Map(entries) => RedisValue::Map(
                entries
                    .into_iter()
                    .map(|(key, value)| (RedisValue::from(key), RedisValue::from(value)))
                    .collect(),
            ),
            redis::Value::Attribute { data, attributes } => RedisValue::Attribute {
                data: Box::new(RedisValue::from(*data)),
                attributes: attributes
                    .into_iter()
                    .map(|(key, value)| (RedisValue::from(key), RedisValue::from(value)))
                    .collect(),
            },
            redis::Value::Set(values) => {
                RedisValue::Set(values.into_iter().map(RedisValue::from).collect())
            }
            redis::Value::Double(value) => RedisValue::Float(value),
            redis::Value::Boolean(value) => RedisValue::Bool(value),
            redis::Value::VerbatimString { format, text } => RedisValue::VerbatimString {
                format: format!("{format:?}"),
                text,
            },
            redis::Value::BigNumber(value) => RedisValue::BigNumber(value.to_string()),
            redis::Value::Push { data, .. } => {
                RedisValue::Push(data.into_iter().map(RedisValue::from).collect())
            }
            redis::Value::ServerError(error) => RedisValue::ServerError(format!("{error:?}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redis_value_nil() {
        let value = RedisValue::Nil;
        assert!(value.is_nil());
        assert_eq!(value.as_string(), None);
        assert_eq!(value.as_i64(), None);
    }

    #[test]
    fn test_redis_value_int() {
        let value = RedisValue::Int(42);
        assert!(!value.is_nil());
        assert_eq!(value.as_i64(), Some(42));
        assert_eq!(value.as_string(), None);
    }

    #[test]
    fn test_redis_value_float() {
        let value = RedisValue::Float(123.456);
        assert_eq!(value.as_f64(), Some(123.456));
        assert_eq!(value.as_i64(), None);
    }

    #[test]
    fn test_redis_value_string() {
        let value = RedisValue::String("hello".to_string());
        assert_eq!(value.as_str(), Some("hello"));
        assert_eq!(value.as_string(), Some("hello".to_string()));
        assert_eq!(value.as_i64(), None);
    }

    #[test]
    fn test_redis_value_bool() {
        let value = RedisValue::Bool(true);
        assert_eq!(value.as_bool(), Some(true));
        assert_eq!(value.as_string(), None);
    }

    #[test]
    fn test_redis_value_bytes() {
        let bytes = vec![1, 2, 3, 4];
        let value = RedisValue::Bytes(bytes.clone());
        assert_eq!(value.as_bytes(), Some(bytes.as_slice()));
        assert_eq!(value.as_string(), None);
    }

    #[test]
    fn test_redis_value_array() {
        let arr = vec![
            RedisValue::Int(1),
            RedisValue::String("test".to_string()),
            RedisValue::Nil,
        ];
        let value = RedisValue::Array(arr.clone());
        assert_eq!(value.as_array(), Some(arr.as_slice()));
        assert_eq!(value.as_string(), None);
    }

    #[test]
    fn test_from_string() {
        let value: RedisValue = "hello".into();
        assert_eq!(value, RedisValue::String("hello".to_string()));

        let value: RedisValue = String::from("world").into();
        assert_eq!(value, RedisValue::String("world".to_string()));
    }

    #[test]
    fn test_from_int() {
        let value: RedisValue = 42i64.into();
        assert_eq!(value, RedisValue::Int(42));

        let value: RedisValue = 100i32.into();
        assert_eq!(value, RedisValue::Int(100));
    }

    #[test]
    fn test_from_float() {
        let value: RedisValue = 123.456f64.into();
        assert_eq!(value, RedisValue::Float(123.456));
    }

    #[test]
    fn test_from_bool() {
        let value: RedisValue = true.into();
        assert_eq!(value, RedisValue::Bool(true));

        let value: RedisValue = false.into();
        assert_eq!(value, RedisValue::Bool(false));
    }

    #[test]
    fn test_from_bytes() {
        let bytes = vec![1, 2, 3];
        let value: RedisValue = bytes.clone().into();
        assert_eq!(value, RedisValue::Bytes(bytes));
    }

    #[test]
    fn test_from_redis_value_nil() {
        let redis_val = redis::Value::Nil;
        let value: RedisValue = redis_val.into();
        assert_eq!(value, RedisValue::Nil);
    }

    #[test]
    fn test_from_redis_value_int() {
        let redis_val = redis::Value::Int(42);
        let value: RedisValue = redis_val.into();
        assert_eq!(value, RedisValue::Int(42));
    }

    #[test]
    fn test_from_redis_value_data_utf8() {
        let redis_val = redis::Value::BulkString(b"hello".to_vec());
        let value: RedisValue = redis_val.into();
        assert_eq!(value, RedisValue::String("hello".to_string()));
    }

    #[test]
    fn test_from_redis_value_data_binary() {
        let bytes = vec![0xFF, 0xFE, 0xFD];
        let redis_val = redis::Value::BulkString(bytes.clone());
        let value: RedisValue = redis_val.into();
        assert_eq!(value, RedisValue::Bytes(bytes));
    }

    #[test]
    fn test_from_redis_value_bulk() {
        let redis_val = redis::Value::Array(vec![
            redis::Value::Int(1),
            redis::Value::BulkString(b"test".to_vec()),
        ]);
        let value: RedisValue = redis_val.into();

        if let RedisValue::Array(arr) = value {
            assert_eq!(arr.len(), 2);
            assert_eq!(arr[0], RedisValue::Int(1));
            assert_eq!(arr[1], RedisValue::String("test".to_string()));
        } else {
            panic!("Expected Array");
        }
    }

    #[test]
    fn test_from_redis_value_status() {
        let redis_val = redis::Value::SimpleString("OK".to_string());
        let value: RedisValue = redis_val.into();
        assert_eq!(value, RedisValue::String("OK".to_string()));
    }

    #[test]
    fn test_from_redis_value_okay() {
        let redis_val = redis::Value::Okay;
        let value: RedisValue = redis_val.into();
        assert_eq!(value, RedisValue::Bool(true));
    }

    #[test]
    fn test_clone() {
        let value = RedisValue::String("test".to_string());
        let cloned = value.clone();
        assert_eq!(value, cloned);
    }

    #[test]
    fn test_debug_redacts_payloads() {
        // 标量类型保留可读值，便于排障
        assert!(format!("{:?}", RedisValue::Int(42)).contains("42"));
        assert!(format!("{:?}", RedisValue::Bool(true)).contains("true"));

        // 字符串/字节内容必须脱敏
        let string_debug = format!("{:?}", RedisValue::String("super-secret-token".to_string()));
        assert!(!string_debug.contains("super-secret-token"));
        assert!(string_debug.contains("redacted"));

        let bytes_debug = format!("{:?}", RedisValue::Bytes(b"super-secret".to_vec()));
        assert!(!bytes_debug.contains("super-secret"));
        assert!(bytes_debug.contains("redacted"));

        let map_debug = format!(
            "{:?}",
            RedisValue::Map(vec![(
                RedisValue::String("user".to_string()),
                RedisValue::String("secret".to_string())
            )])
        );
        assert!(!map_debug.contains("secret"));
        assert!(map_debug.contains("redacted"));
    }

    #[test]
    fn test_as_str() {
        // String 变体返回借用
        let value = RedisValue::String("hello".to_string());
        assert_eq!(value.as_str(), Some("hello"));

        // Bytes 变体不自动解码，返回 None
        let value = RedisValue::Bytes(vec![0xFF, 0xFE]);
        assert_eq!(value.as_str(), None);

        // 其他变体一律 None
        assert_eq!(RedisValue::Nil.as_str(), None);
        assert_eq!(RedisValue::Int(1).as_str(), None);
        assert_eq!(RedisValue::Float(1.0).as_str(), None);
        assert_eq!(RedisValue::Bool(true).as_str(), None);
        assert_eq!(RedisValue::Array(vec![]).as_str(), None);
    }

    #[test]
    fn test_as_bytes_includes_utf8_strings() {
        // 恰好是合法 UTF-8 的二进制也必须能通过 as_bytes 取回
        let value = RedisValue::String("abc".to_string());
        assert_eq!(value.as_bytes(), Some(b"abc".as_slice()));
        assert_eq!(
            RedisValue::Bytes(vec![0xFF]).as_bytes(),
            Some([0xFF].as_slice())
        );
    }

    #[test]
    fn test_as_i64_and_f64_accept_numeric_strings() {
        assert_eq!(RedisValue::String("42".to_string()).as_i64(), Some(42));
        assert_eq!(RedisValue::String("1.5".to_string()).as_f64(), Some(1.5));
        assert_eq!(RedisValue::String("nope".to_string()).as_i64(), None);
    }

    #[test]
    fn test_partial_eq() {
        assert_eq!(RedisValue::Nil, RedisValue::Nil);
        assert_eq!(RedisValue::Int(42), RedisValue::Int(42));
        assert_ne!(RedisValue::Int(42), RedisValue::Int(43));
        assert_ne!(RedisValue::Int(42), RedisValue::String("42".to_string()));
    }
}
