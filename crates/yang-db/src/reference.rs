//! 查询热路径使用的受控 SQL 引用与封闭操作符。

use crate::sql_types::QualifiedIdentifier;
use crate::DbError;
use std::fmt;

/// 构建期校验完成的数据库表引用。
///
/// 支持单段表名（`users`）或 `schema.table` 两段限定名。启用 `postgres` feature
/// 时按 PostgreSQL 标识符规则校验（Unicode 字母、非首字符 `$`、内嵌双引号，
/// 每段不超过 63 字节）；否则保持严格 ASCII 规则（MySQL）。渲染时逐段加方言引号。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TableRef {
    raw: String,
}

impl TableRef {
    /// 校验并创建表引用。
    ///
    /// `postgres` feature 下接受与 [`crate::postgres::identifier`] 一致的名称，
    /// 超长（> 63 字节）或非法字符返回 [`DbError::InvalidArgument`]。
    pub fn new(value: impl Into<String>) -> Result<Self, DbError> {
        let raw = value.into();
        #[cfg(feature = "postgres")]
        QualifiedIdentifier::parse_postgres_with_max_parts(&raw, 2)?;
        #[cfg(not(feature = "postgres"))]
        QualifiedIdentifier::parse_with_max_parts(&raw, 2)?;
        Ok(Self { raw })
    }

    /// 返回未加方言引号的已校验表名。
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    #[doc(hidden)]
    pub fn __from_validated_literal(value: &'static str) -> Self {
        Self {
            raw: value.to_string(),
        }
    }

    #[doc(hidden)]
    pub fn __from_validated_owned(raw: String) -> Self {
        Self { raw }
    }
}

impl fmt::Display for TableRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.raw)
    }
}

/// 构建期校验完成的列引用。
///
/// 支持 `column`、`table.column` 与 `schema.table.column` 三种形态（最多三段）。
/// 启用 `postgres` feature 时按 PostgreSQL 标识符规则校验（Unicode 字母、
/// 非首字符 `$`、内嵌双引号，每段不超过 63 字节）；否则保持严格 ASCII 规则
/// （MySQL）。渲染时逐段加方言引号。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldRef {
    raw: String,
    #[cfg(feature = "mysql")]
    mysql: String,
    #[cfg(feature = "postgres")]
    postgres: String,
}

impl FieldRef {
    /// 校验并创建字段引用。
    ///
    /// `postgres` feature 下接受与 [`crate::postgres::identifier`] 一致的名称，
    /// 超长（> 63 字节）或非法字符返回 [`DbError::InvalidArgument`]。
    pub fn new(value: impl Into<String>) -> Result<Self, DbError> {
        let raw = value.into();
        #[cfg(feature = "postgres")]
        {
            let postgres = QualifiedIdentifier::parse_postgres(&raw)?.render_postgres();
            Ok(Self {
                // 两 feature 同时启用时，MySQL 渲染仍只做反引号包裹：PG 规则
                // 禁止反引号，因此不会破坏引号封闭；MySQL 自身的受控入口
                // （mysql::identifier）仍按严格 ASCII 规则二次校验。
                #[cfg(feature = "mysql")]
                mysql: quote_qualified(&raw, '`'),
                postgres,
                raw,
            })
        }
        #[cfg(not(feature = "postgres"))]
        {
            let identifier = QualifiedIdentifier::parse(&raw)?;
            // 无 MySQL feature 时仅需完成校验，渲染结果不落字段。
            let _ = &identifier;
            Ok(Self {
                #[cfg(feature = "mysql")]
                mysql: identifier.render('`'),
                raw,
            })
        }
    }

    /// 返回未加方言引号的已校验字段名。
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    #[doc(hidden)]
    pub fn __from_validated_literal(value: &'static str) -> Self {
        Self {
            #[cfg(feature = "mysql")]
            mysql: value
                .split('.')
                .map(|part| format!("`{part}`"))
                .collect::<Vec<_>>()
                .join("."),
            #[cfg(feature = "postgres")]
            postgres: value
                .split('.')
                .map(|part| format!("\"{part}\""))
                .collect::<Vec<_>>()
                .join("."),
            raw: value.to_string(),
        }
    }

    #[doc(hidden)]
    pub fn __from_validated_owned(raw: String) -> Self {
        Self {
            #[cfg(feature = "mysql")]
            mysql: raw
                .split('.')
                .map(|part| format!("`{part}`"))
                .collect::<Vec<_>>()
                .join("."),
            #[cfg(feature = "postgres")]
            postgres: raw
                .split('.')
                .map(|part| format!("\"{part}\""))
                .collect::<Vec<_>>()
                .join("."),
            raw,
        }
    }

    #[cfg(feature = "mysql")]
    #[doc(hidden)]
    pub fn mysql_quoted(&self) -> &str {
        &self.mysql
    }

    #[cfg(feature = "postgres")]
    #[doc(hidden)]
    pub fn postgres_quoted(&self) -> &str {
        &self.postgres
    }
}

#[doc(hidden)]
pub const fn __validate_table_literal(value: &str) {
    validate_literal(value, 2);
}

#[doc(hidden)]
pub const fn __validate_field_literal(value: &str) {
    validate_literal(value, 3);
}

