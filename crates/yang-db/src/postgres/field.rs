/// 字段类型标记（PostgreSQL）
///
/// 与 MySQL 后端保持一致的字段类型枚举。`Blob` 在 PostgreSQL 中对应 `BYTEA`，
/// `Json` 对应 `JSON`/`JSONB`，其余沿用通用语义。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum FieldType {
    /// 标准类型（无需特殊处理）
    Standard,
    /// JSON / JSONB 类型
    Json,
    /// TIMESTAMP / TIMESTAMPTZ 类型
    DateTime,
    /// 以整数存储的时间戳
    Timestamp,
    /// NUMERIC / DECIMAL 类型
    Decimal,
    /// BYTEA 类型
    Blob,
    /// TEXT 类型
    Text,
}

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
    /// 连接的表名
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
