//! 数据库记录类型
//!
//! 提供 `Record` 类型，用于在不知道具体 Rust 实体时安全读取 MySQL 查询结果。
//! 每一行数据以 `serde_json::Map<String, serde_json::Value>` 的形式存储，
//! 支持所有常见 MySQL 类型到 JSON 类型的映射。
//!
//! # MySQL 类型映射规则
//!
//! | MySQL 类型 | JSON 类型 |
//! |-----------|----------|
//! | INT / BIGINT | Number (i64) |
//! | FLOAT / DOUBLE | Number (f64) |
//! | DECIMAL / NUMERIC | String（保留精度，NEWDECIMAL 协议本就是字符串编码） |
//! | VARCHAR / TEXT / CHAR | String |
//! | BOOLEAN / TINYINT(1) | Bool |
//! | DATE / DATETIME / TIMESTAMP | String (ISO 8601) |
//! | NULL | Null |
//! | BLOB / BINARY | String (Base64) |
//! | JSON | Object / Array |
//!
//! # 示例
//!
//! ```rust,ignore
//! use yang_base::table::Record;
//!
//! // 通过 sqlx::FromRow 自动从查询结果构建
//! let rows: Vec<Record> = query.all().await?;
//! for row in rows {
//!     println!("{:?}", row.as_map());
//! }
//! ```

use crate::error::BaseError;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// 动态数据库记录
///
/// 以键值对形式存储一行数据库查询结果，支持任意表结构。
/// 字段名为键，字段值为 JSON 值。
///
/// # 字段
///
/// - `columns`：列名到 JSON 值的映射
///
/// # 示例
///
/// ```rust,ignore
/// use yang_base::table::Record;
/// use serde_json::json;
///
/// let row = Record::new()
///     .set("id", json!(1))
///     .set("name", json!("Alice"));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct Record {
    columns: serde_json::Map<String, serde_json::Value>,
}

impl Record {
    /// 创建空记录。
    pub fn new() -> Self {
        Self {
            columns: serde_json::Map::new(),
        }
    }

    /// 链式写入字段值。
    pub fn set(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.columns.insert(key.into(), value.into());
        self
    }

    /// 写入字段值并返回被替换的旧值。
    pub fn insert(
        &mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Option<serde_json::Value> {
        self.columns.insert(key.into(), value.into())
    }

    /// 获取指定列的原始 JSON 值。
    ///
    /// # 参数
    ///
    /// - `key`：列名
    ///
    /// # 返回
    ///
    /// - `Some(&Value)`：列存在时返回值的引用
    /// - `None`：列不存在
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        if key.trim().is_empty() {
            return None;
        }

        self.columns.get(key)
    }

    /// 读取必需字段并转换为目标类型。
    pub fn require<T>(&self, key: &str) -> Result<T, BaseError>
    where
        T: DeserializeOwned,
    {
        let value = self
            .get(key)
            .ok_or_else(|| BaseError::FieldNotFound("record".to_string(), key.to_string()))?;
        serde_json::from_value(value.clone())
            .map_err(|error| BaseError::InvalidFieldType(key.to_string(), error.to_string()))
    }

    /// 读取可选字段；字段不存在或为 `null` 时返回 `None`。
    pub fn optional<T>(&self, key: &str) -> Result<Option<T>, BaseError>
    where
        T: DeserializeOwned,
    {
        let Some(value) = self.get(key) else {
            return Ok(None);
        };
        if value.is_null() {
            return Ok(None);
        }
        serde_json::from_value(value.clone())
            .map(Some)
            .map_err(|error| BaseError::InvalidFieldType(key.to_string(), error.to_string()))
    }

    /// 返回字段映射的只读引用。
    pub fn as_map(&self) -> &serde_json::Map<String, serde_json::Value> {
        &self.columns
    }

    /// 消费记录并返回字段映射。
    pub fn into_map(self) -> serde_json::Map<String, serde_json::Value> {
        self.columns
    }

    /// 消费记录并返回查询写入层使用的 HashMap。
    #[cfg(feature = "mysql")]
    pub(crate) fn into_columns(self) -> std::collections::HashMap<String, serde_json::Value> {
        self.columns.into_iter().collect()
    }
}

impl Default for Record {
    fn default() -> Self {
        Self::new()
    }
}

impl From<serde_json::Map<String, serde_json::Value>> for Record {
    fn from(columns: serde_json::Map<String, serde_json::Value>) -> Self {
        Self { columns }
    }
}

/// 将 Record 转换为 serde_json::Value（Object 类型）。
impl From<Record> for serde_json::Value {
    fn from(row: Record) -> Self {
        serde_json::Value::Object(row.columns)
    }
}

/// 实现 sqlx::FromRow，支持从 MySQL 查询结果动态解码
///
/// 按 MySQL 列类型将值映射到对应的 JSON 类型：
/// - INT/BIGINT → i64
/// - FLOAT/DOUBLE → f64
/// - DECIMAL/NUMERIC → String（保留精度）
/// - VARCHAR/TEXT/CHAR → String
/// - BOOLEAN/TINYINT(1) → Bool
/// - DATE/DATETIME/TIMESTAMP → ISO 8601 字符串
/// - NULL → Null
/// - BLOB/BINARY → Base64 字符串
/// - JSON → Object/Array
#[cfg(feature = "mysql")]
impl<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> for Record {
    fn from_row(row: &'r sqlx::mysql::MySqlRow) -> Result<Self, sqlx::Error> {
        use sqlx::Column;
        use sqlx::Row;
        use sqlx::TypeInfo;
        use sqlx::ValueRef;

        let mut columns = serde_json::Map::new();

        // 遍历所有列，按类型解码
        for col in row.columns() {
            let col_name = col.name().to_string();
            let type_name = col.type_info().name().to_uppercase();

            // 先检查是否为 NULL
            let is_null: bool = row
                .try_get_raw(col.ordinal())
                .map(|v| v.is_null())
                .unwrap_or(false);

            if is_null {
                // NULL 值映射为 JSON Null
                columns.insert(col_name, serde_json::Value::Null);
                continue;
            }

            // 按 MySQL 类型名称解码
            let json_value = decode_mysql_column(row, col.ordinal(), &type_name)?;
            columns.insert(col_name, json_value);
        }

        Ok(Record { columns })
    }
}