/// 编译期字面量校验：`max_parts` 为允许的最大段数（点号数 + 1）。
const fn validate_literal(value: &str, max_parts: usize) {
    let bytes = value.as_bytes();
    assert!(!bytes.is_empty(), "SQL identifier cannot be empty");
    let mut index = 0;
    let mut segment_start = true;
    let mut dots = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'.' {
            assert!(!segment_start, "identifier segment cannot be empty");
            dots += 1;
            assert!(dots < max_parts, "SQL identifier has too many segments");
            segment_start = true;
        } else if segment_start {
            assert!(
                byte == b'_' || byte.is_ascii_alphabetic(),
                "identifier must start with ASCII letter or underscore"
            );
            segment_start = false;
        } else {
            assert!(
                byte == b'_' || byte.is_ascii_alphanumeric(),
                "identifier contains invalid character"
            );
        }
        index += 1;
    }
    assert!(!segment_start, "identifier segment cannot be empty");
}

impl fmt::Display for FieldRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.raw)
    }
}

/// QueryBuilder 接受的封闭比较操作符。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum CompareOp {
    /// 等于。
    Eq,
    /// 不等于。
    Ne,
    /// 大于。
    Gt,
    /// 小于。
    Lt,
    /// 大于等于。
    Gte,
    /// 小于等于。
    Lte,
    /// SQL LIKE。
    Like,
    /// 大小写不敏感 LIKE（PostgreSQL `ILIKE`；MySQL 退化为 `LIKE`）。
    ILike,
    /// 大小写不敏感 LIKE 取反（PostgreSQL `NOT ILIKE`；MySQL 退化为 `NOT LIKE`）。
    NotILike,
    /// POSIX 正则匹配（PostgreSQL `~`；MySQL `REGEXP`）。
    RegexMatch,
    /// 大小写不敏感 POSIX 正则匹配（PostgreSQL `~*`；MySQL `REGEXP`，区分大小写取决于 collation）。
    RegexIMatch,
    /// POSIX 正则不匹配（PostgreSQL `!~`；MySQL `NOT REGEXP`）。
    NotRegexMatch,
    /// 大小写不敏感 POSIX 正则不匹配（PostgreSQL `!~*`；MySQL `NOT REGEXP`）。
    NotRegexIMatch,
    /// NULL-safe 不相等（PostgreSQL `IS DISTINCT FROM`；MySQL 不支持）。
    IsDistinctFrom,
    /// NULL-safe 相等（PostgreSQL `IS NOT DISTINCT FROM`；MySQL 不支持）。
    IsNotDistinctFrom,
}

/// 受控排序方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SortOrder {
    /// 升序。
    Asc,
    /// 降序。
    Desc,
}

/// 受控字段引用组成的布尔查询树。
#[derive(Debug, Clone, PartialEq)]
pub enum Predicate {
    /// 字段比较。
    Compare(FieldRef, CompareOp, serde_json::Value),
    /// IN。
    In(FieldRef, Vec<serde_json::Value>),
    /// NOT IN。
    NotIn(FieldRef, Vec<serde_json::Value>),
    /// BETWEEN。
    Between(FieldRef, serde_json::Value, serde_json::Value),
    /// IS NULL。
    IsNull(FieldRef),
    /// IS NOT NULL。
    IsNotNull(FieldRef),
    /// AND 组。
    And(Vec<Predicate>),
    /// OR 组。
    Or(Vec<Predicate>),
}

/// 受控 SELECT 聚合表达式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectExpr {
    aggregate: Aggregate,
    field: Option<FieldRef>,
    alias: Option<FieldRef>,
    cast_double: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Aggregate {
    Count,
    Sum,
    Avg,
    Min,
    Max,
}

impl SelectExpr {
    /// `COUNT(*)`。
    pub fn count_all() -> Self {
        Self {
            aggregate: Aggregate::Count,
            field: None,
            alias: None,
            cast_double: false,
        }
    }

    /// `COUNT(field)`。
    pub fn count(field: &FieldRef) -> Self {
        Self::new(Aggregate::Count, field)
    }

    /// `SUM(field)`。
    pub fn sum(field: &FieldRef) -> Self {
        Self::new(Aggregate::Sum, field)
    }

    /// `AVG(field)`。
    pub fn avg(field: &FieldRef) -> Self {
        Self::new(Aggregate::Avg, field)
    }

    /// `MIN(field)`。
    pub fn min(field: &FieldRef) -> Self {
        Self::new(Aggregate::Min, field)
    }

    /// `MAX(field)`。
    pub fn max(field: &FieldRef) -> Self {
        Self::new(Aggregate::Max, field)
    }

    fn new(aggregate: Aggregate, field: &FieldRef) -> Self {
        Self {
            aggregate,
            field: Some(field.clone()),
            alias: None,
            cast_double: false,
        }
    }

    /// 将聚合结果转换为方言对应的双精度浮点数。
    pub fn cast_double(mut self) -> Self {
        self.cast_double = true;
        self
    }

    /// 设置受控输出别名。
    pub fn alias(mut self, alias: &FieldRef) -> Self {
        self.alias = Some(alias.clone());
        self
    }

    #[cfg(feature = "mysql")]
    pub(crate) fn mysql_sql(&self) -> String {
        self.render('`', "DOUBLE")
    }

    #[cfg(feature = "postgres")]
    pub(crate) fn postgres_sql(&self) -> String {
        self.render('"', "DOUBLE PRECISION")
    }

    /// 渲染不含输出别名的 PostgreSQL 聚合表达式。
    ///
    /// 用于 `HAVING COUNT(*) > 1` 等场景：HAVING 中不允许引用 SELECT 别名，
    /// 也不能携带 `AS` 片段。
    #[cfg(feature = "postgres")]
    pub(crate) fn postgres_sql_without_alias(&self) -> String {
        self.render_expression('"', "DOUBLE PRECISION")
    }

    #[cfg(any(feature = "mysql", feature = "postgres"))]
    fn render(&self, quote: char, double_type: &str) -> String {
        let expression = self.render_expression(quote, double_type);
        self.alias.as_ref().map_or(expression.clone(), |alias| {
            format!("{expression} AS {}", quote_qualified(alias.as_str(), quote))
        })
    }

