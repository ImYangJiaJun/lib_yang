//! 数据库记录类型
//!
//! 提供 `Record` 类型，用于在不知道具体 Rust 实体时安全读取 MySQL / PostgreSQL
//! 查询结果。每一行数据以 `serde_json::Map<String, serde_json::Value>` 的形式存储，
//! 支持两个后端常见类型到 JSON 类型的映射。
//!
//! # MySQL 类型映射规则
//!
//! | MySQL 类型 | JSON 类型 |
//! |-----------|----------|
//! | INT / BIGINT / MEDIUMINT / SMALLINT | Number (i64) |
//! | TINYINT | Number (i8) |
//! | TINYINT UNSIGNED | Number (u8) |
//! | FLOAT / DOUBLE | Number (f64) |
//! | DECIMAL / NUMERIC | String（保留精度，NEWDECIMAL 协议本就是字符串编码） |
//! | VARCHAR / TEXT / CHAR | String |
//! | BOOLEAN（即 TINYINT(1)，sqlx 归一化） | Bool |
//! | DATE / DATETIME / TIMESTAMP | String (ISO 8601) |
//! | NULL | Null |
//! | BLOB / BINARY | String (Base64) |
//! | JSON | Object / Array |
//!
//! # PostgreSQL 类型映射规则
//!
//! 按 `PgColumn::type_info()` 的 OID / `PgTypeKind` 分派，而不是按类型名字符串，
//! 避免同名的用户自定义类型被误判。
//!
//! | PostgreSQL 类型 | JSON 类型 |
//! |-----------------|----------|
//! | BOOL | Bool |
//! | INT2 / INT4 / INT8 | Number (i64) |
//! | FLOAT4 / FLOAT8 | Number (f64)（NaN / Infinity 无法表示，返回解码错误） |
//! | NUMERIC | String（十进制文本，保留 scale） |
//! | TEXT / VARCHAR / BPCHAR / NAME / ENUM | String |
//! | BYTEA | String (Base64) |
//! | DATE | String（ISO 8601 `YYYY-MM-DD`） |
//! | TIME | String（ISO 8601 `HH:MM:SS[.fffffffff]`） |
//! | TIMETZ | String（ISO 8601 `HH:MM:SS[.f]±HH:MM`） |
//! | TIMESTAMP | String（ISO 8601 `YYYY-MM-DDTHH:MM:SS[.f]`，无时区） |
//! | TIMESTAMPTZ | String（UTC RFC 3339，如 `2026-11-08T10:00:00Z`） |
//! | INTERVAL | Object `{"months":i32,"days":i32,"microseconds":i64}` |
//! | UUID | String（连字符小写形式） |
//! | JSON / JSONB | 对应 JSON 值 |
//! | INET / CIDR | String（PostgreSQL 文本表示，含掩码时形如 `192.168.1.1/24`） |
//! | int4range / int8range / numrange / daterange / tstzrange | Range 对象（见下） |
//! | 数组（一维） | JSON Array，元素按上表转换，数组内 NULL → JSON Null |
//! | SQL NULL | Null |
//!
//! ## Range 契约
//!
//! 非空 range 输出为 `{"lower": ..., "upper": ..., "lower_inclusive": bool, "upper_inclusive": bool}`：
//! 端点按元素类型的标量规则解码；无界端点用 JSON `null` 表示，对应侧的
//! `*_inclusive` 固定为 `false`。空 range 显式输出 `{"empty": true}`，与
//! `(,)`（两端无界但非空）严格区分。按内置 range OID 分派；未知 range 元素类型
//! （如 `tsrange` 或用户自定义 range）fail-closed。
//!
//! ## 自定义类型解码器
//!
//! 内置规则无法解码的自定义类型（composite / 扩展类型）可由调用方通过
//! [`register_pg_type_decoder`] 注册解码器。解码分派顺序为：注册表 → 内置规则
//! → 结构化错误；重复注册同一 OID 会覆盖旧解码器。
//!
//! ## PostgreSQL 解码约定
//!
//! - 列名保持数据库返回的原样；重复列名时后者覆盖前者（JSON Object 无法保留重复键）。
//! - SQL NULL 与 JSONB 字面量 `null` 的序列化值都是 JSON `Null`，但后者额外登记在
//!   sidecar 集合中，可用 [`Record::is_sql_null`] / [`Record::is_json_null`] 区分。
//! - 未知 OID / 不支持的类型（如 POINT、未注册的 COMPOSITE、未知 RANGE）返回
//!   `sqlx::Error::ColumnDecode`，消息包含列名、序号、OID 与类型名，绝不静默置 Null。
//! - 数组仅支持一维（sqlx 的数组解码限制）；多维数组返回解码错误。
//! - INTERVAL 用无损的三段整数对象表示：PostgreSQL 的 months / days / microseconds
//!   三者符号可以不一致，ISO 8601 duration 无法精确表达这种组合。
//! - sqlx 的多数解码实现同时支持二进制（扩展协议）与文本（简单协议）结果格式；
//!   例外是 INTERVAL 的文本格式 sqlx 尚未实现，此时返回解码错误而非错误值。
//! - 自定义 ENUM / DOMAIN / 自定义数组依赖扩展协议解析出的类型元数据；简单协议
//!   （如 `sqlx::raw_sql`）无法解析自定义 OID，会返回带 OID 的结构化错误。
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
    /// PostgreSQL 中值为 JSON 字面量 `null` 的 `json`/`jsonb` 列名。
    ///
    /// JSON 值与 SQL NULL 在解包后都是 [`serde_json::Value::Null`]，无法从值本身
    /// 区分。该集合只记录「列值确实是 JSON null」的列，用于
    /// [`Record::is_json_null`] 与 [`Record::is_sql_null`]；不参与序列化，
    /// `get()` 与 JSON 输出行为保持不变。
    #[serde(skip)]
    #[schemars(skip)]
    json_nulls: std::collections::BTreeSet<String>,
}

impl Record {
    /// 创建空记录。
    pub fn new() -> Self {
        Self {
            columns: serde_json::Map::new(),
            json_nulls: std::collections::BTreeSet::new(),
        }
    }

    /// 该列是否为 SQL NULL（区别于 JSON 字面量 `null`）。
    ///
    /// 列不存在时返回 `false`。
    pub fn is_sql_null(&self, key: &str) -> bool {
        self.columns
            .get(key)
            .is_some_and(serde_json::Value::is_null)
            && !self.json_nulls.contains(key)
    }

    /// 该列是否为 PostgreSQL `json`/`jsonb` 的 JSON 字面量 `null`。
    pub fn is_json_null(&self, key: &str) -> bool {
        self.json_nulls.contains(key)
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

    /// 移除指定列并返回其原值。
    ///
    /// 同时从 JSON null sidecar 中移除该列，避免列已不存在却仍被
    /// [`Record::is_json_null`] 判定为 JSON null。
    ///
    /// # 返回
    ///
    /// - `Some(Value)`：列存在时返回被移除的值
    /// - `None`：列不存在
    pub fn remove_column(&mut self, key: &str) -> Option<serde_json::Value> {
        self.json_nulls.remove(key);
        self.columns.remove(key)
    }

    /// 只保留指定列，其余列及其 JSON null 登记一并移除。
    ///
    /// 用于多列投影：投影结果必须保留 [`Record::is_json_null`] 的区分能力，
    /// 不能经 `into_map()` + [`Record::from`] 重建（那会丢失 sidecar）。
    pub fn retain_columns(&mut self, keys: &[&str]) {
        self.columns.retain(|name, _| keys.contains(&name.as_str()));
        self.json_nulls.retain(|name| keys.contains(&name.as_str()));
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
        Self {
            columns,
            json_nulls: std::collections::BTreeSet::new(),
        }
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
/// - INT/BIGINT/MEDIUMINT/SMALLINT → i64
/// - TINYINT → i8
/// - TINYINT UNSIGNED → u8
/// - FLOAT/DOUBLE → f64
/// - DECIMAL/NUMERIC → String（保留精度）
/// - VARCHAR/TEXT/CHAR → String
/// - BOOLEAN（即 TINYINT(1)）→ Bool
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

        Ok(Record {
            columns,
            json_nulls: std::collections::BTreeSet::new(),
        })
    }
}

/// MySQL 整数族列在解码时的语义分类。
///
/// sqlx 会把 `tinyint(1)` 归一化为 `BOOLEAN`（列宽 == 1），其余 tinyint 保持数值语义。
/// 抽出纯函数便于在无 MySQL 环境下单测「列名 → 解码分支」的映射，防止
/// BOOLEAN / TINYINT / TINYINT UNSIGNED 之间的语义再次漂移（验证需求: M24）。
#[cfg(feature = "mysql")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MysqlIntegerKind {
    /// 常规整数（INT/BIGINT/MEDIUMINT/SMALLINT 及其无符号变体），解码为 i64。
    Int64,
    /// tinyint(1)（sqlx 归一化为 BOOLEAN），解码为 bool。
    Boolean,
    /// 普通 tinyint，保持数值语义，解码为 i8。
    TinyInt,
    /// 无符号 tinyint，解码为 u8。
    TinyIntUnsigned,
    /// 非整数族类型，交由后续分支按字符串/浮点/日期等处理。
    Other,
}

