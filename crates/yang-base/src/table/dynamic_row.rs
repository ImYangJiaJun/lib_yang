//! 动态行类型
//!
//! 提供 `DynamicRow` 类型，用于在不知道具体表结构时动态读取 MySQL 查询结果。
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
//! use yang_base::table::DynamicRow;
//!
//! // 通过 sqlx::FromRow 自动从查询结果构建
//! let rows: Vec<DynamicRow> = query.select().await?;
//! for row in rows {
//!     println!("{:?}", row.columns);
//! }
//! ```

use serde::{Deserialize, Serialize};

/// 动态行类型
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
/// use yang_base::table::DynamicRow;
/// use serde_json::json;
///
/// let mut row = DynamicRow::new();
/// row.columns.insert("id".to_string(), json!(1));
/// row.columns.insert("name".to_string(), json!("Alice"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicRow {
    /// 列名到 JSON 值的映射
    ///
    /// 键为列名，值为对应的 JSON 值（已按 MySQL 类型转换）
    pub columns: serde_json::Map<String, serde_json::Value>,
}

impl DynamicRow {
    /// 创建空的动态行
    pub fn new() -> Self {
        Self {
            columns: serde_json::Map::new(),
        }
    }

    /// 获取指定列的值
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
}

impl Default for DynamicRow {
    fn default() -> Self {
        Self::new()
    }
}

/// 将 DynamicRow 转换为 serde_json::Value（Object 类型）
impl From<DynamicRow> for serde_json::Value {
    fn from(row: DynamicRow) -> Self {
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
impl<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> for DynamicRow {
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

        Ok(DynamicRow { columns })
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
        let mut row = DynamicRow::new();
        row.columns.insert("".to_string(), json!(1));
        row.columns.insert("   ".to_string(), json!(2));
        row.columns.insert("name".to_string(), json!("Alice"));

        assert_eq!(
            row.get("name").and_then(|value| value.as_str()),
            Some("Alice")
        );
        assert_eq!(row.get(""), None);
        assert_eq!(row.get("   "), None);
    }
}