    /// 渲染聚合表达式本体（不含输出别名）。
    #[cfg(any(feature = "mysql", feature = "postgres"))]
    fn render_expression(&self, quote: char, double_type: &str) -> String {
        let function = match self.aggregate {
            Aggregate::Count => "COUNT",
            Aggregate::Sum => "SUM",
            Aggregate::Avg => "AVG",
            Aggregate::Min => "MIN",
            Aggregate::Max => "MAX",
        };
        let operand = self.field.as_ref().map_or_else(
            || "*".to_string(),
            |field| quote_qualified(field.as_str(), quote),
        );
        let aggregate = format!("{function}({operand})");
        if self.cast_double {
            format!("CAST({aggregate} AS {double_type})")
        } else {
            aggregate
        }
    }
}

/// 受控表达式操作数：已校验列引用或标量字面量。
///
/// 只允许这两种形态——列名在构造 [`FieldRef`] 时已按方言标识符规则校验，
/// 字面量一律作为绑定参数，渲染层不会把调用方字符串拼进 SQL 文本。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExprOperand {
    /// 已校验列引用，渲染时按方言加引号。
    Column(FieldRef),
    /// 整数绑定参数（如字符串函数长度）。
    Int(i64),
    /// 文本绑定参数（如格式串、填充串、比较或回退字面量）。
    Text(String),
}

impl ExprOperand {
    /// 列引用操作数。
    pub fn column(field: &FieldRef) -> Self {
        Self::Column(field.clone())
    }

    /// 整数操作数。
    pub fn int(value: i64) -> Self {
        Self::Int(value)
    }

    /// 文本操作数。
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }
}

/// 受控表达式的绑定参数值，同时作为 `COALESCE` / `NULLIF` 字面量的类型化载体。
///
/// 从 `&str` / `String` 构造时按「安全推断」决定绑定类型：`true` / `false` 按布尔；
/// 可解析为 `i64` 按整数；可解析为有限浮点/十进制按 `numeric`（保留原始十进制
/// 文本，避免经 `f64` 丢精度）；其余按文本。
///
/// 推断对数字样式的文本列字面量是陷阱：`COALESCE(text_col, "0012")` 会按整数
/// 绑定，UUID / JSONB 列上的 `"0"` 也无法安全推断——这类场景请用
/// [`SqlExprParam::text`] 强制文本（必要时由调用方配合列类型 cast 保证匹配）。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SqlExprParam {
    /// 整数绑定参数。
    Int(i64),
    /// 精确十进制（`numeric`）绑定参数，保存原始十进制文本。
    Numeric(String),
    /// 布尔绑定参数。
    Bool(bool),
    /// 文本绑定参数。
    Text(String),
}

impl SqlExprParam {
    /// 强制以文本绑定，用于 UUID / JSONB 等无法从字符串安全推断的列类型。
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    fn infer(value: &str) -> Self {
        match value {
            "true" => Self::Bool(true),
            "false" => Self::Bool(false),
            _ => {
                if let Ok(int) = value.parse::<i64>() {
                    Self::Int(int)
                } else if value.parse::<f64>().is_ok_and(f64::is_finite) {
                    // 保留原始文本（如 "0.00"），numeric 绑定不走 f64 转换。
                    Self::Numeric(value.to_string())
                } else {
                    Self::Text(value.to_string())
                }
            }
        }
    }
}

impl From<&str> for SqlExprParam {
    fn from(value: &str) -> Self {
        Self::infer(value)
    }
}

impl From<String> for SqlExprParam {
    fn from(value: String) -> Self {
        Self::infer(&value)
    }
}

impl From<&String> for SqlExprParam {
    fn from(value: &String) -> Self {
        Self::infer(value)
    }
}