/// 判定 MySQL 列类型名对应的整数族解码分支。
#[cfg(feature = "mysql")]
fn mysql_integer_kind(type_name: &str) -> MysqlIntegerKind {
    // 整数类型：INT、BIGINT、MEDIUMINT、SMALLINT（含 UNSIGNED 变体）
    if type_name.contains("INT") && !type_name.contains("TINYINT") {
        return MysqlIntegerKind::Int64;
    }
    match type_name {
        // tinyint(1) 在 MySQL 协议中被 sqlx 归一化为 BOOLEAN（列宽 == 1）
        "BOOLEAN" => MysqlIntegerKind::Boolean,
        "TINYINT" => MysqlIntegerKind::TinyInt,
        "TINYINT UNSIGNED" => MysqlIntegerKind::TinyIntUnsigned,
        _ => MysqlIntegerKind::Other,
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

    match mysql_integer_kind(type_name) {
        // 整数类型：INT、BIGINT、MEDIUMINT、SMALLINT（含 UNSIGNED 变体）
        MysqlIntegerKind::Int64 => {
            let val: i64 = row.try_get(ordinal)?;
            return Ok(serde_json::Value::Number(val.into()));
        }
        // tinyint(1) 在 MySQL 协议中被 sqlx 归一化为 BOOLEAN（列宽 == 1）；
        // 仅该形态映射为 bool，其余 tinyint 保持数值语义，避免与 FieldType::Integer 声明冲突。
        MysqlIntegerKind::Boolean => {
            let val: i8 = row.try_get(ordinal)?;
            return Ok(serde_json::Value::Bool(val != 0));
        }
        // 普通 tinyint 保持数值语义，不再映射为 bool。
        MysqlIntegerKind::TinyInt => {
            let val: i8 = row.try_get(ordinal)?;
            return Ok(serde_json::Value::Number(i64::from(val).into()));
        }
        // 无符号 tinyint 此前落到字符串兜底，try_get::<String> 因类型不兼容必然 Err。
        MysqlIntegerKind::TinyIntUnsigned => {
            let val: u8 = row.try_get(ordinal)?;
            return Ok(serde_json::Value::Number(u64::from(val).into()));
        }
        MysqlIntegerKind::Other => {}
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

/// PostgreSQL 内置 range 的元素类别。
#[cfg(feature = "postgres")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PgRangeKind {
    Int4,
    Int8,
    Numeric,
    Date,
    TimestampTz,
}

/// PostgreSQL 标量列的解码类别。
///
/// 按 OID 归类后再统一走标量与数组两条路径，保证同一个类型在标量和数组中的
/// JSON 表示完全一致。
#[cfg(feature = "postgres")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PgScalarKind {
    Bool,
    Int2,
    Int4,
    Int8,
    Float4,
    Float8,
    Numeric,
    /// TEXT / VARCHAR / BPCHAR / NAME。
    Text,
    Bytea,
    Date,
    Time,
    TimeTz,
    Timestamp,
    TimestampTz,
    Interval,
    Uuid,
    /// JSON / JSONB。
    Json,
    Inet,
    Cidr,
    /// 用户自定义枚举，按标签文本输出。
    Enum,
    /// 内置 range 类型，解码为 JSON 对象。
    Range(PgRangeKind),
}

/// 列的解码计划。
#[cfg(feature = "postgres")]
#[derive(Debug, Clone, PartialEq, Eq)]
enum PgDecodePlan {
    /// 标量类型。
    Scalar(PgScalarKind),
    /// 一维数组，元素沿用同一套标量解码。
    Array(Box<PgDecodePlan>),
    /// 调用方注册的自定义解码器。
    Custom(u32),
    /// 不支持的类型，携带类型名与 OID 供结构化错误使用。
    Unsupported(String),
}

/// 注册型 PostgreSQL 解码器收到的原始列值。
///
/// 扩展协议（prepared statement）结果为 [`PgValue::Binary`]，简单协议
/// （如 `sqlx::raw_sql`）结果为 [`PgValue::Text`]。
#[cfg(feature = "postgres")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PgValue<'a> {
    /// 二进制（扩展协议）结果格式。
    Binary(&'a [u8]),
    /// 文本（简单协议）结果格式。
    Text(&'a str),
}

#[cfg(feature = "postgres")]
impl<'a> PgValue<'a> {
    /// 二进制格式的原始字节；文本格式返回 `None`。
    pub fn as_bytes(&self) -> Option<&'a [u8]> {
        match self {
            Self::Binary(bytes) => Some(bytes),
            Self::Text(_) => None,
        }
    }

    /// 文本格式的字符串；二进制格式返回 `None`。
    pub fn as_str(&self) -> Option<&'a str> {
        match self {
            Self::Binary(_) => None,
            Self::Text(text) => Some(text),
        }
    }
}

/// 自定义 PostgreSQL 类型解码器。
#[cfg(feature = "postgres")]
pub type PgTypeDecoder = for<'a> fn(&PgValue<'a>) -> Result<serde_json::Value, BaseError>;

/// 进程级自定义解码器注册表。
///
/// 这是 [`register_pg_type_decoder`] 明确要求的扩展点，不是资源所有权单例；
/// 读写均经 `RwLock`，中毒时取回内部值继续服务，避免解码路径 panic。
#[cfg(feature = "postgres")]
static PG_TYPE_DECODERS: std::sync::OnceLock<
    std::sync::RwLock<std::collections::HashMap<u32, PgTypeDecoder>>,
> = std::sync::OnceLock::new();

#[cfg(feature = "postgres")]
fn pg_type_decoders() -> &'static std::sync::RwLock<std::collections::HashMap<u32, PgTypeDecoder>> {
    PG_TYPE_DECODERS.get_or_init(|| std::sync::RwLock::new(std::collections::HashMap::new()))
}

/// 注册自定义 OID 解码器。
///
/// 注册后，该 OID 优先于内置规则解码。重复注册同一 OID 会覆盖旧解码器
/// （幂等；线程安全）。解码器返回的 [`BaseError`] 会被包装为带列上下文的
/// `sqlx::Error::ColumnDecode`。
#[cfg(feature = "postgres")]
pub fn register_pg_type_decoder(oid: u32, decoder: PgTypeDecoder) {
    let mut decoders = match pg_type_decoders().write() {
        Ok(decoders) => decoders,
        Err(poisoned) => poisoned.into_inner(),
    };
    decoders.insert(oid, decoder);
}

#[cfg(feature = "postgres")]
fn lookup_pg_decoder(oid: u32) -> Option<PgTypeDecoder> {
    let decoders = match pg_type_decoders().read() {
        Ok(decoders) => decoders,
        Err(poisoned) => poisoned.into_inner(),
    };
    decoders.get(&oid).copied()
}

