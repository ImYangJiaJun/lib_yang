/// 字段类型标记（PostgreSQL）
///
/// 与 MySQL 后端保持一致的字段类型枚举。`Blob` 在 PostgreSQL 中对应 `BYTEA`，
/// `Json` 对应 `JSON`/`JSONB`，其余沿用通用语义。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum FieldType {
    /// 标准类型（无需特殊处理）
    Standard,
    /// BOOLEAN 类型（空数组/全 NULL 数组需要它绑定为 `bool[]`）
    Bool,
    /// JSON / JSONB 类型
    Json,
    /// JSONB 类型（插入时与 JSON 同样按原生 serde_json 值绑定）
    Jsonb,
    /// TIMESTAMP（无时区）类型
    DateTime,
    /// TIMESTAMPTZ（带时区）类型
    TimestampTz,
    /// DATE 类型
    Date,
    /// TIME 类型
    Time,
    /// UUID 类型
    Uuid,
    /// 以整数存储的时间戳
    Timestamp,
    /// NUMERIC / DECIMAL 类型
    Decimal,
    /// BYTEA 类型
    Blob,
    /// TEXT 类型
    Text,
    /// 数组类型（元素类型递归描述，如 `TEXT[]` → `Array(Box::new(Text))`）
    Array(Box<FieldType>),
    /// INET：IPv4/IPv6 主机地址或网络地址
    Inet,
    /// CIDR：IPv4/IPv6 网络段
    Cidr,
    /// MACADDR：6 字节 MAC 地址
    MacAddr,
    /// MACADDR8：8 字节 MAC 地址（PostgreSQL 10+）
    MacAddr8,
    /// BIT：定长位串
    Bit,
    /// VARBIT：变长位串
    Varbit,
    /// 范围类型（`RANGE`）：元素类型递归描述，如 `int4range` → `Range(Standard)`、
    /// `tsrange` → `Range(DateTime)`。
    ///
    /// 无整数宽度提示的 [`FieldType::Standard`] 视为 PostgreSQL 默认的 32 位整数
    /// 范围 `int4range`；其余可映射元素类型见 [`FieldType::postgres_type_name`]。
    Range(Box<FieldType>),
    /// 多范围类型（`MULTIRANGE`，PostgreSQL 14+）：元素类型递归描述，
    /// 如 `int4multirange` → `Multirange(Standard)`。
    Multirange(Box<FieldType>),
}

impl FieldType {
    /// 返回该字段类型对应的 PostgreSQL 类型名（`format_type` 风格，如
    /// `integer`、`text[]`），用于 DDL 生成与类型核对。
    ///
    /// [`FieldType::Standard`] 表示没有额外类型提示，返回 `None`；数组按
    /// 递归元素类型追加 `[]`（元素类型未知时同样返回 `None`）。
    pub fn postgres_type_name(&self) -> Option<String> {
        Some(match self {
            Self::Standard => return None,
            Self::Bool => "boolean".to_string(),
            Self::Array(inner) => format!("{}[]", inner.postgres_type_name()?),
            Self::Json => "json".to_string(),
            Self::Jsonb => "jsonb".to_string(),
            Self::DateTime => "timestamp".to_string(),
            Self::TimestampTz => "timestamptz".to_string(),
            Self::Date => "date".to_string(),
            Self::Time => "time".to_string(),
            Self::Uuid => "uuid".to_string(),
            // 以整数存储的 Unix 时间戳，SQL 侧对应 BIGINT。
            Self::Timestamp => "bigint".to_string(),
            Self::Decimal => "numeric".to_string(),
            Self::Blob => "bytea".to_string(),
            Self::Text => "text".to_string(),
            Self::Inet => "inet".to_string(),
            Self::Cidr => "cidr".to_string(),
            Self::MacAddr => "macaddr".to_string(),
            Self::MacAddr8 => "macaddr8".to_string(),
            Self::Bit => "bit".to_string(),
            Self::Varbit => "varbit".to_string(),
            Self::Range(inner) => range_type_name(inner, false)?.to_string(),
            Self::Multirange(inner) => range_type_name(inner, true)?.to_string(),
        })
    }
}

/// 元素类型 → PostgreSQL 内置范围/多范围类型名。
///
/// 只映射有确定范围类型的内置标量；其余（文本、JSON、数组、嵌套范围等）返回
/// `None`，避免把未知类型静默猜成某个范围。
fn range_type_name(element: &FieldType, multirange: bool) -> Option<&'static str> {
    Some(match (element, multirange) {
        (FieldType::Standard, false) => "int4range",
        (FieldType::Standard, true) => "int4multirange",
        (FieldType::Timestamp, false) => "int8range",
        (FieldType::Timestamp, true) => "int8multirange",
        (FieldType::Decimal, false) => "numrange",
        (FieldType::Decimal, true) => "nummultirange",
        (FieldType::DateTime, false) => "tsrange",
        (FieldType::DateTime, true) => "tsmultirange",
        (FieldType::TimestampTz, false) => "tstzrange",
        (FieldType::TimestampTz, true) => "tstzmultirange",
        (FieldType::Date, false) => "daterange",
        (FieldType::Date, true) => "datemultirange",
        _ => return None,
    })
}