/// 根据 MySQL 列类型解码列值为 JSON 值
///
/// # 参数
///
/// - `row`：MySQL 行
/// - `ordinal`：列索引
/// - `type_name`：MySQL 类型名称（大写）
///
/// # 返回
///
/// - `Ok(Value)`：解码成功
/// - `Err(sqlx::Error)`：解码失败
#[cfg(feature = "mysql")]
fn decode_mysql_column(
    row: &sqlx::mysql::MySqlRow,
    ordinal: usize,
    type_name: &str,
) -> Result<serde_json::Value, sqlx::Error> {
    use base64::Engine;
    use sqlx::Row;

    // 整数类型：INT、BIGINT、MEDIUMINT、SMALLINT
    if type_name.contains("INT") && !type_name.contains("TINYINT") {
        let val: i64 = row.try_get(ordinal)?;
        return Ok(serde_json::Value::Number(val.into()));
    }

    // TINYINT(1) 通常用作 BOOLEAN，但 MySQL 中 BOOLEAN 实际上是 TINYINT
    // 按照 MySQL 约定，TINYINT 映射为 bool（0 = false，非 0 = true）
    if type_name == "BOOLEAN" || type_name == "TINYINT" {
        let val: i8 = row.try_get(ordinal)?;
        return Ok(serde_json::Value::Bool(val != 0));
    }

    // 定点类型：DECIMAL、NUMERIC
    // NEWDECIMAL 协议本就是字符串编码，读字符串以保留精度，避免 f64 舍入误差。
    if type_name == "DECIMAL" || type_name == "NUMERIC" {
        let val: String = row.try_get_unchecked(ordinal)?;
        return Ok(serde_json::Value::String(val));
    }

    // 浮点类型：FLOAT、DOUBLE
    if type_name == "FLOAT" || type_name == "DOUBLE" {
        let val: f64 = row.try_get(ordinal)?;
        // NaN 或 Infinity 无法编码为 JSON Number，返回结构化解码错误而非静默归零
        let json_num = serde_json::Number::from_f64(val).ok_or_else(|| {
            sqlx::Error::Decode("浮点数值为 NaN 或 Infinity 无法编码为 JSON Number".into())
        })?;
        return Ok(serde_json::Value::Number(json_num));
    }

    // 日期时间类型：DATE、DATETIME、TIMESTAMP
    if type_name == "DATE" {
        let val: chrono::NaiveDate = row.try_get(ordinal)?;
        // 转换为 ISO 8601 格式字符串
        return Ok(serde_json::Value::String(
            val.format("%Y-%m-%d").to_string(),
        ));
    }

    if type_name == "DATETIME" || type_name == "TIMESTAMP" {
        let val: chrono::NaiveDateTime = row.try_get(ordinal)?;
        // 转换为 ISO 8601 格式字符串
        return Ok(serde_json::Value::String(
            val.format("%Y-%m-%dT%H:%M:%S").to_string(),
        ));
    }

    if type_name == "TIME" {
        let val: chrono::NaiveTime = row.try_get(ordinal)?;
        return Ok(serde_json::Value::String(
            val.format("%H:%M:%S").to_string(),
        ));
    }

    // 二进制类型：BLOB、BINARY、VARBINARY、LONGBLOB、MEDIUMBLOB、TINYBLOB
    if type_name.contains("BLOB") || type_name.contains("BINARY") {
        let val: Vec<u8> = row.try_get(ordinal)?;
        // 转换为 Base64 字符串
        let encoded = base64::engine::general_purpose::STANDARD.encode(&val);
        return Ok(serde_json::Value::String(encoded));
    }

    // JSON 类型：直接解析为 JSON 值
    if type_name == "JSON" {
        let val: serde_json::Value = row.try_get(ordinal)?;
        return Ok(val);
    }

    // 字符串类型：VARCHAR、TEXT、CHAR、LONGTEXT、MEDIUMTEXT、TINYTEXT、ENUM、SET
    // 以及其他未知类型，尝试作为字符串读取
    let val: String = row.try_get(ordinal)?;
    Ok(serde_json::Value::String(val))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn get_rejects_blank_column_name() {
        let mut row = Record::new();
        row.insert("", json!(1));
        row.insert("   ", json!(2));
        row.insert("name", json!("Alice"));

        assert_eq!(
            row.get("name").and_then(|value| value.as_str()),
            Some("Alice")
        );
        assert_eq!(row.get(""), None);
        assert_eq!(row.get("   "), None);
    }

    #[test]
    fn serializes_as_plain_object_and_checks_types() {
        let row = Record::new().set("id", 7).set("name", "Alice");

        assert_eq!(
            serde_json::to_value(&row).expect("记录应可序列化"),
            json!({
                "id": 7,
                "name": "Alice"
            })
        );
        assert_eq!(row.require::<i64>("id").expect("id 应为整数"), 7);
        assert!(row.require::<String>("id").is_err());
        assert!(row.require::<String>("missing").is_err());
    }
}