/// 按 PostgreSQL 内置类型的稳定 OID 归类标量类型。
///
/// 只在 `PgTypeKind::Simple` 分支使用；未知 OID 返回 `None`，由调用方转成结构化错误。
#[cfg(feature = "postgres")]
fn pg_scalar_kind(oid: u32) -> Option<PgScalarKind> {
    Some(match oid {
        // pg_type.dat 中的内置 OID 是稳定的。
        16 => PgScalarKind::Bool,
        21 => PgScalarKind::Int2,
        23 => PgScalarKind::Int4,
        20 => PgScalarKind::Int8,
        700 => PgScalarKind::Float4,
        701 => PgScalarKind::Float8,
        1700 => PgScalarKind::Numeric,
        25 | 1043 | 1042 | 19 => PgScalarKind::Text,
        17 => PgScalarKind::Bytea,
        1082 => PgScalarKind::Date,
        1083 => PgScalarKind::Time,
        1266 => PgScalarKind::TimeTz,
        1114 => PgScalarKind::Timestamp,
        1184 => PgScalarKind::TimestampTz,
        1186 => PgScalarKind::Interval,
        2950 => PgScalarKind::Uuid,
        114 | 3802 => PgScalarKind::Json,
        869 => PgScalarKind::Inet,
        650 => PgScalarKind::Cidr,
        // 内置 range 类型的 OID 同样是稳定的；只识别本轮承诺的五种。
        3904 => PgScalarKind::Range(PgRangeKind::Int4),
        3926 => PgScalarKind::Range(PgRangeKind::Int8),
        3906 => PgScalarKind::Range(PgRangeKind::Numeric),
        3912 => PgScalarKind::Range(PgRangeKind::Date),
        3910 => PgScalarKind::Range(PgRangeKind::TimestampTz),
        _ => return None,
    })
}

/// 按 PostgreSQL 内置数组类型的稳定 OID 归类其元素类型。
///
/// 直接识别内置数组 OID 可以绕开 `PgTypeKind::Array`，从而同时适配
/// 扩展协议（二进制）与简单协议（文本）结果。
#[cfg(feature = "postgres")]
fn pg_array_element_kind(oid: u32) -> Option<PgScalarKind> {
    Some(match oid {
        1000 => PgScalarKind::Bool,
        1001 => PgScalarKind::Bytea,
        1003 => PgScalarKind::Text,
        1005 => PgScalarKind::Int2,
        1007 => PgScalarKind::Int4,
        1009 => PgScalarKind::Text,
        1014 => PgScalarKind::Text,
        1015 => PgScalarKind::Text,
        1016 => PgScalarKind::Int8,
        1021 => PgScalarKind::Float4,
        1022 => PgScalarKind::Float8,
        1041 => PgScalarKind::Inet,
        651 => PgScalarKind::Cidr,
        1115 => PgScalarKind::Timestamp,
        1182 => PgScalarKind::Date,
        1183 => PgScalarKind::Time,
        1185 => PgScalarKind::TimestampTz,
        1187 => PgScalarKind::Interval,
        1231 => PgScalarKind::Numeric,
        1270 => PgScalarKind::TimeTz,
        199 => PgScalarKind::Json,
        2951 => PgScalarKind::Uuid,
        3807 => PgScalarKind::Json,
        _ => return None,
    })
}

/// 生成不支持类型的描述（类型名 + OID），用于结构化解码错误。
#[cfg(feature = "postgres")]
fn pg_unsupported_description(oid: Option<u32>, type_name: &str) -> String {
    match oid {
        Some(oid) => format!("{type_name}（OID {oid}）"),
        None => format!("{type_name}（OID 未知）"),
    }
}

/// 根据列类型信息生成解码计划。
///
/// 先按稳定的内置 OID 直接分派标量与数组；只有自定义类型才依赖 `PgTypeKind`
/// 递归（`Array` / `Domain` / `Enum`）。
#[cfg(feature = "postgres")]
fn pg_decode_plan(type_info: &sqlx::postgres::PgTypeInfo) -> PgDecodePlan {
    use sqlx::postgres::PgTypeKind;
    use sqlx::TypeInfo;

    let oid = type_info.oid().map(|oid| oid.0);

    // 注册表优先于内置规则，允许调用方覆盖任意 OID 的解码方式。
    if let Some(oid) = oid {
        if lookup_pg_decoder(oid).is_some() {
            return PgDecodePlan::Custom(oid);
        }
    }

    // 内置标量与内置数组的 OID 是稳定的，且不依赖类型名解析。
    if let Some(kind) = oid.and_then(pg_scalar_kind) {
        return PgDecodePlan::Scalar(kind);
    }
    if let Some(kind) = oid.and_then(pg_array_element_kind) {
        return PgDecodePlan::Array(Box::new(PgDecodePlan::Scalar(kind)));
    }

    // 简单协议（如 sqlx::raw_sql）无法解析自定义类型的 OID，sqlx 会留下
    // `DeclareWithOid`，其 name() 为 "?"；此时调用 kind() 会 panic（sqlx 内部
    // unreachable!），必须提前转成结构化错误。
    if type_info.name() == "?" {
        let detail = match oid {
            Some(oid) => {
                format!("未解析的自定义类型（OID {oid}；文本协议无法解析类型名，请使用扩展协议）")
            }
            None => "未解析的自定义类型（OID 未知）".to_string(),
        };
        return PgDecodePlan::Unsupported(detail);
    }

    match type_info.kind() {
        PgTypeKind::Array(element) => PgDecodePlan::Array(Box::new(pg_decode_plan(element))),
        PgTypeKind::Domain(inner) => pg_decode_plan(inner),
        kind => pg_decode_plan_from_kind(oid, type_info.name(), kind),
    }
}

/// 类型类别分派（不含 Array / Domain 递归，便于在无数据库环境做单元测试）。
#[cfg(feature = "postgres")]
fn pg_decode_plan_from_kind(
    oid: Option<u32>,
    type_name: &str,
    kind: &sqlx::postgres::PgTypeKind,
) -> PgDecodePlan {
    use sqlx::postgres::PgTypeKind;

    match kind {
        PgTypeKind::Enum(_) => PgDecodePlan::Scalar(PgScalarKind::Enum),
        PgTypeKind::Composite(_) => PgDecodePlan::Unsupported(format!(
            "复合类型 {} 没有注册解码器；请调用 yang_base::table::register_pg_type_decoder(oid, decoder)",
            pg_unsupported_description(oid, type_name)
        )),
        PgTypeKind::Range(_) => PgDecodePlan::Unsupported(format!(
            "range 类型 {} 不在内置支持列表（int4range/int8range/numrange/daterange/tstzrange）；\
             请调用 yang_base::table::register_pg_type_decoder(oid, decoder)",
            pg_unsupported_description(oid, type_name)
        )),
        PgTypeKind::Simple => match oid.and_then(pg_scalar_kind) {
            Some(kind) => PgDecodePlan::Scalar(kind),
            None => PgDecodePlan::Unsupported(pg_unsupported_description(oid, type_name)),
        },
        // Array / Domain 由 pg_decode_plan 递归处理；其余结构（Pseudo）不在本轮能力范围内。
        _ => PgDecodePlan::Unsupported(pg_unsupported_description(oid, type_name)),
    }
}

/// 构造带列上下文的「不支持类型」错误。
#[cfg(feature = "postgres")]
fn unsupported_pg_type_error(column_name: &str, ordinal: usize, detail: &str) -> sqlx::Error {
    sqlx::Error::ColumnDecode {
        index: format!("{column_name}#{ordinal}"),
        source: format!("不支持的 PostgreSQL 列类型：{detail}").into(),
    }
}

/// `INET` / `CIDR` 的轻量解码包装。
///
/// sqlx 默认不启用 `ipnetwork` feature，这里只按 PostgreSQL 二进制/文本格式提取
/// 地址与掩码，再按 `INET` 或 `CIDR` 的文本表示输出。
#[cfg(feature = "postgres")]
#[derive(Debug, Clone)]
struct PgInetValue {
    address: String,
    bits: u8,
    max_bits: u8,
}

#[cfg(feature = "postgres")]
impl PgInetValue {
    /// 输出 PostgreSQL 文本表示：`INET` 在掩码等于全宽时省略 `/bits`，`CIDR` 始终保留。
    fn to_json(&self, is_cidr: bool) -> serde_json::Value {
        if is_cidr || self.bits != self.max_bits {
            serde_json::Value::String(format!("{}/{}", self.address, self.bits))
        } else {
            serde_json::Value::String(self.address.clone())
        }
    }
}