impl From<i64> for SqlExprParam {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<f64> for SqlExprParam {
    fn from(value: f64) -> Self {
        if value.is_finite() {
            Self::Numeric(value.to_string())
        } else {
            // NaN / Infinity 不是合法 numeric 文本，退回文本绑定。
            Self::Text(value.to_string())
        }
    }
}

impl From<bool> for SqlExprParam {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

/// 受控表达式的渲染结果：固定 SQL 片段（自带占位符）与按占位符顺序排列的参数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenderedSqlExpr {
    pub(crate) sql: String,
    pub(crate) params: Vec<SqlExprParam>,
}

#[cfg(feature = "mysql")]
impl From<SqlExprParam> for crate::mysql::SqlValue {
    fn from(param: SqlExprParam) -> Self {
        match param {
            SqlExprParam::Int(value) => Self::Int(value),
            SqlExprParam::Numeric(value) => match value.parse::<f64>() {
                Ok(number) => Self::Float(number),
                Err(_) => Self::String(value),
            },
            SqlExprParam::Bool(value) => Self::Bool(value),
            SqlExprParam::Text(value) => Self::String(value),
        }
    }
}

#[cfg(feature = "postgres")]
impl From<SqlExprParam> for crate::postgres::SqlValue {
    fn from(param: SqlExprParam) -> Self {
        match param {
            SqlExprParam::Int(value) => Self::Int(value),
            // 精确十进制按 BigDecimal 绑定为 NUMERIC，避免经 f64 丢精度。
            SqlExprParam::Numeric(value) => match value.parse::<bigdecimal::BigDecimal>() {
                Ok(number) => Self::Decimal(number),
                Err(_) => Self::String(value),
            },
            SqlExprParam::Bool(value) => Self::Bool(value),
            SqlExprParam::Text(value) => Self::String(value),
        }
    }
}

/// 受控服务端 SQL 标量表达式。
///
/// 只能由白名单构造函数创建：渲染进 SQL 文本的是固定函数名与已校验列引用，
/// 动态部分（偏移秒数、格式串、填充串、字面量）一律走绑定参数，调用方无法
/// 注入任意 SQL。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SqlExpr {
    /// 服务端当前 Unix 时间戳（秒）。
    UnixTimestamp,
    /// 服务端当前 Unix 时间戳加偏移秒数；偏移量以绑定参数传递。
    UnixTimestampAdd(i64),
    /// 服务端当前时间：PostgreSQL `now()`，MySQL `CURRENT_TIMESTAMP`。
    Now,
    /// `COALESCE(列, 字面量)`；字面量按 [`SqlExprParam`] 的类型绑定。
    Coalesce {
        field: FieldRef,
        fallback: SqlExprParam,
    },
    /// `GREATEST(操作数...)`，至少两个操作数。
    ///
    /// NULL 语义与方言相关：PostgreSQL 忽略 NULL 操作数（全 NULL 才返回 NULL），
    /// MySQL 只要有一个 NULL 操作数就返回 NULL。
    Greatest(Vec<ExprOperand>),
    /// `LEAST(操作数...)`，至少两个操作数。
    ///
    /// NULL 语义与方言相关：PostgreSQL 忽略 NULL 操作数（全 NULL 才返回 NULL），
    /// MySQL 只要有一个 NULL 操作数就返回 NULL。
    Least(Vec<ExprOperand>),
    /// `NULLIF(列, 字面量)`；字面量按 [`SqlExprParam`] 的类型绑定。
    NullIf {
        field: FieldRef,
        value: SqlExprParam,
    },
    /// `strpos(列, 字面量)`（MySQL 渲染为等价的 `LOCATE(字面量, 列)`）。
    Strpos { field: FieldRef, needle: String },
    /// `lpad(列, 长度, 填充串)`。
    Lpad {
        field: FieldRef,
        length: i64,
        fill: String,
    },
    /// `rpad(列, 长度, 填充串)`。
    Rpad {
        field: FieldRef,
        length: i64,
        fill: String,
    },
    /// `to_char(列, 格式串)`（仅 PostgreSQL；MySQL 渲染返回
    /// [`DbError::UnsupportedOperator`](crate::DbError::UnsupportedOperator)，
    /// 因为 PG 格式串与 MySQL `DATE_FORMAT` 记号不等价）。
    ToChar { field: FieldRef, format: String },
}

impl SqlExpr {
    /// 服务端当前 Unix 时间戳（秒）：MySQL `UNIX_TIMESTAMP()`，
    /// PostgreSQL `EXTRACT(EPOCH FROM now())`。
    pub fn unix_timestamp() -> Self {
        Self::UnixTimestamp
    }

    /// 服务端当前 Unix 时间戳加 `seconds` 秒；`seconds` 以绑定参数传递，可正可负。
    pub fn unix_timestamp_add(seconds: i64) -> Self {
        Self::UnixTimestampAdd(seconds)
    }

    /// 服务端当前时间。
    pub fn now() -> Self {
        Self::Now
    }

    /// `COALESCE(field, fallback)`；`fallback` 以绑定参数传递，其绑定类型由
    /// [`SqlExprParam`] 的 `From` 推断（`&str` / i64 / f64 / bool），可用
    /// [`SqlExprParam::text`] 强制文本。
    pub fn coalesce(field: &FieldRef, fallback: impl Into<SqlExprParam>) -> Self {
        Self::Coalesce {
            field: field.clone(),
            fallback: fallback.into(),
        }
    }

    /// `GREATEST(operands...)`；调用方必须提供至少两个操作数。
    ///
    /// NULL 语义与方言相关：PostgreSQL 忽略 NULL，MySQL 任一 NULL 即返回 NULL。
    pub fn greatest(operands: Vec<ExprOperand>) -> Self {
        debug_assert!(operands.len() >= 2, "GREATEST 至少需要两个操作数");
        Self::Greatest(operands)
    }

    /// `LEAST(operands...)`；调用方必须提供至少两个操作数。
    ///
    /// NULL 语义与方言相关：PostgreSQL 忽略 NULL，MySQL 任一 NULL 即返回 NULL。
    pub fn least(operands: Vec<ExprOperand>) -> Self {
        debug_assert!(operands.len() >= 2, "LEAST 至少需要两个操作数");
        Self::Least(operands)
    }

    /// `NULLIF(field, value)`；`value` 以绑定参数传递，绑定类型规则同
    /// [`SqlExpr::coalesce`]。
    pub fn nullif(field: &FieldRef, value: impl Into<SqlExprParam>) -> Self {
        Self::NullIf {
            field: field.clone(),
            value: value.into(),
        }
    }

    /// `strpos(field, needle)`；`needle` 以绑定参数传递。
    pub fn strpos(field: &FieldRef, needle: impl Into<String>) -> Self {
        Self::Strpos {
            field: field.clone(),
            needle: needle.into(),
        }
    }

    /// `lpad(field, length, fill)`；长度与填充串以绑定参数传递。
    pub fn lpad(field: &FieldRef, length: i64, fill: impl Into<String>) -> Self {
        Self::Lpad {
            field: field.clone(),
            length,
            fill: fill.into(),
        }
    }

    /// `rpad(field, length, fill)`；长度与填充串以绑定参数传递。
    pub fn rpad(field: &FieldRef, length: i64, fill: impl Into<String>) -> Self {
        Self::Rpad {
            field: field.clone(),
            length,
            fill: fill.into(),
        }
    }

