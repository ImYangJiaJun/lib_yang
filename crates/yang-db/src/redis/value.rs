/// Redis 值类型
///
/// 表示 Redis 中的各种数据类型，提供类型安全的值表示
#[derive(Debug, Clone, PartialEq)]
pub enum RedisValue {
    /// 空值
    Nil,
    /// 整数
    Int(i64),
    /// 浮点数
    Float(f64),
    /// 字符串
    String(String),
    /// 字节数组
    Bytes(Vec<u8>),
    /// 数组
    Array(Vec<RedisValue>),
    /// 布尔值
    Bool(bool),
}

impl RedisValue {
    /// 转换为字符串
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
        match self {
            RedisValue::String(s) => Some(s.clone()),
            _ => None,
        }
    }

    /// 转换为整数
    ///
    /// # 返回
    /// - `Some(i64)`: 如果值是整数类型
    /// - `None`: 如果值不是整数类型
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            RedisValue::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// 转换为浮点数
    ///
    /// # 返回
    /// - `Some(f64)`: 如果值是浮点数类型
    /// - `None`: 如果值不是浮点数类型
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            RedisValue::Float(f) => Some(*f),
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
            RedisValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// 转换为字节数组引用
    ///
    /// # 返回
    /// - `Some(&[u8])`: 如果值是字节数组类型
    /// - `None`: 如果值不是字节数组类型
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            RedisValue::Bytes(b) => Some(b.as_slice()),
            _ => None,
        }
    }

    /// 转换为数组引用
    ///
    /// # 返回
    /// - `Some(&[RedisValue])`: 如果值是数组类型
    /// - `None`: 如果值不是数组类型
    pub fn as_array(&self) -> Option<&[RedisValue]> {
        match self {
            RedisValue::Array(arr) => Some(arr.as_slice()),
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
    fn from(s: String) -> Self {
        RedisValue::String(s)
    }
}

impl From<&str> for RedisValue {
    fn from(s: &str) -> Self {
        RedisValue::String(s.to_string())
    }
}

impl From<i64> for RedisValue {
    fn from(i: i64) -> Self {
        RedisValue::Int(i)
    }
}

impl From<i32> for RedisValue {
    fn from(i: i32) -> Self {
        RedisValue::Int(i as i64)
    }
}

impl From<f64> for RedisValue {
    fn from(f: f64) -> Self {
        RedisValue::Float(f)
    }
}

impl From<bool> for RedisValue {
    fn from(b: bool) -> Self {
        RedisValue::Bool(b)
    }
}

impl From<Vec<u8>> for RedisValue {
    fn from(bytes: Vec<u8>) -> Self {
        RedisValue::Bytes(bytes)
    }
}

// 实现 redis::Value 到 RedisValue 的转换
impl From<redis::Value> for RedisValue {
    fn from(value: redis::Value) -> Self {
        match value {
            redis::Value::Nil => RedisValue::Nil,
            redis::Value::Int(i) => RedisValue::Int(i),
            redis::Value::BulkString(bytes) => {
                // 尝试转换为 UTF-8 字符串：from_utf8 按值消费 bytes，
                // 失败时通过 into_bytes() 取回原始字节，避免多余 clone
                match String::from_utf8(bytes) {
                    Ok(s) => RedisValue::String(s),
                    Err(e) => RedisValue::Bytes(e.into_bytes()),
                }
            }
            redis::Value::Array(values) => {
                RedisValue::Array(values.into_iter().map(RedisValue::from).collect())
            }
            redis::Value::SimpleString(s) => RedisValue::String(s),
            redis::Value::Okay => RedisValue::Bool(true),
            redis::Value::Map(_) => {
                // Map 类型转换为字符串表示
                RedisValue::String(format!("{:?}", value))
            }
            redis::Value::Attribute { .. } => {
                // Attribute 类型转换为字符串表示
                RedisValue::String(format!("{:?}", value))
            }
            redis::Value::Set(_) => {
                // Set 类型转换为字符串表示
                RedisValue::String(format!("{:?}", value))
            }
            redis::Value::Double(f) => RedisValue::Float(f),
            redis::Value::Boolean(b) => RedisValue::Bool(b),
            redis::Value::VerbatimString { .. } => {
                // VerbatimString 转换为字符串表示
                RedisValue::String(format!("{:?}", value))
            }
            redis::Value::BigNumber(_) => {
                // BigNumber 转换为字符串表示
                RedisValue::String(format!("{:?}", value))
            }
            redis::Value::Push { .. } => {
                // Push 类型转换为字符串表示
                RedisValue::String(format!("{:?}", value))
            }
            // 其他未知类型
            _ => RedisValue::String(format!("{:?}", value)),
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
    fn test_debug() {
        let value = RedisValue::Int(42);
        let debug_str = format!("{:?}", value);
        assert!(debug_str.contains("Int"));
        assert!(debug_str.contains("42"));
    }

    #[test]
    fn test_partial_eq() {
        assert_eq!(RedisValue::Nil, RedisValue::Nil);
        assert_eq!(RedisValue::Int(42), RedisValue::Int(42));
        assert_ne!(RedisValue::Int(42), RedisValue::Int(43));
        assert_ne!(RedisValue::Int(42), RedisValue::String("42".to_string()));
    }
}