#[cfg(feature = "postgres")]
impl sqlx::Type<sqlx::Postgres> for PgInetValue {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("inet")
    }
}

#[cfg(feature = "postgres")]
impl sqlx::postgres::PgHasArrayType for PgInetValue {
    fn array_type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("_inet")
    }
}

#[cfg(feature = "postgres")]
impl<'r> sqlx::Decode<'r, sqlx::Postgres> for PgInetValue {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        match value.format() {
            sqlx::postgres::PgValueFormat::Binary => parse_pg_inet_bytes(value.as_bytes()?),
            sqlx::postgres::PgValueFormat::Text => parse_pg_inet_text(value.as_str()?),
        }
    }
}

/// 解析 `INET` / `CIDR` 的二进制格式：
/// family(1) + bits(1) + is_cidr(1) + 地址字节数(1) + 地址字节
/// （IPv4 4 字节 / IPv6 16 字节）。
#[cfg(feature = "postgres")]
fn parse_pg_inet_bytes(bytes: &[u8]) -> Result<PgInetValue, sqlx::error::BoxDynError> {
    if bytes.len() < 4 {
        return Err("INET 二进制值长度不足 4 字节".into());
    }

    let family = bytes[0];
    let bits = bytes[1];
    let address_len = usize::from(bytes[3]);
    let address = &bytes[4..];

    let (address, max_bits) = match (family, address_len, address.len()) {
        // PGSQL_AF_INET
        (2, 4, 4) => (
            std::net::Ipv4Addr::new(address[0], address[1], address[2], address[3]).to_string(),
            32,
        ),
        // PGSQL_AF_INET6
        (3, 16, 16) => {
            let octets: [u8; 16] = address.try_into().map_err(|_| "INET 二进制地址长度非法")?;
            (std::net::Ipv6Addr::from(octets).to_string(), 128)
        }
        _ => {
            return Err(format!(
                "未知的 INET 地址族或长度：family={family}, address_len={address_len}, len={}",
                address.len()
            )
            .into())
        }
    };

    if bits > max_bits {
        return Err(format!("INET 掩码位数 {bits} 超出 {max_bits}").into());
    }

    Ok(PgInetValue {
        address,
        bits,
        max_bits,
    })
}

/// 解析文本格式的 `INET` / `CIDR`（简单协议结果）。
#[cfg(feature = "postgres")]
fn parse_pg_inet_text(text: &str) -> Result<PgInetValue, sqlx::error::BoxDynError> {
    let (address, bits) = match text.split_once('/') {
        Some((address, bits)) => (address, Some(bits)),
        None => (text, None),
    };

    let max_bits = if address.contains(':') { 128 } else { 32 };
    let bits = match bits {
        Some(bits) => bits.parse::<u8>()?,
        None => max_bits,
    };
    if bits > max_bits {
        return Err(format!("INET 掩码位数 {bits} 超出 {max_bits}").into());
    }

    // 解析一次地址，顺带统一为 IPv6 压缩表示。
    let address = if max_bits == 128 {
        address.parse::<std::net::Ipv6Addr>()?.to_string()
    } else {
        address.parse::<std::net::Ipv4Addr>()?.to_string()
    };

    Ok(PgInetValue {
        address,
        bits,
        max_bits,
    })
}

/// 千分位以下的纳秒按 ISO 8601 输出：整秒不带小数点，否则去掉尾部零。
#[cfg(feature = "postgres")]
fn format_pg_time(time: chrono::NaiveTime) -> String {
    use chrono::Timelike;
    let base = time.format("%H:%M:%S").to_string();
    let nanos = time.nanosecond();
    if nanos == 0 {
        base
    } else {
        let fraction = format!("{nanos:09}");
        format!("{base}.{}", fraction.trim_end_matches('0'))
    }
}

/// 输出 `±HH:MM[:SS]` 形式的时区偏移。
#[cfg(feature = "postgres")]
fn format_pg_offset(offset: chrono::FixedOffset) -> String {
    let seconds = offset.local_minus_utc();
    let sign = if seconds < 0 { '-' } else { '+' };
    let seconds = seconds.abs();
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    if secs == 0 {
        format!("{sign}{hours:02}:{minutes:02}")
    } else {
        format!("{sign}{hours:02}:{minutes:02}:{secs:02}")
    }
}

#[cfg(feature = "postgres")]
fn pg_bool_to_json(value: bool) -> Result<serde_json::Value, sqlx::Error> {
    Ok(serde_json::Value::Bool(value))
}

#[cfg(feature = "postgres")]
fn pg_i64_to_json(value: i64) -> Result<serde_json::Value, sqlx::Error> {
    Ok(serde_json::Value::Number(value.into()))
}

#[cfg(feature = "postgres")]
fn pg_f64_to_json(value: f64) -> Result<serde_json::Value, sqlx::Error> {
    // NaN / Infinity 无法编码为 JSON Number，返回结构化解码错误而非静默归零。
    serde_json::Number::from_f64(value)
        .map(serde_json::Value::Number)
        .ok_or_else(|| {
            sqlx::Error::Decode("浮点数值为 NaN 或 Infinity，无法编码为 JSON Number".into())
        })
}

#[cfg(feature = "postgres")]
fn pg_bigdecimal_to_json(value: sqlx::types::BigDecimal) -> Result<serde_json::Value, sqlx::Error> {
    // 保留 NUMERIC 的十进制 scale，不经过 f64。
    Ok(serde_json::Value::String(value.to_string()))
}

#[cfg(feature = "postgres")]
fn pg_string_to_json(value: String) -> Result<serde_json::Value, sqlx::Error> {
    Ok(serde_json::Value::String(value))
}

#[cfg(feature = "postgres")]
fn pg_bytea_to_json(value: Vec<u8>) -> Result<serde_json::Value, sqlx::Error> {
    use base64::Engine;
    Ok(serde_json::Value::String(
        base64::engine::general_purpose::STANDARD.encode(value),
    ))
}

#[cfg(feature = "postgres")]
fn pg_date_to_json(value: chrono::NaiveDate) -> Result<serde_json::Value, sqlx::Error> {
    Ok(serde_json::Value::String(
        value.format("%Y-%m-%d").to_string(),
    ))
}

#[cfg(feature = "postgres")]
fn pg_time_to_json(value: chrono::NaiveTime) -> Result<serde_json::Value, sqlx::Error> {
    Ok(serde_json::Value::String(format_pg_time(value)))
}

#[cfg(feature = "postgres")]
fn pg_timetz_to_json(
    value: sqlx::postgres::types::PgTimeTz<chrono::NaiveTime, chrono::FixedOffset>,
) -> Result<serde_json::Value, sqlx::Error> {
    Ok(serde_json::Value::String(format!(
        "{}{}",
        format_pg_time(value.time),
        format_pg_offset(value.offset)
    )))
}

#[cfg(feature = "postgres")]
fn pg_timestamp_to_json(value: chrono::NaiveDateTime) -> Result<serde_json::Value, sqlx::Error> {
    Ok(serde_json::Value::String(format!(
        "{}T{}",
        value.date().format("%Y-%m-%d"),
        format_pg_time(value.time())
    )))
}

#[cfg(feature = "postgres")]
fn pg_timestamptz_to_json(
    value: chrono::DateTime<chrono::Utc>,
) -> Result<serde_json::Value, sqlx::Error> {
    // sqlx 已把 TIMESTAMPTZ 归一化为 UTC；RFC 3339 是 ISO 8601 的子集。
    Ok(serde_json::Value::String(
        value.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true),
    ))
}

#[cfg(feature = "postgres")]
fn pg_interval_to_json(
    value: sqlx::postgres::types::PgInterval,
) -> Result<serde_json::Value, sqlx::Error> {
    // 三段整数无损表达 PostgreSQL interval；各段符号可能独立。
    Ok(serde_json::json!({
        "months": value.months,
        "days": value.days,
        "microseconds": value.microseconds,
    }))
}

