//! 查询热路径使用的受控 SQL 引用与封闭操作符。

use crate::sql_types::{Identifier, QualifiedIdentifier};
use crate::DbError;
use std::fmt;

/// 构建期校验完成的数据库表引用。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TableRef {
    raw: String,
}

impl TableRef {
    /// 校验并创建表引用。
    pub fn new(value: impl Into<String>) -> Result<Self, DbError> {
        let raw = value.into();
        Identifier::parse(&raw)?;
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

/// 构建期校验完成的列或 `table.column` 引用。
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
    pub fn new(value: impl Into<String>) -> Result<Self, DbError> {
        let raw = value.into();
        let identifier = QualifiedIdentifier::parse(&raw)?;
        let _ = &identifier;
        Ok(Self {
            #[cfg(feature = "mysql")]
            mysql: identifier.render('`'),
            #[cfg(feature = "postgres")]
            postgres: identifier.render('"'),
            raw,
        })
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
    validate_literal(value, false);
}

#[doc(hidden)]
pub const fn __validate_field_literal(value: &str) {
    validate_literal(value, true);
}

const fn validate_literal(value: &str, qualified: bool) {
    let bytes = value.as_bytes();
    assert!(!bytes.is_empty(), "SQL identifier cannot be empty");
    let mut index = 0;
    let mut segment_start = true;
    let mut dots = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'.' {
            assert!(qualified, "table identifier cannot be qualified");
            assert!(!segment_start, "identifier segment cannot be empty");
            dots += 1;
            assert!(dots <= 1, "field identifier accepts at most table.field");
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

    #[cfg(any(feature = "mysql", feature = "postgres"))]
    fn render(&self, quote: char, double_type: &str) -> String {
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
        let expression = if self.cast_double {
            format!("CAST({aggregate} AS {double_type})")
        } else {
            aggregate
        };
        self.alias.as_ref().map_or(expression.clone(), |alias| {
            format!("{expression} AS {}", quote_qualified(alias.as_str(), quote))
        })
    }
}

/// 受控服务端 SQL 标量表达式。
///
/// 只能由白名单构造函数创建：渲染进 SQL 文本的是固定片段，动态部分一律走
/// 绑定参数，调用方无法注入任意 SQL。目前覆盖服务端 Unix 时间戳场景
/// （MySQL `UNIX_TIMESTAMP()`），后续新增函数按同一模式扩展枚举变体即可。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SqlExpr {
    /// 服务端当前 Unix 时间戳（秒）。
    UnixTimestamp,
    /// 服务端当前 Unix 时间戳加偏移秒数；偏移量以绑定参数传递。
    UnixTimestampAdd(i64),
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

    /// 渲染 MySQL 片段，返回（固定 SQL 片段，可选的 i64 绑定参数）。
    #[cfg(feature = "mysql")]
    pub(crate) fn mysql_render(&self) -> (&'static str, Option<i64>) {
        match self {
            Self::UnixTimestamp => ("UNIX_TIMESTAMP()", None),
            Self::UnixTimestampAdd(seconds) => ("UNIX_TIMESTAMP() + ?", Some(*seconds)),
        }
    }

    /// 渲染 PostgreSQL 片段；`placeholder` 为携带参数时 `$n` 占位符的序号。
    ///
    /// PostgreSQL 构造器尚未接入表达式入口，此渲染当前仅由单元测试覆盖，
    /// 待 PG 侧需要时直接复用。
    #[cfg(feature = "postgres")]
    #[allow(dead_code)]
    pub(crate) fn postgres_render(&self, placeholder: usize) -> (String, Option<i64>) {
        match self {
            Self::UnixTimestamp => ("EXTRACT(EPOCH FROM now())".to_string(), None),
            Self::UnixTimestampAdd(seconds) => (
                format!("EXTRACT(EPOCH FROM now()) + ${placeholder}"),
                Some(*seconds),
            ),
        }
    }
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
        for invalid in ["", "users;DROP", "COUNT(*)", "users.id.extra", "用户"] {
            assert!(TableRef::new(invalid).is_err());
            assert!(FieldRef::new(invalid).is_err());
        }
    }

    #[test]
    fn server_expressions_render_fixed_fragments_with_bound_dynamic_parts() {
        #[cfg(feature = "mysql")]
        {
            assert_eq!(
                SqlExpr::unix_timestamp().mysql_render(),
                ("UNIX_TIMESTAMP()", None)
            );
            assert_eq!(
                SqlExpr::unix_timestamp_add(900).mysql_render(),
                ("UNIX_TIMESTAMP() + ?", Some(900))
            );
            // 负偏移同样走绑定参数，片段保持固定。
            assert_eq!(
                SqlExpr::unix_timestamp_add(-30).mysql_render(),
                ("UNIX_TIMESTAMP() + ?", Some(-30))
            );
        }
        #[cfg(feature = "postgres")]
        {
            assert_eq!(
                SqlExpr::unix_timestamp().postgres_render(1),
                ("EXTRACT(EPOCH FROM now())".to_string(), None)
            );
            assert_eq!(
                SqlExpr::unix_timestamp_add(900).postgres_render(3),
                ("EXTRACT(EPOCH FROM now()) + $3".to_string(), Some(900))
            );
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
            .order(&username, SortOrder::Desc)
            .try_to_sql()
            .expect("受控 PostgreSQL 查询应生成 SQL");

        assert!(sql.contains("\"org_user\""));
        assert!(sql.contains("\"username\""));
        assert!(sql.contains("$1"));
        assert!(!sql.contains("alice"));
    }
}