/// PostgreSQL 会话 GUC 基线。
///
/// 本常量只声明基线值，基线由**连接建立方**（[`crate::postgres::Database`] 的
/// `after_connect` 钩子）应用，本 crate 不在每次查询时重复 `SET`。基线目标是让
/// 文本格式与 Rust 侧编解码保持确定：
///
/// - `TimeZone=UTC`：`TIMESTAMPTZ` 文本输出与 `now()` 展示统一为 UTC；
/// - `DateStyle=ISO, YMD`：日期文本格式与 `chrono` 的 ISO 解析一致，且与建连时
///   实际下发的 `SET DateStyle = 'ISO, YMD'` 保持一致；
/// - `IntervalStyle=postgres`：`INTERVAL` 文本格式固定；
/// - `extra_float_digits=3`：浮点文本往返不丢精度（PostgreSQL 12+ 默认值 1
///   已够用，这里显式固定以覆盖旧版本实例）；
/// - `standard_conforming_strings=on`：反斜杠按字面处理，避免字符串转义歧义。
pub const SESSION_GUC_BASELINE: &[(&str, &str)] = &[
    ("TimeZone", "UTC"),
    ("DateStyle", "ISO, YMD"),
    ("IntervalStyle", "postgres"),
    ("extra_float_digits", "3"),
    ("standard_conforming_strings", "on"),
];

/// JOIN 类型
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum JoinType {
    Inner,
    Left,
    Right,
}

/// JOIN 子句
#[derive(Debug, Clone)]
pub struct JoinClause {
    /// JOIN 类型
    pub join_type: JoinType,
    /// 连接的表名（已校验的标识符原文，未加引号；渲染期经 `quote_identifier` 校验+转义）
    pub table: String,
    /// ON 条件
    pub on: String,
}

/// ORDER BY 子句
#[derive(Debug, Clone)]
pub struct OrderClause {
    /// 字段名
    pub field: String,
    /// 是否升序
    pub asc: bool,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn postgres_type_name_maps_existing_and_new_types() {
        let cases = [
            (FieldType::Bool, "boolean"),
            (FieldType::Json, "json"),
            (FieldType::Jsonb, "jsonb"),
            (FieldType::DateTime, "timestamp"),
            (FieldType::TimestampTz, "timestamptz"),
            (FieldType::Date, "date"),
            (FieldType::Time, "time"),
            (FieldType::Uuid, "uuid"),
            (FieldType::Timestamp, "bigint"),
            (FieldType::Decimal, "numeric"),
            (FieldType::Blob, "bytea"),
            (FieldType::Text, "text"),
            (FieldType::Inet, "inet"),
            (FieldType::Cidr, "cidr"),
            (FieldType::MacAddr, "macaddr"),
            (FieldType::MacAddr8, "macaddr8"),
            (FieldType::Bit, "bit"),
            (FieldType::Varbit, "varbit"),
        ];
        for (field_type, expected) in cases {
            assert_eq!(
                field_type.postgres_type_name().as_deref(),
                Some(expected),
                "{field_type:?} 应映射到 PostgreSQL 类型名 {expected}"
            );
        }
    }

    #[test]
    fn postgres_type_name_is_recursive_for_arrays() {
        assert_eq!(
            FieldType::Array(Box::new(FieldType::Text))
                .postgres_type_name()
                .as_deref(),
            Some("text[]")
        );
        assert_eq!(
            FieldType::Array(Box::new(FieldType::Inet))
                .postgres_type_name()
                .as_deref(),
            Some("inet[]")
        );
        assert_eq!(
            FieldType::Array(Box::new(FieldType::Blob))
                .postgres_type_name()
                .as_deref(),
            Some("bytea[]")
        );
        // 元素类型无明确映射时，数组也没有可命名的 PostgreSQL 类型。
        assert_eq!(
            FieldType::Array(Box::new(FieldType::Standard)).postgres_type_name(),
            None
        );
        assert_eq!(FieldType::Standard.postgres_type_name(), None);
    }

    #[test]
    fn postgres_type_name_maps_builtin_range_and_multirange_types() {
        let ranges = [
            (FieldType::Standard, "int4range", "int4multirange"),
            (FieldType::Timestamp, "int8range", "int8multirange"),
            (FieldType::Decimal, "numrange", "nummultirange"),
            (FieldType::DateTime, "tsrange", "tsmultirange"),
            (FieldType::TimestampTz, "tstzrange", "tstzmultirange"),
            (FieldType::Date, "daterange", "datemultirange"),
        ];
        for (element, range, multirange) in ranges {
            assert_eq!(
                FieldType::Range(Box::new(element.clone()))
                    .postgres_type_name()
                    .as_deref(),
                Some(range),
                "Range({element:?}) 应映射到 {range}"
            );
            assert_eq!(
                FieldType::Multirange(Box::new(element))
                    .postgres_type_name()
                    .as_deref(),
                Some(multirange)
            );
        }

        // 没有内置范围类型的元素必须显式返回 None，而不是猜测。
        for element in [FieldType::Text, FieldType::Json, FieldType::Uuid] {
            assert_eq!(
                FieldType::Range(Box::new(element.clone())).postgres_type_name(),
                None
            );
            assert_eq!(
                FieldType::Multirange(Box::new(element)).postgres_type_name(),
                None
            );
        }
    }

    #[test]
    fn session_guc_baseline_pins_deterministic_formats() {
        assert_eq!(
            SESSION_GUC_BASELINE,
            [
                ("TimeZone", "UTC"),
                ("DateStyle", "ISO, YMD"),
                ("IntervalStyle", "postgres"),
                ("extra_float_digits", "3"),
                ("standard_conforming_strings", "on"),
            ]
            .as_slice(),
            "会话 GUC 基线是类型编解码确定性的契约，变更需同步文档与集成测试"
        );
    }

    /// 基线的 DateStyle 必须与建连时实际下发的 `SET DateStyle` 一致（ISO, YMD）。
    #[test]
    fn session_guc_baseline_datestyle_matches_set_statement() {
        let datestyle = SESSION_GUC_BASELINE
            .iter()
            .find(|(name, _)| *name == "DateStyle")
            .map(|(_, value)| *value)
            .expect("基线必须包含 DateStyle");
        assert_eq!(datestyle, "ISO, YMD");
    }
}