#[cfg(feature = "postgres")]
fn pg_uuid_to_json(value: sqlx::types::Uuid) -> Result<serde_json::Value, sqlx::Error> {
    Ok(serde_json::Value::String(value.to_string()))
}

#[cfg(feature = "postgres")]
fn pg_json_to_json(value: serde_json::Value) -> Result<serde_json::Value, sqlx::Error> {
    Ok(value)
}

#[cfg(feature = "postgres")]
fn pg_inet_to_json(value: PgInetValue, is_cidr: bool) -> Result<serde_json::Value, sqlx::Error> {
    Ok(value.to_json(is_cidr))
}

/// 判断 range 原始值是否为空 range。
///
/// sqlx 的 `PgRange` 解码把空 range 的两端都还原为 `Unbounded`，与 `(,)`
/// 无法区分；这里在解码前按原始协议判定：二进制首字节的 `EMPTY` 标志位为
/// `0x01`，文本格式恰为 `empty`。
#[cfg(feature = "postgres")]
fn pg_range_is_empty(value: &sqlx::postgres::PgValueRef<'_>) -> Result<bool, sqlx::Error> {
    match value.format() {
        sqlx::postgres::PgValueFormat::Binary => {
            let bytes = value.as_bytes().map_err(sqlx::Error::Decode)?;
            Ok(bytes.first().is_some_and(|flags| flags & 0x01 != 0))
        }
        sqlx::postgres::PgValueFormat::Text => {
            let text = value.as_str().map_err(sqlx::Error::Decode)?;
            Ok(text == "empty")
        }
    }
}

/// 把一个 range 端点转换为 `(JSON 值, 是否闭区间)`；无界端点用 `null` + `false`。
#[cfg(feature = "postgres")]
fn pg_range_bound_to_json<T>(
    bound: std::ops::Bound<T>,
    convert: impl Fn(T) -> Result<serde_json::Value, sqlx::Error>,
) -> Result<(serde_json::Value, bool), sqlx::Error> {
    match bound {
        std::ops::Bound::Included(value) => Ok((convert(value)?, true)),
        std::ops::Bound::Excluded(value) => Ok((convert(value)?, false)),
        std::ops::Bound::Unbounded => Ok((serde_json::Value::Null, false)),
    }
}

/// 按文档契约把 range 转为 JSON 对象；空 range 明确输出 `{"empty": true}`。
#[cfg(feature = "postgres")]
fn pg_range_to_json<T>(
    range: sqlx::postgres::types::PgRange<T>,
    empty: bool,
    convert: impl Fn(T) -> Result<serde_json::Value, sqlx::Error>,
) -> Result<serde_json::Value, sqlx::Error> {
    if empty {
        return Ok(serde_json::json!({ "empty": true }));
    }
    let (lower, lower_inclusive) = pg_range_bound_to_json(range.start, &convert)?;
    let (upper, upper_inclusive) = pg_range_bound_to_json(range.end, convert)?;
    Ok(serde_json::json!({
        "lower": lower,
        "upper": upper,
        "lower_inclusive": lower_inclusive,
        "upper_inclusive": upper_inclusive,
    }))
}

/// 解码内置 range 列。
#[cfg(feature = "postgres")]
fn decode_pg_range(
    row: &sqlx::postgres::PgRow,
    ordinal: usize,
    kind: PgRangeKind,
) -> Result<serde_json::Value, sqlx::Error> {
    use sqlx::Row;

    let raw = row.try_get_raw(ordinal)?;
    let empty = pg_range_is_empty(&raw)?;
    // 空 range 在文本协议下的原始值是字面量 `empty`，交给 `PgRange<T>` 解码
    // 会先撞上元素类型解析错误（ParseIntError 等）；直接返回契约 JSON。
    if empty {
        return Ok(serde_json::json!({ "empty": true }));
    }

    match kind {
        PgRangeKind::Int4 => {
            let range: sqlx::postgres::types::PgRange<i32> = row.try_get_unchecked(ordinal)?;
            pg_range_to_json(range, empty, |value| pg_i64_to_json(i64::from(value)))
        }
        PgRangeKind::Int8 => {
            let range: sqlx::postgres::types::PgRange<i64> = row.try_get_unchecked(ordinal)?;
            pg_range_to_json(range, empty, pg_i64_to_json)
        }
        PgRangeKind::Numeric => {
            let range: sqlx::postgres::types::PgRange<sqlx::types::BigDecimal> =
                row.try_get_unchecked(ordinal)?;
            pg_range_to_json(range, empty, pg_bigdecimal_to_json)
        }
        PgRangeKind::Date => {
            let range: sqlx::postgres::types::PgRange<chrono::NaiveDate> =
                row.try_get_unchecked(ordinal)?;
            pg_range_to_json(range, empty, pg_date_to_json)
        }
        PgRangeKind::TimestampTz => {
            let range: sqlx::postgres::types::PgRange<chrono::DateTime<chrono::Utc>> =
                row.try_get_unchecked(ordinal)?;
            pg_range_to_json(range, empty, pg_timestamptz_to_json)
        }
    }
}

/// 调用注册表解码未知类型列。
#[cfg(feature = "postgres")]
fn decode_pg_custom(
    row: &sqlx::postgres::PgRow,
    ordinal: usize,
    column_name: &str,
    oid: u32,
) -> Result<serde_json::Value, sqlx::Error> {
    use sqlx::Row;

    let Some(decoder) = lookup_pg_decoder(oid) else {
        return Err(unsupported_pg_type_error(
            column_name,
            ordinal,
            &format!("OID {oid} 未注册解码器"),
        ));
    };

    let raw = row.try_get_raw(ordinal)?;
    let value = match raw.format() {
        sqlx::postgres::PgValueFormat::Binary => {
            PgValue::Binary(raw.as_bytes().map_err(sqlx::Error::Decode)?)
        }
        sqlx::postgres::PgValueFormat::Text => {
            PgValue::Text(raw.as_str().map_err(sqlx::Error::Decode)?)
        }
    };
    decoder(&value).map_err(|error| {
        unsupported_pg_type_error(
            column_name,
            ordinal,
            &format!("OID {oid} 的注册解码器失败: {error}"),
        )
    })
}

/// 解码单个 PostgreSQL 标量列。
#[cfg(feature = "postgres")]
fn decode_pg_scalar(
    row: &sqlx::postgres::PgRow,
    ordinal: usize,
    kind: PgScalarKind,
) -> Result<serde_json::Value, sqlx::Error> {
    use sqlx::Row;

    // 这里全部使用 `try_get_unchecked`：类型已经由 OID / PgTypeKind 精确匹配，
    // 且 Domain / Enum 的类型信息无法通过 `Type::compatible` 的默认规则匹配。
    match kind {
        PgScalarKind::Bool => pg_bool_to_json(row.try_get_unchecked(ordinal)?),
        PgScalarKind::Int2 => {
            pg_i64_to_json(i64::from(row.try_get_unchecked::<i16, _>(ordinal)?))
        }
        PgScalarKind::Int4 => {
            pg_i64_to_json(i64::from(row.try_get_unchecked::<i32, _>(ordinal)?))
        }
        PgScalarKind::Int8 => pg_i64_to_json(row.try_get_unchecked::<i64, _>(ordinal)?),
        PgScalarKind::Float4 => {
            pg_f64_to_json(f64::from(row.try_get_unchecked::<f32, _>(ordinal)?))
        }
        PgScalarKind::Float8 => pg_f64_to_json(row.try_get_unchecked::<f64, _>(ordinal)?),
        PgScalarKind::Numeric => {
            pg_bigdecimal_to_json(row.try_get_unchecked::<sqlx::types::BigDecimal, _>(ordinal)?)
        }
        PgScalarKind::Text | PgScalarKind::Enum => {
            pg_string_to_json(row.try_get_unchecked::<String, _>(ordinal)?)
        }
        PgScalarKind::Bytea => {
            pg_bytea_to_json(row.try_get_unchecked::<Vec<u8>, _>(ordinal)?)
        }
        PgScalarKind::Date => {
            pg_date_to_json(row.try_get_unchecked::<chrono::NaiveDate, _>(ordinal)?)
        }
        PgScalarKind::Time => {
            pg_time_to_json(row.try_get_unchecked::<chrono::NaiveTime, _>(ordinal)?)
        }
        PgScalarKind::TimeTz => pg_timetz_to_json(row.try_get_unchecked::<
            sqlx::postgres::types::PgTimeTz<chrono::NaiveTime, chrono::FixedOffset>,
            _,
        >(ordinal)?),
        PgScalarKind::Timestamp => {
            pg_timestamp_to_json(row.try_get_unchecked::<chrono::NaiveDateTime, _>(ordinal)?)
        }
        PgScalarKind::TimestampTz => {
            pg_timestamptz_to_json(row.try_get_unchecked::<chrono::DateTime<chrono::Utc>, _>(
                ordinal,
            )?)
        }
        PgScalarKind::Interval => pg_interval_to_json(
            row.try_get_unchecked::<sqlx::postgres::types::PgInterval, _>(ordinal)?,
        ),
        PgScalarKind::Uuid => {
            pg_uuid_to_json(row.try_get_unchecked::<sqlx::types::Uuid, _>(ordinal)?)
        }
        PgScalarKind::Json => {
            pg_json_to_json(row.try_get_unchecked::<serde_json::Value, _>(ordinal)?)
        }
        PgScalarKind::Inet => {
            pg_inet_to_json(row.try_get_unchecked::<PgInetValue, _>(ordinal)?, false)
        }
        PgScalarKind::Cidr => {
            pg_inet_to_json(row.try_get_unchecked::<PgInetValue, _>(ordinal)?, true)
        }
        PgScalarKind::Range(kind) => decode_pg_range(row, ordinal, kind),
    }
}