    /// `to_char(field, format)`；仅 PostgreSQL 支持。PostgreSQL 的格式记号
    /// （`YYYY-MM-DD`）与 MySQL `DATE_FORMAT`（`%Y-%m-%d`）不等价，因此 MySQL
    /// 渲染返回 [`DbError::UnsupportedOperator`](crate::DbError::UnsupportedOperator)。
    pub fn to_char(field: &FieldRef, format: impl Into<String>) -> Self {
        Self::ToChar {
            field: field.clone(),
            format: format.into(),
        }
    }

    /// 渲染 MySQL 片段。
    ///
    /// `strpos` 映射为 `LOCATE`；`GREATEST` / `LEAST` 的 NULL 语义与 PostgreSQL
    /// 不同（MySQL 任一 NULL 即返回 NULL）。`to_char` 的 PG 格式串在 MySQL 没有
    /// 等价记号，返回 [`DbError::UnsupportedOperator`](crate::DbError::UnsupportedOperator)。
    #[cfg(feature = "mysql")]
    pub(crate) fn mysql_render(&self) -> Result<RenderedSqlExpr, DbError> {
        let mut params = Vec::new();
        let sql = match self {
            Self::UnixTimestamp => "UNIX_TIMESTAMP()".to_string(),
            Self::UnixTimestampAdd(seconds) => {
                params.push(SqlExprParam::Int(*seconds));
                "UNIX_TIMESTAMP() + ?".to_string()
            }
            Self::Now => "CURRENT_TIMESTAMP".to_string(),
            Self::Coalesce { field, fallback } => {
                params.push(fallback.clone());
                format!("COALESCE({}, ?)", field.mysql_quoted())
            }
            Self::Greatest(operands) => mysql_function("GREATEST", operands, &mut params),
            Self::Least(operands) => mysql_function("LEAST", operands, &mut params),
            Self::NullIf { field, value } => {
                params.push(value.clone());
                format!("NULLIF({}, ?)", field.mysql_quoted())
            }
            Self::Strpos { field, needle } => {
                params.push(SqlExprParam::Text(needle.clone()));
                format!("LOCATE(?, {})", field.mysql_quoted())
            }
            Self::Lpad {
                field,
                length,
                fill,
            } => {
                params.push(SqlExprParam::Int(*length));
                params.push(SqlExprParam::Text(fill.clone()));
                format!("LPAD({}, ?, ?)", field.mysql_quoted())
            }
            Self::Rpad {
                field,
                length,
                fill,
            } => {
                params.push(SqlExprParam::Int(*length));
                params.push(SqlExprParam::Text(fill.clone()));
                format!("RPAD({}, ?, ?)", field.mysql_quoted())
            }
            Self::ToChar { .. } => {
                return Err(DbError::UnsupportedOperator(
                    "to_char 仅支持 PostgreSQL：PG 格式串与 MySQL DATE_FORMAT 记号不等价"
                        .to_string(),
                ));
            }
        };
        Ok(RenderedSqlExpr { sql, params })
    }

    /// 渲染 PostgreSQL 片段；`placeholder` 为第一个绑定参数的 `$n` 序号。
    #[cfg(feature = "postgres")]
    pub(crate) fn postgres_render(&self, placeholder: usize) -> RenderedSqlExpr {
        let mut params = Vec::new();
        let sql = match self {
            Self::UnixTimestamp => "EXTRACT(EPOCH FROM now())".to_string(),
            Self::UnixTimestampAdd(seconds) => {
                params.push(SqlExprParam::Int(*seconds));
                format!("EXTRACT(EPOCH FROM now()) + ${placeholder}")
            }
            Self::Now => "now()".to_string(),
            Self::Coalesce { field, fallback } => {
                params.push(fallback.clone());
                format!("COALESCE({}, ${placeholder})", field.postgres_quoted())
            }
            Self::Greatest(operands) => {
                postgres_function("GREATEST", operands, placeholder, &mut params)
            }
            Self::Least(operands) => postgres_function("LEAST", operands, placeholder, &mut params),
            Self::NullIf { field, value } => {
                params.push(value.clone());
                format!("NULLIF({}, ${placeholder})", field.postgres_quoted())
            }
            Self::Strpos { field, needle } => {
                params.push(SqlExprParam::Text(needle.clone()));
                format!("strpos({}, ${placeholder})", field.postgres_quoted())
            }
            Self::Lpad {
                field,
                length,
                fill,
            } => {
                params.push(SqlExprParam::Int(*length));
                params.push(SqlExprParam::Text(fill.clone()));
                format!(
                    "lpad({}, ${}::integer, ${})",
                    field.postgres_quoted(),
                    placeholder,
                    placeholder + 1
                )
            }
            Self::Rpad {
                field,
                length,
                fill,
            } => {
                params.push(SqlExprParam::Int(*length));
                params.push(SqlExprParam::Text(fill.clone()));
                format!(
                    "rpad({}, ${}::integer, ${})",
                    field.postgres_quoted(),
                    placeholder,
                    placeholder + 1
                )
            }
            Self::ToChar { field, format } => {
                params.push(SqlExprParam::Text(format.clone()));
                format!("to_char({}, ${placeholder})", field.postgres_quoted())
            }
        };
        RenderedSqlExpr { sql, params }
    }
}

#[cfg(feature = "mysql")]
fn mysql_function(name: &str, operands: &[ExprOperand], params: &mut Vec<SqlExprParam>) -> String {
    let rendered = operands
        .iter()
        .map(|operand| match operand {
            ExprOperand::Column(field) => field.mysql_quoted().to_string(),
            ExprOperand::Int(value) => {
                params.push(SqlExprParam::Int(*value));
                "?".to_string()
            }
            ExprOperand::Text(value) => {
                params.push(SqlExprParam::Text(value.clone()));
                "?".to_string()
            }
        })
        .collect::<Vec<_>>();
    format!("{name}({})", rendered.join(", "))
}