/// 解码一维 PostgreSQL 数组；元素转换与标量路径共用同一套函数。
#[cfg(feature = "postgres")]
fn decode_pg_array(
    row: &sqlx::postgres::PgRow,
    ordinal: usize,
    column_name: &str,
    element: &PgDecodePlan,
) -> Result<serde_json::Value, sqlx::Error> {
    use sqlx::Row;

    let PgDecodePlan::Scalar(kind) = element else {
        return Err(match element {
            PgDecodePlan::Unsupported(detail) => {
                unsupported_pg_type_error(column_name, ordinal, detail)
            }
            _ => sqlx::Error::Decode("不支持多维 PostgreSQL 数组".into()),
        });
    };

    // 数组内 NULL 用 `Option<T>` 保留；PostgreSQL 数组允许 NULL 元素。
    macro_rules! decode_elements {
        ($ty:ty, $convert:expr) => {{
            let values: Vec<Option<$ty>> = row.try_get_unchecked(ordinal)?;
            let mut elements = Vec::with_capacity(values.len());
            for value in values {
                elements.push(match value {
                    Some(value) => $convert(value)?,
                    None => serde_json::Value::Null,
                });
            }
            Ok(serde_json::Value::Array(elements))
        }};
    }

    match kind {
        PgScalarKind::Bool => decode_elements!(bool, pg_bool_to_json),
        PgScalarKind::Int2 => {
            decode_elements!(i16, |value: i16| { pg_i64_to_json(i64::from(value)) })
        }
        PgScalarKind::Int4 => {
            decode_elements!(i32, |value: i32| { pg_i64_to_json(i64::from(value)) })
        }
        PgScalarKind::Int8 => decode_elements!(i64, pg_i64_to_json),
        PgScalarKind::Float4 => {
            decode_elements!(f32, |value: f32| { pg_f64_to_json(f64::from(value)) })
        }
        PgScalarKind::Float8 => decode_elements!(f64, pg_f64_to_json),
        PgScalarKind::Numeric => {
            decode_elements!(sqlx::types::BigDecimal, pg_bigdecimal_to_json)
        }
        PgScalarKind::Text | PgScalarKind::Enum => {
            decode_elements!(String, pg_string_to_json)
        }
        PgScalarKind::Bytea => decode_elements!(Vec<u8>, pg_bytea_to_json),
        PgScalarKind::Date => decode_elements!(chrono::NaiveDate, pg_date_to_json),
        PgScalarKind::Time => decode_elements!(chrono::NaiveTime, pg_time_to_json),
        PgScalarKind::TimeTz => decode_elements!(
            sqlx::postgres::types::PgTimeTz<chrono::NaiveTime, chrono::FixedOffset>,
            pg_timetz_to_json
        ),
        PgScalarKind::Timestamp => {
            decode_elements!(chrono::NaiveDateTime, pg_timestamp_to_json)
        }
        PgScalarKind::TimestampTz => {
            decode_elements!(chrono::DateTime<chrono::Utc>, pg_timestamptz_to_json)
        }
        PgScalarKind::Interval => {
            decode_elements!(sqlx::postgres::types::PgInterval, pg_interval_to_json)
        }
        PgScalarKind::Uuid => decode_elements!(sqlx::types::Uuid, pg_uuid_to_json),
        PgScalarKind::Json => decode_elements!(serde_json::Value, pg_json_to_json),
        PgScalarKind::Inet => decode_elements!(PgInetValue, |value: PgInetValue| {
            pg_inet_to_json(value, false)
        }),
        PgScalarKind::Cidr => decode_elements!(PgInetValue, |value: PgInetValue| {
            pg_inet_to_json(value, true)
        }),
        // range 数组的 OID 不在内置分派表中，正常不会走到这里；显式 fail-closed。
        PgScalarKind::Range(_) => Err(sqlx::Error::Decode("不支持 PostgreSQL range 数组".into())),
    }
}

/// 解码单个 PostgreSQL 列。
#[cfg(feature = "postgres")]
fn decode_pg_column(
    row: &sqlx::postgres::PgRow,
    ordinal: usize,
    column_name: &str,
    plan: &PgDecodePlan,
) -> Result<serde_json::Value, sqlx::Error> {
    match plan {
        PgDecodePlan::Scalar(kind) => decode_pg_scalar(row, ordinal, *kind),
        PgDecodePlan::Array(element) => decode_pg_array(row, ordinal, column_name, element),
        PgDecodePlan::Custom(oid) => decode_pg_custom(row, ordinal, column_name, *oid),
        PgDecodePlan::Unsupported(detail) => {
            Err(unsupported_pg_type_error(column_name, ordinal, detail))
        }
    }
}

/// 实现 `sqlx::FromRow`，支持从 PostgreSQL 查询结果动态解码。
///
/// 与 MySQL 实现并存，按 feature 分别编译。类型分派基于 OID / `PgTypeKind`，
/// 不依赖可能被用户自定义类型复用的类型名。
#[cfg(feature = "postgres")]
impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for Record {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Column;
        use sqlx::Row;
        use sqlx::ValueRef;

        let mut columns = serde_json::Map::new();
        let mut json_nulls = std::collections::BTreeSet::new();

        for col in row.columns() {
            let name = col.name().to_string();
            let ordinal = col.ordinal();

            // SQL NULL 直接映射为 JSON Null，不再推断具体类型。
            if row.try_get_raw(ordinal)?.is_null() {
                columns.insert(name, serde_json::Value::Null);
                continue;
            }

            let plan = pg_decode_plan(col.type_info());
            let value = decode_pg_column(row, ordinal, &name, &plan)?;
            // json/jsonb 的 JSON 字面量 null 解码后同样是 Value::Null，这里单独登记，
            // 使调用方能与 SQL NULL 区分（序列化输出仍保持 null）。
            if value.is_null() && matches!(plan, PgDecodePlan::Scalar(PgScalarKind::Json)) {
                json_nulls.insert(name.clone());
            }
            // 列名保持数据库原样；重复列名时后者覆盖前者。
            columns.insert(name, value);
        }

        Ok(Record {
            columns,
            json_nulls,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_null_sidecar_distinguishes_sql_null() {
        let mut row = Record::new()
            .set("sql_null", json!(null))
            .set("json_null", json!(null));
        row.json_nulls.insert("json_null".to_string());

        assert!(row.is_sql_null("sql_null"));
        assert!(!row.is_json_null("sql_null"));
        assert!(row.is_json_null("json_null"));
        assert!(!row.is_sql_null("json_null"));
        assert!(!row.is_sql_null("missing"));

        // 既有读取与序列化行为不变：两者都仍是 JSON null。
        assert_eq!(row.get("json_null"), Some(&json!(null)));
        assert_eq!(
            serde_json::to_value(&row).expect("记录应可序列化"),
            json!({ "sql_null": null, "json_null": null })
        );
    }

    #[test]
    fn remove_column_preserves_json_null_sidecar() {
        let mut row = Record::new()
            .set("json_null", json!(null))
            .set("sql_null", json!(null))
            .set("extra", json!(null));
        row.json_nulls.insert("json_null".to_string());
        row.json_nulls.insert("extra".to_string());

        assert_eq!(row.remove_column("extra"), Some(json!(null)));
        assert!(
            !row.is_json_null("extra"),
            "被移除列的 JSON null 登记必须清除"
        );
        assert_eq!(row.get("extra"), None);
        assert_eq!(row.remove_column("extra"), None);
        assert_eq!(row.remove_column("missing"), None);

        // 其余列的 sidecar 不受影响。
        assert!(row.is_json_null("json_null"));
        assert!(!row.is_sql_null("json_null"));
        assert!(row.is_sql_null("sql_null"));
    }

    #[test]
    fn retain_columns_preserves_json_null_sidecar() {
        let mut row = Record::new()
            .set("keep_json", json!(null))
            .set("keep_sql", json!(null))
            .set("drop", json!(null));
        row.json_nulls.insert("keep_json".to_string());
        row.json_nulls.insert("drop".to_string());

        row.retain_columns(&["keep_json", "keep_sql"]);

        assert_eq!(row.as_map().len(), 2);
        assert!(row.is_json_null("keep_json"));
        assert!(!row.is_sql_null("keep_json"));
        assert!(row.is_sql_null("keep_sql"));
        assert_eq!(row.get("drop"), None);
        assert!(
            !row.is_json_null("drop"),
            "被裁掉列的 JSON null 登记必须清除"
        );
    }

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

    #[cfg(feature = "mysql")]
    #[test]
    fn mysql_integer_kind_distinguishes_boolean_tinyint_and_unsigned() {
        // 只有 tinyint(1)（sqlx 归一化为 BOOLEAN）才映射 bool，其余保持数值语义。
        assert_eq!(mysql_integer_kind("BOOLEAN"), MysqlIntegerKind::Boolean);
        assert_eq!(mysql_integer_kind("TINYINT"), MysqlIntegerKind::TinyInt);
        assert_eq!(
            mysql_integer_kind("TINYINT UNSIGNED"),
            MysqlIntegerKind::TinyIntUnsigned
        );
        assert_eq!(mysql_integer_kind("INT"), MysqlIntegerKind::Int64);
        assert_eq!(mysql_integer_kind("INT UNSIGNED"), MysqlIntegerKind::Int64);
        assert_eq!(mysql_integer_kind("BIGINT"), MysqlIntegerKind::Int64);
        assert_eq!(mysql_integer_kind("MEDIUMINT"), MysqlIntegerKind::Int64);
        assert_eq!(mysql_integer_kind("SMALLINT"), MysqlIntegerKind::Int64);
        assert_eq!(mysql_integer_kind("VARCHAR"), MysqlIntegerKind::Other);
    }

    #[cfg(feature = "postgres")]
    #[test]
    fn pg_scalar_kind_covers_required_oids() {
        // 覆盖需求列出的全部标量族，未知 OID 必须返回 None 而不是默认字符串。
        for (oid, expected) in [
            (16, PgScalarKind::Bool),
            (21, PgScalarKind::Int2),
            (23, PgScalarKind::Int4),
            (20, PgScalarKind::Int8),
            (700, PgScalarKind::Float4),
            (701, PgScalarKind::Float8),
            (1700, PgScalarKind::Numeric),
            (25, PgScalarKind::Text),
            (1043, PgScalarKind::Text),
            (1042, PgScalarKind::Text),
            (19, PgScalarKind::Text),
            (17, PgScalarKind::Bytea),
            (1082, PgScalarKind::Date),
            (1083, PgScalarKind::Time),
            (1266, PgScalarKind::TimeTz),
            (1114, PgScalarKind::Timestamp),
            (1184, PgScalarKind::TimestampTz),
            (1186, PgScalarKind::Interval),
            (2950, PgScalarKind::Uuid),
            (114, PgScalarKind::Json),
            (3802, PgScalarKind::Json),
            (869, PgScalarKind::Inet),
            (650, PgScalarKind::Cidr),
            (3904, PgScalarKind::Range(PgRangeKind::Int4)),
            (3926, PgScalarKind::Range(PgRangeKind::Int8)),
            (3906, PgScalarKind::Range(PgRangeKind::Numeric)),
            (3912, PgScalarKind::Range(PgRangeKind::Date)),
            (3910, PgScalarKind::Range(PgRangeKind::TimestampTz)),
        ] {
            assert_eq!(pg_scalar_kind(oid), Some(expected), "OID {oid} 映射漂移");
        }

        // POINT / 未列入内置清单的 range / 自定义类型等不得被静默归类。
        assert_eq!(pg_scalar_kind(600), None);
        assert_eq!(pg_scalar_kind(50_000), None);
        // tsrange 不在本轮承诺的五种内置 range 内，必须 fail-closed。
        assert_eq!(pg_scalar_kind(3908), None);
        assert_eq!(pg_scalar_kind(3905), None);
    }

    #[cfg(feature = "postgres")]
    #[test]
    fn pg_array_element_kind_covers_required_arrays() {
        assert_eq!(pg_array_element_kind(1000), Some(PgScalarKind::Bool));
        assert_eq!(pg_array_element_kind(1005), Some(PgScalarKind::Int2));
        assert_eq!(pg_array_element_kind(1007), Some(PgScalarKind::Int4));
        assert_eq!(pg_array_element_kind(1016), Some(PgScalarKind::Int8));
        assert_eq!(pg_array_element_kind(1021), Some(PgScalarKind::Float4));
        assert_eq!(pg_array_element_kind(1022), Some(PgScalarKind::Float8));
        assert_eq!(pg_array_element_kind(1231), Some(PgScalarKind::Numeric));
        assert_eq!(pg_array_element_kind(1009), Some(PgScalarKind::Text));
        assert_eq!(pg_array_element_kind(1003), Some(PgScalarKind::Text));
        assert_eq!(pg_array_element_kind(1014), Some(PgScalarKind::Text));
        assert_eq!(pg_array_element_kind(1015), Some(PgScalarKind::Text));
        assert_eq!(pg_array_element_kind(1001), Some(PgScalarKind::Bytea));
        assert_eq!(pg_array_element_kind(1182), Some(PgScalarKind::Date));
        assert_eq!(pg_array_element_kind(1183), Some(PgScalarKind::Time));
        assert_eq!(pg_array_element_kind(1270), Some(PgScalarKind::TimeTz));
        assert_eq!(pg_array_element_kind(1115), Some(PgScalarKind::Timestamp));
        assert_eq!(pg_array_element_kind(1185), Some(PgScalarKind::TimestampTz));
        assert_eq!(pg_array_element_kind(1187), Some(PgScalarKind::Interval));
        assert_eq!(pg_array_element_kind(2951), Some(PgScalarKind::Uuid));
        assert_eq!(pg_array_element_kind(199), Some(PgScalarKind::Json));
        assert_eq!(pg_array_element_kind(3807), Some(PgScalarKind::Json));
        assert_eq!(pg_array_element_kind(1041), Some(PgScalarKind::Inet));
        assert_eq!(pg_array_element_kind(651), Some(PgScalarKind::Cidr));

        // 内置标量 OID 不应当被误判为数组。
        assert_eq!(pg_array_element_kind(23), None);
        assert_eq!(pg_array_element_kind(600), None);
    }

    #[cfg(feature = "postgres")]
    #[test]
    fn pg_decode_plan_classifies_enum_simple_and_unsupported() {
        use sqlx::postgres::PgTypeKind;
        use std::sync::Arc;

        let simple = PgTypeKind::Simple;
        assert_eq!(
            pg_decode_plan_from_kind(Some(23), "INT4", &simple),
            PgDecodePlan::Scalar(PgScalarKind::Int4)
        );

        // 用户自定义枚举按标签文本解码。
        let enum_kind = PgTypeKind::Enum(Arc::from(vec!["happy".to_string()]));
        assert_eq!(
            pg_decode_plan_from_kind(Some(50_000), "mood", &enum_kind),
            PgDecodePlan::Scalar(PgScalarKind::Enum)
        );

        // 未知 OID 的描述必须带类型名与 OID，供结构化错误使用。
        match pg_decode_plan_from_kind(Some(600), "POINT", &simple) {
            PgDecodePlan::Unsupported(detail) => {
                assert!(detail.contains("POINT"), "{detail}");
                assert!(detail.contains("600"), "{detail}");
            }
            other => panic!("POINT 应判定为不支持，实得 {other:?}"),
        }

        // COMPOSITE / RANGE 等结构类型同样判为不支持。
        let composite =
            PgTypeKind::Composite(Arc::from(Vec::<(String, sqlx::postgres::PgTypeInfo)>::new()));
        assert!(matches!(
            pg_decode_plan_from_kind(Some(50_001), "pair", &composite),
            PgDecodePlan::Unsupported(ref detail) if detail.contains("pair")
        ));
    }

    #[cfg(feature = "postgres")]
    #[test]
    fn pg_time_formatting_is_iso8601() {
        use chrono::NaiveTime;

        let whole = NaiveTime::from_hms_opt(1, 2, 3).expect("合法时间");
        assert_eq!(format_pg_time(whole), "01:02:03");

        let fraction = NaiveTime::from_hms_nano_opt(12, 34, 56, 789_000_000).expect("合法时间");
        assert_eq!(format_pg_time(fraction), "12:34:56.789");

        // 纳秒级精度去尾零。
        let nanos = NaiveTime::from_hms_nano_opt(0, 0, 0, 123_400_000).expect("合法时间");
        assert_eq!(format_pg_time(nanos), "00:00:00.1234");

        let date = chrono::NaiveDate::from_ymd_opt(2026, 11, 8).expect("合法日期");
        let timestamp = date
            .and_hms_nano_opt(10, 30, 0, 500_000_000)
            .expect("合法时间");
        assert_eq!(
            pg_timestamp_to_json(timestamp).expect("日期时间应可编码"),
            json!("2026-11-08T10:30:00.5")
        );
    }

    #[cfg(feature = "postgres")]
    #[test]
    fn pg_timetz_formatting_includes_offset() {
        use chrono::{FixedOffset, NaiveTime};
        use sqlx::postgres::types::PgTimeTz;

        let time = NaiveTime::from_hms_opt(10, 30, 0).expect("合法时间");
        let east = FixedOffset::east_opt(8 * 3600).expect("合法偏移");
        assert_eq!(format_pg_offset(east), "+08:00");
        assert_eq!(
            pg_timetz_to_json(PgTimeTz { time, offset: east }).expect("timetz 应可编码"),
            json!("10:30:00+08:00")
        );

        let west = FixedOffset::east_opt(-(5 * 3600 + 30 * 60)).expect("合法偏移");
        assert_eq!(format_pg_offset(west), "-05:30");
    }

    #[cfg(feature = "postgres")]
    #[test]
    fn pg_interval_maps_to_lossless_object() {
        use sqlx::postgres::types::PgInterval;

        let value = pg_interval_to_json(PgInterval {
            months: 14,
            days: 3,
            microseconds: 14_706_789_000,
        })
        .expect("interval 应可编码");

        assert_eq!(
            value,
            json!({"months": 14, "days": 3, "microseconds": 14_706_789_000_i64})
        );
    }

    #[cfg(feature = "postgres")]
    #[test]
    fn pg_inet_binary_and_text_formats() {
        // AF_INET + 24 位掩码 + 4 字节地址长度 + 192.168.1.1
        let masked = [2_u8, 24, 0, 4, 192, 168, 1, 1];
        assert_eq!(
            parse_pg_inet_bytes(&masked)
                .expect("IPv4 应可解析")
                .to_json(false),
            json!("192.168.1.1/24")
        );

        // 掩码等于全宽时 INET 省略 /bits，CIDR 始终保留。
        let host = [2_u8, 32, 0, 4, 10, 0, 0, 1];
        assert_eq!(
            parse_pg_inet_bytes(&host)
                .expect("IPv4 应可解析")
                .to_json(false),
            json!("10.0.0.1")
        );
        assert_eq!(
            parse_pg_inet_bytes(&host)
                .expect("IPv4 应可解析")
                .to_json(true),
            json!("10.0.0.1/32")
        );

        // AF_INET6 + ::1
        let mut v6 = [0_u8; 20];
        v6[0] = 3;
        v6[1] = 128;
        v6[3] = 16;
        v6[19] = 1;
        assert_eq!(
            parse_pg_inet_bytes(&v6)
                .expect("IPv6 应可解析")
                .to_json(false),
            json!("::1")
        );

        assert_eq!(
            parse_pg_inet_text("192.168.1.1/24")
                .expect("文本 INET 应可解析")
                .to_json(false),
            json!("192.168.1.1/24")
        );
        assert_eq!(
            parse_pg_inet_text("::1")
                .expect("文本 INET 应可解析")
                .to_json(false),
            json!("::1")
        );
    }

    #[cfg(feature = "postgres")]
    #[test]
    fn pg_float_rejects_non_finite_values() {
        assert_eq!(pg_f64_to_json(2.25).expect("有限浮点应可编码"), json!(2.25));
        assert!(pg_f64_to_json(f64::NAN).is_err());
        assert!(pg_f64_to_json(f64::INFINITY).is_err());
    }

    #[cfg(feature = "postgres")]
    #[test]
    fn pg_range_bounds_follow_documented_json_contract() {
        use sqlx::postgres::types::PgRange;
        use std::ops::Bound;

        let bounded = PgRange {
            start: Bound::Included(1_i32),
            end: Bound::Excluded(5_i32),
        };
        assert_eq!(
            pg_range_to_json(bounded, false, |value: i32| pg_i64_to_json(i64::from(
                value
            )))
            .expect("range 应可编码"),
            json!({
                "lower": 1,
                "upper": 5,
                "lower_inclusive": true,
                "upper_inclusive": false,
            })
        );

        let unbounded = PgRange::<i64> {
            start: Bound::Unbounded,
            end: Bound::Included(10),
        };
        assert_eq!(
            pg_range_to_json(unbounded, false, pg_i64_to_json).expect("无界 range 应可编码"),
            json!({
                "lower": null,
                "upper": 10,
                "lower_inclusive": false,
                "upper_inclusive": true,
            })
        );

        // 空 range 与 `(,)` 必须明确区分。
        let empty = PgRange::<i64> {
            start: Bound::Unbounded,
            end: Bound::Unbounded,
        };
        assert_eq!(
            pg_range_to_json(empty, true, pg_i64_to_json).expect("空 range 应可编码"),
            json!({"empty": true})
        );
    }

    #[cfg(feature = "postgres")]
    #[test]
    fn pg_decoder_registry_prefers_and_overwrites() {
        fn first(_: &PgValue<'_>) -> Result<serde_json::Value, BaseError> {
            Ok(json!("first"))
        }
        fn second(_: &PgValue<'_>) -> Result<serde_json::Value, BaseError> {
            Ok(json!("second"))
        }

        let oid = 4_000_000_001_u32;
        register_pg_type_decoder(oid, first);
        let decoder = lookup_pg_decoder(oid).expect("注册后应能查回解码器");
        assert_eq!(
            decoder(&PgValue::Text("x")).expect("解码器应成功"),
            json!("first")
        );

        // 重复注册同一 OID 覆盖旧解码器。
        register_pg_type_decoder(oid, second);
        let decoder = lookup_pg_decoder(oid).expect("重复注册后应能查回解码器");
        assert_eq!(
            decoder(&PgValue::Text("x")).expect("解码器应成功"),
            json!("second")
        );
    }
}