#[cfg(feature = "postgres")]
fn postgres_function(
    name: &str,
    operands: &[ExprOperand],
    placeholder: usize,
    params: &mut Vec<SqlExprParam>,
) -> String {
    let rendered = operands
        .iter()
        .map(|operand| match operand {
            ExprOperand::Column(field) => field.postgres_quoted().to_string(),
            ExprOperand::Int(value) => {
                let index = placeholder + params.len();
                params.push(SqlExprParam::Int(*value));
                format!("${index}")
            }
            ExprOperand::Text(value) => {
                let index = placeholder + params.len();
                params.push(SqlExprParam::Text(value.clone()));
                format!("${index}")
            }
        })
        .collect::<Vec<_>>();
    format!("{name}({})", rendered.join(", "))
}

#[cfg(any(feature = "mysql", feature = "postgres"))]
fn quote_qualified(value: &str, quote: char) -> String {
    value
        .split('.')
        .map(|segment| format!("{quote}{segment}{quote}"))
        .collect::<Vec<_>>()
        .join(".")
}

#[cfg(any(feature = "mysql", feature = "postgres"))]
impl SortOrder {
    pub(crate) const fn is_ascending(self) -> bool {
        matches!(self, Self::Asc)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn controlled_references_validate_once_and_render_both_dialects() {
        let table = TableRef::new("org_user").expect("合法表名应创建成功");
        let field = FieldRef::new("org_user.username").expect("合法限定字段应创建成功");

        assert_eq!(table.as_str(), "org_user");
        assert_eq!(field.as_str(), "org_user.username");
        #[cfg(feature = "mysql")]
        assert_eq!(field.mysql_quoted(), "`org_user`.`username`");
        #[cfg(feature = "postgres")]
        assert_eq!(field.postgres_quoted(), "\"org_user\".\"username\"");
    }

    #[test]
    fn controlled_references_reject_expressions_and_injection_payloads() {
        for invalid in ["", "users;DROP", "COUNT(*)", "a.b.c.d", "a..b"] {
            assert!(TableRef::new(invalid).is_err(), "表引用应拒绝 {invalid:?}");
            assert!(
                FieldRef::new(invalid).is_err(),
                "字段引用应拒绝 {invalid:?}"
            );
        }
        // 未启用 PostgreSQL 规则时，中文标识符仍按 MySQL 严格 ASCII 规则拒绝。
        #[cfg(not(feature = "postgres"))]
        for invalid in ["用户", "a$b", "a\"b"] {
            assert!(TableRef::new(invalid).is_err(), "表引用应拒绝 {invalid:?}");
            assert!(
                FieldRef::new(invalid).is_err(),
                "字段引用应拒绝 {invalid:?}"
            );
        }
        // 三段字段合法，但表名最多两段。
        assert!(FieldRef::new("users.id.extra").is_ok());
        assert!(TableRef::new("users.id.extra").is_err());
    }

    /// B42/B43：PostgreSQL 规则下 TableRef/FieldRef 接受 Unicode、`$` 与内嵌双引号，
    /// 并统一执行每段 63 字节上限。
    #[cfg(feature = "postgres")]
    #[test]
    #[allow(clippy::expect_used)]
    fn postgres_references_accept_extended_identifiers_and_cap_63_bytes() {
        let table = TableRef::new("订单").expect("中文表名应合法");
        assert_eq!(table.as_str(), "订单");

        let field = FieldRef::new("金额").expect("中文字段名应合法");
        assert_eq!(field.as_str(), "金额");
        assert_eq!(field.postgres_quoted(), "\"金额\"");

        let qualified = FieldRef::new("订单.金额").expect("中文限定字段名应合法");
        assert_eq!(qualified.postgres_quoted(), "\"订单\".\"金额\"");

        let dollar = FieldRef::new("a$b").expect("非首字符 `$` 应合法");
        assert_eq!(dollar.postgres_quoted(), "\"a$b\"");

        // 内嵌双引号按 SQL 标准双写。
        let quoted = FieldRef::new("a\"b").expect("内嵌双引号应合法");
        assert_eq!(quoted.postgres_quoted(), "\"a\"\"b\"");

        // 63 字节可接受，64 字节明确拒绝（表名与字段名一致）。中文 21 字 = 63 字节。
        assert!(TableRef::new("a".repeat(63)).is_ok());
        assert!(FieldRef::new("a".repeat(63)).is_ok());
        assert!(TableRef::new("用".repeat(21)).is_ok());
        for oversized in ["a".repeat(64), "用".repeat(22)] {
            let error = TableRef::new(oversized.clone()).expect_err("超长表名必须拒绝");
            assert!(matches!(error, DbError::InvalidArgument(_)), "{error:?}");
            assert!(FieldRef::new(oversized).is_err(), "超长字段名必须拒绝");
        }
    }

    #[test]
    fn qualified_schema_references_render_every_segment() {
        let table = TableRef::new("audit.users").expect("schema.table 应合法");
        let field = FieldRef::new("audit.users.id").expect("schema.table.column 应合法");
        assert_eq!(table.as_str(), "audit.users");
        assert_eq!(field.as_str(), "audit.users.id");
        #[cfg(feature = "mysql")]
        assert_eq!(field.mysql_quoted(), "`audit`.`users`.`id`");
        #[cfg(feature = "postgres")]
        assert_eq!(field.postgres_quoted(), "\"audit\".\"users\".\"id\"");
    }

    #[test]
    fn server_expressions_render_fixed_fragments_with_bound_dynamic_parts() {
        #[cfg(feature = "mysql")]
        {
            let rendered = SqlExpr::unix_timestamp()
                .mysql_render()
                .expect("MySQL 表达式渲染应成功");
            assert_eq!(rendered.sql, "UNIX_TIMESTAMP()");
            assert!(rendered.params.is_empty());

            let rendered = SqlExpr::unix_timestamp_add(900)
                .mysql_render()
                .expect("MySQL 表达式渲染应成功");
            assert_eq!(rendered.sql, "UNIX_TIMESTAMP() + ?");
            assert_eq!(rendered.params, vec![SqlExprParam::Int(900)]);

            // 负偏移同样走绑定参数，片段保持固定。
            let rendered = SqlExpr::unix_timestamp_add(-30)
                .mysql_render()
                .expect("MySQL 表达式渲染应成功");
            assert_eq!(rendered.sql, "UNIX_TIMESTAMP() + ?");
            assert_eq!(rendered.params, vec![SqlExprParam::Int(-30)]);
        }
        #[cfg(feature = "postgres")]
        {
            let rendered = SqlExpr::unix_timestamp().postgres_render(1);
            assert_eq!(rendered.sql, "EXTRACT(EPOCH FROM now())");
            assert!(rendered.params.is_empty());

            let rendered = SqlExpr::unix_timestamp_add(900).postgres_render(3);
            assert_eq!(rendered.sql, "EXTRACT(EPOCH FROM now()) + $3");
            assert_eq!(rendered.params, vec![SqlExprParam::Int(900)]);

            let rendered = SqlExpr::now().postgres_render(1);
            assert_eq!(rendered.sql, "now()");
            assert!(rendered.params.is_empty());
        }
    }

    #[test]
    fn string_expressions_render_dialect_specific_sql_with_bound_operands() {
        let field = FieldRef::new("t.c").expect("限定字段名应合法");

        #[cfg(feature = "mysql")]
        {
            let rendered = SqlExpr::coalesce(&field, "x")
                .mysql_render()
                .expect("MySQL 表达式渲染应成功");
            assert_eq!(rendered.sql, "COALESCE(`t`.`c`, ?)");
            assert_eq!(rendered.params, vec![SqlExprParam::Text("x".into())]);

            let operands = vec![
                ExprOperand::column(&field),
                ExprOperand::int(7),
                ExprOperand::text("z"),
            ];
            let rendered = SqlExpr::greatest(operands)
                .mysql_render()
                .expect("MySQL 表达式渲染应成功");
            assert_eq!(rendered.sql, "GREATEST(`t`.`c`, ?, ?)");
            assert_eq!(
                rendered.params,
                vec![SqlExprParam::Int(7), SqlExprParam::Text("z".into())]
            );

            let operands = vec![ExprOperand::column(&field), ExprOperand::int(1)];
            let rendered = SqlExpr::least(operands)
                .mysql_render()
                .expect("MySQL 表达式渲染应成功");
            assert_eq!(rendered.sql, "LEAST(`t`.`c`, ?)");
            assert_eq!(rendered.params, vec![SqlExprParam::Int(1)]);

            let rendered = SqlExpr::nullif(&field, "")
                .mysql_render()
                .expect("MySQL 表达式渲染应成功");
            assert_eq!(rendered.sql, "NULLIF(`t`.`c`, ?)");
            assert_eq!(rendered.params, vec![SqlExprParam::Text(String::new())]);

            let rendered = SqlExpr::strpos(&field, "needle")
                .mysql_render()
                .expect("MySQL 表达式渲染应成功");
            assert_eq!(rendered.sql, "LOCATE(?, `t`.`c`)");
            assert_eq!(rendered.params, vec![SqlExprParam::Text("needle".into())]);

            let rendered = SqlExpr::lpad(&field, 5, "*")
                .mysql_render()
                .expect("MySQL 表达式渲染应成功");
            assert_eq!(rendered.sql, "LPAD(`t`.`c`, ?, ?)");
            assert_eq!(
                rendered.params,
                vec![SqlExprParam::Int(5), SqlExprParam::Text("*".into())]
            );

            let rendered = SqlExpr::rpad(&field, 6, "-")
                .mysql_render()
                .expect("MySQL 表达式渲染应成功");
            assert_eq!(rendered.sql, "RPAD(`t`.`c`, ?, ?)");
            assert_eq!(
                rendered.params,
                vec![SqlExprParam::Int(6), SqlExprParam::Text("-".into())]
            );

            // PG 格式串在 MySQL 没有等价记号：明确返回 UnsupportedOperator。
            let error = SqlExpr::to_char(&field, "%Y-%m")
                .mysql_render()
                .expect_err("MySQL 不支持 to_char");
            assert!(
                matches!(error, DbError::UnsupportedOperator(_)),
                "{error:?}"
            );
        }

        #[cfg(feature = "postgres")]
        {
            let rendered = SqlExpr::coalesce(&field, "x").postgres_render(1);
            assert_eq!(rendered.sql, "COALESCE(\"t\".\"c\", $1)");
            assert_eq!(rendered.params, vec![SqlExprParam::Text("x".into())]);

            let operands = vec![
                ExprOperand::column(&field),
                ExprOperand::int(7),
                ExprOperand::text("z"),
            ];
            let rendered = SqlExpr::greatest(operands).postgres_render(1);
            assert_eq!(rendered.sql, "GREATEST(\"t\".\"c\", $1, $2)");
            assert_eq!(
                rendered.params,
                vec![SqlExprParam::Int(7), SqlExprParam::Text("z".into())]
            );

            let operands = vec![ExprOperand::column(&field), ExprOperand::int(1)];
            let rendered = SqlExpr::least(operands).postgres_render(2);
            assert_eq!(rendered.sql, "LEAST(\"t\".\"c\", $2)");
            assert_eq!(rendered.params, vec![SqlExprParam::Int(1)]);

            let rendered = SqlExpr::nullif(&field, "").postgres_render(1);
            assert_eq!(rendered.sql, "NULLIF(\"t\".\"c\", $1)");
            assert_eq!(rendered.params, vec![SqlExprParam::Text(String::new())]);

            let rendered = SqlExpr::strpos(&field, "needle").postgres_render(1);
            assert_eq!(rendered.sql, "strpos(\"t\".\"c\", $1)");
            assert_eq!(rendered.params, vec![SqlExprParam::Text("needle".into())]);

            let rendered = SqlExpr::lpad(&field, 5, "*").postgres_render(1);
            assert_eq!(rendered.sql, "lpad(\"t\".\"c\", $1::integer, $2)");
            assert_eq!(
                rendered.params,
                vec![SqlExprParam::Int(5), SqlExprParam::Text("*".into())]
            );

            let rendered = SqlExpr::rpad(&field, 6, "-").postgres_render(1);
            assert_eq!(rendered.sql, "rpad(\"t\".\"c\", $1::integer, $2)");
            assert_eq!(
                rendered.params,
                vec![SqlExprParam::Int(6), SqlExprParam::Text("-".into())]
            );

            let rendered = SqlExpr::to_char(&field, "YYYY-MM").postgres_render(4);
            assert_eq!(rendered.sql, "to_char(\"t\".\"c\", $4)");
            assert_eq!(rendered.params, vec![SqlExprParam::Text("YYYY-MM".into())]);
        }
    }

    /// COALESCE / NULLIF 字面量按 `SqlExprParam` 推断类型绑定，而不是固定 TEXT。
    #[test]
    fn expression_literals_infer_binding_types() {
        let field = FieldRef::new("t.c").expect("限定字段名应合法");

        assert_eq!(SqlExprParam::from("0"), SqlExprParam::Int(0));
        assert_eq!(SqlExprParam::from("-7"), SqlExprParam::Int(-7));
        assert_eq!(
            SqlExprParam::from("0.00"),
            SqlExprParam::Numeric("0.00".into())
        );
        assert_eq!(SqlExprParam::from("true"), SqlExprParam::Bool(true));
        assert_eq!(SqlExprParam::from("false"), SqlExprParam::Bool(false));
        assert_eq!(SqlExprParam::from("x"), SqlExprParam::Text("x".into()));
        assert_eq!(
            SqlExprParam::text("0"),
            SqlExprParam::Text("0".into()),
            "text() 必须绕过数值推断"
        );

        #[cfg(feature = "postgres")]
        {
            let rendered = SqlExpr::coalesce(&field, "0").postgres_render(1);
            assert_eq!(rendered.sql, "COALESCE(\"t\".\"c\", $1)");
            assert_eq!(rendered.params, vec![SqlExprParam::Int(0)]);

            let rendered = SqlExpr::nullif(&field, "7").postgres_render(2);
            assert_eq!(rendered.sql, "NULLIF(\"t\".\"c\", $2)");
            assert_eq!(rendered.params, vec![SqlExprParam::Int(7)]);

            let rendered = SqlExpr::coalesce(&field, "0.00").postgres_render(1);
            assert_eq!(rendered.params, vec![SqlExprParam::Numeric("0.00".into())]);

            let rendered = SqlExpr::coalesce(&field, SqlExprParam::text("0")).postgres_render(1);
            assert_eq!(rendered.params, vec![SqlExprParam::Text("0".into())]);
        }
    }

    #[cfg(feature = "mysql")]
    #[tokio::test]
    async fn mysql_query_builder_consumes_controlled_references_without_value_interpolation() {
        let pool = sqlx::mysql::MySqlPoolOptions::new()
            .connect_lazy("mysql://root:test@127.0.0.1:3306/test")
            .expect("lazy MySQL pool 应创建成功");
        let database = crate::Database::from_pool(pool, crate::DatabaseConfig::default())
            .expect("默认 MySQL 配置应有效");
        let table = TableRef::new("org_user").expect("固定表名有效");
        let username = FieldRef::new("username").expect("固定字段名有效");

        let sql = database
            .table(&table)
            .field(&username)
            .where_and(&username, CompareOp::Eq, "alice")
            .expect("固定受控条件应合法")
            .order(&username, SortOrder::Asc)
            .try_to_sql()
            .expect("受控 MySQL 查询应生成 SQL");

        assert!(sql.contains("`org_user`"));
        assert!(sql.contains("`username`"));
        assert!(sql.contains('?'));
        assert!(!sql.contains("alice"));
    }

    #[cfg(feature = "postgres")]
    #[tokio::test]
    async fn postgres_query_builder_consumes_controlled_references_without_value_interpolation() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://postgres:test@127.0.0.1:5432/test")
            .expect("lazy PostgreSQL pool 应创建成功");
        let database = crate::PgDatabase::from_pool(pool, crate::PgDatabaseConfig::default())
            .expect("默认 PostgreSQL 配置应有效");
        let table = TableRef::new("org_user").expect("固定表名有效");
        let username = FieldRef::new("username").expect("固定字段名有效");

        let sql = database
            .table(&table)
            .field(&username)
            .where_and(&username, CompareOp::Eq, "alice")
            .expect("固定受控条件应合法")
            .order(&username, SortOrder::Desc)
            .try_to_sql()
            .expect("受控 PostgreSQL 查询应生成 SQL");

        assert!(sql.contains("\"org_user\""));
        assert!(sql.contains("\"username\""));
        assert!(sql.contains("$1"));
        assert!(!sql.contains("alice"));
    }
}
